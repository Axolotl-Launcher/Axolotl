//! Disk admission and local pressure measurements for download scheduling.

use crate::util::download::constants::{
    LOCAL_CPU_PRESSURE_PERCENT, LOCAL_FALLBACK_WRITE_CONCURRENCY,
    LOCAL_PRESSURE_SAMPLE_TTL, LOCAL_WRITE_EWMA_ALPHA, LOCAL_WRITE_PRESSURE_MS,
};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};
use std::time::Instant;
use tokio::sync::Semaphore;

use super::constants::{
    HDD_RANGE_CONCURRENCY, PRESSURE_RANGE_CONCURRENCY, SSD_RANGE_CONCURRENCY,
    UNKNOWN_DISK_RANGE_CONCURRENCY,
};

static FALLBACK_WRITES: LazyLock<Arc<Semaphore>> = LazyLock::new(|| {
    Arc::new(Semaphore::new(LOCAL_FALLBACK_WRITE_CONCURRENCY))
});
static WRITE_SAMPLES: LazyLock<Mutex<HashMap<PathBuf, WriteSample>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static CPU_PERCENT: std::sync::atomic::AtomicU32 =
    std::sync::atomic::AtomicU32::new(0);
static DISKS: std::sync::OnceLock<Vec<(PathBuf, sysinfo::DiskKind)>> =
    std::sync::OnceLock::new();
static DISKS_READY: tokio::sync::OnceCell<()> =
    tokio::sync::OnceCell::const_new();

async fn initialize_disks() {
    DISKS_READY
        .get_or_init(|| async {
            let disks = tokio::task::spawn_blocking(|| {
                sysinfo::Disks::new_with_refreshed_list()
                    .iter()
                    .map(|disk| (disk.mount_point().to_path_buf(), disk.kind()))
                    .collect()
            })
            .await
            .unwrap_or_default();
            let _ = DISKS.set(disks);
        })
        .await;
}

#[derive(Clone, Copy, Default)]
struct WriteSample {
    latency_ms: f64,
    updated: Option<Instant>,
}

impl WriteSample {
    fn observe(&mut self, queue_ms: f64, write_ms: f64) {
        let pressure_ms = queue_ms + write_ms;
        self.latency_ms = if self.updated.is_some() {
            self.latency_ms * (1.0 - LOCAL_WRITE_EWMA_ALPHA)
                + pressure_ms * LOCAL_WRITE_EWMA_ALPHA
        } else {
            pressure_ms
        };
        self.updated = Some(Instant::now());
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Pressure {
    pub(crate) cpu_percent: u32,
    pub(crate) write_latency_ms: f64,
}

impl Pressure {
    pub(crate) fn blocks_expansion(self) -> bool {
        self.cpu_percent >= LOCAL_CPU_PRESSURE_PERCENT
            || self.write_latency_ms >= LOCAL_WRITE_PRESSURE_MS
    }
}

fn volume(path: &Path) -> PathBuf {
    volume_for_mounts(path, DISKS.get().map_or(&[], Vec::as_slice))
}

fn volume_for_mounts(
    path: &Path,
    disks: &[(PathBuf, sysinfo::DiskKind)],
) -> PathBuf {
    disks
        .iter()
        .filter(|(mount, _)| path.starts_with(mount))
        .max_by_key(|(mount, _)| mount.as_os_str().len())
        .map(|(mount, _)| mount.clone())
        .unwrap_or_else(|| {
            path.components()
                .take(if cfg!(windows) { 2 } else { 1 })
                .collect()
        })
}

pub(crate) fn pressure(path: &Path) -> Pressure {
    let samples = WRITE_SAMPLES.lock();
    Pressure {
        cpu_percent: CPU_PERCENT.load(std::sync::atomic::Ordering::Relaxed),
        write_latency_ms: samples
            .get(&volume(path))
            .filter(|sample| {
                sample
                    .updated
                    .is_some_and(|at| at.elapsed() < LOCAL_PRESSURE_SAMPLE_TTL)
            })
            .map_or(0.0, |sample| sample.latency_ms),
    }
}

pub(crate) fn network_pressure() -> Pressure {
    Pressure {
        cpu_percent: CPU_PERCENT.load(std::sync::atomic::Ordering::Relaxed),
        write_latency_ms: WRITE_SAMPLES
            .lock()
            .values()
            .filter(|sample| {
                sample
                    .updated
                    .is_some_and(|at| at.elapsed() < LOCAL_PRESSURE_SAMPLE_TTL)
            })
            .map(|sample| sample.latency_ms)
            .fold(0.0, f64::max),
    }
}

pub(crate) async fn sample_cpu() {
    let _ = tokio::task::spawn_blocking(|| {
        static SYSTEM: LazyLock<Mutex<sysinfo::System>> =
            LazyLock::new(|| Mutex::new(sysinfo::System::new()));
        let mut system = SYSTEM.lock();
        system.refresh_cpu_usage();
        CPU_PERCENT.store(
            system.global_cpu_usage().clamp(0.0, 100.0) as u32,
            std::sync::atomic::Ordering::Relaxed,
        );
    })
    .await;
}

pub(crate) fn disk_range_limit(kind: sysinfo::DiskKind) -> usize {
    match kind {
        sysinfo::DiskKind::HDD => HDD_RANGE_CONCURRENCY,
        sysinfo::DiskKind::SSD => SSD_RANGE_CONCURRENCY,
        sysinfo::DiskKind::Unknown(_) => UNKNOWN_DISK_RANGE_CONCURRENCY,
    }
}

pub(crate) async fn range_limit(path: &Path) -> usize {
    initialize_disks().await;
    let limit = DISKS
        .get()
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .filter(|(mount, _)| path.starts_with(mount))
        .max_by_key(|(mount, _)| mount.as_os_str().len())
        .map_or(UNKNOWN_DISK_RANGE_CONCURRENCY, |(_, kind)| {
            disk_range_limit(*kind)
        });
    if pressure(path).blocks_expansion() {
        limit.min(PRESSURE_RANGE_CONCURRENCY)
    } else {
        limit
    }
}

pub(crate) async fn write<F, T>(
    path: &Path,
    bytes: u64,
    operation: F,
) -> std::io::Result<T>
where
    F: Future<Output = std::io::Result<T>>,
{
    let state = crate::State::get_if_initialized();
    let semaphore = state
        .as_ref()
        .map(|state| &state.io_semaphore.0)
        .unwrap_or(&FALLBACK_WRITES);
    write_with_budget(path, bytes, semaphore, operation).await
}

async fn write_with_budget<F, T>(
    path: &Path,
    bytes: u64,
    semaphore: &Semaphore,
    operation: F,
) -> std::io::Result<T>
where
    F: Future<Output = std::io::Result<T>>,
{
    initialize_disks().await;
    let waiting = Instant::now();
    let _permit = semaphore.acquire().await.map_err(std::io::Error::other)?;
    let queue_ms = waiting.elapsed().as_secs_f64() * 1000.0;
    let started = Instant::now();
    let result = operation.await;
    let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
    let mut samples = WRITE_SAMPLES.lock();
    if samples.len() > 64 {
        samples.retain(|_, sample| {
            sample
                .updated
                .is_some_and(|at| at.elapsed() < LOCAL_PRESSURE_SAMPLE_TTL)
        });
    }
    let sample = samples.entry(volume(path)).or_default();
    sample.observe(queue_ms, elapsed_ms);
    drop(samples);
    tracing::trace!(path = %path.display(), bytes, queue_ms, write_ms = elapsed_ms, "Download disk operation");
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn queue_wait_alone_can_block_network_expansion() {
        let mut sample = WriteSample::default();
        sample.observe(80.0, 0.1);
        assert!(
            Pressure {
                cpu_percent: 0,
                write_latency_ms: sample.latency_ms
            }
            .blocks_expansion()
        );
        let mut sample = WriteSample::default();
        sample.observe(0.1, 1.0);
        assert!(
            !Pressure {
                cpu_percent: 0,
                write_latency_ms: sample.latency_ms
            }
            .blocks_expansion()
        );
    }

    #[test]
    fn pressure_keys_use_the_longest_matching_mount() {
        let mounts = vec![
            (PathBuf::from("/"), sysinfo::DiskKind::Unknown(0)),
            (PathBuf::from("/mnt/hdd"), sysinfo::DiskKind::HDD),
            (PathBuf::from("/mnt/ssd"), sysinfo::DiskKind::SSD),
        ];
        let hdd = volume_for_mounts(Path::new("/mnt/hdd/mod.jar"), &mounts);
        let ssd = volume_for_mounts(Path::new("/mnt/ssd/mod.jar"), &mounts);
        assert_ne!(hdd, ssd);
        assert_eq!(hdd, PathBuf::from("/mnt/hdd"));
        assert_eq!(ssd, PathBuf::from("/mnt/ssd"));
        assert_eq!(
            volume_for_mounts(Path::new("/mnt/ssd-other/file"), &mounts),
            PathBuf::from("/")
        );
    }

    #[test]
    fn disk_and_local_pressure_limit_range_growth() {
        assert_eq!(disk_range_limit(sysinfo::DiskKind::HDD), 1);
        assert_eq!(disk_range_limit(sysinfo::DiskKind::SSD), 8);
        assert!(
            Pressure {
                cpu_percent: LOCAL_CPU_PRESSURE_PERCENT,
                write_latency_ms: 0.0
            }
            .blocks_expansion()
        );
        assert!(
            Pressure {
                cpu_percent: 20,
                write_latency_ms: 51.0
            }
            .blocks_expansion()
        );
        assert!(!Pressure::default().blocks_expansion());
        assert!(
            !Pressure {
                cpu_percent: LOCAL_CPU_PRESSURE_PERCENT - 1,
                write_latency_ms: 0.0
            }
            .blocks_expansion()
        );
    }

    #[tokio::test]
    async fn writes_obey_budget_and_release_permits_after_errors() {
        let budget = Semaphore::new(2);
        let active = AtomicUsize::new(0);
        let peak = AtomicUsize::new(0);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("write-test");
        let results = futures::future::join_all((0..8).map(|_| {
            write_with_budget(&path, 1, &budget, async {
                let count = active.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(count, Ordering::SeqCst);
                tokio::task::yield_now().await;
                active.fetch_sub(1, Ordering::SeqCst);
                Err::<(), _>(std::io::Error::other("injected write failure"))
            })
        }))
        .await;
        assert!(results.iter().all(Result::is_err));
        assert_eq!(peak.load(Ordering::SeqCst), 2);
        assert_eq!(budget.available_permits(), 2);
    }
}

//! Persisted route reputation for the native download engine.

use crate::util::download::constants::{
    NATIVE_REPUTATION_DISABLE_FAILURES, NATIVE_REPUTATION_DISABLE_SECS,
    NATIVE_REPUTATION_EWMA_ALPHA, NATIVE_REPUTATION_TMP,
};
use crate::util::fetch::ProxyPolicy;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{LazyLock, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::util::download::constants::{
    NATIVE_REPUTATION_FILE_NAME as FILE_NAME,
    NATIVE_REPUTATION_MAX_AGE_SECS as MAX_AGE_SECS,
    NATIVE_REPUTATION_MAX_ENTRIES as MAX_ENTRIES,
    NATIVE_REPUTATION_SCHEMA_VERSION as SCHEMA_VERSION,
    NATIVE_REPUTATION_WRITE_DELAY as WRITE_DELAY,
};
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum NativeTransport {
    H2Single,
    H2MultiRange,
    Http1MultiRange,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ReputationKey {
    family: String,
    authority: String,
    proxy: ProxyPolicy,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub(crate) struct NativeRouteReputation {
    pub(crate) success_samples: u32,
    pub(crate) failure_samples: u32,
    #[serde(default)]
    pub(crate) consecutive_failures: u32,
    pub(crate) ttfb_ms: Option<f64>,
    pub(crate) throughput_bps: Option<f64>,
    updated_at: u64,
}

#[derive(Serialize, Deserialize)]
struct PersistedStore {
    version: u32,
    routes: Vec<PersistedRoute>,
    #[serde(default)]
    transports: Vec<PersistedTransport>,
}

#[derive(Serialize, Deserialize)]
struct PersistedRoute {
    family: String,
    authority: String,
    proxy: ProxyPolicy,
    #[serde(flatten)]
    health: NativeRouteReputation,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub(crate) struct NativeTransportReputation {
    pub(crate) success_samples: u32,
    pub(crate) throughput_bps: Option<f64>,
    #[serde(default)]
    pub(crate) failure_samples: u32,
    #[serde(default)]
    pub(crate) consecutive_failures: u32,
    #[serde(default)]
    disabled_until: u64,
    updated_at: u64,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct TransportKey {
    authority: String,
    proxy: ProxyPolicy,
    transport: NativeTransport,
}

#[derive(Serialize, Deserialize)]
struct PersistedTransport {
    authority: String,
    proxy: ProxyPolicy,
    transport: NativeTransport,
    #[serde(flatten)]
    health: NativeTransportReputation,
}

static REPUTATION: LazyLock<
    Mutex<HashMap<ReputationKey, NativeRouteReputation>>,
> = LazyLock::new(|| Mutex::new(HashMap::new()));
static TRANSPORT_REPUTATION: LazyLock<
    Mutex<HashMap<TransportKey, NativeTransportReputation>>,
> = LazyLock::new(|| Mutex::new(HashMap::new()));
static LOAD: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();
static PATH: OnceLock<PathBuf> = OnceLock::new();
static GENERATION: AtomicU64 = AtomicU64::new(0);
static WRITE_SCHEDULED: AtomicBool = AtomicBool::new(false);

pub(crate) async fn load_if_needed() {
    LOAD.get_or_init(|| async {
		let Some(path) = reputation_path() else {
			return;
		};
		let _ = PATH.set(path.clone());
		let Ok(contents) = tokio::fs::read(&path).await else {
			return;
		};
		let Ok(store) = serde_json::from_slice::<PersistedStore>(&contents) else {
			tracing::warn!(path = %path.display(), "Ignoring invalid native download reputation");
			return;
		};
		if store.version != SCHEMA_VERSION {
			return;
		}
		let cutoff = now_secs().saturating_sub(MAX_AGE_SECS);
		let mut reputation = REPUTATION.lock();
		for route in store.routes.into_iter().take(MAX_ENTRIES) {
			if route.health.updated_at < cutoff {
				continue;
			}
			reputation.insert(
				ReputationKey {
					family: route.family,
					authority: route.authority,
					proxy: route.proxy,
				},
				route.health,
			);
		}
		drop(reputation);
		let mut transports = TRANSPORT_REPUTATION.lock();
		for transport in store.transports.into_iter().take(MAX_ENTRIES) {
			if transport.health.updated_at < cutoff {
				continue;
			}
			transports.insert(
				TransportKey {
					authority: transport.authority,
					proxy: transport.proxy,
					transport: transport.transport,
				},
				transport.health,
			);
		}
	})
	.await;
}

pub(crate) fn get_transport(
    authority: &str,
    proxy: ProxyPolicy,
    transport: NativeTransport,
) -> Option<NativeTransportReputation> {
    TRANSPORT_REPUTATION
        .lock()
        .get(&TransportKey {
            authority: super::proxy_context::authority_key(authority, proxy),
            proxy,
            transport,
        })
        .copied()
}

pub(crate) fn record_transport_success(
    authority: &str,
    proxy: ProxyPolicy,
    transport: NativeTransport,
    throughput_bps: f64,
) {
    if !throughput_bps.is_finite() || throughput_bps <= 0.0 {
        return;
    }
    let mut reputation = TRANSPORT_REPUTATION.lock();
    let entry = reputation
        .entry(TransportKey {
            authority: super::proxy_context::authority_key(authority, proxy),
            proxy,
            transport,
        })
        .or_default();
    entry.success_samples = entry.success_samples.saturating_add(1);
    entry.consecutive_failures = 0;
    entry.disabled_until = 0;
    entry.throughput_bps =
        Some(update_ewma(entry.throughput_bps, throughput_bps));
    entry.updated_at = now_secs();
    drop(reputation);
    schedule_write();
}

pub(crate) fn transport_enabled(
    authority: &str,
    proxy: ProxyPolicy,
    transport: NativeTransport,
) -> bool {
    get_transport(authority, proxy, transport)
        .is_none_or(|health| health.disabled_until <= now_secs())
}

pub(crate) fn record_transport_failure(
    authority: &str,
    proxy: ProxyPolicy,
    transport: NativeTransport,
) {
    let mut reputation = TRANSPORT_REPUTATION.lock();
    let entry = reputation
        .entry(TransportKey {
            authority: super::proxy_context::authority_key(authority, proxy),
            proxy,
            transport,
        })
        .or_default();
    entry.failure_samples = entry.failure_samples.saturating_add(1);
    entry.consecutive_failures = entry.consecutive_failures.saturating_add(1);
    entry.updated_at = now_secs();
    if entry.consecutive_failures >= NATIVE_REPUTATION_DISABLE_FAILURES {
        entry.disabled_until =
            now_secs().saturating_add(NATIVE_REPUTATION_DISABLE_SECS);
    }
    drop(reputation);
    schedule_write();
}

pub(crate) fn get(
    family: &str,
    authority: &str,
    proxy: ProxyPolicy,
) -> Option<NativeRouteReputation> {
    REPUTATION
        .lock()
        .get(&ReputationKey {
            family: family.to_string(),
            authority: super::proxy_context::authority_key(authority, proxy),
            proxy,
        })
        .copied()
}

pub(crate) fn record_success(
    family: &str,
    authority: &str,
    proxy: ProxyPolicy,
    ttfb_ms: f64,
    throughput_bps: Option<f64>,
) {
    let mut reputation = REPUTATION.lock();
    let entry = reputation
        .entry(ReputationKey {
            family: family.to_string(),
            authority: super::proxy_context::authority_key(authority, proxy),
            proxy,
        })
        .or_default();
    entry.success_samples = entry.success_samples.saturating_add(1);
    entry.consecutive_failures = 0;
    entry.ttfb_ms = Some(update_ewma(entry.ttfb_ms, ttfb_ms));
    if let Some(throughput_bps) = throughput_bps {
        entry.throughput_bps =
            Some(update_ewma(entry.throughput_bps, throughput_bps));
    }
    entry.updated_at = now_secs();
    drop(reputation);
    schedule_write();
}

pub(crate) fn record_transfer_success(
    family: &str,
    authority: &str,
    proxy: ProxyPolicy,
    throughput_bps: f64,
) {
    if !throughput_bps.is_finite() || throughput_bps <= 0.0 {
        return;
    }
    let mut reputation = REPUTATION.lock();
    let entry = reputation
        .entry(ReputationKey {
            family: family.to_string(),
            authority: super::proxy_context::authority_key(authority, proxy),
            proxy,
        })
        .or_default();
    entry.success_samples = entry.success_samples.saturating_add(1);
    entry.consecutive_failures = 0;
    entry.throughput_bps =
        Some(update_ewma(entry.throughput_bps, throughput_bps));
    entry.updated_at = now_secs();
    drop(reputation);
    schedule_write();
}

pub(crate) fn record_failure(
    family: &str,
    authority: &str,
    proxy: ProxyPolicy,
) {
    let mut reputation = REPUTATION.lock();
    let entry = reputation
        .entry(ReputationKey {
            family: family.to_string(),
            authority: super::proxy_context::authority_key(authority, proxy),
            proxy,
        })
        .or_default();
    entry.failure_samples = entry.failure_samples.saturating_add(1);
    entry.consecutive_failures = entry.consecutive_failures.saturating_add(1);
    entry.updated_at = now_secs();
    drop(reputation);
    schedule_write();
}

fn update_ewma(current: Option<f64>, sample: f64) -> f64 {
    current.map_or(sample, |current| {
        current * (1.0 - NATIVE_REPUTATION_EWMA_ALPHA)
            + sample * NATIVE_REPUTATION_EWMA_ALPHA
    })
}

fn schedule_write() {
    GENERATION.fetch_add(1, Ordering::AcqRel);
    let Ok(runtime) = tokio::runtime::Handle::try_current() else {
        return;
    };
    if WRITE_SCHEDULED.swap(true, Ordering::AcqRel) {
        return;
    }
    runtime.spawn(async {
		loop {
			let generation = GENERATION.load(Ordering::Acquire);
			tokio::time::sleep(WRITE_DELAY).await;
			if let Err(error) = write_snapshot().await {
				tracing::warn!(%error, "Failed to persist native download reputation");
			}
			if GENERATION.load(Ordering::Acquire) == generation {
				WRITE_SCHEDULED.store(false, Ordering::Release);
				if GENERATION.load(Ordering::Acquire) == generation
					|| WRITE_SCHEDULED.swap(true, Ordering::AcqRel)
				{
					break;
				}
			}
		}
	});
}

async fn write_snapshot() -> std::io::Result<()> {
    let Some(path) = PATH.get().cloned().or_else(reputation_path) else {
        return Ok(());
    };
    let mut routes = REPUTATION
        .lock()
        .iter()
        .map(|(key, health)| PersistedRoute {
            family: key.family.clone(),
            authority: key.authority.clone(),
            proxy: key.proxy,
            health: *health,
        })
        .collect::<Vec<_>>();
    routes.sort_unstable_by_key(|route| {
        std::cmp::Reverse(route.health.updated_at)
    });
    routes.truncate(MAX_ENTRIES);
    let mut transports = TRANSPORT_REPUTATION
        .lock()
        .iter()
        .map(|(key, health)| PersistedTransport {
            authority: key.authority.clone(),
            proxy: key.proxy,
            transport: key.transport,
            health: *health,
        })
        .collect::<Vec<_>>();
    transports.sort_unstable_by_key(|transport| {
        std::cmp::Reverse(transport.health.updated_at)
    });
    transports.truncate(MAX_ENTRIES);
    let bytes = serde_json::to_vec(&PersistedStore {
        version: SCHEMA_VERSION,
        routes,
        transports,
    })
    .map_err(std::io::Error::other)?;
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let temporary = temporary_path(&path);
    tokio::fs::write(&temporary, bytes).await?;
    if let Err(error) = tokio::fs::rename(&temporary, &path).await {
        if error.kind() != std::io::ErrorKind::AlreadyExists {
            let _ = tokio::fs::remove_file(&temporary).await;
            return Err(error);
        }
        tokio::fs::remove_file(&path).await?;
        tokio::fs::rename(&temporary, &path).await?;
    }
    Ok(())
}

fn reputation_path() -> Option<PathBuf> {
    crate::State::get_if_initialized()
        .map(|state| state.directories.settings_dir.join(FILE_NAME))
}

fn temporary_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(NATIVE_REPUTATION_TMP);
    path.with_file_name(name)
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_failures_disable_only_range_transport_until_success() {
        let authority = "range-failure-memory.invalid:443";
        for _ in 0..3 {
            record_transport_failure(
                authority,
                ProxyPolicy::Direct,
                NativeTransport::H2MultiRange,
            );
        }
        assert!(!transport_enabled(
            authority,
            ProxyPolicy::Direct,
            NativeTransport::H2MultiRange
        ));
        assert!(transport_enabled(
            authority,
            ProxyPolicy::Direct,
            NativeTransport::H2Single
        ));
        record_transport_success(
            authority,
            ProxyPolicy::Direct,
            NativeTransport::H2MultiRange,
            1024.0,
        );
        assert!(transport_enabled(
            authority,
            ProxyPolicy::Direct,
            NativeTransport::H2MultiRange
        ));
    }

    #[test]
    fn transport_reputation_is_independent() {
        record_transport_success(
            "transport-reputation.example:443",
            ProxyPolicy::Direct,
            NativeTransport::H2Single,
            1024.0,
        );
        assert!(
            get_transport(
                "transport-reputation.example:443",
                ProxyPolicy::Direct,
                NativeTransport::H2Single,
            )
            .is_some()
        );
        assert!(
            get_transport(
                "transport-reputation.example:443",
                ProxyPolicy::Direct,
                NativeTransport::Http1MultiRange,
            )
            .is_none()
        );
        assert!(
            get_transport(
                "transport-reputation.example:443",
                ProxyPolicy::Direct,
                NativeTransport::H2MultiRange,
            )
            .is_none()
        );
        record_transport_success(
            "transport-reputation.example:443",
            ProxyPolicy::Direct,
            NativeTransport::H2MultiRange,
            4096.0,
        );
        assert_eq!(
            get_transport(
                "transport-reputation.example:443",
                ProxyPolicy::Direct,
                NativeTransport::H2Single,
            )
            .unwrap()
            .throughput_bps,
            Some(1024.0),
        );
    }
}

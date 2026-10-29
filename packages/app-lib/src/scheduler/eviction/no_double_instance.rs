//! "Abort, then rebuild at once" must never leave two instances alive: the
//! rebuild waits on the rebuild lock until the old instance's tasks have
//! drained to zero. Counted with a live-instance gauge.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use tokio::task::JoinSet;

use super::{Built, EvictionSlot, RebuildOutcome};

#[derive(Default)]
struct Gauge {
    live: AtomicUsize,
    peak: AtomicUsize,
    built: AtomicUsize,
}

/// Held by an instance's task; counts the instance live until dropped.
struct Alive(Arc<Gauge>);

impl Alive {
    fn new(gauge: &Arc<Gauge>) -> Self {
        let live = gauge.live.fetch_add(1, Ordering::SeqCst) + 1;
        gauge.peak.fetch_max(live, Ordering::SeqCst);
        Self(Arc::clone(gauge))
    }
}

impl Drop for Alive {
    fn drop(&mut self) {
        self.0.live.fetch_sub(1, Ordering::SeqCst);
    }
}

fn instance(gauge: &Arc<Gauge>) -> Built<()> {
    gauge.built.fetch_add(1, Ordering::SeqCst);
    let alive = Alive::new(gauge);
    let mut tasks = JoinSet::new();
    tasks.spawn(async move {
        let _alive = alive;
        std::future::pending::<()>().await;
    });
    Built { service: (), tasks }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn abort_then_immediate_rebuild_never_runs_two_instances() {
    let gauge = Arc::new(Gauge::default());
    let slot = Arc::new(EvictionSlot::live(instance(&gauge)));
    for _ in 0..50 {
        let evict = {
            let slot = Arc::clone(&slot);
            tokio::spawn(async move { slot.evict().await })
        };
        let rebuild = {
            let slot = Arc::clone(&slot);
            let gauge = Arc::clone(&gauge);
            tokio::spawn(async move {
                slot.rebuild(Instant::now(), || async move {
                    Ok::<_, String>(instance(&gauge))
                })
                .await
            })
        };
        let _ = evict.await.unwrap();
        let outcome = rebuild.await.unwrap();
        assert!(matches!(
            outcome,
            RebuildOutcome::Live | RebuildOutcome::AlreadyLive
        ));
        if slot.state().await != super::SlotState::Live {
            slot.rebuild(Instant::now(), || async {
                Ok::<_, String>(instance(&gauge))
            })
            .await;
        }
    }
    assert_eq!(
        gauge.peak.load(Ordering::SeqCst),
        1,
        "two instances ran at once"
    );
    assert_eq!(gauge.live.load(Ordering::SeqCst), 1);
    assert!(
        gauge.built.load(Ordering::SeqCst) > 1,
        "rebuilds did happen"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_rebuilds_build_one_instance() {
    let gauge = Arc::new(Gauge::default());
    let slot = Arc::new(EvictionSlot::live(instance(&gauge)));
    slot.evict().await;
    let rebuilds: Vec<_> = (0..8)
        .map(|_| {
            let slot = Arc::clone(&slot);
            let gauge = Arc::clone(&gauge);
            tokio::spawn(async move {
                slot.rebuild(Instant::now(), || async move {
                    Ok::<_, String>(instance(&gauge))
                })
                .await
            })
        })
        .collect();
    let mut live = 0;
    for rebuild in rebuilds {
        if rebuild.await.unwrap() == RebuildOutcome::Live {
            live += 1;
        }
    }
    assert_eq!(live, 1);
    assert_eq!(
        gauge.built.load(Ordering::SeqCst),
        2,
        "the first and one rebuild"
    );
    assert_eq!(gauge.peak.load(Ordering::SeqCst), 1);
}

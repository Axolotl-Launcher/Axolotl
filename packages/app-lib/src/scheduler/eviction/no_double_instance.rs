//! "Abort, then rebuild at once" must never leave two instances alive: the
//! rebuild waits on the rebuild lock until the old instance's tasks have
//! drained to zero. Counted with a live-instance gauge.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use tokio::task::JoinSet;

use super::{Built, RebuildOutcome, SlotRegistry};

fn eligible() -> super::criteria::Swappable {
    super::criteria::LOG_RING_BUFFER
        .token()
        .expect("the log ring buffer is the unconditional L1 member")
}

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
    let registry = SlotRegistry::new();
    let slot = registry.slot(1, eligible(), instance(&gauge));
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
    let registry = SlotRegistry::new();
    let slot = registry.slot(1, eligible(), instance(&gauge));
    slot.evict().await;
    // Hold every entry that actually starts inside its rebuild until all the
    // callers have had their chance. Without this the spawned rebuilds could
    // run one after another and the counts would pass even if simultaneous
    // attempts were not excluded.
    let entered = Arc::new(AtomicUsize::new(0));
    let (started_tx, mut started_rx) = tokio::sync::mpsc::channel::<()>(8);
    let hold = Arc::new(tokio::sync::Notify::new());
    let rebuilds: Vec<_> = (0..8)
        .map(|_| {
            let slot = Arc::clone(&slot);
            let gauge = Arc::clone(&gauge);
            let entered = Arc::clone(&entered);
            let started_tx = started_tx.clone();
            let hold = Arc::clone(&hold);
            tokio::spawn(async move {
                slot.rebuild(Instant::now(), || async move {
                    entered.fetch_add(1, Ordering::SeqCst);
                    let _ = started_tx.send(()).await;
                    hold.notified().await;
                    Ok::<_, String>(instance(&gauge))
                })
                .await
            })
        })
        .collect();
    drop(started_tx);
    // Wait until either the first entry has started and is held, or the
    // remaining callers have returned (they must all have seen the slot busy).
    let first = started_rx.recv().await;
    assert!(first.is_some(), "one rebuild must enter its entry");
    // Give the other seven a moment to reach the slot, then release.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    hold.notify_waiters();
    let mut live = 0;
    for rebuild in rebuilds {
        if rebuild.await.unwrap() == RebuildOutcome::Live {
            live += 1;
        }
    }
    assert_eq!(
        entered.load(Ordering::SeqCst),
        1,
        "only one rebuild entry may start"
    );
    assert_eq!(live, 1);
    assert_eq!(
        gauge.built.load(Ordering::SeqCst),
        2,
        "the first and one rebuild"
    );
    assert_eq!(gauge.peak.load(Ordering::SeqCst), 1);
}

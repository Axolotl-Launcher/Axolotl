//! Quiesce, the only stop signal L0 obeys. Cancel drops work on the spot;
//! quiesce lets an L0 task finish its current unit and return at the next
//! safety point. L0 never takes a cancellation token.
//!
//! A safety point is a place where all four hold:
//! (a) the task holds no resource, (b) no state transition is half done,
//! (c) returning now is safe, and (d) the current unit is not blocked on a
//! shared resource. (a) and (d) are checked statically: the L0 module may not
//! acquire a total-order resource (`sched-l0-resource-check.mjs`). (b) and (c)
//! are the task's contract. At run time every safety point is recorded as
//! `L0_SAFETY_POINT_AT` in the control table.

use std::ops::ControlFlow;
use std::sync::Arc;
use tokio::sync::Notify;

use super::control::ControlTable;

#[derive(Clone)]
pub struct QuiesceToken {
    control: Arc<ControlTable>,
    wake: Arc<Notify>,
}

impl QuiesceToken {
    pub fn new(control: Arc<ControlTable>) -> Self {
        Self {
            control,
            wake: Arc::new(Notify::new()),
        }
    }

    pub fn request(&self) {
        self.control.request_quiesce();
        self.wake.notify_waiters();
    }

    pub fn is_requested(&self) -> bool {
        self.control.quiesce_requested()
    }

    /// Resolves once quiesce is requested, for selecting against idle waits.
    pub async fn requested(&self) {
        loop {
            let woken = self.wake.notified();
            tokio::pin!(woken);
            woken.as_mut().enable();
            if self.is_requested() {
                return;
            }
            woken.await;
        }
    }

    /// Marks a safety point. Call it only where (a)–(d) hold; it breaks when
    /// the task should return now.
    pub fn safety_point(&self) -> ControlFlow<()> {
        self.control.record_safety_point();
        if self.is_requested() {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    #[tokio::test]
    async fn quiesce_finishes_the_current_unit_then_returns() {
        let control = Arc::new(ControlTable::new());
        let token = QuiesceToken::new(control.clone());
        let started = Arc::new(AtomicUsize::new(0));
        let finished = Arc::new(AtomicUsize::new(0));
        let in_unit = Arc::new(Notify::new());
        let resume = Arc::new(Notify::new());
        let task = tokio::spawn({
            let (token, started, finished, in_unit, resume) = (
                token.clone(),
                started.clone(),
                finished.clone(),
                in_unit.clone(),
                resume.clone(),
            );
            async move {
                loop {
                    started.fetch_add(1, Ordering::SeqCst);
                    in_unit.notify_one();
                    resume.notified().await;
                    finished.fetch_add(1, Ordering::SeqCst);
                    if token.safety_point().is_break() {
                        return;
                    }
                }
            }
        });
        in_unit.notified().await;
        token.request();
        resume.notify_one();
        tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .expect("the task returns at its next safety point")
            .unwrap();
        assert_eq!(started.load(Ordering::SeqCst), 1);
        assert_eq!(finished.load(Ordering::SeqCst), 1, "the unit completed");
        assert!(
            control.safety_point_at() > 0,
            "the safety point was recorded"
        );
    }

    #[tokio::test]
    async fn an_idle_task_wakes_on_quiesce() {
        let token = QuiesceToken::new(Arc::new(ControlTable::new()));
        let waiter = tokio::spawn({
            let token = token.clone();
            async move { token.requested().await }
        });
        tokio::task::yield_now().await;
        token.request();
        tokio::time::timeout(Duration::from_secs(1), waiter)
            .await
            .expect("requested() resolves after the request")
            .unwrap();
        // Already requested: resolves at once.
        tokio::time::timeout(Duration::from_millis(100), token.requested())
            .await
            .unwrap();
    }
}

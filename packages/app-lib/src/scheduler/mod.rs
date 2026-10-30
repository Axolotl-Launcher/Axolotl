//! Foundations of the five-level task scheduler. This stage only observes:
//! leaf locking, the debug resource-order guard, the L0 supervisor skeleton and
//! the process-wide panic hook. Nothing here changes how existing tasks run.

pub mod leaf;
pub mod order;
pub mod panic;
pub mod supervisor;

#[cfg(test)]
mod budget;

#[cfg(test)]
mod panic_isolation_tests {
    use super::supervisor::{Exit, Supervisor};
    use std::time::Duration;

    #[tokio::test]
    async fn panicking_l0_task_is_recorded_and_others_keep_running() {
        let mut supervisor = Supervisor::new();
        let (finished_tx, finished_rx) = tokio::sync::oneshot::channel();
        supervisor.spawn("injected-panic", async {
            panic!("injected L0 failure");
        });
        supervisor.spawn("survivor", async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let _ = finished_tx.send(());
        });

        let mut exits = Vec::new();
        while let Some(exit) = supervisor.join_next().await {
            exits.push(exit);
        }

        assert!(exits.iter().any(|exit| matches!(
            exit,
            Exit::Panicked { name: "injected-panic", message }
                if message.contains("injected L0 failure")
        )));
        assert!(exits.contains(&Exit::Finished("survivor")));
        assert!(finished_rx.await.is_ok(), "the survivor ran to completion");
    }

    #[tokio::test]
    async fn panicking_task_is_not_restarted() {
        let mut supervisor = Supervisor::new();
        supervisor.spawn("once", async {
            panic!("only once");
        });
        assert!(matches!(
            supervisor.join_next().await,
            Some(Exit::Panicked { name: "once", .. })
        ));
        assert!(supervisor.is_empty());
        assert_eq!(supervisor.join_next().await, None);
    }
}

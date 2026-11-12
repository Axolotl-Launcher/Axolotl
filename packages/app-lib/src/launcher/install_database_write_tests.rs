use super::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::Semaphore;

#[tokio::test]
async fn minecraft_database_operation_starts_only_after_permit() {
    let semaphore = Semaphore::new(0);
    let cancellation = CancellationToken::new();
    let operation_polled = Arc::new(AtomicBool::new(false));
    let operation_flag = Arc::clone(&operation_polled);

    let result = tokio::time::timeout(
        Duration::from_millis(50),
        run_install_database_write(&semaphore, &cancellation, async move {
            operation_flag.store(true, Ordering::SeqCst);
            Ok::<(), crate::Error>(())
        }),
    )
    .await;

    assert!(result.is_err());
    assert!(!operation_polled.load(Ordering::SeqCst));
}

#[tokio::test]
async fn minecraft_database_wait_is_cancelable() {
    let semaphore = Semaphore::new(0);
    let cancellation = CancellationToken::new();
    let trigger = cancellation.clone();
    let cancel_task = tokio::spawn(async move {
        tokio::task::yield_now().await;
        trigger.cancel();
    });

    let error = tokio::time::timeout(
        Duration::from_millis(100),
        run_install_database_write(&semaphore, &cancellation, async {
            Ok::<(), crate::Error>(())
        }),
    )
    .await
    .expect("database semaphore wait should stop on cancellation")
    .unwrap_err();
    cancel_task.await.unwrap();

    assert!(error.to_string().contains("canceled"));
}

#[tokio::test]
async fn minecraft_database_operation_runs_with_permit() {
    let semaphore = Semaphore::new(1);
    let cancellation = CancellationToken::new();
    let operation_polled = Arc::new(AtomicBool::new(false));
    let operation_flag = Arc::clone(&operation_polled);

    run_install_database_write(&semaphore, &cancellation, async move {
        operation_flag.store(true, Ordering::SeqCst);
        Ok::<(), crate::Error>(())
    })
    .await
    .unwrap();

    assert!(operation_polled.load(Ordering::SeqCst));
    assert_eq!(semaphore.available_permits(), 1);
}

#[tokio::test]
async fn minecraft_database_cancellation_waits_for_started_operation() {
    let semaphore = Arc::new(Semaphore::new(1));
    let cancellation = CancellationToken::new();
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (finish_tx, finish_rx) = tokio::sync::oneshot::channel();
    let task_semaphore = Arc::clone(&semaphore);
    let task_cancellation = cancellation.clone();
    let task = tokio::spawn(async move {
        run_install_database_write(
            &task_semaphore,
            &task_cancellation,
            async move {
                let _ = started_tx.send(());
                let _ = finish_rx.await;
                Ok::<(), crate::Error>(())
            },
        )
        .await
    });

    started_rx.await.unwrap();
    cancellation.cancel();
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert!(!task.is_finished());

    finish_tx.send(()).unwrap();
    let error = task.await.unwrap().unwrap_err();
    assert!(error.to_string().contains("canceled"));
    assert_eq!(semaphore.available_permits(), 1);
}

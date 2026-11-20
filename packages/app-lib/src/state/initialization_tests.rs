use super::{Initialization, InitializationUnavailable};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

#[tokio::test(start_paused = true)]
async fn commands_wait_for_startup_while_the_initializer_can_read_state() {
    use super::{STARTUP_CONTEXT, state_access_ready};

    let initialization = Initialization::new();
    let guard = initialization.begin();
    let startup_complete = AtomicBool::new(false);
    let ready =
        || state_access_ready(true, startup_complete.load(Ordering::Acquire));
    let command = initialization.wait(ready);
    tokio::pin!(command);

    assert!(
        tokio::time::timeout(Duration::from_secs(11), &mut command)
            .await
            .is_err()
    );
    STARTUP_CONTEXT
        .scope((), async {
            assert!(ready());
            assert!(!state_access_ready(false, false));
        })
        .await;
    assert!(!ready());
    startup_complete.store(true, Ordering::Release);
    drop(guard);
    assert_eq!(command.await, Ok(()));
}

#[tokio::test(start_paused = true)]
async fn active_initialization_waits_past_ten_seconds_until_state_is_ready() {
    let initialization = Initialization::new();
    let ready = AtomicBool::new(false);
    let guard = initialization.begin();
    let wait = initialization.wait(|| ready.load(Ordering::Acquire));
    tokio::pin!(wait);
    assert!(
        tokio::time::timeout(Duration::from_secs(60), &mut wait)
            .await
            .is_err()
    );
    ready.store(true, Ordering::Release);
    drop(guard);
    assert_eq!(wait.await, Ok(()));
}

#[tokio::test(start_paused = true)]
async fn missing_initializer_still_fails_in_ten_seconds() {
    let initialization = Initialization::new();
    let started = tokio::time::Instant::now();
    assert_eq!(
        initialization.wait(|| false).await,
        Err(InitializationUnavailable::NeverStarted)
    );
    assert_eq!(started.elapsed(), Duration::from_secs(10));
}

#[tokio::test(start_paused = true)]
async fn an_initializer_started_during_the_initial_grace_period_can_run_long() {
    let initialization = Initialization::new();
    let wait = initialization.wait(|| false);
    tokio::pin!(wait);
    assert!(
        tokio::time::timeout(Duration::from_secs(5), &mut wait)
            .await
            .is_err()
    );
    let guard = initialization.begin();
    assert!(
        tokio::time::timeout(Duration::from_secs(60), &mut wait)
            .await
            .is_err()
    );
    drop(guard);
    assert_eq!(wait.await, Err(InitializationUnavailable::Stopped));
}

#[tokio::test(start_paused = true)]
async fn failed_attempt_wakes_waiters_without_an_additional_timeout() {
    let initialization = Initialization::new();
    let guard = initialization.begin();
    let wait = initialization.wait(|| false);
    tokio::pin!(wait);
    assert!(
        tokio::time::timeout(Duration::from_secs(1), &mut wait)
            .await
            .is_err()
    );
    let stopped = tokio::time::Instant::now();
    drop(guard);
    assert_eq!(wait.await, Err(InitializationUnavailable::Stopped));
    assert_eq!(stopped.elapsed(), Duration::ZERO);
}

#[tokio::test(start_paused = true)]
async fn cancellation_drops_the_initializer_guard_and_wakes_waiters() {
    let initialization = Initialization::new();
    let initializer = async {
        let _guard = initialization.begin();
        std::future::pending::<()>().await;
    };
    let mut initializer = Box::pin(initializer);
    assert!(
        tokio::time::timeout(Duration::from_secs(1), &mut initializer)
            .await
            .is_err()
    );
    let wait = initialization.wait(|| false);
    tokio::pin!(wait);
    assert!(
        tokio::time::timeout(Duration::from_secs(1), &mut wait)
            .await
            .is_err()
    );
    drop(initializer);
    assert_eq!(wait.await, Err(InitializationUnavailable::Stopped));
}

#[tokio::test(start_paused = true)]
async fn another_initializer_keeps_waiters_alive_when_one_caller_leaves() {
    let initialization = Initialization::new();
    let first = initialization.begin();
    let second = initialization.begin();
    drop(first);
    let wait = initialization.wait(|| false);
    tokio::pin!(wait);
    assert!(
        tokio::time::timeout(Duration::from_secs(60), &mut wait)
            .await
            .is_err()
    );
    drop(second);
    assert_eq!(wait.await, Err(InitializationUnavailable::Stopped));
}

#[tokio::test(start_paused = true)]
async fn retry_after_failure_can_publish_state_for_new_waiters() {
    let initialization = Initialization::new();
    drop(initialization.begin());
    assert_eq!(
        initialization.wait(|| false).await,
        Err(InitializationUnavailable::Stopped)
    );
    let retry = initialization.begin();
    let ready = AtomicBool::new(false);
    let wait = initialization.wait(|| ready.load(Ordering::Acquire));
    tokio::pin!(wait);
    assert!(
        tokio::time::timeout(Duration::from_secs(60), &mut wait)
            .await
            .is_err()
    );
    ready.store(true, Ordering::Release);
    drop(retry);
    assert_eq!(wait.await, Ok(()));
}

#[tokio::test(start_paused = true)]
async fn state_published_before_waiter_registration_is_immediately_available() {
    let initialization = Initialization::new();
    drop(initialization.begin());
    assert_eq!(initialization.wait(|| true).await, Ok(()));
}

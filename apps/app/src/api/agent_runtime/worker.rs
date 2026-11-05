use super::{state::*, types::Job};
use std::{
    sync::atomic::Ordering,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

pub async fn ensure(app: &AppHandle) -> Result<(), String> {
    let runtime = app.state::<AgentRuntime>();
    let _creation = runtime.worker_creation.lock().await;
    if app.get_webview_window(WORKER_LABEL).is_none() {
        let id = uuid::Uuid::new_v4().to_string();
        runtime.queue.lock().await.worker = Some(Worker {
            id: id.clone(),
            ready: false,
            last_seen: Instant::now(),
        });
        let close_app = app.clone();
        let close_id = id.clone();
        let dev_url = if cfg!(debug_assertions) {
            app.config().build.dev_url.clone()
        } else {
            None
        };
        let window = WebviewWindowBuilder::new(
            app,
            WORKER_LABEL,
            WebviewUrl::App("agent-runtime.html".into()),
        )
        .visible(false)
        .skip_taskbar(true)
        .focused(false)
        .decorations(false)
        .initialization_script(format!(
            "window.__AXOLOTL_AGENT_WORKER_ID__={};",
            serde_json::to_string(&id).unwrap()
        ))
        .on_navigation(move |url| {
            url.path() == "/agent-runtime.html"
                && ((url.scheme() == "tauri"
                    && url.host_str() == Some("localhost"))
                    || (matches!(url.scheme(), "http" | "https")
                        && url.host_str() == Some("tauri.localhost")
                        && url.port().is_none())
                    || dev_url
                        .as_ref()
                        .is_some_and(|dev| url.origin() == dev.origin()))
        })
        .build()
        .map_err(|e| e.to_string())?;
        window.on_window_event(move |event| {
            if matches!(event, tauri::WindowEvent::Destroyed) {
                let app = close_app.clone();
                let id = close_id.clone();
                tauri::async_runtime::spawn(async move {
                    lost(&app, &id).await;
                });
            }
        });
        let health_app = app.clone();
        tauri::async_runtime::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(10)).await;
                let stale = {
                    let runtime = health_app.state::<AgentRuntime>();
                    let queue = runtime.queue.lock().await;
                    let Some(worker) = &queue.worker else { return };
                    if worker.id != id {
                        return;
                    }
                    worker.last_seen.elapsed() > Duration::from_secs(30)
                };
                if stale {
                    lost(&health_app, &id).await;
                    if let Some(window) =
                        health_app.get_webview_window(WORKER_LABEL)
                    {
                        let _ = window.destroy();
                    }
                    return;
                }
            }
        });
    }
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let notified = runtime.wake.notified();
            if runtime.queue.lock().await.worker.as_ref().is_some_and(|worker| worker.ready) { return; }
            notified.await;
        }
    }).await.map_err(|_| "Agent runtime could not start; retry after checking launcher initialization".to_string())
}

async fn lost(app: &AppHandle, worker_id: &str) {
    let runtime = app.state::<AgentRuntime>();
    let runs = {
        let mut queue = runtime.queue.lock().await;
        if !queue
            .worker
            .as_ref()
            .is_some_and(|worker| worker.id == worker_id)
        {
            return;
        }
        queue.worker = None;
        queue.pending.clear();
        queue.runs.values().cloned().collect::<Vec<_>>()
    };
    for run in runs {
        let state = run.state.swap(TERMINAL, Ordering::AcqRel);
        run.cancel.cancel();
        run.leased.store(false, Ordering::Release);
        let from = match state {
            QUEUED => agent_window::persistence::AttemptState::Queued,
            RUNNING => agent_window::persistence::AttemptState::Running,
            AWAITING => agent_window::persistence::AttemptState::Awaiting,
            _ => continue,
        };
        if let Ok(store) = runtime.store().await {
            let _ = store
                .transition(
                    &run.request.attempt_id,
                    from,
                    agent_window::persistence::AttemptState::Interrupted,
                )
                .await;
        }
    }
    runtime.wake.notify_waiters();
}

#[derive(serde::Serialize)]
pub struct Poll {
    pub job: Option<Job>,
    pub cancelled: Vec<String>,
}

pub async fn next(
    runtime: &AgentRuntime,
    worker_id: &str,
) -> Result<Poll, String> {
    let job = runtime.claim(worker_id).await?;
    if job.is_none() {
        let _ = tokio::time::timeout(
            Duration::from_secs(1),
            runtime.wake.notified(),
        )
        .await;
    }
    let cancelled = runtime
        .queue
        .lock()
        .await
        .runs
        .iter()
        .filter(|(_, run)| run.cancel.is_cancelled())
        .map(|(id, _)| id.clone())
        .collect();
    Ok(Poll { job, cancelled })
}

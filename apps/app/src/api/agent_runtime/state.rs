use super::types::{Job, SubmitRequest};
use agent_window::persistence::{AgentStore, Attempt, AttemptState};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering},
    },
    time::Instant,
};
use tauri::{AppHandle, Manager};
use tokio::sync::{Mutex, Notify, OnceCell};
use tokio_util::sync::CancellationToken;

pub const WORKER_LABEL: &str = "agent-runtime";
pub const QUEUED: u8 = 0;
pub const RUNNING: u8 = 1;
pub const AWAITING: u8 = 2;
pub const TERMINAL: u8 = 3;

pub struct Run {
    pub request: SubmitRequest,
    pub job: Mutex<Job>,
    pub model: theseus::ai::chat::ChatModel,
    pub cancel: CancellationToken,
    pub state: AtomicU8,
    pub leased: AtomicBool,
    pub models: AtomicUsize,
    pub tools: AtomicUsize,
    pub scope_id: String,
    pub lease_id: Mutex<String>,
    pub latest_model: Mutex<serde_json::Value>,
    pub model_results:
        Mutex<HashMap<String, (serde_json::Value, serde_json::Value)>>,
    pub model_serial: Mutex<()>,
    pub write_serial: Mutex<()>,
    pub control: Mutex<()>,
}

pub struct Worker {
    pub id: String,
    pub ready: bool,
    pub last_seen: Instant,
}

#[derive(Default)]
pub struct Queue {
    pub deleted_threads: HashSet<String>,
    pub cancelled: HashSet<String>,
    pub worker: Option<Worker>,
    pub runs: HashMap<String, Arc<Run>>,
    pub pending: VecDeque<String>,
}

#[derive(Default)]
pub struct AgentRuntime {
    pub workspace_mutations: Mutex<()>,
    pub database: OnceCell<AgentStore>,
    pub queue: Mutex<Queue>,
    pub wake: Notify,
    pub worker_creation: Mutex<()>,
}

impl AgentRuntime {
    pub async fn store(&self) -> Result<&AgentStore, String> {
        self.database
            .get_or_try_init(|| async {
                let state = theseus::State::get_if_initialized()
                    .ok_or("Launcher is still initializing")?;
                let directory = state
                    .active_database_directory()
                    .map_err(|e| e.to_string())?;
                AgentStore::open(&directory)
                    .await
                    .map_err(|e| e.to_string())
            })
            .await
    }

    pub async fn worker(&self, worker_id: &str) -> Result<(), String> {
        let queue = self.queue.lock().await;
        let worker = queue
            .worker
            .as_ref()
            .filter(|worker| worker.id == worker_id)
            .ok_or("Worker has expired")?;
        if !worker.ready {
            return Err("Worker is not ready".into());
        }
        Ok(())
    }

    pub async fn run(&self, id: &str) -> Result<Arc<Run>, String> {
        let run = self
            .queue
            .lock()
            .await
            .runs
            .get(id)
            .cloned()
            .ok_or("Unknown run attempt")?;
        if run.cancel.is_cancelled()
            || run.state.load(Ordering::Acquire) == TERMINAL
        {
            return Err("Run attempt is no longer active".into());
        }
        Ok(run)
    }

    pub async fn claim(&self, worker_id: &str) -> Result<Option<Job>, String> {
        let mut queue = self.queue.lock().await;
        let worker = queue
            .worker
            .as_mut()
            .filter(|worker| worker.id == worker_id && worker.ready)
            .ok_or("Worker has expired")?;
        worker.last_seen = Instant::now();
        if queue
            .runs
            .values()
            .filter(|run| run.leased.load(Ordering::Acquire))
            .count()
            >= 4
        {
            return Ok(None);
        }
        let eligible = queue.pending.iter().position(|id| {
            let Some(run) = queue.runs.get(id) else {
                return false;
            };
            !run.cancel.is_cancelled()
                && !run.leased.load(Ordering::Acquire)
                && !queue.runs.iter().any(|(other_id, other)| {
                    other_id != id
                        && other.request.thread_id == run.request.thread_id
                        && matches!(
                            other.state.load(Ordering::Acquire),
                            RUNNING | AWAITING
                        )
                })
        });
        let Some(index) = eligible else {
            return Ok(None);
        };
        let id = queue.pending.remove(index).unwrap();
        let run = queue.runs[&id].clone();
        if run.state.load(Ordering::Acquire) == QUEUED {
            if !self
                .store()
                .await?
                .transition(&id, AttemptState::Queued, AttemptState::Running)
                .await
                .map_err(|e| e.to_string())?
            {
                return Err("Queued attempt state changed".into());
            }
            run.state.store(RUNNING, Ordering::Release);
        }
        run.leased.store(true, Ordering::Release);
        let mut job = run.job.lock().await;
        if job.resume.is_none() {
            let current = job.messages.last().cloned();
            job.messages = self
                .store()
                .await?
                .conversation_transcript(
                    &run.request.thread_id,
                    &run.request.provider_id,
                    &run.request.model_id,
                )
                .await
                .map_err(|e| e.to_string())?;
            if let Some(current) = current {
                job.messages.push(current);
            }
        }
        job.lease_id = uuid::Uuid::new_v4().to_string();
        *run.lease_id.lock().await = job.lease_id.clone();
        job.model_turns = run.models.load(Ordering::Acquire);
        job.tool_calls = run.tools.load(Ordering::Acquire);
        Ok(Some(job.clone()))
    }

    pub async fn publish(
        &self,
        attempt: &str,
        event_id: &str,
        event: serde_json::Value,
    ) -> Result<(), String> {
        self.store()
            .await?
            .append_event(
                attempt,
                event_id,
                &agent_window::persistence::redact(event),
            )
            .await
            .map_err(|e| e.to_string())?;
        self.wake.notify_waiters();
        Ok(())
    }

    pub async fn cancel(&self, id: &str) -> Result<(), String> {
        let run = {
            let mut queue = self.queue.lock().await;
            queue.cancelled.insert(id.to_owned());
            queue.runs.get(id).cloned()
        };
        let Some(run) = run else { return Ok(()) };
        run.cancel.cancel();
        let _control = run.control.lock().await;
        self.queue
            .lock()
            .await
            .pending
            .retain(|pending| pending != id);
        let previous = run.state.swap(TERMINAL, Ordering::AcqRel);
        run.leased.store(false, Ordering::Release);
        if previous == TERMINAL {
            return Ok(());
        }
        let from = self
            .store()
            .await?
            .attempt(id)
            .await
            .map_err(|e| e.to_string())?
            .ok_or("Run record missing")?
            .state;
        self.store()
            .await?
            .transition(id, from, AttemptState::Cancelled)
            .await
            .map_err(|e| e.to_string())?;
        self.wake.notify_waiters();
        Ok(())
    }
}

pub async fn shutdown(app: &AppHandle) {
    let Some(runtime) = app.try_state::<AgentRuntime>() else {
        return;
    };
    let ids: Vec<_> = runtime.queue.lock().await.runs.keys().cloned().collect();
    for id in ids {
        if let Err(error) = runtime.cancel(&id).await {
            tracing::warn!(%error, "Could not stop Agent task during shutdown");
        }
    }
}

pub fn budget(counter: &AtomicUsize, limit: usize) -> Result<(), String> {
    counter
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
            (value < limit).then_some(value + 1)
        })
        .map(|_| ())
        .map_err(|_| {
            "Agent work limit reached; continue the task explicitly".into()
        })
}

pub fn attempt(request: &SubmitRequest) -> Attempt {
    Attempt {
        id: request.attempt_id.clone(),
        thread_id: request.thread_id.clone(),
        turn_id: request.turn_id.clone(),
        state: AttemptState::Queued,
        computer_use: request.computer_use,
    }
}

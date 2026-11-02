mod execution;
mod native_jobs;
mod state;
mod types;
mod worker;

use agent_window::{
    AgentWindow,
    persistence::{Approval, ApprovalBatch, AttemptState},
};
use serde_json::{Value, json};
use state::*;
pub use state::{WORKER_LABEL, shutdown};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering},
    },
    time::{Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Manager, State, WebviewWindow};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use types::*;

fn ui(window: &WebviewWindow) -> Result<(), String> {
    if window.label() != agent_window::WINDOW_LABEL {
        return Err("Only Agent Window can submit tasks or decisions".into());
    }
    Ok(())
}

async fn worker(
    runtime: &AgentRuntime,
    window: &WebviewWindow,
    worker_id: &str,
) -> Result<(), String> {
    if window.label() != WORKER_LABEL {
        return Err(
            "Only the background Agent runtime may use this command".into()
        );
    }
    runtime.worker(worker_id).await
}

async fn leased_run(
    runtime: &AgentRuntime,
    attempt_id: &str,
    lease_id: &str,
) -> Result<Arc<Run>, String> {
    let run = runtime.run(attempt_id).await?;
    if !run.leased.load(Ordering::Acquire)
        || *run.lease_id.lock().await != lease_id
    {
        return Err("Worker run lease has expired".into());
    }
    Ok(run)
}

#[tauri::command]
async fn agent_runtime_load(
    window: WebviewWindow,
    runtime: State<'_, AgentRuntime>,
) -> Result<Value, String> {
    ui(&window)?;
    let _workspace = runtime.workspace_mutations.lock().await;
    let head = runtime
        .store()
        .await?
        .workspace_head()
        .await
        .map_err(|e| e.to_string())?;
    Ok(
        json!({"workspace":head.workspace,"workspace_revision":head.revision,"threads":runtime.store().await?.threads().await.map_err(|e|e.to_string())?}),
    )
}

#[tauri::command]
async fn agent_runtime_save(
    window: WebviewWindow,
    runtime: State<'_, AgentRuntime>,
    workspace: Value,
    expected_revision: i64,
    deleted_thread_ids: Option<Vec<String>>,
) -> Result<i64, String> {
    ui(&window)?;
    let _workspace = runtime.workspace_mutations.lock().await;
    let store = runtime.store().await?;
    let head = store.workspace_head().await.map_err(|e| e.to_string())?;
    if head.revision != expected_revision {
        return Err(
            "Agent workspace changed; reload saved history before retrying"
                .into(),
        );
    }
    let deleted = deleted_thread_ids.unwrap_or_default();
    let mut attempts = agent_window::persistence::validate_workspace_update(
        head.workspace.as_ref(),
        &workspace,
        &deleted,
    )
    .map_err(|e| e.to_string())?;
    attempts.extend(
        runtime
            .queue
            .lock()
            .await
            .runs
            .iter()
            .filter(|(_, run)| deleted.contains(&run.request.thread_id))
            .map(|(id, _)| id.clone()),
    );
    for attempt in attempts {
        runtime.cancel(&attempt).await?;
    }
    let mut queue = runtime.queue.lock().await;
    let revision = store
        .save_workspace(&workspace, expected_revision, &deleted)
        .await
        .map_err(|e| e.to_string())?;
    queue.deleted_threads.extend(deleted.iter().cloned());
    queue
        .runs
        .retain(|_, run| !deleted.contains(&run.request.thread_id));
    Ok(revision)
}

#[tauri::command]
async fn agent_runtime_import(
    window: WebviewWindow,
    runtime: State<'_, AgentRuntime>,
    import_id: String,
    threads: Vec<Value>,
) -> Result<bool, String> {
    ui(&window)?;
    let threads = threads
        .into_iter()
        .map(|thread| {
            let id = thread["id"]
                .as_str()
                .ok_or("Imported task has no identity")?
                .to_owned();
            Ok((id, thread))
        })
        .collect::<Result<Vec<_>, String>>()?;
    runtime
        .store()
        .await?
        .import_threads(&import_id, &threads)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn agent_runtime_delete(
    window: WebviewWindow,
    runtime: State<'_, AgentRuntime>,
    thread_id: String,
) -> Result<(), String> {
    ui(&window)?;
    let ids: Vec<_> = runtime
        .queue
        .lock()
        .await
        .runs
        .iter()
        .filter(|(_, run)| run.request.thread_id == thread_id)
        .map(|(id, _)| id.clone())
        .collect();
    for id in &ids {
        runtime.cancel(id).await?;
    }
    runtime
        .store()
        .await?
        .delete_thread(&thread_id)
        .await
        .map_err(|e| e.to_string())?;
    let mut queue = runtime.queue.lock().await;
    for id in ids {
        queue.runs.remove(&id);
    }
    Ok(())
}

#[tauri::command]
async fn agent_runtime_snapshot(
    window: WebviewWindow,
    runtime: State<'_, AgentRuntime>,
    thread_id: String,
) -> Result<Value, String> {
    ui(&window)?;
    let store = runtime.store().await?;
    let attempts = store
        .attempts(&thread_id)
        .await
        .map_err(|e| e.to_string())?;
    let mut results = serde_json::Map::new();
    for attempt in &attempts {
        results.insert(
            attempt.id.clone(),
            store
                .applied_result(&attempt.id)
                .await
                .map_err(|e| e.to_string())?,
        );
    }
    Ok(json!({"attempts":attempts,"results":results}))
}

#[tauri::command]
async fn agent_runtime_events(
    window: WebviewWindow,
    runtime: State<'_, AgentRuntime>,
    thread_id: String,
    after: i64,
) -> Result<Value, String> {
    ui(&window)?;
    Ok(json!(
        runtime
            .store()
            .await?
            .events(&thread_id, after, 500)
            .await
            .map_err(|e| e.to_string())?
    ))
}

#[tauri::command]
async fn agent_runtime_submit(
    app: AppHandle,
    window: WebviewWindow,
    runtime: State<'_, AgentRuntime>,
    request: SubmitRequest,
) -> Result<Value, String> {
    ui(&window)?;
    for id in [&request.attempt_id, &request.thread_id, &request.turn_id] {
        uuid::Uuid::parse_str(id).map_err(|_| "Invalid task identity")?;
    }
    if request.text.len() > 200_000
        || request.instructions.len() > 8000
        || request.attachments.len() > 10
        || (request.text.trim().is_empty() && request.attachments.is_empty())
    {
        return Err("Request is empty or too large".into());
    }
    let mut text = request.text.clone();
    for attachment in &request.attachments {
        let content = attachment["text"].as_str().filter(|text| !text.is_empty() && text.len() <= 2 * 1024 * 1024).ok_or("Finish reading attachments before sending; filenames alone are not readable content")?;
        let name = attachment["name"].as_str().unwrap_or("Attachment");
        text.push_str(&format!(
            "\n<attachment name={}>\n{}\n</attachment>",
            serde_json::to_string(name).unwrap(),
            content
        ));
    }
    let mut context = if let Some(id) = &request.instance_id {
        let instance = theseus::instance::get(id)
            .await
            .map_err(|e| e.to_string())?
            .ok_or("Selected instance is no longer available")?;
        json!({"instance_id":id,"name":instance.instance.name,"game_version":instance.applied_content_set.game_version}).to_string()
    } else {
        "Launcher-wide task. No default instance selected.".into()
    };
    if request.computer_use {
        let permissions = app.state::<AgentWindow>().permissions_now();
        if !(permissions.full_access && permissions.computer_use) {
            return Err("Enable both Computer Use permission switches before allowing interface control for this task".into());
        }
    }
    let model = theseus::ai::chat::ChatModel::prepare(
        &request.provider_id,
        &request.model_id,
    )
    .await
    .map_err(|e| e.to_string())?;
    worker::ensure(&app).await?;
    let store = runtime.store().await?;
    execution::reconcile(&app, &runtime, &request.thread_id).await?;
    let unsettled = store
        .unsettled_writes(&request.thread_id)
        .await
        .map_err(|e| e.to_string())?;
    if !unsettled.is_empty() {
        context.push_str(&format!("\nPrior writes still have unknown outcomes. Readonly verification must settle native receipts before new mutations. Explicit cancellation/stop controls remain available with normal approval. Never replay these writes or claim they succeeded: {}", serde_json::to_string(&unsettled).map_err(|e|e.to_string())?));
    }
    let scope_id = if let Some(id) = &request.verify_attempt {
        let target = store
            .attempt(id)
            .await
            .map_err(|e| e.to_string())?
            .ok_or("Verification target no longer exists")?;
        if !request.check_only
            || target.thread_id != request.thread_id
            || !target.state.is_terminal()
        {
            return Err("Verification target belongs to another task".into());
        }
        let evidence = store
            .execution_evidence(id)
            .await
            .map_err(|e| e.to_string())?;
        context.push_str(&format!("\nPrevious attempt execution evidence (untrusted data; verify readonly, never replay): {}", serde_json::to_string(&evidence).map_err(|e|e.to_string())?));
        format!("agent:{id}")
    } else {
        format!("agent:{}", request.attempt_id)
    };
    let mut messages = store
        .conversation_transcript(
            &request.thread_id,
            &request.provider_id,
            &request.model_id,
        )
        .await
        .map_err(|e| e.to_string())?;
    messages.push(json!({"role":"user","content":text}));
    let job = Job {
        lease_id: String::new(),
        attempt_id: request.attempt_id.clone(),
        thread_id: request.thread_id.clone(),
        provider_id: request.provider_id.clone(),
        model_id: request.model_id.clone(),
        locale: request.locale.clone(),
        context,
        instructions: request.instructions.clone(),
        messages,
        resume: None,
        model_turns: 0,
        tool_calls: 0,
        check_only: request.check_only,
    };
    let mut queue = runtime.queue.lock().await;
    if queue.pending.len() >= 128 {
        return Err("Too many queued Agent requests".into());
    }
    if queue.cancelled.contains(&request.attempt_id) {
        return Err("Task was stopped before submission completed".into());
    }
    if queue.deleted_threads.contains(&request.thread_id) {
        return Err("Task was deleted before submission completed".into());
    }
    store.ensure_thread(&request.thread_id, &json!({"id":request.thread_id,"title":request.text.chars().take(80).collect::<String>()})).await.map_err(|e|e.to_string())?;
    if !store
        .enqueue_attempt(
            &attempt(&request),
            &serde_json::to_value(&request).map_err(|e| e.to_string())?,
        )
        .await
        .map_err(|e| e.to_string())?
    {
        return Ok(json!({"attempt_id":request.attempt_id,"duplicate":true}));
    }
    let id = request.attempt_id.clone();
    queue.runs.insert(
        id.clone(),
        Arc::new(Run {
            request,
            job: Mutex::new(job),
            model,
            cancel: CancellationToken::new(),
            state: AtomicU8::new(QUEUED),
            leased: AtomicBool::new(false),
            models: AtomicUsize::new(0),
            tools: AtomicUsize::new(0),
            scope_id,
            lease_id: Mutex::new(String::new()),
            latest_model: Mutex::new(Value::Null),
            model_results: Mutex::new(Default::default()),
            model_serial: Mutex::new(()),
            write_serial: Mutex::new(()),
            control: Mutex::new(()),
        }),
    );
    queue.pending.push_back(id.clone());
    drop(queue);
    runtime.wake.notify_waiters();
    Ok(json!({"attempt_id":id}))
}

#[tauri::command]
async fn agent_runtime_cancel(
    window: WebviewWindow,
    runtime: State<'_, AgentRuntime>,
    attempt_id: String,
) -> Result<(), String> {
    ui(&window)?;
    runtime.cancel(&attempt_id).await
}

#[tauri::command]
async fn agent_runtime_decide(
    window: WebviewWindow,
    runtime: State<'_, AgentRuntime>,
    batch_id: String,
    revision: i64,
    selected: Vec<String>,
) -> Result<Value, String> {
    ui(&window)?;
    let store = runtime.store().await?;
    let batch = store
        .batch(&batch_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("Approval batch not found")?;
    let run = runtime
        .queue
        .lock()
        .await
        .runs
        .get(&batch.attempt_id)
        .cloned();
    let _control = if let Some(run) = &run {
        Some(run.control.lock().await)
    } else {
        None
    };
    let changed = store
        .decide_batch(&batch_id, revision, &selected)
        .await
        .map_err(|e| e.to_string())?;
    if changed {
        let run = run.as_ref().ok_or("Approval cannot resume after a launcher restart; continue the task manually")?;
        if run.cancel.is_cancelled() {
            return Err(
                "Task was stopped before approval could resume it".into()
            );
        }
        run.job.lock().await.resume = Some(
            json!({"call_ids":batch.actions.iter().map(|a|&a.call_id).collect::<Vec<_>>(),"decisions":batch.actions.iter().map(|a| if selected.contains(&a.call_id) {json!({"type":"approve"})} else {json!({"type":"reject","message":"User excluded this action. It was not executed. Do not retry it."})}).collect::<Vec<_>>()}),
        );
        run.state.store(RUNNING, Ordering::Release);
        runtime
            .queue
            .lock()
            .await
            .pending
            .push_front(batch.attempt_id);
        runtime.wake.notify_waiters();
    }
    Ok(json!({"changed":changed}))
}

#[tauri::command]
async fn agent_worker_ready(
    window: WebviewWindow,
    runtime: State<'_, AgentRuntime>,
    worker_id: String,
) -> Result<(), String> {
    if window.label() != WORKER_LABEL {
        return Err("Only the Agent worker may become ready".into());
    }
    let mut queue = runtime.queue.lock().await;
    let worker = queue
        .worker
        .as_mut()
        .filter(|worker| worker.id == worker_id)
        .ok_or("Worker bootstrap expired")?;
    worker.ready = true;
    worker.last_seen = Instant::now();
    drop(queue);
    runtime.wake.notify_waiters();
    Ok(())
}

#[tauri::command]
async fn agent_worker_next(
    window: WebviewWindow,
    runtime: State<'_, AgentRuntime>,
    worker_id: String,
) -> Result<worker::Poll, String> {
    worker(&runtime, &window, &worker_id).await?;
    worker::next(&runtime, &worker_id).await
}

#[tauri::command]
async fn agent_worker_catalog(
    app: AppHandle,
    window: WebviewWindow,
    runtime: State<'_, AgentRuntime>,
    worker_id: String,
    lease_id: String,
    attempt_id: String,
) -> Result<Value, String> {
    worker(&runtime, &window, &worker_id).await?;
    let run = leased_run(&runtime, &attempt_id, &lease_id).await?;
    let unsettled = runtime
        .store()
        .await?
        .has_unsettled_writes(&run.request.thread_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(json!(
        launcher_tools::registry::catalog()
            .iter()
            .filter(|entry| execution::allowed(&app, &run, entry).is_ok())
            .filter(|entry| execution::available_with_unsettled(
                entry, unsettled
            ))
            .collect::<Vec<_>>()
    ))
}

#[tauri::command]
async fn agent_worker_model(
    window: WebviewWindow,
    runtime: State<'_, AgentRuntime>,
    worker_id: String,
    lease_id: String,
    attempt_id: String,
    request_id: String,
    turn: Value,
    on_event: tauri::ipc::Channel<Value>,
) -> Result<Value, String> {
    worker(&runtime, &window, &worker_id).await?;
    let run = leased_run(&runtime, &attempt_id, &lease_id).await?;
    let _serial = run.model_serial.lock().await;
    if let Some((input, result)) =
        run.model_results.lock().await.get(&request_id)
    {
        if input != &turn {
            return Err(
                "Model request identity reused with different messages".into(),
            );
        }
        return Ok(result.clone());
    }
    budget(&run.models, 64)?;
    let input = serde_json::from_value(turn.clone())
        .map_err(|e| format!("Invalid model turn: {e}"))?;
    theseus::ai::chat::ChatModel::validate_agent_turn(&input)
        .map_err(|e| e.to_string())?;
    runtime
        .store()
        .await?
        .save_model_input(
            &attempt_id,
            &request_id,
            turn["messages"]
                .as_array()
                .ok_or("Model messages must be an array")?,
        )
        .await
        .map_err(|e| e.to_string())?;
    let emit = |event_type: &str, text: &str| {
        if !run.cancel.is_cancelled() {
            let _ = on_event
                .send(json!({"event_type":event_type,"data":{"text":text}}));
        }
    };
    let reply = tokio::select! {
        biased;
        _ = run.cancel.cancelled() => return Err("Agent task stopped".into()),
        reply = run.model.turn_for_agent(input, &emit) => reply.map_err(|e|e.to_string())?,
    };
    for call in &reply.tool_calls {
        if call.name == "write_todos" {
            budget(&run.tools, 256)?;
        }
    }
    let reply = serde_json::to_value(reply).map_err(|e| e.to_string())?;
    runtime
        .store()
        .await?
        .save_model_reply(&attempt_id, &request_id, &reply)
        .await
        .map_err(|e| e.to_string())?;
    *run.latest_model.lock().await = reply.clone();
    run.model_results
        .lock()
        .await
        .insert(request_id, (turn, reply.clone()));
    Ok(reply)
}

#[tauri::command]
async fn agent_worker_tool(
    app: AppHandle,
    window: WebviewWindow,
    runtime: State<'_, AgentRuntime>,
    worker_id: String,
    lease_id: String,
    attempt_id: String,
    request: ToolRequest,
) -> Result<Value, String> {
    worker(&runtime, &window, &worker_id).await?;
    let run = leased_run(&runtime, &attempt_id, &lease_id).await?;
    if runtime
        .store()
        .await?
        .execution(&attempt_id, &request.call_id)
        .await
        .map_err(|e| e.to_string())?
        .is_none()
    {
        let latest = run.latest_model.lock().await;
        if !latest["tool_calls"].as_array().is_some_and(|calls| {
            calls.iter().any(|call| {
                call["id"] == request.call_id
                    && call["name"] == request.name
                    && call["args"] == Value::Object(request.arguments.clone())
            })
        }) {
            return Err("Tool call does not match the model response".into());
        }
    }
    execution::tool(&app, &runtime, run, request).await
}

#[tauri::command]
async fn agent_worker_approval(
    app: AppHandle,
    window: WebviewWindow,
    runtime: State<'_, AgentRuntime>,
    worker_id: String,
    lease_id: String,
    attempt_id: String,
    actions: Vec<ReviewAction>,
) -> Result<(), String> {
    worker(&runtime, &window, &worker_id).await?;
    let run = leased_run(&runtime, &attempt_id, &lease_id).await?;
    let _control = run.control.lock().await;
    if run.cancel.is_cancelled() {
        return Err("Task stopped".into());
    }
    let unsettled = runtime
        .store()
        .await?
        .has_unsettled_writes(&run.request.thread_id)
        .await
        .map_err(|e| e.to_string())?;
    let latest = run.latest_model.lock().await;
    let id = uuid::Uuid::new_v4().to_string();
    let expires_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs() as i64
        + 600;
    let mut approvals = Vec::new();
    let mut changes = Vec::new();
    for (index, action) in actions.iter().enumerate() {
        let entry = launcher_tools::registry::get(&action.name)
            .ok_or("Unknown approval tool")?;
        if entry.read_only || run.request.check_only {
            return Err("Only business writes require approval".into());
        }
        launcher_tools::registry::validate(
            entry,
            action
                .arguments
                .as_object()
                .ok_or("Tool arguments must be an object")?,
        )
        .map_err(|e| e.to_string())?;
        execution::allowed(&app, &run, entry)?;
        if !execution::available_with_unsettled(entry, unsettled) {
            return Err("Unconfirmed writes require readonly verification or explicit cancellation before new mutations".into());
        }
        if !latest["tool_calls"].as_array().is_some_and(|calls| {
            calls.iter().any(|call| {
                call["id"] == action.call_id
                    && call["name"] == action.name
                    && call["args"] == action.arguments
            })
        }) {
            return Err("Approval does not match the model tool call".into());
        }
        approvals.push(Approval {
            id: format!("{id}:{index}"),
            attempt_id: attempt_id.clone(),
            call_id: action.call_id.clone(),
            tool: action.name.clone(),
            arguments: action.arguments.clone(),
            revision: 1,
            expires_at,
        });
        changes.push(json!({"name":action.call_id,"note":action.arguments.to_string(),"meta":action.name,"kind":if entry.destructive {"remove"} else {"setting"},"manual":false}));
    }
    drop(latest);
    runtime
        .store()
        .await?
        .prepare_batch(&ApprovalBatch {
            id: id.clone(),
            attempt_id: attempt_id.clone(),
            revision: 1,
            expires_at,
            actions: approvals,
        })
        .await
        .map_err(|e| e.to_string())?;
    runtime.publish(&attempt_id, &uuid::Uuid::new_v4().to_string(), json!({"type":"plan","plan":{"id":id,"revision":1,"expires_at":expires_at,"title":"Review launcher changes","summary":"","changes":changes}})).await?;
    run.state.store(AWAITING, Ordering::Release);
    runtime.wake.notify_waiters();
    Ok(())
}

#[tauri::command]
async fn agent_worker_event(
    window: WebviewWindow,
    runtime: State<'_, AgentRuntime>,
    worker_id: String,
    lease_id: String,
    attempt_id: String,
    event_id: String,
    event: Value,
) -> Result<(), String> {
    worker(&runtime, &window, &worker_id).await?;
    leased_run(&runtime, &attempt_id, &lease_id).await?;
    if !matches!(event["type"].as_str(), Some("text" | "step" | "progress")) {
        return Err("Worker event type is not allowed".into());
    }
    runtime.publish(&attempt_id, &event_id, event).await
}

#[tauri::command]
async fn agent_worker_checkpoint(
    window: WebviewWindow,
    runtime: State<'_, AgentRuntime>,
    worker_id: String,
    lease_id: String,
    attempt_id: String,
    request: CheckpointRequest,
) -> Result<Value, String> {
    worker(&runtime, &window, &worker_id).await?;
    leased_run(&runtime, &attempt_id, &lease_id).await?;
    let store = runtime.store().await?;
    match request {
        CheckpointRequest::Get { namespace, id } => Ok(json!(
            store
                .graph_checkpoints(
                    &attempt_id,
                    &namespace,
                    id.as_deref(),
                    None,
                    1
                )
                .await
                .map_err(|e| e.to_string())?
        )),
        CheckpointRequest::List {
            namespace,
            id,
            before,
            limit,
        } => Ok(json!(
            store
                .graph_checkpoints(
                    &attempt_id,
                    &namespace,
                    id.as_deref(),
                    before.as_deref(),
                    limit.unwrap_or(1000)
                )
                .await
                .map_err(|e| e.to_string())?
        )),
        CheckpointRequest::Put { namespace, record } => {
            store
                .put_graph_checkpoint(&attempt_id, &namespace, &record)
                .await
                .map_err(|e| e.to_string())?;
            Ok(Value::Null)
        }
        CheckpointRequest::PutWrites {
            namespace,
            id,
            writes,
        } => {
            store
                .put_graph_writes(&attempt_id, &namespace, &id, &writes)
                .await
                .map_err(|e| e.to_string())?;
            Ok(Value::Null)
        }
    }
}

#[tauri::command]
async fn agent_worker_complete(
    window: WebviewWindow,
    runtime: State<'_, AgentRuntime>,
    worker_id: String,
    lease_id: String,
    attempt_id: String,
    outcome: Value,
) -> Result<(), String> {
    worker(&runtime, &window, &worker_id).await?;
    let run = leased_run(&runtime, &attempt_id, &lease_id).await?;
    let _control = run.control.lock().await;
    if run.cancel.is_cancelled() {
        return Err("Task stopped".into());
    }
    if outcome["status"] == "awaiting" {
        if run.state.load(Ordering::Acquire) != AWAITING
            && run.job.lock().await.resume.is_none()
        {
            return Err(
                "Worker reported an approval without a persisted batch".into(),
            );
        }
        run.leased.store(false, Ordering::Release);
        runtime.wake.notify_waiters();
        return Ok(());
    }
    let from = runtime
        .store()
        .await?
        .attempt(&attempt_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("Run record missing")?
        .state;
    let to = if outcome["status"] == "completed" {
        let latest = run.latest_model.lock().await;
        if latest["content"] != outcome["message"]["content"]
            || latest["tool_calls"]
                .as_array()
                .is_some_and(|calls| !calls.is_empty())
        {
            return Err(
                "Final response does not match the completed model turn".into(),
            );
        }
        let input = run
            .job
            .lock()
            .await
            .messages
            .last()
            .cloned()
            .ok_or("Task input is missing")?;
        runtime
            .store()
            .await?
            .set_message(
                &run.request.thread_id,
                Some(&attempt_id),
                &format!("{attempt_id}:user"),
                &input,
            )
            .await
            .map_err(|e| e.to_string())?;
        runtime
            .store()
            .await?
            .set_message(
                &run.request.thread_id,
                Some(&attempt_id),
                &format!("{attempt_id}:reply"),
                &latest,
            )
            .await
            .map_err(|e| e.to_string())?;
        runtime
            .publish(
                &attempt_id,
                &format!("{attempt_id}:done"),
                json!({"type":"done","applied":runtime.store().await?.applied_result(&attempt_id).await.map_err(|e|e.to_string())?}),
            )
            .await?;
        AttemptState::Completed
    } else {
        runtime.publish(&attempt_id, &uuid::Uuid::new_v4().to_string(), json!({"type":"error","title":"Agent task failed","text":outcome["error"].as_str().unwrap_or("Execution stopped; inspect current state before retrying"),"retryable":false})).await?;
        AttemptState::Failed
    };
    runtime
        .store()
        .await?
        .transition(&attempt_id, from, to)
        .await
        .map_err(|e| e.to_string())?;
    run.state.store(TERMINAL, Ordering::Release);
    run.leased.store(false, Ordering::Release);
    runtime.wake.notify_waiters();
    Ok(())
}

pub fn init() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri::plugin::Builder::new("agent-runtime")
        .invoke_handler(tauri::generate_handler![
            agent_runtime_load,
            agent_runtime_save,
            agent_runtime_import,
            agent_runtime_delete,
            agent_runtime_snapshot,
            agent_runtime_events,
            agent_runtime_submit,
            agent_runtime_cancel,
            agent_runtime_decide,
            agent_worker_ready,
            agent_worker_next,
            agent_worker_catalog,
            agent_worker_model,
            agent_worker_tool,
            agent_worker_approval,
            agent_worker_event,
            agent_worker_checkpoint,
            agent_worker_complete
        ])
        .setup(|app, _| {
            app.manage(AgentRuntime::default());
            Ok(())
        })
        .build()
}

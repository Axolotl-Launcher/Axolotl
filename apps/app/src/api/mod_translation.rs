use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{Emitter, Runtime};

use crate::mod_translation;
use crate::mod_translation::analyze::{AnalysisSummary, JarInspection};
use crate::mod_translation::error::TranslateErrorCode;
use crate::mod_translation::repair::{RepairActivity, RepairEmitter};
use crate::mod_translation::translate::{
    PreparedTranslationWorkspace, TranslationReport, TranslationSample,
};

const WORKSPACE_SUBDIR: &str = "mod-translation";
const TASK_EVENT: &str = "mod-translation-task-event";
const ANALYSIS_TTL: Duration = Duration::from_secs(30 * 60);
const MAX_SNAPSHOT_EVENTS: usize = 400;

struct TaskHandle {
    cancel: Arc<AtomicBool>,
    sequence: AtomicU64,
    event_gate: Mutex<()>,
    snapshot: Mutex<TaskSnapshot>,
    lock_keys: Vec<String>,
}

struct PreparedAnalysis {
    created_at: SystemTime,
    input_path: PathBuf,
    input_hash: String,
    workspace: PathBuf,
    inspection: JarInspection,
}

static TASKS: OnceLock<Mutex<HashMap<String, Arc<TaskHandle>>>> =
    OnceLock::new();
static ACTIVE_LOCKS: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
static ANALYSES: OnceLock<Mutex<HashMap<String, PreparedAnalysis>>> =
    OnceLock::new();
/// Running analyses by the request ID their caller chose, and cancels that
/// arrived before their analysis was registered.
static ANALYSIS_REQUESTS: OnceLock<Mutex<AnalysisRequests>> = OnceLock::new();

#[derive(Default)]
struct AnalysisRequests {
    running: HashMap<String, Arc<AtomicBool>>,
    early: HashMap<String, SystemTime>,
    /// Analyses registered for a request, so a cancel that arrives after the
    /// result was produced can still discard it.
    finished: HashMap<String, (String, SystemTime)>,
}

const EARLY_ANALYSIS_CANCEL_TTL: Duration = Duration::from_secs(30);

fn analysis_requests() -> &'static Mutex<AnalysisRequests> {
    ANALYSIS_REQUESTS.get_or_init(Mutex::default)
}

struct AnalysisRequest(String);

impl AnalysisRequest {
    fn register(id: String, cancel: &Arc<AtomicBool>) -> Self {
        if let Ok(mut requests) = analysis_requests().lock() {
            if requests.early.remove(&id).is_some_and(|at| {
                at.elapsed()
                    .is_ok_and(|elapsed| elapsed < EARLY_ANALYSIS_CANCEL_TTL)
            }) {
                cancel.store(true, Ordering::Relaxed);
            }
            requests.running.insert(id.clone(), Arc::clone(cancel));
        }
        Self(id)
    }
}

impl Drop for AnalysisRequest {
    fn drop(&mut self) {
        if let Ok(mut requests) = analysis_requests().lock() {
            requests.running.remove(&self.0);
        }
    }
}

fn tasks() -> &'static Mutex<HashMap<String, Arc<TaskHandle>>> {
    TASKS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn active_locks() -> &'static Mutex<HashMap<String, String>> {
    ACTIVE_LOCKS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn analyses() -> &'static Mutex<HashMap<String, PreparedAnalysis>> {
    ANALYSES.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn init<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("mod-translation")
        .invoke_handler(tauri::generate_handler![
            mod_translation_analyze,
            mod_translation_translate,
            mod_translation_cancel,
            mod_translation_cancel_analysis,
            mod_translation_list_tasks,
            mod_translation_get_task,
            mod_translation_dismiss_task,
        ])
        .build()
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModTranslationOptions {
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
    #[serde(default = "default_deep_batch_size")]
    pub deep_batch_size: usize,
    #[serde(default)]
    pub generate_mod_name: bool,
    #[serde(default = "default_repair_enabled")]
    pub repair_enabled: bool,
    #[serde(default)]
    pub class_text_enabled: bool,
    #[serde(default = "default_class_batch")]
    pub max_class_batch: usize,
}

impl Default for ModTranslationOptions {
    fn default() -> Self {
        Self {
            batch_size: default_batch_size(),
            deep_batch_size: default_deep_batch_size(),
            generate_mod_name: false,
            repair_enabled: default_repair_enabled(),
            class_text_enabled: false,
            max_class_batch: default_class_batch(),
        }
    }
}

fn default_batch_size() -> usize {
    40
}

fn default_deep_batch_size() -> usize {
    24
}

fn default_repair_enabled() -> bool {
    true
}

fn default_class_batch() -> usize {
    16
}

impl From<ModTranslationOptions>
    for mod_translation::translate::TranslateOptions
{
    fn from(value: ModTranslationOptions) -> Self {
        Self {
            batch_size: value.batch_size.clamp(10, 80),
            deep_batch_size: value.deep_batch_size.clamp(4, 40),
            generate_mod_name: value.generate_mod_name,
            repair_enabled: value.repair_enabled,
            class_text_enabled: value.class_text_enabled,
            max_class_batch: value.max_class_batch.clamp(4, 20),
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModTranslationAnalysis {
    pub analysis_id: String,
    pub input_hash: String,
    #[serde(flatten)]
    pub summary: AnalysisSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskFailure {
    pub code: String,
    pub message: String,
    pub details: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModTranslationTaskEvent {
    pub event_id: String,
    pub task_id: String,
    pub sequence: u64,
    pub occurred_at: String,
    pub event_type: String,
    pub status: String,
    pub progress: Option<TranslationSample>,
    pub activity: Option<RepairActivity>,
    pub report: Option<TranslationReport>,
    pub error: Option<TaskFailure>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskSnapshot {
    pub task_id: String,
    pub input_path: PathBuf,
    pub output_path: PathBuf,
    pub input_hash: String,
    pub started_at: String,
    pub updated_at: String,
    pub status: String,
    pub sequence: u64,
    pub progress: Option<TranslationSample>,
    pub activities: Vec<RepairActivity>,
    pub report: Option<TranslationReport>,
    pub error: Option<TaskFailure>,
    pub events: Vec<ModTranslationTaskEvent>,
}

async fn workspace_root() -> crate::api::Result<PathBuf> {
    let state = theseus::State::get().await?;
    Ok(state.directories.caches_dir().join(WORKSPACE_SUBDIR))
}

fn to_command_error(error: mod_translation::error::TranslateError) -> String {
    tracing::error!(code = ?error.code, message = %error.message, "mod translation command failed");
    error.user_message()
}

#[tauri::command]
pub async fn mod_translation_analyze(
    input_path: String,
    request_id: Option<String>,
) -> Result<ModTranslationAnalysis, String> {
    let cancel = Arc::new(AtomicBool::new(false));
    let finished_request = request_id.clone();
    let _request = request_id.map(|id| AnalysisRequest::register(id, &cancel));
    let root = workspace_root().await.map_err(|error| error.to_string())?;
    let input = PathBuf::from(&input_path);
    if !input.is_file() {
        return Err("找不到输入 JAR 文件".to_string());
    }
    let input_for_analysis = input.clone();
    let blocking_cancel = Arc::clone(&cancel);
    let (directory, inspection, input_hash) =
        tauri::async_runtime::spawn_blocking(move || {
            let input_hash =
                mod_translation::analyze::input_file_hash_cancellable(
                    &input_for_analysis,
                    &blocking_cancel,
                )
                .map_err(to_command_error)?;
            let (directory, inspection) =
                mod_translation::analyze::extract_and_inspect_cancellable(
                    &input_for_analysis,
                    &root,
                    &blocking_cancel,
                )
                .map_err(to_command_error)?;
            Ok::<_, String>((directory, inspection, input_hash))
        })
        .await
        .map_err(|error| error.to_string())??;
    // A cancel that arrived after the last checkpoint still discards the
    // workspace instead of registering an analysis nobody waits for.
    if cancel.load(Ordering::Relaxed) {
        let _ = std::fs::remove_dir_all(&directory);
        return Err(TranslateErrorCode::Cancelled.as_str().to_string());
    }
    let analysis_id = format!("ANALYSIS-{}", uuid::Uuid::new_v4());
    let summary = mod_translation::translate::analysis_summary(&inspection);
    let mut registry = analyses()
        .lock()
        .map_err(|_| "分析注册表不可用".to_string())?;
    prune_analyses(&mut registry);
    registry.insert(
        analysis_id.clone(),
        PreparedAnalysis {
            created_at: SystemTime::now(),
            input_path: input,
            input_hash: input_hash.clone(),
            workspace: directory,
            inspection,
        },
    );
    drop(registry);
    // Decide under the request lock whether a cancel already arrived; a cancel
    // after this point finds the analysis among the finished requests instead.
    if let Some(request_id) = finished_request {
        let canceled = {
            let mut requests = analysis_requests()
                .lock()
                .map_err(|_| "分析注册表不可用".to_string())?;
            if cancel.load(Ordering::Relaxed) {
                true
            } else {
                requests.finished.retain(|_, (_, at)| {
                    at.elapsed().is_ok_and(|elapsed| {
                        elapsed < EARLY_ANALYSIS_CANCEL_TTL
                    })
                });
                if requests.finished.len() < 64 {
                    requests.finished.insert(
                        request_id,
                        (analysis_id.clone(), SystemTime::now()),
                    );
                }
                false
            }
        };
        if canceled {
            discard_analysis(&analysis_id);
            return Err(TranslateErrorCode::Cancelled.as_str().to_string());
        }
    }
    Ok(ModTranslationAnalysis {
        analysis_id,
        input_hash,
        summary,
    })
}

/// Removes a registered analysis and its unpacked workspace.
fn discard_analysis(analysis_id: &str) {
    let prepared = analyses()
        .lock()
        .ok()
        .and_then(|mut registry| registry.remove(analysis_id));
    if let Some(prepared) = prepared {
        let _ = std::fs::remove_dir_all(prepared.workspace);
    }
}

async fn validate_ai_config(
    provider_id: &str,
    model_id: &str,
) -> Result<(), String> {
    let state = theseus::ai::get_state()
        .await
        .map_err(|error| error.to_string())?;
    if !state.settings.enabled {
        return Err(format!(
            "{}: 请在 AI 设置中启用 AI 功能后再试",
            TranslateErrorCode::AiDisabled.as_str()
        ));
    }
    let Some(provider) = state
        .providers
        .iter()
        .find(|provider| provider.provider_id == provider_id)
    else {
        return Err(format!(
            "{}: 未找到已配置的 AI 提供商 {provider_id}，请先在 AI 设置中添加",
            TranslateErrorCode::AiProviderDisabled.as_str()
        ));
    };
    if !provider.enabled {
        return Err(format!(
            "{}: AI 提供商 {provider_id} 当前已禁用，请在 AI 设置中启用",
            TranslateErrorCode::AiProviderDisabled.as_str()
        ));
    }
    if model_id.trim().is_empty()
        || !provider.models.iter().any(|model| model.id == model_id)
    {
        return Err(format!(
            "{}: 请先在翻译设置或 AI 设置中选择模型",
            TranslateErrorCode::AiModelNotSelected.as_str()
        ));
    }
    Ok(())
}

#[tauri::command]
pub async fn mod_translation_translate<R: Runtime>(
    app: tauri::AppHandle<R>,
    input_path: String,
    output_path: String,
    provider_id: String,
    model_id: String,
    analysis_id: Option<String>,
    input_hash: Option<String>,
    options: Option<ModTranslationOptions>,
) -> Result<TaskSnapshot, String> {
    validate_ai_config(&provider_id, &model_id).await?;
    let input = PathBuf::from(&input_path);
    let output = PathBuf::from(&output_path);
    if !input.is_file() {
        return Err("找不到输入 JAR 文件".to_string());
    }
    if !mod_translation::jar::is_clean_absolute_path(&output) {
        return Err("输出路径必须是合法的绝对路径".to_string());
    }
    if output.exists() {
        return Err(format!("输出文件已存在：{}", output.display()));
    }
    let actual_hash = {
        let input = input.clone();
        tauri::async_runtime::spawn_blocking(move || {
            mod_translation::analyze::input_file_hash(&input)
        })
        .await
        .map_err(|error| error.to_string())?
        .map_err(to_command_error)?
    };
    let root = workspace_root().await.map_err(|error| error.to_string())?;
    let task_id = new_task_id();
    let lock_keys = vec![
        format!("output:{}", normalized_path_lock_key(&output)),
        format!("checkpoint:{actual_hash}"),
    ];
    claim_locks(&task_id, &lock_keys)?;
    let prepared = match take_prepared_analysis(
        analysis_id.as_deref(),
        input_hash.as_deref(),
        &actual_hash,
        &input,
    ) {
        Ok(prepared) => prepared,
        Err(error) => {
            release_lock_keys(&task_id, &lock_keys);
            return Err(error);
        }
    };
    let now = iso_now();
    let snapshot = TaskSnapshot {
        task_id: task_id.clone(),
        input_path: input.clone(),
        output_path: output.clone(),
        input_hash: actual_hash,
        started_at: now.clone(),
        updated_at: now,
        status: "running".to_string(),
        sequence: 0,
        progress: None,
        activities: Vec::new(),
        report: None,
        error: None,
        events: Vec::new(),
    };
    let handle = Arc::new(TaskHandle {
        cancel: Arc::new(AtomicBool::new(false)),
        sequence: AtomicU64::new(0),
        event_gate: Mutex::new(()),
        snapshot: Mutex::new(snapshot.clone()),
        lock_keys,
    });
    match tasks().lock() {
        Ok(mut registry) => {
            registry.insert(task_id.clone(), handle.clone());
        }
        Err(_) => {
            release_lock_keys(&task_id, &handle.lock_keys);
            if let Some(prepared) = &prepared {
                let _ = std::fs::remove_dir_all(&prepared.workspace);
            }
            return Err("任务注册表不可用".to_string());
        }
    }

    let mut translate_options: mod_translation::translate::TranslateOptions =
        options.unwrap_or_default().into();
    translate_options.provider_id = provider_id;
    translate_options.model_id = model_id;
    let emitter = make_progress_emitter(app.clone(), handle.clone());
    let repair_emitter = make_repair_emitter(app.clone(), handle.clone());
    let cancel = handle.cancel.clone();
    tauri::async_runtime::spawn(async move {
        let result = mod_translation::translate::run_translation_task(
            task_id.clone(),
            input,
            output,
            translate_options,
            root,
            prepared,
            cancel,
            emitter,
            repair_emitter,
        )
        .await;
        if let Err(error) = result {
            ensure_terminal_error(&app, &handle, &error);
        }
        release_locks(&handle);
    });
    Ok(snapshot)
}

#[tauri::command]
pub fn mod_translation_cancel(task_id: String) -> Result<(), String> {
    if let Some(handle) = tasks()
        .lock()
        .map_err(|_| "任务注册表不可用".to_string())?
        .get(&task_id)
    {
        handle.cancel.store(true, Ordering::Relaxed);
    }
    Ok(())
}

/// Stops a running analysis at its next checkpoint.
#[tauri::command]
pub fn mod_translation_cancel_analysis(
    request_id: String,
) -> Result<(), String> {
    let mut requests = analysis_requests()
        .lock()
        .map_err(|_| "分析注册表不可用".to_string())?;
    if let Some(cancel) = requests.running.get(&request_id) {
        cancel.store(true, Ordering::Relaxed);
        return Ok(());
    }
    if let Some((analysis_id, _)) = requests.finished.remove(&request_id) {
        drop(requests);
        discard_analysis(&analysis_id);
        return Ok(());
    }
    requests.early.retain(|_, at| {
        at.elapsed()
            .is_ok_and(|elapsed| elapsed < EARLY_ANALYSIS_CANCEL_TTL)
    });
    if requests.early.len() < 64 {
        requests.early.insert(request_id, SystemTime::now());
    }
    Ok(())
}

#[tauri::command]
pub fn mod_translation_list_tasks() -> Result<Vec<TaskSnapshot>, String> {
    let registry =
        tasks().lock().map_err(|_| "任务注册表不可用".to_string())?;
    let mut snapshots = registry
        .values()
        .filter_map(|handle| {
            handle.snapshot.lock().ok().map(|snapshot| snapshot.clone())
        })
        .collect::<Vec<_>>();
    snapshots.sort_by(|left, right| right.started_at.cmp(&left.started_at));
    Ok(snapshots)
}

#[tauri::command]
pub fn mod_translation_get_task(
    task_id: String,
) -> Result<Option<TaskSnapshot>, String> {
    let registry =
        tasks().lock().map_err(|_| "任务注册表不可用".to_string())?;
    Ok(registry.get(&task_id).and_then(|handle| {
        handle.snapshot.lock().ok().map(|snapshot| snapshot.clone())
    }))
}

#[tauri::command]
pub fn mod_translation_dismiss_task(task_id: String) -> Result<(), String> {
    let mut registry =
        tasks().lock().map_err(|_| "任务注册表不可用".to_string())?;
    let Some(handle) = registry.get(&task_id) else {
        return Ok(());
    };
    let running = handle
        .snapshot
        .lock()
        .map_err(|_| "任务快照不可用".to_string())?
        .status
        == "running";
    if running {
        return Err("运行中的任务不能移除，请先取消".to_string());
    }
    registry.remove(&task_id);
    Ok(())
}

fn make_progress_emitter<R: Runtime>(
    app: tauri::AppHandle<R>,
    handle: Arc<TaskHandle>,
) -> mod_translation::translate::EventEmitter {
    Arc::new(move |progress: TranslationSample| {
        let report = progress.report.as_deref().and_then(|value| {
            serde_json::from_str::<TranslationReport>(value).ok()
        });
        let error = if progress.finished && !progress.ok {
            Some(parse_failure(
                progress.report.as_deref().unwrap_or(&progress.message),
            ))
        } else {
            None
        };
        emit_task_event(
            &app,
            &handle,
            "progress",
            Some(progress),
            None,
            report,
            error,
        );
    })
}

fn make_repair_emitter<R: Runtime>(
    app: tauri::AppHandle<R>,
    handle: Arc<TaskHandle>,
) -> RepairEmitter {
    Arc::new(move |activity: RepairActivity| {
        emit_task_event(
            &app,
            &handle,
            "activity",
            None,
            Some(activity),
            None,
            None,
        );
    })
}

fn emit_task_event<R: Runtime>(
    app: &tauri::AppHandle<R>,
    handle: &Arc<TaskHandle>,
    event_type: &str,
    progress: Option<TranslationSample>,
    activity: Option<RepairActivity>,
    report: Option<TranslationReport>,
    error: Option<TaskFailure>,
) {
    let Ok(_event_guard) = handle.event_gate.lock() else {
        return;
    };
    let event = {
        let Ok(mut snapshot) = handle.snapshot.lock() else {
            return;
        };
        let sequence = handle.sequence.fetch_add(1, Ordering::SeqCst) + 1;
        let occurred_at = iso_now();
        if let Some(progress) = &progress {
            snapshot.progress = Some(progress.clone());
            if progress.finished {
                snapshot.status =
                    if progress.ok { "completed" } else { "failed" }
                        .to_string();
            }
        }
        if let Some(activity) = &activity {
            snapshot.activities.push(activity.clone());
            if snapshot.activities.len() > MAX_SNAPSHOT_EVENTS {
                snapshot.activities.remove(0);
            }
        }
        if report.is_some() {
            snapshot.report = report.clone();
        }
        if error.is_some() {
            snapshot.error = error.clone();
            snapshot.status = "failed".to_string();
        }
        snapshot.sequence = sequence;
        snapshot.updated_at = occurred_at.clone();
        let event = ModTranslationTaskEvent {
            event_id: format!("EVENT-{}", uuid::Uuid::new_v4()),
            task_id: snapshot.task_id.clone(),
            sequence,
            occurred_at,
            event_type: event_type.to_string(),
            status: snapshot.status.clone(),
            progress,
            activity,
            report,
            error,
        };
        snapshot.events.push(event.clone());
        if snapshot.events.len() > MAX_SNAPSHOT_EVENTS {
            snapshot.events.remove(0);
        }
        event
    };
    if let Err(error) = app.emit(TASK_EVENT, event) {
        tracing::warn!(error = %error, "mod translation task event emit failed");
    }
}

fn ensure_terminal_error<R: Runtime>(
    app: &tauri::AppHandle<R>,
    handle: &Arc<TaskHandle>,
    error: &mod_translation::error::TranslateError,
) {
    let already_terminal = handle
        .snapshot
        .lock()
        .map(|snapshot| {
            snapshot.status != "running" && snapshot.error.is_some()
        })
        .unwrap_or(false);
    if already_terminal {
        return;
    }
    emit_task_event(
        app,
        handle,
        "finished",
        None,
        None,
        None,
        Some(TaskFailure {
            code: error.code.as_str().to_string(),
            message: error.message.clone(),
            details: None,
        }),
    );
}

fn parse_failure(value: &str) -> TaskFailure {
    let (code, message) = value
        .split_once(": ")
        .map(|(code, message)| (code.to_string(), message.to_string()))
        .unwrap_or_else(|| ("UNKNOWN".to_string(), value.to_string()));
    let details = message
        .rsplit_once('；')
        .and_then(|(_, json)| serde_json::from_str::<Value>(json).ok());
    TaskFailure {
        code,
        message,
        details,
    }
}

fn take_prepared_analysis(
    analysis_id: Option<&str>,
    requested_hash: Option<&str>,
    actual_hash: &str,
    input_path: &PathBuf,
) -> Result<Option<PreparedTranslationWorkspace>, String> {
    let Some(analysis_id) = analysis_id else {
        return Ok(None);
    };
    let mut registry = analyses()
        .lock()
        .map_err(|_| "分析注册表不可用".to_string())?;
    prune_analyses(&mut registry);
    let Some(prepared) = registry.remove(analysis_id) else {
        return Ok(None);
    };
    let matches = requested_hash == Some(prepared.input_hash.as_str())
        && prepared.input_hash == actual_hash
        && prepared.input_path == *input_path
        && prepared.workspace.is_dir();
    if !matches {
        let _ = std::fs::remove_dir_all(prepared.workspace);
        return Ok(None);
    }
    Ok(Some(PreparedTranslationWorkspace {
        workspace: prepared.workspace,
        inspection: prepared.inspection,
        input_hash: prepared.input_hash,
    }))
}

fn prune_analyses(registry: &mut HashMap<String, PreparedAnalysis>) {
    let now = SystemTime::now();
    let expired = registry
        .iter()
        .filter_map(|(id, prepared)| {
            now.duration_since(prepared.created_at)
                .ok()
                .filter(|age| *age > ANALYSIS_TTL)
                .map(|_| id.clone())
        })
        .collect::<Vec<_>>();
    for id in expired {
        if let Some(prepared) = registry.remove(&id) {
            let _ = std::fs::remove_dir_all(prepared.workspace);
        }
    }
}

fn claim_locks(task_id: &str, keys: &[String]) -> Result<(), String> {
    let mut locks = active_locks()
        .lock()
        .map_err(|_| "任务锁不可用".to_string())?;
    if let Some(key) = keys.iter().find(|key| locks.contains_key(*key)) {
        return Err(format!("该输出或检查点已有运行中的任务：{key}"));
    }
    for key in keys {
        locks.insert(key.clone(), task_id.to_string());
    }
    Ok(())
}

fn normalized_path_lock_key(path: &std::path::Path) -> String {
    let normalized = path.to_string_lossy().replace('\\', "/");
    if cfg!(windows) {
        normalized.to_lowercase()
    } else {
        normalized
    }
}

fn release_locks(handle: &TaskHandle) {
    let task_id = handle
        .snapshot
        .lock()
        .ok()
        .map(|snapshot| snapshot.task_id.clone())
        .unwrap_or_default();
    release_lock_keys(&task_id, &handle.lock_keys);
}

fn release_lock_keys(task_id: &str, keys: &[String]) {
    if let Ok(mut locks) = active_locks().lock() {
        for key in keys {
            if locks.get(key).is_some_and(|owner| owner == task_id) {
                locks.remove(key);
            }
        }
    }
}

fn new_task_id() -> String {
    let timestamp = chrono::Utc::now().format("%Y%m%dT%H%M%S").to_string();
    format!("TASK-{timestamp}-{}", uuid::Uuid::new_v4().simple())
}

fn iso_now() -> String {
    chrono::Utc::now().to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn handle(task_id: &str, lock_keys: Vec<String>) -> TaskHandle {
        let now = iso_now();
        TaskHandle {
            cancel: Arc::new(AtomicBool::new(false)),
            sequence: AtomicU64::new(0),
            event_gate: Mutex::new(()),
            snapshot: Mutex::new(TaskSnapshot {
                task_id: task_id.to_string(),
                input_path: PathBuf::from("C:/mods/demo.jar"),
                output_path: PathBuf::from("C:/mods/demo-zh_cn.jar"),
                input_hash: "hash".to_string(),
                started_at: now.clone(),
                updated_at: now,
                status: "running".to_string(),
                sequence: 0,
                progress: None,
                activities: Vec::new(),
                report: None,
                error: None,
                events: Vec::new(),
            }),
            lock_keys,
        }
    }

    #[test]
    fn duplicate_output_or_checkpoint_locks_are_rejected_until_release() {
        let key = format!("output:test-{}", uuid::Uuid::new_v4());
        claim_locks("TASK-a", std::slice::from_ref(&key)).unwrap();
        assert!(claim_locks("TASK-b", std::slice::from_ref(&key)).is_err());
        let first = handle("TASK-a", vec![key.clone()]);
        release_locks(&first);
        claim_locks("TASK-b", std::slice::from_ref(&key)).unwrap();
        let second = handle("TASK-b", vec![key]);
        release_locks(&second);
    }

    #[test]
    fn task_failures_preserve_the_backend_error_code() {
        let failure =
            parse_failure("UNSUPPORTED_RESOURCE: assets/demo/data.txt");
        assert_eq!(failure.code, "UNSUPPORTED_RESOURCE");
        assert_eq!(failure.message, "assets/demo/data.txt");
    }
}

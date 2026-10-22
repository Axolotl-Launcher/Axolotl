//! Server process control: starting, stopping, and monitoring the JVM.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use base64::Engine;
use chrono::Utc;
use dashmap::{DashMap, mapref::entry::Entry};
use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};
use std::sync::LazyLock;
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

use crate::event::ServerPayloadType;
use crate::event::emit::emit_server;
use crate::state::{clear_log_buffer, get_log_buffer, push_log_line};
use crate::util::io::IOError;
use crate::util::java_environment;
use crate::{ErrorKind, Result};

use super::files::safe_relative_join;
use super::logs::{
    analyze_exit_reason, stream_server_pty_output, tail_server_log_file,
};
use super::manifest::{
    read_manifest, resolve_jar_name, server_path, write_manifest,
};

const DEFAULT_MEMORY_MB: u32 = 2048;
const STOP_TIMEOUT_SECS: u64 = 60;
const MAX_CONSOLE_ROWS: u16 = 8192;

struct ServerProcess {
    child: tokio::sync::Mutex<Box<dyn portable_pty::Child + Send + Sync>>,
    input: tokio::sync::Mutex<Box<dyn std::io::Write + Send>>,
    pty_master: tokio::sync::Mutex<Box<dyn MasterPty + Send>>,
    stop_requested: AtomicBool,
    ready: AtomicBool,
    stop_command: parking_lot::RwLock<String>,
}

static SERVER_PROCESSES: LazyLock<DashMap<String, Arc<ServerProcess>>> =
    LazyLock::new(DashMap::new);

/// Synchronous start-in-flight guard. Reserving the slot before the first
/// `.await` prevents concurrent `start` calls (e.g. double-clicks) from both
/// passing the running check and spawning two JVMs on the same directory.
static SERVER_STARTING: LazyLock<DashMap<String, CancellationToken>> =
    LazyLock::new(DashMap::new);

pub(super) fn is_running(server_id: &str) -> bool {
    SERVER_PROCESSES.contains_key(server_id)
        || SERVER_STARTING.contains_key(server_id)
}

pub(super) fn is_starting(server_id: &str) -> bool {
    SERVER_STARTING.contains_key(server_id)
        && !SERVER_PROCESSES.contains_key(server_id)
}
pub(super) fn is_stopping(server_id: &str) -> bool {
    SERVER_PROCESSES
        .get(server_id)
        .is_some_and(|process| process.stop_requested.load(Ordering::SeqCst))
        || SERVER_STARTING
            .get(server_id)
            .is_some_and(|token| token.is_cancelled())
}
pub(super) fn has_process(server_id: &str) -> bool {
    SERVER_PROCESSES.contains_key(server_id)
}

pub(super) fn observe_server_output(server_id: &str, line: &str) {
    if line.contains("Starting minecraft server version")
        || (line.contains("Done (") && line.contains("For help"))
    {
        if let Some(process) = SERVER_PROCESSES.get(server_id) {
            process.ready.store(true, Ordering::SeqCst);
        }
    }
}

/// Whether a server process is currently tracked. Used by the log-file tailer
/// to stop following once the server has exited.
pub(super) fn is_server_running(server_id: &str) -> bool {
    SERVER_PROCESSES.contains_key(server_id)
}

/// Ids of every dedicated server that is starting or running; their console
/// buffers must survive maintenance cleanups.
pub(crate) fn running_server_ids() -> Vec<String> {
    SERVER_PROCESSES
        .iter()
        .map(|entry| entry.key().clone())
        .chain(SERVER_STARTING.iter().map(|entry| entry.key().clone()))
        .collect()
}

pub async fn start(
    server_id: &str,
    java_path: Option<String>,
    memory_mb: Option<u32>,
    jvm_args: Option<Vec<String>>,
) -> Result<()> {
    let cancellation = CancellationToken::new();
    match SERVER_STARTING.entry(server_id.to_string()) {
        Entry::Vacant(entry) if !SERVER_PROCESSES.contains_key(server_id) => {
            entry.insert(cancellation.clone());
        }
        _ => {
            return Err(ErrorKind::InputError(
                "Server is already running".to_string(),
            )
            .as_error());
        }
    }
    clear_log_buffer(server_id);
    super::console::reset_console(server_id);
    emit_server(server_id, ServerPayloadType::Starting)
        .await
        .ok();
    let result =
        start_inner(server_id, java_path, memory_mb, jvm_args, &cancellation)
            .await;
    SERVER_STARTING.remove(server_id);
    if !SERVER_PROCESSES.contains_key(server_id) {
        if cancellation.is_cancelled() {
            log_server_step(server_id, "Server startup cancelled").await;
        } else if let Err(error) = &result {
            log_server_step(
                server_id,
                &format!("Server startup failed: {error}"),
            )
            .await;
        }
        emit_server(
            server_id,
            ServerPayloadType::Stopped {
                crashed: false,
                reason: None,
            },
        )
        .await
        .ok();
    }
    if cancellation.is_cancelled() {
        Ok(())
    } else {
        result
    }
}

async fn start_inner(
    server_id: &str,
    java_path: Option<String>,
    memory_mb: Option<u32>,
    jvm_args: Option<Vec<String>>,
    cancellation: &CancellationToken,
) -> Result<()> {
    let dir = server_path(server_id).await?;
    let mut manifest = read_manifest(&dir).await?;
    let launch_args = if manifest.server_type == "forge" {
        forge_launch_args(&dir)?
    } else if manifest.server_type == "neoforge" {
        neoforge_launch_args(
            &dir,
            &manifest.game_version,
            manifest.loader_version.as_deref(),
        )?
    } else {
        let jar_name = resolve_jar_name(&manifest);
        let jar_path = safe_relative_join(&dir, &jar_name)?;
        if !jar_path.exists() {
            return Err(ErrorKind::LauncherError(format!(
                "Server jar not found: {jar_name}. Download the server files first."
            ))
            .as_error());
        }
        vec!["-jar".to_string(), jar_name, "nogui".to_string()]
    };

    let java = java_path
        .or_else(|| manifest.java_path.clone())
        .unwrap_or_else(|| "java".to_string());
    let java = java_environment::console_java_executable(&java)?;
    let memory = memory_mb
        .or(manifest.memory_mb)
        .unwrap_or(DEFAULT_MEMORY_MB);

    if let Some(hook) = manifest.pre_launch_hook.clone() {
        tokio::select! {
            biased;
            _ = cancellation.cancelled() => return Ok(()),
            result = run_pre_launch_hook(server_id, &dir, &hook) => result?,
        }
    }
    if manifest.server_type == "fabric" {
        tokio::select! {
            biased;
            _ = cancellation.cancelled() => return Ok(()),
            result = super::fabric::prepare(server_id, &dir, &manifest, cancellation) => result?,
        }
    }
    if cancellation.is_cancelled() {
        return Ok(());
    }
    // Remove the previous log before spawning Java, never a newly created log.
    let _ = tokio::fs::remove_file(dir.join("logs/latest.log")).await;

    // Ensure eula.txt exists (create with eula=false if missing)
    let eula_path = dir.join("eula.txt");
    let eula_created = !eula_path.exists();
    if eula_created {
        tokio::fs::write(&eula_path, "eula=false\n")
            .await
            .map_err(|e| IOError::with_path(e, &eula_path))?;
    }

    let mut args = vec![format!("-Xmx{memory}M")];
    args.extend(jvm_args.unwrap_or_else(|| manifest.jvm_args.clone()));
    args.extend(launch_args);
    let removed_environment = [
        "DYLD_LIBRARY_PATH",
        "DYLD_FALLBACK_LIBRARY_PATH",
        "DYLD_FRAMEWORK_PATH",
        "DYLD_FALLBACK_FRAMEWORK_PATH",
        "DYLD_INSERT_LIBRARIES",
    ];

    let pair = native_pty_system()
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| {
            ErrorKind::LauncherError(format!(
                "Failed to create server console PTY: {e}"
            ))
            .as_error()
        })?;
    let reader = pair.master.try_clone_reader().map_err(|e| {
        ErrorKind::LauncherError(format!(
            "Failed to capture server console output: {e}"
        ))
        .as_error()
    })?;
    let writer = pair.master.take_writer().map_err(|e| {
        ErrorKind::LauncherError(format!(
            "Failed to capture server console input: {e}"
        ))
        .as_error()
    })?;
    let mut command = CommandBuilder::new(&java);
    java_environment::configure_pty_java_environment(&mut command);
    command.args(&args);
    command.cwd(&dir);
    command.env("TERM", "xterm-256color");
    command.env("COLORTERM", "truecolor");
    for variable in removed_environment {
        command.env_remove(variable);
    }
    let mut child = pair.slave.spawn_command(command).map_err(|e| {
        ErrorKind::LauncherError(format!("Failed to start server process: {e}"))
            .as_error()
    })?;
    drop(pair.slave);

    manifest.last_started_at = Some(Utc::now());
    manifest.last_exit_crashed = false;
    if let Err(error) = write_manifest(&dir, &manifest).await {
        let _ = child.kill();
        return Err(error);
    }

    log_server_step(
        server_id,
        &format!(
            "Starting server '{}' ({} · Minecraft {})",
            manifest.name, manifest.server_type, manifest.game_version,
        ),
    )
    .await;
    log_server_step(server_id, &format!("Java: {}", java.display())).await;
    log_server_step(server_id, &format!("Memory: {memory} MB")).await;
    if eula_created {
        log_server_step(
            server_id,
            "eula.txt not found — created with eula=false. Accept the EULA to start the server.",
        )
        .await;
    }
    log_server_step(
        server_id,
        &format!(
            "Launching {} server ({} · nogui)",
            manifest.server_type,
            resolve_jar_name(&manifest),
        ),
    )
    .await;
    let process = Arc::new(ServerProcess {
        child: tokio::sync::Mutex::new(child),
        input: tokio::sync::Mutex::new(writer),
        pty_master: tokio::sync::Mutex::new(pair.master),
        stop_requested: AtomicBool::new(false),
        ready: AtomicBool::new(false),
        stop_command: parking_lot::RwLock::new(manifest.stop_command.clone()),
    });
    SERVER_PROCESSES.insert(server_id.to_string(), process.clone());

    tokio::spawn(stream_server_pty_output(server_id.to_string(), reader));
    // Keep the structured log view available independently of terminal rendering.
    tokio::spawn(tail_server_log_file(server_id.to_string(), dir.clone()));
    tokio::spawn(monitor_server_process(
        server_id.to_string(),
        dir,
        process.clone(),
    ));

    if cancellation.is_cancelled() {
        process.stop_requested.store(true, Ordering::SeqCst);
        process.child.lock().await.kill()?;
    }
    emit_server(server_id, ServerPayloadType::Started)
        .await
        .ok();
    Ok(())
}

/// Runs the manifest's pre-launch hook before the server JVM starts. The hook
/// is a shell command executed with the server directory as its working
/// directory; a non-zero exit aborts the start so callers can surface the
/// failure.
async fn run_pre_launch_hook(
    server_id: &str,
    dir: &Path,
    hook: &str,
) -> Result<()> {
    let mut parts = shlex::split(hook).ok_or_else(|| {
        ErrorKind::InputError("Invalid pre-launch hook".to_string()).as_error()
    })?;
    if parts.is_empty() {
        return Ok(());
    }
    let program = parts.remove(0);
    push_log_line(server_id, format!("[hook] Running pre-launch hook: {hook}"));
    let mut command = Command::new(&program);
    java_environment::configure_child_environment(command.as_std_mut());
    let status = command
        .args(&parts)
        .current_dir(dir)
        .kill_on_drop(true)
        .status()
        .await
        .map_err(|e| {
            ErrorKind::LauncherError(format!(
                "Failed to run pre-launch hook: {e}"
            ))
            .as_error()
        })?;
    if !status.success() {
        let code = status
            .code()
            .map(|code| code.to_string())
            .unwrap_or_else(|| "terminated".to_string());
        return Err(ErrorKind::LauncherError(format!(
            "Pre-launch hook failed with exit code {code}"
        ))
        .as_error());
    }
    push_log_line(server_id, "[hook] Pre-launch hook finished".to_string());
    Ok(())
}

async fn write_console(
    process: Arc<ServerProcess>,
    bytes: Vec<u8>,
) -> std::io::Result<()> {
    // PTY writes can block under backpressure; never block the async runtime.
    tokio::task::spawn_blocking(move || {
        let mut writer = process.input.blocking_lock();
        writer.write_all(&bytes)?;
        writer.flush()
    })
    .await
    .map_err(std::io::Error::other)?
}

fn command_bytes(command: &str) -> Vec<u8> {
    format!("{}\r", command.trim_end_matches(['\r', '\n'])).into_bytes()
}

fn stop_command_bytes(command: &str) -> Result<Vec<u8>> {
    let command = super::manifest::normalize_stop_command(command)?;
    Ok(if command == "^C" {
        vec![0x03]
    } else {
        command_bytes(&command)
    })
}

pub(super) fn update_stop_command(server_id: &str, command: &str) {
    if let Some(process) = SERVER_PROCESSES.get(server_id) {
        *process.stop_command.write() = command.to_string();
    }
}

pub async fn send_console_input(server_id: &str, data: &str) -> Result<()> {
    let process = SERVER_PROCESSES
        .get(server_id)
        .map(|entry| entry.value().clone())
        .ok_or_else(|| {
            ErrorKind::InputError("Server is not running".to_string())
                .as_error()
        })?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|e| {
            ErrorKind::InputError(format!("Invalid console input: {e}"))
                .as_error()
        })?;
    if bytes.len() > 64 * 1024 {
        return Err(ErrorKind::InputError(
            "Console input exceeds 64 KiB".to_string(),
        )
        .as_error());
    }
    write_console(process, bytes).await.map_err(|e| {
        ErrorKind::LauncherError(format!("Failed to send command: {e}"))
            .as_error()
    })?;
    Ok(())
}

pub async fn resize_console(
    server_id: &str,
    cols: u16,
    rows: u16,
) -> Result<()> {
    let process = SERVER_PROCESSES
        .get(server_id)
        .map(|entry| entry.value().clone())
        .ok_or_else(|| {
            ErrorKind::InputError("Server is not running".to_string())
                .as_error()
        })?;
    process
        .pty_master
        .lock()
        .await
        .resize(PtySize {
            rows: rows.clamp(1, MAX_CONSOLE_ROWS),
            cols: cols.clamp(2, 500),
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| {
            ErrorKind::LauncherError(format!(
                "Failed to resize server console: {e}"
            ))
            .as_error()
        })?;
    Ok(())
}

pub async fn stop(server_id: &str) -> Result<()> {
    let cancelled_start = if let Some(starting) = SERVER_STARTING.get(server_id)
    {
        starting.cancel();
        true
    } else {
        false
    };
    let process = SERVER_PROCESSES
        .get(server_id)
        .map(|entry| entry.value().clone());
    let Some(process) = process else {
        if cancelled_start {
            emit_server(server_id, ServerPayloadType::Stopping)
                .await
                .ok();
            return Ok(());
        }
        return Err(ErrorKind::InputError("Server is not running".to_string())
            .as_error());
    };
    let command = process.stop_command.read().clone();
    let bytes = stop_command_bytes(&command)?;
    if process.stop_requested.swap(true, Ordering::SeqCst) {
        return Ok(());
    }
    emit_server(server_id, ServerPayloadType::Stopping)
        .await
        .ok();
    if !process.ready.load(Ordering::SeqCst) && command == "stop" {
        // A bootstrapper cannot read Minecraft commands before the server starts.
        process.child.lock().await.kill()?;
        return Ok(());
    }
    let watchdog = process.clone();
    let id = server_id.to_string();
    // Arm this before writing: a full PTY input pipe must not prevent the timeout.
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(STOP_TIMEOUT_SECS))
            .await;
        let current =
            SERVER_PROCESSES.get(&id).map(|entry| entry.value().clone());
        if current
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, &watchdog))
        {
            let _ = watchdog.child.lock().await.kill();
        }
    });
    tokio::spawn(async move {
        // Raw editing controls (such as Ctrl+U) become literal input on cooked consoles.
        if write_console(process.clone(), bytes).await.is_err() {
            let _ = process.child.lock().await.kill();
        }
    });
    Ok(())
}

pub async fn kill(server_id: &str) -> Result<()> {
    let cancelled = if let Some(starting) = SERVER_STARTING.get(server_id) {
        starting.cancel();
        true
    } else {
        false
    };
    let process = SERVER_PROCESSES
        .get(server_id)
        .map(|entry| entry.value().clone());
    let Some(process) = process else {
        if cancelled {
            return Ok(());
        }
        return Err(ErrorKind::InputError("Server is not running".to_string())
            .as_error());
    };
    process.stop_requested.store(true, Ordering::SeqCst);
    emit_server(server_id, ServerPayloadType::Stopping)
        .await
        .ok();
    // A blocked input pipe must never prevent a forced termination.
    let _ = tokio::time::timeout(
        std::time::Duration::from_millis(250),
        write_console(process.clone(), vec![0x03]),
    )
    .await;
    let mut child = process.child.lock().await;
    if child.try_wait()?.is_none() {
        child.kill()?;
    }
    Ok(())
}

/// Gracefully stops every running dedicated server and kills whatever is
/// still alive when the shutdown budget expires. Returns how many processes
/// had to be killed.
pub async fn shutdown_all() -> usize {
    const SHUTDOWN_BUDGET: std::time::Duration =
        std::time::Duration::from_secs(5);

    let server_ids: Vec<String> = SERVER_PROCESSES
        .iter()
        .map(|entry| entry.key().clone())
        .collect();
    for server_id in &server_ids {
        // Reuse configured stop commands and non-blocking PTY writes.
        let _ = stop(server_id).await;
    }

    let deadline = tokio::time::Instant::now() + SHUTDOWN_BUDGET;
    while !SERVER_PROCESSES.is_empty() {
        if tokio::time::Instant::now() >= deadline {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    let remaining: Vec<Arc<ServerProcess>> = SERVER_PROCESSES
        .iter()
        .map(|entry| entry.value().clone())
        .collect();
    let mut killed = 0;
    for process in remaining {
        process.stop_requested.store(true, Ordering::SeqCst);
        let mut child = process.child.lock().await;
        let was_running = child
            .try_wait()
            .map(|status| status.is_none())
            .unwrap_or(true);
        if was_running && child.kill().is_ok() {
            killed += 1;
        }
    }
    killed
}

async fn monitor_server_process(
    server_id: String,
    dir: PathBuf,
    process: Arc<ServerProcess>,
) {
    loop {
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        let exit_status = {
            let mut child = process.child.lock().await;
            match child.try_wait() {
                Ok(Some(status)) => Some(status.success()),
                Ok(None) => continue,
                Err(error) => {
                    tracing::warn!(
                        %error,
                        server_id = %server_id,
                        "Failed to poll server process; keeping it registered"
                    );
                    continue;
                }
            }
        };

        SERVER_PROCESSES.remove(&server_id);
        let stop_requested = process.stop_requested.load(Ordering::SeqCst);
        let eula_accepted = read_eula_accepted(&dir).await;
        let crashed = exit_status
            .map(|success| !success && !stop_requested && eula_accepted)
            .unwrap_or(false);

        // Classify self-exits from the tail of the console output so the UI
        // can react (e.g. offer the EULA dialog). User-requested stops and
        // unmatched exits stay unclassified. The brief settle wait lets the
        // output-stream tasks flush their final lines into the buffer first.
        let reason = if stop_requested {
            None
        } else {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            analyze_exit_reason(&get_log_buffer(&server_id))
        };

        if let Ok(mut manifest) = read_manifest(&dir).await {
            manifest.last_exit_crashed = crashed;
            let _ = write_manifest(&dir, &manifest).await;
        }

        emit_server(&server_id, ServerPayloadType::Stopped { crashed, reason })
            .await
            .ok();
        return;
    }
}

/// Builds the JVM launch arguments for a Forge server. Modern Forge (1.17+)
/// ships `@args` files that enumerate the classpath and main class; legacy Forge
/// (<=1.16) produces a single runnable `forge-*.jar`.
fn forge_launch_args(dir: &Path) -> Result<Vec<String>> {
    let forge_dir = dir
        .join("libraries")
        .join("net")
        .join("minecraftforge")
        .join("forge");
    if let Ok(entries) = std::fs::read_dir(&forge_dir) {
        let args_file = if cfg!(windows) {
            "win_args.txt"
        } else {
            "unix_args.txt"
        };
        for entry in entries.flatten() {
            let candidate = entry.path().join(args_file);
            if candidate.is_file() {
                let mut args = Vec::new();
                if dir.join("user_jvm_args.txt").exists() {
                    args.push("@user_jvm_args.txt".to_string());
                }
                args.push(format!("@{}", candidate.to_string_lossy()));
                args.push("nogui".to_string());
                return Ok(args);
            }
        }
    }
    if let Some(jar) = find_forge_jar(dir) {
        return Ok(vec!["-jar".to_string(), jar, "nogui".to_string()]);
    }
    Err(ErrorKind::LauncherError(
        "Forge server files are missing. Reinstall the server.".to_string(),
    )
    .as_error())
}

/// Use the recorded build, rather than whichever version directory is found first.
pub(super) fn neoforge_launch_args(
    dir: &Path,
    game_version: &str,
    loader_version: Option<&str>,
) -> Result<Vec<String>> {
    let build = loader_version.ok_or_else(|| {
        ErrorKind::LauncherError(
            "NeoForge version is missing. Reinstall the server.".to_string(),
        )
        .as_error()
    })?;
    let (artifact, version) = if game_version == "1.20.1" {
        ("forge", format!("{game_version}-{build}"))
    } else {
        ("neoforge", build.to_string())
    };
    let relative = format!(
        "libraries/net/neoforged/{artifact}/{version}/{}",
        if cfg!(windows) {
            "win_args.txt"
        } else {
            "unix_args.txt"
        }
    );
    let candidate = safe_relative_join(dir, &relative)?;
    if !candidate.is_file() {
        return Err(ErrorKind::LauncherError(
            "NeoForge server files are missing. Reinstall the server."
                .to_string(),
        )
        .as_error());
    }
    let mut args = Vec::new();
    if dir.join("user_jvm_args.txt").is_file() {
        args.push("@user_jvm_args.txt".to_string());
    }
    args.push(format!("@{relative}"));
    args.push("nogui".to_string());
    Ok(args)
}

fn find_forge_jar(dir: &Path) -> Option<String> {
    let entry = std::fs::read_dir(dir).ok()?.flatten().find(|e| {
        e.file_name().to_string_lossy().starts_with("forge-")
            && e.path().extension().is_some_and(|ext| ext == "jar")
    })?;
    Some(entry.file_name().to_string_lossy().into_owned())
}

async fn read_eula_accepted(dir: &Path) -> bool {
    match tokio::fs::read_to_string(dir.join("eula.txt")).await {
        Ok(text) => text
            .lines()
            .find_map(|line| line.split_once('='))
            .filter(|(key, _)| key.trim() == "eula")
            .is_some_and(|(_, value)| {
                value.trim().eq_ignore_ascii_case("true")
            }),
        Err(_) => false,
    }
}

/// Emits a timestamped, info-level line to the server console: it is both
/// persisted to the log buffer and pushed as a live `Log` event, so startup
/// progress is visible even before the JVM produces any output of its own.
pub(super) async fn log_server_step(server_id: &str, message: &str) {
    let line = format!(
        "{} [Axolotl/INFO]: {}",
        chrono::Local::now().format("%H:%M:%S"),
        message,
    );
    push_log_line(server_id, line.clone());
    super::console::publish_console_output(
        server_id,
        format!("{line}\r\n").as_bytes(),
    )
    .await;
    emit_server(server_id, ServerPayloadType::Log { line })
        .await
        .ok();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neoforge_launches_only_the_recorded_build() {
        let temp = tempfile::tempdir().unwrap();
        let args_file = if cfg!(windows) {
            "win_args.txt"
        } else {
            "unix_args.txt"
        };
        for (game, build, relative) in [
            (
                "1.21.1",
                "21.1.172",
                "libraries/net/neoforged/neoforge/21.1.172",
            ),
            (
                "1.20.1",
                "47.1.106",
                "libraries/net/neoforged/forge/1.20.1-47.1.106",
            ),
        ] {
            let path = temp.path().join(relative);
            std::fs::create_dir_all(&path).unwrap();
            std::fs::write(path.join(args_file), "MainClass").unwrap();
            std::fs::write(
                temp.path().join("user_jvm_args.txt"),
                "-Dtest=true",
            )
            .unwrap();
            assert_eq!(
                neoforge_launch_args(temp.path(), game, Some(build)).unwrap(),
                vec![
                    "@user_jvm_args.txt".to_string(),
                    format!("@{relative}/{args_file}"),
                    "nogui".to_string(),
                ]
            );
        }
        assert!(
            neoforge_launch_args(temp.path(), "1.21.1", Some("21.1.999"))
                .is_err()
        );
        assert!(neoforge_launch_args(temp.path(), "1.21.1", None).is_err());
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn javaw_selection_connects_java_stdin_stdout_and_stderr_to_the_server_pty()
     {
        // Exercise the real runtime when a JDK is available, rather than using
        // a shell process as a substitute for Java's console behavior.
        let windowed = std::env::var_os("PATH").and_then(|paths| {
            std::env::split_paths(&paths)
                .map(|directory| directory.join("javaw.exe"))
                .find(|path| {
                    path.is_file() && path.with_file_name("javac.exe").is_file()
                })
        });
        let Some(windowed) = windowed else {
            eprintln!("Java PTY integration requires a JDK on PATH");
            return;
        };
        let java = java_environment::console_java_executable(
            windowed.to_str().unwrap(),
        )
        .unwrap();
        assert_eq!(java, windowed.with_file_name("java.exe"));
        let temporary = tempfile::tempdir().unwrap();
        let source = temporary.path().join("ConsoleProbe.java");
        std::fs::write(&source, r#"
import java.io.BufferedReader;
import java.io.InputStreamReader;
class ConsoleProbe {
    public static void main(String[] args) throws Exception {
        System.out.println("JAVA-STDOUT-READY");
        System.err.println("JAVA-STDERR-READY");
        String command = new BufferedReader(new InputStreamReader(System.in)).readLine();
        if (!"say hello".equals(command)) throw new AssertionError(command);
        System.out.println("JAVA-STDIN-ACK:" + command);
    }
}
"#).unwrap();
        let pair = native_pty_system().openpty(PtySize::default()).unwrap();
        let reader = pair.master.try_clone_reader().unwrap();
        let writer = pair.master.take_writer().unwrap();
        let mut command = CommandBuilder::new(&java);
        java_environment::configure_pty_java_environment(&mut command);
        command.arg(&source);
        command.cwd(temporary.path());
        let child = pair.slave.spawn_command(command).unwrap();
        drop(pair.slave);
        let process = Arc::new(ServerProcess {
            child: tokio::sync::Mutex::new(child),
            input: tokio::sync::Mutex::new(writer),
            pty_master: tokio::sync::Mutex::new(pair.master),
            stop_requested: AtomicBool::new(false),
            ready: AtomicBool::new(false),
            stop_command: parking_lot::RwLock::new("stop".to_string()),
        });
        let id = "test-java-pty-stdio";
        SERVER_PROCESSES.insert(id.to_string(), process.clone());
        super::super::console::reset_console(id);
        let output =
            tokio::spawn(stream_server_pty_output(id.to_string(), reader));
        let result =
            tokio::time::timeout(std::time::Duration::from_secs(30), async {
                let mut sent = false;
                loop {
                    let delta = serde_json::to_value(
                        super::super::console::get_console_delta(id, None),
                    )
                    .unwrap();
                    let bytes = base64::engine::general_purpose::STANDARD
                        .decode(delta["data"].as_str().unwrap())
                        .unwrap();
                    let text = String::from_utf8_lossy(&bytes);
                    if !sent
                        && text.contains("JAVA-STDOUT-READY")
                        && text.contains("JAVA-STDERR-READY")
                    {
                        send_console_input(
                            id,
                            &base64::engine::general_purpose::STANDARD
                                .encode(command_bytes("say hello")),
                        )
                        .await
                        .unwrap();
                        sent = true;
                    }
                    if text.contains("JAVA-STDIN-ACK:say hello") {
                        return;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(10))
                        .await;
                }
            })
            .await;
        let delta = serde_json::to_value(
            super::super::console::get_console_delta(id, None),
        )
        .unwrap();
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(delta["data"].as_str().unwrap())
            .unwrap();
        let _ = process.child.lock().await.kill();
        SERVER_PROCESSES.remove(id);
        drop(process);
        let _ = tokio::time::timeout(std::time::Duration::from_secs(5), output)
            .await;
        super::super::console::remove_console(id);
        result.unwrap_or_else(|error| {
            panic!(
                "Java PTY failed: {error}; output: {}",
                String::from_utf8_lossy(&bytes)
            )
        });
    }

    #[derive(Debug, Clone)]
    struct TestChild {
        killed: Arc<AtomicBool>,
        gate: Option<Arc<(std::sync::Mutex<bool>, std::sync::Condvar)>>,
    }

    impl portable_pty::ChildKiller for TestChild {
        fn kill(&mut self) -> std::io::Result<()> {
            self.killed.store(true, Ordering::SeqCst);
            if let Some(gate) = &self.gate {
                *gate.0.lock().unwrap() = false;
                gate.1.notify_all();
            }
            Ok(())
        }
        fn clone_killer(
            &self,
        ) -> Box<dyn portable_pty::ChildKiller + Send + Sync> {
            Box::new(self.clone())
        }
    }

    impl portable_pty::Child for TestChild {
        fn try_wait(
            &mut self,
        ) -> std::io::Result<Option<portable_pty::ExitStatus>> {
            Ok(self
                .killed
                .load(Ordering::SeqCst)
                .then(|| portable_pty::ExitStatus::with_exit_code(0)))
        }
        fn wait(&mut self) -> std::io::Result<portable_pty::ExitStatus> {
            Ok(portable_pty::ExitStatus::with_exit_code(0))
        }
        fn process_id(&self) -> Option<u32> {
            None
        }
        #[cfg(windows)]
        fn as_raw_handle(&self) -> Option<std::os::windows::io::RawHandle> {
            None
        }
    }

    struct TestWriter {
        bytes: Arc<std::sync::Mutex<Vec<u8>>>,
        gate: Option<Arc<(std::sync::Mutex<bool>, std::sync::Condvar)>>,
    }

    impl std::io::Write for TestWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if let Some(gate) = &self.gate {
                let mut blocked = gate.0.lock().unwrap();
                while *blocked {
                    blocked = gate.1.wait(blocked).unwrap();
                }
            }
            self.bytes.lock().unwrap().extend(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn tracked_process(
        id: &str,
        ready: bool,
        blocked: bool,
    ) -> (
        Arc<ServerProcess>,
        TestChild,
        Arc<std::sync::Mutex<Vec<u8>>>,
    ) {
        let pair = native_pty_system().openpty(PtySize::default()).unwrap();
        let gate = blocked.then(|| {
            Arc::new((std::sync::Mutex::new(true), std::sync::Condvar::new()))
        });
        let child = TestChild {
            killed: Arc::new(AtomicBool::new(false)),
            gate: gate.clone(),
        };
        let bytes = Arc::new(std::sync::Mutex::new(Vec::new()));
        let process = Arc::new(ServerProcess {
            child: tokio::sync::Mutex::new(Box::new(child.clone())),
            input: tokio::sync::Mutex::new(Box::new(TestWriter {
                bytes: bytes.clone(),
                gate,
            })),
            pty_master: tokio::sync::Mutex::new(pair.master),
            stop_requested: AtomicBool::new(false),
            ready: AtomicBool::new(ready),
            stop_command: parking_lot::RwLock::new("stop".to_string()),
        });
        SERVER_PROCESSES.insert(id.to_string(), process.clone());
        (process, child, bytes)
    }

    #[tokio::test]
    async fn stop_cancels_download_preparation_before_a_jvm_exists() {
        let id = "test-stop-preparation";
        let cancellation = CancellationToken::new();
        SERVER_STARTING.insert(id.to_string(), cancellation.clone());
        assert!(is_running(id));
        assert!(is_starting(id));
        assert!(!has_process(id));
        let result = stop(id).await;
        SERVER_STARTING.remove(id);
        result.unwrap();
        assert!(cancellation.is_cancelled());
        assert!(!is_running(id));
    }

    #[tokio::test]
    async fn stop_terminates_a_bootstrapper_that_cannot_accept_server_commands()
    {
        let id = "test-stop-bootstrapper";
        let (_, child, bytes) = tracked_process(id, false, false);
        let result = stop(id).await;
        SERVER_PROCESSES.remove(id);
        result.unwrap();
        assert!(child.killed.load(Ordering::SeqCst));
        assert!(bytes.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn ready_server_receives_a_plain_stop_command_without_raw_editor_controls()
     {
        let id = "test-stop-ready";
        let (process, child, bytes) = tracked_process(id, false, false);
        observe_server_output(
            id,
            "[Server thread/INFO]: Done (1.2s)! For help, type \"help\"",
        );
        assert!(process.ready.load(Ordering::SeqCst));
        stop(id).await.unwrap();
        let result =
            tokio::time::timeout(std::time::Duration::from_secs(2), async {
                while bytes.lock().unwrap().is_empty() {
                    tokio::time::sleep(std::time::Duration::from_millis(1))
                        .await;
                }
            })
            .await;
        SERVER_PROCESSES.remove(id);
        result.unwrap();
        assert_eq!(*bytes.lock().unwrap(), b"stop\r");
        assert!(!child.killed.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn stop_returns_even_when_the_pty_input_pipe_is_blocked() {
        use portable_pty::ChildKiller;
        let id = "test-stop-backpressure";
        let (_, mut child, _) = tracked_process(id, true, true);
        let result = tokio::time::timeout(
            std::time::Duration::from_millis(100),
            stop(id),
        )
        .await;
        // Always release the task-owned fake pipe, including when this regression fails.
        child.kill().unwrap();
        SERVER_PROCESSES.remove(id);
        result.unwrap().unwrap();
    }

    #[test]
    fn native_pty_accepts_enter_and_reports_resized_dimensions() {
        use std::io::{Read, Write};
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .unwrap();
        pair.master
            .resize(PtySize {
                rows: 35,
                cols: 110,
                pixel_width: 0,
                pixel_height: 0,
            })
            .unwrap();
        let size = pair.master.get_size().unwrap();
        assert_eq!((size.rows, size.cols), (35, 110));
        #[cfg(windows)]
        let command = {
            let mut command = CommandBuilder::new("powershell.exe");
            command.args(["-NoProfile", "-NonInteractive", "-Command", "$line = [Console]::ReadLine(); [Console]::WriteLine('PTY-ACK:' + $line)"]);
            command
        };
        #[cfg(not(windows))]
        let command = {
            let mut command = CommandBuilder::new("sh");
            command
                .args(["-c", "read -r line; printf 'PTY-ACK:%s\\n' \"$line\""]);
            command
        };
        let mut reader = pair.master.try_clone_reader().unwrap();
        let mut writer = pair.master.take_writer().unwrap();
        let mut child = pair.slave.spawn_command(command).unwrap();
        drop(pair.slave);
        let (sender, receiver) = std::sync::mpsc::channel();
        let reader_task = std::thread::spawn(move || {
            let mut bytes = [0u8; 4096];
            while let Ok(count) = reader.read(&mut bytes) {
                if count == 0 || sender.send(bytes[..count].to_vec()).is_err() {
                    break;
                }
            }
        });
        writer.write_all(&command_bytes("say hello")).unwrap();
        writer.flush().unwrap();
        let mut handshake =
            super::super::console::ConsoleHandshake::new(cfg!(windows));
        let deadline =
            std::time::Instant::now() + std::time::Duration::from_secs(15);
        let mut output = Vec::new();
        while let Some(remaining) =
            deadline.checked_duration_since(std::time::Instant::now())
        {
            let Ok(bytes) = receiver.recv_timeout(remaining) else {
                break;
            };
            let (bytes, reply) = handshake.consume(&bytes);
            if reply {
                writer.write_all(b"\x1b[1;1R").unwrap();
                writer.write_all(&command_bytes("say hello")).unwrap();
                writer.flush().unwrap();
            }
            output.extend(bytes);
            if String::from_utf8_lossy(&output).contains("PTY-ACK:say hello") {
                break;
            }
        }
        let _ = child.kill();
        let _ = child.wait();
        drop(writer);
        drop(pair.master);
        drop(receiver);
        reader_task.join().unwrap();
        assert!(
            String::from_utf8_lossy(&output).contains("PTY-ACK:say hello"),
            "{}",
            String::from_utf8_lossy(&output)
        );
    }

    #[test]
    fn whole_line_commands_use_terminal_enter_without_duplicate_terminators() {
        assert_eq!(command_bytes("say hello"), b"say hello\r");
        assert_eq!(command_bytes("say hello\r\n"), b"say hello\r");
    }

    #[test]
    fn stop_commands_support_literal_commands_and_case_insensitive_ctrl_c() {
        assert_eq!(
            stop_command_bytes("shutdown now").unwrap(),
            b"shutdown now\r"
        );
        for value in ["^C", "^c", " ^c "] {
            assert_eq!(stop_command_bytes(value).unwrap(), b"\x03");
        }
        assert_eq!(stop_command_bytes("").unwrap(), b"stop\r");
        assert!(stop_command_bytes("stop\nkill").is_err());
    }

    #[tokio::test]
    async fn configured_stop_command_updates_the_running_process_and_sends_ctrl_c()
     {
        let id = "test-custom-stop";
        let (process, child, bytes) = tracked_process(id, false, false);
        update_stop_command(id, "^c");
        stop(id).await.unwrap();
        let result =
            tokio::time::timeout(std::time::Duration::from_secs(2), async {
                while bytes.lock().unwrap().is_empty() {
                    tokio::task::yield_now().await;
                }
            })
            .await;
        SERVER_PROCESSES.remove(id);
        result.unwrap();
        assert_eq!(*bytes.lock().unwrap(), b"\x03");
        assert!(!child.killed.load(Ordering::SeqCst));
        assert!(process.stop_requested.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn terminate_sends_ctrl_c_before_killing_the_process() {
        let id = "test-force-terminate";
        let (_, child, bytes) = tracked_process(id, true, false);
        let result = kill(id).await;
        SERVER_PROCESSES.remove(id);
        result.unwrap();
        assert_eq!(*bytes.lock().unwrap(), b"\x03");
        assert!(child.killed.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn terminate_cannot_be_blocked_by_a_stuck_pty_write() {
        let id = "test-force-backpressure";
        let (_, child, _) = tracked_process(id, true, true);
        let result =
            tokio::time::timeout(std::time::Duration::from_secs(2), kill(id))
                .await;
        if result.is_err() {
            use portable_pty::ChildKiller;
            let mut child = child.clone();
            child.kill().unwrap();
        }
        SERVER_PROCESSES.remove(id);
        result.unwrap().unwrap();
        assert!(child.killed.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn shutdown_all_returns_directly_without_running_servers() {
        let killed = tokio::time::timeout(
            std::time::Duration::from_millis(500),
            shutdown_all(),
        )
        .await
        .expect("shutdown_all must not wait when no servers are tracked");

        assert_eq!(killed, 0);
    }

    #[tokio::test]
    async fn shutdown_all_uses_configured_stop_commands() {
        let id = "test-shutdown-custom-stop";
        let (_, child, bytes) = tracked_process(id, true, false);
        update_stop_command(id, "^c");
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(7),
            shutdown_all(),
        )
        .await;
        SERVER_PROCESSES.remove(id);
        assert_eq!(result.unwrap(), 1);
        assert_eq!(*bytes.lock().unwrap(), b"\x03");
        assert!(child.killed.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn shutdown_all_cannot_be_blocked_by_a_stuck_pty_write() {
        let id = "test-shutdown-backpressure";
        let (_, mut child, _) = tracked_process(id, true, true);
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(7),
            shutdown_all(),
        )
        .await;
        SERVER_PROCESSES.remove(id);
        // Release the fake pipe even when the shutdown regression fails.
        use portable_pty::ChildKiller;
        child.kill().unwrap();
        assert_eq!(result.unwrap(), 1);
        assert!(child.killed.load(Ordering::SeqCst));
    }
}

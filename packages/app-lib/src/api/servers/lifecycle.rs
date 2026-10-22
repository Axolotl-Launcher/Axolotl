//! Server process control: starting, stopping, and monitoring the JVM.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use base64::Engine;
use chrono::Utc;
use dashmap::{DashMap, DashSet};
use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};
use std::sync::LazyLock;
use tokio::io::AsyncWriteExt;
use tokio::process::{Child, ChildStdin, Command};

use crate::event::ServerPayloadType;
use crate::event::emit::emit_server;
use crate::state::{clear_log_buffer, get_log_buffer, push_log_line};
use crate::util::io::IOError;
use crate::util::java_environment;
use crate::{ErrorKind, Result};

use super::files::safe_relative_join;
use super::logs::{
    analyze_exit_reason, stream_server_output, stream_server_pty_output,
    tail_server_log_file,
};
use super::manifest::{
    read_manifest, resolve_jar_name, server_path, write_manifest,
};

const DEFAULT_MEMORY_MB: u32 = 2048;
const STOP_TIMEOUT_SECS: u64 = 60;
const MAX_CONSOLE_ROWS: u16 = 8192;

struct ServerProcess {
    child: tokio::sync::Mutex<ServerChild>,
    input: tokio::sync::Mutex<ServerInput>,
    pty_master: Option<tokio::sync::Mutex<Box<dyn MasterPty + Send>>>,
    stop_requested: AtomicBool,
}

enum ServerChild {
    Piped(Child),
    Pty(Box<dyn portable_pty::Child + Send + Sync>),
}

enum ServerInput {
    Piped(ChildStdin),
    Pty(Box<dyn std::io::Write + Send>),
}

impl ServerInput {
    async fn write_all(&mut self, data: &[u8]) -> std::io::Result<()> {
        match self {
            Self::Piped(stdin) => {
                stdin.write_all(data).await?;
                stdin.flush().await
            }
            Self::Pty(writer) => {
                writer.write_all(data)?;
                writer.flush()
            }
        }
    }
}

impl ServerChild {
    fn try_wait(&mut self) -> std::io::Result<Option<bool>> {
        match self {
            Self::Piped(child) => {
                child.try_wait().map(|status| status.map(|s| s.success()))
            }
            Self::Pty(child) => {
                child.try_wait().map(|status| status.map(|s| s.success()))
            }
        }
    }

    async fn kill(&mut self) -> std::io::Result<()> {
        match self {
            Self::Piped(child) => child.kill().await,
            Self::Pty(child) => child.kill(),
        }
    }
}

static SERVER_PROCESSES: LazyLock<DashMap<String, Arc<ServerProcess>>> =
    LazyLock::new(DashMap::new);

/// Synchronous start-in-flight guard. Reserving the slot before the first
/// `.await` prevents concurrent `start` calls (e.g. double-clicks) from both
/// passing the running check and spawning two JVMs on the same directory.
static SERVER_STARTING: LazyLock<DashSet<String>> = LazyLock::new(DashSet::new);

pub(super) fn is_running(server_id: &str) -> bool {
    SERVER_PROCESSES.contains_key(server_id)
}

/// Whether a server process is currently tracked. Used by the log-file tailer
/// to stop following once the server has exited.
pub(super) fn is_server_running(server_id: &str) -> bool {
    SERVER_PROCESSES.contains_key(server_id)
}

pub async fn start(
    server_id: &str,
    java_path: Option<String>,
    memory_mb: Option<u32>,
    jvm_args: Option<Vec<String>>,
) -> Result<()> {
    if SERVER_PROCESSES.contains_key(server_id)
        || !SERVER_STARTING.insert(server_id.to_string())
    {
        return Err(ErrorKind::InputError(
            "Server is already running".to_string(),
        )
        .as_error());
    }
    let result = start_inner(server_id, java_path, memory_mb, jvm_args).await;
    SERVER_STARTING.remove(server_id);
    result
}

async fn start_inner(
    server_id: &str,
    java_path: Option<String>,
    memory_mb: Option<u32>,
    jvm_args: Option<Vec<String>>,
) -> Result<()> {
    let dir = server_path(server_id).await?;
    let mut manifest = read_manifest(&dir).await?;
    let launch_args = if uses_pty_transport(&manifest.server_type) {
        forge_launch_args(&dir)?
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
    let memory = memory_mb
        .or(manifest.memory_mb)
        .unwrap_or(DEFAULT_MEMORY_MB);

    if let Some(hook) = manifest.pre_launch_hook.clone() {
        run_pre_launch_hook(server_id, &dir, &hook).await?;
    }

    // Ensure eula.txt exists (create with eula=false if missing)
    let eula_path = dir.join("eula.txt");
    let eula_created = !eula_path.exists();
    if eula_created {
        tokio::fs::write(&eula_path, "eula=false\n")
            .await
            .map_err(|e| IOError::with_path(e, &eula_path))?;
    }

    let mut args = vec![format!("-Xmx{memory}M")];
    if uses_pty_transport(&manifest.server_type) {
        args.push("-Dorg.jline.reader.props.list-max=0".to_string());
    }
    args.extend(jvm_args.unwrap_or_else(|| manifest.jvm_args.clone()));
    args.extend(launch_args);
    let removed_environment = [
        "DYLD_LIBRARY_PATH",
        "DYLD_FALLBACK_LIBRARY_PATH",
        "DYLD_FRAMEWORK_PATH",
        "DYLD_FALLBACK_FRAMEWORK_PATH",
        "DYLD_INSERT_LIBRARIES",
    ];

    let (mut child, input, pty_master, stdout, stderr, pty_reader) =
        if uses_pty_transport(&manifest.server_type) {
            let pair = native_pty_system()
                .openpty(PtySize {
                    rows: 12,
                    cols: 80,
                    pixel_width: 0,
                    pixel_height: 0,
                })
                .map_err(|e| {
                    ErrorKind::LauncherError(format!(
                        "Failed to create Forge console PTY: {e}"
                    ))
                    .as_error()
                })?;
            let reader = pair.master.try_clone_reader().map_err(|e| {
                ErrorKind::LauncherError(format!(
                    "Failed to capture Forge console output: {e}"
                ))
                .as_error()
            })?;
            let writer = pair.master.take_writer().map_err(|e| {
                ErrorKind::LauncherError(format!(
                    "Failed to capture Forge console input: {e}"
                ))
                .as_error()
            })?;
            let mut command = CommandBuilder::new(&java);
            java_environment::configure_pty_java_environment(&mut command);
            command.args(&args);
            command.cwd(&dir);
            command.env("TERM", "xterm-256color");
            for variable in removed_environment {
                command.env_remove(variable);
            }
            let child = pair.slave.spawn_command(command).map_err(|e| {
                ErrorKind::LauncherError(format!(
                    "Failed to start Forge server process: {e}"
                ))
                .as_error()
            })?;
            (
                ServerChild::Pty(child),
                ServerInput::Pty(writer),
                Some(tokio::sync::Mutex::new(pair.master)),
                None,
                None,
                Some(reader),
            )
        } else {
            let mut command = java_environment::tokio_command(&java);
            command.args(&args);
            command.current_dir(&dir);
            for variable in removed_environment {
                command.env_remove(variable);
            }
            command.stdout(std::process::Stdio::piped());
            command.stderr(std::process::Stdio::piped());
            command.stdin(std::process::Stdio::piped());
            command.kill_on_drop(true);

            let mut child = command.spawn().map_err(|e| {
                ErrorKind::LauncherError(format!(
                    "Failed to start server process: {e}"
                ))
                .as_error()
            })?;
            let stdout = child.stdout.take();
            let stderr = child.stderr.take();
            let stdin = child.stdin.take().ok_or_else(|| {
                ErrorKind::LauncherError(
                    "Server stdin could not be captured".to_string(),
                )
                .as_error()
            })?;
            (
                ServerChild::Piped(child),
                ServerInput::Piped(stdin),
                None,
                stdout,
                stderr,
                None,
            )
        };

    manifest.last_started_at = Some(Utc::now());
    manifest.last_exit_crashed = false;
    if let Err(error) = write_manifest(&dir, &manifest).await {
        let _ = child.kill().await;
        return Err(error);
    }

    clear_log_buffer(server_id);

    // Start each run from a clean log file. Minecraft's log4j appender appends
    // to logs/latest.log across launches, so without truncating it the file
    // tailer would replay the previous run's history into the fresh buffer on
    // every restart.
    let _ = std::fs::remove_file(dir.join("logs").join("latest.log"));

    // Surface every startup step in the console. A loader's first launch (e.g.
    // Fabric downloading the Minecraft server) can stay silent for a long time,
    // so these lines stop the console from looking frozen.
    let loader_first_run =
        matches!(manifest.server_type.as_str(), "fabric" | "quilt")
            && !safe_relative_join(
                &dir,
                &format!("{}-server-launch.jar", manifest.server_type),
            )?
            .exists();

    log_server_step(
        server_id,
        &format!(
            "Starting server '{}' ({} · Minecraft {})",
            manifest.name, manifest.server_type, manifest.game_version,
        ),
    )
    .await;
    log_server_step(server_id, &format!("Java: {java}")).await;
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
    if loader_first_run {
        log_server_step(
            server_id,
            "First launch: downloading Minecraft server files. This may take a few minutes — the console will keep updating as it progresses.",
        )
        .await;
    }

    let process = Arc::new(ServerProcess {
        child: tokio::sync::Mutex::new(child),
        input: tokio::sync::Mutex::new(input),
        pty_master,
        stop_requested: AtomicBool::new(false),
    });
    SERVER_PROCESSES.insert(server_id.to_string(), process.clone());

    if let Some(stdout) = stdout {
        tokio::spawn(stream_server_output(server_id.to_string(), stdout));
    }
    if let Some(stderr) = stderr {
        tokio::spawn(stream_server_output(server_id.to_string(), stderr));
    }
    if let Some(reader) = pty_reader {
        tokio::spawn(stream_server_pty_output(server_id.to_string(), reader));
    }
    // The process pipes (above) capture JVM/installer output, but the server's
    // own log4j console output is normally written to logs/latest.log rather
    // than the stdout pipe. Tail that file so the console always shows the
    // complete, lossless server log (matching what's on disk).
    tokio::spawn(tail_server_log_file(server_id.to_string(), dir.clone()));
    tokio::spawn(monitor_server_process(server_id.to_string(), dir, process));

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

pub async fn send_command(server_id: &str, command: &str) -> Result<()> {
    let process = SERVER_PROCESSES
        .get(server_id)
        .map(|entry| entry.value().clone())
        .ok_or_else(|| {
            ErrorKind::InputError("Server is not running".to_string())
                .as_error()
        })?;
    let mut input = process.input.lock().await;
    input
        .write_all(&command_bytes(command))
        .await
        .map_err(|e| {
            ErrorKind::LauncherError(format!("Failed to send command: {e}"))
                .as_error()
        })?;
    Ok(())
}

fn uses_pty_transport(server_type: &str) -> bool {
    server_type == "forge"
}

fn command_bytes(command: &str) -> Vec<u8> {
    format!("{command}\n").into_bytes()
}

pub async fn send_console_input(server_id: &str, data: &str) -> Result<()> {
    let process = SERVER_PROCESSES
        .get(server_id)
        .map(|entry| entry.value().clone())
        .ok_or_else(|| {
            ErrorKind::InputError("Server is not running".to_string())
                .as_error()
        })?;
    if process.pty_master.is_none() {
        return Err(ErrorKind::InputError(
            "Raw console input is only available for Forge servers".to_string(),
        )
        .as_error());
    }
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
    let mut input = process.input.lock().await;
    input.write_all(&bytes).await.map_err(|e| {
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
    let master = process.pty_master.as_ref().ok_or_else(|| {
        ErrorKind::InputError(
            "Console resizing is only available for Forge servers".to_string(),
        )
        .as_error()
    })?;
    master
        .lock()
        .await
        .resize(PtySize {
            rows: rows.clamp(4, MAX_CONSOLE_ROWS),
            cols: cols.clamp(20, 500),
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| {
            ErrorKind::LauncherError(format!(
                "Failed to resize Forge console: {e}"
            ))
            .as_error()
        })?;
    Ok(())
}

pub async fn stop(server_id: &str) -> Result<()> {
    let process = SERVER_PROCESSES
        .get(server_id)
        .map(|entry| entry.value().clone())
        .ok_or_else(|| {
            ErrorKind::InputError("Server is not running".to_string())
                .as_error()
        })?;
    process.stop_requested.store(true, Ordering::SeqCst);
    let mut input = process.input.lock().await;
    let _ = input.write_all(b"stop\n").await;

    let watchdog = process.clone();
    let server_id = server_id.to_string();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(STOP_TIMEOUT_SECS))
            .await;
        if let Some(current) = SERVER_PROCESSES.get(&server_id)
            && current.stop_requested.load(Ordering::SeqCst)
        {
            let _ = watchdog.child.lock().await.kill().await;
        }
    });
    Ok(())
}

pub async fn kill(server_id: &str) -> Result<()> {
    let process = SERVER_PROCESSES
        .get(server_id)
        .map(|entry| entry.value().clone())
        .ok_or_else(|| {
            ErrorKind::InputError("Server is not running".to_string())
                .as_error()
        })?;
    process.stop_requested.store(true, Ordering::SeqCst);
    let mut child = process.child.lock().await;
    child.kill().await?;
    Ok(())
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
                Ok(Some(success)) => Some(success),
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
async fn log_server_step(server_id: &str, message: &str) {
    let line = format!(
        "{} [Axolotl/INFO]: {}",
        chrono::Local::now().format("%H:%M:%S"),
        message,
    );
    push_log_line(server_id, line.clone());
    emit_server(server_id, ServerPayloadType::Log { line })
        .await
        .ok();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct SharedWriter(Arc<std::sync::Mutex<Vec<u8>>>);

    impl std::io::Write for SharedWriter {
        fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buffer);
            Ok(buffer.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn selects_pty_only_for_forge() {
        assert!(uses_pty_transport("forge"));
        assert!(!uses_pty_transport("vanilla"));
        assert!(!uses_pty_transport("fabric"));
        assert!(!uses_pty_transport("neoforge"));
    }

    #[test]
    fn whole_line_commands_end_with_one_newline() {
        assert_eq!(command_bytes("say hello"), b"say hello\n");
    }

    #[tokio::test]
    async fn pty_input_preserves_raw_bytes() {
        let captured = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut input =
            ServerInput::Pty(Box::new(SharedWriter(captured.clone())));
        input.write_all(b"\x1b[D\t\x7f").await.unwrap();
        assert_eq!(*captured.lock().unwrap(), b"\x1b[D\t\x7f");
    }
}

use super::page;
use base64::Engine;
use launcher_tools::{Command, ToolError, log_text};
use serde_json::{Value, json};
use theseus::servers::{self, ServerInfo};
mod install;

fn error(error: theseus::Error) -> ToolError {
    tracing::warn!(%error,"MCP dedicated server operation failed");
    ToolError::new(
        "server_operation_failed",
        "Dedicated server operation failed; inspect server_get lifecycle/EULA/install status and censored server logs",
    )
}
fn summary(info: ServerInfo) -> Value {
    let manifest = info.manifest;
    json!({"server_id":manifest.id,"name":manifest.name,"server_type":manifest.server_type,"game_version":manifest.game_version,
        "loader_version":manifest.loader_version,"java_path":manifest.java_path,"memory_mib":manifest.memory_mb,"effective_memory_mib":manifest.memory_mb.unwrap_or(2048),
        "running":info.running,"starting":info.starting,"stopping":info.stopping,"port":info.port,"online_mode":info.online_mode.unwrap_or(true),
        "eula_exists":info.eula_exists,"eula_accepted":info.eula_accepted,"install_state":manifest.install_state,"has_install_error":manifest.install_error.is_some(),
        "has_custom_jvm_arguments":!manifest.jvm_args.is_empty(),"has_pre_launch_hook":manifest.pre_launch_hook.is_some(),"has_custom_stop_command":manifest.stop_command!="stop",
        "pinned":manifest.home_pinned_at.is_some(),"created_at":manifest.created_at,"last_started_at":manifest.last_started_at,"last_exit_crashed":manifest.last_exit_crashed})
}

pub(super) fn require_stopped(info: &ServerInfo) -> Result<(), ToolError> {
    if info.running || info.starting || info.stopping {
        return Err(ToolError::new(
            "server_active",
            "Wait for server shutdown before editing or deleting it",
        ));
    }
    Ok(())
}

pub(super) async fn info(id: &str) -> Result<ServerInfo, ToolError> {
    if id.is_empty()
        || id.contains("..")
        || !id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
    {
        return Err(ToolError::new(
            "invalid_server_id",
            "Use a managed server ID returned by server_list",
        ));
    }
    let info = servers::get(id).await.map_err(error)?;
    if info.manifest.id != id {
        return Err(ToolError::new(
            "server_identity_mismatch",
            "Server manifest does not match its directory ID",
        ));
    }
    let metadata =
        tokio::fs::symlink_metadata(&info.path).await.map_err(|_| {
            ToolError::new(
                "server_path_unavailable",
                "Cannot inspect server directory",
            )
        })?;
    let linked = metadata.file_type().is_symlink();
    #[cfg(windows)]
    let linked = {
        use std::os::windows::fs::MetadataExt;
        linked || metadata.file_attributes() & 0x400 != 0
    };
    if linked {
        return Err(ToolError::new(
            "linked_server_directory",
            "Managed server operations cannot use linked directories",
        ));
    }
    Ok(info)
}

async fn validate_java(path: Option<&str>) -> Result<(), ToolError> {
    if let Some(path) = path {
        let java = theseus::jre::get_registered_java_versions()
            .await
            .map_err(error)?;
        if !java.runtimes.iter().any(|runtime| runtime.path == path) {
            return Err(ToolError::new(
                "unregistered_java",
                "Choose an exact Java path returned by java_list",
            ));
        }
    }
    Ok(())
}
fn validate_memory(
    memory: Option<u32>,
    total_mib: u64,
) -> Result<(), ToolError> {
    if memory.is_some_and(|m| {
        !(256..=1048576).contains(&m) || u64::from(m) > total_mib
    }) {
        return Err(ToolError::new(
            "invalid_server_memory",
            "Server memory must be at least 256 MiB and cannot exceed physical RAM",
        ));
    }
    Ok(())
}

fn command_bytes(command: &str) -> Result<Vec<u8>, ToolError> {
    if command.trim().is_empty()
        || command.chars().count() > 4096
        || command.chars().any(char::is_control)
    {
        return Err(ToolError::new(
            "invalid_console_command",
            "Use a single nonempty text command without terminal control characters",
        ));
    }
    Ok(format!("{command}\r\n").into_bytes())
}

fn recent_lines(lines: Vec<String>) -> (String, bool) {
    let mut size = 0usize;
    let mut retained = Vec::new();
    let count = lines.len();
    for line in lines.into_iter().rev() {
        if size.saturating_add(line.len()).saturating_add(1) > 512 * 1024 {
            break;
        }
        size += line.len() + 1;
        retained.push(line);
    }
    let truncated = retained.len() != count;
    retained.reverse();
    (retained.join("\n"), truncated)
}

async fn logs(id: &str) -> Result<(String, bool), ToolError> {
    info(id).await?;
    let (text, truncated) =
        recent_lines(servers::get_log_buffer(id).await.map_err(error)?);
    let state = theseus::State::get().await.map_err(error)?;
    Ok((
        theseus::install::censor_shared_text(text, &state)
            .await
            .map_err(error)?,
        truncated,
    ))
}

pub(super) async fn execute(command: Command) -> Result<Value, ToolError> {
    match command {
        command @ (Command::ServerInstallForge { .. }
        | Command::ServerInstallNeoforge { .. }
        | Command::ServerInstallModpack { .. }) => {
            install::execute(command).await
        }
        Command::ServerList { offset, limit } => {
            let mut servers = servers::list().await.map_err(error)?;
            servers.sort_by(|a, b| a.manifest.id.cmp(&b.manifest.id));
            page(servers.into_iter().map(summary).collect(), offset, limit)
        }
        Command::ServerGet { server_id } => {
            Ok(summary(info(&server_id).await?))
        }
        Command::ServerCreate {
            name,
            server_type,
            game_version,
            loader_version,
            registered_java_path,
            memory_mib,
        } => {
            validate_java(registered_java_path.as_deref()).await?;
            validate_memory(
                memory_mib,
                theseus::jre::get_max_memory().await.map_err(error)? / 1024,
            )?;
            let manifest = servers::create(
                &name,
                &server_type,
                &game_version,
                loader_version,
                registered_java_path,
                memory_mib,
            )
            .await
            .map_err(error)?;
            Ok(summary(info(&manifest.id).await?))
        }
        Command::ServerUpdateSettings {
            server_id,
            name,
            registered_java_path,
            memory_mib,
            pinned,
        } => {
            if name.is_none()
                && registered_java_path.is_none()
                && memory_mib.is_none()
                && pinned.is_none()
            {
                return Err(ToolError::new(
                    "no_changes",
                    "Provide at least one setting to change",
                ));
            }
            require_stopped(&info(&server_id).await?)?;
            validate_java(registered_java_path.as_deref()).await?;
            validate_memory(
                memory_mib,
                theseus::jre::get_max_memory().await.map_err(error)? / 1024,
            )?;
            servers::update_settings(
                &server_id,
                name,
                registered_java_path,
                memory_mib,
                None,
                None,
                pinned,
                None,
            )
            .await
            .map_err(error)?;
            Ok(summary(info(&server_id).await?))
        }
        Command::ServerStart { server_id } => {
            let current = info(&server_id).await?;
            require_stopped(&current)?;
            if current.manifest.install_state.is_some() {
                return Err(ToolError::new(
                    "server_install_incomplete",
                    "Finish or repair server installation before starting",
                ));
            }
            if !current.eula_accepted {
                return Err(ToolError::new(
                    "server_eula_required",
                    "Accept the server EULA in the launcher before starting this server",
                ));
            }
            servers::start(&server_id, None, None, None)
                .await
                .map_err(error)?;
            Ok(summary(info(&server_id).await?))
        }
        Command::ServerWaitReady {
            server_id,
            timeout_seconds,
        } => {
            info(&server_id).await?;
            Ok(
                json!({"server_id":server_id,"ready":servers::wait_until_ready(&server_id,timeout_seconds.clamp(1,20)*1000).await.map_err(error)?}),
            )
        }
        Command::ServerStop { server_id } => {
            let current = info(&server_id).await?;
            if current.running || current.starting {
                servers::stop(&server_id).await.map_err(error)?;
            }
            Ok(summary(info(&server_id).await?))
        }
        Command::ServerKill { server_id } => {
            let current = info(&server_id).await?;
            if current.running || current.starting {
                servers::kill(&server_id).await.map_err(error)?;
            }
            Ok(summary(info(&server_id).await?))
        }
        Command::ServerDelete { server_id } => {
            require_stopped(&info(&server_id).await?)?;
            servers::delete(&server_id).await.map_err(error)?;
            Ok(json!({"server_id":server_id,"deleted":true}))
        }
        Command::ServerConsoleSend { server_id, command } => {
            let current = info(&server_id).await?;
            if !current.running || current.starting || current.stopping {
                return Err(ToolError::new(
                    "server_not_running",
                    "Wait until this server is running and no longer starting/stopping",
                ));
            }
            let data = base64::engine::general_purpose::STANDARD
                .encode(command_bytes(&command)?);
            servers::send_console_input(&server_id, &data)
                .await
                .map_err(error)?;
            Ok(json!({"server_id":server_id,"command_sent":true}))
        }
        Command::ServerLogsRead {
            server_id,
            offset,
            max_bytes,
        } => {
            let (text, truncated) = logs(&server_id).await?;
            Ok(log_text::read(&text, offset, max_bytes, truncated))
        }
        Command::ServerLogsSearch {
            server_id,
            query,
            start_line,
            limit,
        } => {
            let (text, truncated) = logs(&server_id).await?;
            Ok(log_text::search(
                &text, &query, start_line, limit, truncated,
            ))
        }
        _ => Err(ToolError::new(
            "unsupported_command",
            "Not a dedicated server operation",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn server() -> ServerInfo {
        serde_json::from_value::<servers::ServerManifest>(json!({"id":"server-123","name":"Server","serverType":"vanilla","gameVersion":"1.21",
            "jvmArgs":["-Dsecret=private-value"],"preLaunchHook":"private-hook","stopCommand":"private-stop","installError":"private-error",
            "createdAt":"2026-01-01T00:00:00Z"})).map(|manifest|ServerInfo {manifest,path:"C:/private-path".into(),running:false,starting:false,stopping:false,eula_exists:true,eula_accepted:true,port:Some(25565),online_mode:None}).unwrap()
    }
    #[test]
    fn lifecycle_and_settings_are_visible_without_commands_or_diagnostics() {
        let value = summary(server());
        assert_eq!(value["effective_memory_mib"], 2048);
        assert_eq!(value["online_mode"], true);
        assert_eq!(value["has_pre_launch_hook"], true);
        assert!(!value.to_string().contains("private"));
    }
    #[test]
    fn active_lifecycle_states_prevent_deletion_or_editing() {
        let mut server = server();
        assert!(require_stopped(&server).is_ok());
        server.starting = true;
        assert!(require_stopped(&server).is_err());
        server.starting = false;
        server.running = true;
        assert!(require_stopped(&server).is_err());
        server.running = false;
        server.stopping = true;
        assert!(require_stopped(&server).is_err());
    }
    #[test]
    fn memory_and_console_inputs_are_bounded() {
        assert!(validate_memory(Some(4096), 8192).is_ok());
        assert!(validate_memory(Some(16384), 8192).is_err());
        assert_eq!(command_bytes("say hello").unwrap(), b"say hello\r\n");
        for command in ["", "stop\nremove", "say \u{1b}[A"] {
            assert!(command_bytes(command).is_err());
        }
    }
    #[test]
    fn log_bounds_keep_recent_complete_lines() {
        let (text, truncated) = recent_lines(vec![
            "x".repeat(512 * 1024),
            "newest token must stay whole".into(),
        ]);
        assert!(truncated);
        assert_eq!(text, "newest token must stay whole");
    }
}

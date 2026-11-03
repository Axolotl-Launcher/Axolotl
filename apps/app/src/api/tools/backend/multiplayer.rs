use super::{LauncherBackend, business_error, page, serialize};
use launcher_tools::{BackendContext, Command, ToolError, log_text};
use serde_json::{Value, json};
use theseus::{hongshi as h, multiplayer as m, terracotta as t};
fn provider(s: &str) -> Result<m::MultiplayerProvider, ToolError> {
    match s {
        "terracotta" => Ok(m::MultiplayerProvider::Terracotta),
        "hongshi" => Ok(m::MultiplayerProvider::Hongshi),
        _ => Err(ToolError::new(
            "invalid_provider",
            "Unknown multiplayer provider",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn state_summary_hides_room_codes_machine_ids_and_raw_errors() {
        let mut terracotta = t::TerracottaState::default();
        terracotta.room_code = Some("private-room-code".into());
        terracotta.error_message = Some("private-error".into());
        terracotta.players.push(t::PlayerInfo {
            machine_id: "private-machine-id".into(),
            name: "Player".into(),
            vendor: "vendor".into(),
            kind: t::TerracottaPlayerKind::Guest,
        });
        let mut hongshi = h::HongshiState::default();
        hongshi.public_address = Some("private-address".into());
        hongshi.error_message = Some("private-error".into());
        let result = summary(m::MultiplayerState {
            active_provider: None,
            providers: vec![],
            terracotta,
            hongshi,
        });
        assert_eq!(result["terracotta"]["player_count"], 1);
        for private in [
            "private-room-code",
            "private-machine-id",
            "private-error",
            "private-address",
        ] {
            assert!(!result.to_string().contains(private));
        }
    }
}
fn error(e: impl std::fmt::Display) -> ToolError {
    tracing::warn!(%e,"MCP multiplayer operation failed");
    ToolError::new(
        "multiplayer_failed",
        "Multiplayer operation failed; inspect provider status or censored diagnostics",
    )
}
fn summary(s: m::MultiplayerState) -> Value {
    json!({"active_provider":s.active_provider,"providers":s.providers,"terracotta":{"status":s.terracotta.status,"server_port":s.terracotta.server_port,"player_count":s.terracotta.players.len(),"download_progress":s.terracotta.download_progress,"download_stage":s.terracotta.download_stage,"binary_installed":s.terracotta.binary_installed,"installed_version":s.terracotta.installed_version,"error_type":s.terracotta.error_type},"hongshi":{"status":s.hongshi.status,"supported":s.hongshi.supported,"local_port":s.hongshi.local_port,"node":s.hongshi.node,"binary_installed":s.hongshi.binary_installed,"download_progress":s.hongshi.download_progress,"bound_instance_id":s.hongshi.bound_instance_id,"error_type":s.hongshi.error_type,"port_changed":s.hongshi.port_changed}})
}
pub(super) async fn execute(
    _: &LauncherBackend,
    command: Command,
    _: BackendContext,
) -> Result<Value, ToolError> {
    match command {
        Command::MultiplayerGetState {} => {
            return Ok(summary(m::get_state().await));
        }
        Command::MultiplayerGetConnection {} => {
            let s = m::get_state().await;
            return Ok(
                json!({"provider":s.active_provider,"room_code":s.terracotta.room_code,"public_address":s.hongshi.public_address,"server_port":s.terracotta.server_port}),
            );
        }
        Command::MultiplayerListNodes {
            force_refresh,
            offset,
            limit,
        } => {
            return page(
                h::get_nodes(force_refresh.unwrap_or(false))
                    .await
                    .map_err(error)?,
                offset.unwrap_or(0),
                limit.unwrap_or(50),
            );
        }
        Command::MultiplayerDetectPorts { offset, limit } => {
            return page(
                h::get_detected_ports().await,
                offset.unwrap_or(0),
                limit.unwrap_or(50),
            );
        }
        Command::MultiplayerSetProvider { provider: p } => {
            m::switch_provider(provider(&p)?).await.map_err(error)?
        }
        Command::MultiplayerPrepareProvider { provider: p } => {
            match provider(&p)? {
                m::MultiplayerProvider::Terracotta => {
                    m::prepare_terracotta().await.map_err(error)?
                }
                m::MultiplayerProvider::Hongshi => {
                    h::download().await.map_err(error)?
                }
            }
        }
        Command::MultiplayerHost {
            provider: p,
            player_name,
            room_code,
            local_port,
            node_name,
            instance_id,
        } => {
            let request = match provider(&p)? {
                m::MultiplayerProvider::Terracotta => {
                    if local_port.is_some()
                        || node_name.is_some()
                        || instance_id.is_some()
                    {
                        return Err(ToolError::new(
                            "invalid_host_fields",
                            "Hongshi fields cannot be used for Terracotta",
                        ));
                    }
                    m::MultiplayerHostRequest::Terracotta {
                        player_name: player_name.ok_or_else(|| {
                            ToolError::new(
                                "player_name_required",
                                "Terracotta hosting requires player_name",
                            )
                        })?,
                        room_code,
                    }
                }
                m::MultiplayerProvider::Hongshi => {
                    if player_name.is_some() || room_code.is_some() {
                        return Err(ToolError::new(
                            "invalid_host_fields",
                            "Terracotta fields cannot be used for Hongshi",
                        ));
                    }
                    let port = local_port.ok_or_else(|| {
                        ToolError::new(
                            "port_required",
                            "Select a detected Minecraft LAN port",
                        )
                    })? as u16;
                    if !h::get_detected_ports().await.iter().any(|p| {
                        p.port == port
                            && instance_id
                                .as_ref()
                                .is_none_or(|id| id == &p.instance_id)
                    }) {
                        return Err(ToolError::new(
                            "port_not_detected",
                            "Select a current Minecraft port from multiplayer_detect_ports",
                        ));
                    }
                    if let Some(name) = &node_name {
                        if !h::get_nodes(false)
                            .await
                            .map_err(error)?
                            .iter()
                            .any(|n| &n.name == name)
                        {
                            return Err(ToolError::new(
                                "node_not_found",
                                "Select a node from multiplayer_list_nodes",
                            ));
                        }
                    }
                    m::MultiplayerHostRequest::Hongshi {
                        local_port: port,
                        node_name,
                        instance_id,
                    }
                }
            };
            m::host(request).await.map_err(error)?;
        }
        Command::MultiplayerJoin {
            room_code,
            player_name,
        } => m::join(m::MultiplayerJoinRequest {
            provider: m::MultiplayerProvider::Terracotta,
            room_code,
            player_name,
        })
        .await
        .map_err(error)?,
        Command::MultiplayerStop {} => m::stop().await.map_err(error)?,
        Command::MultiplayerReset {} => m::reset().await.map_err(error)?,
        Command::MultiplayerCheckUpdate {} => {
            return serialize(t::check_for_update().await.map_err(error)?);
        }
        Command::MultiplayerUpdate {} => {
            return serialize(t::update_terracotta().await.map_err(error)?);
        }
        c @ (Command::MultiplayerLogsRead { .. }
        | Command::MultiplayerGetDiagnostics { .. }) => {
            let diagnostics =
                matches!(c, Command::MultiplayerGetDiagnostics { .. });
            let (Command::MultiplayerLogsRead { offset, max_bytes }
            | Command::MultiplayerGetDiagnostics { offset, max_bytes }) = c
            else {
                unreachable!()
            };
            let raw = if diagnostics {
                t::get_diagnostic_report().await
            } else {
                t::get_logs().await
            }
            .map_err(error)?;
            let state = theseus::State::get().await.map_err(business_error)?;
            let text = theseus::install::censor_shared_text(raw, &state)
                .await
                .map_err(business_error)?;
            return Ok(log_text::read(
                &text,
                offset.unwrap_or(0),
                max_bytes.unwrap_or(32768),
                false,
            ));
        }
        _ => {
            return Err(ToolError::new(
                "unsupported_command",
                "Not a multiplayer operation",
            ));
        }
    }
    Ok(summary(m::get_state().await))
}

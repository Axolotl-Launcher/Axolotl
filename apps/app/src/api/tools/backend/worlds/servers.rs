use super::super::{business_error, page, serialize};
use super::{paths, stopped, summary};
use launcher_tools::{Command, PackPolicy, ToolError};
use serde_json::{Value, json};
use theseus::worlds::{
    self, ServerEntryExpectation, ServerPackStatus, WorldDetails,
};

fn pack(policy: PackPolicy) -> ServerPackStatus {
    match policy {
        PackPolicy::Enabled => ServerPackStatus::Enabled,
        PackPolicy::Disabled => ServerPackStatus::Disabled,
        PackPolicy::Prompt => ServerPackStatus::Prompt,
    }
}
fn address(address: &str) -> Result<(), ToolError> {
    if !launcher_tools::world_input::valid_address(address) {
        return Err(ToolError::new(
            "invalid_address",
            "Use a Minecraft hostname/IP with optional port, not a URL",
        ));
    }
    theseus::server_address::parse_server_address(address)
        .map_err(business_error)?;
    Ok(())
}

fn status_summary(status: worlds::ServerStatus) -> Value {
    json!({"online":true,"ping_ms":status.ping,"version":status.version.map(|version|json!({"name":version.name.chars().take(256).collect::<String>(),"protocol":version.protocol,"legacy":version.legacy})),"players":status.players.map(|players|json!({"online":players.online,"max":players.max})),"enforces_secure_chat":status.enforces_secure_chat})
}

pub(super) async fn execute(command: Command) -> Result<Value, ToolError> {
    match command {
        Command::ServerEntryList {
            instance_id,
            offset,
            limit,
        } => {
            paths::server_list(&instance_id).await?;
            let mut entries = worlds::get_instance_worlds(&instance_id)
                .await
                .map_err(business_error)?;
            entries.retain(|entry| {
                matches!(entry.details, WorldDetails::Server { .. })
            });
            entries.sort_by_key(|entry| match entry.details {
                WorldDetails::Server { index, .. } => index,
                _ => 0,
            });
            page(entries.into_iter().map(summary).collect(), offset, limit)
        }
        Command::ServerEntryAdd {
            instance_id,
            name,
            address: addr,
            pack_status,
        } => {
            address(&addr)?;
            stopped(&instance_id).await?;
            paths::server_list(&instance_id).await?;
            let index = worlds::add_server_to_instance(
                &instance_id,
                name,
                addr,
                pack(pack_status),
                None,
                None,
            )
            .await
            .map_err(business_error)?;
            Ok(json!({"index":index,"added":true}))
        }
        Command::ServerEntryUpdate {
            instance_id,
            index,
            expected_name,
            expected_address,
            expected_pack_status,
            name,
            address: addr,
            pack_status,
        } => {
            address(&addr)?;
            stopped(&instance_id).await?;
            paths::server_list(&instance_id).await?;
            worlds::edit_server_in_instance_checked(
                &instance_id,
                index,
                Some(ServerEntryExpectation {
                    name: expected_name,
                    address: expected_address,
                    pack_status: pack(expected_pack_status),
                }),
                name,
                addr,
                pack(pack_status),
            )
            .await
            .map_err(business_error)?;
            Ok(json!({"index":index,"updated":true}))
        }
        Command::ServerEntryRemove {
            instance_id,
            index,
            expected_name,
            expected_address,
            expected_pack_status,
        } => {
            stopped(&instance_id).await?;
            paths::server_list(&instance_id).await?;
            worlds::remove_server_from_instance_checked(
                &instance_id,
                index,
                Some(ServerEntryExpectation {
                    name: expected_name,
                    address: expected_address,
                    pack_status: pack(expected_pack_status),
                }),
            )
            .await
            .map_err(business_error)?;
            Ok(json!({"index":index,"removed":true}))
        }
        Command::ServerPing {
            address: addr,
            instance_id,
        } => {
            address(&addr)?;
            let task = async {
                let protocol = match instance_id {
                    Some(id) => worlds::get_instance_protocol_version(&id)
                        .await
                        .map_err(business_error)?,
                    None => None,
                };
                worlds::get_server_status(&addr, protocol)
                    .await
                    .map_err(business_error)
            };
            let status =
                tokio::time::timeout(std::time::Duration::from_secs(15), task)
                    .await
                    .map_err(|_| {
                        ToolError::new(
                            "ping_timeout",
                            "Server status query timed out",
                        )
                    })??;
            Ok(status_summary(status))
        }
        Command::InstanceGetProtocolVersion { instance_id } => serialize(
            worlds::get_instance_protocol_version(&instance_id)
                .await
                .map_err(business_error)?,
        ),
        Command::InstanceJoinServer {
            instance_id,
            address: addr,
            offline_mode,
        } => {
            address(&addr)?;
            stopped(&instance_id).await?;
            let process = theseus::instance::run(
                &instance_id,
                theseus::instance::QuickPlayType::Server(
                    theseus::server_address::ServerAddress::Unresolved(addr),
                ),
                offline_mode,
            )
            .await
            .map_err(business_error)?;
            Ok(json!({"process":process}))
        }
        _ => Err(ToolError::new(
            "unsupported_command",
            "Not a world/server entry operation",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ping_summary_omits_player_identities_icons_and_raw_description() {
        let status:worlds::ServerStatus=serde_json::from_value(json!({"description":{"text":"private server text"},"players":{"max":20,"online":1,"sample":[{"id":"private-uuid","name":"private-player"}]},"favicon":"data:image/png;base64,secret","ping":12})).unwrap();
        let value = status_summary(status);
        assert_eq!(value["players"]["online"], 1);
        assert_eq!(value["ping_ms"], 12);
        assert!(!value.to_string().contains("private"));
        assert!(!value.to_string().contains("secret"));
    }
}

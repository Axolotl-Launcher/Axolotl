mod paths;
mod servers;
use super::{business_error, page, serialize};
use launcher_tools::{Command, Difficulty, GameMode, ToolError, WorldDisplay};
use serde_json::{Value, json};
use theseus::worlds::{self, World, WorldDetails, WorldSettingsPatch};

pub(super) async fn stopped(instance_id: &str) -> Result<(), ToolError> {
    if !theseus::process::get_by_instance_id(instance_id)
        .await
        .map_err(business_error)?
        .is_empty()
    {
        return Err(ToolError::new(
            "game_running",
            "Stop this instance's game before modifying worlds or its server list",
        ));
    }
    Ok(())
}

fn summary(world: World) -> Value {
    match world.details {
        WorldDetails::Singleplayer {
            path,
            game_mode,
            hardcore,
            locked,
        } => {
            json!({"world_id":path,"name":world.name,"last_played":world.last_played,"display_status":world.display_status,"game_mode":game_mode,"hardcore":hardcore,"locked":locked})
        }
        WorldDetails::Server {
            index,
            address,
            pack_status,
            ..
        } => {
            json!({"index":index,"name":world.name,"address":address,"pack_status":pack_status,"last_played":world.last_played,"display_status":world.display_status})
        }
    }
}

fn settings_patch(
    game_mode: Option<GameMode>,
    difficulty: Option<Difficulty>,
    allow_commands: Option<bool>,
) -> Result<WorldSettingsPatch, ToolError> {
    if game_mode.is_none() && difficulty.is_none() && allow_commands.is_none() {
        return Err(ToolError::new(
            "no_changes",
            "Choose a game_mode, difficulty or allow_commands value",
        ));
    }
    Ok(WorldSettingsPatch {
        game_mode: game_mode.map(|mode| match mode {
            GameMode::Survival => worlds::SingleplayerGameMode::Survival,
            GameMode::Creative => worlds::SingleplayerGameMode::Creative,
            GameMode::Adventure => worlds::SingleplayerGameMode::Adventure,
            GameMode::Spectator => worlds::SingleplayerGameMode::Spectator,
        }),
        difficulty: difficulty.map(|value| match value {
            Difficulty::Peaceful => worlds::WorldDifficulty::Peaceful,
            Difficulty::Easy => worlds::WorldDifficulty::Easy,
            Difficulty::Normal => worlds::WorldDifficulty::Normal,
            Difficulty::Hard => worlds::WorldDifficulty::Hard,
        }),
        allow_commands,
        ..Default::default()
    })
}

pub(super) async fn execute(command: Command) -> Result<Value, ToolError> {
    match command {
        Command::WorldList {
            instance_id,
            offset,
            limit,
        } => {
            let mut worlds = worlds::get_instance_worlds(&instance_id)
                .await
                .map_err(business_error)?;
            worlds.retain(|world| {
                matches!(world.details, WorldDetails::Singleplayer { .. })
            });
            worlds.sort_by(|a, b| a.world_id().cmp(b.world_id()));
            page(worlds.into_iter().map(summary).collect(), offset, limit)
        }
        Command::WorldGet {
            instance_id,
            world_id,
        } => {
            paths::world(&instance_id, &world_id).await?;
            let data = worlds::get_world_level_data(&instance_id, &world_id)
                .await
                .map_err(business_error)?;
            let rule_count = data.game_rules.len();
            Ok(
                json!({"world_id":world_id,"name":data.name,"game_mode":data.game_mode,"difficulty":data.difficulty,"difficulty_locked":data.difficulty_locked,
                "hardcore":data.hardcore,"allow_commands":data.allow_commands,"seed":data.seed,"version_name":data.version_name,
                "last_played":data.last_played,"modded":data.modded,"locked":data.locked,"game_rule_count":rule_count,"game_rules_truncated":rule_count>100,
                "game_rules":data.game_rules.into_iter().take(100).map(|rule|json!({"key":rule.key.chars().take(128).collect::<String>(),"value":rule.value.chars().take(256).collect::<String>()})).collect::<Vec<_>>()}),
            )
        }
        Command::WorldRename {
            instance_id,
            world_id,
            name,
        } => {
            stopped(&instance_id).await?;
            let root = paths::world(&instance_id, &world_id).await?;
            worlds::update_world_settings(
                &root,
                &world_id,
                WorldSettingsPatch {
                    name: Some(name.trim().into()),
                    ..Default::default()
                },
            )
            .await
            .map_err(business_error)?;
            Ok(summary(
                worlds::get_singleplayer_world(&instance_id, &world_id)
                    .await
                    .map_err(business_error)?,
            ))
        }
        Command::WorldSetDisplayStatus {
            instance_id,
            world_id,
            status,
        } => {
            paths::world(&instance_id, &world_id).await?;
            let status = match status {
                WorldDisplay::Normal => worlds::DisplayStatus::Normal,
                WorldDisplay::Hidden => worlds::DisplayStatus::Hidden,
                WorldDisplay::Favorite => worlds::DisplayStatus::Favorite,
            };
            worlds::set_world_display_status(
                &instance_id,
                worlds::WorldType::Singleplayer,
                &world_id,
                status,
            )
            .await
            .map_err(business_error)?;
            Ok(json!({"world_id":world_id,"display_status":status}))
        }
        Command::WorldUpdateSettings {
            instance_id,
            world_id,
            game_mode,
            difficulty,
            allow_commands,
        } => {
            let patch = settings_patch(game_mode, difficulty, allow_commands)?;
            stopped(&instance_id).await?;
            let root = paths::world(&instance_id, &world_id).await?;
            worlds::update_world_settings(&root, &world_id, patch)
                .await
                .map_err(business_error)?;
            Ok(json!({"updated":true,"world_id":world_id}))
        }
        Command::WorldBackup {
            instance_id,
            world_id,
        } => {
            stopped(&instance_id).await?;
            let root = paths::world(&instance_id, &world_id).await?;
            paths::backup_destination(&root).await?;
            let size = worlds::backup_world(&root, &world_id)
                .await
                .map_err(business_error)?;
            Ok(
                json!({"world_id":world_id,"archive_size_bytes":size,"destination":"instance/backups"}),
            )
        }
        Command::WorldDelete {
            instance_id,
            world_id,
        } => {
            stopped(&instance_id).await?;
            let root = paths::world(&instance_id, &world_id).await?;
            worlds::delete_world(&root, &world_id)
                .await
                .map_err(business_error)?;
            Ok(json!({"world_id":world_id,"deleted":true}))
        }
        Command::InstanceLaunchWorld {
            instance_id,
            world_id,
            offline_mode,
        } => {
            stopped(&instance_id).await?;
            paths::world(&instance_id, &world_id).await?;
            serialize(
                json!({"process":theseus::instance::run(&instance_id,theseus::instance::QuickPlayType::Singleplayer(world_id),offline_mode).await.map_err(business_error)?}),
            )
        }
        command => servers::execute(command).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn world_patches_preserve_unspecified_fields_and_keep_explicit_false() {
        assert!(settings_patch(None, None, None).is_err());
        let patch =
            settings_patch(None, Some(Difficulty::Hard), Some(false)).unwrap();
        assert_eq!(patch.allow_commands, Some(false));
        assert!(patch.game_mode.is_none());
        assert!(patch.name.is_none());
        assert!(patch.seed.is_none());
        assert!(patch.game_rules.is_none());
        assert_eq!(patch.difficulty, Some(worlds::WorldDifficulty::Hard));
    }
    #[test]
    fn summaries_do_not_expose_icons_or_native_paths() {
        let world:World=serde_json::from_value(json!({"name":"World","icon":"C:/private/icon.png","display_status":"favorite","type":"singleplayer","path":"save","game_mode":"creative","hardcore":false,"locked":true})).unwrap();
        let value = summary(world);
        assert_eq!(value["world_id"], "save");
        assert_eq!(value["locked"], true);
        assert!(!value.to_string().contains("private"));
    }
}

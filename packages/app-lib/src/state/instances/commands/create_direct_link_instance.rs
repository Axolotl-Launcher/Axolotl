use crate::api::pack::import::{
    ImportLauncherType,
    direct_link::{direct_link_group, resolve_direct_link},
};
use crate::launcher::ExternalGameDirMode;
use crate::state::instances::{
    ContentSet, ContentSetStatus, ContentSourceKind, Instance,
    InstanceLaunchOverrides, InstanceLink, LoaderComponent,
    adapters::sqlite::{content_rows, instance_rows, loader_component_rows},
};
use crate::state::{
    InstanceInstallStage, LauncherFeatureVersion, ReleaseChannel, State,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::info;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateDirectLinkInstance {
    /// Display name; defaults to the actual version JSON stem.
    pub name: Option<String>,
    /// Same launcher identity used by the existing import API.
    pub launcher_type: ImportLauncherType,
    /// Root selected for launcher scanning/import.
    pub base_path: PathBuf,
    /// Scanned launcher instance name/folder identity.
    pub instance_folder: String,
    /// Pre-resolved version directory, including compatible-mode selections.
    #[serde(default)]
    pub instance_path: Option<String>,
    #[serde(default)]
    pub game_dir_mode: Option<ExternalGameDirMode>,
}

pub(crate) async fn create_direct_link_instance(
    input: CreateDirectLinkInstance,
    state: &State,
) -> crate::Result<Instance> {
    let _metadata_usage = crate::storage::usage::lease().await?;
    let resolved = resolve_direct_link(
        input.launcher_type,
        input.base_path,
        input.instance_folder,
        input.instance_path,
    )
    .await?;

    info!(
        launcher = resolved.launcher_key(),
        version_id = %resolved.version_id,
        version_json = %resolved.version_json.display(),
        "Creating directly associated instance"
    );

    let name = input
        .name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| resolved.version_id.clone());
    // Reserves a unique relative path for the instance record only; no
    // profile directory is ever created for directly associated instances.
    let (path, _full_path) =
        super::create_instance::resolve_instance_path(&name, None, state)
            .await?;

    let now = Utc::now();
    let instance_id = format!("local:{}", Uuid::new_v4());
    let content_set_id = format!("content-set:{}", Uuid::new_v4());
    let instance = Instance {
        id: instance_id.clone(),
        path: path.clone(),
        applied_content_set_id: Some(content_set_id.clone()),
        // Nothing to install: every file already sits in place inside the
        // linked `.minecraft`, so the instance is launchable immediately.
        install_stage: InstanceInstallStage::Installed,
        launcher_feature_version: LauncherFeatureVersion::MOST_RECENT,
        update_channel: ReleaseChannel::Release,
        name,
        icon_path: None,
        symlink_target: None,
        linked_launcher: Some(resolved.launcher_key().to_string()),
        linked_launcher_root: Some(
            resolved.launcher_root.to_string_lossy().to_string(),
        ),
        linked_dot_minecraft: Some(
            resolved.dot_minecraft.to_string_lossy().to_string(),
        ),
        linked_version_id: Some(resolved.version_id.clone()),
        linked_version_json_path: Some(
            resolved.version_json.to_string_lossy().to_string(),
        ),
        linked_game_dir_mode: input
            .game_dir_mode
            .map(|mode| mode.key().to_string()),
        // Directly associated instances resolve their game directory from the
        // link metadata; the managed `game_dir_override` never applies.
        game_dir_override: None,
        created: now,
        modified: now,
        last_played: None,
        pinned_at: None,
        submitted_time_played: 0,
        recent_time_played: 0,
    };
    let content_set = ContentSet {
        id: content_set_id.clone(),
        instance_id: instance_id.clone(),
        name: "Default".to_string(),
        source_kind: ContentSourceKind::Local,
        status: ContentSetStatus::Available,
        game_version: resolved.game_version,
        protocol_version: None,
        loader: resolved.loader,
        // The loader is installed and managed by the external launcher; the
        // version parsed from the version JSON is not a reliable display value
        // for directly associated instances, so it is intentionally nulled.
        loader_version: None,
        revision: 0,
        created: now,
        modified: now,
    };
    let launch_overrides = InstanceLaunchOverrides::empty(instance_id.clone());

    let mut tx = state.pool.begin().await?;
    instance_rows::insert_instance(&instance, &mut tx).await?;
    sqlx::query(
        "INSERT INTO instance_sync_preferences (instance_id, feature, enabled)
         SELECT ?, feature, new_instance_default FROM sync_feature_settings",
    )
    .bind(&instance_id)
    .execute(&mut *tx)
    .await?;
    instance_rows::set_direct_link_fields(
        &instance.id,
        &instance_rows::DirectLinkFields {
            launcher: instance.linked_launcher.clone(),
            launcher_root: instance.linked_launcher_root.clone(),
            dot_minecraft: instance.linked_dot_minecraft.clone(),
            version_id: instance.linked_version_id.clone(),
            version_json_path: instance.linked_version_json_path.clone(),
            game_dir_mode: instance.linked_game_dir_mode.clone(),
        },
        &mut tx,
    )
    .await?;
    content_rows::insert_content_set(&content_set, &mut tx).await?;
    loader_component_rows::replace_loader_components(
        &instance_id,
        &LoaderComponent::from_legacy_projection(
            instance_id.clone(),
            resolved.loader,
            None,
        ),
        &mut tx,
    )
    .await?;
    instance_rows::upsert_instance_link(
        &instance_id,
        &InstanceLink::Unmanaged,
        &mut tx,
    )
    .await?;
    let groups = direct_link_group(&resolved.dot_minecraft)
        .into_iter()
        .collect::<Vec<_>>();
    instance_rows::replace_instance_groups(&instance_id, &groups, &mut tx)
        .await?;
    instance_rows::upsert_instance_launch_overrides(&launch_overrides, &mut tx)
        .await?;
    tx.commit().await?;

    let content_root = super::sync_content_files::instance_content_root(
        &state.directories,
        &instance,
    )?;
    crate::state::instances::watcher::watch_instance_folder(
        &instance.id,
        &instance.path,
        &content_root,
        &state.file_watcher,
    )
    .await;
    if let Err(error) = crate::api::instance::reconcile_screenshots_with_state(
        &instance.id,
        state,
    )
    .await
    {
        tracing::debug!(%error, "Initial linked screenshot index reconciliation failed");
    }

    Ok(instance)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{DirectoryInfo, ModLoader};
    use serde_json::json;
    use sqlx::sqlite::SqlitePoolOptions;
    use std::path::Path;
    use std::sync::Arc;
    use tempfile::TempDir;

    fn write_version(
        root: &Path,
        folder: &str,
        json_stem: &str,
        value: serde_json::Value,
    ) -> std::io::Result<PathBuf> {
        let dir = root.join("versions").join(folder);
        std::fs::create_dir_all(&dir)?;
        let json_path = dir.join(format!("{json_stem}.json"));
        std::fs::write(&json_path, serde_json::to_vec_pretty(&value).unwrap())?;
        Ok(json_path)
    }

    async fn test_state_with_pool() -> crate::Result<(TempDir, Arc<State>)> {
        let temp = TempDir::new()?;
        let dirs = DirectoryInfo {
            settings_dir: temp.path().join("settings"),
            config_dir: temp.path().join("config"),
            app_identifier: "test".to_string(),
        };
        std::fs::create_dir_all(dirs.instances_dir())?;

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;
        sqlx::migrate!().run(&pool).await?;

        Ok((temp, crate::state::test_state(dirs, pool).await?))
    }

    #[tokio::test]
    async fn persists_resolved_direct_link_fields_without_creating_profile()
    -> crate::Result<()> {
        let (_temp, state) = test_state_with_pool().await?;
        let minecraft = TempDir::new()?;
        let json_path = write_version(
            minecraft.path(),
            "ui-folder",
            "1.20.1-forge",
            json!({
                "id": "1.20.1-forge",
                "inheritsFrom": "1.20.1",
                "mainClass": "forge.Main",
                "libraries": [
                    { "name": "net.minecraftforge:forge:1.20.1-47.4.0" }
                ]
            }),
        )?;
        let source_before = std::fs::read(&json_path)?;

        let instance = create_direct_link_instance(
            CreateDirectLinkInstance {
                name: Some("My Forge".to_string()),
                launcher_type: ImportLauncherType::Generic,
                base_path: minecraft.path().to_path_buf(),
                instance_folder: "versions/ui-folder".to_string(),
                instance_path: None,
                game_dir_mode: None,
            },
            &state,
        )
        .await?;

        assert_eq!(instance.name, "My Forge");
        assert_eq!(instance.linked_launcher.as_deref(), Some("generic"));
        assert_eq!(instance.linked_version_id.as_deref(), Some("1.20.1-forge"));
        assert_eq!(instance.install_stage, InstanceInstallStage::Installed);

        let stored =
            instance_rows::get_instance_by_id(&instance.id, &state.pool)
                .await?
                .expect("instance row");
        let canonical_root = crate::util::io::canonicalize(minecraft.path())?;
        let canonical_json = crate::util::io::canonicalize(&json_path)?;
        assert_eq!(stored.linked_launcher.as_deref(), Some("generic"));
        assert_eq!(
            stored.linked_launcher_root.as_deref(),
            Some(canonical_root.to_string_lossy().as_ref())
        );
        assert_eq!(
            stored.linked_dot_minecraft.as_deref(),
            Some(canonical_root.to_string_lossy().as_ref())
        );
        assert_eq!(
            stored.linked_version_json_path.as_deref(),
            Some(canonical_json.to_string_lossy().as_ref())
        );
        assert_eq!(stored.linked_version_id.as_deref(), Some("1.20.1-forge"));

        let metadata = instance_rows::get_instance_metadata_by_id(
            &instance.id,
            &state.pool,
        )
        .await?
        .expect("metadata row");
        assert_eq!(metadata.applied_content_set.loader, ModLoader::Forge);
        assert_eq!(metadata.applied_content_set.game_version, "1.20.1");
        // Directly associated instances intentionally project no loader
        // version: the loader is installed and managed by the external
        // launcher, so the parsed value is not shown.
        assert_eq!(metadata.applied_content_set.loader_version, None);

        assert!(
            !state
                .directories
                .instances_dir()
                .join(&instance.path)
                .exists()
        );
        assert_eq!(std::fs::read(json_path)?, source_before);

        Ok(())
    }

    #[tokio::test]
    async fn compatible_mode_fields_round_trip() -> crate::Result<()> {
        let (_temp, state) = test_state_with_pool().await?;
        let minecraft = TempDir::new()?;
        let json_path = write_version(
            minecraft.path(),
            "1.20.4",
            "1.20.4",
            json!({
                "id": "1.20.4",
                "mainClass": "net.minecraft.client.main.Main",
                "type": "release"
            }),
        )?;

        let instance = create_direct_link_instance(
            CreateDirectLinkInstance {
                name: None,
                launcher_type: ImportLauncherType::PCL2CE,
                base_path: minecraft.path().to_path_buf(),
                instance_folder: "Friendly Name".to_string(),
                instance_path: Some(
                    json_path
                        .parent()
                        .expect("version dir")
                        .to_string_lossy()
                        .to_string(),
                ),
                game_dir_mode: None,
            },
            &state,
        )
        .await?;

        assert_eq!(instance.name, "1.20.4");
        assert_eq!(instance.linked_launcher.as_deref(), Some("pcl2_ce"));
        assert_eq!(instance.linked_version_id.as_deref(), Some("1.20.4"));

        Ok(())
    }

    #[tokio::test]
    async fn inherits_enabled_sync_defaults() -> crate::Result<()> {
        let (_temp, state) = test_state_with_pool().await?;
        sqlx::query(
            "UPDATE sync_feature_settings
             SET globally_enabled = 1, new_instance_default = 1
             WHERE feature = 'multiplayer_servers'",
        )
        .execute(&state.pool)
        .await?;

        let minecraft = TempDir::new()?;
        let json_path = write_version(
            minecraft.path(),
            "1.20.4",
            "1.20.4",
            json!({
                "id": "1.20.4",
                "mainClass": "net.minecraft.client.main.Main",
                "type": "release"
            }),
        )?;

        let instance = create_direct_link_instance(
            CreateDirectLinkInstance {
                name: None,
                launcher_type: ImportLauncherType::Generic,
                base_path: minecraft.path().to_path_buf(),
                instance_folder: "1.20.4".to_string(),
                instance_path: Some(
                    json_path
                        .parent()
                        .expect("version dir")
                        .to_string_lossy()
                        .to_string(),
                ),
                game_dir_mode: None,
            },
            &state,
        )
        .await?;

        let reloaded = crate::state::get_instance(&instance.id, &state.pool)
            .await?
            .expect("created instance should reload");
        assert!(reloaded.synced_options.multiplayer_servers);
        assert!(!reloaded.synced_options.command_history);

        Ok(())
    }
}

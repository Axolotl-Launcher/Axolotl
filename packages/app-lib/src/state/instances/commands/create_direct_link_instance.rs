use crate::api::pack::import::{
    ImportLauncherType, direct_link::resolve_direct_link,
};
use crate::launcher::ExternalGameDirMode;
use crate::state::instances::{
    ContentSet, ContentSetStatus, ContentSourceKind, Instance,
    InstanceLaunchOverrides, InstanceLink, LoaderComponent,
    adapters::sqlite::{instance_rows, loader_component_rows},
};
use crate::state::{
    InstanceInstallStage, LauncherFeatureVersion, ReleaseChannel, State,
    local_id,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::info;
use uuid::Uuid;

use crate::state::instances::adapters::sqlite::{content_sets, links};

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
    create_direct_link_instance_at_root(input, None, state).await
}

pub(super) async fn create_direct_link_instance_at_root(
    input: CreateDirectLinkInstance,
    configured_root: Option<&Path>,
    state: &State,
) -> crate::Result<Instance> {
    crate::state::store::recovery_mode::ensure_writable(&state.pool)?;
    let _metadata_usage = crate::storage::usage::lease().await?;
    let mut resolved = resolve_direct_link(
        input.launcher_type,
        input.base_path,
        input.instance_folder,
        input.instance_path,
    )
    .await?;
    if let Some(root) = configured_root {
        resolved.retain_configured_root(root);
    }

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
    let instance_id = local_id(Uuid::new_v4());
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

    let mut unit = crate::state::store::atom(&state.data).await?;
    instance_rows::insert_instance(&instance, unit.transaction()).await?;
    crate::state::store::instance_sync_preferences::seed_instance_defaults(
        &mut unit,
        &instance_id,
    )
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
        unit.transaction(),
    )
    .await?;
    content_sets::insert_content_set(&content_set, unit.transaction()).await?;
    loader_component_rows::replace_loader_components(
        &instance_id,
        &LoaderComponent::from_legacy_projection(
            instance_id.clone(),
            resolved.loader,
            None,
        ),
        unit.transaction(),
    )
    .await?;
    links::upsert_instance_link(
        &instance_id,
        &InstanceLink::Unmanaged,
        unit.transaction(),
    )
    .await?;
    instance_rows::upsert_instance_launch_overrides(
        &launch_overrides,
        unit.transaction(),
    )
    .await?;
    unit.commit().await?;

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

    use crate::state::instances::adapters::sqlite::{
        content_sets, files, links,
    };

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
    async fn scanning_the_same_root_only_reports_real_changes()
    -> crate::Result<()> {
        use crate::state::instances::{
            ExternalMinecraftRoot, adapters::sqlite::manifest_rows,
            sync_direct_link_instances,
        };

        let temp = TempDir::new()?;
        let dirs = DirectoryInfo {
            settings_dir: temp.path().join("settings"),
            config_dir: temp.path().join("config"),
            app_identifier: "test".to_string(),
        };
        let pool =
            crate::state::store::databases::open(&dirs.settings_dir).await?;
        let state = crate::state::test_state(dirs, pool).await?;
        let minecraft = TempDir::new()?;
        let manifest = |version: &str| json!({ "id": "linked", "inheritsFrom": version, "mainClass": "net.minecraft.client.main.Main" });
        let source = write_version(
            minecraft.path(),
            "linked",
            "linked",
            manifest("1.20.1"),
        )?;
        let roots = vec![ExternalMinecraftRoot {
            path: minecraft.path().to_path_buf(),
            mode: ExternalGameDirMode::Isolated,
        }];

        let initial = sync_direct_link_instances(roots.clone(), &state).await?;
        assert_eq!(initial.imported, 1);
        assert!(initial.errors.is_empty(), "{:?}", initial.errors);
        let first = crate::state::list_instances(&state.pool)
            .await?
            .pop()
            .unwrap();
        let pending = manifest_rows::pending(&state.pool).await?;

        let unchanged =
            sync_direct_link_instances(roots.clone(), &state).await?;
        assert_eq!(
            (unchanged.imported, unchanged.updated, unchanged.removed),
            (0, 0, 0)
        );
        assert!(unchanged.errors.is_empty(), "{:?}", unchanged.errors);
        assert_eq!(manifest_rows::pending(&state.pool).await?, pending);

        let updated_json = serde_json::to_vec(&manifest("1.20.4"))?;
        std::fs::write(&source, &updated_json)?;
        let changed = sync_direct_link_instances(roots, &state).await?;
        assert_eq!(
            (changed.imported, changed.updated, changed.removed),
            (0, 1, 0)
        );
        assert!(changed.errors.is_empty(), "{:?}", changed.errors);
        let current =
            crate::state::get_instance(&first.instance.id, &state.pool)
                .await?
                .unwrap();
        assert_eq!(current.applied_content_set.game_version, "1.20.4");
        assert_eq!(std::fs::read(&source)?, updated_json);
        Ok(())
    }

    #[cfg(any(windows, unix))]
    #[tokio::test]
    async fn syncing_a_moved_hmcl_root_preserves_the_instance_and_content()
    -> crate::Result<()> {
        use crate::state::instances::{
            ExternalMinecraftRoot, adapters::filesystem,
            commands::instance_content_root, sync_direct_link_instances,
        };
        use crate::state::{EditInstance, InstanceFile};

        let (_data, state) = test_state_with_pool().await?;
        let external = TempDir::new()?;
        let launcher = external.path().join("hmcl");
        let original = launcher.join(".minecraft");
        let moved = external.path().join("moved").join(".minecraft");
        let hmcl_config = launcher.join(".hmcl");
        std::fs::create_dir_all(&hmcl_config)?;
        std::fs::write(
            hmcl_config.join("hmcl.json"),
            br#"{"configurations":{"Local":{"gameDir":".minecraft"}}}"#,
        )?;
        write_version(
            &original,
            "linked",
            "linked",
            json!({ "id": "linked", "inheritsFrom": "1.20.1", "mainClass": "net.minecraft.client.main.Main" }),
        )?;
        std::fs::create_dir_all(original.join("mods"))?;
        let relative_path = "mods/example.jar";
        std::fs::write(original.join(relative_path), b"mod")?;

        let configured = ExternalMinecraftRoot {
            path: original.clone(),
            mode: ExternalGameDirMode::Automatic,
        };
        let report =
            sync_direct_link_instances(vec![configured.clone()], &state)
                .await?;
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        assert_eq!(report.imported, 1);
        let first = crate::state::list_instances(&state.pool)
            .await?
            .pop()
            .unwrap();
        assert_eq!(first.instance.linked_launcher.as_deref(), Some("hmcl"));
        let id = &first.instance.id;
        crate::state::edit_instance(
            id,
            EditInstance {
                name: Some("My HMCL instance".to_string()),
                recent_time_played: Some(615),
                ..EditInstance::default()
            },
            &state.pool,
        )
        .await?;
        let now = Utc::now();
        let mut tx = state.pool.begin().await?;
        files::upsert_instance_file(
            &InstanceFile {
                id: "tracked-mod".to_string(),
                instance_id: id.clone(),
                relative_path: relative_path.to_string(),
                file_name: "example.jar".to_string(),
                enabled: true,
                sha1: "7dd30f0a95d522bfc058be4e75847f8b6df9f76b".to_string(),
                size: 3,
                missing: false,
                added_at: now,
                modified_at: now,
                local_mod_data: None,
                icon_path: None,
            },
            &mut tx,
        )
        .await?;
        tx.commit().await?;

        // The reported move takes place between launcher sessions. Release
        // the original watch before renaming, as Windows holds its handles.
        let original_watch_root =
            instance_content_root(&state.directories, &first.instance)?;
        crate::state::instances::watcher::unwatch_instance_folder(
            &first.instance.path,
            &original_watch_root,
            &state.file_watcher,
        )
        .await;

        std::fs::create_dir_all(moved.parent().unwrap())?;
        let mut renamed = std::fs::rename(&original, &moved);
        for _ in 0..20 {
            if !cfg!(windows)
                || !matches!(
                    renamed
                        .as_ref()
                        .err()
                        .and_then(std::io::Error::raw_os_error),
                    Some(5 | 32)
                )
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            renamed = std::fs::rename(&original, &moved);
        }
        renamed.expect("move the unwatched HMCL game directory");
        #[cfg(windows)]
        junction::create(&moved, &original).expect("create NTFS junction");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&moved, &original)?;

        let report =
            sync_direct_link_instances(vec![configured.clone()], &state)
                .await?;
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        assert_eq!(
            (report.imported, report.updated, report.removed),
            (0, 0, 0)
        );
        let current = crate::state::get_instance(id, &state.pool)
            .await?
            .expect("the original instance survives the move");
        assert_eq!(current.instance.name, "My HMCL instance");
        assert_eq!(current.instance.recent_time_played, 615);
        assert_eq!(current.instance.linked_launcher.as_deref(), Some("hmcl"));
        assert_eq!(
            current.instance.linked_dot_minecraft,
            first.instance.linked_dot_minecraft
        );
        assert_eq!(
            current.instance.linked_version_json_path,
            first.instance.linked_version_json_path
        );
        assert_eq!(
            current.applied_content_set.id,
            first.applied_content_set.id
        );
        let root =
            instance_content_root(&state.directories, &current.instance)?;
        assert_eq!(
            crate::util::io::canonicalize(&root)?,
            crate::util::io::canonicalize(&moved)?
        );
        assert!(
            filesystem::scan_content_files_from(&root, "")?
                .iter()
                .any(|file| file.relative_path == relative_path)
        );
        let files = files::get_instance_files(id, &state.pool).await?;
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].id, "tracked-mod");
        assert!(!files[0].missing);

        let physical = ExternalMinecraftRoot {
            path: moved.clone(),
            mode: ExternalGameDirMode::Automatic,
        };
        let unchanged = sync_direct_link_instances(
            vec![physical.clone(), configured.clone()],
            &state,
        )
        .await?;
        assert!(unchanged.errors.is_empty(), "{:?}", unchanged.errors);
        assert_eq!(
            (unchanged.imported, unchanged.updated, unchanged.removed),
            (0, 0, 0)
        );

        let conflict = sync_direct_link_instances(
            vec![
                ExternalMinecraftRoot {
                    mode: ExternalGameDirMode::Shared,
                    ..configured.clone()
                },
                ExternalMinecraftRoot {
                    mode: ExternalGameDirMode::Isolated,
                    ..physical
                },
            ],
            &state,
        )
        .await?;
        assert_eq!(conflict.errors.len(), 1);
        assert_eq!(
            (conflict.imported, conflict.updated, conflict.removed),
            (0, 0, 0)
        );

        #[cfg(windows)]
        junction::delete(&original)?;
        #[cfg(unix)]
        std::fs::remove_file(&original)?;
        std::fs::rename(&moved, &original)?;

        let other = ExternalMinecraftRoot {
            path: external.path().join("other").join(".minecraft"),
            mode: ExternalGameDirMode::Automatic,
        };
        std::fs::create_dir_all(other.path.join("versions/linked"))?;
        std::fs::copy(
            original.join("versions/linked/linked.json"),
            other.path.join("versions/linked/linked.json"),
        )?;
        let returned = sync_direct_link_instances(
            vec![configured.clone(), other.clone()],
            &state,
        )
        .await?;
        assert!(returned.errors.is_empty(), "{:?}", returned.errors);
        assert_eq!(
            (returned.imported, returned.updated, returned.removed),
            (1, 0, 0)
        );
        let original_instance = crate::state::get_instance(id, &state.pool)
            .await?
            .expect("the original association survives the round trip");
        assert_eq!(
            original_instance.applied_content_set.id,
            first.applied_content_set.id
        );
        assert_eq!(original_instance.instance.recent_time_played, 615);
        assert!(original.join(relative_path).is_file());

        let mut tx = state.pool.begin().await?;
        instance_rows::set_direct_link_fields(
            id,
            &instance_rows::DirectLinkFields {
                launcher: original_instance.instance.linked_launcher,
                launcher_root: original_instance.instance.linked_launcher_root,
                dot_minecraft: Some(moved.to_string_lossy().into_owned()),
                version_id: original_instance.instance.linked_version_id,
                version_json_path: Some(
                    moved
                        .join("versions/linked/linked.json")
                        .to_string_lossy()
                        .into_owned(),
                ),
                game_dir_mode: original_instance.instance.linked_game_dir_mode,
            },
            &mut tx,
        )
        .await?;
        tx.commit().await?;
        let uncertain =
            sync_direct_link_instances(vec![configured, other], &state).await?;
        assert_eq!(
            (uncertain.imported, uncertain.updated, uncertain.removed),
            (0, 0, 0)
        );
        assert_eq!(uncertain.errors.len(), 1);
        assert!(crate::state::get_instance(id, &state.pool).await?.is_some());
        Ok(())
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

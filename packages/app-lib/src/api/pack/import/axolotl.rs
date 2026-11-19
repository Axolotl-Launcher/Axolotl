use std::path::PathBuf;

use serde::Deserialize;

use crate::{
    State,
    install::{InstallPhaseDetails, InstallProgressReporter},
    state::{
        AppliedContentSetPatch, ContentSourceKind, EditInstance,
        InstanceInstallStage, InstanceLaunchOverridesPatch, InstanceLink,
        ModLoader, ReleaseChannel, instances::InstanceLaunchOverridesData,
    },
    util::io,
};

use super::{ImportOverrides, finish_import, generic, recache_icon};

const CONFIG_FILE_NAME: &str = "axolotl_config.json";
const CONFIG_SCHEMA_VERSION: u32 = 1;

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct AxolotlConfigFile {
    pub schema_version: u32,
    pub instance_id: String,
    pub path: String,
    pub generated_at: chrono::DateTime<chrono::Utc>,
    pub name: String,
    pub icon_path: Option<String>,
    pub update_channel: String,
    pub symlink_target: Option<String>,
    pub groups: Vec<String>,
    pub content_set: AxolotlContentSet,
    pub link: crate::state::instances::InstanceLink,
    pub launch_overrides: InstanceLaunchOverridesData,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct AxolotlContentSet {
    pub source_kind: String,
    pub game_version: String,
    pub protocol_version: Option<u32>,
    pub loader: String,
    pub loader_version: Option<String>,
}

/// Imports an Axolotl instance by reading `axolotl_config.json` and applying
/// the migratable fields to the new profile. Invalid or unsupported configs
/// fall back to the generic instance import so the game files still arrive.
pub(crate) async fn import_axolotl(
    source_path: PathBuf,
    instance_id: &str,
    reporter: InstallProgressReporter,
    details: InstallPhaseDetails,
    symlink: bool,
    overrides: &ImportOverrides,
) -> crate::Result<()> {
    let config_path = source_path.join(CONFIG_FILE_NAME);
    let content = io::read_any_encoding_to_string(&config_path)
        .await
        .unwrap_or_else(|error| {
            tracing::warn!(
                "Axolotl import: could not read {}: {error}; falling back to generic import",
                config_path.display()
            );
            (String::new(), encoding_rs::UTF_8)
        })
        .0;

    let config = match serde_json::from_str::<AxolotlConfigFile>(&content) {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(
                "Axolotl import: invalid config {}: {error}; falling back to generic import",
                config_path.display()
            );
            return generic::import_generic(
                source_path,
                instance_id,
                reporter,
                details,
                symlink,
                overrides,
                None,
            )
            .await;
        }
    };
    tracing::debug!(
        "Axolotl import: config instance_id={} path={} generated_at={} symlink_target={:?}",
        config.instance_id,
        config.path,
        config.generated_at,
        config.symlink_target
    );

    if config.schema_version != CONFIG_SCHEMA_VERSION
        || config.content_set.game_version.trim().is_empty()
    {
        tracing::warn!(
            "Axolotl import: unsupported schema or missing game version for {}; falling back to generic import",
            config_path.display()
        );
        return generic::import_generic(
            source_path,
            instance_id,
            reporter,
            details,
            symlink,
            overrides,
            None,
        )
        .await;
    }

    let icon = match config.icon_path.as_ref() {
        Some(path) => recache_icon(source_path.join(path)).await?,
        None => None,
    };
    let state = State::get().await?;
    apply_imported_config(&state, instance_id, config, icon).await?;

    finish_import(
        instance_id,
        source_path,
        &state.io_semaphore,
        reporter,
        details,
        symlink,
    )
    .await
}

async fn apply_imported_config(
    state: &State,
    instance_id: &str,
    config: AxolotlConfigFile,
    icon_path: Option<PathBuf>,
) -> crate::Result<()> {
    let source_kind =
        ContentSourceKind::from_str(&config.content_set.source_kind)
            .unwrap_or(ContentSourceKind::Local);
    let loader = ModLoader::try_from_string(&config.content_set.loader)?;
    let update_channel = ReleaseChannel::from_key(&config.update_channel);
    let link = match config.link {
        InstanceLink::Unmanaged => None,
        link => Some(link),
    };

    crate::state::edit_instance(
        instance_id,
        EditInstance {
            install_stage: Some(InstanceInstallStage::PackInstalling),
            name: Some(config.name.clone()),
            icon_path: Some(icon_path.map(|p| p.to_string_lossy().to_string())),
            update_channel: Some(update_channel),
            groups: Some(config.groups.clone()),
            link,
            content_set_patch: Some(AppliedContentSetPatch {
                source_kind: Some(source_kind),
                game_version: Some(config.content_set.game_version.clone()),
                protocol_version: Some(config.content_set.protocol_version),
                loader: Some(loader),
                loader_version: Some(config.content_set.loader_version.clone()),
            }),
            launch_overrides: Some(InstanceLaunchOverridesPatch {
                java_path: Some(config.launch_overrides.java_path.clone()),
                extra_launch_args: Some(
                    config.launch_overrides.extra_launch_args.clone(),
                ),
                custom_env_vars: Some(
                    config.launch_overrides.custom_env_vars.clone(),
                ),
                memory: Some(config.launch_overrides.memory),
                force_fullscreen: Some(
                    config.launch_overrides.force_fullscreen,
                ),
                maximize_window: Some(config.launch_overrides.maximize_window),
                game_resolution: Some(config.launch_overrides.game_resolution),
                window_title: Some(
                    config.launch_overrides.window_title.clone(),
                ),
                launch_preparation_timeout: Some(
                    config.launch_overrides.launch_preparation_timeout,
                ),
                hooks: Some(config.launch_overrides.hooks.clone()),
            }),
            ..EditInstance::default()
        },
        &state.pool,
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        install::{
            InstallJobStatus, InstallRequest,
            model::{InstallJobState, InstallTarget},
        },
        state::{CreateInstance, DirectoryInfo},
    };

    #[tokio::test]
    async fn axolotl_import_keeps_unmanaged_link_during_its_active_job() {
        let root = tempfile::tempdir().unwrap();
        let directories = DirectoryInfo {
            settings_dir: root.path().join("settings"),
            config_dir: root.path().join("config"),
            app_identifier: "test".into(),
        };
        let pool =
            crate::state::store::databases::open(&directories.settings_dir)
                .await
                .unwrap();
        let state = crate::state::test_state(directories, pool).await.unwrap();
        let instance = crate::state::create_instance(
            CreateInstance {
                name: "Pending import".into(),
                path: None,
                game_version: "unknown".into(),
                loader: ModLoader::Vanilla,
                loader_version: None,
                icon_path: None,
                link: InstanceLink::Unmanaged,
                symlink_target: None,
                game_dir_override: None,
            },
            &state,
        )
        .await
        .unwrap();
        let mut job = InstallJobState::new(InstallRequest::ImportInstance {
            launcher_type: super::super::ImportLauncherType::Axolotl,
            base_path: root.path().into(),
            instance_folder: "old-instance".into(),
            instance_path: None,
            symlink: false,
            game_version: None,
            loader: None,
            loader_version: None,
            game_dir_override: None,
        });
        job.target = InstallTarget::NewInstance {
            instance_id: Some(instance.id.clone()),
        };
        crate::install::store::insert(
            uuid::Uuid::new_v4(),
            &job,
            InstallJobStatus::Running,
            &state,
        )
        .await
        .unwrap();

        let config = serde_json::from_value(serde_json::json!({
            "schema_version": 1,
            "instance_id": "local:old-instance",
            "path": "old-instance",
            "generated_at": "2026-09-27T00:00:00Z",
            "name": "Old instance",
            "update_channel": "release",
            "groups": [],
            "content_set": {"source_kind": "local", "game_version": "1.21.1", "loader": "fabric", "loader_version": "0.19.3"},
            "link": "unmanaged",
            "launch_overrides": {}
        }))
        .unwrap();
        apply_imported_config(&state, &instance.id, config, None)
            .await
            .unwrap();
        let imported = crate::state::get_instance(&instance.id, &state.pool)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(imported.instance.name, "Old instance");
        assert_eq!(imported.applied_content_set.game_version, "1.21.1");
        assert_eq!(imported.applied_content_set.loader, ModLoader::Fabric);
        assert!(matches!(imported.link, InstanceLink::Unmanaged));

        let error = crate::state::edit_instance(
            &instance.id,
            EditInstance {
                link: Some(InstanceLink::Unmanaged),
                ..EditInstance::default()
            },
            &state.pool,
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("active instance task"));
    }
}

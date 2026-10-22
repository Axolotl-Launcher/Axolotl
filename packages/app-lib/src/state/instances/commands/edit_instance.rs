use crate::state::instances::{
    ContentSourceKind, Instance, InstanceLaunchOverrides, InstanceLink,
    LoaderComponent,
    adapters::sqlite::{
        config_sync_rows, content_rows, instance_rows, loader_component_rows,
    },
    config_sync,
};
use crate::state::{
    Hooks, InstanceInstallStage, LauncherFeatureVersion, MemorySettings,
    ModLoader, ReleaseChannel, WindowSize,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EditInstance {
    pub install_stage: Option<InstanceInstallStage>,
    pub launcher_feature_version: Option<LauncherFeatureVersion>,
    pub name: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "serde_with::rust::double_option"
    )]
    pub icon_path: Option<Option<String>>,
    pub update_channel: Option<ReleaseChannel>,
    pub groups: Option<Vec<String>>,
    pub link: Option<InstanceLink>,
    pub launch_overrides: Option<InstanceLaunchOverridesPatch>,
    pub content_set_patch: Option<AppliedContentSetPatch>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "serde_with::rust::double_option"
    )]
    pub last_played: Option<Option<DateTime<Utc>>>,
    pub submitted_time_played: Option<u64>,
    pub recent_time_played: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "serde_with::rust::double_option"
    )]
    pub symlink_target: Option<Option<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "serde_with::rust::double_option"
    )]
    pub game_dir_override: Option<Option<String>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct InstanceLaunchOverridesPatch {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "serde_with::rust::double_option"
    )]
    pub java_path: Option<Option<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "serde_with::rust::double_option"
    )]
    pub extra_launch_args: Option<Option<Vec<String>>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "serde_with::rust::double_option"
    )]
    pub custom_env_vars: Option<Option<Vec<(String, String)>>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "serde_with::rust::double_option"
    )]
    pub memory: Option<Option<MemorySettings>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "serde_with::rust::double_option"
    )]
    pub force_fullscreen: Option<Option<bool>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "serde_with::rust::double_option"
    )]
    pub maximize_window: Option<Option<bool>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "serde_with::rust::double_option"
    )]
    pub game_resolution: Option<Option<WindowSize>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "serde_with::rust::double_option"
    )]
    pub window_title: Option<Option<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "serde_with::rust::double_option"
    )]
    pub launch_preparation_timeout: Option<Option<u64>>,
    pub hooks: Option<Hooks>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AppliedContentSetPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_kind: Option<ContentSourceKind>,
    pub game_version: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "serde_with::rust::double_option"
    )]
    pub protocol_version: Option<Option<u32>>,
    pub loader: Option<ModLoader>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "serde_with::rust::double_option"
    )]
    pub loader_version: Option<Option<String>>,
}

pub(crate) async fn edit_instance(
    instance_id: &str,
    patch: EditInstance,
    pool: &SqlitePool,
) -> crate::Result<Instance> {
    let _metadata_usage = crate::storage::usage::lease().await?;
    let mut instance = instance_rows::get_instance_by_id(instance_id, pool)
        .await?
        .ok_or_else(|| {
            crate::ErrorKind::InputError("Unknown instance".to_string())
        })?;
    let now = Utc::now();

    apply_instance_patch(&mut instance, &patch, now);
    let loader_projection_changed =
        patch.content_set_patch.as_ref().is_some_and(|patch| {
            patch.loader.is_some() || patch.loader_version.is_some()
        });

    let mut content_set = match patch.content_set_patch {
        Some(content_set_patch) => {
            let applied_content_set =
                content_rows::get_applied_content_set(&instance.id, pool)
                    .await?
                    .ok_or_else(|| {
                        crate::ErrorKind::InputError(format!(
                            "Instance {} has no applied content set",
                            instance.id
                        ))
                    })?;
            Some(apply_content_set_patch(
                applied_content_set,
                content_set_patch,
                now,
            ))
        }
        None => None,
    };
    let mut launch_overrides = match patch.launch_overrides {
        Some(launch_patch) => {
            let current = instance_rows::get_instance_launch_overrides(
                &instance.id,
                pool,
            )
            .await?
            .unwrap_or_else(|| {
                InstanceLaunchOverrides::empty(instance.id.clone())
            });
            Some(apply_launch_overrides_patch(current, launch_patch))
        }
        None => None,
    };

    let mut tx = pool.begin().await?;
    instance_rows::update_instance(&instance, &mut tx).await?;

    if let Some(content_set) = content_set.as_mut() {
        content_rows::update_content_set(content_set, &mut tx).await?;
        if loader_projection_changed {
            let components = LoaderComponent::from_legacy_projection(
                instance.id.clone(),
                content_set.loader,
                content_set.loader_version.clone(),
            );
            loader_component_rows::replace_loader_components(
                &instance.id,
                &components,
                &mut tx,
            )
            .await?;
        }
    }

    if let Some(link) = &patch.link {
        instance_rows::upsert_instance_link(&instance.id, link, &mut tx)
            .await?;
        if matches!(link, InstanceLink::Unmanaged) {
            let content_set_id = content_set
                .as_ref()
                .map(|content_set| content_set.id.as_str())
                .or(instance.applied_content_set_id.as_deref());
            if let Some(content_set_id) = content_set_id {
                let belongs: i64 = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM instance_content_sets WHERE id = ? AND instance_id = ?)").bind(content_set_id).bind(&instance.id).fetch_one(&mut *tx).await?;
                if belongs == 0 {
                    return Err(crate::ErrorKind::InputError(
                        "Applied content set does not belong to this instance"
                            .into(),
                    )
                    .into());
                }
                let busy: i64 = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM install_jobs WHERE instance_id = ? AND status IN ('queued', 'running', 'canceling', 'waiting_for_user'))").bind(&instance.id).fetch_one(&mut *tx).await?;
                if busy != 0 {
                    return Err(crate::ErrorKind::InputError("Wait for the active instance task before unlinking the pack".into()).into());
                }
                sqlx::query("INSERT INTO resource_pack_unlinks(instance_id, content_set_id, unlinked_at) VALUES (?, ?, ?) ON CONFLICT(instance_id) DO UPDATE SET content_set_id = excluded.content_set_id, unlinked_at = excluded.unlinked_at").bind(&instance.id).bind(content_set_id).bind(now.timestamp()).execute(&mut *tx).await?;
                sqlx::query(
                    "UPDATE instance_content_sets SET source_kind = 'local', modified = ? WHERE id = ?",
                )
                .bind(now.timestamp())
                .bind(content_set_id)
                .execute(&mut *tx)
                .await?;
                sqlx::query(
                    "UPDATE instance_content_entries SET source_kind = 'local', ownership_kind = 'user_added', modified_at = ? WHERE content_set_id = ? AND ownership_kind = 'pack_managed'",
                )
                .bind(now.timestamp())
                .bind(content_set_id)
                .execute(&mut *tx)
                .await?;
                sqlx::query("DELETE FROM instance_pack_members WHERE content_set_id = ?")
                    .bind(content_set_id)
                    .execute(&mut *tx)
                    .await?;
                content_rows::bump_content_set_revision_in_transaction(
                    content_set_id,
                    &mut tx,
                )
                .await?;
            }
        } else {
            sqlx::query(
                "DELETE FROM resource_pack_unlinks WHERE instance_id = ?",
            )
            .bind(&instance.id)
            .execute(&mut *tx)
            .await?;
        }
    }

    if let Some(groups) = &patch.groups {
        instance_rows::replace_instance_groups(&instance.id, groups, &mut tx)
            .await?;
    }

    if let Some(overrides) = launch_overrides.as_mut() {
        instance_rows::upsert_instance_launch_overrides(overrides, &mut tx)
            .await?;
    }

    config_sync_rows::upsert_config_updated_at(&instance.id, &mut *tx).await?;
    tx.commit().await?;

    config_sync::mark_dirty(&instance.id);

    Ok(instance)
}

pub(crate) async fn restore_instance_metadata(
    metadata: &crate::state::InstanceMetadata,
    pool: &SqlitePool,
) -> crate::Result<()> {
    let mut tx = pool.begin().await?;
    let instance = metadata.instance.clone();
    let content_set = metadata.applied_content_set.clone();
    let launch_overrides = metadata.launch_overrides.clone();

    instance_rows::update_instance(&instance, &mut tx).await?;
    content_rows::update_content_set(&content_set, &mut tx).await?;
    loader_component_rows::replace_loader_components(
        &instance.id,
        &metadata.loader_components,
        &mut tx,
    )
    .await?;
    instance_rows::upsert_instance_link(&instance.id, &metadata.link, &mut tx)
        .await?;
    instance_rows::replace_instance_groups(
        &instance.id,
        &metadata.groups,
        &mut tx,
    )
    .await?;
    instance_rows::upsert_instance_launch_overrides(&launch_overrides, &mut tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

pub(crate) async fn repair_recorded_pack_unlink(
    instance_id: &str,
    pool: &SqlitePool,
) -> crate::Result<()> {
    let mut tx = pool.begin().await?;
    let set: Option<String> = sqlx::query_scalar("SELECT marker.content_set_id FROM resource_pack_unlinks marker JOIN instances instance ON instance.id = marker.instance_id LEFT JOIN instance_links link ON link.instance_id = instance.id WHERE marker.instance_id = ? AND instance.applied_content_set_id = marker.content_set_id AND COALESCE(link.link_kind, 'unmanaged') = 'unmanaged' AND (EXISTS(SELECT 1 FROM instance_pack_members WHERE content_set_id = marker.content_set_id) OR EXISTS(SELECT 1 FROM instance_content_entries WHERE content_set_id = marker.content_set_id AND ownership_kind = 'pack_managed'))").bind(instance_id).fetch_optional(&mut *tx).await?;
    if let Some(set) = set {
        sqlx::query("UPDATE instance_content_entries SET ownership_kind = 'user_added', source_kind = 'local' WHERE content_set_id = ? AND ownership_kind = 'pack_managed'").bind(&set).execute(&mut *tx).await?;
        sqlx::query(
            "DELETE FROM instance_pack_members WHERE content_set_id = ?",
        )
        .bind(&set)
        .execute(&mut *tx)
        .await?;
        sqlx::query("UPDATE instance_content_sets SET source_kind = 'local' WHERE id = ?").bind(&set).execute(&mut *tx).await?;
        content_rows::bump_content_set_revision_in_transaction(&set, &mut tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

fn apply_instance_patch(
    instance: &mut Instance,
    patch: &EditInstance,
    now: DateTime<Utc>,
) {
    if let Some(install_stage) = patch.install_stage {
        instance.install_stage = install_stage;
    }
    if let Some(launcher_feature_version) = patch.launcher_feature_version {
        instance.launcher_feature_version = launcher_feature_version;
    }
    if let Some(name) = &patch.name {
        instance.name = name.clone();
    }
    if let Some(icon_path) = &patch.icon_path {
        instance.icon_path = icon_path.clone();
    }
    if let Some(update_channel) = patch.update_channel {
        instance.update_channel = update_channel;
    }
    if let Some(last_played) = &patch.last_played {
        instance.last_played = *last_played;
    }
    if let Some(submitted_time_played) = patch.submitted_time_played {
        instance.submitted_time_played = submitted_time_played;
    }
    if let Some(recent_time_played) = patch.recent_time_played {
        instance.recent_time_played = recent_time_played;
    }
    if let Some(symlink_target) = &patch.symlink_target {
        instance.symlink_target = symlink_target.clone();
    }
    if let Some(game_dir_override) = &patch.game_dir_override {
        instance.game_dir_override = game_dir_override.clone();
    }

    instance.modified = now;
}

fn apply_content_set_patch(
    mut content_set: crate::state::instances::ContentSet,
    patch: AppliedContentSetPatch,
    now: DateTime<Utc>,
) -> crate::state::instances::ContentSet {
    if let Some(game_version) = patch.game_version {
        content_set.game_version = game_version;
    }
    if let Some(source_kind) = patch.source_kind {
        content_set.source_kind = source_kind;
    }
    if let Some(protocol_version) = patch.protocol_version {
        content_set.protocol_version = protocol_version;
    }
    if let Some(loader) = patch.loader {
        content_set.loader = loader;
    }
    if let Some(loader_version) = patch.loader_version {
        content_set.loader_version = loader_version;
    }

    content_set.modified = now;
    content_set
}

fn apply_launch_overrides_patch(
    mut overrides: InstanceLaunchOverrides,
    patch: InstanceLaunchOverridesPatch,
) -> InstanceLaunchOverrides {
    if let Some(java_path) = patch.java_path {
        overrides.java_path = java_path;
    }
    if let Some(extra_launch_args) = patch.extra_launch_args {
        overrides.extra_launch_args = extra_launch_args;
    }
    if let Some(custom_env_vars) = patch.custom_env_vars {
        overrides.custom_env_vars = custom_env_vars;
    }
    if let Some(memory) = patch.memory {
        overrides.memory = memory;
    }
    if let Some(force_fullscreen) = patch.force_fullscreen {
        overrides.force_fullscreen = force_fullscreen;
    }
    if let Some(maximize_window) = patch.maximize_window {
        overrides.maximize_window = maximize_window;
    }
    if let Some(game_resolution) = patch.game_resolution {
        overrides.game_resolution = game_resolution;
    }
    if let Some(window_title) = patch.window_title {
        overrides.window_title =
            window_title.filter(|value| !value.trim().is_empty());
    }
    if let Some(timeout) = patch.launch_preparation_timeout {
        overrides.launch_preparation_timeout = timeout;
    }
    if let Some(hooks) = patch.hooks {
        overrides.hooks = hooks;
    }

    overrides
}

//! The rows a previous launcher's profiles become here.
//!
//! One profile becomes an instance, the default content set it applies, the
//! link it recorded, its groups and its launch overrides. They are written
//! together, so a profile never leaves a half-written instance behind.

use crate::state::instances::{
    InstanceLaunchOverrides, InstanceLaunchOverridesData, playtime_to_storage,
};
use crate::state::{
    Hooks, InstanceInstallStage, LauncherFeatureVersion, MemorySettings,
    ModLoader, ReleaseChannel, WindowSize, local_id,
};
use chrono::{DateTime, Utc};
use sqlx::{Sqlite, Transaction};
use uuid::Uuid;

/// What the profile was a modpack of, in this launcher's terms.
pub(crate) enum LegacyLink {
    Unmanaged,
    ModrinthModpack {
        project_id: String,
        version_id: String,
    },
    ServerProject {
        project_id: String,
    },
}

/// One profile the previous launcher left behind.
pub(crate) struct LegacyProfile {
    pub path: String,
    pub install_stage: InstanceInstallStage,
    pub name: String,
    pub icon_path: Option<String>,
    pub game_version: String,
    pub loader: ModLoader,
    pub loader_version: Option<String>,
    pub groups: Vec<String>,
    pub link: LegacyLink,
    pub created: DateTime<Utc>,
    pub modified: DateTime<Utc>,
    pub last_played: Option<DateTime<Utc>>,
    pub submitted_time_played: u64,
    pub recent_time_played: u64,
    pub java_path: Option<String>,
    pub extra_launch_args: Option<Vec<String>>,
    pub custom_env_vars: Option<Vec<(String, String)>>,
    pub memory: Option<MemorySettings>,
    pub force_fullscreen: Option<bool>,
    pub game_resolution: Option<WindowSize>,
    pub hooks: Hooks,
}

pub(crate) async fn import_profile(
    profile: &LegacyProfile,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    // An instance id, unrelated to how content projects are identified.
    let instance_id = local_id(Uuid::new_v4());
    let content_set_id = format!("content-set:{}", Uuid::new_v4());
    let instance_id_str = instance_id.as_str();
    let content_set_id_str = content_set_id.as_str();
    let path = profile.path.as_str();
    let install_stage = profile.install_stage.as_str();
    let launcher_feature_version = LauncherFeatureVersion::None.as_str();
    let update_channel = ReleaseChannel::Release.key();
    let name = profile.name.as_str();
    let icon_path = profile.icon_path.as_deref();
    let game_version = profile.game_version.as_str();
    let loader = profile.loader.as_str();
    let loader_version = profile.loader_version.as_deref();
    let created = profile.created.timestamp();
    let modified = profile.modified.timestamp();
    let last_played = profile.last_played.map(|value| value.timestamp());
    let submitted_time_played = playtime_to_storage(
        profile.submitted_time_played,
        "submitted_time_played",
    )?;
    let recent_time_played =
        playtime_to_storage(profile.recent_time_played, "recent_time_played")?;

    sqlx::query(
        "
        INSERT OR REPLACE INTO instances (
            id,
            path,
            applied_content_set_id,
            install_stage,
            launcher_feature_version,
            update_channel,
            name,
            icon_path,
            created,
            modified,
            last_played,
            submitted_time_played,
            recent_time_played
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ",
    )
    .bind(instance_id_str)
    .bind(path)
    .bind(content_set_id_str)
    .bind(install_stage)
    .bind(launcher_feature_version)
    .bind(update_channel)
    .bind(name)
    .bind(icon_path)
    .bind(created)
    .bind(modified)
    .bind(last_played)
    .bind(submitted_time_played)
    .bind(recent_time_played)
    .execute(&mut **tx)
    .await?;

    let (
        source_kind,
        link_kind,
        modrinth_project_id,
        modrinth_version_id,
        server_project_id,
    ) = match &profile.link {
        LegacyLink::Unmanaged => ("local", "unmanaged", None, None, None),
        LegacyLink::ModrinthModpack {
            project_id,
            version_id,
        } => (
            "modrinth_modpack",
            "modrinth_modpack",
            Some(project_id.as_str()),
            Some(version_id.as_str()),
            None,
        ),
        LegacyLink::ServerProject { project_id } => (
            "server_project",
            "server_project",
            None,
            None,
            Some(project_id.as_str()),
        ),
    };

    sqlx::query(
        "
        INSERT OR REPLACE INTO instance_content_sets (
            id,
            instance_id,
            name,
            source_kind,
            status,
            game_version,
            protocol_version,
            loader,
            loader_version,
            created,
            modified
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ",
    )
    .bind(content_set_id_str)
    .bind(instance_id_str)
    .bind("Default")
    .bind(source_kind)
    .bind("available")
    .bind(game_version)
    .bind(None::<i64>)
    .bind(loader)
    .bind(loader_version)
    .bind(created)
    .bind(modified)
    .execute(&mut **tx)
    .await?;

    sqlx::query(
        "
        INSERT OR REPLACE INTO instance_links (
            instance_id,
            link_kind,
            modrinth_project_id,
            modrinth_version_id,
            server_project_id,
            content_project_id,
            content_version_id,
            hosting_server_id,
            hosting_instance_ids,
            hosting_active_instance_id,
            shared_instance_id
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, jsonb(?), ?, ?)
        ",
    )
    .bind(instance_id_str)
    .bind(link_kind)
    .bind(modrinth_project_id)
    .bind(modrinth_version_id)
    .bind(server_project_id)
    .bind(None::<&str>)
    .bind(None::<&str>)
    .bind(None::<&str>)
    .bind("[]")
    .bind(None::<&str>)
    .bind(None::<&str>)
    .execute(&mut **tx)
    .await?;

    for group in &profile.groups {
        sqlx::query(
            "
            INSERT OR IGNORE INTO instance_groups (instance_id, group_name)
            VALUES (?, ?)
            ",
        )
        .bind(instance_id_str)
        .bind(group.as_str())
        .execute(&mut **tx)
        .await?;
    }

    let overrides = InstanceLaunchOverrides {
        instance_id: instance_id.clone(),
        java_path: profile.java_path.clone(),
        extra_launch_args: profile.extra_launch_args.clone(),
        custom_env_vars: profile.custom_env_vars.clone(),
        memory: profile.memory,
        force_fullscreen: profile.force_fullscreen,
        maximize_window: None,
        game_resolution: profile.game_resolution,
        window_title: None,
        launch_preparation_timeout: None,
        hooks: profile.hooks.clone(),
    };
    let overrides_data =
        serde_json::to_string(&InstanceLaunchOverridesData::from(&overrides))?;

    sqlx::query(
        "
        INSERT OR REPLACE INTO instance_launch_overrides (
            instance_id,
            overrides
        )
        VALUES (?, jsonb(?))
        ",
    )
    .bind(instance_id_str)
    .bind(overrides_data)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

#[cfg(test)]
#[path = "legacy_import_tests.rs"]
mod tests;

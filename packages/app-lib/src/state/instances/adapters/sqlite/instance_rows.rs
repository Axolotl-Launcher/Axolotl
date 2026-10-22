#![allow(dead_code)]

use crate::state::instances::{
    ContentSet, ContentSetStatus, ContentSourceKind, Instance,
    InstanceLaunchContext, InstanceLaunchOverrides,
    InstanceLaunchOverridesData, InstanceLink, playtime_to_storage,
};
use crate::state::{
    InstanceInstallStage, LauncherFeatureVersion, ModLoader, ReleaseChannel,
};
use chrono::{DateTime, TimeZone, Utc};
use serde::de::DeserializeOwned;
use sqlx::{Executor, Sqlite, SqlitePool, Transaction};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Clone, Debug, sqlx::FromRow)]
pub(crate) struct InstanceScreenshotSource {
    pub id: String,
    pub name: String,
    pub path: String,
}

pub(crate) async fn get_instance_screenshot_source(
    instance_id: &str,
    pool: &SqlitePool,
) -> crate::Result<Option<InstanceScreenshotSource>> {
    Ok(sqlx::query_as::<_, InstanceScreenshotSource>(
        "SELECT id, name, path FROM instances WHERE id = ?",
    )
    .bind(instance_id)
    .fetch_optional(pool)
    .await?)
}

pub(crate) async fn list_screenshot_sources(
    pool: &SqlitePool,
) -> crate::Result<Vec<InstanceScreenshotSource>> {
    Ok(sqlx::query_as::<_, InstanceScreenshotSource>(
        "SELECT id, name, path FROM instances ORDER BY name, id",
    )
    .fetch_all(pool)
    .await?)
}

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct InstanceRow {
    pub id: String,
    pub path: String,
    pub applied_content_set_id: Option<String>,
    pub install_stage: String,
    pub launcher_feature_version: String,
    pub update_channel: String,
    pub name: String,
    pub icon_path: Option<String>,
    pub symlink_target: Option<String>,
    pub game_dir_override: Option<String>,
    pub created: i64,
    pub modified: i64,
    pub last_played: Option<i64>,
    pub pinned_at: Option<i64>,
    pub submitted_time_played: i64,
    pub recent_time_played: i64,
}

/// Direct-association ("直接关联") columns of an instance.
///
/// These are read through runtime queries instead of the compile-time checked
/// macros because the checked-in `.sqlx` cache was prepared against the schema
/// before these columns existed and cannot be regenerated without a live
/// database; keeping every existing macro query byte-identical avoids
/// invalidating that cache.
#[derive(Debug, Default, Clone)]
pub(crate) struct DirectLinkFields {
    pub launcher: Option<String>,
    pub launcher_root: Option<String>,
    pub dot_minecraft: Option<String>,
    pub version_id: Option<String>,
    pub version_json_path: Option<String>,
    pub game_dir_mode: Option<String>,
}

impl DirectLinkFields {
    fn apply_to(&self, instance: &mut Instance) {
        instance.linked_launcher = self.launcher.clone();
        instance.linked_launcher_root = self.launcher_root.clone();
        instance.linked_dot_minecraft = self.dot_minecraft.clone();
        instance.linked_version_id = self.version_id.clone();
        instance.linked_version_json_path = self.version_json_path.clone();
        instance.linked_game_dir_mode = self.game_dir_mode.clone();
    }
}

pub(crate) async fn get_direct_link_fields<'e, E>(
    id: &str,
    exec: E,
) -> crate::Result<DirectLinkFields>
where
    E: Executor<'e, Database = Sqlite>,
{
    let row: Option<(
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    )> = sqlx::query_as(
        "
		SELECT
			linked_launcher,
			linked_launcher_root,
			linked_dot_minecraft,
			linked_version_id,
			linked_version_json_path
			, linked_game_dir_mode
		FROM instances
		WHERE id = ?
		",
    )
    .bind(id)
    .fetch_optional(exec)
    .await?;

    Ok(row
        .map(
            |(
                launcher,
                launcher_root,
                dot_minecraft,
                version_id,
                version_json_path,
                game_dir_mode,
            )| DirectLinkFields {
                launcher,
                launcher_root,
                dot_minecraft,
                version_id,
                version_json_path,
                game_dir_mode,
            },
        )
        .unwrap_or_default())
}

/// Sets the direct-association columns of an instance.
pub(crate) async fn set_direct_link_fields(
    id: &str,
    fields: &DirectLinkFields,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    let launcher = fields.launcher.as_deref();
    let launcher_root = fields.launcher_root.as_deref();
    let dot_minecraft = fields.dot_minecraft.as_deref();
    let version_id = fields.version_id.as_deref();
    let version_json_path = fields.version_json_path.as_deref();
    let game_dir_mode = fields.game_dir_mode.as_deref();

    sqlx::query(
        "
		UPDATE instances
		SET
			linked_launcher = ?,
			linked_launcher_root = ?,
			linked_dot_minecraft = ?,
			linked_version_id = ?,
			linked_version_json_path = ?
			, linked_game_dir_mode = ?
		WHERE id = ?
		",
    )
    .bind(launcher)
    .bind(launcher_root)
    .bind(dot_minecraft)
    .bind(version_id)
    .bind(version_json_path)
    .bind(game_dir_mode)
    .bind(id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

impl TryFrom<InstanceRow> for Instance {
    type Error = crate::Error;

    fn try_from(row: InstanceRow) -> crate::Result<Self> {
        Ok(Self {
            id: row.id,
            path: row.path,
            applied_content_set_id: row.applied_content_set_id,
            install_stage: InstanceInstallStage::from_str(&row.install_stage),
            launcher_feature_version: LauncherFeatureVersion::from_str(
                &row.launcher_feature_version,
            ),
            update_channel: ReleaseChannel::from_key(&row.update_channel),
            name: row.name,
            icon_path: row.icon_path,
            symlink_target: row.symlink_target,
            linked_launcher: None,
            linked_launcher_root: None,
            linked_dot_minecraft: None,
            linked_version_id: None,
            linked_version_json_path: None,
            linked_game_dir_mode: None,
            game_dir_override: row.game_dir_override,
            created: timestamp(row.created),
            modified: timestamp(row.modified),
            last_played: row.last_played.and_then(optional_timestamp),
            pinned_at: row.pinned_at.and_then(optional_timestamp),
            submitted_time_played: unsigned(
                row.submitted_time_played,
                "submitted_time_played",
            )?,
            recent_time_played: unsigned(
                row.recent_time_played,
                "recent_time_played",
            )?,
        })
    }
}

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct InstanceLinkRow {
    pub instance_id: String,
    pub link_kind: String,
    pub modrinth_project_id: Option<String>,
    pub modrinth_version_id: Option<String>,
    pub server_project_id: Option<String>,
    pub content_project_id: Option<String>,
    pub content_version_id: Option<String>,
    pub hosting_server_id: Option<String>,
    pub hosting_instance_ids: Option<String>,
    pub hosting_active_instance_id: Option<String>,
    pub shared_instance_id: Option<String>,
    pub imported_name: Option<String>,
    pub imported_version_number: Option<String>,
    pub imported_filename: Option<String>,
}

impl TryFrom<InstanceLinkRow> for InstanceLink {
    type Error = crate::Error;

    fn try_from(row: InstanceLinkRow) -> crate::Result<Self> {
        match row.link_kind.as_str() {
            "unmanaged" => Ok(Self::Unmanaged),
            "modrinth_modpack" => Ok(Self::ModrinthModpack {
                project_id: required(
                    row.modrinth_project_id,
                    "modrinth_project_id",
                )?,
                version_id: required(
                    row.modrinth_version_id,
                    "modrinth_version_id",
                )?,
            }),
            "curseforge_modpack" => Ok(Self::CurseForgeModpack {
                project_id: required(
                    row.modrinth_project_id,
                    "modrinth_project_id",
                )?,
                version_id: required(
                    row.modrinth_version_id,
                    "modrinth_version_id",
                )?,
            }),
            "server_project" => Ok(Self::ServerProject {
                project_id: required(
                    row.server_project_id,
                    "server_project_id",
                )?,
            }),
            "server_project_modpack" => Ok(Self::ServerProjectModpack {
                server_project_id: required(
                    row.server_project_id,
                    "server_project_id",
                )?,
                content_project_id: required(
                    row.content_project_id,
                    "content_project_id",
                )?,
                content_version_id: required(
                    row.content_version_id,
                    "content_version_id",
                )?,
            }),
            "imported_modpack" => Ok(Self::ImportedModpack {
                project_id: row.modrinth_project_id,
                version_id: row.modrinth_version_id,
                name: row.imported_name,
                version_number: row.imported_version_number,
                filename: row.imported_filename,
            }),
            "shared_instance" => Ok(Self::SharedInstance {
                shared_instance_id: parse_uuid(
                    row.shared_instance_id,
                    "shared_instance_id",
                )?,
            }),
            other => Err(crate::ErrorKind::InputError(format!(
                "Unknown instance link kind {other}"
            ))
            .into()),
        }
    }
}

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct InstanceLaunchOverridesRow {
    pub instance_id: String,
    pub overrides: String,
}

#[derive(Debug)]
pub(crate) struct InstanceMetadataRecord {
    pub instance: Instance,
    pub applied_content_set: ContentSet,
    pub link: InstanceLink,
    pub groups: Vec<String>,
    pub launch_overrides: InstanceLaunchOverrides,
}

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct InstanceDisplayInfo {
    pub id: String,
    pub name: String,
}

#[derive(Debug, sqlx::FromRow)]
struct InstanceMetadataRow {
    id: String,
    path: String,
    applied_content_set_id: Option<String>,
    install_stage: String,
    launcher_feature_version: String,
    update_channel: String,
    name: String,
    icon_path: Option<String>,
    symlink_target: Option<String>,
    linked_launcher: Option<String>,
    linked_launcher_root: Option<String>,
    linked_dot_minecraft: Option<String>,
    linked_version_id: Option<String>,
    linked_version_json_path: Option<String>,
    linked_game_dir_mode: Option<String>,
    game_dir_override: Option<String>,
    created: i64,
    modified: i64,
    last_played: Option<i64>,
    pinned_at: Option<i64>,
    submitted_time_played: i64,
    recent_time_played: i64,
    content_set_id: Option<String>,
    content_set_instance_id: Option<String>,
    content_set_name: Option<String>,
    content_set_source_kind: Option<String>,
    content_set_status: Option<String>,
    content_set_game_version: Option<String>,
    content_set_protocol_version: Option<i64>,
    content_set_loader: Option<String>,
    content_set_loader_version: Option<String>,
    content_set_revision: Option<i64>,
    content_set_created: Option<i64>,
    content_set_modified: Option<i64>,
    link_kind: String,
    modrinth_project_id: Option<String>,
    modrinth_version_id: Option<String>,
    server_project_id: Option<String>,
    content_project_id: Option<String>,
    content_version_id: Option<String>,
    hosting_server_id: Option<String>,
    hosting_instance_ids: Option<String>,
    hosting_active_instance_id: Option<String>,
    shared_instance_id: Option<String>,
    imported_name: Option<String>,
    imported_version_number: Option<String>,
    imported_filename: Option<String>,
    groups: String,
    launch_overrides: Option<String>,
}

impl TryFrom<InstanceLaunchOverridesRow> for InstanceLaunchOverrides {
    type Error = crate::Error;

    fn try_from(row: InstanceLaunchOverridesRow) -> crate::Result<Self> {
        let data =
            serde_json::from_str::<InstanceLaunchOverridesData>(&row.overrides)
                .map_err(|err| {
                    crate::ErrorKind::InputError(format!(
                        "Invalid launch overrides JSON: {err}"
                    ))
                    .as_error()
                })?;

        Ok(data.into_launch_overrides(row.instance_id))
    }
}

impl InstanceMetadataRow {
    fn into_record(self) -> crate::Result<InstanceMetadataRecord> {
        let instance_id = self.id.clone();
        let direct_link = DirectLinkFields {
            launcher: self.linked_launcher.clone(),
            launcher_root: self.linked_launcher_root.clone(),
            dot_minecraft: self.linked_dot_minecraft.clone(),
            version_id: self.linked_version_id.clone(),
            version_json_path: self.linked_version_json_path.clone(),
            game_dir_mode: self.linked_game_dir_mode.clone(),
        };
        let mut instance = InstanceRow {
            id: self.id,
            path: self.path,
            applied_content_set_id: self.applied_content_set_id,
            install_stage: self.install_stage,
            launcher_feature_version: self.launcher_feature_version,
            update_channel: self.update_channel,
            name: self.name,
            icon_path: self.icon_path,
            symlink_target: self.symlink_target,
            game_dir_override: self.game_dir_override,
            created: self.created,
            modified: self.modified,
            last_played: self.last_played,
            pinned_at: self.pinned_at,
            submitted_time_played: self.submitted_time_played,
            recent_time_played: self.recent_time_played,
        }
        .try_into()?;
        direct_link.apply_to(&mut instance);
        let applied_content_set = ContentSet {
            id: required(self.content_set_id, "instance_content_sets.id")?,
            instance_id: required(
                self.content_set_instance_id,
                "instance_content_sets.instance_id",
            )?,
            name: required(
                self.content_set_name,
                "instance_content_sets.name",
            )?,
            source_kind: ContentSourceKind::from_str(&required(
                self.content_set_source_kind,
                "instance_content_sets.source_kind",
            )?)?,
            status: ContentSetStatus::from_str(&required(
                self.content_set_status,
                "instance_content_sets.status",
            )?)?,
            game_version: required(
                self.content_set_game_version,
                "instance_content_sets.game_version",
            )?,
            protocol_version: self
                .content_set_protocol_version
                .map(|value| value as u32),
            loader: ModLoader::try_from_string(&required(
                self.content_set_loader,
                "instance_content_sets.loader",
            )?)?,
            // Directly associated instances keep their loader managed by the
            // external launcher; the parsed version is not a reliable display
            // value, so it stays null for them.
            loader_version: if self.linked_launcher.is_some() {
                None
            } else {
                self.content_set_loader_version
            },
            revision: unsigned(
                required_i64(
                    self.content_set_revision,
                    "instance_content_sets.revision",
                )?,
                "instance_content_sets.revision",
            )?,
            created: timestamp(required_i64(
                self.content_set_created,
                "instance_content_sets.created",
            )?),
            modified: timestamp(required_i64(
                self.content_set_modified,
                "instance_content_sets.modified",
            )?),
        };
        let link = InstanceLinkRow {
            instance_id: instance_id.clone(),
            link_kind: self.link_kind,
            modrinth_project_id: self.modrinth_project_id,
            modrinth_version_id: self.modrinth_version_id,
            server_project_id: self.server_project_id,
            content_project_id: self.content_project_id,
            content_version_id: self.content_version_id,
            hosting_server_id: self.hosting_server_id,
            hosting_instance_ids: self.hosting_instance_ids,
            hosting_active_instance_id: self.hosting_active_instance_id,
            shared_instance_id: self.shared_instance_id,
            imported_name: self.imported_name,
            imported_version_number: self.imported_version_number,
            imported_filename: self.imported_filename,
        }
        .try_into()?;
        let groups = parse_groups(self.groups)?;
        let launch_overrides =
            launch_overrides_from_json(instance_id, self.launch_overrides)?;

        Ok(InstanceMetadataRecord {
            instance,
            applied_content_set,
            link,
            groups,
            launch_overrides,
        })
    }

    fn into_launch_context(self) -> crate::Result<InstanceLaunchContext> {
        let record = self.into_record()?;

        Ok(InstanceLaunchContext {
            instance: record.instance,
            applied_content_set: record.applied_content_set,
            link: record.link,
            launch_overrides: record.launch_overrides,
        })
    }
}

pub(crate) async fn get_instance_by_id<'e, E>(
    id: &str,
    exec: E,
) -> crate::Result<Option<Instance>>
where
    E: Executor<'e, Database = Sqlite> + Copy,
{
    let row = sqlx::query_as::<_, InstanceRow>(
        "
		SELECT
			id,
			path,
			applied_content_set_id,
			install_stage,
			launcher_feature_version,
			update_channel,
			name,
			icon_path,
			symlink_target,
			game_dir_override,
			created,
			modified,
			last_played,
			pinned_at,
			submitted_time_played,
			recent_time_played
		FROM instances
		WHERE id = ?
		",
    )
    .bind(id)
    .fetch_optional(exec)
    .await?;

    let Some(row) = row else {
        return Ok(None);
    };
    let mut instance: Instance = row.try_into()?;
    let fields = get_direct_link_fields(id, exec).await?;
    fields.apply_to(&mut instance);

    Ok(Some(instance))
}

pub(crate) async fn get_instance_by_path<'e, E>(
    path: &str,
    exec: E,
) -> crate::Result<Option<Instance>>
where
    E: Executor<'e, Database = Sqlite> + Copy,
{
    let row = sqlx::query_as::<_, InstanceRow>(
        "
		SELECT
			id,
			path,
			applied_content_set_id,
			install_stage,
			launcher_feature_version,
			update_channel,
			name,
			icon_path,
			symlink_target,
			game_dir_override,
			created,
			modified,
			last_played,
			pinned_at,
			submitted_time_played,
			recent_time_played
		FROM instances
		WHERE path = ?
		",
    )
    .bind(path)
    .fetch_optional(exec)
    .await?;

    let Some(row) = row else {
        return Ok(None);
    };
    let mut instance: Instance = row.try_into()?;
    let fields = get_direct_link_fields(&instance.id, exec).await?;
    fields.apply_to(&mut instance);

    Ok(Some(instance))
}

pub(crate) async fn get_instance_path_by_id<'e, E>(
    id: &str,
    exec: E,
) -> crate::Result<Option<String>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let path = sqlx::query_scalar!(
        "
        SELECT path
        FROM instances
        WHERE id = ?
        ",
        id,
    )
    .fetch_optional(exec)
    .await?;

    Ok(path)
}

pub(crate) async fn get_instance_path_and_game_dir_override_by_id<'e, E>(
    id: &str,
    exec: E,
) -> crate::Result<Option<(String, Option<String>)>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let row = sqlx::query!(
        "
        SELECT path AS \"path!: String\",
               game_dir_override AS \"game_dir_override?: String\"
        FROM instances
        WHERE id = ?
        ",
        id,
    )
    .fetch_optional(exec)
    .await?;

    Ok(row.map(|r| (r.path, r.game_dir_override)))
}

/// Look up the game-dir override for an instance matched by its relative
/// `path` (the file hash cache keys embed `instance.path`, not the instance id).
pub(crate) async fn get_game_dir_override_by_path<'e, E>(
    instance_path: &str,
    exec: E,
) -> crate::Result<Option<String>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let row = sqlx::query!(
        "
        SELECT game_dir_override AS \"game_dir_override?: String\"
        FROM instances
        WHERE path = ?
        ",
        instance_path,
    )
    .fetch_optional(exec)
    .await?;

    Ok(row.and_then(|r| r.game_dir_override))
}

pub(crate) async fn get_instance_display_info<'e, E>(
    id: &str,
    exec: E,
) -> crate::Result<Option<InstanceDisplayInfo>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let row = sqlx::query_as!(
        InstanceDisplayInfo,
        "
        SELECT id, name
        FROM instances
        WHERE id = ?
        ",
        id,
    )
    .fetch_optional(exec)
    .await?;

    Ok(row)
}

pub(crate) async fn get_instance_metadata_by_id(
    id: &str,
    pool: &SqlitePool,
) -> crate::Result<Option<InstanceMetadataRecord>> {
    let row = sqlx::query_as::<_, InstanceMetadataRow>(
        r#"
        SELECT
            i.id,
            i.path,
            i.applied_content_set_id,
            i.install_stage,
            i.launcher_feature_version,
            i.update_channel,
            i.name,
            i.icon_path,
            i.symlink_target,
            i.linked_launcher,
            i.linked_launcher_root,
            i.linked_dot_minecraft,
            i.linked_version_id,
            i.linked_version_json_path,
            i.linked_game_dir_mode,
            i.game_dir_override,
            i.created,
            i.modified,
            i.last_played,
			i.pinned_at,
            i.submitted_time_played,
            i.recent_time_played,
            cs.id AS content_set_id,
            cs.instance_id AS content_set_instance_id,
            cs.name AS content_set_name,
            cs.source_kind AS content_set_source_kind,
            cs.status AS content_set_status,
            cs.game_version AS content_set_game_version,
            cs.protocol_version AS content_set_protocol_version,
            cs.loader AS content_set_loader,
            cs.loader_version AS content_set_loader_version,
            cs.revision AS content_set_revision,
            cs.created AS content_set_created,
            cs.modified AS content_set_modified,
            COALESCE(link.link_kind, 'unmanaged') AS link_kind,
            link.modrinth_project_id,
            link.modrinth_version_id,
            link.server_project_id,
            link.content_project_id,
            link.content_version_id,
            link.hosting_server_id,
            json(link.hosting_instance_ids) AS hosting_instance_ids,
            link.hosting_active_instance_id,
            link.shared_instance_id,
            link.imported_name,
            link.imported_version_number,
            link.imported_filename,
            COALESCE((
                SELECT json_group_array(group_name)
                FROM (
                    SELECT group_name
                    FROM instance_groups
                    WHERE instance_id = i.id
                    ORDER BY group_name
                )
            ), '[]') AS groups,
            json(overrides.overrides) AS launch_overrides
        FROM instances i
        LEFT JOIN instance_content_sets cs
            ON cs.id = i.applied_content_set_id
            AND cs.instance_id = i.id
        LEFT JOIN instance_links link
            ON link.instance_id = i.id
        LEFT JOIN instance_launch_overrides overrides
            ON overrides.instance_id = i.id
        WHERE i.id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    row.map(InstanceMetadataRow::into_record).transpose()
}

pub(crate) async fn get_instance_metadata_many(
    ids: &[&str],
    pool: &SqlitePool,
) -> crate::Result<Vec<InstanceMetadataRecord>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }

    let ids_json = serde_json::to_string(ids)?;
    let rows = sqlx::query_as::<_, InstanceMetadataRow>(
        r#"
        WITH requested AS (
            SELECT value AS id, key AS ord
            FROM json_each(?)
        )
        SELECT
            i.id,
            i.path,
            i.applied_content_set_id,
            i.install_stage,
            i.launcher_feature_version,
            i.update_channel,
            i.name,
            i.icon_path,
            i.symlink_target,
            i.linked_launcher,
            i.linked_launcher_root,
            i.linked_dot_minecraft,
            i.linked_version_id,
            i.linked_version_json_path,
            i.linked_game_dir_mode,
            i.game_dir_override,
            i.created,
            i.modified,
            i.last_played,
			i.pinned_at,
            i.submitted_time_played,
            i.recent_time_played,
            cs.id AS content_set_id,
            cs.instance_id AS content_set_instance_id,
            cs.name AS content_set_name,
            cs.source_kind AS content_set_source_kind,
            cs.status AS content_set_status,
            cs.game_version AS content_set_game_version,
            cs.protocol_version AS content_set_protocol_version,
            cs.loader AS content_set_loader,
            cs.loader_version AS content_set_loader_version,
            cs.revision AS content_set_revision,
            cs.created AS content_set_created,
            cs.modified AS content_set_modified,
            COALESCE(link.link_kind, 'unmanaged') AS link_kind,
            link.modrinth_project_id,
            link.modrinth_version_id,
            link.server_project_id,
            link.content_project_id,
            link.content_version_id,
            link.hosting_server_id,
            json(link.hosting_instance_ids) AS hosting_instance_ids,
            link.hosting_active_instance_id,
            link.shared_instance_id,
            link.imported_name,
            link.imported_version_number,
            link.imported_filename,
            COALESCE((
                SELECT json_group_array(group_name)
                FROM (
                    SELECT group_name
                    FROM instance_groups
                    WHERE instance_id = i.id
                    ORDER BY group_name
                )
            ), '[]') AS groups,
            json(overrides.overrides) AS launch_overrides
        FROM requested
        INNER JOIN instances i
            ON i.id = requested.id
        LEFT JOIN instance_content_sets cs
            ON cs.id = i.applied_content_set_id
            AND cs.instance_id = i.id
        LEFT JOIN instance_links link
            ON link.instance_id = i.id
        LEFT JOIN instance_launch_overrides overrides
            ON overrides.instance_id = i.id
        ORDER BY requested.ord
        "#,
    )
    .bind(ids_json)
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(InstanceMetadataRow::into_record)
        .collect()
}

pub(crate) async fn list_instance_metadata(
    pool: &SqlitePool,
) -> crate::Result<Vec<InstanceMetadataRecord>> {
    let rows = sqlx::query_as::<_, InstanceMetadataRow>(
        r#"
        SELECT
            i.id,
            i.path,
            i.applied_content_set_id,
            i.install_stage,
            i.launcher_feature_version,
            i.update_channel,
            i.name,
            i.icon_path,
            i.symlink_target,
            i.linked_launcher,
            i.linked_launcher_root,
            i.linked_dot_minecraft,
            i.linked_version_id,
            i.linked_version_json_path,
            i.linked_game_dir_mode,
            i.game_dir_override,
            i.created,
            i.modified,
            i.last_played,
			i.pinned_at,
            i.submitted_time_played,
            i.recent_time_played,
            cs.id AS content_set_id,
            cs.instance_id AS content_set_instance_id,
            cs.name AS content_set_name,
            cs.source_kind AS content_set_source_kind,
            cs.status AS content_set_status,
            cs.game_version AS content_set_game_version,
            cs.protocol_version AS content_set_protocol_version,
            cs.loader AS content_set_loader,
            cs.loader_version AS content_set_loader_version,
            cs.revision AS content_set_revision,
            cs.created AS content_set_created,
            cs.modified AS content_set_modified,
            COALESCE(link.link_kind, 'unmanaged') AS link_kind,
            link.modrinth_project_id,
            link.modrinth_version_id,
            link.server_project_id,
            link.content_project_id,
            link.content_version_id,
            link.hosting_server_id,
            json(link.hosting_instance_ids) AS hosting_instance_ids,
            link.hosting_active_instance_id,
            link.shared_instance_id,
            link.imported_name,
            link.imported_version_number,
            link.imported_filename,
            COALESCE((
                SELECT json_group_array(group_name)
                FROM (
                    SELECT group_name
                    FROM instance_groups
                    WHERE instance_id = i.id
                    ORDER BY group_name
                )
            ), '[]') AS groups,
            json(overrides.overrides) AS launch_overrides
        FROM instances i
        LEFT JOIN instance_content_sets cs
            ON cs.id = i.applied_content_set_id
            AND cs.instance_id = i.id
        LEFT JOIN instance_links link
            ON link.instance_id = i.id
        LEFT JOIN instance_launch_overrides overrides
            ON overrides.instance_id = i.id
        "#,
    )
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(InstanceMetadataRow::into_record)
        .collect()
}

pub(crate) async fn get_instance_launch_context(
    instance_id: &str,
    pool: &SqlitePool,
) -> crate::Result<Option<InstanceLaunchContext>> {
    let row = sqlx::query_as::<_, InstanceMetadataRow>(
        r#"
        SELECT
            i.id,
            i.path,
            i.applied_content_set_id,
            i.install_stage,
            i.launcher_feature_version,
            i.update_channel,
            i.name,
            i.icon_path,
            i.symlink_target,
            i.linked_launcher,
            i.linked_launcher_root,
            i.linked_dot_minecraft,
            i.linked_version_id,
            i.linked_version_json_path,
            i.linked_game_dir_mode,
            i.game_dir_override,
            i.created,
            i.modified,
            i.last_played,
			i.pinned_at,
            i.submitted_time_played,
            i.recent_time_played,
            cs.id AS content_set_id,
            cs.instance_id AS content_set_instance_id,
            cs.name AS content_set_name,
            cs.source_kind AS content_set_source_kind,
            cs.status AS content_set_status,
            cs.game_version AS content_set_game_version,
            cs.protocol_version AS content_set_protocol_version,
            cs.loader AS content_set_loader,
            cs.loader_version AS content_set_loader_version,
            cs.revision AS content_set_revision,
            cs.created AS content_set_created,
            cs.modified AS content_set_modified,
            COALESCE(link.link_kind, 'unmanaged') AS link_kind,
            link.modrinth_project_id,
            link.modrinth_version_id,
            link.server_project_id,
            link.content_project_id,
            link.content_version_id,
            link.hosting_server_id,
            json(link.hosting_instance_ids) AS hosting_instance_ids,
            link.hosting_active_instance_id,
            link.shared_instance_id,
            link.imported_name,
            link.imported_version_number,
            link.imported_filename,
            '[]' AS groups,
            json(overrides.overrides) AS launch_overrides
        FROM instances i
        LEFT JOIN instance_content_sets cs
            ON cs.id = i.applied_content_set_id
            AND cs.instance_id = i.id
        LEFT JOIN instance_links link
            ON link.instance_id = i.id
        LEFT JOIN instance_launch_overrides overrides
            ON overrides.instance_id = i.id
        WHERE i.id = ?
        "#,
    )
    .bind(instance_id)
    .fetch_optional(pool)
    .await?;

    row.map(InstanceMetadataRow::into_launch_context)
        .transpose()
}

pub(crate) async fn list_instances(
    pool: &SqlitePool,
) -> crate::Result<Vec<Instance>> {
    let rows = sqlx::query_as::<_, InstanceRow>(
        "
		SELECT
			id,
			path,
			applied_content_set_id,
			install_stage,
			launcher_feature_version,
			update_channel,
			name,
			icon_path,
			symlink_target,
			game_dir_override,
			created,
			modified,
			last_played,
			pinned_at,
			submitted_time_played,
			recent_time_played
		FROM instances
		",
    )
    .fetch_all(pool)
    .await?;

    let mut instances = Vec::with_capacity(rows.len());
    for row in rows {
        let mut instance: Instance = row.try_into()?;
        let fields = get_direct_link_fields(&instance.id, pool).await?;
        fields.apply_to(&mut instance);
        instances.push(instance);
    }

    Ok(instances)
}

pub(crate) async fn get_instance_link<'e, E>(
    instance_id: &str,
    exec: E,
) -> crate::Result<InstanceLink>
where
    E: Executor<'e, Database = Sqlite>,
{
    let row = sqlx::query_as!(
        InstanceLinkRow,
        r#"
        SELECT
            instance_id,
            link_kind,
            modrinth_project_id,
            modrinth_version_id,
            server_project_id,
            content_project_id,
            content_version_id,
            hosting_server_id,
            json(hosting_instance_ids) AS "hosting_instance_ids?: String",
            hosting_active_instance_id,
            shared_instance_id,
            imported_name,
            imported_version_number,
            imported_filename
        FROM instance_links
        WHERE instance_id = ?
        "#,
        instance_id,
    )
    .fetch_optional(exec)
    .await?;

    match row {
        Some(row) => row.try_into(),
        None => Ok(InstanceLink::Unmanaged),
    }
}

pub(crate) async fn get_instance_groups<'e, E>(
    instance_id: &str,
    exec: E,
) -> crate::Result<Vec<String>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let rows = sqlx::query_scalar!(
        "
		SELECT group_name
		FROM instance_groups
		WHERE instance_id = ?
		ORDER BY group_name
		",
        instance_id,
    )
    .fetch_all(exec)
    .await?;

    Ok(rows)
}

pub(crate) async fn get_instance_launch_overrides<'e, E>(
    instance_id: &str,
    exec: E,
) -> crate::Result<Option<InstanceLaunchOverrides>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let row = sqlx::query_as!(
        InstanceLaunchOverridesRow,
        r#"
		SELECT
			instance_id,
			json(overrides) AS "overrides!: String"
		FROM instance_launch_overrides
		WHERE instance_id = ?
		"#,
        instance_id,
    )
    .fetch_optional(exec)
    .await?;

    row.map(TryInto::try_into).transpose()
}

pub(crate) async fn insert_instance(
    instance: &Instance,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    let id = instance.id.as_str();
    let path = instance.path.as_str();
    let applied_content_set_id = instance.applied_content_set_id.as_deref();
    let install_stage = instance.install_stage.as_str();
    let launcher_feature_version = instance.launcher_feature_version.as_str();
    let update_channel = instance.update_channel.key();
    let name = instance.name.as_str();
    let icon_path = instance.icon_path.as_deref();
    let symlink_target = instance.symlink_target.as_deref();
    let game_dir_override = instance.game_dir_override.as_deref();
    let created = instance.created.timestamp();
    let modified = instance.modified.timestamp();
    let last_played = instance.last_played.map(|value| value.timestamp());
    let pinned_at = instance.pinned_at.map(|value| value.timestamp());
    let submitted_time_played = playtime_to_storage(
        instance.submitted_time_played,
        "submitted_time_played",
    )?;
    let recent_time_played =
        playtime_to_storage(instance.recent_time_played, "recent_time_played")?;

    sqlx::query!(
        "
		INSERT INTO instances (
			id,
			path,
			applied_content_set_id,
			install_stage,
			launcher_feature_version,
			update_channel,
			name,
			icon_path,
			symlink_target,
			game_dir_override,
			created,
			modified,
			last_played,
			pinned_at,
			submitted_time_played,
			recent_time_played
		)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
		",
        id,
        path,
        applied_content_set_id,
        install_stage,
        launcher_feature_version,
        update_channel,
        name,
        icon_path,
        symlink_target,
        game_dir_override,
        created,
        modified,
        last_played,
        pinned_at,
        submitted_time_played,
        recent_time_played,
    )
    .execute(&mut **tx)
    .await?;

    Ok(())
}

pub(crate) async fn update_instance(
    instance: &Instance,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    let id = instance.id.as_str();
    let path = instance.path.as_str();
    let applied_content_set_id = instance.applied_content_set_id.as_deref();
    let install_stage = instance.install_stage.as_str();
    let launcher_feature_version = instance.launcher_feature_version.as_str();
    let update_channel = instance.update_channel.key();
    let name = instance.name.as_str();
    let icon_path = instance.icon_path.as_deref();
    let symlink_target = instance.symlink_target.as_deref();
    let game_dir_override = instance.game_dir_override.as_deref();
    let modified = instance.modified.timestamp();
    let last_played = instance.last_played.map(|value| value.timestamp());
    let pinned_at = instance.pinned_at.map(|value| value.timestamp());
    let submitted_time_played = playtime_to_storage(
        instance.submitted_time_played,
        "submitted_time_played",
    )?;
    let recent_time_played =
        playtime_to_storage(instance.recent_time_played, "recent_time_played")?;

    sqlx::query!(
        "
		UPDATE instances
		SET
			path = ?,
			applied_content_set_id = ?,
			install_stage = ?,
			launcher_feature_version = ?,
			update_channel = ?,
			name = ?,
			icon_path = ?,
			symlink_target = ?,
			game_dir_override = ?,
			modified = ?,
			last_played = ?,
			pinned_at = ?,
			submitted_time_played = ?,
			recent_time_played = ?
		WHERE id = ?
		",
        path,
        applied_content_set_id,
        install_stage,
        launcher_feature_version,
        update_channel,
        name,
        icon_path,
        symlink_target,
        game_dir_override,
        modified,
        last_played,
        pinned_at,
        submitted_time_played,
        recent_time_played,
        id,
    )
    .execute(&mut **tx)
    .await?;

    Ok(())
}

pub(crate) async fn upsert_instance_link(
    instance_id: &str,
    link: &InstanceLink,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    let columns = instance_link_columns(link)?;
    if !matches!(link, InstanceLink::Unmanaged) {
        sqlx::query("DELETE FROM resource_pack_unlinks WHERE instance_id = ?")
            .bind(instance_id)
            .execute(&mut **tx)
            .await?;
    }
    let modrinth_project_id = columns.modrinth_project_id.as_deref();
    let modrinth_version_id = columns.modrinth_version_id.as_deref();
    let server_project_id = columns.server_project_id.as_deref();
    let content_project_id = columns.content_project_id.as_deref();
    let content_version_id = columns.content_version_id.as_deref();
    let hosting_server_id = columns.hosting_server_id.as_deref();
    let hosting_instance_ids = columns.hosting_instance_ids.as_deref();
    let hosting_active_instance_id =
        columns.hosting_active_instance_id.as_deref();
    let shared_instance_id = columns.shared_instance_id.as_deref();
    let imported_name = columns.imported_name.as_deref();
    let imported_version_number = columns.imported_version_number.as_deref();
    let imported_filename = columns.imported_filename.as_deref();

    sqlx::query!(
        "
		INSERT INTO instance_links (
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
			shared_instance_id,
			imported_name,
			imported_version_number,
			imported_filename
		)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?, jsonb(?), ?, ?, ?, ?, ?)
		ON CONFLICT (instance_id) DO UPDATE SET
			link_kind = excluded.link_kind,
			modrinth_project_id = excluded.modrinth_project_id,
			modrinth_version_id = excluded.modrinth_version_id,
			server_project_id = excluded.server_project_id,
			content_project_id = excluded.content_project_id,
			content_version_id = excluded.content_version_id,
			hosting_server_id = excluded.hosting_server_id,
			hosting_instance_ids = excluded.hosting_instance_ids,
			hosting_active_instance_id = excluded.hosting_active_instance_id,
			shared_instance_id = excluded.shared_instance_id,
			imported_name = excluded.imported_name,
			imported_version_number = excluded.imported_version_number,
			imported_filename = excluded.imported_filename
		",
        instance_id,
        columns.link_kind,
        modrinth_project_id,
        modrinth_version_id,
        server_project_id,
        content_project_id,
        content_version_id,
        hosting_server_id,
        hosting_instance_ids,
        hosting_active_instance_id,
        shared_instance_id,
        imported_name,
        imported_version_number,
        imported_filename,
    )
    .execute(&mut **tx)
    .await?;

    Ok(())
}

pub(crate) async fn replace_instance_groups(
    instance_id: &str,
    groups: &[String],
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    sqlx::query!(
        "
		DELETE FROM instance_groups
		WHERE instance_id = ?
		",
        instance_id,
    )
    .execute(&mut **tx)
    .await?;

    for group in groups {
        sqlx::query!(
            "
			INSERT OR IGNORE INTO instance_groups (instance_id, group_name)
			VALUES (?, ?)
			",
            instance_id,
            group,
        )
        .execute(&mut **tx)
        .await?;
    }

    Ok(())
}

pub(crate) async fn upsert_instance_launch_overrides(
    overrides: &InstanceLaunchOverrides,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    let overrides_data =
        serde_json::to_string(&InstanceLaunchOverridesData::from(overrides))?;
    let instance_id = overrides.instance_id.as_str();

    sqlx::query!(
        "
		INSERT INTO instance_launch_overrides (
			instance_id,
			overrides
		)
		VALUES (?, jsonb(?))
		ON CONFLICT (instance_id) DO UPDATE SET
			overrides = excluded.overrides
		",
        instance_id,
        overrides_data,
    )
    .execute(&mut **tx)
    .await?;

    Ok(())
}

pub(crate) async fn delete_instance_by_id(
    instance_id: &str,
    pool: &SqlitePool,
) -> crate::Result<()> {
    sqlx::query!(
        "
		DELETE FROM instances
		WHERE id = ?
		",
        instance_id,
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub(crate) async fn set_instance_sync_preference(
    instance_id: &str,
    option: crate::state::SyncedOption,
    enabled: bool,
    pool: &SqlitePool,
) -> crate::Result<()> {
    sqlx::query(
        "INSERT INTO instance_sync_preferences (instance_id, feature, enabled)
         VALUES (?, ?, ?)
         ON CONFLICT(instance_id, feature) DO UPDATE SET enabled = excluded.enabled",
    )
    .bind(instance_id)
    .bind(option.as_str())
    .bind(enabled)
    .execute(pool)
    .await?;
    Ok(())
}

pub(crate) async fn get_instance_synced_options(
    instance_id: &str,
    pool: &SqlitePool,
) -> crate::Result<crate::state::InstanceSyncedOptions> {
    let rows = sqlx::query("SELECT feature, enabled FROM instance_sync_preferences WHERE instance_id = ?")
        .bind(instance_id).fetch_all(pool).await?;
    let mut options = crate::state::InstanceSyncedOptions::default();
    for row in rows {
        let feature: String = sqlx::Row::get(&row, "feature");
        let enabled: bool = sqlx::Row::get(&row, "enabled");
        apply_sync_preference(&mut options, &feature, enabled);
    }
    Ok(options)
}

pub(crate) async fn get_instance_synced_options_many(
    instance_ids: &[&str],
    pool: &SqlitePool,
) -> crate::Result<HashMap<String, crate::state::InstanceSyncedOptions>> {
    use sqlx::Row;
    let rows = sqlx::query(
        "SELECT instance_id, feature, enabled FROM instance_sync_preferences
         WHERE instance_id IN (SELECT value FROM json_each(?))",
    )
    .bind(serde_json::to_string(instance_ids)?)
    .fetch_all(pool)
    .await?;
    let mut grouped = HashMap::new();
    for row in rows {
        let id: String = row.try_get("instance_id")?;
        let feature: String = row.try_get("feature")?;
        let enabled: bool = row.try_get("enabled")?;
        apply_sync_preference(
            grouped.entry(id).or_default(),
            &feature,
            enabled,
        );
    }
    Ok(grouped)
}

fn apply_sync_preference(
    options: &mut crate::state::InstanceSyncedOptions,
    feature: &str,
    enabled: bool,
) {
    match feature {
        "game_options" => options.game_options = enabled,
        "command_history" => options.command_history = enabled,
        "multiplayer_servers" => options.multiplayer_servers = enabled,
        "creative_hotbars" => options.creative_hotbars = enabled,
        "screenshots" => options.screenshots = enabled,
        "resource_packs" => options.resource_packs = enabled,
        "data_packs" => options.data_packs = enabled,
        _ => {}
    }
}

struct InstanceLinkColumns {
    link_kind: &'static str,
    modrinth_project_id: Option<String>,
    modrinth_version_id: Option<String>,
    server_project_id: Option<String>,
    content_project_id: Option<String>,
    content_version_id: Option<String>,
    hosting_server_id: Option<String>,
    hosting_instance_ids: Option<String>,
    hosting_active_instance_id: Option<String>,
    shared_instance_id: Option<String>,
    imported_name: Option<String>,
    imported_version_number: Option<String>,
    imported_filename: Option<String>,
}

fn instance_link_columns(
    link: &InstanceLink,
) -> crate::Result<InstanceLinkColumns> {
    match link {
        InstanceLink::Unmanaged => Ok(InstanceLinkColumns {
            link_kind: "unmanaged",
            modrinth_project_id: None,
            modrinth_version_id: None,
            server_project_id: None,
            content_project_id: None,
            content_version_id: None,
            hosting_server_id: None,
            hosting_instance_ids: None,
            hosting_active_instance_id: None,
            shared_instance_id: None,
            imported_name: None,
            imported_version_number: None,
            imported_filename: None,
        }),
        InstanceLink::ModrinthModpack {
            project_id,
            version_id,
        } => Ok(InstanceLinkColumns {
            link_kind: "modrinth_modpack",
            modrinth_project_id: Some(project_id.clone()),
            modrinth_version_id: Some(version_id.clone()),
            server_project_id: None,
            content_project_id: None,
            content_version_id: None,
            hosting_server_id: None,
            hosting_instance_ids: None,
            hosting_active_instance_id: None,
            shared_instance_id: None,
            imported_name: None,
            imported_version_number: None,
            imported_filename: None,
        }),
        InstanceLink::CurseForgeModpack {
            project_id,
            version_id,
        } => Ok(InstanceLinkColumns {
            link_kind: "curseforge_modpack",
            modrinth_project_id: Some(project_id.clone()),
            modrinth_version_id: Some(version_id.clone()),
            server_project_id: None,
            content_project_id: None,
            content_version_id: None,
            hosting_server_id: None,
            hosting_instance_ids: None,
            hosting_active_instance_id: None,
            shared_instance_id: None,
            imported_name: None,
            imported_version_number: None,
            imported_filename: None,
        }),
        InstanceLink::ServerProject { project_id } => Ok(InstanceLinkColumns {
            link_kind: "server_project",
            modrinth_project_id: None,
            modrinth_version_id: None,
            server_project_id: Some(project_id.clone()),
            content_project_id: None,
            content_version_id: None,
            hosting_server_id: None,
            hosting_instance_ids: None,
            hosting_active_instance_id: None,
            shared_instance_id: None,
            imported_name: None,
            imported_version_number: None,
            imported_filename: None,
        }),
        InstanceLink::ServerProjectModpack {
            server_project_id,
            content_project_id,
            content_version_id,
        } => Ok(InstanceLinkColumns {
            link_kind: "server_project_modpack",
            modrinth_project_id: None,
            modrinth_version_id: None,
            server_project_id: Some(server_project_id.clone()),
            content_project_id: Some(content_project_id.clone()),
            content_version_id: Some(content_version_id.clone()),
            hosting_server_id: None,
            hosting_instance_ids: None,
            hosting_active_instance_id: None,
            shared_instance_id: None,
            imported_name: None,
            imported_version_number: None,
            imported_filename: None,
        }),
        InstanceLink::ImportedModpack {
            project_id,
            version_id,
            name,
            version_number,
            filename,
        } => Ok(InstanceLinkColumns {
            link_kind: "imported_modpack",
            modrinth_project_id: project_id.clone(),
            modrinth_version_id: version_id.clone(),
            server_project_id: None,
            content_project_id: None,
            content_version_id: None,
            hosting_server_id: None,
            hosting_instance_ids: None,
            hosting_active_instance_id: None,
            shared_instance_id: None,
            imported_name: name.clone(),
            imported_version_number: version_number.clone(),
            imported_filename: filename.clone(),
        }),
        InstanceLink::SharedInstance { shared_instance_id } => {
            Ok(InstanceLinkColumns {
                link_kind: "shared_instance",
                modrinth_project_id: None,
                modrinth_version_id: None,
                server_project_id: None,
                content_project_id: None,
                content_version_id: None,
                hosting_server_id: None,
                hosting_instance_ids: None,
                hosting_active_instance_id: None,
                shared_instance_id: Some(shared_instance_id.to_string()),
                imported_name: None,
                imported_version_number: None,
                imported_filename: None,
            })
        }
    }
}

fn required(value: Option<String>, column: &str) -> crate::Result<String> {
    value.ok_or_else(|| {
        crate::ErrorKind::InputError(format!(
            "Missing required instance link column {column}"
        ))
        .into()
    })
}

fn required_i64(value: Option<i64>, column: &str) -> crate::Result<i64> {
    value.ok_or_else(|| {
        crate::ErrorKind::InputError(format!(
            "Missing required instance metadata column {column}"
        ))
        .into()
    })
}

fn parse_groups(value: String) -> crate::Result<Vec<String>> {
    serde_json::from_str(&value).map_err(|err| {
        crate::ErrorKind::InputError(format!(
            "Invalid instance groups JSON: {err}"
        ))
        .into()
    })
}

fn launch_overrides_from_json(
    instance_id: String,
    value: Option<String>,
) -> crate::Result<InstanceLaunchOverrides> {
    match value {
        Some(overrides) if overrides != "null" => InstanceLaunchOverridesRow {
            instance_id,
            overrides,
        }
        .try_into(),
        _ => Ok(InstanceLaunchOverrides::empty(instance_id)),
    }
}

fn parse_uuid(value: Option<String>, column: &str) -> crate::Result<Uuid> {
    let value = required(value, column)?;

    value.parse().map_err(|err| {
        crate::ErrorKind::InputError(format!("Invalid {column}: {err}")).into()
    })
}

fn parse_optional_uuid(
    value: Option<String>,
    column: &str,
) -> crate::Result<Option<Uuid>> {
    value
        .map(|value| {
            value.parse().map_err(|err| {
                crate::ErrorKind::InputError(format!("Invalid {column}: {err}"))
                    .into()
            })
        })
        .transpose()
}

fn parse_optional_json<T>(
    value: Option<String>,
    column: &str,
) -> crate::Result<Option<T>>
where
    T: DeserializeOwned,
{
    let Some(value) = value else {
        return Ok(None);
    };

    if value == "null" {
        return Ok(None);
    }

    serde_json::from_str(&value).map(Some).map_err(|err| {
        crate::ErrorKind::InputError(format!(
            "Invalid launch override JSON in {column}: {err}"
        ))
        .into()
    })
}

fn timestamp(value: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(value, 0)
        .single()
        .unwrap_or_else(Utc::now)
}

fn optional_timestamp(value: i64) -> Option<DateTime<Utc>> {
    Utc.timestamp_opt(value, 0).single()
}

fn unsigned(value: i64, column: &str) -> crate::Result<u64> {
    if value < 0 {
        return Err(crate::ErrorKind::InputError(format!(
            "Expected {column} to be non-negative"
        ))
        .into());
    }

    Ok(value as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    #[tokio::test]
    async fn metadata_queries_preserve_applied_content_set_revision() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::raw_sql(
            r#"
            CREATE TABLE instances (
                id TEXT PRIMARY KEY,
                path TEXT NOT NULL,
                applied_content_set_id TEXT,
                install_stage TEXT NOT NULL,
                launcher_feature_version TEXT NOT NULL,
                update_channel TEXT NOT NULL,
                name TEXT NOT NULL,
                icon_path TEXT,
                symlink_target TEXT,
                linked_launcher TEXT,
                linked_launcher_root TEXT,
                linked_dot_minecraft TEXT,
                linked_version_id TEXT,
                linked_version_json_path TEXT,
                linked_game_dir_mode TEXT,
                game_dir_override TEXT,
                created INTEGER NOT NULL,
                modified INTEGER NOT NULL,
                last_played INTEGER,
                pinned_at INTEGER,
                submitted_time_played INTEGER NOT NULL,
                recent_time_played INTEGER NOT NULL
            );
            CREATE TABLE instance_content_sets (
                id TEXT PRIMARY KEY,
                instance_id TEXT NOT NULL,
                name TEXT NOT NULL,
                source_kind TEXT NOT NULL,
                status TEXT NOT NULL,
                game_version TEXT NOT NULL,
                protocol_version INTEGER,
                loader TEXT NOT NULL,
                loader_version TEXT,
                revision INTEGER NOT NULL,
                created INTEGER NOT NULL,
                modified INTEGER NOT NULL
            );
            CREATE TABLE instance_links (
                instance_id TEXT PRIMARY KEY,
                link_kind TEXT,
                modrinth_project_id TEXT,
                modrinth_version_id TEXT,
                server_project_id TEXT,
                content_project_id TEXT,
                content_version_id TEXT,
                hosting_server_id TEXT,
                hosting_instance_ids TEXT,
                hosting_active_instance_id TEXT,
                shared_instance_id TEXT,
                imported_name TEXT,
                imported_version_number TEXT,
                imported_filename TEXT
            );
            CREATE TABLE instance_groups (
                instance_id TEXT NOT NULL,
                group_name TEXT NOT NULL
            );
            CREATE TABLE instance_launch_overrides (
                instance_id TEXT PRIMARY KEY,
                overrides TEXT
            );
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            r#"
            INSERT INTO instances (
                id, path, applied_content_set_id, install_stage,
                launcher_feature_version, update_channel, name, created,
                modified, submitted_time_played, recent_time_played
            ) VALUES (?, ?, ?, ?, ?, ?, ?, 1, 1, 0, 0)
            "#,
        )
        .bind("instance")
        .bind("instance-path")
        .bind("content-set")
        .bind(InstanceInstallStage::NotInstalled.as_str())
        .bind(LauncherFeatureVersion::MOST_RECENT.as_str())
        .bind(ReleaseChannel::Release.key())
        .bind("Instance")
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            r#"
            INSERT INTO instance_content_sets (
                id, instance_id, name, source_kind, status, game_version,
                loader, revision, created, modified
            ) VALUES (?, ?, ?, ?, ?, ?, ?, 5, 1, 1)
            "#,
        )
        .bind("content-set")
        .bind("instance")
        .bind("Default")
        .bind(ContentSourceKind::Local.as_str())
        .bind(ContentSetStatus::Available.as_str())
        .bind("1.21.5")
        .bind(ModLoader::Vanilla.as_str())
        .execute(&pool)
        .await
        .unwrap();

        let by_id = get_instance_metadata_by_id("instance", &pool)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(by_id.applied_content_set.revision, 5);

        let many = get_instance_metadata_many(&["instance"], &pool)
            .await
            .unwrap();
        assert_eq!(many[0].applied_content_set.revision, 5);

        let listed = list_instance_metadata(&pool).await.unwrap();
        assert_eq!(listed[0].applied_content_set.revision, 5);

        let launch = get_instance_launch_context("instance", &pool)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(launch.applied_content_set.revision, 5);
    }
}

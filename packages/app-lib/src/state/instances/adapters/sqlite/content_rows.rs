#![allow(dead_code)]

use crate::state::instances::{
    ContentDependencyEdge, ContentDependencyKind, ContentEntry,
    ContentOwnershipKind, ContentRequirement, ContentSet, ContentSetRemoteRef,
    ContentSetRemoteRefType, ContentSetStatus, ContentSetSyncProvider,
    ContentSetSyncState, ContentSetSyncStatus, ContentSourceKind,
    ContentUpdateCheck, InstanceFile, ManualDownloadOperationKind,
    ManualDownloadState, PackMember, PackMemberMaterializationState,
    PackMemberOverrideKind, PendingManualDownload,
};
use crate::state::{
    ContentProvider, ContentProviderRef, ModLoader, ProjectType, ReleaseChannel,
};
use chrono::{DateTime, TimeZone, Utc};
use sqlx::{Executor, Row, Sqlite, SqlitePool, Transaction};
use uuid::Uuid;

/// Ensures the instance a content write is about to reference still exists.
///
/// Runs inside the write transaction so a concurrent instance deletion that
/// already committed is reported as a clean error instead of a raw SQLite
/// foreign-key violation (`SQLITE_CONSTRAINT_FOREIGNKEY`, code 787).
pub(crate) async fn ensure_instance_exists(
    instance_id: &str,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    let instance_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM instances WHERE id = ?)",
    )
    .bind(instance_id)
    .fetch_one(&mut **tx)
    .await?;
    if !instance_exists {
        return Err(crate::ErrorKind::InputError(
            "This instance has been deleted".to_string(),
        )
        .into());
    }

    Ok(())
}

/// Ensures the instance and content set a content write is about to reference
/// still exist, returning a clean error instead of a foreign-key violation.
pub(crate) async fn ensure_content_write_parents(
    instance_id: &str,
    content_set_id: &str,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    ensure_instance_exists(instance_id, tx).await?;

    let content_set_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM instance_content_sets WHERE id = ?)",
    )
    .bind(content_set_id)
    .fetch_one(&mut **tx)
    .await?;
    if !content_set_exists {
        return Err(crate::ErrorKind::InputError(
            "The content set for this instance has been deleted".to_string(),
        )
        .into());
    }

    Ok(())
}

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct ContentSetRow {
    pub id: String,
    pub instance_id: String,
    pub name: String,
    pub source_kind: String,
    pub status: String,
    pub game_version: String,
    pub protocol_version: Option<i64>,
    pub loader: String,
    pub loader_version: Option<String>,
    pub revision: i64,
    pub created: i64,
    pub modified: i64,
}

impl TryFrom<ContentSetRow> for ContentSet {
    type Error = crate::Error;

    fn try_from(row: ContentSetRow) -> crate::Result<Self> {
        Ok(Self {
            id: row.id,
            instance_id: row.instance_id,
            name: row.name,
            source_kind: ContentSourceKind::from_str(&row.source_kind)?,
            status: ContentSetStatus::from_str(&row.status)?,
            game_version: row.game_version,
            protocol_version: row.protocol_version.map(|value| value as u32),
            loader: ModLoader::try_from_string(&row.loader)?,
            loader_version: row.loader_version,
            revision: unsigned(row.revision, "instance_content_sets.revision")?,
            created: timestamp(row.created),
            modified: timestamp(row.modified),
        })
    }
}

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct ContentSetRemoteRefRow {
    pub content_set_id: String,
    pub ref_type: String,
    pub ref_id: String,
}

impl TryFrom<ContentSetRemoteRefRow> for ContentSetRemoteRef {
    type Error = crate::Error;

    fn try_from(row: ContentSetRemoteRefRow) -> crate::Result<Self> {
        Ok(Self {
            content_set_id: row.content_set_id,
            ref_type: ContentSetRemoteRefType::from_str(&row.ref_type)?,
            ref_id: row.ref_id,
        })
    }
}

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct ContentSetSyncStateRow {
    pub content_set_id: String,
    pub provider: String,
    pub applied_update_id: Option<String>,
    pub latest_available_update_id: Option<String>,
    pub checked_at: Option<i64>,
    pub status: String,
}

impl TryFrom<ContentSetSyncStateRow> for ContentSetSyncState {
    type Error = crate::Error;

    fn try_from(row: ContentSetSyncStateRow) -> crate::Result<Self> {
        Ok(Self {
            content_set_id: row.content_set_id,
            provider: ContentSetSyncProvider::from_str(&row.provider)?,
            applied_update_id: row.applied_update_id,
            latest_available_update_id: row.latest_available_update_id,
            checked_at: row.checked_at.and_then(optional_timestamp),
            status: ContentSetSyncStatus::from_str(&row.status)?,
        })
    }
}

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct InstanceFileRow {
    pub id: String,
    pub instance_id: String,
    pub relative_path: String,
    pub file_name: String,
    pub enabled: i64,
    pub sha1: String,
    pub size: i64,
    pub missing: i64,
    pub added_at: i64,
    pub modified_at: i64,
    pub local_mod_data: Option<String>,
    pub icon_path: Option<String>,
}

impl TryFrom<InstanceFileRow> for InstanceFile {
    type Error = crate::Error;

    fn try_from(row: InstanceFileRow) -> crate::Result<Self> {
        Ok(Self {
            id: row.id,
            instance_id: row.instance_id,
            relative_path: row.relative_path,
            file_name: row.file_name,
            enabled: row.enabled == 1,
            sha1: row.sha1,
            size: unsigned(row.size, "size")?,
            missing: row.missing == 1,
            added_at: timestamp(row.added_at),
            modified_at: timestamp(row.modified_at),
            local_mod_data: row.local_mod_data,
            icon_path: row.icon_path,
        })
    }
}

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct ContentEntryRow {
    pub id: String,
    pub instance_id: String,
    pub content_set_id: String,
    pub file_id: Option<String>,
    pub project_type: String,
    pub source_kind: String,
    pub ownership_kind: String,
    pub auto_dependency: i64,
    pub server_requirement: String,
    pub client_requirement: String,
    pub enabled: i64,
    pub added_at: i64,
    pub modified_at: i64,
}

impl TryFrom<ContentEntryRow> for ContentEntry {
    type Error = crate::Error;

    fn try_from(row: ContentEntryRow) -> crate::Result<Self> {
        Ok(Self {
            id: row.id,
            instance_id: row.instance_id,
            content_set_id: row.content_set_id,
            file_id: row.file_id,
            project_type: project_type_from_str(&row.project_type)?,
            source_kind: ContentSourceKind::from_str(&row.source_kind)?,
            ownership_kind: ContentOwnershipKind::from_str(
                &row.ownership_kind,
            )?,
            auto_dependency: row.auto_dependency == 1,
            server_requirement: ContentRequirement::from_str(
                &row.server_requirement,
            )?,
            client_requirement: ContentRequirement::from_str(
                &row.client_requirement,
            )?,
            enabled: row.enabled == 1,
            added_at: timestamp(row.added_at),
            modified_at: timestamp(row.modified_at),
        })
    }
}

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct ContentUpdateCheckRow {
    pub content_entry_id: String,
    pub update_channel: String,
    pub provider: Option<String>,
    pub provider_project_id: Option<String>,
    pub provider_release_id: Option<String>,
    pub checked_at: i64,
}

impl From<ContentUpdateCheckRow> for ContentUpdateCheck {
    fn from(row: ContentUpdateCheckRow) -> Self {
        Self {
            content_entry_id: row.content_entry_id,
            update_channel: ReleaseChannel::from_key(&row.update_channel),
            provider: row
                .provider
                .as_deref()
                .and_then(|value| ContentProvider::from_str(value).ok()),
            provider_project_id: row.provider_project_id,
            provider_release_id: row.provider_release_id,
            checked_at: timestamp(row.checked_at),
        }
    }
}

pub(crate) async fn get_applied_content_set<'e, E>(
    instance_id: &str,
    exec: E,
) -> crate::Result<Option<ContentSet>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let row = sqlx::query_as::<_, ContentSetRow>(
        "
		SELECT cs.*
		FROM instances i
		INNER JOIN instance_content_sets cs
			ON cs.id = i.applied_content_set_id
		WHERE i.id = ?
		",
    )
    .bind(instance_id)
    .fetch_optional(exec)
    .await?;

    row.map(TryInto::try_into).transpose()
}

pub(crate) async fn get_content_set<'e, E>(
    content_set_id: &str,
    exec: E,
) -> crate::Result<Option<ContentSet>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let row = sqlx::query_as::<_, ContentSetRow>(
        "
		SELECT *
		FROM instance_content_sets
		WHERE id = ?
		",
    )
    .bind(content_set_id)
    .fetch_optional(exec)
    .await?;

    row.map(TryInto::try_into).transpose()
}

pub(crate) async fn get_content_sets_for_instance<'e, E>(
    instance_id: &str,
    exec: E,
) -> crate::Result<Vec<ContentSet>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let rows = sqlx::query_as::<_, ContentSetRow>(
        "
		SELECT *
		FROM instance_content_sets
		WHERE instance_id = ?
		ORDER BY created ASC, id ASC
		",
    )
    .bind(instance_id)
    .fetch_all(exec)
    .await?;

    rows.into_iter().map(TryInto::try_into).collect()
}

pub(crate) async fn insert_content_set(
    content_set: &ContentSet,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    let id = content_set.id.as_str();
    let instance_id = content_set.instance_id.as_str();
    let name = content_set.name.as_str();
    let source_kind = content_set.source_kind.as_str();
    let status = content_set.status.as_str();
    let game_version = content_set.game_version.as_str();
    let protocol_version =
        content_set.protocol_version.map(|value| value as i64);
    let loader = content_set.loader.as_str();
    let loader_version = content_set.loader_version.as_deref();
    let revision = content_set.revision as i64;
    let created = content_set.created.timestamp();
    let modified = content_set.modified.timestamp();

    sqlx::query(
        "
		INSERT INTO instance_content_sets (
			id,
			instance_id,
			name,
			source_kind,
			status,
			game_version,
			protocol_version,
			loader,
			loader_version,
			revision,
			created,
			modified
		)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
		",
    )
    .bind(id)
    .bind(instance_id)
    .bind(name)
    .bind(source_kind)
    .bind(status)
    .bind(game_version)
    .bind(protocol_version)
    .bind(loader)
    .bind(loader_version)
    .bind(revision)
    .bind(created)
    .bind(modified)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

pub(crate) async fn update_content_set(
    content_set: &ContentSet,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    let id = content_set.id.as_str();
    let name = content_set.name.as_str();
    let source_kind = content_set.source_kind.as_str();
    let status = content_set.status.as_str();
    let game_version = content_set.game_version.as_str();
    let protocol_version =
        content_set.protocol_version.map(|value| value as i64);
    let loader = content_set.loader.as_str();
    let loader_version = content_set.loader_version.as_deref();
    let revision = content_set.revision as i64;
    let modified = content_set.modified.timestamp();

    sqlx::query(
        "
		UPDATE instance_content_sets
		SET
			name = ?,
			source_kind = ?,
			status = ?,
			game_version = ?,
			protocol_version = ?,
			loader = ?,
			loader_version = ?,
			revision = ?,
			modified = ?
		WHERE id = ?
		",
    )
    .bind(name)
    .bind(source_kind)
    .bind(status)
    .bind(game_version)
    .bind(protocol_version)
    .bind(loader)
    .bind(loader_version)
    .bind(revision)
    .bind(modified)
    .bind(id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

pub(crate) async fn get_content_set_remote_refs<'e, E>(
    content_set_id: &str,
    exec: E,
) -> crate::Result<Vec<ContentSetRemoteRef>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let rows = sqlx::query_as!(
        ContentSetRemoteRefRow,
        "
		SELECT *
		FROM instance_content_set_remote_refs
		WHERE content_set_id = ?
		ORDER BY ref_type ASC
		",
        content_set_id,
    )
    .fetch_all(exec)
    .await?;

    rows.into_iter().map(TryInto::try_into).collect()
}

pub(crate) async fn get_content_set_sync_state<'e, E>(
    content_set_id: &str,
    exec: E,
) -> crate::Result<Option<ContentSetSyncState>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let row = sqlx::query_as!(
        ContentSetSyncStateRow,
        "
		SELECT *
		FROM instance_content_set_sync_state
		WHERE content_set_id = ?
		",
        content_set_id,
    )
    .fetch_optional(exec)
    .await?;

    row.map(TryInto::try_into).transpose()
}

pub(crate) async fn get_instance_files<'e, E>(
    instance_id: &str,
    exec: E,
) -> crate::Result<Vec<InstanceFile>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let rows = sqlx::query_as::<_, InstanceFileRow>(
        "
		SELECT *
		FROM instance_files
		WHERE instance_id = ?
		ORDER BY relative_path ASC
		",
    )
    .bind(instance_id)
    .fetch_all(exec)
    .await?;

    rows.into_iter().map(TryInto::try_into).collect()
}

pub(crate) async fn mark_instance_files_missing(
    instance_id: &str,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    let modified_at = Utc::now().timestamp();

    sqlx::query!(
        "
		UPDATE instance_files
		SET
			missing = 1,
			modified_at = ?
		WHERE instance_id = ?
		",
        modified_at,
        instance_id,
    )
    .execute(&mut **tx)
    .await?;

    Ok(())
}

pub(crate) async fn upsert_instance_file(
    file: &InstanceFile,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    let id = file.id.as_str();
    let instance_id = file.instance_id.as_str();
    let relative_path = file.relative_path.as_str();
    let file_name = file.file_name.as_str();
    let enabled = i64::from(file.enabled);
    let sha1 = file.sha1.as_str();
    let size = file.size as i64;
    let missing = i64::from(file.missing);
    let added_at = file.added_at.timestamp();
    let modified_at = file.modified_at.timestamp();
    let local_mod_data = file.local_mod_data.as_deref();
    let icon_path = file.icon_path.as_deref();

    sqlx::query!(
        "
		INSERT INTO instance_files (
			id,
			instance_id,
			relative_path,
			file_name,
			enabled,
			sha1,
			size,
			missing,
			added_at,
			modified_at,
			local_mod_data,
			icon_path
		)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
		ON CONFLICT (instance_id, relative_path) DO UPDATE SET
			file_name = excluded.file_name,
			enabled = excluded.enabled,
			sha1 = excluded.sha1,
			size = excluded.size,
			missing = excluded.missing,
			modified_at = excluded.modified_at,
			local_mod_data = COALESCE(excluded.local_mod_data, instance_files.local_mod_data),
			icon_path = COALESCE(excluded.icon_path, instance_files.icon_path)
		",
        id,
        instance_id,
        relative_path,
        file_name,
        enabled,
        sha1,
        size,
        missing,
        added_at,
        modified_at,
        local_mod_data,
        icon_path,
    )
    .execute(&mut **tx)
    .await?;

    Ok(())
}

pub(crate) async fn get_content_entries<'e, E>(
    content_set_id: &str,
    exec: E,
) -> crate::Result<Vec<ContentEntry>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let rows = sqlx::query_as::<_, ContentEntryRow>(
        "
		SELECT id, instance_id, content_set_id, file_id, project_type,
			source_kind, ownership_kind, auto_dependency, server_requirement,
			client_requirement, enabled,
			added_at, modified_at
		FROM instance_content_entries
		WHERE content_set_id = ?
		ORDER BY added_at ASC, id ASC
		",
    )
    .bind(content_set_id)
    .fetch_all(exec)
    .await?;

    rows.into_iter().map(TryInto::try_into).collect()
}

pub(crate) async fn get_content_update_check<'e, E>(
    content_entry_id: &str,
    exec: E,
) -> crate::Result<Option<ContentUpdateCheck>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let row = sqlx::query_as::<_, ContentUpdateCheckRow>(
        "
		SELECT content_entry_id, update_channel, provider,
			provider_project_id, provider_release_id, checked_at
		FROM instance_content_update_checks
		WHERE content_entry_id = ?
		",
    )
    .bind(content_entry_id)
    .fetch_optional(exec)
    .await?;

    Ok(row.map(Into::into))
}

pub(crate) struct UpsertInstanceFile<'a> {
    pub instance_id: &'a str,
    pub relative_path: &'a str,
    pub file_name: &'a str,
    pub enabled: bool,
    pub sha1: &'a str,
    pub size: u64,
    pub missing: bool,
    pub local_mod_data: Option<&'a str>,
    pub icon_path: Option<&'a str>,
}

pub(crate) async fn get_instance_file_by_relative_path(
    instance_id: &str,
    relative_path: &str,
    pool: &SqlitePool,
) -> crate::Result<Option<InstanceFile>> {
    let row = sqlx::query_as::<_, InstanceFileRow>(
        "
		SELECT *
		FROM instance_files
		WHERE instance_id = ? AND relative_path = ?
		",
    )
    .bind(instance_id)
    .bind(relative_path)
    .fetch_optional(pool)
    .await?;

    row.map(TryInto::try_into).transpose()
}

pub(crate) async fn upsert_instance_file_from_parts(
    input: UpsertInstanceFile<'_>,
    pool: &SqlitePool,
) -> crate::Result<InstanceFile> {
    let mut tx = pool.begin().await?;
    let file =
        upsert_instance_file_from_parts_in_transaction(input, &mut tx).await?;
    tx.commit().await?;
    Ok(file)
}

pub(crate) async fn upsert_instance_file_from_parts_in_transaction(
    input: UpsertInstanceFile<'_>,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<InstanceFile> {
    let existing: Option<InstanceFile> = sqlx::query_as::<_, InstanceFileRow>(
        "SELECT * FROM instance_files
         WHERE instance_id = ? AND relative_path = ?",
    )
    .bind(input.instance_id)
    .bind(input.relative_path)
    .fetch_optional(&mut **tx)
    .await?
    .map(|row| row.try_into())
    .transpose()?;
    let local_mod_data = input
        .local_mod_data
        .map(ToString::to_string)
        .or_else(|| existing.as_ref().and_then(|f| f.local_mod_data.clone()));
    let icon_path = input
        .icon_path
        .map(ToString::to_string)
        .or_else(|| existing.as_ref().and_then(|f| f.icon_path.clone()));

    let file = InstanceFile {
        id: existing
            .as_ref()
            .map(|file| file.id.clone())
            .unwrap_or_else(|| format!("instance-file:{}", Uuid::new_v4())),
        instance_id: input.instance_id.to_string(),
        relative_path: input.relative_path.to_string(),
        file_name: input.file_name.to_string(),
        enabled: input.enabled,
        sha1: input.sha1.to_string(),
        size: input.size,
        missing: input.missing,
        added_at: existing
            .as_ref()
            .map(|file| file.added_at)
            .unwrap_or_else(Utc::now),
        modified_at: Utc::now(),
        local_mod_data,
        icon_path,
    };

    upsert_instance_file(&file, &mut *tx).await?;

    Ok(file)
}

pub(crate) async fn rename_instance_file(
    instance_id: &str,
    old_relative_path: &str,
    new_relative_path: &str,
    new_file_name: &str,
    enabled: bool,
    pool: &SqlitePool,
) -> crate::Result<Option<InstanceFile>> {
    let mut tx = pool.begin().await?;

    let file = rename_instance_file_in_transaction(
        instance_id,
        old_relative_path,
        new_relative_path,
        new_file_name,
        enabled,
        &mut tx,
    )
    .await?;
    tx.commit().await?;

    Ok(file)
}

pub(crate) async fn rename_instance_file_in_transaction(
    instance_id: &str,
    old_relative_path: &str,
    new_relative_path: &str,
    new_file_name: &str,
    enabled: bool,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<Option<InstanceFile>> {
    let enabled = i64::from(enabled);
    let modified_at = Utc::now().timestamp();

    let source_id = sqlx::query_scalar!(
        "
		SELECT id
		FROM instance_files
		WHERE instance_id = ? AND relative_path = ?
		",
        instance_id,
        old_relative_path,
    )
    .fetch_optional(&mut **tx)
    .await?;
    let target_id = sqlx::query_scalar!(
        "
		SELECT id
		FROM instance_files
		WHERE instance_id = ? AND relative_path = ?
		",
        instance_id,
        new_relative_path,
    )
    .fetch_optional(&mut **tx)
    .await?;

    if let (Some(source_id), Some(target_id)) =
        (source_id.as_deref(), target_id.as_deref())
        && source_id != target_id
    {
        sqlx::query!(
            "
				DELETE FROM instance_content_entries
				WHERE id IN (
					SELECT target_entry.id
					FROM instance_content_entries target_entry
					WHERE target_entry.instance_id = ?
						AND target_entry.file_id = ?
						AND EXISTS (
							SELECT 1
							FROM instance_content_entries source_entry
							WHERE source_entry.instance_id = target_entry.instance_id
								AND source_entry.content_set_id = target_entry.content_set_id
								AND source_entry.file_id = ?
						)
				)
				",
            instance_id,
            target_id,
            source_id,
        )
        .execute(&mut **tx)
        .await?;

        sqlx::query!(
            "
				UPDATE instance_content_entries
				SET file_id = ?, modified_at = ?
				WHERE instance_id = ? AND file_id = ?
				",
            source_id,
            modified_at,
            instance_id,
            target_id,
        )
        .execute(&mut **tx)
        .await?;

        sqlx::query!(
            "
				DELETE FROM instance_files
				WHERE id = ?
				",
            target_id,
        )
        .execute(&mut **tx)
        .await?;
    }

    sqlx::query!(
        "
		UPDATE instance_files
		SET
			relative_path = ?,
			file_name = ?,
			enabled = ?,
			missing = 0,
			modified_at = ?
		WHERE instance_id = ? AND relative_path = ?
		",
        new_relative_path,
        new_file_name,
        enabled,
        modified_at,
        instance_id,
        old_relative_path,
    )
    .execute(&mut **tx)
    .await?;

    let row = sqlx::query_as::<_, InstanceFileRow>(
        "SELECT * FROM instance_files
         WHERE instance_id = ? AND relative_path = ?",
    )
    .bind(instance_id)
    .bind(new_relative_path)
    .fetch_optional(&mut **tx)
    .await?;

    row.map(TryInto::try_into).transpose()
}

pub(crate) async fn move_instance_file_in_transaction(
    instance_id: &str,
    old_relative_path: &str,
    new_relative_path: &str,
    new_file_name: &str,
    enabled: bool,
    sha1: &str,
    size: u64,
    local_mod_data: Option<&str>,
    icon_path: Option<&str>,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<Option<InstanceFile>> {
    let source_id = sqlx::query_scalar!(
        "
		SELECT id
		FROM instance_files
		WHERE instance_id = ? AND relative_path = ?
		",
        instance_id,
        old_relative_path,
    )
    .fetch_optional(&mut **tx)
    .await?;
    let target_id = sqlx::query_scalar!(
        "
		SELECT id
		FROM instance_files
		WHERE instance_id = ? AND relative_path = ?
		",
        instance_id,
        new_relative_path,
    )
    .fetch_optional(&mut **tx)
    .await?;
    let Some(source_id) = source_id else {
        return Ok(None);
    };
    if target_id
        .as_deref()
        .is_some_and(|target_id| target_id != source_id)
    {
        return Ok(None);
    }

    sqlx::query(
        "
		UPDATE instance_files
		SET
			relative_path = ?,
			file_name = ?,
			enabled = ?,
			sha1 = ?,
			size = ?,
			missing = 0,
			modified_at = ?,
			local_mod_data = COALESCE(?, local_mod_data),
			icon_path = COALESCE(?, icon_path)
		WHERE instance_id = ? AND relative_path = ?
		",
    )
    .bind(new_relative_path)
    .bind(new_file_name)
    .bind(i64::from(enabled))
    .bind(sha1)
    .bind(size as i64)
    .bind(Utc::now().timestamp())
    .bind(local_mod_data)
    .bind(icon_path)
    .bind(instance_id)
    .bind(old_relative_path)
    .execute(&mut **tx)
    .await?;

    let row = sqlx::query_as::<_, InstanceFileRow>(
        "SELECT * FROM instance_files
         WHERE instance_id = ? AND relative_path = ?",
    )
    .bind(instance_id)
    .bind(new_relative_path)
    .fetch_optional(&mut **tx)
    .await?;

    row.map(TryInto::try_into).transpose()
}

pub(crate) async fn adopt_untracked_file_in_transaction(
    instance_id: &str,
    new_relative_path: &str,
    tracked_file_id: &str,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<bool> {
    let untracked_file_id = sqlx::query_scalar!(
        "
		SELECT id
		FROM instance_files
		WHERE instance_id = ? AND relative_path = ?
		",
        instance_id,
        new_relative_path,
    )
    .fetch_optional(&mut **tx)
    .await?;
    let Some(untracked_file_id) = untracked_file_id else {
        return Ok(false);
    };
    if untracked_file_id == tracked_file_id {
        return Ok(false);
    }
    let updated = sqlx::query(
        "UPDATE instance_content_entries
         SET file_id = ?, modified_at = ?
         WHERE instance_id = ? AND file_id = ?",
    )
    .bind(untracked_file_id)
    .bind(Utc::now().timestamp())
    .bind(instance_id)
    .bind(tracked_file_id)
    .execute(&mut **tx)
    .await?;
    if updated.rows_affected() == 0 {
        return Ok(false);
    }

    sqlx::query(
        "DELETE FROM instance_files
         WHERE instance_id = ? AND id = ?",
    )
    .bind(instance_id)
    .bind(tracked_file_id)
    .execute(&mut **tx)
    .await?;

    Ok(true)
}

pub(crate) async fn remove_instance_file_by_relative_path(
    instance_id: &str,
    relative_path: &str,
    pool: &SqlitePool,
) -> crate::Result<()> {
    sqlx::query!(
        "
		DELETE FROM instance_files
		WHERE instance_id = ? AND relative_path = ?
		",
        instance_id,
        relative_path,
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub(crate) struct UpsertContentEntry<'a> {
    pub instance_id: &'a str,
    pub content_set_id: &'a str,
    pub file_id: Option<&'a str>,
    pub project_type: ProjectType,
    pub source_kind: ContentSourceKind,
    pub ownership_kind: ContentOwnershipKind,
    pub auto_dependency: bool,
    pub server_requirement: ContentRequirement,
    pub client_requirement: ContentRequirement,
    pub enabled: bool,
}

pub(crate) async fn get_content_entry_by_id(
    id: &str,
    pool: &SqlitePool,
) -> crate::Result<Option<ContentEntry>> {
    let row = sqlx::query_as::<_, ContentEntryRow>(
        "
		SELECT id, instance_id, content_set_id, file_id, project_type,
			source_kind, ownership_kind, auto_dependency, server_requirement,
			client_requirement, enabled,
			added_at, modified_at
		FROM instance_content_entries
		WHERE id = ?
		",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    row.map(TryInto::try_into).transpose()
}

pub(crate) async fn get_content_entry_by_file(
    content_set_id: &str,
    file_id: &str,
    pool: &SqlitePool,
) -> crate::Result<Option<ContentEntry>> {
    let row = sqlx::query_as::<_, ContentEntryRow>(
        "
		SELECT id, instance_id, content_set_id, file_id, project_type,
			source_kind, ownership_kind, auto_dependency, server_requirement,
			client_requirement, enabled,
			added_at, modified_at
		FROM instance_content_entries
		WHERE content_set_id = ? AND file_id = ?
		ORDER BY modified_at DESC
		LIMIT 1
		",
    )
    .bind(content_set_id)
    .bind(file_id)
    .fetch_optional(pool)
    .await?;

    row.map(TryInto::try_into).transpose()
}

pub(crate) async fn get_content_entry_by_relative_path(
    content_set_id: &str,
    relative_path: &str,
    pool: &SqlitePool,
) -> crate::Result<Option<ContentEntry>> {
    let row = sqlx::query_as::<_, ContentEntryRow>(
        "
		SELECT entry.id, entry.instance_id, entry.content_set_id, entry.file_id,
			entry.project_type, entry.source_kind, entry.ownership_kind,
			entry.auto_dependency, entry.server_requirement,
			entry.client_requirement, entry.enabled,
			entry.added_at, entry.modified_at
		FROM instance_content_entries entry
		INNER JOIN instance_files file ON file.id = entry.file_id
		WHERE entry.content_set_id = ? AND file.relative_path = ?
		ORDER BY entry.modified_at DESC
		LIMIT 1
		",
    )
    .bind(content_set_id)
    .bind(relative_path)
    .fetch_optional(pool)
    .await?;

    row.map(TryInto::try_into).transpose()
}

pub(crate) async fn upsert_content_entry_from_parts(
    input: UpsertContentEntry<'_>,
    pool: &SqlitePool,
) -> crate::Result<ContentEntry> {
    let mut tx = pool.begin().await?;
    let entry =
        upsert_content_entry_from_parts_in_transaction(input, &mut tx).await?;
    tx.commit().await?;
    Ok(entry)
}

pub(crate) async fn upsert_content_entry_from_parts_in_transaction(
    input: UpsertContentEntry<'_>,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<ContentEntry> {
    let existing_id = if let Some(file_id) = input.file_id {
        sqlx::query_scalar::<_, String>(
            "SELECT id
			 FROM instance_content_entries
			 WHERE content_set_id = ? AND file_id = ?
			 ORDER BY modified_at DESC
			 LIMIT 1",
        )
        .bind(input.content_set_id)
        .bind(file_id)
        .fetch_optional(&mut **tx)
        .await?
    } else {
        None
    };
    let now = Utc::now();
    let entry = ContentEntry {
        id: existing_id
            .unwrap_or_else(|| format!("content-entry:{}", Uuid::new_v4())),
        instance_id: input.instance_id.to_string(),
        content_set_id: input.content_set_id.to_string(),
        file_id: input.file_id.map(ToString::to_string),
        project_type: input.project_type,
        source_kind: input.source_kind,
        ownership_kind: input.ownership_kind,
        auto_dependency: input.auto_dependency,
        server_requirement: input.server_requirement,
        client_requirement: input.client_requirement,
        enabled: input.enabled,
        added_at: now,
        modified_at: now,
    };

    let original_added_at = sqlx::query_scalar::<_, i64>(
        "SELECT added_at FROM instance_content_entries WHERE id = ?",
    )
    .bind(&entry.id)
    .fetch_optional(&mut **tx)
    .await?
    .and_then(|timestamp| Utc.timestamp_opt(timestamp, 0).single())
    .unwrap_or(entry.added_at);
    let id = entry.id.as_str();
    let entry_instance_id = entry.instance_id.as_str();
    let content_set_id = entry.content_set_id.as_str();
    let file_id = entry.file_id.as_deref();
    let project_type = entry.project_type.get_name();
    let source_kind = entry.source_kind.as_str();
    let ownership_kind = entry.ownership_kind.as_str();
    let auto_dependency = i64::from(entry.auto_dependency);
    let server_requirement = entry.server_requirement.as_str();
    let client_requirement = entry.client_requirement.as_str();
    let enabled = i64::from(entry.enabled);
    let added_at = original_added_at.timestamp();
    let modified_at = entry.modified_at.timestamp();

    sqlx::query(
        "
		INSERT INTO instance_content_entries (
			id,
			instance_id,
			content_set_id,
			file_id,
			project_type,
			source_kind,
			ownership_kind,
			auto_dependency,
			server_requirement,
			client_requirement,
			enabled,
			added_at,
			modified_at
		)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
		ON CONFLICT(id) DO UPDATE SET
			file_id = excluded.file_id,
			project_type = excluded.project_type,
			source_kind = excluded.source_kind,
			ownership_kind = excluded.ownership_kind,
			auto_dependency = excluded.auto_dependency,
			server_requirement = excluded.server_requirement,
			client_requirement = excluded.client_requirement,
			enabled = excluded.enabled,
			modified_at = excluded.modified_at
		",
    )
    .bind(id)
    .bind(entry_instance_id)
    .bind(content_set_id)
    .bind(file_id)
    .bind(project_type)
    .bind(source_kind)
    .bind(ownership_kind)
    .bind(auto_dependency)
    .bind(server_requirement)
    .bind(client_requirement)
    .bind(enabled)
    .bind(added_at)
    .bind(modified_at)
    .execute(&mut **tx)
    .await?;

    Ok(ContentEntry {
        added_at: original_added_at,
        ..entry
    })
}

pub(crate) async fn upsert_content_provider_ref(
    content_entry_id: &str,
    provider_ref: &ContentProviderRef,
    origin: bool,
    pool: &SqlitePool,
) -> crate::Result<()> {
    let mut tx = pool.begin().await?;
    upsert_content_provider_ref_in_transaction(
        content_entry_id,
        provider_ref,
        origin,
        &mut tx,
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

pub(crate) async fn upsert_content_provider_ref_in_transaction(
    content_entry_id: &str,
    provider_ref: &ContentProviderRef,
    origin: bool,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    let provider = provider_ref.provider().as_str();
    let project_id = provider_ref.database_project_id();
    let release_id = provider_ref.database_release_id();
    let file_id = provider_ref.database_file_id();

    if origin {
        sqlx::query(
            "UPDATE instance_content_provider_refs
             SET is_origin = 0
             WHERE content_entry_id = ?",
        )
        .bind(content_entry_id)
        .execute(&mut **tx)
        .await?;
    }

    let updated = sqlx::query(
        "UPDATE instance_content_provider_refs
         SET is_origin = CASE WHEN ? = 1 THEN 1 ELSE is_origin END
         WHERE content_entry_id = ?
           AND provider = ?
           AND provider_project_id = ?
           AND provider_release_id IS ?
           AND provider_file_id IS ?",
    )
    .bind(i64::from(origin))
    .bind(content_entry_id)
    .bind(provider)
    .bind(&project_id)
    .bind(&release_id)
    .bind(&file_id)
    .execute(&mut **tx)
    .await?;

    if updated.rows_affected() == 0 {
        sqlx::query(
            "INSERT INTO instance_content_provider_refs (
                content_entry_id,
                provider,
                provider_project_id,
                provider_release_id,
				provider_file_id,
                is_origin
             ) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(content_entry_id)
        .bind(provider)
        .bind(project_id)
        .bind(release_id)
        .bind(file_id)
        .bind(i64::from(origin))
        .execute(&mut **tx)
        .await?;
    }

    Ok(())
}

pub(crate) async fn get_content_provider_refs(
    content_entry_id: &str,
    pool: &SqlitePool,
) -> crate::Result<Vec<ContentProviderRef>> {
    let rows = sqlx::query(
        "SELECT provider, provider_project_id, provider_release_id, provider_file_id
         FROM instance_content_provider_refs
         WHERE content_entry_id = ?
         ORDER BY is_origin DESC, provider ASC, provider_project_id ASC,
            provider_release_id IS NULL ASC, provider_release_id ASC",
    )
    .bind(content_entry_id)
    .fetch_all(pool)
    .await?;

    let mut refs = Vec::with_capacity(rows.len());
    for row in rows {
        let provider = row.try_get::<String, _>("provider")?;
        let project_id = row.try_get::<String, _>("provider_project_id")?;
        let release_id =
            row.try_get::<Option<String>, _>("provider_release_id")?;
        let file_id = row.try_get::<Option<String>, _>("provider_file_id")?;
        let provider_ref = ContentProviderRef::from_database(
            &provider,
            &project_id,
            release_id.as_deref(),
			file_id.as_deref(),
        )
        .map_err(|error| {
            crate::ErrorKind::InputError(format!(
                "Invalid provider reference for content entry {content_entry_id}: {error}"
            ))
        })?;
        refs.push(provider_ref);
    }

    Ok(refs)
}

pub(crate) async fn get_content_provider_refs_with_origin(
    content_entry_id: &str,
    pool: &SqlitePool,
) -> crate::Result<Vec<(ContentProviderRef, bool)>> {
    let rows = sqlx::query(
        "SELECT provider, provider_project_id, provider_release_id, provider_file_id, is_origin
         FROM instance_content_provider_refs
         WHERE content_entry_id = ?
         ORDER BY is_origin DESC, provider ASC, provider_project_id ASC,
            provider_release_id IS NULL ASC, provider_release_id ASC",
    )
    .bind(content_entry_id)
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|row| {
            let provider = row.try_get::<String, _>("provider")?;
            let project_id = row.try_get::<String, _>("provider_project_id")?;
            let release_id =
                row.try_get::<Option<String>, _>("provider_release_id")?;
            let file_id =
                row.try_get::<Option<String>, _>("provider_file_id")?;
            let origin = row.try_get::<i64, _>("is_origin")? != 0;
            let provider_ref = ContentProviderRef::from_database(
                &provider,
                &project_id,
                release_id.as_deref(),
                file_id.as_deref(),
            )?;
            Ok((provider_ref, origin))
        })
        .collect()
}

pub(crate) async fn invalidate_exact_provider_refs_for_file_in_transaction(
    content_set_id: &str,
    file_id: &str,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<bool> {
    let rows = sqlx::query(
        "SELECT ref.content_entry_id, ref.provider,
            ref.provider_project_id, ref.is_origin
         FROM instance_content_provider_refs ref
         INNER JOIN instance_content_entries entry
            ON entry.id = ref.content_entry_id
         WHERE entry.content_set_id = ? AND entry.file_id = ?
            AND ref.provider_release_id IS NOT NULL
         ORDER BY ref.is_origin DESC, ref.provider,
            ref.provider_project_id",
    )
    .bind(content_set_id)
    .bind(file_id)
    .fetch_all(&mut **tx)
    .await?;
    if rows.is_empty() {
        return Ok(false);
    }

    for row in &rows {
        let entry_id = row.try_get::<String, _>("content_entry_id")?;
        let provider = row.try_get::<String, _>("provider")?;
        let project_id = row.try_get::<String, _>("provider_project_id")?;
        let origin = row.try_get::<i64, _>("is_origin")? != 0;
        let project_ref = ContentProviderRef::from_database(
            &provider,
            &project_id,
            None,
            None,
        )?;
        upsert_content_provider_ref_in_transaction(
            &entry_id,
            &project_ref,
            origin,
            tx,
        )
        .await?;
    }
    sqlx::query(
        "DELETE FROM instance_content_provider_refs
         WHERE content_entry_id IN (
            SELECT id FROM instance_content_entries
            WHERE content_set_id = ? AND file_id = ?
         ) AND provider_release_id IS NOT NULL",
    )
    .bind(content_set_id)
    .bind(file_id)
    .execute(&mut **tx)
    .await?;

    Ok(true)
}

pub(crate) async fn restore_content_entry_in_transaction(
    entry: &ContentEntry,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    sqlx::query(
        "INSERT INTO instance_content_entries (
            id, instance_id, content_set_id, file_id, project_type,
            source_kind, ownership_kind, auto_dependency, server_requirement,
            client_requirement, enabled, added_at, modified_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
            instance_id = excluded.instance_id,
            content_set_id = excluded.content_set_id,
            file_id = excluded.file_id,
            project_type = excluded.project_type,
            source_kind = excluded.source_kind,
            ownership_kind = excluded.ownership_kind,
            auto_dependency = excluded.auto_dependency,
            server_requirement = excluded.server_requirement,
            client_requirement = excluded.client_requirement,
            enabled = excluded.enabled,
            added_at = excluded.added_at,
            modified_at = excluded.modified_at",
    )
    .bind(&entry.id)
    .bind(&entry.instance_id)
    .bind(&entry.content_set_id)
    .bind(&entry.file_id)
    .bind(entry.project_type.get_name())
    .bind(entry.source_kind.as_str())
    .bind(entry.ownership_kind.as_str())
    .bind(i64::from(entry.auto_dependency))
    .bind(entry.server_requirement.as_str())
    .bind(entry.client_requirement.as_str())
    .bind(i64::from(entry.enabled))
    .bind(entry.added_at.timestamp())
    .bind(entry.modified_at.timestamp())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn restore_content_update_check_in_transaction(
    check: &ContentUpdateCheck,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    sqlx::query(
        "INSERT INTO instance_content_update_checks (
            content_entry_id, update_channel, provider,
            provider_project_id, provider_release_id, checked_at
         ) VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(content_entry_id) DO UPDATE SET
            update_channel = excluded.update_channel,
            provider = excluded.provider,
            provider_project_id = excluded.provider_project_id,
            provider_release_id = excluded.provider_release_id,
            checked_at = excluded.checked_at",
    )
    .bind(&check.content_entry_id)
    .bind(check.update_channel.key())
    .bind(check.provider.map(ContentProvider::as_str))
    .bind(&check.provider_project_id)
    .bind(&check.provider_release_id)
    .bind(check.checked_at.timestamp())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn get_content_origin_provider(
    content_entry_id: &str,
    pool: &SqlitePool,
) -> crate::Result<Option<ContentProvider>> {
    let provider = sqlx::query_scalar::<_, String>(
        "SELECT provider
         FROM instance_content_provider_refs
         WHERE content_entry_id = ? AND is_origin = 1",
    )
    .bind(content_entry_id)
    .fetch_optional(pool)
    .await?;

    provider
        .as_deref()
        .map(ContentProvider::from_str)
        .transpose()
}

pub(crate) async fn get_content_entry_by_provider_ref(
    content_set_id: &str,
    provider: ContentProvider,
    project_id: &str,
    release_id: &str,
    pool: &SqlitePool,
) -> crate::Result<Option<ContentEntry>> {
    let row = sqlx::query_as::<_, ContentEntryRow>(
        "
		SELECT entry.id, entry.instance_id, entry.content_set_id, entry.file_id,
			entry.project_type, entry.source_kind, entry.ownership_kind,
			entry.auto_dependency, entry.server_requirement,
			entry.client_requirement, entry.enabled,
			entry.added_at, entry.modified_at
		FROM instance_content_entries entry
		INNER JOIN instance_content_provider_refs ref
			ON ref.content_entry_id = entry.id
		WHERE entry.content_set_id = ?
			AND ref.provider = ?
			AND ref.provider_project_id = ?
			AND ref.provider_release_id = ?
		ORDER BY entry.modified_at DESC
		LIMIT 1
		",
    )
    .bind(content_set_id)
    .bind(provider.as_str())
    .bind(project_id)
    .bind(release_id)
    .fetch_optional(pool)
    .await?;

    row.map(TryInto::try_into).transpose()
}

pub(crate) async fn get_content_entry_by_provider_project(
    content_set_id: &str,
    provider: ContentProvider,
    project_id: &str,
    pool: &SqlitePool,
) -> crate::Result<Option<ContentEntry>> {
    let row = sqlx::query_as::<_, ContentEntryRow>(
        "
		SELECT entry.id, entry.instance_id, entry.content_set_id, entry.file_id,
			entry.project_type, entry.source_kind, entry.ownership_kind,
			entry.auto_dependency, entry.server_requirement,
			entry.client_requirement, entry.enabled,
			entry.added_at, entry.modified_at
		FROM instance_content_entries entry
		INNER JOIN instance_content_provider_refs ref
			ON ref.content_entry_id = entry.id
		WHERE entry.content_set_id = ?
			AND ref.provider = ?
			AND ref.provider_project_id = ?
		ORDER BY entry.modified_at DESC
		LIMIT 1
		",
    )
    .bind(content_set_id)
    .bind(provider.as_str())
    .bind(project_id)
    .fetch_optional(pool)
    .await?;

    row.map(TryInto::try_into).transpose()
}

pub(crate) async fn set_content_entry_auto_dependency(
    content_entry_id: &str,
    auto_dependency: bool,
    pool: &SqlitePool,
) -> crate::Result<()> {
    sqlx::query(
        "UPDATE instance_content_entries
         SET auto_dependency = ?, modified_at = ?
         WHERE id = ?",
    )
    .bind(i64::from(auto_dependency))
    .bind(Utc::now().timestamp())
    .bind(content_entry_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub(crate) async fn set_content_entry_auto_dependency_in_transaction(
    content_entry_id: &str,
    auto_dependency: bool,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    sqlx::query(
        "UPDATE instance_content_entries
         SET auto_dependency = ?, modified_at = ?
         WHERE id = ?",
    )
    .bind(i64::from(auto_dependency))
    .bind(Utc::now().timestamp())
    .bind(content_entry_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn get_dependency_backfilled_entry_ids(
    content_set_id: &str,
    pool: &SqlitePool,
) -> crate::Result<std::collections::HashSet<String>> {
    let rows = sqlx::query_scalar::<_, String>(
        "SELECT id FROM instance_content_entries
         WHERE content_set_id = ? AND dependency_backfilled = 1",
    )
    .bind(content_set_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().collect())
}

pub(crate) async fn set_content_entry_dependency_backfilled_in_transaction(
    content_entry_id: &str,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    sqlx::query(
        "UPDATE instance_content_entries
         SET dependency_backfilled = 1, modified_at = ?
         WHERE id = ?",
    )
    .bind(Utc::now().timestamp())
    .bind(content_entry_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn upsert_content_dependency_edge_in_transaction(
    edge: &ContentDependencyEdge,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    sqlx::query(
        "
		INSERT INTO instance_content_dependencies (
			id,
			content_set_id,
			parent_entry_id,
			child_entry_id,
			evidence_provider,
			parent_provider,
			child_provider,
			dependency_kind,
			parent_project_id,
			parent_release_id,
			child_project_id,
			child_release_id,
			created_at,
			modified_at
		)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
		ON CONFLICT (
			content_set_id,
			parent_entry_id,
			child_entry_id,
			dependency_kind
		) DO UPDATE SET
			evidence_provider = excluded.evidence_provider,
			parent_provider = excluded.parent_provider,
			child_provider = excluded.child_provider,
			parent_project_id = excluded.parent_project_id,
			parent_release_id = excluded.parent_release_id,
			child_project_id = excluded.child_project_id,
			child_release_id = excluded.child_release_id,
			modified_at = excluded.modified_at
		",
    )
    .bind(&edge.id)
    .bind(&edge.content_set_id)
    .bind(&edge.parent_entry_id)
    .bind(&edge.child_entry_id)
    .bind(edge.evidence_provider.as_str())
    .bind(edge.parent_provider.as_str())
    .bind(edge.child_provider.as_str())
    .bind(edge.dependency_kind.as_str())
    .bind(&edge.parent_project_id)
    .bind(&edge.parent_release_id)
    .bind(&edge.child_project_id)
    .bind(&edge.child_release_id)
    .bind(edge.created_at.timestamp())
    .bind(edge.modified_at.timestamp())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn replace_content_dependency_edges_for_parent(
    content_set_id: &str,
    parent_entry_id: &str,
    edges: &[ContentDependencyEdge],
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    sqlx::query(
        "DELETE FROM instance_content_dependencies
         WHERE content_set_id = ? AND parent_entry_id = ?",
    )
    .bind(content_set_id)
    .bind(parent_entry_id)
    .execute(&mut **tx)
    .await?;

    for edge in edges {
        upsert_content_dependency_edge_in_transaction(edge, tx).await?;
    }
    Ok(())
}

pub(crate) async fn get_content_dependency_edges(
    content_set_id: &str,
    pool: &SqlitePool,
) -> crate::Result<Vec<ContentDependencyEdge>> {
    let rows = sqlx::query(
        "SELECT id, content_set_id, parent_entry_id, child_entry_id,
                evidence_provider, parent_provider, child_provider,
                dependency_kind, parent_project_id,
                parent_release_id, child_project_id, child_release_id,
                created_at, modified_at
         FROM instance_content_dependencies
         WHERE content_set_id = ?
         ORDER BY created_at ASC",
    )
    .bind(content_set_id)
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|row| {
            let evidence_provider = ContentProvider::from_str(
                &row.try_get::<String, _>("evidence_provider")?,
            )?;
            let parent_provider = ContentProvider::from_str(
                &row.try_get::<String, _>("parent_provider")?,
            )?;
            let child_provider = ContentProvider::from_str(
                &row.try_get::<String, _>("child_provider")?,
            )?;
            let dependency_kind = ContentDependencyKind::from_str(
                &row.try_get::<String, _>("dependency_kind")?,
            )?;
            let created_at = timestamp(row.try_get::<i64, _>("created_at")?);
            let modified_at = timestamp(row.try_get::<i64, _>("modified_at")?);

            Ok(ContentDependencyEdge {
                id: row.try_get("id")?,
                content_set_id: row.try_get("content_set_id")?,
                parent_entry_id: row.try_get("parent_entry_id")?,
                child_entry_id: row.try_get("child_entry_id")?,
                evidence_provider,
                parent_provider,
                child_provider,
                dependency_kind,
                parent_project_id: row.try_get("parent_project_id")?,
                parent_release_id: row.try_get("parent_release_id")?,
                child_project_id: row.try_get("child_project_id")?,
                child_release_id: row.try_get("child_release_id")?,
                created_at,
                modified_at,
            })
        })
        .collect()
}

pub(crate) async fn get_orphaned_auto_dependencies(
    content_set_id: &str,
    pool: &SqlitePool,
) -> crate::Result<Vec<ContentEntry>> {
    let rows = sqlx::query_as::<_, ContentEntryRow>(
        "
		SELECT entry.id, entry.instance_id, entry.content_set_id, entry.file_id,
			entry.project_type, entry.source_kind, entry.ownership_kind,
			entry.auto_dependency, entry.server_requirement,
			entry.client_requirement, entry.enabled,
			entry.added_at, entry.modified_at
		FROM instance_content_entries entry
		WHERE entry.content_set_id = ?
			AND entry.auto_dependency = 1
			AND NOT EXISTS (
				SELECT 1
				FROM instance_content_dependencies edge
				WHERE edge.child_entry_id = entry.id
					AND edge.content_set_id = entry.content_set_id
			)
		ORDER BY entry.added_at ASC
		",
    )
    .bind(content_set_id)
    .fetch_all(pool)
    .await?;

    rows.into_iter().map(TryInto::try_into).collect()
}

pub(crate) async fn set_content_entry_enabled_for_file(
    content_set_id: &str,
    file_id: &str,
    enabled: bool,
    pool: &SqlitePool,
) -> crate::Result<bool> {
    let enabled = i64::from(enabled);
    let modified_at = Utc::now().timestamp();

    let result = sqlx::query!(
        "
		UPDATE instance_content_entries
		SET enabled = ?, modified_at = ?
		WHERE content_set_id = ? AND file_id = ?
		",
        enabled,
        modified_at,
        content_set_id,
        file_id,
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

pub(crate) async fn remove_content_entries_for_file(
    content_set_id: &str,
    file_id: &str,
    pool: &SqlitePool,
) -> crate::Result<()> {
    sqlx::query!(
        "
		DELETE FROM instance_content_entries
		WHERE content_set_id = ? AND file_id = ?
		",
        content_set_id,
        file_id,
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub(crate) async fn upsert_content_update_check(
    content_entry_id: &str,
    update_channel: ReleaseChannel,
    provider: Option<ContentProvider>,
    provider_project_id: Option<&str>,
    provider_release_id: Option<&str>,
    pool: &SqlitePool,
) -> crate::Result<()> {
    let update_channel = update_channel.key();
    let provider = provider.map(ContentProvider::as_str);
    let checked_at = Utc::now().timestamp();

    sqlx::query(
        "
		INSERT INTO instance_content_update_checks (
			content_entry_id,
			update_channel,
			provider,
			provider_project_id,
			provider_release_id,
			checked_at
		)
		SELECT ?, ?, ?, ?, ?, ?
		WHERE EXISTS (
			SELECT 1 FROM instance_content_entries WHERE id = ?
		)
		ON CONFLICT(content_entry_id) DO UPDATE SET
			update_channel = excluded.update_channel,
			provider = excluded.provider,
			provider_project_id = excluded.provider_project_id,
			provider_release_id = excluded.provider_release_id,
			checked_at = excluded.checked_at
		",
    )
    .bind(content_entry_id)
    .bind(update_channel)
    .bind(provider)
    .bind(provider_project_id)
    .bind(provider_release_id)
    .bind(checked_at)
    .bind(content_entry_id)
    .execute(pool)
    .await?;

    Ok(())
}

#[derive(Debug, sqlx::FromRow)]
struct PackMemberRow {
    id: String,
    content_set_id: String,
    content_entry_id: Option<String>,
    member_key: String,
    project_type: String,
    expected_relative_path: String,
    provider: Option<String>,
    provider_project_id: Option<String>,
    provider_release_id: Option<String>,
    required: i64,
    expected_sha1: Option<String>,
    expected_size: Option<i64>,
    expected_fingerprint: Option<i64>,
    materialization_state: String,
    override_kind: String,
    reconciled: i64,
    created_at: i64,
    modified_at: i64,
}

impl TryFrom<PackMemberRow> for PackMember {
    type Error = crate::Error;

    fn try_from(row: PackMemberRow) -> crate::Result<Self> {
        Ok(Self {
            id: row.id,
            content_set_id: row.content_set_id,
            content_entry_id: row.content_entry_id,
            member_key: row.member_key,
            project_type: project_type_from_str(&row.project_type)?,
            expected_relative_path: row.expected_relative_path,
            provider: row
                .provider
                .as_deref()
                .map(ContentProvider::from_str)
                .transpose()?,
            provider_project_id: row.provider_project_id,
            provider_release_id: row.provider_release_id,
            required: row.required != 0,
            expected_sha1: row.expected_sha1,
            expected_size: row
                .expected_size
                .map(|value| unsigned(value, "expected_size"))
                .transpose()?,
            expected_fingerprint: row
                .expected_fingerprint
                .map(|value| unsigned(value, "expected_fingerprint"))
                .transpose()?,
            materialization_state: PackMemberMaterializationState::from_str(
                &row.materialization_state,
            )?,
            override_kind: PackMemberOverrideKind::from_str(
                &row.override_kind,
            )?,
            reconciled: row.reconciled != 0,
            created_at: timestamp(row.created_at),
            modified_at: timestamp(row.modified_at),
        })
    }
}

#[derive(Debug, sqlx::FromRow)]
struct PendingManualDownloadRow {
    id: String,
    instance_id: String,
    pack_member_id: Option<String>,
    content_entry_id: Option<String>,
    operation_kind: String,
    operation_target_id: Option<String>,
    project_type: String,
    provider: String,
    provider_project_id: String,
    provider_release_id: String,
    file_name: String,
    website_url: Option<String>,
    target_relative_path: String,
    expected_sha1: Option<String>,
    expected_size: Option<i64>,
    expected_fingerprint: Option<i64>,
    state: String,
    context: String,
    created_at: i64,
    modified_at: i64,
}

impl TryFrom<PendingManualDownloadRow> for PendingManualDownload {
    type Error = crate::Error;

    fn try_from(row: PendingManualDownloadRow) -> crate::Result<Self> {
        Ok(Self {
            id: row.id,
            instance_id: row.instance_id,
            pack_member_id: row.pack_member_id,
            content_entry_id: row.content_entry_id,
            operation_kind: ManualDownloadOperationKind::from_str(
                &row.operation_kind,
            )?,
            operation_target_id: row.operation_target_id,
            project_type: project_type_from_str(&row.project_type)?,
            provider: ContentProvider::from_str(&row.provider)?,
            provider_project_id: row.provider_project_id,
            provider_release_id: row.provider_release_id,
            file_name: row.file_name,
            website_url: row.website_url,
            target_relative_path: row.target_relative_path,
            expected_sha1: row.expected_sha1,
            expected_size: row
                .expected_size
                .map(|value| unsigned(value, "expected_size"))
                .transpose()?,
            expected_fingerprint: row
                .expected_fingerprint
                .map(|value| unsigned(value, "expected_fingerprint"))
                .transpose()?,
            state: ManualDownloadState::from_str(&row.state)?,
            context: serde_json::from_str(&row.context).map_err(|error| {
                crate::ErrorKind::InputError(format!(
                    "Invalid pending manual download context: {error}"
                ))
            })?,
            created_at: timestamp(row.created_at),
            modified_at: timestamp(row.modified_at),
        })
    }
}

pub(crate) async fn get_pack_members(
    content_set_id: &str,
    pool: &SqlitePool,
) -> crate::Result<Vec<PackMember>> {
    let rows = sqlx::query_as::<_, PackMemberRow>(
        "SELECT * FROM instance_pack_members
         WHERE content_set_id = ?
         ORDER BY created_at ASC, id ASC",
    )
    .bind(content_set_id)
    .fetch_all(pool)
    .await?;

    rows.into_iter().map(TryInto::try_into).collect()
}

pub(crate) async fn get_materialized_curseforge_downloads(
    instance_id: &str,
    pool: &SqlitePool,
) -> crate::Result<Vec<(String, String)>> {
    let rows = sqlx::query(
        "SELECT DISTINCT ref.provider_project_id, ref.provider_release_id
         FROM instances instance
         INNER JOIN instance_content_entries entry
            ON entry.content_set_id = instance.applied_content_set_id
         INNER JOIN instance_files file
            ON file.id = entry.file_id AND file.instance_id = instance.id
         INNER JOIN instance_content_provider_refs ref
            ON ref.content_entry_id = entry.id
            AND ref.provider = 'curseforge'
         LEFT JOIN instance_pack_members member
            ON member.content_set_id = entry.content_set_id
            AND member.content_entry_id = entry.id
         WHERE instance.id = ?
            AND file.missing = 0
            AND ref.provider_release_id IS NOT NULL
            AND (
                member.id IS NULL
                OR (
                    member.provider = 'curseforge'
                    AND member.provider_project_id = ref.provider_project_id
                    AND member.provider_release_id = ref.provider_release_id
                    AND member.materialization_state = 'present'
                )
            )",
    )
    .bind(instance_id)
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|row| {
            Ok((
                row.try_get::<String, _>("provider_project_id")?,
                row.try_get::<String, _>("provider_release_id")?,
            ))
        })
        .collect()
}

#[derive(Clone, Debug)]
pub(crate) struct ContentMutationTarget {
    pub entry_id: Option<String>,
    pub member_id: Option<String>,
    pub relative_path: Option<String>,
    pub ownership_kind: ContentOwnershipKind,
    pub project_type: ProjectType,
    pub provider: Option<ContentProvider>,
    pub provider_project_id: Option<String>,
    pub provider_release_id: Option<String>,
}

pub(crate) async fn get_content_mutation_target(
    instance_id: &str,
    target_id: &str,
    pool: &SqlitePool,
) -> crate::Result<Option<ContentMutationTarget>> {
    // Hash identification creates non-origin refs, including for unpacked mods
    // after disconnecting a pack. Match the update check's Modrinth fallback:
    // use those refs only when no origin/member or other provider takes priority.
    let row = sqlx::query(
        "SELECT entry.id AS entry_id,
			member.id AS member_id,
			file.relative_path,
			COALESCE(
				NULLIF(entry.ownership_kind, ''),
				CASE
					WHEN member.id IS NULL THEN 'user_added'
					ELSE 'pack_managed'
				END
			) AS ownership_kind,
			COALESCE(
				NULLIF(entry.project_type, ''),
				NULLIF(member.project_type, '')
			) AS project_type,
			COALESCE(origin.provider, member.provider, identified.provider) AS provider,
			COALESCE(origin.provider_project_id, member.provider_project_id, identified.provider_project_id)
				AS provider_project_id,
			COALESCE(origin.provider_release_id, member.provider_release_id, identified.provider_release_id)
				AS provider_release_id
		 FROM instance_content_sets content_set
		 INNER JOIN instances instance
			ON instance.applied_content_set_id = content_set.id
		 LEFT JOIN instance_pack_members member
			ON member.content_set_id = content_set.id AND member.id = ?
		 LEFT JOIN instance_content_entries entry
			ON entry.content_set_id = content_set.id
			AND (entry.id = ? OR entry.id = member.content_entry_id)
		 LEFT JOIN instance_files file
			ON file.instance_id = instance.id
			AND (file.id = ? OR file.id = entry.file_id)
		 LEFT JOIN instance_content_provider_refs origin
			ON origin.content_entry_id = entry.id AND origin.is_origin = 1
		 LEFT JOIN instance_content_provider_refs identified
			ON origin.provider IS NULL AND member.provider IS NULL
			AND identified.rowid = (
				SELECT ref.rowid FROM instance_content_provider_refs ref
				WHERE ref.content_entry_id = entry.id AND ref.provider = 'modrinth'
					AND NOT EXISTS (
						SELECT 1 FROM instance_content_provider_refs other
						WHERE other.content_entry_id = entry.id AND other.provider != 'modrinth'
					)
				ORDER BY ref.provider_project_id, ref.provider_release_id IS NULL,
					ref.provider_release_id
				LIMIT 1
			)
		 WHERE instance.id = ?
			AND (entry.id = ? OR member.id = ? OR file.id = ?)
		 LIMIT 1",
    )
    .bind(target_id)
    .bind(target_id)
    .bind(target_id)
    .bind(instance_id)
    .bind(target_id)
    .bind(target_id)
    .bind(target_id)
    .fetch_optional(pool)
    .await?;

    row.map(|row| {
		let relative_path = row.try_get::<Option<String>, _>("relative_path")?;
		let project_type = row
			.try_get::<Option<String>, _>("project_type")?
			.as_deref()
			.map(project_type_from_str)
			.transpose()?
			.or_else(|| {
				relative_path.as_deref().and_then(
					crate::state::instances::adapters::filesystem::project_type_from_relative_path,
				)
			})
			.ok_or_else(|| {
				crate::ErrorKind::InputError(
					"Unable to determine the selected content project type"
						.to_string(),
				)
			})?;

		Ok(ContentMutationTarget {
			entry_id: row.try_get("entry_id")?,
			member_id: row.try_get("member_id")?,
			relative_path,
			ownership_kind: ContentOwnershipKind::from_str(
				&row.try_get::<String, _>("ownership_kind")?,
			)?,
			project_type,
            provider: row
                .try_get::<Option<String>, _>("provider")?
                .as_deref()
                .map(ContentProvider::from_str)
                .transpose()?,
            provider_project_id: row.try_get("provider_project_id")?,
            provider_release_id: row.try_get("provider_release_id")?,
        })
    })
    .transpose()
}

pub(crate) async fn set_pack_member_override_in_transaction(
    member_id: &str,
    materialization_state: PackMemberMaterializationState,
    override_kind: PackMemberOverrideKind,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<bool> {
    let result = sqlx::query(
        "UPDATE instance_pack_members
		 SET materialization_state = ?, override_kind = ?, modified_at = ?
		 WHERE id = ?",
    )
    .bind(materialization_state.as_str())
    .bind(override_kind.as_str())
    .bind(Utc::now().timestamp())
    .bind(member_id)
    .execute(&mut **tx)
    .await?;
    Ok(result.rows_affected() > 0)
}

pub(crate) async fn get_pending_manual_downloads(
    instance_id: &str,
    pool: &SqlitePool,
) -> crate::Result<Vec<PendingManualDownload>> {
    let rows = sqlx::query_as::<_, PendingManualDownloadRow>(
        "SELECT * FROM instance_pending_manual_downloads
         WHERE instance_id = ? AND state IN ('waiting', 'matched', 'error')
         ORDER BY created_at ASC, id ASC",
    )
    .bind(instance_id)
    .fetch_all(pool)
    .await?;

    rows.into_iter().map(TryInto::try_into).collect()
}

/// Restores membership for legacy pack-owned files without inventing remote
/// identity or treating their current bytes as the pack's expected artifact.
/// Entry identity keeps missing and disabled files at the same path distinct.
pub(crate) async fn repair_pack_members_in_transaction(
    content_set_id: &str,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<bool> {
    let result = sqlx::query(
        "INSERT INTO instance_pack_members (
            id, content_set_id, content_entry_id, member_key, project_type,
            expected_relative_path, required, materialization_state,
            override_kind, reconciled, created_at, modified_at
         ) SELECT 'pack-member:entry:' || entry.id, entry.content_set_id, entry.id,
            'entry:' || entry.id, entry.project_type,
            file.pack_path, 1,
            CASE WHEN file.missing = 1 THEN 'missing' ELSE 'present' END,
            CASE WHEN file.enabled = 0 THEN 'disabled' ELSE 'none' END,
            0, entry.added_at, entry.modified_at
         FROM instance_content_entries entry
         JOIN (
            SELECT *, CASE WHEN substr(relative_path, -9) = '.disabled'
                THEN substr(relative_path, 1, length(relative_path) - 9)
                ELSE relative_path END AS pack_path
            FROM instance_files
         ) file ON file.id = entry.file_id
         WHERE entry.content_set_id = ? AND entry.ownership_kind = 'pack_managed'
            AND NOT EXISTS (
                SELECT 1 FROM instance_pack_members member
                WHERE member.content_set_id = entry.content_set_id
                    AND member.content_entry_id = entry.id
            )",
    )
    .bind(content_set_id)
    .execute(&mut **tx)
    .await?;
    Ok(result.rows_affected() != 0)
}

pub(crate) async fn upsert_pack_member_in_transaction(
    member: &PackMember,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    sqlx::query(
        "INSERT INTO instance_pack_members (
            id, content_set_id, content_entry_id, member_key, project_type,
            expected_relative_path, provider, provider_project_id,
            provider_release_id, required, expected_sha1, expected_size,
            expected_fingerprint, materialization_state, override_kind,
            reconciled, created_at, modified_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
		 ON CONFLICT(content_set_id, member_key) DO UPDATE SET
			content_entry_id = excluded.content_entry_id,
			project_type = excluded.project_type,
			expected_relative_path = excluded.expected_relative_path,
            provider = excluded.provider,
            provider_project_id = excluded.provider_project_id,
            provider_release_id = excluded.provider_release_id,
			required = CASE
				WHEN instance_pack_members.reconciled = 0 THEN excluded.required
				ELSE instance_pack_members.required
			END,
            expected_sha1 = excluded.expected_sha1,
            expected_size = excluded.expected_size,
            expected_fingerprint = excluded.expected_fingerprint,
            materialization_state = excluded.materialization_state,
			override_kind = CASE
                WHEN excluded.override_kind = 'none' THEN override_kind
                ELSE excluded.override_kind
			END,
			reconciled = excluded.reconciled,
            modified_at = excluded.modified_at",
    )
    .bind(&member.id)
    .bind(&member.content_set_id)
    .bind(&member.content_entry_id)
    .bind(&member.member_key)
    .bind(member.project_type.get_name())
    .bind(&member.expected_relative_path)
    .bind(member.provider.map(ContentProvider::as_str))
    .bind(&member.provider_project_id)
    .bind(&member.provider_release_id)
    .bind(i64::from(member.required))
    .bind(&member.expected_sha1)
    .bind(member.expected_size.map(|value| value as i64))
    .bind(member.expected_fingerprint.map(|value| value as i64))
    .bind(member.materialization_state.as_str())
    .bind(member.override_kind.as_str())
    .bind(i64::from(member.reconciled))
    .bind(member.created_at.timestamp())
    .bind(member.modified_at.timestamp())
    .execute(&mut **tx)
    .await?;

    Ok(())
}

pub(crate) async fn upsert_pending_manual_download_in_transaction(
    download: &PendingManualDownload,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    sqlx::query(
        "INSERT INTO instance_pending_manual_downloads (
            id, instance_id, pack_member_id, content_entry_id,
            operation_kind, operation_target_id, project_type, provider,
            provider_project_id, provider_release_id, file_name, website_url,
            target_relative_path, expected_sha1, expected_size,
            expected_fingerprint, state, context, created_at, modified_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(instance_id, operation_kind, provider,
            provider_project_id, provider_release_id) WHERE state IN ('waiting', 'matched')
         DO UPDATE SET
            pack_member_id = excluded.pack_member_id,
            content_entry_id = excluded.content_entry_id,
            file_name = excluded.file_name,
            website_url = excluded.website_url,
            target_relative_path = excluded.target_relative_path,
            expected_sha1 = excluded.expected_sha1,
            expected_size = excluded.expected_size,
            expected_fingerprint = excluded.expected_fingerprint,
            context = excluded.context,
            modified_at = excluded.modified_at",
    )
    .bind(&download.id)
    .bind(&download.instance_id)
    .bind(&download.pack_member_id)
    .bind(&download.content_entry_id)
    .bind(download.operation_kind.as_str())
    .bind(&download.operation_target_id)
    .bind(download.project_type.get_name())
    .bind(download.provider.as_str())
    .bind(&download.provider_project_id)
    .bind(&download.provider_release_id)
    .bind(&download.file_name)
    .bind(&download.website_url)
    .bind(&download.target_relative_path)
    .bind(&download.expected_sha1)
    .bind(download.expected_size.map(|value| value as i64))
    .bind(download.expected_fingerprint.map(|value| value as i64))
    .bind(download.state.as_str())
    .bind(download.context.to_string())
    .bind(download.created_at.timestamp())
    .bind(download.modified_at.timestamp())
    .execute(&mut **tx)
    .await?;

    Ok(())
}

pub(crate) async fn complete_pending_manual_download(
    instance_id: &str,
    provider_project_id: &str,
    provider_release_id: &str,
    content_entry_id: Option<&str>,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    sqlx::query(
        "UPDATE instance_pending_manual_downloads
         SET state = 'imported', content_entry_id = ?, modified_at = ?
         WHERE instance_id = ? AND provider = 'curseforge'
            AND provider_project_id = ? AND provider_release_id = ?
            AND state IN ('waiting', 'matched', 'error')",
    )
    .bind(content_entry_id)
    .bind(Utc::now().timestamp())
    .bind(instance_id)
    .bind(provider_project_id)
    .bind(provider_release_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

pub(crate) async fn bump_content_set_revision_in_transaction(
    content_set_id: &str,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<u64> {
    let revision = sqlx::query_scalar::<_, i64>(
        "UPDATE instance_content_sets
         SET revision = revision + 1, modified = ?
         WHERE id = ?
         RETURNING revision",
    )
    .bind(Utc::now().timestamp())
    .bind(content_set_id)
    .fetch_one(&mut **tx)
    .await?;
    unsigned(revision, "instance_content_sets.revision")
}

fn project_type_from_str(value: &str) -> crate::Result<ProjectType> {
    match value {
        "mod" => Ok(ProjectType::Mod),
        "datapack" => Ok(ProjectType::DataPack),
        "resourcepack" => Ok(ProjectType::ResourcePack),
        "shader" | "shaderpack" => Ok(ProjectType::ShaderPack),
        "schematic" => Ok(ProjectType::Schematic),
        "world_save" => Ok(ProjectType::WorldSave),
        other => Err(crate::ErrorKind::InputError(format!(
            "Unknown content project type {other}"
        ))
        .into()),
    }
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
    use crate::state::{ModrinthProjectId, ModrinthVersionId};
    use sqlx::sqlite::SqlitePoolOptions;

    async fn test_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory SQLite pool");
        sqlx::query("CREATE TABLE instances (id TEXT PRIMARY KEY)")
            .execute(&pool)
            .await
            .expect("instances table");
        sqlx::query("CREATE TABLE instance_content_sets (id TEXT PRIMARY KEY)")
            .execute(&pool)
            .await
            .expect("instance_content_sets table");
        pool
    }

    #[tokio::test]
    async fn materialized_curseforge_downloads_require_current_exact_content() {
        let pool = test_pool().await;
        sqlx::query(
            "ALTER TABLE instances ADD COLUMN applied_content_set_id TEXT NULL",
        )
        .execute(&pool)
        .await
        .expect("applied content set column");
        sqlx::raw_sql(
            "
            CREATE TABLE instance_files (
                id TEXT PRIMARY KEY,
                instance_id TEXT NOT NULL,
                missing INTEGER NOT NULL
            );
            CREATE TABLE instance_content_entries (
                id TEXT PRIMARY KEY,
                content_set_id TEXT NOT NULL,
                file_id TEXT NULL
            );
            CREATE TABLE instance_content_provider_refs (
                content_entry_id TEXT NOT NULL,
                provider TEXT NOT NULL,
                provider_project_id TEXT NOT NULL,
                provider_release_id TEXT NULL
            );
            CREATE TABLE instance_pack_members (
                id TEXT PRIMARY KEY,
                content_set_id TEXT NOT NULL,
                content_entry_id TEXT NULL,
                provider TEXT NULL,
                provider_project_id TEXT NULL,
                provider_release_id TEXT NULL,
                materialization_state TEXT NOT NULL
            );
            ",
        )
        .execute(&pool)
        .await
        .expect("content tables");
        sqlx::query(
            "INSERT INTO instance_content_sets (id) VALUES ('target-set'), ('other-set')",
        )
        .execute(&pool)
        .await
        .expect("content sets");
        sqlx::query(
            "INSERT INTO instances (id, applied_content_set_id)
             VALUES ('target', 'target-set'), ('other', 'other-set')",
        )
        .execute(&pool)
        .await
        .expect("instances");
        sqlx::query(
            "INSERT INTO instance_files (id, instance_id, missing) VALUES
                ('file-present', 'target', 0),
                ('file-member-missing', 'target', 0),
                ('file-untracked', 'target', 0),
                ('file-missing', 'target', 1),
                ('file-other', 'other', 0)",
        )
        .execute(&pool)
        .await
        .expect("files");
        sqlx::query(
            "INSERT INTO instance_content_entries (id, content_set_id, file_id) VALUES
                ('entry-present', 'target-set', 'file-present'),
                ('entry-member-missing', 'target-set', 'file-member-missing'),
                ('entry-untracked', 'target-set', 'file-untracked'),
                ('entry-missing', 'target-set', 'file-missing'),
                ('entry-other', 'other-set', 'file-other')",
        )
        .execute(&pool)
        .await
        .expect("entries");
        sqlx::query(
            "INSERT INTO instance_content_provider_refs (
                content_entry_id, provider, provider_project_id,
                provider_release_id
             ) VALUES
                ('entry-present', 'curseforge', '1', '10'),
                ('entry-member-missing', 'curseforge', '1', '11'),
                ('entry-untracked', 'curseforge', '2', '20'),
                ('entry-missing', 'curseforge', '3', '30'),
                ('entry-other', 'curseforge', '9', '90')",
        )
        .execute(&pool)
        .await
        .expect("provider refs");
        sqlx::query(
            "INSERT INTO instance_pack_members (
                id, content_set_id, content_entry_id, provider,
                provider_project_id, provider_release_id,
                materialization_state
             ) VALUES
                ('member-present', 'target-set', 'entry-present',
                    'curseforge', '1', '10', 'present'),
                ('member-missing', 'target-set', 'entry-member-missing',
                    'curseforge', '1', '11', 'missing'),
                ('member-other', 'other-set', 'entry-other',
                    'curseforge', '9', '90', 'present')",
        )
        .execute(&pool)
        .await
        .expect("pack members");

        let materialized =
            get_materialized_curseforge_downloads("target", &pool)
                .await
                .expect("materialized CurseForge downloads")
                .into_iter()
                .collect::<std::collections::HashSet<_>>();

        assert_eq!(
            materialized,
            std::collections::HashSet::from([
                ("1".to_string(), "10".to_string()),
                ("2".to_string(), "20".to_string()),
            ])
        );
    }

    #[tokio::test]
    async fn instance_existence_check_passes_for_existing_rows() {
        let pool = test_pool().await;
        sqlx::query("INSERT INTO instances (id) VALUES ('instance')")
            .execute(&pool)
            .await
            .expect("insert instance");
        let mut tx = pool.begin().await.expect("begin transaction");

        ensure_instance_exists("instance", &mut tx)
            .await
            .expect("existing instance passes");
    }

    #[tokio::test]
    async fn in_place_upgrade_lists_new_origin_release_before_old_ref() {
        let pool = test_pool().await;
        sqlx::raw_sql(
            "
			CREATE TABLE instance_content_provider_refs (
				id INTEGER PRIMARY KEY AUTOINCREMENT,
				content_entry_id TEXT NOT NULL,
				provider TEXT NOT NULL,
				provider_project_id TEXT NOT NULL,
				provider_release_id TEXT NULL,
                provider_file_id TEXT NULL,
				is_origin INTEGER NOT NULL DEFAULT 0
			);
			CREATE UNIQUE INDEX instance_content_provider_refs_identity
				ON instance_content_provider_refs (
					content_entry_id,
					provider,
					provider_project_id,
					COALESCE(provider_release_id, '')
				);
			CREATE UNIQUE INDEX instance_content_provider_refs_origin
				ON instance_content_provider_refs(content_entry_id)
				WHERE is_origin = 1;
			",
        )
        .execute(&pool)
        .await
        .expect("provider ref table");
        let old = ContentProviderRef::Modrinth {
            project_id: ModrinthProjectId::new("AANobbMI").unwrap(),
            version_id: Some(ModrinthVersionId::new("7pwil2dy").unwrap()),
        };
        let target = ContentProviderRef::Modrinth {
            project_id: ModrinthProjectId::new("AANobbMI").unwrap(),
            version_id: Some(ModrinthVersionId::new("vf7UgZpC").unwrap()),
        };

        upsert_content_provider_ref("entry", &old, true, &pool)
            .await
            .unwrap();
        upsert_content_provider_ref("entry", &target, true, &pool)
            .await
            .unwrap();

        assert_eq!(
            get_content_provider_refs("entry", &pool).await.unwrap(),
            vec![target.clone(), old.clone()]
        );
        assert_eq!(
            get_content_provider_refs_with_origin("entry", &pool)
                .await
                .unwrap(),
            vec![(target, true), (old, false)]
        );
    }

    #[tokio::test]
    async fn instance_existence_check_reports_deleted_instances() {
        let pool = test_pool().await;
        let mut tx = pool.begin().await.expect("begin transaction");

        let error = ensure_instance_exists("missing", &mut tx)
            .await
            .expect_err("missing instance must fail");
        assert!(
            error.to_string().contains("This instance has been deleted"),
            "unexpected error: {error}"
        );
    }

    #[tokio::test]
    async fn content_write_parents_check_requires_instance_and_set() {
        let pool = test_pool().await;
        sqlx::query("INSERT INTO instances (id) VALUES ('instance')")
            .execute(&pool)
            .await
            .expect("insert instance");
        sqlx::query("INSERT INTO instance_content_sets (id) VALUES ('set')")
            .execute(&pool)
            .await
            .expect("insert set");
        let mut tx = pool.begin().await.expect("begin transaction");

        ensure_content_write_parents("instance", "set", &mut tx)
            .await
            .expect("existing parents pass");

        let missing_set =
            ensure_content_write_parents("instance", "gone", &mut tx)
                .await
                .expect_err("missing content set must fail");
        assert!(
            missing_set
                .to_string()
                .contains("content set for this instance has been deleted"),
            "unexpected error: {missing_set}"
        );

        let missing_instance =
            ensure_content_write_parents("gone", "set", &mut tx)
                .await
                .expect_err("missing instance must fail");
        assert!(
            missing_instance
                .to_string()
                .contains("This instance has been deleted"),
            "unexpected error: {missing_instance}"
        );
    }

    #[tokio::test]
    async fn checks_run_inside_transactions() {
        let pool = test_pool().await;
        let mut tx = pool.begin().await.expect("begin transaction");

        ensure_instance_exists("instance", &mut tx)
            .await
            .expect_err("missing instance must fail inside a transaction");
        tx.rollback().await.expect("rollback transaction");

        sqlx::query("INSERT INTO instances (id) VALUES ('instance')")
            .execute(&pool)
            .await
            .expect("insert instance in transaction");
        let mut tx = pool.begin().await.expect("begin transaction");
        ensure_instance_exists("instance", &mut tx)
            .await
            .expect("existing instance passes inside a transaction");
    }

    async fn mutation_target_pool() -> SqlitePool {
        let pool = test_pool().await;
        sqlx::query(
            "ALTER TABLE instances ADD COLUMN applied_content_set_id TEXT NULL",
        )
        .execute(&pool)
        .await
        .expect("applied content set column");
        sqlx::raw_sql(
            "
			CREATE TABLE instance_files (
				id TEXT PRIMARY KEY,
				instance_id TEXT NOT NULL,
				relative_path TEXT NOT NULL
			);
			CREATE TABLE instance_content_entries (
				id TEXT PRIMARY KEY,
				content_set_id TEXT NOT NULL,
				file_id TEXT NULL,
				ownership_kind TEXT NOT NULL,
				project_type TEXT NOT NULL
			);
			CREATE TABLE instance_pack_members (
				id TEXT PRIMARY KEY,
				content_set_id TEXT NOT NULL,
				content_entry_id TEXT NULL,
				project_type TEXT NOT NULL,
				provider TEXT NULL,
				provider_project_id TEXT NULL,
				provider_release_id TEXT NULL
			);
			CREATE TABLE instance_content_provider_refs (
				content_entry_id TEXT NOT NULL,
				provider TEXT NOT NULL,
				provider_project_id TEXT NOT NULL,
				provider_release_id TEXT NULL,
				is_origin INTEGER NOT NULL
			);
			",
        )
        .execute(&pool)
        .await
        .expect("content tables");
        sqlx::query("INSERT INTO instance_content_sets (id) VALUES ('set')")
            .execute(&pool)
            .await
            .expect("insert content set");
        sqlx::query(
			"INSERT INTO instances (id, applied_content_set_id) VALUES ('instance', 'set')",
		)
		.execute(&pool)
		.await
		.expect("insert instance");
        sqlx::query(
            "INSERT INTO instance_files (id, instance_id, relative_path)
			 VALUES ('file', 'instance', 'mods/example.jar')",
        )
        .execute(&pool)
        .await
        .expect("insert untracked file");

        pool
    }

    #[tokio::test]
    async fn mutation_target_supports_untracked_instance_files() {
        let pool = mutation_target_pool().await;

        let target = get_content_mutation_target("instance", "file", &pool)
            .await
            .expect("resolve mutation target")
            .expect("mutation target exists");

        assert_eq!(target.entry_id, None);
        assert_eq!(target.member_id, None);
        assert_eq!(target.relative_path.as_deref(), Some("mods/example.jar"));
        assert_eq!(target.ownership_kind, ContentOwnershipKind::UserAdded);
        assert_eq!(target.project_type, ProjectType::Mod);
    }

    #[tokio::test]
    async fn mutation_target_resolves_identified_mod_after_pack_disconnect() {
        let pool = mutation_target_pool().await;
        sqlx::raw_sql(
            "INSERT INTO instance_content_entries VALUES
                ('entry', 'set', 'file', 'user_added', 'mod');
             INSERT INTO instance_content_provider_refs VALUES
                ('entry', 'modrinth', 'AANobbMI', '7pwil2dy', 0);",
        )
        .execute(&pool)
        .await
        .unwrap();

        let target = get_content_mutation_target("instance", "entry", &pool)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(target.provider, Some(ContentProvider::Modrinth));
        assert_eq!(target.provider_project_id.as_deref(), Some("AANobbMI"));
        assert_eq!(target.provider_release_id.as_deref(), Some("7pwil2dy"));
        assert_eq!(target.relative_path.as_deref(), Some("mods/example.jar"));
        assert_eq!(target.ownership_kind, ContentOwnershipKind::UserAdded);

        // A different provider ref must prevent an implicit provider switch.
        sqlx::query(
            "INSERT INTO instance_content_provider_refs VALUES
            ('entry', 'curseforge', '123', '456', 0)",
        )
        .execute(&pool)
        .await
        .unwrap();
        let target = get_content_mutation_target("instance", "entry", &pool)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(target.provider, None);

        sqlx::query(
            "UPDATE instance_content_provider_refs SET is_origin = 1
            WHERE provider = 'curseforge'",
        )
        .execute(&pool)
        .await
        .unwrap();
        let target = get_content_mutation_target("instance", "entry", &pool)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(target.provider, Some(ContentProvider::CurseForge));
        assert_eq!(target.provider_project_id.as_deref(), Some("123"));
        assert_eq!(target.provider_release_id.as_deref(), Some("456"));
    }

    #[tokio::test]
    async fn update_check_upsert_ignores_deleted_content_entries() {
        let pool = test_pool().await;
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .expect("enable foreign keys");
        sqlx::query(
            "CREATE TABLE instance_content_entries (id TEXT PRIMARY KEY)",
        )
        .execute(&pool)
        .await
        .expect("instance_content_entries table");
        sqlx::query(
            "
            CREATE TABLE instance_content_update_checks (
                content_entry_id TEXT PRIMARY KEY NOT NULL,
                update_channel TEXT NOT NULL,
                provider TEXT NULL,
                provider_project_id TEXT NULL,
                provider_release_id TEXT NULL,
                checked_at INTEGER NOT NULL,
                FOREIGN KEY (content_entry_id)
                    REFERENCES instance_content_entries(id)
                    ON DELETE CASCADE
            )
            ",
        )
        .execute(&pool)
        .await
        .expect("instance_content_update_checks table");

        upsert_content_update_check(
            "deleted-entry",
            ReleaseChannel::Release,
            Some(ContentProvider::CurseForge),
            Some("285109"),
            Some("4612979"),
            &pool,
        )
        .await
        .expect("deleted entry is ignored");

        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM instance_content_update_checks",
        )
        .fetch_one(&pool)
        .await
        .expect("count update checks");
        assert_eq!(count, 0);
    }
}

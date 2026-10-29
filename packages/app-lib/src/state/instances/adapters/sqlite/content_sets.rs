use super::values::{optional_timestamp, timestamp, unsigned};
use crate::state::ModLoader;
use crate::state::instances::{
    ContentSet, ContentSetRemoteRef, ContentSetRemoteRefType, ContentSetStatus,
    ContentSetSyncProvider, ContentSetSyncState, ContentSetSyncStatus,
    ContentSourceKind,
};
use chrono::Utc;
use sqlx::{Executor, Sqlite, Transaction};

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

/// Records the loader the instance's components project onto its applied
/// content set.
pub(crate) async fn set_applied_loader_in_transaction(
    instance_id: &str,
    loader: ModLoader,
    loader_version: Option<&str>,
    modified: i64,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    sqlx::query(
        "UPDATE instance_content_sets
         SET loader = ?, loader_version = ?, modified = ?
         WHERE id = (
            SELECT applied_content_set_id FROM instances WHERE id = ?
         )",
    )
    .bind(loader.as_str())
    .bind(loader_version)
    .bind(modified)
    .bind(instance_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

/// Records the loader version an instance launches with.
pub(crate) async fn set_applied_loader_version_in_transaction(
    instance_id: &str,
    loader_version: Option<&str>,
    modified: i64,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    sqlx::query!(
        "
        UPDATE instance_content_sets
        SET loader_version = ?, modified = ?
        WHERE id = (
            SELECT applied_content_set_id
            FROM instances
            WHERE id = ?
        )
        ",
        loader_version,
        modified,
        instance_id,
    )
    .execute(&mut **tx)
    .await?;

    Ok(())
}

/// Records the protocol version an instance's content set speaks.
pub(crate) async fn set_applied_protocol_version_in_transaction(
    instance_id: &str,
    protocol_version: Option<u32>,
    modified: i64,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    let protocol_version = protocol_version.map(i64::from);

    sqlx::query!(
        "
        UPDATE instance_content_sets
        SET protocol_version = ?, modified = ?
        WHERE id = (
            SELECT applied_content_set_id
            FROM instances
            WHERE id = ?
        )
        ",
        protocol_version,
        modified,
        instance_id,
    )
    .execute(&mut **tx)
    .await?;

    Ok(())
}

/// Reads the applied content set's identity and revision for a revision check.
pub(crate) async fn get_applied_content_set_revision<'e, E>(
    instance_id: &str,
    exec: E,
) -> crate::Result<Option<(String, i64)>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let current = sqlx::query_as(
        "SELECT content.id, content.revision FROM instances instance JOIN instance_content_sets content ON content.id = instance.applied_content_set_id WHERE instance.id = ?",
    )
    .bind(instance_id)
    .fetch_optional(exec)
    .await?;

    Ok(current)
}

/// Reads the game version and loader the content set an instance applies runs.
pub(crate) async fn get_applied_content_set_target<'e, E>(
    instance_id: &str,
    exec: E,
) -> crate::Result<Option<(String, String)>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let target = sqlx::query_as(
        "SELECT content_set.game_version, content_set.loader
         FROM instances instance
         INNER JOIN instance_content_sets content_set
            ON content_set.id = instance.applied_content_set_id
         WHERE instance.id = ?",
    )
    .bind(instance_id)
    .fetch_optional(exec)
    .await?;

    Ok(target)
}

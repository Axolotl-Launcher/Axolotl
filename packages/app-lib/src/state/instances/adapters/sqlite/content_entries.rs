use super::values::{project_type_from_str, timestamp};
use crate::state::instances::{
    ContentDependencyEdge, ContentDependencyKind, ContentEntry,
    ContentOwnershipKind, ContentRequirement, ContentSourceKind,
    ContentUpdateCheck,
};
use crate::state::{
    ContentProvider, ContentProviderRef, ProjectType, ReleaseChannel,
};
use chrono::{TimeZone, Utc};
use sqlx::{Executor, Row, Sqlite, SqlitePool, Transaction};
use uuid::Uuid;

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

#[cfg(test)]
#[path = "content_entries_tests.rs"]
mod tests;

/// Reads every provider reference of a content set, with the provider it marks
/// as the entry's origin.
pub(crate) async fn get_provider_refs_with_origin_for_content_set(
    content_set_id: &str,
    pool: &SqlitePool,
) -> crate::Result<Vec<(String, ContentProviderRef, Option<ContentProvider>)>> {
    let rows = sqlx::query(
        "SELECT ref.content_entry_id, ref.provider,
                ref.provider_project_id, ref.provider_release_id,
                ref.provider_file_id,
                ref.is_origin
         FROM instance_content_provider_refs ref
         INNER JOIN instance_content_entries entry
            ON entry.id = ref.content_entry_id
         WHERE entry.content_set_id = ?
         ORDER BY ref.content_entry_id, ref.is_origin DESC, ref.provider,
            ref.provider_project_id, ref.provider_release_id IS NULL,
            ref.provider_release_id",
    )
    .bind(content_set_id)
    .fetch_all(pool)
    .await?;

    let mut references = Vec::with_capacity(rows.len());
    for row in rows {
        let entry_id = row.try_get::<String, _>("content_entry_id")?;
        let provider = row.try_get::<String, _>("provider")?;
        let provider_project_id =
            row.try_get::<String, _>("provider_project_id")?;
        let provider_release_id =
            row.try_get::<Option<String>, _>("provider_release_id")?;
        let provider_file_id =
            row.try_get::<Option<String>, _>("provider_file_id")?;
        let reference = ContentProviderRef::from_database(
            &provider,
            &provider_project_id,
            provider_release_id.as_deref(),
            provider_file_id.as_deref(),
        )?;
        let origin = (row.try_get::<i64, _>("is_origin")? != 0)
            .then(|| ContentProvider::from_str(&provider))
            .transpose()?;
        references.push((entry_id, reference, origin));
    }

    Ok(references)
}

/// Releases every pack-managed entry of the set as user-added content.
pub(crate) async fn release_pack_managed_entries_in_transaction(
    content_set_id: &str,
    modified_at: i64,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    sqlx::query(
        "UPDATE instance_content_entries
         SET ownership_kind = 'user_added', modified_at = ?
         WHERE content_set_id = ? AND ownership_kind = 'pack_managed'",
    )
    .bind(modified_at)
    .bind(content_set_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

/// Hands an entry back to the pack that just claimed it.
pub(crate) async fn claim_entry_for_pack_in_transaction(
    entry_id: &str,
    modified_at: i64,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    sqlx::query(
        "UPDATE instance_content_entries
         SET ownership_kind = 'pack_managed', modified_at = ?
         WHERE id = ?",
    )
    .bind(modified_at)
    .bind(entry_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

/// Reads the ownership of the entry that claims a file path in the instance's
/// applied content set.
pub(crate) async fn get_ownership_kind_for_path(
    instance_id: &str,
    relative_path: &str,
    pool: &SqlitePool,
) -> crate::Result<Option<ContentOwnershipKind>> {
    let ownership = sqlx::query_scalar::<_, String>(
        "SELECT entry.ownership_kind
         FROM instance_files file
         INNER JOIN instance_content_entries entry ON entry.file_id = file.id
         INNER JOIN instances instance ON instance.id = entry.instance_id
         WHERE file.instance_id = ? AND file.relative_path = ?
            AND entry.content_set_id = instance.applied_content_set_id
         ORDER BY entry.modified_at DESC
         LIMIT 1",
    )
    .bind(instance_id)
    .bind(relative_path)
    .fetch_optional(pool)
    .await?;

    ownership
        .as_deref()
        .map(ContentOwnershipKind::from_str)
        .transpose()
}

/// Reads the newest Modrinth entry of the content set that names a release.
pub(crate) async fn get_modrinth_entry_id_by_release<'e, E>(
    content_set_id: &str,
    project_id: &str,
    release_id: &str,
    exec: E,
) -> crate::Result<Option<String>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let id = sqlx::query_scalar::<_, String>(
        "SELECT entry.id FROM instance_content_entries entry JOIN instance_content_provider_refs ref ON ref.content_entry_id = entry.id WHERE entry.content_set_id = ? AND ref.provider = 'modrinth' AND ref.provider_project_id = ? AND ref.provider_release_id = ? ORDER BY entry.modified_at DESC LIMIT 1",
    )
    .bind(content_set_id)
    .bind(project_id)
    .bind(release_id)
    .fetch_optional(exec)
    .await?;

    Ok(id)
}

/// Reads the newest entry that claims one file of the content set.
pub(crate) async fn get_entry_id_by_file_path<'e, E>(
    content_set_id: &str,
    instance_id: &str,
    relative_path: &str,
    exec: E,
) -> crate::Result<Option<String>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let id = sqlx::query_scalar::<_, String>(
        "SELECT entry.id FROM instance_content_entries entry JOIN instance_files file ON file.id = entry.file_id WHERE entry.content_set_id = ? AND file.instance_id = ? AND file.relative_path = ?",
    )
    .bind(content_set_id)
    .bind(instance_id)
    .bind(relative_path)
    .fetch_optional(exec)
    .await?;

    Ok(id)
}

/// Records whether one file's entries are enabled, telling the caller how many
/// rows it reached so a file without an entry can be classified afterwards.
pub(crate) async fn set_entries_enabled_for_file_in_transaction(
    content_set_id: &str,
    file_id: &str,
    enabled: bool,
    modified_at: i64,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<u64> {
    let result = sqlx::query(
        "UPDATE instance_content_entries
         SET enabled = ?, modified_at = ?
         WHERE content_set_id = ? AND file_id = ?",
    )
    .bind(i64::from(enabled))
    .bind(modified_at)
    .bind(content_set_id)
    .bind(file_id)
    .execute(&mut **tx)
    .await?;

    Ok(result.rows_affected())
}

/// Drops the entries that claim one file.
pub(crate) async fn delete_entries_for_file_in_transaction(
    content_set_id: &str,
    file_id: &str,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    sqlx::query(
        "DELETE FROM instance_content_entries
         WHERE content_set_id = ? AND file_id = ?",
    )
    .bind(content_set_id)
    .bind(file_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

/// Drops every provider reference of an entry.
pub(crate) async fn delete_provider_refs_for_entry_in_transaction(
    entry_id: &str,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    sqlx::query(
        "DELETE FROM instance_content_provider_refs WHERE content_entry_id = ?",
    )
    .bind(entry_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

/// Drops the dependency edges an entry is the parent of.
pub(crate) async fn delete_dependency_edges_for_parent_in_transaction(
    entry_id: &str,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    sqlx::query(
        "DELETE FROM instance_content_dependencies WHERE parent_entry_id = ?",
    )
    .bind(entry_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

/// Drops the update check an entry held.
pub(crate) async fn delete_update_check_for_entry_in_transaction(
    entry_id: &str,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    sqlx::query(
        "DELETE FROM instance_content_update_checks WHERE content_entry_id = ?",
    )
    .bind(entry_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

/// Reads the newest entry that claims a path of the content set, which the
/// caller needs to exist.
pub(crate) async fn require_entry_id_by_relative_path<'e, E>(
    content_set_id: &str,
    relative_path: &str,
    exec: E,
) -> crate::Result<String>
where
    E: Executor<'e, Database = Sqlite>,
{
    let id = sqlx::query_scalar::<_, String>(
        "SELECT entry.id FROM instance_content_entries entry JOIN instance_files file ON file.id = entry.file_id WHERE entry.content_set_id = ? AND file.relative_path = ? ORDER BY entry.modified_at DESC LIMIT 1",
    )
    .bind(content_set_id)
    .bind(relative_path)
    .fetch_one(exec)
    .await?;

    Ok(id)
}

/// Reads the provider references of a content set's entries, keyed by the file
/// each entry claims, with the provider that marks it as the origin.
pub(crate) async fn get_provider_refs_by_file_for_content_set(
    content_set_id: &str,
    pool: &SqlitePool,
) -> crate::Result<Vec<(String, ContentProviderRef, Option<ContentProvider>)>> {
    let rows = sqlx::query(
        "SELECT entry.file_id, ref.provider, ref.provider_project_id,
            ref.provider_release_id, ref.provider_file_id, ref.is_origin
         FROM instance_content_entries entry
         JOIN instance_content_provider_refs ref ON ref.content_entry_id = entry.id
         WHERE entry.content_set_id = ? AND entry.file_id IS NOT NULL
         ORDER BY entry.id, ref.is_origin DESC, ref.provider, ref.provider_project_id,
            ref.provider_release_id IS NULL, ref.provider_release_id",
    )
    .bind(content_set_id)
    .fetch_all(pool)
    .await?;

    let mut references = Vec::with_capacity(rows.len());
    for row in rows {
        let Some(file_id) = row.try_get::<Option<String>, _>("file_id")? else {
            continue;
        };
        let provider = row.try_get::<String, _>("provider")?;
        let project_id = row.try_get::<String, _>("provider_project_id")?;
        let release_id =
            row.try_get::<Option<String>, _>("provider_release_id")?;
        let file_id_ref =
            row.try_get::<Option<String>, _>("provider_file_id")?;
        let reference = ContentProviderRef::from_database(
            &provider,
            &project_id,
            release_id.as_deref(),
            file_id_ref.as_deref(),
        )?;
        let origin = (row.try_get::<i64, _>("is_origin")? != 0)
            .then(|| reference.provider());
        references.push((file_id, reference, origin));
    }

    Ok(references)
}

/// Reads the provider references of an instance's files, keyed by relative
/// path, with the origin provider and the source kind of the claiming entry.
pub(crate) async fn get_provider_refs_by_path(
    instance_id: &str,
    pool: &SqlitePool,
) -> crate::Result<
    Vec<(
        String,
        ContentProviderRef,
        Option<ContentProvider>,
        Option<ContentSourceKind>,
    )>,
> {
    let rows = sqlx::query(
        "SELECT file.relative_path, ref.provider, ref.provider_project_id,
                ref.provider_release_id, ref.provider_file_id, ref.is_origin, entry.source_kind
         FROM instance_files file
         INNER JOIN instance_content_entries entry ON entry.file_id = file.id
         INNER JOIN instance_content_provider_refs ref
            ON ref.content_entry_id = entry.id
         WHERE file.instance_id = ?
         ORDER BY file.relative_path, ref.is_origin DESC, ref.provider,
            ref.provider_project_id, ref.provider_release_id IS NULL,
            ref.provider_release_id",
    )
    .bind(instance_id)
    .fetch_all(pool)
    .await?;

    let mut references = Vec::with_capacity(rows.len());
    for row in rows {
        let provider = ContentProvider::from_str(row.try_get("provider")?)?;
        let path = row.try_get::<String, _>("relative_path")?;
        let provenance = ContentProviderRef::from_database(
            provider.as_str(),
            row.try_get("provider_project_id")?,
            row.try_get::<Option<String>, _>("provider_release_id")?
                .as_deref(),
            row.try_get::<Option<String>, _>("provider_file_id")?
                .as_deref(),
        )?;
        let origin =
            (row.try_get::<i64, _>("is_origin")? != 0).then_some(provider);
        let source_kind = row
            .try_get::<Option<String>, _>("source_kind")?
            .and_then(|value| ContentSourceKind::from_str(&value).ok());
        references.push((path, provenance, origin, source_kind));
    }

    Ok(references)
}

/// Reads the provider and project id of every provider reference behind the
/// content set's present files.
pub(crate) async fn get_provider_project_keys_for_present_files(
    content_set_id: &str,
    pool: &SqlitePool,
) -> crate::Result<Vec<(String, String)>> {
    let rows = sqlx::query(
        "SELECT DISTINCT ref.provider, ref.provider_project_id
         FROM instance_content_entries entry
         INNER JOIN instance_files file ON file.id = entry.file_id
         INNER JOIN instance_content_provider_refs ref
            ON ref.content_entry_id = entry.id
         WHERE entry.content_set_id = ? AND file.missing = 0",
    )
    .bind(content_set_id)
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|row| {
            Ok((
                row.try_get::<String, _>("provider")?,
                row.try_get::<String, _>("provider_project_id")?,
            ))
        })
        .collect()
}

/// Forgets the entries a shared pack left in `instance_id` for one path.
pub(crate) async fn delete_shared_entries_for_path_in_transaction(
    instance_id: &str,
    relative_path: &str,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    sqlx::query(
        "DELETE FROM instance_content_entries
         WHERE instance_id = ?
           AND source_kind = 'shared_instance'
           AND file_id IN (
             SELECT id FROM instance_files
             WHERE instance_id = ? AND relative_path = ?
           )",
    )
    .bind(instance_id)
    .bind(instance_id)
    .bind(relative_path)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

/// Records whether one entry is enabled.
pub(crate) async fn set_entry_enabled_in_transaction(
    entry_id: &str,
    enabled: bool,
    modified_at: i64,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    sqlx::query(
        "UPDATE instance_content_entries SET enabled = ?, modified_at = ? WHERE id = ?",
    )
    .bind(i64::from(enabled))
    .bind(modified_at)
    .bind(entry_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

/// Gives one entry the id the caller minted for it.
pub(crate) async fn rewrite_entry_id_in_transaction(
    entry_id: &str,
    new_entry_id: &str,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    sqlx::query("UPDATE instance_content_entries SET id = ? WHERE id = ?")
        .bind(new_entry_id)
        .bind(entry_id)
        .execute(&mut **tx)
        .await?;

    Ok(())
}

/// Restores what an entry says about where it came from and who owns it.
pub(crate) async fn set_entry_provenance_in_transaction(
    entry_id: &str,
    ownership_kind: ContentOwnershipKind,
    source_kind: ContentSourceKind,
    auto_dependency: bool,
    server_requirement: ContentRequirement,
    client_requirement: ContentRequirement,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    sqlx::query(
        "UPDATE instance_content_entries SET ownership_kind = ?, source_kind = ?, auto_dependency = ?, server_requirement = ?, client_requirement = ? WHERE id = ?",
    )
    .bind(ownership_kind.as_str())
    .bind(source_kind.as_str())
    .bind(i64::from(auto_dependency))
    .bind(server_requirement.as_str())
    .bind(client_requirement.as_str())
    .bind(entry_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

/// Drops every entry a content set holds.
pub(crate) async fn delete_entries_for_set_in_transaction(
    content_set_id: &str,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    sqlx::query(
        "DELETE FROM instance_content_entries WHERE content_set_id = ?",
    )
    .bind(content_set_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

/// Drops only the entries a content set holds because a pack owns them.
pub(crate) async fn delete_pack_managed_entries_for_set_in_transaction(
    content_set_id: &str,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    sqlx::query(
        "DELETE FROM instance_content_entries
         WHERE content_set_id = ? AND ownership_kind = 'pack_managed'",
    )
    .bind(content_set_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

/// Drops every dependency edge a content set holds.
pub(crate) async fn delete_dependency_edges_for_set_in_transaction(
    content_set_id: &str,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    sqlx::query(
        "DELETE FROM instance_content_dependencies WHERE content_set_id = ?",
    )
    .bind(content_set_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

/// Reads the module metadata an instance's present entries carry, as the entry
/// each belongs to and the text it is stored as.
pub(crate) async fn get_local_mod_data_by_entry<'e, E>(
    instance_id: &str,
    exec: E,
) -> crate::Result<Vec<(String, String)>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let rows = sqlx::query_as(
        "SELECT entry.id, file.local_mod_data FROM instance_content_entries entry JOIN instance_files file ON file.id = entry.file_id WHERE entry.instance_id = ? AND file.missing = 0 AND file.local_mod_data IS NOT NULL",
    )
    .bind(instance_id)
    .fetch_all(exec)
    .await?;

    Ok(rows)
}

/// What an entry says about one CurseForge-linked path of an instance.
pub(crate) struct CurseForgeLinkedFile {
    pub provider_project_id: String,
    pub provider_release_id: Option<String>,
    pub project_type: String,
    pub ownership_kind: String,
    pub game_version: String,
    pub loader: String,
}

/// Reads the newest entry behind one path of an instance that carries a
/// CurseForge reference.
pub(crate) async fn get_curseforge_linked_file<'e, E>(
    instance_id: &str,
    relative_path: &str,
    exec: E,
) -> crate::Result<Option<CurseForgeLinkedFile>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let row = sqlx::query(
        "SELECT ref.provider_project_id, ref.provider_release_id, entry.project_type,
                entry.ownership_kind,
                content_set.game_version, content_set.loader
         FROM instance_files file
         INNER JOIN instance_content_entries entry ON entry.file_id = file.id
         INNER JOIN instance_content_provider_refs ref
            ON ref.content_entry_id = entry.id AND ref.provider = 'curseforge'
         INNER JOIN instance_content_sets content_set
            ON content_set.id = entry.content_set_id
         WHERE file.instance_id = ? AND file.relative_path = ?
         ORDER BY entry.modified_at DESC
         LIMIT 1",
    )
    .bind(instance_id)
    .bind(relative_path)
    .fetch_optional(exec)
    .await?;

    let Some(row) = row else {
        return Ok(None);
    };

    Ok(Some(CurseForgeLinkedFile {
        provider_project_id: row.try_get("provider_project_id")?,
        provider_release_id: row.try_get("provider_release_id")?,
        project_type: row.try_get("project_type")?,
        ownership_kind: row.try_get("ownership_kind")?,
        game_version: row.try_get("game_version")?,
        loader: row.try_get("loader")?,
    }))
}
/// What an entry says about one path of the content set an instance applies.
pub(crate) struct AppliedContentTarget {
    pub project_type: String,
    pub ownership_kind: String,
    pub game_version: String,
    pub loader: String,
}

/// Reads the newest entry behind one path whose content set the instance still
/// applies.
pub(crate) async fn get_applied_content_target<'e, E>(
    instance_id: &str,
    relative_path: &str,
    exec: E,
) -> crate::Result<Option<AppliedContentTarget>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let row = sqlx::query(
        "SELECT entry.project_type, entry.ownership_kind,
                content_set.game_version, content_set.loader
         FROM instance_files file
         INNER JOIN instance_content_entries entry ON entry.file_id = file.id
         INNER JOIN instances instance ON instance.id = entry.instance_id
         INNER JOIN instance_content_sets content_set
            ON content_set.id = entry.content_set_id
         WHERE file.instance_id = ? AND file.relative_path = ?
            AND entry.content_set_id = instance.applied_content_set_id
         ORDER BY entry.modified_at DESC
         LIMIT 1",
    )
    .bind(instance_id)
    .bind(relative_path)
    .fetch_optional(exec)
    .await?;

    let Some(row) = row else {
        return Ok(None);
    };

    Ok(Some(AppliedContentTarget {
        project_type: row.try_get("project_type")?,
        ownership_kind: row.try_get("ownership_kind")?,
        game_version: row.try_get("game_version")?,
        loader: row.try_get("loader")?,
    }))
}

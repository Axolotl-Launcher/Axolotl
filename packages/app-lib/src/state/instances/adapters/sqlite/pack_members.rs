use super::values::{project_type_from_str, timestamp, unsigned};

use super::content_sets::bump_content_set_revision_in_transaction;
use crate::state::instances::{
    ContentOwnershipKind, ManualDownloadOperationKind, ManualDownloadState,
    PackMember, PackMemberMaterializationState, PackMemberOverrideKind,
    PendingManualDownload,
};
use crate::state::{ContentProvider, ProjectType};
use chrono::{DateTime, Utc};
use sqlx::{Executor, Row, Sqlite, SqlitePool, Transaction};

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct PackMemberRow {
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
pub(crate) struct PendingManualDownloadRow {
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

/// Detaches an instance's applied content set from the pack it came from: the
/// pack's members go, its entries become user-added content, and the content
/// set turns local.
///
/// Rejects a content set that belongs to another instance, and an instance
/// whose install job is queued, running, canceling or waiting for the user.
pub(crate) async fn unlink_pack_in_transaction(
    instance_id: &str,
    content_set_id: &str,
    unlinked_at: DateTime<Utc>,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    let belongs: i64 = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM instance_content_sets WHERE id = ? AND instance_id = ?)",
    )
    .bind(content_set_id)
    .bind(instance_id)
    .fetch_one(&mut **tx)
    .await?;
    if belongs == 0 {
        return Err(crate::ErrorKind::InputError(
            "Applied content set does not belong to this instance".into(),
        )
        .into());
    }

    if crate::install::store::has_unfinished_job_in_transaction(instance_id, tx)
        .await?
    {
        return Err(crate::ErrorKind::InputError(
            "Wait for the active instance task before unlinking the pack"
                .into(),
        )
        .into());
    }

    sqlx::query(
        "INSERT INTO resource_pack_unlinks(instance_id, content_set_id, unlinked_at) VALUES (?, ?, ?) ON CONFLICT(instance_id) DO UPDATE SET content_set_id = excluded.content_set_id, unlinked_at = excluded.unlinked_at",
    )
    .bind(instance_id)
    .bind(content_set_id)
    .bind(unlinked_at.timestamp())
    .execute(&mut **tx)
    .await?;
    detach_pack_content_in_transaction(
        content_set_id,
        Some(unlinked_at.timestamp()),
        tx,
    )
    .await
}

/// Turns a content set local: its entries become user-added content, its pack
/// members go, and the revision advances.
///
/// A fresh unlink stamps `modified` and `modified_at`; replaying one from a
/// marker has no clock of its own and leaves those columns alone.
async fn detach_pack_content_in_transaction(
    content_set_id: &str,
    modified: Option<i64>,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    match modified {
        Some(modified) => {
            sqlx::query(
                "UPDATE instance_content_sets SET source_kind = 'local', modified = ? WHERE id = ?",
            )
            .bind(modified)
            .bind(content_set_id)
            .execute(&mut **tx)
            .await?;
            sqlx::query(
                "UPDATE instance_content_entries SET source_kind = 'local', ownership_kind = 'user_added', modified_at = ? WHERE content_set_id = ? AND ownership_kind = 'pack_managed'",
            )
            .bind(modified)
            .bind(content_set_id)
            .execute(&mut **tx)
            .await?;
        }
        None => {
            sqlx::query("UPDATE instance_content_entries SET ownership_kind = 'user_added', source_kind = 'local' WHERE content_set_id = ? AND ownership_kind = 'pack_managed'")
                .bind(content_set_id)
                .execute(&mut **tx)
                .await?;
            sqlx::query("UPDATE instance_content_sets SET source_kind = 'local' WHERE id = ?")
                .bind(content_set_id)
                .execute(&mut **tx)
                .await?;
        }
    }
    sqlx::query("DELETE FROM instance_pack_members WHERE content_set_id = ?")
        .bind(content_set_id)
        .execute(&mut **tx)
        .await?;
    bump_content_set_revision_in_transaction(content_set_id, tx).await?;

    Ok(())
}

/// Drops the recorded pack unlink for an instance that is linked again.
pub(crate) async fn relink_pack_in_transaction(
    instance_id: &str,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    sqlx::query("DELETE FROM resource_pack_unlinks WHERE instance_id = ?")
        .bind(instance_id)
        .execute(&mut **tx)
        .await?;

    Ok(())
}

/// Re-applies a recorded pack unlink that a stale write left behind.
///
/// Acts only while the marker still names the applied content set of an
/// unmanaged instance and that set still carries pack content; otherwise the
/// instance has moved on and this does nothing.
pub(crate) async fn reconcile_pack_unlink_in_transaction(
    instance_id: &str,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    let set: Option<String> = sqlx::query_scalar("SELECT marker.content_set_id FROM resource_pack_unlinks marker JOIN instances instance ON instance.id = marker.instance_id LEFT JOIN instance_links link ON link.instance_id = instance.id WHERE marker.instance_id = ? AND instance.applied_content_set_id = marker.content_set_id AND COALESCE(link.link_kind, 'unmanaged') = 'unmanaged' AND (EXISTS(SELECT 1 FROM instance_pack_members WHERE content_set_id = marker.content_set_id) OR EXISTS(SELECT 1 FROM instance_content_entries WHERE content_set_id = marker.content_set_id AND ownership_kind = 'pack_managed'))")
        .bind(instance_id)
        .fetch_optional(&mut **tx)
        .await?;
    if let Some(set) = set {
        detach_pack_content_in_transaction(&set, None, tx).await?;
    }

    Ok(())
}

#[cfg(test)]
#[path = "pack_members_tests.rs"]
mod tests;

/// Whether the content set still has a member the pack snapshot has not
/// reconciled against.
pub(crate) async fn has_unreconciled_members(
    content_set_id: &str,
    pool: &SqlitePool,
) -> crate::Result<bool> {
    let unreconciled: i64 = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1 FROM instance_pack_members
            WHERE content_set_id = ? AND reconciled = 0
        )",
    )
    .bind(content_set_id)
    .fetch_one(pool)
    .await?;

    Ok(unreconciled != 0)
}

/// Whether a required CurseForge member of the content set is still missing.
pub(crate) async fn has_missing_required_curseforge_member(
    content_set_id: &str,
    pool: &SqlitePool,
) -> crate::Result<bool> {
    let missing: i64 = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1 FROM instance_pack_members member
            WHERE member.content_set_id = ?
                AND member.provider = 'curseforge'
                AND member.required = 1
                AND member.materialization_state = 'missing'
        )",
    )
    .bind(content_set_id)
    .fetch_one(pool)
    .await?;

    Ok(missing != 0)
}

/// Reads the CurseForge downloads the instance is still waiting on.
pub(crate) async fn get_curseforge_pending_downloads<'e, E>(
    instance_id: &str,
    exec: E,
) -> crate::Result<Vec<(String, String)>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let rows = sqlx::query(
        "SELECT provider_project_id, provider_release_id
         FROM instance_pending_manual_downloads
         WHERE instance_id = ? AND provider = 'curseforge'
            AND state IN ('waiting', 'matched', 'error')",
    )
    .bind(instance_id)
    .fetch_all(exec)
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

/// Records whether the pack requires a member, once reconciliation placed it.
pub(crate) async fn set_pack_member_required_in_transaction(
    member_id: &str,
    required: bool,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    sqlx::query(
        "UPDATE instance_pack_members
         SET required = ?, reconciled = 1 WHERE id = ?",
    )
    .bind(i64::from(required))
    .bind(member_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

/// Drops a member the current pack no longer contains.
pub(crate) async fn delete_pack_member_in_transaction(
    member_id: &str,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    sqlx::query("DELETE FROM instance_pack_members WHERE id = ?")
        .bind(member_id)
        .execute(&mut **tx)
        .await?;

    Ok(())
}

/// Reads the local members the pack snapshot has not reconciled yet, as their
/// id and expected path.
pub(crate) async fn get_unreconciled_local_members<'e, E>(
    content_set_id: &str,
    exec: E,
) -> crate::Result<Vec<(String, String)>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let rows = sqlx::query_as(
        "SELECT id, expected_relative_path FROM instance_pack_members
         WHERE content_set_id = ? AND provider IS NULL AND reconciled = 0",
    )
    .bind(content_set_id)
    .fetch_all(exec)
    .await?;

    Ok(rows)
}

/// Marks one local member as reconciled.
pub(crate) async fn mark_pack_member_reconciled_in_transaction(
    member_id: &str,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    sqlx::query("UPDATE instance_pack_members SET reconciled = 1 WHERE id = ?")
        .bind(member_id)
        .execute(&mut **tx)
        .await?;

    Ok(())
}

/// A content entry the pack snapshot may claim.
pub(crate) struct CurseForgeReconciliationCandidate {
    pub entry_id: String,
    pub relative_path: String,
    pub enabled: bool,
    pub missing: bool,
    pub provider_project_id: Option<String>,
    pub provider_release_id: Option<String>,
}

/// Reads the entries of a content set with their CurseForge reference, newest
/// first, as the candidates a pack snapshot may claim.
pub(crate) async fn get_curseforge_reconciliation_candidates<'e, E>(
    content_set_id: &str,
    exec: E,
) -> crate::Result<Vec<CurseForgeReconciliationCandidate>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let rows = sqlx::query(
        "SELECT entry.id AS entry_id, file.relative_path, entry.enabled,
            file.missing, ref.provider_project_id, ref.provider_release_id
         FROM instance_content_entries entry
         INNER JOIN instance_files file ON file.id = entry.file_id
         LEFT JOIN instance_content_provider_refs ref
            ON ref.content_entry_id = entry.id AND ref.provider = 'curseforge'
         WHERE entry.content_set_id = ?
         ORDER BY entry.modified_at DESC",
    )
    .bind(content_set_id)
    .fetch_all(exec)
    .await?;

    rows.into_iter()
        .map(|row| {
            Ok(CurseForgeReconciliationCandidate {
                entry_id: row.try_get("entry_id")?,
                relative_path: row.try_get("relative_path")?,
                enabled: row.try_get::<i64, _>("enabled")? != 0,
                missing: row.try_get::<i64, _>("missing")? != 0,
                provider_project_id: row.try_get("provider_project_id")?,
                provider_release_id: row.try_get("provider_release_id")?,
            })
        })
        .collect()
}

/// Reads the members of a content set as reconciliation sees them: keyed by
/// their member key, with the override path of local ones.
pub(crate) async fn get_reconciliation_members<'e, E>(
    content_set_id: &str,
    exec: E,
) -> crate::Result<
    Vec<(
        String,
        String,
        PackMemberOverrideKind,
        PackMemberMaterializationState,
        Option<String>,
    )>,
>
where
    E: Executor<'e, Database = Sqlite>,
{
    let rows = sqlx::query(
        "SELECT id, member_key, override_kind, materialization_state,
            CASE WHEN provider IS NULL THEN expected_relative_path END AS override_path
         FROM instance_pack_members WHERE content_set_id = ?",
    )
    .bind(content_set_id)
    .fetch_all(exec)
    .await?;

    rows.into_iter()
        .map(|row| {
            Ok((
                row.try_get::<String, _>("member_key")?,
                row.try_get::<String, _>("id")?,
                PackMemberOverrideKind::from_str(
                    &row.try_get::<String, _>("override_kind")?,
                )?,
                PackMemberMaterializationState::from_str(
                    &row.try_get::<String, _>("materialization_state")?,
                )?,
                row.try_get::<Option<String>, _>("override_path")?,
            ))
        })
        .collect()
}

/// Reads the CurseForge members of a content set as the manual download pass
/// needs them, keyed by their provider ids.
pub(crate) async fn get_curseforge_member_rows<'e, E>(
    content_set_id: &str,
    exec: E,
) -> crate::Result<
    Vec<(
        (Option<String>, Option<String>),
        String,
        Option<String>,
        PackMemberMaterializationState,
        PackMemberOverrideKind,
    )>,
>
where
    E: Executor<'e, Database = Sqlite>,
{
    let rows = sqlx::query(
        "SELECT id, content_entry_id, provider_project_id,
                provider_release_id, materialization_state, override_kind
         FROM instance_pack_members
         WHERE content_set_id = ? AND provider = 'curseforge'",
    )
    .bind(content_set_id)
    .fetch_all(exec)
    .await?;

    rows.into_iter()
        .map(|row| {
            Ok((
                (
                    row.try_get::<Option<String>, _>("provider_project_id")?,
                    row.try_get::<Option<String>, _>("provider_release_id")?,
                ),
                row.try_get::<String, _>("id")?,
                row.try_get::<Option<String>, _>("content_entry_id")?,
                PackMemberMaterializationState::from_str(
                    &row.try_get::<String, _>("materialization_state")?,
                )?,
                PackMemberOverrideKind::from_str(
                    &row.try_get::<String, _>("override_kind")?,
                )?,
            ))
        })
        .collect()
}

/// Records how the pack now treats the members that point at one file.
pub(crate) async fn set_override_for_file_in_transaction(
    content_set_id: &str,
    file_id: &str,
    override_kind: PackMemberOverrideKind,
    modified_at: i64,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    sqlx::query(
        "UPDATE instance_pack_members
         SET override_kind = ?, materialization_state = 'present',
             modified_at = ?
         WHERE content_entry_id IN (
            SELECT id FROM instance_content_entries
            WHERE content_set_id = ? AND file_id = ?
         )",
    )
    .bind(override_kind.as_str())
    .bind(modified_at)
    .bind(content_set_id)
    .bind(file_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

/// Detaches the members that point at one file, keeping them as removed
/// overrides of the pack.
pub(crate) async fn detach_members_for_file_in_transaction(
    content_set_id: &str,
    file_id: &str,
    modified_at: i64,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    sqlx::query(
        "UPDATE instance_pack_members
         SET content_entry_id = NULL,
             materialization_state = 'removed',
             override_kind = 'removed',
             modified_at = ?
         WHERE content_entry_id IN (
            SELECT id FROM instance_content_entries
            WHERE content_set_id = ? AND file_id = ?
         )",
    )
    .bind(modified_at)
    .bind(content_set_id)
    .bind(file_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

/// Records a member of the shared pack `pack_id`, replacing what its member
/// key held.
pub(crate) async fn upsert_shared_pack_member_in_transaction(
    pack_id: &str,
    content_set_id: &str,
    content_entry_id: &str,
    project_type: &str,
    expected_relative_path: &str,
    expected_sha1: &str,
    expected_size: i64,
    modified_at: i64,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    sqlx::query(
        "INSERT INTO instance_pack_members
         (id, content_set_id, content_entry_id, member_key, project_type,
          expected_relative_path, required, expected_sha1, expected_size,
          materialization_state, override_kind, reconciled, created_at, modified_at)
         VALUES (?, ?, ?, ?, ?, ?, 0, ?, ?, 'present', 'none', 1, ?, ?)
         ON CONFLICT(content_set_id, member_key) DO UPDATE SET
           content_entry_id=excluded.content_entry_id,
           expected_sha1=excluded.expected_sha1, expected_size=excluded.expected_size,
           materialization_state='present', override_kind='none',
           reconciled=1, modified_at=excluded.modified_at",
    )
    .bind(format!("pack-member:{pack_id}"))
    .bind(content_set_id)
    .bind(content_entry_id)
    .bind(format!("shared:{pack_id}"))
    .bind(project_type)
    .bind(expected_relative_path)
    .bind(expected_sha1)
    .bind(expected_size)
    .bind(modified_at)
    .bind(modified_at)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

/// Forgets the member the shared pack `pack_id` left in one instance's
/// content sets.
pub(crate) async fn delete_shared_member_in_transaction(
    pack_id: &str,
    instance_id: &str,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    sqlx::query(
        "DELETE FROM instance_pack_members
         WHERE member_key = ?
           AND content_set_id IN (
             SELECT id FROM instance_content_sets WHERE instance_id = ?
           )",
    )
    .bind(format!("shared:{pack_id}"))
    .bind(instance_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

/// Drops every member a content set holds.
pub(crate) async fn delete_pack_members_for_set_in_transaction(
    content_set_id: &str,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    sqlx::query("DELETE FROM instance_pack_members WHERE content_set_id = ?")
        .bind(content_set_id)
        .execute(&mut **tx)
        .await?;

    Ok(())
}

/// Records that the user picked a version for the member one entry belongs to.
pub(crate) async fn mark_entry_version_override_in_transaction(
    entry_id: &str,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    sqlx::query(
        "UPDATE instance_pack_members SET override_kind = 'version' WHERE content_entry_id = ?",
    )
    .bind(entry_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

/// Records that the user picked a version for the member one path of a content
/// set belongs to, answering how many members it reached.
pub(crate) async fn mark_path_version_override_in_transaction(
    content_set_id: &str,
    relative_path: &str,
    modified_at: i64,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<u64> {
    let updated = sqlx::query(
        "UPDATE instance_pack_members
         SET override_kind = 'version', modified_at = ?
         WHERE content_entry_id IN (
            SELECT entry.id
            FROM instance_content_entries entry
            INNER JOIN instance_files file ON file.id = entry.file_id
            WHERE entry.content_set_id = ? AND file.relative_path = ?
         )",
    )
    .bind(modified_at)
    .bind(content_set_id)
    .bind(relative_path)
    .execute(&mut **tx)
    .await?;

    Ok(updated.rows_affected())
}

/// The instances still waiting on a CurseForge manual download.
pub(crate) async fn get_instance_ids_with_pending_curseforge_downloads<'e, E>(
    exec: E,
) -> crate::Result<Vec<String>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let rows = sqlx::query_scalar(
        "SELECT DISTINCT instance_id
         FROM instance_pending_manual_downloads
         WHERE provider = 'curseforge'
            AND state IN ('waiting', 'matched', 'error')",
    )
    .fetch_all(exec)
    .await?;

    Ok(rows)
}

/// The member one content set keeps under a member key.
pub(crate) async fn find_member_id_by_key<'e, E>(
    content_set_id: &str,
    member_key: &str,
    exec: E,
) -> crate::Result<Option<String>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let id = sqlx::query_scalar(
        "SELECT id FROM instance_pack_members
         WHERE content_set_id = ? AND member_key = ?",
    )
    .bind(content_set_id)
    .bind(member_key)
    .fetch_optional(exec)
    .await?;

    Ok(id)
}

/// The entry the member one content set keeps under a member key claims.
pub(crate) async fn find_member_entry_id<'e, E>(
    content_set_id: &str,
    member_key: &str,
    exec: E,
) -> crate::Result<Option<String>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let entry_id: Option<Option<String>> = sqlx::query_scalar(
        "SELECT content_entry_id FROM instance_pack_members
         WHERE content_set_id = ? AND member_key = ?",
    )
    .bind(content_set_id)
    .bind(member_key)
    .fetch_optional(exec)
    .await?;

    Ok(entry_id.flatten())
}

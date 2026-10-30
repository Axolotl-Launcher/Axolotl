//! Atomic reconciliation of files discovered outside the instance write lock.

use super::content_file_fingerprints::FileFingerprintRecord;
use super::{Collection as _, Handle};
use crate::state::instances::adapters::sqlite;
use crate::state::instances::{ContentSet, Instance, InstanceFile};
use std::collections::{HashMap, HashSet};

pub(crate) async fn get_instance(
    handle: &Handle,
    instance_id: &str,
) -> crate::Result<Option<Instance>> {
    sqlite::instance_rows::get_instance_by_id(instance_id, handle.pool()).await
}

pub(crate) async fn get_content_set(
    handle: &Handle,
    instance_id: &str,
) -> crate::Result<Option<ContentSet>> {
    sqlite::content_sets::get_applied_content_set(instance_id, handle.pool())
        .await
}

pub(crate) async fn get_files(
    handle: &Handle,
    instance_id: &str,
) -> crate::Result<Vec<InstanceFile>> {
    sqlite::files::get_instance_files(instance_id, handle.pool()).await
}

pub(crate) async fn get_entry_file_ids(
    handle: &Handle,
    content_set_id: &str,
) -> crate::Result<HashSet<String>> {
    Ok(sqlite::content_entries::get_content_entries(
        content_set_id,
        handle.pool(),
    )
    .await?
    .into_iter()
    .filter_map(|entry| entry.file_id)
    .collect())
}

pub(crate) struct ScannedFiles<'a> {
    pub complete: bool,
    pub create_entries: bool,
    pub instance_id: &'a str,
    pub content_set: &'a Option<ContentSet>,
    pub files: &'a [InstanceFile],
    pub reclaims: &'a HashMap<String, String>,
    pub merges: &'a HashMap<String, String>,
    pub externally_changed_file_ids: &'a HashSet<String>,
    pub refreshed_metadata_file_ids: &'a HashSet<String>,
    pub fingerprints: &'a HashMap<String, FileFingerprintRecord>,
}

pub(crate) async fn set_scanned<F, Fut>(
    handle: &Handle,
    scan: ScannedFiles<'_>,
    still_current: F,
) -> crate::Result<Option<Vec<InstanceFile>>>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let ScannedFiles {
        complete,
        create_entries,
        instance_id,
        content_set,
        files,
        reclaims,
        merges,
        externally_changed_file_ids,
        refreshed_metadata_file_ids,
        fingerprints,
    } = scan;
    let mut tx = handle.pool().begin_with("BEGIN IMMEDIATE").await?;
    sqlite::write_guards::ensure_instance_exists(instance_id, &mut tx).await?;
    if complete {
        sqlite::files::mark_instance_files_missing(instance_id, &mut tx)
            .await?;
    }
    let mut invalidated_provider_identity =
        !externally_changed_file_ids.is_empty();
    for file_id in externally_changed_file_ids {
        sqlx::query("UPDATE instance_files SET local_mod_data = NULL, icon_path = NULL WHERE instance_id = ? AND id = ?").bind(instance_id).bind(file_id).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM instance_content_dependencies WHERE parent_entry_id IN (SELECT id FROM instance_content_entries WHERE instance_id = ? AND file_id = ?)").bind(instance_id).bind(file_id).execute(&mut *tx).await?;
    }
    if let Some(content_set) = content_set.as_ref() {
        let refreshed_metadata_file_ids: Vec<_> =
            refreshed_metadata_file_ids.iter().collect();
        for file_ids in refreshed_metadata_file_ids.chunks(400) {
            let mut query = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
                "DELETE FROM instance_content_dependencies WHERE content_set_id = ",
            );
            query.push_bind(&content_set.id).push(" AND evidence_provider = 'local' AND parent_entry_id IN (SELECT entry.id FROM instance_content_entries entry WHERE entry.content_set_id = ");
            query
                .push_bind(&content_set.id)
                .push(" AND entry.file_id IN (");
            let mut separated = query.separated(", ");
            for id in file_ids {
                separated.push_bind(*id);
            }
            separated.push_unseparated(") AND NOT EXISTS (SELECT 1 FROM instance_content_provider_refs ref WHERE ref.content_entry_id = entry.id))");
            let removed = query.build().execute(&mut *tx).await?;
            invalidated_provider_identity |= removed.rows_affected() > 0;
        }
        for file_id in externally_changed_file_ids {
            invalidated_provider_identity |= sqlite::content_entries::invalidate_exact_provider_refs_for_file_in_transaction(
                &content_set.id,
                file_id,
                &mut tx,
            )
            .await?;
        }
    }

    // Upsert with a fresh id lookup inside the transaction. The ids assigned
    // during the scan may be stale if a concurrent operation (e.g. batch
    // disable renaming files to `.disabled`) moved a row after the snapshot;
    // reusing a stale id against the moved row would trip the UNIQUE
    // constraint on `instance_files.id` (code 1555).
    let mut synced_files: Vec<InstanceFile> = Vec::with_capacity(files.len());
    for file in files {
        let synced = if let Some(tracked_file_id) =
            merges.get(&file.relative_path)
        {
            sqlite::files::adopt_untracked_file_in_transaction(
                instance_id,
                &file.relative_path,
                tracked_file_id,
                &mut tx,
            )
            .await?;
            upsert_scanned_file(instance_id, file, &mut tx).await?
        } else if let Some(old_relative_path) =
            reclaims.get(&file.relative_path)
        {
            match sqlite::files::move_instance_file_in_transaction(
                instance_id,
                old_relative_path,
                &file.relative_path,
                &file.file_name,
                file.enabled,
                &file.sha1,
                file.size,
                file.local_mod_data.as_deref(),
                file.icon_path.as_deref(),
                &mut tx,
            )
            .await?
            {
                Some(file) => file,
                None => upsert_scanned_file(instance_id, file, &mut tx).await?,
            }
        } else {
            upsert_scanned_file(instance_id, file, &mut tx).await?
        };
        synced_files.push(synced);
    }

    if create_entries && let Some(content_set) = content_set {
        for file in &synced_files {
            let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM instance_content_entries WHERE content_set_id = ? AND file_id = ?)")
                .bind(&content_set.id).bind(&file.id).fetch_one(&mut *tx).await?;
            if exists {
                continue;
            }
            let Some(kind) = crate::state::instances::adapters::filesystem::project_type_from_relative_path(&file.relative_path) else { continue; };
            sqlite::content_entries::upsert_content_entry_from_parts_in_transaction(
                sqlite::content_entries::UpsertContentEntry {
                    instance_id, content_set_id: &content_set.id, file_id: Some(&file.id), project_type: kind,
                    source_kind: crate::state::ContentSourceKind::Local,
                    ownership_kind: crate::state::ContentOwnershipKind::UserAdded, auto_dependency: false,
                    server_requirement: crate::state::ContentRequirement::Unknown,
                    client_requirement: crate::state::ContentRequirement::Unknown, enabled: file.enabled,
                }, &mut tx,
            ).await?;
            invalidated_provider_identity = true;
        }
    }
    if invalidated_provider_identity
        && let Some(content_set) = content_set.as_ref()
    {
        sqlite::content_sets::bump_content_set_revision_in_transaction(
            &content_set.id,
            &mut tx,
        )
        .await?;
    }

    if !still_current().await {
        tx.rollback().await?;
        return Ok(None);
    }
    for file in &synced_files {
        if let Some(record) = fingerprints.get(&file.relative_path)
            && record.sha1().eq_ignore_ascii_case(&file.sha1)
            && record.size == file.size
        {
            super::content_file_fingerprints::ContentFileFingerprints::write(
                &mut tx,
                &super::content_file_fingerprints::FingerprintKey {
                    instance_id: instance_id.into(),
                    file_id: file.id.clone(),
                },
                Some(record),
            )
            .await?;
        }
    }
    if complete {
        for table in [
            "content_file_fingerprints",
            "content_refresh_checks",
            "content_refresh_presentations",
        ] {
            let query = format!(
                "DELETE FROM {table} WHERE instance_id = ? AND file_id IN (SELECT id FROM instance_files WHERE instance_id = ? AND missing = 1)"
            );
            sqlx::query(&query)
                .bind(instance_id)
                .bind(instance_id)
                .execute(&mut *tx)
                .await?;
        }
    }
    tx.commit().await?;
    Ok(Some(synced_files))
}

async fn upsert_scanned_file(
    instance_id: &str,
    file: &InstanceFile,
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
) -> crate::Result<InstanceFile> {
    sqlite::files::upsert_instance_file_from_parts_in_transaction(
        sqlite::files::UpsertInstanceFile {
            instance_id,
            relative_path: &file.relative_path,
            file_name: &file.file_name,
            enabled: file.enabled,
            sha1: &file.sha1,
            size: file.size,
            missing: false,
            local_mod_data: file.local_mod_data.as_deref(),
            icon_path: file.icon_path.as_deref(),
        },
        tx,
    )
    .await
}

pub(crate) async fn set_discovered_entries(
    handle: &Handle,
    instance_id: &str,
) -> crate::Result<()> {
    use crate::state::{
        ContentOwnershipKind, ContentRequirement, ContentSourceKind,
    };
    let mut tx = handle.pool().begin_with("BEGIN IMMEDIATE").await?;
    let content_set =
        sqlite::content_sets::get_applied_content_set(instance_id, &mut *tx)
            .await?
            .ok_or_else(|| {
                crate::ErrorKind::InputError(
                    "Instance has no applied content set".into(),
                )
            })?;
    let rows: Vec<(String, String, i64)> = sqlx::query_as(
        "SELECT file.id, file.relative_path, file.enabled FROM instance_files file WHERE file.instance_id = ? AND file.missing = 0 AND NOT EXISTS (SELECT 1 FROM instance_content_entries entry WHERE entry.content_set_id = ? AND entry.file_id = file.id)"
    ).bind(instance_id).bind(&content_set.id).fetch_all(&mut *tx).await?;
    let mut changed = false;
    for (id, path, enabled) in rows {
        let Some(kind) = crate::state::instances::adapters::filesystem::project_type_from_relative_path(&path) else { continue };
        if !matches!(
            kind,
            crate::state::ProjectType::Mod
                | crate::state::ProjectType::ResourcePack
                | crate::state::ProjectType::ShaderPack
                | crate::state::ProjectType::DataPack
        ) {
            continue;
        }
        sqlite::content_entries::upsert_content_entry_from_parts_in_transaction(
            sqlite::content_entries::UpsertContentEntry {
                instance_id,
                content_set_id: &content_set.id,
                file_id: Some(&id),
                project_type: kind,
                source_kind: ContentSourceKind::Local,
                ownership_kind: ContentOwnershipKind::UserAdded,
                auto_dependency: false,
                server_requirement: ContentRequirement::Unknown,
                client_requirement: ContentRequirement::Unknown,
                enabled: enabled != 0,
            },
            &mut tx,
        )
        .await?;
        changed = true;
    }
    changed |= sqlite::pack_members::repair_pack_members_in_transaction(
        &content_set.id,
        &mut tx,
    )
    .await?;
    if changed {
        sqlite::content_sets::bump_content_set_revision_in_transaction(
            &content_set.id,
            &mut tx,
        )
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

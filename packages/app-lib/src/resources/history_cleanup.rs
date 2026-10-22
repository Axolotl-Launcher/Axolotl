use super::*;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

fn invalid(message: impl Into<String>) -> ResourceError {
    ResourceError::InvalidRequest(message.into())
}
fn path_identity(path: &str) -> String {
    if cfg!(windows) {
        path.to_lowercase()
    } else {
        path.to_owned()
    }
}

#[cfg(test)]
mod path_tests {
    #[test]
    fn backup_path_identity_preserves_case_on_case_sensitive_platforms() {
        assert_eq!(
            super::path_identity("mods/A.zip_previous.zip.old")
                == super::path_identity("mods/a.zip_previous.zip.old"),
            cfg!(windows)
        );
    }
}
fn db(error: sqlx::Error) -> ResourceError {
    ResourceError::Database(error.to_string())
}

#[derive(Serialize, Deserialize)]
pub(super) struct CleanupDraft {
    #[serde(flatten)]
    pub(super) plan: ResourceHistoryCleanupPlan,
    #[serde(default)]
    pub(super) moves: Vec<String>,
    #[serde(default)]
    pub(super) committed: bool,
}

async fn recover(draft: &CleanupDraft, root: &Path) -> ResourceResult<()> {
    for (file, temporary) in draft.plan.files.iter().zip(&draft.moves) {
        let temporary = super::execution::checked_path(root, temporary)?;
        if !temporary.exists() {
            continue;
        }
        if !super::execution::matches_file(&temporary, &file.integrity).await? {
            return Err(invalid("cleanup recovery file changed"));
        }
        if draft.committed {
            tokio::fs::remove_file(temporary)
                .await
                .map_err(|error| invalid(error.to_string()))?;
        } else {
            let original =
                super::execution::checked_path(root, &file.relative_path)?;
            if original.exists() {
                return Err(invalid(
                    "backup path occupied during cleanup recovery",
                ));
            }
            tokio::fs::rename(temporary, original)
                .await
                .map_err(|error| invalid(error.to_string()))?;
        }
    }
    Ok(())
}

async fn recover_pending(
    instance_id: &str,
    root: &Path,
    state: &crate::State,
) -> ResourceResult<()> {
    let rows: Vec<String> = sqlx::query_scalar("SELECT payload FROM resource_history_cleanup_plans WHERE instance_id = ?").bind(instance_id).fetch_all(&state.pool).await.map_err(db)?;
    for payload in rows {
        let mut draft: CleanupDraft = serde_json::from_str(&payload)?;
        if draft.moves.is_empty() {
            continue;
        }
        idle(instance_id, state).await?;
        recover(&draft, root).await?;
        draft.moves.clear();
        sqlx::query("UPDATE resource_history_cleanup_plans SET payload = ?, consumed = 1 WHERE id = ?").bind(serde_json::to_string(&draft)?).bind(draft.plan.id.as_str()).execute(&state.pool).await.map_err(db)?;
    }
    Ok(())
}

async fn root(
    instance_id: &str,
    state: &crate::State,
) -> ResourceResult<PathBuf> {
    let scope = crate::state::instances::commands::resolve_content_scope(
        instance_id,
        None,
        state,
    )
    .await?;
    if scope.instance.is_direct_linked()
        || scope.instance.symlink_target.is_some()
    {
        return Err(invalid(
            "old cleanup is only available for Axolotl-owned instances",
        ));
    }
    let root = crate::state::instances::commands::instance_content_root(
        &state.directories,
        &scope.instance,
    )?;
    let owned = crate::util::io::canonicalize(
        state.directories.instances_dir().join(&scope.instance.path),
    )
    .map_err(|error| invalid(error.to_string()))?;
    let instances =
        crate::util::io::canonicalize(state.directories.instances_dir())
            .map_err(|error| invalid(error.to_string()))?;
    if !root.starts_with(&owned) || !owned.starts_with(&instances) {
        return Err(invalid(
            "old cleanup cannot delete files in an external game directory",
        ));
    }
    Ok(root)
}

async fn idle(instance_id: &str, state: &crate::State) -> ResourceResult<()> {
    for job in crate::install::store::list(false, state).await? {
        let snapshot = job.snapshot();
        if matches!(
            job.status,
            crate::install::InstallJobStatus::Queued
                | crate::install::InstallJobStatus::Running
                | crate::install::InstallJobStatus::Canceling
                | crate::install::InstallJobStatus::WaitingForUser
        ) && (snapshot.instance_id.as_deref() == Some(instance_id)
            || snapshot.source_instance_id.as_deref() == Some(instance_id))
        {
            return Err(invalid(
                "wait for the active instance task before cleaning backups",
            ));
        }
    }
    let journal: i64 = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM resource_execution_journals journal JOIN resource_operation_plans plan ON plan.id = journal.plan_id WHERE plan.instance_id = ?)").bind(instance_id).fetch_one(&state.pool).await.map_err(db)?;
    if journal != 0 {
        return Err(invalid(
            "resource recovery must complete before cleaning backups",
        ));
    }
    Ok(())
}

pub(crate) fn recognizable(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    if let Some(value) = name
        .strip_prefix(".resource-")
        .and_then(|name| name.strip_suffix(".old"))
    {
        return value.rsplit_once('-').is_some_and(|(id, index)| {
            uuid::Uuid::parse_str(id).is_ok() && index.parse::<usize>().is_ok()
        });
    }
    let Some(name) = name.strip_suffix(".old") else {
        return false;
    };
    [".jar_", ".zip_"].iter().any(|separator| {
        name.split_once(separator).is_some_and(|(current, old)| {
            !current.is_empty()
                && (old.ends_with(".jar") || old.ends_with(".zip"))
        })
    })
}

async fn file_integrity(path: &Path) -> ResourceResult<ArtifactIntegrity> {
    let (size, sha1) = crate::util::fetch::sha1_file_async(path).await?;
    Ok(ArtifactIntegrity {
        size: Some(size),
        hashes: vec![ContentHash {
            algorithm: HashAlgorithm::Sha1,
            value: sha1,
        }],
    })
}

pub async fn scan(
    instance_id: String,
) -> ResourceResult<Vec<ResourceBackupFile>> {
    let state = crate::State::get().await?;
    let _lock = state.lock_instance_content_exclusive(&instance_id).await;
    scan_with_state(&instance_id, &state).await
}

pub(crate) async fn scan_with_state(
    instance_id: &str,
    state: &crate::State,
) -> ResourceResult<Vec<ResourceBackupFile>> {
    let root = root(instance_id, state).await?;
    recover_pending(instance_id, &root, state).await?;
    let histories =
        super::history::list_with_state(instance_id, None, state).await?;
    let mut result = Vec::new();
    let mut known = HashSet::new();
    for history in histories {
        let Some(relative) = history.backup_relative_path else {
            continue;
        };
        if !plans::safe_relative_path(&relative) {
            continue;
        }
        let path = super::execution::checked_path(&root, &relative)?;
        let exists = path.is_file();
        let integrity = if exists {
            file_integrity(&path).await?
        } else {
            ArtifactIntegrity {
                size: None,
                hashes: vec![],
            }
        };
        known.insert(relative.clone());
        result.push(ResourceBackupFile {
            original_file_name: history
                .previous
                .original_file_name
                .clone()
                .or_else(|| {
                    history
                        .previous
                        .relative_path
                        .rsplit('/')
                        .next()
                        .map(str::to_owned)
                }),
            id: history.id.clone(),
            history_id: Some(history.id),
            relative_path: relative,
            integrity,
            rollback_available: history.backup_available,
            recognized: true,
            exists,
        });
    }
    let mut pending = vec![root.clone()];
    while let Some(directory) = pending.pop() {
        let mut reader = tokio::fs::read_dir(&directory)
            .await
            .map_err(|error| invalid(error.to_string()))?;
        while let Some(entry) = reader
            .next_entry()
            .await
            .map_err(|error| invalid(error.to_string()))?
        {
            let metadata = tokio::fs::symlink_metadata(entry.path())
                .await
                .map_err(|error| invalid(error.to_string()))?;
            if super::export::scan::linked(&metadata) {
                continue;
            }
            let kind = metadata.file_type();
            let path = entry.path();
            let relative = path
                .strip_prefix(&root)
                .map_err(|error| invalid(error.to_string()))?
                .to_string_lossy()
                .replace('\\', "/");
            if kind.is_dir() {
                if !super::export::categories::forbidden(&format!(
                    "{relative}/"
                )) {
                    pending.push(path);
                }
            } else if kind.is_file()
                && relative.ends_with(".old")
                && !known.contains(&relative)
            {
                let path = super::execution::checked_path(&root, &relative)?;
                let id = HistoryId::new(format!(
                    "old:{}",
                    sha1_smol::Sha1::from(relative.as_bytes()).hexdigest()
                ))?;
                result.push(ResourceBackupFile {
                    original_file_name: None,
                    id,
                    history_id: None,
                    recognized: recognizable(&relative),
                    relative_path: relative,
                    integrity: file_integrity(&path).await?,
                    rollback_available: false,
                    exists: true,
                });
            }
        }
    }
    result.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(result)
}

pub async fn prepare(
    instance_id: String,
    ids: Vec<HistoryId>,
) -> ResourceResult<ResourceHistoryCleanupPlan> {
    let state = crate::State::get().await?;
    let _lock = state.lock_instance_content_exclusive(&instance_id).await;
    prepare_with_state(&instance_id, ids, &state).await
}

pub(crate) async fn prepare_with_state(
    instance_id: &str,
    ids: Vec<HistoryId>,
    state: &crate::State,
) -> ResourceResult<ResourceHistoryCleanupPlan> {
    idle(instance_id, state).await?;
    let selected: HashSet<_> = ids.iter().collect();
    if ids.is_empty() || ids.len() != selected.len() {
        return Err(invalid("select distinct backup files"));
    }
    let mut files: Vec<_> = scan_with_state(instance_id, state)
        .await?
        .into_iter()
        .filter(|file| selected.contains(&file.id))
        .collect();
    if files.len() != ids.len()
        || files.iter().any(|file| !file.recognized || !file.exists)
    {
        return Err(invalid(
            "selected backups are missing or have no verified backup provenance",
        ));
    }
    let entries = super::history::list_with_state(instance_id, None, state)
        .await?
        .into_iter()
        .filter(|entry| selected.contains(&entry.id))
        .collect();
    let mut paths = HashSet::new();
    files.retain(|file| paths.insert(path_identity(&file.relative_path)));
    let plan = ResourceHistoryCleanupPlan {
        id: PlanId::new(uuid::Uuid::new_v4().to_string())?,
        revision: 1,
        instance_id: instance_id.into(),
        instance_revision: plans::instance_revision(&state.pool, instance_id)
            .await?,
        entries,
        files,
        issues: vec![],
    };
    validate(&plan, state).await?;
    sqlx::query("INSERT INTO resource_history_cleanup_plans(id, instance_id, revision, payload) VALUES (?, ?, ?, ?)").bind(plan.id.as_str()).bind(instance_id).bind(plan.revision).bind(serde_json::to_string(&plan)?).execute(&state.pool).await.map_err(db)?;
    Ok(plan)
}

async fn validate(
    plan: &ResourceHistoryCleanupPlan,
    state: &crate::State,
) -> ResourceResult<()> {
    idle(&plan.instance_id, state).await?;
    if plan.instance_revision
        != plans::instance_revision(&state.pool, &plan.instance_id).await?
    {
        return Err(invalid("instance changed since cleanup review"));
    }
    let root = root(&plan.instance_id, state).await?;
    let selected: HashSet<_> =
        plan.entries.iter().map(|entry| &entry.id).collect();
    let rows = sqlx::query("SELECT id, backup_relative_path FROM resource_history WHERE instance_id = ? AND backup_available = 1").bind(&plan.instance_id).fetch_all(&state.pool).await.map_err(db)?;
    let mut retained_paths = HashSet::new();
    for row in rows {
        let id = HistoryId::new(row.get::<String, _>("id"))?;
        if !selected.contains(&id)
            && let Some(path) =
                row.get::<Option<String>, _>("backup_relative_path")
        {
            retained_paths.insert(path_identity(&path));
        }
    }
    let current_paths: Vec<String> = sqlx::query_scalar("SELECT file.relative_path FROM instance_files file JOIN instance_content_entries entry ON entry.file_id = file.id WHERE file.instance_id = ? UNION ALL SELECT relative_path FROM store_instance_files WHERE instance_id = ?").bind(&plan.instance_id).bind(&plan.instance_id).fetch_all(&state.pool).await.map_err(db)?;
    let current_paths: HashSet<_> = current_paths
        .into_iter()
        .map(|path| path_identity(&path))
        .collect();
    for file in &plan.files {
        let path = super::execution::checked_path(&root, &file.relative_path)?;
        if !file.recognized
            || (file.history_id.is_none() && !recognizable(&file.relative_path))
            || !super::execution::matches_file(&path, &file.integrity).await?
        {
            return Err(invalid("backup changed since cleanup review"));
        }
        let path_key = path_identity(&file.relative_path);
        if current_paths.contains(&path_key) {
            return Err(invalid(
                "backup is still referenced as current instance content",
            ));
        }
        if retained_paths.contains(&path_key) {
            return Err(invalid(
                "backup is still required by another history entry",
            ));
        }
    }
    Ok(())
}

pub async fn execute(
    id: PlanId,
    revision: u32,
) -> ResourceResult<ResourceHistoryCleanupResult> {
    let state = crate::State::get().await?;
    execute_with_state(id, revision, &state).await
}

pub(crate) async fn execute_with_state(
    id: PlanId,
    revision: u32,
    state: &crate::State,
) -> ResourceResult<ResourceHistoryCleanupResult> {
    let _writer = state.acquire_install_db_permit().await?;
    let payload: String = sqlx::query_scalar("SELECT payload FROM resource_history_cleanup_plans WHERE id = ? AND revision = ? AND consumed = 0").bind(id.as_str()).bind(revision).fetch_optional(&state.pool).await.map_err(db)?.ok_or_else(|| invalid("cleanup plan expired or already executed"))?;
    let mut draft: CleanupDraft = serde_json::from_str(&payload)?;
    let plan = draft.plan.clone();
    let _lock = state
        .lock_instance_content_exclusive(&plan.instance_id)
        .await;
    let consumed: i64 = sqlx::query_scalar(
        "SELECT consumed FROM resource_history_cleanup_plans WHERE id = ?",
    )
    .bind(id.as_str())
    .fetch_one(&state.pool)
    .await
    .map_err(db)?;
    if consumed != 0 {
        return Err(invalid("cleanup plan already executed"));
    }
    let root = root(&plan.instance_id, state).await?;
    if !draft.moves.is_empty() {
        recover(&draft, &root).await?;
        return Err(invalid(
            "interrupted cleanup recovered; scan backups again",
        ));
    }
    validate(&plan, state).await?;
    for (index, file) in plan.files.iter().enumerate() {
        let parent = file
            .relative_path
            .rsplit_once('/')
            .map(|(parent, _)| format!("{parent}/"))
            .unwrap_or_default();
        let temporary = format!(
            "{parent}.resource-cleanup-{}-{index}.old",
            plan.id.as_str()
        );
        if super::execution::checked_path(&root, &temporary)?.exists() {
            return Err(invalid("cleanup staging path occupied"));
        }
        draft.moves.push(temporary);
    }
    sqlx::query("UPDATE resource_history_cleanup_plans SET payload = ? WHERE id = ? AND consumed = 0").bind(serde_json::to_string(&draft)?).bind(id.as_str()).execute(&state.pool).await.map_err(db)?;
    let commit = async {
        for (file, temporary) in plan.files.iter().zip(&draft.moves) {
            let path = super::execution::checked_path(&root, &file.relative_path)?;
            if !super::execution::matches_file(&path, &file.integrity).await? { return Err(invalid("backup changed while cleaning")); }
            tokio::fs::rename(path, super::execution::checked_path(&root, temporary)?).await.map_err(|error| invalid(error.to_string()))?;
        }
        let mut tx = state.pool.begin().await.map_err(db)?;
        for entry in &plan.entries {
            let history_id = &entry.id;
                sqlx::query("UPDATE resource_history SET backup_available = 0 WHERE id = ? AND instance_id = ?").bind(history_id.as_str()).bind(&plan.instance_id).execute(&mut *tx).await.map_err(db)?;
                sqlx::query("DELETE FROM store_retained_refs WHERE reason = ?").bind(format!("resource-history:{}", history_id.as_str())).execute(&mut *tx).await.map_err(db)?;
        }
        draft.committed = true;
        sqlx::query("UPDATE resource_history_cleanup_plans SET payload = ?, consumed = 1 WHERE id = ?").bind(serde_json::to_string(&draft)?).bind(id.as_str()).execute(&mut *tx).await.map_err(db)?;
        tx.commit().await.map_err(db)?;
        Ok::<_, ResourceError>(())
    }.await;
    if let Err(error) = commit {
        draft.committed = false;
        recover(&draft, &root).await?;
        draft.moves.clear();
        sqlx::query("UPDATE resource_history_cleanup_plans SET payload = ?, consumed = 1 WHERE id = ?").bind(serde_json::to_string(&draft)?).bind(id.as_str()).execute(&state.pool).await.map_err(db)?;
        return Err(error);
    }
    let mut result = ResourceHistoryCleanupResult {
        removed: vec![],
        failures: vec![],
    };
    for (file, temporary) in plan.files.iter().zip(&draft.moves) {
        let path = super::execution::checked_path(&root, temporary)?;
        let outcome = async {
            if !super::execution::matches_file(&path, &file.integrity).await? {
                return Err(invalid("backup changed while cleaning"));
            }
            tokio::fs::remove_file(&path)
                .await
                .map_err(|error| invalid(error.to_string()))?;
            Ok::<_, ResourceError>(())
        }
        .await;
        match outcome {
            Ok(()) => {
                result.removed.push(file.id.clone());
                for entry in &plan.entries {
                    if entry.backup_relative_path.as_ref().is_some_and(|path| {
                        path.eq_ignore_ascii_case(&file.relative_path)
                    }) && entry.id != file.id
                    {
                        result.removed.push(entry.id.clone());
                    }
                }
            }
            Err(error) => {
                result.failures.push((file.id.clone(), error.to_string()))
            }
        }
    }
    if result.failures.is_empty() {
        draft.moves.clear();
        sqlx::query("UPDATE resource_history_cleanup_plans SET payload = ? WHERE id = ?").bind(serde_json::to_string(&draft)?).bind(id.as_str()).execute(&state.pool).await.map_err(db)?;
    }
    Ok(result)
}

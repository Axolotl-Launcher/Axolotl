use crate::event::{
    InstanceBackupOperationFinalState, InstanceBackupOperationType,
    InstanceBackupProgressPayload, InstanceBackupProgressStage,
};
use crate::state::instances::adapters::sqlite::instance_rows;
use crate::state::{Settings, State};
use crate::util::io;
use chrono::Utc;
use dashmap::DashMap;
use dashmap::mapref::entry::Entry;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::migrate::Migrator;
use sqlx::sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions,
};
use sqlx::{Row, SqlitePool};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Component, Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock};
use std::time::{Duration, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

static BACKUP_MIGRATOR: Migrator = sqlx::migrate!("./backup_migrations");
static REPOSITORY_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
static INSTANCE_MAINTENANCE_LOCKS: LazyLock<DashMap<String, Arc<Mutex<()>>>> =
    LazyLock::new(DashMap::new);
struct BackupCancellation {
    token: CancellationToken,
    cancellable: Arc<AtomicBool>,
}

static BACKUP_CANCELLATIONS: LazyLock<DashMap<Uuid, BackupCancellation>> =
    LazyLock::new(DashMap::new);
static ACTIVE_INSTANCE_OPERATIONS: LazyLock<DashMap<String, Uuid>> =
    LazyLock::new(DashMap::new);

const DATABASE_FILE: &str = "backup.db";
const OBJECTS_DIR: &str = "objects";
const STAGING_DIR: &str = ".staging";
const RESTORE_PREFIX: &str = ".backup-restore-";
const RESTORE_JOURNAL: &str = "restore.json";
const RESTORE_COMMITTED: &str = "committed";
const REPOSITORY_OPERATION_KEY: &str = "__backup_repository__";

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstanceBackupEligibility {
    Eligible,
    NotInstalled,
    SymlinkInstance,
    DirectLinkedInstance,
    ExternalGameDirectory,
    RootMissing,
    RootIsLink,
}

#[derive(Clone, Debug, Serialize)]
pub struct BackupRepositoryStatus {
    pub path: String,
    pub initialized: bool,
    pub available: bool,
    pub stored_size: u64,
    pub logical_size: u64,
    pub snapshot_count: u64,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct InstanceBackupConfig {
    pub instance_id: String,
    pub enabled: bool,
    pub eligibility: InstanceBackupEligibility,
    pub excluded_paths: Vec<BackupExclusion>,
    pub snapshot_count: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BackupExclusionKind {
    File,
    Directory,
}

impl BackupExclusionKind {
    const fn as_str(&self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Directory => "directory",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct BackupExclusion {
    pub path: String,
    pub kind: BackupExclusionKind,
}

#[derive(Clone, Debug, Serialize)]
pub struct BackupSnapshot {
    pub id: String,
    pub instance_id: String,
    pub instance_name: String,
    pub created_at: i64,
    pub file_count: u64,
    pub symlink_count: u64,
    pub logical_size: u64,
    pub added_size: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct BackupDeleteSummary {
    pub snapshot_count: u64,
    pub logical_size: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct BackupRestorePreview {
    pub added_files: u64,
    pub modified_files: u64,
    pub deleted_files: u64,
    pub plan_token: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BackupOperationType {
    Create,
    Restore,
    RepositoryMove,
    RestorePreview,
}

impl BackupOperationType {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Restore => "restore",
            Self::RepositoryMove => "repository_move",
            Self::RestorePreview => "restore_preview",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BackupOperationState {
    Queued,
    Scanning,
    Hashing,
    Saving,
    Validating,
    Materializing,
    Applying,
    Completed,
    Cancelled,
    Failed,
    Interrupted,
}

impl BackupOperationState {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Scanning => "scanning",
            Self::Hashing => "hashing",
            Self::Saving => "saving",
            Self::Validating => "validating",
            Self::Materializing => "materializing",
            Self::Applying => "applying",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
            Self::Interrupted => "interrupted",
        }
    }

    const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed
                | Self::Cancelled
                | Self::Failed
                | Self::Interrupted
        )
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct BackupOperation {
    pub id: Uuid,
    pub operation_type: BackupOperationType,
    pub instance_id: String,
    pub snapshot_id: Option<String>,
    pub state: BackupOperationState,
    pub processed_bytes: u64,
    pub total_bytes: u64,
    pub cancellable: bool,
    pub cancel_requested: bool,
    pub error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub finished_at: Option<i64>,
    pub restore_preview: Option<BackupRestorePreview>,
}

#[derive(Clone, Debug)]
struct PendingEntry {
    path: String,
    kind: EntryKind,
    object_hash: Option<String>,
    size: u64,
    modified_at: Option<i64>,
    unix_mode: Option<u32>,
    link_target: Option<String>,
    link_is_directory: Option<bool>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EntryKind {
    File,
    Directory,
    Symlink,
}

#[derive(Clone, Debug)]
struct RestoreEntry {
    path: String,
    kind: EntryKind,
    object_hash: Option<String>,
    size: u64,
    unix_mode: Option<i64>,
    link_target: Option<String>,
    link_is_directory: Option<bool>,
}

#[derive(Debug)]
struct RestorePlan {
    preview: BackupRestorePreview,
    roots: Vec<RestoreJournalRoot>,
    target_entries: Vec<RestoreEntry>,
}

impl EntryKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Directory => "directory",
            Self::Symlink => "symlink",
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct RestoreJournal {
    instance_id: String,
    roots: Vec<RestoreJournalRoot>,
    #[serde(default)]
    whole_root: bool,
    #[serde(default)]
    exclusions: Vec<BackupExclusion>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct RestoreJournalRoot {
    path: String,
    had_current: bool,
}

pub(crate) async fn lock_instance_maintenance(
    instance_id: &str,
) -> tokio::sync::OwnedMutexGuard<()> {
    INSTANCE_MAINTENANCE_LOCKS
        .entry(instance_id.to_string())
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone()
        .lock_owned()
        .await
}

fn default_repository_path(state: &State) -> PathBuf {
    state
        .directories
        .config_dir
        .join("Backups")
        .join("instances")
}

async fn repository_path(state: &State) -> crate::Result<PathBuf> {
    let settings = Settings::get(&state.pool).await?;
    Ok(settings
        .backup_repository_path
        .filter(|path| !path.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| default_repository_path(state)))
}

async fn open_repository(
    state: &State,
    create: bool,
) -> crate::Result<Option<(PathBuf, SqlitePool)>> {
    let root = repository_path(state).await?;
    Ok(open_repository_at(&root, create)
        .await?
        .map(|pool| (root, pool)))
}

async fn open_repository_at(
    root: &Path,
    create: bool,
) -> crate::Result<Option<SqlitePool>> {
    let database = root.join(DATABASE_FILE);
    if !database.try_exists()? && !create {
        return Ok(None);
    }
    if create {
        io::create_dir_all(root.join(OBJECTS_DIR)).await?;
        io::create_dir_all(root.join(STAGING_DIR)).await?;
    }
    let options = SqliteConnectOptions::new()
        .filename(&database)
        .create_if_missing(create)
        .journal_mode(SqliteJournalMode::Wal)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(options)
        .await?;
    BACKUP_MIGRATOR.run(&pool).await?;
    Ok(Some(pool))
}

fn parse_backup_operation_type(
    value: &str,
) -> crate::Result<BackupOperationType> {
    match value {
        "create" => Ok(BackupOperationType::Create),
        "restore" => Ok(BackupOperationType::Restore),
        "repository_move" => Ok(BackupOperationType::RepositoryMove),
        "restore_preview" => Ok(BackupOperationType::RestorePreview),
        _ => Err(crate::ErrorKind::FSError(format!(
            "Invalid backup operation type: {value}"
        ))
        .into()),
    }
}

fn parse_backup_operation_state(
    value: &str,
) -> crate::Result<BackupOperationState> {
    match value {
        "queued" => Ok(BackupOperationState::Queued),
        "scanning" => Ok(BackupOperationState::Scanning),
        "hashing" => Ok(BackupOperationState::Hashing),
        "saving" => Ok(BackupOperationState::Saving),
        "validating" => Ok(BackupOperationState::Validating),
        "materializing" => Ok(BackupOperationState::Materializing),
        "applying" => Ok(BackupOperationState::Applying),
        "completed" => Ok(BackupOperationState::Completed),
        "cancelled" => Ok(BackupOperationState::Cancelled),
        "failed" => Ok(BackupOperationState::Failed),
        "interrupted" => Ok(BackupOperationState::Interrupted),
        _ => Err(crate::ErrorKind::FSError(format!(
            "Invalid backup operation state: {value}"
        ))
        .into()),
    }
}

fn backup_operation_from_row(
    row: &sqlx::sqlite::SqliteRow,
) -> crate::Result<BackupOperation> {
    Ok(BackupOperation {
        id: Uuid::parse_str(row.get::<String, _>("id").as_str())
            .map_err(|error| crate::ErrorKind::FSError(error.to_string()))?,
        operation_type: parse_backup_operation_type(
            row.get::<String, _>("operation_type").as_str(),
        )?,
        instance_id: row.get("instance_id"),
        snapshot_id: row.get("snapshot_id"),
        state: parse_backup_operation_state(
            row.get::<String, _>("state").as_str(),
        )?,
        processed_bytes: row.get::<i64, _>("processed_bytes").max(0) as u64,
        total_bytes: row.get::<i64, _>("total_bytes").max(0) as u64,
        cancellable: row.get::<i64, _>("cancellable") != 0,
        cancel_requested: row.get::<i64, _>("cancel_requested") != 0,
        error: row.get("error"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        finished_at: row.get("finished_at"),
        restore_preview: row
            .try_get::<Option<i64>, _>("preview_added_files")?
            .map(|added_files| BackupRestorePreview {
                added_files: added_files.max(0) as u64,
                modified_files: row
                    .get::<Option<i64>, _>("preview_modified_files")
                    .unwrap_or(0)
                    .max(0) as u64,
                deleted_files: row
                    .get::<Option<i64>, _>("preview_deleted_files")
                    .unwrap_or(0)
                    .max(0) as u64,
                plan_token: row
                    .get::<Option<String>, _>("preview_plan_token")
                    .unwrap_or_default(),
            }),
    })
}

async fn create_persisted_operation(
    id: Uuid,
    instance_id: &str,
    operation_type: BackupOperationType,
    snapshot_id: Option<&str>,
) -> crate::Result<()> {
    let state = State::get().await?;
    let Some((_, pool)) = open_repository(&state, true).await? else {
        return Err(crate::ErrorKind::FSError(
            "Unable to open backup repository".to_string(),
        )
        .into());
    };
    if let Err(error) = reserve_instance_operation(instance_id, id) {
        pool.close().await;
        return Err(error);
    }
    let now = Utc::now().timestamp_millis();
    let persist_result: crate::Result<()> = async {
		let active_count: i64 = sqlx::query_scalar(
			"SELECT COUNT(*) FROM backup_operations WHERE instance_id = ?
			 AND state NOT IN ('completed', 'cancelled', 'failed', 'interrupted')",
		)
		.bind(instance_id)
		.fetch_one(&pool)
		.await?;
		if active_count > 0 {
			return Err(crate::ErrorKind::InputError(
				"Another backup operation is already running for this instance".to_string(),
			)
			.into());
		}
		sqlx::query(
			"INSERT INTO backup_operations
			 (id, operation_type, instance_id, snapshot_id, state, created_at, updated_at)
			 VALUES (?, ?, ?, ?, 'queued', ?, ?)",
		)
		.bind(id.to_string())
		.bind(operation_type.as_str())
		.bind(instance_id)
		.bind(snapshot_id)
		.bind(now)
		.bind(now)
		.execute(&pool)
		.await?;
		Ok(())
	}.await;
    pool.close().await;
    if persist_result.is_err() {
        ACTIVE_INSTANCE_OPERATIONS.remove(instance_id);
    }
    persist_result
}

fn reserve_instance_operation(
    instance_id: &str,
    id: Uuid,
) -> crate::Result<()> {
    match ACTIVE_INSTANCE_OPERATIONS.entry(instance_id.to_string()) {
        Entry::Occupied(_) => {
            return Err(crate::ErrorKind::InputError(
                "Another backup operation is already running for this instance"
                    .to_string(),
            )
            .into());
        }
        Entry::Vacant(entry) => {
            entry.insert(id);
        }
    }
    Ok(())
}

async fn update_persisted_operation(
    operation_id: Uuid,
    state: BackupOperationState,
    processed_bytes: Option<u64>,
    total_bytes: Option<u64>,
    cancellable: Option<bool>,
    error: Option<&str>,
    snapshot_id: Option<&str>,
) {
    if !State::initialized() {
        return;
    }
    let Ok(state_handle) = State::get().await else {
        return;
    };
    let Ok(Some((_, pool))) = open_repository(&state_handle, false).await
    else {
        return;
    };
    let now = Utc::now().timestamp_millis();
    let finished_at = state.is_terminal().then_some(now);
    let _ = sqlx::query(
		"UPDATE backup_operations SET state = ?, processed_bytes = COALESCE(?, processed_bytes),
		 total_bytes = COALESCE(?, total_bytes), cancellable = COALESCE(?, cancellable),
		 error = COALESCE(?, error), snapshot_id = COALESCE(?, snapshot_id),
		 updated_at = ?, finished_at = COALESCE(?, finished_at) WHERE id = ?",
	)
	.bind(state.as_str())
	.bind(processed_bytes.map(|value| value as i64))
	.bind(total_bytes.map(|value| value as i64))
	.bind(cancellable.map(i64::from))
	.bind(error)
	.bind(snapshot_id)
	.bind(now)
	.bind(finished_at)
	.bind(operation_id.to_string())
	.execute(&pool)
	.await;
    pool.close().await;
}

async fn store_restore_preview(
    operation_id: Uuid,
    preview: &BackupRestorePreview,
) {
    if !State::initialized() {
        return;
    }
    let Ok(state) = State::get().await else {
        return;
    };
    let Ok(Some((_, pool))) = open_repository(&state, false).await else {
        return;
    };
    let _ = sqlx::query(
        "UPDATE backup_operations SET preview_added_files = ?,
         preview_modified_files = ?, preview_deleted_files = ?,
         preview_plan_token = ? WHERE id = ?",
    )
    .bind(preview.added_files as i64)
    .bind(preview.modified_files as i64)
    .bind(preview.deleted_files as i64)
    .bind(&preview.plan_token)
    .bind(operation_id.to_string())
    .execute(&pool)
    .await;
    pool.close().await;
}

pub async fn list_operations(
    instance_id: Option<&str>,
    active_only: bool,
) -> crate::Result<Vec<BackupOperation>> {
    let state = State::get().await?;
    let Some((_, pool)) = open_repository(&state, false).await? else {
        return Ok(Vec::new());
    };
    let mut query = String::from(
        "SELECT id, operation_type, instance_id, snapshot_id, state,
		 processed_bytes, total_bytes, cancellable, cancel_requested, error,
		 created_at, updated_at, finished_at, preview_added_files,
		 preview_modified_files, preview_deleted_files, preview_plan_token
		 FROM backup_operations WHERE 1 = 1",
    );
    if instance_id.is_some() {
        query.push_str(" AND instance_id = ?");
    }
    if active_only {
        query.push_str(" AND state NOT IN ('completed', 'cancelled', 'failed', 'interrupted')");
    }
    query.push_str(" ORDER BY created_at DESC");
    let mut request = sqlx::query(&query);
    if let Some(instance_id) = instance_id {
        request = request.bind(instance_id);
    }
    let rows = request.fetch_all(&pool).await?;
    pool.close().await;
    rows.iter().map(backup_operation_from_row).collect()
}

pub async fn has_active_operations() -> crate::Result<bool> {
    if !ACTIVE_INSTANCE_OPERATIONS.is_empty() {
        return Ok(true);
    }
    Ok(!list_operations(None, true).await?.is_empty())
}

pub async fn has_active_repository_move() -> crate::Result<bool> {
    if ACTIVE_INSTANCE_OPERATIONS.contains_key(REPOSITORY_OPERATION_KEY) {
        return Ok(true);
    }
    Ok(list_operations(None, true).await?.iter().any(|operation| {
        operation.operation_type == BackupOperationType::RepositoryMove
    }))
}

pub async fn interrupt_active_operations() -> crate::Result<u64> {
    let state = State::get().await?;
    let Some((_, pool)) = open_repository(&state, false).await? else {
        return Ok(0);
    };
    let now = Utc::now().timestamp_millis();
    let result = sqlx::query(
        "UPDATE backup_operations SET state = 'interrupted', cancellable = 0,
		 error = COALESCE(error, 'The launcher exited while the operation was running'),
		 updated_at = ?, finished_at = ?
		 WHERE state NOT IN ('completed', 'cancelled', 'failed', 'interrupted')",
    )
    .bind(now)
    .bind(now)
    .execute(&pool)
    .await?;
    pool.close().await;
    Ok(result.rows_affected())
}

fn normalize_relative_directory(path: &str) -> crate::Result<String> {
    let normalized = path.trim().replace('\\', "/");
    if normalized.is_empty() || normalized == "." {
        return Err(crate::ErrorKind::InputError(
            "Backup directory must be a non-empty relative path".to_string(),
        )
        .into());
    }
    let path = Path::new(&normalized);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir
                    | Component::RootDir
                    | Component::Prefix(_)
            )
        })
    {
        return Err(crate::ErrorKind::InputError(format!(
            "Backup directory must stay inside the instance: {normalized}"
        ))
        .into());
    }
    let parts: Vec<_> = normalized
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect();
    if parts.is_empty() {
        return Err(crate::ErrorKind::InputError(
            "Backup directory must not resolve to the instance root"
                .to_string(),
        )
        .into());
    }
    Ok(parts.join("/"))
}

fn canonicalize_exclusions(
    exclusions: Vec<BackupExclusion>,
) -> crate::Result<Vec<BackupExclusion>> {
    let mut normalized = exclusions
        .into_iter()
        .map(|exclusion| {
            Ok(BackupExclusion {
                path: normalize_relative_directory(&exclusion.path)?,
                kind: exclusion.kind,
            })
        })
        .collect::<crate::Result<Vec<_>>>()?;
    normalized.sort_by(|left, right| {
        left.path
            .matches('/')
            .count()
            .cmp(&right.path.matches('/').count())
            .then_with(|| left.path.cmp(&right.path))
    });
    let mut result: Vec<BackupExclusion> = Vec::new();
    for exclusion in normalized {
        if let Some(existing) = result
            .iter_mut()
            .find(|existing| existing.path == exclusion.path)
        {
            if exclusion.kind == BackupExclusionKind::Directory {
                existing.kind = BackupExclusionKind::Directory;
            }
            continue;
        }
        if result.iter().any(|parent| {
            parent.kind == BackupExclusionKind::Directory
                && exclusion.path.starts_with(&format!("{}/", parent.path))
        }) {
            continue;
        }
        result.push(exclusion);
    }
    Ok(result)
}

fn ensure_canonical_exclusions(
    exclusions: &[BackupExclusion],
) -> crate::Result<()> {
    let canonical = canonicalize_exclusions(exclusions.to_vec())?;
    if canonical.len() != exclusions.len()
        || exclusions
            .iter()
            .any(|exclusion| !canonical.contains(exclusion))
    {
        return Err(crate::ErrorKind::FSError(
            "Invalid backup exclusion paths".to_string(),
        )
        .into());
    }
    Ok(())
}

async fn instance_and_root(
    instance_id: &str,
    state: &State,
) -> crate::Result<(crate::state::Instance, PathBuf)> {
    let instance = instance_rows::get_instance_by_id(instance_id, &state.pool)
        .await?
        .ok_or_else(|| {
            crate::ErrorKind::InputError("Unknown instance".to_string())
        })?;
    let root = state.directories.instances_dir().join(&instance.path);
    Ok((instance, root))
}

async fn eligibility_for(
    instance_id: &str,
    state: &State,
) -> crate::Result<InstanceBackupEligibility> {
    let (instance, root) = instance_and_root(instance_id, state).await?;
    if instance.install_stage != crate::state::InstanceInstallStage::Installed {
        return Ok(InstanceBackupEligibility::NotInstalled);
    }
    if instance.symlink_target.is_some() {
        return Ok(InstanceBackupEligibility::SymlinkInstance);
    }
    if instance.is_direct_linked() {
        return Ok(InstanceBackupEligibility::DirectLinkedInstance);
    }
    if instance
        .game_dir_override
        .as_deref()
        .is_some_and(|path| !path.trim().is_empty())
    {
        return Ok(InstanceBackupEligibility::ExternalGameDirectory);
    }
    let metadata = match tokio::fs::symlink_metadata(&root).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(InstanceBackupEligibility::RootMissing);
        }
        Err(error) => return Err(error.into()),
    };
    if io::is_symlink_or_reparse(&metadata) {
        return Ok(InstanceBackupEligibility::RootIsLink);
    }
    Ok(InstanceBackupEligibility::Eligible)
}

async fn require_eligible(
    instance_id: &str,
    state: &State,
) -> crate::Result<PathBuf> {
    let eligibility = eligibility_for(instance_id, state).await?;
    if !matches!(eligibility, InstanceBackupEligibility::Eligible) {
        return Err(crate::ErrorKind::InputError(format!(
            "Instance is not eligible for backups: {eligibility:?}"
        ))
        .into());
    }
    Ok(instance_and_root(instance_id, state).await?.1)
}

async fn validate_exclusion_paths(
    root: &Path,
    exclusions: Vec<BackupExclusion>,
) -> crate::Result<Vec<BackupExclusion>> {
    let exclusions = canonicalize_exclusions(exclusions)?;
    validate_exclusion_ancestors(root, &exclusions).await?;
    for exclusion in &exclusions {
        let absolute = join_relative(root, &exclusion.path);
        match tokio::fs::symlink_metadata(&absolute).await {
            Ok(metadata) if io::is_symlink_or_reparse(&metadata) => {
                if let Ok(target) = tokio::fs::metadata(&absolute).await {
                    let kind_matches = match exclusion.kind {
                        BackupExclusionKind::File => target.is_file(),
                        BackupExclusionKind::Directory => target.is_dir(),
                    };
                    if !kind_matches {
                        return Err(crate::ErrorKind::InputError(format!(
                            "Backup exclusion type does not match: {}",
                            exclusion.path
                        ))
                        .into());
                    }
                }
            }
            Ok(metadata)
                if exclusion.kind == BackupExclusionKind::Directory
                    && !metadata.is_dir()
                    && !io::is_symlink_or_reparse(&metadata) =>
            {
                return Err(crate::ErrorKind::InputError(format!(
                    "Backup exclusion is not a directory: {}",
                    exclusion.path
                ))
                .into());
            }
            Ok(metadata)
                if exclusion.kind == BackupExclusionKind::File
                    && metadata.is_dir()
                    && !io::is_symlink_or_reparse(&metadata) =>
            {
                return Err(crate::ErrorKind::InputError(format!(
                    "Backup exclusion is not a file: {}",
                    exclusion.path
                ))
                .into());
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(exclusions)
}

pub async fn exclusion_from_absolute_path(
    instance_id: &str,
    selected_path: PathBuf,
    kind: BackupExclusionKind,
) -> crate::Result<BackupExclusion> {
    if !selected_path.is_absolute() {
        return Err(crate::ErrorKind::InputError(
            "Backup exclusion must be selected by absolute path".to_string(),
        )
        .into());
    }
    let state = State::get().await?;
    let root = require_eligible(instance_id, &state).await?;
    let canonical_root = io::canonicalize(&root)?;
    let name = selected_path.file_name().ok_or_else(|| {
        crate::ErrorKind::InputError(
            "The instance root cannot be excluded from its backup".to_string(),
        )
    })?;
    let parent = selected_path.parent().ok_or_else(|| {
        crate::ErrorKind::InputError(
            "Backup exclusion has no parent directory".to_string(),
        )
    })?;
    let canonical_parent = io::canonicalize(parent)?;
    let relative_parent = canonical_parent
        .strip_prefix(&canonical_root)
        .map_err(|_| {
            crate::ErrorKind::InputError(
                "Backup exclusions must stay inside the instance".to_string(),
            )
        })?;
    let relative = relative_parent.join(name);
    let relative = relative
        .to_str()
        .ok_or_else(|| crate::ErrorKind::UTFError(relative.clone()))?
        .replace('\\', "/");
    let mut exclusions = validate_exclusion_paths(
        &root,
        vec![BackupExclusion {
            path: relative,
            kind,
        }],
    )
    .await?;
    Ok(exclusions.remove(0))
}

async fn validate_exclusion_ancestors(
    root: &Path,
    exclusions: &[BackupExclusion],
) -> crate::Result<()> {
    ensure_canonical_exclusions(exclusions)?;
    for exclusion in exclusions {
        let components: Vec<_> = exclusion.path.split('/').collect();
        let mut current = root.to_path_buf();
        for component in components.iter().take(components.len() - 1) {
            current.push(component);
            match tokio::fs::symlink_metadata(&current).await {
                Ok(metadata) if io::is_symlink_or_reparse(&metadata) => {
                    return Err(crate::ErrorKind::InputError(format!(
                        "Backup exclusion has a linked parent: {}",
                        exclusion.path
                    ))
                    .into());
                }
                Ok(metadata) if !metadata.is_dir() => {
                    return Err(crate::ErrorKind::InputError(format!(
                        "Backup exclusion parent is not a directory: {}",
                        exclusion.path
                    ))
                    .into());
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    break;
                }
                Err(error) => return Err(error.into()),
            }
        }
    }
    Ok(())
}

fn join_relative(root: &Path, relative: &str) -> PathBuf {
    relative
        .split('/')
        .fold(root.to_path_buf(), |path, part| path.join(part))
}

pub async fn repository_status() -> crate::Result<BackupRepositoryStatus> {
    let state = State::get().await?;
    let path = repository_path(&state).await?;
    let path_string = path.to_string_lossy().to_string();
    let repository = match open_repository(&state, false).await {
        Ok(repository) => repository,
        Err(error) => {
            return Ok(BackupRepositoryStatus {
                path: path_string,
                initialized: path.join(DATABASE_FILE).try_exists()?,
                available: false,
                stored_size: 0,
                logical_size: 0,
                snapshot_count: 0,
                error: Some(error.to_string()),
            });
        }
    };
    let Some((_, pool)) = repository else {
        return Ok(BackupRepositoryStatus {
            path: path_string,
            initialized: false,
            available: true,
            stored_size: 0,
            logical_size: 0,
            snapshot_count: 0,
            error: None,
        });
    };
    let snapshot_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM backup_snapshots")
            .fetch_one(&pool)
            .await?;
    let logical_size: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(logical_size), 0) FROM backup_snapshots",
    )
    .fetch_one(&pool)
    .await?;
    let stored_size: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(size), 0) FROM backup_objects WHERE ref_count > 0",
    )
    .fetch_one(&pool)
    .await?;
    pool.close().await;
    Ok(BackupRepositoryStatus {
        path: path_string,
        initialized: true,
        available: true,
        stored_size: stored_size.max(0) as u64,
        logical_size: logical_size.max(0) as u64,
        snapshot_count: snapshot_count.max(0) as u64,
        error: None,
    })
}

pub async fn instance_config(
    instance_id: &str,
) -> crate::Result<InstanceBackupConfig> {
    let state = State::get().await?;
    let eligibility = eligibility_for(instance_id, &state).await?;
    let Some((_, pool)) = open_repository(&state, false).await? else {
        return Ok(InstanceBackupConfig {
            instance_id: instance_id.to_string(),
            enabled: false,
            eligibility,
            excluded_paths: Vec::new(),
            snapshot_count: 0,
        });
    };
    let enabled: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM instance_backup_configs WHERE instance_id = ?)",
    )
    .bind(instance_id)
    .fetch_one(&pool)
    .await?;
    let excluded_paths = if enabled {
        let rows: Vec<(String, String)> = sqlx::query_as(
			"SELECT path, kind FROM backup_exclusions WHERE instance_id = ? ORDER BY path",
		)
		.bind(instance_id)
		.fetch_all(&pool)
		.await?;
        rows.into_iter()
            .map(|(path, kind)| {
                Ok(BackupExclusion {
                    path,
                    kind: parse_exclusion_kind(&kind)?,
                })
            })
            .collect::<crate::Result<Vec<_>>>()?
    } else {
        Vec::new()
    };
    let snapshot_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM backup_snapshots WHERE instance_id = ?",
    )
    .bind(instance_id)
    .fetch_one(&pool)
    .await?;
    pool.close().await;
    Ok(InstanceBackupConfig {
        instance_id: instance_id.to_string(),
        enabled,
        eligibility,
        excluded_paths,
        snapshot_count: snapshot_count.max(0) as u64,
    })
}

fn parse_exclusion_kind(value: &str) -> crate::Result<BackupExclusionKind> {
    match value {
        "file" => Ok(BackupExclusionKind::File),
        "directory" => Ok(BackupExclusionKind::Directory),
        _ => Err(crate::ErrorKind::FSError(format!(
            "Unknown backup exclusion kind: {value}"
        ))
        .into()),
    }
}

pub async fn enable(
    instance_id: &str,
    excluded_paths: Vec<BackupExclusion>,
) -> crate::Result<InstanceBackupConfig> {
    let state = State::get().await?;
    let _repository_guard = REPOSITORY_LOCK.lock().await;
    let root = require_eligible(instance_id, &state).await?;
    let excluded_paths =
        validate_exclusion_paths(&root, excluded_paths).await?;
    let (instance, _) = instance_and_root(instance_id, &state).await?;
    let (_, pool) = open_repository(&state, true).await?.ok_or_else(|| {
        crate::ErrorKind::OtherError(
            "Backup repository could not be initialized".to_string(),
        )
    })?;
    let now = Utc::now().timestamp_millis();
    let mut tx = pool.begin().await?;
    sqlx::query(
        "INSERT INTO instance_backup_configs
         (instance_id, instance_name, created_at, modified_at)
         VALUES (?, ?, ?, ?)
         ON CONFLICT(instance_id) DO UPDATE SET
            instance_name = excluded.instance_name,
            modified_at = excluded.modified_at",
    )
    .bind(instance_id)
    .bind(&instance.name)
    .bind(now)
    .bind(now)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM backup_exclusions WHERE instance_id = ?")
        .bind(instance_id)
        .execute(&mut *tx)
        .await?;
    for exclusion in &excluded_paths {
        sqlx::query(
			"INSERT INTO backup_exclusions (instance_id, path, kind) VALUES (?, ?, ?)",
		)
		.bind(instance_id)
		.bind(&exclusion.path)
		.bind(exclusion.kind.as_str())
		.execute(&mut *tx)
		.await?;
    }
    tx.commit().await?;
    pool.close().await;
    drop(_repository_guard);
    instance_config(instance_id).await
}

pub async fn update_exclusions(
    instance_id: &str,
    excluded_paths: Vec<BackupExclusion>,
) -> crate::Result<InstanceBackupConfig> {
    let state = State::get().await?;
    let _repository_guard = REPOSITORY_LOCK.lock().await;
    let root = require_eligible(instance_id, &state).await?;
    let excluded_paths =
        validate_exclusion_paths(&root, excluded_paths).await?;
    let Some((_, pool)) = open_repository(&state, false).await? else {
        return Err(crate::ErrorKind::InputError(
            "Backups are not enabled for this instance".to_string(),
        )
        .into());
    };
    let mut tx = pool.begin().await?;
    let updated = sqlx::query(
        "UPDATE instance_backup_configs SET modified_at = ? WHERE instance_id = ?",
    )
    .bind(Utc::now().timestamp_millis())
    .bind(instance_id)
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() == 0 {
        return Err(crate::ErrorKind::InputError(
            "Backups are not enabled for this instance".to_string(),
        )
        .into());
    }
    sqlx::query("DELETE FROM backup_exclusions WHERE instance_id = ?")
        .bind(instance_id)
        .execute(&mut *tx)
        .await?;
    for exclusion in &excluded_paths {
        sqlx::query(
			"INSERT INTO backup_exclusions (instance_id, path, kind) VALUES (?, ?, ?)",
		)
		.bind(instance_id)
		.bind(&exclusion.path)
		.bind(exclusion.kind.as_str())
		.execute(&mut *tx)
		.await?;
    }
    tx.commit().await?;
    pool.close().await;
    drop(_repository_guard);
    instance_config(instance_id).await
}

pub async fn disable(instance_id: &str) -> crate::Result<()> {
    let state = State::get().await?;
    let _repository_guard = REPOSITORY_LOCK.lock().await;
    let Some((_, pool)) = open_repository(&state, false).await? else {
        return Ok(());
    };
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM backup_snapshots WHERE instance_id = ?",
    )
    .bind(instance_id)
    .fetch_one(&pool)
    .await?;
    if count > 0 {
        return Err(crate::ErrorKind::InputError(
            "Delete every snapshot before disabling backups".to_string(),
        )
        .into());
    }
    sqlx::query("DELETE FROM instance_backup_configs WHERE instance_id = ?")
        .bind(instance_id)
        .execute(&pool)
        .await?;
    pool.close().await;
    Ok(())
}

pub async fn list_snapshots(
    instance_id: &str,
) -> crate::Result<Vec<BackupSnapshot>> {
    let state = State::get().await?;
    let Some((_, pool)) = open_repository(&state, false).await? else {
        return Ok(Vec::new());
    };
    let rows = sqlx::query(
        "SELECT id, instance_id, instance_name, created_at, file_count,
                symlink_count, logical_size, added_size
         FROM backup_snapshots WHERE instance_id = ?
         ORDER BY created_at DESC, id DESC",
    )
    .bind(instance_id)
    .fetch_all(&pool)
    .await?;
    pool.close().await;
    Ok(rows.into_iter().map(snapshot_from_row).collect())
}

fn snapshot_from_row(row: sqlx::sqlite::SqliteRow) -> BackupSnapshot {
    BackupSnapshot {
        id: row.get("id"),
        instance_id: row.get("instance_id"),
        instance_name: row.get("instance_name"),
        created_at: row.get("created_at"),
        file_count: row.get::<i64, _>("file_count").max(0) as u64,
        symlink_count: row.get::<i64, _>("symlink_count").max(0) as u64,
        logical_size: row.get::<i64, _>("logical_size").max(0) as u64,
        added_size: row.get::<i64, _>("added_size").max(0) as u64,
    }
}

async fn emit_backup_progress(
    operation_id: Uuid,
    instance_id: Option<&str>,
    operation_type: InstanceBackupOperationType,
    stage: InstanceBackupProgressStage,
    processed_bytes: u64,
    total_bytes: u64,
    final_state: Option<InstanceBackupOperationFinalState>,
    snapshot_id: Option<&str>,
    message: Option<String>,
) {
    let persisted_state = match &stage {
        InstanceBackupProgressStage::Scanning => {
            Some(BackupOperationState::Scanning)
        }
        InstanceBackupProgressStage::Hashing => {
            Some(BackupOperationState::Hashing)
        }
        InstanceBackupProgressStage::Saving => {
            Some(BackupOperationState::Saving)
        }
        InstanceBackupProgressStage::Validating => {
            Some(BackupOperationState::Validating)
        }
        InstanceBackupProgressStage::Copying
        | InstanceBackupProgressStage::Restoring => {
            Some(BackupOperationState::Materializing)
        }
        InstanceBackupProgressStage::Completed => {
            Some(BackupOperationState::Completed)
        }
        InstanceBackupProgressStage::Cancelled => {
            Some(BackupOperationState::Cancelled)
        }
        InstanceBackupProgressStage::Failed => {
            Some(BackupOperationState::Failed)
        }
        InstanceBackupProgressStage::Deleting => None,
    };
    if let Some(state) = persisted_state {
        update_persisted_operation(
            operation_id,
            state,
            Some(processed_bytes),
            Some(total_bytes),
            None,
            message.as_deref(),
            snapshot_id,
        )
        .await;
    }
    let _ = crate::event::emit::emit_instance_backup_progress(
        InstanceBackupProgressPayload {
            operation_id,
            instance_id: instance_id.map(str::to_string),
            operation_type,
            stage,
            processed_bytes,
            total_bytes,
            final_state,
            snapshot_id: snapshot_id.map(str::to_string),
            message,
        },
    )
    .await;
}

fn register_backup_operation(
    operation_id: Uuid,
    instance_id: &str,
) -> (CancellationToken, Arc<AtomicBool>) {
    let cancellation = CancellationToken::new();
    let cancellable = Arc::new(AtomicBool::new(true));
    BACKUP_CANCELLATIONS.insert(
        operation_id,
        BackupCancellation {
            token: cancellation.clone(),
            cancellable: Arc::clone(&cancellable),
        },
    );
    tracing::debug!(%operation_id, instance_id, "Registered instance backup operation");
    (cancellation, cancellable)
}

fn unregister_backup_operation(instance_id: &str, operation_id: Uuid) {
    BACKUP_CANCELLATIONS.remove(&operation_id);
    let should_remove = ACTIVE_INSTANCE_OPERATIONS
        .get(instance_id)
        .is_some_and(|active| *active == operation_id);
    if should_remove {
        ACTIVE_INSTANCE_OPERATIONS.remove(instance_id);
    }
}

pub(crate) fn has_active_instance_operation(instance_id: &str) -> bool {
    ACTIVE_INSTANCE_OPERATIONS.contains_key(instance_id)
}

pub async fn start_snapshot(instance_id: String) -> crate::Result<Uuid> {
    let operation_id = Uuid::new_v4();
    create_persisted_operation(
        operation_id,
        &instance_id,
        BackupOperationType::Create,
        None,
    )
    .await?;
    let (cancellation, cancellable) =
        register_backup_operation(operation_id, &instance_id);
    tokio::spawn(async move {
        let result = create_snapshot(
            &instance_id,
            operation_id,
            cancellation.clone(),
            Arc::clone(&cancellable),
        )
        .await;
        let (stage, final_state, snapshot_id, total_bytes, message) =
            match result {
                Ok(snapshot) => (
                    InstanceBackupProgressStage::Completed,
                    InstanceBackupOperationFinalState::Completed,
                    Some(snapshot.id),
                    snapshot.logical_size,
                    None,
                ),
                Err(error) if cancellation.is_cancelled() => (
                    InstanceBackupProgressStage::Cancelled,
                    InstanceBackupOperationFinalState::Cancelled,
                    None,
                    0,
                    Some(error.to_string()),
                ),
                Err(error) => (
                    InstanceBackupProgressStage::Failed,
                    InstanceBackupOperationFinalState::Failed,
                    None,
                    0,
                    Some(error.to_string()),
                ),
            };
        let operation_state = match final_state {
            InstanceBackupOperationFinalState::Completed => {
                BackupOperationState::Completed
            }
            InstanceBackupOperationFinalState::Cancelled => {
                BackupOperationState::Cancelled
            }
            InstanceBackupOperationFinalState::Failed => {
                BackupOperationState::Failed
            }
        };
        update_persisted_operation(
            operation_id,
            operation_state,
            Some(total_bytes),
            Some(total_bytes),
            Some(false),
            message.as_deref(),
            snapshot_id.as_deref(),
        )
        .await;
        let _ = crate::event::emit::emit_instance_backup_progress(
            InstanceBackupProgressPayload {
                operation_id,
                instance_id: Some(instance_id.clone()),
                operation_type: InstanceBackupOperationType::Create,
                stage,
                processed_bytes: total_bytes,
                total_bytes,
                final_state: Some(final_state),
                snapshot_id,
                message,
            },
        )
        .await;
        unregister_backup_operation(&instance_id, operation_id);
    });
    Ok(operation_id)
}

pub async fn cancel_backup(operation_id: Uuid) -> crate::Result<bool> {
    let Some(operation) = BACKUP_CANCELLATIONS.get(&operation_id) else {
        return Ok(false);
    };
    if operation
        .cancellable
        .compare_exchange(true, false, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Ok(false);
    }
    let token = operation.token.clone();
    drop(operation);
    token.cancel();
    if State::initialized() {
        update_cancel_requested(operation_id).await?;
    }
    Ok(true)
}

async fn update_cancel_requested(operation_id: Uuid) -> crate::Result<()> {
    let state = State::get().await?;
    let Some((_, pool)) = open_repository(&state, false).await? else {
        return Err(crate::ErrorKind::FSError(
            "Unable to open backup repository".to_string(),
        )
        .into());
    };
    sqlx::query(
		"UPDATE backup_operations SET cancel_requested = 1, updated_at = ? WHERE id = ?",
	)
	.bind(Utc::now().timestamp_millis())
	.bind(operation_id.to_string())
	.execute(&pool)
	.await?;
    pool.close().await;
    Ok(())
}

pub async fn create_snapshot(
    instance_id: &str,
    operation_id: Uuid,
    cancellation: CancellationToken,
    cancellable: Arc<AtomicBool>,
) -> crate::Result<BackupSnapshot> {
    create_snapshot_inner(
        instance_id,
        operation_id,
        &cancellation,
        &cancellable,
    )
    .await
}

async fn create_snapshot_inner(
    instance_id: &str,
    operation_id: Uuid,
    cancellation: &CancellationToken,
    cancellable: &AtomicBool,
) -> crate::Result<BackupSnapshot> {
    let state = State::get().await?;
    let _maintenance_guard = lock_instance_maintenance(instance_id).await;
    let _instance_guard =
        state.lock_instance_content_exclusive(instance_id).await;
    if state.process_manager.has_instance_process(instance_id) {
        return Err(crate::ErrorKind::InputError(
            "Close the instance before creating a backup".to_string(),
        )
        .into());
    }
    let _repository_guard = REPOSITORY_LOCK.lock().await;
    let root = require_eligible(instance_id, &state).await?;
    let (instance, _) = instance_and_root(instance_id, &state).await?;
    let Some((repository, pool)) = open_repository(&state, false).await? else {
        return Err(crate::ErrorKind::InputError(
            "Backups are not enabled for this instance".to_string(),
        )
        .into());
    };
    let enabled: bool = sqlx::query_scalar(
		"SELECT EXISTS(SELECT 1 FROM instance_backup_configs WHERE instance_id = ?)",
	)
	.bind(instance_id)
	.fetch_one(&pool)
	.await?;
    if !enabled {
        return Err(crate::ErrorKind::InputError(
            "Backups are not enabled for this instance".to_string(),
        )
        .into());
    }
    let exclusion_rows: Vec<(String, String)> = sqlx::query_as(
		"SELECT path, kind FROM backup_exclusions WHERE instance_id = ? ORDER BY path",
	)
	.bind(instance_id)
	.fetch_all(&pool)
	.await?;
    let exclusions = exclusion_rows
        .into_iter()
        .map(|(path, kind)| {
            Ok(BackupExclusion {
                path,
                kind: parse_exclusion_kind(&kind)?,
            })
        })
        .collect::<crate::Result<Vec<_>>>()?;
    validate_exclusion_ancestors(&root, &exclusions).await?;
    let staging = repository.join(STAGING_DIR).join(operation_id.to_string());
    io::create_dir_all(&staging).await?;
    let mut created_objects = HashSet::new();
    let mut scan_result = scan_snapshot_attempt(
        &root,
        &repository,
        &staging,
        &exclusions,
        operation_id,
        instance_id,
        cancellation,
        &mut created_objects,
    )
    .await;
    if scan_result.is_err() && !cancellation.is_cancelled() {
        cleanup_uncommitted_objects(&repository, &pool, &created_objects).await;
        created_objects.clear();
        let _ = io::remove_dir_all(&staging).await;
        io::create_dir_all(&staging).await?;
        scan_result = scan_snapshot_attempt(
            &root,
            &repository,
            &staging,
            &exclusions,
            operation_id,
            instance_id,
            cancellation,
            &mut created_objects,
        )
        .await;
    }
    let (entries, added_size) = match scan_result {
        Ok(result) => result,
        Err(error) => {
            cleanup_uncommitted_objects(&repository, &pool, &created_objects)
                .await;
            let _ = io::remove_dir_all(&staging).await;
            pool.close().await;
            return Err(error);
        }
    };
    if cancellation.is_cancelled() {
        cleanup_uncommitted_objects(&repository, &pool, &created_objects).await;
        let _ = io::remove_dir_all(&staging).await;
        pool.close().await;
        return Err(crate::ErrorKind::OtherError(
            "Backup was canceled".to_string(),
        )
        .into());
    }

    let snapshot_id = Uuid::new_v4().to_string();
    let created_at = Utc::now().timestamp_millis();
    let file_count = entries
        .iter()
        .filter(|entry| entry.kind == EntryKind::File)
        .count() as u64;
    let symlink_count = entries
        .iter()
        .filter(|entry| entry.kind == EntryKind::Symlink)
        .count() as u64;
    let logical_size = entries.iter().map(|entry| entry.size).sum::<u64>();
    if !cancellable.swap(false, Ordering::AcqRel) {
        cleanup_uncommitted_objects(&repository, &pool, &created_objects).await;
        let _ = io::remove_dir_all(&staging).await;
        pool.close().await;
        return Err(crate::ErrorKind::OtherError(
            "Backup was canceled".to_string(),
        )
        .into());
    }
    emit_backup_progress(
        operation_id,
        Some(instance_id),
        InstanceBackupOperationType::Create,
        InstanceBackupProgressStage::Saving,
        logical_size,
        logical_size,
        None,
        None,
        None,
    )
    .await;
    let persist_result: crate::Result<()> = async {
        let mut tx = pool.begin().await?;
        sqlx::query(
            "INSERT INTO backup_snapshots
		 (id, instance_id, instance_name, created_at, file_count,
		  symlink_count, logical_size, added_size, scope_mode)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'excluded_paths')",
        )
        .bind(&snapshot_id)
        .bind(instance_id)
        .bind(&instance.name)
        .bind(created_at)
        .bind(file_count as i64)
        .bind(symlink_count as i64)
        .bind(logical_size as i64)
        .bind(added_size as i64)
        .execute(&mut *tx)
        .await?;
        for exclusion in &exclusions {
            sqlx::query(
                "INSERT INTO snapshot_exclusions (snapshot_id, path, kind)
			 VALUES (?, ?, ?)",
            )
            .bind(&snapshot_id)
            .bind(&exclusion.path)
            .bind(exclusion.kind.as_str())
            .execute(&mut *tx)
            .await?;
        }
        for entry in &entries {
            if let Some(hash) = &entry.object_hash {
                sqlx::query(
                    "INSERT INTO backup_objects (hash, size, ref_count)
                 VALUES (?, ?, 1)
                 ON CONFLICT(hash) DO UPDATE SET ref_count = ref_count + 1",
                )
                .bind(hash)
                .bind(entry.size as i64)
                .execute(&mut *tx)
                .await?;
            }
            sqlx::query(
                "INSERT INTO snapshot_entries
             (snapshot_id, path, kind, object_hash, size, modified_at,
              unix_mode, link_target, link_is_directory)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&snapshot_id)
            .bind(&entry.path)
            .bind(entry.kind.as_str())
            .bind(entry.object_hash.as_deref())
            .bind(entry.size as i64)
            .bind(entry.modified_at)
            .bind(entry.unix_mode.map(i64::from))
            .bind(entry.link_target.as_deref())
            .bind(entry.link_is_directory)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }
    .await;
    if let Err(error) = persist_result {
        cleanup_uncommitted_objects(&repository, &pool, &created_objects).await;
        let _ = io::remove_dir_all(&staging).await;
        pool.close().await;
        return Err(error);
    }
    let _ = io::remove_dir_all(&staging).await;
    pool.close().await;
    Ok(BackupSnapshot {
        id: snapshot_id,
        instance_id: instance_id.to_string(),
        instance_name: instance.name,
        created_at,
        file_count,
        symlink_count,
        logical_size,
        added_size,
    })
}

async fn scan_snapshot_attempt(
    root: &Path,
    repository: &Path,
    staging: &Path,
    exclusions: &[BackupExclusion],
    operation_id: Uuid,
    instance_id: &str,
    cancellation: &CancellationToken,
    created_objects: &mut HashSet<String>,
) -> crate::Result<(Vec<PendingEntry>, u64)> {
    emit_backup_progress(
        operation_id,
        Some(instance_id),
        InstanceBackupOperationType::Create,
        InstanceBackupProgressStage::Scanning,
        0,
        0,
        None,
        None,
        None,
    )
    .await;
    let total_bytes = measure_snapshot(root, exclusions, cancellation).await?;
    emit_backup_progress(
        operation_id,
        Some(instance_id),
        InstanceBackupOperationType::Create,
        InstanceBackupProgressStage::Hashing,
        0,
        total_bytes,
        None,
        None,
        None,
    )
    .await;
    scan_snapshot(
        root,
        repository,
        staging,
        exclusions,
        operation_id,
        instance_id,
        total_bytes,
        cancellation,
        created_objects,
    )
    .await
}

async fn measure_snapshot(
    root: &Path,
    exclusions: &[BackupExclusion],
    cancellation: &CancellationToken,
) -> crate::Result<u64> {
    let mut total_bytes = 0u64;
    let mut pending = vec![(root.to_path_buf(), String::new())];
    while let Some((directory, relative_directory)) = pending.pop() {
        let mut read_dir = tokio::fs::read_dir(&directory).await?;
        while let Some(child) = read_dir.next_entry().await? {
            if cancellation.is_cancelled() {
                return Err(crate::ErrorKind::OtherError(
                    "Backup was canceled".to_string(),
                )
                .into());
            }
            let Some(name) = child.file_name().to_str().map(str::to_string)
            else {
                return Err(crate::ErrorKind::UTFError(child.path()).into());
            };
            let relative = if relative_directory.is_empty() {
                name
            } else {
                format!("{relative_directory}/{name}")
            };
            if is_excluded(&relative, exclusions) {
                continue;
            }
            let path = child.path();
            let metadata = tokio::fs::symlink_metadata(&path).await?;
            if io::is_symlink_or_reparse(&metadata) {
                continue;
            }
            if metadata.is_dir() {
                pending.push((path, relative));
            } else if metadata.is_file() {
                total_bytes = total_bytes.saturating_add(metadata.len());
            }
        }
    }
    Ok(total_bytes)
}

async fn scan_snapshot(
    root: &Path,
    repository: &Path,
    staging: &Path,
    exclusions: &[BackupExclusion],
    operation_id: Uuid,
    instance_id: &str,
    total_bytes: u64,
    cancellation: &CancellationToken,
    created_objects: &mut HashSet<String>,
) -> crate::Result<(Vec<PendingEntry>, u64)> {
    let mut entries = Vec::new();
    let mut scanned_directories = Vec::new();
    let mut added_size = 0u64;
    let mut processed_bytes = 0u64;
    let mut pending = Vec::new();
    let mut root_names = Vec::new();
    let mut read_root = tokio::fs::read_dir(root).await?;
    while let Some(child) = read_root.next_entry().await? {
        let Some(name) = child.file_name().to_str().map(str::to_string) else {
            return Err(crate::ErrorKind::UTFError(child.path()).into());
        };
        root_names.push(name.clone());
        let child_path = child.path();
        let child_metadata = tokio::fs::symlink_metadata(&child_path).await?;
        pending.push((child_path, name, child_metadata));
    }
    root_names.sort();
    scanned_directories.push((root.to_path_buf(), root_names));
    while let Some((absolute, relative, metadata)) = pending.pop() {
        if cancellation.is_cancelled() {
            return Err(crate::ErrorKind::OtherError(
                "Backup was canceled".to_string(),
            )
            .into());
        }
        if is_excluded(&relative, exclusions) {
            continue;
        }
        if io::is_symlink_or_reparse(&metadata) {
            let target = tokio::fs::read_link(&absolute).await?;
            let target = target
                .to_str()
                .ok_or_else(|| crate::ErrorKind::UTFError(absolute.clone()))?;
            entries.push(PendingEntry {
                path: relative,
                kind: EntryKind::Symlink,
                object_hash: None,
                size: 0,
                modified_at: modified_millis(&metadata),
                unix_mode: unix_mode(&metadata),
                link_target: Some(target.to_string()),
                link_is_directory: Some(
                    tokio::fs::metadata(&absolute)
                        .await
                        .is_ok_and(|metadata| metadata.is_dir()),
                ),
            });
            continue;
        }
        if metadata.is_dir() {
            entries.push(PendingEntry {
                path: relative.clone(),
                kind: EntryKind::Directory,
                object_hash: None,
                size: 0,
                modified_at: modified_millis(&metadata),
                unix_mode: unix_mode(&metadata),
                link_target: None,
                link_is_directory: None,
            });
            let mut read_dir = tokio::fs::read_dir(&absolute).await?;
            let mut child_names = Vec::new();
            while let Some(child) = read_dir.next_entry().await? {
                let Some(name) = child.file_name().to_str().map(str::to_string)
                else {
                    return Err(crate::ErrorKind::UTFError(child.path()).into());
                };
                let child_relative = format!("{relative}/{name}");
                child_names.push(name);
                let child_path = child.path();
                let child_metadata =
                    tokio::fs::symlink_metadata(&child_path).await?;
                pending.push((child_path, child_relative, child_metadata));
            }
            child_names.sort();
            scanned_directories.push((absolute, child_names));
            continue;
        }
        if !metadata.is_file() {
            return Err(crate::ErrorKind::FSError(format!(
                "Unsupported backup entry: {}",
                absolute.display()
            ))
            .into());
        }
        let (hash, size, added) = hash_and_store_object(
            &absolute,
            &metadata,
            repository,
            staging,
            cancellation,
        )
        .await?;
        if added {
            created_objects.insert(hash.clone());
            added_size = added_size.saturating_add(size);
        }
        processed_bytes = processed_bytes.saturating_add(size);
        emit_backup_progress(
            operation_id,
            Some(instance_id),
            InstanceBackupOperationType::Create,
            InstanceBackupProgressStage::Hashing,
            processed_bytes,
            total_bytes,
            None,
            None,
            None,
        )
        .await;
        entries.push(PendingEntry {
            path: relative,
            kind: EntryKind::File,
            object_hash: Some(hash),
            size,
            modified_at: modified_millis(&metadata),
            unix_mode: unix_mode(&metadata),
            link_target: None,
            link_is_directory: None,
        });
    }
    for (directory, expected_names) in scanned_directories {
        let mut actual_names = Vec::new();
        let mut read_dir = tokio::fs::read_dir(&directory).await?;
        while let Some(child) = read_dir.next_entry().await? {
            let Some(name) = child.file_name().to_str().map(str::to_string)
            else {
                return Err(crate::ErrorKind::UTFError(child.path()).into());
            };
            actual_names.push(name);
        }
        actual_names.sort();
        if actual_names != expected_names {
            return Err(crate::ErrorKind::FSError(format!(
                "Directory changed while it was being backed up: {}",
                directory.display()
            ))
            .into());
        }
    }
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    Ok((entries, added_size))
}

fn is_excluded(relative: &str, exclusions: &[BackupExclusion]) -> bool {
    exclusions.iter().any(|exclusion| {
        relative == exclusion.path
            || (exclusion.kind == BackupExclusionKind::Directory
                && relative.starts_with(&format!("{}/", exclusion.path)))
    })
}

async fn hash_and_store_object(
    source: &Path,
    before: &std::fs::Metadata,
    repository: &Path,
    staging: &Path,
    cancellation: &CancellationToken,
) -> crate::Result<(String, u64, bool)> {
    let temp = staging.join(Uuid::new_v4().to_string());
    let mut input = tokio::fs::File::open(source).await?;
    let mut output = tokio::fs::File::create(&temp).await?;
    let mut hasher = Sha256::new();
    let mut size = 0u64;
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        if cancellation.is_cancelled() {
            let _ = tokio::fs::remove_file(&temp).await;
            return Err(crate::ErrorKind::OtherError(
                "Backup was canceled".to_string(),
            )
            .into());
        }
        let read = input.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        tokio::io::AsyncWriteExt::write_all(&mut output, &buffer[..read])
            .await?;
        size = size.saturating_add(read as u64);
    }
    tokio::io::AsyncWriteExt::flush(&mut output).await?;
    drop(output);
    let after = tokio::fs::metadata(source).await?;
    if before.len() != after.len()
        || before.modified().ok() != after.modified().ok()
    {
        let _ = tokio::fs::remove_file(&temp).await;
        return Err(crate::ErrorKind::FSError(format!(
            "File changed while it was being backed up: {}",
            source.display()
        ))
        .into());
    }
    let hash = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let object = object_path(repository, &hash);
    if object.try_exists()? {
        let existing_size = tokio::fs::metadata(&object).await?.len();
        let _ = tokio::fs::remove_file(&temp).await;
        if existing_size != size {
            return Err(crate::ErrorKind::FSError(format!(
                "Backup object size mismatch for {hash}"
            ))
            .into());
        }
        return Ok((hash, size, false));
    }
    if let Some(parent) = object.parent() {
        io::create_dir_all(parent).await?;
    }
    tokio::fs::rename(&temp, &object).await?;
    Ok((hash, size, true))
}

fn object_path(repository: &Path, hash: &str) -> PathBuf {
    repository
        .join(OBJECTS_DIR)
        .join(&hash[..2])
        .join(&hash[2..])
}

fn modified_millis(metadata: &std::fs::Metadata) -> Option<i64> {
    metadata.modified().ok().and_then(|modified| {
        modified
            .duration_since(UNIX_EPOCH)
            .ok()
            .and_then(|duration| i64::try_from(duration.as_millis()).ok())
    })
}

#[cfg(unix)]
fn unix_mode(metadata: &std::fs::Metadata) -> Option<u32> {
    use std::os::unix::fs::PermissionsExt;
    Some(metadata.permissions().mode())
}

#[cfg(not(unix))]
fn unix_mode(_metadata: &std::fs::Metadata) -> Option<u32> {
    None
}

async fn cleanup_uncommitted_objects(
    repository: &Path,
    pool: &SqlitePool,
    hashes: &HashSet<String>,
) {
    for hash in hashes {
        let referenced: Result<bool, _> = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM backup_objects WHERE hash = ?)",
        )
        .bind(hash)
        .fetch_one(pool)
        .await;
        if matches!(referenced, Ok(false)) {
            let _ = tokio::fs::remove_file(object_path(repository, hash)).await;
        }
    }
}

async fn snapshot_operation_metadata(
    snapshot_id: &str,
) -> crate::Result<(String, u64)> {
    let state = State::get().await?;
    let Some((_, pool)) = open_repository(&state, false).await? else {
        return Err(crate::ErrorKind::InputError(
            "Unknown backup snapshot".to_string(),
        )
        .into());
    };
    let metadata: (String, i64) = sqlx::query_as(
        "SELECT instance_id, logical_size FROM backup_snapshots WHERE id = ?",
    )
    .bind(snapshot_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| {
        crate::ErrorKind::InputError("Unknown backup snapshot".to_string())
    })?;
    pool.close().await;
    Ok((metadata.0, metadata.1.max(0) as u64))
}

pub async fn delete_snapshot(snapshot_id: &str) -> crate::Result<()> {
    let operation_id = Uuid::new_v4();
    emit_backup_progress(
        operation_id,
        None,
        InstanceBackupOperationType::Delete,
        InstanceBackupProgressStage::Deleting,
        0,
        0,
        None,
        Some(snapshot_id),
        None,
    )
    .await;
    let (metadata, result) =
        match snapshot_operation_metadata(snapshot_id).await {
            Ok((instance_id, total_bytes)) => {
                emit_backup_progress(
                    operation_id,
                    Some(&instance_id),
                    InstanceBackupOperationType::Delete,
                    InstanceBackupProgressStage::Deleting,
                    0,
                    total_bytes,
                    None,
                    Some(snapshot_id),
                    None,
                )
                .await;
                let result = delete_snapshot_inner(snapshot_id).await;
                (Some((instance_id, total_bytes)), result)
            }
            Err(error) => (None, Err(error)),
        };
    let (instance_id, total_bytes) = metadata
        .as_ref()
        .map(|(instance_id, total_bytes)| {
            (Some(instance_id.as_str()), *total_bytes)
        })
        .unwrap_or((None, 0));
    let (stage, final_state, processed_bytes, message) = match &result {
        Ok(()) => (
            InstanceBackupProgressStage::Completed,
            InstanceBackupOperationFinalState::Completed,
            total_bytes,
            None,
        ),
        Err(error) => (
            InstanceBackupProgressStage::Failed,
            InstanceBackupOperationFinalState::Failed,
            0,
            Some(error.to_string()),
        ),
    };
    emit_backup_progress(
        operation_id,
        instance_id,
        InstanceBackupOperationType::Delete,
        stage,
        processed_bytes,
        total_bytes,
        Some(final_state),
        Some(snapshot_id),
        message,
    )
    .await;
    result
}

async fn delete_snapshot_inner(snapshot_id: &str) -> crate::Result<()> {
    let state = State::get().await?;
    let _repository_guard = REPOSITORY_LOCK.lock().await;
    let Some((repository, pool)) = open_repository(&state, false).await? else {
        return Err(crate::ErrorKind::InputError(
            "Unknown backup snapshot".to_string(),
        )
        .into());
    };
    delete_snapshots_in_pool(&repository, &pool, Some(snapshot_id), None)
        .await?;
    pool.close().await;
    Ok(())
}

async fn delete_snapshots_in_pool(
    repository: &Path,
    pool: &SqlitePool,
    snapshot_id: Option<&str>,
    instance_id: Option<&str>,
) -> crate::Result<()> {
    let rows = if let Some(snapshot_id) = snapshot_id {
        sqlx::query(
            "SELECT object_hash, COUNT(*) AS refs FROM snapshot_entries
             WHERE snapshot_id = ? AND object_hash IS NOT NULL
             GROUP BY object_hash",
        )
        .bind(snapshot_id)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query(
            "SELECT e.object_hash, COUNT(*) AS refs FROM snapshot_entries e
             JOIN backup_snapshots s ON s.id = e.snapshot_id
             WHERE s.instance_id = ? AND e.object_hash IS NOT NULL
             GROUP BY e.object_hash",
        )
        .bind(instance_id)
        .fetch_all(pool)
        .await?
    };
    let mut tx = pool.begin().await?;
    let deleted = if let Some(snapshot_id) = snapshot_id {
        sqlx::query("DELETE FROM backup_snapshots WHERE id = ?")
            .bind(snapshot_id)
            .execute(&mut *tx)
            .await?
    } else {
        sqlx::query("DELETE FROM backup_snapshots WHERE instance_id = ?")
            .bind(instance_id)
            .execute(&mut *tx)
            .await?
    };
    if snapshot_id.is_some() && deleted.rows_affected() == 0 {
        return Err(crate::ErrorKind::InputError(
            "Unknown backup snapshot".to_string(),
        )
        .into());
    }
    for row in rows {
        let hash: String = row.get("object_hash");
        let refs: i64 = row.get("refs");
        sqlx::query(
            "UPDATE backup_objects SET ref_count = ref_count - ? WHERE hash = ?",
        )
        .bind(refs)
        .bind(&hash)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT OR IGNORE INTO object_gc_queue (hash, queued_at)
             SELECT hash, ? FROM backup_objects WHERE hash = ? AND ref_count = 0",
        )
        .bind(Utc::now().timestamp_millis())
        .bind(&hash)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    run_gc(repository, pool).await
}

async fn run_gc(repository: &Path, pool: &SqlitePool) -> crate::Result<()> {
    let hashes: Vec<String> = sqlx::query_scalar(
        "SELECT hash FROM object_gc_queue ORDER BY queued_at",
    )
    .fetch_all(pool)
    .await?;
    for hash in hashes {
        match tokio::fs::remove_file(object_path(repository, &hash)).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                tracing::warn!(%hash, %error, "Failed to remove unreferenced backup object");
                continue;
            }
        }
        let mut tx = pool.begin().await?;
        sqlx::query("DELETE FROM object_gc_queue WHERE hash = ?")
            .bind(&hash)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "DELETE FROM backup_objects WHERE hash = ? AND ref_count = 0",
        )
        .bind(&hash)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
    }
    Ok(())
}

/// Completes interrupted restores before instances can be launched or edited.
pub async fn recover_repository() -> crate::Result<()> {
    let state = State::get().await?;
    let _repository_guard = REPOSITORY_LOCK.lock().await;
    recover_interrupted_restores(&state).await?;
    let Some((_, pool)) = open_repository(&state, false).await? else {
        return Ok(());
    };
    let now = Utc::now().timestamp_millis();
    sqlx::query(
        "UPDATE backup_operations SET state = 'interrupted', cancellable = 0,
		 error = COALESCE(error, 'The launcher exited while the operation was running'),
		 updated_at = ?, finished_at = ?
		 WHERE state NOT IN ('completed', 'cancelled', 'failed', 'interrupted')",
    )
    .bind(now)
    .bind(now)
    .execute(&pool)
    .await?;
    pool.close().await;
    Ok(())
}

/// Storage housekeeping is serialized with backup operations but does not gate startup.
pub async fn maintain_repository() -> crate::Result<()> {
    let state = State::get().await?;
    let _repository_guard = REPOSITORY_LOCK.lock().await;
    cleanup_pending_repository_paths(&state).await?;
    cleanup_committed_restores(&state.directories.instances_dir()).await?;
    let Some((repository, pool)) = open_repository(&state, false).await? else {
        return Ok(());
    };
    run_gc(&repository, &pool).await?;

    let pending: Vec<String> = sqlx::query_scalar(
        "SELECT instance_id FROM pending_instance_deletions ORDER BY marked_at",
    )
    .fetch_all(&pool)
    .await?;
    for instance_id in pending {
        if instance_rows::get_instance_by_id(&instance_id, &state.pool)
            .await?
            .is_none()
        {
            delete_snapshots_in_pool(
                &repository,
                &pool,
                None,
                Some(&instance_id),
            )
            .await?;
            sqlx::query(
                "DELETE FROM instance_backup_configs WHERE instance_id = ?",
            )
            .bind(&instance_id)
            .execute(&pool)
            .await?;
        }
        sqlx::query(
            "DELETE FROM pending_instance_deletions WHERE instance_id = ?",
        )
        .bind(&instance_id)
        .execute(&pool)
        .await?;
    }

    let staging = repository.join(STAGING_DIR);
    let _ = io::remove_dir_all(&staging).await;
    io::create_dir_all(&staging).await?;
    sweep_unindexed_objects(&repository, &pool).await?;
    pool.close().await;
    Ok(())
}

async fn cleanup_pending_repository_paths(state: &State) -> crate::Result<()> {
    let current = repository_path(state).await?;
    let paths: Vec<String> = sqlx::query_scalar(
        "SELECT path FROM pending_backup_repository_cleanups ORDER BY created_at",
    )
    .fetch_all(&state.pool)
    .await?;
    for stored_path in paths {
        let path = PathBuf::from(&stored_path);
        if !path.is_absolute() || path == current {
            tracing::warn!(
                path = %path.display(),
                "Ignoring invalid pending backup repository cleanup path"
            );
            continue;
        }
        if path.try_exists()?
            && let Err(error) = io::remove_dir_all(&path).await
        {
            tracing::warn!(
                path = %path.display(),
                %error,
                "Failed to retry old backup repository cleanup"
            );
            continue;
        }
        sqlx::query(
            "DELETE FROM pending_backup_repository_cleanups WHERE path = ?",
        )
        .bind(&stored_path)
        .execute(&state.pool)
        .await?;
    }
    Ok(())
}

async fn recover_interrupted_restores(state: &State) -> crate::Result<()> {
    let instances = state.directories.instances_dir();
    let mut entries = match tokio::fs::read_dir(&instances).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    };
    while let Some(entry) = entries.next_entry().await? {
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if !name.starts_with(RESTORE_PREFIX)
            || !entry.file_type().await?.is_dir()
        {
            continue;
        }
        let operation_root = entry.path();
        if tokio::fs::symlink_metadata(operation_root.join(RESTORE_COMMITTED))
            .await
            .is_ok()
        {
            continue;
        }
        let journal_bytes = match tokio::fs::read(
            operation_root.join(RESTORE_JOURNAL),
        )
        .await
        {
            Ok(bytes) => bytes,
            Err(error) => {
                tracing::warn!(
                    path = %operation_root.display(),
                    %error,
                    "Cannot recover interrupted backup restore without its journal"
                );
                continue;
            }
        };
        let journal: RestoreJournal =
            match serde_json::from_slice(&journal_bytes) {
                Ok(journal) => journal,
                Err(error) => {
                    tracing::warn!(
                        path = %operation_root.display(),
                        %error,
                        "Cannot parse interrupted backup restore journal"
                    );
                    continue;
                }
            };
        let (_, root) = instance_and_root(&journal.instance_id, state).await?;
        if journal.whole_root {
            recover_whole_root_restore(
                &root,
                &operation_root,
                &journal.exclusions,
            )
            .await?;
        } else {
            recover_restore_operation(&root, &operation_root, &journal.roots)
                .await?;
        }
    }
    Ok(())
}

async fn cleanup_committed_restores(instances: &Path) -> crate::Result<()> {
    let mut entries = match tokio::fs::read_dir(instances).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    };
    while let Some(entry) = entries.next_entry().await? {
        if !entry
            .file_name()
            .to_string_lossy()
            .starts_with(RESTORE_PREFIX)
            || !entry.file_type().await?.is_dir()
        {
            continue;
        }
        if tokio::fs::symlink_metadata(entry.path().join(RESTORE_COMMITTED))
            .await
            .is_ok()
        {
            io::remove_dir_all(entry.path()).await?;
        }
    }
    Ok(())
}

async fn recover_whole_root_restore(
    root: &Path,
    operation_root: &Path,
    exclusions: &[BackupExclusion],
) -> crate::Result<()> {
    ensure_canonical_exclusions(exclusions)?;
    let staged = operation_root.join("staged");
    let rollback_root = operation_root.join("rollback").join("instance");
    if tokio::fs::symlink_metadata(&rollback_root).await.is_ok() {
        validate_exclusion_ancestors(&rollback_root, exclusions).await?;
        for exclusion in exclusions.iter().rev() {
            let destination = join_relative(&rollback_root, &exclusion.path);
            let from_current = join_relative(root, &exclusion.path);
            let from_staged = join_relative(&staged, &exclusion.path);
            let source = if tokio::fs::symlink_metadata(&from_current)
                .await
                .is_ok()
            {
                Some(from_current)
            } else if tokio::fs::symlink_metadata(&from_staged).await.is_ok() {
                Some(from_staged)
            } else {
                None
            };
            if let Some(source) = source {
                remove_path_if_exists(&destination).await;
                if let Some(parent) = destination.parent() {
                    io::create_dir_all(parent).await?;
                }
                rename_backup_path(&source, &destination).await?;
            }
        }
        remove_path_if_exists(root).await;
        if let Some(parent) = root.parent() {
            io::create_dir_all(parent).await?;
        }
        rename_backup_path(&rollback_root, root).await?;
    }
    io::remove_dir_all(operation_root).await?;
    Ok(())
}

async fn recover_restore_operation(
    root: &Path,
    operation_root: &Path,
    roots: &[RestoreJournalRoot],
) -> crate::Result<()> {
    let rollback = operation_root.join("rollback");
    for journal_root in roots.iter().rev() {
        let normalized = normalize_relative_directory(&journal_root.path)?;
        if normalized != journal_root.path {
            return Err(crate::ErrorKind::FSError(
                "Invalid path in backup restore journal".to_string(),
            )
            .into());
        }
        let current = join_relative(root, &normalized);
        let old = join_relative(&rollback, &normalized);
        if tokio::fs::symlink_metadata(&old).await.is_ok() {
            remove_path_if_exists(&current).await;
            if let Some(parent) = current.parent() {
                io::create_dir_all(parent).await?;
            }
            rename_backup_path(&old, &current).await?;
        } else if !journal_root.had_current {
            remove_path_if_exists(&current).await;
        }
    }
    io::remove_dir_all(operation_root).await?;
    Ok(())
}

async fn remove_path_if_exists(path: &Path) {
    let _ = io::remove_dir_all(path).await;
    let _ = tokio::fs::remove_file(path).await;
}

async fn rename_backup_path(
    source: &Path,
    destination: &Path,
) -> crate::Result<()> {
    const ATTEMPTS: u32 = 5;
    for attempt in 0..ATTEMPTS {
        match tokio::fs::rename(source, destination).await {
            Ok(()) => return Ok(()),
            Err(error)
                if error.kind() == std::io::ErrorKind::PermissionDenied
                    && attempt + 1 < ATTEMPTS =>
            {
                tokio::time::sleep(Duration::from_millis(
                    40 * u64::from(attempt + 1),
                ))
                .await;
            }
            Err(error) => {
                return Err(crate::ErrorKind::FSError(format!(
                    "Could not move {} to {}: {error}",
                    source.display(),
                    destination.display()
                ))
                .into());
            }
        }
    }
    unreachable!()
}

async fn mark_restore_committed(operation_root: &Path) -> crate::Result<()> {
    let mut marker =
        tokio::fs::File::create(operation_root.join(RESTORE_COMMITTED)).await?;
    marker.write_all(b"committed").await?;
    marker.sync_all().await?;
    Ok(())
}

async fn sweep_unindexed_objects(
    repository: &Path,
    pool: &SqlitePool,
) -> crate::Result<()> {
    let objects = repository.join(OBJECTS_DIR);
    let mut prefixes = tokio::fs::read_dir(&objects).await?;
    while let Some(prefix) = prefixes.next_entry().await? {
        if !prefix.file_type().await?.is_dir() {
            continue;
        }
        let Some(prefix_name) = prefix.file_name().to_str().map(str::to_string)
        else {
            continue;
        };
        let mut files = tokio::fs::read_dir(prefix.path()).await?;
        while let Some(file) = files.next_entry().await? {
            if !file.file_type().await?.is_file() {
                continue;
            }
            let Some(file_name) = file.file_name().to_str().map(str::to_string)
            else {
                continue;
            };
            let hash = format!("{prefix_name}{file_name}");
            if hash.len() != 64
                || !hash.bytes().all(|byte| byte.is_ascii_hexdigit())
            {
                continue;
            }
            let indexed: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM backup_objects WHERE hash = ?)",
            )
            .bind(&hash)
            .fetch_one(pool)
            .await?;
            if !indexed {
                tokio::fs::remove_file(file.path()).await?;
            }
        }
        let _ = tokio::fs::remove_dir(prefix.path()).await;
    }
    Ok(())
}

pub async fn instance_delete_summary(
    instance_id: &str,
) -> crate::Result<BackupDeleteSummary> {
    let state = State::get().await?;
    let Some((_, pool)) = open_repository(&state, false).await? else {
        return Ok(BackupDeleteSummary {
            snapshot_count: 0,
            logical_size: 0,
        });
    };
    let row = sqlx::query(
        "SELECT COUNT(*) AS count, COALESCE(SUM(logical_size), 0) AS size
         FROM backup_snapshots WHERE instance_id = ?",
    )
    .bind(instance_id)
    .fetch_one(&pool)
    .await?;
    pool.close().await;
    Ok(BackupDeleteSummary {
        snapshot_count: row.get::<i64, _>("count").max(0) as u64,
        logical_size: row.get::<i64, _>("size").max(0) as u64,
    })
}

pub(crate) async fn delete_instance_backups(
    instance_id: &str,
) -> crate::Result<()> {
    let state = State::get().await?;
    let _repository_guard = REPOSITORY_LOCK.lock().await;
    let Some((repository, pool)) = open_repository(&state, false).await? else {
        return Ok(());
    };
    delete_snapshots_in_pool(&repository, &pool, None, Some(instance_id))
        .await?;
    sqlx::query("DELETE FROM instance_backup_configs WHERE instance_id = ?")
        .bind(instance_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM pending_instance_deletions WHERE instance_id = ?")
        .bind(instance_id)
        .execute(&pool)
        .await?;
    pool.close().await;
    Ok(())
}

pub(crate) async fn begin_instance_deletion(
    instance_id: &str,
) -> crate::Result<()> {
    let state = State::get().await?;
    let _repository_guard = REPOSITORY_LOCK.lock().await;
    let Some((_, pool)) = open_repository(&state, false).await? else {
        return Ok(());
    };
    sqlx::query(
        "INSERT INTO pending_instance_deletions (instance_id, marked_at)
         VALUES (?, ?)
         ON CONFLICT(instance_id) DO UPDATE SET marked_at = excluded.marked_at",
    )
    .bind(instance_id)
    .bind(Utc::now().timestamp_millis())
    .execute(&pool)
    .await?;
    pool.close().await;
    Ok(())
}

pub(crate) async fn cancel_instance_deletion(
    instance_id: &str,
) -> crate::Result<()> {
    let state = State::get().await?;
    let _repository_guard = REPOSITORY_LOCK.lock().await;
    let Some((_, pool)) = open_repository(&state, false).await? else {
        return Ok(());
    };
    sqlx::query("DELETE FROM pending_instance_deletions WHERE instance_id = ?")
        .bind(instance_id)
        .execute(&pool)
        .await?;
    pool.close().await;
    Ok(())
}

pub(crate) async fn ensure_backup_eligible_edit(
    instance_id: &str,
    patch: &crate::state::EditInstance,
) -> crate::Result<()> {
    let becomes_ineligible =
        patch.symlink_target.as_ref().is_some_and(Option::is_some)
            || patch
                .game_dir_override
                .as_ref()
                .and_then(|path| path.as_deref())
                .is_some_and(|path| !path.trim().is_empty());
    if !becomes_ineligible {
        return Ok(());
    }
    let state = State::get().await?;
    let Some((_, pool)) = open_repository(&state, false).await? else {
        return Ok(());
    };
    let enabled: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM instance_backup_configs WHERE instance_id = ?)",
    )
    .bind(instance_id)
    .fetch_one(&pool)
    .await?;
    pool.close().await;
    if enabled {
        return Err(crate::ErrorKind::InputError(
            "Disable backups before changing this instance to use an external directory"
                .to_string(),
        )
        .into());
    }
    Ok(())
}

pub async fn start_restore_preview(snapshot_id: &str) -> crate::Result<Uuid> {
    let (instance_id, total_bytes) =
        snapshot_operation_metadata(snapshot_id).await?;
    let operation_id = Uuid::new_v4();
    create_persisted_operation(
        operation_id,
        &instance_id,
        BackupOperationType::RestorePreview,
        Some(snapshot_id),
    )
    .await?;
    let (cancellation, cancellable) =
        register_backup_operation(operation_id, &instance_id);
    let snapshot_id = snapshot_id.to_string();
    tokio::spawn(async move {
        let progress = RestorePreviewProgress {
            operation_id,
            instance_id: instance_id.clone(),
            total_bytes,
            cancellation: cancellation.clone(),
        };
        let result = preview_restore_inner(&snapshot_id, Some(&progress)).await;
        let (stage, final_state, processed_bytes, message, state) =
            match &result {
                Ok(preview) => {
                    store_restore_preview(operation_id, preview).await;
                    (
                        InstanceBackupProgressStage::Completed,
                        InstanceBackupOperationFinalState::Completed,
                        total_bytes,
                        None,
                        BackupOperationState::Completed,
                    )
                }
                Err(error) if cancellation.is_cancelled() => (
                    InstanceBackupProgressStage::Cancelled,
                    InstanceBackupOperationFinalState::Cancelled,
                    0,
                    Some(error.to_string()),
                    BackupOperationState::Cancelled,
                ),
                Err(error) => (
                    InstanceBackupProgressStage::Failed,
                    InstanceBackupOperationFinalState::Failed,
                    0,
                    Some(error.to_string()),
                    BackupOperationState::Failed,
                ),
            };
        cancellable.store(false, Ordering::Release);
        update_persisted_operation(
            operation_id,
            state,
            Some(processed_bytes),
            Some(total_bytes),
            Some(false),
            message.as_deref(),
            Some(&snapshot_id),
        )
        .await;
        emit_backup_progress(
            operation_id,
            Some(&instance_id),
            InstanceBackupOperationType::RestorePreview,
            stage,
            processed_bytes,
            total_bytes,
            Some(final_state),
            Some(&snapshot_id),
            message,
        )
        .await;
        unregister_backup_operation(&instance_id, operation_id);
    });
    Ok(operation_id)
}

struct RestorePreviewProgress {
    operation_id: Uuid,
    instance_id: String,
    total_bytes: u64,
    cancellation: CancellationToken,
}

async fn preview_restore_inner(
    snapshot_id: &str,
    progress: Option<&RestorePreviewProgress>,
) -> crate::Result<BackupRestorePreview> {
    let state = State::get().await?;
    let (instance_id, _) = snapshot_operation_metadata(snapshot_id).await?;
    let _maintenance_guard = lock_instance_maintenance(&instance_id).await;
    let _instance_guard =
        state.lock_instance_content_exclusive(&instance_id).await;
    if state.process_manager.has_instance_process(&instance_id) {
        return Err(crate::ErrorKind::InputError(
            "Close the instance before restoring a backup".to_string(),
        )
        .into());
    }
    let _repository_guard = REPOSITORY_LOCK.lock().await;
    let Some((_, pool)) = open_repository(&state, false).await? else {
        return Err(crate::ErrorKind::InputError(
            "Unknown backup snapshot".to_string(),
        )
        .into());
    };
    let root = require_eligible(&instance_id, &state).await?;
    let plan =
        build_restore_plan(snapshot_id, &instance_id, &root, &pool, progress)
            .await?;
    pool.close().await;
    Ok(plan.preview)
}

pub async fn restore_snapshot(
    snapshot_id: &str,
    plan_token: &str,
) -> crate::Result<Uuid> {
    let (instance_id, total_bytes) =
        snapshot_operation_metadata(snapshot_id).await?;
    let operation_id = Uuid::new_v4();
    create_persisted_operation(
        operation_id,
        &instance_id,
        BackupOperationType::Restore,
        Some(snapshot_id),
    )
    .await?;
    let (cancellation, cancellable) =
        register_backup_operation(operation_id, &instance_id);
    let snapshot_id = snapshot_id.to_string();
    let plan_token = plan_token.to_string();
    tokio::spawn(async move {
        emit_backup_progress(
            operation_id,
            Some(&instance_id),
            InstanceBackupOperationType::Restore,
            InstanceBackupProgressStage::Validating,
            0,
            total_bytes,
            None,
            Some(&snapshot_id),
            None,
        )
        .await;
        if cancellation.is_cancelled() {
            update_persisted_operation(
                operation_id,
                BackupOperationState::Cancelled,
                Some(0),
                Some(total_bytes),
                Some(false),
                Some("Backup restore was canceled"),
                Some(&snapshot_id),
            )
            .await;
            unregister_backup_operation(&instance_id, operation_id);
            return;
        }
        let result = restore_snapshot_inner(
            &snapshot_id,
            &plan_token,
            &cancellable,
            operation_id,
        )
        .await;
        let (stage, final_state, processed_bytes, message, state) = match result
        {
            Ok(()) => (
                InstanceBackupProgressStage::Completed,
                InstanceBackupOperationFinalState::Completed,
                total_bytes,
                None,
                BackupOperationState::Completed,
            ),
            Err(error) if cancellation.is_cancelled() => (
                InstanceBackupProgressStage::Cancelled,
                InstanceBackupOperationFinalState::Cancelled,
                0,
                Some(error.to_string()),
                BackupOperationState::Cancelled,
            ),
            Err(error) => (
                InstanceBackupProgressStage::Failed,
                InstanceBackupOperationFinalState::Failed,
                0,
                Some(error.to_string()),
                BackupOperationState::Failed,
            ),
        };
        update_persisted_operation(
            operation_id,
            state,
            Some(processed_bytes),
            Some(total_bytes),
            Some(false),
            message.as_deref(),
            Some(&snapshot_id),
        )
        .await;
        emit_backup_progress(
            operation_id,
            Some(&instance_id),
            InstanceBackupOperationType::Restore,
            stage,
            processed_bytes,
            total_bytes,
            Some(final_state),
            Some(&snapshot_id),
            message,
        )
        .await;
        unregister_backup_operation(&instance_id, operation_id);
    });
    Ok(operation_id)
}

async fn build_restore_plan(
    snapshot_id: &str,
    expected_instance_id: &str,
    root: &Path,
    pool: &SqlitePool,
    progress: Option<&RestorePreviewProgress>,
) -> crate::Result<RestorePlan> {
    let (instance_id, scope_mode): (String, String) = sqlx::query_as(
        "SELECT instance_id, scope_mode FROM backup_snapshots WHERE id = ?",
    )
    .bind(snapshot_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| {
        crate::ErrorKind::InputError("Unknown backup snapshot".to_string())
    })?;
    if instance_id != expected_instance_id {
        return Err(crate::ErrorKind::FSError(
            "Backup snapshot changed while preparing its restore".to_string(),
        )
        .into());
    }
    let whole_root = match scope_mode.as_str() {
        "included_roots" => false,
        "excluded_paths" => true,
        _ => {
            return Err(crate::ErrorKind::FSError(format!(
                "Unknown backup snapshot scope: {scope_mode}"
            ))
            .into());
        }
    };
    let selected_roots: Vec<(String, bool)> = sqlx::query_as(
        "SELECT path, existed FROM snapshot_roots WHERE snapshot_id = ? ORDER BY path",
    )
    .bind(snapshot_id)
    .fetch_all(pool)
    .await?;
    for (path, _) in &selected_roots {
        if normalize_relative_directory(path)? != *path {
            return Err(crate::ErrorKind::FSError(
                "Invalid root path in backup snapshot".to_string(),
            )
            .into());
        }
    }
    let exclusion_rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT path, kind FROM snapshot_exclusions WHERE snapshot_id = ? ORDER BY path",
    )
    .bind(snapshot_id)
    .fetch_all(pool)
    .await?;
    let raw_exclusions = exclusion_rows
        .into_iter()
        .map(|(path, kind)| {
            Ok(BackupExclusion {
                path,
                kind: parse_exclusion_kind(&kind)?,
            })
        })
        .collect::<crate::Result<Vec<_>>>()?;
    let exclusions = canonicalize_exclusions(raw_exclusions.clone())?;
    if exclusions.len() != raw_exclusions.len()
        || raw_exclusions
            .iter()
            .any(|exclusion| !exclusions.contains(exclusion))
    {
        return Err(crate::ErrorKind::FSError(
            "Invalid exclusion paths in backup snapshot".to_string(),
        )
        .into());
    }
    validate_exclusion_ancestors(root, &exclusions).await?;

    let rows = sqlx::query(
        "SELECT path, kind, object_hash, size, unix_mode, link_target,
                link_is_directory
         FROM snapshot_entries WHERE snapshot_id = ? ORDER BY length(path), path",
    )
    .bind(snapshot_id)
    .fetch_all(pool)
    .await?;
    let mut target = BTreeMap::new();
    for row in rows {
        let path: String = row.get("path");
        if normalize_relative_directory(&path)? != path
            || is_excluded(&path, &exclusions)
        {
            return Err(crate::ErrorKind::FSError(
                "Invalid path in backup snapshot".to_string(),
            )
            .into());
        }
        let kind = match row.get::<String, _>("kind").as_str() {
            "file" => EntryKind::File,
            "directory" => EntryKind::Directory,
            "symlink" => EntryKind::Symlink,
            kind => {
                return Err(crate::ErrorKind::FSError(format!(
                    "Unsupported backup entry kind: {kind}"
                ))
                .into());
            }
        };
        if kind == EntryKind::Symlink
            && exclusions.iter().any(|exclusion| {
                exclusion.path.starts_with(&format!("{path}/"))
            })
        {
            return Err(crate::ErrorKind::FSError(
                "Backup snapshot links through an excluded path".to_string(),
            )
            .into());
        }
        let entry = RestoreEntry {
            path: path.clone(),
            kind,
            object_hash: row.try_get("object_hash")?,
            size: row.get::<i64, _>("size").max(0) as u64,
            unix_mode: row.try_get("unix_mode")?,
            link_target: row.try_get("link_target")?,
            link_is_directory: row.try_get("link_is_directory")?,
        };
        if target.insert(path, entry).is_some() {
            return Err(crate::ErrorKind::FSError(
                "Backup snapshot contains duplicate paths".to_string(),
            )
            .into());
        }
    }
    let current = scan_restore_entries(
        root,
        whole_root,
        &selected_roots,
        &exclusions,
        progress,
    )
    .await?;
    Ok(compare_restore_entries(snapshot_id, current, target))
}

async fn scan_restore_entries(
    root: &Path,
    whole_root: bool,
    selected_roots: &[(String, bool)],
    exclusions: &[BackupExclusion],
    progress: Option<&RestorePreviewProgress>,
) -> crate::Result<BTreeMap<String, RestoreEntry>> {
    let mut pending = Vec::new();
    if whole_root {
        let mut children = tokio::fs::read_dir(root).await?;
        while let Some(child) = children.next_entry().await? {
            let name = child
                .file_name()
                .to_str()
                .map(str::to_string)
                .ok_or_else(|| crate::ErrorKind::UTFError(child.path()))?;
            pending.push((child.path(), name));
        }
    } else {
        for (path, _) in selected_roots {
            pending.push((join_relative(root, path), path.clone()));
        }
    }

    let mut entries = BTreeMap::new();
    let mut processed_bytes = 0u64;
    while let Some((absolute, relative)) = pending.pop() {
        if progress.is_some_and(|progress| progress.cancellation.is_cancelled())
        {
            return Err(crate::ErrorKind::OtherError(
                "Restore preview was canceled".to_string(),
            )
            .into());
        }
        if is_excluded(&relative, exclusions) {
            continue;
        }
        let metadata = match tokio::fs::symlink_metadata(&absolute).await {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                continue;
            }
            Err(error) => return Err(error.into()),
        };
        if io::is_symlink_or_reparse(&metadata) {
            let target = tokio::fs::read_link(&absolute).await?;
            let target = target
                .to_str()
                .ok_or_else(|| crate::ErrorKind::UTFError(absolute.clone()))?;
            entries.insert(
                relative.clone(),
                RestoreEntry {
                    path: relative,
                    kind: EntryKind::Symlink,
                    object_hash: None,
                    size: 0,
                    unix_mode: unix_mode(&metadata).map(i64::from),
                    link_target: Some(target.to_string()),
                    link_is_directory: Some(
                        tokio::fs::metadata(&absolute)
                            .await
                            .is_ok_and(|metadata| metadata.is_dir()),
                    ),
                },
            );
            continue;
        }
        if metadata.is_dir() {
            entries.insert(
                relative.clone(),
                RestoreEntry {
                    path: relative.clone(),
                    kind: EntryKind::Directory,
                    object_hash: None,
                    size: 0,
                    unix_mode: unix_mode(&metadata).map(i64::from),
                    link_target: None,
                    link_is_directory: None,
                },
            );
            let mut children = tokio::fs::read_dir(&absolute).await?;
            while let Some(child) = children.next_entry().await? {
                let name =
                    child.file_name().to_str().map(str::to_string).ok_or_else(
                        || crate::ErrorKind::UTFError(child.path()),
                    )?;
                pending.push((child.path(), format!("{relative}/{name}")));
            }
            continue;
        }
        if !metadata.is_file() {
            return Err(crate::ErrorKind::FSError(format!(
                "Unsupported instance entry: {}",
                absolute.display()
            ))
            .into());
        }
        if let Some(progress) = progress {
            emit_backup_progress(
                progress.operation_id,
                Some(&progress.instance_id),
                InstanceBackupOperationType::RestorePreview,
                InstanceBackupProgressStage::Hashing,
                processed_bytes,
                progress.total_bytes,
                None,
                None,
                None,
            )
            .await;
        }
        let (hash, size) = hash_restore_file(
            &absolute,
            &metadata,
            progress.map(|progress| &progress.cancellation),
        )
        .await?;
        processed_bytes = processed_bytes.saturating_add(size);
        entries.insert(
            relative.clone(),
            RestoreEntry {
                path: relative,
                kind: EntryKind::File,
                object_hash: Some(hash),
                size,
                unix_mode: unix_mode(&metadata).map(i64::from),
                link_target: None,
                link_is_directory: None,
            },
        );
    }
    Ok(entries)
}

async fn hash_restore_file(
    path: &Path,
    before: &std::fs::Metadata,
    cancellation: Option<&CancellationToken>,
) -> crate::Result<(String, u64)> {
    let mut file = tokio::fs::File::open(path).await.map_err(|error| {
        crate::ErrorKind::FSError(format!(
            "Could not read {} while preparing the restore: {error}",
            path.display()
        ))
    })?;
    let mut hasher = Sha256::new();
    let mut size = 0u64;
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        if cancellation.is_some_and(CancellationToken::is_cancelled) {
            return Err(crate::ErrorKind::OtherError(
                "Restore preview was canceled".to_string(),
            )
            .into());
        }
        let read = file.read(&mut buffer).await.map_err(|error| {
            crate::ErrorKind::FSError(format!(
                "Could not read {} while preparing the restore: {error}",
                path.display()
            ))
        })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        size = size.saturating_add(read as u64);
    }
    let after = tokio::fs::metadata(path).await?;
    if before.len() != after.len()
        || before.modified().ok() != after.modified().ok()
    {
        return Err(crate::ErrorKind::FSError(format!(
            "File changed while preparing the restore: {}",
            path.display()
        ))
        .into());
    }
    Ok((
        hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
        size,
    ))
}

fn restore_entries_equal(
    current: Option<&RestoreEntry>,
    target: Option<&RestoreEntry>,
) -> bool {
    let (Some(current), Some(target)) = (current, target) else {
        return current.is_none() && target.is_none();
    };
    if current.kind != target.kind {
        return false;
    }
    match current.kind {
        EntryKind::Directory => true,
        EntryKind::File => {
            current.object_hash == target.object_hash
                && current.unix_mode == target.unix_mode
        }
        EntryKind::Symlink => {
            current.link_target == target.link_target
                && current.link_is_directory == target.link_is_directory
        }
    }
}

fn compare_restore_entries(
    snapshot_id: &str,
    current: BTreeMap<String, RestoreEntry>,
    target: BTreeMap<String, RestoreEntry>,
) -> RestorePlan {
    let paths = current
        .keys()
        .chain(target.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut added_files = 0u64;
    let mut modified_files = 0u64;
    let mut deleted_files = 0u64;
    let mut changed_paths = Vec::new();
    let mut token = Sha256::new();
    token.update(snapshot_id.as_bytes());
    for path in paths {
        let current_entry = current.get(&path);
        let target_entry = target.get(&path);
        update_restore_plan_token(&mut token, &path, current_entry);
        update_restore_plan_token(&mut token, &path, target_entry);
        if restore_entries_equal(current_entry, target_entry) {
            continue;
        }
        changed_paths.push(path);
        match (current_entry, target_entry) {
            (None, Some(target)) if target.kind != EntryKind::Directory => {
                added_files += 1;
            }
            (Some(current), None) if current.kind != EntryKind::Directory => {
                deleted_files += 1;
            }
            (Some(current), Some(target))
                if current.kind != EntryKind::Directory
                    || target.kind != EntryKind::Directory =>
            {
                modified_files += 1;
            }
            _ => {}
        }
    }
    changed_paths.sort_by(|left, right| {
        left.matches('/')
            .count()
            .cmp(&right.matches('/').count())
            .then_with(|| left.cmp(right))
    });
    let mut roots: Vec<RestoreJournalRoot> = Vec::new();
    for path in changed_paths {
        if roots.iter().any(|root| {
            path == root.path || path.starts_with(&format!("{}/", root.path))
        }) {
            continue;
        }
        let had_current = current.keys().any(|current_path| {
            current_path == &path
                || current_path.starts_with(&format!("{path}/"))
        });
        roots.push(RestoreJournalRoot { path, had_current });
    }
    RestorePlan {
        preview: BackupRestorePreview {
            added_files,
            modified_files,
            deleted_files,
            plan_token: token
                .finalize()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect(),
        },
        roots,
        target_entries: target.into_values().collect(),
    }
}

fn update_restore_plan_token(
    token: &mut Sha256,
    path: &str,
    entry: Option<&RestoreEntry>,
) {
    token.update(path.as_bytes());
    token.update([0]);
    let Some(entry) = entry else {
        token.update(b"missing\0");
        return;
    };
    token.update(entry.kind.as_str().as_bytes());
    token.update([0]);
    token.update(entry.object_hash.as_deref().unwrap_or("").as_bytes());
    token.update([0]);
    token.update(entry.link_target.as_deref().unwrap_or("").as_bytes());
    token.update([0]);
    token.update(entry.size.to_le_bytes());
    token.update(entry.unix_mode.unwrap_or_default().to_le_bytes());
    token.update([u8::from(entry.link_is_directory.unwrap_or(false))]);
}

fn restore_path_is_selected(path: &str, roots: &[RestoreJournalRoot]) -> bool {
    roots.iter().any(|root| {
        path == root.path || path.starts_with(&format!("{}/", root.path))
    })
}

async fn materialize_restore_plan(
    repository: &Path,
    staged: &Path,
    entries: &[RestoreEntry],
    roots: &[RestoreJournalRoot],
) -> crate::Result<()> {
    for entry in entries {
        if !restore_path_is_selected(&entry.path, roots) {
            continue;
        }
        let destination = join_relative(staged, &entry.path);
        match entry.kind {
            EntryKind::Directory => io::create_dir_all(&destination).await?,
            EntryKind::File => {
                let hash = entry.object_hash.as_deref().ok_or_else(|| {
                    crate::ErrorKind::FSError(format!(
                        "Backup file {} has no object",
                        entry.path
                    ))
                })?;
                verify_object(repository, hash, entry.size).await?;
                if let Some(parent) = destination.parent() {
                    io::create_dir_all(parent).await?;
                }
                tokio::fs::copy(object_path(repository, hash), &destination)
                    .await?;
                apply_unix_mode(&destination, entry.unix_mode).await?;
            }
            EntryKind::Symlink => {
                if let Some(parent) = destination.parent() {
                    io::create_dir_all(parent).await?;
                }
                let target = entry.link_target.as_deref().ok_or_else(|| {
                    crate::ErrorKind::FSError(format!(
                        "Backup link {} has no target",
                        entry.path
                    ))
                })?;
                create_recorded_symlink(
                    PathBuf::from(target),
                    destination,
                    entry.link_is_directory.unwrap_or(false),
                )
                .await?;
            }
        }
    }
    Ok(())
}

async fn apply_restore_roots(
    root: &Path,
    operation_root: &Path,
    roots: &[RestoreJournalRoot],
) -> crate::Result<()> {
    let staged = operation_root.join("staged");
    let rollback = operation_root.join("rollback");
    for journal_root in roots {
        let current = join_relative(root, &journal_root.path);
        let old = join_relative(&rollback, &journal_root.path);
        if tokio::fs::symlink_metadata(&current).await.is_ok() {
            if let Some(parent) = old.parent() {
                io::create_dir_all(parent).await?;
            }
            rename_backup_path(&current, &old).await?;
        }
        let new = join_relative(&staged, &journal_root.path);
        if tokio::fs::symlink_metadata(&new).await.is_ok() {
            if let Some(parent) = current.parent() {
                io::create_dir_all(parent).await?;
            }
            rename_backup_path(&new, &current).await?;
        }
    }
    Ok(())
}

async fn restore_snapshot_inner(
    snapshot_id: &str,
    expected_plan_token: &str,
    cancellable: &AtomicBool,
    operation_id: Uuid,
) -> crate::Result<()> {
    let state = State::get().await?;
    let (instance_id, _) = snapshot_operation_metadata(snapshot_id).await?;
    let _maintenance_guard = lock_instance_maintenance(&instance_id).await;
    let _instance_guard =
        state.lock_instance_content_exclusive(&instance_id).await;
    if state.process_manager.has_instance_process(&instance_id) {
        return Err(crate::ErrorKind::InputError(
            "Close the instance before restoring a backup".to_string(),
        )
        .into());
    }
    let _repository_guard = REPOSITORY_LOCK.lock().await;
    let Some((repository, pool)) = open_repository(&state, false).await? else {
        return Err(crate::ErrorKind::InputError(
            "Unknown backup snapshot".to_string(),
        )
        .into());
    };
    let root = require_eligible(&instance_id, &state).await?;
    let plan =
        build_restore_plan(snapshot_id, &instance_id, &root, &pool, None)
            .await?;
    if plan.preview.plan_token != expected_plan_token {
        pool.close().await;
        return Err(crate::ErrorKind::InputError(
            "The instance changed after the restore preview; review the updated operations before restoring"
                .to_string(),
        )
        .into());
    }
    if plan.roots.is_empty() {
        pool.close().await;
        return Ok(());
    }
    if !cancellable.swap(false, Ordering::AcqRel) {
        pool.close().await;
        return Err(crate::ErrorKind::OtherError(
            "Backup restore was canceled".to_string(),
        )
        .into());
    }
    update_persisted_operation(
        operation_id,
        BackupOperationState::Materializing,
        Some(0),
        None,
        Some(false),
        None,
        Some(snapshot_id),
    )
    .await;

    let restore_journal_id = Uuid::new_v4().to_string();
    let operation_root = state
        .directories
        .instances_dir()
        .join(format!("{RESTORE_PREFIX}{restore_journal_id}"));
    let staged = operation_root.join("staged");
    let rollback = operation_root.join("rollback");
    io::create_dir_all(&staged).await?;
    io::create_dir_all(&rollback).await?;
    let journal = RestoreJournal {
        instance_id: instance_id.clone(),
        roots: plan.roots.clone(),
        whole_root: false,
        exclusions: Vec::new(),
    };
    let mut journal_file =
        tokio::fs::File::create(operation_root.join(RESTORE_JOURNAL)).await?;
    journal_file
        .write_all(&serde_json::to_vec(&journal)?)
        .await?;
    journal_file.sync_all().await?;
    drop(journal_file);
    let materialize = materialize_restore_plan(
        &repository,
        &staged,
        &plan.target_entries,
        &plan.roots,
    )
    .await;
    if let Err(error) = materialize {
        let _ = io::remove_dir_all(&operation_root).await;
        pool.close().await;
        return Err(error);
    }
    update_persisted_operation(
        operation_id,
        BackupOperationState::Applying,
        None,
        None,
        Some(false),
        None,
        Some(snapshot_id),
    )
    .await;

    let replace_result =
        apply_restore_roots(&root, &operation_root, &plan.roots).await;
    if let Err(error) = replace_result {
        let recovery =
            recover_restore_operation(&root, &operation_root, &journal.roots)
                .await;
        pool.close().await;
        if let Err(recovery_error) = recovery {
            return Err(crate::ErrorKind::OtherError(format!(
				"Backup restore failed: {error}; rollback failed: {recovery_error}"
			))
            .into());
        }
        return Err(error);
    }
    if let Err(error) = mark_restore_committed(&operation_root).await {
        let recovery =
            recover_restore_operation(&root, &operation_root, &journal.roots)
                .await;
        pool.close().await;
        if let Err(recovery_error) = recovery {
            return Err(crate::ErrorKind::OtherError(format!(
				"Backup restore could not be committed: {error}; rollback failed: {recovery_error}"
			))
			.into());
        }
        return Err(error);
    }
    io::remove_dir_all(&operation_root).await?;
    pool.close().await;
    Ok(())
}

#[cfg(test)]
async fn replace_whole_root(
    root: &Path,
    operation_root: &Path,
    exclusions: &[BackupExclusion],
) -> crate::Result<()> {
    ensure_canonical_exclusions(exclusions)?;
    let staged = operation_root.join("staged");
    let rollback_root = operation_root.join("rollback").join("instance");
    rename_backup_path(root, &rollback_root).await?;
    for exclusion in exclusions {
        let source = join_relative(&rollback_root, &exclusion.path);
        if tokio::fs::symlink_metadata(&source).await.is_err() {
            continue;
        }
        let destination = join_relative(&staged, &exclusion.path);
        remove_path_if_exists(&destination).await;
        if let Some(parent) = destination.parent() {
            io::create_dir_all(parent).await?;
        }
        rename_backup_path(&source, &destination).await?;
    }
    if let Some(parent) = root.parent() {
        io::create_dir_all(parent).await?;
    }
    rename_backup_path(&staged, root).await?;
    Ok(())
}

async fn verify_object(
    repository: &Path,
    expected_hash: &str,
    expected_size: u64,
) -> crate::Result<()> {
    if expected_hash.len() != 64
        || !expected_hash.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(crate::ErrorKind::FSError(
            "Backup object has an invalid SHA-256 identifier".to_string(),
        )
        .into());
    }
    let path = object_path(repository, expected_hash);
    let metadata = tokio::fs::metadata(&path).await?;
    if metadata.len() != expected_size {
        return Err(crate::ErrorKind::FSError(format!(
            "Backup object {expected_hash} has an unexpected size"
        ))
        .into());
    }
    let mut file = tokio::fs::File::open(&path).await?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        let read = file.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let actual = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    if actual != expected_hash {
        return Err(crate::ErrorKind::FSError(format!(
            "Backup object {expected_hash} failed SHA-256 verification"
        ))
        .into());
    }
    Ok(())
}

async fn create_recorded_symlink(
    target: PathBuf,
    link: PathBuf,
    is_directory: bool,
) -> crate::Result<()> {
    tokio::task::spawn_blocking(move || {
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(target, link)
        }
        #[cfg(windows)]
        {
            if is_directory {
                symlink_rs::symlink_dir(target, link)
            } else {
                symlink_rs::symlink_file(target, link)
            }
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = (target, link, is_directory);
            Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "Symbolic links are unsupported on this platform",
            ))
        }
    })
    .await
    .map_err(|error| {
        crate::ErrorKind::OtherError(format!(
            "Symbolic link restore task failed: {error}"
        ))
    })??;
    Ok(())
}

#[cfg(unix)]
async fn apply_unix_mode(path: &Path, mode: Option<i64>) -> crate::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    if let Some(mode) = mode {
        tokio::fs::set_permissions(
            path,
            std::fs::Permissions::from_mode(mode as u32),
        )
        .await?;
    }
    Ok(())
}

#[cfg(not(unix))]
async fn apply_unix_mode(
    _path: &Path,
    _mode: Option<i64>,
) -> crate::Result<()> {
    Ok(())
}

pub(crate) async fn move_default_repository_for_launcher_directory(
    source: &Path,
    destination: &Path,
) -> crate::Result<Option<PathBuf>> {
    if source == destination {
        return Ok(None);
    }
    let source_exists = source.join(DATABASE_FILE).try_exists()?;
    let destination_exists = destination.join(DATABASE_FILE).try_exists()?;
    if destination_exists {
        validate_repository_at(destination).await?;
        if source_exists && source.try_exists()? {
            return match io::remove_dir_all(source).await {
                Ok(()) => Ok(None),
                Err(error) => {
                    tracing::warn!(
                        path = %source.display(),
                        %error,
                        "Failed to remove the old default backup repository"
                    );
                    Ok(Some(source.to_path_buf()))
                }
            };
        }
        return Ok(None);
    }
    if !source_exists {
        return Ok(None);
    }
    if destination.try_exists()? {
        let mut entries = tokio::fs::read_dir(destination).await?;
        if entries.next_entry().await?.is_some() {
            return Err(crate::ErrorKind::DirectoryMoveError(format!(
                "The default backup repository destination is not empty: {}",
                destination.display()
            ))
            .into());
        }
    } else {
        io::create_dir_all(destination).await?;
    }
    let destination_parent = destination.parent().ok_or_else(|| {
        crate::ErrorKind::DirectoryMoveError(format!(
            "The default backup repository destination has no parent: {}",
            destination.display()
        ))
    })?;
    io::create_dir_all(destination_parent).await?;
    let pool = open_repository_at(source, false).await?.ok_or_else(|| {
        crate::ErrorKind::FSError(
            "The default backup repository disappeared during migration"
                .to_string(),
        )
    })?;
    sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(&pool)
        .await?;
    pool.close().await;

    if io::is_same_disk(source, destination_parent).unwrap_or(false) {
        tokio::fs::remove_dir(destination).await?;
        rename_backup_path(source, destination).await?;
        return Ok(None);
    }

    let temporary = destination_parent
        .join(format!(".axolotl-backup-move-{}", Uuid::new_v4()));
    let copy_result: crate::Result<()> = async {
        io::copy_dir(source, &temporary).await?;
        validate_repository_at(&temporary).await?;
        tokio::fs::remove_dir(destination).await?;
        rename_backup_path(&temporary, destination).await?;
        Ok(())
    }
    .await;
    if let Err(error) = copy_result {
        let _ = io::remove_dir_all(&temporary).await;
        if !destination.try_exists()? {
            let _ = io::create_dir_all(destination).await;
        }
        return Err(error);
    }
    match io::remove_dir_all(source).await {
        Ok(()) => Ok(None),
        Err(error) => {
            tracing::warn!(
                path = %source.display(),
                %error,
                "Failed to remove the old default backup repository"
            );
            Ok(Some(source.to_path_buf()))
        }
    }
}

pub async fn start_repository_move(
    destination: PathBuf,
) -> crate::Result<Uuid> {
    let operation_id = Uuid::new_v4();
    let total_bytes = repository_status()
        .await
        .map(|status| status.stored_size)
        .unwrap_or(0);
    create_persisted_operation(
        operation_id,
        REPOSITORY_OPERATION_KEY,
        BackupOperationType::RepositoryMove,
        None,
    )
    .await?;
    update_persisted_operation(
        operation_id,
        BackupOperationState::Queued,
        Some(0),
        Some(total_bytes),
        Some(false),
        None,
        None,
    )
    .await;
    tokio::spawn(async move {
        emit_backup_progress(
            operation_id,
            None,
            InstanceBackupOperationType::RepositoryMove,
            InstanceBackupProgressStage::Copying,
            0,
            total_bytes,
            None,
            None,
            None,
        )
        .await;
        let result = move_repository_inner(destination).await;
        let (stage, final_state, processed_bytes, message, state) = match result
        {
            Ok(()) => (
                InstanceBackupProgressStage::Completed,
                InstanceBackupOperationFinalState::Completed,
                total_bytes,
                None,
                BackupOperationState::Completed,
            ),
            Err(error) => (
                InstanceBackupProgressStage::Failed,
                InstanceBackupOperationFinalState::Failed,
                0,
                Some(error.to_string()),
                BackupOperationState::Failed,
            ),
        };
        update_persisted_operation(
            operation_id,
            state,
            Some(processed_bytes),
            Some(total_bytes),
            Some(false),
            message.as_deref(),
            None,
        )
        .await;
        emit_backup_progress(
            operation_id,
            None,
            InstanceBackupOperationType::RepositoryMove,
            stage,
            processed_bytes,
            total_bytes,
            Some(final_state),
            None,
            message,
        )
        .await;
        unregister_backup_operation(REPOSITORY_OPERATION_KEY, operation_id);
    });
    Ok(operation_id)
}

async fn move_repository_inner(destination: PathBuf) -> crate::Result<()> {
    let state = State::get().await?;
    let _repository_guard = REPOSITORY_LOCK.lock().await;
    let source = repository_path(&state).await?;
    if !destination.is_absolute() {
        return Err(crate::ErrorKind::InputError(
            "The backup repository destination must be an absolute path"
                .to_string(),
        )
        .into());
    }
    if source == destination {
        return Ok(());
    }
    let destination_existed = destination.try_exists()?;
    if destination_existed {
        let mut entries = tokio::fs::read_dir(&destination).await?;
        if entries.next_entry().await?.is_some() {
            return Err(crate::ErrorKind::InputError(
                "The new backup repository directory must be empty".to_string(),
            )
            .into());
        }
    } else {
        io::create_dir_all(&destination).await?;
    }
    let source_exists = source.join(DATABASE_FILE).try_exists()?;
    let mut same_disk_move = false;
    if source_exists {
        let destination_parent = destination.parent().ok_or_else(|| {
            crate::ErrorKind::InputError(
                "The backup repository destination has no parent".to_string(),
            )
        })?;
        io::create_dir_all(destination_parent).await?;
        let canonical_source = io::canonicalize(&source)?;
        let canonical_destination_parent =
            io::canonicalize(destination_parent)?;
        if canonical_destination_parent.starts_with(&canonical_source) {
            return Err(crate::ErrorKind::InputError(
                "The backup repository cannot be moved inside itself"
                    .to_string(),
            )
            .into());
        }
        if let Some((_, pool)) = open_repository(&state, false).await? {
            sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
                .execute(&pool)
                .await?;
            pool.close().await;
        }
        same_disk_move =
            io::is_same_disk(&source, destination_parent).unwrap_or(false);
        if same_disk_move {
            let _ = tokio::fs::remove_dir(&destination).await;
            rename_backup_path(&source, &destination).await?;
        } else {
            let temporary = destination_parent
                .join(format!(".axolotl-backup-move-{}", Uuid::new_v4()));
            let copy_result: crate::Result<()> = async {
                io::copy_dir(&source, &temporary).await?;
                validate_repository_at(&temporary).await?;
                tokio::fs::remove_dir(&destination).await?;
                rename_backup_path(&temporary, &destination).await?;
                Ok(())
            }
            .await;
            if let Err(error) = copy_result {
                let _ = io::remove_dir_all(&temporary).await;
                if !destination.try_exists()? {
                    let _ = io::create_dir_all(&destination).await;
                }
                return Err(error);
            }
        }
    }
    let stored_path = if destination == default_repository_path(&state) {
        None
    } else {
        Some(destination.to_string_lossy().to_string())
    };
    let update_result: crate::Result<()> = async {
        let mut tx = state.pool.begin().await?;
        sqlx::query(
            "UPDATE settings SET backup_repository_path = ? WHERE id = 0",
        )
        .bind(stored_path)
        .execute(&mut *tx)
        .await?;
        if source_exists && !same_disk_move {
            sqlx::query(
                "INSERT INTO pending_backup_repository_cleanups (path, created_at)
                 VALUES (?, ?)
                 ON CONFLICT(path) DO UPDATE SET created_at = excluded.created_at",
            )
            .bind(source.to_string_lossy().to_string())
            .bind(Utc::now().timestamp_millis())
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }
    .await;
    if let Err(error) = update_result {
        if source_exists && same_disk_move {
            let _ = rename_backup_path(&destination, &source).await;
            if destination_existed {
                let _ = io::create_dir_all(&destination).await;
            }
        } else if source_exists {
            let _ = io::remove_dir_all(&destination).await;
            if destination_existed {
                let _ = io::create_dir_all(&destination).await;
            }
        }
        return Err(error);
    }
    if source_exists && !same_disk_move {
        let cleanup_succeeded = if source.try_exists()? {
            match io::remove_dir_all(&source).await {
                Ok(()) => true,
                Err(error) => {
                    tracing::warn!(path = %source.display(), %error, "Failed to remove old backup repository");
                    false
                }
            }
        } else {
            true
        };
        if cleanup_succeeded {
            sqlx::query(
                "DELETE FROM pending_backup_repository_cleanups WHERE path = ?",
            )
            .bind(source.to_string_lossy().to_string())
            .execute(&state.pool)
            .await?;
        }
    }
    Ok(())
}

async fn validate_repository_at(root: &Path) -> crate::Result<()> {
    let options = SqliteConnectOptions::from_str(&format!(
        "sqlite://{}",
        root.join(DATABASE_FILE)
            .to_string_lossy()
            .replace('\\', "/")
    ))?
    .read_only(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await?;
    let integrity: String = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_one(&pool)
        .await?;
    let foreign_key_errors: Vec<(String, i64, String, i64)> =
        sqlx::query_as("PRAGMA foreign_key_check")
            .fetch_all(&pool)
            .await?;
    let schema_name: String = sqlx::query_scalar(
        "SELECT schema_name FROM repository_meta WHERE id = 0",
    )
    .fetch_one(&pool)
    .await?;
    let objects: Vec<(String, i64)> = sqlx::query_as(
        "SELECT hash, size FROM backup_objects WHERE ref_count > 0",
    )
    .fetch_all(&pool)
    .await?;
    pool.close().await;
    if integrity != "ok" {
        return Err(crate::ErrorKind::FSError(format!(
            "Moved backup database failed its integrity check: {integrity}"
        ))
        .into());
    }
    if !foreign_key_errors.is_empty() {
        return Err(crate::ErrorKind::FSError(
            "Moved backup database failed its foreign key check".to_string(),
        )
        .into());
    }
    if schema_name != "axolotl-instance-backups" {
        return Err(crate::ErrorKind::FSError(
            "Moved directory is not an Axolotl backup repository".to_string(),
        )
        .into());
    }
    for (hash, size) in objects {
        verify_object(root, &hash, size.max(0) as u64).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn committed_restore_housekeeping_leaves_unfinished_restores_and_instances_untouched()
     {
        let directory = tempfile::tempdir().unwrap();
        let committed =
            directory.path().join(format!("{RESTORE_PREFIX}complete"));
        let pending = directory.path().join(format!("{RESTORE_PREFIX}pending"));
        let instance = directory.path().join("normal-instance");
        for root in [&committed, &pending, &instance] {
            tokio::fs::create_dir_all(root).await.unwrap();
            tokio::fs::write(root.join("data"), b"preserved")
                .await
                .unwrap();
        }
        tokio::fs::write(committed.join(RESTORE_COMMITTED), b"")
            .await
            .unwrap();
        tokio::fs::write(instance.join(RESTORE_COMMITTED), b"")
            .await
            .unwrap();
        cleanup_committed_restores(directory.path()).await.unwrap();
        assert!(!committed.exists());
        assert!(pending.join("data").exists());
        assert!(instance.join("data").exists());
    }

    fn restore_directory(path: &str) -> RestoreEntry {
        RestoreEntry {
            path: path.to_string(),
            kind: EntryKind::Directory,
            object_hash: None,
            size: 0,
            unix_mode: None,
            link_target: None,
            link_is_directory: None,
        }
    }

    fn restore_file(path: &str, hash: &str) -> RestoreEntry {
        RestoreEntry {
            path: path.to_string(),
            kind: EntryKind::File,
            object_hash: Some(hash.to_string()),
            size: 1,
            unix_mode: None,
            link_target: None,
            link_is_directory: None,
        }
    }

    #[test]
    fn restore_plan_counts_file_changes_and_keeps_minimal_roots() {
        let current = BTreeMap::from([
            ("config".to_string(), restore_directory("config")),
            (
                "config/delete.txt".to_string(),
                restore_file("config/delete.txt", "delete"),
            ),
            (
                "config/modify.txt".to_string(),
                restore_file("config/modify.txt", "old"),
            ),
            (
                "config/same.txt".to_string(),
                restore_file("config/same.txt", "same"),
            ),
        ]);
        let target = BTreeMap::from([
            ("config".to_string(), restore_directory("config")),
            (
                "config/add.txt".to_string(),
                restore_file("config/add.txt", "add"),
            ),
            (
                "config/modify.txt".to_string(),
                restore_file("config/modify.txt", "new"),
            ),
            (
                "config/same.txt".to_string(),
                restore_file("config/same.txt", "same"),
            ),
        ]);

        let plan = compare_restore_entries("snapshot", current, target);

        assert_eq!(plan.preview.added_files, 1);
        assert_eq!(plan.preview.modified_files, 1);
        assert_eq!(plan.preview.deleted_files, 1);
        assert_eq!(
            plan.roots
                .iter()
                .map(|root| root.path.as_str())
                .collect::<Vec<_>>(),
            ["config/add.txt", "config/delete.txt", "config/modify.txt"]
        );
    }

    #[test]
    fn restore_plan_collapses_descendants_when_entry_type_changes() {
        let current = BTreeMap::from([
            ("cache".to_string(), restore_directory("cache")),
            (
                "cache/data.bin".to_string(),
                restore_file("cache/data.bin", "old"),
            ),
        ]);
        let target = BTreeMap::from([(
            "cache".to_string(),
            restore_file("cache", "new"),
        )]);

        let plan = compare_restore_entries("snapshot", current, target);

        assert_eq!(plan.preview.modified_files, 1);
        assert_eq!(plan.preview.deleted_files, 1);
        assert_eq!(plan.roots.len(), 1);
        assert_eq!(plan.roots[0].path, "cache");
        assert!(plan.roots[0].had_current);
    }

    #[tokio::test]
    async fn restore_scan_hashes_files_and_skips_exclusions() {
        let temp = tempfile::tempdir().unwrap();
        tokio::fs::create_dir_all(temp.path().join("config"))
            .await
            .unwrap();
        tokio::fs::create_dir_all(temp.path().join("caches"))
            .await
            .unwrap();
        tokio::fs::write(temp.path().join("config/value.txt"), b"value")
            .await
            .unwrap();
        tokio::fs::write(temp.path().join("caches/value.bin"), b"ignored")
            .await
            .unwrap();

        let entries = scan_restore_entries(
            temp.path(),
            true,
            &[],
            &[BackupExclusion {
                path: "caches".to_string(),
                kind: BackupExclusionKind::Directory,
            }],
            None,
        )
        .await
        .unwrap();

        assert!(entries.contains_key("config"));
        assert!(entries.contains_key("config/value.txt"));
        assert!(!entries.contains_key("caches"));
        assert_eq!(
            entries["config/value.txt"].object_hash.as_deref(),
            Some(
                "cd42404d52ad55ccfa9aca4adc828aa5800ad9d385a0671fbcbf724118320619"
            )
        );
    }

    #[tokio::test]
    async fn incremental_restore_replaces_only_changed_paths() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("instance");
        let operation_root = temp.path().join("operation");
        tokio::fs::create_dir_all(root.join("config"))
            .await
            .unwrap();
        tokio::fs::create_dir_all(operation_root.join("staged/config"))
            .await
            .unwrap();
        tokio::fs::create_dir_all(operation_root.join("rollback"))
            .await
            .unwrap();
        tokio::fs::write(root.join("config/unchanged.txt"), b"same")
            .await
            .unwrap();
        tokio::fs::write(root.join("config/changed.txt"), b"current")
            .await
            .unwrap();
        tokio::fs::write(
            operation_root.join("staged/config/changed.txt"),
            b"backup",
        )
        .await
        .unwrap();

        apply_restore_roots(
            &root,
            &operation_root,
            &[RestoreJournalRoot {
                path: "config/changed.txt".to_string(),
                had_current: true,
            }],
        )
        .await
        .unwrap();

        assert_eq!(
            tokio::fs::read(root.join("config/unchanged.txt"))
                .await
                .unwrap(),
            b"same"
        );
        assert_eq!(
            tokio::fs::read(root.join("config/changed.txt"))
                .await
                .unwrap(),
            b"backup"
        );
        assert_eq!(
            tokio::fs::read(operation_root.join("rollback/config/changed.txt"))
                .await
                .unwrap(),
            b"current"
        );
    }

    #[tokio::test]
    async fn cancellation_is_rejected_after_snapshot_saving_starts() {
        let operation_id = Uuid::new_v4();
        let (cancellation, _cancellable) =
            register_backup_operation(operation_id, "instance");
        assert!(cancel_backup(operation_id).await.unwrap());
        assert!(cancellation.is_cancelled());
        BACKUP_CANCELLATIONS.remove(&operation_id);

        let operation_id = Uuid::new_v4();
        let (cancellation, cancellable) =
            register_backup_operation(operation_id, "instance");
        cancellable.store(false, Ordering::Release);
        assert!(!cancel_backup(operation_id).await.unwrap());
        assert!(!cancellation.is_cancelled());
        BACKUP_CANCELLATIONS.remove(&operation_id);
    }

    #[test]
    fn only_one_operation_can_reserve_an_instance() {
        let instance_id = format!("instance-{}", Uuid::new_v4());
        let first = Uuid::new_v4();
        assert!(reserve_instance_operation(&instance_id, first).is_ok());
        assert!(
            reserve_instance_operation(&instance_id, Uuid::new_v4()).is_err()
        );
        assert!(has_active_instance_operation(&instance_id));
        ACTIVE_INSTANCE_OPERATIONS.remove(&instance_id);
        assert!(!has_active_instance_operation(&instance_id));
    }

    #[test]
    fn normalizes_and_deduplicates_exclusions() {
        assert_eq!(
            canonicalize_exclusions(vec![
                BackupExclusion {
                    path: "saves/world".to_string(),
                    kind: BackupExclusionKind::Directory,
                },
                BackupExclusion {
                    path: "config\\bigdb.sqlite".to_string(),
                    kind: BackupExclusionKind::File,
                },
                BackupExclusion {
                    path: "saves".to_string(),
                    kind: BackupExclusionKind::Directory,
                },
            ])
            .unwrap(),
            [
                BackupExclusion {
                    path: "saves".to_string(),
                    kind: BackupExclusionKind::Directory,
                },
                BackupExclusion {
                    path: "config/bigdb.sqlite".to_string(),
                    kind: BackupExclusionKind::File,
                },
            ]
        );
    }

    #[test]
    fn rejects_paths_outside_the_instance() {
        for path in ["", ".", "..", "../saves", "/saves", "C:\\saves"] {
            assert!(normalize_relative_directory(path).is_err(), "{path}");
        }
    }

    #[tokio::test]
    async fn independent_repository_migration_creates_expected_schema() {
        let temp = tempfile::tempdir().unwrap();
        let database = temp.path().join(DATABASE_FILE);
        let options = SqliteConnectOptions::new()
            .filename(&database)
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        BACKUP_MIGRATOR.run(&pool).await.unwrap();
        let tables: Vec<String> = sqlx::query_scalar(
            "SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert!(tables.contains(&"backup_snapshots".to_string()));
        assert!(tables.contains(&"backup_exclusions".to_string()));
        assert!(tables.contains(&"snapshot_entries".to_string()));
        assert!(tables.contains(&"snapshot_exclusions".to_string()));
        assert!(tables.contains(&"backup_objects".to_string()));
        assert!(tables.contains(&"backup_operations".to_string()));
        assert!(!tables.contains(&"backup_selections".to_string()));
        let operation_columns: Vec<String> = sqlx::query_scalar(
			"SELECT name FROM pragma_table_info('backup_operations') ORDER BY cid",
		)
		.fetch_all(&pool)
		.await
		.unwrap();
        assert!(operation_columns.contains(&"cancellable".to_string()));
        assert!(operation_columns.contains(&"cancel_requested".to_string()));
        let foreign_key_errors: Vec<(String, i64, String, i64)> =
            sqlx::query_as("PRAGMA foreign_key_check")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert!(foreign_key_errors.is_empty());
    }

    #[tokio::test]
    async fn operation_cancellation_migration_preserves_existing_operations() {
        let temp = tempfile::tempdir().unwrap();
        let options = SqliteConnectOptions::new()
            .filename(temp.path().join(DATABASE_FILE))
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!(
            "../../../backup_migrations/20260920200000_operations.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO backup_operations
			 (id, operation_type, instance_id, state, created_at, updated_at)
			 VALUES ('operation', 'create', 'instance', 'hashing', 1, 1)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::raw_sql(include_str!(
			"../../../backup_migrations/20260920210000_operation_cancellation.sql"
		))
        .execute(&pool)
        .await
        .unwrap();

        let row: (String, i64) = sqlx::query_as(
			"SELECT state, cancel_requested FROM backup_operations WHERE id = 'operation'",
		)
		.fetch_one(&pool)
		.await
		.unwrap();
        assert_eq!(row, ("hashing".to_string(), 0));
        let foreign_key_errors: Vec<(String, i64, String, i64)> =
            sqlx::query_as("PRAGMA foreign_key_check")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert!(foreign_key_errors.is_empty());
    }

    #[tokio::test]
    async fn repository_move_migration_preserves_operations_and_extends_type() {
        let temp = tempfile::tempdir().unwrap();
        let options = SqliteConnectOptions::new()
            .filename(temp.path().join(DATABASE_FILE))
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!(
            "../../../backup_migrations/20260920200000_operations.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();
        sqlx::raw_sql(include_str!(
            "../../../backup_migrations/20260920210000_operation_cancellation.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO backup_operations
             (id, operation_type, instance_id, state, created_at, updated_at)
             VALUES ('create-operation', 'create', 'instance', 'completed', 1, 1)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::raw_sql(include_str!(
            "../../../backup_migrations/20260920223000_repository_move_operations.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();
        let existing: String = sqlx::query_scalar(
            "SELECT operation_type FROM backup_operations WHERE id = 'create-operation'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(existing, "create");
        sqlx::query(
            "INSERT INTO backup_operations
             (id, operation_type, instance_id, state, created_at, updated_at)
             VALUES ('move-operation', 'repository_move', ?, 'queued', 2, 2)",
        )
        .bind(REPOSITORY_OPERATION_KEY)
        .execute(&pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn restore_preview_migration_preserves_operations_and_result_columns()
    {
        let temp = tempfile::tempdir().unwrap();
        let options = SqliteConnectOptions::new()
            .filename(temp.path().join(DATABASE_FILE))
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        for migration in [
            include_str!(
                "../../../backup_migrations/20260920200000_operations.sql"
            ),
            include_str!(
                "../../../backup_migrations/20260920210000_operation_cancellation.sql"
            ),
            include_str!(
                "../../../backup_migrations/20260920223000_repository_move_operations.sql"
            ),
        ] {
            sqlx::raw_sql(migration).execute(&pool).await.unwrap();
        }
        sqlx::query(
            "INSERT INTO backup_operations
             (id, operation_type, instance_id, state, created_at, updated_at)
             VALUES ('existing', 'restore', 'instance', 'completed', 1, 1)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::raw_sql(include_str!(
            "../../../backup_migrations/20260920230000_restore_preview_operations.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();
        let existing: String = sqlx::query_scalar(
            "SELECT operation_type FROM backup_operations WHERE id = 'existing'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(existing, "restore");
        sqlx::query(
            "INSERT INTO backup_operations
             (id, operation_type, instance_id, state, created_at, updated_at,
              preview_added_files, preview_modified_files, preview_deleted_files,
              preview_plan_token)
             VALUES ('preview', 'restore_preview', 'instance', 'completed', 2, 2,
                     1, 2, 3, 'token')",
        )
        .execute(&pool)
        .await
        .unwrap();
        let result: (i64, i64, i64, String) = sqlx::query_as(
            "SELECT preview_added_files, preview_modified_files,
                    preview_deleted_files, preview_plan_token
             FROM backup_operations WHERE id = 'preview'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(result, (1, 2, 3, "token".to_string()));
    }

    #[tokio::test]
    async fn repository_validation_rejects_foreign_key_violations() {
        let temp = tempfile::tempdir().unwrap();
        tokio::fs::create_dir_all(temp.path().join(OBJECTS_DIR))
            .await
            .unwrap();
        let options = SqliteConnectOptions::new()
            .filename(temp.path().join(DATABASE_FILE))
            .create_if_missing(true)
            .foreign_keys(false);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        BACKUP_MIGRATOR.run(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO backup_snapshots
             (id, instance_id, instance_name, created_at, file_count,
              symlink_count, logical_size, added_size)
             VALUES ('snapshot', 'missing', 'Missing', 0, 0, 0, 0, 0)",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool.close().await;

        let error = validate_repository_at(temp.path()).await.unwrap_err();
        assert!(error.to_string().contains("foreign key check"));
    }

    #[tokio::test]
    async fn default_repository_follows_launcher_directory_move_idempotently() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("old/Backups/instances");
        let destination = temp.path().join("new/Backups/instances");
        let pool = open_repository_at(&source, true).await.unwrap().unwrap();
        pool.close().await;

        let pending = move_default_repository_for_launcher_directory(
            &source,
            &destination,
        )
        .await
        .unwrap();
        assert!(pending.is_none());
        assert!(!source.exists());
        validate_repository_at(&destination).await.unwrap();

        let repeated = move_default_repository_for_launcher_directory(
            &source,
            &destination,
        )
        .await
        .unwrap();
        assert!(repeated.is_none());
        validate_repository_at(&destination).await.unwrap();
    }

    #[tokio::test]
    async fn full_instance_scan_includes_future_content_and_honors_exclusions()
    {
        let temp = tempfile::tempdir().unwrap();
        let instance = temp.path().join("instance");
        let repository = temp.path().join("repository");
        let staging = repository.join(STAGING_DIR).join("first");
        tokio::fs::create_dir_all(instance.join("config"))
            .await
            .unwrap();
        tokio::fs::create_dir_all(instance.join("caches"))
            .await
            .unwrap();
        tokio::fs::create_dir_all(repository.join(OBJECTS_DIR))
            .await
            .unwrap();
        tokio::fs::create_dir_all(&staging).await.unwrap();
        tokio::fs::write(instance.join("config/first.toml"), b"same")
            .await
            .unwrap();
        tokio::fs::write(instance.join("config/bigdb.sqlite"), b"ignored")
            .await
            .unwrap();
        tokio::fs::write(instance.join("caches/data.bin"), b"ignored")
            .await
            .unwrap();
        tokio::fs::write(instance.join("options.txt"), b"root")
            .await
            .unwrap();
        let exclusions = [
            BackupExclusion {
                path: "caches".to_string(),
                kind: BackupExclusionKind::Directory,
            },
            BackupExclusion {
                path: "config/bigdb.sqlite".to_string(),
                kind: BackupExclusionKind::File,
            },
        ];

        let mut first_objects = HashSet::new();
        let (first_entries, first_added) = scan_snapshot(
            &instance,
            &repository,
            &staging,
            &exclusions,
            Uuid::new_v4(),
            "instance",
            0,
            &CancellationToken::new(),
            &mut first_objects,
        )
        .await
        .unwrap();
        assert_eq!(
            first_entries
                .iter()
                .filter(|entry| entry.kind == EntryKind::File)
                .count(),
            2
        );
        assert!(
            first_entries
                .iter()
                .any(|entry| entry.path == "options.txt")
        );
        assert!(
            !first_entries
                .iter()
                .any(|entry| entry.path.starts_with("caches"))
        );
        assert!(
            !first_entries
                .iter()
                .any(|entry| entry.path == "config/bigdb.sqlite")
        );
        assert_eq!(first_added, 8);

        tokio::fs::write(instance.join("config/second.toml"), b"new")
            .await
            .unwrap();
        tokio::fs::create_dir_all(instance.join("logs"))
            .await
            .unwrap();
        tokio::fs::write(instance.join("logs/latest.log"), b"future")
            .await
            .unwrap();
        let second_staging = repository.join(STAGING_DIR).join("second");
        tokio::fs::create_dir_all(&second_staging).await.unwrap();
        let mut second_objects = HashSet::new();
        let (second_entries, second_added) = scan_snapshot(
            &instance,
            &repository,
            &second_staging,
            &exclusions,
            Uuid::new_v4(),
            "instance",
            0,
            &CancellationToken::new(),
            &mut second_objects,
        )
        .await
        .unwrap();
        assert_eq!(
            second_entries
                .iter()
                .filter(|entry| entry.kind == EntryKind::File)
                .count(),
            4
        );
        assert!(
            second_entries
                .iter()
                .any(|entry| entry.path == "logs/latest.log")
        );
        assert_eq!(second_added, 9, "only new content should be stored");
        assert_eq!(second_objects.len(), 2);
    }

    #[tokio::test]
    async fn deleting_last_snapshot_reference_collects_the_object() {
        let temp = tempfile::tempdir().unwrap();
        let repository = temp.path();
        tokio::fs::create_dir_all(repository.join(OBJECTS_DIR))
            .await
            .unwrap();
        let options = SqliteConnectOptions::new()
            .filename(repository.join(DATABASE_FILE))
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        BACKUP_MIGRATOR.run(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO instance_backup_configs
             (instance_id, instance_name, created_at, modified_at)
             VALUES ('instance', 'Instance', 1, 1)",
        )
        .execute(&pool)
        .await
        .unwrap();
        let hash = "a".repeat(64);
        let object = object_path(repository, &hash);
        tokio::fs::create_dir_all(object.parent().unwrap())
            .await
            .unwrap();
        tokio::fs::write(&object, b"same").await.unwrap();
        sqlx::query(
            "INSERT INTO backup_objects (hash, size, ref_count)
             VALUES (?, 4, 2)",
        )
        .bind(&hash)
        .execute(&pool)
        .await
        .unwrap();
        for snapshot in ["first", "second"] {
            sqlx::query(
                "INSERT INTO backup_snapshots
                 (id, instance_id, instance_name, created_at, file_count,
                  symlink_count, logical_size, added_size)
                 VALUES (?, 'instance', 'Instance', 1, 1, 0, 4, 0)",
            )
            .bind(snapshot)
            .execute(&pool)
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO snapshot_entries
                 (snapshot_id, path, kind, object_hash, size)
                 VALUES (?, 'config/file', 'file', ?, 4)",
            )
            .bind(snapshot)
            .bind(&hash)
            .execute(&pool)
            .await
            .unwrap();
        }

        delete_snapshots_in_pool(repository, &pool, Some("first"), None)
            .await
            .unwrap();
        assert!(object.exists());
        let ref_count: i64 = sqlx::query_scalar(
            "SELECT ref_count FROM backup_objects WHERE hash = ?",
        )
        .bind(&hash)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(ref_count, 1);

        delete_snapshots_in_pool(repository, &pool, Some("second"), None)
            .await
            .unwrap();
        assert!(!object.exists());
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM backup_objects WHERE hash = ?)",
        )
        .bind(&hash)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(!exists);
    }

    #[tokio::test]
    async fn interrupted_restore_rolls_back_every_selected_root() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("instance");
        let operation_root = temp.path().join("operation");
        tokio::fs::create_dir_all(operation_root.join("rollback/config"))
            .await
            .unwrap();
        tokio::fs::create_dir_all(root.join("config"))
            .await
            .unwrap();
        tokio::fs::create_dir_all(root.join("saves")).await.unwrap();
        tokio::fs::write(
            operation_root.join("rollback/config/value.txt"),
            b"before",
        )
        .await
        .unwrap();
        tokio::fs::write(root.join("config/value.txt"), b"restored")
            .await
            .unwrap();
        tokio::fs::write(root.join("saves/new.txt"), b"restored")
            .await
            .unwrap();

        recover_restore_operation(
            &root,
            &operation_root,
            &[
                RestoreJournalRoot {
                    path: "config".to_string(),
                    had_current: true,
                },
                RestoreJournalRoot {
                    path: "saves".to_string(),
                    had_current: false,
                },
            ],
        )
        .await
        .unwrap();

        assert_eq!(
            tokio::fs::read(root.join("config/value.txt"))
                .await
                .unwrap(),
            b"before"
        );
        assert!(!root.join("saves").exists());
        assert!(!operation_root.exists());
    }

    #[tokio::test]
    async fn interrupted_whole_root_restore_preserves_excluded_content() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("instance");
        let operation_root = temp.path().join("operation");
        let rollback_root = operation_root.join("rollback/instance");
        tokio::fs::create_dir_all(rollback_root.join("config"))
            .await
            .unwrap();
        tokio::fs::create_dir_all(root.join("caches"))
            .await
            .unwrap();
        tokio::fs::write(rollback_root.join("config/value.txt"), b"before")
            .await
            .unwrap();
        tokio::fs::write(root.join("config.txt"), b"restored")
            .await
            .unwrap();
        tokio::fs::write(root.join("caches/data.bin"), b"preserved")
            .await
            .unwrap();

        recover_whole_root_restore(
            &root,
            &operation_root,
            &[BackupExclusion {
                path: "caches".to_string(),
                kind: BackupExclusionKind::Directory,
            }],
        )
        .await
        .unwrap();

        assert_eq!(
            tokio::fs::read(root.join("config/value.txt"))
                .await
                .unwrap(),
            b"before"
        );
        assert_eq!(
            tokio::fs::read(root.join("caches/data.bin")).await.unwrap(),
            b"preserved"
        );
        assert!(!root.join("config.txt").exists());
        assert!(!operation_root.exists());
    }

    #[tokio::test]
    async fn whole_root_restore_replaces_managed_content_and_keeps_exclusions()
    {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("instance");
        let operation_root = temp.path().join("operation");
        tokio::fs::create_dir_all(root.join("caches"))
            .await
            .unwrap();
        tokio::fs::create_dir_all(operation_root.join("staged/config"))
            .await
            .unwrap();
        tokio::fs::create_dir_all(operation_root.join("rollback"))
            .await
            .unwrap();
        tokio::fs::write(root.join("old.txt"), b"old")
            .await
            .unwrap();
        tokio::fs::write(root.join("caches/data.bin"), b"preserved")
            .await
            .unwrap();
        tokio::fs::write(
            operation_root.join("staged/config/value.txt"),
            b"snapshot",
        )
        .await
        .unwrap();

        replace_whole_root(
            &root,
            &operation_root,
            &[BackupExclusion {
                path: "caches".to_string(),
                kind: BackupExclusionKind::Directory,
            }],
        )
        .await
        .unwrap();

        assert_eq!(
            tokio::fs::read(root.join("config/value.txt"))
                .await
                .unwrap(),
            b"snapshot"
        );
        assert_eq!(
            tokio::fs::read(root.join("caches/data.bin")).await.unwrap(),
            b"preserved"
        );
        assert!(!root.join("old.txt").exists());
        assert_eq!(
            tokio::fs::read(operation_root.join("rollback/instance/old.txt"))
                .await
                .unwrap(),
            b"old"
        );
    }
}

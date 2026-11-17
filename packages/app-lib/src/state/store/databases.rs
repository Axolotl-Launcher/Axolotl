//! Stable, attached database domains. Application release numbers never decide
//! whether these files may be opened. The runtime core contract is additive;
//! the frozen historical baseline and SQLx migrations are import-only inputs.
//!
//! A durable main database and rollback journals are intentional: SQLite's
//! super-journal makes transactions spanning attached files atomic. Changing
//! these connections to WAL would lose that property.

use serde::Deserialize;
use sqlx::sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions,
};
use sqlx::{Connection, Executor, SqliteConnection, SqlitePool};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

const DOMAINS: &[&str] = &[
    "instances",
    "accounts",
    "sync",
    "media",
    "operations",
    "storage",
    "records",
    "telemetry",
];

#[derive(Deserialize)]
struct Baseline {
    tables: Vec<Table>,
    objects: Vec<Object>,
}

#[derive(Deserialize)]
struct Table {
    name: String,
    domain: String,
    sql: String,
    columns: Vec<String>,
    foreign_keys: Vec<ForeignKey>,
    seeds: Vec<String>,
}

#[derive(Deserialize)]
struct ForeignKey {
    parent: String,
    columns: Vec<ForeignColumn>,
    on_delete: String,
    on_update: String,
}

#[derive(Deserialize)]
struct ForeignColumn {
    child: String,
    parent: String,
}

#[derive(Deserialize)]
struct Object {
    kind: String,
    name: String,
    table: String,
    sql: String,
}

fn legacy_baseline() -> &'static Baseline {
    static VALUE: OnceLock<Baseline> = OnceLock::new();
    VALUE.get_or_init(|| {
        serde_json::from_str(include_str!("database_baseline.json"))
            .expect("the checked-in database baseline is valid")
    })
}

/// The runtime contract has no release gate and only relational authorities.
fn baseline() -> &'static Baseline {
    static VALUE: OnceLock<Baseline> = OnceLock::new();
    VALUE.get_or_init(|| {
        serde_json::from_str(include_str!("core_baseline.json"))
            .expect("the checked-in core contract is valid")
    })
}

fn quote(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

fn qualified(table: &Table) -> String {
    format!("{}.{}", quote(&table.domain), quote(&table.name))
}

pub(crate) fn directory(settings_dir: &Path) -> PathBuf {
    settings_dir.join("databases")
}

pub(crate) fn exists(settings_dir: &Path) -> bool {
    directory(settings_dir).join("system.db").is_file()
}

async fn attached_domains(
    connection: &mut SqliteConnection,
) -> Result<HashSet<String>, sqlx::Error> {
    Ok(
        sqlx::query_scalar::<_, String>(
            "SELECT name FROM pragma_database_list",
        )
        .fetch_all(connection)
        .await?
        .into_iter()
        .collect(),
    )
}

/// Collection cleanup cannot infer the references held by a newer protocol.
/// Preserve files until a reader that understands those records is available.
pub(crate) async fn ensure_cleanup_supported(
    pool: &SqlitePool,
) -> crate::Result<()> {
    super::recovery_mode::ensure_writable(pool)?;
    if pool
        .connect_options()
        .get_filename()
        .file_name()
        .is_none_or(|name| name != "system.db")
    {
        return Ok(());
    }
    if !super::compatibility::unsupported_instances(pool)
        .await?
        .is_empty()
    {
        return Err(crate::ErrorKind::InputError(
            "Cleanup is paused because some instances use a newer data format"
                .into(),
        )
        .into());
    }
    if super::compatibility::unknown_recovery_payloads(pool).await? {
        return Err(crate::ErrorKind::InputError("Cleanup is paused because a newer recovery protocol may retain files".into()).into());
    }
    let mut connection = pool.acquire().await?;
    let attached = attached_domains(&mut connection).await?;
    let file = pool.connect_options().get_filename().to_path_buf();
    let root = file
        .parent()
        .and_then(Path::parent)
        .expect("domain data root");
    super::legacy_documents::ensure_cleanup_supported(root).await?;
    super::recovery_documents::ensure_cleanup_supported(pool).await?;
    let shared = crate::state::sync_state::root(
        &crate::state::persistence::Persistence::new(root),
    )
    .await?;
    crate::state::sync_state::ensure_supported(&shared).await?;
    if DOMAINS
        .iter()
        .filter(|domain| {
            !matches!(
                **domain,
                "accounts"
                    | "telemetry"
                    | "media"
                    | "records"
                    | "sync"
                    | "operations"
            )
        })
        .any(|domain| !attached.contains(*domain))
    {
        return Err(crate::ErrorKind::InputError("Cleanup is paused until independent document references can be checked".into()).into());
    }
    for domain in attached {
        if !DOMAINS.contains(&domain.as_str()) {
            continue;
        }
        let count: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT COUNT(*) FROM {}.extension_records",
            quote(&domain)
        )))
        .fetch_one(pool)
        .await?;
        if count != 0 {
            return Err(crate::ErrorKind::InputError("Cleanup is paused because newer extension records may retain files".into()).into());
        }
    }
    Ok(())
}

pub(super) async fn cross_domain_issues(
    connection: &mut SqliteConnection,
) -> Result<usize, sqlx::Error> {
    let mut issues = 0;
    let attached = attached_domains(connection).await?;
    for table in &baseline().tables {
        if !attached.contains(&table.domain) {
            continue;
        }
        for foreign in &table.foreign_keys {
            let parent = baseline()
                .tables
                .iter()
                .find(|candidate| candidate.name == foreign.parent)
                .expect("foreign parent");
            if table.domain == parent.domain {
                continue;
            }
            if !attached.contains(&parent.domain) {
                continue;
            }
            let present = foreign
                .columns
                .iter()
                .map(|column| {
                    format!("child.{} IS NOT NULL", quote(&column.child))
                })
                .collect::<Vec<_>>()
                .join(" AND ");
            let matching = foreign
                .columns
                .iter()
                .map(|column| {
                    format!(
                        "parent.{} = child.{}",
                        quote(&column.parent),
                        quote(&column.child)
                    )
                })
                .collect::<Vec<_>>()
                .join(" AND ");
            let statement = format!(
                "SELECT COUNT(*) FROM {} AS child WHERE {present} AND NOT EXISTS(SELECT 1 FROM {} AS parent WHERE {matching})",
                qualified(table),
                qualified(parent)
            );
            let count: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(statement))
                .fetch_one(&mut *connection)
                .await?;
            issues += count as usize;
        }
    }
    Ok(issues)
}

/// Reserve every domain before copying its committed rollback-journal image.
/// The final directory appears only after the complete set has been synced.
pub(crate) async fn backup(
    pool: &SqlitePool,
    settings_dir: &Path,
) -> crate::Result<PathBuf> {
    backup_with_copy_hook(pool, settings_dir, || async {}).await
}

async fn backup_with_copy_hook<F, Fut>(
    pool: &SqlitePool,
    settings_dir: &Path,
    before_copy: F,
) -> crate::Result<PathBuf>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    let parent = crate::state::db_backup::app_db_backup_dir_for(
        &directory(settings_dir).join("system.db"),
    )?;
    tokio::fs::create_dir_all(&parent).await?;
    let id = uuid::Uuid::new_v4();
    let staging = parent.join(format!(".{id}.staging"));
    let destination = parent.join(id.to_string());
    tokio::fs::create_dir(&staging).await?;
    // Ordinary settings writers take this lock before writing their JSON.
    // Keep their documents and the attached SQL image in the same snapshot.
    // sched-order: theseus::api::settings::SETTINGS_WRITE
    let _settings = crate::api::settings::SETTINGS_WRITE.lock().await;
    let (mut transaction, transaction_diagnostics) = loop {
        super::configuration::flush_committed(pool).await?;
        let (mut transaction, transaction_diagnostics) =
            crate::state::store::lock_diagnostics::begin(
                pool,
                "BEGIN IMMEDIATE",
            )
            .await?;
        let pending: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM main.document_commits)",
        )
        .fetch_one(&mut *transaction)
        .await?;
        if !pending {
            break (transaction, transaction_diagnostics);
        }
        transaction_diagnostics
            .observe("rollback_wait", transaction.rollback())
            .await?;
        transaction_diagnostics.complete("rolled_back");
    };
    // Reserve core before document locks, matching ordinary coordinated writers.
    // sched-order: theseus::state::documents::WRITES
    let _documents = crate::state::documents::WRITES.lock().await;
    let _document_files =
        super::configuration_documents::lock_files(settings_dir).await?;
    let result = async {
        let mut exclusions = Vec::<String>::new();
        let domains = attached_domains(&mut *transaction).await?;
        for name in std::iter::once("system").chain(
            DOMAINS
                .iter()
                .copied()
                .filter(|name| domains.contains(*name)),
        ) {
            let file = format!("{name}.db");
            let target = staging.join(&file);
            tokio::fs::copy(directory(settings_dir).join(&file), &target)
                .await?;
            tokio::fs::OpenOptions::new()
                .write(true)
                .open(&target)
                .await?
                .sync_all()
                .await?;
        }
        let settings = settings_dir.join("settings");
        let target_settings = staging.join("settings");
        tokio::fs::create_dir(&target_settings).await?;
        if tokio::fs::try_exists(&settings).await? {
            let mut entries = tokio::fs::read_dir(&settings).await?;
            while let Some(entry) = entries.next_entry().await? {
                if !entry.file_type().await?.is_file()
                    || entry.path().extension().and_then(|part| part.to_str())
                        != Some("json")
                {
                    continue;
                }
                let target = target_settings.join(entry.file_name());
                tokio::fs::copy(entry.path(), &target).await?;
                tokio::fs::OpenOptions::new()
                    .write(true)
                    .open(&target)
                    .await?
                    .sync_all()
                    .await?;
            }
        }
        // The sync descriptor names the immutable generation frozen below.
        // Other independent domains capture their own authority after core is free.
        backup_receipts(settings_dir, &staging, &mut exclusions, true).await?;
        if let Err(error)=super::sync_documents::backup_in(&mut transaction,&crate::state::persistence::Persistence::new(settings_dir),&staging.join("synced-options")).await{
            tracing::warn!(%error,"Backup excludes unavailable shared sync state");
            exclusions.push("sync".into());
            crate::state::persistence::document::write_json(&staging.join("sync-backup-status.json"),serde_json::to_vec(&serde_json::json!({"unavailable":true}))?).await?;
        }
        if let Err(error)=crate::state::instances::manifest::backup(settings_dir,&staging).await {
            tracing::warn!(%error,"Instance recovery copies excluded from this core snapshot");
            exclusions.push("instance-recovery-copies".into());
        }
        let mut absent_settings = Vec::new();
        for relative in known_settings_authorities() {
            if staging.join(&relative).exists() { continue; }
            let previous = settings_dir.join(format!("{relative}.previous"));
            match crate::state::persistence::document::snapshot_json(&previous)? {
                Some(bytes) => crate::state::persistence::document::write_blob(&staging.join(&relative), bytes).await?,
                None => absent_settings.push(relative),
            }
        }
        Ok::<_, crate::Error>((exclusions, absent_settings))
    }
    .await;
    transaction_diagnostics
        .observe("rollback_wait", transaction.rollback())
        .await?;
    transaction_diagnostics.complete("rolled_back");
    // Keep the freeze interval limited to coordinated mutable authorities.
    // Operation generations are immutable; their exact selection is now in
    // staging/system.db and can be copied without retaining the live writer.
    drop(_document_files);
    drop(_documents);
    drop(_settings);
    let (mut exclusions, absent_settings) = match result {
        Ok(result) => result,
        Err(error) => {
            tracing::warn!(path = %staging.display(), "Incomplete database snapshot retained for inspection");
            return Err(error);
        }
    };
    before_copy().await;
    let result = async {
        backup_receipts(settings_dir, &staging, &mut exclusions, false).await?;
        if let Err(error) = super::records_db::backup(settings_dir, &staging.join("records.db")).await {
            tracing::warn!(%error,"Core snapshot excludes unavailable optional daily history");
            exclusions.push("daily-playtime".into());
            crate::state::persistence::document::write_json(&staging.join("backup-status.json"),serde_json::to_vec(&serde_json::json!({"optional_exclusions":["records.db"]}))?).await?;
        }
        if let Err(error)=super::operations_db::backup(settings_dir,&staging.join(super::operations_db::FILE)).await {
            tracing::warn!(%error,"Optional task history excluded from core snapshot");
            exclusions.push("operation-history".into());
        }
        super::recovery_documents::backup(&staging.join("system.db"),&staging).await?;
        let mut absent_authorities = backup_independent_files(settings_dir,&staging).await?;
        absent_authorities.extend(absent_settings);
        capture_missing_authorities(settings_dir, &staging, &mut exclusions, &mut absent_authorities).await?;
        crate::state::persistence::document::write_json(&staging.join("snapshot.json"),serde_json::to_vec(&serde_json::json!({"protocol":"axolotl-storage-snapshot","version":1,"absent_authorities":absent_authorities,"optional_exclusions":exclusions,"core":["system.db","instances.db","storage.db"]}))?).await?;
        tokio::fs::write(staging.join("complete"), b"axolotl database set\n")
            .await?;
        tokio::fs::rename(&staging, &destination).await?;
        Ok::<(), crate::Error>(())
    }
    .await;
    if result.is_err() {
        // A partial snapshot never has the final directory name.
        tracing::warn!(path = %staging.display(), "Incomplete database snapshot retained for inspection");
    }
    result?;
    Ok(destination)
}

async fn backup_receipts(
    settings_dir: &Path,
    staging: &Path,
    exclusions: &mut Vec<String>,
    sync_only: bool,
) -> crate::Result<()> {
    // Keep takeover receipts with the snapshot. Restoring only the SQL
    // files must never make a retired domain active again.
    let receipts = settings_dir.join("state/persistence/imports");
    if tokio::fs::try_exists(&receipts).await? {
        let target = staging.join("state/persistence/imports");
        tokio::fs::create_dir_all(&target).await?;
        let mut entries = tokio::fs::read_dir(&receipts).await?;
        while let Some(entry) = entries.next_entry().await? {
            if entry.path().extension().and_then(|value| value.to_str())
                != Some("json")
            {
                continue;
            }
            if !entry.file_type().await?.is_file() {
                return Err(crate::ErrorKind::FSError(
                    "Cannot snapshot a nonregular handover receipt".into(),
                )
                .into());
            }
            let name = entry
                .path()
                .file_stem()
                .and_then(|value| value.to_str())
                .map(str::to_owned)
                .ok_or_else(|| {
                    crate::ErrorKind::InputError(
                        "Invalid handover receipt name".into(),
                    )
                })?;
            if (name == "sync") != sync_only {
                continue;
            }
            let snapshot = crate::state::persistence::import::snapshot(
                &crate::state::persistence::Persistence::new(settings_dir),
                &name,
            )
            .await;
            let files = match snapshot {
                Ok(files) => files,
                Err(error) => {
                    tracing::warn!(domain=%name,%error,"Core backup preserves unavailable independent state as raw evidence");
                    exclusions.push(name.clone());
                    preserve_raw_receipt(settings_dir, &staging, &name).await?;
                    continue;
                }
            };
            for (relative, bytes) in files {
                let destination = staging.join(relative);
                if destination.extension().and_then(|value| value.to_str())
                    == Some("json")
                {
                    crate::state::persistence::document::write_json(
                        &destination,
                        bytes,
                    )
                    .await?;
                } else {
                    crate::state::persistence::document::write_blob(
                        &destination,
                        bytes,
                    )
                    .await?;
                }
            }
        }
    }
    Ok(())
}

/// Damaged authorities and receipts remain evidence, never usable takeover markers.
async fn preserve_raw_receipt(
    root: &Path,
    staging: &Path,
    name: &str,
) -> crate::Result<()> {
    let relative =
        PathBuf::from("state/persistence/imports").join(format!("{name}.json"));
    let source = root.join(&relative);
    if tokio::fs::try_exists(&source).await? {
        copy_regular(&source, &staging.join("unavailable").join(&relative))
            .await?;
        if let Ok(bytes) = tokio::fs::read(&source).await {
            if let Ok(value) =
                serde_json::from_slice::<serde_json::Value>(&bytes)
            {
                if let Some(target) = value["data"]["target"].as_str() {
                    let relative = Path::new(target);
                    if !relative.is_absolute()
                        && relative.components().all(|part| {
                            matches!(part, std::path::Component::Normal(_))
                        })
                    {
                        let source = root.join(relative);
                        if tokio::fs::try_exists(&source).await? {
                            copy_regular(
                                &source,
                                &staging.join("unavailable").join(relative),
                            )
                            .await?;
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
async fn copy_regular(source: &Path, target: &Path) -> crate::Result<()> {
    if !tokio::fs::symlink_metadata(source)
        .await?
        .file_type()
        .is_file()
    {
        return Err(crate::ErrorKind::FSError(
            "Snapshot refuses a linked or nonregular authority".into(),
        )
        .into());
    }
    tokio::fs::create_dir_all(target.parent().expect("snapshot parent"))
        .await?;
    tokio::fs::copy(source, target).await?;
    tokio::fs::OpenOptions::new()
        .write(true)
        .open(target)
        .await?
        .sync_all()
        .await?;
    Ok(())
}
/// Preserve file authorities that predate receipt-based import, as well as
/// opaque originals. Refuse traversal through links in the private state tree.
async fn capture_missing_authorities(
    settings_dir: &Path,
    staging: &Path,
    exclusions: &mut Vec<String>,
    absent_authorities: &mut Vec<String>,
) -> crate::Result<()> {
    for (name, relative, _) in IMPORT_AUTHORITIES {
        if exclusions.iter().any(|excluded| excluded == name) {
            continue;
        }
        let marker = format!("state/persistence/imports/{name}.json");
        if staging.join(relative).exists() && staging.join(&marker).exists() {
            continue;
        }
        if crate::state::persistence::document::absent_pair(
            settings_dir.join(relative),
            settings_dir.join(&marker),
        )
        .await?
        {
            absent_authorities.push(relative.into());
            absent_authorities.push(marker);
        } else {
            // A domain may have been created after the receipt directory
            // was enumerated. It must be captured or explicitly excluded.
            match crate::state::persistence::import::snapshot(
                &crate::state::persistence::Persistence::new(settings_dir),
                name,
            )
            .await
            {
                Ok(files) => {
                    for (relative, bytes) in files {
                        crate::state::persistence::document::write_blob(
                            &staging.join(relative),
                            bytes,
                        )
                        .await?;
                    }
                }
                Err(error) => {
                    tracing::warn!(domain=name,%error,"Backup excludes an authority created during receipt capture");
                    exclusions.push(name.into());
                    preserve_raw_receipt(settings_dir, &staging, name).await?;
                }
            }
        }
    }
    Ok(())
}

pub(super) const IMPORT_AUTHORITIES: [(&str, &str, &str); 8] = [
    ("accounts", "accounts.json", "axolotl-accounts"),
    ("telemetry", "state/telemetry.json", "axolotl-telemetry"),
    ("media", "state/media.json", "axolotl-media"),
    ("worlds", "state/worlds.json", "axolotl-world-choices"),
    ("joins", "state/joins.json", "axolotl-server-joins"),
    (
        "shared_logs",
        "state/shared_logs.json",
        "axolotl-shared-logs",
    ),
    (
        "resource_favorites",
        "state/resource_favorites.json",
        "axolotl-resource-favorites",
    ),
    (
        "instance_pins",
        "state/instance_pins.json",
        "axolotl-instance-pins",
    ),
];

pub(super) fn known_settings_authorities() -> Vec<String> {
    crate::state::documents::settings::DOMAINS
        .iter()
        .map(|(name, _, _)| *name)
        .chain(["ai", "translation", "log_share", "java", "login_proof"])
        .map(|name| format!("settings/{name}.json"))
        .collect()
}

pub(super) fn known_absence_authorities() -> Vec<String> {
    let mut paths = STANDALONE_AUTHORITIES
        .iter()
        .map(|name| (*name).to_owned())
        .collect::<Vec<_>>();
    paths.extend(known_settings_authorities());
    for (name, target, _) in IMPORT_AUTHORITIES {
        paths.push(target.into());
        paths.push(format!("state/persistence/imports/{name}.json"));
    }
    paths
}

pub(super) const STANDALONE_AUTHORITIES: [&str; 3] = [
    "instance_groups.json",
    "favorite_groups.json",
    "axolotl_config.json",
];

async fn backup_independent_files(
    root: &Path,
    staging: &Path,
) -> crate::Result<Vec<String>> {
    let mut absent = Vec::new();
    for name in STANDALONE_AUTHORITIES {
        let bytes = match name {
            "instance_groups.json" => {
                crate::state::instance_groups::snapshot(root)?
            }
            "favorite_groups.json" => {
                crate::state::favorite_groups::snapshot(root)?
            }
            _ => match crate::state::persistence::document::snapshot_json(
                &root.join(name),
            )? {
                Some(bytes) => Some(bytes),
                None => crate::state::persistence::document::snapshot_json(
                    &root.join(format!("{name}.previous")),
                )?,
            },
        };
        match bytes {
            Some(bytes) => {
                crate::state::persistence::document::write_blob(
                    &staging.join(name),
                    bytes,
                )
                .await?
            }
            None => absent.push(name.to_owned()),
        }
    }
    let mut pending =
        vec![PathBuf::from("state"), PathBuf::from("media/skins")];
    while let Some(relative) = pending.pop() {
        let source = root.join(&relative);
        if !tokio::fs::try_exists(&source).await? {
            continue;
        }
        let metadata = tokio::fs::symlink_metadata(&source).await?;
        if !metadata.is_dir() {
            continue;
        }
        let mut entries = tokio::fs::read_dir(&source).await?;
        while let Some(entry) = entries.next_entry().await? {
            let relative = relative.join(entry.file_name());
            if relative == Path::new("state/persistence/imports")
                || relative == Path::new("state/instances")
            {
                continue;
            }
            let kind = entry.file_type().await?;
            if kind.is_dir() {
                pending.push(relative);
            } else if kind.is_file()
                && !entry.file_name().to_string_lossy().ends_with(".lock")
            {
                let target = staging.join(&relative);
                if !tokio::fs::try_exists(&target).await? {
                    copy_regular(
                        &entry.path(),
                        &staging.join("unavailable").join(&relative),
                    )
                    .await?;
                }
            }
        }
    }
    Ok(absent)
}

/// Keep at most three complete snapshot sets, taking a new one at most daily.
pub(crate) async fn maintain_backups(
    pool: &SqlitePool,
    settings_dir: &Path,
) -> crate::Result<()> {
    let parent = crate::state::db_backup::app_db_backup_dir_for(
        &directory(settings_dir).join("system.db"),
    )?;
    tokio::fs::create_dir_all(&parent).await?;
    let mut entries = tokio::fs::read_dir(&parent).await?;
    let mut snapshots = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        if !entry.file_type().await?.is_dir()
            || uuid::Uuid::parse_str(&entry.file_name().to_string_lossy())
                .is_err()
        {
            continue;
        }
        let marker = entry.path().join("complete");
        if let Ok(metadata) = tokio::fs::metadata(marker).await {
            snapshots.push((metadata.modified()?, entry.path()));
        }
    }
    snapshots.sort_by_key(|(modified, _)| *modified);
    let recent = snapshots.last().is_some_and(|(modified, _)| {
        std::time::SystemTime::now()
            .duration_since(*modified)
            .is_ok_and(|age| age < Duration::from_secs(24 * 60 * 60))
    });
    if !recent {
        let destination = backup(pool, settings_dir).await?;
        snapshots.push((std::time::SystemTime::now(), destination));
    }
    let remove = snapshots.len().saturating_sub(3);
    let resolved_parent = tokio::fs::canonicalize(&parent).await?;
    for (_, path) in snapshots.into_iter().take(remove) {
        let resolved = tokio::fs::canonicalize(&path).await?;
        if resolved.parent() != Some(resolved_parent.as_path()) {
            return Err(crate::ErrorKind::FSError("Snapshot retention target escaped the database backup directory".into()).into());
        }
        tokio::fs::remove_dir_all(resolved).await?;
    }
    Ok(())
}

/// All connections attach only the relational instance and ownership files.
/// Independent documents, history and derived databases are never attachments.
pub(crate) async fn open(settings_dir: &Path) -> crate::Result<SqlitePool> {
    if tokio::fs::try_exists(
        settings_dir.join("databases/storage-restore.json"),
    )
    .await?
    {
        let _ownership = crate::state::db::acquire_domain_ownership(
            &directory(settings_dir).join("system.db"),
        )
        .await?;
        super::maintenance::finish_restore(settings_dir).await?;
    }
    super::legacy_documents::initialize(settings_dir).await;
    let retired = crate::state::persistence::import::retired_database_domains(
        settings_dir,
    )
    .await?;
    let pool = open_with_retired(settings_dir, retired).await?;
    if let Err(error) = super::records_db::initialize(settings_dir, None).await
    {
        tracing::warn!(%error,"Optional daily history unavailable; core startup continues");
    }
    if !["app.db", "release/app.db", "beta/app.db"]
        .iter()
        .any(|path| settings_dir.join(path).is_file())
        || settings_dir.join("settings/bootstrap.json").is_file()
    {
        super::sync_documents::initialize(settings_dir, None).await;
    }
    Ok(pool)
}

async fn open_with_retired(
    settings_dir: &Path,
    retired: Vec<String>,
) -> crate::Result<SqlitePool> {
    let domains = Arc::new(
        DOMAINS
            .iter()
            .copied()
            .filter(|domain| {
                !matches!(
                    *domain,
                    "accounts"
                        | "telemetry"
                        | "media"
                        | "records"
                        | "sync"
                        | "operations"
                )
            })
            .filter(|domain| {
                !retired.iter().any(|name| name.as_str() == *domain)
            })
            .map(str::to_owned)
            .collect::<Vec<_>>(),
    );
    let root = directory(settings_dir);
    tokio::fs::create_dir_all(&root).await?;
    let ownership = Arc::new(
        crate::state::db::acquire_domain_ownership(&root.join("system.db"))
            .await?,
    );
    super::maintenance::finish_restore(settings_dir).await?;
    if !root.join("system.db").is_file()
        && ["instances", "storage"]
            .iter()
            .any(|domain| root.join(format!("{domain}.db")).is_file())
    {
        return Err(crate::ErrorKind::FSError("The database coordinator is missing; existing domain files were preserved instead of reimported".into()).into());
    }
    let memory_name = String::new();
    let options = SqliteConnectOptions::new()
        .filename(directory(settings_dir).join("system.db"))
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Delete)
        .synchronous(sqlx::sqlite::SqliteSynchronous::Full)
        .busy_timeout(Duration::from_secs(30))
        .foreign_keys(true);
    let mut keeper = SqliteConnection::connect_with(&options).await?;
    let has_marker: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM main.sqlite_schema WHERE name = 'database_imports')").fetch_one(&mut keeper).await?;
    if has_marker {
        let completed: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM main.database_imports WHERE source='app.db' AND completed=1)").fetch_one(&mut keeper).await?;
        if completed
            && domains.iter().any(|domain| {
                !directory(settings_dir)
                    .join(format!("{domain}.db"))
                    .is_file()
            })
        {
            return Err(crate::ErrorKind::FSError("A durable database domain is missing; restore the complete database set before continuing".into()).into());
        }
    }
    initialize_connection(&mut keeper, &root, &memory_name, true, &domains)
        .await?;
    let pool = SqlitePoolOptions::new()
        // Preserve the previous launcher pool budget: waiting SQLite writers
        // must not consume every slot needed by an active writer's readers.
        .max_connections(100)
        .min_connections(1)
        .idle_timeout(None)
        .max_lifetime(None)
        .after_connect(move |connection, _| {
            let root = root.clone();
            let memory_name = memory_name.clone();
            let ownership = ownership.clone();
            let domains = domains.clone();
            Box::pin(async move {
                let _ownership = ownership;
                initialize_connection(
                    connection,
                    &root,
                    &memory_name,
                    false,
                    &domains,
                )
                .await
            })
        })
        .connect_with(options)
        .await?;
    super::configuration::recover_and_load(&mut keeper, settings_dir).await?;
    keeper.close().await?;
    Ok(pool)
}

async fn initialize_connection(
    connection: &mut SqliteConnection,
    root: &Path,
    memory_name: &str,
    durable: bool,
    domains: &[String],
) -> Result<(), sqlx::Error> {
    if durable {
        return attach_connection(connection, root, memory_name, true, domains)
            .await;
    }
    // Runtime setup only reads durable schemas and creates connection-local
    // TEMP objects. Keep each statement in autocommit so its read locks end
    // before the next statement; a new reader must not reserve the core writer.
    attach_connection(connection, root, memory_name, false, domains).await
}

async fn attach_connection(
    connection: &mut SqliteConnection,
    root: &Path,
    _memory_name: &str,
    durable: bool,
    domains: &[String],
) -> Result<(), sqlx::Error> {
    for domain in domains {
        sqlx::query(sqlx::AssertSqlSafe(
            format!("ATTACH DATABASE ? AS {}", quote(domain)).as_str(),
        ))
        .bind(root.join(format!("{domain}.db")).to_string_lossy().as_ref())
        .execute(&mut *connection)
        .await?;
        if durable {
            connection
                .execute(sqlx::AssertSqlSafe(
                    format!("PRAGMA {}.journal_mode = DELETE", quote(domain))
                        .as_str(),
                ))
                .await?;
            connection
                .execute(sqlx::AssertSqlSafe(
                    format!("PRAGMA {}.synchronous = FULL", quote(domain))
                        .as_str(),
                ))
                .await?;
        }
    }
    if !durable {
        return create_schema(connection, false).await;
    }
    connection.execute("BEGIN IMMEDIATE").await?;
    let result = create_schema(connection, true).await;
    match result {
        Ok(()) => connection.execute("COMMIT").await.map(|_| ()),
        Err(error) => {
            let _ = connection.execute("ROLLBACK").await;
            Err(error)
        }
    }
}

async fn create_schema(
    connection: &mut SqliteConnection,
    durable: bool,
) -> Result<(), sqlx::Error> {
    let attached = attached_domains(connection).await?;
    if durable {
        super::sync_documents::install(connection).await?;
        super::configuration::install_decisions(connection).await?;
        super::recovery_documents::install(connection).await?;
        connection.execute("CREATE TABLE IF NOT EXISTS main.database_imports (source TEXT PRIMARY KEY NOT NULL, completed INTEGER NOT NULL)").await?;
        for table in &baseline().tables {
            if !attached.contains(&table.domain) {
                continue;
            }
            let body = &table.sql[table.sql.find('(').expect("table body")..];
            connection
                .execute(sqlx::AssertSqlSafe(
                    format!(
                        "CREATE TABLE IF NOT EXISTS {} {body}",
                        qualified(table)
                    )
                    .as_str(),
                ))
                .await?;
            for seed in &table.seeds {
                connection
                    .execute(sqlx::AssertSqlSafe(seed.as_str()))
                    .await?;
            }
        }
        for domain in DOMAINS {
            if !attached.contains(*domain) {
                continue;
            }
            connection.execute(sqlx::AssertSqlSafe(format!("CREATE TABLE IF NOT EXISTS {}.extension_records (namespace TEXT NOT NULL, id TEXT NOT NULL, format TEXT NOT NULL, payload BLOB NOT NULL, PRIMARY KEY(namespace,id))", quote(domain)).as_str())).await?;
        }
    }
    crate::state::instances::adapters::sqlite::manifest_rows::install(
        connection, durable,
    )
    .await?;
    for object in &baseline().objects {
        if object.name == "resource_favorite_links" {
            continue;
        }
        // Views are instance/storage-only projections. Table-owned indexes
        // and triggers disappear together with their retired domain.
        if let Some(table) = baseline()
            .tables
            .iter()
            .find(|table| table.name == object.table)
            && !attached.contains(&table.domain)
        {
            continue;
        }
        let sql = match object.kind.as_str() {
            "index" if !durable => continue,
            "index" => {
                let table = baseline()
                    .tables
                    .iter()
                    .find(|table| table.name == object.table)
                    .expect("index table");
                let on = object.sql.find("ON ").expect("index ON");
                let unique = if object.sql.starts_with("CREATE UNIQUE") {
                    "UNIQUE "
                } else {
                    ""
                };
                format!(
                    "CREATE {unique}INDEX IF NOT EXISTS {}.{}{}",
                    quote(&table.domain),
                    quote(&object.name),
                    format!(" {}", &object.sql[on..])
                )
            }
            "view" => object.sql.replacen(
                "CREATE VIEW",
                "CREATE TEMP VIEW IF NOT EXISTS",
                1,
            ),
            "trigger" => {
                let table = baseline()
                    .tables
                    .iter()
                    .find(|table| table.name == object.table)
                    .expect("trigger table");
                object
                    .sql
                    .replacen(
                        "CREATE TRIGGER",
                        "CREATE TEMP TRIGGER IF NOT EXISTS",
                        1,
                    )
                    .replacen(
                        &format!("ON {}", object.table),
                        &format!("ON {}", qualified(table)),
                        1,
                    )
            }
            _ => continue,
        };
        connection
            .execute(sqlx::AssertSqlSafe(sql.as_str()))
            .await?;
    }
    for table in &baseline().tables {
        if !attached.contains(&table.domain) {
            continue;
        }
        for (index, foreign) in table.foreign_keys.iter().enumerate() {
            let parent = baseline()
                .tables
                .iter()
                .find(|table| table.name == foreign.parent)
                .expect("foreign parent");
            if parent.domain == table.domain {
                continue;
            }
            if !attached.contains(&parent.domain) {
                continue;
            }
            let present = foreign
                .columns
                .iter()
                .map(|column| {
                    format!("NEW.{} IS NOT NULL", quote(&column.child))
                })
                .collect::<Vec<_>>()
                .join(" AND ");
            let matches_new = foreign
                .columns
                .iter()
                .map(|column| {
                    format!(
                        "{} = NEW.{}",
                        quote(&column.parent),
                        quote(&column.child)
                    )
                })
                .collect::<Vec<_>>()
                .join(" AND ");
            let matches_old = foreign
                .columns
                .iter()
                .map(|column| {
                    format!(
                        "{} = OLD.{}",
                        quote(&column.child),
                        quote(&column.parent)
                    )
                })
                .collect::<Vec<_>>()
                .join(" AND ");
            for event in ["INSERT", "UPDATE"] {
                let name = quote(&format!(
                    "domain_{}_{}_{}",
                    table.name, index, event
                ));
                let sql = format!(
                    "CREATE TEMP TRIGGER IF NOT EXISTS {name} BEFORE {event} ON {} WHEN {present} AND NOT EXISTS (SELECT 1 FROM {} WHERE {matches_new}) BEGIN SELECT RAISE(ABORT, 'cross-domain foreign key'); END",
                    qualified(table),
                    qualified(parent)
                );
                connection
                    .execute(sqlx::AssertSqlSafe(sql.as_str()))
                    .await?;
            }
            for (event, action) in [
                ("DELETE", &foreign.on_delete),
                ("UPDATE", &foreign.on_update),
            ] {
                let assignments = foreign
                    .columns
                    .iter()
                    .map(|column| {
                        format!(
                            "{} = NEW.{}",
                            quote(&column.child),
                            quote(&column.parent)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let nulls = foreign
                    .columns
                    .iter()
                    .map(|column| format!("{} = NULL", quote(&column.child)))
                    .collect::<Vec<_>>()
                    .join(", ");
                let effect = match action.as_str() {
                    "CASCADE" if event == "DELETE" => format!(
                        "DELETE FROM {} WHERE {matches_old};",
                        quote(&table.name)
                    ),
                    "CASCADE" => format!(
                        "UPDATE {} SET {assignments} WHERE {matches_old};",
                        quote(&table.name)
                    ),
                    "SET NULL" => format!(
                        "UPDATE {} SET {nulls} WHERE {matches_old};",
                        quote(&table.name)
                    ),
                    _ => format!(
                        "SELECT RAISE(ABORT, 'cross-domain foreign key') WHERE EXISTS (SELECT 1 FROM {} WHERE {matches_old});",
                        qualified(table)
                    ),
                };
                let name = quote(&format!(
                    "domain_{}_{}_parent_{}",
                    table.name, index, event
                ));
                let changed = foreign
                    .columns
                    .iter()
                    .map(|column| {
                        format!(
                            "NEW.{} IS NOT OLD.{}",
                            quote(&column.parent),
                            quote(&column.parent)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" OR ");
                let condition = if event == "UPDATE" {
                    format!(" WHEN {changed}")
                } else {
                    String::new()
                };
                connection.execute(sqlx::AssertSqlSafe(format!("CREATE TEMP TRIGGER IF NOT EXISTS {name} BEFORE {event} ON {}{condition} BEGIN {effect} END", qualified(parent)).as_str())).await?;
            }
        }
    }
    let owned = baseline()
        .tables
        .iter()
        .filter(|table| attached.contains(&table.domain))
        .map(|table| {
            (
                table.domain.as_str(),
                table.name.as_str(),
                table.name == "instances"
                    || table
                        .columns
                        .iter()
                        .any(|column| column == "instance_id"),
            )
        })
        .collect::<Vec<_>>();
    super::compatibility::install(connection, &owned).await?;
    Ok(())
}

/// Validate a closed snapshot against this build's additive relational contract.
pub(crate) async fn validate_snapshot(
    connection: &mut SqliteConnection,
) -> crate::Result<()> {
    for schema in ["main", "instances", "storage"] {
        let status: Vec<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(
            format!("PRAGMA {schema}.quick_check"),
        ))
        .fetch_all(&mut *connection)
        .await?;
        if status != ["ok"] {
            return Err(crate::ErrorKind::InputError(
                "Snapshot database integrity check failed".into(),
            )
            .into());
        }
        if !sqlx::query(sqlx::AssertSqlSafe(format!(
            "PRAGMA {schema}.foreign_key_check"
        )))
        .fetch_all(&mut *connection)
        .await?
        .is_empty()
        {
            return Err(crate::ErrorKind::InputError(
                "Snapshot database references are incomplete".into(),
            )
            .into());
        }
    }
    for table in &baseline().tables {
        let columns = table
            .columns
            .iter()
            .map(|column| quote(column))
            .collect::<Vec<_>>()
            .join(",");
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT {columns} FROM {} LIMIT 0",
            qualified(table)
        )))
        .execute(&mut *connection)
        .await?;
    }
    if cross_domain_issues(connection).await? != 0 {
        return Err(crate::ErrorKind::InputError(
            "Snapshot cross-domain references are incomplete".into(),
        )
        .into());
    }
    Ok(())
}

/// Copy a fully migrated legacy snapshot exactly once. Configuration is handed
/// directly to its documents before the core import is marked complete.
pub(crate) async fn import(
    pool: &SqlitePool,
    source: &Path,
    settings_dir: &Path,
) -> crate::Result<()> {
    let completed: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM main.database_imports WHERE source = 'app.db' AND completed = 1)").fetch_one(pool).await?;
    if completed {
        return Ok(());
    }
    super::configuration::import_legacy(source, settings_dir).await?;
    let legacy = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new().filename(source).read_only(true),
        )
        .await?;
    let groups =
        crate::state::instance_groups::import_from_pool(&legacy, settings_dir)
            .await;
    legacy.close().await;
    groups?;
    let mut connection = pool.acquire().await?;
    sqlx::query("ATTACH DATABASE ? AS legacy_import")
        .bind(source.to_string_lossy().as_ref())
        .execute(&mut *connection)
        .await?;
    let triggers: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM temp.sqlite_schema WHERE type = 'trigger'",
    )
    .fetch_all(&mut *connection)
    .await?;
    for trigger in triggers {
        connection
            .execute(sqlx::AssertSqlSafe(
                format!("DROP TRIGGER temp.{}", quote(&trigger)).as_str(),
            ))
            .await?;
    }
    // The import attachment is the tenth (SQLite's default maximum).
    connection.execute("PRAGMA foreign_keys = OFF").await?;
    connection.execute("BEGIN IMMEDIATE").await?;
    let result = async {
        let attached = attached_domains(&mut connection).await?;
        for legacy_table in &legacy_baseline().tables {
            let Some(table)=baseline().tables.iter().find(|table|table.name==legacy_table.name) else {continue};
            if !attached.contains(&table.domain) { continue; }
            if table.name == "cache" { continue; }
            let columns = table.columns.iter().map(|column| quote(column)).collect::<Vec<_>>().join(", ");
            connection.execute(sqlx::AssertSqlSafe(format!("DELETE FROM {}", qualified(table)).as_str())).await?;
            connection.execute(sqlx::AssertSqlSafe(format!("INSERT INTO {} ({columns}) SELECT {columns} FROM legacy_import.{}", qualified(table), quote(&table.name)).as_str())).await?;
        }
        Ok::<(), sqlx::Error>(())
    }.await;
    match result {
        Ok(()) => {
            connection.execute("COMMIT").await?;
        }
        Err(error) => {
            let _ = connection.execute("ROLLBACK").await;
            let _ = connection.execute("DETACH DATABASE legacy_import").await;
            return Err(error.into());
        }
    }
    connection.execute("DETACH DATABASE legacy_import").await?;
    connection.execute("PRAGMA foreign_keys = ON").await?;
    create_schema(&mut connection, false).await?;
    drop(connection);
    sqlx::query("INSERT INTO main.database_imports (source, completed) VALUES ('app.db', 1) ON CONFLICT(source) DO UPDATE SET completed = 1").execute(pool).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn backup_captures_a_domain_created_after_receipt_enumeration() {
        let root = tempfile::tempdir().unwrap();
        let staged = tempfile::tempdir().unwrap();
        let persistence =
            crate::state::persistence::Persistence::new(root.path());
        let target = persistence
            .document("accounts.json", "axolotl-accounts", 1)
            .unwrap();
        crate::state::persistence::import::Import::new(
            &persistence,
            "accounts",
            target,
        )
        .unwrap()
        .run(
            || async {
                Ok(serde_json::Map::from_iter([
                    ("accounts".into(), serde_json::json!({})),
                    ("future".into(), serde_json::json!("preserved")),
                ]))
            },
            |_| Ok(()),
        )
        .await
        .unwrap();
        let mut excluded = Vec::new();
        let mut absent = Vec::new();
        capture_missing_authorities(
            root.path(),
            staged.path(),
            &mut excluded,
            &mut absent,
        )
        .await
        .unwrap();
        assert!(excluded.is_empty());
        assert!(!absent.iter().any(|name| name == "accounts.json"));
        assert!(
            staged
                .path()
                .join("state/persistence/imports/accounts.json")
                .is_file()
        );
        let value: serde_json::Value = serde_json::from_slice(
            &tokio::fs::read(staged.path().join("accounts.json"))
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(value["data"]["future"], "preserved");
    }

    #[tokio::test]
    async fn backup_never_completes_with_a_corrupt_standalone_authority() {
        let root = tempfile::tempdir().unwrap();
        let pool = open(root.path()).await.unwrap();
        tokio::fs::write(root.path().join("favorite_groups.json"), b"{")
            .await
            .unwrap();
        assert!(backup(&pool, root.path()).await.is_err());
        let parent = crate::state::db_backup::app_db_backup_dir_for(
            &root.path().join("databases/system.db"),
        )
        .unwrap();
        let mut entries = tokio::fs::read_dir(parent).await.unwrap();
        while let Some(entry) = entries.next_entry().await.unwrap() {
            assert!(!entry.path().join("complete").exists());
        }
        pool.close().await;
    }

    #[tokio::test]
    async fn backup_releases_core_before_copying_frozen_operation_generations()
    {
        let temporary = tempfile::tempdir().unwrap();
        let pool = open(temporary.path()).await.unwrap();
        {
            let mut connection = pool.acquire().await.unwrap();
            super::super::recovery_documents::write(
                &mut connection,
                "example",
                "operation",
                Some(&serde_json::json!({"count": 1})),
            )
            .await
            .unwrap();
        }
        let original: String = sqlx::query_scalar(
            "SELECT object_path FROM main.operation_publications WHERE kind='example' AND id='operation'",
        ).fetch_one(&pool).await.unwrap();
        let (captured, frozen) = tokio::sync::oneshot::channel();
        let (resume, released) = tokio::sync::oneshot::channel();
        let backup = backup_with_copy_hook(&pool, temporary.path(), || async {
            captured.send(()).unwrap();
            released.await.unwrap();
        });
        let foreground = async {
            frozen.await.unwrap();
            tokio::time::timeout(Duration::from_secs(2), async {
                // Consume the existing idle connection so another reader must
                // pass the real after_connect write-reservation path.
                let held = pool.acquire().await.unwrap();
                let mut connection = pool.acquire().await.unwrap();
                let _: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM instances")
                    .fetch_one(&mut *connection).await.unwrap();
                super::super::recovery_documents::write(
                    &mut connection, "example", "operation", Some(&serde_json::json!({"count": 2})),
                ).await.unwrap();
                drop(connection);
                drop(held);
                let handle = super::super::Handle::new(pool.clone());
                super::super::configuration_documents::set_preferred_source(&handle, "curseforge")
                    .await.unwrap();
                crate::state::instances::adapters::sqlite::instance_rows::list_instances(&pool)
                    .await.unwrap();
            }).await.expect("bulk backup must not retain core or document writer locks");
            resume.send(()).unwrap();
        };
        let (destination, ()) = tokio::join!(backup, foreground);
        let destination = destination.unwrap();
        let records: Vec<serde_json::Value> = serde_json::from_slice(
            &tokio::fs::read(
                destination.join("operation-records/references.json"),
            )
            .await
            .unwrap(),
        )
        .unwrap();
        let record = records
            .iter()
            .find(|record| record[0] == "example" && record[1] == "operation")
            .unwrap();
        assert_eq!(record[4], original);
        let envelope = destination
            .join("operation-records")
            .join(crate::state::recovery::record::digest(original.as_bytes()))
            .join(Path::new(&original).file_name().unwrap());
        let value = crate::state::recovery::record::read(
            &envelope,
            record[5].as_str().unwrap(),
            "example",
            "operation",
            None,
            1,
        )
        .await
        .unwrap();
        assert_eq!(value["count"], 1);
        let live: serde_json::Value =
            super::super::recovery_documents::read_raw(
                &mut *pool.acquire().await.unwrap(),
                "example",
                "operation",
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(live["count"], 2);
    }

    #[tokio::test]
    async fn backup_refuses_selected_operation_files_deleted_after_core_capture()
     {
        let temporary = tempfile::tempdir().unwrap();
        let pool = open(temporary.path()).await.unwrap();
        super::super::recovery_documents::write(
            &mut *pool.acquire().await.unwrap(),
            "example",
            "operation",
            Some(&serde_json::json!({"count": 1})),
        )
        .await
        .unwrap();
        let original: String = sqlx::query_scalar(
            "SELECT object_path FROM main.operation_publications WHERE kind='example' AND id='operation'",
        ).fetch_one(&pool).await.unwrap();
        let parent = Path::new(&original).parent().unwrap().to_path_buf();
        assert!(parent.starts_with(
            tokio::fs::canonicalize(temporary.path()).await.unwrap()
        ));
        let result = backup_with_copy_hook(&pool, temporary.path(), || async {
            tokio::fs::remove_dir_all(parent).await.unwrap();
        })
        .await;
        assert!(
            result.is_err(),
            "missing selected operation payload cannot form a complete backup"
        );
        let backup_parent = crate::state::db_backup::app_db_backup_dir_for(
            &directory(temporary.path()).join("system.db"),
        )
        .unwrap();
        let mut entries = tokio::fs::read_dir(backup_parent).await.unwrap();
        while let Some(entry) = entries.next_entry().await.unwrap() {
            assert!(!entry.path().join("complete").exists());
        }
    }

    #[tokio::test]
    async fn backup_keeps_selected_previous_envelope_when_primary_is_missing() {
        let temporary = tempfile::tempdir().unwrap();
        let pool = open(temporary.path()).await.unwrap();
        super::super::recovery_documents::write(
            &mut *pool.acquire().await.unwrap(),
            "example",
            "operation",
            Some(&serde_json::json!({"count": 1})),
        )
        .await
        .unwrap();
        let original: String = sqlx::query_scalar(
            "SELECT object_path FROM main.operation_publications WHERE kind='example' AND id='operation'",
        ).fetch_one(&pool).await.unwrap();
        let destination =
            backup_with_copy_hook(&pool, temporary.path(), || async {
                tokio::fs::remove_file(&original).await.unwrap();
            })
            .await
            .unwrap();
        let record: (String, String) = sqlx::query_as(
            "SELECT object_path,sha256 FROM main.operation_publications WHERE kind='example' AND id='operation'",
        ).fetch_one(&pool).await.unwrap();
        let envelope = destination
            .join("operation-records")
            .join(crate::state::recovery::record::digest(record.0.as_bytes()))
            .join(Path::new(&record.0).file_name().unwrap());
        let value = crate::state::recovery::record::read(
            &envelope,
            &record.1,
            "example",
            "operation",
            None,
            1,
        )
        .await
        .unwrap();
        assert_eq!(value["count"], 1);
    }

    #[tokio::test]
    async fn retired_domains_are_not_opened_recreated_or_reimported() {
        let temporary = tempfile::tempdir().unwrap();
        let pool = open(temporary.path()).await.unwrap();
        sqlx::query("INSERT INTO main.database_imports VALUES('app.db',1)")
            .execute(&pool)
            .await
            .unwrap();
        pool.close().await;
        drop(pool);
        for domain in ["accounts", "telemetry", "media", "sync"] {
            tokio::fs::write(
                directory(temporary.path()).join(format!("{domain}.db")),
                b"obsolete corrupt database",
            )
            .await
            .unwrap();
        }
        // Takeover already completed on the first open. A newer authority
        // stays unavailable and must not be replaced from the retired SQL.
        let newer_media = br#"{"protocol":"axolotl-media","schema_version":2,"revision":1,"data":{"future_reference":"retain"}}"#;
        tokio::fs::write(
            temporary.path().join("state/media.json"),
            newer_media,
        )
        .await
        .unwrap();
        let pool = open(temporary.path()).await.unwrap();
        let mut first = pool.acquire().await.unwrap();
        let mut second = pool.acquire().await.unwrap();
        for connection in [&mut first, &mut second] {
            let attached = attached_domains(connection).await.unwrap();
            for domain in ["accounts", "telemetry", "media", "sync"] {
                assert!(!attached.contains(domain));
            }
            for domain in ["instances", "storage"] {
                assert!(attached.contains(domain));
            }
            assert_eq!(cross_domain_issues(connection).await.unwrap(), 0);
            sqlx::query("INSERT INTO instances(id,path,install_stage,launcher_feature_version,name,created,modified) VALUES ('retired-test','path','installed','none','test',0,0)").execute(&mut **connection).await.unwrap();
            sqlx::query("DELETE FROM instances WHERE id='retired-test'")
                .execute(&mut **connection)
                .await
                .unwrap();
        }
        drop(first);
        drop(second);
        assert!(ensure_cleanup_supported(&pool).await.is_err());
        assert_eq!(
            tokio::fs::read(temporary.path().join("state/media.json"))
                .await
                .unwrap(),
            newer_media
        );
        for domain in ["accounts", "telemetry", "media", "sync"] {
            assert_eq!(
                tokio::fs::read(
                    directory(temporary.path()).join(format!("{domain}.db"))
                )
                .await
                .unwrap(),
                b"obsolete corrupt database"
            );
        }
        pool.close().await;
    }

    #[tokio::test]
    async fn a_retired_missing_domain_stays_missing_when_legacy_import_retries()
    {
        let temporary = tempfile::tempdir().unwrap();
        let legacy_path = temporary.path().join("legacy.db");
        let legacy = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(&legacy_path)
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        sqlx::migrate!().run(&legacy).await.unwrap();
        sqlx::query("INSERT INTO telemetry_identity(id,installation_id) VALUES(0,'old-identity')").execute(&legacy).await.unwrap();
        legacy.close().await;
        let persistence =
            crate::state::persistence::Persistence::new(temporary.path());
        let document = persistence
            .document("state/telemetry.json", "test", 1)
            .unwrap();
        crate::state::persistence::import::Import::new(
            &persistence,
            "telemetry",
            document,
        )
        .unwrap()
        .run(|| async { Ok(serde_json::Map::new()) }, |_| Ok(()))
        .await
        .unwrap();
        let pool = open(temporary.path()).await.unwrap();
        import(&pool, &legacy_path, temporary.path()).await.unwrap();
        assert!(!directory(temporary.path()).join("telemetry.db").exists());
        pool.close().await;
    }

    #[tokio::test]
    async fn unreadable_handover_does_not_make_retired_sql_data_available() {
        let temporary = tempfile::tempdir().unwrap();
        let receipts = temporary.path().join("state/persistence/imports");
        tokio::fs::create_dir_all(&receipts).await.unwrap();
        tokio::fs::write(receipts.join("accounts.json"), b"broken")
            .await
            .unwrap();
        let pool = open(temporary.path()).await.unwrap();
        assert!(
            !attached_domains(&mut pool.acquire().await.unwrap())
                .await
                .unwrap()
                .contains("accounts")
        );
        assert!(!directory(temporary.path()).join("accounts.db").exists());
        assert_eq!(
            tokio::fs::read(receipts.join("accounts.json"))
                .await
                .unwrap(),
            b"broken"
        );
        pool.close().await;
    }

    #[tokio::test]
    async fn snapshot_keeps_retirement_receipt_and_document_instead_of_old_database()
     {
        let temporary = tempfile::tempdir().unwrap();
        let persistence =
            crate::state::persistence::Persistence::new(temporary.path());
        let document = persistence
            .document("state/accounts.json", "test", 1)
            .unwrap();
        crate::state::persistence::import::Import::new(
            &persistence,
            "accounts",
            document.clone(),
        )
        .unwrap()
        .run(
            || async {
                Ok(serde_json::Map::from_iter([(
                    "metadata".into(),
                    serde_json::json!("account"),
                )]))
            },
            |_| Ok(()),
        )
        .await
        .unwrap();
        let pool = open(temporary.path()).await.unwrap();
        let snapshot = backup(&pool, temporary.path()).await.unwrap();
        assert!(!snapshot.join("accounts.db").exists());
        assert!(
            snapshot
                .join("state/persistence/imports/accounts.json")
                .is_file()
        );
        assert_eq!(
            tokio::fs::read(snapshot.join("state/accounts.json"))
                .await
                .unwrap(),
            tokio::fs::read(document.path()).await.unwrap()
        );
        assert!(snapshot.join("complete").is_file());
        pool.close().await;
    }

    #[tokio::test]
    async fn every_connection_attaches_domains_without_application_migrations()
    {
        let temporary = tempfile::tempdir().unwrap();
        let pool = open(temporary.path()).await.unwrap();
        let mut first = pool.acquire().await.unwrap();
        let mut second = pool.acquire().await.unwrap();
        for connection in [&mut first, &mut second] {
            let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pragma_database_list WHERE name IN ('instances', 'accounts', 'sync', 'media', 'operations', 'storage', 'records', 'telemetry', 'configuration')").fetch_one(&mut **connection).await.unwrap();
            assert_eq!(count, 2);
            let migrations: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM main.sqlite_schema WHERE name = '_sqlx_migrations'").fetch_one(&mut **connection).await.unwrap();
            assert_eq!(migrations, 0);
            let forbidden:i64=sqlx::query_scalar("SELECT COUNT(*) FROM pragma_database_list WHERE name IN ('configuration','operations')").fetch_one(&mut **connection).await.unwrap();
            assert_eq!(forbidden, 0);
            for schema in std::iter::once("main").chain(
                DOMAINS.iter().copied().filter(|domain| {
                    !matches!(
                        *domain,
                        "accounts"
                            | "telemetry"
                            | "media"
                            | "records"
                            | "sync"
                            | "operations"
                    )
                }),
            ) {
                let mode: String = sqlx::query_scalar(sqlx::AssertSqlSafe(
                    format!("PRAGMA {}.journal_mode", quote(schema)),
                ))
                .fetch_one(&mut **connection)
                .await
                .unwrap();
                let synchronous: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(
                    format!("PRAGMA {}.synchronous", quote(schema)),
                ))
                .fetch_one(&mut **connection)
                .await
                .unwrap();
                assert_eq!(mode, "delete", "{schema}");
                assert_eq!(synchronous, 2, "{schema}");
            }
        }
        drop(first);
        drop(second);
        pool.close().await;
        assert!(
            !directory(temporary.path())
                .join("configuration.db")
                .exists()
        );
        for domain in DOMAINS.iter().filter(|domain| {
            !matches!(
                **domain,
                "accounts"
                    | "telemetry"
                    | "media"
                    | "records"
                    | "sync"
                    | "operations"
            )
        }) {
            assert!(
                directory(temporary.path())
                    .join(format!("{domain}.db"))
                    .is_file()
            );
        }
    }

    #[tokio::test]
    async fn core_ownership_foreign_keys_and_rollback_are_preserved() {
        let temporary = tempfile::tempdir().unwrap();
        let pool = open(temporary.path()).await.unwrap();
        sqlx::query("INSERT INTO instances(id,path,install_stage,launcher_feature_version,name,created,modified) VALUES('instance','profile','installed','none','name',0,0)").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO store_blobs(digest,size,state,created_at,last_used_at) VALUES('digest',1,'ready',0,0)").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO store_instance_files(instance_id,relative_path,digest,created_at,modified_at) VALUES('instance','mods/a.jar','digest',0,0)").execute(&pool).await.unwrap();
        assert!(sqlx::query("INSERT INTO store_instance_files(instance_id,relative_path,digest,created_at,modified_at) VALUES('instance','mods/b.jar','missing',0,0)").execute(&pool).await.is_err());
        let mut tx = pool.begin().await.unwrap();
        sqlx::query(
            "DELETE FROM store_instance_files WHERE instance_id='instance'",
        )
        .execute(&mut *tx)
        .await
        .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM store_instance_files"
            )
            .fetch_one(&mut *tx)
            .await
            .unwrap(),
            0
        );
        tx.rollback().await.unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM store_instance_files"
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
            1
        );
    }

    #[tokio::test]
    async fn composite_foreign_keys_match_the_complete_parent_key() {
        let temporary = tempfile::tempdir().unwrap();
        let pool = open(temporary.path()).await.unwrap();
        for id in ["first", "second"] {
            sqlx::query("INSERT INTO instances(id,path,install_stage,launcher_feature_version,name,created,modified) VALUES(?,?,'installed','none','test',0,0)")
                .bind(id).bind(id).execute(&pool).await.unwrap();
        }
        for (instance, feature) in [
            ("first", crate::state::SyncedOption::Screenshots),
            ("second", crate::state::SyncedOption::GameOptions),
        ] {
            crate::state::instances::adapters::sqlite::instance_rows::set_instance_sync_preference(instance,feature,true,&pool).await.unwrap();
        }
        use super::super::instance_sync_checkpoints::{
            CheckpointKey, CheckpointRecord, CheckpointStatus,
            InstanceSyncCheckpoints,
        };
        let handle = super::super::Handle::new(pool.clone());
        let record = CheckpointRecord {
            expected_sha1: "hash".into(),
            merge_base: None,
            source_revision: 0,
            status: CheckpointStatus::Ready,
            link_mode: None,
        };
        let key = |feature: &str| CheckpointKey {
            instance_id: "first".into(),
            feature: feature.into(),
            variant: "default".into(),
        };
        assert!(
            InstanceSyncCheckpoints::set(
                &handle,
                &key("game_options"),
                Some(&record)
            )
            .await
            .is_err()
        );
        InstanceSyncCheckpoints::set(
            &handle,
            &key("screenshots"),
            Some(&record),
        )
        .await
        .unwrap();
        crate::state::instances::adapters::sqlite::instance_rows::set_instance_sync_preference("second",crate::state::SyncedOption::GameOptions,false,&pool).await.unwrap();
        assert!(
            InstanceSyncCheckpoints::get(&handle, &key("screenshots"))
                .await
                .unwrap()
                .is_some()
        );
    }

    #[tokio::test]
    async fn a_new_reader_initializes_while_an_existing_writer_is_reserved() {
        let temporary = tempfile::tempdir().unwrap();
        let pool = open(temporary.path()).await.unwrap();
        sqlx::query("INSERT INTO instances(id,path,install_stage,launcher_feature_version,name,created,modified) VALUES('reader-test','old','installed','none','Reader',0,0)")
            .execute(&pool).await.unwrap();
        let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
        sqlx::query(
            "UPDATE instances SET path='uncommitted' WHERE id='reader-test'",
        )
        .execute(&mut *transaction)
        .await
        .unwrap();
        let read = tokio::time::timeout(Duration::from_millis(200), async {
            let mut reader = pool.acquire().await.unwrap();
            let path = crate::state::instances::adapters::sqlite::instance_rows::get_instance_path_by_id(
                "reader-test", &mut *reader,
            ).await.unwrap();
            assert_eq!(path.as_deref(), Some("old"));
            reader
        }).await.expect("new reader initialization must not wait for the core writer reservation");
        transaction.rollback().await.unwrap();
        let mut reader = read;
        // The runtime connection still has its normal publication tracking and
        // cross-domain relationship guards after being initialized concurrently.
        let revision: i64 = sqlx::query_scalar("SELECT revision FROM instance_recovery_publications WHERE instance_id='reader-test'")
            .fetch_one(&mut *reader).await.unwrap();
        sqlx::query(
            "UPDATE instances SET name='Changed' WHERE id='reader-test'",
        )
        .execute(&mut *reader)
        .await
        .unwrap();
        let changed: i64 = sqlx::query_scalar("SELECT revision FROM instance_recovery_publications WHERE instance_id='reader-test'")
            .fetch_one(&mut *reader).await.unwrap();
        assert!(changed > revision);
        assert_eq!(cross_domain_issues(&mut reader).await.unwrap(), 0);
        assert_eq!(crate::state::instances::adapters::sqlite::instance_rows::get_instance_path_by_id(
            "reader-test", &mut *reader,
        ).await.unwrap().as_deref(), Some("old"));
    }

    #[tokio::test]
    async fn cold_readers_and_repeated_core_writers_make_progress_together() {
        let temporary = tempfile::tempdir().unwrap();
        let pool = open(temporary.path()).await.unwrap();
        sqlx::query("INSERT INTO instances(id,path,install_stage,launcher_feature_version,name,created,modified) VALUES('stress-test','profile','installed','none','Original',0,0)")
            .execute(&pool).await.unwrap();
        let writer = async {
            for round in 0..64 {
                let mut transaction =
                    pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
                sqlx::query(
                    "UPDATE instances SET modified=? WHERE id='stress-test'",
                )
                .bind(round)
                .execute(&mut *transaction)
                .await
                .unwrap();
                tokio::task::yield_now().await;
                transaction.commit().await.unwrap();
            }
        };
        let readers = futures::future::join_all((0..8).map(|_| async {
            for _ in 0..8 {
                let mut reader = pool.acquire().await.unwrap();
                let path = crate::state::instances::adapters::sqlite::instance_rows::get_instance_path_by_id(
                    "stress-test", &mut *reader,
                ).await.unwrap();
                assert_eq!(path.as_deref(), Some("profile"));
                // Discard the connection so the next round exercises after_connect.
                reader.close().await.unwrap();
            }
        }));
        tokio::time::timeout(Duration::from_secs(20), async {
            tokio::join!(writer, readers);
        }).await.expect("cold connection initialization and core writers must both complete");
        let changed: i64 = sqlx::query_scalar(
            "SELECT modified FROM instances WHERE id='stress-test'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(changed, 63);
    }

    #[tokio::test]
    async fn legacy_import_is_once_only_and_hands_configuration_to_documents() {
        let temporary = tempfile::tempdir().unwrap();
        let source = temporary.path().join("legacy.db");
        let legacy = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(&source)
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        sqlx::migrate!().run(&legacy).await.unwrap();
        sqlx::query(
            "UPDATE settings SET default_page = 'library', version = 3",
        )
        .execute(&legacy)
        .await
        .unwrap();
        sqlx::query("INSERT INTO instances(id,path,install_stage,launcher_feature_version,name,created,modified) VALUES ('instance','path','installed','none','original',0,0)").execute(&legacy).await.unwrap();
        legacy.close().await;
        let pool = open(temporary.path()).await.unwrap();
        import(&pool, &source, temporary.path()).await.unwrap();
        let appearance: serde_json::Value = serde_json::from_slice(
            &tokio::fs::read(temporary.path().join("settings/appearance.json"))
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(appearance["data"]["default_page"], "Library");
        let name: String = sqlx::query_scalar(
            "SELECT name FROM instances WHERE id='instance'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(name, "original");
        sqlx::query("UPDATE instances SET name='edited' WHERE id='instance'")
            .execute(&pool)
            .await
            .unwrap();
        import(&pool, &source, temporary.path()).await.unwrap();
        let name: String = sqlx::query_scalar(
            "SELECT name FROM instances WHERE id='instance'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(name, "edited");
        pool.close().await;
    }

    #[tokio::test]
    async fn legacy_row_migration_does_not_migrate_existing_document_hooks_again()
     {
        let temporary = tempfile::tempdir().unwrap();
        let directory = temporary.path().join("settings");
        tokio::fs::create_dir(&directory).await.unwrap();
        let original = br#"{"schema_version":2,"data":{"hooks":{"wrapper":"\"already quoted command\""}}}"#;
        tokio::fs::write(directory.join("game.json"), original)
            .await
            .unwrap();
        crate::state::documents::settings::identify(temporary.path());
        let source = temporary.path().join("legacy.db");
        let legacy = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(&source)
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        sqlx::migrate!().run(&legacy).await.unwrap();
        sqlx::query("UPDATE settings SET version=1,hook_wrapper='legacy unquoted command'").execute(&legacy).await.unwrap();
        legacy.close().await;
        let pool = open(temporary.path()).await.unwrap();
        import(&pool, &source, temporary.path()).await.unwrap();
        assert_eq!(
            tokio::fs::read(directory.join("game.json")).await.unwrap(),
            original
        );
        let count:i64=sqlx::query_scalar("SELECT COUNT(*) FROM pragma_database_list WHERE name='configuration'").fetch_one(&pool).await.unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn imported_proxy_and_provider_credentials_live_in_json_after_reopen()
    {
        use crate::state::{curseforge_settings, proxy_settings};
        use crate::util::proxy::{ProxyConfig, ProxyMode};
        let temporary = tempfile::tempdir().unwrap();
        let source = temporary.path().join("legacy.db");
        let legacy = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(&source)
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        sqlx::migrate!().run(&legacy).await.unwrap();
        sqlx::query("UPDATE settings SET proxy_mode='custom',proxy_url='http://proxy.test:8080',proxy_username='test-user',proxy_password='test-password',curseforge_api_key='test-import-key'").execute(&legacy).await.unwrap();
        legacy.close().await;
        let pool = open(temporary.path()).await.unwrap();
        import(&pool, &source, temporary.path()).await.unwrap();
        pool.close().await;
        drop(pool);
        let pool = open(temporary.path()).await.unwrap();
        let proxy = proxy_settings::get(&pool).await.unwrap();
        assert_eq!(proxy.mode, ProxyMode::Custom);
        assert_eq!(proxy.password, "test-password");
        assert_eq!(
            curseforge_settings::get(&pool).await.unwrap().as_deref(),
            Some("test-import-key")
        );
        proxy_settings::set(
            &pool,
            &ProxyConfig {
                mode: ProxyMode::Custom,
                url: "http://other.test:9090".into(),
                username: "changed-user".into(),
                password: "changed-password".into(),
            },
        )
        .await
        .unwrap();
        curseforge_settings::set(&pool, Some("changed-key"))
            .await
            .unwrap();
        pool.close().await;
        drop(pool);
        let pool = open(temporary.path()).await.unwrap();
        let proxy = proxy_settings::get(&pool).await.unwrap();
        assert_eq!(proxy.url, "http://other.test:9090");
        assert_eq!(proxy.username, "changed-user");
        assert_eq!(proxy.password, "changed-password");
        assert_eq!(
            curseforge_settings::get(&pool).await.unwrap().as_deref(),
            Some("changed-key")
        );
        assert!(temporary.path().join("settings/proxy.json").is_file());
        assert!(
            temporary
                .path()
                .join("settings/content_providers.json")
                .is_file()
        );
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_instance_and_preference_writes_reserve_domains_in_order()
     {
        use super::super::instance_sync_preferences::{
            InstancePreferenceKey, InstancePreferenceRecord,
            InstanceSyncPreferences,
        };
        let temporary = tempfile::tempdir().unwrap();
        let pool = open(temporary.path()).await.unwrap();
        let write = |prefix: &'static str| {
            let pool = pool.clone();
            async move {
                let handle = super::super::Handle::new(pool);
                for index in 0..3 {
                    let id = format!("{prefix}-{index}");
                    let mut unit = super::super::atom(&handle).await.unwrap();
                    sqlx::query("INSERT INTO instances(id,path,install_stage,launcher_feature_version,name,created,modified) VALUES(?,?,'installed','none','concurrent',0,0)").bind(&id).bind(&id).execute(unit.connection()).await.unwrap();
                    InstanceSyncPreferences::atom_set(
                        &mut unit,
                        &InstancePreferenceKey {
                            instance_id: id,
                            feature: "screenshots".into(),
                        },
                        Some(&InstancePreferenceRecord { enabled: true }),
                    )
                    .await
                    .unwrap();
                    unit.commit().await.unwrap();
                }
            }
        };
        tokio::time::timeout(Duration::from_secs(20), async {
            tokio::join!(
                write("first"),
                write("second"),
                write("third"),
                write("fourth")
            );
        })
        .await
        .expect("domain writers must not deadlock");
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM instances")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 12);
        pool.close().await;
        drop(pool);
        let pool = open(temporary.path()).await.unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM instances")
                .fetch_one(&pool)
                .await
                .unwrap(),
            12
        );
        let preferences = InstanceSyncPreferences::get_all(
            &super::super::Handle::new(pool.clone()),
        )
        .await
        .unwrap();
        assert_eq!(preferences.len(), 12);
        assert!(preferences.iter().all(|(_, record)| record.enabled));
        assert_eq!(
            cross_domain_issues(&mut pool.acquire().await.unwrap())
                .await
                .unwrap(),
            0
        );
    }

    #[tokio::test]
    async fn optional_database_damage_never_attaches_to_core() {
        let temporary = tempfile::tempdir().unwrap();
        tokio::fs::create_dir_all(directory(temporary.path()))
            .await
            .unwrap();
        for name in [
            "operations.db",
            "operations-history.db",
            "records.db",
            "derived.db",
            "accounts.db",
            "sync.db",
            "media.db",
            "telemetry.db",
        ] {
            tokio::fs::write(
                directory(temporary.path()).join(name),
                b"corrupt optional source",
            )
            .await
            .unwrap();
        }
        // Coordinator absence with old domains is intentionally refused; create
        // it first so this fixture represents a handed-over database set.
        let main = SqliteConnection::connect_with(
            &SqliteConnectOptions::new()
                .filename(directory(temporary.path()).join("system.db"))
                .create_if_missing(true),
        )
        .await
        .unwrap();
        main.close().await.unwrap();
        let pool = open(temporary.path()).await.unwrap();
        let domains = attached_domains(&mut pool.acquire().await.unwrap())
            .await
            .unwrap();
        assert_eq!(
            domains
                .into_iter()
                .filter(|name| name != "temp")
                .collect::<HashSet<_>>(),
            HashSet::from_iter([
                "main".into(),
                "instances".into(),
                "storage".into()
            ])
        );
        pool.close().await;
    }
}

/// Build a disposable read-only projection from validated recovery copies.
/// Source database files are never opened for mutation by this path.
pub(crate) async fn open_recovery(root: &Path) -> crate::Result<SqlitePool> {
    let snapshots = crate::state::instances::manifest::list(root).await?;
    if snapshots.is_empty() {
        return Err(crate::ErrorKind::FSError(
            "No complete instance recovery copies are available".into(),
        )
        .into());
    }
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .idle_timeout(None)
        .max_lifetime(None)
        .connect_with(
            SqliteConnectOptions::new()
                .in_memory(true)
                .foreign_keys(false),
        )
        .await?;
    let mut connection = pool.acquire().await?;
    for domain in ["instances", "storage"] {
        connection
            .execute(sqlx::AssertSqlSafe(format!(
                "ATTACH DATABASE ':memory:' AS {}",
                quote(domain)
            )))
            .await?;
    }
    create_schema(&mut connection, true).await?;
    let triggers: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM temp.sqlite_schema WHERE type='trigger'",
    )
    .fetch_all(&mut *connection)
    .await?;
    for trigger in triggers {
        connection
            .execute(sqlx::AssertSqlSafe(format!(
                "DROP TRIGGER temp.{}",
                quote(&trigger)
            )))
            .await?;
    }
    let mut imported = 0usize;
    for snapshot in snapshots {
        let id = snapshot.metadata.instance.id.clone();
        let mut transaction = connection.begin().await?;
        match import_recovery_snapshot(&mut transaction, &snapshot).await {
            Ok(()) => {
                transaction.commit().await?;
                imported += 1;
            }
            Err(error) => {
                transaction.rollback().await?;
                tracing::warn!(instance_id=%id,%error,"Unsupported or incomplete recovery copy retained; skipping this instance");
            }
        }
    }
    if imported == 0 {
        return Err(crate::ErrorKind::FSError(
            "No supported complete instance recovery copies are available"
                .into(),
        )
        .into());
    }
    if let Err(error) =
        super::configuration::recover_and_load(&mut connection, root).await
    {
        tracing::warn!(%error,"Optional preference projection is unavailable in instance recovery mode");
    }
    connection.execute("PRAGMA query_only=ON").await?;
    drop(connection);
    super::recovery_mode::register(&pool, root);
    Ok(pool)
}

async fn import_recovery_snapshot(
    connection: &mut SqliteConnection,
    snapshot: &crate::state::instances::manifest::RecoverySnapshot,
) -> crate::Result<()> {
    let id = &snapshot.metadata.instance.id;
    // Validate references against this copy before importing into a projection
    // that may already contain other recovered instances.
    let ids = |table: &str| {
        snapshot
            .relationships
            .get(table)
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|row| row.get("id").and_then(serde_json::Value::as_str))
            .collect::<HashSet<_>>()
    };
    let sets = ids("instance_content_sets");
    let files = ids("instance_files");
    let entries = ids("instance_content_entries");
    let members = ids("instance_pack_members");
    for (table, rows) in &snapshot.relationships {
        let Some(rows) = rows.as_array() else {
            continue;
        };
        for row in rows {
            for (key, targets) in [
                ("content_set_id", &sets),
                ("content_entry_id", &entries),
                ("parent_entry_id", &entries),
                ("child_entry_id", &entries),
                ("entry_id", &entries),
                ("file_id", &files),
                ("pack_member_id", &members),
            ] {
                if let Some(target) =
                    row.get(key).and_then(serde_json::Value::as_str)
                    && !targets.contains(target)
                {
                    return Err(crate::ErrorKind::InputError(format!("Recovery relationship {table}.{key} escapes its instance copy")).into());
                }
            }
        }
    }

    for table in &baseline().tables {
        let Some(rows) = snapshot
            .relationships
            .get(&table.name)
            .and_then(serde_json::Value::as_array)
        else {
            continue;
        };
        if table.domain != "instances" {
            continue;
        }
        for raw in rows {
            let object = raw.as_object().ok_or_else(|| {
                crate::ErrorKind::InputError(
                    "Recovery row must be an object".into(),
                )
            })?;
            if object
                .get("instance_id")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|owner| owner != id)
                || (table.name == "instances"
                    && object.get("id").and_then(serde_json::Value::as_str)
                        != Some(id.as_str()))
            {
                return Err(crate::ErrorKind::InputError(
                    "Recovery row belongs to another instance".into(),
                )
                .into());
            }
            for key in [
                "content_set_id",
                "content_entry_id",
                "parent_entry_id",
                "child_entry_id",
                "entry_id",
                "file_id",
                "pack_member_id",
            ] {
                let Some(target) =
                    object.get(key).and_then(serde_json::Value::as_str)
                else {
                    continue;
                };
                let ownership = match key {
                    "content_set_id" => {
                        "SELECT instance_id FROM instance_content_sets WHERE id=?"
                    }
                    "file_id" => {
                        "SELECT instance_id FROM instance_files WHERE id=?"
                    }
                    "pack_member_id" => {
                        "SELECT content.instance_id FROM instance_pack_members member JOIN instance_content_sets content ON content.id=member.content_set_id WHERE member.id=?"
                    }
                    _ => {
                        "SELECT instance_id FROM instance_content_entries WHERE id=?"
                    }
                };
                let owner: Option<String> =
                    sqlx::query_scalar(sqlx::AssertSqlSafe(ownership))
                        .bind(target)
                        .fetch_optional(&mut *connection)
                        .await?;
                if owner.as_deref().is_some_and(|owner| owner != id) {
                    return Err(crate::ErrorKind::InputError(
                        "Recovery relationship points into another instance"
                            .into(),
                    )
                    .into());
                }
            }
            if let Some(key) =
                object.get("id").and_then(serde_json::Value::as_str)
                && table.columns.iter().any(|column| column == "instance_id")
            {
                let owner: Option<String> =
                    sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
                        "SELECT instance_id FROM {} WHERE id=?",
                        qualified(table)
                    )))
                    .bind(key)
                    .fetch_optional(&mut *connection)
                    .await?;
                if owner.as_deref().is_some_and(|owner| owner != id) {
                    return Err(crate::ErrorKind::InputError(
                        "Recovery identity collides with another instance"
                            .into(),
                    )
                    .into());
                }
            }
            let columns = table
                .columns
                .iter()
                .filter(|name| object.contains_key(*name))
                .collect::<Vec<_>>();
            if columns.is_empty() {
                return Err(crate::ErrorKind::InputError(
                    "Recovery row has no supported columns".into(),
                )
                .into());
            }
            let shared = matches!(
                table.name.as_str(),
                "resources"
                    | "resource_source_projects"
                    | "resource_source_types"
                    | "resource_aliases"
                    | "resource_identity_evidence"
            );
            let conflict = if shared {
                " ON CONFLICT DO NOTHING"
            } else {
                ""
            };
            let sql = format!(
                "INSERT INTO {} ({}) VALUES ({}){conflict}",
                qualified(table),
                columns
                    .iter()
                    .map(|name| quote(name))
                    .collect::<Vec<_>>()
                    .join(","),
                vec!["?"; columns.len()].join(",")
            );
            let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
            for name in columns {
                let value = &object[name];
                query = match value {
                    serde_json::Value::Null => query.bind(None::<String>),
                    serde_json::Value::Number(value) if value.is_i64() => {
                        query.bind(value.as_i64())
                    }
                    serde_json::Value::Number(value) => {
                        query.bind(value.as_f64())
                    }
                    serde_json::Value::String(value) => {
                        query.bind(value.clone())
                    }
                    serde_json::Value::Object(value)
                        if value.contains_key("sqlite_blob") =>
                    {
                        query.bind(serde_json::from_value::<Vec<u8>>(
                            value["sqlite_blob"].clone(),
                        )?)
                    }
                    _ => {
                        return Err(crate::ErrorKind::InputError(
                            "Unsupported recovery SQL value".into(),
                        )
                        .into());
                    }
                };
            }
            query.execute(&mut *connection).await?;
        }
    }
    if super::compatibility::unsupported_on(connection)
        .await?
        .contains(id)
    {
        return Err(crate::ErrorKind::InputError(
            "Recovery copy contains unsupported persistent data".into(),
        )
        .into());
    }
    let record=crate::state::instances::adapters::sqlite::instance_rows::recovery_metadata_on(id,connection).await?.ok_or_else(||crate::ErrorKind::InputError("Recovery identity is missing".into()))?;
    let metadata = &snapshot.metadata;
    if record.instance.id != *id
        || record.instance.path != metadata.instance.path
        || record.applied_content_set.id != metadata.applied_content_set.id
        || record.applied_content_set.revision
            != metadata.applied_content_set.revision
        || record.applied_content_set.instance_id != *id
    {
        return Err(crate::ErrorKind::InputError("Recovery descriptor disagrees with authoritative relationship rows".into()).into());
    }
    crate::state::instances::adapters::sqlite::loader_component_rows::recovery_components_on(id,connection).await?;
    Ok(())
}

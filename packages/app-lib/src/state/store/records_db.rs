//! Optional, release-independent daily history. Never attached to the core.
use crate::state::persistence::{
    Persistence,
    document::Snapshot,
    import::{self, Import},
    status::Availability,
};
use futures::TryStreamExt;
use serde_json::{Map, Value, json};
use sqlx::{
    Connection, Row, SqliteConnection, SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, Weak};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

pub(super) const FILE: &str = "records.db";
const RELATIVE_FILE: &str = "data/records.db";
pub(super) const DOMAIN: &str = "daily-playtime";
pub(super) const RECEIPT: &str = "data/persistence/imports/daily-playtime.json";
const DESCRIPTOR: &str = "axolotl-playtime-store";
const FORMAT: &str = "axolotl-daily-playtime";
const FENCE: &str = "records:daily-playtime";
const COMMITTED: i64 = 1;
const EXCLUDED: i64 = 2;
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);
const TABLE: &str = "CREATE TABLE IF NOT EXISTS instance_daily_playtime (played_on TEXT NOT NULL,instance_id TEXT NOT NULL,instance_name TEXT NOT NULL,played_seconds INTEGER NOT NULL DEFAULT 0,session_count INTEGER NOT NULL DEFAULT 0,PRIMARY KEY(played_on,instance_id))";

pub(crate) async fn initialize(
    root: &Path,
    snapshot: Option<PathBuf>,
    core: Option<&SqlitePool>,
) -> crate::Result<()> {
    open(root, snapshot, core).await?.close().await;
    Ok(())
}

fn validate_descriptor(snapshot: &Snapshot) -> crate::Result<()> {
    validate_descriptor_data(snapshot.data())
}

fn validate_descriptor_data(
    data: Option<&Map<String, Value>>,
) -> crate::Result<()> {
    let valid = data.is_some_and(|data| {
        data.get("file").and_then(Value::as_str) == Some(RELATIVE_FILE)
            && data.get("contract").and_then(Value::as_u64) == Some(1)
    });
    if !valid {
        return Err(crate::ErrorKind::InputError(
            "Unsupported daily history contract".into(),
        )
        .into());
    }
    Ok(())
}

async fn open(
    root: &Path,
    snapshot: Option<PathBuf>,
    core: Option<&SqlitePool>,
) -> crate::Result<SqlitePool> {
    let fence = if let Some(core) = core {
        read_fence(core).await?
    } else {
        None
    };
    if fence == Some(EXCLUDED) {
        return Err(crate::ErrorKind::FSError(
            "Daily history was excluded from the restored snapshot".into(),
        )
        .into());
    }
    if fence.is_some_and(|value| value != COMMITTED) {
        return Err(crate::ErrorKind::InputError(
            "Unknown daily history handover decision".into(),
        )
        .into());
    }
    let persistence = Persistence::new(root);
    let target =
        persistence.document(crate::state::layout::PLAYTIME, DESCRIPTOR, 1)?;
    let path = crate::state::layout::data(root).join(FILE);
    let import_root = root.to_owned();
    let import_path = path.clone();
    let result = Import::new(&persistence, DOMAIN, target)?
        .run_recovering_descriptor(
            move |verify_only| async move {
                if verify_only || fence == Some(COMMITTED) {
                    validate_snapshot(&import_path).await?;
                } else {
                    adopt(&import_root, &import_path, snapshot).await?;
                }
                Ok(Map::from_iter([
                    ("file".into(), json!(RELATIVE_FILE)),
                    ("contract".into(), json!(1)),
                ]))
            },
            validate_descriptor,
        )
        .await?;
    if !matches!(
        result.availability,
        Availability::Ready | Availability::Recovered
    ) {
        return Err(crate::ErrorKind::FSError(
            "Daily history authority unavailable".into(),
        )
        .into());
    }
    require_file(&path).await?;
    let pool = SqlitePoolOptions::new()
        .max_connections(2)
        .connect_with(options(&path).await?)
        .await?;
    {
        let mut connection = pool.acquire().await?;
        validate_connection(&mut connection).await?;
    }
    if let Some(core) = core
        && fence.is_none()
    {
        // SQL is committed and both JSON handover files are published first.
        // If interrupted, the SQL marker supports retry without legacy rows.
        sqlx::query(
            "INSERT INTO main.database_imports (source, completed) VALUES (?, ?) ON CONFLICT(source) DO NOTHING",
        )
        .bind(FENCE)
        .bind(COMMITTED)
        .execute(core)
        .await?;
    }
    Ok(pool)
}

async fn read_fence(core: &SqlitePool) -> crate::Result<Option<i64>> {
    Ok(sqlx::query_scalar(
        "SELECT completed FROM main.database_imports WHERE source = ?",
    )
    .bind(FENCE)
    .fetch_optional(core)
    .await?)
}

async fn options(path: &Path) -> crate::Result<SqliteConnectOptions> {
    let path = crate::state::db_backup::sqlite_native_path(path).await?;
    Ok(SqliteConnectOptions::new()
        .filename(path.as_ref())
        .busy_timeout(BUSY_TIMEOUT))
}

async fn require_file(path: &Path) -> crate::Result<()> {
    if !tokio::fs::try_exists(path).await? {
        return Err(crate::ErrorKind::FSError(format!(
            "Daily history authority is missing at {}; its completed handover prevents importing retired data again",
            path.display()
        )).into());
    }
    Ok(())
}

async fn validate_connection(
    connection: &mut SqliteConnection,
) -> crate::Result<()> {
    let format: (String, i64) = sqlx::query_as(
        "SELECT protocol,version FROM axolotl_records_format WHERE singleton=1",
    )
    .fetch_one(&mut *connection)
    .await?;
    if format.0 != FORMAT || format.1 != 1 {
        return Err(crate::ErrorKind::InputError(
            "Daily history uses an unsupported protocol".into(),
        )
        .into());
    }
    validate_table(connection).await
}

async fn validate_table(
    connection: &mut SqliteConnection,
) -> crate::Result<()> {
    sqlx::query("SELECT played_on,instance_id,instance_name,played_seconds,session_count FROM instance_daily_playtime LIMIT 0")
        .execute(&mut *connection).await?;
    let keys: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM pragma_table_info('instance_daily_playtime') WHERE pk > 0 ORDER BY pk",
    )
    .fetch_all(connection)
    .await?;
    if keys != ["played_on", "instance_id"] {
        return Err(crate::ErrorKind::InputError(
            "Daily history uses an unsupported primary key".into(),
        )
        .into());
    }
    Ok(())
}

async fn has_format(connection: &mut SqliteConnection) -> crate::Result<bool> {
    Ok(sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name = 'axolotl_records_format')")
        .fetch_one(connection).await?)
}

async fn adopt(
    root: &Path,
    path: &Path,
    source: Option<PathBuf>,
) -> crate::Result<()> {
    let snapshot = source.or_else(|| {
        let pending = root.join(crate::state::layout::IMPORT_SNAPSHOT);
        pending.is_file().then_some(pending)
    });
    let exists = tokio::fs::try_exists(path).await?;
    if !exists && snapshot.is_none() && legacy_exists(root) {
        return Err(awaiting_snapshot());
    }
    tokio::fs::create_dir_all(path.parent().expect("database root")).await?;
    let mut connection = SqliteConnection::connect_with(
        &options(path).await?.create_if_missing(!exists),
    )
    .await?;
    if has_format(&mut connection).await? {
        return validate_connection(&mut connection).await;
    }
    let mut tx = connection.begin_with("BEGIN IMMEDIATE").await?;
    if has_format(&mut tx).await? {
        return validate_connection(&mut tx).await;
    }
    let unknown_objects: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name NOT GLOB 'sqlite_*' AND NOT ((type = 'table' AND name = 'instance_daily_playtime') OR (type = 'index' AND name = 'instance_daily_playtime_played_on' AND tbl_name = 'instance_daily_playtime')))",
    )
    .fetch_one(&mut *tx)
    .await?;
    if unknown_objects {
        return Err(crate::ErrorKind::InputError(
            "An unmarked daily history database contains unknown objects"
                .into(),
        )
        .into());
    }
    sqlx::query(TABLE).execute(&mut *tx).await?;
    validate_table(&mut tx).await?;
    let populated: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM instance_daily_playtime)",
    )
    .fetch_one(&mut *tx)
    .await?;
    if populated {
        return Err(crate::ErrorKind::InputError(
            "An unmarked daily history database already contains records"
                .into(),
        )
        .into());
    }
    if let Some(snapshot) = snapshot {
        import_rows(&mut tx, &snapshot).await?;
    } else if legacy_exists(root) {
        return Err(awaiting_snapshot());
    }
    sqlx::query("CREATE INDEX IF NOT EXISTS instance_daily_playtime_played_on ON instance_daily_playtime(played_on)")
        .execute(&mut *tx).await?;
    sqlx::query("CREATE TABLE axolotl_records_format (singleton INTEGER PRIMARY KEY CHECK(singleton=1),protocol TEXT NOT NULL,version INTEGER NOT NULL)")
        .execute(&mut *tx).await?;
    sqlx::query("INSERT INTO axolotl_records_format(singleton,protocol,version) VALUES(1,?,1)")
        .bind(FORMAT).execute(&mut *tx).await?;
    tx.commit().await?;
    connection.close().await?;
    Ok(())
}

fn legacy_exists(root: &Path) -> bool {
    ["app.db", "release/app.db", "beta/app.db"]
        .iter()
        .any(|name| root.join(name).is_file())
}

fn awaiting_snapshot() -> crate::Error {
    crate::ErrorKind::FSError(
        "Daily history awaits its migrated legacy snapshot".into(),
    )
    .into()
}

async fn import_rows(
    target: &mut SqliteConnection,
    snapshot: &Path,
) -> crate::Result<()> {
    let mut source = SqliteConnection::connect_with(
        &options(snapshot).await?.read_only(true),
    )
    .await?;
    let mut skipped = 0;
    if !super::legacy_documents::columns(&mut source, "instance_daily_playtime")
        .await?
        .is_empty()
    {
        let mut rows = sqlx::query("SELECT played_on,instance_id,instance_name,played_seconds,session_count FROM instance_daily_playtime")
            .fetch(&mut source);
        while let Some(row) = rows.try_next().await? {
            let (
                Ok(Some(day)),
                Ok(Some(id)),
                Ok(Some(name)),
                Ok(Some(seconds)),
                Ok(Some(count)),
            ) = (
                row.try_get::<Option<String>, _>(0),
                row.try_get::<Option<String>, _>(1),
                row.try_get::<Option<String>, _>(2),
                row.try_get::<Option<i64>, _>(3),
                row.try_get::<Option<i64>, _>(4),
            )
            else {
                skipped += 1;
                continue;
            };
            if seconds < 0
                || count < 0
                || chrono::NaiveDate::parse_from_str(&day, "%Y-%m-%d").is_err()
            {
                skipped += 1;
                continue;
            }
            sqlx::query("INSERT INTO instance_daily_playtime(played_on,instance_id,instance_name,played_seconds,session_count) VALUES(?,?,?,?,?) ON CONFLICT(played_on,instance_id) DO NOTHING")
                .bind(day).bind(id).bind(name).bind(seconds).bind(count)
                .execute(&mut *target).await?;
        }
    }
    source.close().await?;
    if skipped != 0 {
        tracing::warn!(
            skipped,
            "Skipped unreadable legacy daily history rows; original snapshot retained"
        );
    }
    Ok(())
}

#[derive(Default)]
struct HistoryPool {
    pool: Option<SqlitePool>,
    retry_after: Option<Instant>,
}
pub(crate) async fn pool_for(core: &SqlitePool) -> Option<SqlitePool> {
    let owner = core.connect_options();
    if owner
        .get_filename()
        .file_name()
        .is_none_or(|name| name != "system.db")
    {
        #[cfg(test)]
        {
            return Some(core.clone());
        }
        #[cfg(not(test))]
        {
            return None;
        }
    }
    type HistoryPoolEntry =
        (Weak<SqliteConnectOptions>, Arc<Mutex<HistoryPool>>);
    static POOLS: OnceLock<
        std::sync::Mutex<std::collections::HashMap<usize, HistoryPoolEntry>>,
    > = OnceLock::new();
    let cell = {
        let mut pools = POOLS
            .get_or_init(std::sync::Mutex::default)
            .lock()
            .expect("daily history pools");
        pools.retain(|_, (owner, _)| owner.strong_count() != 0);
        pools
            .entry(Arc::as_ptr(&owner) as usize)
            .or_insert_with(|| (Arc::downgrade(&owner), Arc::default()))
            .1
            .clone()
    };
    let mut entry = cell.lock().await;
    if let Some(pool) = entry.pool.as_ref().filter(|pool| !pool.is_closed()) {
        return Some(pool.clone());
    }
    if entry
        .retry_after
        .is_some_and(|after| Instant::now() < after)
    {
        return None;
    }
    let root = super::persistence_for_pool(core);
    match open(root.root(), None, Some(core)).await {
        Ok(pool) => {
            entry.pool = Some(pool.clone());
            entry.retry_after = None;
            Some(pool)
        }
        Err(error) => {
            entry.retry_after = Some(Instant::now() + Duration::from_secs(1));
            tracing::warn!(%error, "Daily history unavailable; retrying on a later request");
            None
        }
    }
}

pub(super) async fn validate_snapshot(path: &Path) -> crate::Result<()> {
    require_file(path).await?;
    let mut connection =
        SqliteConnection::connect_with(&options(path).await?.read_only(true))
            .await?;
    validate_connection(&mut connection).await?;
    connection.close().await?;
    Ok(())
}

pub(super) async fn stage_restore_decision(
    connection: &mut SqliteConnection,
    available: bool,
) -> crate::Result<()> {
    sqlx::query(
        "INSERT INTO main.database_imports (source, completed) VALUES (?, ?) ON CONFLICT(source) DO UPDATE SET completed = excluded.completed",
    )
    .bind(FENCE)
    .bind(if available { COMMITTED } else { EXCLUDED })
    .execute(connection)
    .await?;
    Ok(())
}

fn validate_descriptor_bytes(bytes: &[u8]) -> crate::Result<()> {
    let descriptor: Value = serde_json::from_slice(bytes)?;
    if descriptor["protocol"] != DESCRIPTOR
        || descriptor["schema_version"] != 1
        || descriptor["revision"].as_u64().is_none()
    {
        return Err(crate::ErrorKind::InputError(
            "Unsupported daily history descriptor".into(),
        )
        .into());
    }
    validate_descriptor_data(descriptor["data"].as_object())
}

pub(super) async fn validate_backup(staging: &Path) -> crate::Result<()> {
    let descriptor = crate::state::layout::PLAYTIME;
    let files = import::snapshot(&Persistence::new(staging), DOMAIN).await?;
    let (_, bytes) = files
        .iter()
        .find(|(path, _)| path == Path::new(descriptor))
        .ok_or_else(|| {
            crate::ErrorKind::InputError(
                "Daily history receipt names a different authority".into(),
            )
        })?;
    validate_descriptor_bytes(bytes)?;
    validate_snapshot(&staging.join(FILE)).await
}

/// The detached history uses a SQLite snapshot rather than copying a live file.
pub(crate) async fn backup(
    root: &Path,
    staging: &Path,
    core: Option<&SqlitePool>,
) -> crate::Result<()> {
    if let Some(core) = core
        && read_fence(core)
            .await?
            .is_some_and(|decision| decision != COMMITTED)
    {
        return Err(crate::ErrorKind::FSError(
            "Daily history authority is unavailable after restore".into(),
        )
        .into());
    }
    let source = crate::state::layout::data(root).join(FILE);
    require_file(&source).await?;
    let files = import::snapshot(&Persistence::new(root), DOMAIN).await?;
    let (_, bytes) = files
        .iter()
        .find(|(path, _)| path == Path::new(crate::state::layout::PLAYTIME))
        .ok_or_else(|| {
            crate::ErrorKind::InputError(
                "Daily history receipt names a different authority".into(),
            )
        })?;
    validate_descriptor_bytes(bytes)?;
    let mut connection = SqliteConnection::connect_with(
        &options(&source).await?.read_only(true),
    )
    .await?;
    validate_connection(&mut connection).await?;
    for (relative, bytes) in files {
        crate::state::persistence::document::write_blob(
            &staging.join(relative),
            bytes,
        )
        .await?;
    }
    crate::state::db_backup::create_verified_sqlite_snapshot(
        &mut connection,
        &staging.join(FILE),
    )
    .await?;
    connection.close().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn missing_retired_daily_history_is_not_recreated_from_legacy_rows() {
        let temporary = tempfile::tempdir().unwrap();
        initialize(temporary.path(), None, None).await.unwrap();
        let receipt_path = temporary
            .path()
            .join("data/persistence/imports/daily-playtime.json");
        let authority_path =
            temporary.path().join("data/persistence/records.json");
        let receipt = tokio::fs::read(&receipt_path).await.unwrap();
        let authority = tokio::fs::read(&authority_path).await.unwrap();
        let records_path = temporary.path().join("data/records.db");
        tokio::fs::remove_file(&records_path).await.unwrap();
        let mut legacy = sqlx::SqliteConnection::connect_with(
            &SqliteConnectOptions::new()
                .filename(
                    temporary.path().join("data/persistence/legacy-app.db"),
                )
                .create_if_missing(true),
        )
        .await
        .unwrap();
        sqlx::query(TABLE).execute(&mut legacy).await.unwrap();
        sqlx::query("INSERT INTO instance_daily_playtime VALUES('2026-11-16','retired','Must not return',99,1)")
            .execute(&mut legacy).await.unwrap();
        legacy.close().await.unwrap();
        let error = open(temporary.path(), None, None).await.unwrap_err();
        assert!(matches!(error.raw.as_ref(), crate::ErrorKind::FSError(_)));
        assert!(
            error
                .to_string()
                .contains("Daily history authority is missing")
        );
        assert!(!records_path.exists());
        assert_eq!(tokio::fs::read(receipt_path).await.unwrap(), receipt);
        assert_eq!(tokio::fs::read(authority_path).await.unwrap(), authority);
    }

    #[tokio::test]
    async fn unmarked_empty_history_imports_valid_rows_only() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path();
        let source = root.join("app.db");
        let mut legacy = SqliteConnection::connect_with(
            &options(&source).await.unwrap().create_if_missing(true),
        )
        .await
        .unwrap();
        sqlx::query(TABLE).execute(&mut legacy).await.unwrap();
        sqlx::raw_sql("INSERT INTO instance_daily_playtime VALUES('2026-11-16','good','Preserved',99,1),('2026-11-16','blob','Unreadable',X'ABCDEF',1),('2026-11-16','negative','Unreadable',-1,1),('bad date','bad-date','Unreadable',10,1)")
            .execute(&mut legacy).await.unwrap();
        legacy.close().await.unwrap();
        let original = tokio::fs::read(&source).await.unwrap();
        let core = super::super::databases::open(root).await.unwrap();
        let records = root.join("data").join(FILE);
        let mut empty = SqliteConnection::connect_with(
            &options(&records).await.unwrap().create_if_missing(true),
        )
        .await
        .unwrap();
        sqlx::query(TABLE).execute(&mut empty).await.unwrap();
        empty.close().await.unwrap();

        initialize(root, Some(source.clone()), Some(&core))
            .await
            .unwrap();
        let history = pool_for(&core).await.unwrap();
        let rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT instance_id,played_seconds FROM instance_daily_playtime",
        )
        .fetch_all(&history)
        .await
        .unwrap();
        assert_eq!(rows, [("good".to_owned(), 99)]);
        assert_eq!(tokio::fs::read(source).await.unwrap(), original);
        history.close().await;
        core.close().await;
    }

    #[tokio::test]
    async fn committed_core_fence_blocks_import_after_all_history_artifacts_are_lost()
     {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path();
        let core = super::super::databases::open(root).await.unwrap();
        let retained = root.join(crate::state::layout::IMPORT_SNAPSHOT);
        let mut legacy = SqliteConnection::connect_with(
            &options(&retained).await.unwrap().create_if_missing(true),
        )
        .await
        .unwrap();
        sqlx::query(TABLE).execute(&mut legacy).await.unwrap();
        sqlx::query("INSERT INTO instance_daily_playtime VALUES('2026-11-16','retired','Old',99,1)")
            .execute(&mut legacy).await.unwrap();
        legacy.close().await.unwrap();
        for path in [RELATIVE_FILE, crate::state::layout::PLAYTIME, RECEIPT] {
            tokio::fs::remove_file(root.join(path)).await.unwrap();
            let previous = root.join(format!("{path}.previous"));
            if previous.is_file() {
                tokio::fs::remove_file(previous).await.unwrap();
            }
        }
        assert_eq!(read_fence(&core).await.unwrap(), Some(COMMITTED));
        assert!(open(root, None, Some(&core)).await.is_err());
        assert!(!root.join(RELATIVE_FILE).exists());
        assert!(!root.join(RECEIPT).exists());
        core.close().await;
    }

    #[tokio::test]
    async fn valid_receipt_recovers_its_missing_descriptor_from_existing_sql() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path();
        initialize(root, None, None).await.unwrap();
        let descriptor = root.join(crate::state::layout::PLAYTIME);
        tokio::fs::remove_file(&descriptor).await.unwrap();
        let previous =
            root.join(format!("{}.previous", crate::state::layout::PLAYTIME));
        if previous.is_file() {
            tokio::fs::remove_file(previous).await.unwrap();
        }
        initialize(root, None, None).await.unwrap();
        assert!(descriptor.is_file());
    }

    #[tokio::test]
    async fn backup_excludes_a_future_history_format_with_a_valid_receipt() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path();
        let core = super::super::databases::open(root).await.unwrap();
        let source = root.join(RELATIVE_FILE);
        let mut history =
            SqliteConnection::connect_with(&options(&source).await.unwrap())
                .await
                .unwrap();
        sqlx::query("UPDATE axolotl_records_format SET version=9000")
            .execute(&mut history)
            .await
            .unwrap();
        history.close().await.unwrap();
        let original = tokio::fs::read(&source).await.unwrap();

        let snapshot =
            super::super::databases::backup(&core, root).await.unwrap();
        let manifest: Value = serde_json::from_slice(
            &tokio::fs::read(snapshot.join("snapshot.json"))
                .await
                .unwrap(),
        )
        .unwrap();
        assert!(
            manifest["optional_exclusions"]
                .as_array()
                .unwrap()
                .iter()
                .any(|domain| domain == DOMAIN)
        );
        assert!(!snapshot.join(FILE).exists());
        assert_eq!(tokio::fs::read(source).await.unwrap(), original);
        core.close().await;
    }

    #[tokio::test]
    async fn temporary_exclusive_lock_does_not_disable_daily_history_for_the_session()
     {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path();
        let core = super::super::databases::open(root).await.unwrap();
        let mut writer = SqliteConnection::connect_with(
            &options(&root.join(RELATIVE_FILE)).await.unwrap(),
        )
        .await
        .unwrap();
        sqlx::query("BEGIN EXCLUSIVE")
            .execute(&mut writer)
            .await
            .unwrap();
        assert!(pool_for(&core).await.is_none());
        sqlx::query("ROLLBACK").execute(&mut writer).await.unwrap();
        writer.close().await.unwrap();
        tokio::time::sleep(Duration::from_secs(1)).await;
        pool_for(&core)
            .await
            .expect("history should reopen after the lock is released")
            .close()
            .await;
        core.close().await;
    }

    #[cfg(target_os = "windows")]
    #[tokio::test]
    async fn canonical_windows_root_can_reopen_daily_history_and_back_it_up() {
        let temporary = tempfile::tempdir().unwrap();
        let canonical =
            tokio::fs::canonicalize(temporary.path()).await.unwrap();
        assert!(canonical.to_string_lossy().starts_with(r"\\?\"));
        let path = canonical.join("data/records.db");
        assert!(!path.to_string_lossy().contains('/'));
        let core = super::super::databases::open(&canonical).await.unwrap();
        let pool = pool_for(&core).await.expect(
            "canonical core path should resolve the existing daily history",
        );
        sqlx::query("INSERT INTO instance_daily_playtime VALUES('2026-11-16','one','Example',42,1)")
            .execute(&pool).await.unwrap();
        pool.close().await;
        let staging = canonical.join("history-backup");
        tokio::fs::create_dir(&staging).await.unwrap();
        backup(&canonical, &staging, Some(&core)).await.unwrap();
        let mut snapshot = sqlx::SqliteConnection::connect_with(
            &SqliteConnectOptions::new()
                .filename(staging.join(FILE))
                .read_only(true),
        )
        .await
        .unwrap();
        let seconds: i64 = sqlx::query_scalar("SELECT played_seconds FROM instance_daily_playtime WHERE instance_id='one'")
            .fetch_one(&mut snapshot).await.unwrap();
        assert_eq!(seconds, 42);
        snapshot.close().await.unwrap();
        core.close().await;
    }
}

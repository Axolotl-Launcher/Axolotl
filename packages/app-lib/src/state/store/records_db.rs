//! Optional, release-independent daily history. Never attached to the core.
use crate::state::persistence::{
    Persistence, import::Import, status::Availability,
};
use serde_json::{Map, Value, json};
use sqlx::{
    Connection, SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use tokio::sync::{Mutex, OnceCell};

const TABLE: &str = "CREATE TABLE IF NOT EXISTS instance_daily_playtime (played_on TEXT NOT NULL,instance_id TEXT NOT NULL,instance_name TEXT NOT NULL,played_seconds INTEGER NOT NULL DEFAULT 0,session_count INTEGER NOT NULL DEFAULT 0,PRIMARY KEY(played_on,instance_id))";

pub(crate) async fn initialize(
    root: &Path,
    snapshot: Option<PathBuf>,
) -> crate::Result<()> {
    let persistence = Persistence::new(root);
    let target = persistence.document(
        crate::state::layout::PLAYTIME,
        "axolotl-playtime-store",
        1,
    )?;
    let path = root.join("data/records.db");
    let root = root.to_owned();
    let result=Import::new(&persistence,"daily-playtime",target)?.run(move||async move{
        let snapshot=match snapshot {Some(path)=>Some(path),None=>{let pending=root.join(crate::state::layout::IMPORT_SNAPSHOT);if pending.is_file(){Some(pending)}else{None}}};
        if !path.is_file() && snapshot.is_none() && ["app.db","release/app.db","beta/app.db"].iter().any(|name|root.join(name).is_file()) {
            return Err(crate::ErrorKind::FSError("Daily history awaits its migrated legacy snapshot".into()).into());
        }
        tokio::fs::create_dir_all(path.parent().expect("database root")).await?;
        let pool=SqlitePoolOptions::new().max_connections(1).connect_with(SqliteConnectOptions::new().filename(&path).create_if_missing(true).busy_timeout(std::time::Duration::from_secs(5))).await?;
        let result=async {
            let mut tx=pool.begin_with("BEGIN IMMEDIATE").await?;
            let existing:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='instance_daily_playtime')").fetch_one(&mut *tx).await?;
            sqlx::query(TABLE).execute(&mut *tx).await?;
            sqlx::query("CREATE INDEX IF NOT EXISTS instance_daily_playtime_played_on ON instance_daily_playtime(played_on)").execute(&mut *tx).await?;
            let has_format:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='axolotl_records_format')").fetch_one(&mut *tx).await?;
            if !has_format {
                sqlx::query("CREATE TABLE axolotl_records_format (singleton INTEGER PRIMARY KEY CHECK(singleton=1),protocol TEXT NOT NULL,version INTEGER NOT NULL)").execute(&mut *tx).await?;
                sqlx::query("INSERT INTO axolotl_records_format(singleton,protocol,version) VALUES(1,'axolotl-daily-playtime',1)").execute(&mut *tx).await?;
            }
            let format:(String,i64)=sqlx::query_as("SELECT protocol,version FROM axolotl_records_format WHERE singleton=1").fetch_one(&mut *tx).await?;
            if format!=("axolotl-daily-playtime".into(),1){return Err(crate::ErrorKind::InputError("Daily history uses a newer protocol".into()).into());}
            sqlx::query("SELECT played_on,instance_id,instance_name,played_seconds,session_count FROM instance_daily_playtime LIMIT 0").execute(&mut *tx).await?;
            if let Some(snapshot)=snapshot.filter(|_|!existing) {
                let mut source=sqlx::SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(snapshot).read_only(true)).await?;
                sqlx::query("BEGIN").execute(&mut source).await?;
                if !super::legacy_documents::columns(&mut source,"instance_daily_playtime").await?.is_empty(){
                    let rows=sqlx::query_as::<_,(String,String,String,i64,i64)>("SELECT played_on,instance_id,instance_name,played_seconds,session_count FROM instance_daily_playtime").fetch_all(&mut source).await?;
                    for(day,id,name,seconds,count)in rows {
                        sqlx::query("INSERT INTO instance_daily_playtime(played_on,instance_id,instance_name,played_seconds,session_count) VALUES(?,?,?,?,?) ON CONFLICT(played_on,instance_id) DO NOTHING").bind(day).bind(id).bind(name).bind(seconds).bind(count).execute(&mut *tx).await?;
                    }
                }
                source.close().await?;
            }
            tx.commit().await?;
            Ok::<_,crate::Error>(())
        }.await;
        pool.close().await;result?;
        Ok(Map::from_iter([("file".into(),json!("data/records.db")),("contract".into(),json!(1))]))
    },|snapshot|{
        let data=snapshot.data().ok_or_else(||crate::ErrorKind::InputError("Missing daily history contract".into()))?;
        if data.get("file").and_then(Value::as_str)!=Some("data/records.db")||data.get("contract").and_then(Value::as_u64)!=Some(1){return Err(crate::ErrorKind::InputError("Unsupported daily history contract".into()).into());}Ok(())
    }).await?;
    if !matches!(
        result.availability,
        Availability::Ready | Availability::Recovered
    ) {
        return Err(crate::ErrorKind::FSError(
            "Daily history authority unavailable".into(),
        )
        .into());
    }
    Ok(())
}
async fn open(root: &Path) -> crate::Result<SqlitePool> {
    initialize(root, None).await.map_err(|error| {
        error.with_context(format!(
            "Preparing daily history in {}",
            root.display()
        ))
    })?;
    let path = root.join("data/records.db");
    if !tokio::fs::try_exists(&path).await? {
        return Err(crate::ErrorKind::FSError(format!(
            "Daily history authority is missing at {}; its completed handover prevents importing retired data again",
            path.display()
        )).into());
    }
    let pool = SqlitePoolOptions::new()
        .max_connections(2)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(&path)
                .create_if_missing(false)
                .busy_timeout(std::time::Duration::from_secs(5)),
        )
        .await
        .map_err(|error| {
            crate::Error::from(error).with_context(format!(
                "Opening daily history {} (file present: {})",
                path.display(),
                path.is_file()
            ))
        })?;
    let format: (String, i64) = sqlx::query_as(
        "SELECT protocol,version FROM axolotl_records_format WHERE singleton=1",
    )
    .fetch_one(&pool)
    .await?;
    if format != ("axolotl-daily-playtime".into(), 1) {
        pool.close().await;
        return Err(crate::ErrorKind::InputError(
            "Daily history uses a newer protocol".into(),
        )
        .into());
    }
    sqlx::query("SELECT played_on,instance_id,instance_name,played_seconds,session_count FROM instance_daily_playtime LIMIT 0").execute(&pool).await?;
    Ok(pool)
}
pub(crate) async fn pool_for(core: &SqlitePool) -> Option<SqlitePool> {
    if core
        .connect_options()
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
    type Entry = Arc<OnceCell<Option<SqlitePool>>>;
    static POOLS: OnceLock<Mutex<std::collections::HashMap<PathBuf, Entry>>> =
        OnceLock::new();
    let root = super::persistence_for_pool(core).root().to_owned();
    let cell = {
        let mut pools = POOLS
            .get_or_init(|| Mutex::new(std::collections::HashMap::new()))
            .lock()
            .await;
        pools
            .entry(root.clone())
            .or_insert_with(|| Arc::new(OnceCell::new()))
            .clone()
    };
    cell.get_or_init(||async {match open(&root).await{Ok(pool)=>Some(pool),Err(error)=>{tracing::warn!(%error,"Daily history disabled for this session; core instances remain available");None}}}).await.clone()
}
/// The detached history uses a SQLite snapshot rather than copying a live file.
pub(crate) async fn backup(
    root: &Path,
    destination: &Path,
) -> crate::Result<bool> {
    let source = root.join("data/records.db");
    if !source.is_file() {
        if root
            .join(crate::state::layout::IMPORT_RECEIPTS)
            .join("daily-playtime.json")
            .is_file()
        {
            return Err(crate::ErrorKind::FSError(
                "Daily history authority is missing".into(),
            )
            .into());
        }
        return Ok(false);
    }
    let mut connection = sqlx::SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(source)
            .read_only(true)
            .busy_timeout(std::time::Duration::from_secs(5)),
    )
    .await?;
    crate::state::db_backup::create_verified_sqlite_snapshot(
        &mut connection,
        destination,
    )
    .await?;
    connection.close().await?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn missing_retired_daily_history_is_not_recreated_from_legacy_rows() {
        let temporary = tempfile::tempdir().unwrap();
        initialize(temporary.path(), None).await.unwrap();
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
        let error = open(temporary.path()).await.unwrap_err();
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

    #[cfg(target_os = "windows")]
    #[tokio::test]
    async fn canonical_windows_root_can_reopen_daily_history_and_back_it_up() {
        let temporary = tempfile::tempdir().unwrap();
        initialize(temporary.path(), None).await.unwrap();
        let canonical =
            tokio::fs::canonicalize(temporary.path()).await.unwrap();
        assert!(canonical.to_string_lossy().starts_with(r"\\?\"));
        let path = canonical.join("data/records.db");
        assert!(!path.to_string_lossy().contains('/'));
        let core = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(canonical.join("data/system.db"))
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        let pool = pool_for(&core).await.expect(
            "canonical core path should resolve the existing daily history",
        );
        sqlx::query("INSERT INTO instance_daily_playtime VALUES('2026-11-16','one','Example',42,1)")
            .execute(&pool).await.unwrap();
        pool.close().await;
        let destination = canonical.join("history-backup.db");
        assert!(backup(&canonical, &destination).await.unwrap());
        let mut snapshot = sqlx::SqliteConnection::connect_with(
            &SqliteConnectOptions::new()
                .filename(destination)
                .read_only(true),
        )
        .await
        .unwrap();
        let seconds: i64 = sqlx::query_scalar("SELECT played_seconds FROM instance_daily_playtime WHERE instance_id='one'")
            .fetch_one(&mut snapshot).await.unwrap();
        assert_eq!(seconds, 42);
        snapshot.close().await.unwrap();
    }
}

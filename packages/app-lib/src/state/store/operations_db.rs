//! Optional task history. No connection to this service is attached to the core.
use sqlx::{
    Executor, SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use std::path::Path;

pub(crate) const FILE: &str = "operations-history.db";

pub(crate) async fn open(root: &Path) -> crate::Result<SqlitePool> {
    let directory = crate::state::layout::cache(root);
    tokio::fs::create_dir_all(&directory).await?;
    let pool = SqlitePoolOptions::new()
        .max_connections(2)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(directory.join(FILE))
                .create_if_missing(true)
                .foreign_keys(false)
                .busy_timeout(std::time::Duration::from_secs(5)),
        )
        .await?;
    let result = async {
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
        tx.execute("CREATE TABLE IF NOT EXISTS history_contract(singleton INTEGER PRIMARY KEY CHECK(singleton=1),protocol TEXT NOT NULL,version INTEGER NOT NULL)").await?;
        tx.execute("INSERT OR IGNORE INTO history_contract VALUES(1,'axolotl-operation-history',1)").await?;
        let contract:(String,i64)=sqlx::query_as("SELECT protocol,version FROM history_contract WHERE singleton=1").fetch_one(&mut *tx).await?;
        if contract != ("axolotl-operation-history".into(),1) {
            return Err(crate::ErrorKind::InputError("Unsupported task history protocol retained".into()).into());
        }
        tx.execute("CREATE TABLE IF NOT EXISTS task_history(id TEXT PRIMARY KEY,instance_id TEXT,kind TEXT NOT NULL,status TEXT NOT NULL,created INTEGER NOT NULL,finished INTEGER,payload TEXT NOT NULL)").await?;
        tx.execute("CREATE INDEX IF NOT EXISTS task_history_instance ON task_history(instance_id,created)").await?;
        tx.commit().await?;
        Ok::<_,crate::Error>(())
    }.await;
    if let Err(error) = result {
        pool.close().await;
        return Err(error);
    }
    Ok(pool)
}

/// Only preserved experimental or already migrated images may feed import.
/// A missing new authority never permits reading an older channel database.
pub(crate) async fn legacy(root: &Path) -> crate::Result<Option<SqlitePool>> {
    for path in [
        root.join("databases/operations.db"),
        root.join(crate::state::layout::IMPORT_SNAPSHOT),
    ] {
        if !tokio::fs::try_exists(&path).await? {
            continue;
        }
        return Ok(Some(
            SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(
                    SqliteConnectOptions::new()
                        .filename(path)
                        .read_only(true)
                        .create_if_missing(false)
                        .busy_timeout(std::time::Duration::from_secs(5)),
                )
                .await?,
        ));
    }
    Ok(None)
}

/// Completion history is expendable; recovery never consults this table.
pub(crate) async fn archive(
    root: &Path,
    id: &str,
    instance: Option<&str>,
    kind: &str,
    status: &str,
    created: i64,
    finished: Option<i64>,
    payload: &str,
) -> crate::Result<()> {
    let pool = open(root).await?;
    let result=sqlx::query("INSERT INTO task_history(id,instance_id,kind,status,created,finished,payload) VALUES(?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET status=excluded.status,finished=excluded.finished,payload=excluded.payload")
        .bind(id).bind(instance).bind(kind).bind(status).bind(created).bind(finished).bind(payload).execute(&pool).await;
    pool.close().await;
    result?;
    Ok(())
}

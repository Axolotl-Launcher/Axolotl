use super::{CURRENT_APP_VERSION, open_read_only_db, read_stored_app_version};
use serde::{Deserialize, Serialize};
use sqlx::Connection;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const FULL_CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Debug, Deserialize, Serialize)]
struct StartupHealth {
    app_version: String,
    checked_at: u64,
    clean_shutdown: bool,
    database: Option<FileStamp>,
    wal: Option<FileStamp>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
struct FileStamp {
    len: u64,
    modified_nanos: u128,
}

fn health_path(database: &Path) -> PathBuf {
    super::sqlite_sidecar_path(database, ".startup-health.json")
}

fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

async fn file_stamp(path: &Path) -> std::io::Result<Option<FileStamp>> {
    match tokio::fs::metadata(path).await {
        Ok(metadata) => Ok(Some(FileStamp {
            len: metadata.len(),
            modified_nanos: metadata
                .modified()?
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
        })),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

async fn read_health(database: &Path) -> Option<StartupHealth> {
    serde_json::from_slice(&tokio::fs::read(health_path(database)).await.ok()?)
        .ok()
}

async fn write_health(
    database: &Path,
    health: &StartupHealth,
) -> crate::Result<()> {
    use tokio::io::AsyncWriteExt;
    let target = health_path(database);
    let temporary =
        target.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let result = async {
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .await?;
        file.write_all(&serde_json::to_vec(health)?).await?;
        file.sync_all().await?;
        drop(file);
        tokio::fs::rename(&temporary, &target).await?;
        Ok::<(), crate::Error>(())
    }
    .await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(temporary).await;
    }
    result
}

async fn can_skip_full_check(
    database: &Path,
    health: &StartupHealth,
    now: u64,
) -> crate::Result<bool> {
    if !health.clean_shutdown
        || health.app_version != CURRENT_APP_VERSION
        || now < health.checked_at
        || now - health.checked_at >= FULL_CHECK_INTERVAL.as_secs()
        || health.database.is_none()
        || file_stamp(database).await? != health.database
        || file_stamp(&super::sqlite_sidecar_path(database, "-wal")).await?
            != health.wal
    {
        return Ok(false);
    }
    let mut connection = open_read_only_db(database).await?;
    let version = read_stored_app_version(&mut connection).await?;
    connection.close().await?;
    Ok(version.as_deref() == Some(CURRENT_APP_VERSION))
}

/// Marks the session dirty before any startup writes; only a verified clean shutdown enables the fast path.
pub(crate) async fn prepare_app_db(database: &Path) -> crate::Result<()> {
    let previous = read_health(database).await;
    let now = now_seconds();
    let skip = match previous.as_ref() {
        Some(health) => {
            match can_skip_full_check(database, health, now).await {
                Ok(skip) => skip,
                Err(error) => {
                    tracing::debug!(%error, "Database startup metadata could not be validated; checking the full database");
                    false
                }
            }
        }
        None => false,
    };
    if !skip {
        super::restore_corrupt_app_db_if_needed(database).await?;
    }
    let checked_at = if skip {
        previous.unwrap().checked_at
    } else {
        now
    };
    write_health(
        database,
        &StartupHealth {
            app_version: CURRENT_APP_VERSION.to_string(),
            checked_at,
            clean_shutdown: false,
            database: None,
            wal: None,
        },
    )
    .await?;
    tracing::debug!(
        full_check = !skip,
        "App database startup health check completed"
    );
    Ok(())
}

/// Call after closing the pool, so no connection can write after the clean file stamps are captured.
pub(crate) async fn mark_clean_shutdown(database: &Path) -> crate::Result<()> {
    let Some(mut health) = read_health(database).await else {
        return Ok(());
    };
    health.database = file_stamp(database).await?;
    health.wal =
        file_stamp(&super::sqlite_sidecar_path(database, "-wal")).await?;
    health.clean_shutdown = true;
    write_health(database, &health).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::ConnectOptions;
    use sqlx::sqlite::SqliteConnectOptions;

    async fn fixture() -> (tempfile::TempDir, PathBuf) {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("app.db");
        let mut connection = SqliteConnectOptions::new()
            .filename(&database)
            .create_if_missing(true)
            .connect()
            .await
            .unwrap();
        sqlx::raw_sql("CREATE TABLE app_metadata(key TEXT PRIMARY KEY, value TEXT); CREATE TABLE marker(value TEXT);").execute(&mut connection).await.unwrap();
        sqlx::query("INSERT INTO app_metadata VALUES ('app_version', ?)")
            .bind(CURRENT_APP_VERSION)
            .execute(&mut connection)
            .await
            .unwrap();
        connection.close().await.unwrap();
        prepare_app_db(&database).await.unwrap();
        mark_clean_shutdown(&database).await.unwrap();
        (directory, database)
    }

    #[tokio::test]
    async fn clean_same_version_startup_skips_full_check_but_marks_the_new_session_dirty()
     {
        let (_directory, database) = fixture().await;
        let previous = read_health(&database).await.unwrap();
        assert!(
            can_skip_full_check(&database, &previous, now_seconds())
                .await
                .unwrap()
        );
        prepare_app_db(&database).await.unwrap();
        let dirty = read_health(&database).await.unwrap();
        assert!(!dirty.clean_shutdown);
        assert!(
            !can_skip_full_check(&database, &dirty, now_seconds())
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn version_changes_expiry_and_external_database_changes_require_full_checks()
     {
        let (_directory, database) = fixture().await;
        let mut health = read_health(&database).await.unwrap();
        let now = now_seconds();
        assert!(
            !can_skip_full_check(
                &database,
                &health,
                now + FULL_CHECK_INTERVAL.as_secs()
            )
            .await
            .unwrap()
        );
        assert!(
            !can_skip_full_check(&database, &health, health.checked_at - 1)
                .await
                .unwrap()
        );
        health.app_version = "different-software-version".into();
        assert!(!can_skip_full_check(&database, &health, now).await.unwrap());
        health.app_version = CURRENT_APP_VERSION.into();
        tokio::fs::write(
            super::super::sqlite_sidecar_path(&database, "-wal"),
            b"changed",
        )
        .await
        .unwrap();
        assert!(!can_skip_full_check(&database, &health, now).await.unwrap());
        tokio::fs::remove_file(super::super::sqlite_sidecar_path(
            &database, "-wal",
        ))
        .await
        .unwrap();
        let connection = open_read_only_db(&database).await.unwrap();
        connection.close().await.unwrap();
        tokio::fs::write(&database, b"replaced database")
            .await
            .unwrap();
        assert!(!can_skip_full_check(&database, &health, now).await.unwrap());
    }

    #[tokio::test]
    async fn closed_wal_pool_can_use_the_next_clean_startup_fast_path() {
        let (_directory, database) = fixture().await;
        prepare_app_db(&database).await.unwrap();
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(2)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(&database)
                    .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal),
            )
            .await
            .unwrap();
        sqlx::query("INSERT INTO marker VALUES ('written during session')")
            .execute(&pool)
            .await
            .unwrap();
        pool.close().await;
        mark_clean_shutdown(&database).await.unwrap();
        let health = read_health(&database).await.unwrap();
        assert!(
            can_skip_full_check(&database, &health, now_seconds())
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn missing_or_invalid_health_metadata_requires_a_new_verified_session()
     {
        let (_directory, database) = fixture().await;
        tokio::fs::write(health_path(&database), b"invalid metadata")
            .await
            .unwrap();
        prepare_app_db(&database).await.unwrap();
        assert!(!read_health(&database).await.unwrap().clean_shutdown);
        tokio::fs::write(&database, b"corrupt database without backup")
            .await
            .unwrap();
        assert!(prepare_app_db(&database).await.is_err());
    }
}

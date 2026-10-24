use sqlx::sqlite::{SqliteConnectOptions, SqliteConnection};
use sqlx::{ConnectOptions, Connection};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

mod startup;
pub(crate) use startup::{mark_clean_shutdown, prepare_app_db};

static SNAPSHOT_WRITE: tokio::sync::Mutex<()> =
    tokio::sync::Mutex::const_new(());

const CURRENT_APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const REQUIRED_APP_DB_TABLES: &[&str] =
    &["_sqlx_migrations", "instances", "settings"];
const MAX_APP_DB_BACKUPS: usize = 3;

enum IntegrityStatus {
    Healthy,
    Corrupt(String),
}

pub(crate) async fn restore_corrupt_app_db_if_needed(
    db_path: &Path,
) -> crate::Result<()> {
    if !db_path.try_exists()? {
        return Ok(());
    }

    let backup_dir = app_db_backup_dir_for(db_path)?;
    restore_corrupt_app_db_from(db_path, &backup_dir).await
}

async fn restore_corrupt_app_db_from(
    db_path: &Path,
    backup_dir: &Path,
) -> crate::Result<()> {
    let corruption = match check_database_integrity(db_path).await? {
        IntegrityStatus::Healthy => return Ok(()),
        IntegrityStatus::Corrupt(corruption) => corruption,
    };

    tracing::error!(
        database = %db_path.display(),
        corruption,
        "App database integrity check failed"
    );

    let Some(backup_path) = latest_healthy_app_db_backup(backup_dir).await?
    else {
        return Err(crate::ErrorKind::FSError(format!(
            "App database {} is corrupted, and no healthy backup is available in {}",
            db_path.display(),
            backup_dir.display()
        ))
        .into());
    };

    crate::util::io::create_dir_all(backup_dir).await?;
    let corrupt_path = next_corrupt_database_path(backup_dir).await?;
    let restore_staging_path = next_restore_staging_path(db_path).await?;

    tokio::fs::copy(&backup_path, &restore_staging_path).await?;
    if !matches!(
        check_database_integrity(&restore_staging_path).await?,
        IntegrityStatus::Healthy
    ) {
        cleanup_staged_database(&restore_staging_path).await;
        return Err(crate::ErrorKind::FSError(format!(
            "App database backup {} became invalid while preparing recovery",
            backup_path.display()
        ))
        .into());
    }

    if let Err(error) = remove_database_sidecars(&restore_staging_path).await {
        cleanup_staged_database(&restore_staging_path).await;
        return Err(error);
    }
    if let Err(error) = archive_database_files(db_path, &corrupt_path).await {
        cleanup_staged_database(&restore_staging_path).await;
        return Err(error);
    }
    if let Err(error) = tokio::fs::rename(&restore_staging_path, db_path).await
    {
        let rollback_result =
            restore_archived_database_files(&corrupt_path, db_path).await;
        cleanup_staged_database(&restore_staging_path).await;
        if let Err(rollback_error) = rollback_result {
            return Err(crate::ErrorKind::FSError(format!(
                "Failed to activate recovered app database: {error}; failed to restore original database: {rollback_error}"
            ))
            .into());
        }
        return Err(error.into());
    }

    tracing::warn!(
        database = %db_path.display(),
        backup = %backup_path.display(),
        corrupt_archive = %corrupt_path.display(),
        "Recovered corrupted app database from latest healthy backup"
    );

    Ok(())
}

async fn check_database_integrity(
    db_path: &Path,
) -> crate::Result<IntegrityStatus> {
    let started = std::time::Instant::now();
    // The await must happen before the macro: a temporary created inside the
    // field list would be held across the await point and make the caller's
    // future non-Send, which breaks every Tauri command that reaches it.
    let bytes = tokio::fs::metadata(db_path).await?.len();
    tracing::info!(
        database = %db_path.display(),
        bytes,
        "Checking app database integrity; large databases may take longer"
    );
    let options = SqliteConnectOptions::new()
        .filename(db_path)
        .busy_timeout(Duration::from_secs(30))
        .read_only(true)
        .create_if_missing(false);
    let mut conn = match options.connect().await {
        Ok(conn) => conn,
        Err(error) if is_sqlite_corruption(&error) => {
            return Ok(IntegrityStatus::Corrupt(error.to_string()));
        }
        Err(error) => return Err(error.into()),
    };

    let result = sqlx::query_scalar::<_, String>("PRAGMA quick_check(1)")
        .fetch_all(&mut conn)
        .await;
    let status = match result {
        Ok(rows) if rows.len() == 1 && rows[0] == "ok" => {
            IntegrityStatus::Healthy
        }
        Ok(rows) => IntegrityStatus::Corrupt(rows.join("; ")),
        Err(error) if is_sqlite_corruption(&error) => {
            IntegrityStatus::Corrupt(error.to_string())
        }
        Err(error) => return Err(error.into()),
    };
    conn.close().await?;

    tracing::info!(
        elapsed_ms = started.elapsed().as_millis(),
        "App database integrity check finished"
    );

    Ok(status)
}

fn is_sqlite_corruption(error: &sqlx::Error) -> bool {
    let sqlx::Error::Database(error) = error else {
        return false;
    };
    error
        .code()
        .and_then(|code| code.parse::<i32>().ok())
        .is_some_and(|code| matches!(code & 0xff, 11 | 26))
}

async fn latest_healthy_app_db_backup(
    backup_dir: &Path,
) -> crate::Result<Option<PathBuf>> {
    let mut candidates = Vec::new();
    let mut entries = match tokio::fs::read_dir(backup_dir).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(None);
        }
        Err(error) => return Err(error.into()),
    };

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|name| name.to_str())
        else {
            continue;
        };
        if !file_name.starts_with("app-db-before-")
            || path.extension().and_then(|extension| extension.to_str())
                != Some("db")
        {
            continue;
        }
        let modified = entry.metadata().await?.modified().unwrap_or(UNIX_EPOCH);
        candidates.push((modified, path));
    }
    candidates.sort_by(|left, right| right.0.cmp(&left.0));

    for (_, candidate) in candidates {
        if matches!(
            check_database_integrity(&candidate).await,
            Ok(IntegrityStatus::Healthy)
        ) && is_app_database(&candidate).await?
        {
            return Ok(Some(candidate));
        }
    }

    Ok(None)
}

async fn is_app_database(db_path: &Path) -> crate::Result<bool> {
    let mut conn = open_read_only_db(db_path).await?;
    let required_tables = serde_json::to_string(REQUIRED_APP_DB_TABLES)?;
    let table_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master
         WHERE type = 'table'
            AND name IN (SELECT value FROM json_each(?))",
    )
    .bind(required_tables)
    .fetch_one(&mut conn)
    .await?;
    conn.close().await?;

    Ok(table_count == REQUIRED_APP_DB_TABLES.len() as i64)
}

async fn archive_database_files(
    db_path: &Path,
    archive_path: &Path,
) -> crate::Result<()> {
    tokio::fs::rename(db_path, archive_path).await?;
    let mut archived_sidecars = Vec::new();

    for suffix in ["-wal", "-shm"] {
        let source = sqlite_sidecar_path(db_path, suffix);
        if !source.try_exists()? {
            continue;
        }
        let destination = sqlite_sidecar_path(archive_path, suffix);
        if let Err(error) = tokio::fs::rename(&source, &destination).await {
            for (archived, original) in archived_sidecars.into_iter().rev() {
                let _ = tokio::fs::rename(archived, original).await;
            }
            let _ = tokio::fs::rename(archive_path, db_path).await;
            return Err(error.into());
        }
        archived_sidecars.push((destination, source));
    }

    Ok(())
}

async fn restore_archived_database_files(
    archive_path: &Path,
    db_path: &Path,
) -> crate::Result<()> {
    tokio::fs::rename(archive_path, db_path).await?;
    for suffix in ["-wal", "-shm"] {
        let archived = sqlite_sidecar_path(archive_path, suffix);
        if archived.try_exists()? {
            tokio::fs::rename(archived, sqlite_sidecar_path(db_path, suffix))
                .await?;
        }
    }
    Ok(())
}

async fn next_corrupt_database_path(
    backup_dir: &Path,
) -> crate::Result<PathBuf> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    for suffix in 1.. {
        let suffix = (suffix > 1).then(|| format!("-{suffix}"));
        let path = backup_dir.join(format!(
            "app-db-corrupt-{timestamp}{}.db",
            suffix.as_deref().unwrap_or_default()
        ));
        if !path.try_exists()? {
            return Ok(path);
        }
    }
    unreachable!()
}

async fn next_restore_staging_path(db_path: &Path) -> crate::Result<PathBuf> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    for suffix in 1.. {
        let suffix = (suffix > 1).then(|| format!("-{suffix}"));
        let path = db_path.with_file_name(format!(
            "app.db.restore-{timestamp}{}.tmp",
            suffix.as_deref().unwrap_or_default()
        ));
        if !path.try_exists()? {
            return Ok(path);
        }
    }
    unreachable!()
}

fn sqlite_sidecar_path(db_path: &Path, suffix: &str) -> PathBuf {
    let mut path = OsString::from(db_path.as_os_str());
    path.push(suffix);
    PathBuf::from(path)
}

async fn remove_database_sidecars(db_path: &Path) -> crate::Result<()> {
    for suffix in ["-wal", "-shm"] {
        let path = sqlite_sidecar_path(db_path, suffix);
        match tokio::fs::remove_file(path).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

async fn cleanup_staged_database(db_path: &Path) {
    let _ = tokio::fs::remove_file(db_path).await;
    let _ = remove_database_sidecars(db_path).await;
}

pub(crate) async fn maybe_backup_existing_app_db(
    db_path: &Path,
) -> crate::Result<()> {
    let _guard = SNAPSHOT_WRITE.lock().await;
    if !db_path.try_exists()? {
        tracing::debug!(
            "Skipping pre-migration app database backup because {} does not exist",
            db_path.display()
        );
        return Ok(());
    }

    tracing::debug!(
        "Inspecting {} for a pre-migration app database backup",
        db_path.display()
    );

    let mut conn = match open_read_only_db(db_path).await {
        Ok(conn) => conn,
        Err(err) => {
            tracing::error!(
                "Failed to open {} read-only before migrations: {err}",
                db_path.display()
            );
            return Err(err);
        }
    };

    let has_user_tables = match has_user_tables(&mut conn).await {
        Ok(has_user_tables) => has_user_tables,
        Err(err) => {
            tracing::error!(
                "Failed to inspect app database tables before migrations: {err}"
            );
            return Err(err);
        }
    };
    if !has_user_tables {
        tracing::debug!(
            "Skipping pre-migration app database backup because {} has no app data tables",
            db_path.display()
        );
        return Ok(());
    }

    let stored_version = match read_stored_app_version(&mut conn).await {
        Ok(version) => version,
        Err(err) => {
            tracing::error!(
                "Failed to read stored app database version before migrations: {err}"
            );
            return Err(err);
        }
    };
    if stored_version.as_deref() == Some(CURRENT_APP_VERSION) {
        tracing::debug!(
            "Skipping pre-migration app database backup because app version is already recorded as {CURRENT_APP_VERSION}"
        );
        return Ok(());
    }

    let stored_version = stored_version.as_deref().unwrap_or("unknown");
    let backup_dir = match app_db_backup_dir_for(db_path) {
        Ok(path) => path,
        Err(err) => {
            tracing::error!(
                "Failed to resolve app database backup directory before migrations: {err}"
            );
            return Err(err);
        }
    };
    let backup_path = match next_backup_path(
        &backup_dir,
        stored_version,
        CURRENT_APP_VERSION,
    )
    .await
    {
        Ok(path) => path,
        Err(err) => {
            tracing::error!(
                "Failed to choose app database backup path in {} before migrations: {err}",
                backup_dir.display()
            );
            return Err(err);
        }
    };

    tracing::info!(
        "Creating pre-migration app database backup from version {stored_version} before opening with version {CURRENT_APP_VERSION} at {}",
        backup_path.display()
    );

    if let Err(err) =
        create_verified_sqlite_snapshot(&mut conn, &backup_path).await
    {
        tracing::error!(
            "Failed to create pre-migration app database backup at {}: {err}",
            backup_path.display()
        );
        return Err(err);
    }

    tracing::info!(
        "Created pre-migration app database backup at {}",
        backup_path.display()
    );

    Ok(())
}

pub(crate) async fn backup_app_db_for_update(
    db_path: &Path,
    target_version: &str,
) -> crate::Result<PathBuf> {
    let _guard = SNAPSHOT_WRITE.lock().await;
    if !db_path.try_exists()? {
        return Err(crate::ErrorKind::FSError(format!(
            "Cannot back up missing app database {}",
            db_path.display()
        ))
        .into());
    }

    let channel = db_path
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .unwrap_or("release");
    let backup_dir = app_db_backup_dir_for(db_path)?;
    crate::util::io::create_dir_all(&backup_dir).await?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let backup_path = backup_dir.join(format!(
        "app-db-before-update-{}-{}-{timestamp}.db",
        sanitize_version_for_filename(channel),
        sanitize_version_for_filename(target_version),
    ));

    let mut conn = open_read_only_db(db_path).await?;
    create_verified_sqlite_snapshot(&mut conn, &backup_path).await?;
    conn.close().await?;

    tracing::info!(
        database = %db_path.display(),
        backup = %backup_path.display(),
        target_version,
        "Created app database backup before update"
    );
    Ok(backup_path)
}

pub(crate) async fn maintain_app_db_backups(
    database: &Path,
) -> crate::Result<()> {
    let _guard = SNAPSHOT_WRITE.lock().await;
    prune_app_db_backups(&app_db_backup_dir_for(database)?).await
}

async fn open_read_only_db(db_path: &Path) -> crate::Result<SqliteConnection> {
    let conn_options = SqliteConnectOptions::new()
        .filename(db_path)
        .busy_timeout(Duration::from_secs(30))
        .read_only(true)
        .create_if_missing(false);

    Ok(conn_options.connect().await?)
}

pub fn app_db_backup_dir() -> crate::Result<PathBuf> {
    if let Some(path) = std::env::var_os("THESEUS_DB_BACKUP_DIR") {
        return Ok(PathBuf::from(path));
    }

    let app_identifier = if let Some(dir_info) =
        crate::state::DirectoryInfo::global_handle_if_ready()
    {
        dir_info.app_identifier.clone()
    } else {
        crate::brand::BUNDLE_IDENTIFIER.to_string()
    };

    let base =
        crate::state::DirectoryInfo::initial_settings_dir_path(&app_identifier)
            .ok_or(crate::ErrorKind::FSError(
                "Could not find valid config dir for app database backups"
                    .to_string(),
            ))?;

    Ok(base.join("Backups").join("app-db"))
}

fn app_db_backup_dir_for(db_path: &Path) -> crate::Result<PathBuf> {
    let settings_dir =
        db_path.parent().and_then(Path::parent).ok_or_else(|| {
            crate::ErrorKind::FSError(format!(
                "App database path {} has no settings directory",
                db_path.display()
            ))
        })?;

    let backup_dir = std::env::var_os("THESEUS_DB_BACKUP_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| settings_dir.join("Backups").join("app-db"));
    match db_path
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
    {
        Some("beta" | "release") => Ok(backup_dir.join(
            db_path
                .parent()
                .and_then(Path::file_name)
                .expect("database channel directory has a name"),
        )),
        _ => Ok(backup_dir),
    }
}

async fn prune_app_db_backups(backup_dir: &Path) -> crate::Result<()> {
    let mut candidates = Vec::new();
    let mut entries = match tokio::fs::read_dir(backup_dir).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    };

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if name.starts_with("app-db-before-")
            && path.extension().and_then(|extension| extension.to_str())
                == Some("db")
        {
            let modified =
                entry.metadata().await?.modified().unwrap_or(UNIX_EPOCH);
            candidates.push((modified, path));
        }
    }

    candidates.sort_by(|left, right| right.0.cmp(&left.0));
    let protected_prefix = format!(
        "app-db-before-{}-from-",
        sanitize_version_for_filename(CURRENT_APP_VERSION)
    );
    let protected = candidates
        .iter()
        .find(|(_, path)| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(&protected_prefix))
        })
        .map(|(_, path)| path.clone());
    let mut retained = candidates
        .iter()
        .take(MAX_APP_DB_BACKUPS)
        .map(|(_, path)| path.clone())
        .collect::<Vec<_>>();
    if let Some(protected) = protected
        && !retained.contains(&protected)
    {
        if retained.len() == MAX_APP_DB_BACKUPS {
            retained.pop();
        }
        retained.push(protected);
    }
    for (_, path) in candidates
        .into_iter()
        .filter(|(_, path)| !retained.contains(path))
    {
        if let Err(error) = tokio::fs::remove_file(&path).await
            && error.kind() != std::io::ErrorKind::NotFound
        {
            return Err(error.into());
        }
    }
    Ok(())
}

async fn has_user_tables(conn: &mut SqliteConnection) -> crate::Result<bool> {
    let count = sqlx::query_scalar!(
        "
		SELECT COUNT(*)
		FROM sqlite_master
		WHERE type = 'table'
			AND name NOT LIKE 'sqlite_%'
			AND name NOT IN ('_sqlx_migrations', 'app_metadata')
		",
    )
    .fetch_one(&mut *conn)
    .await?;

    Ok(count > 0)
}

async fn read_stored_app_version(
    conn: &mut SqliteConnection,
) -> crate::Result<Option<String>> {
    if !has_table(conn, "app_metadata").await? {
        return Ok(None);
    }

    Ok(sqlx::query_scalar!(
        "SELECT value FROM app_metadata WHERE key = 'app_version'"
    )
    .fetch_optional(&mut *conn)
    .await?)
}

async fn has_table(
    conn: &mut SqliteConnection,
    table_name: &str,
) -> crate::Result<bool> {
    let count = sqlx::query_scalar!(
        "
		SELECT COUNT(*)
		FROM sqlite_master
		WHERE type = 'table' AND name = ?
		",
        table_name,
    )
    .fetch_one(&mut *conn)
    .await?;

    Ok(count > 0)
}

async fn next_backup_path(
    backup_dir: &Path,
    stored_version: &str,
    current_version: &str,
) -> crate::Result<PathBuf> {
    crate::util::io::create_dir_all(backup_dir).await?;

    let stored_version = sanitize_version_for_filename(stored_version);
    let current_version = sanitize_version_for_filename(current_version);

    let backup_path = backup_dir.join(format!(
        "app-db-before-{current_version}-from-{stored_version}.db"
    ));
    if !backup_path.try_exists()? {
        return Ok(backup_path);
    }

    for suffix in 2.. {
        let backup_path = backup_dir.join(format!(
            "app-db-before-{current_version}-from-{stored_version}-{suffix}.db"
        ));
        if !backup_path.try_exists()? {
            return Ok(backup_path);
        }
    }

    unreachable!()
}

fn sanitize_version_for_filename(version: &str) -> String {
    let mut sanitized = String::new();
    let mut replaced_last_char = false;

    for character in version.chars() {
        if character.is_ascii_alphanumeric()
            || character == '.'
            || character == '-'
            || character == '_'
        {
            sanitized.push(character);
            replaced_last_char = false;
        } else if !replaced_last_char {
            sanitized.push('-');
            replaced_last_char = true;
        }
    }

    let sanitized = sanitized.trim_matches(&['.', '-', '_'][..]);
    if sanitized.is_empty() {
        "unknown".to_string()
    } else {
        sanitized.to_string()
    }
}

async fn create_sqlite_snapshot(
    conn: &mut SqliteConnection,
    backup_path: &Path,
) -> crate::Result<()> {
    let backup_path = backup_path
        .to_str()
        .ok_or_else(|| crate::ErrorKind::UTFError(backup_path.to_path_buf()))?;

    sqlx::query!("VACUUM INTO ?", backup_path)
        .execute(&mut *conn)
        .await?;

    Ok(())
}

async fn create_verified_sqlite_snapshot(
    conn: &mut SqliteConnection,
    backup_path: &Path,
) -> crate::Result<()> {
    let staging =
        backup_path.with_extension(format!("{}.partial", uuid::Uuid::new_v4()));
    let result = async {
        create_sqlite_snapshot(conn, &staging).await?;
        if !matches!(
            check_database_integrity(&staging).await?,
            IntegrityStatus::Healthy
        ) {
            return Err(crate::ErrorKind::FSError(
                "Database backup failed its integrity check".to_string(),
            )
            .into());
        }
        tokio::fs::OpenOptions::new()
            .write(true)
            .open(&staging)
            .await?
            .sync_all()
            .await?;
        tokio::fs::rename(&staging, backup_path).await?;
        Ok(())
    }
    .await;
    if result.is_err() {
        cleanup_staged_database(&staging).await;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn set_app_version(path: &Path, version: &str) {
        let mut conn = SqliteConnectOptions::new()
            .filename(path)
            .connect()
            .await
            .unwrap();
        sqlx::query("CREATE TABLE IF NOT EXISTS app_metadata(key TEXT PRIMARY KEY, value TEXT)").execute(&mut conn).await.unwrap();
        sqlx::query(
            "INSERT OR REPLACE INTO app_metadata VALUES ('app_version', ?)",
        )
        .bind(version)
        .execute(&mut conn)
        .await
        .unwrap();
        conn.close().await.unwrap();
    }

    #[tokio::test]
    async fn pre_migration_backup_is_triggered_by_software_version_and_preserves_the_old_database()
     {
        let temp = tempfile::tempdir().unwrap();
        let database = temp.path().join("release").join("app.db");
        tokio::fs::create_dir_all(database.parent().unwrap())
            .await
            .unwrap();
        create_test_app_db(&database, "before-migration").await;
        set_app_version(&database, "previous-software-version").await;
        maybe_backup_existing_app_db(&database).await.unwrap();
        let backup_dir = app_db_backup_dir_for(&database).unwrap();
        let backup = latest_healthy_app_db_backup(&backup_dir)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(read_marker(&backup).await, "before-migration");
        let mut source = SqliteConnectOptions::new()
            .filename(&database)
            .connect()
            .await
            .unwrap();
        sqlx::query("UPDATE recovery_marker SET value = 'after-migration'")
            .execute(&mut source)
            .await
            .unwrap();
        source.close().await.unwrap();
        set_app_version(&database, CURRENT_APP_VERSION).await;
        maybe_backup_existing_app_db(&database).await.unwrap();
        assert_eq!(read_marker(&backup).await, "before-migration");
        let mut files = tokio::fs::read_dir(&backup_dir).await.unwrap();
        let mut count = 0;
        while files.next_entry().await.unwrap().is_some() {
            count += 1;
        }
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn snapshot_failure_cannot_publish_a_backup_or_modify_the_source() {
        let temp = tempfile::tempdir().unwrap();
        let database = temp.path().join("app.db");
        create_test_app_db(&database, "unchanged").await;
        let target = temp.path().join("missing-directory").join("snapshot.db");
        let mut source = open_read_only_db(&database).await.unwrap();
        assert!(
            create_verified_sqlite_snapshot(&mut source, &target)
                .await
                .is_err()
        );
        source.close().await.unwrap();
        assert!(!target.exists());
        assert_eq!(read_marker(&database).await, "unchanged");
    }

    #[tokio::test]
    async fn verified_snapshots_include_committed_wal_changes_without_copying_the_live_database_file()
     {
        let temp = tempfile::tempdir().unwrap();
        let database = temp.path().join("release").join("app.db");
        tokio::fs::create_dir_all(database.parent().unwrap())
            .await
            .unwrap();
        create_test_app_db(&database, "before-wal").await;
        let mut writer = SqliteConnectOptions::new()
            .filename(&database)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .pragma("wal_autocheckpoint", "0")
            .connect()
            .await
            .unwrap();
        sqlx::query("UPDATE recovery_marker SET value = 'committed-in-wal'")
            .execute(&mut writer)
            .await
            .unwrap();
        let snapshot = backup_app_db_for_update(&database, "test-update")
            .await
            .unwrap();
        assert_eq!(read_marker(&snapshot).await, "committed-in-wal");
        writer.close().await.unwrap();
    }

    #[tokio::test]
    async fn housekeeping_protects_the_current_versions_pre_migration_snapshot()
    {
        let temp = tempfile::tempdir().unwrap();
        let database = temp.path().join("beta").join("app.db");
        tokio::fs::create_dir_all(database.parent().unwrap())
            .await
            .unwrap();
        create_test_app_db(&database, "current").await;
        let directory = app_db_backup_dir_for(&database).unwrap();
        tokio::fs::create_dir_all(&directory).await.unwrap();
        let anchor = directory.join(format!(
            "app-db-before-{}-from-old.db",
            sanitize_version_for_filename(CURRENT_APP_VERSION)
        ));
        create_test_app_db(&anchor, "downgrade-point").await;
        std::fs::OpenOptions::new()
            .write(true)
            .open(&anchor)
            .unwrap()
            .set_modified(UNIX_EPOCH + Duration::from_secs(1))
            .unwrap();
        for index in 0..MAX_APP_DB_BACKUPS {
            create_test_app_db(
                &directory.join(format!("app-db-before-update-{index}.db")),
                "newer",
            )
            .await;
        }
        maintain_app_db_backups(&database).await.unwrap();
        assert_eq!(read_marker(&anchor).await, "downgrade-point");
        let mut files = tokio::fs::read_dir(directory).await.unwrap();
        let mut count = 0;
        while files.next_entry().await.unwrap().is_some() {
            count += 1;
        }
        assert_eq!(count, MAX_APP_DB_BACKUPS);
    }

    async fn create_test_app_db(path: &Path, marker: &str) {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true);
        let mut conn = options.connect().await.unwrap();
        sqlx::raw_sql(
            "CREATE TABLE _sqlx_migrations (version INTEGER PRIMARY KEY);
             CREATE TABLE instances (id TEXT PRIMARY KEY);
             CREATE TABLE settings (id INTEGER PRIMARY KEY);
             CREATE TABLE recovery_marker (value TEXT NOT NULL);",
        )
        .execute(&mut conn)
        .await
        .unwrap();
        sqlx::query("INSERT INTO recovery_marker (value) VALUES (?)")
            .bind(marker)
            .execute(&mut conn)
            .await
            .unwrap();
        conn.close().await.unwrap();
    }

    async fn read_marker(path: &Path) -> String {
        let mut conn = open_read_only_db(path).await.unwrap();
        let marker = sqlx::query_scalar("SELECT value FROM recovery_marker")
            .fetch_one(&mut conn)
            .await
            .unwrap();
        conn.close().await.unwrap();
        marker
    }

    #[tokio::test]
    async fn healthy_database_is_not_replaced() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("app.db");
        let backup_dir = temp.path().join("Backups").join("app-db");
        tokio::fs::create_dir_all(&backup_dir).await.unwrap();
        create_test_app_db(&db_path, "current").await;
        create_test_app_db(
            &backup_dir.join("app-db-before-test-from-old.db"),
            "backup",
        )
        .await;

        restore_corrupt_app_db_from(&db_path, &backup_dir)
            .await
            .unwrap();

        assert_eq!(read_marker(&db_path).await, "current");
    }

    #[tokio::test]
    async fn corrupted_database_restores_latest_healthy_backup() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("app.db");
        let backup_dir = temp.path().join("Backups").join("app-db");
        tokio::fs::create_dir_all(&backup_dir).await.unwrap();
        tokio::fs::write(&db_path, b"not a sqlite database")
            .await
            .unwrap();
        create_test_app_db(
            &backup_dir.join("app-db-before-test-from-old.db"),
            "backup",
        )
        .await;

        restore_corrupt_app_db_from(&db_path, &backup_dir)
            .await
            .unwrap();

        assert_eq!(read_marker(&db_path).await, "backup");
        let mut entries = tokio::fs::read_dir(&backup_dir).await.unwrap();
        let mut found_corrupt_archive = false;
        while let Some(entry) = entries.next_entry().await.unwrap() {
            found_corrupt_archive |= entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with("app-db-corrupt-"));
        }
        assert!(found_corrupt_archive);
    }

    #[tokio::test]
    async fn corrupted_database_without_backup_is_left_untouched() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("app.db");
        let backup_dir = temp.path().join("Backups").join("app-db");
        let corrupt_bytes = b"not a sqlite database";
        tokio::fs::write(&db_path, corrupt_bytes).await.unwrap();

        let error = restore_corrupt_app_db_from(&db_path, &backup_dir)
            .await
            .expect_err("recovery must require a healthy backup");

        assert!(error.to_string().contains("no healthy backup"));
        assert_eq!(tokio::fs::read(&db_path).await.unwrap(), corrupt_bytes);
    }

    #[tokio::test]
    async fn update_backup_creates_a_healthy_channel_snapshot() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("release").join("app.db");
        tokio::fs::create_dir_all(db_path.parent().unwrap())
            .await
            .unwrap();
        create_test_app_db(&db_path, "release").await;

        let backup_path =
            backup_app_db_for_update(&db_path, "1.10.0").await.unwrap();

        assert!(backup_path.exists());
        assert_eq!(
            backup_path.parent().unwrap(),
            temp.path().join("Backups").join("app-db").join("release")
        );
        assert_eq!(read_marker(&backup_path).await, "release");
        assert!(matches!(
            check_database_integrity(&backup_path).await.unwrap(),
            IntegrityStatus::Healthy
        ));
    }

    #[tokio::test]
    async fn update_backup_defers_pruning_until_background_maintenance() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("release").join("app.db");
        tokio::fs::create_dir_all(db_path.parent().unwrap())
            .await
            .unwrap();
        create_test_app_db(&db_path, "release").await;

        let backup_dir =
            temp.path().join("Backups").join("app-db").join("release");
        tokio::fs::create_dir_all(&backup_dir).await.unwrap();
        for index in 0..3 {
            create_test_app_db(
                &backup_dir.join(format!("app-db-before-old-{index}.db")),
                "old",
            )
            .await;
        }

        backup_app_db_for_update(&db_path, "1.10.0").await.unwrap();

        let mut before = tokio::fs::read_dir(&backup_dir).await.unwrap();
        let mut count = 0;
        while before.next_entry().await.unwrap().is_some() {
            count += 1;
        }
        assert_eq!(count, MAX_APP_DB_BACKUPS + 1);
        maintain_app_db_backups(&db_path).await.unwrap();

        let mut entries = tokio::fs::read_dir(&backup_dir).await.unwrap();
        let mut snapshots = 0;
        while let Some(entry) = entries.next_entry().await.unwrap() {
            if entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with("app-db-before-"))
            {
                snapshots += 1;
            }
        }
        assert_eq!(snapshots, MAX_APP_DB_BACKUPS);
    }
}

use crate::state::DirectoryInfo;
use fs4::tokio::AsyncFileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha384};
use sqlx::migrate::{Migration, Migrator};
use sqlx::sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions,
};
use sqlx::{Pool, Sqlite};
use std::path::{Path, PathBuf};
use std::time::Duration;

static MIGRATOR: Migrator = sqlx::migrate!();

const UPDATE_CHANNEL_STATE_FILE: &str = "update-channel.json";
const LEGACY_APP_DB_FILE: &str = "app.db";
// Records an in-flight channel database reconciliation inside the target
// channel directory; see reconcile_default_channel_database.
const CHANNEL_RECONCILE_MARKER_FILE: &str = ".channel-reconcile";
// Serializes concurrent channel database reconciliations across processes
// (see reconcile_default_channel_database).
const CHANNEL_RECONCILE_LOCK_FILE: &str = ".channel-reconcile.lock";

const INITIAL_MIGRATION_VERSION: i64 = 20240711194701;
const COLLIDING_JAVA_DISCOVERY_MIGRATION_VERSION: i64 = 20260722120000;
const JAVA_DISCOVERY_MIGRATION_VERSION: i64 = 20260722120001;
const TEMPORARY_JAVA_DISCOVERY_MIGRATION_VERSION: i64 = 20260723121000;
const COLLIDING_HOME_DASHBOARD_MIGRATION_VERSION: i64 = 20260725170000;
const HOME_DASHBOARD_MIGRATION_VERSION: i64 = 20260727110000;
const PROVIDER_QUALIFIED_CONTENT_MIGRATION_VERSION: i64 = 20260728100000;
const OFFICIAL_PREFERRED_DOWNLOAD_SOURCE_MIGRATION_VERSION: i64 =
    20260802121000;
#[cfg(test)]
const RECONCILE_PROVIDER_QUALIFIED_CONTENT_MIGRATION_VERSION: i64 =
    20260728110000;
#[cfg(test)]
const JAVA_DEFAULT_VERSIONS_MIGRATION_VERSION: i64 = 20260729110000;
#[cfg(test)]
const SYSTEM_PROXY_SETTING_MIGRATION_VERSION: i64 = 20260802122000;
const INSTANCE_CONTENT_OWNERSHIP_MIGRATION_VERSION: i64 = 20260803120000;
#[cfg(test)]
const CONTENT_DEPENDENCY_EDGES_MIGRATION_VERSION: i64 = 20260815000000;
#[cfg(test)]
const CONTENT_DEPENDENCY_LOCAL_PROVIDER_MIGRATION_VERSION: i64 = 20260815000001;
#[cfg(test)]
const CONTENT_DEPENDENCY_BACKFILL_MARKER_MIGRATION_VERSION: i64 =
    20260815000002;
#[cfg(test)]
const CONTENT_DEPENDENCY_ENDPOINT_PROVIDERS_MIGRATION_VERSION: i64 =
    20260817130000;
#[cfg(test)]
const CURSEFORGE_DOWNLOAD_RESTRICTION_BYPASS_MIGRATION_VERSION: i64 =
    20260817120000;
const AI_PROVIDER_MIGRATION_VERSION: i64 = 20260805120000;
const PROXY_CONFIG_MIGRATION_VERSION: i64 = 20260820120000;
const CONTENT_FAVORITES_MIGRATION_VERSION: i64 = 20260820200000;

// This migration was changed by the launcher rebrand after it had already
// shipped. Keep the checksums of the original LF and CRLF variants so existing
// installations can move to the current canonical migration without losing
// their database.
const LEGACY_INITIAL_MIGRATION_CHECKSUMS: &[&str] = &[
    "49364b3e1b0d0169579ed93eb1f8e215216b84300a816891d0d922d3e03c69101e17e2bbe91ac1f54234c77cbd6b8bc3",
    "d95bfef1c3b2b530d2efd810202c85f93a9342ab40497b15653eea9b129806333cf610eebcecfa91accaa53a14bfc5df",
];
const COLLIDING_JAVA_DISCOVERY_MIGRATION_CHECKSUMS: &[&str] = &[
    "986c9afb410ad7086617c3707611c3b9a46be69bc33e2a0bd1b32611266301f536e28137a47b11337622a953c29ad595",
    "bfb8686214294786f8e81ea05f06bb08deeb4183da3d1230ebf379bc2ba9f5c5521f3590306bea668c292e03c3aacd85",
];
const TEMPORARY_JAVA_DISCOVERY_MIGRATION_CHECKSUMS: &[&str] = &[
    "35cbd4e0a4528bee302f06000e0971aad2f575488ebb5c04ec6849e15efc6f3f996395c11f9cc431c62ba4d9e3a41cc3",
    "7b99e048d7eb88cbfdd913cc3d799c6acabd58142204cde336da593fa3ca6d4f44336fd5d42a5822ab1e0b485352eb9b",
];
const COLLIDING_HOME_DASHBOARD_MIGRATION_CHECKSUMS: &[&str] = &[
    "d75220a25e54880d29ba179c0e7ca04e975b8ac9cc35c1e3569300b15570a0cd7239b353a2fe5733105b76a56215fda2",
    "f2806d4e16a055545dc8b90ced7896c85c1c26a5c8ec00fd44fe86fee6e4fe8c2413dfa20b16d1a8e227a0b55fa1e766",
];
const LEGACY_PROVIDER_QUALIFIED_CONTENT_MIGRATION_CHECKSUMS: &[&str] = &[
    "58ca9f7fe905f3d51ce68e03422f0f60035f4d4278738d2432869528d8a5951b4c053090a7e605e32797206b5e744010",
];
const LEGACY_OFFICIAL_PREFERRED_DOWNLOAD_SOURCE_MIGRATION_CHECKSUMS: &[&str] =
    &[
        "2fe6c8d9276c35e9400ab2e56ee1f30209db192c4edffc1dc6987bd79cbc04a101d0b2988dac1ba34d77df2acf869a5e",
    ];
const LEGACY_AI_PROVIDER_MIGRATION_CHECKSUMS: &[&str] = &[
    "eb25e9694a8a2e1787a399635a17c90b9785cc5599d79e31a72c15017422efaeb36cd0b6ec349368128097da7ecaafc1",
];

pub(crate) async fn connect(
    app_identifier: &str,
) -> crate::Result<Pool<Sqlite>> {
    let settings_dir = DirectoryInfo::initial_settings_dir_path(app_identifier)
        .ok_or(crate::ErrorKind::FSError(
            "Could not find valid config dir".to_string(),
        ))?;

    crate::util::io::create_dir_all(&settings_dir).await?;

    migrate_legacy_release_database(&settings_dir).await?;
    reconcile_default_channel_database(&settings_dir).await?;
    let db_path = app_db_path(&settings_dir).await?;
    let db_dir = db_path.parent().ok_or_else(|| {
        crate::ErrorKind::FSError(format!(
            "App database path {} has no parent directory",
            db_path.display()
        ))
    })?;
    crate::util::io::create_dir_all(db_dir).await?;

    connect_app_db(&db_path).await
}

pub async fn copy_release_database_to_beta(
    app_identifier: &str,
) -> crate::Result<()> {
    let settings_dir = DirectoryInfo::initial_settings_dir_path(app_identifier)
        .ok_or(crate::ErrorKind::FSError(
            "Could not find valid config dir".to_string(),
        ))?;
    let release_path = settings_dir.join("release").join(LEGACY_APP_DB_FILE);
    if !release_path.try_exists()? {
        return Err(crate::ErrorKind::FSError(
            "The Release database does not exist".to_string(),
        )
        .into());
    }

    let beta_dir = settings_dir.join("beta");
    crate::util::io::create_dir_all(&beta_dir).await?;
    let beta_path = beta_dir.join(LEGACY_APP_DB_FILE);
    if beta_path.try_exists()? {
        return Err(crate::ErrorKind::FSError(
            "The Beta database already exists".to_string(),
        )
        .into());
    }

    let release_pool = open_app_db_pool(&release_path).await?;
    let escaped_path = beta_path.to_string_lossy().replace('\'', "''");
    sqlx::query(&format!("VACUUM INTO '{escaped_path}'"))
        .execute(&release_pool)
        .await?;
    release_pool.close().await;

    if let Err(error) = open_app_db_pool(&beta_path).await {
        let _ = tokio::fs::remove_file(&beta_path).await;
        return Err(error);
    }

    tracing::info!(
        source = %release_path.display(),
        destination = %beta_path.display(),
        "Copied the Release database into the Beta update channel"
    );
    Ok(())
}

pub async fn beta_database_exists(app_identifier: &str) -> crate::Result<bool> {
    let settings_dir = DirectoryInfo::initial_settings_dir_path(app_identifier)
        .ok_or(crate::ErrorKind::FSError(
            "Could not find valid config dir".to_string(),
        ))?;
    Ok(settings_dir
        .join("beta")
        .join(LEGACY_APP_DB_FILE)
        .try_exists()?)
}

pub async fn current_app_database_path(
    app_identifier: &str,
) -> crate::Result<PathBuf> {
    let settings_dir = DirectoryInfo::initial_settings_dir_path(app_identifier)
        .ok_or(crate::ErrorKind::FSError(
            "Could not find valid config dir".to_string(),
        ))?;
    app_db_path(&settings_dir).await
}

pub async fn copy_database_between_channels(
    app_identifier: &str,
    source_channel: &str,
    target_channel: &str,
) -> crate::Result<()> {
    if !matches!(source_channel, "release" | "beta")
        || !matches!(target_channel, "release" | "beta")
        || source_channel == target_channel
    {
        return Err(crate::ErrorKind::InputError(
            "Database channels must be different Release or Beta channels"
                .to_string(),
        )
        .into());
    }

    let settings_dir = DirectoryInfo::initial_settings_dir_path(app_identifier)
        .ok_or(crate::ErrorKind::FSError(
            "Could not find valid config dir".to_string(),
        ))?;
    let active_channel = resolve_update_channel(&settings_dir).await?;
    if target_channel == active_channel {
        return Err(crate::ErrorKind::InputError(
            "The active database cannot be overwritten while Axolotl is running".to_string(),
        )
        .into());
    }

    let source_path =
        settings_dir.join(source_channel).join(LEGACY_APP_DB_FILE);
    let target_dir = settings_dir.join(target_channel);
    let target_path = target_dir.join(LEGACY_APP_DB_FILE);
    if !source_path.try_exists()? {
        return Err(crate::ErrorKind::FSError(format!(
            "The {source_channel} database does not exist"
        ))
        .into());
    }

    crate::util::io::create_dir_all(&target_dir).await?;
    let temporary_path = target_dir.join(format!("{LEGACY_APP_DB_FILE}.tmp"));
    let source_pool = open_app_db_pool(&source_path).await?;
    let escaped_path = temporary_path.to_string_lossy().replace('\'', "''");
    let result = sqlx::query(&format!("VACUUM INTO '{escaped_path}'"))
        .execute(&source_pool)
        .await;
    source_pool.close().await;
    result?;

    tokio::fs::rename(&temporary_path, &target_path).await?;
    tracing::info!(
        source = %source_path.display(),
        destination = %target_path.display(),
        "Overwrote the inactive update channel database"
    );
    Ok(())
}

pub async fn backup_current_app_db_for_update(
    app_identifier: &str,
    target_version: &str,
) -> crate::Result<PathBuf> {
    let settings_dir = DirectoryInfo::initial_settings_dir_path(app_identifier)
        .ok_or(crate::ErrorKind::FSError(
            "Could not find valid config dir".to_string(),
        ))?;
    let db_path = app_db_path(&settings_dir).await?;
    super::db_backup::backup_app_db_for_update(&db_path, target_version).await
}

async fn app_db_path(settings_dir: &Path) -> crate::Result<PathBuf> {
    let channel = resolve_update_channel(settings_dir).await?;
    Ok(settings_dir.join(channel).join(LEGACY_APP_DB_FILE))
}

async fn resolve_update_channel(
    settings_dir: &Path,
) -> crate::Result<&'static str> {
    Ok(read_explicit_update_channel(settings_dir)
        .await?
        .unwrap_or_else(default_update_channel))
}

/// Persisted update channel state of the launcher.
///
/// Serialized to `update-channel.json` inside the settings directory (see
/// [`update_channel_state_file_path`]). A missing file, or a file without an
/// `active_channel` value, means the user has not chosen a channel yet; see
/// [`default_update_channel`].
#[derive(Default, Deserialize, Serialize)]
pub struct UpdateChannelState {
    pub active_channel: Option<String>,
    pub immediate_update_fetch: Option<bool>,
    pub updates_paused: Option<bool>,
}

/// Path of the update channel state file inside the settings directory.
pub fn update_channel_state_file_path(settings_dir: &Path) -> PathBuf {
    settings_dir.join(UPDATE_CHANNEL_STATE_FILE)
}

/// Reads the persisted update channel state.
///
/// A missing file is reported as the default state; malformed contents are
/// reported as an error so each caller can decide how strictly to treat them.
pub async fn read_update_channel_state(
    settings_dir: &Path,
) -> crate::Result<UpdateChannelState> {
    let path = update_channel_state_file_path(settings_dir);
    match tokio::fs::read_to_string(&path).await {
        Ok(contents) => serde_json::from_str(&contents).map_err(|error| {
            crate::ErrorKind::OtherError(format!(
                "Failed to parse update channel state {}: {error}",
                path.display()
            ))
            .into()
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(UpdateChannelState::default())
        }
        Err(error) => Err(error.into()),
    }
}

/// Reads the update channel the user explicitly chose, if any.
///
/// Returns `None` when the user has not made a choice yet: either the
/// `update-channel.json` file is missing, or it carries no `active_channel`
/// value. This lets fresh installs fall back to the channel of the current
/// build (see [`default_update_channel`]) instead of always defaulting to the
/// Release channel. Unreadable or malformed state files are treated as "no
/// choice" here to keep startup resilient; strict callers can use
/// [`read_update_channel_state`] directly.
async fn read_explicit_update_channel(
    settings_dir: &Path,
) -> crate::Result<Option<&'static str>> {
    let Some(channel) = read_update_channel_state(settings_dir)
        .await
        .ok()
        .and_then(|state| state.active_channel)
    else {
        return Ok(None);
    };

    match channel.as_str() {
        "release" => Ok(Some("release")),
        "beta" => Ok(Some("beta")),
        other => {
            let path = update_channel_state_file_path(settings_dir);
            Err(crate::ErrorKind::FSError(format!(
                "Invalid update channel {other:?} in {}",
                path.display()
            ))
            .into())
        }
    }
}

/// Resolves the default update channel for a given app version.
///
/// The Release channel is the only stable channel, so any version carrying a
/// pre-release segment (`-beta`, `-rc`, ...) belongs to a pre-release channel.
/// The launcher currently ships a single pre-release channel, Beta; additional
/// channels can be mapped here as they are introduced.
fn default_update_channel_for(version: &str) -> &'static str {
    match version.split_once('-') {
        None => "release",
        Some(_) => "beta",
    }
}

/// Default update channel of the current build, used whenever the user has
/// not explicitly chosen an update channel.
///
/// Derived at compile time from this crate's version (`CARGO_PKG_VERSION`),
/// which the release workflow keeps in sync with the app version. Untagged
/// development builds therefore inherit the channel of the most recent tag.
pub fn default_update_channel() -> &'static str {
    default_update_channel_for(env!("CARGO_PKG_VERSION"))
}

/// When the user has not explicitly chosen an update channel, keeps an
/// existing app database aligned with the update channel of the current
/// build.
///
/// Fresh pre-release builds previously fell back to the Release channel and
/// created their database under `<settings>/release/app.db`. If such a
/// database exists while the build's own default channel has none, move it
/// over so the database follows the build instead of silently starting over
/// with an empty database in the default channel's directory.
///
/// The `-wal` and `-shm` sidecars are moved before the main database, making
/// the main database rename the commit point of the migration. Because a
/// crashed attempt leaves those sidecars in the target directory without the
/// main database - a shape that cannot be told apart from foreign leftover
/// files by name alone - the migration first records which channel it is
/// moving from in a marker file inside the target directory. The marker lets
/// a later startup resume an interrupted migration, while target sidecars
/// without a matching marker are treated as foreign and stop the migration
/// before databases of different lineages could be mixed. Nothing is ever
/// deleted or overwritten except the marker file, which this logic owns.
async fn reconcile_default_channel_database(
    settings_dir: &Path,
) -> crate::Result<()> {
    if read_explicit_update_channel(settings_dir).await?.is_some() {
        return Ok(());
    }

    // Serialize first-run migrations across processes. The OS releases the
    // lock automatically if we crash mid-migration, and a waiter re-runs the
    // whole state machine below once it acquires the lock, so an interrupted
    // attempt by another process is absorbed the same way a crashed one is.
    let lock_path = settings_dir.join(CHANNEL_RECONCILE_LOCK_FILE);
    crate::util::io::create_dir_all(settings_dir).await?;
    let lock_file = tokio::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
        .await?;
    lock_file.lock_exclusive().map_err(|error| {
        crate::ErrorKind::FSError(format!(
            "Failed to lock {}: {error}",
            lock_path.display()
        ))
    })?;

    let channel = default_update_channel();
    let other_channel = if channel == "release" {
        "beta"
    } else {
        "release"
    };

    let source = settings_dir.join(other_channel).join(LEGACY_APP_DB_FILE);
    let target_dir = settings_dir.join(channel);
    let target = target_dir.join(LEGACY_APP_DB_FILE);
    let marker = target_dir.join(CHANNEL_RECONCILE_MARKER_FILE);

    if target.try_exists()? {
        // A migration that completed but crashed before removing its marker
        // leaves both the marker and an empty source directory behind.
        if !source.try_exists()?
            && matches!(
                marker_matches(&marker, other_channel).await,
                MarkerState::Matches
            )
        {
            remove_reconcile_marker(&marker).await;
        }
        return Ok(());
    }
    if !source.try_exists()? {
        return Ok(());
    }

    crate::util::io::create_dir_all(&target_dir).await?;

    match marker_matches(&marker, other_channel).await {
        MarkerState::Missing => {
            // Without a marker there is no way to claim a target sidecar as
            // ours; it could belong to a different database of the same name,
            // so refuse to move the main database next to it.
            for suffix in ["-wal", "-shm"] {
                if sidecar_path(&target, suffix).try_exists()? {
                    tracing::warn!(
                        source = %source.display(),
                        destination = %target.display(),
                        "Channel reconciliation aborted: the target channel \
                         contains an unowned {suffix} file; the existing \
                         database was left in place"
                    );
                    return Ok(());
                }
            }
            write_reconcile_marker(&marker, other_channel).await?;
        }
        MarkerState::Mismatched => {
            tracing::warn!(
                marker = %marker.display(),
                expected = other_channel,
                "Channel reconciliation aborted: the target channel contains \
                 a reconciliation marker from another migration; the existing \
                 database was left in place"
            );
            return Ok(());
        }
        MarkerState::Matches => {}
    }

    let mut conflict = false;
    for suffix in ["-wal", "-shm"] {
        let source_sidecar = sidecar_path(&source, suffix);
        if !source_sidecar.try_exists()? {
            // Already moved by an interrupted attempt, or never existed.
            continue;
        }
        let target_sidecar = sidecar_path(&target, suffix);
        if target_sidecar.try_exists()? {
            tracing::warn!(
                source = %source_sidecar.display(),
                destination = %target_sidecar.display(),
                "Channel reconciliation aborted: both channels have a \
                 {suffix} file; leaving the existing database in place"
            );
            conflict = true;
            continue;
        }
        tokio::fs::rename(&source_sidecar, &target_sidecar).await?;
    }
    if conflict {
        return Ok(());
    }

    tokio::fs::rename(&source, &target).await?;
    remove_reconcile_marker(&marker).await;
    tracing::info!(
        source = %source.display(),
        destination = %target.display(),
        channel,
        "Moved the existing app database into the default update channel"
    );
    Ok(())
}

enum MarkerState {
    Missing,
    Mismatched,
    Matches,
}

async fn marker_matches(marker: &Path, channel: &str) -> MarkerState {
    match tokio::fs::read_to_string(marker).await {
        Ok(contents) if contents == channel => MarkerState::Matches,
        Ok(_) => MarkerState::Mismatched,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            MarkerState::Missing
        }
        Err(_) => MarkerState::Mismatched,
    }
}

async fn write_reconcile_marker(
    marker: &Path,
    channel: &str,
) -> crate::Result<()> {
    let temporary_path = marker.with_extension("tmp");
    tokio::fs::write(&temporary_path, channel).await?;
    tokio::fs::rename(&temporary_path, marker).await?;
    Ok(())
}

async fn remove_reconcile_marker(marker: &Path) {
    if let Err(error) = tokio::fs::remove_file(marker).await
        && error.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!(
            path = %marker.display(),
            "Failed to remove channel reconciliation marker: {error}"
        );
    }
}

async fn migrate_legacy_release_database(
    settings_dir: &Path,
) -> crate::Result<()> {
    let legacy_path = settings_dir.join(LEGACY_APP_DB_FILE);
    if !legacy_path.try_exists()? {
        return Ok(());
    }

    let release_dir = settings_dir.join("release");
    let release_path = release_dir.join(LEGACY_APP_DB_FILE);
    if release_path.try_exists()? {
        return Ok(());
    }

    crate::util::io::create_dir_all(&release_dir).await?;
    tokio::fs::rename(&legacy_path, &release_path).await?;
    for suffix in ["-wal", "-shm"] {
        let legacy_sidecar = sidecar_path(&legacy_path, suffix);
        if legacy_sidecar.try_exists()? {
            tokio::fs::rename(
                &legacy_sidecar,
                sidecar_path(&release_path, suffix),
            )
            .await?;
        }
    }

    tracing::info!(
        database = %release_path.display(),
        "Migrated the legacy app database into the Release update channel"
    );
    Ok(())
}

fn sidecar_path(path: &Path, suffix: &str) -> PathBuf {
    let mut path = path.as_os_str().to_owned();
    path.push(suffix);
    PathBuf::from(path)
}

async fn connect_app_db(db_path: &Path) -> crate::Result<Pool<Sqlite>> {
    super::db_backup::prepare_app_db(db_path).await?;
    if let Err(err) =
        super::db_backup::maybe_backup_existing_app_db(db_path).await
    {
        tracing::warn!(
            "Continuing without a pre-migration app database backup: {err}"
        );
    }
    open_migrated_app_db(db_path).await
}

async fn open_migrated_app_db(db_path: &Path) -> crate::Result<Pool<Sqlite>> {
    let pool = open_app_db_pool(db_path).await?;

    if let Err(err) = stale_data_cleanup(&pool).await {
        tracing::warn!(
            "Failed to clean up stale data from state database before migrations: {err}"
        );
    }

    reconcile_existing_home_dashboard_migration(&pool).await?;
    reconcile_legacy_provider_qualified_content_migration(&pool).await?;
    reconcile_legacy_official_preferred_download_source_migration(&pool)
        .await?;
    reconcile_legacy_ai_provider_migration(&pool).await?;
    reconcile_compatible_migration_checksums(&pool).await?;
    reconcile_existing_java_discovery_migration(&pool).await?;
    reconcile_existing_proxy_config_migration(&pool).await?;
    reconcile_existing_content_favorites_migration(&pool).await?;
    reconcile_pending_instance_content_ownership_duplicates(&pool).await?;
    MIGRATOR.run(&pool).await?;
    record_current_app_version(&pool).await?;

    if let Err(err) = stale_data_cleanup(&pool).await {
        tracing::warn!(
            "Failed to clean up stale data from state database: {err}"
        );
    }

    Ok(pool)
}

async fn reconcile_pending_instance_content_ownership_duplicates(
    pool: &Pool<Sqlite>,
) -> crate::Result<()> {
    let has_migrations_table: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1 FROM sqlite_master
            WHERE type = 'table' AND name = '_sqlx_migrations'
        )",
    )
    .fetch_one(pool)
    .await?;
    if !has_migrations_table {
        return Ok(());
    }

    let migration_applied: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1 FROM _sqlx_migrations
            WHERE version = ? AND success = TRUE
        )",
    )
    .bind(INSTANCE_CONTENT_OWNERSHIP_MIGRATION_VERSION)
    .fetch_one(pool)
    .await?;
    if migration_applied {
        return Ok(());
    }

    let legacy_schema_matches: bool = sqlx::query_scalar(
        "SELECT
            EXISTS(SELECT 1 FROM pragma_table_info('instance_content_entries') WHERE name = 'source_kind')
            AND NOT EXISTS(SELECT 1 FROM pragma_table_info('instance_content_entries') WHERE name = 'ownership_kind')
            AND EXISTS(SELECT 1 FROM pragma_table_info('instance_files') WHERE name = 'relative_path')
            AND EXISTS(SELECT 1 FROM pragma_table_info('instance_links') WHERE name = 'link_kind')
            AND EXISTS(SELECT 1 FROM pragma_table_info('instance_content_provider_refs') WHERE name = 'is_origin')
            AND NOT EXISTS(
                SELECT 1 FROM sqlite_master
                WHERE type = 'table' AND name = 'instance_pack_members'
            )",
    )
    .fetch_one(pool)
    .await?;
    if !legacy_schema_matches {
        return Ok(());
    }

    let result = sqlx::query(
        r#"
        WITH migration_candidates AS (
            SELECT
                entry.id,
                entry.content_set_id,
                CASE
                    WHEN origin.provider IS NOT NULL THEN
                        origin.provider || ':' || origin.provider_project_id || ':' || entry.project_type
                    ELSE 'path:' || lower(replace(file.relative_path, '\', '/'))
                END AS member_key,
                file.missing,
                entry.enabled AS entry_enabled,
                file.enabled AS file_enabled,
                entry.modified_at AS entry_modified_at,
                file.modified_at AS file_modified_at
            FROM instance_content_entries entry
            INNER JOIN instance_files file ON file.id = entry.file_id
            INNER JOIN instance_links link ON link.instance_id = entry.instance_id
            LEFT JOIN instance_content_provider_refs origin
                ON origin.content_entry_id = entry.id
                AND origin.is_origin = 1
            WHERE
                (link.link_kind IN ('modrinth_modpack', 'server_project_modpack')
                    AND entry.source_kind = 'modrinth_modpack')
                OR (link.link_kind = 'curseforge_modpack'
                    AND entry.source_kind = 'curseforge')
                OR (link.link_kind = 'imported_modpack'
                    AND entry.source_kind IN (
                        'imported_modpack',
                        'modrinth_modpack',
                        'curseforge'
                    ))
        ),
        ranked_candidates AS (
            SELECT
                id,
                row_number() OVER (
                    PARTITION BY content_set_id, member_key
                    ORDER BY
                        CASE WHEN missing = 0 THEN 0 ELSE 1 END,
                        CASE
                            WHEN entry_enabled = 1 AND file_enabled = 1
                                THEN 0
                            ELSE 1
                        END,
                        entry_modified_at DESC,
                        file_modified_at DESC,
                        id
                ) AS member_rank
            FROM migration_candidates
        )
        UPDATE instance_content_entries
        SET source_kind = 'local'
        WHERE id IN (
            SELECT id FROM ranked_candidates WHERE member_rank > 1
        )
        "#,
    )
    .execute(pool)
    .await?;

    if result.rows_affected() > 0 {
        tracing::warn!(
            version = INSTANCE_CONTENT_OWNERSHIP_MIGRATION_VERSION,
            reclassified_entries = result.rows_affected(),
            "Reclassified duplicate legacy pack members before migration"
        );
    }

    Ok(())
}

/// Reconciles historical migration checksums that differ only because of line
/// endings, plus the known pre-rebrand form of the initial migration.
///
/// SQLx hashes the raw migration bytes, so an otherwise identical migration
/// built with LF, CRLF, or mixed line endings receives a different checksum.
/// Unknown checksums are deliberately left untouched for SQLx to reject.
async fn reconcile_compatible_migration_checksums(
    pool: &Pool<Sqlite>,
) -> crate::Result<()> {
    let has_migrations_table: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations')",
    )
    .fetch_one(pool)
    .await?;

    if !has_migrations_table {
        return Ok(());
    }

    let applied_migrations: Vec<(i64, Vec<u8>)> =
        sqlx::query_as("SELECT version, checksum FROM _sqlx_migrations")
            .fetch_all(pool)
            .await?;

    for (version, applied_checksum) in applied_migrations {
        let Some(migration) = MIGRATOR
            .iter()
            .find(|migration| migration.version == version)
        else {
            continue;
        };
        let current_checksum: &[u8] = migration.checksum.as_ref();

        if applied_checksum.as_slice() == current_checksum {
            continue;
        }

        if version == COLLIDING_JAVA_DISCOVERY_MIGRATION_VERSION
            && COLLIDING_JAVA_DISCOVERY_MIGRATION_CHECKSUMS
                .contains(&checksum_as_hex(&applied_checksum).as_str())
        {
            reconcile_colliding_java_discovery_migration(pool).await?;
            update_migration_checksum(pool, version, current_checksum).await?;
            tracing::warn!(
                version,
                "Reconciled colliding Java discovery migration version"
            );
            continue;
        }

        if !is_compatible_migration_checksum(
            version,
            &applied_checksum,
            migration,
        ) {
            continue;
        }

        update_migration_checksum(pool, version, current_checksum).await?;

        tracing::warn!(
            version,
            "Reconciled a compatible historical migration checksum"
        );
    }

    Ok(())
}

async fn update_migration_checksum(
    pool: &Pool<Sqlite>,
    version: i64,
    checksum: &[u8],
) -> crate::Result<()> {
    sqlx::query("UPDATE _sqlx_migrations SET checksum = ? WHERE version = ?")
        .bind(checksum)
        .bind(version)
        .execute(pool)
        .await?;
    Ok(())
}

async fn reconcile_legacy_ai_provider_migration(
    pool: &Pool<Sqlite>,
) -> crate::Result<()> {
    let has_migrations_table: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations')",
    )
    .fetch_one(pool)
    .await?;
    if !has_migrations_table {
        return Ok(());
    }

    let applied_checksum: Option<Vec<u8>> = sqlx::query_scalar(
        "SELECT checksum FROM _sqlx_migrations WHERE version = ? AND success = TRUE",
    )
    .bind(AI_PROVIDER_MIGRATION_VERSION)
    .fetch_optional(pool)
    .await?;
    let Some(applied_checksum) = applied_checksum else {
        return Ok(());
    };
    if !LEGACY_AI_PROVIDER_MIGRATION_CHECKSUMS
        .contains(&checksum_as_hex(&applied_checksum).as_str())
    {
        return Ok(());
    }

    let ai_settings_columns: Vec<(String, String, i64, Option<String>, i64)> =
        sqlx::query_as(
            "SELECT name, type, \"notnull\", dflt_value, pk
             FROM pragma_table_info('ai_settings') ORDER BY cid",
        )
        .fetch_all(pool)
        .await?;
    let provider_config_columns: Vec<(
        String,
        String,
        i64,
        Option<String>,
        i64,
    )> = sqlx::query_as(
        "SELECT name, type, \"notnull\", dflt_value, pk
             FROM pragma_table_info('ai_provider_configs') ORDER BY cid",
    )
    .fetch_all(pool)
    .await?;
    let provider_model_columns: Vec<(
        String,
        String,
        i64,
        Option<String>,
        i64,
    )> = sqlx::query_as(
        "SELECT name, type, \"notnull\", dflt_value, pk
             FROM pragma_table_info('ai_provider_models') ORDER BY cid",
    )
    .fetch_all(pool)
    .await?;
    let translation_columns: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM pragma_table_info('translation_settings')
         WHERE name IN ('ai_provider_id', 'ai_model_id') ORDER BY name",
    )
    .fetch_all(pool)
    .await?;
    let ai_settings_match = ai_settings_columns
        == vec![
            ("id".to_string(), "INTEGER".to_string(), 1, None, 1),
            (
                "enabled".to_string(),
                "INTEGER".to_string(),
                1,
                Some("TRUE".to_string()),
                0,
            ),
        ];
    let provider_config_match = provider_config_columns
        == vec![
            ("provider_id".to_string(), "TEXT".to_string(), 1, None, 1),
            ("custom_name".to_string(), "TEXT".to_string(), 0, None, 0),
            (
                "protocol".to_string(),
                "TEXT".to_string(),
                1,
                Some("'openai'".to_string()),
                0,
            ),
            (
                "enabled".to_string(),
                "INTEGER".to_string(),
                1,
                Some("FALSE".to_string()),
                0,
            ),
            (
                "endpoint".to_string(),
                "TEXT".to_string(),
                1,
                Some("''".to_string()),
                0,
            ),
            (
                "settings".to_string(),
                "TEXT".to_string(),
                1,
                Some("'{}'".to_string()),
                0,
            ),
        ];
    let provider_model_match = provider_model_columns
        == vec![
            ("provider_id".to_string(), "TEXT".to_string(), 1, None, 1),
            ("model_id".to_string(), "TEXT".to_string(), 1, None, 2),
            (
                "display_name".to_string(),
                "TEXT".to_string(),
                1,
                Some("''".to_string()),
                0,
            ),
            (
                "enabled".to_string(),
                "INTEGER".to_string(),
                1,
                Some("TRUE".to_string()),
                0,
            ),
            (
                "source".to_string(),
                "TEXT".to_string(),
                1,
                Some("'custom'".to_string()),
                0,
            ),
        ];
    if !ai_settings_match
        || !provider_config_match
        || !provider_model_match
        || translation_columns != ["ai_model_id", "ai_provider_id"]
    {
        return Ok(());
    }

    let migration = MIGRATOR
        .iter()
        .find(|migration| migration.version == AI_PROVIDER_MIGRATION_VERSION)
        .expect("AI provider migration should be embedded");
    update_migration_checksum(
        pool,
        AI_PROVIDER_MIGRATION_VERSION,
        migration.checksum.as_ref(),
    )
    .await?;
    tracing::warn!(
        version = AI_PROVIDER_MIGRATION_VERSION,
        "Reconciled the validated legacy AI provider schema"
    );
    Ok(())
}

async fn reconcile_legacy_official_preferred_download_source_migration(
    pool: &Pool<Sqlite>,
) -> crate::Result<()> {
    let has_migrations_table: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations')",
    )
    .fetch_one(pool)
    .await?;
    if !has_migrations_table {
        return Ok(());
    }

    let applied_checksum: Option<Vec<u8>> = sqlx::query_scalar(
        "SELECT checksum FROM _sqlx_migrations WHERE version = ? AND success = TRUE",
    )
    .bind(OFFICIAL_PREFERRED_DOWNLOAD_SOURCE_MIGRATION_VERSION)
    .fetch_optional(pool)
    .await?;
    let Some(applied_checksum) = applied_checksum else {
        return Ok(());
    };
    if !LEGACY_OFFICIAL_PREFERRED_DOWNLOAD_SOURCE_MIGRATION_CHECKSUMS
        .contains(&checksum_as_hex(&applied_checksum).as_str())
    {
        return Ok(());
    }

    let source_columns: Vec<(String, String, i64, Option<String>, i64)> =
        sqlx::query_as(
            "SELECT name, type, \"notnull\", dflt_value, pk
             FROM pragma_table_info('settings')
             WHERE name IN (
                'minecraft_metadata_source',
                'minecraft_file_source',
                'modrinth_source',
                'curseforge_source'
             )",
        )
        .fetch_all(pool)
        .await?;
    let source_column_names = [
        "minecraft_metadata_source",
        "minecraft_file_source",
        "modrinth_source",
        "curseforge_source",
    ];
    let source_columns_match = source_columns.len()
        == source_column_names.len()
        && source_columns.iter().all(
            |(name, data_type, not_null, default_value, primary_key)| {
                source_column_names.contains(&name.as_str())
                    && data_type.eq_ignore_ascii_case("TEXT")
                    && *not_null == 1
                    && default_value.as_deref() == Some("'auto'")
                    && *primary_key == 0
            },
        );

    let settings_sql: Option<String> = sqlx::query_scalar(
        "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'settings'",
    )
    .fetch_optional(pool)
    .await?;
    let source_checks_match = settings_sql.as_ref().is_some_and(|sql| {
        source_column_names.iter().all(|column| {
            sql.contains(&format!(
                "{column} IN ('auto', 'official_only', 'mirror_preferred', 'official_preferred')"
            ))
        })
    });
    let minimal_home_foreign_key_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1 FROM pragma_foreign_key_list('settings')
            WHERE \"from\" = 'minimal_home_instance_id'
                AND \"table\" = 'instances'
                AND \"to\" = 'id'
                AND on_delete = 'SET NULL'
        )",
    )
    .fetch_one(pool)
    .await?;
    let legacy_table_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1 FROM sqlite_master
            WHERE type = 'table'
                AND name = 'settings_with_official_preferred'
        )",
    )
    .fetch_one(pool)
    .await?;
    if !source_columns_match
        || !source_checks_match
        || !minimal_home_foreign_key_exists
        || legacy_table_exists
    {
        return Ok(());
    }

    let migration = MIGRATOR
        .iter()
        .find(|migration| {
            migration.version
                == OFFICIAL_PREFERRED_DOWNLOAD_SOURCE_MIGRATION_VERSION
        })
        .expect(
            "official-preferred download source migration should be embedded",
        );
    update_migration_checksum(
        pool,
        OFFICIAL_PREFERRED_DOWNLOAD_SOURCE_MIGRATION_VERSION,
        migration.checksum.as_ref(),
    )
    .await?;

    tracing::warn!(
        version = OFFICIAL_PREFERRED_DOWNLOAD_SOURCE_MIGRATION_VERSION,
        "Reconciled the validated official-preferred download source schema"
    );
    Ok(())
}

async fn reconcile_legacy_provider_qualified_content_migration(
    pool: &Pool<Sqlite>,
) -> crate::Result<()> {
    let has_migrations_table: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations')",
    )
    .fetch_one(pool)
    .await?;
    if !has_migrations_table {
        return Ok(());
    }

    let applied_checksum: Option<Vec<u8>> = sqlx::query_scalar(
        "SELECT checksum FROM _sqlx_migrations WHERE version = ? AND success = TRUE",
    )
    .bind(PROVIDER_QUALIFIED_CONTENT_MIGRATION_VERSION)
    .fetch_optional(pool)
    .await?;
    let Some(applied_checksum) = applied_checksum else {
        return Ok(());
    };
    if !LEGACY_PROVIDER_QUALIFIED_CONTENT_MIGRATION_CHECKSUMS
        .contains(&checksum_as_hex(&applied_checksum).as_str())
    {
        return Ok(());
    }

    let schema_is_legacy_provider_qualified: bool = sqlx::query_scalar(
        "SELECT
            EXISTS(SELECT 1 FROM pragma_table_info('instance_content_provider_refs') WHERE name = 'content_entry_id' AND pk = 1)
            AND EXISTS(SELECT 1 FROM pragma_table_info('instance_content_provider_refs') WHERE name = 'provider' AND pk = 2)
            AND EXISTS(SELECT 1 FROM pragma_table_info('instance_content_provider_refs') WHERE name = 'provider_project_id')
            AND EXISTS(SELECT 1 FROM pragma_table_info('instance_content_provider_refs') WHERE name = 'provider_release_id')
            AND EXISTS(SELECT 1 FROM pragma_table_info('instance_content_provider_refs') WHERE name = 'is_origin')
            AND NOT EXISTS(SELECT 1 FROM pragma_table_info('instance_content_provider_refs') WHERE name = 'id')
            AND NOT EXISTS(SELECT 1 FROM pragma_table_info('instance_content_entries') WHERE name IN ('project_id', 'version_id'))
            AND EXISTS(SELECT 1 FROM pragma_table_info('instance_content_update_checks') WHERE name = 'provider')
            AND EXISTS(SELECT 1 FROM pragma_table_info('instance_content_update_checks') WHERE name = 'provider_project_id')
            AND EXISTS(SELECT 1 FROM pragma_table_info('instance_content_update_checks') WHERE name = 'provider_release_id')",
    )
    .fetch_one(pool)
    .await?;
    if !schema_is_legacy_provider_qualified {
        return Ok(());
    }

    let migration = MIGRATOR
        .iter()
        .find(|migration| {
            migration.version == PROVIDER_QUALIFIED_CONTENT_MIGRATION_VERSION
        })
        .expect("provider-qualified content migration should be embedded");
    update_migration_checksum(
        pool,
        PROVIDER_QUALIFIED_CONTENT_MIGRATION_VERSION,
        migration.checksum.as_ref(),
    )
    .await?;

    tracing::warn!(
        version = PROVIDER_QUALIFIED_CONTENT_MIGRATION_VERSION,
        "Reconciled the validated legacy provider-qualified content schema"
    );
    Ok(())
}

async fn reconcile_existing_home_dashboard_migration(
    pool: &Pool<Sqlite>,
) -> crate::Result<()> {
    let has_migrations_table: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations')",
    )
    .fetch_one(pool)
    .await?;
    if !has_migrations_table {
        return Ok(());
    }

    let applied_checksum: Option<Vec<u8>> = sqlx::query_scalar(
        "SELECT checksum FROM _sqlx_migrations WHERE version = ? AND success = TRUE",
    )
    .bind(COLLIDING_HOME_DASHBOARD_MIGRATION_VERSION)
    .fetch_optional(pool)
    .await?;
    let checksum_is_known = applied_checksum.as_ref().is_some_and(|checksum| {
        COLLIDING_HOME_DASHBOARD_MIGRATION_CHECKSUMS
            .contains(&checksum_as_hex(checksum).as_str())
    });
    if !checksum_is_known {
        return Ok(());
    }

    let pinned_at_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('instances') WHERE name = 'pinned_at')",
    )
    .fetch_one(pool)
    .await?;
    let playtime_table_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'instance_daily_playtime')",
    )
    .fetch_one(pool)
    .await?;
    let playtime_index_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'index' AND name = 'instance_daily_playtime_played_on')",
    )
    .fetch_one(pool)
    .await?;
    if !pinned_at_exists || !playtime_table_exists || !playtime_index_exists {
        return Ok(());
    }

    let colliding_migration = MIGRATOR
        .iter()
        .find(|migration| {
            migration.version == COLLIDING_HOME_DASHBOARD_MIGRATION_VERSION
        })
        .expect("transparent background blur migration should be embedded");
    let home_dashboard_migration = MIGRATOR
        .iter()
        .find(|migration| migration.version == HOME_DASHBOARD_MIGRATION_VERSION)
        .expect("home dashboard migration should be embedded");

    let mut transaction = pool.begin().await?;
    let blur_column_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('settings') WHERE name = 'transparent_background_blur')",
    )
    .fetch_one(&mut *transaction)
    .await?;
    if !blur_column_exists {
        sqlx::query(
            "ALTER TABLE settings ADD COLUMN transparent_background_blur INTEGER NOT NULL DEFAULT FALSE",
        )
        .execute(&mut *transaction)
        .await?;
    }

    sqlx::query(
        "UPDATE _sqlx_migrations SET description = ?, checksum = ? WHERE version = ?",
    )
    .bind(colliding_migration.description.as_ref())
    .bind(colliding_migration.checksum.as_ref())
    .bind(COLLIDING_HOME_DASHBOARD_MIGRATION_VERSION)
    .execute(&mut *transaction)
    .await?;

    let canonical_applied: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM _sqlx_migrations WHERE version = ?)",
    )
    .bind(HOME_DASHBOARD_MIGRATION_VERSION)
    .fetch_one(&mut *transaction)
    .await?;
    if !canonical_applied {
        sqlx::query(
            "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time) VALUES (?, ?, TRUE, ?, 0)",
        )
        .bind(home_dashboard_migration.version)
        .bind(home_dashboard_migration.description.as_ref())
        .bind(home_dashboard_migration.checksum.as_ref())
        .execute(&mut *transaction)
        .await?;
    }
    transaction.commit().await?;

    tracing::warn!(
        old_version = COLLIDING_HOME_DASHBOARD_MIGRATION_VERSION,
        canonical_version = HOME_DASHBOARD_MIGRATION_VERSION,
        "Reconciled colliding Home dashboard migration version"
    );
    Ok(())
}

async fn reconcile_colliding_java_discovery_migration(
    pool: &Pool<Sqlite>,
) -> crate::Result<()> {
    let onboarding_version_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('settings') WHERE name = 'onboarding_version')",
    )
    .fetch_one(pool)
    .await?;
    if !onboarding_version_exists {
        sqlx::query(
            "ALTER TABLE settings ADD COLUMN onboarding_version INTEGER NOT NULL DEFAULT 0",
        )
        .execute(pool)
        .await?;
    }

    let instance_tour_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('settings') WHERE name = 'onboarding_instance_tour_completed')",
    )
    .fetch_one(pool)
    .await?;
    if !instance_tour_exists {
        sqlx::query(
            "ALTER TABLE settings ADD COLUMN onboarding_instance_tour_completed INTEGER NOT NULL DEFAULT TRUE",
        )
        .execute(pool)
        .await?;
        sqlx::query(
            "UPDATE settings SET onboarding_instance_tour_completed = CASE WHEN onboarded = 1 THEN TRUE ELSE FALSE END",
        )
        .execute(pool)
        .await?;
    }

    Ok(())
}

async fn reconcile_existing_java_discovery_migration(
    pool: &Pool<Sqlite>,
) -> crate::Result<()> {
    let has_migrations_table: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations')",
    )
    .fetch_one(pool)
    .await?;
    if !has_migrations_table {
        return Ok(());
    }

    let columns: Vec<(String, String, i64, i64)> = sqlx::query_as(
        "SELECT name, type, \"notnull\", pk FROM pragma_table_info('discovered_javas') ORDER BY cid",
    )
    .fetch_all(pool)
    .await?;
    let expected_columns = [
        ("path", "TEXT", 1, 1),
        ("major_version", "INTEGER", 1, 0),
        ("full_version", "TEXT", 1, 0),
        ("architecture", "TEXT", 1, 0),
        ("file_size", "INTEGER", 1, 0),
        ("file_mtime_ms", "INTEGER", 1, 0),
    ];
    let schema_matches = columns.len() == expected_columns.len()
        && columns.iter().zip(expected_columns).all(
            |((name, data_type, not_null, primary_key), expected)| {
                name == expected.0
                    && data_type.eq_ignore_ascii_case(expected.1)
                    && *not_null == expected.2
                    && *primary_key == expected.3
            },
        );
    if !schema_matches {
        return Ok(());
    }

    let migration = MIGRATOR
        .iter()
        .find(|migration| migration.version == JAVA_DISCOVERY_MIGRATION_VERSION)
        .expect("Java discovery migration should be embedded");
    let canonical_applied: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM _sqlx_migrations WHERE version = ?)",
    )
    .bind(JAVA_DISCOVERY_MIGRATION_VERSION)
    .fetch_one(pool)
    .await?;
    let temporary_checksum: Option<Vec<u8>> = sqlx::query_scalar(
        "SELECT checksum FROM _sqlx_migrations WHERE version = ?",
    )
    .bind(TEMPORARY_JAVA_DISCOVERY_MIGRATION_VERSION)
    .fetch_optional(pool)
    .await?;
    let temporary_is_known =
        temporary_checksum.as_ref().is_some_and(|checksum| {
            TEMPORARY_JAVA_DISCOVERY_MIGRATION_CHECKSUMS
                .contains(&checksum_as_hex(checksum).as_str())
        });

    let mut transaction = pool.begin().await?;
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS discovered_javas_major_version ON discovered_javas (major_version)",
    )
    .execute(&mut *transaction)
    .await?;
    if !canonical_applied {
        sqlx::query(
            "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time) VALUES (?, ?, TRUE, ?, 0)",
        )
        .bind(migration.version)
        .bind(migration.description.as_ref())
        .bind(migration.checksum.as_ref())
        .execute(&mut *transaction)
        .await?;
    }
    if temporary_is_known {
        sqlx::query("DELETE FROM _sqlx_migrations WHERE version = ?")
            .bind(TEMPORARY_JAVA_DISCOVERY_MIGRATION_VERSION)
            .execute(&mut *transaction)
            .await?;
    }
    transaction.commit().await?;

    tracing::warn!(
        version = JAVA_DISCOVERY_MIGRATION_VERSION,
        removed_temporary_version = temporary_is_known,
        "Reconciled existing Java discovery table with canonical migration"
    );
    Ok(())
}

async fn reconcile_existing_proxy_config_migration(
    pool: &Pool<Sqlite>,
) -> crate::Result<()> {
    let has_migrations_table: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations')",
    )
    .fetch_one(pool)
    .await?;
    if !has_migrations_table {
        return Ok(());
    }

    let migration_applied: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM _sqlx_migrations WHERE version = ?)",
    )
    .bind(PROXY_CONFIG_MIGRATION_VERSION)
    .fetch_one(pool)
    .await?;
    if migration_applied {
        return Ok(());
    }

    let columns: Vec<(String, String, i64, Option<String>, i64)> =
        sqlx::query_as(
            "SELECT name, type, \"notnull\", dflt_value, pk
             FROM pragma_table_info('settings')
             WHERE name IN (
                'proxy_mode', 'proxy_url', 'proxy_username', 'proxy_password'
             )
             ORDER BY cid",
        )
        .fetch_all(pool)
        .await?;
    let expected_columns = [
        ("proxy_mode", "TEXT", 1, Some("'system'"), 0),
        ("proxy_url", "TEXT", 1, Some("''"), 0),
        ("proxy_username", "TEXT", 1, Some("''"), 0),
        ("proxy_password", "TEXT", 1, Some("''"), 0),
    ];
    let columns_match = columns.len() == expected_columns.len()
        && columns.iter().zip(expected_columns).all(
            |(
                (name, data_type, not_null, default_value, primary_key),
                expected,
            )| {
                name == expected.0
                    && data_type.eq_ignore_ascii_case(expected.1)
                    && *not_null == expected.2
                    && default_value.as_deref() == expected.3
                    && *primary_key == expected.4
            },
        );
    if !columns_match {
        return Ok(());
    }

    let settings_sql: Option<String> = sqlx::query_scalar(
        "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'settings'",
    )
    .fetch_optional(pool)
    .await?;
    if !settings_sql.as_deref().is_some_and(|sql| {
        sql.contains("CHECK (proxy_mode IN ('none', 'system', 'custom'))")
    }) {
        return Ok(());
    }

    let migration = MIGRATOR
        .iter()
        .find(|migration| migration.version == PROXY_CONFIG_MIGRATION_VERSION)
        .expect("Proxy config migration should be embedded");
    sqlx::query(
        "INSERT INTO _sqlx_migrations (
            version, description, success, checksum, execution_time
         ) VALUES (?, ?, TRUE, ?, 0)",
    )
    .bind(migration.version)
    .bind(migration.description.as_ref())
    .bind(migration.checksum.as_ref())
    .execute(pool)
    .await?;

    tracing::warn!(
        version = PROXY_CONFIG_MIGRATION_VERSION,
        "Reconciled fully applied proxy config schema with canonical migration"
    );
    Ok(())
}

/// Protects an old database that still holds the legacy favorites table but
/// lost the record of the migration that created it; this can go once the
/// lowest version a downgrade supports is above `20260820200000`.
async fn reconcile_existing_content_favorites_migration(
    pool: &Pool<Sqlite>,
) -> crate::Result<()> {
    let has_migrations_table: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations')",
    )
    .fetch_one(pool)
    .await?;
    if !has_migrations_table {
        return Ok(());
    }

    let migration_applied: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM _sqlx_migrations WHERE version = ?)",
    )
    .bind(CONTENT_FAVORITES_MIGRATION_VERSION)
    .fetch_one(pool)
    .await?;
    if migration_applied {
        return Ok(());
    }

    let columns: Vec<(String, String, i64, Option<String>, i64)> =
        sqlx::query_as(
            "SELECT name, type, \"notnull\", dflt_value, pk
             FROM pragma_table_info('content_favorites')
             ORDER BY cid",
        )
        .fetch_all(pool)
        .await?;
    let expected_columns = [
        ("provider", "TEXT", 1, None, 1),
        ("project_id", "TEXT", 1, None, 2),
        ("content_type", "TEXT", 1, None, 0),
        ("saved_at", "INTEGER", 1, None, 0),
    ];
    let columns_match = columns.len() == expected_columns.len()
        && columns.iter().zip(expected_columns).all(
            |(
                (name, data_type, not_null, default_value, primary_key),
                expected,
            )| {
                name == expected.0
                    && data_type.eq_ignore_ascii_case(expected.1)
                    && *not_null == expected.2
                    && default_value.as_deref() == expected.3
                    && *primary_key == expected.4
            },
        );
    if !columns_match {
        return Ok(());
    }

    let table_sql: Option<String> = sqlx::query_scalar(
        "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'content_favorites'",
    )
    .fetch_optional(pool)
    .await?;
    if !table_sql.as_deref().is_some_and(|sql| {
        sql.contains("CHECK (provider IN ('modrinth', 'curseforge'))")
            && sql.contains("CHECK (length(project_id) > 0)")
            && sql.contains(
                "CHECK (content_type IN ('mod', 'resourcepack', 'datapack', 'shader'))",
            )
            && sql.contains("PRIMARY KEY (provider, project_id)")
    }) {
        return Ok(());
    }

    let indexes: Vec<(String, i64, String, i64)> = sqlx::query_as(
        "SELECT name, \"unique\", origin, partial
         FROM pragma_index_list('content_favorites')
         ORDER BY name",
    )
    .fetch_all(pool)
    .await?;
    let indexes_match = indexes.len() == 2
        && indexes.iter().any(|index| {
            index.0 == "content_favorites_saved_at_idx"
                && index.1 == 0
                && index.2 == "c"
                && index.3 == 0
        })
        && indexes.iter().any(|index| {
            index.0.starts_with("sqlite_autoindex_content_favorites_")
                && index.1 == 1
                && index.2 == "pk"
                && index.3 == 0
        });
    if !indexes_match {
        return Ok(());
    }

    let saved_at_index_columns: Vec<(String, i64)> = sqlx::query_as(
        "SELECT name, desc
         FROM pragma_index_xinfo('content_favorites_saved_at_idx')
         WHERE key = 1
         ORDER BY seqno",
    )
    .fetch_all(pool)
    .await?;
    if saved_at_index_columns != [("saved_at".to_string(), 1)] {
        return Ok(());
    }

    let foreign_key_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_foreign_key_list('content_favorites')",
    )
    .fetch_one(pool)
    .await?;
    if foreign_key_count != 0 {
        return Ok(());
    }

    let migration = MIGRATOR
        .iter()
        .find(|migration| {
            migration.version == CONTENT_FAVORITES_MIGRATION_VERSION
        })
        .expect("Content favorites migration should be embedded");
    sqlx::query(
        "INSERT INTO _sqlx_migrations (
            version, description, success, checksum, execution_time
         ) VALUES (?, ?, TRUE, ?, 0)",
    )
    .bind(migration.version)
    .bind(migration.description.as_ref())
    .bind(migration.checksum.as_ref())
    .execute(pool)
    .await?;

    tracing::warn!(
        version = CONTENT_FAVORITES_MIGRATION_VERSION,
        "Reconciled fully applied content favorites schema with canonical migration"
    );
    Ok(())
}

fn is_compatible_migration_checksum(
    version: i64,
    applied_checksum: &[u8],
    migration: &Migration,
) -> bool {
    let normalized_lf = migration.sql.replace("\r\n", "\n").replace('\r', "\n");
    let normalized_crlf = normalized_lf.replace('\n', "\r\n");

    if checksum_matches(applied_checksum, normalized_lf.as_bytes())
        || checksum_matches(applied_checksum, normalized_crlf.as_bytes())
    {
        return true;
    }

    version == INITIAL_MIGRATION_VERSION
        && LEGACY_INITIAL_MIGRATION_CHECKSUMS
            .contains(&checksum_as_hex(applied_checksum).as_str())
}

fn checksum_matches(checksum: &[u8], contents: &[u8]) -> bool {
    let calculated: [u8; 48] = Sha384::digest(contents).into();
    checksum == calculated
}

fn checksum_as_hex(checksum: &[u8]) -> String {
    use std::fmt::Write;

    checksum.iter().fold(
        String::with_capacity(checksum.len() * 2),
        |mut output, byte| {
            let _ = write!(output, "{byte:02x}");
            output
        },
    )
}

async fn open_app_db_pool(db_path: &Path) -> crate::Result<Pool<Sqlite>> {
    let conn_options = SqliteConnectOptions::new()
        .filename(db_path)
        .busy_timeout(Duration::from_secs(30))
        .journal_mode(SqliteJournalMode::Wal)
        .optimize_on_close(true, None)
        .create_if_missing(true);

    Ok(SqlitePoolOptions::new()
        .max_connections(100)
        .connect_with(conn_options)
        .await?)
}

async fn record_current_app_version(pool: &Pool<Sqlite>) -> crate::Result<()> {
    sqlx::query!(
        "
        INSERT INTO app_metadata (key, value, updated_at)
        VALUES ('app_version', ?, unixepoch())
        ON CONFLICT(key) DO UPDATE SET
            value = excluded.value,
            updated_at = excluded.updated_at
        ",
        env!("CARGO_PKG_VERSION"),
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Cleans up data from the database that is no longer referenced, but must be
/// kept around for a little while to allow users to recover from accidental
/// deletions.
async fn stale_data_cleanup(pool: &Pool<Sqlite>) -> crate::Result<()> {
    let mut tx = pool.begin().await?;

    let has_skin_tables = sqlx::query!(
        "SELECT COUNT(*) AS \"count!: i64\" FROM sqlite_master WHERE type = 'table' AND name IN ('custom_minecraft_skins', 'minecraft_users')",
    )
    .fetch_one(&mut *tx)
    .await?
    .count == 2;

    if has_skin_tables {
        sqlx::query!(
            "DELETE FROM custom_minecraft_skins WHERE minecraft_user_uuid NOT IN (SELECT uuid FROM minecraft_users)"
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    Ok(())
}

#[cfg(test)]
#[path = "db_tests.rs"]
mod tests;

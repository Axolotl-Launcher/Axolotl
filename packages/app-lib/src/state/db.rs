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
    super::db_backup::maybe_backup_existing_app_db(db_path).await?;
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
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[tokio::test]
    async fn version_backup_precedes_migration_validation_and_survives_an_upgrade_failure()
     {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("release").join("app.db");
        tokio::fs::create_dir_all(database.parent().unwrap())
            .await
            .unwrap();
        let pool = connect_app_db(&database).await.unwrap();
        sqlx::query("UPDATE app_metadata SET value = 'previous-software-version' WHERE key = 'app_version'")
            .execute(&pool).await.unwrap();
        // An unknown applied migration must still fail; it must not suppress the version backup.
        sqlx::query("INSERT INTO _sqlx_migrations(version, description, success, checksum, execution_time) VALUES (99999999999999, 'unavailable-build', TRUE, X'00', 1)")
            .execute(&pool).await.unwrap();
        pool.close().await;
        assert!(connect_app_db(&database).await.is_err());
        let backup_dir = directory
            .path()
            .join("Backups")
            .join("app-db")
            .join("release");
        let mut entries = tokio::fs::read_dir(backup_dir).await.unwrap();
        let snapshot = entries.next_entry().await.unwrap().unwrap().path();
        assert_eq!(
            snapshot.extension().and_then(|value| value.to_str()),
            Some("db")
        );
        let backup = open_app_db_pool(&snapshot).await.unwrap();
        let version: String = sqlx::query_scalar(
            "SELECT value FROM app_metadata WHERE key = 'app_version'",
        )
        .fetch_one(&backup)
        .await
        .unwrap();
        assert_eq!(version, "previous-software-version");
        backup.close().await;
        let source = open_app_db_pool(&database).await.unwrap();
        let version: String = sqlx::query_scalar(
            "SELECT value FROM app_metadata WHERE key = 'app_version'",
        )
        .fetch_one(&source)
        .await
        .unwrap();
        assert_eq!(version, "previous-software-version");
        source.close().await;
    }

    const TERRACOTTA_PUBLIC_NODES_MIGRATION_VERSION: i64 = 20260812120000;
    const INSTANCE_LOADER_COMPONENTS_MIGRATION_VERSION: i64 = 20260819120000;
    const INSTANCE_POST_UPGRADE_NOTICES_MIGRATION_VERSION: i64 = 20260824000000;
    const MCARCHIVE_PROVIDER_FILE_ID_MIGRATION_VERSION: i64 = 20260823020000;

    #[tokio::test]
    async fn post_upgrade_notices_migrate_fresh_and_existing_databases() {
        for previous_schema in [false, true] {
            let pool = SqlitePoolOptions::new()
                .max_connections(1)
                .connect("sqlite::memory:")
                .await
                .unwrap();
            sqlx::query("PRAGMA foreign_keys = ON")
                .execute(&pool)
                .await
                .unwrap();
            if previous_schema {
                let previous_migrator = Migrator {
                    migrations: std::borrow::Cow::Owned(
                        MIGRATOR
                            .iter()
                            .filter(|migration| {
                                migration.version
                                    < INSTANCE_POST_UPGRADE_NOTICES_MIGRATION_VERSION
                            })
                            .cloned()
                            .collect(),
                    ),
                    ..Migrator::DEFAULT
                };
                previous_migrator.run(&pool).await.unwrap();
            }
            MIGRATOR.run(&pool).await.unwrap();

            let table_exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'instance_post_upgrade_notices')",
            )
            .fetch_one(&pool)
            .await
            .unwrap();
            assert!(table_exists);
            let foreign_key_errors: Vec<(String, i64, String, i64)> =
                sqlx::query_as("PRAGMA foreign_key_check")
                    .fetch_all(&pool)
                    .await
                    .unwrap();
            assert!(foreign_key_errors.is_empty());
        }
    }
    #[tokio::test]
    async fn loader_components_migrate_fresh_and_existing_instances() {
        let fresh = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&fresh)
            .await
            .unwrap();
        MIGRATOR.run(&fresh).await.unwrap();
        let table_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(
				SELECT 1 FROM sqlite_master
				WHERE type = 'table' AND name = 'instance_loader_components'
			)",
        )
        .fetch_one(&fresh)
        .await
        .unwrap();
        assert!(table_exists);
        let fresh_foreign_key_errors: Vec<(String, i64, String, i64)> =
            sqlx::query_as("PRAGMA foreign_key_check")
                .fetch_all(&fresh)
                .await
                .unwrap();
        assert!(fresh_foreign_key_errors.is_empty());

        let upgraded = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&upgraded)
            .await
            .unwrap();
        let previous_migrator = Migrator {
            migrations: std::borrow::Cow::Owned(
                MIGRATOR
                    .iter()
                    .filter(|migration| {
                        migration.version
                            < INSTANCE_LOADER_COMPONENTS_MIGRATION_VERSION
                    })
                    .cloned()
                    .collect(),
            ),
            ..Migrator::DEFAULT
        };
        previous_migrator.run(&upgraded).await.unwrap();
        for (id, loader, loader_version) in [
            ("forge", "forge", Some("14.23.5.2860")),
            ("optifine", "optifine", Some("HD_U_G5")),
            ("vanilla", "vanilla", None),
        ] {
            let content_set_id = format!("{id}-set");
            sqlx::query(
                "INSERT INTO instances (
					id, path, applied_content_set_id, install_stage,
					launcher_feature_version, update_channel, name,
					created, modified, submitted_time_played,
					recent_time_played
				 ) VALUES (?, ?, ?, 'installed', 'migrated_launch_hooks',
					'release', ?, 1, 1, 0, 0)",
            )
            .bind(id)
            .bind(id)
            .bind(&content_set_id)
            .bind(id)
            .execute(&upgraded)
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO instance_content_sets (
					id, instance_id, name, source_kind, status,
					game_version, loader, loader_version, created, modified
				 ) VALUES (?, ?, 'Default', 'local', 'available',
					'1.12.2', ?, ?, 1, 1)",
            )
            .bind(&content_set_id)
            .bind(id)
            .bind(loader)
            .bind(loader_version)
            .execute(&upgraded)
            .await
            .unwrap();
        }

        MIGRATOR.run(&upgraded).await.unwrap();

        let rows: Vec<(String, String, Option<String>, String)> =
            sqlx::query_as(
                "SELECT instance_id, kind, version, role
				 FROM instance_loader_components
				 ORDER BY instance_id, role DESC, kind",
            )
            .fetch_all(&upgraded)
            .await
            .unwrap();
        assert_eq!(
            rows,
            vec![
                (
                    "forge".to_string(),
                    "forge".to_string(),
                    Some("14.23.5.2860".to_string()),
                    "primary".to_string(),
                ),
                (
                    "optifine".to_string(),
                    "vanilla".to_string(),
                    None,
                    "primary".to_string(),
                ),
                (
                    "optifine".to_string(),
                    "optifine".to_string(),
                    Some("HD_U_G5".to_string()),
                    "adjunct".to_string(),
                ),
                (
                    "vanilla".to_string(),
                    "vanilla".to_string(),
                    None,
                    "primary".to_string(),
                ),
            ]
        );
        let malformed_metadata = sqlx::query(
            "UPDATE instance_loader_components
			 SET provider_metadata = '{'
			 WHERE instance_id = 'forge'",
        )
        .execute(&upgraded)
        .await;
        assert!(malformed_metadata.is_err());
        let upgraded_foreign_key_errors: Vec<(String, i64, String, i64)> =
            sqlx::query_as("PRAGMA foreign_key_check")
                .fetch_all(&upgraded)
                .await
                .unwrap();
        assert!(upgraded_foreign_key_errors.is_empty());
    }

    #[tokio::test]
    async fn mcarchive_provider_file_ids_migrate_fresh_and_preserve_existing_references()
     {
        let fresh = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&fresh)
            .await
            .unwrap();
        MIGRATOR.run(&fresh).await.unwrap();
        let fresh_has_file_id: bool = sqlx::query_scalar(
            "SELECT EXISTS(
                SELECT 1 FROM pragma_table_info('instance_content_provider_refs')
                WHERE name = 'provider_file_id'
            )",
        )
        .fetch_one(&fresh)
        .await
        .unwrap();
        assert!(fresh_has_file_id);

        let upgraded = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&upgraded)
            .await
            .unwrap();
        let previous_migrator = Migrator {
            migrations: std::borrow::Cow::Owned(
                MIGRATOR
                    .iter()
                    .filter(|migration| {
                        migration.version
                            < MCARCHIVE_PROVIDER_FILE_ID_MIGRATION_VERSION
                    })
                    .cloned()
                    .collect(),
            ),
            ..Migrator::DEFAULT
        };
        previous_migrator.run(&upgraded).await.unwrap();
        sqlx::raw_sql(
            r#"
            INSERT INTO instances (
                id, path, applied_content_set_id, install_stage,
                launcher_feature_version, name, created, modified
            ) VALUES
                ('mcarchive-migration-instance', 'mcarchive-migration-instance',
                    'mcarchive-migration-set', 'installed', '1',
                    'MCArchive migration', 1, 1);

            INSERT INTO instance_content_sets (
                id, instance_id, name, source_kind, status, game_version,
                loader, created, modified
            ) VALUES
                ('mcarchive-migration-set', 'mcarchive-migration-instance',
                    'Default', 'local', 'available', '1.6.2', 'vanilla', 1, 1);

            UPDATE instances
            SET applied_content_set_id = 'mcarchive-migration-set'
            WHERE id = 'mcarchive-migration-instance';

            INSERT INTO instance_content_entries (
                id, instance_id, content_set_id, file_id, project_type,
                source_kind, server_requirement, client_requirement,
                enabled, added_at, modified_at
            ) VALUES
                ('modrinth-reference', 'mcarchive-migration-instance',
                    'mcarchive-migration-set', NULL, 'mod', 'local',
                    'required', 'required', 1, 1, 1),
                ('curseforge-reference', 'mcarchive-migration-instance',
                    'mcarchive-migration-set', NULL, 'mod', 'local',
                    'required', 'required', 1, 1, 1),
                ('mcarchive-reference', 'mcarchive-migration-instance',
                    'mcarchive-migration-set', NULL, 'mod', 'mcarchive',
                    'required', 'required', 1, 1, 1);

            INSERT INTO instance_content_provider_refs (
                content_entry_id, provider, provider_project_id,
                provider_release_id, is_origin
            ) VALUES
                ('modrinth-reference', 'modrinth', 'modrinth-project',
                    'modrinth-version', 1),
                ('curseforge-reference', 'curseforge', '42', '7', 1),
                ('mcarchive-reference', 'mcarchive', 'project-uuid',
                    'version-uuid', 1);
            "#,
        )
        .execute(&upgraded)
        .await
        .unwrap();

        MIGRATOR.run(&upgraded).await.unwrap();

        let references: Vec<(String, String, Option<String>, Option<String>)> =
            sqlx::query_as(
                "SELECT provider, provider_project_id, provider_release_id, provider_file_id
                 FROM instance_content_provider_refs
                 ORDER BY provider",
            )
            .fetch_all(&upgraded)
            .await
            .unwrap();
        assert_eq!(
            references,
            vec![
                (
                    "curseforge".to_string(),
                    "42".to_string(),
                    Some("7".to_string()),
                    Some("7".to_string()),
                ),
                (
                    "mcarchive".to_string(),
                    "project-uuid".to_string(),
                    Some("version-uuid".to_string()),
                    None,
                ),
                (
                    "modrinth".to_string(),
                    "modrinth-project".to_string(),
                    Some("modrinth-version".to_string()),
                    None,
                ),
            ]
        );

        sqlx::query(
            "INSERT INTO instance_content_provider_refs (
                content_entry_id, provider, provider_project_id,
                provider_release_id, provider_file_id, is_origin
            ) VALUES (?, 'mcarchive', 'project-uuid', 'version-uuid', 'file-uuid', 0)",
        )
        .bind("mcarchive-reference")
        .execute(&upgraded)
        .await
        .unwrap();
        let mcarchive_file_id: Option<String> = sqlx::query_scalar(
            "SELECT provider_file_id FROM instance_content_provider_refs
             WHERE content_entry_id = 'mcarchive-reference'
               AND provider_file_id = 'file-uuid'",
        )
        .fetch_one(&upgraded)
        .await
        .unwrap();
        assert_eq!(mcarchive_file_id.as_deref(), Some("file-uuid"));

        let old_table_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(
                SELECT 1 FROM sqlite_master
                WHERE type = 'table'
                    AND name = 'instance_content_provider_refs_old'
            )",
        )
        .fetch_one(&upgraded)
        .await
        .unwrap();
        assert!(!old_table_exists);
        let foreign_key_errors: Vec<(String, i64, String, i64)> =
            sqlx::query_as("PRAGMA foreign_key_check")
                .fetch_all(&upgraded)
                .await
                .unwrap();
        assert!(foreign_key_errors.is_empty());
    }

    #[allow(dead_code)]
    async fn settings_snapshot(pool: &Pool<Sqlite>) -> Vec<(String, String)> {
        let columns: Vec<String> = sqlx::query_scalar(
            "SELECT name FROM pragma_table_info('settings') ORDER BY cid",
        )
        .fetch_all(pool)
        .await
        .unwrap();
        let mut snapshot = Vec::with_capacity(columns.len());
        for column in columns {
            let escaped = column.replace('"', "\"\"");
            let value: String = sqlx::query_scalar(&format!(
                "SELECT quote(\"{escaped}\") FROM settings WHERE id = 0"
            ))
            .fetch_one(pool)
            .await
            .unwrap();
            snapshot.push((column, value));
        }
        snapshot
    }

    #[tokio::test]
    async fn terracotta_public_nodes_migration_preserves_existing_settings() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .unwrap();
        let previous_migrator = Migrator {
            migrations: std::borrow::Cow::Owned(
                MIGRATOR
                    .iter()
                    .filter(|migration| {
                        migration.version
                            < TERRACOTTA_PUBLIC_NODES_MIGRATION_VERSION
                    })
                    .cloned()
                    .collect(),
            ),
            ..Migrator::DEFAULT
        };
        previous_migrator.run(&pool).await.unwrap();
        sqlx::query("UPDATE settings SET locale = 'zh-TW' WHERE id = 0")
            .execute(&pool)
            .await
            .unwrap();

        MIGRATOR.run(&pool).await.unwrap();

        let (locale, nodes): (String, String) = sqlx::query_as(
            "SELECT locale, json(terracotta_public_nodes) FROM settings WHERE id = 0",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(locale, "zh-TW");
        assert_eq!(nodes, "[\"wss://center.node.1tmc.top\"]");
        let foreign_key_errors: Vec<(String, i64, String, i64)> =
            sqlx::query_as("PRAGMA foreign_key_check")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert!(foreign_key_errors.is_empty());
    }

    #[tokio::test]
    async fn curseforge_download_restriction_bypass_migration_defaults_on() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .unwrap();
        let previous_migrator = Migrator {
            migrations: std::borrow::Cow::Owned(
                MIGRATOR
                    .iter()
                    .filter(|migration| {
                        migration.version
                            < CURSEFORGE_DOWNLOAD_RESTRICTION_BYPASS_MIGRATION_VERSION
                    })
                    .cloned()
                    .collect(),
            ),
            ..Migrator::DEFAULT
        };
        previous_migrator.run(&pool).await.unwrap();
        sqlx::query("UPDATE settings SET locale = 'zh-CN' WHERE id = 0")
            .execute(&pool)
            .await
            .unwrap();

        MIGRATOR.run(&pool).await.unwrap();

        let (locale, bypass): (String, bool) = sqlx::query_as(
            "SELECT locale, bypass_curseforge_download_restrictions FROM settings WHERE id = 0",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(locale, "zh-CN");
        assert!(bypass);
        let foreign_key_errors: Vec<(String, i64, String, i64)> =
            sqlx::query_as("PRAGMA foreign_key_check")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert!(foreign_key_errors.is_empty());
    }

    #[test]
    fn default_channel_resolves_from_version() {
        for (version, expected) in [
            ("1.9.6", "release"),
            ("1.9.6.0", "release"),
            ("1.9.6-beta.3", "beta"),
            ("1.9.6-rc.1", "beta"),
            ("1.9.6-alpha.2", "beta"),
        ] {
            assert_eq!(
                default_update_channel_for(version),
                expected,
                "unexpected default channel for {version}"
            );
        }
    }

    #[tokio::test]
    async fn update_channel_falls_back_without_explicit_choice() {
        let directory = tempfile::tempdir().unwrap();
        let settings_dir = directory.path();

        assert_eq!(
            read_explicit_update_channel(settings_dir).await.unwrap(),
            None
        );
        assert_eq!(
            resolve_update_channel(settings_dir).await.unwrap(),
            default_update_channel()
        );

        std::fs::write(
            settings_dir.join(UPDATE_CHANNEL_STATE_FILE),
            r#"{"immediate_update_fetch":true}"#,
        )
        .unwrap();
        assert_eq!(
            read_explicit_update_channel(settings_dir).await.unwrap(),
            None
        );
        assert_eq!(
            resolve_update_channel(settings_dir).await.unwrap(),
            default_update_channel()
        );
    }

    #[tokio::test]
    async fn update_channel_honors_explicit_selection() {
        let directory = tempfile::tempdir().unwrap();
        let settings_dir = directory.path();

        std::fs::write(
            settings_dir.join(UPDATE_CHANNEL_STATE_FILE),
            r#"{"active_channel":"release"}"#,
        )
        .unwrap();
        assert_eq!(
            read_explicit_update_channel(settings_dir).await.unwrap(),
            Some("release")
        );
        assert_eq!(
            resolve_update_channel(settings_dir).await.unwrap(),
            "release"
        );

        std::fs::write(
            settings_dir.join(UPDATE_CHANNEL_STATE_FILE),
            r#"{"active_channel":"beta"}"#,
        )
        .unwrap();
        assert_eq!(
            read_explicit_update_channel(settings_dir).await.unwrap(),
            Some("beta")
        );
        assert_eq!(resolve_update_channel(settings_dir).await.unwrap(), "beta");

        std::fs::write(
            settings_dir.join(UPDATE_CHANNEL_STATE_FILE),
            r#"{"active_channel":"dev"}"#,
        )
        .unwrap();
        assert!(read_explicit_update_channel(settings_dir).await.is_err());
        assert!(resolve_update_channel(settings_dir).await.is_err());
    }

    #[tokio::test]
    async fn reconcile_moves_database_without_explicit_choice() {
        let channel = default_update_channel();
        let other = if channel == "release" {
            "beta"
        } else {
            "release"
        };
        let directory = tempfile::tempdir().unwrap();
        let settings_dir = directory.path();

        let source_dir = settings_dir.join(other);
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::write(source_dir.join(LEGACY_APP_DB_FILE), "existing data")
            .unwrap();
        std::fs::write(
            source_dir.join(format!("{LEGACY_APP_DB_FILE}-wal")),
            "wal",
        )
        .unwrap();

        reconcile_default_channel_database(settings_dir)
            .await
            .unwrap();

        let target_dir = settings_dir.join(channel);
        assert_eq!(
            std::fs::read_to_string(target_dir.join(LEGACY_APP_DB_FILE))
                .unwrap(),
            "existing data"
        );
        assert_eq!(
            std::fs::read_to_string(
                target_dir.join(format!("{LEGACY_APP_DB_FILE}-wal"))
            )
            .unwrap(),
            "wal"
        );
        assert!(!source_dir.join(LEGACY_APP_DB_FILE).exists());
        // The migration marker is removed once the move has completed.
        assert!(!target_dir.join(CHANNEL_RECONCILE_MARKER_FILE).exists());
    }

    #[tokio::test]
    async fn reconcile_never_overwrites_or_deletes() {
        let channel = default_update_channel();
        let other = if channel == "release" {
            "beta"
        } else {
            "release"
        };
        let directory = tempfile::tempdir().unwrap();
        let settings_dir = directory.path();

        let source_dir = settings_dir.join(other);
        std::fs::create_dir_all(&source_dir).unwrap();
        let source_db = source_dir.join(LEGACY_APP_DB_FILE);
        std::fs::write(&source_db, "untouched").unwrap();

        // An explicit channel choice disables reconciliation entirely.
        std::fs::write(
            settings_dir.join(UPDATE_CHANNEL_STATE_FILE),
            r#"{"active_channel":"release"}"#,
        )
        .unwrap();
        reconcile_default_channel_database(settings_dir)
            .await
            .unwrap();
        assert!(source_db.exists());
        assert!(!settings_dir.join(channel).join(LEGACY_APP_DB_FILE).exists());

        // An existing database in the default channel is left alone.
        std::fs::remove_file(settings_dir.join(UPDATE_CHANNEL_STATE_FILE))
            .unwrap();
        let default_dir = settings_dir.join(channel);
        std::fs::create_dir_all(&default_dir).unwrap();
        let default_db = default_dir.join(LEGACY_APP_DB_FILE);
        std::fs::write(&default_db, "default data").unwrap();
        reconcile_default_channel_database(settings_dir)
            .await
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(&default_db).unwrap(),
            "default data"
        );
        assert_eq!(std::fs::read_to_string(&source_db).unwrap(), "untouched");
    }

    #[tokio::test]
    async fn reconcile_resumes_after_an_interrupted_attempt() {
        let channel = default_update_channel();
        let other = if channel == "release" {
            "beta"
        } else {
            "release"
        };
        let directory = tempfile::tempdir().unwrap();
        let settings_dir = directory.path();

        // A previous attempt crashed after writing its marker and moving the
        // -wal sidecar; the marker attributes those files to this migration.
        let source_dir = settings_dir.join(other);
        std::fs::create_dir_all(&source_dir).unwrap();
        let source_db = source_dir.join(LEGACY_APP_DB_FILE);
        std::fs::write(&source_db, "existing data").unwrap();
        let target_dir = settings_dir.join(channel);
        std::fs::create_dir_all(&target_dir).unwrap();
        std::fs::write(target_dir.join(CHANNEL_RECONCILE_MARKER_FILE), other)
            .unwrap();
        std::fs::write(
            target_dir.join(format!("{LEGACY_APP_DB_FILE}-wal")),
            "wal",
        )
        .unwrap();

        reconcile_default_channel_database(settings_dir)
            .await
            .unwrap();

        assert_eq!(
            std::fs::read_to_string(target_dir.join(LEGACY_APP_DB_FILE))
                .unwrap(),
            "existing data"
        );
        assert_eq!(
            std::fs::read_to_string(
                target_dir.join(format!("{LEGACY_APP_DB_FILE}-wal"))
            )
            .unwrap(),
            "wal"
        );
        assert!(!source_db.exists());
        assert!(!target_dir.join(CHANNEL_RECONCILE_MARKER_FILE).exists());
    }

    #[tokio::test]
    async fn reconcile_aborts_on_foreign_orphan_sidecar() {
        let channel = default_update_channel();
        let other = if channel == "release" {
            "beta"
        } else {
            "release"
        };
        let directory = tempfile::tempdir().unwrap();
        let settings_dir = directory.path();

        // The target channel holds a WAL file that no migration marker claims;
        // it may belong to a different database of the same name.
        let source_dir = settings_dir.join(other);
        std::fs::create_dir_all(&source_dir).unwrap();
        let source_db = source_dir.join(LEGACY_APP_DB_FILE);
        std::fs::write(&source_db, "existing data").unwrap();
        let target_dir = settings_dir.join(channel);
        std::fs::create_dir_all(&target_dir).unwrap();
        let target_wal = target_dir.join(format!("{LEGACY_APP_DB_FILE}-wal"));
        std::fs::write(&target_wal, "foreign wal").unwrap();

        reconcile_default_channel_database(settings_dir)
            .await
            .unwrap();

        // Nothing is moved, created, or deleted.
        assert_eq!(
            std::fs::read_to_string(&source_db).unwrap(),
            "existing data"
        );
        assert_eq!(
            std::fs::read_to_string(&target_wal).unwrap(),
            "foreign wal"
        );
        assert!(!target_dir.join(LEGACY_APP_DB_FILE).exists());
        assert!(!target_dir.join(CHANNEL_RECONCILE_MARKER_FILE).exists());
    }

    #[tokio::test]
    async fn reconcile_aborts_on_mismatched_marker() {
        let channel = default_update_channel();
        let other = if channel == "release" {
            "beta"
        } else {
            "release"
        };
        let directory = tempfile::tempdir().unwrap();
        let settings_dir = directory.path();

        let source_dir = settings_dir.join(other);
        std::fs::create_dir_all(&source_dir).unwrap();
        let source_db = source_dir.join(LEGACY_APP_DB_FILE);
        std::fs::write(&source_db, "existing data").unwrap();
        let target_dir = settings_dir.join(channel);
        std::fs::create_dir_all(&target_dir).unwrap();
        std::fs::write(target_dir.join(CHANNEL_RECONCILE_MARKER_FILE), channel)
            .unwrap();

        reconcile_default_channel_database(settings_dir)
            .await
            .unwrap();

        assert_eq!(
            std::fs::read_to_string(&source_db).unwrap(),
            "existing data"
        );
        assert!(!target_dir.join(LEGACY_APP_DB_FILE).exists());
        assert_eq!(
            std::fs::read_to_string(
                target_dir.join(CHANNEL_RECONCILE_MARKER_FILE)
            )
            .unwrap(),
            channel
        );
    }

    #[tokio::test]
    async fn reconcile_aborts_when_target_sidecar_appears_mid_flight() {
        let channel = default_update_channel();
        let other = if channel == "release" {
            "beta"
        } else {
            "release"
        };
        let directory = tempfile::tempdir().unwrap();
        let settings_dir = directory.path();

        // The marker matches, but a foreign sidecar appeared next to the one
        // this migration already moved; do not move the main database.
        let source_dir = settings_dir.join(other);
        std::fs::create_dir_all(&source_dir).unwrap();
        let source_db = source_dir.join(LEGACY_APP_DB_FILE);
        std::fs::write(&source_db, "existing data").unwrap();
        let source_wal = source_dir.join(format!("{LEGACY_APP_DB_FILE}-wal"));
        std::fs::write(&source_wal, "source wal").unwrap();
        let target_dir = settings_dir.join(channel);
        std::fs::create_dir_all(&target_dir).unwrap();
        std::fs::write(target_dir.join(CHANNEL_RECONCILE_MARKER_FILE), other)
            .unwrap();
        let target_wal = target_dir.join(format!("{LEGACY_APP_DB_FILE}-wal"));
        std::fs::write(&target_wal, "target wal").unwrap();

        reconcile_default_channel_database(settings_dir)
            .await
            .unwrap();

        assert_eq!(
            std::fs::read_to_string(&source_db).unwrap(),
            "existing data"
        );
        assert_eq!(std::fs::read_to_string(&source_wal).unwrap(), "source wal");
        assert_eq!(std::fs::read_to_string(&target_wal).unwrap(), "target wal");
        assert!(!target_dir.join(LEGACY_APP_DB_FILE).exists());
        assert!(target_dir.join(CHANNEL_RECONCILE_MARKER_FILE).exists());
    }

    #[tokio::test]
    async fn reconcile_cleans_up_marker_after_commit_crash() {
        let channel = default_update_channel();
        let other = if channel == "release" {
            "beta"
        } else {
            "release"
        };
        let directory = tempfile::tempdir().unwrap();
        let settings_dir = directory.path();

        // The main database was moved, but the process crashed before the
        // marker could be removed.
        let target_dir = settings_dir.join(channel);
        std::fs::create_dir_all(&target_dir).unwrap();
        let default_db = target_dir.join(LEGACY_APP_DB_FILE);
        std::fs::write(&default_db, "default data").unwrap();
        std::fs::write(target_dir.join(CHANNEL_RECONCILE_MARKER_FILE), other)
            .unwrap();

        reconcile_default_channel_database(settings_dir)
            .await
            .unwrap();

        assert_eq!(
            std::fs::read_to_string(&default_db).unwrap(),
            "default data"
        );
        assert!(!target_dir.join(CHANNEL_RECONCILE_MARKER_FILE).exists());
    }

    fn initial_migration() -> &'static Migration {
        MIGRATOR
            .iter()
            .find(|migration| migration.version == INITIAL_MIGRATION_VERSION)
            .expect("initial migration should be embedded")
    }

    fn provider_qualified_content_migration() -> &'static Migration {
        MIGRATOR
            .iter()
            .find(|migration| {
                migration.version
                    == PROVIDER_QUALIFIED_CONTENT_MIGRATION_VERSION
            })
            .expect("provider-qualified content migration should be embedded")
    }

    fn official_preferred_download_source_migration() -> &'static Migration {
        MIGRATOR
            .iter()
            .find(|migration| {
                migration.version
                    == OFFICIAL_PREFERRED_DOWNLOAD_SOURCE_MIGRATION_VERSION
            })
            .expect(
                "official-preferred download source migration should be embedded",
            )
    }

    fn ai_provider_migration() -> &'static Migration {
        MIGRATOR
            .iter()
            .find(|migration| {
                migration.version == AI_PROVIDER_MIGRATION_VERSION
            })
            .expect("AI provider migration should be embedded")
    }

    fn reconcile_provider_qualified_content_migration() -> &'static Migration {
        MIGRATOR
            .iter()
            .find(|migration| {
                migration.version
                    == RECONCILE_PROVIDER_QUALIFIED_CONTENT_MIGRATION_VERSION
            })
            .expect(
                "provider content reconciliation migration should be embedded",
            )
    }

    fn java_default_versions_migration() -> &'static Migration {
        MIGRATOR
            .iter()
            .find(|migration| {
                migration.version == JAVA_DEFAULT_VERSIONS_MIGRATION_VERSION
            })
            .expect("Java default versions migration should be embedded")
    }

    fn instance_content_ownership_migration() -> &'static Migration {
        MIGRATOR
            .iter()
            .find(|migration| {
                migration.version
                    == INSTANCE_CONTENT_OWNERSHIP_MIGRATION_VERSION
            })
            .expect("instance content ownership migration should be embedded")
    }

    async fn create_previous_java_versions_schema(pool: &Pool<Sqlite>) {
        sqlx::raw_sql(
            "
            CREATE TABLE java_versions (
                major_version INTEGER NOT NULL,
                full_version TEXT NOT NULL,
                architecture TEXT NOT NULL,
                path TEXT NOT NULL PRIMARY KEY,
                distribution TEXT
            );
            CREATE INDEX idx_java_versions_major_version
                ON java_versions(major_version);
            ",
        )
        .execute(pool)
        .await
        .unwrap();
    }

    fn checksum(contents: &[u8]) -> Vec<u8> {
        Sha384::digest(contents).to_vec()
    }

    fn decode_hex(value: &str) -> Vec<u8> {
        value
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| {
                let pair = std::str::from_utf8(pair).expect("ASCII hex");
                u8::from_str_radix(pair, 16).expect("valid hex")
            })
            .collect()
    }

    #[test]
    fn accepts_lf_and_crlf_variants_of_the_same_migration() {
        let migration = initial_migration();
        let lf = migration.sql.replace("\r\n", "\n").replace('\r', "\n");
        let crlf = lf.replace('\n', "\r\n");

        assert!(is_compatible_migration_checksum(
            migration.version,
            &checksum(lf.as_bytes()),
            migration,
        ));
        assert!(is_compatible_migration_checksum(
            migration.version,
            &checksum(crlf.as_bytes()),
            migration,
        ));
    }

    #[test]
    fn accepts_only_the_known_legacy_initial_migration() {
        let migration = initial_migration();
        let legacy_checksum = decode_hex(LEGACY_INITIAL_MIGRATION_CHECKSUMS[0]);

        assert!(is_compatible_migration_checksum(
            INITIAL_MIGRATION_VERSION,
            &legacy_checksum,
            migration,
        ));
        assert!(!is_compatible_migration_checksum(
            INITIAL_MIGRATION_VERSION + 1,
            &legacy_checksum,
            migration,
        ));
    }

    #[test]
    fn rejects_an_unknown_content_change() {
        let migration = initial_migration();
        let changed_checksum = checksum(
            format!("{}\n-- unknown change", migration.sql).as_bytes(),
        );

        assert!(!is_compatible_migration_checksum(
            migration.version,
            &changed_checksum,
            migration,
        ));
    }

    #[tokio::test]
    async fn reconciles_official_preferred_migration_with_trailing_blank_line()
    {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .unwrap();
        let previous_migrator = Migrator {
            migrations: std::borrow::Cow::Owned(
                MIGRATOR
                    .iter()
                    .filter(|migration| {
                        migration.version
                            < OFFICIAL_PREFERRED_DOWNLOAD_SOURCE_MIGRATION_VERSION
                    })
                    .cloned()
                    .collect(),
            ),
            ..Migrator::DEFAULT
        };
        previous_migrator.run(&pool).await.unwrap();
        sqlx::query(
            "UPDATE settings
             SET theme = 'light', locale = 'zh-CN'
             WHERE id = 0",
        )
        .execute(&pool)
        .await
        .unwrap();

        let migration = official_preferred_download_source_migration();
        let legacy_sql = format!("{}\n", migration.sql);
        let legacy_checksum = checksum(legacy_sql.as_bytes());
        assert_eq!(
            checksum_as_hex(&legacy_checksum),
            LEGACY_OFFICIAL_PREFERRED_DOWNLOAD_SOURCE_MIGRATION_CHECKSUMS[0]
        );
        sqlx::raw_sql(&legacy_sql).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO _sqlx_migrations (
                version, description, success, checksum, execution_time
             ) VALUES (?, ?, TRUE, ?, 0)",
        )
        .bind(migration.version)
        .bind(migration.description.as_ref())
        .bind(&legacy_checksum)
        .execute(&pool)
        .await
        .unwrap();

        reconcile_legacy_official_preferred_download_source_migration(&pool)
            .await
            .unwrap();
        let reconciled_checksum: Vec<u8> = sqlx::query_scalar(
            "SELECT checksum FROM _sqlx_migrations WHERE version = ?",
        )
        .bind(migration.version)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(reconciled_checksum, migration.checksum.as_ref());

        MIGRATOR.run(&pool).await.unwrap();

        let proxy_mode: String =
            sqlx::query_scalar("SELECT proxy_mode FROM settings WHERE id = 0")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(proxy_mode, "system");

        let proxy_url: String =
            sqlx::query_scalar("SELECT proxy_url FROM settings WHERE id = 0")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(proxy_url, "");

        let proxy_username: String = sqlx::query_scalar(
            "SELECT proxy_username FROM settings WHERE id = 0",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(proxy_username, "");

        let proxy_password: String = sqlx::query_scalar(
            "SELECT proxy_password FROM settings WHERE id = 0",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(proxy_password, "");

        let foreign_key_errors: Vec<(String, i64, String, i64)> =
            sqlx::query_as("PRAGMA foreign_key_check")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert!(foreign_key_errors.is_empty());
    }

    #[tokio::test]
    async fn reconciles_known_ai_provider_checksum_for_matching_schema() {
        const DISCARD_LEGACY_OPENAI_MIGRATION_VERSION: i64 = 20260805130000;

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let provider_migrator = Migrator {
            migrations: std::borrow::Cow::Owned(
                MIGRATOR
                    .iter()
                    .filter(|migration| {
                        migration.version
                            < DISCARD_LEGACY_OPENAI_MIGRATION_VERSION
                    })
                    .cloned()
                    .collect(),
            ),
            ..Migrator::DEFAULT
        };
        provider_migrator.run(&pool).await.unwrap();

        let migration = ai_provider_migration();
        let legacy_checksum =
            decode_hex(LEGACY_AI_PROVIDER_MIGRATION_CHECKSUMS[0]);
        sqlx::query(
            "UPDATE _sqlx_migrations SET checksum = ? WHERE version = ?",
        )
        .bind(&legacy_checksum)
        .bind(migration.version)
        .execute(&pool)
        .await
        .unwrap();

        reconcile_legacy_ai_provider_migration(&pool).await.unwrap();
        let reconciled_checksum: Vec<u8> = sqlx::query_scalar(
            "SELECT checksum FROM _sqlx_migrations WHERE version = ?",
        )
        .bind(migration.version)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(reconciled_checksum, migration.checksum.as_ref());

        MIGRATOR.run(&pool).await.unwrap();
        let cleanup_column: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('ai_settings')
             WHERE name = 'legacy_openai_credential_cleanup'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(cleanup_column, 1);
        let foreign_key_errors: Vec<(String, i64, String, i64)> =
            sqlx::query_as("PRAGMA foreign_key_check")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert!(foreign_key_errors.is_empty());
    }

    #[tokio::test]
    async fn rejects_legacy_official_preferred_checksum_for_old_schema() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let previous_migrator = Migrator {
            migrations: std::borrow::Cow::Owned(
                MIGRATOR
                    .iter()
                    .filter(|migration| {
                        migration.version
                            < OFFICIAL_PREFERRED_DOWNLOAD_SOURCE_MIGRATION_VERSION
                    })
                    .cloned()
                    .collect(),
            ),
            ..Migrator::DEFAULT
        };
        previous_migrator.run(&pool).await.unwrap();

        let migration = official_preferred_download_source_migration();
        let legacy_checksum = decode_hex(
            LEGACY_OFFICIAL_PREFERRED_DOWNLOAD_SOURCE_MIGRATION_CHECKSUMS[0],
        );
        sqlx::query(
            "INSERT INTO _sqlx_migrations (
                version, description, success, checksum, execution_time
             ) VALUES (?, ?, TRUE, ?, 0)",
        )
        .bind(migration.version)
        .bind(migration.description.as_ref())
        .bind(&legacy_checksum)
        .execute(&pool)
        .await
        .unwrap();

        reconcile_legacy_official_preferred_download_source_migration(&pool)
            .await
            .unwrap();
        let stored_checksum: Vec<u8> = sqlx::query_scalar(
            "SELECT checksum FROM _sqlx_migrations WHERE version = ?",
        )
        .bind(migration.version)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(stored_checksum, legacy_checksum);
        assert!(MIGRATOR.run(&pool).await.is_err());
    }

    #[test]
    fn embedded_migration_versions_are_unique() {
        let mut versions = HashSet::new();
        for migration in MIGRATOR.iter() {
            assert!(
                versions.insert(migration.version),
                "duplicate migration version {}",
                migration.version
            );
        }
    }

    #[tokio::test]
    async fn reconciles_fully_applied_proxy_config_without_migration_record() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .unwrap();
        let previous_migrator = Migrator {
            migrations: std::borrow::Cow::Owned(
                MIGRATOR
                    .iter()
                    .filter(|migration| {
                        migration.version < PROXY_CONFIG_MIGRATION_VERSION
                    })
                    .cloned()
                    .collect(),
            ),
            ..Migrator::DEFAULT
        };
        previous_migrator.run(&pool).await.unwrap();
        let migration = MIGRATOR
            .iter()
            .find(|migration| {
                migration.version == PROXY_CONFIG_MIGRATION_VERSION
            })
            .unwrap();
        sqlx::raw_sql(migration.sql.as_ref())
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "UPDATE settings
             SET proxy_mode = 'custom', proxy_url = 'http://localhost:7890',
                 proxy_username = 'user', proxy_password = 'secret'
             WHERE id = 0",
        )
        .execute(&pool)
        .await
        .unwrap();

        reconcile_existing_proxy_config_migration(&pool)
            .await
            .unwrap();
        let stored_checksum: Vec<u8> = sqlx::query_scalar(
            "SELECT checksum FROM _sqlx_migrations WHERE version = ?",
        )
        .bind(PROXY_CONFIG_MIGRATION_VERSION)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(stored_checksum, migration.checksum.as_ref());

        MIGRATOR.run(&pool).await.unwrap();
        let settings: (String, String, String, String) = sqlx::query_as(
            "SELECT proxy_mode, proxy_url, proxy_username, proxy_password
             FROM settings WHERE id = 0",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            settings,
            (
                "custom".to_string(),
                "http://localhost:7890".to_string(),
                "user".to_string(),
                "secret".to_string(),
            )
        );
        let foreign_key_errors: Vec<(String, i64, String, i64)> =
            sqlx::query_as("PRAGMA foreign_key_check")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert!(foreign_key_errors.is_empty());
    }

    #[tokio::test]
    async fn does_not_reconcile_incomplete_proxy_config_schema() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let previous_migrator = Migrator {
            migrations: std::borrow::Cow::Owned(
                MIGRATOR
                    .iter()
                    .filter(|migration| {
                        migration.version < PROXY_CONFIG_MIGRATION_VERSION
                    })
                    .cloned()
                    .collect(),
            ),
            ..Migrator::DEFAULT
        };
        previous_migrator.run(&pool).await.unwrap();
        sqlx::raw_sql(
            "ALTER TABLE settings ADD COLUMN proxy_mode TEXT NOT NULL DEFAULT 'system';
             ALTER TABLE settings ADD COLUMN proxy_url TEXT NOT NULL DEFAULT '';
             ALTER TABLE settings ADD COLUMN proxy_username TEXT NOT NULL DEFAULT '';
             ALTER TABLE settings ADD COLUMN proxy_password TEXT NOT NULL DEFAULT '';",
        )
        .execute(&pool)
        .await
        .unwrap();

        reconcile_existing_proxy_config_migration(&pool)
            .await
            .unwrap();
        let migration_recorded: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM _sqlx_migrations WHERE version = ?)",
        )
        .bind(PROXY_CONFIG_MIGRATION_VERSION)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(!migration_recorded);
        assert!(MIGRATOR.run(&pool).await.is_err());
    }

    #[tokio::test]
    async fn reconciles_fully_applied_content_favorites_without_migration_record()
     {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .unwrap();
        let previous_migrator = Migrator {
            migrations: std::borrow::Cow::Owned(
                MIGRATOR
                    .iter()
                    .filter(|migration| {
                        migration.version < CONTENT_FAVORITES_MIGRATION_VERSION
                    })
                    .cloned()
                    .collect(),
            ),
            ..Migrator::DEFAULT
        };
        previous_migrator.run(&pool).await.unwrap();
        let migration = MIGRATOR
            .iter()
            .find(|migration| {
                migration.version == CONTENT_FAVORITES_MIGRATION_VERSION
            })
            .unwrap();
        sqlx::raw_sql(migration.sql.as_ref())
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO content_favorites (
                provider, project_id, content_type, saved_at
             ) VALUES
                ('modrinth', 'modrinth-project', 'mod', 10),
                ('curseforge', 'curseforge-project', 'resourcepack', 20)",
        )
        .execute(&pool)
        .await
        .unwrap();

        reconcile_existing_content_favorites_migration(&pool)
            .await
            .unwrap();
        let stored_checksum: Vec<u8> = sqlx::query_scalar(
            "SELECT checksum FROM _sqlx_migrations WHERE version = ?",
        )
        .bind(CONTENT_FAVORITES_MIGRATION_VERSION)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(stored_checksum, migration.checksum.as_ref());

        MIGRATOR.run(&pool).await.unwrap();
        let favorites: Vec<(String, String, String, i64)> = sqlx::query_as(
            "SELECT provider, project_id, content_type, saved_at
             FROM content_favorites
             ORDER BY provider",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(
            favorites,
            [
                (
                    "curseforge".to_string(),
                    "curseforge-project".to_string(),
                    "resourcepack".to_string(),
                    20,
                ),
                (
                    "modrinth".to_string(),
                    "modrinth-project".to_string(),
                    "mod".to_string(),
                    10,
                ),
            ]
        );
        sqlx::query(
            "INSERT INTO content_favorites (
                provider, project_id, content_type, saved_at
             ) VALUES ('mcarchive', 'mcarchive-project', 'mod', 30)",
        )
        .execute(&pool)
        .await
        .unwrap();
        let old_table_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(
                SELECT 1 FROM sqlite_master
                WHERE type = 'table' AND name = 'content_favorites_old'
             )",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(!old_table_exists);
        let foreign_key_errors: Vec<(String, i64, String, i64)> =
            sqlx::query_as("PRAGMA foreign_key_check")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert!(foreign_key_errors.is_empty());
    }

    #[tokio::test]
    async fn does_not_reconcile_incomplete_content_favorites_schema() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let previous_migrator = Migrator {
            migrations: std::borrow::Cow::Owned(
                MIGRATOR
                    .iter()
                    .filter(|migration| {
                        migration.version < CONTENT_FAVORITES_MIGRATION_VERSION
                    })
                    .cloned()
                    .collect(),
            ),
            ..Migrator::DEFAULT
        };
        previous_migrator.run(&pool).await.unwrap();
        sqlx::raw_sql(
            "CREATE TABLE content_favorites (
                provider TEXT NOT NULL,
                project_id TEXT NOT NULL,
                content_type TEXT NOT NULL,
                saved_at INTEGER NOT NULL,
                PRIMARY KEY (provider, project_id)
             );
             CREATE INDEX content_favorites_saved_at_idx
                ON content_favorites (saved_at DESC);",
        )
        .execute(&pool)
        .await
        .unwrap();

        reconcile_existing_content_favorites_migration(&pool)
            .await
            .unwrap();
        let migration_recorded: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM _sqlx_migrations WHERE version = ?)",
        )
        .bind(CONTENT_FAVORITES_MIGRATION_VERSION)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(!migration_recorded);
        assert!(MIGRATOR.run(&pool).await.is_err());
    }

    #[tokio::test]
    async fn proxy_config_migration_preserves_existing_settings() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .unwrap();
        let previous_migrator = Migrator {
            migrations: std::borrow::Cow::Owned(
                MIGRATOR
                    .iter()
                    .filter(|migration| {
                        migration.version
                            < SYSTEM_PROXY_SETTING_MIGRATION_VERSION
                    })
                    .cloned()
                    .collect(),
            ),
            ..Migrator::DEFAULT
        };
        previous_migrator.run(&pool).await.unwrap();
        sqlx::query(
            "
            UPDATE settings
            SET
                max_concurrent_downloads = 17,
                theme = 'light',
                locale = 'zh-CN',
                minecraft_metadata_source = 'official_only',
                minecraft_file_source = 'mirror_preferred',
                modrinth_source = 'auto',
                curseforge_source = 'official_only'
            WHERE id = 0
            ",
        )
        .execute(&pool)
        .await
        .unwrap();

        MIGRATOR.run(&pool).await.unwrap();

        let proxy_mode: String =
            sqlx::query_scalar("SELECT proxy_mode FROM settings WHERE id = 0")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(proxy_mode, "system");

        let proxy_url: String =
            sqlx::query_scalar("SELECT proxy_url FROM settings WHERE id = 0")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(proxy_url, "");

        let proxy_username: String = sqlx::query_scalar(
            "SELECT proxy_username FROM settings WHERE id = 0",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(proxy_username, "");

        let proxy_password: String = sqlx::query_scalar(
            "SELECT proxy_password FROM settings WHERE id = 0",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(proxy_password, "");

        let proxy_column_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('settings')
             WHERE name = 'use_system_proxy'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(proxy_column_count, 0);

        let legacy_table_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(
                SELECT 1 FROM sqlite_master
                WHERE type = 'table'
                    AND name = 'settings_with_official_preferred'
            )",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(!legacy_table_exists);

        let foreign_key_errors: Vec<(String, i64, String, i64)> =
            sqlx::query_as("PRAGMA foreign_key_check")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert!(foreign_key_errors.is_empty());

        sqlx::query(
            "
            UPDATE settings
            SET
                minecraft_metadata_source = 'official_preferred',
                minecraft_file_source = 'official_preferred',
                modrinth_source = 'official_preferred',
                curseforge_source = 'official_preferred'
            WHERE id = 0
            ",
        )
        .execute(&pool)
        .await
        .unwrap();
        let modes: (String, String, String, String) = sqlx::query_as(
            "
            SELECT
                minecraft_metadata_source,
                minecraft_file_source,
                modrinth_source,
                curseforge_source
            FROM settings
            WHERE id = 0
            ",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            modes,
            (
                "official_preferred".to_string(),
                "official_preferred".to_string(),
                "official_preferred".to_string(),
                "official_preferred".to_string(),
            )
        );
        assert!(
            sqlx::query(
                "UPDATE settings SET minecraft_file_source = 'invalid' WHERE id = 0"
            )
            .execute(&pool)
            .await
            .is_err()
        );
    }

    #[tokio::test]
    async fn upgrades_instance_content_ownership_without_losing_files() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .unwrap();
        let previous_migrator = Migrator {
            migrations: std::borrow::Cow::Owned(
                MIGRATOR
                    .iter()
                    .filter(|migration| {
                        migration.version
                            < INSTANCE_CONTENT_OWNERSHIP_MIGRATION_VERSION
                    })
                    .cloned()
                    .collect(),
            ),
            ..Migrator::DEFAULT
        };
        previous_migrator.run(&pool).await.unwrap();
        sqlx::raw_sql(
            r#"
            INSERT INTO instances (
                id, path, applied_content_set_id, install_stage,
                launcher_feature_version, name, created, modified
            ) VALUES
                ('cf', 'cf', NULL, 'installed', '1', 'CF Pack', 1, 1),
                ('mr', 'mr', NULL, 'installed', '1', 'MR Pack', 1, 1),
                ('imported', 'imported', NULL, 'installed', '1',
                    'Imported Pack', 1, 1);

            INSERT INTO instance_content_sets (
                id, instance_id, name, source_kind, status, game_version,
                loader, created, modified
            ) VALUES
                ('cf-set', 'cf', 'Default', 'curseforge', 'applied',
                    '1.12.2', 'forge', 1, 1),
                ('mr-set', 'mr', 'Default', 'modrinth_modpack', 'applied',
                    '1.20.1', 'fabric', 1, 1),
                ('imported-set', 'imported', 'Default', 'imported_modpack',
                    'applied', '1.20.1', 'fabric', 1, 1);

            UPDATE instances SET applied_content_set_id = id || '-set';

            INSERT INTO instance_links (
                instance_id, link_kind, modrinth_project_id,
                modrinth_version_id, curseforge_project_id,
                curseforge_file_id, imported_name, imported_filename
            ) VALUES
                ('cf', 'curseforge_modpack', NULL, NULL, 285109, 4612979,
                    NULL, NULL),
                ('mr', 'modrinth_modpack', 'mr-pack', 'mr-pack-version',
                    NULL, NULL, NULL, NULL),
                ('imported', 'imported_modpack', NULL, NULL, NULL, NULL,
                    'Imported Pack', 'pack.zip');

            INSERT INTO instance_files (
                id, instance_id, relative_path, file_name, enabled, sha1,
                size, missing, added_at, modified_at
            ) VALUES
                ('cf-pack-file', 'cf', 'mods/cf-pack.jar', 'cf-pack.jar',
                    1, 'cf-pack-sha1', 10, 0, 1, 1),
                ('cf-added-file', 'cf', 'mods/cf-added.jar', 'cf-added.jar',
                    1, 'cf-added-sha1', 11, 0, 1, 1),
                ('cf-duplicate-file', 'cf', 'mods/cf-pack-old.jar',
                    'cf-pack-old.jar', 1, 'cf-pack-old-sha1', 10, 1, 1, 2),
                ('mr-pack-file', 'mr', 'mods/mr-pack.jar', 'mr-pack.jar',
                    1, 'mr-pack-sha1', 12, 0, 1, 1),
                ('mr-added-file', 'mr', 'mods/mr-added.jar', 'mr-added.jar',
                    1, 'mr-added-sha1', 13, 0, 1, 1),
                ('imported-file', 'imported', 'mods/imported.jar',
                    'imported.jar', 0, 'imported-sha1', 14, 0, 1, 1),
                ('imported-duplicate-file', 'imported',
                    'MODS\IMPORTED.JAR', 'IMPORTED.JAR', 1,
                    'imported-duplicate-sha1', 14, 1, 1, 2);

            INSERT INTO instance_content_entries (
                id, instance_id, content_set_id, file_id, project_type,
                source_kind, server_requirement, client_requirement,
                enabled, added_at, modified_at
            ) VALUES
                ('cf-pack-entry', 'cf', 'cf-set', 'cf-pack-file', 'mod',
                    'curseforge', 'required', 'required', 1, 1, 1),
                ('cf-added-entry', 'cf', 'cf-set', 'cf-added-file', 'mod',
                    'curseforge', 'required', 'required', 1, 1, 1),
                ('cf-duplicate-entry', 'cf', 'cf-set',
                    'cf-duplicate-file', 'mod', 'curseforge', 'required',
                    'required', 1, 1, 2),
                ('mr-pack-entry', 'mr', 'mr-set', 'mr-pack-file', 'mod',
                    'modrinth_modpack', 'required', 'required', 1, 1, 1),
                ('mr-added-entry', 'mr', 'mr-set', 'mr-added-file', 'mod',
                    'local', 'required', 'required', 1, 1, 1),
                ('imported-entry', 'imported', 'imported-set',
                    'imported-file', 'mod', 'imported_modpack', 'optional',
                    'optional', 0, 1, 1),
                ('imported-duplicate-entry', 'imported', 'imported-set',
                    'imported-duplicate-file', 'mod', 'imported_modpack',
                    'optional', 'optional', 1, 1, 2);

            INSERT INTO instance_content_provider_refs (
                content_entry_id, provider, provider_project_id,
                provider_release_id, is_origin
            ) VALUES
                ('cf-pack-entry', 'curseforge', '123', '456', 1),
                ('cf-duplicate-entry', 'curseforge', '123', '455', 1),
                ('cf-added-entry', 'curseforge', '789', '987', 1),
                ('mr-pack-entry', 'modrinth', 'mr-project', 'mr-version', 1);
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        let file_count_before: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM instance_files")
                .fetch_one(&pool)
                .await
                .unwrap();
        reconcile_pending_instance_content_ownership_duplicates(&pool)
            .await
            .unwrap();
        let reclassified_entries: Vec<(String, String)> = sqlx::query_as(
            "SELECT id, source_kind FROM instance_content_entries
             WHERE id IN ('cf-duplicate-entry', 'imported-duplicate-entry')
             ORDER BY id",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(
            reclassified_entries,
            vec![
                ("cf-duplicate-entry".to_string(), "local".to_string()),
                ("imported-duplicate-entry".to_string(), "local".to_string(),),
            ]
        );
        sqlx::raw_sql(instance_content_ownership_migration().sql.as_ref())
            .execute(&pool)
            .await
            .unwrap();

        let ownership: Vec<(String, String)> = sqlx::query_as(
            "SELECT id, ownership_kind FROM instance_content_entries
             ORDER BY id",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(
            ownership,
            vec![
                ("cf-added-entry".to_string(), "pack_managed".to_string()),
                ("cf-duplicate-entry".to_string(), "user_added".to_string(),),
                ("cf-pack-entry".to_string(), "pack_managed".to_string()),
                (
                    "imported-duplicate-entry".to_string(),
                    "user_added".to_string(),
                ),
                ("imported-entry".to_string(), "pack_managed".to_string()),
                ("mr-added-entry".to_string(), "user_added".to_string()),
                ("mr-pack-entry".to_string(), "pack_managed".to_string()),
            ]
        );
        let members: Vec<(String, Option<String>, i64, String)> =
            sqlx::query_as(
                "SELECT content_entry_id, provider_project_id, reconciled,
                        override_kind
                 FROM instance_pack_members
                 ORDER BY content_entry_id",
            )
            .fetch_all(&pool)
            .await
            .unwrap();
        assert_eq!(members.len(), 4);
        assert!(members.iter().any(|member| {
            member.0 == "cf-pack-entry"
                && member.1.as_deref() == Some("123")
                && member.2 == 0
        }));
        assert!(members.iter().any(|member| {
            member.0 == "cf-added-entry"
                && member.1.as_deref() == Some("789")
                && member.2 == 0
        }));
        assert!(members.iter().any(|member| {
            member.0 == "imported-entry"
                && member.1.is_none()
                && member.3 == "disabled"
        }));
        let provider_refs: Vec<(String, String, String, Option<String>)> =
            sqlx::query_as(
                "SELECT content_entry_id, provider, provider_project_id,
                        provider_release_id
                 FROM instance_content_provider_refs
                 ORDER BY content_entry_id",
            )
            .fetch_all(&pool)
            .await
            .unwrap();
        assert_eq!(
            provider_refs,
            vec![
                (
                    "cf-added-entry".to_string(),
                    "curseforge".to_string(),
                    "789".to_string(),
                    Some("987".to_string()),
                ),
                (
                    "cf-duplicate-entry".to_string(),
                    "curseforge".to_string(),
                    "123".to_string(),
                    Some("455".to_string()),
                ),
                (
                    "cf-pack-entry".to_string(),
                    "curseforge".to_string(),
                    "123".to_string(),
                    Some("456".to_string()),
                ),
                (
                    "mr-pack-entry".to_string(),
                    "modrinth".to_string(),
                    "mr-project".to_string(),
                    Some("mr-version".to_string()),
                ),
            ]
        );
        let file_count_after: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM instance_files")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(file_count_after, file_count_before);
        let foreign_key_errors: Vec<(String, i64, String, i64)> =
            sqlx::query_as("PRAGMA foreign_key_check")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert!(foreign_key_errors.is_empty());
    }

    #[tokio::test]
    async fn creates_java_default_versions_for_a_fresh_database() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .unwrap();
        reconcile_pending_instance_content_ownership_duplicates(&pool)
            .await
            .unwrap();
        MIGRATOR.run(&pool).await.unwrap();

        let defaults: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM java_default_versions")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(defaults, 0);
        let foreign_key_errors: Vec<(String, i64, String, i64)> =
            sqlx::query_as("PRAGMA foreign_key_check")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert!(foreign_key_errors.is_empty());
    }

    #[tokio::test]
    async fn upgrades_multiple_java_installations_with_stable_defaults() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .unwrap();
        create_previous_java_versions_schema(&pool).await;
        sqlx::raw_sql(
            "
            INSERT INTO java_versions (
                major_version, full_version, architecture, path, distribution
            ) VALUES
                (21, '21.0.7', 'aarch64', '/java/zulu-21', 'Azul'),
                (21, '21.0.8', 'aarch64', '/java/temurin-21', 'Eclipse'),
                (17, '17.0.12', 'x86_64', '/java/custom-17', 'Unknown / Custom'),
                (8, '1.8.0_402', 'x86_64', '/java/java-8', NULL);
            ",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::raw_sql(java_default_versions_migration().sql.as_ref())
            .execute(&pool)
            .await
            .unwrap();

        let defaults: Vec<(i64, String)> = sqlx::query_as(
            "SELECT major_version, path FROM java_default_versions
             ORDER BY major_version DESC",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(
            defaults,
            vec![
                (21, "/java/temurin-21".to_string()),
                (17, "/java/custom-17".to_string()),
                (8, "/java/java-8".to_string()),
            ]
        );

        let mismatched_default = sqlx::query(
            "INSERT INTO java_default_versions (major_version, path)
             VALUES (25, '/java/java-8')",
        )
        .execute(&pool)
        .await;
        assert!(mismatched_default.is_err());

        sqlx::query(
            "DELETE FROM java_versions WHERE path = '/java/temurin-21'",
        )
        .execute(&pool)
        .await
        .unwrap();
        let java_21_default: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM java_default_versions
             WHERE major_version = 21",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(java_21_default, 0);

        let remaining_java_versions: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM java_versions")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(remaining_java_versions, 3);
        let foreign_key_errors: Vec<(String, i64, String, i64)> =
            sqlx::query_as("PRAGMA foreign_key_check")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert!(foreign_key_errors.is_empty());
    }

    #[tokio::test]
    async fn upgrades_legacy_content_schema_without_provider_leakage() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(
            r#"
            CREATE TABLE instances (id TEXT PRIMARY KEY);
            CREATE TABLE instance_content_sets (id TEXT PRIMARY KEY);
            CREATE TABLE instance_files (
                id TEXT PRIMARY KEY,
                relative_path TEXT NOT NULL
            );
            CREATE TABLE cache (
                data_type TEXT NOT NULL,
                data TEXT NOT NULL
            );
            CREATE TABLE instance_content_entries (
                id TEXT PRIMARY KEY,
                instance_id TEXT NOT NULL,
                content_set_id TEXT NOT NULL,
                file_id TEXT NULL,
                project_type TEXT NOT NULL,
                project_id TEXT NULL,
                version_id TEXT NULL,
                source_kind TEXT NOT NULL,
                server_requirement TEXT NOT NULL,
                client_requirement TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1,
                added_at INTEGER NOT NULL,
                modified_at INTEGER NOT NULL,
                FOREIGN KEY (instance_id) REFERENCES instances(id),
                FOREIGN KEY (content_set_id)
                    REFERENCES instance_content_sets(id),
                FOREIGN KEY (file_id) REFERENCES instance_files(id)
            );
            CREATE INDEX instance_content_entries_instance_id
                ON instance_content_entries(instance_id);
            CREATE INDEX instance_content_entries_content_set_id
                ON instance_content_entries(content_set_id);
            CREATE INDEX instance_content_entries_file_id
                ON instance_content_entries(file_id);
            CREATE INDEX instance_content_entries_project_id
                ON instance_content_entries(project_id);
            CREATE INDEX instance_content_entries_version_id
                ON instance_content_entries(version_id);
            CREATE INDEX instance_content_entries_source_kind
                ON instance_content_entries(source_kind);
            CREATE TABLE instance_content_update_checks (
                content_entry_id TEXT PRIMARY KEY,
                update_channel TEXT NOT NULL,
                update_version_id TEXT NULL,
                checked_at INTEGER NOT NULL,
                FOREIGN KEY (content_entry_id)
                    REFERENCES instance_content_entries(id)
            );
            CREATE INDEX instance_content_update_checks_update_version_id
                ON instance_content_update_checks(update_version_id);
            CREATE TABLE instance_content_provider_refs (
                content_entry_id TEXT NOT NULL,
                provider TEXT NOT NULL,
                project_id TEXT NOT NULL,
                version_id TEXT NULL,
                primary_ref INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (content_entry_id, provider),
                FOREIGN KEY (content_entry_id)
                    REFERENCES instance_content_entries(id)
            );
            CREATE INDEX instance_content_provider_refs_project
                ON instance_content_provider_refs(provider, project_id);
            CREATE INDEX instance_content_provider_refs_version
                ON instance_content_provider_refs(provider, version_id);
            CREATE UNIQUE INDEX instance_content_provider_refs_primary
                ON instance_content_provider_refs(content_entry_id)
                WHERE primary_ref = 1;

            INSERT INTO instances (id) VALUES ('instance');
            INSERT INTO instance_content_sets (id) VALUES ('set');
            INSERT INTO instance_files (id, relative_path) VALUES
                ('cf-file', 'mods/cf.jar'),
                ('cf-proven-file', 'mods/cf-proven.jar'),
                ('mr-file', 'mods/mr.jar'),
                ('broken-file', 'mods/broken.jar');
            INSERT INTO instance_content_entries (
                id, instance_id, content_set_id, file_id, project_type,
                project_id, version_id, source_kind, server_requirement,
                client_requirement, enabled, added_at, modified_at
            ) VALUES
                ('cf', 'instance', 'set', 'cf-file', 'mod', '42', '7',
                    'curseforge', 'required', 'required', 1, 1, 1),
                ('cf-proven', 'instance', 'set', 'cf-proven-file', 'mod',
                    '99', '11', 'curseforge', 'required', 'required', 1, 1, 1),
                ('mr', 'instance', 'set', 'mr-file', 'mod', 'mr-project',
                    'mr-version', 'local', 'required', 'required', 1, 1, 1),
                ('broken', 'instance', 'set', 'broken-file', 'mod',
                    'not-a-number', 'also-invalid', 'curseforge', 'required',
                    'required', 1, 1, 1);
            INSERT INTO instance_content_provider_refs (
                content_entry_id, provider, project_id, version_id, primary_ref
            ) VALUES
                ('cf', 'modrinth', '42', '7', 1),
                ('cf-proven', 'modrinth', '99', '11', 1),
                ('mr', 'modrinth', 'mr-project', 'mr-version', 1),
                ('broken', 'modrinth', 'not-a-number', 'also-invalid', 1);
            INSERT INTO instance_content_update_checks (
                content_entry_id, update_channel, update_version_id, checked_at
            ) VALUES
                ('cf', 'release', '8', 1),
                ('mr', 'release', 'mr-update', 1);
            INSERT INTO cache (data_type, data) VALUES (
                'file_hash',
                '{"path":"instance/mods/cf-proven.jar","project_id":"verified-mr-project","version_id":"verified-mr-version"}'
            );
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::raw_sql(provider_qualified_content_migration().sql.as_ref())
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(
            reconcile_provider_qualified_content_migration()
                .sql
                .as_ref(),
        )
        .execute(&pool)
        .await
        .unwrap();

        let refs: Vec<(String, String, String, Option<String>, i64)> =
            sqlx::query_as(
                "SELECT content_entry_id, provider, provider_project_id,
                        provider_release_id, is_origin
                 FROM instance_content_provider_refs
                 ORDER BY content_entry_id, provider, provider_project_id",
            )
            .fetch_all(&pool)
            .await
            .unwrap();
        assert_eq!(
            refs,
            vec![
                (
                    "cf".to_string(),
                    "curseforge".to_string(),
                    "42".to_string(),
                    Some("7".to_string()),
                    1,
                ),
                (
                    "cf-proven".to_string(),
                    "curseforge".to_string(),
                    "99".to_string(),
                    Some("11".to_string()),
                    1,
                ),
                (
                    "cf-proven".to_string(),
                    "modrinth".to_string(),
                    "verified-mr-project".to_string(),
                    Some("verified-mr-version".to_string()),
                    0,
                ),
                (
                    "mr".to_string(),
                    "modrinth".to_string(),
                    "mr-project".to_string(),
                    Some("mr-version".to_string()),
                    1,
                ),
            ]
        );

        let update_checks: Vec<(String, String, String, String)> =
            sqlx::query_as(
                "SELECT content_entry_id, provider, provider_project_id,
                        provider_release_id
                 FROM instance_content_update_checks",
            )
            .fetch_all(&pool)
            .await
            .unwrap();
        assert_eq!(
            update_checks,
            vec![(
                "mr".to_string(),
                "modrinth".to_string(),
                "mr-project".to_string(),
                "mr-update".to_string(),
            )]
        );

        let legacy_tables: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master
             WHERE type = 'table' AND name LIKE '%_legacy'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(legacy_tables, 0);

        let foreign_key_errors: Vec<(String, i64, String, i64)> =
            sqlx::query_as("PRAGMA foreign_key_check")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert!(foreign_key_errors.is_empty());
    }

    #[tokio::test]
    async fn reconciles_applied_single_provider_schema_before_forward_migration()
     {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::raw_sql(
            "
            CREATE TABLE _sqlx_migrations (
                version BIGINT PRIMARY KEY,
                description TEXT NOT NULL,
                installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                success BOOLEAN NOT NULL,
                checksum BLOB NOT NULL,
                execution_time BIGINT NOT NULL
            );
            CREATE TABLE instance_content_entries (
                id TEXT PRIMARY KEY,
                instance_id TEXT NOT NULL,
                file_id TEXT NULL,
                source_kind TEXT NOT NULL
            );
            CREATE TABLE instance_files (
                id TEXT PRIMARY KEY,
                relative_path TEXT NOT NULL
            );
            CREATE TABLE cache (
                data_type TEXT NOT NULL,
                data TEXT NOT NULL
            );
            CREATE TABLE instance_content_provider_refs (
                content_entry_id TEXT NOT NULL,
                provider TEXT NOT NULL,
                provider_project_id TEXT NOT NULL,
                provider_release_id TEXT NULL,
                is_origin INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (content_entry_id, provider)
            );
            CREATE INDEX instance_content_provider_refs_project
                ON instance_content_provider_refs(
                    provider,
                    provider_project_id
                );
            CREATE INDEX instance_content_provider_refs_release
                ON instance_content_provider_refs(
                    provider,
                    provider_release_id
                );
            CREATE UNIQUE INDEX instance_content_provider_refs_origin
                ON instance_content_provider_refs(content_entry_id)
                WHERE is_origin = 1;
            CREATE TABLE instance_content_update_checks (
                content_entry_id TEXT PRIMARY KEY,
                provider TEXT NULL,
                provider_project_id TEXT NULL,
                provider_release_id TEXT NULL
            );
            INSERT INTO instance_content_entries (
                id, instance_id, file_id, source_kind
            ) VALUES ('cf', 'instance', 'file', 'curseforge');
            INSERT INTO instance_files (id, relative_path)
            VALUES ('file', 'mods/cf.jar');
            INSERT INTO instance_content_provider_refs (
                content_entry_id,
                provider,
                provider_project_id,
                provider_release_id,
                is_origin
            ) VALUES ('cf', 'modrinth', '42', '7', 1);
            ",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO _sqlx_migrations (
                version, description, success, checksum, execution_time
             ) VALUES (?, 'provider qualified content', TRUE, ?, 0)",
        )
        .bind(PROVIDER_QUALIFIED_CONTENT_MIGRATION_VERSION)
        .bind(decode_hex(
            LEGACY_PROVIDER_QUALIFIED_CONTENT_MIGRATION_CHECKSUMS[0],
        ))
        .execute(&pool)
        .await
        .unwrap();

        reconcile_legacy_provider_qualified_content_migration(&pool)
            .await
            .unwrap();

        let applied_checksum: Vec<u8> = sqlx::query_scalar(
            "SELECT checksum FROM _sqlx_migrations WHERE version = ?",
        )
        .bind(PROVIDER_QUALIFIED_CONTENT_MIGRATION_VERSION)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            applied_checksum,
            provider_qualified_content_migration().checksum.as_ref()
        );

        sqlx::raw_sql(
            reconcile_provider_qualified_content_migration()
                .sql
                .as_ref(),
        )
        .execute(&pool)
        .await
        .unwrap();

        let refs: Vec<(String, String, String, Option<String>, i64)> =
            sqlx::query_as(
                "SELECT content_entry_id, provider, provider_project_id,
                        provider_release_id, is_origin
                 FROM instance_content_provider_refs",
            )
            .fetch_all(&pool)
            .await
            .unwrap();
        assert_eq!(
            refs,
            vec![(
                "cf".to_string(),
                "curseforge".to_string(),
                "42".to_string(),
                Some("7".to_string()),
                1,
            )]
        );
        let has_id: bool = sqlx::query_scalar(
            "SELECT EXISTS(
                SELECT 1
                FROM pragma_table_info('instance_content_provider_refs')
                WHERE name = 'id' AND pk = 1
            )",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(has_id);
    }

    #[tokio::test]
    async fn reconciles_colliding_home_dashboard_migration() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query(
            "
            CREATE TABLE _sqlx_migrations (
                version BIGINT PRIMARY KEY,
                description TEXT NOT NULL,
                installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                success BOOLEAN NOT NULL,
                checksum BLOB NOT NULL,
                execution_time BIGINT NOT NULL
            );
            CREATE TABLE settings (id INTEGER PRIMARY KEY);
            CREATE TABLE instances (id TEXT PRIMARY KEY, pinned_at INTEGER NULL);
            CREATE TABLE instance_daily_playtime (
                played_on TEXT NOT NULL,
                instance_id TEXT NOT NULL,
                instance_name TEXT NOT NULL,
                played_seconds INTEGER NOT NULL DEFAULT 0,
                session_count INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (played_on, instance_id)
            );
            CREATE INDEX instance_daily_playtime_played_on
                ON instance_daily_playtime(played_on);
            ",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time) VALUES (?, 'home-dashboard', TRUE, ?, 0)",
        )
        .bind(COLLIDING_HOME_DASHBOARD_MIGRATION_VERSION)
        .bind(decode_hex(
            COLLIDING_HOME_DASHBOARD_MIGRATION_CHECKSUMS[0],
        ))
        .execute(&pool)
        .await
        .unwrap();

        reconcile_existing_home_dashboard_migration(&pool)
            .await
            .unwrap();
        reconcile_existing_home_dashboard_migration(&pool)
            .await
            .unwrap();

        let blur_column_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info('settings') WHERE name = 'transparent_background_blur')",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(blur_column_exists);

        for version in [
            COLLIDING_HOME_DASHBOARD_MIGRATION_VERSION,
            HOME_DASHBOARD_MIGRATION_VERSION,
        ] {
            let migration = MIGRATOR
                .iter()
                .find(|migration| migration.version == version)
                .unwrap();
            let (description, checksum): (String, Vec<u8>) = sqlx::query_as(
                "SELECT description, checksum FROM _sqlx_migrations WHERE version = ?",
            )
            .bind(version)
            .fetch_one(&pool)
            .await
            .unwrap();
            assert_eq!(description, migration.description.as_ref());
            assert_eq!(checksum, migration.checksum.as_ref());
        }
    }

    #[tokio::test]
    async fn does_not_claim_incomplete_home_dashboard_schema() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query(
            "
            CREATE TABLE _sqlx_migrations (
                version BIGINT PRIMARY KEY,
                description TEXT NOT NULL,
                installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                success BOOLEAN NOT NULL,
                checksum BLOB NOT NULL,
                execution_time BIGINT NOT NULL
            );
            CREATE TABLE settings (id INTEGER PRIMARY KEY);
            CREATE TABLE instances (id TEXT PRIMARY KEY, pinned_at INTEGER NULL);
            ",
        )
        .execute(&pool)
        .await
        .unwrap();
        let legacy_checksum =
            decode_hex(COLLIDING_HOME_DASHBOARD_MIGRATION_CHECKSUMS[0]);
        sqlx::query(
            "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time) VALUES (?, 'home-dashboard', TRUE, ?, 0)",
        )
        .bind(COLLIDING_HOME_DASHBOARD_MIGRATION_VERSION)
        .bind(&legacy_checksum)
        .execute(&pool)
        .await
        .unwrap();

        reconcile_existing_home_dashboard_migration(&pool)
            .await
            .unwrap();

        let canonical_applied: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM _sqlx_migrations WHERE version = ?)",
        )
        .bind(HOME_DASHBOARD_MIGRATION_VERSION)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(!canonical_applied);
        let applied_checksum: Vec<u8> = sqlx::query_scalar(
            "SELECT checksum FROM _sqlx_migrations WHERE version = ?",
        )
        .bind(COLLIDING_HOME_DASHBOARD_MIGRATION_VERSION)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(applied_checksum, legacy_checksum);
    }

    #[tokio::test]
    async fn repairs_schema_from_colliding_java_discovery_migration() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE settings (onboarded INTEGER NOT NULL DEFAULT 0)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO settings (onboarded) VALUES (0), (1)")
            .execute(&pool)
            .await
            .unwrap();

        reconcile_colliding_java_discovery_migration(&pool)
            .await
            .unwrap();

        let values: Vec<(i64, i64)> = sqlx::query_as(
            "SELECT onboarding_version, onboarding_instance_tour_completed FROM settings ORDER BY onboarded",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(values, vec![(0, 0), (0, 1)]);
    }

    #[tokio::test]
    async fn claims_existing_java_table_for_canonical_migration() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query(
            "
            CREATE TABLE _sqlx_migrations (
                version BIGINT PRIMARY KEY,
                description TEXT NOT NULL,
                installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                success BOOLEAN NOT NULL,
                checksum BLOB NOT NULL,
                execution_time BIGINT NOT NULL
            )
            ",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "
            CREATE TABLE discovered_javas (
                path TEXT NOT NULL PRIMARY KEY,
                major_version INTEGER NOT NULL,
                full_version TEXT NOT NULL,
                architecture TEXT NOT NULL,
                file_size INTEGER NOT NULL,
                file_mtime_ms INTEGER NOT NULL
            )
            ",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time) VALUES (?, 'temporary', TRUE, ?, 0)",
        )
        .bind(TEMPORARY_JAVA_DISCOVERY_MIGRATION_VERSION)
        .bind(decode_hex(
            TEMPORARY_JAVA_DISCOVERY_MIGRATION_CHECKSUMS[0],
        ))
        .execute(&pool)
        .await
        .unwrap();

        reconcile_existing_java_discovery_migration(&pool)
            .await
            .unwrap();

        let migration = MIGRATOR
            .iter()
            .find(|migration| {
                migration.version == JAVA_DISCOVERY_MIGRATION_VERSION
            })
            .unwrap();
        let canonical_checksum: Vec<u8> = sqlx::query_scalar(
            "SELECT checksum FROM _sqlx_migrations WHERE version = ?",
        )
        .bind(JAVA_DISCOVERY_MIGRATION_VERSION)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(canonical_checksum, migration.checksum.as_ref());
        let temporary_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM _sqlx_migrations WHERE version = ?)",
        )
        .bind(TEMPORARY_JAVA_DISCOVERY_MIGRATION_VERSION)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(!temporary_exists);
        let index_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'index' AND name = 'discovered_javas_major_version')",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(index_exists);
    }

    #[tokio::test]
    async fn does_not_claim_incompatible_java_table() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query(
            "
            CREATE TABLE _sqlx_migrations (
                version BIGINT PRIMARY KEY,
                description TEXT NOT NULL,
                installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                success BOOLEAN NOT NULL,
                checksum BLOB NOT NULL,
                execution_time BIGINT NOT NULL
            )
            ",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("CREATE TABLE discovered_javas (path TEXT PRIMARY KEY)")
            .execute(&pool)
            .await
            .unwrap();

        reconcile_existing_java_discovery_migration(&pool)
            .await
            .unwrap();

        let canonical_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM _sqlx_migrations WHERE version = ?)",
        )
        .bind(JAVA_DISCOVERY_MIGRATION_VERSION)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(!canonical_exists);
    }

    #[tokio::test]
    async fn upgrades_previous_database_with_nullable_home_widgets() {
        const HOME_WIDGETS_MIGRATION_VERSION: i64 = 20260804120000;

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .unwrap();
        let previous_migrator = Migrator {
            migrations: std::borrow::Cow::Owned(
                MIGRATOR
                    .iter()
                    .filter(|migration| {
                        migration.version < HOME_WIDGETS_MIGRATION_VERSION
                    })
                    .cloned()
                    .collect(),
            ),
            ..Migrator::DEFAULT
        };
        previous_migrator.run(&pool).await.unwrap();
        sqlx::query(
            "UPDATE settings
             SET locale = 'zh-CN', home_layout = 'minimal'
             WHERE id = 0",
        )
        .execute(&pool)
        .await
        .unwrap();

        MIGRATOR.run(&pool).await.unwrap();

        let upgraded: (String, String, Option<String>) = sqlx::query_as(
            "SELECT locale, home_layout, json(home_widgets)
             FROM settings WHERE id = 0",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(upgraded, ("zh-CN".into(), "minimal".into(), None));
        let foreign_key_errors: Vec<(String, i64, String, i64)> =
            sqlx::query_as("PRAGMA foreign_key_check")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert!(foreign_key_errors.is_empty());
    }

    #[tokio::test]
    async fn creates_ai_provider_schema_for_a_fresh_database() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .unwrap();

        MIGRATOR.run(&pool).await.unwrap();

        let enabled: i64 =
            sqlx::query_scalar("SELECT enabled FROM ai_settings WHERE id = 0")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(enabled, 1);
        let legacy_credential_cleanup: i64 = sqlx::query_scalar(
            "SELECT legacy_openai_credential_cleanup FROM ai_settings WHERE id = 0",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(legacy_credential_cleanup, 0);
        let ai_tables: Vec<String> = sqlx::query_scalar(
            "SELECT name FROM sqlite_master
             WHERE type = 'table' AND name LIKE 'ai_%'
             ORDER BY name",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(
            ai_tables,
            vec![
                "ai_provider_configs".to_string(),
                "ai_provider_models".to_string(),
                "ai_settings".to_string(),
            ]
        );
        let translation_ai_columns: Vec<String> = sqlx::query_scalar(
            "SELECT name FROM pragma_table_info('translation_settings')
             WHERE name IN ('ai_provider_id', 'ai_model_id') ORDER BY name",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(
            translation_ai_columns,
            vec!["ai_model_id".to_string(), "ai_provider_id".to_string()]
        );
        let foreign_key_errors: Vec<(String, i64, String, i64)> =
            sqlx::query_as("PRAGMA foreign_key_check")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert!(foreign_key_errors.is_empty());
    }

    #[tokio::test]
    async fn discards_legacy_openai_translation_configuration() {
        const AI_PROVIDER_MIGRATION_VERSION: i64 = 20260805120000;

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .unwrap();
        let previous_migrator = Migrator {
            migrations: std::borrow::Cow::Owned(
                MIGRATOR
                    .iter()
                    .filter(|migration| {
                        migration.version < AI_PROVIDER_MIGRATION_VERSION
                    })
                    .cloned()
                    .collect(),
            ),
            ..Migrator::DEFAULT
        };
        previous_migrator.run(&pool).await.unwrap();
        sqlx::query(
            "UPDATE translation_settings
             SET provider = 'openai-compatible',
                 target_language = 'zh-CN',
                 openai_base_url = 'https://gateway.example.test/v1',
                 openai_model = 'example-chat-model',
                 openai_api_key = 'legacy-secret',
                 openai_system_prompt = 'Preserve terminology.'
             WHERE id = 0",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO translation_cache (key, translation, created_at)
             VALUES ('legacy-cache', 'cached translation', 123)",
        )
        .execute(&pool)
        .await
        .unwrap();

        MIGRATOR.run(&pool).await.unwrap();

        let translation: (String, String, String, String, Option<String>) =
            sqlx::query_as(
                "SELECT provider, ai_provider_id, ai_model_id,
                        openai_system_prompt, openai_api_key
                 FROM translation_settings WHERE id = 0",
            )
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            translation,
            (
                "google".to_string(),
                String::new(),
                String::new(),
                String::new(),
                None,
            )
        );
        let provider_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM ai_provider_configs")
                .fetch_one(&pool)
                .await
                .unwrap();
        let model_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM ai_provider_models")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!((provider_count, model_count), (0, 0));
        let legacy_credential_cleanup: i64 = sqlx::query_scalar(
            "SELECT legacy_openai_credential_cleanup FROM ai_settings WHERE id = 0",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(legacy_credential_cleanup, 1);
        let cached: (String, i64) = sqlx::query_as(
            "SELECT translation, created_at FROM translation_cache
             WHERE key = 'legacy-cache'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(cached, ("cached translation".to_string(), 123));
        let legacy_objects: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master
             WHERE (type = 'table' OR type = 'index')
               AND name = 'legacy_openai_ai_config'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(legacy_objects, 0);
        let foreign_key_errors: Vec<(String, i64, String, i64)> =
            sqlx::query_as("PRAGMA foreign_key_check")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert!(foreign_key_errors.is_empty());
    }

    #[tokio::test]
    async fn ignores_malformed_legacy_ai_provider_configuration() {
        const AI_PROVIDER_MIGRATION_VERSION: i64 = 20260805120000;

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .unwrap();
        let previous_migrator = Migrator {
            migrations: std::borrow::Cow::Owned(
                MIGRATOR
                    .iter()
                    .filter(|migration| {
                        migration.version < AI_PROVIDER_MIGRATION_VERSION
                    })
                    .cloned()
                    .collect(),
            ),
            ..Migrator::DEFAULT
        };
        previous_migrator.run(&pool).await.unwrap();
        sqlx::query(
            "UPDATE translation_settings
             SET provider = 'openai-compatible',
                 openai_base_url = 'not a URL',
                 openai_model = '',
                 openai_api_key = NULL
             WHERE id = 0",
        )
        .execute(&pool)
        .await
        .unwrap();

        MIGRATOR.run(&pool).await.unwrap();

        let translation: (
            String,
            String,
            String,
            String,
            String,
            Option<String>,
            String,
        ) = sqlx::query_as(
            "SELECT provider, ai_provider_id, ai_model_id, openai_base_url,
                    openai_model, openai_api_key, openai_system_prompt
             FROM translation_settings WHERE id = 0",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            translation,
            (
                "google".to_string(),
                String::new(),
                String::new(),
                "https://api.openai.com/v1".to_string(),
                "gpt-4o-mini".to_string(),
                None,
                String::new(),
            )
        );
        let provider_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM ai_provider_configs")
                .fetch_one(&pool)
                .await
                .unwrap();
        let model_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM ai_provider_models")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!((provider_count, model_count), (0, 0));
        let foreign_key_errors: Vec<(String, i64, String, i64)> =
            sqlx::query_as("PRAGMA foreign_key_check")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert!(foreign_key_errors.is_empty());
    }

    #[tokio::test]
    async fn preserves_ai_configuration_created_after_provider_migration() {
        const DISCARD_LEGACY_OPENAI_MIGRATION_VERSION: i64 = 20260805130000;

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .unwrap();
        let provider_migrator = Migrator {
            migrations: std::borrow::Cow::Owned(
                MIGRATOR
                    .iter()
                    .filter(|migration| {
                        migration.version
                            < DISCARD_LEGACY_OPENAI_MIGRATION_VERSION
                    })
                    .cloned()
                    .collect(),
            ),
            ..Migrator::DEFAULT
        };
        provider_migrator.run(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO ai_provider_configs
             (provider_id, custom_name, protocol, enabled, endpoint, settings)
             VALUES ('openai', NULL, 'openai', TRUE,
                     'https://api.openai.com/v1', '{}')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO ai_provider_models
             (provider_id, model_id, display_name, enabled, source)
             VALUES ('openai', 'gpt-5-mini', 'GPT-5 mini', TRUE, 'builtin')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "UPDATE translation_settings
             SET provider = 'ai', ai_provider_id = 'openai',
                 ai_model_id = 'gpt-5-mini',
                 openai_system_prompt = 'Current AI prompt.'
             WHERE id = 0",
        )
        .execute(&pool)
        .await
        .unwrap();

        MIGRATOR.run(&pool).await.unwrap();

        let provider: (String, String, i64) = sqlx::query_as(
            "SELECT provider_id, endpoint, enabled
             FROM ai_provider_configs WHERE provider_id = 'openai'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            provider,
            (
                "openai".to_string(),
                "https://api.openai.com/v1".to_string(),
                1,
            )
        );
        let model: (String, String, String) = sqlx::query_as(
            "SELECT provider_id, model_id, source
             FROM ai_provider_models WHERE provider_id = 'openai'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            model,
            (
                "openai".to_string(),
                "gpt-5-mini".to_string(),
                "builtin".to_string(),
            )
        );
        let translation: (String, String, String, String) = sqlx::query_as(
            "SELECT provider, ai_provider_id, ai_model_id,
                    openai_system_prompt
             FROM translation_settings WHERE id = 0",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            translation,
            (
                "ai".to_string(),
                "openai".to_string(),
                "gpt-5-mini".to_string(),
                "Current AI prompt.".to_string(),
            )
        );
        let legacy_credential_cleanup: i64 = sqlx::query_scalar(
            "SELECT legacy_openai_credential_cleanup FROM ai_settings WHERE id = 0",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(legacy_credential_cleanup, 0);
        let foreign_key_errors: Vec<(String, i64, String, i64)> =
            sqlx::query_as("PRAGMA foreign_key_check")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert!(foreign_key_errors.is_empty());
    }

    #[tokio::test]
    async fn content_dependency_edges_migration_creates_fresh_schema() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .unwrap();
        MIGRATOR.run(&pool).await.unwrap();

        let has_auto_dependency: bool = sqlx::query_scalar(
            "SELECT EXISTS(
                SELECT 1 FROM pragma_table_info('instance_content_entries')
                WHERE name = 'auto_dependency'
            )",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(has_auto_dependency);

        let has_edges: bool = sqlx::query_scalar(
            "SELECT EXISTS(
                SELECT 1 FROM sqlite_master
                WHERE type = 'table'
                    AND name = 'instance_content_dependencies'
            )",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(has_edges);

        let foreign_key_errors: Vec<(String, i64, String, i64)> =
            sqlx::query_as("PRAGMA foreign_key_check")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert!(foreign_key_errors.is_empty());
    }

    #[tokio::test]
    async fn content_dependency_edges_migration_upgrades_previous_schema() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .unwrap();
        let previous_migrator = Migrator {
            migrations: std::borrow::Cow::Owned(
                MIGRATOR
                    .iter()
                    .filter(|migration| {
                        migration.version
                            < CONTENT_DEPENDENCY_EDGES_MIGRATION_VERSION
                    })
                    .cloned()
                    .collect(),
            ),
            ..Migrator::DEFAULT
        };
        previous_migrator.run(&pool).await.unwrap();
        let had_edges: bool = sqlx::query_scalar(
            "SELECT EXISTS(
                SELECT 1 FROM sqlite_master
                WHERE type = 'table'
                    AND name = 'instance_content_dependencies'
            )",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(!had_edges);

        MIGRATOR.run(&pool).await.unwrap();
        let has_edges: bool = sqlx::query_scalar(
            "SELECT EXISTS(
                SELECT 1 FROM sqlite_master
                WHERE type = 'table'
                    AND name = 'instance_content_dependencies'
            )",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(has_edges);

        let foreign_key_errors: Vec<(String, i64, String, i64)> =
            sqlx::query_as("PRAGMA foreign_key_check")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert!(foreign_key_errors.is_empty());
    }

    async fn insert_dependency_edge_fixture(pool: &sqlx::SqlitePool) {
        sqlx::raw_sql(
            r#"
            INSERT INTO instances (
                id, path, applied_content_set_id, install_stage,
                launcher_feature_version, name, created, modified
            ) VALUES
                ('local-edge-instance', 'local-edge-instance', NULL,
                    'installed', '1', 'Local Edge', 1, 1);

            INSERT INTO instance_content_sets (
                id, instance_id, name, source_kind, status, game_version,
                loader, created, modified
            ) VALUES
                ('local-edge-set', 'local-edge-instance', 'Default',
                    'local', 'applied', '1.20.1', 'fabric', 1, 1);

            UPDATE instances SET applied_content_set_id = 'local-edge-set';

            INSERT INTO instance_content_entries (
                id, instance_id, content_set_id, file_id, project_type,
                source_kind, server_requirement, client_requirement,
                enabled, added_at, modified_at
            ) VALUES
                ('parent-entry', 'local-edge-instance', 'local-edge-set',
                    NULL, 'mod', 'local', 'required', 'required', 1, 1, 1),
                ('child-entry', 'local-edge-instance', 'local-edge-set',
                    NULL, 'mod', 'local', 'required', 'required', 1, 1, 1),
                ('child-entry-local', 'local-edge-instance',
                    'local-edge-set', NULL, 'mod', 'local', 'required',
                    'required', 1, 1, 1);
            "#,
        )
        .execute(pool)
        .await
        .unwrap();
    }

    fn dependency_edge_row_legacy(
        provider: &str,
        child_entry_id: &str,
        child_project_id: &str,
    ) -> String {
        format!(
            "INSERT INTO instance_content_dependencies (
                id, content_set_id, parent_entry_id, child_entry_id,
                provider, dependency_kind, parent_project_id,
                parent_release_id, child_project_id, child_release_id,
                created_at, modified_at
            ) VALUES
                ('edge-{provider}', 'local-edge-set', 'parent-entry',
                    '{child_entry_id}', '{provider}', 'required',
                    'local:parent', '1.0.0', '{child_project_id}', '2.0.0',
                    1, 1)"
        )
    }

    fn dependency_edge_row(
        evidence_provider: &str,
        parent_provider: &str,
        child_provider: &str,
        child_entry_id: &str,
        child_project_id: &str,
    ) -> String {
        format!(
            "INSERT INTO instance_content_dependencies (
                id, content_set_id, parent_entry_id, child_entry_id,
                evidence_provider, parent_provider, child_provider,
                dependency_kind, parent_project_id, parent_release_id,
                child_project_id, child_release_id, created_at, modified_at
            ) VALUES
                ('edge-{evidence_provider}-{child_entry_id}',
                    'local-edge-set', 'parent-entry', '{child_entry_id}',
                    '{evidence_provider}', '{parent_provider}',
                    '{child_provider}', 'required', 'local:parent', '1.0.0',
                    '{child_project_id}', '2.0.0', 1, 1)"
        )
    }

    #[tokio::test]
    async fn content_dependency_local_provider_migration_accepts_local_rows_fresh()
     {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .unwrap();
        MIGRATOR.run(&pool).await.unwrap();
        insert_dependency_edge_fixture(&pool).await;

        sqlx::raw_sql(&dependency_edge_row(
            "local",
            "local",
            "local",
            "child-entry-local",
            "local:child-local",
        ))
        .execute(&pool)
        .await
        .unwrap();

        let providers: Vec<String> = sqlx::query_scalar(
            "SELECT evidence_provider FROM instance_content_dependencies",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(providers, ["local"]);

        let foreign_key_errors: Vec<(String, i64, String, i64)> =
            sqlx::query_as("PRAGMA foreign_key_check")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert!(foreign_key_errors.is_empty());
    }

    #[tokio::test]
    async fn content_dependency_local_provider_migration_upgrades_preserving_rows()
     {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .unwrap();
        let previous_migrator = Migrator {
            migrations: std::borrow::Cow::Owned(
                MIGRATOR
                    .iter()
                    .filter(|migration| {
                        migration.version
                            < CONTENT_DEPENDENCY_LOCAL_PROVIDER_MIGRATION_VERSION
                    })
                    .cloned()
                    .collect(),
            ),
            ..Migrator::DEFAULT
        };
        previous_migrator.run(&pool).await.unwrap();
        insert_dependency_edge_fixture(&pool).await;
        sqlx::raw_sql(&dependency_edge_row_legacy(
            "modrinth",
            "child-entry",
            "local:child",
        ))
        .execute(&pool)
        .await
        .unwrap();

        let local_row = dependency_edge_row_legacy(
            "local",
            "child-entry-local",
            "local:child-local",
        )
        .replace("edge-local", "edge-local-before");
        let local_before: Result<sqlx::sqlite::SqliteQueryResult, _> =
            sqlx::raw_sql(&local_row).execute(&pool).await;
        assert!(local_before.is_err());

        MIGRATOR.run(&pool).await.unwrap();

        let providers: Vec<String> = sqlx::query_scalar(
            "SELECT evidence_provider FROM instance_content_dependencies
             ORDER BY id",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(providers, ["modrinth"]);

        sqlx::raw_sql(&dependency_edge_row(
            "local",
            "local",
            "local",
            "child-entry-local",
            "local:child-local",
        ))
        .execute(&pool)
        .await
        .unwrap();

        let providers: Vec<String> = sqlx::query_scalar(
            "SELECT evidence_provider FROM instance_content_dependencies
             ORDER BY id",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(providers, ["local", "modrinth"]);

        let foreign_key_errors: Vec<(String, i64, String, i64)> =
            sqlx::query_as("PRAGMA foreign_key_check")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert!(foreign_key_errors.is_empty());
    }

    #[tokio::test]
    async fn content_dependency_endpoint_providers_migration_creates_provider_qualified_schema()
     {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .unwrap();
        MIGRATOR.run(&pool).await.unwrap();
        insert_dependency_edge_fixture(&pool).await;

        sqlx::raw_sql(&dependency_edge_row(
            "modrinth",
            "curseforge",
            "modrinth",
            "child-entry",
            "mr-child",
        ))
        .execute(&pool)
        .await
        .unwrap();

        let providers: (String, String, String) = sqlx::query_as(
            "SELECT evidence_provider, parent_provider, child_provider
             FROM instance_content_dependencies",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            providers,
            ("modrinth".into(), "curseforge".into(), "modrinth".into())
        );
        let foreign_key_errors: Vec<(String, i64, String, i64)> =
            sqlx::query_as("PRAGMA foreign_key_check")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert!(foreign_key_errors.is_empty());
    }

    #[tokio::test]
    async fn content_dependency_endpoint_providers_migration_backfills_legacy_rows()
     {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .unwrap();
        let previous_migrator = Migrator {
            migrations: std::borrow::Cow::Owned(
                MIGRATOR
                    .iter()
                    .filter(|migration| {
                        migration.version
                            < CONTENT_DEPENDENCY_ENDPOINT_PROVIDERS_MIGRATION_VERSION
                    })
                    .cloned()
                    .collect(),
            ),
            ..Migrator::DEFAULT
        };
        previous_migrator.run(&pool).await.unwrap();
        insert_dependency_edge_fixture(&pool).await;
        sqlx::raw_sql(&dependency_edge_row_legacy(
            "curseforge",
            "child-entry",
            "cf-child",
        ))
        .execute(&pool)
        .await
        .unwrap();

        MIGRATOR.run(&pool).await.unwrap();

        let legacy: (String, String, String) = sqlx::query_as(
            "SELECT evidence_provider, parent_provider, child_provider
             FROM instance_content_dependencies WHERE id = 'edge-curseforge'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            legacy,
            (
                "curseforge".into(),
                "curseforge".into(),
                "curseforge".into()
            )
        );

        sqlx::raw_sql(&dependency_edge_row(
            "modrinth",
            "curseforge",
            "modrinth",
            "child-entry-local",
            "mr-child",
        ))
        .execute(&pool)
        .await
        .unwrap();
        let cross_source: (String, String, String) = sqlx::query_as(
            "SELECT evidence_provider, parent_provider, child_provider
             FROM instance_content_dependencies
             WHERE id = 'edge-modrinth-child-entry-local'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            cross_source,
            ("modrinth".into(), "curseforge".into(), "modrinth".into())
        );
        let foreign_key_errors: Vec<(String, i64, String, i64)> =
            sqlx::query_as("PRAGMA foreign_key_check")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert!(foreign_key_errors.is_empty());
    }

    #[tokio::test]
    async fn content_dependency_backfill_marker_migration_upgrades_preserving_rows()
     {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .unwrap();
        let previous_migrator = Migrator {
            migrations: std::borrow::Cow::Owned(
                MIGRATOR
                    .iter()
                    .filter(|migration| {
                        migration.version
                            < CONTENT_DEPENDENCY_BACKFILL_MARKER_MIGRATION_VERSION
                    })
                    .cloned()
                    .collect(),
            ),
            ..Migrator::DEFAULT
        };
        previous_migrator.run(&pool).await.unwrap();
        insert_dependency_edge_fixture(&pool).await;

        MIGRATOR.run(&pool).await.unwrap();

        let markers: Vec<(String, i64)> = sqlx::query_as(
            "SELECT id, dependency_backfilled
             FROM instance_content_entries ORDER BY id",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(
            markers,
            [
                ("child-entry".to_string(), 0),
                ("child-entry-local".to_string(), 0),
                ("parent-entry".to_string(), 0),
            ]
        );

        sqlx::query(
            "UPDATE instance_content_entries
             SET dependency_backfilled = 1 WHERE id = 'parent-entry'",
        )
        .execute(&pool)
        .await
        .unwrap();

        let foreign_key_errors: Vec<(String, i64, String, i64)> =
            sqlx::query_as("PRAGMA foreign_key_check")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert!(foreign_key_errors.is_empty());
    }
}

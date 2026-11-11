use super::*;
use std::collections::HashSet;

#[tokio::test]
async fn backup_directory_failure_does_not_prevent_connection_or_hide_migration_errors()
 {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("release").join("app.db");
    tokio::fs::create_dir_all(database.parent().unwrap())
        .await
        .unwrap();
    let pool = connect_app_db(&database).await.unwrap();
    sqlx::query("UPDATE app_metadata SET value = 'previous-software-version' WHERE key = 'app_version'")
            .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO app_metadata(key, value) VALUES ('backup-failure-marker', 'preserved')")
            .execute(&pool).await.unwrap();
    pool.close().await;
    tokio::fs::write(directory.path().join("Backups"), "not a directory")
        .await
        .unwrap();

    let pool = connect_app_db(&database).await.unwrap();
    let marker: String = sqlx::query_scalar(
        "SELECT value FROM app_metadata WHERE key = 'backup-failure-marker'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(marker, "preserved");
    sqlx::query("UPDATE app_metadata SET value = 'previous-software-version' WHERE key = 'app_version'")
            .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO _sqlx_migrations(version, description, success, checksum, execution_time) VALUES (99999999999999, 'unavailable-build', TRUE, X'00', 1)")
            .execute(&pool).await.unwrap();
    pool.close().await;
    assert!(connect_app_db(&database).await.is_err());
}

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

const SCHEDULER_TAGS_MIGRATION_VERSION: i64 = 20261110120000;

/// An older build opening a database that has the scheduler collections
/// must refuse it as a downgrade, with the message the frontend expects.
#[tokio::test]
async fn an_older_build_refuses_the_scheduler_schema_as_a_downgrade() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    MIGRATOR.run(&pool).await.unwrap();

    let older = tempfile::tempdir().unwrap();
    let migrations =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    for entry in std::fs::read_dir(migrations).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|extension| extension.to_str())
            != Some("sql")
        {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let Some(version) = name
            .split_once('_')
            .and_then(|(version, _)| version.parse::<i64>().ok())
        else {
            panic!("migration {name} does not start with a numeric version");
        };
        if version < SCHEDULER_TAGS_MIGRATION_VERSION {
            std::fs::copy(&path, older.path().join(&name)).unwrap();
        }
    }
    let older_build = Migrator::new(older.path()).await.unwrap();

    let error: crate::Error = older_build.run(&pool).await.unwrap_err().into();
    assert!(matches!(
        error.raw.as_ref(),
        crate::ErrorKind::SqlxMigrate(
            sqlx::migrate::MigrateError::VersionMissing(
                SCHEDULER_TAGS_MIGRATION_VERSION
            )
        )
    ));
    let message = error.user_facing_message();
    assert!(message.contains("was created by a newer Axolotl build"));
    assert!(message.contains("20261110120000"));
    pool.close().await;
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

    let rows: Vec<(String, String, Option<String>, String)> = sqlx::query_as(
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
    std::fs::write(source_dir.join(format!("{LEGACY_APP_DB_FILE}-wal")), "wal")
        .unwrap();

    reconcile_default_channel_database(settings_dir)
        .await
        .unwrap();

    let target_dir = settings_dir.join(channel);
    assert_eq!(
        std::fs::read_to_string(target_dir.join(LEGACY_APP_DB_FILE)).unwrap(),
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
    std::fs::remove_file(settings_dir.join(UPDATE_CHANNEL_STATE_FILE)).unwrap();
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
    std::fs::write(target_dir.join(format!("{LEGACY_APP_DB_FILE}-wal")), "wal")
        .unwrap();

    reconcile_default_channel_database(settings_dir)
        .await
        .unwrap();

    assert_eq!(
        std::fs::read_to_string(target_dir.join(LEGACY_APP_DB_FILE)).unwrap(),
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
    assert_eq!(std::fs::read_to_string(&target_wal).unwrap(), "foreign wal");
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
        std::fs::read_to_string(target_dir.join(CHANNEL_RECONCILE_MARKER_FILE))
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
            migration.version == PROVIDER_QUALIFIED_CONTENT_MIGRATION_VERSION
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
        .find(|migration| migration.version == AI_PROVIDER_MIGRATION_VERSION)
        .expect("AI provider migration should be embedded")
}

fn reconcile_provider_qualified_content_migration() -> &'static Migration {
    MIGRATOR
        .iter()
        .find(|migration| {
            migration.version
                == RECONCILE_PROVIDER_QUALIFIED_CONTENT_MIGRATION_VERSION
        })
        .expect("provider content reconciliation migration should be embedded")
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
            migration.version == INSTANCE_CONTENT_OWNERSHIP_MIGRATION_VERSION
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
    let changed_checksum =
        checksum(format!("{}\n-- unknown change", migration.sql).as_bytes());

    assert!(!is_compatible_migration_checksum(
        migration.version,
        &changed_checksum,
        migration,
    ));
}

#[tokio::test]
async fn reconciles_official_preferred_migration_with_trailing_blank_line() {
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

    let proxy_username: String =
        sqlx::query_scalar("SELECT proxy_username FROM settings WHERE id = 0")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(proxy_username, "");

    let proxy_password: String =
        sqlx::query_scalar("SELECT proxy_password FROM settings WHERE id = 0")
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
                    migration.version < DISCARD_LEGACY_OPENAI_MIGRATION_VERSION
                })
                .cloned()
                .collect(),
        ),
        ..Migrator::DEFAULT
    };
    provider_migrator.run(&pool).await.unwrap();

    let migration = ai_provider_migration();
    let legacy_checksum = decode_hex(LEGACY_AI_PROVIDER_MIGRATION_CHECKSUMS[0]);
    sqlx::query("UPDATE _sqlx_migrations SET checksum = ? WHERE version = ?")
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
        .find(|migration| migration.version == PROXY_CONFIG_MIGRATION_VERSION)
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
async fn reconciles_fully_applied_content_favorites_without_migration_record() {
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

    // Without the record this run would create the table a second time
    // under `20260820200000`; the retirement migration then drops it.
    MIGRATOR.run(&pool).await.unwrap();
    let remaining: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master
             WHERE type = 'table' AND name = 'content_favorites'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(remaining, 0);
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
                    migration.version < SYSTEM_PROXY_SETTING_MIGRATION_VERSION
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

    let proxy_username: String =
        sqlx::query_scalar("SELECT proxy_username FROM settings WHERE id = 0")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(proxy_username, "");

    let proxy_password: String =
        sqlx::query_scalar("SELECT proxy_password FROM settings WHERE id = 0")
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
    let members: Vec<(String, Option<String>, i64, String)> = sqlx::query_as(
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

    sqlx::query("DELETE FROM java_versions WHERE path = '/java/temurin-21'")
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

    let update_checks: Vec<(String, String, String, String)> = sqlx::query_as(
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
async fn reconciles_applied_single_provider_schema_before_forward_migration() {
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
    sqlx::query("CREATE TABLE settings (onboarded INTEGER NOT NULL DEFAULT 0)")
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
        .find(|migration| migration.version == JAVA_DISCOVERY_MIGRATION_VERSION)
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
async fn moves_derived_records_out_of_the_app_database() {
    const MOVE_DERIVED_RECORDS_VERSION: i64 = 20261029120000;

    let upgrade = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let previous_migrator = Migrator {
        migrations: std::borrow::Cow::Owned(
            MIGRATOR
                .iter()
                .filter(|migration| {
                    migration.version < MOVE_DERIVED_RECORDS_VERSION
                })
                .cloned()
                .collect(),
        ),
        ..Migrator::DEFAULT
    };
    previous_migrator.run(&upgrade).await.unwrap();
    sqlx::raw_sql(
        "INSERT INTO crash_analysis_mod_snapshots
                (instance_id, filename, size, sha256)
             VALUES ('instance', 'example.jar', 1, 'digest');
             INSERT INTO discovered_javas
                (path, major_version, full_version, architecture, file_size,
                 file_mtime_ms)
             VALUES ('/jre/21', 21, '21.0.1', 'x86_64', 1, 1);
             INSERT INTO google_translate_ip_cache (ip, latency_ms, created_at)
             VALUES ('1.1.1.1', 10, 1);
             INSERT INTO telemetry_outbox
                (event_id, event_type, payload, created_at, next_attempt_at,
                 attempts, size_bytes, dedupe_key)
             VALUES ('event', 'heartbeat', jsonb('{}'), 1, 1, 0, 2, 'key');
             INSERT INTO translation_cache (key, translation, created_at)
             VALUES ('key', 'text', 1);",
    )
    .execute(&upgrade)
    .await
    .unwrap();

    MIGRATOR.run(&upgrade).await.unwrap();

    let fresh = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    MIGRATOR.run(&fresh).await.unwrap();

    for pool in [&upgrade, &fresh] {
        let remaining: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table'
                   AND name IN (
                       'crash_analysis_mod_snapshots',
                       'discovered_javas',
                       'google_translate_ip_cache',
                       'translation_cache'
                   )",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        assert_eq!(remaining, 0);
        let outbox: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table' AND name = 'telemetry_outbox'",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        assert_eq!(outbox, 1);
    }
    let queued: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM telemetry_outbox")
            .fetch_one(&upgrade)
            .await
            .unwrap();
    assert_eq!(queued, 1);
}

#[tokio::test]
async fn marks_whose_account_secrets_were_moved_from_a_fresh_and_an_upgraded_database()
 {
    const EXTERNALIZE_SECRETS_VERSION: i64 = 20261102120000;
    const MARKED: [&str; 3] = [
        "minecraft_users",
        "minecraft_device_tokens",
        "modrinth_users",
    ];

    let upgrade = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let previous_migrator = Migrator {
        migrations: std::borrow::Cow::Owned(
            MIGRATOR
                .iter()
                .filter(|migration| {
                    migration.version < EXTERNALIZE_SECRETS_VERSION
                })
                .cloned()
                .collect(),
        ),
        ..Migrator::DEFAULT
    };
    previous_migrator.run(&upgrade).await.unwrap();
    sqlx::raw_sql(
        "INSERT INTO minecraft_users
                (account_id, uuid, active, username, access_token,
                 refresh_token, expires)
             VALUES ('microsoft:example', 'uuid', 1, 'Player', 'access',
                     'refresh', 20);
             INSERT INTO modrinth_users (id, active, session_id, expires)
             VALUES ('account', 1, 'session', 30);",
    )
    .execute(&upgrade)
    .await
    .unwrap();

    MIGRATOR.run(&upgrade).await.unwrap();

    let fresh = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    MIGRATOR.run(&fresh).await.unwrap();

    for pool in [&upgrade, &fresh] {
        for table in MARKED {
            let sql = format!(
                "SELECT COUNT(*) FROM pragma_table_info('{table}')
                     WHERE name = 'secrets_externalized'
                       AND \"notnull\" = 1 AND dflt_value = '0'"
            );
            let marks: i64 =
                sqlx::query_scalar(&sql).fetch_one(pool).await.unwrap();
            assert_eq!(marks, 1, "{table} has no mark column");
        }
    }

    // A row written before the move keeps its tokens and says nothing was
    // moved out of it.
    let (access, refresh, marks): (String, String, i64) = sqlx::query_as(
        "SELECT access_token, refresh_token, secrets_externalized
             FROM minecraft_users WHERE account_id = 'microsoft:example'",
    )
    .fetch_one(&upgrade)
    .await
    .unwrap();
    assert_eq!((access.as_str(), refresh.as_str()), ("access", "refresh"));
    assert_eq!(marks, 0);
}

#[tokio::test]
async fn retires_the_legacy_tables_from_a_fresh_and_an_upgraded_database() {
    const RETIRE_LEGACY_TABLES_VERSION: i64 = 20261031120000;

    let upgrade = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let previous_migrator = Migrator {
        migrations: std::borrow::Cow::Owned(
            MIGRATOR
                .iter()
                .filter(|migration| {
                    migration.version < RETIRE_LEGACY_TABLES_VERSION
                })
                .cloned()
                .collect(),
        ),
        ..Migrator::DEFAULT
    };
    previous_migrator.run(&upgrade).await.unwrap();
    sqlx::raw_sql(
            "INSERT INTO content_favorites
                (provider, project_id, content_type, saved_at)
             VALUES ('modrinth', '12345678', 'mod', 1);
             INSERT INTO instances
                (id, path, install_stage, launcher_feature_version, name,
                 created, modified)
             VALUES ('instance', '/instance', 'installed', '1', 'Example', 1, 2);
             INSERT INTO processes
                (pid, start_time, name, executable, instance_id)
             VALUES (1, 1, 'java', '/jre/bin/java', 'instance');
             INSERT INTO telemetry_error_daily (day, fingerprint)
             VALUES ('2026-01-01', 'fingerprint');",
        )
        .execute(&upgrade)
        .await
        .unwrap();

    MIGRATOR.run(&upgrade).await.unwrap();

    let fresh = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    MIGRATOR.run(&fresh).await.unwrap();

    for pool in [&upgrade, &fresh] {
        let remaining: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table'
                   AND name IN (
                       'content_favorites',
                       'processes',
                       'telemetry_error_daily'
                   )",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        assert_eq!(remaining, 0);
        // Every content write looks its project up through this view.
        let projects: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM resource_legacy_projects")
                .fetch_one(pool)
                .await
                .unwrap();
        assert_eq!(projects, 0);
    }
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
                    migration.version < DISCARD_LEGACY_OPENAI_MIGRATION_VERSION
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

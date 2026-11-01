//! What the launcher settings promise against a migrated database: the defaults
//! the migrations leave, the round trip of every column the record carries, and
//! the upgrades that rewrite a stored settings row.

use crate::state::settings::{
    DownloadSourceMode, HomeLayout, Settings, default_terracotta_public_nodes,
};
use crate::state::store::Handle;

fn handle_of(pool: &sqlx::SqlitePool) -> Handle {
    Handle::new(pool.clone())
}

#[tokio::test]
async fn official_preferred_sources_round_trip_in_a_fresh_database() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();
    let handle = Handle::new(pool.clone());
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

    let settings = Settings::get(&handle).await.unwrap();
    assert_eq!(
        settings.minecraft_metadata_source,
        DownloadSourceMode::OfficialPreferred
    );
    assert_eq!(
        settings.minecraft_file_source,
        DownloadSourceMode::OfficialPreferred
    );
    assert_eq!(
        settings.modrinth_source,
        DownloadSourceMode::OfficialPreferred
    );
    assert_eq!(
        settings.curseforge_source,
        DownloadSourceMode::OfficialPreferred
    );
}

#[tokio::test]
async fn curseforge_bypass_defaults_on_and_round_trips() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();
    let handle = Handle::new(pool.clone());

    let mut settings = Settings::get(&handle).await.unwrap();
    assert!(settings.resource_chinese_filenames);
    assert!(settings.resource_preserve_filenames);
    settings.resource_chinese_filenames = false;
    settings.resource_preserve_filenames = false;
    assert!(settings.bypass_curseforge_download_restrictions);

    settings.bypass_curseforge_download_restrictions = false;
    settings.update(&handle).await.unwrap();

    let reloaded = Settings::get(&handle).await.unwrap();
    assert!(!reloaded.resource_chinese_filenames);
    assert!(!reloaded.resource_preserve_filenames);
    assert!(!reloaded.bypass_curseforge_download_restrictions);
}

#[tokio::test]
async fn default_game_language_defaults_empty_and_round_trips() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();
    let handle = Handle::new(pool.clone());

    let mut settings = Settings::get(&handle).await.unwrap();
    assert_eq!(settings.default_game_language, "");

    settings.default_game_language = "en_us".to_string();
    settings.update(&handle).await.unwrap();

    let reloaded = Settings::get(&handle).await.unwrap();
    assert_eq!(reloaded.default_game_language, "en_us");
}

#[tokio::test]
async fn default_game_language_migration_upgrades_existing_settings_database() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::query(
        "CREATE TABLE settings (id INTEGER PRIMARY KEY CHECK (id = 0))",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO settings (id) VALUES (0)")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::raw_sql(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/20261103120000_default-game-language.sql"
    )))
    .execute(&pool)
    .await
    .unwrap();

    let language: String = sqlx::query_scalar(
        "SELECT default_game_language FROM settings WHERE id = 0",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(language, "");
    assert!(
        sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&pool)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn ignore_ssl_errors_defaults_off_and_round_trips() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();
    let handle = Handle::new(pool.clone());

    let mut settings = Settings::get(&handle).await.unwrap();
    assert!(!settings.ignore_ssl_errors);

    settings.ignore_ssl_errors = true;
    settings.update(&handle).await.unwrap();

    let reloaded = Settings::get(&handle).await.unwrap();
    assert!(reloaded.ignore_ssl_errors);
}

#[tokio::test]
async fn memory_optimization_round_trips_in_a_fresh_database() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();
    let handle = Handle::new(pool.clone());

    let mut settings = Settings::get(&handle).await.unwrap();
    assert!(!settings.memory.optimize_before_launch);

    settings.memory.optimize_before_launch = true;
    settings.update(&handle).await.unwrap();

    let reloaded = Settings::get(&handle).await.unwrap();
    assert!(reloaded.memory.optimize_before_launch);
}

#[tokio::test]
async fn lightweight_mode_setting_defaults_off_and_round_trips() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();
    let handle = Handle::new(pool.clone());

    let mut settings = Settings::get(&handle).await.unwrap();
    assert!(!settings.enter_lightweight_mode_on_game_launch);

    settings.enter_lightweight_mode_on_game_launch = true;
    settings.update(&handle).await.unwrap();

    let reloaded = Settings::get(&handle).await.unwrap();
    assert!(reloaded.enter_lightweight_mode_on_game_launch);
}

#[tokio::test]
async fn nametag_app_font_setting_defaults_off_and_round_trips() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();
    let handle = Handle::new(pool.clone());

    let mut settings = Settings::get(&handle).await.unwrap();
    assert!(!settings.nametag_uses_app_font);

    settings.nametag_uses_app_font = true;
    settings.update(&handle).await.unwrap();

    let reloaded = Settings::get(&handle).await.unwrap();
    assert!(reloaded.nametag_uses_app_font);
}

#[tokio::test]
async fn lightweight_mode_migration_upgrades_existing_settings_database() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::query(
        "CREATE TABLE settings (id INTEGER PRIMARY KEY CHECK (id = 0))",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO settings (id) VALUES (0)")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::raw_sql(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/20260821120001_lightweight-mode.sql"
    )))
    .execute(&pool)
    .await
    .unwrap();

    let enabled: i64 = sqlx::query_scalar(
        "SELECT enter_lightweight_mode_on_game_launch FROM settings WHERE id = 0",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(enabled, 0);
    assert!(
        sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&pool)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn home_widgets_round_trip_in_a_fresh_database() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();
    let handle = Handle::new(pool.clone());

    let expected = serde_json::json!({
        "version": 1,
        "widgets": [
            {
                "id": "calendar-one",
                "kind": "calendar",
                "size": "2x2"
            }
        ]
    });
    let mut settings = Settings::get(&handle).await.unwrap();
    settings.home_widgets = Some(expected.clone());
    settings.update(&handle).await.unwrap();

    let reloaded = Settings::get(&handle).await.unwrap();
    assert_eq!(reloaded.home_widgets, Some(expected));
}

#[tokio::test]
async fn font_columns_upgrade_an_existing_settings_database() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::query(
        "CREATE TABLE settings (id INTEGER PRIMARY KEY CHECK (id = 0))",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO settings (id) VALUES (0)")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::raw_sql(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/20260919120000_custom-fonts.sql"
    )))
    .execute(&pool)
    .await
    .unwrap();

    let fonts: (Option<String>, Option<String>) =
        sqlx::query_as("SELECT ui_font, mono_font FROM settings WHERE id = 0")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(fonts, (None, None));
    assert!(
        sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&pool)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn fonts_default_to_none_and_round_trip_in_a_fresh_database() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();
    let handle = Handle::new(pool.clone());

    let mut settings = Settings::get(&handle).await.unwrap();
    assert_eq!(settings.ui_font, None);
    assert_eq!(settings.mono_font, None);

    settings.ui_font = Some("  Microsoft YaHei  ".to_string());
    settings.mono_font = Some("JetBrains Mono".to_string());
    settings.update(&handle).await.unwrap();

    let reloaded = Settings::get(&handle).await.unwrap();
    assert_eq!(reloaded.ui_font.as_deref(), Some("Microsoft YaHei"));
    assert_eq!(reloaded.mono_font.as_deref(), Some("JetBrains Mono"));

    settings.mono_font = Some("   ".to_string());
    settings.update(&handle).await.unwrap();

    let cleared = Settings::get(&handle).await.unwrap();
    assert_eq!(cleared.mono_font, None);
}

#[tokio::test]
async fn terracotta_public_nodes_default_and_empty_list_round_trip() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();
    let handle = Handle::new(pool.clone());

    let mut settings = Settings::get(&handle).await.unwrap();
    assert_eq!(
        settings.terracotta_public_nodes,
        default_terracotta_public_nodes()
    );

    settings.terracotta_public_nodes.clear();
    settings.update(&handle).await.unwrap();

    let reloaded = Settings::get(&handle).await.unwrap();
    assert!(reloaded.terracotta_public_nodes.is_empty());
}

#[tokio::test]
async fn mojang_auth_source_round_trip_in_a_fresh_database() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();
    let handle = Handle::new(pool.clone());

    let mut settings = Settings::get(&handle).await.unwrap();
    assert_eq!(settings.mojang_auth_source, DownloadSourceMode::Auto);
    settings.mojang_auth_source = DownloadSourceMode::MirrorPreferred;
    settings.update(&handle).await.unwrap();

    let reloaded = Settings::get(&handle).await.unwrap();
    assert_eq!(
        reloaded.mojang_auth_source,
        DownloadSourceMode::MirrorPreferred
    );
}

#[tokio::test]
async fn legacy_mirror_settings_keep_their_previous_intent() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();
    let handle = Handle::new(pool.clone());
    let settings = Settings::get(&handle).await.unwrap();
    assert!(settings.auto_concurrent_downloads);
    assert!(settings.auto_set_java_high_performance_mode);
    assert!(!settings.auto_hide_downloads_button);
    assert_eq!(settings.home_layout, HomeLayout::Standard);
    assert_eq!(settings.minimal_home_instance_id, None);
    assert_eq!(settings.home_widgets, None);
    assert_eq!(settings.minecraft_metadata_source, DownloadSourceMode::Auto);
    assert_eq!(settings.minecraft_file_source, DownloadSourceMode::Auto);
    assert_eq!(settings.modrinth_source, DownloadSourceMode::Auto);
    assert_eq!(settings.curseforge_source, DownloadSourceMode::Auto);
    let mut legacy = serde_json::to_value(settings).unwrap();
    let object = legacy.as_object_mut().unwrap();
    object.remove("minecraft_metadata_source");
    object.remove("minecraft_file_source");
    object.remove("modrinth_source");
    object.remove("curseforge_source");
    object.insert("use_minecraft_mirror".to_string(), true.into());
    object.insert("use_modrinth_mirror".to_string(), false.into());
    object.insert("use_curseforge_mirror".to_string(), false.into());

    let mut migrated: Settings = serde_json::from_value(legacy).unwrap();
    migrated.apply_legacy_download_source_settings();

    assert_eq!(
        migrated.minecraft_metadata_source,
        DownloadSourceMode::MirrorPreferred
    );
    assert_eq!(
        migrated.minecraft_file_source,
        DownloadSourceMode::MirrorPreferred
    );
    assert_eq!(migrated.modrinth_source, DownloadSourceMode::OfficialOnly);
    assert_eq!(migrated.curseforge_source, DownloadSourceMode::OfficialOnly);
}

#[tokio::test]
async fn download_source_reset_migration_sets_all_sources_to_auto() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();
    let handle = Handle::new(pool.clone());
    sqlx::query(
        "
        UPDATE settings
        SET
            minecraft_metadata_source = 'official_only',
            minecraft_file_source = 'mirror_preferred',
            modrinth_source = 'official_only',
            curseforge_source = 'mirror_preferred'
        ",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/20260721120000_reset-download-sources-to-auto.sql"
    )))
    .execute(&pool)
    .await
    .unwrap();

    let settings = Settings::get(&handle).await.unwrap();
    assert_eq!(settings.minecraft_metadata_source, DownloadSourceMode::Auto);
    assert_eq!(settings.minecraft_file_source, DownloadSourceMode::Auto);
    assert_eq!(settings.modrinth_source, DownloadSourceMode::Auto);
    assert_eq!(settings.curseforge_source, DownloadSourceMode::Auto);
}

#[tokio::test]
async fn telemetry_schema_migrates_fresh_and_existing_settings_databases() {
    let fresh = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!().run(&fresh).await.unwrap();
    let settings = Settings::get(&Handle::new(fresh.clone())).await.unwrap();
    assert!(!settings.telemetry);
    assert_eq!(settings.telemetry_consent_version, 0);
    assert!(
        sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&fresh)
            .await
            .unwrap()
            .is_empty()
    );

    let upgrade = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::query(
        "CREATE TABLE settings (id INTEGER PRIMARY KEY CHECK (id = 0), telemetry INTEGER NOT NULL DEFAULT 0, discord_rpc INTEGER NOT NULL DEFAULT 1)",
    )
    .execute(&upgrade)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO settings (id, telemetry, discord_rpc) VALUES (0, 0, 1)",
    )
    .execute(&upgrade)
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/20260814120000_telemetry.sql"
    )))
    .execute(&upgrade)
    .await
    .unwrap();
    let consent_version = sqlx::query_scalar::<_, i64>(
        "SELECT telemetry_consent_version FROM settings WHERE id = 0",
    )
    .fetch_one(&upgrade)
    .await
    .unwrap();
    assert_eq!(consent_version, 0);
    assert!(
        sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&upgrade)
            .await
            .unwrap()
            .is_empty()
    );
}

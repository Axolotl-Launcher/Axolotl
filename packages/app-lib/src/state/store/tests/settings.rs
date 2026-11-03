//! What the launcher settings collection promises.

use super::support;
use crate::state::store::Collection as _;
use crate::state::store::Handle;
use crate::state::store::settings::{
    DownloadSource, HomeLayout, Settings, SettingsRecord, Singleton,
};

fn handle_of(pool: &sqlx::SqlitePool) -> Handle {
    Handle::new(pool.clone())
}

fn record() -> SettingsRecord {
    SettingsRecord {
        max_concurrent_downloads: 7,
        max_concurrent_writes: 5,
        theme: "oled".to_string(),
        default_page: "library".to_string(),
        collapsed_navigation: true,
        advanced_rendering: false,
        native_decorations: true,
        telemetry: true,
        discord_rpc: false,
        developer_mode: true,
        onboarded: true,
        extra_launch_args: "[\"-Xmx4G\"]".to_string(),
        custom_env_vars: "[[\"KEY\",\"VALUE\"]]".to_string(),
        mc_memory_max: 4096,
        mc_force_fullscreen: true,
        mc_game_resolution_x: 1920,
        mc_game_resolution_y: 1080,
        hide_on_process_start: true,
        hook_pre_launch: Some("echo before".to_string()),
        hook_wrapper: None,
        hook_post_exit: Some("echo after".to_string()),
        custom_dir: Some("/somewhere".to_string()),
        prev_custom_dir: None,
        migrated: true,
        personalized_ads: false,
        toggle_sidebar: true,
        feature_flags: "{\"page_transitions\":false}".to_string(),
        hide_nametag_skins_page: true,
        nametag_uses_app_font: true,
        skipped_update: Some("1.0".to_string()),
        pending_update_toast_for_version: None,
        auto_download_updates: Some(false),
        version: 3,
        locale: "zh-CN".to_string(),
        accent_color: "blue".to_string(),
        custom_background_path: Some("/bg.png".to_string()),
        custom_background_blur: 10,
        custom_background_opacity: 50,
        use_minecraft_mirror: true,
        use_modrinth_mirror: false,
        use_curseforge_mirror: true,
        auto_concurrent_downloads: false,
        minecraft_metadata_source: DownloadSource::OfficialOnly,
        minecraft_file_source: DownloadSource::MirrorPreferred,
        modrinth_source: DownloadSource::Auto,
        curseforge_source: DownloadSource::OfficialPreferred,
        onboarding_version: 2,
        onboarding_instance_tour_completed: false,
        sidebar_instance_count: 3,
        mc_memory_auto: true,
        mc_memory_optimize: true,
        auto_set_java_high_performance_mode: false,
        transparent_background: true,
        transparent_background_opacity: 33,
        transparent_background_blur: true,
        home_layout: HomeLayout::Minimal,
        minimal_home_instance_id: None,
        auto_hide_downloads_button: true,
        home_widgets: Some("{\"version\":1}".to_string()),
        mojang_auth_source: DownloadSource::OfficialPreferred,
        terracotta_public_nodes: "[\"wss://node.example.invalid\"]".to_string(),
        telemetry_consent_version: 4,
        bypass_curseforge_download_restrictions: false,
        enter_lightweight_mode_on_game_launch: true,
        close_behavior: "lightweight".to_string(),
        mc_maximize_window: true,
        log_level: "debug".to_string(),
        sync_features_across_devices: true,
        show_files_tab_in_instances: false,
        show_worlds_tab_in_instances: false,
        show_screenshots_tab_in_instances: true,
        show_skin_selector_in_sidebar: false,
        ignore_ssl_errors: true,
        home_widget_background_opacity: 20,
        hidden_nav_items: "[\"screenshots\"]".to_string(),
        custom_window_title_enabled: true,
        default_window_title: "Axolotl".to_string(),
        custom_background_component_opacity: 40,
        ui_font: Some("Inter".to_string()),
        mono_font: Some("JetBrains Mono".to_string()),
        backup_repository_path: Some("/backups".to_string()),
        allow_external_scheme: false,
        allow_privileged_scheme: true,
        doh_enabled: false,
        resource_chinese_filenames: false,
        resource_preserve_filenames: false,
        default_game_language: "en_us".to_string(),
    }
}

#[tokio::test]
async fn the_migrated_row_reads_as_the_settings_it_holds() {
    let handle = support::handle().await;

    let stored = Settings::get(&handle, &Singleton)
        .await
        .unwrap()
        .expect("settings");

    assert_eq!(stored.max_concurrent_downloads, 10);
    assert_eq!(stored.theme, "dark");
    assert_eq!(stored.default_page, "home");
    assert_eq!(stored.extra_launch_args, "[]");
    assert_eq!(stored.custom_env_vars, "[]");
    assert_eq!(stored.feature_flags, "{}");
    assert_eq!(
        stored.terracotta_public_nodes,
        "[\"wss://center.node.1tmc.top\"]"
    );
    assert!(!stored.nametag_uses_app_font);
    assert_eq!(stored.minecraft_file_source, DownloadSource::Auto);
    assert_eq!(stored.home_layout, HomeLayout::Standard);
    assert_eq!(stored.home_widgets, None);
    assert_eq!(stored.auto_download_updates, None);
    assert_eq!(stored.version, 1);
    assert_eq!(stored.default_game_language, "");
}

#[tokio::test]
async fn a_written_record_reads_back_field_for_field() {
    let handle = support::handle().await;
    Settings::set(&handle, &Singleton, Some(&record()))
        .await
        .unwrap();

    assert_eq!(
        Settings::get(&handle, &Singleton).await.unwrap(),
        Some(record())
    );
}

#[tokio::test]
async fn writing_leaves_the_columns_it_does_not_carry_alone() {
    let pool = support::pool().await;
    let handle = Handle::new(pool.clone());
    sqlx::query(
        "
        UPDATE settings
        SET proxy_mode = 'custom', proxy_url = 'http://proxy.invalid:8080',
            curseforge_api_key = 'key', download_engine = 'legacy',
            doh_server = 'https://dns.invalid/query'
        WHERE id = 0
        ",
    )
    .execute(&pool)
    .await
    .unwrap();

    Settings::set(&handle, &Singleton, Some(&record()))
        .await
        .unwrap();

    let row: (String, String, String, String, String) = sqlx::query_as(
        "
        SELECT proxy_mode, proxy_url, curseforge_api_key, download_engine,
            doh_server
        FROM settings
        WHERE id = 0
        ",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        row,
        (
            "custom".to_string(),
            "http://proxy.invalid:8080".to_string(),
            "key".to_string(),
            "legacy".to_string(),
            "https://dns.invalid/query".to_string(),
        )
    );
}

#[tokio::test]
async fn a_source_this_build_cannot_read_is_auto() {
    let pool = support::pool().await;
    let handle = Handle::new(pool.clone());
    sqlx::query("PRAGMA ignore_check_constraints = ON")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE settings SET minecraft_file_source = 'elsewhere',
            home_layout = 'sideways' WHERE id = 0",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("PRAGMA ignore_check_constraints = OFF")
        .execute(&pool)
        .await
        .unwrap();

    let stored = Settings::get(&handle, &Singleton)
        .await
        .unwrap()
        .expect("settings");
    assert_eq!(stored.minecraft_file_source, DownloadSource::Auto);
    assert_eq!(stored.home_layout, HomeLayout::Standard);
}

#[tokio::test]
async fn clearing_removes_the_row() {
    let handle = support::handle().await;
    Settings::set(&handle, &Singleton, None).await.unwrap();

    assert_eq!(Settings::get(&handle, &Singleton).await.unwrap(), None);
    assert!(Settings::get_all(&handle).await.unwrap().is_empty());
}

#[tokio::test]
async fn a_replacement_keeps_the_columns_it_does_not_carry() {
    let pool = support::pool().await;
    let handle = handle_of(&pool);
    sqlx::query(
        "
        UPDATE settings
        SET proxy_url = 'http://proxy.invalid:8080', curseforge_api_key = 'key'
        WHERE id = 0
        ",
    )
    .execute(&pool)
    .await
    .unwrap();

    Settings::set_all(&handle, &[(Singleton, record())])
        .await
        .unwrap();

    let row: (String, String) = sqlx::query_as(
        "SELECT proxy_url, curseforge_api_key FROM settings WHERE id = 0",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        row,
        ("http://proxy.invalid:8080".to_string(), "key".to_string(),)
    );
}

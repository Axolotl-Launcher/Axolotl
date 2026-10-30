//! What the settings patch promises: field-level merges, the privacy fields it
//! must not touch, and the one write it performs at a time.

use super::support;
use crate::api::settings::{
    SETTINGS_WRITE, apply_mojang_auth_source_with, patch_with,
};
use crate::state::DownloadSourceMode;
use crate::state::settings::Settings;
use crate::state::store::Handle;
use serde_json::json;

fn handle_of(pool: &sqlx::SqlitePool) -> Handle {
    Handle::new(pool.clone())
}

#[tokio::test]
async fn mojang_probe_updates_preserve_arguments_and_reject_stale_modes() {
    let pool = support::pool().await;
    let handle = handle_of(&pool);
    patch_with(&handle, json!({"extra_launch_args": ["-Xmx4G", "-Dcustom=keep"], "mojang_auth_source": "auto"})).await.unwrap();
    assert!(
        apply_mojang_auth_source_with(&handle, DownloadSourceMode::Auto, true)
            .await
            .unwrap()
    );
    let mirrored = Settings::get(&handle).await.unwrap();
    assert_eq!(mirrored.extra_launch_args.len(), 7);
    assert!(
        mirrored
            .extra_launch_args
            .contains(&"-Dcustom=keep".to_string())
    );
    patch_with(&handle, json!({"extra_launch_args": ["-Dnew=user-edit"], "mojang_auth_source": "official_only"})).await.unwrap();
    assert!(
        !apply_mojang_auth_source_with(&handle, DownloadSourceMode::Auto, true)
            .await
            .unwrap()
    );
    assert_eq!(
        Settings::get(&handle).await.unwrap().extra_launch_args,
        ["-Dnew=user-edit"]
    );
    // An explicit official mode also ignores an inconsistent mirror request.
    assert!(
        apply_mojang_auth_source_with(
            &handle,
            DownloadSourceMode::OfficialOnly,
            true
        )
        .await
        .unwrap()
    );
    assert_eq!(
        Settings::get(&handle).await.unwrap().extra_launch_args,
        ["-Dnew=user-edit"]
    );
    patch_with(&handle, json!({"mojang_auth_source": "mirror_preferred"}))
        .await
        .unwrap();
    assert!(
        apply_mojang_auth_source_with(
            &handle,
            DownloadSourceMode::MirrorPreferred,
            false
        )
        .await
        .unwrap()
    );
    assert_eq!(
        Settings::get(&handle)
            .await
            .unwrap()
            .extra_launch_args
            .len(),
        6
    );
    patch_with(&handle, json!({"mojang_auth_source": "official_preferred"}))
        .await
        .unwrap();
    assert!(
        apply_mojang_auth_source_with(
            &handle,
            DownloadSourceMode::OfficialPreferred,
            false
        )
        .await
        .unwrap()
    );
    assert_eq!(
        Settings::get(&handle).await.unwrap().extra_launch_args,
        ["-Dnew=user-edit"]
    );
}

#[tokio::test]
async fn settings_patch_preserves_unrelated_fields_and_nested_flags() {
    let pool = support::pool().await;
    let handle = handle_of(&pool);
    let initial = Settings::get(&handle).await.unwrap();
    patch_with(&handle, json!({"ui_font": "Test Font", "feature_flags": {"page_transitions": false}})).await.unwrap();
    patch_with(&handle, json!({"developer_mode": true, "feature_flags": {"show_instance_play_time": false}})).await.unwrap();
    let saved = Settings::get(&handle).await.unwrap();
    assert_eq!(saved.ui_font.as_deref(), Some("Test Font"));
    assert!(saved.developer_mode);
    let flags = serde_json::to_value(&saved.feature_flags).unwrap();
    assert_eq!(flags["page_transitions"], false);
    assert_eq!(flags["show_instance_play_time"], false);
    assert_eq!(saved.locale, initial.locale);
    let saved = patch_with(&handle, json!({"ui_font": null})).await.unwrap();
    assert_eq!(saved.ui_font, None);
    assert!(saved.developer_mode);
}

#[tokio::test]
async fn settings_patch_persists_normalized_default_game_language() {
    let pool = support::pool().await;
    let handle = handle_of(&pool);

    let saved =
        patch_with(&handle, json!({"default_game_language": "  EN-US  "}))
            .await
            .unwrap();

    assert_eq!(saved.default_game_language, "en_us");
    assert_eq!(
        Settings::get(&handle).await.unwrap().default_game_language,
        "en_us"
    );
}

#[tokio::test]
async fn settings_patch_rejects_invalid_default_game_language() {
    let pool = support::pool().await;
    let handle = handle_of(&pool);
    let initial = Settings::get(&handle).await.unwrap();

    assert!(
        patch_with(&handle, json!({"default_game_language": "en US"}),)
            .await
            .is_err()
    );

    let saved = Settings::get(&handle).await.unwrap();
    assert_eq!(
        serde_json::to_value(saved).unwrap(),
        serde_json::to_value(initial).unwrap()
    );
}

#[tokio::test]
async fn settings_patch_rejects_invalid_fields_without_writing() {
    let pool = support::pool().await;
    let handle = handle_of(&pool);
    let initial = Settings::get(&handle).await.unwrap();
    for patch in [
        json!({"unknown_field": true}),
        json!({"developer_mode": "bad"}),
        json!([]),
    ] {
        assert!(patch_with(&handle, patch).await.is_err());
    }
    let saved = Settings::get(&handle).await.unwrap();
    assert_eq!(
        serde_json::to_value(saved).unwrap(),
        serde_json::to_value(initial).unwrap()
    );
}

#[tokio::test]
async fn settings_patch_does_not_override_privacy_consent() {
    let pool = support::pool().await;
    let handle = handle_of(&pool);
    let initial = Settings::get(&handle).await.unwrap();
    let saved = patch_with(&handle, json!({"telemetry": !initial.telemetry, "discord_rpc": !initial.discord_rpc, "telemetry_consent_version": 999})).await.unwrap();
    assert_eq!(saved.telemetry, initial.telemetry);
    assert_eq!(saved.discord_rpc, initial.discord_rpc);
    assert_eq!(
        saved.telemetry_consent_version,
        initial.telemetry_consent_version
    );
}

#[tokio::test]
async fn settings_patch_serializes_concurrent_regions() {
    let pool = support::pool().await;
    let handle = handle_of(&pool);
    let save = |patch| async {
        let _guard = SETTINGS_WRITE.lock().await;
        patch_with(&handle, patch).await.unwrap();
    };
    tokio::join!(
        save(json!({"ui_font": "Concurrent Font"})),
        save(json!({"developer_mode": true}))
    );
    let saved = Settings::get(&handle).await.unwrap();
    assert_eq!(saved.ui_font.as_deref(), Some("Concurrent Font"));
    assert!(saved.developer_mode);
}

#[tokio::test]
async fn settings_patch_coordinates_direct_database_writers() {
    let directory = tempfile::tempdir().unwrap();
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(directory.path().join("settings.db"))
        .create_if_missing(true)
        .busy_timeout(std::time::Duration::from_secs(5));
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(2)
        .connect_with(options)
        .await
        .unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();
    let handle = handle_of(&pool);
    let mut direct = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
    sqlx::query("UPDATE settings SET backup_repository_path = 'moved-repository', mc_force_fullscreen = 1 WHERE id = 0")
            .execute(&mut *direct).await.unwrap();
    let patch_handle = handle.clone();
    let mut task = tokio::spawn(async move {
        patch_with(&patch_handle, json!({"developer_mode": true}))
            .await
            .unwrap()
    });
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(50), &mut task)
            .await
            .is_err()
    );
    direct.commit().await.unwrap();
    task.await.unwrap();
    let saved = Settings::get(&handle).await.unwrap();
    assert_eq!(
        saved.backup_repository_path.as_deref(),
        Some("moved-repository")
    );
    assert!(saved.force_fullscreen);
    assert!(saved.developer_mode);
    pool.close().await;
}

#[tokio::test]
async fn settings_patch_rolls_back_all_fields_if_a_write_fails() {
    let pool = support::pool().await;
    let handle = handle_of(&pool);
    let initial = Settings::get(&handle).await.unwrap();
    sqlx::query("CREATE TRIGGER reject_font BEFORE UPDATE OF ui_font ON settings BEGIN SELECT RAISE(ABORT, 'write failed'); END")
            .execute(&pool).await.unwrap();
    assert!(
        patch_with(
            &handle,
            json!({"developer_mode": true, "ui_font": "Rejected Font"})
        )
        .await
        .is_err()
    );
    let saved = Settings::get(&handle).await.unwrap();
    assert_eq!(
        serde_json::to_value(saved).unwrap(),
        serde_json::to_value(initial).unwrap()
    );
}

//! Theseus settings management interface

pub use crate::{
    State,
    state::{
        DownloadSourceMode, Hooks, MemorySettings, PrivacySettings, Settings,
        WindowSize,
    },
};

static SETTINGS_WRITE: tokio::sync::Mutex<()> =
    tokio::sync::Mutex::const_new(());

fn merge_settings(
    current: &mut serde_json::Value,
    patch: serde_json::Value,
) -> crate::Result<()> {
    if let (Some(target), Some(changes)) =
        (current.as_object_mut(), patch.as_object())
    {
        for (key, value) in changes {
            let existing = target.get_mut(key).ok_or_else(|| {
                crate::ErrorKind::InputError(format!("Unknown setting {key}"))
            })?;
            if matches!(key.as_str(), "feature_flags" | "memory" | "hooks")
                && existing.is_object()
                && value.is_object()
            {
                existing
                    .as_object_mut()
                    .unwrap()
                    .extend(value.as_object().unwrap().clone());
            } else {
                *existing = value.clone();
            }
        }
        Ok(())
    } else {
        Err(crate::ErrorKind::InputError(
            "Settings patch must be an object".to_string(),
        )
        .into())
    }
}

async fn patch_with_pool(
    pool: &sqlx::SqlitePool,
    patch: serde_json::Value,
) -> crate::Result<Settings> {
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
    let current = Settings::get_with_connection(&mut transaction).await?;
    let mut value = serde_json::to_value(&current)?;
    merge_settings(&mut value, patch)?;
    let mut settings: Settings = serde_json::from_value(value)?;
    settings.telemetry = current.telemetry;
    settings.telemetry_consent_version = current.telemetry_consent_version;
    settings.discord_rpc = current.discord_rpc;
    super::terracotta::validate_public_nodes(
        &settings.terracotta_public_nodes,
    )?;
    crate::util::download_dns::DownloadDnsResolver::with_doh(
        settings.doh_enabled,
    )?;
    settings.update_with_connection(&mut transaction).await?;
    let saved = Settings::get_with_connection(&mut transaction).await?;
    transaction.commit().await?;
    Ok(saved)
}

/// Merges field-level changes while serializing settings writes.
pub async fn patch(patch: serde_json::Value) -> crate::Result<Settings> {
    let _guard = SETTINGS_WRITE.lock().await;
    let state = State::get().await?;
    let settings = patch_with_pool(&state.pool, patch).await?;
    state.update_http_client_for_settings(&settings).await?;
    state.update_download_settings(&settings);
    Ok(settings)
}

async fn apply_mojang_auth_source_with_pool(
    pool: &sqlx::SqlitePool,
    expected_source: DownloadSourceMode,
    use_mirror: bool,
) -> crate::Result<bool> {
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
    let mut settings = Settings::get_with_connection(&mut transaction).await?;
    if settings.mojang_auth_source != expected_source {
        return Ok(false);
    }
    // Explicit choices cannot be overridden by a delayed automatic probe.
    let use_mirror = match expected_source {
        DownloadSourceMode::MirrorPreferred => true,
        DownloadSourceMode::OfficialOnly => false,
        DownloadSourceMode::Auto | DownloadSourceMode::OfficialPreferred => {
            use_mirror
        }
    };
    let args = crate::util::mojang::auth_proxy_java_args(
        &settings.extra_launch_args,
        use_mirror,
    );
    if args != settings.extra_launch_args {
        settings.extra_launch_args = args;
        settings.update_with_connection(&mut transaction).await?;
    }
    transaction.commit().await?;
    Ok(true)
}

/// Applies a probe result only while its source mode is still selected, preserving concurrent edits.
pub(crate) async fn apply_mojang_auth_source(
    expected_source: DownloadSourceMode,
    use_mirror: bool,
) -> crate::Result<bool> {
    let _guard = SETTINGS_WRITE.lock().await;
    let state = State::get().await?;
    let applied = apply_mojang_auth_source_with_pool(
        &state.pool,
        expected_source,
        use_mirror,
    )
    .await?;
    if applied {
        state.set_mojang_auth_use_mirror(match expected_source {
            DownloadSourceMode::MirrorPreferred => true,
            DownloadSourceMode::OfficialOnly => false,
            DownloadSourceMode::Auto
            | DownloadSourceMode::OfficialPreferred => use_mirror,
        });
    }
    Ok(applied)
}

/// Gets entire settings
#[tracing::instrument]
pub async fn get() -> crate::Result<Settings> {
    let state = State::get().await?;
    let settings = Settings::get(&state.pool).await?;
    Ok(settings)
}

#[tracing::instrument]
pub async fn get_privacy() -> crate::Result<PrivacySettings> {
    let state = State::get().await?;
    let settings = Settings::get(&state.pool).await?;
    Ok(PrivacySettings {
        telemetry: settings.telemetry,
        discord_rpc: settings.discord_rpc,
        consent_version: settings.telemetry_consent_version,
    })
}

#[tracing::instrument]
pub async fn set_privacy(
    privacy: PrivacySettings,
) -> crate::Result<PrivacySettings> {
    let _guard = SETTINGS_WRITE.lock().await;
    let state = State::get().await?;
    let mut transaction = state.pool.begin().await?;
    sqlx::query(
		"UPDATE settings SET telemetry = ?, discord_rpc = ?, telemetry_consent_version = ? WHERE id = 0",
	)
	.bind(privacy.telemetry)
	.bind(privacy.discord_rpc)
	.bind(privacy.consent_version)
	.execute(&mut *transaction)
	.await?;
    sqlx::query("DELETE FROM telemetry_outbox")
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;

    if let Err(error) =
        crate::telemetry::set_enabled(&state, privacy.telemetry).await
    {
        tracing::debug!(target: "theseus::telemetry", %error, "Failed to apply telemetry state");
    }
    if let Err(error) = state.discord_rpc.clear_to_default(true).await {
        tracing::debug!(target: "theseus::telemetry", %error, "Failed to apply Discord RPC state");
    }
    get_privacy().await
}

#[tracing::instrument]
pub async fn set_telemetry(enabled: bool) -> crate::Result<PrivacySettings> {
    let _guard = SETTINGS_WRITE.lock().await;
    let state = State::get().await?;
    let mut transaction = state.pool.begin().await?;
    sqlx::query("UPDATE settings SET telemetry = ? WHERE id = 0")
        .bind(enabled)
        .execute(&mut *transaction)
        .await?;
    sqlx::query("DELETE FROM telemetry_outbox")
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    if let Err(error) = crate::telemetry::set_enabled(&state, enabled).await {
        tracing::debug!(target: "theseus::telemetry", %error, "Failed to apply telemetry state");
    }
    get_privacy().await
}

#[tracing::instrument]
pub async fn set_discord_rpc(enabled: bool) -> crate::Result<PrivacySettings> {
    let _guard = SETTINGS_WRITE.lock().await;
    let state = State::get().await?;
    sqlx::query("UPDATE settings SET discord_rpc = ? WHERE id = 0")
        .bind(enabled)
        .execute(&state.pool)
        .await?;
    if let Err(error) = state.discord_rpc.clear_to_default(true).await {
        tracing::debug!(target: "theseus::telemetry", %error, "Failed to apply Discord RPC state");
    }
    get_privacy().await
}

#[tracing::instrument]
pub async fn cancel_directory_change(
    app_identifier: &str,
) -> crate::Result<()> {
    // This is called to handle state initialization errors due to folder migrations
    // failing, so fetching a DB connection pool from `State::get` is not reliable here
    let pool = crate::state::db::connect(app_identifier).await?;
    let mut settings = Settings::get(&pool).await?;

    if let Some(prev_custom_dir) = settings.prev_custom_dir {
        settings.prev_custom_dir = None;
        settings.custom_dir = Some(prev_custom_dir);
    }

    settings.update(&pool).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    async fn pool() -> sqlx::SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn mojang_probe_updates_preserve_arguments_and_reject_stale_modes() {
        let pool = pool().await;
        patch_with_pool(&pool, json!({"extra_launch_args": ["-Xmx4G", "-Dcustom=keep"], "mojang_auth_source": "auto"})).await.unwrap();
        assert!(
            apply_mojang_auth_source_with_pool(
                &pool,
                DownloadSourceMode::Auto,
                true
            )
            .await
            .unwrap()
        );
        let mirrored = Settings::get(&pool).await.unwrap();
        assert_eq!(mirrored.extra_launch_args.len(), 7);
        assert!(
            mirrored
                .extra_launch_args
                .contains(&"-Dcustom=keep".to_string())
        );
        patch_with_pool(&pool, json!({"extra_launch_args": ["-Dnew=user-edit"], "mojang_auth_source": "official_only"})).await.unwrap();
        assert!(
            !apply_mojang_auth_source_with_pool(
                &pool,
                DownloadSourceMode::Auto,
                true
            )
            .await
            .unwrap()
        );
        assert_eq!(
            Settings::get(&pool).await.unwrap().extra_launch_args,
            ["-Dnew=user-edit"]
        );
        // An explicit official mode also ignores an inconsistent mirror request.
        assert!(
            apply_mojang_auth_source_with_pool(
                &pool,
                DownloadSourceMode::OfficialOnly,
                true
            )
            .await
            .unwrap()
        );
        assert_eq!(
            Settings::get(&pool).await.unwrap().extra_launch_args,
            ["-Dnew=user-edit"]
        );
        patch_with_pool(
            &pool,
            json!({"mojang_auth_source": "mirror_preferred"}),
        )
        .await
        .unwrap();
        assert!(
            apply_mojang_auth_source_with_pool(
                &pool,
                DownloadSourceMode::MirrorPreferred,
                false
            )
            .await
            .unwrap()
        );
        assert_eq!(
            Settings::get(&pool).await.unwrap().extra_launch_args.len(),
            6
        );
        patch_with_pool(
            &pool,
            json!({"mojang_auth_source": "official_preferred"}),
        )
        .await
        .unwrap();
        assert!(
            apply_mojang_auth_source_with_pool(
                &pool,
                DownloadSourceMode::OfficialPreferred,
                false
            )
            .await
            .unwrap()
        );
        assert_eq!(
            Settings::get(&pool).await.unwrap().extra_launch_args,
            ["-Dnew=user-edit"]
        );
    }

    #[tokio::test]
    async fn settings_patch_preserves_unrelated_fields_and_nested_flags() {
        let pool = pool().await;
        let initial = Settings::get(&pool).await.unwrap();
        patch_with_pool(&pool, json!({"ui_font": "Test Font", "feature_flags": {"page_transitions": false}})).await.unwrap();
        patch_with_pool(&pool, json!({"developer_mode": true, "feature_flags": {"show_instance_play_time": false}})).await.unwrap();
        let saved = Settings::get(&pool).await.unwrap();
        assert_eq!(saved.ui_font.as_deref(), Some("Test Font"));
        assert!(saved.developer_mode);
        let flags = serde_json::to_value(&saved.feature_flags).unwrap();
        assert_eq!(flags["page_transitions"], false);
        assert_eq!(flags["show_instance_play_time"], false);
        assert_eq!(saved.locale, initial.locale);
        let saved = patch_with_pool(&pool, json!({"ui_font": null}))
            .await
            .unwrap();
        assert_eq!(saved.ui_font, None);
        assert!(saved.developer_mode);
    }

    #[tokio::test]
    async fn settings_patch_rejects_invalid_fields_without_writing() {
        let pool = pool().await;
        let initial = Settings::get(&pool).await.unwrap();
        for patch in [
            json!({"unknown_field": true}),
            json!({"developer_mode": "bad"}),
            json!([]),
        ] {
            assert!(patch_with_pool(&pool, patch).await.is_err());
        }
        let saved = Settings::get(&pool).await.unwrap();
        assert_eq!(
            serde_json::to_value(saved).unwrap(),
            serde_json::to_value(initial).unwrap()
        );
    }

    #[tokio::test]
    async fn settings_patch_does_not_override_privacy_consent() {
        let pool = pool().await;
        let initial = Settings::get(&pool).await.unwrap();
        let saved = patch_with_pool(&pool, json!({"telemetry": !initial.telemetry, "discord_rpc": !initial.discord_rpc, "telemetry_consent_version": 999})).await.unwrap();
        assert_eq!(saved.telemetry, initial.telemetry);
        assert_eq!(saved.discord_rpc, initial.discord_rpc);
        assert_eq!(
            saved.telemetry_consent_version,
            initial.telemetry_consent_version
        );
    }

    #[tokio::test]
    async fn settings_patch_serializes_concurrent_regions() {
        let pool = pool().await;
        let save = |patch| async {
            let _guard = SETTINGS_WRITE.lock().await;
            patch_with_pool(&pool, patch).await.unwrap();
        };
        tokio::join!(
            save(json!({"ui_font": "Concurrent Font"})),
            save(json!({"developer_mode": true}))
        );
        let saved = Settings::get(&pool).await.unwrap();
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
        let mut direct = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
        sqlx::query("UPDATE settings SET backup_repository_path = 'moved-repository', mc_force_fullscreen = 1 WHERE id = 0")
            .execute(&mut *direct).await.unwrap();
        let patch_pool = pool.clone();
        let mut task = tokio::spawn(async move {
            patch_with_pool(&patch_pool, json!({"developer_mode": true}))
                .await
                .unwrap()
        });
        assert!(
            tokio::time::timeout(
                std::time::Duration::from_millis(50),
                &mut task
            )
            .await
            .is_err()
        );
        direct.commit().await.unwrap();
        task.await.unwrap();
        let saved = Settings::get(&pool).await.unwrap();
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
        let pool = pool().await;
        let initial = Settings::get(&pool).await.unwrap();
        sqlx::query("CREATE TRIGGER reject_font BEFORE UPDATE OF ui_font ON settings BEGIN SELECT RAISE(ABORT, 'write failed'); END")
            .execute(&pool).await.unwrap();
        assert!(
            patch_with_pool(
                &pool,
                json!({"developer_mode": true, "ui_font": "Rejected Font"})
            )
            .await
            .is_err()
        );
        let saved = Settings::get(&pool).await.unwrap();
        assert_eq!(
            serde_json::to_value(saved).unwrap(),
            serde_json::to_value(initial).unwrap()
        );
    }
}

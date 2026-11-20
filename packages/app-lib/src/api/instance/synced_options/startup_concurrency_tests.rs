use super::*;
use crate::state::store::instance_sync_preferences::{
    InstancePreferenceKey, InstancePreferenceRecord, InstanceSyncPreferences,
};
use crate::state::store::sync_feature_settings::{
    FeatureSettingsKey, FeatureSettingsRecord,
};
use crate::state::{DirectoryInfo, Settings};
use std::time::Duration;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cold_core_pool_serves_reads_during_disabled_sync_config_and_backup() {
    let temporary = tempfile::tempdir().unwrap();
    let dirs = DirectoryInfo {
        settings_dir: temporary.path().join("data/settings"),
        config_dir: temporary.path().join("content"),
        app_identifier: "startup-concurrency-test".into(),
    };
    tokio::fs::create_dir_all(dirs.settings_dir.join("data/settings"))
        .await
        .unwrap();
    tokio::fs::write(
        dirs.settings_dir.join("data/settings/bootstrap.json"),
        serde_json::to_vec(&serde_json::json!({
            "schema_version": 1,
            "data": { "custom_dir": dirs.config_dir }
        }))
        .unwrap(),
    )
    .await
    .unwrap();
    tokio::fs::write(
        dirs.settings_dir.join("data/settings/appearance.json"),
        serde_json::to_vec(&serde_json::json!({
            "schema_version": 1,
            "data": { "minimal_home_instance_id": "instance-0" }
        }))
        .unwrap(),
    )
    .await
    .unwrap();
    let pool = crate::state::store::databases::open(&dirs.settings_dir)
        .await
        .unwrap();
    let mut ids = Vec::new();
    for index in 0..7 {
        let id = format!("instance-{index}");
        let content_set = format!("content-{index}");
        let directory = dirs.instances_dir().join(&id);
        tokio::fs::create_dir_all(&directory).await.unwrap();
        tokio::fs::write(
            directory.join("options.txt"),
            b"version:3465\nfullscreen:false\nlang:en_us\n",
        )
        .await
        .unwrap();
        tokio::fs::write(directory.join("command_history.txt"), b"/help\n")
            .await
            .unwrap();
        sqlx::query("INSERT INTO instances(id,path,install_stage,launcher_feature_version,name,created,modified) VALUES(?,?,'installed','none',?,1,1)")
            .bind(&id).bind(&id).bind(&id).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO instance_content_sets(id,instance_id,name,source_kind,status,game_version,loader,created,modified) VALUES(?,?,?,'local','available','1.20.1','vanilla',1,1)")
            .bind(&content_set).bind(&id).bind(&id).execute(&pool).await.unwrap();
        sqlx::query("UPDATE instances SET applied_content_set_id=? WHERE id=?")
            .bind(content_set)
            .bind(&id)
            .execute(&pool)
            .await
            .unwrap();
        ids.push(id);
    }
    let handle = crate::state::store::Handle::new(pool.clone());
    let mut unit = crate::state::store::atom(&handle).await.unwrap();
    for feature in ["game_options", "command_history", "multiplayer_servers"] {
        SyncFeatureSettings::atom_set(
            &mut unit,
            &FeatureSettingsKey {
                feature: feature.into(),
            },
            Some(&FeatureSettingsRecord {
                globally_enabled: false,
                new_instance_default: false,
            }),
        )
        .await
        .unwrap();
        for id in &ids {
            InstanceSyncPreferences::atom_set(
                &mut unit,
                &InstancePreferenceKey {
                    instance_id: id.clone(),
                    feature: feature.into(),
                },
                Some(&InstancePreferenceRecord { enabled: false }),
            )
            .await
            .unwrap();
        }
    }
    unit.commit().await.unwrap();
    drop(handle);
    pool.close().await;
    drop(pool);
    // A separate open leaves the actual domain pool cold instead of reusing
    // the setup's already initialized connections.
    let pool = crate::state::store::databases::open(&dirs.settings_dir)
        .await
        .unwrap();
    let state = crate::state::test_state(dirs, pool).await.unwrap();
    let reads = async {
        for _ in 0..3 {
            assert_eq!(
                crate::state::list_instances(&state.pool).await?.len(),
                7
            );
            let _ = Settings::get(&state.data).await?;
        }
        Ok::<_, crate::Error>(())
    };
    // These two branches are the body of the startup work the launcher runs
    // under the startup write window; the backup branch takes the same window
    // inside `maintain_database_backups`. Reads deliberately stay outside it:
    // the point of the test is that they keep serving while it is held.
    let sync = async {
        let _write_window = state.lock_startup_database_write().await;
        let _guard = state.lock_synced_options().await;
        create_synced_directories(&state).await?;
        apply_pending_changes(&state).await?;
        for id in &ids {
            let metadata =
                crate::state::get_instance(id, &state.pool).await?.unwrap();
            for option in SyncedOption::ALL {
                // Hotbar detachment resolves remote version capabilities via
                // the global State. Its unlink performs no database work; the
                // remaining disabled-option paths run against this local State.
                if option == SyncedOption::CreativeHotbars {
                    continue;
                }
                detach_option(&metadata, option, &state).await?;
            }
        }
        Ok::<_, crate::Error>(())
    };
    let configs = async {
        let _write_window = state.lock_startup_database_write().await;
        for id in &ids {
            crate::state::instances::config_sync::sync_instance(&state, id)
                .await?;
        }
        Ok::<_, crate::Error>(())
    };
    let settings = async {
        Settings::migrate(&state.data).await?;
        Ok::<_, crate::Error>(())
    };
    let backup = async {
        state.maintain_database_backups().await?;
        Ok::<_, crate::Error>(())
    };
    // The budget bounds only how long a merely slow run may take: a genuine
    // deadlock never completes at any size. CI measures this test inside the
    // instrumented coverage build, where the five concurrent branches racing a
    // cold SQLite pool run far slower than they do uninstrumented.
    async fn branch(
        name: &str,
        work: impl std::future::Future<Output = crate::Result<()>>,
    ) -> crate::Result<()> {
        work.await.map_err(|error| error.with_context(name))
    }
    tokio::time::timeout(Duration::from_secs(120), async {
        tokio::try_join!(
            branch("startup instance and settings reads", reads),
            branch("startup disabled sync reconciliation", sync),
            branch("startup recovery copy publication", configs),
            branch("startup settings migration", settings),
            branch("startup database backup", backup),
        )
    })
    .await
    .expect(
        "disabled sync storage and startup services must not deadlock a cold core pool",
    )
    .unwrap();
    state.shutdown().await;
    state.pool.close().await;
}

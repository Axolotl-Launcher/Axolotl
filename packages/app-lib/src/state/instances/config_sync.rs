use crate::state::instances::{
    InstanceLaunchOverridesData, InstanceMetadata,
    adapters::sqlite::config_sync_rows, get_instance,
};
use crate::state::{DirectoryInfo, State};
use crate::util::io;
use chrono::{DateTime, Utc};
use dashmap::DashSet;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::io::ErrorKind;
use std::path::Path;
use std::sync::{Arc, LazyLock};
use std::time::Duration;

pub(crate) const CONFIG_FILE_NAME: &str = "axolotl_config.json";
pub(crate) const CONFIG_FILE_TEMP_NAME: &str = "axolotl_config.json.tmp";

const CONFIG_SCHEMA_VERSION: u32 = 1;
const DIRTY_POLL_INTERVAL: Duration = Duration::from_secs(1);
const RECONCILE_INTERVAL: Duration = Duration::from_secs(60);

static DIRTY_INSTANCES: LazyLock<DashSet<String>> = LazyLock::new(DashSet::new);

#[derive(Clone, Debug, Serialize, Deserialize)]
struct InstanceConfigFile {
    schema_version: u32,
    instance_id: String,
    path: String,
    generated_at: DateTime<Utc>,
    name: String,
    icon_path: Option<String>,
    update_channel: String,
    symlink_target: Option<String>,
    #[serde(default)]
    game_dir_override: Option<String>,
    groups: Vec<String>,
    content_set: InstanceConfigContentSet,
    link: crate::state::instances::InstanceLink,
    launch_overrides: InstanceLaunchOverridesData,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct InstanceConfigContentSet {
    source_kind: crate::state::instances::ContentSourceKind,
    game_version: String,
    protocol_version: Option<u32>,
    loader: crate::state::ModLoader,
    loader_version: Option<String>,
}

pub(crate) fn mark_dirty(instance_id: impl Into<String>) {
    DIRTY_INSTANCES.insert(instance_id.into());
}

pub(crate) async fn run(state: Arc<State>) {
    if let Err(error) = reconcile_all(&state).await {
        tracing::warn!("Failed to reconcile instance config files: {error}");
    }

    let mut dirty_tick = tokio::time::interval(DIRTY_POLL_INTERVAL);
    dirty_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    dirty_tick.tick().await;
    let mut reconcile_tick = tokio::time::interval(RECONCILE_INTERVAL);
    reconcile_tick
        .set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    reconcile_tick.tick().await;

    loop {
        if state.maintenance_gate.wait_until_resumed().await {
            // One catch-up reconcile per observed pause; the periodic branch
            // below keeps the normal reconcile cadence.
            if let Err(error) = reconcile_all(&state).await {
                tracing::warn!(
                    "Failed to reconcile instance config files: {error}"
                );
            }
        }
        tokio::select! {
            _ = dirty_tick.tick() => {
                let mut dirty = Vec::new();
                DIRTY_INSTANCES.retain(|instance_id| {
                    dirty.push(instance_id.clone());
                    false
                });

                for instance_id in dirty {
                    if let Err(error) = sync_instance(&state, &instance_id).await {
                        tracing::warn!(
                            "Failed to sync instance config for {instance_id}: {error}"
                        );
                    }
                }
            }
            _ = reconcile_tick.tick() => {
                if let Err(error) = reconcile_all(&state).await {
                    tracing::warn!(
                        "Failed to reconcile instance config files: {error}"
                    );
                }
            }
        }
    }
}

pub(crate) async fn sync_instance(
    state: &State,
    instance_id: &str,
) -> crate::Result<()> {
    sync_instance_with_dirs(&state.directories, &state.pool, instance_id).await
}

async fn sync_instance_with_dirs(
    dirs: &DirectoryInfo,
    pool: &SqlitePool,
    instance_id: &str,
) -> crate::Result<()> {
    let Some(metadata) = get_instance(instance_id, pool).await? else {
        return Ok(());
    };

    // Directly associated instances have no profile folder of their own and
    // must never write into the linked `.minecraft`; skip them entirely.
    if metadata.instance.linked_dot_minecraft.is_some() {
        return Ok(());
    }

    let config_path = config_path(dirs, &metadata.instance.path);
    let temp_path = config_path.with_file_name(CONFIG_FILE_TEMP_NAME);

    match io::read(&config_path).await {
        Ok(existing) if config_matches(&metadata, &existing) => {
            config_sync_rows::update_config_sync_generated_at(
                instance_id,
                Utc::now().timestamp(),
                pool,
            )
            .await?;
            return Ok(());
        }
        Ok(_) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }

    let bytes = serde_json::to_vec_pretty(&config_file(&metadata, Utc::now()))?;
    write_config_file(&config_path, &temp_path, &bytes).await?;
    config_sync_rows::update_config_sync_generated_at(
        instance_id,
        Utc::now().timestamp(),
        pool,
    )
    .await?;

    Ok(())
}

pub(crate) async fn reconcile_all(state: &State) -> crate::Result<()> {
    reconcile_all_with_dirs(&state.directories, &state.pool).await
}

async fn reconcile_all_with_dirs(
    dirs: &DirectoryInfo,
    pool: &SqlitePool,
) -> crate::Result<()> {
    let rows = config_sync_rows::list_instance_config_sync_rows(pool).await?;

    for row in rows {
        let file_exists = tokio::fs::try_exists(config_path(dirs, &row.path))
            .await
            .unwrap_or(false);
        let needs_sync = !file_exists
            || row.generated_at.is_none()
            || row.config_updated_at.is_none()
            || row.generated_at < row.config_updated_at;

        if !needs_sync {
            continue;
        }

        if let Err(error) =
            sync_instance_with_dirs(dirs, pool, &row.instance_id).await
        {
            tracing::warn!(
                "Failed to sync instance config for {}: {error}",
                row.instance_id
            );
        }
    }

    Ok(())
}

pub(crate) async fn remove_config_file(
    dirs: &DirectoryInfo,
    instance_path: &str,
) -> crate::Result<()> {
    remove_if_exists(&config_path(dirs, instance_path)).await?;
    remove_if_exists(
        &dirs
            .instances_dir()
            .join(instance_path)
            .join(CONFIG_FILE_TEMP_NAME),
    )
    .await?;

    Ok(())
}

fn config_path(
    dirs: &DirectoryInfo,
    instance_path: &str,
) -> std::path::PathBuf {
    dirs.instances_dir()
        .join(instance_path)
        .join(CONFIG_FILE_NAME)
}

fn config_file(
    metadata: &InstanceMetadata,
    generated_at: DateTime<Utc>,
) -> InstanceConfigFile {
    let instance = &metadata.instance;

    InstanceConfigFile {
        schema_version: CONFIG_SCHEMA_VERSION,
        instance_id: instance.id.clone(),
        path: instance.path.clone(),
        generated_at,
        name: instance.name.clone(),
        icon_path: instance.icon_path.clone(),
        update_channel: instance.update_channel.key().to_string(),
        symlink_target: instance.symlink_target.clone(),
        game_dir_override: instance.game_dir_override.clone(),
        groups: metadata.groups.clone(),
        content_set: InstanceConfigContentSet {
            source_kind: metadata.applied_content_set.source_kind,
            game_version: metadata.applied_content_set.game_version.clone(),
            protocol_version: metadata.applied_content_set.protocol_version,
            loader: metadata.applied_content_set.loader,
            loader_version: metadata.applied_content_set.loader_version.clone(),
        },
        link: metadata.link.clone(),
        launch_overrides: InstanceLaunchOverridesData::from(
            &metadata.launch_overrides,
        ),
    }
}

fn config_matches(metadata: &InstanceMetadata, existing: &[u8]) -> bool {
    let Ok(existing_config) =
        serde_json::from_slice::<InstanceConfigFile>(existing)
    else {
        return false;
    };

    serde_json::to_vec_pretty(&config_file(
        metadata,
        existing_config.generated_at,
    ))
    .map(|expected| expected == existing)
    .unwrap_or(false)
}

async fn write_config_file(
    config_path: &Path,
    temp_path: &Path,
    bytes: &[u8],
) -> crate::Result<()> {
    io::write(temp_path, bytes).await?;

    let rename_result = io::retry_windows_sharing_violation(
        config_path,
        "renaming axolotl_config.json",
        || tokio::fs::rename(temp_path, config_path),
    )
    .await;

    if let Err(error) = rename_result {
        let _ = io::remove_file(temp_path).await;
        return Err(io::IOError::with_path(error, config_path).into());
    }

    Ok(())
}

async fn remove_if_exists(path: &Path) -> crate::Result<()> {
    match io::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::instances::{
        ContentSet, ContentSetStatus, ContentSourceKind, CreateInstance,
        EditInstance, Instance, InstanceLaunchOverrides,
        InstanceLaunchOverridesPatch, InstanceLink, create_instance,
        edit_instance,
    };
    use crate::state::{
        InstanceInstallStage, LauncherFeatureVersion, ModLoader, ReleaseChannel,
    };
    use sqlx::sqlite::SqlitePoolOptions;
    use std::time::Duration;
    use tempfile::TempDir;

    async fn test_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory SQLite pool");
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .expect("enable foreign keys");
        sqlx::migrate!().run(&pool).await.expect("migrations");
        pool
    }

    fn test_dirs() -> (TempDir, DirectoryInfo) {
        let temp = tempfile::tempdir().expect("temp dir");
        let dirs = DirectoryInfo {
            settings_dir: temp.path().to_path_buf(),
            config_dir: temp.path().to_path_buf(),
            app_identifier: "test".to_string(),
        };
        std::fs::create_dir_all(dirs.instances_dir()).expect("instances dir");
        (temp, dirs)
    }

    async fn insert_instance(
        dirs: &DirectoryInfo,
        pool: &SqlitePool,
        instance_id: &str,
        instance_path: &str,
        name: &str,
    ) {
        std::fs::create_dir_all(dirs.instances_dir().join(instance_path))
            .expect("instance dir");
        let now = Utc::now().timestamp();
        let content_set_id = format!("content-set:{instance_id}");

        sqlx::query(
            "
			INSERT INTO instances (
				id,
				path,
				applied_content_set_id,
				install_stage,
				launcher_feature_version,
				update_channel,
				name,
				icon_path,
				symlink_target,
				created,
				modified,
				last_played,
				pinned_at,
				submitted_time_played,
				recent_time_played
			)
			VALUES (?, ?, ?, ?, ?, ?, ?, NULL, NULL, ?, ?, NULL, NULL, 0, 0)
			",
        )
        .bind(instance_id)
        .bind(instance_path)
        .bind(&content_set_id)
        .bind("not_installed")
        .bind("migrated_launch_hooks")
        .bind("release")
        .bind(name)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await
        .expect("insert instance");

        sqlx::query(
            "
			INSERT INTO instance_content_sets (
				id,
				instance_id,
				name,
				source_kind,
				status,
				game_version,
				protocol_version,
				loader,
				loader_version,
				revision,
				created,
				modified
			)
			VALUES (?, ?, 'Default', 'local', 'available', '1.21.4',
				NULL, 'vanilla', NULL, 0, ?, ?)
			",
        )
        .bind(&content_set_id)
        .bind(instance_id)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await
        .expect("insert content set");

        sqlx::query(
            "INSERT INTO instance_links (instance_id, link_kind)
			 VALUES (?, 'unmanaged')",
        )
        .bind(instance_id)
        .execute(pool)
        .await
        .expect("insert link");

        sqlx::query(
            "INSERT INTO instance_launch_overrides (instance_id, overrides)
			 VALUES (?, jsonb('{}'))",
        )
        .bind(instance_id)
        .execute(pool)
        .await
        .expect("insert launch overrides");
    }

    fn sample_metadata() -> InstanceMetadata {
        let now = Utc::now();
        let instance = Instance {
            id: "local:serialization".to_string(),
            path: "Serialized Instance".to_string(),
            applied_content_set_id: Some("content-set:serialization".into()),
            install_stage: InstanceInstallStage::Installed,
            launcher_feature_version: LauncherFeatureVersion::MOST_RECENT,
            update_channel: ReleaseChannel::Beta,
            name: "Serialized Instance".to_string(),
            icon_path: Some(r"C:\absolute\icon.png".to_string()),
            symlink_target: Some(r"Z:\target".to_string()),
            linked_launcher: None,
            linked_launcher_root: None,
            linked_dot_minecraft: None,
            linked_version_id: None,
            linked_version_json_path: None,
            linked_game_dir_mode: None,
            game_dir_override: Some(r"D:\Games\.minecraft".to_string()),
            created: now,
            modified: now,
            last_played: Some(now),
            pinned_at: Some(now),
            submitted_time_played: 10,
            recent_time_played: 20,
        };
        let mut launch_overrides =
            InstanceLaunchOverrides::empty(instance.id.clone());
        launch_overrides.java_path = Some(r"C:\Java\bin\java.exe".to_string());

        InstanceMetadata {
            synced_options: Default::default(),
            instance,
            applied_content_set: ContentSet {
                id: "content-set:serialization".to_string(),
                instance_id: "local:serialization".to_string(),
                name: "Default".to_string(),
                source_kind: ContentSourceKind::ModrinthModpack,
                status: ContentSetStatus::Available,
                game_version: "1.21.4".to_string(),
                protocol_version: Some(767),
                loader: ModLoader::Fabric,
                loader_version: Some("0.16.9".to_string()),
                revision: 5,
                created: now,
                modified: now,
            },
            link: InstanceLink::ModrinthModpack {
                project_id: "project".to_string(),
                version_id: "version".to_string(),
            },
            groups: vec!["Group A".to_string()],
            launch_overrides,
            loader_components: Vec::new(),
        }
    }

    fn read_config(
        dirs: &DirectoryInfo,
        instance_path: &str,
    ) -> InstanceConfigFile {
        serde_json::from_slice(
            &std::fs::read(config_path(dirs, instance_path))
                .expect("read config"),
        )
        .expect("parse config")
    }

    #[test]
    fn serialization_round_trip_preserves_metadata_and_paths() {
        let config = config_file(&sample_metadata(), Utc::now());
        let bytes = serde_json::to_vec_pretty(&config).unwrap();
        let roundtrip: InstanceConfigFile =
            serde_json::from_slice(&bytes).unwrap();

        assert_eq!(roundtrip.schema_version, 1);
        assert_eq!(roundtrip.instance_id, "local:serialization");
        assert_eq!(roundtrip.path, "Serialized Instance");
        assert_eq!(roundtrip.name, "Serialized Instance");
        assert_eq!(roundtrip.update_channel, "beta");
        assert_eq!(
            roundtrip.icon_path.as_deref(),
            Some(r"C:\absolute\icon.png")
        );
        assert_eq!(roundtrip.symlink_target.as_deref(), Some(r"Z:\target"));
        assert_eq!(
            roundtrip.game_dir_override.as_deref(),
            Some(r"D:\Games\.minecraft")
        );
        assert_eq!(roundtrip.groups, vec!["Group A".to_string()]);
        assert_eq!(
            roundtrip.launch_overrides.java_path.as_deref(),
            Some(r"C:\Java\bin\java.exe")
        );
        match roundtrip.link {
            InstanceLink::ModrinthModpack {
                project_id,
                version_id,
            } => {
                assert_eq!(project_id, "project");
                assert_eq!(version_id, "version");
            }
            other => panic!("unexpected link: {other:?}"),
        }

        let text = String::from_utf8(bytes).unwrap();
        assert!(!text.contains("last_played"));
        assert!(!text.contains("pinned_at"));
        assert!(!text.contains("revision"));
        assert!(!text.contains("install_stage"));
    }

    #[tokio::test]
    async fn create_instance_marks_state_and_writes_config_file() {
        let (temp, dirs) = test_dirs();
        let pool = test_pool().await;
        let state_dirs = DirectoryInfo {
            settings_dir: dirs.settings_dir.clone(),
            config_dir: dirs.config_dir.clone(),
            app_identifier: dirs.app_identifier.clone(),
        };
        let state = crate::state::test_state(state_dirs, pool.clone())
            .await
            .expect("test state");

        let instance = create_instance(
            CreateInstance {
                name: "My Instance".to_string(),
                path: None,
                game_version: "1.21.4".to_string(),
                loader: ModLoader::Vanilla,
                loader_version: None,
                icon_path: None,
                link: InstanceLink::Unmanaged,
                symlink_target: None,
                game_dir_override: None,
            },
            &state,
        )
        .await
        .expect("create instance");

        let state_rows: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM instance_config_sync_state
			 WHERE instance_id = ?",
        )
        .bind(&instance.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(state_rows, 1);

        sync_instance(&state, &instance.id)
            .await
            .expect("sync config");
        let config = read_config(&dirs, &instance.path);
        assert_eq!(config.instance_id, instance.id);
        assert_eq!(config.name, "My Instance");

        drop(state);
        drop(temp);
    }

    #[tokio::test]
    async fn edit_instance_updates_name_and_java_path() {
        let (temp, dirs) = test_dirs();
        let pool = test_pool().await;
        insert_instance(
            &dirs,
            &pool,
            "local:edit",
            "Edit Instance",
            "Old Name",
        )
        .await;
        config_sync_rows::upsert_config_updated_at("local:edit", &pool)
            .await
            .unwrap();
        sync_instance_with_dirs(&dirs, &pool, "local:edit")
            .await
            .unwrap();

        edit_instance(
            "local:edit",
            EditInstance {
                name: Some("New Name".to_string()),
                launch_overrides: Some(InstanceLaunchOverridesPatch {
                    java_path: Some(Some(r"C:\Java\java.exe".to_string())),
                    ..InstanceLaunchOverridesPatch::default()
                }),
                ..EditInstance::default()
            },
            &pool,
        )
        .await
        .expect("edit instance");
        sync_instance_with_dirs(&dirs, &pool, "local:edit")
            .await
            .unwrap();

        let config = read_config(&dirs, "Edit Instance");
        assert_eq!(config.name, "New Name");
        assert_eq!(
            config.launch_overrides.java_path.as_deref(),
            Some(r"C:\Java\java.exe")
        );

        drop(temp);
    }

    #[tokio::test]
    async fn unchanged_content_does_not_rewrite_file() {
        let (temp, dirs) = test_dirs();
        let pool = test_pool().await;
        insert_instance(
            &dirs,
            &pool,
            "local:unchanged",
            "Unchanged Instance",
            "Name",
        )
        .await;
        config_sync_rows::upsert_config_updated_at("local:unchanged", &pool)
            .await
            .unwrap();
        sync_instance_with_dirs(&dirs, &pool, "local:unchanged")
            .await
            .unwrap();

        let path = config_path(&dirs, "Unchanged Instance");
        let before_bytes = std::fs::read(&path).unwrap();
        let before_modified =
            std::fs::metadata(&path).unwrap().modified().unwrap();
        sqlx::query(
            "UPDATE instance_config_sync_state
			 SET config_updated_at = generated_at + 100
			 WHERE instance_id = ?",
        )
        .bind("local:unchanged")
        .execute(&pool)
        .await
        .unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;

        sync_instance_with_dirs(&dirs, &pool, "local:unchanged")
            .await
            .unwrap();

        let after_bytes = std::fs::read(&path).unwrap();
        let after_modified =
            std::fs::metadata(&path).unwrap().modified().unwrap();
        assert_eq!(before_bytes, after_bytes);
        assert_eq!(before_modified, after_modified);

        drop(temp);
    }

    #[tokio::test]
    async fn reconcile_all_rebuilds_missing_file() {
        let (temp, dirs) = test_dirs();
        let pool = test_pool().await;
        insert_instance(
            &dirs,
            &pool,
            "local:rebuild",
            "Rebuild Instance",
            "Name",
        )
        .await;
        config_sync_rows::upsert_config_updated_at("local:rebuild", &pool)
            .await
            .unwrap();

        reconcile_all_with_dirs(&dirs, &pool).await.unwrap();

        assert!(config_path(&dirs, "Rebuild Instance").is_file());
        let generated_at = config_sync_rows::get_config_sync_generated_at(
            "local:rebuild",
            &pool,
        )
        .await
        .unwrap();
        assert!(generated_at.is_some());

        drop(temp);
    }

    #[tokio::test]
    async fn reconcile_all_skips_when_state_is_latest() {
        let (temp, dirs) = test_dirs();
        let pool = test_pool().await;
        insert_instance(
            &dirs,
            &pool,
            "local:latest",
            "Latest Instance",
            "Name",
        )
        .await;
        config_sync_rows::upsert_config_updated_at("local:latest", &pool)
            .await
            .unwrap();
        sync_instance_with_dirs(&dirs, &pool, "local:latest")
            .await
            .unwrap();
        let path = config_path(&dirs, "Latest Instance");
        let before_modified =
            std::fs::metadata(&path).unwrap().modified().unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;

        reconcile_all_with_dirs(&dirs, &pool).await.unwrap();

        let after_modified =
            std::fs::metadata(&path).unwrap().modified().unwrap();
        assert_eq!(before_modified, after_modified);

        drop(temp);
    }

    #[tokio::test]
    async fn corrupt_json_is_rewritten() {
        let (temp, dirs) = test_dirs();
        let pool = test_pool().await;
        insert_instance(
            &dirs,
            &pool,
            "local:corrupt",
            "Corrupt Instance",
            "Name",
        )
        .await;
        config_sync_rows::upsert_config_updated_at("local:corrupt", &pool)
            .await
            .unwrap();
        std::fs::write(
            config_path(&dirs, "Corrupt Instance"),
            b"not valid json",
        )
        .unwrap();

        sync_instance_with_dirs(&dirs, &pool, "local:corrupt")
            .await
            .unwrap();

        read_config(&dirs, "Corrupt Instance");

        drop(temp);
    }

    #[tokio::test]
    async fn deletion_cascades_state_and_removes_config_file() {
        let (temp, dirs) = test_dirs();
        let pool = test_pool().await;
        insert_instance(
            &dirs,
            &pool,
            "local:delete",
            "Delete Instance",
            "Name",
        )
        .await;
        config_sync_rows::upsert_config_updated_at("local:delete", &pool)
            .await
            .unwrap();
        sync_instance_with_dirs(&dirs, &pool, "local:delete")
            .await
            .unwrap();
        let path = config_path(&dirs, "Delete Instance");
        let temp_path = path.with_file_name(CONFIG_FILE_TEMP_NAME);
        std::fs::write(&temp_path, b"tmp").unwrap();

        remove_config_file(&dirs, "Delete Instance").await.unwrap();
        assert!(!path.exists());
        assert!(!temp_path.exists());

        sqlx::query("DELETE FROM instances WHERE id = ?")
            .bind("local:delete")
            .execute(&pool)
            .await
            .unwrap();
        let state_rows: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM instance_config_sync_state
			 WHERE instance_id = ?",
        )
        .bind("local:delete")
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(state_rows, 0);

        drop(temp);
    }
}

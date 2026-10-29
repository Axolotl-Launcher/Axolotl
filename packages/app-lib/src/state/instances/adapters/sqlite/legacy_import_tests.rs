use super::*;
use crate::state::instances::adapters::sqlite::instance_rows;

async fn pool_with_legacy_tables() -> sqlx::SqlitePool {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("in-memory SQLite pool");
    sqlx::raw_sql(
        "
        CREATE TABLE instances (
            id TEXT PRIMARY KEY,
            path TEXT NOT NULL,
            applied_content_set_id TEXT NULL,
            install_stage TEXT NOT NULL,
            launcher_feature_version TEXT NOT NULL,
            update_channel TEXT NOT NULL,
            name TEXT NOT NULL,
            icon_path TEXT NULL,
            created INTEGER NOT NULL,
            modified INTEGER NOT NULL,
            last_played INTEGER NULL,
            submitted_time_played INTEGER NOT NULL,
            recent_time_played INTEGER NOT NULL
        );
        CREATE TABLE instance_content_sets (
            id TEXT PRIMARY KEY,
            instance_id TEXT NOT NULL,
            name TEXT NOT NULL,
            source_kind TEXT NOT NULL,
            status TEXT NOT NULL,
            game_version TEXT NOT NULL,
            protocol_version INTEGER NULL,
            loader TEXT NOT NULL,
            loader_version TEXT NULL,
            created INTEGER NOT NULL,
            modified INTEGER NOT NULL
        );
        CREATE TABLE instance_links (
            instance_id TEXT PRIMARY KEY,
            link_kind TEXT NOT NULL,
            modrinth_project_id TEXT NULL,
            modrinth_version_id TEXT NULL,
            server_project_id TEXT NULL,
            content_project_id TEXT NULL,
            content_version_id TEXT NULL,
            hosting_server_id TEXT NULL,
            hosting_instance_ids TEXT NULL,
            hosting_active_instance_id TEXT NULL,
            shared_instance_id TEXT NULL,
            imported_name TEXT NULL,
            imported_version_number TEXT NULL,
            imported_filename TEXT NULL
        );
        CREATE TABLE instance_groups (
            instance_id TEXT NOT NULL,
            group_name TEXT NOT NULL,
            PRIMARY KEY (instance_id, group_name)
        );
        CREATE TABLE instance_launch_overrides (
            instance_id TEXT PRIMARY KEY,
            overrides TEXT NOT NULL
        );
        ",
    )
    .execute(&pool)
    .await
    .expect("legacy launcher tables");
    pool
}

fn profile() -> LegacyProfile {
    LegacyProfile {
        path: "profile-path".to_string(),
        install_stage: InstanceInstallStage::Installed,
        name: "Profile".to_string(),
        icon_path: Some("icon.png".to_string()),
        game_version: "1.21.5".to_string(),
        loader: ModLoader::Fabric,
        loader_version: Some("0.16.0".to_string()),
        groups: vec!["Survival".to_string(), "Survival".to_string()],
        link: LegacyLink::ModrinthModpack {
            project_id: "project".to_string(),
            version_id: "version".to_string(),
        },
        created: DateTime::from_timestamp(1_700_000_000, 0).unwrap(),
        modified: DateTime::from_timestamp(1_700_000_100, 0).unwrap(),
        last_played: None,
        submitted_time_played: 3600,
        recent_time_played: 60,
        java_path: Some("/java".to_string()),
        extra_launch_args: Some(vec!["-test".to_string()]),
        custom_env_vars: None,
        memory: None,
        force_fullscreen: None,
        game_resolution: None,
        hooks: Hooks::default(),
    }
}

#[tokio::test]
async fn a_profile_becomes_an_instance_with_one_content_set_and_link() {
    let pool = pool_with_legacy_tables().await;
    let mut unit = pool.begin().await.unwrap();
    import_profile(&profile(), &mut unit).await.unwrap();
    unit.commit().await.unwrap();

    let (instance_id, install_stage, launcher_feature_version, update_channel): (
        String,
        String,
        String,
        String,
    ) = sqlx::query_as(
        "SELECT id, install_stage, launcher_feature_version, update_channel
         FROM instances",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(install_stage, InstanceInstallStage::Installed.as_str());
    assert_eq!(
        launcher_feature_version,
        LauncherFeatureVersion::None.as_str()
    );
    assert_eq!(update_channel, ReleaseChannel::Release.key());

    let (name, source_kind, status, loader, loader_version): (
        String,
        String,
        String,
        String,
        Option<String>,
    ) = sqlx::query_as(
        "SELECT name, source_kind, status, loader, loader_version
         FROM instance_content_sets
         WHERE instance_id = ?",
    )
    .bind(&instance_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(name, "Default");
    assert_eq!(source_kind, "modrinth_modpack");
    assert_eq!(status, "available");
    assert_eq!(loader, ModLoader::Fabric.as_str());
    assert_eq!(loader_version.as_deref(), Some("0.16.0"));

    let (link_kind, project_id, version_id): (String, String, String) =
        sqlx::query_as(
            "SELECT link_kind, modrinth_project_id, modrinth_version_id
             FROM instance_links
             WHERE instance_id = ?",
        )
        .bind(&instance_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(link_kind, "modrinth_modpack");
    assert_eq!(project_id, "project");
    assert_eq!(version_id, "version");
}

#[tokio::test]
async fn the_same_group_name_is_written_once() {
    let pool = pool_with_legacy_tables().await;
    let mut unit = pool.begin().await.unwrap();
    import_profile(&profile(), &mut unit).await.unwrap();
    unit.commit().await.unwrap();

    let groups: Vec<String> =
        sqlx::query_scalar("SELECT group_name FROM instance_groups")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(groups, vec!["Survival".to_string()]);
}

#[tokio::test]
async fn the_launch_overrides_read_back_through_the_instance_half() {
    let pool = pool_with_legacy_tables().await;
    let mut unit = pool.begin().await.unwrap();
    import_profile(&profile(), &mut unit).await.unwrap();
    unit.commit().await.unwrap();

    let instance_id: String = sqlx::query_scalar("SELECT id FROM instances")
        .fetch_one(&pool)
        .await
        .unwrap();
    let overrides =
        instance_rows::get_instance_launch_overrides(&instance_id, &pool)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(overrides.instance_id, instance_id);
    assert_eq!(overrides.java_path.as_deref(), Some("/java"));
}

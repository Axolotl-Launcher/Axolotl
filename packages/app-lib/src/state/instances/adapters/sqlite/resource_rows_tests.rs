use super::*;
use crate::resources::*;
use sqlx::{migrate::Migrator, sqlite::SqlitePoolOptions};
use std::borrow::Cow;

const VERSION: i64 = 20261011120000;
const MIGRATOR: Migrator = sqlx::migrate!();

async fn previous_database() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let previous = Migrator {
        migrations: Cow::Owned(
            MIGRATOR
                .iter()
                .filter(|migration| migration.version < VERSION)
                .cloned()
                .collect(),
        ),
        ..Migrator::DEFAULT
    };
    previous.run(&pool).await.unwrap();
    pool
}

async fn upgrade(pool: &SqlitePool) {
    MIGRATOR.run(pool).await.unwrap();
    assert!(
        sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(pool)
            .await
            .unwrap()
            .is_empty()
    );
}

async fn business_rows(pool: &SqlitePool) {
    sqlx::raw_sql(r#"
        INSERT INTO instances(id, path, install_stage, launcher_feature_version, name, created, modified)
            VALUES ('instance', '/instance', 'installed', '1', 'Example', 10, 20);
        INSERT INTO instance_content_sets(id, instance_id, name, source_kind, status, game_version, loader, created, modified)
            VALUES ('root', 'instance', 'Root', 'local', 'applied', '1.20.1', 'fabric', 10, 20),
                   ('world', 'instance', 'World', 'local', 'applied', '1.20.1', 'fabric', 10, 20);
        INSERT INTO instance_files(id, instance_id, relative_path, file_name, enabled, sha1, size, missing, added_at, modified_at)
            VALUES ('file-a', 'instance', 'mods/custom-a.jar', 'custom-a.jar', 0, 'hash', 100, 0, 10, 20),
                   ('file-b', 'instance', 'saves/world/datapacks/custom-b.zip', 'custom-b.zip', 1, 'hash', 100, 0, 10, 20);
        INSERT INTO instance_content_entries(id, instance_id, content_set_id, file_id, project_type, source_kind, server_requirement, client_requirement, enabled, added_at, modified_at, ownership_kind)
            VALUES ('a', 'instance', 'root', 'file-a', 'mod', 'modrinth', 'unknown', 'unknown', 0, 11, 21, 'user_added'),
                   ('b', 'instance', 'world', 'file-b', 'datapack', 'curseforge', 'unknown', 'unknown', 1, 12, 22, 'pack_managed'),
                   ('ambiguous', 'instance', 'root', NULL, 'mod', 'modrinth', 'unknown', 'unknown', 1, 13, 23, 'user_added');
        INSERT INTO instance_content_provider_refs(content_entry_id, provider, provider_project_id, provider_release_id, provider_file_id, is_origin)
            VALUES ('a', 'modrinth', '12345678', 'releaseA', 'artifact-a', 1),
                   ('a', 'curseforge', '12345678', '101', '101', 0),
                   ('b', 'modrinth', '12345678', 'releaseB', 'artifact-b', 1),
                   ('ambiguous', 'modrinth', 'abcdefgh', NULL, NULL, 0),
                   ('ambiguous', 'curseforge', '76543210', NULL, NULL, 0);
        INSERT INTO content_favorites(provider, project_id, content_type, saved_at)
            VALUES ('modrinth', '12345678', 'mod', 123),
                   ('curseforge', '12345678', 'mod', 456),
                   ('modrinth', 'slug-is-not-an-id', 'mod', 789),
                   ('curseforge', '000123', 'mod', 800),
                   ('mcarchive', '12345678-1234-5678-1234-567812345678', 'mod', 801),
                   ('mcarchive', 'not-a-uuid', 'mod', 802);
        INSERT INTO instance_pack_members(id, content_set_id, content_entry_id, member_key, project_type, expected_relative_path, provider, provider_project_id, provider_release_id, required, created_at, modified_at)
            VALUES ('member', 'world', 'b', 'key', 'datapack', 'saves/world/datapacks/custom-b.zip', 'curseforge', '87654321', '102', 1, 30, 40);
        INSERT INTO install_jobs(id, instance_id, kind, status, state, created, modified)
            VALUES ('frozen', 'instance', 'install_content', 'queued',
                '{"request":{"type":"install_content","project_id":"12345678","version_id":"releaseA"}}', 1, 2);
        INSERT INTO cache(id, data_type, data, expires) VALUES ('bad', 'project', 'malformed', 0);
    "#).execute(pool).await.unwrap();
}

fn project(source: SourceId, id: &str) -> SourceProjectRef {
    SourceProjectRef {
        source,
        project_id: SourceProjectId::new(id).unwrap(),
    }
}

#[tokio::test]
async fn batched_identity_lookup_keeps_provider_keys_and_missing_projects_across_chunks()
 {
    let pool = previous_database().await;
    upgrade(&pool).await;
    let store = ResourceStore::new(pool.clone());
    let mr = project(SourceId::modrinth(), "12345678");
    let cf = project(SourceId::curseforge(), "12345678");
    let mr_id = store
        .ensure_project(&mr, &[ResourceType::Mod])
        .await
        .unwrap();
    let cf_id = store
        .ensure_project(&cf, &[ResourceType::Mod])
        .await
        .unwrap();
    let mut projects = (1..=801)
        .map(|id| project(SourceId::curseforge(), &id.to_string()))
        .collect::<Vec<_>>();
    projects.extend([mr.clone(), cf.clone(), mr.clone()]);
    let found = store.lookup_many(&projects).await.unwrap();
    assert_eq!(found.len(), 2);
    assert_eq!(found[&mr], mr_id);
    assert_eq!(found[&cf], cf_id);
    assert_ne!(mr_id, cf_id);
    assert!(store.lookup_many(&[]).await.unwrap().is_empty());
    assert!(
        store
            .lookup_many(&[project(SourceId::modrinth(), "a-slug")])
            .await
            .is_err()
    );
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM resource_source_projects")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 2);
    pool.close().await;
}

fn metadata(project: SourceProjectRef) -> SourceProjectMetadata {
    SourceProjectMetadata {
        environments: vec![],
        categories: vec![],
        game_versions: vec![],
        loaders: vec![],
        gallery: vec![],
        project,
        slug: Some("same-name".into()),
        title: "Same name".into(),
        summary: String::new(),
        icon_url: None,
        types: vec![ResourceType::Mod],
        authors: vec![],
        license: None,
        downloads: None,
        followers: None,
        created_at: None,
        project_published_at: None,
        updated_at: None,
        first_published_at: None,
        latest_published_at: None,
    }
}

fn artifact(project: SourceProjectRef, hash: &str) -> ResourceArtifact {
    let release_id = if project.source == SourceId::curseforge() {
        "101"
    } else {
        "release1"
    };
    ResourceArtifact {
        reference: SourceArtifactRef {
            release: SourceReleaseRef {
                project,
                release_id: SourceReleaseId::new(release_id).unwrap(),
            },
            artifact_id: SourceArtifactId::new("file").unwrap(),
        },
        file_name: Some("mod.jar".into()),
        role: ArtifactRole::Primary,
        integrity: ArtifactIntegrity {
            size: Some(100),
            hashes: vec![ContentHash {
                algorithm: HashAlgorithm::Sha256,
                value: hash.repeat(64),
            }],
        },
        acquisition: ArtifactAcquisition::Unavailable,
        available: None,
        dependencies: vec![],
    }
}

#[tokio::test]
async fn fresh_identity_database_and_exact_previous_upgrade_preserve_business_facts()
 {
    let fresh = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    upgrade(&fresh).await;
    let fresh_store = ResourceStore::new(fresh);
    assert_eq!(
        fresh_store.preferred_source().await.unwrap(),
        SourceId::modrinth()
    );
    assert!(
        fresh_store
            .lookup(&project(SourceId::modrinth(), "12345678"))
            .await
            .unwrap()
            .is_none()
    );

    let pool = previous_database().await;
    business_rows(&pool).await;
    let state: String = sqlx::query_scalar(
        "SELECT state FROM install_jobs WHERE id = 'frozen'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    upgrade(&pool).await;
    let store = ResourceStore::new(pool.clone());
    let mr = store
        .lookup(&project(SourceId::modrinth(), "12345678"))
        .await
        .unwrap()
        .unwrap();
    let cf = store
        .lookup(&project(SourceId::curseforge(), "12345678"))
        .await
        .unwrap()
        .unwrap();
    assert_ne!(mr, cf);
    let entries = store.entry_resources("instance").await.unwrap();
    assert_eq!(entries["a"], mr);
    assert_eq!(entries["b"], mr);
    assert!(!entries.contains_key("ambiguous"));
    assert_ne!(
        store.pack_resources("instance").await.unwrap()["member"],
        mr
    );
    let rows = crate::state::content_favorites::list(&pool).await.unwrap();
    assert_eq!(
        rows.iter()
            .find(|row| row.project_id == "12345678"
                && row.provider.as_str() == "modrinth")
            .unwrap()
            .saved_at,
        123
    );
    assert!(
        rows.iter()
            .filter(|row| matches!(
                row.project_id.as_str(),
                "slug-is-not-an-id" | "000123" | "not-a-uuid"
            ))
            .all(|row| row.resource_id.is_none())
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM install_jobs WHERE id = 'frozen'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        state
    );
    let file = sqlx::query("SELECT id, relative_path, enabled, added_at FROM instance_files WHERE id = 'file-a'").fetch_one(&pool).await.unwrap();
    assert_eq!(file.get::<String, _>("relative_path"), "mods/custom-a.jar");
    assert!(!file.get::<bool, _>("enabled"));
    assert_eq!(file.get::<i64, _>("added_at"), 10);
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT ownership_kind FROM instance_content_entries WHERE id = 'b'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        "pack_managed"
    );
}

#[tokio::test]
async fn legacy_writes_are_atomic_and_follow_origin_changes_and_deletion() {
    let pool = previous_database().await;
    upgrade(&pool).await;
    business_rows(&pool).await;
    let store = ResourceStore::new(pool.clone());
    for _ in 0..2 {
        let favorite = crate::state::content_favorites::add(
            crate::state::ContentFavoriteInput {
                provider: crate::state::ContentFavoriteProvider::Modrinth,
                project_id: "12345678".into(),
                content_type: crate::state::ContentFavoriteType::Mod,
            },
            &pool,
        )
        .await
        .unwrap();
        assert_eq!(favorite.saved_at, 123);
        assert!(favorite.resource_id.is_some());
    }
    assert_eq!(store.entry_resources("instance").await.unwrap().len(), 2);
    let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM resources")
        .fetch_one(&pool)
        .await
        .unwrap();
    let mut transaction = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO content_favorites VALUES ('modrinth', 'rollback', 'mod', 999)").execute(&mut *transaction).await.unwrap();
    transaction.rollback().await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM resources")
            .fetch_one(&pool)
            .await
            .unwrap(),
        before
    );
    assert!(
        store
            .lookup(&project(SourceId::modrinth(), "rollback"))
            .await
            .unwrap()
            .is_none()
    );

    sqlx::query("UPDATE instance_content_provider_refs SET is_origin = 0 WHERE content_entry_id = 'a'").execute(&pool).await.unwrap();
    assert!(
        !store
            .entry_resources("instance")
            .await
            .unwrap()
            .contains_key("a")
    );
    sqlx::query("UPDATE instance_content_provider_refs SET is_origin = 1 WHERE content_entry_id = 'a' AND provider = 'curseforge'").execute(&pool).await.unwrap();
    assert_eq!(
        store.entry_resources("instance").await.unwrap()["a"],
        store
            .lookup(&project(SourceId::curseforge(), "12345678"))
            .await
            .unwrap()
            .unwrap()
    );
    sqlx::query("UPDATE instance_content_provider_refs SET provider_project_id = 'broken-origin' WHERE content_entry_id = 'a' AND is_origin = 1").execute(&pool).await.unwrap();
    assert!(
        !store
            .entry_resources("instance")
            .await
            .unwrap()
            .contains_key("a")
    );
    sqlx::query("DELETE FROM instances WHERE id = 'instance'")
        .execute(&pool)
        .await
        .unwrap();
    assert!(store.entry_resources("instance").await.unwrap().is_empty());
    assert!(store.pack_resources("instance").await.unwrap().is_empty());
    assert!(
        sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&pool)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn stable_catalog_survives_main_database_reopen_and_repeated_migration() {
    let directory = tempfile::tempdir().unwrap();
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(directory.path().join("app.db"))
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options.clone())
        .await
        .unwrap();
    upgrade(&pool).await;
    let store = ResourceStore::new(pool.clone());
    let reference =
        project(SourceId::new("future-source").unwrap(), "opaque-project");
    let id = store
        .ensure_project(&reference, &[ResourceType::Mod])
        .await
        .unwrap();
    store.add_alias(&reference, "saved-locator").await.unwrap();
    store.set_preferred_source(&reference.source).await.unwrap();
    upgrade(&pool).await;
    assert_eq!(store.ensure_project(&reference, &[]).await.unwrap(), id);
    pool.close().await;
    let reopened = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap();
    let store = ResourceStore::new(reopened.clone());
    assert_eq!(store.lookup(&reference).await.unwrap(), Some(id.clone()));
    assert_eq!(store.projects(&id).await.unwrap(), vec![reference.clone()]);
    assert_eq!(
        store
            .alias_projects(&reference.source, "saved-locator")
            .await
            .unwrap(),
        vec![reference.clone()]
    );
    assert_eq!(store.preferred_source().await.unwrap(), reference.source);
    reopened.close().await;
}

#[tokio::test]
async fn confirmed_merges_keep_old_ids_alias_ambiguity_favorites_and_cache_independence()
 {
    let pool = previous_database().await;
    business_rows(&pool).await;
    upgrade(&pool).await;
    let store = ResourceStore::new(pool.clone());
    let left = metadata(project(SourceId::modrinth(), "12345678"));
    let right = metadata(project(SourceId::curseforge(), "12345678"));
    let left_id = store.lookup(&left.project).await.unwrap().unwrap();
    let right_id = store.lookup(&right.project).await.unwrap().unwrap();
    let mut a = artifact(left.project.clone(), "a");
    let b = artifact(right.project.clone(), "a");
    a.role = ArtifactRole::Attachment;
    assert!(store.confirm_projects(&left, &right, &a, &b).await.is_err());
    a.role = ArtifactRole::Primary;
    a.integrity.size = None;
    assert!(store.confirm_projects(&left, &right, &a, &b).await.is_err());
    a.integrity.size = Some(100);
    a.integrity.hashes[0].value = "a".into();
    assert!(store.confirm_projects(&left, &right, &a, &b).await.is_err());
    a.integrity.hashes[0].value = "a".repeat(64);
    let mut incompatible = right.clone();
    incompatible.types = vec![ResourceType::Shader];
    assert!(
        store
            .confirm_projects(&left, &incompatible, &a, &b)
            .await
            .is_err()
    );
    let winner = store.confirm_projects(&left, &right, &a, &b).await.unwrap();
    store.confirm_projects(&left, &right, &a, &b).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM resource_identity_evidence"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    assert_eq!(store.resolve(&left_id).await.unwrap(), Some(winner.clone()));
    assert_eq!(
        store.resolve(&right_id).await.unwrap(),
        Some(winner.clone())
    );
    let extra = metadata(project(SourceId::modrinth(), "abcdefgh"));
    let extra_id = store
        .ensure_project(&extra.project, &extra.types)
        .await
        .unwrap();
    let c = artifact(extra.project.clone(), "a");
    let winner = store
        .confirm_projects(&right, &extra, &b, &c)
        .await
        .unwrap();
    for old in [&left_id, &right_id, &extra_id] {
        assert_eq!(store.resolve(old).await.unwrap(), Some(winner.clone()));
        assert_eq!(store.projects(old).await.unwrap().len(), 3);
    }
    store.add_alias(&left.project, "same-slug").await.unwrap();
    store.add_alias(&extra.project, "same-slug").await.unwrap();
    assert_eq!(
        store
            .alias_projects(&SourceId::modrinth(), "same-slug")
            .await
            .unwrap()
            .len(),
        2
    );
    let favorites = crate::state::content_favorites::list(&pool).await.unwrap();
    assert_eq!(
        favorites
            .iter()
            .filter(|row| row.project_id == "12345678")
            .map(|row| row.resource_id.as_deref())
            .collect::<Vec<_>>(),
        vec![Some(winner.as_str()), Some(winner.as_str())]
    );
    assert_eq!(
        favorites
            .iter()
            .filter(|row| row.project_id == "12345678")
            .map(|row| row.saved_at)
            .collect::<Vec<_>>(),
        vec![456, 123]
    );
    let future = SourceId::new("future-source").unwrap();
    store.set_preferred_source(&future).await.unwrap();
    let directory = tempfile::tempdir().unwrap();
    let cache = crate::resources::cache::ResourceCache::open(
        directory.path().join("resource-cache.db"),
    )
    .await;
    cache.clear().await.unwrap();
    cache.close().await;
    assert_eq!(store.preferred_source().await.unwrap(), future);
    assert_eq!(store.lookup(&left.project).await.unwrap(), Some(winner));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM resource_identity_evidence"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        2
    );
    assert!(
        sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&pool)
            .await
            .unwrap()
            .is_empty()
    );
}

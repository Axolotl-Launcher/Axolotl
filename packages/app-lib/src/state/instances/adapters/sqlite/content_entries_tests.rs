use super::*;
use crate::state::instances::adapters::sqlite::test_support::test_pool;
use crate::state::{
    ContentProvider, ContentProviderRef, ModrinthProjectId, ModrinthVersionId,
    ReleaseChannel,
};

#[tokio::test]
async fn in_place_upgrade_lists_new_origin_release_before_old_ref() {
    let pool = test_pool().await;
    sqlx::raw_sql(
        "
        CREATE TABLE instance_content_provider_refs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            content_entry_id TEXT NOT NULL,
            provider TEXT NOT NULL,
            provider_project_id TEXT NOT NULL,
            provider_release_id TEXT NULL,
            provider_file_id TEXT NULL,
            is_origin INTEGER NOT NULL DEFAULT 0
        );
        CREATE UNIQUE INDEX instance_content_provider_refs_identity
            ON instance_content_provider_refs (
                content_entry_id,
                provider,
                provider_project_id,
                COALESCE(provider_release_id, '')
            );
        CREATE UNIQUE INDEX instance_content_provider_refs_origin
            ON instance_content_provider_refs(content_entry_id)
            WHERE is_origin = 1;
        ",
    )
    .execute(&pool)
    .await
    .expect("provider ref table");
    let old = ContentProviderRef::Modrinth {
        project_id: ModrinthProjectId::new("AANobbMI").unwrap(),
        version_id: Some(ModrinthVersionId::new("7pwil2dy").unwrap()),
    };
    let target = ContentProviderRef::Modrinth {
        project_id: ModrinthProjectId::new("AANobbMI").unwrap(),
        version_id: Some(ModrinthVersionId::new("vf7UgZpC").unwrap()),
    };

    upsert_content_provider_ref("entry", &old, true, &pool)
        .await
        .unwrap();
    upsert_content_provider_ref("entry", &target, true, &pool)
        .await
        .unwrap();

    assert_eq!(
        get_content_provider_refs("entry", &pool).await.unwrap(),
        vec![target.clone(), old.clone()]
    );
    assert_eq!(
        get_content_provider_refs_with_origin("entry", &pool)
            .await
            .unwrap(),
        vec![(target, true), (old, false)]
    );
}

#[tokio::test]
async fn update_check_upsert_ignores_deleted_content_entries() {
    let pool = test_pool().await;
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await
        .expect("enable foreign keys");
    sqlx::query("CREATE TABLE instance_content_entries (id TEXT PRIMARY KEY)")
        .execute(&pool)
        .await
        .expect("instance_content_entries table");
    sqlx::query(
        "
        CREATE TABLE instance_content_update_checks (
            content_entry_id TEXT PRIMARY KEY NOT NULL,
            update_channel TEXT NOT NULL,
            provider TEXT NULL,
            provider_project_id TEXT NULL,
            provider_release_id TEXT NULL,
            checked_at INTEGER NOT NULL,
            FOREIGN KEY (content_entry_id)
                REFERENCES instance_content_entries(id)
                ON DELETE CASCADE
        )
        ",
    )
    .execute(&pool)
    .await
    .expect("instance_content_update_checks table");

    upsert_content_update_check(
        "deleted-entry",
        ReleaseChannel::Release,
        Some(ContentProvider::CurseForge),
        Some("285109"),
        Some("4612979"),
        &pool,
    )
    .await
    .expect("deleted entry is ignored");

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM instance_content_update_checks",
    )
    .fetch_one(&pool)
    .await
    .expect("count update checks");
    assert_eq!(count, 0);
}

async fn pool_with_entry_rows() -> sqlx::SqlitePool {
    let pool = test_pool().await;
    sqlx::raw_sql(
        "
        CREATE TABLE instance_content_entries (
            id TEXT PRIMARY KEY,
            instance_id TEXT NOT NULL,
            content_set_id TEXT NOT NULL,
            file_id TEXT NULL,
            project_type TEXT NOT NULL DEFAULT 'mod',
            source_kind TEXT NOT NULL,
            ownership_kind TEXT NOT NULL,
            auto_dependency INTEGER NOT NULL DEFAULT 0,
            server_requirement TEXT NOT NULL DEFAULT 'unknown',
            client_requirement TEXT NOT NULL DEFAULT 'unknown',
            enabled INTEGER NOT NULL DEFAULT 1,
            added_at INTEGER NOT NULL DEFAULT 1,
            modified_at INTEGER NOT NULL DEFAULT 1
        );
        ",
    )
    .execute(&pool)
    .await
    .expect("content entry table");
    sqlx::query(
        "INSERT INTO instance_content_entries
            (id, instance_id, content_set_id, source_kind, ownership_kind)
         VALUES ('entry', 'instance', 'set', 'local', 'user_added')",
    )
    .execute(&pool)
    .await
    .unwrap();
    pool
}

#[tokio::test]
async fn an_entry_reads_back_the_state_it_was_written_in() {
    let pool = pool_with_entry_rows().await;

    let mut tx = pool.begin().await.unwrap();
    set_entry_enabled_in_transaction("entry", false, 2, &mut tx)
        .await
        .unwrap();
    set_entry_provenance_in_transaction(
        "entry",
        ContentOwnershipKind::PackManaged,
        ContentSourceKind::ImportedModpack,
        true,
        ContentRequirement::Required,
        ContentRequirement::Optional,
        &mut tx,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    let entry = get_content_entry_by_id("entry", &pool)
        .await
        .unwrap()
        .unwrap();

    assert!(!entry.enabled);
    assert_eq!(entry.modified_at.timestamp(), 2);
    assert_eq!(entry.ownership_kind, ContentOwnershipKind::PackManaged);
    assert_eq!(entry.source_kind, ContentSourceKind::ImportedModpack);
    assert!(entry.auto_dependency);
    assert_eq!(entry.server_requirement, ContentRequirement::Required);
    assert_eq!(entry.client_requirement, ContentRequirement::Optional);
}

#[tokio::test]
async fn an_entry_can_take_the_id_the_caller_minted() {
    let pool = pool_with_entry_rows().await;

    let mut tx = pool.begin().await.unwrap();
    rewrite_entry_id_in_transaction("entry", "minted", &mut tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();

    assert!(
        get_content_entry_by_id("entry", &pool)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        get_content_entry_by_id("minted", &pool)
            .await
            .unwrap()
            .is_some()
    );
}

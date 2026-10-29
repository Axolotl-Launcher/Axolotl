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

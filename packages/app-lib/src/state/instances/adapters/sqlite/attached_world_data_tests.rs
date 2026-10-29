use super::*;
use crate::state::instances::adapters::sqlite::test_support::test_pool;

async fn pool_with_worlds() -> sqlx::SqlitePool {
    let pool = test_pool().await;
    sqlx::raw_sql(
        "
        CREATE TABLE attached_world_data (
            instance_id TEXT NOT NULL,
            world_type TEXT CHECK ( world_type in ('singleplayer', 'server') ) NOT NULL,
            world_id TEXT NOT NULL,
            display_status TEXT NOT NULL DEFAULT 'normal',
            project_id TEXT NULL,
            content_kind TEXT NULL,

            PRIMARY KEY (instance_id, world_type, world_id)
        );
        ",
    )
    .execute(&pool)
    .await
    .expect("attached world data table");
    pool
}

#[tokio::test]
async fn every_column_reads_back_with_the_others_untouched() {
    let pool = pool_with_worlds().await;

    set_display_status("instance", "singleplayer", "world", "favorite", &pool)
        .await
        .unwrap();
    set_project_id("instance", "singleplayer", "world", "project", &pool)
        .await
        .unwrap();
    set_content_kind("instance", "singleplayer", "world", "modpack", &pool)
        .await
        .unwrap();

    let row = get_for_world("instance", "singleplayer", "world", &pool)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.display_status, "favorite");
    assert_eq!(row.project_id.as_deref(), Some("project"));
    assert_eq!(row.content_kind.as_deref(), Some("modpack"));

    let all = get_all_for_instance("instance", &pool).await.unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].world_type, "singleplayer");
    assert_eq!(all[0].world_id, "world");
}

#[tokio::test]
async fn an_unknown_world_holds_nothing() {
    let pool = pool_with_worlds().await;

    assert!(
        get_for_world("instance", "server", "missing", &pool)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn removing_a_world_leaves_the_others_recorded() {
    let pool = pool_with_worlds().await;
    set_display_status("instance", "singleplayer", "world", "favorite", &pool)
        .await
        .unwrap();
    set_display_status("instance", "singleplayer", "other", "hidden", &pool)
        .await
        .unwrap();

    remove_for_world("instance", "singleplayer", "world", &pool)
        .await
        .unwrap();

    let all = get_all_for_instance("instance", &pool).await.unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].world_id, "other");
}

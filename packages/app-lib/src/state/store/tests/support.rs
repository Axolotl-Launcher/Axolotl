//! The databases the tests work on, and the rows a collection refers to
//! before it can store anything.

use sqlx::SqlitePool;

use crate::state::store::Handle;
use crate::state::store::derived_db::{DerivedDb, FILE_NAME};

/// A database of its own, with every migration applied.
pub(super) async fn pool() -> SqlitePool {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();
    pool
}

/// A handle on a database of its own.
pub(super) async fn handle() -> Handle {
    Handle::new(pool().await)
}

/// A handle on a derived database of its own, with every derived migration
/// applied, and the directory that holds it.
pub(super) async fn derived() -> (tempfile::TempDir, Handle) {
    let directory = tempfile::tempdir().unwrap();
    let db = DerivedDb::open(directory.path().join(FILE_NAME)).await;
    let handle = db.handle();
    (directory, handle)
}

/// The same database, reached the way the collections that live there are.
pub(super) async fn derived_db() -> (tempfile::TempDir, DerivedDb) {
    let directory = tempfile::tempdir().unwrap();
    let db = DerivedDb::open(directory.path().join(FILE_NAME)).await;
    (directory, db)
}

/// A derived database this build cannot open, which stays disabled for the run.
pub(super) async fn disabled_derived() -> (tempfile::TempDir, DerivedDb) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join(FILE_NAME);
    tokio::fs::write(&path, b"not a database").await.unwrap();
    let db = DerivedDb::open(path).await;
    (directory, db)
}

/// A handle on a database of its own, with `instances` already there.
pub(super) async fn handle_with_instances(instances: &[&str]) -> Handle {
    let pool = pool().await;
    for instance_id in instances {
        add_instance(&pool, instance_id).await;
    }
    Handle::new(pool)
}

/// An instance a record may point at.
pub(super) async fn add_instance(pool: &SqlitePool, instance_id: &str) {
    sqlx::query(
        "INSERT INTO instances
            (id, path, install_stage, launcher_feature_version,
             name, created, modified)
         VALUES (?, ?, 'installed', '1', ?, 1, 1)",
    )
    .bind(instance_id)
    .bind(instance_id)
    .bind(instance_id)
    .execute(pool)
    .await
    .unwrap();
}

/// Turns `feature` on for an instance, which a checkpoint needs to exist.
pub(super) async fn enable_feature(
    pool: &SqlitePool,
    instance_id: &str,
    feature: &str,
) {
    sqlx::query(
        "INSERT INTO instance_sync_preferences (instance_id, feature, enabled)
         VALUES (?, ?, 1)",
    )
    .bind(instance_id)
    .bind(feature)
    .execute(pool)
    .await
    .unwrap();
}

/// A resource a provider project may map to.
pub(super) async fn add_resource(pool: &SqlitePool, resource_id: &str) {
    sqlx::query("INSERT INTO resources (id) VALUES (?)")
        .bind(resource_id)
        .execute(pool)
        .await
        .unwrap();
}

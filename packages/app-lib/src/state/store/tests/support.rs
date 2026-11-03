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

/// The same database for a test outside this half: `sqlx` may be spelled here
/// alone, so a module that needs a migrated database borrows one instead of
/// opening it and running the migrations itself.
pub(crate) async fn migrated_pool() -> SqlitePool {
    pool().await
}

/// A migrated database kept on disk, reached through more than one connection.
///
/// An in-memory database is one per connection, so a test that has to read
/// while a unit holds its connection open cannot use one: the read would wait
/// for the connection the unit holds. A file is what two connections share.
pub(crate) async fn migrated_file_pool(
    directory: &std::path::Path,
) -> SqlitePool {
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(directory.join("app.db"))
        .create_if_missing(true);
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(2)
        .connect_with(options)
        .await
        .unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();
    pool
}

/// A database of its own with no tables, for a test that lays out the ones it
/// needs rather than running the migrations. One connection, so every
/// statement sees the same in-memory database.
pub(crate) async fn empty_pool() -> SqlitePool {
    sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("in-memory SQLite pool")
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

/// An install job a record may name, with the instance it was started for.
///
/// `content_update_jobs.job_id` and `resource_operation_plans.job_id` are
/// foreign keys into `install_jobs`, and `install_jobs.instance_id` into
/// `instances`: a test that writes a record naming a job has to write the rows
/// it names first, because the connection opens with `foreign_keys` on.
pub(super) async fn seed_install_job(
    pool: &SqlitePool,
    job_id: &str,
    instance_id: Option<&str>,
) {
    sqlx::query(
        "INSERT INTO install_jobs
            (id, instance_id, kind, status, state, created, modified, dismissed)
         VALUES (?, ?, 'pack', 'running', '{\"progress\":{}}', 1, 1, 0)",
    )
    .bind(job_id)
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

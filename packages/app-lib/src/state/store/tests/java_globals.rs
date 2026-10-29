//! What the Java runtime collections promise.

use super::support;
use crate::state::store::Collection as _;
use crate::state::store::Handle;
use crate::state::store::java_default_versions::{
    JavaDefaultVersionRecord, JavaDefaultVersions,
};
use crate::state::store::java_versions::{JavaVersionRecord, JavaVersions};

const JAVA_17: &str = "/java/17/bin/java";
const JAVA_21: &str = "/java/21/bin/java";

fn handle_of(pool: &sqlx::SqlitePool) -> Handle {
    Handle::new(pool.clone())
}

fn runtime(major_version: u32, full_version: &str) -> JavaVersionRecord {
    JavaVersionRecord {
        major_version,
        full_version: full_version.to_string(),
        architecture: "x86_64".to_string(),
        distribution: None,
    }
}

fn default_version(path: &str) -> JavaDefaultVersionRecord {
    JavaDefaultVersionRecord {
        path: path.to_string(),
    }
}

#[tokio::test]
async fn a_runtime_round_trips_and_clears() {
    let handle = support::handle().await;
    let path = JAVA_17.to_string();

    JavaVersions::set(&handle, &path, Some(&runtime(17, "17.0.9")))
        .await
        .unwrap();

    assert_eq!(
        JavaVersions::get(&handle, &path).await.unwrap(),
        Some(runtime(17, "17.0.9"))
    );

    JavaVersions::set(&handle, &path, None).await.unwrap();
    assert_eq!(JavaVersions::get(&handle, &path).await.unwrap(), None);
}

#[tokio::test]
async fn writing_a_runtime_again_replaces_every_column() {
    let handle = support::handle().await;
    let path = JAVA_17.to_string();
    let mut moved = runtime(21, "21.0.1");
    moved.distribution = Some("temurin".to_string());

    JavaVersions::set(&handle, &path, Some(&runtime(17, "17.0.9")))
        .await
        .unwrap();
    JavaVersions::set(&handle, &path, Some(&moved))
        .await
        .unwrap();

    assert_eq!(
        JavaVersions::get(&handle, &path).await.unwrap(),
        Some(moved)
    );
    assert_eq!(JavaVersions::get_all(&handle).await.unwrap().len(), 1);
}

#[tokio::test]
async fn a_whole_collection_round_trips_and_drops_what_it_leaves_out() {
    let handle = support::handle().await;
    let seventeen = JAVA_17.to_string();
    let twenty_one = JAVA_21.to_string();
    JavaVersions::set(&handle, &seventeen, Some(&runtime(17, "17.0.9")))
        .await
        .unwrap();
    JavaVersions::set(&handle, &twenty_one, Some(&runtime(21, "21.0.1")))
        .await
        .unwrap();
    JavaDefaultVersions::set(&handle, &17, Some(&default_version(&seventeen)))
        .await
        .unwrap();

    JavaVersions::set_all(
        &handle,
        &[(twenty_one.clone(), runtime(21, "21.0.2"))],
    )
    .await
    .unwrap();

    assert_eq!(
        JavaVersions::get_all(&handle).await.unwrap(),
        vec![(twenty_one, runtime(21, "21.0.2"))]
    );
    assert_eq!(JavaDefaultVersions::get(&handle, &17).await.unwrap(), None);
}

#[tokio::test]
async fn a_scope_reads_only_that_major_versions_runtimes() {
    let handle = support::handle().await;
    let second_17 = "/java/17-backup/bin/java".to_string();
    JavaVersions::set(
        &handle,
        &JAVA_17.to_string(),
        Some(&runtime(17, "17.0.9")),
    )
    .await
    .unwrap();
    JavaVersions::set(&handle, &second_17, Some(&runtime(17, "17.0.8")))
        .await
        .unwrap();
    JavaVersions::set(
        &handle,
        &JAVA_21.to_string(),
        Some(&runtime(21, "21.0.1")),
    )
    .await
    .unwrap();

    let scoped = JavaVersions::get_scoped(&handle, &17).await.unwrap();

    assert_eq!(scoped.len(), 2);
    assert!(scoped.iter().all(|(_, record)| record.major_version == 17));
}

#[tokio::test]
async fn a_scoped_replace_refuses_another_major_version() {
    let handle = support::handle().await;

    assert!(
        JavaVersions::set_scoped(
            &handle,
            &17,
            &[(JAVA_21.to_string(), runtime(21, "21.0.1"))],
        )
        .await
        .is_err()
    );
    assert!(JavaVersions::get_all(&handle).await.unwrap().is_empty());
}

#[tokio::test]
async fn clearing_a_scope_removes_its_runtimes_and_their_defaults() {
    let handle = support::handle().await;
    let seventeen = JAVA_17.to_string();
    let twenty_one = JAVA_21.to_string();
    JavaVersions::set(&handle, &seventeen, Some(&runtime(17, "17.0.9")))
        .await
        .unwrap();
    JavaVersions::set(&handle, &twenty_one, Some(&runtime(21, "21.0.1")))
        .await
        .unwrap();
    JavaDefaultVersions::set(&handle, &17, Some(&default_version(&seventeen)))
        .await
        .unwrap();

    JavaVersions::set_scoped(&handle, &17, &[]).await.unwrap();

    assert_eq!(
        JavaVersions::get_all(&handle).await.unwrap(),
        vec![(twenty_one, runtime(21, "21.0.1"))]
    );
    assert_eq!(JavaDefaultVersions::get(&handle, &17).await.unwrap(), None);
}

#[tokio::test]
async fn a_default_round_trips_per_major_version_and_clears() {
    let handle = support::handle().await;
    let seventeen = JAVA_17.to_string();
    let twenty_one = JAVA_21.to_string();
    JavaVersions::set(&handle, &seventeen, Some(&runtime(17, "17.0.9")))
        .await
        .unwrap();
    JavaVersions::set(&handle, &twenty_one, Some(&runtime(21, "21.0.1")))
        .await
        .unwrap();

    JavaDefaultVersions::set(&handle, &17, Some(&default_version(&seventeen)))
        .await
        .unwrap();
    JavaDefaultVersions::set(&handle, &21, Some(&default_version(&twenty_one)))
        .await
        .unwrap();

    assert_eq!(
        JavaDefaultVersions::get(&handle, &17).await.unwrap(),
        Some(default_version(&seventeen))
    );

    JavaDefaultVersions::set(&handle, &17, None).await.unwrap();
    assert_eq!(JavaDefaultVersions::get(&handle, &17).await.unwrap(), None);
    assert_eq!(
        JavaDefaultVersions::get(&handle, &21).await.unwrap(),
        Some(default_version(&twenty_one))
    );
}

#[tokio::test]
async fn a_default_replacement_keeps_what_it_lists() {
    let handle = support::handle().await;
    let seventeen = JAVA_17.to_string();
    let twenty_one = JAVA_21.to_string();
    JavaVersions::set(&handle, &seventeen, Some(&runtime(17, "17.0.9")))
        .await
        .unwrap();
    JavaVersions::set(&handle, &twenty_one, Some(&runtime(21, "21.0.1")))
        .await
        .unwrap();
    JavaDefaultVersions::set(&handle, &17, Some(&default_version(&seventeen)))
        .await
        .unwrap();
    JavaDefaultVersions::set(&handle, &21, Some(&default_version(&twenty_one)))
        .await
        .unwrap();

    JavaDefaultVersions::set_all(
        &handle,
        &[(21, default_version(&twenty_one))],
    )
    .await
    .unwrap();

    assert_eq!(
        JavaDefaultVersions::get_all(&handle).await.unwrap(),
        vec![(21, default_version(&twenty_one))]
    );
}

#[tokio::test]
async fn a_default_naming_no_runtime_is_refused() {
    let handle = support::handle().await;

    assert!(
        JavaDefaultVersions::set(&handle, &17, Some(&default_version(JAVA_17)))
            .await
            .is_err()
    );
    assert_eq!(JavaDefaultVersions::get(&handle, &17).await.unwrap(), None);
}

#[tokio::test]
async fn a_runtime_whose_major_version_does_not_fit_is_ignored() {
    let pool = support::pool().await;
    let handle = handle_of(&pool);
    sqlx::query(
        "INSERT INTO java_versions
            (major_version, full_version, architecture, path, distribution)
         VALUES (-1, '1.0', 'x86_64', '/java/old/bin/java', NULL)",
    )
    .execute(&pool)
    .await
    .unwrap();

    assert!(JavaVersions::get_all(&handle).await.unwrap().is_empty());
    assert_eq!(
        JavaVersions::get(&handle, &"/java/old/bin/java".to_string())
            .await
            .unwrap(),
        None
    );
}

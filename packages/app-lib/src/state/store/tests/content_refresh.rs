use super::support;
use crate::resources::{ContentHash, HashAlgorithm};
use crate::state::store::content_file_fingerprints::{
    ContentFileFingerprints, FileFingerprintRecord, FileModifiedTime,
    FingerprintKey,
};
use crate::state::store::content_refresh_checks::{
    CheckAttempt, ContentRefreshChecks, RefreshCheckContext, RefreshCheckKey,
    RefreshCheckRecord,
};
use crate::state::store::{Collection as _, Handle, atom};

async fn seed(pool: &sqlx::SqlitePool) {
    support::add_instance(pool, "instance").await;
    support::add_instance(pool, "other").await;
    sqlx::raw_sql(
        "INSERT INTO instance_content_sets (id, instance_id, name, source_kind, status, game_version, loader, created, modified)
         VALUES ('set', 'instance', 'Main', 'local', 'available', '1.21', 'fabric', 1, 1);
         INSERT INTO instance_files (id, instance_id, relative_path, file_name, enabled, sha1, size, added_at, modified_at)
         VALUES ('file', 'instance', 'mods/a.jar', 'a.jar', 1, 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', 3, 1, 1);
         INSERT INTO instance_content_entries (id, instance_id, content_set_id, file_id, project_type, source_kind, server_requirement, client_requirement, added_at, modified_at)
         VALUES ('entry', 'instance', 'set', 'file', 'mod', 'local', 'optional', 'optional', 1, 1);"
    ).execute(pool).await.unwrap();
}

async fn handle() -> Handle {
    let pool = support::pool().await;
    seed(&pool).await;
    Handle::new(pool)
}

/// The same instance, with the content set it applied and a second one it can
/// switch to.
async fn seed_with_second_set(pool: &sqlx::SqlitePool) {
    seed(pool).await;
    sqlx::raw_sql(
        "INSERT INTO instance_content_sets (id, instance_id, name, source_kind, status, game_version, loader, created, modified)
         VALUES ('second', 'instance', 'Second', 'local', 'available', '1.21', 'fabric', 1, 1);
         UPDATE instances SET applied_content_set_id = 'set' WHERE id = 'instance';"
    ).execute(pool).await.unwrap();
}

fn fingerprint_key() -> FingerprintKey {
    FingerprintKey {
        instance_id: "instance".into(),
        file_id: "file".into(),
    }
}

fn fingerprint() -> FileFingerprintRecord {
    FileFingerprintRecord {
        content_root: "E:/instances/main".into(),
        relative_path: "mods/a.jar".into(),
        size: 3,
        modified: FileModifiedTime {
            seconds: 10,
            nanos: 123456789,
        },
        hashes: vec![ContentHash {
            algorithm: HashAlgorithm::Sha1,
            value: "a".repeat(40),
        }],
        curseforge_fingerprint: Some(123),
    }
}

fn check_key() -> RefreshCheckKey {
    RefreshCheckKey {
        instance_id: "instance".into(),
        entry_id: "entry".into(),
    }
}

fn check() -> RefreshCheckRecord {
    RefreshCheckRecord {
        file_id: "file".into(),
        context: RefreshCheckContext {
            location_key: "location".into(),
            content_root: "E:/instances/main".into(),
            sha1: "a".repeat(40),
            size: 3,
            current_release: None,
            target: crate::resources::ipc::ResourceTarget {
                game_versions: vec!["1.21".into()],
                loaders: vec!["fabric".into()],
                channels: vec![],
            },
            preferred_source: crate::resources::SourceId::new("modrinth")
                .unwrap(),
        },
        last_success: None,
        attempted_at: 100,
        attempt: CheckAttempt::Unrecognized,
        error: None,
    }
}

#[tokio::test]
async fn attributes_and_checks_round_trip_and_failed_attempt_preserves_success()
{
    let handle = handle().await;
    let mut record = check();
    let candidate = serde_json::from_value(serde_json::json!({
        "project": { "source": "curseforge", "projectId": "123" }, "releaseId": "456"
    })).unwrap();
    record.succeeded(100, Some(candidate));
    let successful = record.last_success.clone();
    let mut unit = atom(&handle).await.unwrap();
    ContentFileFingerprints::atom_set(
        &mut unit,
        &fingerprint_key(),
        Some(&fingerprint()),
    )
    .await
    .unwrap();
    ContentRefreshChecks::atom_set(&mut unit, &check_key(), Some(&record))
        .await
        .unwrap();
    unit.commit().await.unwrap();
    record.failed(101, "offline".into());
    ContentRefreshChecks::set(&handle, &check_key(), Some(&record))
        .await
        .unwrap();
    let stored = ContentRefreshChecks::get(&handle, &check_key())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.last_success, successful);
    assert_eq!(stored.attempt, CheckAttempt::Failed);
    assert_eq!(stored.error.as_deref(), Some("offline"));
    assert_eq!(
        ContentFileFingerprints::get(&handle, &fingerprint_key())
            .await
            .unwrap(),
        Some(fingerprint())
    );
    record.succeeded(102, None);
    ContentRefreshChecks::set(&handle, &check_key(), Some(&record))
        .await
        .unwrap();
    assert_eq!(
        ContentRefreshChecks::get(&handle, &check_key())
            .await
            .unwrap(),
        Some(record)
    );
}

#[tokio::test]
async fn scoped_empty_and_rollback_are_atomic() {
    let handle = handle().await;
    ContentFileFingerprints::set(
        &handle,
        &fingerprint_key(),
        Some(&fingerprint()),
    )
    .await
    .unwrap();
    ContentRefreshChecks::set(&handle, &check_key(), Some(&check()))
        .await
        .unwrap();
    let mut unit = atom(&handle).await.unwrap();
    ContentFileFingerprints::atom_set_scoped(
        &mut unit,
        &"instance".into(),
        &[],
    )
    .await
    .unwrap();
    ContentRefreshChecks::atom_set_scoped(&mut unit, &"instance".into(), &[])
        .await
        .unwrap();
    drop(unit);
    assert_eq!(
        ContentFileFingerprints::get_scoped(&handle, &"instance".into())
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        ContentRefreshChecks::get_scoped(&handle, &"other".into())
            .await
            .unwrap()
            .len(),
        0
    );
    ContentFileFingerprints::set_scoped(&handle, &"instance".into(), &[])
        .await
        .unwrap();
    ContentRefreshChecks::set_scoped(&handle, &"instance".into(), &[])
        .await
        .unwrap();
    assert!(
        ContentRefreshChecks::get_all(&handle)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        ContentFileFingerprints::get_all(&handle)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn wrong_instance_is_rejected_and_deleted_files_remove_results() {
    let handle = handle().await;
    let wrong = FingerprintKey {
        instance_id: "other".into(),
        ..fingerprint_key()
    };
    assert!(
        ContentFileFingerprints::set(&handle, &wrong, Some(&fingerprint()))
            .await
            .is_err()
    );
    let wrong = RefreshCheckKey {
        instance_id: "other".into(),
        ..check_key()
    };
    assert!(
        ContentRefreshChecks::set(&handle, &wrong, Some(&check()))
            .await
            .is_err()
    );
    ContentFileFingerprints::set(
        &handle,
        &fingerprint_key(),
        Some(&fingerprint()),
    )
    .await
    .unwrap();
    ContentRefreshChecks::set(&handle, &check_key(), Some(&check()))
        .await
        .unwrap();
    sqlx::query("DELETE FROM instance_files WHERE id = 'file'")
        .execute(handle.pool())
        .await
        .unwrap();
    assert!(
        ContentFileFingerprints::get_all(&handle)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        ContentRefreshChecks::get_all(&handle)
            .await
            .unwrap()
            .is_empty()
    );
    let violations: Vec<(String, i64, String, i64)> =
        sqlx::query_as("PRAGMA foreign_key_check")
            .fetch_all(handle.pool())
            .await
            .unwrap();
    assert!(violations.is_empty());
}

#[tokio::test]
async fn malformed_rows_are_skipped_without_hiding_valid_records() {
    let handle = handle().await;
    ContentFileFingerprints::set(
        &handle,
        &fingerprint_key(),
        Some(&fingerprint()),
    )
    .await
    .unwrap();
    ContentRefreshChecks::set(&handle, &check_key(), Some(&check()))
        .await
        .unwrap();
    sqlx::query("UPDATE content_refresh_checks SET payload = '{\"context\":{\"preferred_source\":\"\"}}'").execute(handle.pool()).await.unwrap();
    assert!(
        ContentRefreshChecks::get_all(&handle)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        ContentFileFingerprints::get_all(&handle)
            .await
            .unwrap()
            .len(),
        1
    );
    let mut invalid = fingerprint();
    invalid.modified.nanos = 1_000_000_000;
    assert!(
        ContentFileFingerprints::set(
            &handle,
            &fingerprint_key(),
            Some(&invalid)
        )
        .await
        .is_err()
    );
    sqlx::query("UPDATE content_file_fingerprints SET payload = 'not json'")
        .execute(handle.pool())
        .await
        .unwrap();
    assert!(
        ContentFileFingerprints::get_all(&handle)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn refresh_migration_preserves_existing_rows_and_establishes_no_unverified_baseline()
 {
    let migrations = sqlx::migrate!();
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let previous = sqlx::migrate::Migrator {
        migrations: std::borrow::Cow::Owned(
            migrations
                .iter()
                .filter(|migration| migration.version < 20261027120000)
                .cloned()
                .collect(),
        ),
        ..sqlx::migrate::Migrator::DEFAULT
    };
    previous.run(&pool).await.unwrap();
    seed(&pool).await;
    migrations.run(&pool).await.unwrap();
    let handle = Handle::new(pool);
    assert!(
        ContentFileFingerprints::get_all(&handle)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        ContentRefreshChecks::get_all(&handle)
            .await
            .unwrap()
            .is_empty()
    );
    let files: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM instance_files")
        .fetch_one(handle.pool())
        .await
        .unwrap();
    assert_eq!(files, 1);
    let violations: Vec<(String, i64, String, i64)> =
        sqlx::query_as("PRAGMA foreign_key_check")
            .fetch_all(handle.pool())
            .await
            .unwrap();
    assert!(violations.is_empty());
}

#[test]
fn fingerprint_attributes_require_precise_time_path_and_root() {
    let record = fingerprint();
    assert!(record.matches(
        &record.content_root,
        &record.relative_path,
        record.size,
        Some(&record.modified)
    ));
    assert!(!record.matches(
        &record.content_root,
        &record.relative_path,
        record.size,
        None
    ));
    let mut modified = record.modified.clone();
    modified.nanos += 1;
    assert!(!record.matches(
        &record.content_root,
        &record.relative_path,
        record.size,
        Some(&modified)
    ));
    assert!(!record.matches(
        "another-root",
        &record.relative_path,
        record.size,
        Some(&record.modified)
    ));
    assert!(!record.matches(
        &record.content_root,
        "mods/b.jar",
        record.size,
        Some(&record.modified)
    ));
}

#[tokio::test]
async fn saved_packs_and_scan_failures_answer_on_the_derived_database() {
    use crate::state::store::content_refresh_pack::*;
    use crate::state::store::content_refresh_scan_failures::*;
    let (_directory, derived) = support::derived_db().await;
    let key = ScanFailureKey {
        instance_id: "instance".into(),
        location_key: "root".into(),
        relative_path: "mods/new.jar".into(),
    };
    ContentRefreshScanFailures::upsert(
        &derived,
        &key,
        &ScanFailureRecord {
            attempted_at: 42,
            error: "locked".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        ContentRefreshScanFailures::failed(&derived, "instance")
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(
        ContentRefreshScanFailures::failed(&derived, "other")
            .await
            .unwrap()
            .is_empty()
    );
    ContentRefreshScanFailures::remove(&derived, &key)
        .await
        .unwrap();
    assert!(
        ContentRefreshScanFailures::failed(&derived, "instance")
            .await
            .unwrap()
            .is_empty()
    );

    let pack = PackKey {
        instance_id: "instance".into(),
        content_set_id: "set".into(),
    };
    ContentRefreshPacks::upsert(
        &derived,
        &pack,
        &PackRecord {
            location_key: "root".into(),
            link_key: "link".into(),
            saved_at: 10,
            pack: None,
            attempted_at: 10,
            last_error: None,
        },
    )
    .await
    .unwrap();
    assert!(
        ContentRefreshPacks::saved(&derived, &pack)
            .await
            .unwrap()
            .is_some()
    );
    ContentRefreshPacks::forget_instance(&derived, "instance")
        .await
        .unwrap();
    assert!(
        ContentRefreshPacks::saved(&derived, &pack)
            .await
            .unwrap()
            .is_none()
    );
    derived.pool().close().await;
}

/// The old schema removed a saved pack in two cases: the instance went away, or
/// the content set it named did. A switch between content sets was neither, so
/// it left the row alone; the rows now live in another database, and a switch
/// still leaves them alone while the deletion of the instance removes them.
#[tokio::test]
async fn a_content_set_switch_keeps_the_saved_pack_of_the_set_it_left() {
    use crate::state::store::content_refresh_pack::*;
    let pool = support::pool().await;
    seed_with_second_set(&pool).await;
    let (_directory, derived) = support::derived_db().await;
    let handle = Handle::new(pool.clone());
    let left = PackKey {
        instance_id: "instance".into(),
        content_set_id: "set".into(),
    };
    ContentRefreshPacks::upsert(
        &derived,
        &left,
        &PackRecord {
            location_key: "root".into(),
            link_key: "link".into(),
            saved_at: 10,
            pack: None,
            attempted_at: 10,
            last_error: None,
        },
    )
    .await
    .unwrap();
    assert!(
        crate::state::store::content_snapshot::get_with_saved_pack(
            &handle, &derived, "instance"
        )
        .await
        .unwrap()
        .pack_presentation
        .is_some()
    );

    sqlx::query(
        "UPDATE instances SET applied_content_set_id = 'second'
         WHERE id = 'instance'",
    )
    .execute(&pool)
    .await
    .unwrap();

    let switched = crate::state::store::content_snapshot::get_with_saved_pack(
        &handle, &derived, "instance",
    )
    .await
    .unwrap();
    assert_eq!(switched.content_set.id, "second");
    assert!(
        switched.pack_presentation.is_none(),
        "the set the instance switched to saved no pack of its own"
    );
    assert!(
        ContentRefreshPacks::saved(&derived, &left)
            .await
            .unwrap()
            .is_some(),
        "a switch is not a deletion, so the row of the set it left stays"
    );
    assert_eq!(
        ContentRefreshPacks::instance_ids(&derived).await.unwrap(),
        vec!["instance".to_string()]
    );
    pool.close().await;
    derived.pool().close().await;
}

/// A database this build cannot open keeps answering: the caller reads nothing
/// and its write is dropped rather than reported as a failure.
#[tokio::test]
async fn a_disabled_derived_database_holds_no_refresh_state() {
    use crate::state::store::content_refresh_pack::*;
    use crate::state::store::content_refresh_scan_failures::*;
    let (_directory, derived) = support::disabled_derived().await;
    assert!(derived.is_disabled());
    let pack = PackKey {
        instance_id: "instance".into(),
        content_set_id: "set".into(),
    };
    let failure = ScanFailureKey {
        instance_id: "instance".into(),
        location_key: "root".into(),
        relative_path: "mods/new.jar".into(),
    };

    ContentRefreshPacks::upsert(
        &derived,
        &pack,
        &PackRecord {
            location_key: "root".into(),
            link_key: "link".into(),
            saved_at: 10,
            pack: None,
            attempted_at: 10,
            last_error: None,
        },
    )
    .await
    .unwrap();
    ContentRefreshScanFailures::upsert(
        &derived,
        &failure,
        &ScanFailureRecord {
            attempted_at: 42,
            error: "locked".into(),
        },
    )
    .await
    .unwrap();
    ContentRefreshPacks::forget_instance(&derived, "instance")
        .await
        .unwrap();

    assert!(
        ContentRefreshPacks::saved(&derived, &pack)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        ContentRefreshScanFailures::failed(&derived, "instance")
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        ContentRefreshPacks::instance_ids(&derived)
            .await
            .unwrap()
            .is_empty()
    );
}

#[test]
fn checks_are_reused_only_for_the_same_file_and_selection_conditions() {
    let mut previous = check();
    previous.succeeded(100, None);
    assert_eq!(
        RefreshCheckRecord::for_context(
            Some(previous.clone()),
            previous.file_id.clone(),
            previous.context.clone(),
            101
        ),
        previous
    );
    let mut contexts = vec![previous.context.clone(); 6];
    contexts[0].content_root.push_str("/different");
    contexts[1].sha1 = "b".repeat(40);
    contexts[2].size += 1;
    contexts[3].target.game_versions = vec!["1.22".into()];
    contexts[4].target.loaders = vec!["forge".into()];
    contexts[5].preferred_source =
        crate::resources::SourceId::new("curseforge").unwrap();
    for context in contexts {
        assert!(
            RefreshCheckRecord::for_context(
                Some(previous.clone()),
                previous.file_id.clone(),
                context,
                101
            )
            .last_success
            .is_none()
        );
    }
    assert!(
        RefreshCheckRecord::for_context(
            Some(previous.clone()),
            "new-file".into(),
            previous.context,
            101
        )
        .last_success
        .is_none()
    );
}

#[tokio::test]
async fn results_survive_closing_and_reopening_the_database() {
    let temp = tempfile::tempdir().unwrap();
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(temp.path().join("app.db"))
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options.clone())
        .await
        .unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();
    seed(&pool).await;
    let handle = Handle::new(pool.clone());
    ContentFileFingerprints::set(
        &handle,
        &fingerprint_key(),
        Some(&fingerprint()),
    )
    .await
    .unwrap();
    let mut result = check();
    result.succeeded(100, Some(serde_json::from_value(serde_json::json!({
        "project": { "source": "modrinth", "projectId": "12345678" }, "releaseId": "release1"
    })).unwrap()));
    result.failed(101, "offline".into());
    ContentRefreshChecks::set(&handle, &check_key(), Some(&result))
        .await
        .unwrap();
    pool.close().await;
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap();
    let handle = Handle::new(pool);
    assert_eq!(
        ContentFileFingerprints::get(&handle, &fingerprint_key())
            .await
            .unwrap(),
        Some(fingerprint())
    );
    assert_eq!(
        ContentRefreshChecks::get(&handle, &check_key())
            .await
            .unwrap(),
        Some(result)
    );
    handle.pool().close().await;
}

#[test]
fn filesystem_time_preserves_nanoseconds_on_both_sides_of_epoch() {
    use std::time::{Duration, UNIX_EPOCH};
    assert_eq!(
        FileModifiedTime::from_system_time(
            UNIX_EPOCH + Duration::new(10, 123456700)
        ),
        Some(FileModifiedTime {
            seconds: 10,
            nanos: 123456700
        })
    );
    assert_eq!(
        FileModifiedTime::from_system_time(
            UNIX_EPOCH - Duration::new(10, 123456700)
        ),
        Some(FileModifiedTime {
            seconds: -11,
            nanos: 876543300
        })
    );
}

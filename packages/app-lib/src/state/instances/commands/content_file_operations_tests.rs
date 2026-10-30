use super::*;
use crate::state::DirectoryInfo;
use crate::state::instances::adapters::sqlite::instance_rows;
use crate::state::instances::commands::{
    remove_project, sync_content_files, toggle_content_entries,
    toggle_disable_project,
};
use std::path::PathBuf;
use std::sync::Arc;

use crate::state::instances::adapters::sqlite::{content_sets, files};

async fn fixture() -> (tempfile::TempDir, Arc<State>, PathBuf, Vec<String>) {
    let temp = tempfile::tempdir().unwrap();
    let directories = DirectoryInfo {
        settings_dir: temp.path().to_owned(),
        config_dir: temp.path().to_owned(),
        app_identifier: "file-journal-test".into(),
    };
    let root = directories.instances_dir().join("example");
    std::fs::create_dir_all(root.join("mods")).unwrap();
    std::fs::write(root.join("mods/a.jar"), b"a bytes").unwrap();
    std::fs::write(root.join("mods/b.jar"), b"b bytes").unwrap();
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(temp.path().join("state.db"))
                .create_if_missing(true)
                .foreign_keys(true),
        )
        .await
        .unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();
    sqlx::raw_sql("INSERT INTO instances(id, path, install_stage, launcher_feature_version, name, created, modified) VALUES ('instance', 'example', 'installed', '1', 'Example', 1, 1); INSERT INTO instance_content_sets(id, instance_id, name, source_kind, status, game_version, loader, created, modified) VALUES ('set', 'instance', 'Example', 'local', 'available', '1.20.1', 'fabric', 1, 1); UPDATE instances SET applied_content_set_id = 'set' WHERE id = 'instance';").execute(&pool).await.unwrap();
    let state = crate::state::test_state(directories, pool).await.unwrap();
    let ids = sync_content_files("instance", &state)
        .await
        .unwrap()
        .into_iter()
        .map(|file| file.id)
        .collect::<Vec<_>>();
    toggle_content_entries("instance", &ids, Some(true), &state)
        .await
        .unwrap();
    (temp, state, root, ids)
}

async fn journal_count(state: &State) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM content_file_operations")
        .fetch_one(&state.pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn toggles_and_ten_remote_refreshes_reuse_the_same_byte_evidence() {
    use crate::util::download::verified_file::track_scans;

    use crate::state::instances::adapters::sqlite::{content_sets, files};
    let (_temp, state, _root, ids) = fixture().await;
    let cancel = tokio_util::sync::CancellationToken::new();
    for enabled in [false, true] {
        let (result, scans) = track_scans(toggle_content_entries(
            "instance",
            &ids,
            Some(enabled),
            &state,
        ))
        .await;
        result.unwrap();
        assert_eq!(
            scans, 0,
            "controlled moves reuse a validated attribute baseline"
        );
        let (result, scans) =
            track_scans(sync_content_files("instance", &state)).await;
        result.unwrap();
        assert_eq!(scans, 0, "the renamed baseline survives the next scan");
    }
    for _ in 0..10 {
        let (result, scans) =
            track_scans(crate::resources::installed::refresh_snapshot(
                &state, "instance", &cancel,
            ))
            .await;
        result.unwrap();
        assert_eq!(
            scans, 0,
            "force-remote refresh cannot reread unchanged content"
        );
    }
    state.pool.close().await;
}

#[tokio::test]
async fn direct_and_batch_toggle_restore_files_after_database_failure() {
    for single in [true, false] {
        let (_temp, state, root, ids) = fixture().await;
        let revision =
            content_sets::get_applied_content_set("instance", &state.pool)
                .await
                .unwrap()
                .unwrap()
                .revision;
        sqlx::raw_sql("CREATE TRIGGER fail_toggle BEFORE UPDATE ON instance_content_entries BEGIN SELECT RAISE(FAIL, 'injected toggle failure'); END;").execute(&state.pool).await.unwrap();
        let result = if single {
            toggle_disable_project(
                "instance",
                "mods/a.jar",
                Some(false),
                &state,
            )
            .await
            .map(|_| ())
        } else {
            toggle_content_entries("instance", &ids, Some(false), &state)
                .await
                .map(|_| ())
        };
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("injected toggle failure")
        );
        for name in ["a", "b"] {
            assert!(root.join(format!("mods/{name}.jar")).exists());
            assert!(!root.join(format!("mods/{name}.jar.disabled")).exists());
        }
        assert_eq!(journal_count(&state).await, 0);
        assert_eq!(
            content_sets::get_applied_content_set("instance", &state.pool)
                .await
                .unwrap()
                .unwrap()
                .revision,
            revision
        );
        assert!(
            files::get_instance_files("instance", &state.pool)
                .await
                .unwrap()
                .iter()
                .all(|file| file.enabled)
        );
        sqlx::raw_sql("DROP TRIGGER fail_toggle")
            .execute(&state.pool)
            .await
            .unwrap();
        toggle_content_entries("instance", &ids, Some(false), &state)
            .await
            .unwrap();
        assert!(
            files::get_instance_files("instance", &state.pool)
                .await
                .unwrap()
                .iter()
                .all(|file| !file.enabled)
        );
        state.pool.close().await;
    }
}

#[tokio::test]
async fn deletion_database_failure_restores_the_original_file() {
    let (_temp, state, root, _) = fixture().await;
    sqlx::raw_sql("CREATE TRIGGER fail_delete BEFORE DELETE ON instance_files BEGIN SELECT RAISE(FAIL, 'injected delete failure'); END;").execute(&state.pool).await.unwrap();
    assert!(
        remove_project("instance", "mods/a.jar", &state)
            .await
            .unwrap_err()
            .to_string()
            .contains("injected delete failure")
    );
    assert_eq!(std::fs::read(root.join("mods/a.jar")).unwrap(), b"a bytes");
    assert_eq!(journal_count(&state).await, 0);
    assert!(
        files::get_instance_file_by_relative_path(
            "instance",
            "mods/a.jar",
            &state.pool
        )
        .await
        .unwrap()
        .is_some()
    );
    state.pool.close().await;
}

#[tokio::test]
async fn restarted_recovery_restores_uncommitted_files_and_finishes_committed_deletion()
 {
    for committed in [false, true] {
        let (_temp, state, root, _) = fixture().await;
        prepare(
            &state,
            "instance",
            &root,
            vec![("mods/a.jar".into(), "mods/a.removing".into())],
            true,
        )
        .await
        .unwrap();
        publish(&state, "instance", &root).await.unwrap();
        if committed {
            let mut tx = state.pool.begin().await.unwrap();
            sqlx::query("DELETE FROM instance_content_entries WHERE file_id IN (SELECT id FROM instance_files WHERE relative_path = 'mods/a.jar')").execute(&mut *tx).await.unwrap();
            sqlx::query(
                "DELETE FROM instance_files WHERE relative_path = 'mods/a.jar'",
            )
            .execute(&mut *tx)
            .await
            .unwrap();
            content_sets::bump_content_set_revision_in_transaction(
                "set", &mut tx,
            )
            .await
            .unwrap();
            mark_committed("instance", &mut tx).await.unwrap();
            tx.commit().await.unwrap();
        }
        let directories = DirectoryInfo {
            settings_dir: state.directories.settings_dir.clone(),
            config_dir: state.directories.config_dir.clone(),
            app_identifier: state.directories.app_identifier.clone(),
        };
        let db = state.pool.connect_options().get_filename().to_owned();
        state.pool.close().await;
        drop(state);
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(db)
                    .foreign_keys(true),
            )
            .await
            .unwrap();
        let reopened =
            crate::state::test_state(directories, pool).await.unwrap();
        recover_content_file_operations(&reopened).await.unwrap();
        assert_eq!(root.join("mods/a.jar").exists(), !committed);
        assert!(!root.join("mods/a.removing").exists());
        assert_eq!(journal_count(&reopened).await, 0);
        assert_eq!(
            files::get_instance_file_by_relative_path(
                "instance",
                "mods/a.jar",
                &reopened.pool
            )
            .await
            .unwrap()
            .is_some(),
            !committed
        );
        assert!(
            sqlx::query("PRAGMA foreign_key_check")
                .fetch_all(&reopened.pool)
                .await
                .unwrap()
                .is_empty()
        );
        reopened.pool.close().await;
    }
}

#[tokio::test]
async fn rollback_conflicts_keep_the_journal_and_do_not_overwrite_external_files()
 {
    let (_temp, state, root, _) = fixture().await;
    prepare(
        &state,
        "instance",
        &root,
        vec![("mods/a.jar".into(), "mods/a.jar.disabled".into())],
        false,
    )
    .await
    .unwrap();
    publish(&state, "instance", &root).await.unwrap();
    std::fs::write(root.join("mods/a.jar"), b"external replacement").unwrap();
    let error = rollback_error(
        &state,
        "instance",
        &root,
        recovery_error("database failure"),
    )
    .await;
    assert!(error.to_string().contains("database failure"));
    assert!(error.to_string().contains("recovery requires retry"));
    assert_eq!(journal_count(&state).await, 1);
    assert_eq!(
        std::fs::read(root.join("mods/a.jar")).unwrap(),
        b"external replacement"
    );
    std::fs::remove_file(root.join("mods/a.jar")).unwrap();
    recover_content_file_operations(&state).await.unwrap();
    assert_eq!(std::fs::read(root.join("mods/a.jar")).unwrap(), b"a bytes");
    assert_eq!(journal_count(&state).await, 0);
    state.pool.close().await;
}

#[tokio::test]
async fn failed_committed_cleanup_stays_successful_and_can_be_retried() {
    let (_temp, state, root, _) = fixture().await;
    prepare(
        &state,
        "instance",
        &root,
        vec![("mods/a.jar".into(), "mods/a.removing".into())],
        true,
    )
    .await
    .unwrap();
    publish(&state, "instance", &root).await.unwrap();
    let mut tx = state.pool.begin().await.unwrap();
    mark_committed("instance", &mut tx).await.unwrap();
    tx.commit().await.unwrap();
    std::fs::write(root.join("mods/a.removing"), b"external content").unwrap();
    finish(&state, "instance", &root).await;
    assert_eq!(journal_count(&state).await, 1);
    assert_eq!(
        std::fs::read(root.join("mods/a.removing")).unwrap(),
        b"external content"
    );
    std::fs::write(root.join("mods/a.removing"), b"a bytes").unwrap();
    recover_content_file_operations(&state).await.unwrap();
    assert!(!root.join("mods/a.removing").exists());
    assert_eq!(journal_count(&state).await, 0);
    state.pool.close().await;
}

#[tokio::test]
async fn a_scan_recovers_a_dropped_toggle_before_indexing_files() {
    let (_temp, state, root, _) = fixture().await;
    prepare(
        &state,
        "instance",
        &root,
        vec![("mods/a.jar".into(), "mods/a.jar.disabled".into())],
        false,
    )
    .await
    .unwrap();
    publish(&state, "instance", &root).await.unwrap();
    let files = sync_content_files("instance", &state).await.unwrap();
    assert!(files.iter().all(|file| file.enabled));
    assert!(root.join("mods/a.jar").exists());
    assert_eq!(journal_count(&state).await, 0);
    assert!(
        instance_rows::get_instance_by_id("instance", &state.pool)
            .await
            .unwrap()
            .is_some()
    );
    state.pool.close().await;
}

#[tokio::test]
async fn a_partial_publish_failure_is_recoverable_without_losing_the_first_file()
 {
    let (_temp, state, root, _) = fixture().await;
    prepare(
        &state,
        "instance",
        &root,
        vec![
            ("mods/a.jar".into(), "mods/a.jar.disabled".into()),
            ("mods/b.jar".into(), "mods/b.jar.disabled".into()),
        ],
        false,
    )
    .await
    .unwrap();
    std::fs::remove_file(root.join("mods/b.jar")).unwrap();
    let failure = publish(&state, "instance", &root).await.unwrap_err();
    assert!(root.join("mods/a.jar.disabled").exists());
    let failure = rollback_error(&state, "instance", &root, failure).await;
    assert!(failure.to_string().contains("recovery requires retry"));
    assert_eq!(journal_count(&state).await, 1);
    std::fs::write(root.join("mods/b.jar"), b"b bytes").unwrap();
    recover_content_file_operations(&state).await.unwrap();
    assert_eq!(std::fs::read(root.join("mods/a.jar")).unwrap(), b"a bytes");
    assert!(!root.join("mods/a.jar.disabled").exists());
    assert_eq!(journal_count(&state).await, 0);
    state.pool.close().await;
}

#[tokio::test]
async fn committed_toggle_is_not_rolled_back_when_journal_cleanup_fails() {
    let (_temp, state, root, _) = fixture().await;
    sqlx::raw_sql("CREATE TRIGGER fail_cleanup BEFORE DELETE ON content_file_operations BEGIN SELECT RAISE(FAIL, 'cleanup failure'); END;")
        .execute(&state.pool).await.unwrap();
    let path =
        toggle_disable_project("instance", "mods/a.jar", Some(false), &state)
            .await
            .unwrap();
    assert_eq!(path, "mods/a.jar.disabled");
    assert_eq!(journal_count(&state).await, 1);
    let file = files::get_instance_file_by_relative_path(
        "instance",
        &path,
        &state.pool,
    )
    .await
    .unwrap()
    .unwrap();
    assert!(!file.enabled);
    sqlx::raw_sql("DROP TRIGGER fail_cleanup")
        .execute(&state.pool)
        .await
        .unwrap();
    recover_content_file_operations(&state).await.unwrap();
    assert!(root.join(path).exists());
    assert!(!root.join("mods/a.jar").exists());
    assert_eq!(journal_count(&state).await, 0);
    state.pool.close().await;
}

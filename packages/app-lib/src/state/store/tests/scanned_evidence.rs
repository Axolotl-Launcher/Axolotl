//! Scanner-owned revision changes preserve only already committed byte evidence.
use super::super::{
    Handle, content_evidence,
    content_refresh_checks::{
        CheckAttempt, ContentRefreshChecks, RefreshCheckContext,
        RefreshCheckKey, RefreshCheckRecord,
    },
    instance_content_files::{self, ScannedFiles},
};
use crate::state::instances::InstanceFile;
use chrono::{TimeZone, Utc};
use std::collections::{HashMap, HashSet};

fn observed(id: &str, path: &str, hash: &str) -> InstanceFile {
    InstanceFile {
        id: id.into(),
        instance_id: "instance".into(),
        relative_path: path.into(),
        file_name: path.rsplit('/').next().unwrap().into(),
        enabled: true,
        sha1: hash.repeat(40),
        size: 3,
        missing: false,
        added_at: Utc.timestamp_opt(1, 0).unwrap(),
        modified_at: Utc.timestamp_opt(1, 0).unwrap(),
        local_mod_data: None,
        icon_path: None,
    }
}

async fn fixture() -> (tempfile::TempDir, Handle, RefreshCheckRecord) {
    let root = tempfile::tempdir().unwrap();
    let pool = super::super::databases::open(root.path()).await.unwrap();
    sqlx::raw_sql("INSERT INTO instances(id,path,name,install_stage,launcher_feature_version,created,modified) VALUES('instance','profile','Example','installed','none',1,1);INSERT INTO instance_content_sets(id,instance_id,name,source_kind,status,game_version,loader,revision,created,modified) VALUES('set','instance','Main','local','available','1.21','fabric',1,1,1);UPDATE instances SET applied_content_set_id='set' WHERE id='instance';INSERT INTO instance_files(id,instance_id,relative_path,file_name,enabled,sha1,size,added_at,modified_at) VALUES('file','instance','mods/a.jar','a.jar',1,'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',3,1,1);INSERT INTO instance_content_entries(id,instance_id,content_set_id,file_id,project_type,source_kind,ownership_kind,server_requirement,client_requirement,enabled,added_at,modified_at) VALUES('entry','instance','set','file','mod','local','user_added','optional','optional',1,1,1);").execute(&pool).await.unwrap();
    let handle = Handle::new(pool);
    let instance = instance_content_files::get_instance(&handle, "instance")
        .await
        .unwrap()
        .unwrap();
    let guard = content_evidence::current(&handle, "instance")
        .await
        .unwrap()
        .remove("file")
        .unwrap();
    let check = RefreshCheckRecord {
        file_id: "file".into(),
        context: RefreshCheckContext {
            content_revision: None,
            location_key: super::super::content_refresh_checks::location_key(
                &instance,
            ),
            content_root: root
                .path()
                .join("profiles/profile")
                .to_string_lossy()
                .into_owned(),
            sha1: "a".repeat(40),
            size: 3,
            current_release: None,
            target: crate::resources::ipc::ResourceTarget {
                game_versions: vec!["1.21".into()],
                loaders: vec!["fabric".into()],
                channels: crate::resources::plans::update_channels(
                    instance.update_channel,
                ),
            },
            preferred_source: crate::resources::SourceId::modrinth(),
        },
        last_success: None,
        attempted_at: 1,
        attempt: CheckAttempt::Unrecognized,
        error: None,
    };
    let mut payload = serde_json::to_value(&check).unwrap();
    payload["future_sibling"] = serde_json::json!({"keep":"exact"});
    content_evidence::try_write_batch(
        &handle,
        "instance",
        vec![content_evidence::Write {
            kind: "check",
            id: "entry".into(),
            guard,
            payload: serde_json::to_string(&payload).unwrap(),
        }],
        content_evidence::generation(&handle).await,
    )
    .await
    .unwrap();
    (root, handle, check)
}

async fn scan(
    handle: &Handle,
    files: &[InstanceFile],
    changed: &HashSet<String>,
) {
    let set = instance_content_files::get_content_set(handle, "instance")
        .await
        .unwrap();
    instance_content_files::set_scanned(
        handle,
        ScannedFiles {
            cache_generation: content_evidence::generation(handle).await,
            complete: true,
            create_entries: true,
            instance_id: "instance",
            content_set: &set,
            files,
            reclaims: &HashMap::new(),
            merges: &HashMap::new(),
            externally_changed_file_ids: changed,
            refreshed_metadata_file_ids: &HashSet::new(),
            fingerprints: &HashMap::new(),
        },
        || async { true },
    )
    .await
    .unwrap()
    .unwrap();
}

#[tokio::test]
async fn adding_an_unrelated_file_retains_exact_completed_evidence_but_late_old_writes_do_not_land()
 {
    let (root, handle, check) = fixture().await;
    let previous = content_evidence::current(&handle, "instance")
        .await
        .unwrap()
        .remove("file")
        .unwrap();
    scan(
        &handle,
        &[
            observed("file", "mods/a.jar", "a"),
            observed("new", "mods/new.jar", "b"),
        ],
        &HashSet::new(),
    )
    .await;
    let current = content_evidence::current(&handle, "instance")
        .await
        .unwrap()
        .remove("file")
        .unwrap();
    assert_ne!(previous.revision, current.revision);
    let key = RefreshCheckKey {
        instance_id: "instance".into(),
        entry_id: "entry".into(),
    };
    assert_eq!(
        ContentRefreshChecks::get(&handle, &key).await.unwrap(),
        Some(check.clone())
    );
    let mut late = check.clone();
    late.attempted_at = 999;
    content_evidence::write(&handle, "check", "entry", &previous, &late).await;
    assert_eq!(
        ContentRefreshChecks::get(&handle, &key).await.unwrap(),
        Some(check)
    );
    let cache = sqlx::SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new()
            .filename(root.path().join("cache/resources.db")),
    )
    .await
    .unwrap();
    let payload:String=sqlx::query_scalar("SELECT payload FROM content_evidence WHERE kind='check' AND record_id='entry'").fetch_one(&cache).await.unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&payload).unwrap()["future_sibling"],
        serde_json::json!({"keep":"exact"})
    );
    cache.close().await;
}

#[tokio::test]
async fn changed_bytes_and_changed_entry_association_cannot_inherit_a_completed_result()
 {
    for association_changed in [false, true] {
        let (_root, handle, _) = fixture().await;
        let mut old = observed("file", "mods/a.jar", "a");
        if association_changed {
            sqlx::query("UPDATE instance_content_entries SET file_id=NULL WHERE id='entry'").execute(handle.pool()).await.unwrap();
        } else {
            old.sha1 = "c".repeat(40);
        }
        let changed = if association_changed {
            HashSet::new()
        } else {
            HashSet::from(["file".into()])
        };
        scan(
            &handle,
            &[old, observed("new", "mods/new.jar", "b")],
            &changed,
        )
        .await;
        assert!(
            ContentRefreshChecks::get(
                &handle,
                &RefreshCheckKey {
                    instance_id: "instance".into(),
                    entry_id: "entry".into()
                }
            )
            .await
            .unwrap()
            .is_none()
        );
    }
}

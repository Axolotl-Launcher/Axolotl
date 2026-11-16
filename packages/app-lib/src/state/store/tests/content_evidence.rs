//! Evidence is disposable; identity and ownership remain in the core.
use crate::state::store::{
    self, Handle,
    content_evidence::{self, Guard, Parsed},
};
use serde_json::json;

async fn fixture() -> (tempfile::TempDir, Handle) {
    let directory = tempfile::tempdir().unwrap();
    let core = store::databases::open(directory.path()).await.unwrap();
    sqlx::raw_sql("INSERT INTO instances(id,path,install_stage,launcher_feature_version,name,created,modified) VALUES('instance','profile','installed','none','Example',0,0);INSERT INTO instance_content_sets(id,instance_id,name,source_kind,status,game_version,loader,created,modified) VALUES('set','instance','Main','local','available','1.21','fabric',0,0);UPDATE instances SET applied_content_set_id='set' WHERE id='instance';INSERT INTO instance_files(id,instance_id,relative_path,file_name,enabled,sha1,size,missing,added_at,modified_at) VALUES('file','instance','mods/a.jar','a.jar',1,'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',3,0,0,0);INSERT INTO instance_content_entries(id,instance_id,content_set_id,file_id,project_type,source_kind,ownership_kind,server_requirement,client_requirement,added_at,modified_at) VALUES('entry','instance','set','file','mod','local','user_added','optional','optional',0,0);").execute(&core).await.unwrap();
    (directory, Handle::new(core))
}
async fn guard(handle: &Handle) -> Guard {
    content_evidence::current(handle, "instance")
        .await
        .unwrap()
        .remove("file")
        .unwrap()
}

#[tokio::test]
async fn late_evidence_never_describes_a_new_revision_or_location() {
    let (_directory, handle) = fixture().await;
    let old = guard(&handle).await;
    let parsed = Parsed {
        metadata: Some("old metadata".into()),
        icon: None,
    };
    content_evidence::write(&handle, "parsed", "file", &old, &parsed).await;
    assert!(
        content_evidence::read::<Parsed>(
            &handle, "parsed", "instance", "file", &old
        )
        .await
        .is_some()
    );
    sqlx::query(
        "UPDATE instance_content_sets SET revision=revision+1 WHERE id='set'",
    )
    .execute(handle.pool())
    .await
    .unwrap();
    let current = guard(&handle).await;
    assert_ne!(old, current);
    content_evidence::write(&handle, "parsed", "file", &old, &parsed).await;
    assert!(
        content_evidence::read::<Parsed>(
            &handle, "parsed", "instance", "file", &current
        )
        .await
        .is_none()
    );
    sqlx::query("UPDATE instances SET game_dir_override='changed-root' WHERE id='instance'").execute(handle.pool()).await.unwrap();
    assert_ne!(current, guard(&handle).await);
}

#[tokio::test]
async fn cache_loss_and_newer_payload_do_not_remove_core_relationships() {
    let (directory, handle) = fixture().await;
    let current = guard(&handle).await;
    content_evidence::write(
        &handle,
        "parsed",
        "file",
        &current,
        &json!({"schemaVersion":2,"metadata":"future","icon":null}),
    )
    .await;
    assert!(
        content_evidence::read::<Parsed>(
            &handle, "parsed", "instance", "file", &current
        )
        .await
        .is_none()
    );
    content_evidence::remove(&handle, "parsed", "instance", "file").await;
    assert!(
        content_evidence::read::<Parsed>(
            &handle, "parsed", "instance", "file", &current
        )
        .await
        .is_none()
    );
    let owner: String = sqlx::query_scalar(
        "SELECT ownership_kind FROM instance_content_entries WHERE id='entry'",
    )
    .fetch_one(handle.pool())
    .await
    .unwrap();
    assert_eq!(owner, "user_added");
    assert!(directory.path().join("databases/derived.db").is_file());
}

#[tokio::test]
async fn inaccessible_optional_cache_leaves_the_core_usable() {
    let (directory, handle) = fixture().await;
    tokio::fs::write(
        directory.path().join("databases/derived.db"),
        b"not SQLite",
    )
    .await
    .unwrap();
    let current = guard(&handle).await;
    assert!(
        content_evidence::read::<Parsed>(
            &handle, "parsed", "instance", "file", &current
        )
        .await
        .is_none()
    );
    content_evidence::write(
        &handle,
        "parsed",
        "file",
        &current,
        &Parsed {
            metadata: Some("optional".into()),
            icon: None,
        },
    )
    .await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM instance_files")
            .fetch_one(handle.pool())
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        tokio::fs::read(directory.path().join("databases/derived.db"))
            .await
            .unwrap(),
        b"not SQLite"
    );
}

#[tokio::test]
async fn native_facts_do_not_adopt_files_or_delete_ownership() {
    let (_directory, handle) = fixture().await;
    let instance =
        store::instance_content_files::get_instance(&handle, "instance")
            .await
            .unwrap()
            .unwrap();
    let mut files =
        store::instance_content_files::get_files(&handle, "instance")
            .await
            .unwrap();
    let root = tempfile::tempdir().unwrap();
    let observed =
        crate::state::instances::adapters::filesystem::ScannedContentFile {
            relative_path: "mods/new.jar".into(),
            file_name: "new.jar".into(),
            enabled: true,
            size: 999,
            modified: Some(std::time::SystemTime::UNIX_EPOCH),
            hash_cache_key: String::new(),
        };
    let current = crate::state::instances::content_facts::apply(
        &handle,
        &instance,
        root.path().to_owned(),
        &mut files,
        vec![observed],
    )
    .await
    .unwrap();
    assert!(current.is_empty());
    assert_eq!(files.len(), 2);
    assert!(files.iter().any(|file| file.id == "file" && file.missing));
    assert!(files.iter().any(|file| file.relative_path == "mods/new.jar"
        && file.size == 999
        && file.sha1.is_empty()
        && !file.missing));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM instance_files")
            .fetch_one(handle.pool())
            .await
            .unwrap(),
        1
    );
    assert_eq!(sqlx::query_scalar::<_,String>("SELECT ownership_kind FROM instance_content_entries WHERE id='entry'").fetch_one(handle.pool()).await.unwrap(),"user_added");
}

#[tokio::test]
async fn inaccessible_native_root_is_an_error_not_an_empty_scan() {
    let (_directory, handle) = fixture().await;
    let root = tempfile::tempdir().unwrap();
    let blocked = root.path().join("unreadable-root");
    std::fs::write(&blocked, b"not a directory").unwrap();
    assert!(
        crate::state::instances::adapters::filesystem::scan_content_files_from(
            &blocked, ""
        )
        .is_err()
    );
    assert!(
        crate::state::instances::adapters::filesystem::scan_content_files_from(
            &root.path().join("missing"),
            ""
        )
        .is_err()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM instance_content_entries WHERE file_id='file'"
        )
        .fetch_one(handle.pool())
        .await
        .unwrap(),
        1
    );
}

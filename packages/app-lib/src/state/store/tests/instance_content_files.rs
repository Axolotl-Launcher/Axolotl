//! Complete and progressive scan reconciliation preserve content ownership.

use super::support;
use crate::state::instances::InstanceFile;
use crate::state::instances::adapters::sqlite;
use crate::state::store::{Handle, instance_content_files};
use chrono::{TimeZone, Utc};
use std::collections::{HashMap, HashSet};

fn file(instance: &str, id: &str, path: &str) -> InstanceFile {
    InstanceFile {
        id: id.into(),
        instance_id: instance.into(),
        relative_path: path.into(),
        file_name: path.rsplit('/').next().unwrap().into(),
        enabled: !path.ends_with(".disabled"),
        sha1: format!("hash-{id}"),
        size: 42,
        missing: false,
        added_at: Utc.timestamp_opt(12, 0).unwrap(),
        modified_at: Utc.timestamp_opt(34, 0).unwrap(),
        local_mod_data: Some("{\"name\":\"local metadata\"}".into()),
        icon_path: Some("cached-icon.png".into()),
    }
}

async fn seed() -> Handle {
    let handle = support::handle_with_instances(&["instance", "other"]).await;
    for instance in ["instance", "other"] {
        sqlx::query("INSERT INTO instance_content_sets(id, instance_id, name, source_kind, status, game_version, loader, created, modified) VALUES (?, ?, 'Example', 'local', 'available', '1.20.1', 'fabric', 1, 1)")
            .bind(format!("set-{instance}")).bind(instance)
            .execute(handle.pool()).await.unwrap();
    }
    for (id, path) in [
        ("tracked", "mods/example.jar"),
        ("deleted", "schematics/old/house.litematic"),
        ("world", "saves/My World/level.dat"),
        ("plugin", "plugins/example.jar"),
        ("disabled-plugin", "plugins/example.jar.disabled"),
        ("sidecar", "shaderpacks/BSL.zip.txt"),
        ("backup", "mods/new.jar_old.jar.old"),
    ] {
        let mut record = file("instance", id, path);
        record.missing = id == "deleted";
        let mut tx = handle.pool().begin().await.unwrap();
        sqlite::files::upsert_instance_file(&record, &mut tx)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        add_entry(&handle, &record).await;
    }
    let record = file("other", "other-sidecar", "shaderpacks/BSL.zip.txt");
    let mut tx = handle.pool().begin().await.unwrap();
    sqlite::files::upsert_instance_file(&record, &mut tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    add_entry(&handle, &record).await;
    sqlx::query("UPDATE instances SET applied_content_set_id = ? WHERE id = ?")
        .bind("set-instance")
        .bind("instance")
        .execute(handle.pool())
        .await
        .unwrap();
    handle
}

async fn add_entry(handle: &Handle, key: &InstanceFile) {
    sqlx::query("INSERT INTO instance_content_entries(id, instance_id, content_set_id, file_id, project_type, source_kind, ownership_kind, server_requirement, client_requirement, enabled, added_at, modified_at) VALUES (?, ?, ?, ?, 'mod', 'local', 'user_added', 'required', 'required', 1, 12, 34)")
        .bind(format!("entry-{}", key.id)).bind(&key.instance_id)
        .bind(format!("set-{}", key.instance_id)).bind(&key.id)
        .execute(handle.pool()).await.unwrap();
    sqlx::query("INSERT INTO instance_content_provider_refs(content_entry_id, provider, provider_project_id, provider_release_id, is_origin) VALUES (?, 'modrinth', 'pluginproject', 'release', 1)")
        .bind(format!("entry-{}", key.id)).execute(handle.pool()).await.unwrap();
    sqlx::query("INSERT INTO resource_installed_artifacts(entry_id, artifact, scope) VALUES (?, 'exact-artifact', 'plugin-scope')")
        .bind(format!("entry-{}", key.id)).execute(handle.pool()).await.unwrap();
    sqlx::query("INSERT INTO resource_entry_graphs(entry_id, nodes, edges) VALUES (?, 'nodes', 'edges')")
        .bind(format!("entry-{}", key.id)).execute(handle.pool()).await.unwrap();
}

async fn metadata(
    handle: &Handle,
) -> Vec<(String, String, String, String, String, String)> {
    sqlx::query_as("SELECT entry.id, ref.provider_release_id, artifact.artifact, artifact.scope, graph.nodes, graph.edges FROM instance_content_entries entry JOIN instance_content_provider_refs ref ON ref.content_entry_id = entry.id JOIN resource_installed_artifacts artifact ON artifact.entry_id = entry.id JOIN resource_entry_graphs graph ON graph.entry_id = entry.id ORDER BY entry.id")
        .fetch_all(handle.pool()).await.unwrap()
}

async fn reconcile(
    handle: &Handle,
    complete: bool,
    current: bool,
    files: &[InstanceFile],
) -> Option<Vec<InstanceFile>> {
    let content_set =
        instance_content_files::get_content_set(handle, "instance")
            .await
            .unwrap();
    instance_content_files::set_scanned(
        handle,
        instance_content_files::ScannedFiles {
            complete,
            create_entries: false,
            instance_id: "instance",
            content_set: &content_set,
            files,
            reclaims: &HashMap::new(),
            merges: &HashMap::new(),
            externally_changed_file_ids: &HashSet::new(),
            refreshed_metadata_file_ids: &HashSet::new(),
            fingerprints: &HashMap::new(),
        },
        || async move { current },
    )
    .await
    .unwrap()
}

async fn all_files(handle: &Handle) -> Vec<InstanceFile> {
    let mut files = instance_content_files::get_files(handle, "instance")
        .await
        .unwrap();
    files.extend(
        instance_content_files::get_files(handle, "other")
            .await
            .unwrap(),
    );
    files
}

#[tokio::test]
async fn scan_scope_cleanup_preserves_independently_tracked_content_and_metadata()
 {
    let handle = seed().await;
    let before = metadata(&handle).await;
    let before_files = all_files(&handle).await;
    let revision = instance_content_files::get_content_set(&handle, "instance")
        .await
        .unwrap()
        .unwrap()
        .revision;
    assert!(reconcile(&handle, true, true, &[]).await.is_some());

    let mut ids: Vec<_> = all_files(&handle)
        .await
        .into_iter()
        .map(|record| {
            let mut original = before_files
                .iter()
                .find(|old| old.id == record.id)
                .unwrap()
                .clone();
            if record.instance_id == "instance" {
                // Complete scans mark unreported files missing before cleanup.
                // Retaining independently managed records preserves their identity
                // and metadata, while that existing scan bookkeeping still runs.
                original.missing = true;
                original.modified_at = record.modified_at;
            }
            assert_eq!(
                serde_json::to_value(&record).unwrap(),
                serde_json::to_value(original).unwrap()
            );
            record.id
        })
        .collect();
    ids.sort();
    assert_eq!(
        ids,
        [
            "deleted",
            "disabled-plugin",
            "other-sidecar",
            "plugin",
            "tracked",
            "world"
        ]
    );
    let expected: Vec<_> = before
        .into_iter()
        .filter(|row| row.0 != "entry-sidecar" && row.0 != "entry-backup")
        .collect();
    assert_eq!(metadata(&handle).await, expected);
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM instance_content_entries")
            .fetch_one(handle.pool())
            .await
            .unwrap();
    assert_eq!(
        count, 6,
        "stale entries must be deleted instead of detached"
    );
    assert_eq!(
        instance_content_files::get_content_set(&handle, "instance")
            .await
            .unwrap()
            .unwrap()
            .revision,
        revision + 1
    );
    assert!(
        sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(handle.pool())
            .await
            .unwrap()
            .is_empty()
    );

    // A second cleanup has no entries left to remove and must not invalidate
    // the set revision again.
    reconcile(&handle, true, true, &[]).await.unwrap();
    assert_eq!(
        instance_content_files::get_content_set(&handle, "instance")
            .await
            .unwrap()
            .unwrap()
            .revision,
        revision + 1
    );
}

#[tokio::test]
async fn scan_scope_cleanup_rolls_back_with_upserts_and_revision_on_stale_snapshot()
 {
    let handle = seed().await;
    let before = metadata(&handle).await;
    let before_files = serde_json::to_value(all_files(&handle).await).unwrap();
    let revision = instance_content_files::get_content_set(&handle, "instance")
        .await
        .unwrap()
        .unwrap()
        .revision;
    let mut scanned = file("instance", "tracked", "mods/example.jar");
    scanned.sha1 = "pending-new-hash".into();
    assert!(reconcile(&handle, true, false, &[scanned]).await.is_none());
    assert_eq!(metadata(&handle).await, before);
    assert_eq!(
        serde_json::to_value(all_files(&handle).await).unwrap(),
        before_files
    );
    assert_eq!(
        instance_content_files::get_content_set(&handle, "instance")
            .await
            .unwrap()
            .unwrap()
            .revision,
        revision
    );
}

#[tokio::test]
async fn scan_scope_cleanup_waits_for_complete_scan() {
    let handle = seed().await;
    let before = metadata(&handle).await;
    let before_files = serde_json::to_value(all_files(&handle).await).unwrap();
    let revision = instance_content_files::get_content_set(&handle, "instance")
        .await
        .unwrap()
        .unwrap()
        .revision;
    assert!(reconcile(&handle, false, true, &[]).await.is_some());
    assert_eq!(metadata(&handle).await, before);
    assert_eq!(
        serde_json::to_value(all_files(&handle).await).unwrap(),
        before_files
    );
    assert_eq!(
        instance_content_files::get_content_set(&handle, "instance")
            .await
            .unwrap()
            .unwrap()
            .revision,
        revision
    );
}

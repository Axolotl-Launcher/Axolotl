//! What the crash analysis snapshots promise: one instance's mods are replaced
//! as a whole, and another instance's are left alone.

use super::support;
use crate::state::store::Collection as _;
use crate::state::store::crash_analysis_mod_snapshots::{
    CrashAnalysisModSnapshotKey, CrashAnalysisModSnapshotRecord,
    CrashAnalysisModSnapshots,
};

fn entry() -> CrashAnalysisModSnapshotRecord {
    CrashAnalysisModSnapshotRecord {
        size: 1024,
        sha256: "digest".to_string(),
        project_id: Some("project".to_string()),
        project_title: Some("Project".to_string()),
        icon_url: None,
        version_id: Some("version".to_string()),
        version_number: None,
    }
}

fn key(instance_id: &str, filename: &str) -> CrashAnalysisModSnapshotKey {
    CrashAnalysisModSnapshotKey {
        instance_id: instance_id.to_string(),
        filename: filename.to_string(),
    }
}

#[tokio::test]
async fn a_snapshot_round_trips_per_instance() {
    let (_directory, handle) = support::derived().await;
    let instance_id = "instance".to_string();
    let stored = key(&instance_id, "example.jar");

    CrashAnalysisModSnapshots::set_scoped(
        &handle,
        &instance_id,
        &[(stored.clone(), entry())],
    )
    .await
    .unwrap();

    assert_eq!(
        CrashAnalysisModSnapshots::get_scoped(&handle, &instance_id)
            .await
            .unwrap(),
        vec![(stored.clone(), entry())]
    );
    assert_eq!(
        CrashAnalysisModSnapshots::get(&handle, &stored)
            .await
            .unwrap(),
        Some(entry())
    );

    CrashAnalysisModSnapshots::set_scoped(&handle, &instance_id, &[])
        .await
        .unwrap();
    assert!(
        CrashAnalysisModSnapshots::get_scoped(&handle, &instance_id)
            .await
            .unwrap()
            .is_empty()
    );
}

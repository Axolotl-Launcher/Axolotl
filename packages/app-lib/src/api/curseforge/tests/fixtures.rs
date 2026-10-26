//! Fixtures shared by the CurseForge tests.

use crate::State;
use crate::api::curseforge::constants::MINECRAFT_GAME_ID;
use crate::api::curseforge::dto::{
    CurseForgeCategory, CurseForgeFile, CurseForgeFileHash,
    CurseForgeManualDownload,
};
use crate::api::curseforge::manual::{
    CurseForgeManualDownloadIntegrityMetadata,
    CurseForgeManualDownloadReconciliation,
    curseforge_manual_download_reconciliation,
    import_pending_manual_download_file,
};
use crate::install::InstallJobEventKind;
use crate::state::{InstanceLink, ModLoader};
use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};

#[cfg(test)]
pub(super) fn murmur2(data: &[u8], seed: u32) -> u32 {
    const M: u32 = 0x5bd1e995;
    const R: u32 = 24;
    let mut hash = seed ^ data.len() as u32;
    let mut chunks = data.chunks_exact(4);

    for chunk in &mut chunks {
        let mut value =
            u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        value = value.wrapping_mul(M);
        value ^= value >> R;
        value = value.wrapping_mul(M);
        hash = hash.wrapping_mul(M);
        hash ^= value;
    }

    match chunks.remainder() {
        [a, b, c] => {
            hash ^= (*c as u32) << 16;
            hash ^= (*b as u32) << 8;
            hash ^= *a as u32;
            hash = hash.wrapping_mul(M);
        }
        [a, b] => {
            hash ^= (*b as u32) << 8;
            hash ^= *a as u32;
            hash = hash.wrapping_mul(M);
        }
        [a] => {
            hash ^= *a as u32;
            hash = hash.wrapping_mul(M);
        }
        [] => {}
        _ => unreachable!(),
    }

    hash ^= hash >> 13;
    hash = hash.wrapping_mul(M);
    hash ^= hash >> 15;
    hash
}

pub(super) fn write_override_test_archive(
    path: &Path,
    entries: &[(&str, &[u8])],
) {
    let file = std::fs::File::create(path).unwrap();
    let mut archive = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored);
    archive.start_file("manifest.json", options).unwrap();
    archive
        .write_all(
            br#"{"minecraft":{"version":"1.21.1","modLoaders":[]},"files":[],"overrides":"overrides"}"#,
        )
        .unwrap();
    for (name, contents) in entries {
        archive
            .start_file(format!("overrides/{name}"), options)
            .unwrap();
        archive.write_all(contents).unwrap();
    }
    archive.finish().unwrap();
}

pub(super) fn write_override_size_test_archive(
    path: &Path,
    entries: &[(&str, u32)],
) {
    let contents = entries
        .iter()
        .map(|(name, _)| (*name, b"x".as_slice()))
        .collect::<Vec<_>>();
    write_override_test_archive(path, &contents);
    let mut bytes = std::fs::read(path).unwrap();
    let headers = bytes
        .windows(4)
        .enumerate()
        .filter_map(|(offset, signature)| {
            (signature == b"PK\x01\x02").then_some(offset)
        })
        .collect::<Vec<_>>();
    assert_eq!(headers.len(), entries.len() + 1);
    for (offset, (_, size)) in headers.into_iter().skip(1).zip(entries) {
        bytes[offset + 24..offset + 28].copy_from_slice(&size.to_le_bytes());
    }
    std::fs::write(path, bytes).unwrap();
}

pub(super) fn skipped_curseforge_manual_item(
    id: &str,
    project_id: &str,
    version_id: &str,
) -> crate::install::model::DownloadItemSnapshot {
    crate::install::model::DownloadItemSnapshot {
        id: id.to_string(),
        name: id.to_string(),
        project_id: Some(project_id.to_string()),
        version_id: Some(version_id.to_string()),
        status: crate::install::model::DownloadItemStatus::Skipped,
        bytes_downloaded: 0,
        bytes_total: Some(42),
        attempt: Some(1),
        max_attempts: Some(1),
        error: Some("CurseForge requires manual download".to_string()),
        manual_url: Some("https://www.curseforge.com/download".to_string()),
        request_url: None,
        source: None,
    }
}

pub(super) fn waiting_manual_job_record(
    instance_id: &str,
    request: crate::install::model::InstallRequest,
) -> crate::install::store::InstallJobRecord {
    let mut state = crate::install::model::InstallJobState::new(request);
    state.record_event(InstallJobEventKind::ContentFileSkipped {
        path: "mods/one.jar".to_string(),
        reason: "manual download required".to_string(),
        project_id: Some("1".to_string()),
        version_id: Some("10".to_string()),
        manual_url: Some("https://www.curseforge.com/download".to_string()),
    });
    state.record_event(InstallJobEventKind::WaitingForUser {
        reason:
            crate::install::model::InstallPauseReason::MissingRequiredContent {
                failed_files: 1,
                paths: vec!["mods/one.jar".to_string()],
            },
    });
    state.pause_reason = Some(
        crate::install::model::InstallPauseReason::MissingRequiredContent {
            failed_files: 1,
            paths: vec!["mods/one.jar".to_string()],
        },
    );
    let now = chrono::Utc::now();
    crate::install::store::InstallJobRecord {
        id: uuid::Uuid::new_v4(),
        instance_id: Some(instance_id.to_string()),
        kind: crate::install::model::InstallJobKind::InstallContent,
        status: crate::install::model::InstallJobStatus::WaitingForUser,
        state,
        created: now,
        modified: now,
        finished: None,
        dismissed: false,
    }
}

pub(super) fn apply_recovered_events(
    items: &mut [crate::install::model::DownloadItemSnapshot],
    events: &[InstallJobEventKind],
) {
    for event in events {
        let InstallJobEventKind::ContentFileRecovered { path, bytes } = event
        else {
            continue;
        };
        let item = items.iter_mut().find(|item| item.id == *path).unwrap();
        item.status = crate::install::model::DownloadItemStatus::Completed;
        item.bytes_downloaded = *bytes;
    }
}

pub(super) fn reconcile_and_simulate_resume(
    items: &mut [crate::install::model::DownloadItemSnapshot],
    pending: &HashSet<(String, String)>,
    materialized: &HashSet<(String, String)>,
    resume_count: &mut usize,
) -> CurseForgeManualDownloadReconciliation {
    let result =
        curseforge_manual_download_reconciliation(items, pending, materialized);
    apply_recovered_events(items, &result.recovered);
    let latest =
        curseforge_manual_download_reconciliation(items, pending, materialized);
    if latest.should_resume() {
        *resume_count += 1;
    }
    result
}

#[cfg(not(feature = "tauri"))]
pub(super) async fn stage6_state() -> std::sync::Arc<State> {
    crate::event::EventState::init().await.unwrap();
    let state_root = tempfile::tempdir().unwrap().keep();
    State::init_for_test(state_root.to_string_lossy().to_string())
        .await
        .unwrap()
}

#[cfg(not(feature = "tauri"))]
pub(super) async fn create_stage6_instance(
    label: &str,
) -> (std::sync::Arc<State>, String) {
    let state = stage6_state().await;
    let created = crate::api::instance::create(
        format!("Stage 6 {label} {}", uuid::Uuid::new_v4()),
        "1.20.1".to_string(),
        ModLoader::Vanilla,
        None,
        None,
        InstanceLink::Unmanaged,
        None,
        None,
    )
    .await
    .unwrap();
    (state, created.instance.id)
}

#[cfg(not(feature = "tauri"))]
pub(super) fn stage6_manual_download(
    project_id: u32,
    file_id: u32,
    file_name: &str,
    expected_bytes: &[u8],
) -> CurseForgeManualDownload {
    CurseForgeManualDownload {
        project_id,
        file_id,
        file_name: file_name.to_string(),
        ownership_kind:
            crate::state::instances::ContentOwnershipKind::PackManaged,
        operation_kind:
            crate::state::instances::ManualDownloadOperationKind::PackInstall,
        website_url: None,
        project_type: "mod".to_string(),
        project_slug: format!("stage-6-{project_id}-{file_id}"),
        target_folder: "mods".to_string(),
        hashes: vec![CurseForgeFileHash {
            value: sha1_smol::Sha1::from(expected_bytes).hexdigest(),
            algo: 1,
        }],
        file_length: expected_bytes.len() as u64,
        file_fingerprint: 0,
    }
}

pub(super) fn stage8_legacy_manual_download(
    project_id: u32,
    file_id: u32,
    file_name: &str,
) -> CurseForgeManualDownload {
    CurseForgeManualDownload {
        project_id,
        file_id,
        file_name: file_name.to_string(),
        ownership_kind:
            crate::state::instances::ContentOwnershipKind::PackManaged,
        operation_kind:
            crate::state::instances::ManualDownloadOperationKind::PackInstall,
        website_url: None,
        project_type: "mod".to_string(),
        project_slug: format!("stage-8-{project_id}-{file_id}"),
        target_folder: "mods".to_string(),
        hashes: Vec::new(),
        file_length: 0,
        file_fingerprint: 0,
    }
}

pub(super) fn stage8_integrity_metadata(
    download: &CurseForgeManualDownload,
    hashes: Vec<CurseForgeFileHash>,
    file_length: u64,
    file_fingerprint: u64,
) -> CurseForgeManualDownloadIntegrityMetadata {
    CurseForgeManualDownloadIntegrityMetadata {
        project_id: download.project_id,
        file_id: download.file_id,
        hashes,
        file_length,
        file_fingerprint,
    }
}

pub(super) fn stage8_curseforge_file(
    project_id: u32,
    file_id: u32,
    file_name: &str,
    file_length: u64,
    hashes: Vec<CurseForgeFileHash>,
    file_fingerprint: u64,
) -> CurseForgeFile {
    CurseForgeFile {
        id: file_id,
        game_id: MINECRAFT_GAME_ID,
        mod_id: project_id,
        is_available: true,
        display_name: file_name.to_string(),
        file_name: file_name.to_string(),
        release_type: 1,
        file_status: 4,
        hashes,
        file_date: String::new(),
        file_length,
        download_count: 0,
        file_size_on_disk: Some(file_length),
        download_url: None,
        game_versions: Vec::new(),
        sortable_game_versions: Vec::new(),
        dependencies: Vec::new(),
        expose_as_alternative: None,
        parent_project_file_id: None,
        alternate_file_id: None,
        is_server_pack: None,
        server_pack_file_id: None,
        is_early_access_content: None,
        early_access_end_date: None,
        file_fingerprint,
        modules: Vec::new(),
    }
}

#[cfg(not(feature = "tauri"))]
pub(super) async fn stage8_pending_keys(
    state: &State,
    instance_id: &str,
) -> HashSet<(String, String)> {
    crate::state::instances::adapters::sqlite::content_rows::get_pending_manual_downloads(
        instance_id,
        &state.pool,
    )
    .await
    .unwrap()
    .into_iter()
    .map(|download| {
        (
            download.provider_project_id,
            download.provider_release_id,
        )
    })
    .collect()
}

#[cfg(not(feature = "tauri"))]
pub(super) async fn import_stage6_manual_download(
    instance_id: &str,
    download: &CurseForgeManualDownload,
    bytes: &[u8],
) {
    let source_directory = tempfile::tempdir().unwrap();
    let source = source_directory.path().join(&download.file_name);
    crate::util::io::write(&source, bytes).await.unwrap();
    import_pending_manual_download_file(
        instance_id,
        download.project_id,
        download.file_id,
        source,
    )
    .await
    .unwrap();
}

#[cfg(not(feature = "tauri"))]
pub(super) fn stage6_waiting_manual_job_state(
    instance_id: &str,
    items: &[(&str, &str, &str)],
) -> crate::install::model::InstallJobState {
    let mut state = crate::install::model::InstallJobState::new(
        crate::install::model::InstallRequest::InstallPackToExistingInstance {
            instance_id: instance_id.to_string(),
            location:
                crate::api::pack::install_from::CreatePackLocation::FromFile {
                    path: PathBuf::from(format!(
                        "missing-stage-6-{}.mrpack",
                        uuid::Uuid::new_v4()
                    )),
                },
            post_install_edit: None,
        },
    );
    let paths = items
        .iter()
        .map(|(path, _, _)| (*path).to_string())
        .collect::<Vec<_>>();
    for (path, project_id, file_id) in items {
        state.record_event(InstallJobEventKind::ContentFileQueued {
            path: (*path).to_string(),
            bytes_total: Some(42),
            max_attempts: 1,
        });
        state.record_event(InstallJobEventKind::ContentFileSkipped {
            path: (*path).to_string(),
            reason: "manual download required".to_string(),
            project_id: Some((*project_id).to_string()),
            version_id: Some((*file_id).to_string()),
            manual_url: Some("https://www.curseforge.com/download".to_string()),
        });
    }
    let reason =
        crate::install::model::InstallPauseReason::MissingRequiredContent {
            failed_files: paths.len() as u64,
            paths,
        };
    state.pause_reason = Some(reason.clone());
    state.record_event(InstallJobEventKind::WaitingForUser { reason });
    state
}

#[cfg(not(feature = "tauri"))]
pub(super) async fn insert_stage6_waiting_manual_job(
    state: &State,
    instance_id: &str,
    items: &[(&str, &str, &str)],
) -> uuid::Uuid {
    let job_id = uuid::Uuid::new_v4();
    let job_state = stage6_waiting_manual_job_state(instance_id, items);
    crate::install::store::insert(
        job_id,
        &job_state,
        crate::install::model::InstallJobStatus::WaitingForUser,
        state,
    )
    .await
    .unwrap();
    job_id
}

#[cfg(not(feature = "tauri"))]
pub(super) async fn assert_stage6_job_is_unresolved(
    state: &State,
    job_id: uuid::Uuid,
    item_path: &str,
) {
    let job = crate::install::store::get_required(job_id, state)
        .await
        .unwrap();
    assert_eq!(
        job.status,
        crate::install::model::InstallJobStatus::WaitingForUser
    );
    assert_eq!(
        job.snapshot()
            .items
            .iter()
            .find(|item| item.id == item_path)
            .unwrap()
            .status,
        crate::install::model::DownloadItemStatus::Skipped
    );
    assert_eq!(
        job.state
            .events
            .iter()
            .filter(|event| matches!(
                event.kind,
                InstallJobEventKind::ContentFileRecovered { .. }
            ))
            .count(),
        0
    );
}

pub(super) fn category(
    id: u32,
    class_id: Option<u32>,
    is_class: bool,
) -> CurseForgeCategory {
    CurseForgeCategory {
        id,
        game_id: MINECRAFT_GAME_ID,
        name: id.to_string(),
        slug: id.to_string(),
        url: String::new(),
        icon_url: None,
        date_modified: String::new(),
        is_class: Some(is_class),
        class_id,
        parent_category_id: class_id,
        display_index: Some(0),
    }
}

//! Cheap native facts are observations, never durable ownership records.
use super::Instance;
use crate::state::instances::InstanceFile;
use crate::state::store::{
    Handle,
    content_file_fingerprints::{ContentFileFingerprints, FileModifiedTime},
};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};

pub(crate) async fn overlay(
    handle: &Handle,
    instance: &Instance,
    directories: &crate::state::DirectoryInfo,
    files: &mut Vec<InstanceFile>,
) -> crate::Result<HashSet<String>> {
    let root = super::commands::instance_content_root(directories, instance)?;
    let scan_root = root.clone();
    let scanned = tokio::task::spawn_blocking(move || {
        super::adapters::filesystem::scan_content_files_from(&scan_root, "")
    })
    .await??;
    apply(handle, instance, root, files, scanned).await
}
pub(crate) async fn apply(
    handle: &Handle,
    instance: &Instance,
    root: std::path::PathBuf,
    files: &mut Vec<InstanceFile>,
    scanned: Vec<super::adapters::filesystem::ScannedContentFile>,
) -> crate::Result<HashSet<String>> {
    let observations = scanned
        .into_iter()
        .map(|file| (file.relative_path.clone(), file))
        .collect::<HashMap<_, _>>();
    let mut represented = HashSet::new();
    let mut current = HashSet::new();
    let fingerprints =
        ContentFileFingerprints::get_scoped(handle, &instance.id)
            .await?
            .into_iter()
            .map(|(key, record)| (key.file_id, record))
            .collect::<HashMap<_, _>>();
    for file in files.iter_mut() {
        if let Some(observed) = observations.get(&file.relative_path) {
            represented.insert(file.relative_path.clone());
            file.missing = false;
            file.file_name = observed.file_name.clone();
            file.enabled = observed.enabled;
            file.size = observed.size;
            if let Some(modified) = observed.modified {
                file.modified_at = modified.into();
            }
            let valid = fingerprints.get(&file.id).is_some_and(|record| {
                record.matches(
                    &root.to_string_lossy(),
                    &file.relative_path,
                    observed.size,
                    observed
                        .modified
                        .and_then(FileModifiedTime::from_system_time)
                        .as_ref(),
                )
            });
            if valid {
                current.insert(file.id.clone());
            } else {
                file.sha1.clear();
                file.local_mod_data = None;
                file.icon_path = None;
            }
        } else {
            file.missing = true;
            file.local_mod_data = None;
            file.icon_path = None;
        }
    }
    for (path, observed) in observations {
        if represented.contains(&path) {
            continue;
        }
        let now = chrono::Utc::now();
        let id = format!(
            "native:{:x}",
            Sha256::digest(format!("{}\0{path}", instance.id))
        );
        files.push(InstanceFile {
            id,
            instance_id: instance.id.clone(),
            relative_path: path,
            file_name: observed.file_name,
            enabled: observed.enabled,
            sha1: String::new(),
            size: observed.size,
            missing: false,
            added_at: observed
                .modified
                .map(chrono::DateTime::<chrono::Utc>::from)
                .unwrap_or(now),
            modified_at: observed
                .modified
                .map(chrono::DateTime::<chrono::Utc>::from)
                .unwrap_or(now),
            local_mod_data: None,
            icon_path: None,
        });
    }
    Ok(current)
}

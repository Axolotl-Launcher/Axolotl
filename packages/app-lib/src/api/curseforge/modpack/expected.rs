//! CurseForge modpack install expected.

use crate::api::curseforge::archive::{
    modpack_zip_error, read_modpack_manifest, read_modpack_override_content,
};
use crate::api::curseforge::catalog::{
    get_file, get_files_many, get_project, get_projects,
};
use crate::api::curseforge::download::resolve_download_url as resolve_curseforge_download_url;
use crate::api::curseforge::dto::CurseForgeManualDownload;
use crate::api::curseforge::mapping::project_type_for_class;
use crate::api::curseforge::selection::{
    content_target_folder, managed_project_type, manual_download_from_file,
};
use crate::api::curseforge::validation::validate_file_name;
use crate::api::curseforge::verify::download_curseforge_archive;
use crate::install::{
    InstallJobEventKind, InstallPhaseDetails, InstallPhaseId, InstallProgress,
    InstallProgressReporter,
};
use crate::state::ProjectType;
use crate::util::fetch::FetchProgressFn;
use crate::{ErrorKind, State};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub(crate) struct CurseForgePackExpectedMember {
    pub project_id: u32,
    pub file_id: u32,
    pub project_type: ProjectType,
    pub expected_relative_path: String,
    pub required: bool,
    pub expected_sha1: Option<String>,
    pub expected_size: Option<u64>,
    pub expected_fingerprint: Option<u64>,
    pub manual_download: Option<CurseForgeManualDownload>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CurseForgePackExpectedOverride {
    pub project_type: ProjectType,
    pub expected_relative_path: String,
}

#[derive(Clone, Debug)]
pub(crate) struct CurseForgePackExpectedContent {
    pub members: Vec<CurseForgePackExpectedMember>,
    pub overrides: Vec<CurseForgePackExpectedOverride>,
}

pub(crate) async fn get_modpack_expected_members(
    project_id: u32,
    file_id: u32,
) -> crate::Result<CurseForgePackExpectedContent> {
    get_modpack_expected_members_with_reporter(project_id, file_id, None).await
}

pub(crate) async fn get_modpack_expected_members_with_reporter(
    project_id: u32,
    file_id: u32,
    reporter: Option<&InstallProgressReporter>,
) -> crate::Result<CurseForgePackExpectedContent> {
    let pack_file = get_file(project_id, file_id).await?;
    let project = get_project(project_id).await?;
    let download_url = resolve_curseforge_download_url(
        project_id,
        file_id,
        &project,
        &pack_file,
    )
    .await?
    .ok_or_else(|| {
        ErrorKind::InputError(
            "The linked CurseForge pack archive requires a manual download, so membership cannot be calibrated automatically"
                .to_string(),
        )
    })?;
    let pack_details = InstallPhaseDetails::Modpack {
        project_id: Some(project_id.to_string()),
        version_id: Some(file_id.to_string()),
        title: Some(project.name.clone()),
    };
    if let Some(reporter) = reporter {
        let state = State::get().await?;
        let item_path = state
            .directories
            .caches_dir()
            .join("curseforge")
            .join("modpacks")
            .join(project_id.to_string())
            .join(file_id.to_string())
            .join(crate::util::io::sanitize_download_file_name(
                &pack_file.file_name,
            )?)
            .display()
            .to_string();
        reporter
            .update_with_events(
                InstallPhaseId::ResolvingPack,
                Some(InstallProgress {
                    current: 0,
                    total: pack_file.file_length.max(1),
                    secondary: None,
                }),
                pack_details.clone(),
                vec![InstallJobEventKind::ContentFileQueued {
                    path: item_path,
                    bytes_total: Some(pack_file.file_length),
                    max_attempts: 5,
                }],
            )
            .await?;
        reporter.persist().await?;
    }
    let progress_reporter = reporter.cloned();
    let mut last_downloaded = 0_u64;
    let mut progress = move |current: u64,
                             total: u64|
          -> std::pin::Pin<
        Box<dyn std::future::Future<Output = crate::Result<()>> + Send>,
    > {
        let Some(reporter) = progress_reporter.as_ref() else {
            return Box::pin(async { Ok(()) });
        };
        let min_delta = (total / 200).max(256 * 1024);
        if current < total
            && current.saturating_sub(last_downloaded) < min_delta
        {
            return Box::pin(async { Ok(()) });
        }
        last_downloaded = current;
        let reporter = reporter.clone();
        let details = pack_details.clone();
        Box::pin(async move {
            reporter
                .update(
                    InstallPhaseId::ResolvingPack,
                    Some(InstallProgress {
                        current,
                        total,
                        secondary: None,
                    }),
                    details,
                )
                .await
        })
    };
    let pack_download = download_curseforge_archive(
        project_id,
        file_id,
        &pack_file,
        &download_url,
        Some(&mut progress as &mut FetchProgressFn<'_>),
        reporter,
    )
    .await?;
    let archive_path = pack_download.path;
    let (manifest, overrides) = tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(archive_path)?;
        let mut archive =
            zip::ZipArchive::new(file).map_err(modpack_zip_error)?;
        let manifest = read_modpack_manifest(&mut archive)?;
        let overrides = read_modpack_override_content(&mut archive, &manifest)?;
        Ok::<_, crate::Error>((manifest, overrides))
    })
    .await??;

    let file_ids = manifest
        .files
        .iter()
        .map(|member| member.file_id)
        .collect::<Vec<_>>();
    let project_ids = manifest
        .files
        .iter()
        .map(|member| member.project_id)
        .collect::<Vec<_>>();
    let mut files = HashMap::new();
    for chunk in file_ids.chunks(50) {
        for file in get_files_many(chunk.to_vec()).await? {
            files.insert(file.id, file);
        }
    }
    let mut projects = HashMap::new();
    for chunk in project_ids.chunks(50) {
        for project in get_projects(chunk.to_vec()).await? {
            projects.insert(project.id, project);
        }
    }

    let mut expected = Vec::with_capacity(manifest.files.len());
    for member in manifest.files {
        let file = files.get(&member.file_id).ok_or_else(|| {
            ErrorKind::OtherError(format!(
                "CurseForge file metadata {} is missing",
                member.file_id
            ))
        })?;
        let project = projects.get(&member.project_id).ok_or_else(|| {
            ErrorKind::OtherError(format!(
                "CurseForge project metadata {} is missing",
                member.project_id
            ))
        })?;
        // Saves have their own importer and must not become file-level pack
        // members: pack updates must never replace a player's existing world.
        if project.class_id == Some(17) {
            continue;
        }
        let project_type =
            managed_project_type(project_type_for_class(project.class_id))?;
        validate_file_name(&file.file_name)?;
        let target_folder = content_target_folder(project_type, None)?;
        let manual_download_required = resolve_curseforge_download_url(
            member.project_id,
            member.file_id,
            project,
            file,
        )
        .await?
        .is_none();
        let manual_download = manual_download_required.then(|| {
            manual_download_from_file(
                member.project_id,
                member.file_id,
                file,
                project,
                project_type.get_name(),
                target_folder.clone(),
                crate::state::instances::ContentOwnershipKind::PackManaged,
                crate::state::instances::ManualDownloadOperationKind::PackUpdate,
            )
        });
        expected.push(CurseForgePackExpectedMember {
            project_id: member.project_id,
            file_id: member.file_id,
            project_type,
            expected_relative_path: format!(
                "{}/{}",
                target_folder,
                crate::util::io::sanitize_download_file_name(&file.file_name)?
            ),
            required: member.required,
            expected_sha1: file
                .hashes
                .iter()
                .find(|hash| hash.algo == 1)
                .map(|hash| hash.value.clone()),
            expected_size: Some(file.file_length),
            expected_fingerprint: Some(file.file_fingerprint),
            manual_download,
        });
    }
    Ok(CurseForgePackExpectedContent {
        members: expected,
        overrides,
    })
}

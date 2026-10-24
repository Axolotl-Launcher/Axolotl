//! CurseForge API wire types and modpack manifest shapes.

use super::catalog::{get_file, get_files_many, get_project, get_projects};
use super::download::resolve_download_url as resolve_curseforge_download_url;
use super::mapping::{project_type_for_class, recognized_project_type};
use super::selection::{
    content_target_folder, curseforge_content_validation, managed_project_type,
    manual_download_from_file,
};
use super::update::{
    modpack_zip_error, read_modpack_manifest, read_modpack_override_content,
};
use super::validation::validate_file_name;
use super::verify::{
    CurseForgeVerificationTask, download_curseforge_archive,
    download_curseforge_path, record_installed_curseforge_file_with_permit,
    verify_installed_curseforge_file,
};
use crate::install::{
    InstallJobEventKind, InstallPhaseDetails, InstallPhaseId, InstallProgress,
    InstallProgressReporter,
};
use crate::state::{ContentProvider, ModLoader, ProjectType};
use crate::util::fetch::FetchProgressFn;
use crate::{ErrorKind, State};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::sync::mpsc;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CurseForgeCapabilityStatus {
    MissingKey,
    Ready,
    Unauthorized,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CurseForgeApiKeySource {
    /// Set by the user in the launcher.
    Override,
    /// Injected when the launcher was built, or exported as
    /// `AXOLOTL_CURSEFORGE_API_KEY` before starting it.
    Launcher,
    /// No key is available at all.
    Missing,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CurseForgeCapability {
    pub status: CurseForgeCapabilityStatus,
    pub configured: bool,
    pub key_source: CurseForgeApiKeySource,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgePagination {
    pub index: u32,
    pub page_size: u32,
    pub result_count: u32,
    pub total_count: u32,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct CurseForgeResponse<T> {
    pub(super) data: T,
    #[serde(default)]
    pub(super) pagination: Option<CurseForgePagination>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeSearchRequest {
    pub class_id: u32,
    #[serde(default)]
    pub category_id: Option<u32>,
    #[serde(default)]
    pub category_ids: Vec<u32>,
    #[serde(default)]
    pub search_filter: Option<String>,
    #[serde(default)]
    pub game_version: Option<String>,
    #[serde(default)]
    pub mod_loader_type: Option<u32>,
    #[serde(default)]
    pub sort_field: Option<u32>,
    #[serde(default)]
    pub sort_order: Option<String>,
    #[serde(default)]
    pub index: u32,
    #[serde(default = "default_page_size")]
    pub page_size: u32,
}

fn default_page_size() -> u32 {
    20
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UnifiedSearchResponse {
    pub provider: ContentProvider,
    pub hits: Vec<UnifiedSearchHit>,
    pub offset: u32,
    pub limit: u32,
    pub total_hits: u32,
}

fn deserialize_null_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UnifiedSearchHit {
    pub provider: ContentProvider,
    pub project_id: String,
    pub slug: Option<String>,
    pub author: String,
    pub author_url: Option<String>,
    pub title: String,
    pub description: String,
    pub project_type: String,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub categories: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub versions: Vec<String>,
    pub downloads: u64,
    pub icon_url: Option<String>,
    pub date_created: String,
    pub date_modified: String,
    pub latest_version: Option<String>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub gallery: Vec<String>,
    pub website_url: Option<String>,
    pub source_url: Option<String>,
    pub allow_mod_distribution: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeAuthor {
    pub id: u32,
    pub name: String,
    pub url: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeLinks {
    pub website_url: Option<String>,
    pub wiki_url: Option<String>,
    pub issues_url: Option<String>,
    pub source_url: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeAsset {
    pub id: u32,
    pub mod_id: u32,
    pub title: String,
    pub description: String,
    pub thumbnail_url: String,
    pub url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeCategory {
    pub id: u32,
    pub game_id: u32,
    pub name: String,
    pub slug: String,
    pub url: String,
    pub icon_url: Option<String>,
    pub date_modified: String,
    #[serde(default)]
    pub is_class: Option<bool>,
    pub class_id: Option<u32>,
    pub parent_category_id: Option<u32>,
    pub display_index: Option<i32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeFileIndex {
    pub game_version: String,
    pub file_id: u32,
    pub filename: String,
    pub release_type: u32,
    pub game_version_type_id: Option<u32>,
    pub mod_loader: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeProject {
    pub id: u32,
    pub game_id: u32,
    pub name: String,
    pub slug: String,
    #[serde(default)]
    pub links: CurseForgeLinks,
    pub summary: String,
    pub status: u32,
    pub download_count: u64,
    pub is_featured: bool,
    pub primary_category_id: u32,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub categories: Vec<CurseForgeCategory>,
    pub class_id: Option<u32>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub authors: Vec<CurseForgeAuthor>,
    pub logo: Option<CurseForgeAsset>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub screenshots: Vec<CurseForgeAsset>,
    pub main_file_id: u32,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub latest_files: Vec<CurseForgeFile>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub latest_files_indexes: Vec<CurseForgeFileIndex>,
    pub date_created: String,
    pub date_modified: String,
    pub date_released: String,
    pub allow_mod_distribution: Option<bool>,
    pub game_popularity_rank: Option<i32>,
    pub is_available: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeFileHash {
    pub value: String,
    pub algo: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeFileDependency {
    pub mod_id: u32,
    pub relation_type: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeSortableGameVersion {
    pub game_version_name: String,
    pub game_version_padded: Option<String>,
    pub game_version: Option<String>,
    pub game_version_release_date: Option<String>,
    pub game_version_type_id: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeFileModule {
    pub name: String,
    pub fingerprint: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeFile {
    pub id: u32,
    pub game_id: u32,
    pub mod_id: u32,
    pub is_available: bool,
    pub display_name: String,
    pub file_name: String,
    pub release_type: u32,
    pub file_status: u32,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub hashes: Vec<CurseForgeFileHash>,
    pub file_date: String,
    pub file_length: u64,
    pub download_count: u64,
    pub file_size_on_disk: Option<u64>,
    pub download_url: Option<String>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub game_versions: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub sortable_game_versions: Vec<CurseForgeSortableGameVersion>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub dependencies: Vec<CurseForgeFileDependency>,
    pub expose_as_alternative: Option<bool>,
    pub parent_project_file_id: Option<u32>,
    pub alternate_file_id: Option<u32>,
    pub is_server_pack: Option<bool>,
    pub server_pack_file_id: Option<u32>,
    pub is_early_access_content: Option<bool>,
    pub early_access_end_date: Option<String>,
    pub file_fingerprint: u64,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub modules: Vec<CurseForgeFileModule>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeFilesRequest {
    #[serde(default)]
    pub game_version: Option<String>,
    #[serde(default)]
    pub mod_loader_type: Option<u32>,
    #[serde(default)]
    pub game_version_type_id: Option<u32>,
    #[serde(default)]
    pub index: u32,
    #[serde(default = "default_page_size")]
    pub page_size: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CurseForgeFilesResponse {
    pub files: Vec<CurseForgeFile>,
    pub pagination: CurseForgePagination,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeInstallRequest {
    pub instance_id: String,
    pub project_id: u32,
    pub file_id: u32,
    pub project_type: String,
    #[serde(default)]
    pub ownership_kind: crate::state::instances::ContentOwnershipKind,
    #[serde(default)]
    pub manual_operation_kind:
        crate::state::instances::ManualDownloadOperationKind,
    /// Used to select automatic dependencies. The explicitly requested file is
    /// authoritative, including files pinned by a modpack manifest.
    #[serde(default)]
    pub game_version: Option<String>,
    /// Used to select automatic dependencies, not to reject the requested file.
    #[serde(default)]
    pub mod_loader_type: Option<u32>,
    #[serde(default)]
    pub world_name: Option<String>,
    #[serde(default = "default_true")]
    pub install_dependencies: bool,
    #[serde(default)]
    pub excluded_dependency_project_ids: Vec<u32>,
    #[serde(default)]
    pub force_dependency_project_ids: Vec<u32>,
    /// The immutable dependency selection returned by preview. It is short
    /// lived and tied to the target instance revision.
    #[serde(default)]
    pub dependency_plan_id: Option<String>,
    #[serde(skip)]
    pub(crate) defer_persistence: bool,
    #[serde(skip)]
    pub(crate) verification_tx:
        Option<mpsc::Sender<CurseForgeVerificationTask>>,
    #[serde(skip)]
    pub(crate) pre_resolved_relative_path: Option<String>,
    #[serde(skip)]
    pub(crate) expected_file_name: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeWorldInstallRequest {
    pub instance_id: String,
    pub project_id: u32,
    pub file_id: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeInstalledFile {
    pub project_id: u32,
    pub file_id: u32,
    pub relative_path: String,
    pub dependency: bool,
}

#[derive(Clone)]
pub(crate) struct StagedCurseForgeUpgrade {
    pub path: PathBuf,
    pub file: CurseForgeFile,
    pub project_type: ProjectType,
}

pub(crate) async fn stage_curseforge_upgrade_file(
    project_id: u32,
    file_id: u32,
    project_type: Option<ProjectType>,
    reporter: Option<&InstallProgressReporter>,
) -> crate::Result<StagedCurseForgeUpgrade> {
    let file = get_file(project_id, file_id).await?;
    if file.mod_id != project_id || file.id != file_id {
        return Err(ErrorKind::InputError(
            "CurseForge returned metadata for a different project or file"
                .to_string(),
        )
        .into());
    }
    validate_file_name(&file.file_name)?;
    let project = get_project(project_id).await?;
    let project_type = project_type
        .or_else(|| recognized_project_type(project.class_id))
        .ok_or_else(|| {
            ErrorKind::InputError(
                "CurseForge upgrade project has an unsupported content type"
                    .to_string(),
            )
        })?;
    let url =
        resolve_curseforge_download_url(project_id, file_id, &project, &file)
            .await?
            .ok_or_else(|| {
                ErrorKind::InputError(
            "Selected CurseForge upgrade file requires a manual download"
                .to_string(),
        )
            })?;
    let state = State::get().await?;
    let path = state
        .directories
        .caches_dir()
        .join("content")
        .join("curseforge")
        .join(project_id.to_string())
        .join(file_id.to_string())
        .join(crate::util::io::sanitize_download_file_name(
            &file.file_name,
        )?);
    let tracking = path.display().to_string();
    download_curseforge_path(
        &url,
        &file,
        &path,
        curseforge_content_validation(&file.file_name),
        None,
        reporter.map(|reporter| (reporter, tracking.as_str())),
        None,
        true,
    )
    .await?;
    verify_installed_curseforge_file(&path, &file, None, None).await?;
    Ok(StagedCurseForgeUpgrade {
        path,
        file,
        project_type,
    })
}

pub(crate) async fn apply_staged_curseforge_upgrade_file(
    instance_id: &str,
    staged: StagedCurseForgeUpgrade,
    ownership_kind: crate::state::instances::ContentOwnershipKind,
    relative_path: &str,
) -> crate::Result<String> {
    let state = State::get().await?;
    apply_staged_curseforge_upgrade_file_with_state(
        instance_id,
        staged,
        ownership_kind,
        relative_path,
        &state,
    )
    .await
}

pub(crate) async fn apply_staged_curseforge_upgrade_file_with_state(
    instance_id: &str,
    staged: StagedCurseForgeUpgrade,
    ownership_kind: crate::state::instances::ContentOwnershipKind,
    relative_path: &str,
    state: &State,
) -> crate::Result<String> {
    let scope = crate::state::instances::commands::resolve_content_scope(
        instance_id,
        None,
        state,
    )
    .await?;
    let full_path = crate::state::instances::commands::instance_full_path(
        state,
        &scope.instance,
    )
    .join(relative_path);
    let database_permit = Some(state.acquire_install_db_permit().await?);
    let _instance_lock = state
        .lock_instance_content_with_timeout(
            instance_id,
            std::time::Duration::from_secs(20),
        )
        .await?;
    let previous_path =
        crate::state::materialize_project_download(&staged.path, &full_path)
            .await?;
    let record_result = record_installed_curseforge_file_with_permit(
        instance_id,
        relative_path,
        &full_path,
        &staged.file,
        staged.project_type,
        ownership_kind,
        database_permit,
        state,
    )
    .await;
    match record_result {
        Ok(()) => {
            crate::state::finalize_project_materialization(
                previous_path.as_deref(),
            )
            .await?;
        }
        Err(error) => {
            crate::state::restore_project_materialization(
                &full_path,
                previous_path.as_deref(),
            )
            .await?;
            return Err(error);
        }
    }
    Ok(relative_path.to_string())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeManualDownload {
    pub project_id: u32,
    pub file_id: u32,
    pub file_name: String,
    #[serde(default)]
    pub ownership_kind: crate::state::instances::ContentOwnershipKind,
    #[serde(default)]
    pub operation_kind: crate::state::instances::ManualDownloadOperationKind,
    pub website_url: Option<String>,
    #[serde(default)]
    pub project_type: String,
    #[serde(default)]
    pub project_slug: String,
    #[serde(default)]
    pub target_folder: String,
    #[serde(default)]
    pub hashes: Vec<CurseForgeFileHash>,
    #[serde(default)]
    pub file_length: u64,
    #[serde(default)]
    pub file_fingerprint: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeManualDownloadImport {
    pub project_id: u32,
    pub file_id: u32,
    pub relative_path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeManualDownloadImportError {
    pub project_id: u32,
    pub file_id: u32,
    pub message: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeManualDownloadScanResult {
    pub download_directory: Option<String>,
    pub imported: Vec<CurseForgeManualDownloadImport>,
    pub errors: Vec<CurseForgeManualDownloadImportError>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeFailedDownload {
    pub project_id: u32,
    pub file_id: u32,
    pub file_name: String,
    pub reason: String,
}

#[derive(
    Clone, Debug, Serialize, Deserialize, Eq, Ord, PartialEq, PartialOrd,
)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeSkippedDependency {
    pub project_id: u32,
    pub file_id: Option<u32>,
    pub reason: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeInstallResult {
    pub installed: Vec<CurseForgeInstalledFile>,
    pub manual_downloads: Vec<CurseForgeManualDownload>,
    #[serde(default)]
    pub failed_downloads: Vec<CurseForgeFailedDownload>,
    pub optional_dependencies: Vec<u32>,
    pub incompatible_dependencies: Vec<u32>,
    #[serde(default)]
    pub skipped_dependencies: Vec<CurseForgeSkippedDependency>,
    #[serde(default)]
    pub cross_source_dependencies: Vec<CurseForgeCrossSourceDependency>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeCrossSourceDependency {
    pub parent_project_id: u32,
    pub parent_file_id: u32,
    pub project_id: String,
    pub version_id: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeWorldInstallResult {
    pub world_name: Option<String>,
    pub manual_download: Option<CurseForgeManualDownload>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeRecognitionResult {
    pub scanned: u32,
    pub matched: u32,
    pub linked: Vec<CurseForgeInstalledFile>,
    pub unmatched_paths: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeModpackInstallRequest {
    pub instance_id: String,
    pub project_id: u32,
    pub file_id: u32,
    #[serde(default)]
    pub install_optional: bool,
    #[serde(default)]
    pub allow_target_change: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeModpackInstallResult {
    pub content: CurseForgeInstallResult,
    pub overrides_written: u32,
    pub minecraft_version: String,
    pub loader: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurseForgeModpackTarget {
    pub game_version: String,
    pub loader: ModLoader,
    pub loader_version: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CurseForgeModpackManifest {
    pub(super) minecraft: CurseForgeManifestMinecraft,
    #[serde(default)]
    pub(super) files: Vec<CurseForgeManifestFile>,
    #[serde(default = "default_overrides")]
    pub(super) overrides: String,
    #[serde(default)]
    pub(super) name: Option<String>,
    #[serde(default)]
    pub(super) version: Option<String>,
}

fn default_overrides() -> String {
    "overrides".to_string()
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CurseForgeManifestMinecraft {
    pub(super) version: String,
    #[serde(default)]
    pub(super) mod_loaders: Vec<CurseForgeManifestLoader>,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct CurseForgeManifestLoader {
    pub(super) id: String,
    pub(super) primary: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct CurseForgeManifestFile {
    // CurseForge modpack manifests use projectID/fileID (capital ID), not projectId.
    #[serde(alias = "projectID", alias = "projectId")]
    pub(crate) project_id: u32,
    #[serde(alias = "fileID", alias = "fileId")]
    pub(crate) file_id: u32,
    #[serde(default = "default_true")]
    pub(crate) required: bool,
}

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

pub(crate) fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeFingerprintMatch {
    pub id: u32,
    pub file: CurseForgeFile,
    pub latest_files: Vec<CurseForgeFile>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeFingerprintResult {
    pub is_cache_built: bool,
    #[serde(default)]
    pub exact_matches: Vec<CurseForgeFingerprintMatch>,
    #[serde(default)]
    pub exact_fingerprints: Vec<u64>,
    #[serde(default)]
    pub partial_matches: Vec<Value>,
    #[serde(default)]
    pub partial_match_fingerprints: Value,
    #[serde(default)]
    pub installed_fingerprints: Vec<u64>,
    #[serde(default)]
    pub unmatched_fingerprints: Vec<u64>,
}

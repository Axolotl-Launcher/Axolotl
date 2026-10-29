//! CurseForge API wire types and modpack manifest shapes.

use super::constants::{
    FABRIC_API_CURSEFORGE_PROJECT_ID, QUILTED_FABRIC_API_CURSEFORGE_PROJECT_ID,
};
use super::verify::CurseForgeVerificationTask;
use crate::state::instances::DependencyRelation;
use crate::state::{ContentProvider, ModLoader};
use serde::{Deserialize, Serialize};
use serde_json::Value;
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

impl CurseForgeFileDependency {
    pub(crate) fn new(mod_id: u32, relation: DependencyRelation) -> Self {
        Self {
            mod_id,
            relation_type: relation.as_raw(),
        }
    }

    pub(crate) fn relation(&self) -> DependencyRelation {
        DependencyRelation::from_raw(self.relation_type)
    }

    /// The project a loader installs for this dependency. Quilt replaces the
    /// bundled Fabric API with the Quilted Fabric API.
    pub(crate) fn target_project_id(&self, quilt: bool) -> u32 {
        if quilt && self.mod_id == FABRIC_API_CURSEFORGE_PROJECT_ID {
            QUILTED_FABRIC_API_CURSEFORGE_PROJECT_ID
        } else {
            self.mod_id
        }
    }
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

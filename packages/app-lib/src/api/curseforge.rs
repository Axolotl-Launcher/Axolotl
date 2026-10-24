use crate::api::content_search::{
    chinese_file_title_for_curseforge_slug, localized_content_file_name,
};
use crate::event::LoadingBarType;
use crate::event::emit::{
    emit_loading, init_loading, loading_try_for_each_concurrent,
};
use crate::install::{
    InstallJobEventKind, InstallPhaseDetails, InstallPhaseId, InstallProgress,
    InstallProgressReporter, InstallProgressSecondary,
};
use crate::state::{
    CacheBehaviour, CachedEntry, ContentProvider, ContentProviderRef,
    ContentSourceKind, CurseForgeFileId, CurseForgeProjectId,
    DependencyResolutionEdge, DependencyResolutionIssue,
    DependencyResolutionNode, DependencyResolutionPlan,
    DependencyResolutionTarget, DependencySelectionReason, DownloadSourceMode,
    EditInstance, InstanceInstallStage, InstanceLink, ModLoader,
    ModrinthProjectId, ModrinthVersionId, ProjectType, ReleaseChannel,
    Settings, curseforge_api_key_override_if_ready, curseforge_settings,
};
use crate::util::download::constants::CURSEFORGE_OVERRIDE_EXTRACTION_CONCURRENCY as OVERRIDE_EXTRACTION_CONCURRENCY;
use crate::util::download::constants::MODPACK_DATABASE_BATCH_SIZE;
use crate::util::download::constants::MODPACK_DATABASE_FLUSH_INTERVAL;
use crate::util::download::constants::MODPACK_DATABASE_QUEUE_CAPACITY;
use crate::util::download::constants::MODPACK_FILE_INSTALL_ATTEMPTS;
use crate::util::download::constants::MODPACK_METADATA_CONCURRENCY;
use crate::util::download::constants::MODPACK_VERIFICATION_CONCURRENCY;
use crate::util::download::constants::MODPACK_VERIFICATION_QUEUE_CAPACITY;
use crate::util::download::constants::{
    CURSEFORGE_H2_RANGE_THRESHOLD, CURSEFORGE_REQUESTED_H2_RANGES,
};
use crate::util::fetch::{
    ContentValidation, DownloadRequest, DownloadRouteSource, FetchProgressFn,
    Integrity, ProxyPolicy, ResourceClass, download_to_path,
    resolve_download_routes_for, sha1_file_async, sha1_file_cancellable,
};
use crate::{ErrorKind, State};
use content_management::{DependencyPolicy, DependencyType};
use dashmap::DashMap;
use futures::{StreamExt, stream};
use reqwest::{Method, StatusCode};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{Read, Seek};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex, RwLock};
use std::time::{Duration, Instant};
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[path = "curseforge_validation.rs"]
mod curseforge_validation;
use curseforge_validation::{validate_file_name, validate_world_archive_name};
#[path = "curseforge_download.rs"]
mod curseforge_download;
use curseforge_download::normalized_download_url;
#[path = "curseforge_metrics.rs"]
mod curseforge_metrics;
use curseforge_metrics::CurseForgeDownloadMetrics;
#[path = "curseforge_mapping.rs"]
mod curseforge_mapping;
use curseforge_mapping::{
    filter_categories, mod_loader_to_slug, project_type_for_class, push_query,
    recognized_project_type,
};

const API_BASE_URL: &str = "https://api.curseforge.com";
const MINECRAFT_GAME_ID: u32 = 432;
const MAX_PAGE_SIZE: u32 = 50;

const DEPENDENCY_RELATION_EMBEDDED: u32 = 1;
const DEPENDENCY_RELATION_OPTIONAL: u32 = 2;
pub(crate) const DEPENDENCY_RELATION_REQUIRED: u32 = 3;
const DEPENDENCY_RELATION_TOOL: u32 = 4;
const DEPENDENCY_RELATION_INCOMPATIBLE: u32 = 5;
pub(crate) const DEPENDENCY_RELATION_INCLUDE: u32 = 6;
pub(crate) const FABRIC_API_CURSEFORGE_PROJECT_ID: u32 = 306612;
pub(crate) const QUILTED_FABRIC_API_CURSEFORGE_PROJECT_ID: u32 = 634179;
const CURSEFORGE_LOADER_QUILT: u32 = 5;
const MAX_DEPENDENCY_DEPTH: usize = 32;
const DEPENDENCY_PLAN_TTL: Duration = Duration::from_secs(10 * 60);

static UNAUTHORIZED: AtomicBool = AtomicBool::new(false);
/// Bumped whenever the effective key is replaced. A response that arrives after
/// that can only describe the previous key, so it must not change the flag.
static API_KEY_GENERATION: AtomicU64 = AtomicU64::new(0);
static CATEGORY_CACHE: LazyLock<RwLock<Option<Vec<CurseForgeCategory>>>> =
    LazyLock::new(|| RwLock::new(None));
static MANUAL_IMPORT_SCAN_LOCK: LazyLock<tokio::sync::Mutex<()>> =
    LazyLock::new(|| tokio::sync::Mutex::new(()));
static DEPENDENCY_RESOLUTION_PLANS: LazyLock<
    DashMap<String, CachedDependencyResolutionPlan>,
> = LazyLock::new(DashMap::new);

fn unique_metadata_chunks(mut ids: Vec<u32>) -> Vec<Vec<u32>> {
    ids.sort_unstable();
    ids.dedup();
    ids.chunks(MAX_PAGE_SIZE as usize)
        .map(<[u32]>::to_vec)
        .collect()
}

async fn get_modpack_projects(
    project_ids: Vec<u32>,
) -> crate::Result<HashMap<u32, CurseForgeProject>> {
    let chunks = unique_metadata_chunks(project_ids);
    let project_ids = chunks.iter().flatten().copied().collect::<Vec<_>>();
    let batches = stream::iter(chunks)
        .map(|chunk| async move { get_projects(chunk).await })
        .buffer_unordered(MODPACK_METADATA_CONCURRENCY)
        .collect::<Vec<_>>()
        .await;
    let mut projects = HashMap::new();
    for batch in batches {
        for project in batch? {
            projects.insert(project.id, project);
        }
    }
    let missing = project_ids
        .into_iter()
        .filter(|project_id| !projects.contains_key(project_id))
        .collect::<Vec<_>>();
    let fallbacks = stream::iter(missing)
        .map(|project_id| async move { get_project(project_id).await })
        .buffer_unordered(MODPACK_METADATA_CONCURRENCY)
        .collect::<Vec<_>>()
        .await;
    for project in fallbacks {
        let project = project?;
        projects.insert(project.id, project);
    }
    Ok(projects)
}

async fn get_modpack_files(
    file_ids: Vec<u32>,
) -> crate::Result<HashMap<u32, CurseForgeFile>> {
    let chunks = unique_metadata_chunks(file_ids);
    let batches = stream::iter(chunks)
        .map(|chunk| async move { get_files_many(chunk).await })
        .buffer_unordered(MODPACK_METADATA_CONCURRENCY)
        .collect::<Vec<_>>()
        .await;
    let mut files = HashMap::new();
    for batch in batches {
        for file in batch? {
            files.insert(file.id, file);
        }
    }
    Ok(files)
}

#[derive(Clone)]
struct CachedDependencyResolutionPlan {
    plan: DependencyResolutionPlan,
    expires_at: Instant,
}

static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .read_timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(crate::launcher_user_agent())
        .no_proxy()
        .build()
        .expect("CurseForge client configuration should be valid")
});

#[cfg(debug_assertions)]
static LOCAL_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .read_timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(crate::launcher_user_agent())
        .no_proxy()
        .build()
        .expect("Local CurseForge client configuration should be valid")
});

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
struct CurseForgeResponse<T> {
    data: T,
    #[serde(default)]
    pagination: Option<CurseForgePagination>,
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
struct CurseForgeModpackManifest {
    minecraft: CurseForgeManifestMinecraft,
    #[serde(default)]
    files: Vec<CurseForgeManifestFile>,
    #[serde(default = "default_overrides")]
    overrides: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    version: Option<String>,
}

fn default_overrides() -> String {
    "overrides".to_string()
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurseForgeManifestMinecraft {
    version: String,
    #[serde(default)]
    mod_loaders: Vec<CurseForgeManifestLoader>,
}

#[derive(Clone, Debug, Deserialize)]
struct CurseForgeManifestLoader {
    id: String,
    primary: bool,
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

fn default_true() -> bool {
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

pub fn capability() -> CurseForgeCapability {
    let (key, key_source) = api_key_with_source();
    let configured = key.is_some();
    let status = if !configured {
        CurseForgeCapabilityStatus::MissingKey
    } else if UNAUTHORIZED.load(Ordering::Relaxed) {
        CurseForgeCapabilityStatus::Unauthorized
    } else {
        CurseForgeCapabilityStatus::Ready
    };

    CurseForgeCapability {
        status,
        configured,
        key_source,
    }
}

/// Stores the API key the user entered in the launcher. `None` removes the
/// override so the key the launcher was started or built with applies again,
/// when there is one.
pub async fn set_api_key_override(
    api_key: Option<String>,
) -> crate::Result<CurseForgeCapability> {
    let state = State::get().await?;
    state.update_curseforge_api_key(api_key).await?;
    // Invalidate responses still in flight for the previous key before clearing
    // the flag, so they cannot report an outdated status for the new one.
    API_KEY_GENERATION.fetch_add(1, Ordering::Relaxed);
    UNAUTHORIZED.store(false, Ordering::Relaxed);
    Ok(capability())
}

fn api_key_generation() -> u64 {
    API_KEY_GENERATION.load(Ordering::Relaxed)
}

/// Applies the authorization outcome of a response only while it still belongs
/// to the current key, so a late response cannot speak for a replaced one.
fn record_key_response(generation: u64, unauthorized: bool) {
    if generation != api_key_generation() {
        return;
    }
    UNAUTHORIZED.store(unauthorized, Ordering::Relaxed);
}

/// The API key the user entered in the launcher, if any, for redisplaying it.
pub async fn api_key_override() -> crate::Result<Option<String>> {
    let state = State::get().await?;
    Ok(state.curseforge_api_key_override())
}

pub async fn validate_credentials() -> crate::Result<CurseForgeCapability> {
    let generation = api_key_generation();
    let _: CurseForgeResponse<Vec<CurseForgeProject>> = request_json(
        Method::GET,
        "/v1/mods/search",
        vec![
            ("gameId".to_string(), MINECRAFT_GAME_ID.to_string()),
            ("classId".to_string(), "6".to_string()),
            ("pageSize".to_string(), "1".to_string()),
        ],
        None,
        MirrorPolicy::OfficialOnly,
    )
    .await?;
    record_key_response(generation, false);
    Ok(capability())
}

pub async fn search_projects(
    request: CurseForgeSearchRequest,
) -> crate::Result<UnifiedSearchResponse> {
    let page_size = request.page_size.clamp(1, MAX_PAGE_SIZE);
    let mut query = vec![
        ("gameId".to_string(), MINECRAFT_GAME_ID.to_string()),
        ("classId".to_string(), request.class_id.to_string()),
        ("index".to_string(), request.index.to_string()),
        ("pageSize".to_string(), page_size.to_string()),
    ];
    if request.category_ids.is_empty() {
        push_query(&mut query, "categoryId", request.category_id);
    } else if request.category_ids.len() == 1 {
        // Single-category requests are more widely compatible as categoryId.
        push_query(
            &mut query,
            "categoryId",
            request.category_ids.first().copied(),
        );
    } else {
        let category_ids = request
            .category_ids
            .iter()
            .copied()
            .take(10)
            .collect::<Vec<_>>();
        query.push((
            "categoryIds".to_string(),
            serde_json::to_string(&category_ids)?,
        ));
    }
    push_query(&mut query, "searchFilter", request.search_filter);
    push_query(&mut query, "gameVersion", request.game_version);
    push_query(&mut query, "modLoaderType", request.mod_loader_type);
    push_query(&mut query, "sortField", request.sort_field);
    push_query(&mut query, "sortOrder", request.sort_order);

    let response: CurseForgeResponse<Vec<CurseForgeProject>> = request_json(
        Method::GET,
        "/v1/mods/search",
        query,
        None,
        MirrorPolicy::MirrorFirst,
    )
    .await?;
    let pagination = response.pagination.unwrap_or(CurseForgePagination {
        index: request.index,
        page_size,
        result_count: response.data.len() as u32,
        total_count: response.data.len() as u32,
    });

    Ok(UnifiedSearchResponse {
        provider: ContentProvider::CurseForge,
        hits: response
            .data
            .into_iter()
            .map(UnifiedSearchHit::from)
            .collect(),
        offset: pagination.index,
        limit: pagination.page_size,
        total_hits: pagination.total_count,
    })
}

pub async fn get_project(project_id: u32) -> crate::Result<CurseForgeProject> {
    let state = State::get().await?;
    CachedEntry::get_curseforge_project(
        &CurseForgeProjectId::new(project_id)?,
        None,
        &state.pool,
        &state.api_semaphore,
    )
    .await?
    .ok_or_else(|| {
        ErrorKind::OtherError(format!(
            "CurseForge project {project_id} was not found"
        ))
        .as_error()
    })
}

pub async fn get_projects(
    project_ids: Vec<u32>,
) -> crate::Result<Vec<CurseForgeProject>> {
    get_projects_with_cache_behaviour(project_ids, None).await
}

pub async fn get_projects_with_cache_behaviour(
    project_ids: Vec<u32>,
    cache_behaviour: Option<CacheBehaviour>,
) -> crate::Result<Vec<CurseForgeProject>> {
    let project_ids = project_ids
        .into_iter()
        .map(CurseForgeProjectId::new)
        .collect::<crate::Result<Vec<_>>>()?;
    let state = State::get().await?;
    CachedEntry::get_curseforge_project_many(
        &project_ids,
        cache_behaviour,
        &state.pool,
        &state.api_semaphore,
    )
    .await
}

pub async fn get_description(project_id: u32) -> crate::Result<String> {
    let response: CurseForgeResponse<String> = request_json(
        Method::GET,
        &format!("/v1/mods/{project_id}/description"),
        Vec::new(),
        None,
        MirrorPolicy::MirrorFirst,
    )
    .await?;
    Ok(response.data)
}

pub async fn get_files_page(
    project_id: u32,
    request: CurseForgeFilesRequest,
) -> crate::Result<CurseForgeFilesResponse> {
    let page_size = request.page_size.clamp(1, MAX_PAGE_SIZE);
    let mut query = vec![
        ("index".to_string(), request.index.to_string()),
        ("pageSize".to_string(), page_size.to_string()),
    ];
    push_query(&mut query, "gameVersion", request.game_version);
    push_query(&mut query, "modLoaderType", request.mod_loader_type);
    push_query(
        &mut query,
        "gameVersionTypeId",
        request.game_version_type_id,
    );

    let response: CurseForgeResponse<Vec<CurseForgeFile>> = request_json(
        Method::GET,
        &format!("/v1/mods/{project_id}/files"),
        query,
        None,
        MirrorPolicy::MirrorFirst,
    )
    .await?;
    let result_count = response.data.len() as u32;
    let pagination = response.pagination.unwrap_or(CurseForgePagination {
        index: request.index,
        page_size,
        result_count,
        total_count: result_count,
    });

    Ok(CurseForgeFilesResponse {
        files: response.data,
        pagination,
    })
}

/// Fetches every page of a CurseForge project's files and returns the complete
/// file list, since the API caps each page at `MAX_PAGE_SIZE` entries.
pub async fn get_files(
    project_id: u32,
    request: CurseForgeFilesRequest,
) -> crate::Result<CurseForgeFilesResponse> {
    let page_size = request.page_size.clamp(1, MAX_PAGE_SIZE);
    let mut files = Vec::new();
    let mut index = request.index;

    let total_count = loop {
        let response = get_files_page(
            project_id,
            CurseForgeFilesRequest {
                index,
                page_size,
                ..request.clone()
            },
        )
        .await?;
        let pagination = response.pagination;
        files.extend(response.files);

        if files.len() as u32 >= pagination.total_count
            || pagination.result_count == 0
        {
            break pagination.total_count;
        }
        index += pagination.result_count;
    };

    let result_count = files.len() as u32;
    Ok(CurseForgeFilesResponse {
        files,
        pagination: CurseForgePagination {
            index: request.index,
            page_size,
            result_count,
            total_count,
        },
    })
}

pub async fn get_file(
    project_id: u32,
    file_id: u32,
) -> crate::Result<CurseForgeFile> {
    let state = State::get().await?;
    crate::resources::legacy_catalog::exact(
        crate::resources::SourceId::curseforge(),
        Some(project_id.to_string()),
        file_id.to_string(),
        &state,
    )
    .await?
    .ok_or_else(|| {
        ErrorKind::InputError("CurseForge file not found".into()).into()
    })
}

pub async fn get_files_many(
    file_ids: Vec<u32>,
) -> crate::Result<Vec<CurseForgeFile>> {
    use crate::resources::{
        SourceId, SourceReleaseId,
        providers::{ProviderQuery, ProviderValue},
    };
    let state = State::get().await?;
    let service = state
        .resources()
        .await
        .map_err(crate::resources::protocol::error)?;
    let queries = file_ids
        .into_iter()
        .map(|id| {
            Ok(ProviderQuery::Release {
                project_id: None,
                release_id: SourceReleaseId::new(id.to_string())
                    .map_err(crate::resources::protocol::error)?,
                slug: None,
            })
        })
        .collect::<crate::Result<Vec<_>>>()?;
    let values = service
        .presentation_many(
            &SourceId::curseforge(),
            queries,
            &tokio_util::sync::CancellationToken::new(),
        )
        .await;
    let mut files = Vec::new();
    for value in values {
        match value.map_err(crate::resources::protocol::error)?.value {
            ProviderValue::Release(document) => {
                files.push(serde_json::from_value(
                    document.protocol.ok_or_else(|| {
                        ErrorKind::InputError(
                            "CurseForge file projection missing".into(),
                        )
                    })?,
                )?)
            }
            ProviderValue::NotFound => {}
            _ => {
                return Err(ErrorKind::InputError(
                    "unexpected CurseForge file response".into(),
                )
                .into());
            }
        }
    }
    Ok(files)
}

pub async fn get_changelog(
    project_id: u32,
    file_id: u32,
) -> crate::Result<String> {
    let response: CurseForgeResponse<String> = request_json(
        Method::GET,
        &format!("/v1/mods/{project_id}/files/{file_id}/changelog"),
        Vec::new(),
        None,
        MirrorPolicy::MirrorFirst,
    )
    .await?;
    Ok(response.data)
}

pub async fn get_download_url(
    project_id: u32,
    file_id: u32,
) -> crate::Result<Option<String>> {
    let path = format!("/v1/mods/{project_id}/files/{file_id}/download-url");
    let response: CurseForgeResponse<Option<String>> = request_json(
        Method::GET,
        &path,
        Vec::new(),
        None,
        MirrorPolicy::MirrorFirst,
    )
    .await?;
    if let Some(url) = normalized_download_url(response.data) {
        return Ok(Some(url));
    }
    if api_key().is_none() {
        return Ok(None);
    }

    let response: CurseForgeResponse<Option<String>> = request_json(
        Method::GET,
        &path,
        Vec::new(),
        None,
        MirrorPolicy::OfficialOnly,
    )
    .await?;
    Ok(normalized_download_url(response.data))
}

use curseforge_download::resolve_download_url as resolve_curseforge_download_url;

pub(crate) fn derived_curseforge_download_url(
    file_id: u32,
    file_name: &str,
) -> crate::Result<String> {
    validate_file_name(file_name)?;
    Ok(format!(
        "https://edge.forgecdn.net/files/{}/{}/{}",
        file_id / 1000,
        file_id % 1000,
        urlencoding::encode(file_name)
    ))
}

pub async fn get_categories(
    class_id: Option<u32>,
) -> crate::Result<Vec<CurseForgeCategory>> {
    let cached = CATEGORY_CACHE.read().ok().and_then(|cache| cache.clone());
    let categories = if let Some(categories) = cached {
        categories
    } else {
        let response: CurseForgeResponse<Vec<CurseForgeCategory>> =
            request_json(
                Method::GET,
                "/v1/categories",
                vec![("gameId".to_string(), MINECRAFT_GAME_ID.to_string())],
                None,
                MirrorPolicy::MirrorFirst,
            )
            .await?;

        if let Ok(mut cache) = CATEGORY_CACHE.write() {
            *cache = Some(response.data.clone());
        }
        response.data
    };

    Ok(filter_categories(categories, class_id))
}

pub async fn match_fingerprints(
    fingerprints: Vec<u64>,
) -> crate::Result<CurseForgeFingerprintResult> {
    let response: CurseForgeResponse<CurseForgeFingerprintResult> =
        request_json(
            Method::POST,
            &format!("/v1/fingerprints/{MINECRAFT_GAME_ID}"),
            Vec::new(),
            Some(json!({ "fingerprints": fingerprints })),
            MirrorPolicy::MirrorFirst,
        )
        .await?;
    Ok(response.data)
}

/// Resource adapters share routing and credential handling without consulting
/// the legacy main-database resource cache.
pub(crate) async fn resource_request(
    method: Method,
    path: &str,
    query: Vec<(String, String)>,
    body: Option<Value>,
) -> crate::Result<Value> {
    request_json(method, path, query, body, MirrorPolicy::MirrorFirst).await
}

pub async fn install_file(
    request: CurseForgeInstallRequest,
) -> crate::Result<CurseForgeInstallResult> {
    install_file_with_metrics(request, None).await
}

pub async fn install_file_with_reporter(
    request: CurseForgeInstallRequest,
    reporter: InstallProgressReporter,
) -> crate::Result<CurseForgeInstallResult> {
    let metrics = CurseForgeDownloadMetrics::with_reporter(reporter.clone());
    let result = install_file_with_metrics(request, Some(&metrics)).await?;
    metrics.finish(&reporter).await?;
    Ok(result)
}

pub async fn install_world_with_reporter(
    request: CurseForgeWorldInstallRequest,
    reporter: InstallProgressReporter,
) -> crate::Result<CurseForgeWorldInstallResult> {
    let state = State::get().await?;
    let instance =
        crate::state::get_instance(&request.instance_id, &state.pool)
            .await?
            .ok_or_else(|| {
                ErrorKind::InputError("Unknown instance".to_string())
            })?;
    if instance.instance.install_stage != InstanceInstallStage::Installed {
        return Err(ErrorKind::InputError(
            "Maps can only be added to an installed instance".to_string(),
        )
        .into());
    }

    let project = get_project(request.project_id).await?;
    if project.id != request.project_id || project.class_id != Some(17) {
        return Err(ErrorKind::InputError(
            "The selected CurseForge project is not a Minecraft world"
                .to_string(),
        )
        .into());
    }

    let file = get_file(request.project_id, request.file_id).await?;
    if file.mod_id != request.project_id {
        return Err(ErrorKind::InputError(
            "CurseForge returned a file for a different project".to_string(),
        )
        .into());
    }
    validate_world_archive_name(&file.file_name)?;

    install_world_file(
        &request.instance_id,
        &project,
        &file,
        Some(&reporter),
        crate::state::instances::ContentOwnershipKind::UserAdded,
        crate::state::instances::ManualDownloadOperationKind::ContentInstall,
    )
    .await
}

async fn install_world_file(
    instance_id: &str,
    project: &CurseForgeProject,
    file: &CurseForgeFile,
    reporter: Option<&InstallProgressReporter>,
    ownership: crate::state::instances::ContentOwnershipKind,
    operation: crate::state::instances::ManualDownloadOperationKind,
) -> crate::Result<CurseForgeWorldInstallResult> {
    validate_world_archive_name(&file.file_name)?;
    if project.class_id != Some(17) || file.mod_id != project.id {
        return Err(ErrorKind::InputError(
            "World file does not belong to the selected world project".into(),
        )
        .into());
    }
    let state = State::get().await?;
    let download_url =
        resolve_curseforge_download_url(project.id, file.id, project, file)
            .await?;
    let Some(download_url) = download_url else {
        let manual_download = manual_download_from_file(
            project.id,
            file.id,
            file,
            project,
            "world",
            "saves".to_string(),
            ownership,
            operation,
        );
        persist_manual_download(instance_id, &manual_download).await?;
        return Ok(CurseForgeWorldInstallResult {
            manual_download: Some(manual_download),
            ..Default::default()
        });
    };

    let staging_directory = tempfile::tempdir()?;
    let staging_path = staging_directory.path().join(
        crate::util::io::sanitize_download_file_name(&file.file_name)?,
    );
    let tracking_path = format!("worlds/{}/{}", project.id, file.id);
    download_curseforge_path(
        &download_url,
        file,
        &staging_path,
        curseforge_content_validation(&file.file_name),
        None,
        reporter.map(|reporter| (reporter, tracking_path.as_str())),
        None,
        true,
    )
    .await?;
    let _critical = crate::install::critical_section::enter();
    let world_name = crate::state::instances::commands::import_world_save(
        &state,
        instance_id,
        &staging_path,
        None,
    )
    .await?;

    Ok(CurseForgeWorldInstallResult {
        world_name: Some(world_name),
        ..Default::default()
    })
}

#[derive(Clone, Copy)]
struct PendingCurseForgeFile {
    project_id: u32,
    file_id: u32,
    item_type: ProjectType,
    dependency: bool,
    parent_project_id: Option<u32>,
    parent_file_id: Option<u32>,
    dependency_kind: Option<crate::state::instances::ContentDependencyKind>,
    depth: usize,
}

#[derive(Clone)]
struct PreviewPendingCurseForgeFile {
    project_id: u32,
    file_id: u32,
    item_type: ProjectType,
    depth: usize,
    ancestors: HashSet<(u32, u32)>,
}

#[derive(Clone)]
struct CurseForgeDependencyEdgeCandidate {
    parent_project_id: u32,
    parent_file_id: u32,
    child_project_id: u32,
    child_file_id: u32,
    dependency_kind: crate::state::instances::ContentDependencyKind,
}

#[derive(Clone)]
struct ModrinthFallbackPlan {
    parent_project_id: u32,
    parent_file_id: u32,
    plan: content_management::ResolveContentPlan,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeModrinthFallbackPreview {
    pub project_id: String,
    pub version_id: String,
    pub title: String,
    pub version_number: String,
    pub parent_project_id: u32,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default = "default_true")]
    pub required: bool,
}

async fn install_file_with_metrics(
    request: CurseForgeInstallRequest,
    download_metrics: Option<&CurseForgeDownloadMetrics>,
) -> crate::Result<CurseForgeInstallResult> {
    let state = State::get().await?;
    let project_type = managed_project_type(&request.project_type)?;
    if let Some(plan) =
        load_dependency_resolution_plan(&request, &state).await?
    {
        let result = install_file_from_resolution_plan(
            &request,
            &plan,
            download_metrics,
            &state,
        )
        .await;
        if result.is_ok() {
            DEPENDENCY_RESOLUTION_PLANS.remove(&plan.id);
        }
        return result;
    }
    let mut result = CurseForgeInstallResult::default();
    let mut visited = HashSet::new();
    let mut projects = HashMap::<u32, CurseForgeProject>::new();
    let mut edge_candidates = Vec::new();
    let mut modrinth_fallbacks = Vec::new();
    let mut fallback_parents = HashSet::new();
    let installed_project_ids =
        crate::state::get_installed_project_ids_for_instance(
            &request.instance_id,
            None,
            &state,
        )
        .await?;
    let mut pending = vec![PendingCurseForgeFile {
        project_id: request.project_id,
        file_id: request.file_id,
        item_type: project_type,
        dependency: false,
        parent_project_id: None,
        parent_file_id: None,
        dependency_kind: None,
        depth: 0,
    }];

    while let Some(pending_file) = pending.pop() {
        if !visited.insert((pending_file.project_id, pending_file.file_id)) {
            continue;
        }
        let project_id = pending_file.project_id;
        let file_id = pending_file.file_id;
        let item_type = pending_file.item_type;
        let dependency = pending_file.dependency;

        let file = get_file(project_id, file_id).await?;
        let project = match projects.get(&project_id) {
            Some(project) => project.clone(),
            None => {
                let project = get_project(project_id).await?;
                projects.insert(project_id, project.clone());
                project
            }
        };
        enqueue_curseforge_dependencies(
            &pending_file,
            &file,
            project_id,
            file_id,
            item_type,
            &request,
            &state,
            &installed_project_ids,
            &mut projects,
            &mut pending,
            &mut result,
            &mut modrinth_fallbacks,
            &mut fallback_parents,
        )
        .await?;

        let download_url = resolve_curseforge_download_url(
            project_id, file_id, &project, &file,
        )
        .await?;
        let Some(download_url) = download_url else {
            let target_folder = content_target_folder(
                item_type,
                request.world_name.as_deref(),
            )?;
            let manual_download = manual_download_from_file(
                project_id,
                file_id,
                &file,
                &project,
                item_type.get_name(),
                target_folder,
                request.ownership_kind,
                request.manual_operation_kind,
            );
            persist_manual_download(&request.instance_id, &manual_download)
                .await?;
            result.manual_downloads.push(manual_download);
            continue;
        };

        validate_file_name(&file.file_name)?;
        let downloaded =
            download_installed_file(DownloadInstalledFileRequest {
                instance_id: &request.instance_id,
                url: &download_url,
                file: &file,
                project_type: item_type,
                world_name: request.world_name.as_deref(),
                project_id,
                file_id,
                project_slug: &project.slug,
                ownership_kind: request.ownership_kind,
                download_metrics,
                defer_persistence: request.defer_persistence,
                verification_tx: request.verification_tx.as_ref(),
                pre_resolved_relative_path: request
                    .pre_resolved_relative_path
                    .as_deref(),
                expected_file_name: request.expected_file_name.as_deref(),
            })
            .await?;
        let relative_path = downloaded.relative_path.clone();
        result.installed.push(CurseForgeInstalledFile {
            project_id,
            file_id,
            relative_path: relative_path.clone(),
            dependency,
        });
        if let (
            Some(parent_project_id),
            Some(parent_file_id),
            Some(dependency_kind),
        ) = (
            pending_file.parent_project_id,
            pending_file.parent_file_id,
            pending_file.dependency_kind,
        ) {
            edge_candidates.push(CurseForgeDependencyEdgeCandidate {
                parent_project_id,
                parent_file_id,
                child_project_id: project_id,
                child_file_id: file_id,
                dependency_kind,
            });
        }
    }

    finalize_install_result(
        &request.instance_id,
        &modrinth_fallbacks,
        &mut result,
        &edge_candidates,
        &state,
    )
    .await?;
    Ok(result)
}

fn dependency_policy(
    operation: crate::state::instances::ManualDownloadOperationKind,
) -> DependencyPolicy {
    if operation
        == crate::state::instances::ManualDownloadOperationKind::ContentUpdate
    {
        DependencyPolicy::RequiredOnly
    } else {
        DependencyPolicy::IncludeOptional
    }
}

fn should_resolve_dependency(
    relation_type: u32,
    operation: crate::state::instances::ManualDownloadOperationKind,
) -> bool {
    let dependency_type = match relation_type {
        DEPENDENCY_RELATION_REQUIRED => DependencyType::Required,
        DEPENDENCY_RELATION_OPTIONAL | DEPENDENCY_RELATION_INCLUDE => {
            DependencyType::Optional
        }
        _ => return false,
    };
    dependency_policy(operation).allows(dependency_type)
}

async fn enqueue_curseforge_dependencies(
    pending_file: &PendingCurseForgeFile,
    file: &CurseForgeFile,
    project_id: u32,
    file_id: u32,
    item_type: ProjectType,
    request: &CurseForgeInstallRequest,
    state: &State,
    installed_project_ids: &[String],
    projects: &mut HashMap<u32, CurseForgeProject>,
    pending: &mut Vec<PendingCurseForgeFile>,
    result: &mut CurseForgeInstallResult,
    modrinth_fallbacks: &mut Vec<ModrinthFallbackPlan>,
    fallback_parents: &mut HashSet<(u32, u32)>,
) -> crate::Result<()> {
    if !request.install_dependencies {
        return Ok(());
    }
    if pending_file.depth >= MAX_DEPENDENCY_DEPTH {
        result
            .skipped_dependencies
            .push(CurseForgeSkippedDependency {
                project_id,
                file_id: Some(file_id),
                reason: "dependency_depth_exceeded".to_string(),
            });
        return Ok(());
    }

    for dependency_ref in &file.dependencies {
        match dependency_ref.relation_type {
            DEPENDENCY_RELATION_EMBEDDED | DEPENDENCY_RELATION_TOOL => {
                result
                    .skipped_dependencies
                    .push(CurseForgeSkippedDependency {
                        project_id: dependency_ref.mod_id,
                        file_id: None,
                        reason: if dependency_ref.relation_type
                            == DEPENDENCY_RELATION_EMBEDDED
                        {
                            "embedded"
                        } else {
                            "tool"
                        }
                        .to_string(),
                    });
            }
            DEPENDENCY_RELATION_INCOMPATIBLE => {
                result.incompatible_dependencies.push(dependency_ref.mod_id)
            }
            DEPENDENCY_RELATION_OPTIONAL
            | DEPENDENCY_RELATION_INCLUDE
            | DEPENDENCY_RELATION_REQUIRED => {
                if !should_resolve_dependency(
                    dependency_ref.relation_type,
                    request.manual_operation_kind,
                ) {
                    continue;
                }
                let dependency_project_id = if request.mod_loader_type
                    == Some(CURSEFORGE_LOADER_QUILT)
                    && dependency_ref.mod_id == FABRIC_API_CURSEFORGE_PROJECT_ID
                {
                    QUILTED_FABRIC_API_CURSEFORGE_PROJECT_ID
                } else {
                    dependency_ref.mod_id
                };
                if request
                    .excluded_dependency_project_ids
                    .contains(&dependency_project_id)
                {
                    result.skipped_dependencies.push(
                        CurseForgeSkippedDependency {
                            project_id: dependency_ref.mod_id,
                            file_id: None,
                            reason: "excluded_by_user".to_string(),
                        },
                    );
                    continue;
                }
                if installed_project_ids
                    .contains(&format!("curseforge:{dependency_project_id}"))
                    && !request
                        .force_dependency_project_ids
                        .contains(&dependency_project_id)
                {
                    result.skipped_dependencies.push(
                        CurseForgeSkippedDependency {
                            project_id: dependency_ref.mod_id,
                            file_id: None,
                            reason: "already_installed".to_string(),
                        },
                    );
                    continue;
                }
                let dependency_project = match projects
                    .get(&dependency_project_id)
                {
                    Some(project) => project.clone(),
                    None => {
                        let project =
                            get_project(dependency_project_id).await?;
                        projects.insert(dependency_project_id, project.clone());
                        project
                    }
                };
                let Some(dependency_type) =
                    recognized_project_type(dependency_project.class_id)
                else {
                    result.failed_downloads.push(CurseForgeFailedDownload {
                        project_id: dependency_project_id,
                        file_id: 0,
                        file_name: dependency_project.name.clone(),
                        reason: "The dependency project type is not supported"
                            .to_string(),
                    });
                    continue;
                };
                if let Some(selected) = select_dependency_file(
                    dependency_project_id,
                    request.game_version.clone(),
                    request.mod_loader_type,
                )
                .await?
                {
                    pending.push(PendingCurseForgeFile {
                        project_id: dependency_project_id,
                        file_id: selected.file.id,
                        item_type: dependency_type,
                        dependency: true,
                        parent_project_id: Some(project_id),
                        parent_file_id: Some(file_id),
                        dependency_kind: Some(if dependency_ref.relation_type
                            == DEPENDENCY_RELATION_REQUIRED
                        {
                            crate::state::instances::ContentDependencyKind::Required
                        } else {
                            crate::state::instances::ContentDependencyKind::Include
                        }),
                        depth: pending_file.depth + 1,
                    });
                } else if fallback_parents.insert((project_id, file_id)) {
                    match resolve_modrinth_fallback_plan(
                        file, item_type, request, state,
                    )
                    .await
                    {
                        Ok(Some(plan)) if !plan.dependencies.is_empty() => {
                            modrinth_fallbacks.push(ModrinthFallbackPlan {
                                parent_project_id: project_id,
                                parent_file_id: file_id,
                                plan,
                            });
                        }
                        Ok(Some(_) | None) => {
                            result.skipped_dependencies.push(
                                CurseForgeSkippedDependency {
                                    project_id: dependency_project_id,
                                    file_id: None,
                                    reason: "no_compatible_version".to_string(),
                                },
                            );
                        }
                        Err(error) => {
                            tracing::warn!(
                                project_id = dependency_project_id,
                                parent_project_id = project_id,
                                parent_file_id = file_id,
                                "Modrinth SHA-1 dependency fallback failed: {error}"
                            );
                            result.skipped_dependencies.push(
                                CurseForgeSkippedDependency {
                                    project_id: dependency_project_id,
                                    file_id: None,
                                    reason: "modrinth_lookup_failed"
                                        .to_string(),
                                },
                            );
                        }
                    }
                } else {
                    result.skipped_dependencies.push(
                        CurseForgeSkippedDependency {
                            project_id: dependency_project_id,
                            file_id: None,
                            reason: "no_compatible_version".to_string(),
                        },
                    );
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// Completes the common post-processing for a CurseForge installation.
///
/// Keeping fallback installation, result normalization, and dependency-edge
/// persistence together makes the main dependency walk easier to reason about
/// and gives preview/resolution-plan flows a single place to reuse later.
async fn finalize_install_result(
    instance_id: &str,
    modrinth_fallbacks: &[ModrinthFallbackPlan],
    result: &mut CurseForgeInstallResult,
    edge_candidates: &[CurseForgeDependencyEdgeCandidate],
    state: &State,
) -> crate::Result<()> {
    install_modrinth_fallbacks(instance_id, modrinth_fallbacks, result, state)
        .await?;

    result.optional_dependencies.sort_unstable();
    result.optional_dependencies.dedup();
    result.incompatible_dependencies.sort_unstable();
    result.incompatible_dependencies.dedup();
    result.skipped_dependencies.sort_unstable();
    result.skipped_dependencies.dedup();

    persist_curseforge_dependency_edges(instance_id, edge_candidates, state)
        .await?;
    Ok(())
}

async fn load_dependency_resolution_plan(
    request: &CurseForgeInstallRequest,
    state: &State,
) -> crate::Result<Option<DependencyResolutionPlan>> {
    let Some(plan_id) = request.dependency_plan_id.as_deref() else {
        return Ok(None);
    };
    let Some(cached) = DEPENDENCY_RESOLUTION_PLANS
        .get(plan_id)
        .map(|cached| cached.clone())
    else {
        return Err(ErrorKind::InputError(
            "The dependency resolution plan has expired".to_string(),
        )
        .into());
    };
    if cached.expires_at <= Instant::now() {
        DEPENDENCY_RESOLUTION_PLANS.remove(plan_id);
        return Err(ErrorKind::InputError(
            "The dependency resolution plan has expired".to_string(),
        )
        .into());
    }
    let expected_primary =
        curseforge_content_ref(request.project_id, request.file_id)?;
    if cached.plan.instance_id != request.instance_id
        || cached.plan.primary != expected_primary
        || cached.plan.target.minecraft_version != request.game_version
        || cached.plan.target.loader
            != request
                .mod_loader_type
                .map(mod_loader_to_slug)
                .map(str::to_string)
        || cached.plan.target.content_type.as_deref()
            != Some(&request.project_type)
    {
        return Err(ErrorKind::InputError(
			"The dependency resolution plan does not match this install request"
				.to_string(),
		)
		.into());
    }
    let current_revision =
        instance_content_revision(&request.instance_id, state).await?;
    if cached.plan.instance_revision != current_revision {
        DEPENDENCY_RESOLUTION_PLANS.remove(plan_id);
        return Err(ErrorKind::InputError(
			"The instance content changed after dependency preview; preview again"
				.to_string(),
		)
		.into());
    }
    Ok(Some(cached.plan))
}

async fn install_file_from_resolution_plan(
    request: &CurseForgeInstallRequest,
    plan: &DependencyResolutionPlan,
    download_metrics: Option<&CurseForgeDownloadMetrics>,
    state: &State,
) -> crate::Result<CurseForgeInstallResult> {
    let primary_type = managed_project_type(&request.project_type)?;
    let mut result = CurseForgeInstallResult::default();
    let mut installed = HashSet::new();
    let mut failed = HashSet::new();

    match install_fixed_curseforge_content(
        request,
        request.project_id,
        request.file_id,
        primary_type,
        false,
        plan.primary_expected_sha1.as_deref(),
        plan.primary_expected_size,
        download_metrics,
        &mut result,
    )
    .await
    {
        Ok(true) => {
            installed.insert(plan.primary.clone());
        }
        Ok(false) => {
            failed.insert(plan.primary.clone());
        }
        Err(error) => {
            result.failed_downloads.push(CurseForgeFailedDownload {
                project_id: request.project_id,
                file_id: request.file_id,
                file_name: format!(
                    "CurseForge {}:{}",
                    request.project_id, request.file_id
                ),
                reason: error.to_string(),
            });
            failed.insert(plan.primary.clone());
        }
    }

    let known_refs = std::iter::once(plan.primary.clone())
        .chain(plan.nodes.iter().map(|node| node.content.clone()))
        .collect::<HashSet<_>>();
    let excluded_project_ids = request
        .excluded_dependency_project_ids
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    let mut pending = Vec::new();
    for node in plan.nodes.clone() {
        let excluded = match &node.content {
            ContentProviderRef::CurseForge { project_id, .. } => {
                excluded_project_ids.contains(&project_id.get())
            }
            ContentProviderRef::Modrinth { .. } => node
                .parent
                .as_ref()
                .and_then(curseforge_project_id)
                .is_some_and(|project_id| {
                    excluded_project_ids.contains(&project_id)
                }),
            ContentProviderRef::McArchive { .. } => false,
        };
        if excluded {
            result
                .skipped_dependencies
                .push(CurseForgeSkippedDependency {
                    project_id: curseforge_project_id(&node.content)
                        .or_else(|| {
                            node.parent.as_ref().and_then(curseforge_project_id)
                        })
                        .unwrap_or_default(),
                    file_id: curseforge_file_id(&node.content).or_else(|| {
                        node.parent.as_ref().and_then(curseforge_file_id)
                    }),
                    reason: "excluded_by_user".to_string(),
                });
            failed.insert(node.content);
        } else {
            pending.push(node);
        }
    }
    while !pending.is_empty() {
        let mut progressed = false;
        let mut index = 0;
        while index < pending.len() {
            let node = &pending[index];
            let parent_ready = node
                .parent
                .as_ref()
                .is_none_or(|parent| installed.contains(parent));
            let parent_failed = node.parent.as_ref().is_some_and(|parent| {
                failed.contains(parent) || !known_refs.contains(parent)
            });
            if parent_failed {
                result
                    .skipped_dependencies
                    .push(CurseForgeSkippedDependency {
                        project_id: curseforge_project_id(&node.content)
                            .unwrap_or_default(),
                        file_id: curseforge_file_id(&node.content),
                        reason: "parent_not_installed".to_string(),
                    });
                failed.insert(node.content.clone());
                pending.remove(index);
                progressed = true;
                continue;
            }
            if !parent_ready {
                index += 1;
                continue;
            }

            let node = pending.remove(index);
            let installed_node = install_resolution_plan_node(
                request,
                plan,
                &node,
                download_metrics,
                state,
                &mut result,
            )
            .await?;
            if installed_node {
                if let Some(ContentProviderRef::CurseForge {
                    project_id,
                    file_id,
                }) = node.parent.as_ref()
                    && let ContentProviderRef::Modrinth {
                        project_id: child_project,
                        version_id: Some(child_version),
                    } = &node.content
                {
                    result.cross_source_dependencies.push(
                        CurseForgeCrossSourceDependency {
                            parent_project_id: project_id.get(),
                            parent_file_id: file_id
                                .map(CurseForgeFileId::get)
                                .unwrap_or_default(),
                            project_id: child_project.to_string(),
                            version_id: child_version.to_string(),
                        },
                    );
                }
                installed.insert(node.content);
            } else {
                failed.insert(node.content);
            }
            progressed = true;
        }
        if !progressed {
            for node in pending.drain(..) {
                result
                    .skipped_dependencies
                    .push(CurseForgeSkippedDependency {
                        project_id: curseforge_project_id(&node.content)
                            .unwrap_or_default(),
                        file_id: curseforge_file_id(&node.content),
                        reason: "dependency_cycle".to_string(),
                    });
            }
        }
    }

    result.skipped_dependencies.sort_unstable();
    result.skipped_dependencies.dedup();
    persist_resolution_plan_dependency_edges(
        &request.instance_id,
        plan,
        &installed,
        state,
    )
    .await?;
    Ok(result)
}

async fn install_resolution_plan_node(
    request: &CurseForgeInstallRequest,
    plan: &DependencyResolutionPlan,
    node: &DependencyResolutionNode,
    download_metrics: Option<&CurseForgeDownloadMetrics>,
    state: &State,
    result: &mut CurseForgeInstallResult,
) -> crate::Result<bool> {
    match &node.content {
        ContentProviderRef::CurseForge {
            project_id,
            file_id,
        } => {
            let Some(file_id) = file_id else {
                result
                    .skipped_dependencies
                    .push(CurseForgeSkippedDependency {
                        project_id: project_id.get(),
                        file_id: None,
                        reason: "missing_version".to_string(),
                    });
                return Ok(false);
            };
            let project_id = project_id.get();
            let project = get_project(project_id).await?;
            let Some(project_type) = recognized_project_type(project.class_id)
            else {
                result
                    .skipped_dependencies
                    .push(CurseForgeSkippedDependency {
                        project_id,
                        file_id: Some(file_id.get()),
                        reason: "unsupported_project_type".to_string(),
                    });
                return Ok(false);
            };
            match install_fixed_curseforge_content(
                request,
                project_id,
                file_id.get(),
                project_type,
                true,
                node.expected_sha1.as_deref(),
                node.expected_size,
                download_metrics,
                result,
            )
            .await
            {
                Ok(installed) => Ok(installed),
                Err(error) => {
                    result.failed_downloads.push(CurseForgeFailedDownload {
                        project_id,
                        file_id: file_id.get(),
                        file_name: project.name,
                        reason: error.to_string(),
                    });
                    Ok(false)
                }
            }
        }
        ContentProviderRef::Modrinth { .. } => {
            match install_fixed_modrinth_content(request, plan, node, state)
                .await
            {
                Ok(()) => Ok(true),
                Err(error) => {
                    result.failed_downloads.push(CurseForgeFailedDownload {
                        project_id: curseforge_project_id(
                            node.parent.as_ref().unwrap_or(&plan.primary),
                        )
                        .unwrap_or_default(),
                        file_id: curseforge_file_id(
                            node.parent.as_ref().unwrap_or(&plan.primary),
                        )
                        .unwrap_or_default(),
                        file_name: format!(
                            "Modrinth {}",
                            node.content.database_project_id()
                        ),
                        reason: error.to_string(),
                    });
                    Ok(false)
                }
            }
        }
        ContentProviderRef::McArchive { .. } => {
            result
                .skipped_dependencies
                .push(CurseForgeSkippedDependency {
                    project_id: 0,
                    file_id: None,
                    reason: "unsupported_provider".to_string(),
                });
            Ok(false)
        }
    }
}

async fn install_fixed_curseforge_content(
    request: &CurseForgeInstallRequest,
    project_id: u32,
    file_id: u32,
    project_type: ProjectType,
    dependency: bool,
    expected_sha1: Option<&str>,
    expected_size: Option<u64>,
    download_metrics: Option<&CurseForgeDownloadMetrics>,
    result: &mut CurseForgeInstallResult,
) -> crate::Result<bool> {
    let file = get_file(project_id, file_id).await?;
    if file.mod_id != project_id || file.id != file_id {
        return Err(ErrorKind::InputError(
            "CurseForge returned metadata for a different planned file"
                .to_string(),
        )
        .into());
    }
    if expected_sha1.is_some_and(|expected| {
        curseforge_file_sha1(&file).as_deref() != Some(expected)
    }) || expected_size.is_some_and(|expected| file.file_length != expected)
    {
        return Err(ErrorKind::InputError(
            "CurseForge file metadata changed after dependency preview"
                .to_string(),
        )
        .into());
    }
    let project = get_project(project_id).await?;
    let Some(download_url) =
        resolve_curseforge_download_url(project_id, file_id, &project, &file)
            .await?
    else {
        let target_folder =
            content_target_folder(project_type, request.world_name.as_deref())?;
        let manual_download = manual_download_from_file(
            project_id,
            file_id,
            &file,
            &project,
            project_type.get_name(),
            target_folder,
            request.ownership_kind,
            request.manual_operation_kind,
        );
        persist_manual_download(&request.instance_id, &manual_download).await?;
        result.manual_downloads.push(manual_download);
        return Ok(false);
    };
    validate_file_name(&file.file_name)?;
    let relative_path = download_installed_file(DownloadInstalledFileRequest {
        instance_id: &request.instance_id,
        url: &download_url,
        file: &file,
        project_type,
        world_name: request.world_name.as_deref(),
        project_id,
        file_id,
        project_slug: &project.slug,
        ownership_kind: request.ownership_kind,
        download_metrics,
        defer_persistence: request.defer_persistence,
        verification_tx: None,
        pre_resolved_relative_path: None,
        expected_file_name: request.expected_file_name.as_deref(),
    })
    .await?;
    result.installed.push(CurseForgeInstalledFile {
        project_id,
        file_id,
        relative_path: relative_path.relative_path,
        dependency,
    });
    Ok(true)
}

async fn install_fixed_modrinth_content(
    request: &CurseForgeInstallRequest,
    plan: &DependencyResolutionPlan,
    node: &DependencyResolutionNode,
    state: &State,
) -> crate::Result<()> {
    let ContentProviderRef::Modrinth {
        project_id,
        version_id: Some(version_id),
    } = &node.content
    else {
        return Err(ErrorKind::InputError(
            "The planned Modrinth dependency has no exact version".to_string(),
        )
        .into());
    };
    let version = CachedEntry::get_version(
        version_id,
        Some(CacheBehaviour::MustRevalidate),
        &state.pool,
        &state.api_semaphore,
    )
    .await?
    .ok_or_else(|| {
        ErrorKind::InputError(
            "The planned Modrinth dependency is no longer available"
                .to_string(),
        )
    })?;
    if version.project_id != project_id.as_str()
        || !modrinth_version_matches_plan_target(&version, &plan.target)
    {
        return Err(ErrorKind::InputError(
			"The planned Modrinth dependency no longer matches the instance target"
				.to_string(),
		)
		.into());
    }
    let file = version
        .files
        .iter()
        .find(|file| file.primary)
        .or_else(|| version.files.first())
        .ok_or_else(|| {
            ErrorKind::InputError(
                "The planned Modrinth dependency has no downloadable file"
                    .to_string(),
            )
        })?;
    if node.expected_sha1.as_deref()
        != file.hashes.get("sha1").map(String::as_str)
        || node
            .expected_size
            .is_some_and(|size| file.size as u64 != size)
    {
        return Err(ErrorKind::InputError(
			"The planned Modrinth file metadata changed after dependency preview"
				.to_string(),
		)
		.into());
    }
    crate::state::instances::commands::install_resolved_dependency(
        &request.instance_id,
        &content_management::ResolvedContent {
            project_id: project_id.to_string(),
            version_id: version_id.to_string(),
            dependent_on_version_id: None,
            required: true,
            metadata: None,
        },
        state,
    )
    .await?;
    Ok(())
}

fn modrinth_version_matches_plan_target(
    version: &crate::state::Version,
    target: &DependencyResolutionTarget,
) -> bool {
    target.minecraft_version.as_deref().is_none_or(|minecraft| {
        version
            .game_versions
            .iter()
            .any(|candidate| candidate == minecraft)
    }) && target.loader.as_deref().is_none_or(|loader| {
        version
            .loaders
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(loader))
    })
}

fn curseforge_project_id(reference: &ContentProviderRef) -> Option<u32> {
    match reference {
        ContentProviderRef::CurseForge { project_id, .. } => {
            Some(project_id.get())
        }
        ContentProviderRef::Modrinth { .. } => None,
        ContentProviderRef::McArchive { .. } => None,
    }
}

fn curseforge_file_id(reference: &ContentProviderRef) -> Option<u32> {
    match reference {
        ContentProviderRef::CurseForge { file_id, .. } => {
            file_id.map(CurseForgeFileId::get)
        }
        ContentProviderRef::Modrinth { .. } => None,
        ContentProviderRef::McArchive { .. } => None,
    }
}

async fn persist_resolution_plan_dependency_edges(
    instance_id: &str,
    plan: &DependencyResolutionPlan,
    installed: &HashSet<ContentProviderRef>,
    state: &State,
) -> crate::Result<()> {
    if plan.edges.is_empty() {
        return Ok(());
    }
    let database_permit = state.acquire_install_db_permit().await?;
    let _instance_lock = state.lock_instance_content(instance_id).await;
    let scope = crate::state::instances::commands::resolve_content_scope(
        instance_id,
        None,
        state,
    )
    .await?;
    let mut tx = state.pool.begin().await?;
    for edge in &plan.edges {
        if !installed.contains(&edge.parent) || !installed.contains(&edge.child)
        {
            continue;
        }
        let (Some(parent_release), Some(child_release)) = (
            edge.parent.database_release_id(),
            edge.child.database_release_id(),
        ) else {
            continue;
        };
        let Some(parent_entry) = crate::state::instances::adapters::sqlite::content_rows::get_content_entry_by_provider_ref(
			&scope.content_set_id,
			edge.parent.provider(),
			&edge.parent.database_project_id(),
			&parent_release,
			&state.pool,
		)
		.await?
		else {
			continue;
		};
        let Some(child_entry) = crate::state::instances::adapters::sqlite::content_rows::get_content_entry_by_provider_ref(
			&scope.content_set_id,
			edge.child.provider(),
			&edge.child.database_project_id(),
			&child_release,
			&state.pool,
		)
		.await?
		else {
			continue;
		};
        let now = chrono::Utc::now();
        crate::state::instances::adapters::sqlite::content_rows::upsert_content_dependency_edge_in_transaction(
			&crate::state::instances::ContentDependencyEdge {
				id: format!("content-dependency:{}", uuid::Uuid::new_v4()),
				content_set_id: scope.content_set_id.clone(),
				parent_entry_id: parent_entry.id,
				child_entry_id: child_entry.id,
				evidence_provider: edge.evidence_provider,
				parent_provider: edge.parent.provider(),
				child_provider: edge.child.provider(),
				dependency_kind: edge.relation,
				parent_project_id: edge.parent.database_project_id(),
				parent_release_id: parent_release,
				child_project_id: edge.child.database_project_id(),
				child_release_id: child_release,
				created_at: now,
				modified_at: now,
			},
			&mut tx,
		)
		.await?;
    }
    tx.commit().await?;
    drop(database_permit);
    Ok(())
}

async fn persist_curseforge_dependency_edges(
    instance_id: &str,
    candidates: &[CurseForgeDependencyEdgeCandidate],
    state: &State,
) -> crate::Result<()> {
    if candidates.is_empty() {
        return Ok(());
    }
    let database_permit = state.acquire_install_db_permit().await?;
    let _instance_lock = state.lock_instance_content(instance_id).await;
    let scope = crate::state::instances::commands::resolve_content_scope(
        instance_id,
        None,
        state,
    )
    .await?;
    let mut tx = state.pool.begin().await?;
    for candidate in candidates {
        let Some(parent_entry) =
            crate::state::instances::adapters::sqlite::content_rows::get_content_entry_by_provider_ref(
                &scope.content_set_id,
                ContentProvider::CurseForge,
                &candidate.parent_project_id.to_string(),
                &candidate.parent_file_id.to_string(),
                &state.pool,
            )
            .await?
        else {
            continue;
        };
        let Some(child_entry) =
            crate::state::instances::adapters::sqlite::content_rows::get_content_entry_by_provider_ref(
                &scope.content_set_id,
                ContentProvider::CurseForge,
                &candidate.child_project_id.to_string(),
                &candidate.child_file_id.to_string(),
                &state.pool,
            )
            .await?
        else {
            continue;
        };
        crate::state::instances::adapters::sqlite::content_rows::set_content_entry_auto_dependency_in_transaction(
            &child_entry.id,
            true,
            &mut tx,
        )
        .await?;
        let now = chrono::Utc::now();
        crate::state::instances::adapters::sqlite::content_rows::upsert_content_dependency_edge_in_transaction(
            &crate::state::instances::ContentDependencyEdge {
                id: format!("content-dependency:{}", uuid::Uuid::new_v4()),
                content_set_id: scope.content_set_id.clone(),
                parent_entry_id: parent_entry.id,
                child_entry_id: child_entry.id,
                evidence_provider: ContentProvider::CurseForge,
                parent_provider: ContentProvider::CurseForge,
                child_provider: ContentProvider::CurseForge,
                dependency_kind: candidate.dependency_kind,
                parent_project_id: candidate.parent_project_id.to_string(),
                parent_release_id: candidate.parent_file_id.to_string(),
                child_project_id: candidate.child_project_id.to_string(),
                child_release_id: candidate.child_file_id.to_string(),
                created_at: now,
                modified_at: now,
            },
            &mut tx,
        )
        .await?;
    }
    tx.commit().await?;
    drop(database_permit);
    Ok(())
}

async fn install_modrinth_fallbacks(
    instance_id: &str,
    fallbacks: &[ModrinthFallbackPlan],
    result: &mut CurseForgeInstallResult,
    state: &State,
) -> crate::Result<()> {
    let mut installed_versions_by_parent = Vec::new();

    for fallback in fallbacks {
        let mut installed_versions =
            HashSet::from([fallback.plan.primary.version_id.clone()]);
        for dependency in &fallback.plan.dependencies {
            if dependency
                .dependent_on_version_id
                .as_ref()
                .is_some_and(|parent| !installed_versions.contains(parent))
            {
                continue;
            }
            match crate::state::instances::commands::install_resolved_dependency(
                instance_id, dependency, state,
            )
            .await
            {
                Ok(_) => {
                    installed_versions.insert(dependency.version_id.clone());
                    result.cross_source_dependencies.push(
                        CurseForgeCrossSourceDependency {
                            parent_project_id: fallback.parent_project_id,
                            parent_file_id: fallback.parent_file_id,
                            project_id: dependency.project_id.clone(),
                            version_id: dependency.version_id.clone(),
                        },
                    );
                }
                Err(error) => result.failed_downloads.push(
                    CurseForgeFailedDownload {
                        project_id: fallback.parent_project_id,
                        file_id: fallback.parent_file_id,
                        file_name: format!(
                            "Modrinth {} {}",
                            dependency.project_id, dependency.version_id
                        ),
                        reason: error.to_string(),
                    },
                ),
            }
        }
        installed_versions_by_parent.push((fallback, installed_versions));
    }

    persist_modrinth_fallback_dependency_edges(
        instance_id,
        &installed_versions_by_parent,
        state,
    )
    .await
}

async fn persist_modrinth_fallback_dependency_edges(
    instance_id: &str,
    fallbacks: &[(&ModrinthFallbackPlan, HashSet<String>)],
    state: &State,
) -> crate::Result<()> {
    if fallbacks.is_empty() {
        return Ok(());
    }
    let database_permit = state.acquire_install_db_permit().await?;
    let _instance_lock = state.lock_instance_content(instance_id).await;
    let scope = crate::state::instances::commands::resolve_content_scope(
        instance_id,
        None,
        state,
    )
    .await?;
    let mut tx = state.pool.begin().await?;

    for (fallback, installed_versions) in fallbacks {
        for dependency in &fallback.plan.dependencies {
            if !installed_versions.contains(&dependency.version_id) {
                continue;
            }
            let Some(parent_version_id) =
                dependency.dependent_on_version_id.as_deref()
            else {
                continue;
            };
            let parent = if parent_version_id
                == fallback.plan.primary.version_id
            {
                crate::state::instances::adapters::sqlite::content_rows::get_content_entry_by_provider_ref(
                    &scope.content_set_id,
                    ContentProvider::CurseForge,
                    &fallback.parent_project_id.to_string(),
                    &fallback.parent_file_id.to_string(),
                    &state.pool,
                )
                .await?
                .map(|entry| (
                    entry,
                    ContentProvider::CurseForge,
                    fallback.parent_project_id.to_string(),
                    fallback.parent_file_id.to_string(),
                ))
            } else if let Some(parent_content) = fallback
                .plan
                .dependencies
                .iter()
                .find(|content| content.version_id == parent_version_id)
            {
                crate::state::instances::adapters::sqlite::content_rows::get_content_entry_by_provider_ref(
                    &scope.content_set_id,
                    ContentProvider::Modrinth,
                    &parent_content.project_id,
                    &parent_content.version_id,
                    &state.pool,
                )
                .await?
                .map(|entry| (
                    entry,
                    ContentProvider::Modrinth,
                    parent_content.project_id.clone(),
                    parent_content.version_id.clone(),
                ))
            } else {
                None
            };
            let Some((
                parent_entry,
                parent_provider,
                parent_project_id,
                parent_release_id,
            )) = parent
            else {
                continue;
            };
            let Some(child_entry) = crate::state::instances::adapters::sqlite::content_rows::get_content_entry_by_provider_ref(
                &scope.content_set_id,
                ContentProvider::Modrinth,
                &dependency.project_id,
                &dependency.version_id,
                &state.pool,
            )
            .await?
            else {
                continue;
            };
            let now = chrono::Utc::now();
            crate::state::instances::adapters::sqlite::content_rows::upsert_content_dependency_edge_in_transaction(
                &crate::state::instances::ContentDependencyEdge {
                    id: format!("content-dependency:{}", uuid::Uuid::new_v4()),
                    content_set_id: scope.content_set_id.clone(),
                    parent_entry_id: parent_entry.id,
                    child_entry_id: child_entry.id,
                    evidence_provider: ContentProvider::Modrinth,
                    parent_provider,
                    child_provider: ContentProvider::Modrinth,
                    dependency_kind: crate::state::instances::ContentDependencyKind::Required,
                    parent_project_id,
                    parent_release_id,
                    child_project_id: dependency.project_id.clone(),
                    child_release_id: dependency.version_id.clone(),
                    created_at: now,
                    modified_at: now,
                },
                &mut tx,
            )
            .await?;
        }
    }
    tx.commit().await?;
    drop(database_permit);
    Ok(())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgePreviewItem {
    pub project_id: u32,
    pub file_id: u32,
    pub title: String,
    pub version_number: String,
    pub file_name: String,
    pub size: u64,
    pub required_by_project_ids: Vec<u32>,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub version_mismatch: bool,
    #[serde(default)]
    pub selection_reason: Option<CurseForgeDependencySelectionReason>,
    #[serde(default = "default_true")]
    pub required: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeInstallPreview {
    pub plan_id: String,
    pub primary: CurseForgePreviewItem,
    pub dependencies: Vec<CurseForgePreviewItem>,
    #[serde(default)]
    pub modrinth_fallbacks: Vec<CurseForgeModrinthFallbackPreview>,
    pub skipped: Vec<CurseForgeSkippedDependency>,
    pub optional_dependencies: Vec<u32>,
    pub incompatible_dependencies: Vec<u32>,
}

pub async fn preview_install_file(
    request: CurseForgeInstallRequest,
) -> crate::Result<CurseForgeInstallPreview> {
    let state = State::get().await?;
    let primary_type = managed_project_type(&request.project_type)?;
    let primary_file = get_file(request.project_id, request.file_id).await?;
    let primary_project = get_project(request.project_id).await?;
    let primary_ref =
        curseforge_content_ref(request.project_id, request.file_id)?;
    let instance_revision =
        instance_content_revision(&request.instance_id, &state).await?;
    let mut plan = DependencyResolutionPlan {
        id: format!("dependency-resolution-plan:{}", uuid::Uuid::new_v4()),
        instance_id: request.instance_id.clone(),
        instance_revision,
        target: DependencyResolutionTarget {
            minecraft_version: request.game_version.clone(),
            loader: request
                .mod_loader_type
                .map(mod_loader_to_slug)
                .map(str::to_string),
            content_type: Some(request.project_type.clone()),
        },
        primary: primary_ref.clone(),
        primary_expected_sha1: curseforge_file_sha1(&primary_file),
        primary_expected_size: Some(primary_file.file_length),
        nodes: Vec::new(),
        edges: Vec::new(),
        issues: Vec::new(),
    };
    let installed_project_ids = if request.instance_id.is_empty() {
        Vec::new()
    } else {
        crate::state::get_installed_project_ids_for_instance(
            &request.instance_id,
            None,
            &state,
        )
        .await?
    };
    let mut visited = HashSet::new();
    let mut projects = HashMap::<u32, CurseForgeProject>::new();
    projects.insert(request.project_id, primary_project.clone());
    let mut dependencies: Vec<CurseForgePreviewItem> = Vec::new();
    let mut modrinth_fallbacks = Vec::new();
    let mut fallback_parents = HashSet::new();
    let mut skipped = Vec::new();
    let mut optional = Vec::new();
    let mut incompatible = Vec::new();
    let mut pending = vec![PreviewPendingCurseForgeFile {
        project_id: request.project_id,
        file_id: request.file_id,
        item_type: primary_type,
        depth: 0,
        ancestors: HashSet::new(),
    }];

    while let Some(pending_file) = pending.pop() {
        let project_id = pending_file.project_id;
        let file_id = pending_file.file_id;
        let item_type = pending_file.item_type;
        if !visited.insert((project_id, file_id)) {
            continue;
        }
        let file =
            if project_id == request.project_id && file_id == request.file_id {
                primary_file.clone()
            } else {
                get_file(project_id, file_id).await?
            };
        if request.install_dependencies
            && pending_file.depth < MAX_DEPENDENCY_DEPTH
        {
            for dependency_ref in &file.dependencies {
                let dependency_project_id = if request.mod_loader_type
                    == Some(CURSEFORGE_LOADER_QUILT)
                    && dependency_ref.mod_id == FABRIC_API_CURSEFORGE_PROJECT_ID
                {
                    QUILTED_FABRIC_API_CURSEFORGE_PROJECT_ID
                } else {
                    dependency_ref.mod_id
                };
                match dependency_ref.relation_type {
                    DEPENDENCY_RELATION_INCOMPATIBLE => {
                        incompatible.push(dependency_ref.mod_id)
                    }
                    DEPENDENCY_RELATION_EMBEDDED | DEPENDENCY_RELATION_TOOL => {
                        skipped.push(CurseForgeSkippedDependency {
                            project_id: dependency_ref.mod_id,
                            file_id: None,
                            reason: if dependency_ref.relation_type
                                == DEPENDENCY_RELATION_EMBEDDED
                            {
                                "embedded".to_string()
                            } else {
                                "tool".to_string()
                            },
                        })
                    }
                    DEPENDENCY_RELATION_OPTIONAL
                    | DEPENDENCY_RELATION_INCLUDE
                    | DEPENDENCY_RELATION_REQUIRED => {
                        if !should_resolve_dependency(
                            dependency_ref.relation_type,
                            request.manual_operation_kind,
                        ) {
                            continue;
                        }
                        if request
                            .excluded_dependency_project_ids
                            .contains(&dependency_project_id)
                        {
                            skipped.push(CurseForgeSkippedDependency {
                                project_id: dependency_project_id,
                                file_id: None,
                                reason: "excluded_by_user".to_string(),
                            });
                            continue;
                        }
                        if installed_project_ids.contains(&format!(
                            "curseforge:{dependency_project_id}"
                        )) && !request
                            .force_dependency_project_ids
                            .contains(&dependency_project_id)
                        {
                            skipped.push(CurseForgeSkippedDependency {
                                project_id: dependency_project_id,
                                file_id: None,
                                reason: "already_installed".to_string(),
                            });
                            continue;
                        }
                        let dependency_project = match projects
                            .get(&dependency_project_id)
                        {
                            Some(project) => project.clone(),
                            None => {
                                let project =
                                    get_project(dependency_project_id).await?;
                                projects.insert(
                                    dependency_project_id,
                                    project.clone(),
                                );
                                project
                            }
                        };
                        let Some(dependency_type) = recognized_project_type(
                            dependency_project.class_id,
                        ) else {
                            skipped.push(CurseForgeSkippedDependency {
                                project_id: dependency_project_id,
                                file_id: None,
                                reason: "unsupported_project_type".to_string(),
                            });
                            continue;
                        };
                        let Some(selected) = select_dependency_file(
                            dependency_project_id,
                            request.game_version.clone(),
                            request.mod_loader_type,
                        )
                        .await?
                        else {
                            if fallback_parents.insert((project_id, file_id)) {
                                match resolve_modrinth_fallback_plan(
                                    &file, item_type, &request, &state,
                                )
                                .await
                                {
                                    Ok(Some(fallback_plan))
                                        if !fallback_plan
                                            .dependencies
                                            .is_empty() =>
                                    {
                                        append_modrinth_fallback_plan(
                                            &mut plan,
                                            &fallback_plan,
                                            &curseforge_content_ref(
                                                project_id, file_id,
                                            )?,
                                            &state,
                                        )
                                        .await?;
                                        modrinth_fallbacks.extend(
                                            preview_modrinth_fallbacks(
                                                &fallback_plan,
                                                project_id,
                                                &state,
                                            )
                                            .await?,
                                        );
                                        continue;
                                    }
                                    Ok(Some(_) | None) => {}
                                    Err(error) => {
                                        tracing::warn!(
                                            project_id = dependency_project_id,
                                            parent_project_id = project_id,
                                            parent_file_id = file_id,
                                            "Modrinth SHA-1 dependency fallback failed: {error}"
                                        );
                                        skipped.push(
                                            CurseForgeSkippedDependency {
                                                project_id:
                                                    dependency_project_id,
                                                file_id: None,
                                                reason:
                                                    "modrinth_lookup_failed"
                                                        .to_string(),
                                            },
                                        );
                                        continue;
                                    }
                                }
                            }
                            skipped.push(CurseForgeSkippedDependency {
                                project_id: dependency_project_id,
                                file_id: None,
                                reason: "no_compatible_version".to_string(),
                            });
                            continue;
                        };
                        let dependency_file = selected.file;
                        let child_ref = curseforge_content_ref(
                            dependency_project_id,
                            dependency_file.id,
                        )?;
                        if pending_file.ancestors.contains(&(
                            dependency_project_id,
                            dependency_file.id,
                        )) || (dependency_project_id, dependency_file.id)
                            == (project_id, file_id)
                        {
                            skipped.push(CurseForgeSkippedDependency {
                                project_id: dependency_project_id,
                                file_id: Some(dependency_file.id),
                                reason: "dependency_cycle".to_string(),
                            });
                            continue;
                        }
                        let parent_ref =
                            curseforge_content_ref(project_id, file_id)?;
                        plan.edges.push(DependencyResolutionEdge {
							parent: parent_ref.clone(),
							child: child_ref.clone(),
							relation: if dependency_ref.relation_type
								== DEPENDENCY_RELATION_REQUIRED
							{
								crate::state::instances::ContentDependencyKind::Required
							} else {
								crate::state::instances::ContentDependencyKind::Include
							},
							evidence_provider: ContentProvider::CurseForge,
						});
                        if !plan
                            .nodes
                            .iter()
                            .any(|node| node.content == child_ref)
                        {
                            plan.nodes.push(DependencyResolutionNode {
								content: child_ref,
								parent: Some(parent_ref),
							relation: if dependency_ref.relation_type
								== DEPENDENCY_RELATION_REQUIRED
							{
								crate::state::instances::ContentDependencyKind::Required
							} else {
								crate::state::instances::ContentDependencyKind::Include
							},
								source: ContentProvider::CurseForge,
								selection_reason: selected.reason.into(),
								expected_sha1: curseforge_file_sha1(&dependency_file),
								expected_size: Some(dependency_file.file_length),
							});
                        }
                        let existing = dependencies.iter_mut().find(|item| {
                            item.project_id == dependency_project_id
                        });
                        if let Some(existing) = existing {
                            existing.required |= dependency_ref.relation_type
                                == DEPENDENCY_RELATION_REQUIRED;
                            if !existing
                                .required_by_project_ids
                                .contains(&project_id)
                            {
                                existing
                                    .required_by_project_ids
                                    .push(project_id);
                            }
                            continue;
                        }
                        dependencies.push(CurseForgePreviewItem {
                            project_id: dependency_project_id,
                            file_id: dependency_file.id,
                            title: dependency_project.name.clone(),
                            version_number: dependency_file
                                .display_name
                                .clone(),
                            file_name: dependency_file.file_name.clone(),
                            size: dependency_file.file_length,
                            required_by_project_ids: vec![project_id],
                            icon_url: dependency_project
                                .logo
                                .as_ref()
                                .map(|logo| logo.thumbnail_url.clone()),
                            version_mismatch: false,
                            selection_reason: Some(selected.reason),
                            required: dependency_ref.relation_type
                                == DEPENDENCY_RELATION_REQUIRED,
                        });
                        let mut ancestors = pending_file.ancestors.clone();
                        ancestors.insert((project_id, file_id));
                        pending.push(PreviewPendingCurseForgeFile {
                            project_id: dependency_project_id,
                            file_id: dependency_file.id,
                            item_type: dependency_type,
                            depth: pending_file.depth + 1,
                            ancestors,
                        });
                    }
                    _ => {}
                }
            }
        } else if request.install_dependencies {
            skipped.push(CurseForgeSkippedDependency {
                project_id,
                file_id: Some(file_id),
                reason: "dependency_depth_exceeded".to_string(),
            });
        }
    }

    optional.sort_unstable();
    optional.dedup();
    incompatible.sort_unstable();
    incompatible.dedup();
    skipped.sort_unstable();
    skipped.dedup();
    modrinth_fallbacks.sort_by(|left, right| {
        left.project_id
            .cmp(&right.project_id)
            .then_with(|| left.version_id.cmp(&right.version_id))
    });
    modrinth_fallbacks.dedup_by(|left, right| {
        left.project_id == right.project_id
            && left.version_id == right.version_id
    });
    plan.issues.extend(skipped.iter().map(|skipped| {
        DependencyResolutionIssue {
            provider: ContentProvider::CurseForge,
            project_id: skipped.project_id.to_string(),
            parent: None,
            relation: Some(
                crate::state::instances::ContentDependencyKind::Required,
            ),
            reason: skipped.reason.clone(),
        }
    }));
    store_dependency_resolution_plan(plan.clone());

    Ok(CurseForgeInstallPreview {
        plan_id: plan.id,
        primary: CurseForgePreviewItem {
            project_id: request.project_id,
            file_id: request.file_id,
            title: primary_project.name,
            version_number: primary_file.display_name,
            file_name: primary_file.file_name,
            size: primary_file.file_length,
            required_by_project_ids: Vec::new(),
            icon_url: primary_project
                .logo
                .as_ref()
                .map(|logo| logo.thumbnail_url.clone()),
            version_mismatch: false,
            selection_reason: None,
            required: true,
        },
        dependencies,
        modrinth_fallbacks,
        skipped,
        optional_dependencies: optional,
        incompatible_dependencies: incompatible,
    })
}

pub async fn install_modpack(
    request: CurseForgeModpackInstallRequest,
) -> crate::Result<CurseForgeModpackInstallResult> {
    install_modpack_with_reporter(request, None).await
}

pub async fn get_modpack_target(
    project_id: u32,
    file_id: u32,
) -> crate::Result<CurseForgeModpackTarget> {
    let pack_file = get_file(project_id, file_id).await?;
    let project = get_project(project_id).await?;
    let download_url = resolve_curseforge_download_url(
        project_id, file_id, &project, &pack_file,
    )
    .await?
    .ok_or_else(|| {
        ErrorKind::InputError(
            "The CurseForge modpack manifest cannot be downloaded automatically"
                .to_string(),
        )
    })?;

    let icon_url = project.logo.as_ref().and_then(|logo| {
        if !logo.thumbnail_url.is_empty() {
            Some(logo.thumbnail_url.clone())
        } else if !logo.url.is_empty() {
            Some(logo.url.clone())
        } else {
            None
        }
    });
    let loading_bar = init_loading(
        LoadingBarType::PackFileDownload {
            instance_id: String::new(),
            pack_name: project.name.clone(),
            icon: icon_url,
            pack_version: pack_file.display_name.clone(),
        },
        pack_file.file_length.max(1) as f64,
        &format!("Downloading {}", pack_file.file_name),
    )
    .await?;
    let mut last_downloaded = 0_u64;
    let mut progress = |current: u64,
                        _total: u64|
     -> std::pin::Pin<
        Box<dyn std::future::Future<Output = crate::Result<()>> + Send>,
    > {
        let delta = current.saturating_sub(last_downloaded);
        last_downloaded = current;
        let result = emit_loading(
            &loading_bar,
            delta as f64,
            Some("Downloading CurseForge modpack"),
        );
        Box::pin(async move { result })
    };
    let pack_download = download_curseforge_archive(
        project_id,
        file_id,
        &pack_file,
        &download_url,
        Some(&mut progress as &mut FetchProgressFn<'_>),
        None,
    )
    .await?;
    let pack_path = pack_download.path;
    let target = tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&pack_path)?;
        let mut archive =
            zip::ZipArchive::new(file).map_err(modpack_zip_error)?;
        let manifest = read_modpack_manifest(&mut archive)?;
        modpack_target(&manifest)
    })
    .await??;
    Ok(target)
}

async fn download_modpack_archive_with_reporter(
    project_id: u32,
    file_id: u32,
    pack_file: &CurseForgeFile,
    download_url: &str,
    pack_details: InstallPhaseDetails,
    reporter: Option<InstallProgressReporter>,
) -> crate::Result<(PathBuf, CurseForgeModpackManifest)> {
    if let Some(reporter) = reporter.as_ref() {
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
                InstallPhaseId::DownloadingPackFile,
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

    let mut last_downloaded = 0_u64;
    let progress_reporter = reporter.clone();
    let progress_details = pack_details.clone();
    let mut progress = move |current: u64,
                             total: u64|
          -> std::pin::Pin<
        Box<dyn std::future::Future<Output = crate::Result<()>> + Send>,
    > {
        let min_delta = (total / 200).max(256 * 1024);
        if current < total
            && current.saturating_sub(last_downloaded) < min_delta
        {
            return Box::pin(async { Ok(()) });
        }
        last_downloaded = current;
        let reporter = progress_reporter.clone();
        let details = progress_details.clone();
        Box::pin(async move {
            if let Some(reporter) = reporter {
                reporter
                    .update(
                        InstallPhaseId::DownloadingPackFile,
                        Some(InstallProgress {
                            current,
                            total,
                            secondary: None,
                        }),
                        details,
                    )
                    .await?;
            }
            Ok(())
        })
    };
    let progress = reporter
        .is_some()
        .then_some(&mut progress as &mut FetchProgressFn<'_>);
    let pack_download = download_curseforge_archive(
        project_id,
        file_id,
        pack_file,
        download_url,
        progress,
        reporter.as_ref(),
    )
    .await?;
    if let Some(reporter) = reporter.as_ref()
        && pack_download.attempts > 0
    {
        reporter
            .record_download_metrics(
                pack_download.source.as_str(),
                pack_download.fallback_count as u64,
            )
            .await?;
    }
    let pack_path = pack_download.path;
    if let Some(reporter) = reporter.as_ref() {
        reporter
            .update(
                InstallPhaseId::DownloadingPackFile,
                Some(InstallProgress {
                    current: pack_file.file_length,
                    total: pack_file.file_length.max(1),
                    secondary: None,
                }),
                pack_details.clone(),
            )
            .await?;
        reporter
            .update(InstallPhaseId::ReadingPackManifest, None, pack_details)
            .await?;
    }
    let pack_path_for_manifest = pack_path.clone();
    let manifest = tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&pack_path_for_manifest)?;
        let mut archive =
            zip::ZipArchive::new(file).map_err(modpack_zip_error)?;
        read_modpack_manifest(&mut archive)
    })
    .await??;
    Ok((pack_path, manifest))
}

pub async fn install_modpack_with_reporter(
    request: CurseForgeModpackInstallRequest,
    reporter: Option<InstallProgressReporter>,
) -> crate::Result<CurseForgeModpackInstallResult> {
    let reporter =
        reporter.map(InstallProgressReporter::with_pipeline_cancellation);
    let pack_file = get_file(request.project_id, request.file_id).await?;
    let project = get_project(request.project_id).await?;
    let icon_url = project
        .logo
        .as_ref()
        .map(|logo| {
            if !logo.thumbnail_url.is_empty() {
                logo.thumbnail_url.clone()
            } else {
                logo.url.clone()
            }
        })
        .filter(|url| !url.is_empty());
    let download_url = resolve_curseforge_download_url(
        request.project_id,
        request.file_id,
        &project,
        &pack_file,
    )
    .await?;
    let Some(download_url) = download_url else {
        let manual_download = CurseForgeManualDownload {
			project_id: request.project_id,
			file_id: request.file_id,
			file_name: pack_file.file_name.clone(),
			ownership_kind: crate::state::instances::ContentOwnershipKind::PackManaged,
			operation_kind: crate::state::instances::ManualDownloadOperationKind::PackInstall,
			website_url: curseforge_file_page_url(
				project.links.website_url.as_deref(),
				request.file_id,
			),
			project_type: "modpack".to_string(),
			project_slug: project.slug.clone(),
			target_folder: String::new(),
			hashes: pack_file.hashes.clone(),
			file_length: pack_file.file_length,
			file_fingerprint: pack_file.file_fingerprint,
		};
        persist_manual_download(&request.instance_id, &manual_download).await?;
        return Ok(CurseForgeModpackInstallResult {
            content: CurseForgeInstallResult {
                manual_downloads: vec![manual_download],
                ..Default::default()
            },
            ..Default::default()
        });
    };

    let cached_icon_path = if let Some(icon_url) = icon_url.as_ref() {
        match cache_instance_icon_from_url(icon_url).await {
            Ok(path) => {
                let _ = crate::api::instance::edit_icon(
                    &request.instance_id,
                    Some(path.as_path()),
                )
                .await;
                Some(path)
            }
            Err(err) => {
                tracing::warn!(
                    "Failed to cache CurseForge modpack icon: {err}"
                );
                None
            }
        }
    } else {
        None
    };

    let pack_details = InstallPhaseDetails::Modpack {
        project_id: Some(request.project_id.to_string()),
        version_id: Some(request.file_id.to_string()),
        title: Some(project.name.clone()),
    };
    let (pack_path, manifest) = download_modpack_archive_with_reporter(
        request.project_id,
        request.file_id,
        &pack_file,
        &download_url,
        pack_details.clone(),
        reporter.clone(),
    )
    .await?;

    let state = State::get().await?;
    use sqlx::Row;
    let instance_target = sqlx::query(
        "SELECT content_set.game_version, content_set.loader
         FROM instances instance
         INNER JOIN instance_content_sets content_set
            ON content_set.id = instance.applied_content_set_id
         WHERE instance.id = ?",
    )
    .bind(&request.instance_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| {
        ErrorKind::InputError(
            "The selected instance has no active Minecraft installation"
                .to_string(),
        )
    })?;
    let instance_game_version =
        instance_target.try_get::<String, _>("game_version")?;
    let instance_loader = instance_target.try_get::<String, _>("loader")?;
    let target = modpack_target(&manifest)?;
    let loader = (target.loader != ModLoader::Vanilla)
        .then(|| target.loader.as_str().to_string());
    if (instance_game_version != manifest.minecraft.version
        || target.loader.as_str() != instance_loader)
        && !request.allow_target_change
    {
        return Err(ErrorKind::InputError(format!(
				"This modpack targets Minecraft {} with {}, while the selected instance uses {} with {}",
				manifest.minecraft.version,
				loader.as_deref().unwrap_or("vanilla"),
				instance_game_version,
				instance_loader
			))
			.into());
    }

    let content_set = crate::state::instances::adapters::sqlite::content_rows::get_applied_content_set(
			&request.instance_id,
			&state.pool,
		)
		.await?
		.ok_or_else(|| {
			ErrorKind::InputError(
				"Instance has no applied content set".to_string(),
			)
		})?;
    let pack_members = crate::state::instances::adapters::sqlite::content_rows::get_pack_members(
			&content_set.id,
			&state.pool,
		)
		.await?;
    let (selected_files, disabled_pack_projects) =
        select_modpack_manifest_files(
            &manifest,
            &pack_members,
            request.install_optional,
        );
    let loader_type_value = loader.as_deref().and_then(loader_type);
    let project_ids = selected_files
        .iter()
        .map(|file| file.project_id)
        .collect::<Vec<_>>();
    let projects = get_modpack_projects(project_ids).await?;

    let instance_name = crate::api::instance::get(&request.instance_id)
        .await?
        .map(|metadata| metadata.instance.name)
        .unwrap_or_else(|| project.name.clone());
    let total_files = selected_files.len().max(1);
    let file_ids = selected_files
        .iter()
        .map(|file| file.file_id)
        .collect::<Vec<_>>();
    let file_meta = get_modpack_files(file_ids).await?;
    let existing_relative_paths = Arc::new(
        crate::state::instances::adapters::sqlite::content_rows::get_instance_files(
            &request.instance_id,
            &state.pool,
        )
        .await?
        .into_iter()
        .map(|file| file.relative_path)
        .collect::<HashSet<_>>(),
    );
    let prefer_localized_names =
        Settings::get(&state.pool).await?.resource_chinese_filenames;
    let content_total_bytes = selected_files
        .iter()
        .map(|file| {
            file_meta
                .get(&file.file_id)
                .map(|meta| meta.file_length)
                .unwrap_or(0)
        })
        .sum::<u64>();
    // Keep the LoadingBarId in an Arc. LoadingBarId::Drop removes the bar, so
    // cloning the ID itself would destroy progress as soon as the first task
    // finished. Arc clones only share ownership.
    let loading_bar = if reporter.is_none() {
        Some(Arc::new(
            init_loading(
                LoadingBarType::PackDownload {
                    instance_id: request.instance_id.clone(),
                    pack_name: project.name.clone(),
                    icon: cached_icon_path.clone(),
                    pack_id: Some(request.project_id.to_string()),
                    pack_version: Some(request.file_id.to_string()),
                },
                total_files as f64,
                &format!("Downloading {instance_name}"),
            )
            .await?,
        ))
    } else {
        None
    };
    if let Some(loading_bar) = loading_bar.as_ref() {
        let _ = emit_loading(
            loading_bar.as_ref(),
            0.0,
            Some(&format!(
                "0/{total_files} files · 0 / {}",
                format_bytes(content_total_bytes)
            )),
        );
    }
    if let Some(reporter) = reporter.as_ref() {
        reporter
            .update_with_events(
                InstallPhaseId::DownloadingContent,
                Some(InstallProgress {
                    current: 0,
                    total: total_files as u64,
                    secondary: Some(InstallProgressSecondary {
                        current: 0,
                        total: content_total_bytes,
                    }),
                }),
                pack_details.clone(),
                vec![InstallJobEventKind::ContentDownloadStarted {
                    files: total_files as u64,
                    bytes: Some(content_total_bytes),
                }],
            )
            .await?;
    }

    tracing::info!(
        selected_manifest_files = selected_files.len(),
        "Resolved CurseForge modpack manifest files"
    );
    let content = Arc::new(Mutex::new(CurseForgeInstallResult::default()));
    let files_done = Arc::new(AtomicU64::new(0));
    let bytes_done = Arc::new(AtomicU64::new(0));
    let active_downloads = Arc::new(AtomicU64::new(0));
    let cancellation = reporter
        .as_ref()
        .map(InstallProgressReporter::cancellation_token)
        .unwrap_or_default();
    // Keep pipeline memory and staged-file growth bounded independently of
    // the manifest size. The buffer absorbs normal verifier/SQLite jitter;
    // sustained downstream pressure intentionally reaches the producers.
    let (verification_tx, verification_rx) =
        mpsc::channel::<CurseForgeVerificationTask>(
            MODPACK_VERIFICATION_QUEUE_CAPACITY,
        );
    let verification_context = CurseForgeVerificationContext {
        reporter: reporter.clone(),
        loading_bar: loading_bar.clone(),
        details: pack_details.clone(),
        files_done: files_done.clone(),
        bytes_done: bytes_done.clone(),
        active_downloads: active_downloads.clone(),
        total_files: total_files as u64,
        total_bytes: content_total_bytes,
        cancellation: cancellation.clone(),
    };
    let (database_tx, database_rx) = mpsc::channel::<CurseForgeDatabaseTask>(
        MODPACK_DATABASE_QUEUE_CAPACITY,
    );
    let database_worker = spawn_curseforge_database_worker(
        database_rx,
        verification_context.clone(),
    );
    let verification_worker = spawn_curseforge_verification_worker(
        verification_rx,
        verification_context,
        database_tx.clone(),
    );
    let download_metrics = reporter.as_ref().map(|reporter| {
        Arc::new(CurseForgeDownloadMetrics::with_reporter(reporter.clone()))
    });
    let projects = Arc::new(projects);
    let file_meta = Arc::new(file_meta);
    let minecraft_version = manifest.minecraft.version.clone();
    let download_result = loading_try_for_each_concurrent(
        stream::iter(selected_files.into_iter().map(Ok::<_, crate::Error>)),
        Some(state.download_concurrency()),
        // Progress is updated manually with file+byte counts below.
        None,
        1.0,
        total_files,
        None,
        |manifest_file| {
            let content = content.clone();
            let projects = projects.clone();
            let file_meta = file_meta.clone();
            let files_done = files_done.clone();
            let bytes_done = bytes_done.clone();
            let active_downloads = active_downloads.clone();
            let loading_bar = loading_bar.clone();
            let reporter = reporter.clone();
            let download_metrics = download_metrics.clone();
            let pack_details = pack_details.clone();
            let minecraft_version = minecraft_version.clone();
            let request = request.clone();
            let verification_tx = verification_tx.clone();
            let cancellation = cancellation.clone();
            let existing_relative_paths = existing_relative_paths.clone();
            async move {
                if cancellation.is_cancelled() {
                    return Err(ErrorKind::OtherError("download canceled".to_string()).into());
                }
                let project = projects
                    .get(&manifest_file.project_id)
                    .ok_or_else(|| {
                        ErrorKind::OtherError(format!(
                            "CurseForge project metadata is missing for {}",
                            manifest_file.project_id
                        ))
                    })?;
                let project_type = project_type_for_class(project.class_id);
                let folder = modpack_target_folder(project_type)?;
                let meta = file_meta.get(&manifest_file.file_id).ok_or_else(|| {
                    ErrorKind::OtherError(format!(
                        "CurseForge file metadata is missing for {}",
                        manifest_file.file_id
                    ))
                })?;
                let file_name = crate::util::io::sanitize_download_file_name(&meta.file_name)?;
                let original_relative_path = format!("{folder}/{file_name}");
                let localized_candidate = (project_type != "mod" && project_type != "world")
                    .then(|| {
                        chinese_file_title_for_curseforge_slug(&project.slug)
                            .and_then(|title| {
                                localized_content_file_name(&file_name, &title)
                            })
                            .map(|file_name| format!("{folder}/{file_name}"))
                    })
                    .flatten();
                let pre_resolved_relative_path = if existing_relative_paths
                    .contains(&original_relative_path)
                {
                    original_relative_path
                } else if let Some(localized) = localized_candidate {
                    if existing_relative_paths.contains(&localized) || prefer_localized_names {
                        localized
                    } else {
                        original_relative_path
                    }
                } else {
                    original_relative_path
                };

                let mut active_download =
                    ActiveCurseForgeDownload::start(active_downloads.clone());
                let (installed_result, failed_result, failure_reason) =
                    retry_modpack_file_install(
                        &request.instance_id,
                        &manifest_file,
                        project_type,
                        &minecraft_version,
                        loader_type_value,
                        if request.allow_target_change {
                            crate::state::instances::ManualDownloadOperationKind::PackUpdate
                        } else {
                            crate::state::instances::ManualDownloadOperationKind::PackInstall
                        },
                        download_metrics.as_deref(),
                        file_meta
                            .get(&manifest_file.file_id)
                            .map(|file| file.file_name.as_str())
                        .unwrap_or("<unknown>"),
                        cancellation.clone(),
                        Some(verification_tx.clone()),
                        Some(pre_resolved_relative_path),
                        Some((project.clone(), meta.clone())),
                    )
                    .await;
                active_download.finish();

                if cancellation.is_cancelled() {
                    return Err(ErrorKind::OtherError("download canceled".to_string()).into());
                }

                let Some(item_result) = installed_result else {
                    let mut failed_result = failed_result.unwrap_or_default();
                    let file_name = file_meta
                        .get(&manifest_file.file_id)
                        .map(|file| file.file_name.clone())
                        .unwrap_or_else(|| {
                            format!(
                                "project-{}-file-{}",
                                manifest_file.project_id,
                                manifest_file.file_id
                            )
                        });
                    let manual_download =
                        failed_result.manual_downloads.first().cloned();
                    let event = if let Some(manual_download) =
                        manual_download.as_ref()
                    {
                        InstallJobEventKind::ContentFileSkipped {
                            path: manual_download.file_name.clone(),
                            reason: "CurseForge requires manual download"
                                .to_string(),
                            project_id: Some(
                                manifest_file.project_id.to_string(),
                            ),
                            version_id: Some(manifest_file.file_id.to_string()),
                            manual_url: manual_download.website_url.clone(),
                        }
                    } else {
                        let reason = format!(
                            "Failed after {MODPACK_FILE_INSTALL_ATTEMPTS} attempts: {failure_reason}"
                        );
                        failed_result.failed_downloads.push(
                            CurseForgeFailedDownload {
                                project_id: manifest_file.project_id,
                                file_id: manifest_file.file_id,
                                file_name: file_name.clone(),
                                reason: reason.clone(),
                            },
                        );
                        InstallJobEventKind::ContentFileFailed {
                            path: file_name,
                            reason,
                            project_id: Some(
                                manifest_file.project_id.to_string(),
                            ),
                            version_id: Some(manifest_file.file_id.to_string()),
                        }
                    };
                    {
                        let mut content =
                            content.lock().expect("content mutex");
                        merge_install_result(&mut content, failed_result);
                    }
                    report_modpack_progress(
                        loading_bar.as_deref(),
                        reporter.as_ref(),
                        pack_details,
                        &files_done,
                        &bytes_done,
                        &active_downloads,
                        total_files as u64,
                        content_total_bytes,
                        0,
                        event,
                    )
                    .await?;
                    return Ok(());
                };
                if project_type == "world" {
                    report_modpack_progress(loading_bar.as_deref(), reporter.as_ref(), pack_details.clone(), &files_done, &bytes_done, &active_downloads, total_files as u64, content_total_bytes, meta.file_length, InstallJobEventKind::ContentFileCompleted {
                        path: item_result.installed[0].relative_path.clone(), bytes: meta.file_length,
                    }).await?;
                }
                {
                    let mut content = content.lock().expect("content mutex");
                    merge_install_result(&mut content, item_result);
                }
                Ok(())
            }
        },
    )
    .await;

    if let Err(error) = &download_result {
        if let Some(reporter) = &reporter {
            reporter.fail_pipeline(error);
        }
        cancellation.cancel();
    }

    drop(verification_tx);
    let verification_result = verification_worker.await.map_err(|e| {
        ErrorKind::OtherError(format!("verification worker failed: {e}"))
    })?;
    drop(database_tx);
    let database_result = database_worker.await.map_err(|e| {
        ErrorKind::OtherError(format!("database worker failed: {e}"))
    })?;
    if let Some(error) = reporter
        .as_ref()
        .and_then(InstallProgressReporter::pipeline_failure)
    {
        return Err(error);
    }
    verification_result?;
    database_result?;
    download_result?;

    // Every selected file has now passed verification and database
    // registration. Publish an authoritative item-count completion snapshot
    // so a final byte-progress sample cannot leave CurseForge installs at
    // 99% in the frontend.
    if let Some(reporter) = reporter.as_ref() {
        reporter
            .update(
                InstallPhaseId::DownloadingContent,
                Some(InstallProgress {
                    current: total_files as u64,
                    total: total_files as u64,
                    secondary: Some(InstallProgressSecondary {
                        current: content_total_bytes,
                        total: content_total_bytes,
                    }),
                }),
                pack_details.clone(),
            )
            .await?;
    }

    if let (Some(reporter), Some(download_metrics)) =
        (reporter.as_ref(), download_metrics.as_ref())
    {
        download_metrics.finish(reporter).await?;
    }

    let content = Arc::try_unwrap(content)
        .map_err(|_| {
            ErrorKind::OtherError(
                "CurseForge install state was still shared after completion"
                    .to_string(),
            )
        })?
        .into_inner()
        .map_err(|_| {
            ErrorKind::OtherError(
                "CurseForge install state mutex was poisoned".to_string(),
            )
        })?;
    if request.allow_target_change {
        for installed in &content.installed {
            if disabled_pack_projects
                .contains(&installed.project_id.to_string())
                && !installed.relative_path.ends_with(".disabled")
            {
                crate::state::instances::commands::toggle_disable_project(
                    &request.instance_id,
                    &installed.relative_path,
                    Some(false),
                    &state,
                )
                .await?;
            }
        }
    }

    let instance_path =
        crate::api::instance::get_full_path(&request.instance_id).await?;
    if let Some(reporter) = reporter.as_ref() {
        reporter
            .update(
                InstallPhaseId::ExtractingOverrides,
                None,
                pack_details.clone(),
            )
            .await?;
    }
    let update_ready = content.manual_downloads.is_empty()
        && content.failed_downloads.is_empty();
    let should_commit = content.manual_downloads.is_empty()
        && (!request.allow_target_change || update_ready);
    let override_cancellation = reporter
        .as_ref()
        .map(InstallProgressReporter::cancellation_token)
        .unwrap_or_default();
    let materialized_overrides = if should_commit {
        Some(
			crate::api::pack::archive_util::run_cancellable_blocking_instance_write(
                request.instance_id.clone(),
                override_cancellation.clone(),
                move |cancellation| {
                    materialize_modpack_overrides(
                        &pack_path,
                        &instance_path,
                        Some(cancellation),
                    )
                },
            )
            .await?,
        )
    } else {
        None
    };
    let overrides_written = materialized_overrides
        .as_ref()
        .map_or(0, |(files_written, _)| *files_written);
    let post_override_result: crate::Result<()> = async {
		if let Some(reporter) = reporter.as_ref() {
			reporter
				.update(InstallPhaseId::Finalizing, None, pack_details)
				.await?;
		}
        if should_commit && !request.allow_target_change {
            crate::api::instance::edit(
            &request.instance_id,
            EditInstance {
                name: (!request.allow_target_change)
                    .then_some(project.name.clone()),
                icon_path: (!request.allow_target_change).then(|| {
                    cached_icon_path
                        .as_ref()
                        .map(|path| path.to_string_lossy().to_string())
                }),
                link: Some(InstanceLink::CurseForgeModpack {
                    project_id: request.project_id.to_string(),
                    version_id: request.file_id.to_string(),
                }),
                content_set_patch: Some(crate::state::AppliedContentSetPatch {
                    source_kind: Some(ContentSourceKind::CurseForge),
                    game_version: Some(manifest.minecraft.version.clone()),
                    protocol_version: Some(None),
                    loader: Some(target.loader),
                    loader_version: Some(target.loader_version.clone()),
                }),
                ..EditInstance::default()
            },
            )
            .await?;
            let content_set = crate::state::instances::adapters::sqlite::content_rows::get_applied_content_set(
			&request.instance_id,
			&state.pool,
		)
		.await?
		.ok_or_else(|| {
			ErrorKind::InputError(
				"Instance has no applied content set".to_string(),
			)
			})?;
            crate::state::sync_content_files(&request.instance_id, &state)
                .await?;
            match get_modpack_expected_members(
                request.project_id,
                request.file_id,
            )
            .await
            {
                Ok(expected) => {
                    crate::state::instances::commands::reconcile_curseforge_members(
					&request.instance_id,
					&content_set.id,
					&expected,
					&state,
				)
					.await?;
                }
                Err(error) => tracing::warn!(
                    "Unable to persist the complete CurseForge pack manifest: {error}"
                ),
            }
        }
        Ok(())
    }
    .await;
    if let Some((_, replacements)) = materialized_overrides {
        match post_override_result {
            Ok(()) => {
                settle_materialized_modpack_overrides(
                    request.instance_id.clone(),
                    override_cancellation,
                    replacements,
                    true,
                )
                .await?;
            }
            Err(error) => {
                if let Err(rollback_error) =
                    settle_materialized_modpack_overrides(
                        request.instance_id.clone(),
                        override_cancellation,
                        replacements,
                        false,
                    )
                    .await
                {
                    return Err(ErrorKind::OtherError(format!(
                        "{error}; failed to restore CurseForge overrides: {rollback_error}"
                    ))
                    .into());
                }
                return Err(error);
            }
        }
    } else {
        post_override_result?;
    }
    Ok(CurseForgeModpackInstallResult {
        content,
        overrides_written,
        minecraft_version: manifest.minecraft.version,
        loader,
    })
}

fn select_modpack_manifest_files(
    manifest: &CurseForgeModpackManifest,
    pack_members: &[crate::state::instances::PackMember],
    install_optional: bool,
) -> (Vec<CurseForgeManifestFile>, HashSet<String>) {
    let preserved_pack_projects = pack_members
        .iter()
        .filter(|member| {
            matches!(
                member.override_kind,
                crate::state::instances::PackMemberOverrideKind::Version
                    | crate::state::instances::PackMemberOverrideKind::Removed
            )
        })
        .filter_map(|member| member.provider_project_id.clone())
        .collect::<HashSet<_>>();
    let disabled_pack_projects = pack_members
        .iter()
        .filter(|member| {
            member.override_kind
                == crate::state::instances::PackMemberOverrideKind::Disabled
        })
        .filter_map(|member| member.provider_project_id.clone())
        .collect::<HashSet<_>>();
    let installed_pack_releases = pack_members
        .iter()
        .filter(|member| {
            member.materialization_state
                == crate::state::instances::PackMemberMaterializationState::Present
        })
        .filter_map(|member| {
            Some((
                member.provider_project_id.clone()?,
                member.provider_release_id.clone()?,
            ))
        })
        .collect::<HashSet<_>>();
    let selected_files = manifest
        .files
        .iter()
        .filter(|file| file.required || install_optional)
        .filter(|file| {
            !preserved_pack_projects.contains(&file.project_id.to_string())
        })
        .filter(|file| {
            !installed_pack_releases.contains(&(
                file.project_id.to_string(),
                file.file_id.to_string(),
            ))
        })
        .cloned()
        .collect::<Vec<_>>();
    (selected_files, disabled_pack_projects)
}

async fn install_preloaded_modpack_file(
    request: &CurseForgeInstallRequest,
    project: &CurseForgeProject,
    file: &CurseForgeFile,
    download_metrics: Option<&CurseForgeDownloadMetrics>,
) -> crate::Result<CurseForgeInstallResult> {
    if project.id != request.project_id
        || file.mod_id != request.project_id
        || file.id != request.file_id
    {
        return Err(ErrorKind::InputError(
            "Preloaded CurseForge metadata does not match the manifest file"
                .to_string(),
        )
        .into());
    }
    let project_type = managed_project_type(&request.project_type)?;
    let download_url = resolve_curseforge_download_url(
        request.project_id,
        request.file_id,
        project,
        file,
    )
    .await?;
    let Some(download_url) = download_url else {
        let target_folder =
            content_target_folder(project_type, request.world_name.as_deref())?;
        let manual_download = manual_download_from_file(
            request.project_id,
            request.file_id,
            file,
            project,
            project_type.get_name(),
            target_folder,
            request.ownership_kind,
            request.manual_operation_kind,
        );
        persist_manual_download(&request.instance_id, &manual_download).await?;
        return Ok(CurseForgeInstallResult {
            manual_downloads: vec![manual_download],
            ..Default::default()
        });
    };
    validate_file_name(&file.file_name)?;
    let downloaded = download_installed_file(DownloadInstalledFileRequest {
        instance_id: &request.instance_id,
        url: &download_url,
        file,
        project_type,
        world_name: request.world_name.as_deref(),
        project_id: request.project_id,
        file_id: request.file_id,
        project_slug: &project.slug,
        ownership_kind: request.ownership_kind,
        download_metrics,
        defer_persistence: request.defer_persistence,
        verification_tx: request.verification_tx.as_ref(),
        pre_resolved_relative_path: request
            .pre_resolved_relative_path
            .as_deref(),
        expected_file_name: request.expected_file_name.as_deref(),
    })
    .await?;
    Ok(CurseForgeInstallResult {
        installed: vec![CurseForgeInstalledFile {
            project_id: request.project_id,
            file_id: request.file_id,
            relative_path: downloaded.relative_path,
            dependency: false,
        }],
        ..Default::default()
    })
}

async fn retry_modpack_file_install(
    instance_id: &str,
    manifest_file: &CurseForgeManifestFile,
    project_type: &str,
    minecraft_version: &str,
    loader_type_value: Option<u32>,
    manual_operation_kind: crate::state::instances::ManualDownloadOperationKind,
    download_metrics: Option<&CurseForgeDownloadMetrics>,
    expected_file_name: &str,
    cancellation: CancellationToken,
    verification_tx: Option<mpsc::Sender<CurseForgeVerificationTask>>,
    pre_resolved_relative_path: Option<String>,
    preloaded: Option<(CurseForgeProject, CurseForgeFile)>,
) -> (
    Option<CurseForgeInstallResult>,
    Option<CurseForgeInstallResult>,
    String,
) {
    let mut installed_result = None;
    let mut failed_result = None;
    let mut failure_reason = "no file was installed".to_string();
    let mut attempt_failures = Vec::new();
    for attempt in 1..=MODPACK_FILE_INSTALL_ATTEMPTS {
        if cancellation.is_cancelled() {
            return (None, None, "download canceled".to_string());
        }
        let install_request = CurseForgeInstallRequest {
            instance_id: instance_id.to_string(),
            project_id: manifest_file.project_id,
            file_id: manifest_file.file_id,
            project_type: project_type.to_string(),
            ownership_kind:
                crate::state::instances::ContentOwnershipKind::PackManaged,
            manual_operation_kind,
            game_version: Some(minecraft_version.to_string()),
            mod_loader_type: loader_type_value,
            world_name: None,
            install_dependencies: false,
            excluded_dependency_project_ids: Vec::new(),
            force_dependency_project_ids: Vec::new(),
            dependency_plan_id: None,
            defer_persistence: verification_tx.is_some(),
            verification_tx: verification_tx.clone(),
            pre_resolved_relative_path: pre_resolved_relative_path.clone(),
            expected_file_name: Some(expected_file_name.to_string()),
        };
        let result = match preloaded.as_ref() {
            Some((project, file)) if project_type == "world" => {
                install_modpack_world_file(
                    instance_id,
                    project,
                    file,
                    download_metrics
                        .and_then(|metrics| metrics.reporter.as_ref()),
                    manual_operation_kind,
                )
                .await
            }
            Some((project, file)) => {
                install_preloaded_modpack_file(
                    &install_request,
                    project,
                    file,
                    download_metrics,
                )
                .await
            }
            None => {
                install_file_with_metrics(install_request, download_metrics)
                    .await
            }
        };
        match result {
            Ok(item_result) if !item_result.installed.is_empty() => {
                let installed_path = &item_result.installed[0].relative_path;
                let expected_name =
                    crate::util::io::sanitize_download_file_name(
                        expected_file_name,
                    )
                    .ok();
                if project_type == ProjectType::Mod.get_name()
                    && Path::new(installed_path).file_name()
                        != expected_name.as_deref().map(std::ffi::OsStr::new)
                {
                    failure_reason = format!(
                        "CurseForge install context mismatch: project_id={} file_id={} expected_file={} installed_path={}",
                        manifest_file.project_id,
                        manifest_file.file_id,
                        expected_file_name,
                        installed_path,
                    );
                    attempt_failures
                        .push(format!("attempt {attempt}: {failure_reason}"));
                    continue;
                }
                installed_result = Some(item_result);
                break;
            }
            Ok(item_result) => {
                failure_reason = item_result
                    .manual_downloads
                    .first()
                    .map(|file| {
                        format!("{} requires manual download", file.file_name)
                    })
                    .unwrap_or_else(|| "no file was installed".to_string());
                let manual_download_required =
                    !item_result.manual_downloads.is_empty();
                failed_result = Some(item_result);
                if manual_download_required {
                    break;
                }
            }
            Err(error) => failure_reason = install_error_chain(&error),
        }
        attempt_failures.push(format!("attempt {attempt}: {failure_reason}"));
        tracing::warn!(
            project_id = manifest_file.project_id,
            file_id = manifest_file.file_id,
            attempt,
            max_attempts = MODPACK_FILE_INSTALL_ATTEMPTS,
            expected_file = expected_file_name,
            reason = %attempt_failures.last().expect("attempt failure recorded"),
            "Failed to install required CurseForge file"
        );
        if attempt < MODPACK_FILE_INSTALL_ATTEMPTS {
            tokio::select! {
                _ = cancellation.cancelled() => return (None, None, "download canceled".to_string()),
                _ = tokio::time::sleep(Duration::from_millis(250 * attempt as u64)) => {}
            }
        }
    }
    if !attempt_failures.is_empty() {
        failure_reason = attempt_failures.join("\n");
    }
    (installed_result, failed_result, failure_reason)
}

fn install_error_chain(error: &crate::Error) -> String {
    let mut chain = error.to_string();
    let mut source = std::error::Error::source(error);
    while let Some(cause) = source {
        chain.push_str("\nCaused by: ");
        chain.push_str(&cause.to_string());
        source = cause.source();
    }
    chain
}

/// Installs a CurseForge modpack from a local archive on disk (a zip with a
/// `manifest.json`), downloading the listed files through the CurseForge API
/// and extracting the overrides folder. Unlike [`install_modpack_with_reporter`]
/// this needs no project/file id — undownloadable files land on the manual
/// download list exactly like API-driven installs.
pub async fn install_modpack_from_local_archive_with_reporter(
    instance_id: String,
    archive_path: std::path::PathBuf,
    _base_folder: String,
    source_filename: Option<String>,
    install_optional: bool,
    reporter: InstallProgressReporter,
    completion_policy: crate::launcher::InstanceCompletionPolicy,
) -> crate::Result<CurseForgeModpackInstallResult> {
    let manifest_archive_path = archive_path.clone();
    let manifest = tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&manifest_archive_path)?;
        let mut archive =
            zip::ZipArchive::new(file).map_err(modpack_zip_error)?;
        read_modpack_manifest(&mut archive)
    })
    .await??;

    let target = modpack_target(&manifest)?;
    let loader = (target.loader != ModLoader::Vanilla)
        .then(|| target.loader.as_str().to_string());
    let pack_name = manifest
        .name
        .clone()
        .filter(|name| !name.trim().is_empty())
        .or_else(|| {
            source_filename.as_ref().map(|name| {
                std::path::Path::new(name)
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            })
        })
        .unwrap_or_else(|| "CurseForge Modpack".to_string());
    let pack_details = InstallPhaseDetails::Modpack {
        project_id: None,
        version_id: None,
        title: Some(pack_name.clone()),
    };
    reporter
        .update(InstallPhaseId::ResolvingPack, None, pack_details.clone())
        .await?;

    let loader_version = if target.loader != ModLoader::Vanilla {
        crate::launcher::get_loader_version_from_profile(
            &manifest.minecraft.version,
            target.loader,
            target.loader_version.as_deref(),
        )
        .await?
    } else {
        None
    };

    crate::api::instance::edit(
        &instance_id,
        EditInstance {
            install_stage: Some(
                crate::state::InstanceInstallStage::PackInstalling,
            ),
            name: Some(pack_name.clone()),
            link: Some(InstanceLink::ImportedModpack {
                project_id: None,
                version_id: None,
                name: Some(pack_name.clone()),
                version_number: manifest.version.clone(),
                filename: source_filename,
            }),
            content_set_patch: Some(crate::state::AppliedContentSetPatch {
                source_kind: Some(ContentSourceKind::CurseForge),
                game_version: Some(manifest.minecraft.version.clone()),
                protocol_version: Some(None),
                loader: Some(target.loader),
                loader_version: Some(loader_version.map(|version| version.id)),
            }),
            ..EditInstance::default()
        },
    )
    .await?;

    let minecraft_install = (completion_policy
        == crate::launcher::InstanceCompletionPolicy::DeferToInstallJob)
        .then(|| {
            crate::api::pack::parallel_minecraft_install::ParallelMinecraftInstall::start(
                instance_id.clone(),
                reporter.clone(),
            )
        });

    let content = install_local_manifest_files(
        &instance_id,
        manifest.files.clone(),
        install_optional,
        &manifest.minecraft.version,
        loader.as_deref(),
        pack_details.clone(),
        &reporter,
    )
    .await?;

    if !content.manual_downloads.is_empty() {
        if let Some(minecraft_install) = minecraft_install {
            minecraft_install.abort().await;
        }
        return Ok(CurseForgeModpackInstallResult {
            content,
            overrides_written: 0,
            minecraft_version: manifest.minecraft.version,
            loader,
        });
    }

    reporter
        .update(
            InstallPhaseId::ExtractingOverrides,
            None,
            pack_details.clone(),
        )
        .await?;
    let instance_path =
        crate::api::instance::get_full_path(&instance_id).await?;
    let overrides_archive_path = archive_path.clone();
    let override_cancellation = reporter.cancellation_token();
    let (overrides_written, override_replacements) =
		crate::api::pack::archive_util::run_cancellable_blocking_instance_write(
            instance_id.clone(),
            override_cancellation.clone(),
            move |cancellation| {
                materialize_modpack_overrides(
                    &overrides_archive_path,
                    &instance_path,
                    Some(cancellation),
                )
            },
        )
        .await?;

    let post_override_result: crate::Result<()> = async {
        if let Some(minecraft_install) = minecraft_install {
            minecraft_install.join().await?;
        } else {
            crate::launcher::install_minecraft_for_instance_id_with_reporter(
                &instance_id,
                false,
                Some(reporter.clone()),
                completion_policy,
            )
            .await?;
        }
        reporter.clear_context().await?;
        Ok(())
    }
    .await;
    match post_override_result {
        Ok(()) => {
            settle_materialized_modpack_overrides(
                instance_id.clone(),
                override_cancellation,
                override_replacements,
                true,
            )
            .await?;
        }
        Err(error) => {
            if let Err(rollback_error) = settle_materialized_modpack_overrides(
                instance_id.clone(),
                override_cancellation,
                override_replacements,
                false,
            )
            .await
            {
                return Err(ErrorKind::OtherError(format!(
                    "{error}; failed to restore CurseForge ZIP overrides: {rollback_error}"
                ))
                .into());
            }
            return Err(error);
        }
    }

    Ok(CurseForgeModpackInstallResult {
        content,
        overrides_written,
        minecraft_version: manifest.minecraft.version,
        loader,
    })
}

/// Downloads and installs the files listed in a local CurseForge manifest.
/// Mirrors the file loop of [`install_modpack_with_reporter`], simplified to
/// the job-reporter path used by local imports. Also used by the MCBBS
/// installer for its CurseForge-style `files` array.
pub(crate) async fn install_local_manifest_files(
    instance_id: &str,
    manifest_files: Vec<CurseForgeManifestFile>,
    install_optional: bool,
    minecraft_version: &str,
    loader: Option<&str>,
    pack_details: InstallPhaseDetails,
    reporter: &InstallProgressReporter,
) -> crate::Result<CurseForgeInstallResult> {
    let reporter = &reporter.clone().with_pipeline_cancellation();
    let state = State::get().await?;
    let content_set = crate::state::instances::adapters::sqlite::content_rows::get_applied_content_set(
		instance_id,
		&state.pool,
	)
	.await?
	.ok_or_else(|| {
		ErrorKind::InputError(
			"Instance has no applied content set".to_string(),
		)
	})?;
    let installed_releases = crate::state::instances::adapters::sqlite::content_rows::get_pack_members(
		&content_set.id,
		&state.pool,
	)
	.await?
	.into_iter()
	.filter(|member| {
		member.materialization_state
			== crate::state::instances::PackMemberMaterializationState::Present
	})
	.filter_map(|member| {
		Some((member.provider_project_id?, member.provider_release_id?))
	})
	.collect::<HashSet<_>>();
    let selected_files = manifest_files
        .into_iter()
        .filter(|file| file.required || install_optional)
        .filter(|file| {
            !installed_releases.contains(&(
                file.project_id.to_string(),
                file.file_id.to_string(),
            ))
        })
        .collect::<Vec<_>>();
    let loader_type_value = loader.and_then(loader_type);
    let project_ids = selected_files
        .iter()
        .map(|file| file.project_id)
        .collect::<Vec<_>>();
    let projects = get_modpack_projects(project_ids).await?;

    let total_files = selected_files.len().max(1);
    let file_ids = selected_files
        .iter()
        .map(|file| file.file_id)
        .collect::<Vec<_>>();
    let file_meta = get_modpack_files(file_ids).await?;
    let existing_relative_paths = Arc::new(
        crate::state::instances::adapters::sqlite::content_rows::get_instance_files(
            instance_id,
            &state.pool,
        )
        .await?
        .into_iter()
        .map(|file| file.relative_path)
        .collect::<HashSet<_>>(),
    );
    let prefer_localized_names =
        Settings::get(&state.pool).await?.resource_chinese_filenames;
    let content_total_bytes = selected_files
        .iter()
        .map(|file| {
            file_meta
                .get(&file.file_id)
                .map(|meta| meta.file_length)
                .unwrap_or(0)
        })
        .sum::<u64>();
    reporter
        .update_with_events(
            InstallPhaseId::DownloadingContent,
            Some(InstallProgress {
                current: 0,
                total: total_files as u64,
                secondary: Some(InstallProgressSecondary {
                    current: 0,
                    total: content_total_bytes,
                }),
            }),
            pack_details.clone(),
            vec![InstallJobEventKind::ContentDownloadStarted {
                files: total_files as u64,
                bytes: Some(content_total_bytes),
            }],
        )
        .await?;

    tracing::info!(
        selected_manifest_files = selected_files.len(),
        "Resolved local CurseForge modpack manifest files"
    );
    let content = Arc::new(Mutex::new(CurseForgeInstallResult::default()));
    let download_metrics =
        Arc::new(CurseForgeDownloadMetrics::with_reporter(reporter.clone()));
    let projects = Arc::new(projects);
    let file_meta = Arc::new(file_meta);
    let files_done = Arc::new(AtomicU64::new(0));
    let bytes_done = Arc::new(AtomicU64::new(0));
    let active_downloads = Arc::new(AtomicU64::new(0));
    let cancellation = reporter.cancellation_token();
    let (verification_tx, verification_rx) =
        mpsc::channel::<CurseForgeVerificationTask>(
            MODPACK_VERIFICATION_QUEUE_CAPACITY,
        );
    let verification_context = CurseForgeVerificationContext {
        reporter: Some(reporter.clone()),
        loading_bar: None,
        details: pack_details.clone(),
        files_done: files_done.clone(),
        bytes_done: bytes_done.clone(),
        active_downloads: active_downloads.clone(),
        total_files: total_files as u64,
        total_bytes: content_total_bytes,
        cancellation: cancellation.clone(),
    };
    let (database_tx, database_rx) = mpsc::channel::<CurseForgeDatabaseTask>(
        MODPACK_DATABASE_QUEUE_CAPACITY,
    );
    let database_worker = spawn_curseforge_database_worker(
        database_rx,
        verification_context.clone(),
    );
    let verification_worker = spawn_curseforge_verification_worker(
        verification_rx,
        verification_context,
        database_tx.clone(),
    );

    let download_result = loading_try_for_each_concurrent(
        stream::iter(selected_files.into_iter().map(Ok::<_, crate::Error>)),
        Some(state.download_concurrency()),
        None,
        1.0,
        total_files,
        None,
        |manifest_file| {
            let content = content.clone();
            let projects = projects.clone();
            let file_meta = file_meta.clone();
            let files_done = files_done.clone();
            let bytes_done = bytes_done.clone();
            let active_downloads = active_downloads.clone();
            let reporter = reporter.clone();
            let download_metrics = download_metrics.clone();
            let pack_details = pack_details.clone();
            let instance_id = instance_id.to_string();
            let minecraft_version = minecraft_version.to_string();
            let verification_tx = verification_tx.clone();
            let existing_relative_paths = existing_relative_paths.clone();
            let cancellation = cancellation.clone();
            async move {
                if cancellation.is_cancelled() {
                    return Err(
                        ErrorKind::OtherError("download canceled".to_string())
                            .into(),
                    );
                }
                let expected_file_name = file_meta
                    .get(&manifest_file.file_id)
                    .map(|file| file.file_name.as_str())
                    .unwrap_or("<unknown>");
                let project = projects
                    .get(&manifest_file.project_id)
                    .ok_or_else(|| {
                        ErrorKind::OtherError(format!(
                            "CurseForge project metadata is missing for {}",
                            manifest_file.project_id
                        ))
                    })?;
                let project_type = project_type_for_class(project.class_id);
                let folder = modpack_target_folder(project_type)?;
                let meta = file_meta.get(&manifest_file.file_id).ok_or_else(|| {
                    ErrorKind::OtherError(format!(
                        "CurseForge file metadata is missing for {}",
                        manifest_file.file_id
                    ))
                })?;
                let file_name = crate::util::io::sanitize_download_file_name(&meta.file_name)?;
                let original_relative_path = format!("{folder}/{file_name}");
                let localized_candidate = (project_type != "mod" && project_type != "world")
                    .then(|| {
                        chinese_file_title_for_curseforge_slug(&project.slug)
                            .and_then(|title| {
                                localized_content_file_name(&file_name, &title)
                            })
                            .map(|file_name| format!("{folder}/{file_name}"))
                    })
                    .flatten();
                let pre_resolved_relative_path = if existing_relative_paths
                    .contains(&original_relative_path)
                {
                    original_relative_path
                } else if let Some(localized) = localized_candidate {
                    if existing_relative_paths.contains(&localized) || prefer_localized_names {
                        localized
                    } else {
                        original_relative_path
                    }
                } else {
                    original_relative_path
                };

                let mut active_download =
                    ActiveCurseForgeDownload::start(active_downloads.clone());
                let (installed_result, failed_result, failure_reason) =
                    retry_modpack_file_install(
                        &instance_id,
                        &manifest_file,
                        project_type,
                        &minecraft_version,
                        loader_type_value,
                        crate::state::instances::ManualDownloadOperationKind::PackUpdate,
                        Some(&download_metrics),
                        expected_file_name,
                        cancellation.clone(),
                        Some(verification_tx.clone()),
                        Some(pre_resolved_relative_path),
                        Some((project.clone(), meta.clone())),
                    )
                    .await;
                active_download.finish();

                if cancellation.is_cancelled() {
                    return Err(
                        ErrorKind::OtherError("download canceled".to_string())
                            .into(),
                    );
                }

                let Some(item_result) = installed_result else {
                    let mut failed_result = failed_result.unwrap_or_default();
                    let file_name = file_meta
                        .get(&manifest_file.file_id)
                        .map(|file| file.file_name.clone())
                        .unwrap_or_else(|| {
                            format!(
                                "project-{}-file-{}",
                                manifest_file.project_id,
                                manifest_file.file_id
                            )
                        });
                    let manual_download =
                        failed_result.manual_downloads.first().cloned();
                    let event = if let Some(manual_download) =
                        manual_download.as_ref()
                    {
                        InstallJobEventKind::ContentFileSkipped {
                            path: manual_download.file_name.clone(),
                            reason: "CurseForge requires manual download"
                                .to_string(),
                            project_id: Some(
                                manifest_file.project_id.to_string(),
                            ),
                            version_id: Some(manifest_file.file_id.to_string()),
                            manual_url: manual_download.website_url.clone(),
                        }
                    } else {
                        let reason = format!(
                            "Failed after {MODPACK_FILE_INSTALL_ATTEMPTS} attempts: {failure_reason}"
                        );
                        failed_result.failed_downloads.push(
                            CurseForgeFailedDownload {
                                project_id: manifest_file.project_id,
                                file_id: manifest_file.file_id,
                                file_name: file_name.clone(),
                                reason: reason.clone(),
                            },
                        );
                        InstallJobEventKind::ContentFileFailed {
                            path: file_name,
                            reason,
                            project_id: Some(
                                manifest_file.project_id.to_string(),
                            ),
                            version_id: Some(manifest_file.file_id.to_string()),
                        }
                    };
                    {
                        let mut content =
                            content.lock().expect("content mutex");
                        merge_install_result(&mut content, failed_result);
                    }
                    report_modpack_progress(
                        None,
                        Some(&reporter),
                        pack_details,
                        &files_done,
                        &bytes_done,
                        &active_downloads,
                        total_files as u64,
                        content_total_bytes,
                        0,
                        event,
                    )
                    .await?;
                    return Ok(());
                };
                if project_type == "world" {
                    report_modpack_progress(None, Some(&reporter), pack_details.clone(), &files_done, &bytes_done, &active_downloads, total_files as u64, content_total_bytes, meta.file_length, InstallJobEventKind::ContentFileCompleted {
                        path: item_result.installed[0].relative_path.clone(), bytes: meta.file_length,
                    }).await?;
                }
                {
                    let mut content = content.lock().expect("content mutex");
                    merge_install_result(&mut content, item_result);
                }
                Ok(())
            }
        },
    )
    .await;

    if let Err(error) = &download_result {
        reporter.fail_pipeline(error);
        cancellation.cancel();
    }

    drop(verification_tx);
    let verification_result = verification_worker.await.map_err(|error| {
        ErrorKind::OtherError(format!(
            "local CurseForge verification worker failed: {error}"
        ))
    })?;
    drop(database_tx);
    let database_result = database_worker.await.map_err(|error| {
        ErrorKind::OtherError(format!(
            "local CurseForge database worker failed: {error}"
        ))
    })?;
    if let Some(error) = reporter.pipeline_failure() {
        return Err(error);
    }
    verification_result?;
    database_result?;
    download_result?;

    reporter
        .update(
            InstallPhaseId::DownloadingContent,
            Some(InstallProgress {
                current: total_files as u64,
                total: total_files as u64,
                secondary: Some(InstallProgressSecondary {
                    current: content_total_bytes,
                    total: content_total_bytes,
                }),
            }),
            pack_details.clone(),
        )
        .await?;

    download_metrics.finish(reporter).await?;

    Arc::try_unwrap(content)
        .map_err(|_| {
            ErrorKind::OtherError(
                "CurseForge install state was still shared after completion"
                    .to_string(),
            )
        })?
        .into_inner()
        .map_err(|_| {
            ErrorKind::OtherError(
                "CurseForge install state mutex was poisoned".to_string(),
            )
        })
        .map_err(Into::into)
}

pub async fn update_managed_modpack(
    instance_id: &str,
    file_id: u32,
) -> crate::Result<CurseForgeModpackInstallResult> {
    update_managed_modpack_with_reporter(instance_id, file_id, None).await
}

pub(crate) async fn update_managed_modpack_with_reporter(
    instance_id: &str,
    file_id: u32,
    reporter: Option<InstallProgressReporter>,
) -> crate::Result<CurseForgeModpackInstallResult> {
    let state = State::get().await?;
    let metadata = crate::state::instances::commands::get_instance_metadata(
        instance_id,
        &state.pool,
    )
    .await?
    .ok_or_else(|| ErrorKind::InputError("Unknown instance".to_string()))?;
    let project_id = match &metadata.link {
        InstanceLink::CurseForgeModpack { project_id, .. } => {
            project_id.parse::<u32>().map_err(|_| {
                ErrorKind::InputError(
                    "Linked CurseForge project ID is invalid".to_string(),
                )
            })?
        }
        _ => {
            return Err(ErrorKind::InputError(format!(
                "Instance {instance_id} is not a managed CurseForge pack, or has been disconnected."
            ))
            .into());
        }
    };
    let members = crate::state::instances::adapters::sqlite::content_rows::get_pack_members(
		&metadata.applied_content_set.id,
		&state.pool,
	)
	.await?;
    if members.iter().any(|member| !member.reconciled) {
        return Err(ErrorKind::InputError(
			"CurseForge pack membership is not calibrated yet; refresh the content page while online before updating"
				.to_string(),
		)
		.into());
    }
    let expected = get_modpack_expected_members_with_reporter(
        project_id,
        file_id,
        reporter.as_ref(),
    )
    .await?;
    let pack_file = get_file(project_id, file_id).await?;
    let game_version = pack_file
        .game_versions
        .iter()
        .find(|value| looks_like_game_version(value))
        .cloned()
        .unwrap_or_else(|| metadata.applied_content_set.game_version.clone());
    let loader = pack_file
        .game_versions
        .iter()
        .find_map(|value| {
            loader_type(value).map(|_| value.to_ascii_lowercase())
        })
        .or_else(|| {
            Some(metadata.applied_content_set.loader.as_str().to_string())
        });
    let installed_releases = members
        .iter()
        .filter(|member| {
            member.materialization_state
				== crate::state::instances::PackMemberMaterializationState::Present
        })
        .filter_map(|member| {
            Some((
                member.provider_project_id.clone()?,
                member.provider_release_id.clone()?,
            ))
        })
        .collect::<HashSet<_>>();
    let manual_downloads = expected
        .members
        .iter()
        .filter(|member| member.required)
        .filter(|member| {
            !installed_releases.contains(&(
                member.project_id.to_string(),
                member.file_id.to_string(),
            ))
        })
        .filter_map(|member| member.manual_download.clone())
        .collect::<Vec<_>>();
    if !manual_downloads.is_empty() {
        for download in &manual_downloads {
            persist_manual_download(instance_id, download).await?;
        }
        if let Some(reporter) = &reporter {
            let events = manual_downloads
                .iter()
                .map(|download| InstallJobEventKind::ContentFileSkipped {
                    path: download.file_name.clone(),
                    reason: "CurseForge requires manual download".to_string(),
                    project_id: Some(download.project_id.to_string()),
                    version_id: Some(download.file_id.to_string()),
                    manual_url: download.website_url.clone(),
                })
                .collect();
            reporter.record_events(events).await?;
        }
        return Ok(CurseForgeModpackInstallResult {
            content: CurseForgeInstallResult {
                manual_downloads,
                ..CurseForgeInstallResult::default()
            },
            overrides_written: 0,
            minecraft_version: game_version,
            loader,
        });
    }

    let entries = crate::state::instances::adapters::sqlite::content_rows::get_content_entries(
		&metadata.applied_content_set.id,
		&state.pool,
	)
	.await?
	.into_iter()
	.filter_map(|entry| entry.file_id.map(|file_id| (entry.id, file_id)))
	.collect::<HashMap<_, _>>();
    let files = crate::state::instances::adapters::sqlite::content_rows::get_instance_files(
		instance_id,
		&state.pool,
	)
	.await?
	.into_iter()
	.map(|file| (file.id.clone(), file.relative_path))
	.collect::<HashMap<_, _>>();
    let backup = create_curseforge_update_backup(
        &metadata, &members, &entries, &files, &state,
    )
    .await?;
    let result = match install_modpack_with_reporter(
        CurseForgeModpackInstallRequest {
            instance_id: instance_id.to_string(),
            project_id,
            file_id,
            install_optional: false,
            allow_target_change: true,
        },
        reporter,
    )
    .await
    {
        Ok(result) => result,
        Err(error) => {
            rollback_curseforge_update(
                instance_id,
                &metadata,
                &members,
                &backup,
                &[],
                &state,
            )
            .await?;
            return Err(error);
        }
    };
    if !result.content.manual_downloads.is_empty()
        || !result.content.failed_downloads.is_empty()
    {
        rollback_curseforge_update(
            instance_id,
            &metadata,
            &members,
            &backup,
            &result.content.installed,
            &state,
        )
        .await?;
        return Ok(result);
    }

    // `install_modpack_with_reporter` resolves the target manifest and writes
    // the resulting Minecraft/loader metadata before returning. Read that
    // committed content set so the reconciliation edit cannot overwrite a
    // newly selected loader version with the pre-update instance metadata.
    let updated_content_set =
        crate::state::instances::adapters::sqlite::content_rows::get_applied_content_set(
            instance_id,
            &state.pool,
        )
        .await?
        .ok_or_else(|| {
            ErrorKind::InputError(
                "Instance has no applied content set after CurseForge update"
                    .to_string(),
            )
        })?;

    let expected_keys = expected
        .members
        .iter()
        .map(|member| {
            format!(
                "curseforge:{}:{}",
                member.project_id,
                member.project_type.get_name()
            )
        })
        .collect::<HashSet<_>>();
    for member in &members {
        if expected_keys.contains(&member.member_key)
            || member.override_kind
                == crate::state::instances::PackMemberOverrideKind::Version
        {
            continue;
        }
        let Some(entry_id) = member.content_entry_id.as_ref() else {
            continue;
        };
        let Some(file_id) = entries.get(entry_id) else {
            continue;
        };
        let Some(relative_path) = files.get(file_id) else {
            continue;
        };
        crate::state::instances::commands::remove_project(
            instance_id,
            relative_path,
            &state,
        )
        .await?;
    }
    crate::state::sync_content_files(instance_id, &state).await?;
    crate::state::instances::commands::reconcile_curseforge_members(
        instance_id,
        &metadata.applied_content_set.id,
        &expected,
        &state,
    )
    .await?;
    crate::api::instance::edit(
        instance_id,
        EditInstance {
            link: Some(InstanceLink::CurseForgeModpack {
                project_id: project_id.to_string(),
                version_id: file_id.to_string(),
            }),
            content_set_patch: Some(crate::state::AppliedContentSetPatch {
                source_kind: Some(ContentSourceKind::CurseForge),
                game_version: Some(updated_content_set.game_version),
                protocol_version: Some(None),
                loader: Some(updated_content_set.loader),
                loader_version: Some(updated_content_set.loader_version),
            }),
            ..EditInstance::default()
        },
    )
    .await?;
    Ok(result)
}

struct CurseForgeUpdateBackup {
    _directory: tempfile::TempDir,
    files: Vec<(String, String, std::path::PathBuf)>,
}

async fn create_curseforge_update_backup(
    metadata: &crate::state::InstanceMetadata,
    members: &[crate::state::instances::PackMember],
    entries: &HashMap<String, String>,
    files: &HashMap<String, String>,
    state: &State,
) -> crate::Result<CurseForgeUpdateBackup> {
    crate::util::io::create_dir_all(&state.directories.caches_dir()).await?;
    let directory = tempfile::Builder::new()
        .prefix("curseforge-pack-update-")
        .tempdir_in(state.directories.caches_dir())?;
    let instance_path = state.directories.instance_game_dir(&metadata.instance);
    let mut backups = Vec::new();
    for member in members {
        let Some(entry_id) = member.content_entry_id.as_ref() else {
            continue;
        };
        let Some(file_id) = entries.get(entry_id) else {
            continue;
        };
        let Some(relative_path) = files.get(file_id) else {
            continue;
        };
        let source = instance_path.join(relative_path);
        if !source.is_file() {
            continue;
        }
        let backup_path = directory
            .path()
            .join(format!("{}.backup", uuid::Uuid::new_v4()));
        tokio::fs::copy(&source, &backup_path).await?;
        backups.push((
            member.member_key.clone(),
            relative_path.clone(),
            backup_path,
        ));
    }
    Ok(CurseForgeUpdateBackup {
        _directory: directory,
        files: backups,
    })
}

async fn rollback_curseforge_update(
    instance_id: &str,
    metadata: &crate::state::InstanceMetadata,
    old_members: &[crate::state::instances::PackMember],
    backup: &CurseForgeUpdateBackup,
    installed: &[CurseForgeInstalledFile],
    state: &State,
) -> crate::Result<()> {
    let instance_path = state.directories.instance_game_dir(&metadata.instance);
    let old_paths = backup
        .files
        .iter()
        .map(|(_, relative_path, _)| relative_path.as_str())
        .collect::<HashSet<_>>();
    for installed_file in installed {
        if old_paths.contains(installed_file.relative_path.as_str()) {
            continue;
        }
        crate::state::instances::commands::remove_project(
            instance_id,
            &installed_file.relative_path,
            state,
        )
        .await?;
    }
    for (member_key, relative_path, backup_path) in &backup.files {
        let destination = instance_path.join(relative_path);
        if let Some(parent) = destination.parent() {
            crate::util::io::create_dir_all(parent).await?;
        }
        tokio::fs::copy(backup_path, &destination).await?;
        let Some(member) = old_members
            .iter()
            .find(|member| member.member_key == *member_key)
        else {
            continue;
        };
        let (Some(project_id), Some(file_id)) = (
            member.provider_project_id.as_deref(),
            member.provider_release_id.as_deref(),
        ) else {
            continue;
        };
        let provider_ref = ContentProviderRef::CurseForge {
            project_id: CurseForgeProjectId::new(project_id.parse().map_err(
                |_| {
                    ErrorKind::InputError(
                        "Stored CurseForge project ID is invalid".to_string(),
                    )
                },
            )?)?,
            file_id: Some(CurseForgeFileId::new(file_id.parse().map_err(
                |_| {
                    ErrorKind::InputError(
                        "Stored CurseForge file ID is invalid".to_string(),
                    )
                },
            )?)?),
        };
        let (size, sha1) = sha1_file_async(&destination).await?;
        crate::state::record_project_file_atomic(
            instance_id,
            relative_path,
            &sha1,
            size,
            member.project_type,
            ContentSourceKind::CurseForge,
            crate::state::instances::ContentOwnershipKind::PackManaged,
            Some(&provider_ref),
            true,
            None,
            state,
        )
        .await?;
    }

    let current_members = crate::state::instances::adapters::sqlite::content_rows::get_pack_members(
		&metadata.applied_content_set.id,
		&state.pool,
	)
	.await?;
    let _instance_lock = state.lock_instance_content(instance_id).await;
    let mut tx = state.pool.begin_with("BEGIN IMMEDIATE").await?;
    let old_keys = old_members
        .iter()
        .map(|member| member.member_key.as_str())
        .collect::<HashSet<_>>();
    for member in current_members {
        if !old_keys.contains(member.member_key.as_str()) {
            sqlx::query("DELETE FROM instance_pack_members WHERE id = ?")
                .bind(member.id)
                .execute(&mut *tx)
                .await?;
        }
    }
    for old_member in old_members {
        let current_entry_id = sqlx::query_scalar::<_, Option<String>>(
            "SELECT content_entry_id FROM instance_pack_members
			 WHERE content_set_id = ? AND member_key = ?",
        )
        .bind(&metadata.applied_content_set.id)
        .bind(&old_member.member_key)
        .fetch_optional(&mut *tx)
        .await?
        .flatten();
        let mut restored_member = old_member.clone();
        if current_entry_id.is_some() {
            restored_member.content_entry_id = current_entry_id;
        }
        crate::state::instances::adapters::sqlite::content_rows::upsert_pack_member_in_transaction(
			&restored_member,
			&mut tx,
		)
		.await?;
    }
    crate::state::instances::adapters::sqlite::content_rows::bump_content_set_revision_in_transaction(
		&metadata.applied_content_set.id,
		&mut tx,
	)
	.await?;
    tx.commit().await?;
    Ok(())
}

async fn cache_instance_icon_from_url(
    icon_url: &str,
) -> crate::Result<std::path::PathBuf> {
    let normalized =
        crate::util::download::retired_sources::normalize(icon_url)?;
    let icon_url = normalized.as_ref();
    let state = State::get().await?;
    // CurseForge avatar/CDN assets are frequently broken via local system
    // proxies, so always download icons with a direct client.
    let permit = state.fetch_semaphore.0.acquire().await?;
    let response = crate::util::download::native_request::send_cdn_request(
        crate::util::fetch::DIRECT_REQWEST_CLIENT.get(icon_url),
    )
    .await?;
    drop(permit);
    if !response.status().is_success() {
        return Err(ErrorKind::OtherError(format!(
            "CurseForge icon download failed with HTTP {}",
            response.status().as_u16()
        ))
        .into());
    }
    let icon_bytes = response.bytes().await?;
    let filename = icon_url.rsplit('/').next().unwrap_or("icon.png");
    crate::util::fetch::write_cached_icon(
        filename,
        &state.directories.caches_dir(),
        icon_bytes,
        &state.io_semaphore,
    )
    .await
}

struct ModpackOverrideTask {
    index: usize,
    target: PathBuf,
    size: u64,
}

fn collect_modpack_override_tasks<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    manifest: &CurseForgeModpackManifest,
    instance_path: &Path,
    cancellation: Option<&CancellationToken>,
) -> crate::Result<Vec<ModpackOverrideTask>> {
    let prefix = format!("{}/", manifest.overrides.trim_matches('/'));
    let mut tasks_by_target = HashMap::<PathBuf, ModpackOverrideTask>::new();
    for index in 0..archive.len() {
        crate::api::pack::archive_util::check_cancellation(cancellation)?;
        let entry = archive.by_index(index).map_err(modpack_zip_error)?;
        let entry_name =
            crate::pack::detect::decode_zip_entry_name(entry.name_raw());
        if entry.is_dir() || !entry_name.starts_with(&prefix) {
            continue;
        }
        let relative = &entry_name[prefix.len()..];
        let safe_path = safe_archive_relative_path(relative)?;
        let target = instance_path.join(safe_path);
        // Preserve archive order semantics for duplicate targets: the last
        // entry wins, while unique targets can be extracted independently.
        tasks_by_target.insert(
            target.clone(),
            ModpackOverrideTask {
                index,
                target,
                size: entry.size(),
            },
        );
    }
    let total_size = tasks_by_target
        .values()
        .fold(0_u64, |total, task| total.saturating_add(task.size));
    let limit = crate::api::pack::archive_util::EXTRACTION_SIZE_LIMIT;
    if total_size > limit {
        return Err(ErrorKind::InputError(format!(
			"CurseForge modpack overrides exceed the extraction limit: {total_size} unpacked bytes required, limit is {limit} bytes"
		))
		.into());
    }
    Ok(tasks_by_target.into_values().collect())
}

fn materialize_modpack_overrides(
    archive_path: &Path,
    instance_path: &Path,
    cancellation: Option<&tokio_util::sync::CancellationToken>,
) -> crate::Result<(
    u32,
    crate::api::pack::archive_util::StagedArchiveReplacements,
)> {
    let file = std::fs::File::open(archive_path)?;
    let mut archive = zip::ZipArchive::new(file).map_err(modpack_zip_error)?;
    let manifest = read_modpack_manifest(&mut archive)?;
    let tasks = collect_modpack_override_tasks(
        &mut archive,
        &manifest,
        instance_path,
        cancellation,
    )?;
    drop(archive);
    if tasks.is_empty() {
        return Ok((
            0,
            crate::api::pack::archive_util::materialize_staged_archive_entries(
                &[],
                cancellation,
            )?,
        ));
    }
    let targets = tasks
        .iter()
        .map(|task| task.target.clone())
        .collect::<Vec<_>>();
    let worker_count = OVERRIDE_EXTRACTION_CONCURRENCY.min(tasks.len());
    let queue = Arc::new(Mutex::new(VecDeque::from(tasks)));
    let extraction_result = std::thread::scope(|scope| {
        let mut workers = Vec::with_capacity(worker_count);
        for _ in 0..worker_count {
            let queue = Arc::clone(&queue);
            workers.push(scope.spawn(move || -> crate::Result<u32> {
                let file = std::fs::File::open(archive_path).map_err(|error| {
                    crate::util::io::IOError::with_path(error, archive_path)
                })?;
                let mut archive =
                    zip::ZipArchive::new(file).map_err(modpack_zip_error)?;
                let mut files_written = 0_u32;
                loop {
                    crate::api::pack::archive_util::check_cancellation(
                        cancellation,
                    )?;
                    let Some(task) = queue.lock().unwrap().pop_front() else {
                        break;
                    };
                    let mut entry = archive
                        .by_index(task.index)
                        .map_err(modpack_zip_error)?;
                    let written = crate::api::pack::archive_util::write_archive_entry_to_staging(
                        &mut entry,
                        &task.target,
                        cancellation,
                    )?;
                    if written != entry.size() {
                        return Err(ErrorKind::InputError(
                            "CurseForge modpack override was truncated during extraction"
                                .to_string(),
                        )
                        .into());
                    }
                    files_written = files_written.checked_add(1).ok_or_else(|| {
                        ErrorKind::InputError(
                            "CurseForge modpack contains too many override files"
                                .to_string(),
                        )
                    })?;
                }
                Ok(files_written)
            }));
        }
        let mut files_written = 0_u32;
        for worker in workers {
            let count = worker.join().map_err(|_| {
                ErrorKind::OtherError(
                    "CurseForge override extraction worker panicked"
                        .to_string(),
                )
            })??;
            files_written =
                files_written.checked_add(count).ok_or_else(|| {
                    ErrorKind::InputError(
                        "CurseForge modpack contains too many override files"
                            .to_string(),
                    )
                })?;
        }
        Ok(files_written)
    });
    match extraction_result {
        Ok(files_written) => {
            let replacements = crate::api::pack::archive_util::materialize_staged_archive_entries(
                &targets,
                cancellation,
            )?;
            Ok((files_written, replacements))
        }
        Err(error) => {
            if let Err(cleanup_error) =
                crate::api::pack::archive_util::discard_staged_archive_entries(
                    &targets,
                )
            {
                return Err(ErrorKind::OtherError(format!(
                    "{error}; failed to clean staged CurseForge overrides: {cleanup_error}"
                ))
                .into());
            }
            Err(error)
        }
    }
}

#[cfg(test)]
fn extract_modpack_overrides(
    archive_path: &Path,
    instance_path: &Path,
    cancellation: Option<&tokio_util::sync::CancellationToken>,
) -> crate::Result<u32> {
    let (files_written, replacements) = materialize_modpack_overrides(
        archive_path,
        instance_path,
        cancellation,
    )?;
    replacements.finalize()?;
    Ok(files_written)
}

async fn settle_materialized_modpack_overrides(
    instance_id: String,
    cancellation: CancellationToken,
    replacements: crate::api::pack::archive_util::StagedArchiveReplacements,
    commit: bool,
) -> crate::Result<()> {
    crate::api::pack::archive_util::run_blocking_instance_write(
        instance_id,
        cancellation,
        move |_| {
            if commit {
                replacements.finalize()
            } else {
                replacements.rollback()
            }
        },
    )
    .await
}

fn read_modpack_manifest<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
) -> crate::Result<CurseForgeModpackManifest> {
    let mut entry = archive.by_name("manifest.json").map_err(|_| {
        ErrorKind::InputError(
            "CurseForge modpack is missing manifest.json".to_string(),
        )
    })?;
    let mut json = String::new();
    entry.read_to_string(&mut json)?;
    Ok(serde_json::from_str::<CurseForgeModpackManifest>(&json)?)
}

fn read_modpack_override_content<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    manifest: &CurseForgeModpackManifest,
) -> crate::Result<Vec<CurseForgePackExpectedOverride>> {
    let prefix = format!("{}/", manifest.overrides.trim_matches('/'));
    let mut overrides = Vec::new();
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(modpack_zip_error)?;
        let entry_name =
            crate::pack::detect::decode_zip_entry_name(entry.name_raw());
        if entry.is_dir() || !entry_name.starts_with(&prefix) {
            continue;
        }
        let Some(override_content) =
            curseforge_override_content(&entry_name[prefix.len()..])?
        else {
            continue;
        };
        overrides.push(override_content);
    }
    overrides.sort_by(|left, right| {
        left.expected_relative_path
            .cmp(&right.expected_relative_path)
    });
    overrides.dedup_by(|left, right| {
        left.expected_relative_path == right.expected_relative_path
    });
    Ok(overrides)
}

fn curseforge_override_content(
    relative_path: &str,
) -> crate::Result<Option<CurseForgePackExpectedOverride>> {
    let relative_path = safe_archive_relative_path(relative_path)?;
    let Some(project_type) = crate::state::instances::adapters::filesystem::project_type_from_relative_path(
        &relative_path,
    ) else {
        return Ok(None);
    };
    if !crate::state::instances::adapters::filesystem::is_scannable_project_path(
        project_type,
        &relative_path,
    ) {
        return Ok(None);
    }
    Ok(Some(CurseForgePackExpectedOverride {
        project_type,
        expected_relative_path: relative_path,
    }))
}

fn modpack_target(
    manifest: &CurseForgeModpackManifest,
) -> crate::Result<CurseForgeModpackTarget> {
    let Some(manifest_loader) = manifest
        .minecraft
        .mod_loaders
        .iter()
        .find(|loader| loader.primary)
        .or_else(|| manifest.minecraft.mod_loaders.first())
    else {
        return Ok(CurseForgeModpackTarget {
            game_version: manifest.minecraft.version.clone(),
            loader: ModLoader::Vanilla,
            loader_version: None,
        });
    };

    let family = loader_family(&manifest_loader.id);
    let manifest_loader_version = manifest_loader
        .id
        .strip_prefix(family)
        .and_then(|version| version.strip_prefix('-'))
        .filter(|version| !version.is_empty());
    let disguised_cleanroom = family == "forge"
        && manifest.minecraft.version == "1.12.2"
        && manifest_loader_version
            .is_some_and(|version| version.starts_with("0."));
    let loader = match family {
        "cleanroom" => ModLoader::Cleanroom,
        "forge" if disguised_cleanroom => ModLoader::Cleanroom,
        "forge" => ModLoader::Forge,
        "fabric" => ModLoader::Fabric,
        "quilt" => ModLoader::Quilt,
        "neo" | "neoforge" => ModLoader::NeoForge,
        _ => {
            return Err(ErrorKind::InputError(format!(
                "CurseForge modpack uses unsupported loader {}",
                manifest_loader.id
            ))
            .into());
        }
    };
    let loader_version = manifest_loader_version
        .filter(|version| !disguised_cleanroom || version.starts_with("0."))
        .map(str::to_string);

    Ok(CurseForgeModpackTarget {
        game_version: manifest.minecraft.version.clone(),
        loader,
        loader_version,
    })
}

/// Reads the identity fields from a local CurseForge archive before an
/// instance is created, avoiding the temporary default Vanilla metadata.
pub async fn get_local_modpack_target(
    archive_path: &Path,
) -> crate::Result<(String, CurseForgeModpackTarget)> {
    let path = archive_path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(path)?;
        let mut archive =
            zip::ZipArchive::new(file).map_err(modpack_zip_error)?;
        let manifest = read_modpack_manifest(&mut archive)?;
        let name = manifest
            .name
            .clone()
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| "CurseForge Modpack".to_string());
        Ok((name, modpack_target(&manifest)?))
    })
    .await?
}

fn modpack_zip_error(error: zip::result::ZipError) -> crate::Error {
    ErrorKind::InputError(format!(
        "CurseForge modpack archive is invalid: {error}"
    ))
    .into()
}

fn safe_archive_relative_path(value: &str) -> crate::Result<String> {
    let normalized = value.replace('\\', "/");
    let path = Path::new(&normalized);
    if normalized.is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(ErrorKind::InputError(
            "CurseForge modpack contains an invalid override path".to_string(),
        )
        .into());
    }
    Ok(normalized)
}

pub(crate) fn loader_family(loader_id: &str) -> &str {
    loader_id.split('-').next().unwrap_or(loader_id)
}

fn loader_type(loader: &str) -> Option<u32> {
    match loader.trim().to_ascii_lowercase().as_str() {
        "forge" | "cleanroom" => Some(1),
        "fabric" => Some(4),
        "quilt" => Some(5),
        "neoforge" => Some(6),
        _ => None,
    }
}

fn looks_like_game_version(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty()
        && value.as_bytes().first().is_some_and(u8::is_ascii_digit)
        && loader_type(value).is_none()
}

fn merge_install_result(
    target: &mut CurseForgeInstallResult,
    mut source: CurseForgeInstallResult,
) {
    target.installed.append(&mut source.installed);
    target.manual_downloads.append(&mut source.manual_downloads);
    target.failed_downloads.append(&mut source.failed_downloads);
    target
        .optional_dependencies
        .append(&mut source.optional_dependencies);
    target
        .incompatible_dependencies
        .append(&mut source.incompatible_dependencies);
}

pub async fn switch_installed_file_version(
    instance_id: &str,
    relative_path: &str,
    file_id: u32,
    reporter: Option<InstallProgressReporter>,
) -> crate::Result<CurseForgeInstallResult> {
    use sqlx::Row;

    let state = State::get().await?;
    let row = sqlx::query(
		"SELECT ref.provider_project_id, ref.provider_release_id, entry.project_type,
				entry.ownership_kind,
				content_set.game_version, content_set.loader
         FROM instance_files file
         INNER JOIN instance_content_entries entry ON entry.file_id = file.id
         INNER JOIN instance_content_provider_refs ref
            ON ref.content_entry_id = entry.id AND ref.provider = 'curseforge'
         INNER JOIN instance_content_sets content_set
            ON content_set.id = entry.content_set_id
         WHERE file.instance_id = ? AND file.relative_path = ?
         ORDER BY entry.modified_at DESC
         LIMIT 1",
    )
    .bind(instance_id)
    .bind(relative_path)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| {
        ErrorKind::InputError(
            "The selected file is not linked to CurseForge".to_string(),
        )
    })?;
    let project_id = row
        .try_get::<String, _>("provider_project_id")?
        .parse::<u32>()
        .map_err(|_| {
            ErrorKind::InputError(
                "Stored CurseForge project ID is invalid".to_string(),
            )
        })?;
    let current_file_id = row
        .try_get::<Option<String>, _>("provider_release_id")?
        .and_then(|value| value.parse::<u32>().ok());
    let project_type = row.try_get::<String, _>("project_type")?;
    let ownership_kind =
        crate::state::instances::ContentOwnershipKind::from_str(
            &row.try_get::<String, _>("ownership_kind")?,
        )?;
    let game_version = row.try_get::<String, _>("game_version")?;
    let loader = row.try_get::<String, _>("loader")?;
    let mod_loader_type = match loader.as_str() {
        "forge" => Some(1),
        "fabric" => Some(4),
        "quilt" => Some(5),
        "neoforge" => Some(6),
        _ => None,
    };

    if current_file_id == Some(file_id) {
        return Ok(CurseForgeInstallResult::default());
    }

    install_selected_file(
        instance_id,
        relative_path,
        project_id,
        current_file_id,
        file_id,
        project_type,
        ownership_kind,
        game_version,
        mod_loader_type,
        reporter,
    )
    .await
}

pub(crate) async fn prepare_curseforge_content_change_action(
    instance_id: &str,
    action: &mut crate::install::ContentChangeAction,
) -> crate::Result<()> {
    use sqlx::Row;

    let state = State::get().await?;
    let relative_path = action.relative_path.as_deref().ok_or_else(|| {
        ErrorKind::InputError(format!(
            "Content {} is not present on disk",
            action.content_id
        ))
    })?;
    let row = sqlx::query(
        "SELECT entry.project_type, entry.ownership_kind,
				content_set.game_version, content_set.loader
		 FROM instance_files file
		 INNER JOIN instance_content_entries entry ON entry.file_id = file.id
		 INNER JOIN instances instance ON instance.id = entry.instance_id
		 INNER JOIN instance_content_sets content_set
			ON content_set.id = entry.content_set_id
		 WHERE file.instance_id = ? AND file.relative_path = ?
			AND entry.content_set_id = instance.applied_content_set_id
		 ORDER BY entry.modified_at DESC
		 LIMIT 1",
    )
    .bind(instance_id)
    .bind(relative_path)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| {
        ErrorKind::InputError(
            "The selected file is not linked to the active content set"
                .to_string(),
        )
    })?;
    let project_id = action
        .project_id
        .as_deref()
        .ok_or_else(|| {
            ErrorKind::InputError("Missing CurseForge project ID".to_string())
        })?
        .parse::<u32>()
        .map_err(|_| {
            ErrorKind::InputError("Invalid CurseForge project ID".to_string())
        })?;
    let file_id = action.target_release_id.parse::<u32>().map_err(|_| {
        ErrorKind::InputError("Invalid CurseForge file ID".to_string())
    })?;
    let project_type = row.try_get::<String, _>("project_type")?;
    let ownership_kind =
        crate::state::instances::ContentOwnershipKind::from_str(
            &row.try_get::<String, _>("ownership_kind")?,
        )?;
    let loader = row.try_get::<String, _>("loader")?;
    let mod_loader_type = match loader.as_str() {
        "forge" => Some(1),
        "fabric" => Some(4),
        "quilt" => Some(5),
        "neoforge" => Some(6),
        _ => None,
    };
    let preview = preview_install_file(CurseForgeInstallRequest {
        instance_id: instance_id.to_string(),
        project_id,
        file_id,
        project_type,
        ownership_kind,
        manual_operation_kind:
            crate::state::instances::ManualDownloadOperationKind::ContentUpdate,
        game_version: Some(row.try_get::<String, _>("game_version")?),
        mod_loader_type,
        world_name: None,
        install_dependencies: true,
        excluded_dependency_project_ids: Vec::new(),
        force_dependency_project_ids: Vec::new(),
        dependency_plan_id: None,
        defer_persistence: false,
        verification_tx: None,
        pre_resolved_relative_path: None,
        expected_file_name: None,
    })
    .await?;
    let old_provider_file_name = match action.expected_release_id.as_deref() {
        Some(file_id) => Some(
            get_file(
                project_id,
                file_id.parse::<u32>().map_err(|_| {
                    ErrorKind::InputError(
                        "Invalid stored CurseForge file ID".to_string(),
                    )
                })?,
            )
            .await?
            .file_name,
        ),
        None => None,
    };
    action.final_relative_path = Some(
        crate::state::instances::commands::preserved_update_relative_path(
            relative_path,
            old_provider_file_name.as_deref(),
            &preview.primary.file_name,
        )?,
    );
    action.current_provider_file_name = old_provider_file_name;
    action.target_provider_file_name = Some(preview.primary.file_name.clone());
    action.files.clear();
    action.dependencies.clear();

    for (index, item) in std::iter::once(&preview.primary)
        .chain(preview.dependencies.iter())
        .enumerate()
    {
        let file = get_file(item.project_id, item.file_id).await?;
        let project = get_project(item.project_id).await?;
        let url = resolve_curseforge_download_url(
            item.project_id,
            item.file_id,
            &project,
            &file,
        )
        .await?;
        let cache_path = state
            .directories
            .caches_dir()
            .join("content")
            .join("curseforge")
            .join(item.project_id.to_string())
            .join(item.file_id.to_string())
            .join(crate::util::io::sanitize_download_file_name(
                &item.file_name,
            )?);
        let id = format!("curseforge:{}:{}", item.project_id, item.file_id);
        action.files.push(crate::install::ContentChangeFile {
			id: id.clone(),
			role: if index == 0 {
				crate::install::ContentChangeFileRole::Primary
			} else {
				crate::install::ContentChangeFileRole::Dependency
			},
			provider: ContentProvider::CurseForge,
			project_id: item.project_id.to_string(),
			release_id: item.file_id.to_string(),
			file_name: item.file_name.clone(),
			target_relative_path: cache_path.display().to_string(),
			urls: url.clone().into_iter().collect(),
			manual_download_url: url.is_none().then(|| {
				format!(
					"https://www.curseforge.com/minecraft/mc-mods/{}/download/{}/file",
					project.slug, item.file_id
				)
			}),
			integrity: crate::install::ContentChangeFileIntegrity {
				size: Some(item.size),
				sha1: curseforge_file_sha1(&file),
				sha512: None,
				sha256: None,
				md5: file
					.hashes
					.iter()
					.find(|hash| hash.algo == 2)
					.map(|hash| hash.value.clone()),
			},
		});
        for parent in &item.required_by_project_ids {
            action
                .dependencies
                .push(crate::install::ContentChangeDependency {
                    parent_file_id: format!("curseforge:{parent}:unknown"),
                    child_file_id: id.clone(),
                    provider: ContentProvider::CurseForge,
                    project_id: item.project_id.to_string(),
                    release_id: item.file_id.to_string(),
                    kind: Some(if item.required {
                        crate::state::instances::ContentDependencyKind::Required
                    } else {
                        crate::state::instances::ContentDependencyKind::Include
                    }),
                });
        }
    }
    for fallback in preview.modrinth_fallbacks {
        action
            .dependencies
            .push(crate::install::ContentChangeDependency {
                parent_file_id: format!(
                    "curseforge:{}:unknown",
                    fallback.parent_project_id
                ),
                child_file_id: format!("modrinth:{}", fallback.version_id),
                provider: ContentProvider::Modrinth,
                project_id: fallback.project_id.clone(),
                release_id: fallback.version_id.clone(),
                kind: Some(if fallback.required {
                    crate::state::instances::ContentDependencyKind::Required
                } else {
                    crate::state::instances::ContentDependencyKind::Include
                }),
            });
        let prepared =
            crate::state::instances::commands::prepare_version_download(
                instance_id,
                &fallback.version_id,
                crate::util::fetch::DownloadReason::Dependency,
                None,
                &state,
            )
            .await?;
        action.files.push(crate::install::ContentChangeFile {
            id: format!("modrinth:{}", fallback.version_id),
            role: crate::install::ContentChangeFileRole::Dependency,
            provider: ContentProvider::Modrinth,
            project_id: fallback.project_id,
            release_id: fallback.version_id,
            file_name: prepared.file_name.clone(),
            target_relative_path: prepared.path.display().to_string(),
            urls: vec![prepared.url.clone()],
            manual_download_url: None,
            integrity: crate::install::ContentChangeFileIntegrity {
                size: prepared.integrity.size,
                sha1: prepared.integrity.sha1.clone(),
                sha512: prepared.integrity.sha512.clone(),
                sha256: None,
                md5: None,
            },
        });
    }
    action.set_status(crate::install::ContentChangeActionStatus::Prepared);
    Ok(())
}

async fn install_selected_file(
    instance_id: &str,
    relative_path: &str,
    project_id: u32,
    current_file_id: Option<u32>,
    file_id: u32,
    project_type: String,
    ownership_kind: crate::state::instances::ContentOwnershipKind,
    game_version: String,
    mod_loader_type: Option<u32>,
    reporter: Option<InstallProgressReporter>,
) -> crate::Result<CurseForgeInstallResult> {
    let old_provider_file_name = match current_file_id {
        Some(current_file_id) => {
            Some(get_file(project_id, current_file_id).await?.file_name)
        }
        None => None,
    };
    let new_provider_file_name = get_file(project_id, file_id).await?.file_name;
    let request = CurseForgeInstallRequest {
        instance_id: instance_id.to_string(),
        project_id,
        file_id,
        project_type,
        ownership_kind,
        manual_operation_kind:
            crate::state::instances::ManualDownloadOperationKind::ContentUpdate,
        game_version: Some(game_version),
        mod_loader_type,
        world_name: None,
        install_dependencies: true,
        excluded_dependency_project_ids: Vec::new(),
        force_dependency_project_ids: Vec::new(),
        dependency_plan_id: None,
        defer_persistence: false,
        verification_tx: None,
        pre_resolved_relative_path: None,
        expected_file_name: None,
    };
    let mut result = match reporter {
        Some(reporter) => install_file_with_reporter(request, reporter).await?,
        None => install_file(request).await?,
    };
    if ownership_kind
        == crate::state::instances::ContentOwnershipKind::PackManaged
        && let Some(installed) = result
            .installed
            .iter()
            .find(|file| !file.dependency && file.project_id == project_id)
    {
        mark_pack_member_version_override(
            instance_id,
            &installed.relative_path,
        )
        .await?;
    }
    if let Some(primary_index) = result.installed.iter().position(|file| {
        !file.dependency
            && file.project_id == project_id
            && file.relative_path != relative_path
    }) {
        let state = State::get().await?;
        let installed_path =
            result.installed[primary_index].relative_path.clone();
        let final_path =
            crate::state::instances::commands::finalize_updated_project_path(
                instance_id,
                relative_path,
                &installed_path,
                old_provider_file_name.as_deref(),
                &new_provider_file_name,
                &state,
            )
            .await?;
        result.installed[primary_index].relative_path = final_path;
    }
    Ok(result)
}

pub async fn recognize_instance_files(
    instance_id: &str,
) -> crate::Result<CurseForgeRecognitionResult> {
    let mut result = CurseForgeRecognitionResult::default();
    if capability().status != CurseForgeCapabilityStatus::Ready {
        return Ok(result);
    }

    let state = State::get().await?;
    let instance_files =
        crate::api::instance::sync_content_files(instance_id).await?;
    let tracked_file_ids = match crate::state::instances::adapters::sqlite::content_rows::get_applied_content_set(
        instance_id,
        &state.pool,
    )
    .await?
    {
        Some(content_set) => {
            crate::state::instances::adapters::sqlite::content_rows::get_content_entries(
                &content_set.id,
                &state.pool,
            )
            .await?
            .into_iter()
            .filter_map(|entry| entry.file_id)
            .collect::<HashSet<_>>()
        }
        None => HashSet::new(),
    };
    let instance_path =
        crate::api::instance::get_full_path(instance_id).await?;
    let mut fingerprints = Vec::new();
    let mut paths_by_fingerprint =
        HashMap::<u64, Vec<(String, ProjectType)>>::new();

    for file in instance_files.into_iter().filter(|file| !file.missing) {
        if tracked_file_ids.contains(&file.id) {
            continue;
        }
        let Some(project_type) =
            crate::state::instances::adapters::filesystem::project_type_from_relative_path(
                &file.relative_path,
            )
        else {
            continue;
        };
        let full_path = instance_path.join(&file.relative_path);
        let Ok(bytes) = tokio::fs::read(&full_path).await else {
            continue;
        };
        let fingerprint = compute_fingerprint(&bytes) as u64;
        fingerprints.push(fingerprint);
        paths_by_fingerprint
            .entry(fingerprint)
            .or_default()
            .push((file.relative_path, project_type));
    }

    let mut matches = HashMap::new();
    for chunk in fingerprints.chunks(1000) {
        let response = match_fingerprints(chunk.to_vec()).await?;
        for matched in response.exact_matches {
            matches.insert(matched.file.file_fingerprint, matched.file);
        }
    }

    result.scanned = fingerprints.len() as u32;
    let matched_project_ids =
        matches.values().map(|file| file.mod_id).collect::<Vec<_>>();
    let matched_projects = if matched_project_ids.is_empty() {
        HashMap::new()
    } else {
        match get_projects(matched_project_ids).await {
            Ok(projects) => projects
                .into_iter()
                .map(|project| (project.id, project))
                .collect(),
            Err(error) => {
                tracing::warn!(
                    %error,
                    "Failed to resolve CurseForge project types while recognizing instance files"
                );
                HashMap::new()
            }
        }
    };

    for (fingerprint, paths) in paths_by_fingerprint {
        if let Some(file) = matches.get(&fingerprint) {
            for (path, path_project_type) in paths {
                let project_type = matched_projects
                    .get(&file.mod_id)
                    .and_then(|project| {
                        recognized_project_type(project.class_id)
                    })
                    .unwrap_or(path_project_type);
                let full_path = instance_path.join(&path);
                let Some((size, sha1)) = verify_recognized_curseforge_file(
                    &full_path,
                    file.file_fingerprint,
                )
                .await?
                else {
                    continue;
                };
                crate::state::record_verified_curseforge_project_file_atomic(
                    instance_id,
                    &path,
                    &sha1,
                    size,
                    project_type,
                    ContentSourceKind::CurseForge,
                    crate::state::instances::ContentOwnershipKind::UserAdded,
                    CurseForgeProjectId::new(file.mod_id)?,
                    CurseForgeFileId::new(file.id)?,
                    true,
                    &state,
                )
                .await?;
                result.linked.push(CurseForgeInstalledFile {
                    project_id: file.mod_id,
                    file_id: file.id,
                    relative_path: path,
                    dependency: false,
                });
                result.matched += 1;
            }
        } else {
            result
                .unmatched_paths
                .extend(paths.into_iter().map(|(path, _)| path));
        }
    }
    result.unmatched_paths.sort();
    Ok(result)
}

async fn verify_recognized_curseforge_file(
    path: &Path,
    expected_fingerprint: u64,
) -> crate::Result<Option<(u64, String)>> {
    if expected_fingerprint == 0 {
        return Ok(None);
    }
    let (size, sha1, fingerprint) =
        fingerprint_and_sha1_file(path, None).await?;
    if fingerprint as u64 != expected_fingerprint {
        return Ok(None);
    }
    Ok(Some((size, sha1)))
}

pub async fn import_manual_downloads(
    instance_id: &str,
    scan_directory: Option<PathBuf>,
) -> crate::Result<CurseForgeManualDownloadScanResult> {
    let Some(download_directory) = scan_directory.or_else(dirs::download_dir)
    else {
        finalize_curseforge_manual_download_import(instance_id, false).await?;
        return Ok(CurseForgeManualDownloadScanResult::default());
    };
    let mut result = CurseForgeManualDownloadScanResult {
        download_directory: Some(download_directory.to_string_lossy().into()),
        ..Default::default()
    };
    let downloads = list_pending_manual_downloads(instance_id).await?;

    for download in downloads {
        let candidate = match find_manual_download_candidate(
            &download_directory,
            &download,
        )
        .await
        {
            Ok(candidate) => candidate,
            Err(error) => {
                result.errors.push(CurseForgeManualDownloadImportError {
                    project_id: download.project_id,
                    file_id: download.file_id,
                    message: error.to_string(),
                });
                continue;
            }
        };
        let Some((source_path, size, sha1)) = candidate else {
            continue;
        };

        match install_manual_download(
            instance_id,
            &download,
            &source_path,
            size,
            &sha1,
        )
        .await
        {
            Ok(relative_path) => {
                result.imported.push(CurseForgeManualDownloadImport {
                    project_id: download.project_id,
                    file_id: download.file_id,
                    relative_path,
                });
            }
            Err(error) => {
                result.errors.push(CurseForgeManualDownloadImportError {
                    project_id: download.project_id,
                    file_id: download.file_id,
                    message: error.to_string(),
                });
            }
        }
    }

    finalize_curseforge_manual_download_import(
        instance_id,
        !result.imported.is_empty(),
    )
    .await?;

    Ok(result)
}

pub async fn configure_manual_download_watcher(
    enabled: bool,
    scan_directory: Option<PathBuf>,
) -> crate::Result<Option<String>> {
    let state = State::get().await?;
    let directory = if enabled {
        let Some(directory) = scan_directory.or_else(dirs::download_dir) else {
            state
                .file_watcher
                .configure_manual_import_directory(None)
                .await?;
            return Ok(None);
        };
        let directory = tokio::fs::canonicalize(directory).await?;
        if !tokio::fs::metadata(&directory).await?.is_dir() {
            return Err(ErrorKind::InputError(
                "Manual import watch path is not a directory".to_string(),
            )
            .into());
        }
        Some(directory)
    } else {
        None
    };

    state
        .file_watcher
        .configure_manual_import_directory(directory.clone())
        .await?;
    Ok(directory.map(|path| path.to_string_lossy().into_owned()))
}

pub(crate) async fn scan_pending_manual_downloads_in(
    download_directory: &Path,
) -> crate::Result<()> {
    let _scan_guard = MANUAL_IMPORT_SCAN_LOCK.lock().await;
    let state = State::get().await?;
    let instance_ids = sqlx::query_scalar::<_, String>(
        "SELECT DISTINCT instance_id
         FROM instance_pending_manual_downloads
         WHERE provider = 'curseforge'
            AND state IN ('waiting', 'matched', 'error')",
    )
    .fetch_all(&state.pool)
    .await?;

    for instance_id in instance_ids {
        let result = import_manual_downloads(
            &instance_id,
            Some(download_directory.to_path_buf()),
        )
        .await?;
        for error in result.errors {
            tracing::debug!(
                instance_id,
                project_id = error.project_id,
                file_id = error.file_id,
                "Pending manual download not ready: {}",
                error.message
            );
        }
    }

    let waiting_instances = crate::install::store::list(false, &state)
        .await?
        .into_iter()
        .filter(|job| {
            job.status == crate::install::InstallJobStatus::WaitingForUser
                && is_curseforge_manual_download_job(&job.state)
        })
        .filter_map(|job| job.instance_id)
        .collect::<HashSet<_>>();
    for instance_id in waiting_instances {
        reconcile_curseforge_waiting_jobs_for_instance(&instance_id).await?;
    }

    Ok(())
}

pub async fn list_pending_manual_downloads(
    instance_id: &str,
) -> crate::Result<Vec<CurseForgeManualDownload>> {
    let state = State::get().await?;
    Ok(crate::state::instances::adapters::sqlite::content_rows::get_pending_manual_downloads(
		instance_id,
		&state.pool,
	)
	.await?
	.into_iter()
	.filter_map(pending_manual_download)
	.collect())
}

fn pending_manual_download(
    item: crate::state::instances::PendingManualDownload,
) -> Option<CurseForgeManualDownload> {
    if item.provider != ContentProvider::CurseForge {
        return None;
    }
    serde_json::from_value::<CurseForgeManualDownload>(item.context.clone())
        .ok()
        .or_else(|| {
            Some(CurseForgeManualDownload {
                project_id: item.provider_project_id.parse().ok()?,
                file_id: item.provider_release_id.parse().ok()?,
                file_name: item.file_name,
                ownership_kind: if item.pack_member_id.is_some() {
                    crate::state::instances::ContentOwnershipKind::PackManaged
                } else {
                    Default::default()
                },
                operation_kind: item.operation_kind,
                website_url: item.website_url,
                project_type: item.project_type.get_name().to_string(),
                project_slug: String::new(),
                target_folder: Path::new(&item.target_relative_path)
                    .parent()
                    .map(|path| path.to_string_lossy().replace('\\', "/"))
                    .unwrap_or_default(),
                hashes: item
                    .expected_sha1
                    .map(|value| vec![CurseForgeFileHash { value, algo: 1 }])
                    .unwrap_or_default(),
                file_length: item.expected_size.unwrap_or_default(),
                file_fingerprint: item.expected_fingerprint.unwrap_or_default(),
            })
        })
}

#[derive(Clone, Debug)]
struct CurseForgeManualDownloadIntegrityMetadata {
    project_id: u32,
    file_id: u32,
    hashes: Vec<CurseForgeFileHash>,
    file_length: u64,
    file_fingerprint: u64,
}

impl From<CurseForgeFile> for CurseForgeManualDownloadIntegrityMetadata {
    fn from(file: CurseForgeFile) -> Self {
        Self {
            project_id: file.mod_id,
            file_id: file.id,
            hashes: file.hashes,
            file_length: file.file_length,
            file_fingerprint: file.file_fingerprint,
        }
    }
}

fn manual_download_has_integrity_metadata(
    download: &CurseForgeManualDownload,
) -> bool {
    download
        .hashes
        .iter()
        .any(|hash| hash.algo == 1 && !hash.value.trim().is_empty())
        || download.file_fingerprint != 0
}

async fn ensure_manual_download_integrity_metadata(
    download: &CurseForgeManualDownload,
) -> crate::Result<CurseForgeManualDownload> {
    ensure_manual_download_integrity_metadata_with(
        download,
        resolve_manual_download_integrity_metadata,
    )
    .await
}

async fn resolve_manual_download_integrity_metadata(
    project_id: u32,
    file_id: u32,
) -> crate::Result<CurseForgeManualDownloadIntegrityMetadata> {
    Ok(get_file(project_id, file_id).await?.into())
}

async fn ensure_manual_download_integrity_metadata_with<F, Fut>(
    download: &CurseForgeManualDownload,
    resolve_metadata: F,
) -> crate::Result<CurseForgeManualDownload>
where
    F: FnOnce(u32, u32) -> Fut,
    Fut: std::future::Future<
            Output = crate::Result<CurseForgeManualDownloadIntegrityMetadata>,
        >,
{
    if manual_download_has_integrity_metadata(download) {
        return Ok(download.clone());
    }

    let metadata =
        resolve_metadata(download.project_id, download.file_id).await?;
    if metadata.project_id != download.project_id
        || metadata.file_id != download.file_id
    {
        return Err(ErrorKind::InputError(
            "CurseForge returned metadata for a different project or file"
                .to_string(),
        )
        .into());
    }

    let mut hydrated = download.clone();
    hydrated.hashes = metadata.hashes;
    hydrated.file_length = metadata.file_length;
    hydrated.file_fingerprint = metadata.file_fingerprint;
    if !manual_download_has_integrity_metadata(&hydrated) {
        return Err(ErrorKind::InputError(
            "The required CurseForge file has no usable integrity metadata"
                .to_string(),
        )
        .into());
    }
    Ok(hydrated)
}

async fn find_manual_download_candidate(
    download_directory: &Path,
    download: &CurseForgeManualDownload,
) -> crate::Result<Option<(std::path::PathBuf, u64, String)>> {
    validate_file_name(&download.file_name)?;
    let mut entries = match tokio::fs::read_dir(download_directory).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(None);
        }
        Err(error) => return Err(error.into()),
    };

    let mut verified_download = None;
    while let Some(entry) = entries.next_entry().await? {
        let actual_file_name = entry.file_name().to_string_lossy().to_string();
        if !crate::util::downloads::browser_download_file_name_matches(
            &actual_file_name,
            &download.file_name,
        ) {
            continue;
        }
        let path = entry.path();
        tracing::debug!(
            project_id = download.project_id,
            file_id = download.file_id,
            candidate_path = %path.display(),
            "Found CurseForge manual download filename candidate"
        );
        if verified_download.is_none() {
            verified_download = Some(
                ensure_manual_download_integrity_metadata(download).await?,
            );
        }
        match verify_manual_download_candidate_with_integrity(
            &path,
            verified_download
                .as_ref()
                .expect("integrity metadata resolved"),
            true,
        )
        .await
        {
            Ok(Some((size, sha1))) => {
                return Ok(Some((path, size, sha1)));
            }
            Ok(None) => continue,
            Err(error) => {
                tracing::debug!(
                    project_id = download.project_id,
                    file_id = download.file_id,
                    candidate_path = %path.display(),
                    %error,
                    "Unable to inspect CurseForge manual download candidate"
                );
                continue;
            }
        }
    }

    Ok(None)
}

async fn verify_manual_download_candidate(
    path: &Path,
    download: &CurseForgeManualDownload,
    require_matching_name: bool,
) -> crate::Result<Option<(u64, String)>> {
    let download = ensure_manual_download_integrity_metadata(download).await?;
    verify_manual_download_candidate_with_integrity(
        path,
        &download,
        require_matching_name,
    )
    .await
}

async fn verify_manual_download_candidate_with_integrity(
    path: &Path,
    download: &CurseForgeManualDownload,
    require_matching_name: bool,
) -> crate::Result<Option<(u64, String)>> {
    let actual_file_name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    if require_matching_name
        && !crate::util::downloads::browser_download_file_name_matches(
            &actual_file_name,
            &download.file_name,
        )
    {
        trace_manual_download_candidate_rejection(
            path,
            download,
            "name_mismatch",
        );
        return Ok(None);
    }
    let metadata = tokio::fs::symlink_metadata(path).await?;
    if crate::util::io::is_symlink_or_reparse(&metadata) {
        trace_manual_download_candidate_rejection(
            path,
            download,
            "symlink_or_reparse",
        );
        return Ok(None);
    }
    if !metadata.is_file() {
        trace_manual_download_candidate_rejection(path, download, "non_file");
        return Ok(None);
    }
    if download.file_length > 0 && metadata.len() != download.file_length {
        trace_manual_download_candidate_rejection(
            path,
            download,
            "size_mismatch",
        );
        return Ok(None);
    }

    if let Some(expected_sha1) = download
        .hashes
        .iter()
        .find(|hash| hash.algo == 1 && !hash.value.trim().is_empty())
        .map(|hash| hash.value.as_str())
    {
        let (size, sha1) = sha1_file_async(path).await?;
        if !sha1.eq_ignore_ascii_case(expected_sha1) {
            trace_manual_download_candidate_rejection(
                path,
                download,
                "sha1_mismatch",
            );
            return Ok(None);
        }
        return Ok(Some((size, sha1)));
    }
    if download.file_fingerprint == 0 {
        trace_manual_download_candidate_rejection(
            path,
            download,
            "missing_integrity_metadata",
        );
        return Err(ErrorKind::InputError(
            "The required CurseForge file has no usable integrity metadata"
                .to_string(),
        )
        .into());
    }
    let (size, sha1, fingerprint) =
        fingerprint_and_sha1_file(path, None).await?;
    if fingerprint as u64 != download.file_fingerprint {
        trace_manual_download_candidate_rejection(
            path,
            download,
            "fingerprint_mismatch",
        );
        return Ok(None);
    }
    Ok(Some((size, sha1)))
}

fn trace_manual_download_candidate_rejection(
    path: &Path,
    download: &CurseForgeManualDownload,
    reason: &'static str,
) {
    tracing::trace!(
        project_id = download.project_id,
        file_id = download.file_id,
        candidate_path = %path.display(),
        expected_file_name = %download.file_name,
        rejection_reason = reason,
        "Rejected CurseForge manual download candidate"
    );
}

pub(crate) async fn import_pending_manual_download_from_path(
    instance_id: &str,
    source_path: &Path,
) -> crate::Result<Option<String>> {
    let state = State::get().await?;
    let pending = crate::state::instances::adapters::sqlite::content_rows::get_pending_manual_downloads(
        instance_id,
        &state.pool,
    )
    .await?;
    let actual_file_name = source_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let mut matched_pending_name = false;
    for download in pending.into_iter().filter_map(pending_manual_download) {
        if !crate::util::downloads::browser_download_file_name_matches(
            &actual_file_name,
            &download.file_name,
        ) {
            continue;
        }
        matched_pending_name = true;
        let Some((size, sha1)) =
            verify_manual_download_candidate(source_path, &download, true)
                .await?
        else {
            continue;
        };
        let relative_path = install_manual_download(
            instance_id,
            &download,
            source_path,
            size,
            &sha1,
        )
        .await?;
        finalize_curseforge_manual_download_import(instance_id, false).await?;
        return Ok(Some(relative_path));
    }
    if matched_pending_name {
        return Err(ErrorKind::InputError(
            "The selected file does not match the required CurseForge file"
                .to_string(),
        )
        .into());
    }
    Ok(None)
}

pub async fn import_pending_manual_download_file(
    instance_id: &str,
    project_id: u32,
    file_id: u32,
    source_path: PathBuf,
) -> crate::Result<CurseForgeManualDownloadImport> {
    import_pending_manual_download_file_with_integrity_resolver(
        instance_id,
        project_id,
        file_id,
        source_path,
        resolve_manual_download_integrity_metadata,
    )
    .await
}

async fn import_pending_manual_download_file_with_integrity_resolver<F, Fut>(
    instance_id: &str,
    project_id: u32,
    file_id: u32,
    source_path: PathBuf,
    resolve_metadata: F,
) -> crate::Result<CurseForgeManualDownloadImport>
where
    F: FnOnce(u32, u32) -> Fut,
    Fut: std::future::Future<
            Output = crate::Result<CurseForgeManualDownloadIntegrityMetadata>,
        >,
{
    tracing::debug!(
        instance_id,
        project_id,
        file_id,
        source_path = %source_path.display(),
        "Importing selected CurseForge manual download file"
    );
    let pending = list_pending_manual_downloads(instance_id).await?;
    let pending_count = pending.len();
    let download = pending.into_iter().find(|download| {
        download.project_id == project_id && download.file_id == file_id
    });
    tracing::debug!(
        instance_id,
        project_id,
        file_id,
        pending_count,
        pending_found = download.is_some(),
        "Looked up selected CurseForge pending manual download"
    );
    let download = download.ok_or_else(|| {
        ErrorKind::InputError(
            "The selected CurseForge file is not pending for this instance"
                .to_string(),
        )
    })?;
    let download = ensure_manual_download_integrity_metadata_with(
        &download,
        resolve_metadata,
    )
    .await?;
    let Some((size, sha1)) = verify_manual_download_candidate_with_integrity(
        &source_path,
        &download,
        false,
    )
    .await?
    else {
        return Err(ErrorKind::InputError(
            "The selected file does not match the required CurseForge file"
                .to_string(),
        )
        .into());
    };
    let relative_path = install_manual_download(
        instance_id,
        &download,
        &source_path,
        size,
        &sha1,
    )
    .await?;
    finalize_curseforge_manual_download_import(instance_id, true).await?;
    Ok(CurseForgeManualDownloadImport {
        project_id,
        file_id,
        relative_path,
    })
}

async fn finalize_curseforge_manual_download_import(
    instance_id: &str,
    emit_content_changed: bool,
) -> crate::Result<()> {
    let reconciliation =
        reconcile_curseforge_waiting_jobs_for_instance(instance_id).await;
    let content_changed = if emit_content_changed {
        crate::api::instance::emit_content_changed(instance_id).await
    } else {
        Ok(())
    };
    reconciliation?;
    content_changed
}

pub(crate) async fn reconcile_curseforge_waiting_jobs_for_instance(
    instance_id: &str,
) -> crate::Result<()> {
    let state = State::get().await?;
    reconcile_curseforge_waiting_jobs_for_instance_with_state(
        instance_id,
        &state,
    )
    .await
}

pub(crate) async fn reconcile_persisted_curseforge_waiting_jobs(
    state: &State,
) -> crate::Result<()> {
    let mut resume_job = resume_curseforge_install_job;
    reconcile_persisted_curseforge_waiting_jobs_with_resume(
        state,
        &mut resume_job,
    )
    .await
}

async fn reconcile_persisted_curseforge_waiting_jobs_with_resume<F, Fut>(
    state: &State,
    resume_job: &mut F,
) -> crate::Result<()>
where
    F: FnMut(uuid::Uuid) -> Fut,
    Fut: std::future::Future<Output = crate::Result<()>>,
{
    let jobs = crate::install::store::list(false, state).await?;
    for instance_id in curseforge_waiting_job_instance_ids(&jobs) {
        reconcile_curseforge_waiting_jobs_for_instance_with_resume(
            &instance_id,
            state,
            resume_job,
        )
        .await?;
    }
    Ok(())
}

fn curseforge_waiting_job_instance_ids(
    jobs: &[crate::install::store::InstallJobRecord],
) -> Vec<String> {
    let mut instance_ids = jobs
        .iter()
        .filter(|job| is_reconcilable_curseforge_waiting_job(job))
        .filter_map(|job| job.instance_id.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    instance_ids.sort_unstable();
    instance_ids
}

async fn reconcile_curseforge_waiting_jobs_for_instance_with_state(
    instance_id: &str,
    state: &State,
) -> crate::Result<()> {
    let mut resume_job = resume_curseforge_install_job;
    reconcile_curseforge_waiting_jobs_for_instance_with_resume(
        instance_id,
        state,
        &mut resume_job,
    )
    .await
}

async fn resume_curseforge_install_job(
    job_id: uuid::Uuid,
) -> crate::Result<()> {
    crate::install::runner::resume_job(job_id).await?;
    Ok(())
}

async fn reconcile_curseforge_waiting_jobs_for_instance_with_resume<F, Fut>(
    instance_id: &str,
    state: &State,
    resume_job: &mut F,
) -> crate::Result<()>
where
    F: FnMut(uuid::Uuid) -> Fut,
    Fut: std::future::Future<Output = crate::Result<()>>,
{
    let pending = crate::state::instances::adapters::sqlite::content_rows::get_pending_manual_downloads(
		instance_id,
		&state.pool,
	)
	.await?
	.into_iter()
	.filter(|download| download.provider == ContentProvider::CurseForge)
	.map(|download| {
		(download.provider_project_id, download.provider_release_id)
	})
	.collect::<HashSet<_>>();
    let materialized = crate::state::instances::adapters::sqlite::content_rows::get_materialized_curseforge_downloads(
        instance_id,
        &state.pool,
    )
    .await?
    .into_iter()
    .collect::<HashSet<_>>();

    let jobs = crate::install::store::list(false, state).await?;
    let waiting_job_count = jobs
        .iter()
        .filter(|job| {
            job.instance_id.as_deref() == Some(instance_id)
                && is_reconcilable_curseforge_waiting_job(job)
        })
        .count();
    tracing::debug!(
        instance_id,
        pending_key_count = pending.len(),
        waiting_job_count,
        "Starting CurseForge waiting install job reconciliation"
    );
    let mut recovered_item_count = 0usize;
    let mut resume_job_called = false;
    for job in jobs {
        if job.instance_id.as_deref() != Some(instance_id)
            || !is_reconcilable_curseforge_waiting_job(&job)
        {
            continue;
        }

        let operation_lock = state
            .install_job_operation_locks
            .entry(job.id)
            .or_default()
            .clone();
        let _operation_guard = operation_lock.lock().await;
        let current =
            crate::install::store::get_required(job.id, state).await?;
        if current.instance_id.as_deref() != Some(instance_id)
            || !is_reconcilable_curseforge_waiting_job(&current)
        {
            continue;
        }

        let mut reconciled_materialized = materialized.clone();
        reconciled_materialized.extend(
            manually_imported_curseforge_world_downloads(
                instance_id,
                &current.state,
            )
            .await?,
        );
        let reconciliation = curseforge_manual_download_reconciliation(
            &current.state.download_items(),
            &pending,
            &reconciled_materialized,
        );
        for inconsistent in &reconciliation.inconsistent {
            tracing::warn!(
                job_id = %current.id,
                instance_id,
                project_id = %inconsistent.project_id,
                file_id = %inconsistent.file_id,
                item_path = %inconsistent.path,
                reason = inconsistent.reason,
                "CurseForge manual download job state is inconsistent"
            );
        }

        let recovered_count = reconciliation.recovered.len();
        recovered_item_count += recovered_count;
        if recovered_count > 0 {
            InstallProgressReporter::new(current.id, current.state)
                .record_events(reconciliation.recovered)
                .await?;
        }

        let latest = crate::install::store::get_required(job.id, state).await?;
        if latest.status != crate::install::InstallJobStatus::WaitingForUser {
            continue;
        }
        let mut latest_materialized = materialized.clone();
        latest_materialized.extend(
            manually_imported_curseforge_world_downloads(
                instance_id,
                &latest.state,
            )
            .await?,
        );
        let latest_reconciliation = curseforge_manual_download_reconciliation(
            &latest.state.download_items(),
            &pending,
            &latest_materialized,
        );
        tracing::debug!(
            job_id = %latest.id,
            instance_id,
            pending_count = pending.len(),
            manual_skipped_count = reconciliation.manual_skipped_count,
            materialized_exact_match_count =
                reconciliation.materialized_exact_match_count,
            recovered_count,
            unresolved_pending_count =
                latest_reconciliation.unresolved_pending_count,
            inconsistent_count = latest_reconciliation.inconsistent.len(),
            "Reconciled CurseForge waiting install job"
        );

        if latest_reconciliation.should_resume() {
            resume_job_called = true;
            match resume_job(latest.id).await {
                Ok(_) => {
                    tracing::info!(
                        job_id = %latest.id,
                        instance_id,
                        pending_count = pending.len(),
                        manual_skipped_count =
                            reconciliation.manual_skipped_count,
                        materialized_exact_match_count =
                            reconciliation.materialized_exact_match_count,
                        recovered_count,
                        unresolved_pending_count = 0,
                        inconsistent_count = 0,
                        "Reconciled completed CurseForge manual downloads and resuming install job"
                    );
                }
                Err(error) => {
                    let current =
                        crate::install::store::get_required(latest.id, state)
                            .await?;
                    if current.status
                        == crate::install::InstallJobStatus::WaitingForUser
                    {
                        return Err(error);
                    }
                }
            }
        }
    }
    tracing::debug!(
        instance_id,
        pending_key_count = pending.len(),
        waiting_job_count,
        recovered_item_count,
        resume_job_called,
        "Completed CurseForge waiting install job reconciliation"
    );
    Ok(())
}

#[derive(Debug)]
struct CurseForgeManualDownloadInconsistency {
    path: String,
    project_id: String,
    file_id: String,
    reason: &'static str,
}

#[derive(Debug, Default)]
struct CurseForgeManualDownloadReconciliation {
    recovered: Vec<InstallJobEventKind>,
    inconsistent: Vec<CurseForgeManualDownloadInconsistency>,
    manual_skipped_count: usize,
    materialized_exact_match_count: usize,
    unresolved_pending_count: usize,
}

impl CurseForgeManualDownloadReconciliation {
    fn should_resume(&self) -> bool {
        self.unresolved_pending_count == 0 && self.inconsistent.is_empty()
    }
}

fn curseforge_manual_download_reconciliation(
    items: &[crate::install::model::DownloadItemSnapshot],
    pending: &HashSet<(String, String)>,
    materialized: &HashSet<(String, String)>,
) -> CurseForgeManualDownloadReconciliation {
    let mut result = CurseForgeManualDownloadReconciliation::default();
    for item in items.iter().filter(|item| {
        item.status == crate::install::model::DownloadItemStatus::Skipped
            && item.manual_url.is_some()
            && item.project_id.is_some()
            && item.version_id.is_some()
    }) {
        result.manual_skipped_count += 1;
        let project_id = item.project_id.as_ref().expect("filtered project ID");
        let version_id = item.version_id.as_ref().expect("filtered file ID");
        if pending.contains(&(project_id.clone(), version_id.clone())) {
            result.unresolved_pending_count += 1;
        } else if materialized
            .contains(&(project_id.clone(), version_id.clone()))
        {
            result.materialized_exact_match_count += 1;
            result
                .recovered
                .push(InstallJobEventKind::ContentFileRecovered {
                    path: item.id.clone(),
                    bytes: item.bytes_total.unwrap_or(0),
                });
        } else {
            result
                .inconsistent
                .push(CurseForgeManualDownloadInconsistency {
                    path: item.id.clone(),
                    project_id: project_id.clone(),
                    file_id: version_id.clone(),
                    reason: "pending_missing_but_not_materialized",
                });
        }
    }
    result
}

fn is_reconcilable_curseforge_waiting_job(
    job: &crate::install::store::InstallJobRecord,
) -> bool {
    job.status == crate::install::InstallJobStatus::WaitingForUser
        && job.instance_id.is_some()
        && is_curseforge_manual_download_job(&job.state)
        && matches!(
            &job.state.pause_reason,
            Some(crate::install::model::InstallPauseReason::MissingRequiredContent { .. })
        )
        && job.state.download_items().iter().any(|item| {
            item.status == crate::install::model::DownloadItemStatus::Skipped
                && item.manual_url.is_some()
                && item.project_id.is_some()
                && item.version_id.is_some()
        })
}

fn is_curseforge_manual_download_job(
    job: &crate::install::model::InstallJobState,
) -> bool {
    if !matches!(
        job.provider(),
        crate::install::model::InstallJobProvider::CurseForge
            | crate::install::model::InstallJobProvider::Local
    ) {
        return false;
    }
    job.download_items().iter().any(|item| {
        item.manual_url.is_some()
            && item.project_id.is_some()
            && item.version_id.is_some()
    })
}

async fn manually_imported_curseforge_world_downloads(
    instance_id: &str,
    job_state: &crate::install::model::InstallJobState,
) -> crate::Result<HashSet<(String, String)>> {
    let crate::install::model::InstallRequest::InstallCurseForgeWorld {
        request,
        ..
    } = &job_state.request
    else {
        return Ok(HashSet::new());
    };
    let project_id = request.project_id.to_string();
    let file_id = request.file_id.to_string();
    let instance_path =
        crate::api::instance::get_full_path(instance_id).await?;
    let imported = job_state.download_items().into_iter().any(|item| {
        if item.status != crate::install::model::DownloadItemStatus::Skipped
            || item.project_id.as_deref() != Some(project_id.as_str())
            || item.version_id.as_deref() != Some(file_id.as_str())
        {
            return false;
        }
        let Some(file_name) = Path::new(&item.id).file_name() else {
            return false;
        };
        let Some(world_name) = Path::new(file_name).file_stem() else {
            return false;
        };
        instance_path
            .join("saves")
            .join(world_name)
            .join("level.dat")
            .is_file()
    });
    Ok(if imported {
        HashSet::from([(project_id, file_id)])
    } else {
        Default::default()
    })
}

async fn install_manual_download(
    instance_id: &str,
    download: &CurseForgeManualDownload,
    source_path: &Path,
    size: u64,
    sha1: &str,
) -> crate::Result<String> {
    if download.project_type == "modpack" {
        let verified_directory = tempfile::tempdir()?;
        let verified_source =
            verified_directory.path().join("manual-download.pack");
        crate::state::materialize_verified_project_download_copy(
            source_path,
            &verified_source,
            size,
            sha1,
        )
        .await?;
        crate::install::install_pack_to_existing_instance(
            instance_id.to_string(),
            crate::api::pack::install_from::CreatePackLocation::FromFile {
                path: verified_source,
            },
            None,
        )
        .await?;
        let state = State::get().await?;
        let content_set = crate::state::instances::adapters::sqlite::content_rows::get_applied_content_set(
			instance_id,
			&state.pool,
		)
		.await?
		.ok_or_else(|| {
			ErrorKind::InputError(
				"Instance has no applied content set".to_string(),
			)
		})?;
        let _instance_lock = state.lock_instance_content(instance_id).await;
        let mut tx = state.pool.begin_with("BEGIN IMMEDIATE").await?;
        crate::state::instances::adapters::sqlite::content_rows::complete_pending_manual_download(
			instance_id,
			&download.project_id.to_string(),
			&download.file_id.to_string(),
			None,
			&mut tx,
		)
		.await?;
        crate::state::instances::adapters::sqlite::content_rows::bump_content_set_revision_in_transaction(
			&content_set.id,
			&mut tx,
        )
		.await?;
        tx.commit().await?;
        let relative_path = source_path.to_string_lossy().to_string();
        tracing::info!(
            instance_id,
            project_id = download.project_id,
            file_id = download.file_id,
            relative_path = %relative_path,
            "Completed CurseForge pending manual download record"
        );
        return Ok(relative_path);
    }
    if download.project_type == "world" {
        validate_world_archive_name(&download.file_name)?;
        let verified_directory = tempfile::tempdir()?;
        let verified_source = verified_directory.path().join(
            crate::util::io::sanitize_download_file_name(&download.file_name)?,
        );
        crate::state::materialize_verified_project_download_copy(
            source_path,
            &verified_source,
            size,
            sha1,
        )
        .await?;
        let state = State::get().await?;
        let world_name = crate::state::instances::commands::import_world_save(
            &state,
            instance_id,
            &verified_source,
            None,
        )
        .await?;
        complete_manual_world_download(instance_id, download).await?;
        return Ok(format!("saves/{world_name}"));
    }
    let project_type = managed_project_type(&download.project_type)?;
    let target_folder = manual_download_target_folder(download, project_type)?;
    let file_name =
        crate::util::io::sanitize_download_file_name(&download.file_name)?;
    let state = State::get().await?;
    let localized_candidate = if project_type == ProjectType::Mod {
        None
    } else {
        chinese_file_title_for_curseforge_slug(&download.project_slug)
            .and_then(|title| localized_content_file_name(&file_name, &title))
            .map(|file_name| format!("{target_folder}/{file_name}"))
    };
    let relative_path = crate::state::resolve_content_install_relative_path(
        instance_id,
        format!("{target_folder}/{file_name}"),
        localized_candidate,
        &state.pool,
    )
    .await?;
    let full_path = crate::api::instance::get_full_path(instance_id)
        .await?
        .join(&relative_path);
    let previous_path =
        crate::state::materialize_verified_project_download_copy(
            source_path,
            &full_path,
            size,
            sha1,
        )
        .await?;
    let record_result =
        crate::state::record_verified_curseforge_project_file_atomic(
            instance_id,
            &relative_path,
            sha1,
            size,
            project_type,
            ContentSourceKind::CurseForge,
            download.ownership_kind,
            CurseForgeProjectId::new(download.project_id)?,
            CurseForgeFileId::new(download.file_id)?,
            true,
            &state,
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
    tracing::info!(
        instance_id,
        project_id = download.project_id,
        file_id = download.file_id,
        relative_path = %relative_path,
        "Completed CurseForge pending manual download record"
    );
    if download.ownership_kind
        == crate::state::instances::ContentOwnershipKind::PackManaged
        && download.operation_kind
            == crate::state::instances::ManualDownloadOperationKind::ContentUpdate
    {
        mark_pack_member_version_override(instance_id, &relative_path).await?;
    }
    Ok(relative_path)
}

async fn mark_pack_member_version_override(
    instance_id: &str,
    relative_path: &str,
) -> crate::Result<()> {
    let state = State::get().await?;
    let _instance_lock = state.lock_instance_content(instance_id).await;
    let content_set = crate::state::instances::adapters::sqlite::content_rows::get_applied_content_set(
        instance_id,
        &state.pool,
    )
    .await?
    .ok_or_else(|| {
        ErrorKind::InputError(
            "Instance has no applied content set".to_string(),
        )
    })?;
    let mut tx = state.pool.begin_with("BEGIN IMMEDIATE").await?;
    let updated = sqlx::query(
        "UPDATE instance_pack_members
         SET override_kind = 'version', modified_at = ?
         WHERE content_entry_id IN (
            SELECT entry.id
            FROM instance_content_entries entry
            INNER JOIN instance_files file ON file.id = entry.file_id
            WHERE entry.content_set_id = ? AND file.relative_path = ?
         )",
    )
    .bind(chrono::Utc::now().timestamp())
    .bind(&content_set.id)
    .bind(relative_path)
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() > 0 {
        crate::state::instances::adapters::sqlite::content_rows::bump_content_set_revision_in_transaction(
            &content_set.id,
            &mut tx,
        )
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

fn manual_download_target_folder(
    download: &CurseForgeManualDownload,
    project_type: ProjectType,
) -> crate::Result<String> {
    let target_folder = if download.target_folder.is_empty() {
        project_type.get_folder()
    } else {
        &download.target_folder
    };
    if target_folder == project_type.get_folder() {
        return Ok(target_folder.to_string());
    }
    let components = Path::new(target_folder).components().collect::<Vec<_>>();
    if project_type == ProjectType::DataPack
        && components.len() == 3
        && matches!(components[0], Component::Normal(value) if value == "saves")
        && matches!(components[1], Component::Normal(value) if !value.is_empty())
        && matches!(components[2], Component::Normal(value) if value == "datapacks")
    {
        return Ok(target_folder.to_string());
    }
    Err(ErrorKind::InputError(
        "CurseForge manual download has an invalid target folder".to_string(),
    )
    .into())
}

pub(crate) async fn select_latest_compatible_file(
    project_id: u32,
    game_version: Option<String>,
    mod_loader_type: Option<u32>,
    release_channel: Option<ReleaseChannel>,
) -> crate::Result<Option<CurseForgeFile>> {
    let response = get_files(
        project_id,
        CurseForgeFilesRequest {
            game_version,
            mod_loader_type,
            game_version_type_id: None,
            index: 0,
            page_size: MAX_PAGE_SIZE,
        },
    )
    .await?;

    Ok(response.files.into_iter().find(|file| {
        file.is_available
            && match release_channel {
                Some(ReleaseChannel::Release) => file.release_type == 1,
                Some(ReleaseChannel::Beta) => file.release_type <= 2,
                Some(ReleaseChannel::Alpha) | None => true,
            }
    }))
}

/// Why a dependency file was selected. This makes fallback provenance
/// explicit rather than allowing it to look like a native declaration.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CurseForgeDependencySelectionReason {
    NativeStrictMatch,
    Sha1VerifiedModrinthFallback,
}

/// A CurseForge file selected for a dependency after local target validation.
struct SelectedDependencyFile {
    file: CurseForgeFile,
    reason: CurseForgeDependencySelectionReason,
}

#[cfg(debug_assertions)]
fn log_dependency_file_selection(
    project_id: u32,
    target_game_version: &str,
    target_mod_loader_type: Option<u32>,
    source: &'static str,
    file: &CurseForgeFile,
) {
    tracing::warn!(
        project_id,
        target_game_version,
        ?target_mod_loader_type,
        source,
        selected_file_id = file.id,
        selected_file_name = %file.file_name,
        game_versions = ?file.game_versions,
        sortable_game_versions = ?file.sortable_game_versions,
        "CurseForge dependency file selected"
    );
}

fn dependency_target_game_version(game_version: Option<&str>) -> Option<&str> {
    game_version.filter(|version| !version.trim().is_empty())
}

async fn select_dependency_file(
    project_id: u32,
    game_version: Option<String>,
    mod_loader_type: Option<u32>,
) -> crate::Result<Option<SelectedDependencyFile>> {
    let Some(target_game_version) =
        dependency_target_game_version(game_version.as_deref())
    else {
        #[cfg(debug_assertions)]
        tracing::warn!(
            project_id,
            ?mod_loader_type,
            "CurseForge dependency target has no Minecraft version"
        );
        return Ok(None);
    };
    let filtered = get_files(
        project_id,
        CurseForgeFilesRequest {
            game_version: game_version.clone(),
            mod_loader_type,
            game_version_type_id: None,
            index: 0,
            page_size: MAX_PAGE_SIZE,
        },
    )
    .await?
    .files;
    if let Some(file) = select_best_dependency_file(
        filtered,
        Some(target_game_version),
        mod_loader_type,
    ) {
        #[cfg(debug_assertions)]
        log_dependency_file_selection(
            project_id,
            target_game_version,
            mod_loader_type,
            "filtered",
            &file,
        );
        return Ok(Some(SelectedDependencyFile {
            file,
            reason: CurseForgeDependencySelectionReason::NativeStrictMatch,
        }));
    }
    let unfiltered = get_files(
        project_id,
        CurseForgeFilesRequest {
            game_version: None,
            mod_loader_type: None,
            game_version_type_id: None,
            index: 0,
            page_size: MAX_PAGE_SIZE,
        },
    )
    .await?
    .files;
    let selected = select_best_dependency_file(
        unfiltered,
        Some(target_game_version),
        mod_loader_type,
    );
    #[cfg(debug_assertions)]
    if let Some(file) = selected.as_ref() {
        log_dependency_file_selection(
            project_id,
            target_game_version,
            mod_loader_type,
            "unfiltered",
            file,
        );
    }
    Ok(selected.map(|file| SelectedDependencyFile {
        file,
        reason: CurseForgeDependencySelectionReason::NativeStrictMatch,
    }))
}

async fn resolve_modrinth_fallback_plan(
    file: &CurseForgeFile,
    item_type: ProjectType,
    request: &CurseForgeInstallRequest,
    state: &State,
) -> crate::Result<Option<content_management::ResolveContentPlan>> {
    let Some(sha1) = file
        .hashes
        .iter()
        .find(|hash| hash.algo == 1)
        .map(|hash| hash.value.as_str())
    else {
        return Ok(None);
    };
    let (Some(game_version), Some(loader_type)) =
        (request.game_version.as_deref(), request.mod_loader_type)
    else {
        return Ok(None);
    };
    let Some(loader) = mod_loader_from_curseforge_type(loader_type) else {
        return Ok(None);
    };

    let matches = CachedEntry::get_file_many(
        &[sha1],
        Some(CacheBehaviour::MustRevalidate),
        &state.pool,
        &state.api_semaphore,
    )
    .await?;
    let Some(file_match) = matches.into_iter().next() else {
        return Ok(None);
    };
    let version = CachedEntry::get_version(
        &ModrinthVersionId::new(file_match.version_id)?,
        Some(CacheBehaviour::MustRevalidate),
        &state.pool,
        &state.api_semaphore,
    )
    .await?;
    let Some(version) = version else {
        return Ok(None);
    };
    if !modrinth_version_matches_target(&version, game_version, loader) {
        return Ok(None);
    }

    crate::state::instances::commands::resolve_install_plan_for_target_with_policy(
        crate::state::instances::commands::InstanceInstallProjectRequest {
            project_id: version.project_id,
            version_id: Some(version.id),
            content_type: item_type.into(),
            selected: Default::default(),
            excluded_project_ids: Vec::new(),
            force_project_ids: Vec::new(),
        },
        game_version.to_string(),
        loader,
        dependency_policy(request.manual_operation_kind),
        state,
    )
    .await
    .map(Some)
}

fn modrinth_version_matches_target(
    version: &crate::state::Version,
    game_version: &str,
    loader: ModLoader,
) -> bool {
    version
        .game_versions
        .iter()
        .any(|value| value == game_version)
        && version
            .loaders
            .iter()
            .any(|value| value.eq_ignore_ascii_case(loader.as_str()))
}

fn mod_loader_from_curseforge_type(mod_loader_type: u32) -> Option<ModLoader> {
    match mod_loader_type {
        1 => Some(ModLoader::Forge),
        4 => Some(ModLoader::Fabric),
        5 => Some(ModLoader::Quilt),
        6 => Some(ModLoader::NeoForge),
        _ => None,
    }
}

async fn preview_modrinth_fallbacks(
    plan: &content_management::ResolveContentPlan,
    parent_project_id: u32,
    state: &State,
) -> crate::Result<Vec<CurseForgeModrinthFallbackPreview>> {
    let mut preview = Vec::new();
    for dependency in &plan.dependencies {
        let version = CachedEntry::get_version(
            &ModrinthVersionId::new(dependency.version_id.clone())?,
            Some(CacheBehaviour::MustRevalidate),
            &state.pool,
            &state.api_semaphore,
        )
        .await?;
        let Some(version) = version else {
            continue;
        };
        let project = CachedEntry::get_project(
            &ModrinthProjectId::new(dependency.project_id.clone())?,
            Some(CacheBehaviour::MustRevalidate),
            &state.pool,
            &state.api_semaphore,
        )
        .await?;
        preview.push(CurseForgeModrinthFallbackPreview {
            project_id: dependency.project_id.clone(),
            version_id: dependency.version_id.clone(),
            title: project
                .as_ref()
                .map(|project| project.title.clone())
                .unwrap_or_else(|| dependency.project_id.clone()),
            version_number: version.version_number,
            parent_project_id,
            icon_url: project.and_then(|project| project.icon_url),
            required: dependency.required,
        });
    }
    Ok(preview)
}

async fn append_modrinth_fallback_plan(
    resolution_plan: &mut DependencyResolutionPlan,
    fallback_plan: &content_management::ResolveContentPlan,
    curseforge_parent: &ContentProviderRef,
    state: &State,
) -> crate::Result<()> {
    for dependency in &fallback_plan.dependencies {
        let content = ContentProviderRef::Modrinth {
            project_id: ModrinthProjectId::new(dependency.project_id.clone())?,
            version_id: Some(ModrinthVersionId::new(
                dependency.version_id.clone(),
            )?),
        };
        let parent = match dependency.dependent_on_version_id.as_deref() {
            Some(version_id)
                if version_id == fallback_plan.primary.version_id =>
            {
                curseforge_parent.clone()
            }
            Some(version_id) => {
                let Some(parent) = fallback_plan
                    .dependencies
                    .iter()
                    .find(|candidate| candidate.version_id == version_id)
                else {
                    resolution_plan.issues.push(DependencyResolutionIssue {
						provider: ContentProvider::Modrinth,
						project_id: dependency.project_id.clone(),
						parent: Some(curseforge_parent.clone()),
						relation: Some(
							crate::state::instances::ContentDependencyKind::Required,
						),
						reason: "dependency_parent_unresolved".to_string(),
					});
                    continue;
                };
                ContentProviderRef::Modrinth {
                    project_id: ModrinthProjectId::new(
                        parent.project_id.clone(),
                    )?,
                    version_id: Some(ModrinthVersionId::new(
                        parent.version_id.clone(),
                    )?),
                }
            }
            None => curseforge_parent.clone(),
        };
        let version = CachedEntry::get_version(
            &ModrinthVersionId::new(dependency.version_id.clone())?,
            Some(CacheBehaviour::MustRevalidate),
            &state.pool,
            &state.api_semaphore,
        )
        .await?;
        let Some(version) = version else {
            resolution_plan.issues.push(DependencyResolutionIssue {
                provider: ContentProvider::Modrinth,
                project_id: dependency.project_id.clone(),
                parent: Some(parent),
                relation: Some(
                    crate::state::instances::ContentDependencyKind::Required,
                ),
                reason: "missing_version".to_string(),
            });
            continue;
        };
        let file = version
            .files
            .iter()
            .find(|file| file.primary)
            .or_else(|| version.files.first());
        resolution_plan.edges.push(DependencyResolutionEdge {
            parent: parent.clone(),
            child: content.clone(),
            relation: crate::state::instances::ContentDependencyKind::Required,
            evidence_provider: ContentProvider::Modrinth,
        });
        if !resolution_plan
            .nodes
            .iter()
            .any(|node| node.content == content)
        {
            resolution_plan.nodes.push(DependencyResolutionNode {
                content,
                parent: Some(parent),
                relation:
                    crate::state::instances::ContentDependencyKind::Required,
                source: ContentProvider::Modrinth,
                selection_reason:
                    DependencySelectionReason::Sha1VerifiedModrinthFallback,
                expected_sha1: file
                    .and_then(|file| file.hashes.get("sha1").cloned()),
                expected_size: file.map(|file| file.size as u64),
            });
        }
    }
    Ok(())
}

fn curseforge_content_ref(
    project_id: u32,
    file_id: u32,
) -> crate::Result<ContentProviderRef> {
    Ok(ContentProviderRef::CurseForge {
        project_id: CurseForgeProjectId::new(project_id)?,
        file_id: Some(CurseForgeFileId::new(file_id)?),
    })
}

fn curseforge_file_sha1(file: &CurseForgeFile) -> Option<String> {
    file.hashes
        .iter()
        .find(|hash| hash.algo == 1)
        .map(|hash| hash.value.clone())
}

async fn instance_content_revision(
    instance_id: &str,
    state: &State,
) -> crate::Result<Option<u64>> {
    if instance_id.is_empty() {
        return Ok(None);
    }
    Ok(
		crate::state::instances::adapters::sqlite::content_rows::get_applied_content_set(
			instance_id,
			&state.pool,
		)
		.await?
		.map(|content_set| content_set.revision),
	)
}

fn store_dependency_resolution_plan(plan: DependencyResolutionPlan) {
    let now = Instant::now();
    DEPENDENCY_RESOLUTION_PLANS.retain(|_, cached| cached.expires_at > now);
    DEPENDENCY_RESOLUTION_PLANS.insert(
        plan.id.clone(),
        CachedDependencyResolutionPlan {
            plan,
            expires_at: now + DEPENDENCY_PLAN_TTL,
        },
    );
}

impl From<CurseForgeDependencySelectionReason> for DependencySelectionReason {
    fn from(reason: CurseForgeDependencySelectionReason) -> Self {
        match reason {
			CurseForgeDependencySelectionReason::NativeStrictMatch => {
				Self::NativeStrictMatch
			}
			CurseForgeDependencySelectionReason::Sha1VerifiedModrinthFallback => {
				Self::Sha1VerifiedModrinthFallback
			}
		}
    }
}

fn select_best_dependency_file(
    files: Vec<CurseForgeFile>,
    game_version: Option<&str>,
    mod_loader_type: Option<u32>,
) -> Option<CurseForgeFile> {
    files
        .into_iter()
        .filter(|file| {
            file.is_available
                && file_matches_dependency_target(
                    file,
                    game_version,
                    mod_loader_type,
                )
        })
        .max_by(|left, right| {
            dependency_release_rank(left)
                .cmp(&dependency_release_rank(right))
                .then_with(|| left.file_date.cmp(&right.file_date))
                .then_with(|| left.id.cmp(&right.id))
        })
}

fn dependency_release_rank(file: &CurseForgeFile) -> u8 {
    match file.release_type {
        1 => 3,
        2 => 2,
        3 => 1,
        _ => 0,
    }
}

/// Checks unfiltered fallback candidates against the instance target.
fn file_matches_dependency_target(
    file: &CurseForgeFile,
    game_version: Option<&str>,
    mod_loader_type: Option<u32>,
) -> bool {
    let game_version_matches = game_version.is_none_or(|target| {
        let game_versions_match =
            file.game_versions.iter().any(|version| version == target);
        let sortable_game_versions_match =
            file.sortable_game_versions.iter().any(|version| {
                version.game_version.as_deref() == Some(target)
                    || version.game_version_name == target
            });

        match (
            file.game_versions.is_empty(),
            file.sortable_game_versions.is_empty(),
        ) {
            (true, true) => false,
            (false, true) => game_versions_match,
            (true, false) => sortable_game_versions_match,
            (false, false) => {
                game_versions_match && sortable_game_versions_match
            }
        }
    });
    let mod_loader_matches = mod_loader_type.is_none_or(|target| {
        file.game_versions.iter().any(|version| {
            version.eq_ignore_ascii_case(mod_loader_to_slug(target))
        })
    });

    game_version_matches && mod_loader_matches
}

fn modpack_target_folder(project_type: &str) -> crate::Result<String> {
    if project_type == "world" {
        Ok("saves".into())
    } else {
        content_target_folder(managed_project_type(project_type)?, None)
    }
}

async fn install_modpack_world_file(
    instance_id: &str,
    project: &CurseForgeProject,
    file: &CurseForgeFile,
    reporter: Option<&InstallProgressReporter>,
    operation: crate::state::instances::ManualDownloadOperationKind,
) -> crate::Result<CurseForgeInstallResult> {
    validate_world_archive_name(&file.file_name)?;
    if project.class_id != Some(17) || file.mod_id != project.id {
        return Err(ErrorKind::InputError(
            "World file does not belong to the selected world project".into(),
        )
        .into());
    }
    let world_name = Path::new(&file.file_name)
        .file_stem()
        .unwrap()
        .to_string_lossy();
    let path = format!("saves/{world_name}");
    let target = crate::api::instance::get_full_path(instance_id)
        .await?
        .join(&path);
    let world = if target.join("level.dat").is_file() {
        // Resuming a pack or changing its version must preserve the player's save.
        CurseForgeWorldInstallResult {
            world_name: Some(world_name.into_owned()),
            manual_download: None,
        }
    } else {
        install_world_file(
            instance_id,
            project,
            file,
            reporter,
            crate::state::instances::ContentOwnershipKind::UserAdded,
            operation,
        )
        .await?
    };
    Ok(CurseForgeInstallResult {
        installed: world
            .world_name
            .map(|name| CurseForgeInstalledFile {
                project_id: project.id,
                file_id: file.id,
                relative_path: format!("saves/{name}"),
                dependency: false,
            })
            .into_iter()
            .collect(),
        manual_downloads: world.manual_download.into_iter().collect(),
        ..Default::default()
    })
}

fn managed_project_type(value: &str) -> crate::Result<ProjectType> {
    match value {
        "mod" => Ok(ProjectType::Mod),
        "datapack" => Ok(ProjectType::DataPack),
        "resourcepack" => Ok(ProjectType::ResourcePack),
        "shader" | "shaderpack" => Ok(ProjectType::ShaderPack),
        other => Err(ErrorKind::InputError(format!(
            "CurseForge project type {other} uses its dedicated installer"
        ))
        .into()),
    }
}

fn content_target_folder(
    project_type: ProjectType,
    world_name: Option<&str>,
) -> crate::Result<String> {
    if project_type == ProjectType::DataPack
        && let Some(world_name) = world_name
    {
        validate_file_name(world_name)?;
        Ok(format!("saves/{world_name}/datapacks"))
    } else {
        Ok(project_type.get_folder().to_string())
    }
}

fn manual_download_from_file(
    project_id: u32,
    file_id: u32,
    file: &CurseForgeFile,
    project: &CurseForgeProject,
    project_type: &str,
    target_folder: String,
    ownership_kind: crate::state::instances::ContentOwnershipKind,
    operation_kind: crate::state::instances::ManualDownloadOperationKind,
) -> CurseForgeManualDownload {
    CurseForgeManualDownload {
        project_id,
        file_id,
        file_name: file.file_name.clone(),
        ownership_kind,
        operation_kind,
        website_url: curseforge_file_page_url(
            project.links.website_url.as_deref(),
            file_id,
        ),
        project_type: project_type.to_string(),
        project_slug: project.slug.clone(),
        target_folder,
        hashes: file.hashes.clone(),
        file_length: file.file_length,
        file_fingerprint: file.file_fingerprint,
    }
}

async fn persist_manual_modpack_archive(
    instance_id: &str,
    download: &CurseForgeManualDownload,
) -> crate::Result<()> {
    let state = State::get().await?;
    let _instance_lock = state.lock_instance_content(instance_id).await;
    let content_set = crate::state::instances::adapters::sqlite::content_rows::get_applied_content_set(
		instance_id,
		&state.pool,
	)
	.await?
	.ok_or_else(|| {
		ErrorKind::InputError(
			"Instance has no applied content set".to_string(),
		)
	})?;
    let now = chrono::Utc::now();
    let mut tx = state.pool.begin_with("BEGIN IMMEDIATE").await?;
    crate::state::instances::adapters::sqlite::content_rows::upsert_pending_manual_download_in_transaction(
		&crate::state::instances::PendingManualDownload {
			id: format!("manual-download:{}", uuid::Uuid::new_v4()),
			instance_id: instance_id.to_string(),
			pack_member_id: None,
			content_entry_id: None,
			operation_kind: download.operation_kind,
			operation_target_id: Some(download.file_id.to_string()),
			project_type: ProjectType::Mod,
			provider: ContentProvider::CurseForge,
			provider_project_id: download.project_id.to_string(),
			provider_release_id: download.file_id.to_string(),
			file_name: download.file_name.clone(),
			website_url: download.website_url.clone(),
			target_relative_path: download.file_name.clone(),
			expected_sha1: download
				.hashes
				.iter()
				.find(|hash| hash.algo == 1)
				.map(|hash| hash.value.clone()),
			expected_size: (download.file_length > 0)
				.then_some(download.file_length),
			expected_fingerprint: (download.file_fingerprint > 0)
				.then_some(download.file_fingerprint),
			state: crate::state::instances::ManualDownloadState::Waiting,
			context: serde_json::to_value(download)?,
			created_at: now,
			modified_at: now,
		},
		&mut tx,
	)
	.await?;
    crate::state::instances::adapters::sqlite::content_rows::bump_content_set_revision_in_transaction(
		&content_set.id,
		&mut tx,
	)
	.await?;
    tx.commit().await?;
    Ok(())
}

async fn persist_manual_world_archive(
    instance_id: &str,
    download: &CurseForgeManualDownload,
) -> crate::Result<()> {
    validate_world_archive_name(&download.file_name)?;
    let state = State::get().await?;
    let _instance_lock = state.lock_instance_content(instance_id).await;
    let now = chrono::Utc::now();
    let mut tx = state.pool.begin_with("BEGIN IMMEDIATE").await?;
    crate::state::instances::adapters::sqlite::content_rows::upsert_pending_manual_download_in_transaction(
		&crate::state::instances::PendingManualDownload {
			id: format!("manual-download:{}", uuid::Uuid::new_v4()),
			instance_id: instance_id.to_string(),
			pack_member_id: None,
			content_entry_id: None,
			operation_kind: download.operation_kind,
			operation_target_id: None,
			project_type: ProjectType::WorldSave,
			provider: ContentProvider::CurseForge,
			provider_project_id: download.project_id.to_string(),
			provider_release_id: download.file_id.to_string(),
			file_name: download.file_name.clone(),
			website_url: download.website_url.clone(),
			target_relative_path: format!("saves/{}", download.file_name),
			expected_sha1: download
				.hashes
				.iter()
				.find(|hash| hash.algo == 1)
				.map(|hash| hash.value.clone()),
			expected_size: (download.file_length > 0)
				.then_some(download.file_length),
			expected_fingerprint: (download.file_fingerprint > 0)
				.then_some(download.file_fingerprint),
			state: crate::state::instances::ManualDownloadState::Waiting,
			context: serde_json::to_value(download)?,
			created_at: now,
			modified_at: now,
		},
		&mut tx,
	)
	.await?;
    tx.commit().await?;
    Ok(())
}

async fn complete_manual_world_download(
    instance_id: &str,
    download: &CurseForgeManualDownload,
) -> crate::Result<()> {
    let state = State::get().await?;
    let _instance_lock = state.lock_instance_content(instance_id).await;
    let mut tx = state.pool.begin_with("BEGIN IMMEDIATE").await?;
    crate::state::instances::adapters::sqlite::content_rows::complete_pending_manual_download(
		instance_id,
		&download.project_id.to_string(),
		&download.file_id.to_string(),
		None,
		&mut tx,
	)
	.await?;
    tx.commit().await?;
    Ok(())
}

async fn persist_manual_download(
    instance_id: &str,
    download: &CurseForgeManualDownload,
) -> crate::Result<()> {
    if download.project_type == "modpack" {
        return persist_manual_modpack_archive(instance_id, download).await;
    }
    if download.project_type == "world" {
        return persist_manual_world_archive(instance_id, download).await;
    }
    let project_type = managed_project_type(&download.project_type)?;
    let state = State::get().await?;
    let _instance_lock = state.lock_instance_content(instance_id).await;
    let content_set = crate::state::instances::adapters::sqlite::content_rows::get_applied_content_set(
        instance_id,
        &state.pool,
    )
    .await?
    .ok_or_else(|| {
        ErrorKind::InputError(
            "Instance has no applied content set".to_string(),
        )
    })?;
    let target_relative_path = format!(
        "{}/{}",
        download.target_folder.trim_end_matches('/'),
        download.file_name
    );
    let member_key = format!(
        "curseforge:{}:{}",
        download.project_id,
        project_type.get_name()
    );
    let mut tx = state.pool.begin_with("BEGIN IMMEDIATE").await?;
    let now = chrono::Utc::now();
    let pack_member_id = if download.ownership_kind
        == crate::state::instances::ContentOwnershipKind::PackManaged
    {
        let existing_id = sqlx::query_scalar::<_, String>(
            "SELECT id FROM instance_pack_members
             WHERE content_set_id = ? AND member_key = ?",
        )
        .bind(&content_set.id)
        .bind(&member_key)
        .fetch_optional(&mut *tx)
        .await?;
        if download.operation_kind
            == crate::state::instances::ManualDownloadOperationKind::PackUpdate
        {
            existing_id
        } else {
            let member_id = existing_id.unwrap_or_else(|| {
                format!("pack-member:{}", uuid::Uuid::new_v4())
            });
            crate::state::instances::adapters::sqlite::content_rows::upsert_pack_member_in_transaction(
			&crate::state::instances::PackMember {
                id: member_id.clone(),
                content_set_id: content_set.id.clone(),
                content_entry_id: None,
                member_key,
                project_type,
                expected_relative_path: target_relative_path.clone(),
                provider: Some(ContentProvider::CurseForge),
                provider_project_id: Some(download.project_id.to_string()),
                provider_release_id: Some(download.file_id.to_string()),
                required: true,
                expected_sha1: download
                    .hashes
                    .iter()
                    .find(|hash| hash.algo == 1)
                    .map(|hash| hash.value.clone()),
                expected_size: (download.file_length > 0)
                    .then_some(download.file_length),
                expected_fingerprint: (download.file_fingerprint > 0)
                    .then_some(download.file_fingerprint),
                materialization_state: crate::state::instances::PackMemberMaterializationState::PendingManual,
                override_kind: crate::state::instances::PackMemberOverrideKind::None,
                reconciled: true,
                created_at: now,
                modified_at: now,
            },
			&mut tx,
		)
		.await?;
            Some(member_id)
        }
    } else {
        None
    };
    crate::state::instances::adapters::sqlite::content_rows::upsert_pending_manual_download_in_transaction(
        &crate::state::instances::PendingManualDownload {
            id: format!("manual-download:{}", uuid::Uuid::new_v4()),
            instance_id: instance_id.to_string(),
            pack_member_id,
            content_entry_id: None,
            operation_kind: download.operation_kind,
            operation_target_id: None,
            project_type,
            provider: ContentProvider::CurseForge,
            provider_project_id: download.project_id.to_string(),
            provider_release_id: download.file_id.to_string(),
            file_name: download.file_name.clone(),
            website_url: download.website_url.clone(),
            target_relative_path,
            expected_sha1: download
                .hashes
                .iter()
                .find(|hash| hash.algo == 1)
                .map(|hash| hash.value.clone()),
            expected_size: (download.file_length > 0)
                .then_some(download.file_length),
            expected_fingerprint: (download.file_fingerprint > 0)
                .then_some(download.file_fingerprint),
            state: crate::state::instances::ManualDownloadState::Waiting,
            context: serde_json::to_value(download)?,
            created_at: now,
            modified_at: now,
        },
        &mut tx,
    )
    .await?;
    crate::state::instances::adapters::sqlite::content_rows::bump_content_set_revision_in_transaction(
        &content_set.id,
        &mut tx,
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0usize;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

async fn report_modpack_progress(
    loading_bar: Option<&crate::event::LoadingBarId>,
    reporter: Option<&InstallProgressReporter>,
    details: InstallPhaseDetails,
    files_done: &AtomicU64,
    bytes_done: &AtomicU64,
    active_downloads: &AtomicU64,
    total_files: u64,
    total_bytes: u64,
    file_bytes: u64,
    event: InstallJobEventKind,
) -> crate::Result<()> {
    let current_files = files_done.fetch_add(1, Ordering::Relaxed) + 1;
    let current_bytes =
        bytes_done.fetch_add(file_bytes, Ordering::Relaxed) + file_bytes;
    let active = active_downloads.load(Ordering::Relaxed);
    let message = if total_bytes > 0 {
        format!(
            "{current_files}/{total_files} files · {} / {} · {active} downloading in parallel",
            format_bytes(current_bytes.min(total_bytes)),
            format_bytes(total_bytes)
        )
    } else {
        format!(
            "{current_files}/{total_files} files · {active} downloading in parallel"
        )
    };
    if let Some(loading_bar) = loading_bar {
        emit_loading(loading_bar, 1.0, Some(&message))?;
    }
    if let Some(reporter) = reporter {
        reporter
            .update_with_events(
                InstallPhaseId::DownloadingContent,
                Some(InstallProgress {
                    current: current_files,
                    total: total_files,
                    secondary: Some(InstallProgressSecondary {
                        current: current_bytes.min(total_bytes),
                        total: total_bytes,
                    }),
                }),
                details,
                vec![event],
            )
            .await?;
    }
    Ok(())
}

fn is_forge_cdn_url(url: &reqwest::Url) -> bool {
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    host == "forgecdn.net" || host.ends_with(".forgecdn.net")
}

fn curseforge_file_page_url(
    website_url: Option<&str>,
    file_id: u32,
) -> Option<String> {
    let website_url = website_url?;
    let Ok(mut url) = reqwest::Url::parse(website_url) else {
        return Some(website_url.to_owned());
    };
    if !matches!(
        url.host_str(),
        Some("curseforge.com" | "www.curseforge.com" | "legacy.curseforge.com")
    ) {
        return Some(website_url.to_owned());
    }

    let mut path = url.path().trim_end_matches('/').to_string();
    for marker in ["/files/", "/download/"] {
        if let Some(index) = path.rfind(marker) {
            path.truncate(index);
            break;
        }
    }
    url.set_path(&format!("{path}/download/{file_id}"));
    url.set_query(None);
    url.set_fragment(None);
    Some(url.into())
}

fn validate_cdn_url(url: &reqwest::Url) -> crate::Result<()> {
    #[cfg(debug_assertions)]
    if url.scheme() == "http"
        && matches!(
            url.host_str()
                .unwrap_or_default()
                .to_ascii_lowercase()
                .as_str(),
            "127.0.0.1" | "localhost"
        )
    {
        return Ok(());
    }
    if url.scheme() != "https" || !is_forge_cdn_url(url) {
        return Err(ErrorKind::InputError(
            "CurseForge returned a download URL outside its CDN".to_string(),
        )
        .into());
    }
    Ok(())
}

fn curseforge_content_validation(file_name: &str) -> ContentValidation {
    match Path::new(file_name)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("jar" | "zip" | "mrpack") => ContentValidation::Jar,
        _ => ContentValidation::None,
    }
}

fn curseforge_integrity(
    file: &CurseForgeFile,
    validation: ContentValidation,
) -> Integrity {
    Integrity {
        size: Some(file.file_length),
        sha1: file
            .hashes
            .iter()
            .find(|hash| hash.algo == 1)
            .map(|hash| hash.value.clone()),
        md5: file
            .hashes
            .iter()
            .find(|hash| hash.algo == 2)
            .map(|hash| hash.value.clone()),
        content: validation,
        ..Integrity::default()
    }
}

const fn curseforge_modpack_h2_range_concurrency(size: u64) -> Option<usize> {
    if size >= CURSEFORGE_H2_RANGE_THRESHOLD {
        Some(CURSEFORGE_REQUESTED_H2_RANGES)
    } else {
        None
    }
}

fn curseforge_candidate_urls(url: &str) -> crate::Result<Vec<String>> {
    let parsed = reqwest::Url::parse(url).map_err(|_| {
        ErrorKind::InputError(
            "CurseForge returned an invalid download URL".to_string(),
        )
    })?;
    validate_cdn_url(&parsed)?;
    if !is_forge_cdn_url(&parsed) {
        return Ok(Vec::new());
    }

    let original_host = parsed.host_str().unwrap_or_default();
    let mut candidates = Vec::new();
    for host in [
        "edge.forgecdn.net",
        "media.forgecdn.net",
        "mediafilez.forgecdn.net",
    ] {
        if host == original_host {
            continue;
        }
        let mut candidate = parsed.clone();
        candidate.set_host(Some(host)).map_err(|_| {
            ErrorKind::InputError(
                "CurseForge returned an invalid CDN URL".to_string(),
            )
        })?;
        candidates.push(candidate.to_string());
    }
    Ok(candidates)
}

async fn download_curseforge_path(
    url: &str,
    file: &CurseForgeFile,
    destination: &Path,
    validation: ContentValidation,
    progress: Option<&mut FetchProgressFn<'_>>,
    tracking: Option<(&InstallProgressReporter, &str)>,
    h2_range_concurrency: Option<usize>,
    allow_http1_segmented_download: bool,
) -> crate::Result<crate::util::fetch::DownloadResult> {
    let state = State::get().await?;
    let mut request = DownloadRequest::new(url, ResourceClass::CurseForge)
        .with_candidate_urls(curseforge_candidate_urls(url)?)
        .with_integrity(curseforge_integrity(file, validation))
        .with_http1_segmented_download(allow_http1_segmented_download);
    if let Some(concurrency) = h2_range_concurrency {
        request = request.with_h2_range_concurrency(concurrency);
    }
    let parsed = reqwest::Url::parse(url)?;
    if is_forge_cdn_url(&parsed)
        && let Some(key) = api_key()
    {
        request = request.with_header("x-api-key", key);
    }
    if let Some((reporter, item_id)) = tracking {
        request = request.with_install_tracking(
            reporter.clone(),
            item_id,
            file.file_name.clone(),
        );
    }
    download_to_path(
        request,
        destination,
        &state.download_semaphore,
        &state.pool,
        progress,
    )
    .await
}

async fn download_curseforge_archive(
    project_id: u32,
    file_id: u32,
    file: &CurseForgeFile,
    url: &str,
    progress: Option<&mut FetchProgressFn<'_>>,
    reporter: Option<&InstallProgressReporter>,
) -> crate::Result<crate::util::fetch::DownloadResult> {
    validate_file_name(&file.file_name)?;
    let state = State::get().await?;
    let path = state
        .directories
        .caches_dir()
        .join("curseforge")
        .join("modpacks")
        .join(project_id.to_string())
        .join(file_id.to_string())
        .join(crate::util::io::sanitize_download_file_name(
            &file.file_name,
        )?);
    let tracking_item_id = path.display().to_string();
    download_curseforge_path(
        url,
        file,
        &path,
        ContentValidation::Jar,
        progress,
        reporter.map(|reporter| (reporter, tracking_item_id.as_str())),
        curseforge_modpack_h2_range_concurrency(file.file_length),
        true,
    )
    .await
}

struct DownloadInstalledFileRequest<'a> {
    instance_id: &'a str,
    url: &'a str,
    file: &'a CurseForgeFile,
    project_type: ProjectType,
    world_name: Option<&'a str>,
    project_id: u32,
    file_id: u32,
    project_slug: &'a str,
    ownership_kind: crate::state::instances::ContentOwnershipKind,
    download_metrics: Option<&'a CurseForgeDownloadMetrics>,
    defer_persistence: bool,
    verification_tx: Option<&'a mpsc::Sender<CurseForgeVerificationTask>>,
    pre_resolved_relative_path: Option<&'a str>,
    expected_file_name: Option<&'a str>,
}

struct DownloadedCurseForgeFile {
    relative_path: String,
}

pub(crate) struct CurseForgeVerificationTask {
    instance_id: String,
    relative_path: String,
    download_path: PathBuf,
    full_path: PathBuf,
    file: CurseForgeFile,
    project_type: ProjectType,
    ownership_kind: crate::state::instances::ContentOwnershipKind,
    expected_bytes: u64,
    download_result: Option<crate::util::fetch::DownloadResult>,
    cancellation: CancellationToken,
}

impl Drop for CurseForgeVerificationTask {
    fn drop(&mut self) {
        // A worker may stop after another file fails, leaving tasks buffered
        // in the channel. Those staged files are safe to remove and must not
        // accumulate in an existing instance after cancellation.
        let _ = std::fs::remove_file(&self.download_path);
    }
}

#[derive(Clone)]
struct CurseForgeVerificationContext {
    reporter: Option<InstallProgressReporter>,
    loading_bar: Option<Arc<crate::event::LoadingBarId>>,
    details: InstallPhaseDetails,
    files_done: Arc<AtomicU64>,
    bytes_done: Arc<AtomicU64>,
    active_downloads: Arc<AtomicU64>,
    total_files: u64,
    total_bytes: u64,
    cancellation: CancellationToken,
}

struct CurseForgeDatabaseTask {
    instance_id: String,
    record: crate::state::instances::commands::ProjectFileRecord,
    verified_pending: Option<(CurseForgeProjectId, CurseForgeFileId)>,
    expected_bytes: u64,
    full_path: PathBuf,
    previous_path: Option<PathBuf>,
}

struct ActiveCurseForgeDownload {
    counter: Arc<AtomicU64>,
    active: bool,
}

impl ActiveCurseForgeDownload {
    fn start(counter: Arc<AtomicU64>) -> Self {
        counter.fetch_add(1, Ordering::Relaxed);
        Self {
            counter,
            active: true,
        }
    }

    fn finish(&mut self) {
        if self.active {
            self.counter.fetch_sub(1, Ordering::Relaxed);
            self.active = false;
        }
    }
}

impl Drop for ActiveCurseForgeDownload {
    fn drop(&mut self) {
        self.finish();
    }
}

async fn remove_curseforge_staged_file(path: &Path) {
    if tokio::fs::try_exists(path).await.unwrap_or(false)
        && let Err(error) = crate::util::io::remove_file(path).await
    {
        tracing::warn!(
            path = %path.display(),
            %error,
            "Failed to remove staged CurseForge download"
        );
    }
}

async fn restore_curseforge_materialization(
    instance_id: &str,
    full_path: &Path,
    previous_path: Option<&Path>,
) {
    let Ok(state) = State::get().await else {
        return;
    };
    let _instance_lock = state.lock_instance_content(instance_id).await;
    if let Err(error) =
        crate::state::restore_project_materialization(full_path, previous_path)
            .await
    {
        tracing::error!(
            instance_id,
            path = %full_path.display(),
            %error,
            "Failed to restore CurseForge file after database failure"
        );
    }
}

async fn restore_curseforge_materializations(
    instance_id: &str,
    tasks: &[CurseForgeDatabaseTask],
) {
    for task in tasks.iter().rev() {
        restore_curseforge_materialization(
            instance_id,
            &task.full_path,
            task.previous_path.as_deref(),
        )
        .await;
    }
}

async fn finalize_curseforge_materializations(
    tasks: &[CurseForgeDatabaseTask],
) {
    for task in tasks {
        if let Err(error) = crate::state::finalize_project_materialization(
            task.previous_path.as_deref(),
        )
        .await
        {
            tracing::warn!(
                path = %task.full_path.display(),
                %error,
                "Failed to remove previous CurseForge file backup"
            );
        }
    }
}

async fn verify_and_record_curseforge_modpack_file(
    task: CurseForgeVerificationTask,
    database_tx: mpsc::Sender<CurseForgeDatabaseTask>,
) -> crate::Result<()> {
    if task.cancellation.is_cancelled() {
        remove_curseforge_staged_file(&task.download_path).await;
        return Err(ErrorKind::OtherError(
            "CurseForge modpack verification canceled".to_string(),
        )
        .into());
    }

    let state = State::get().await?;
    let verified = match verify_installed_curseforge_file(
        &task.download_path,
        &task.file,
        Some(&task.cancellation),
        task.download_result.as_ref(),
    )
    .await
    {
        Ok(verified) => verified,
        Err(error) => {
            remove_curseforge_staged_file(&task.download_path).await;
            return Err(error);
        }
    };
    if task.cancellation.is_cancelled() {
        remove_curseforge_staged_file(&task.download_path).await;
        return Err(ErrorKind::OtherError(
            "CurseForge modpack verification canceled".to_string(),
        )
        .into());
    }
    // Only materialization needs the per-instance lock. Hashing and database
    // persistence run in separate workers so verification does not serialize
    // unrelated content operations for the whole hash duration.
    let instance_lock = tokio::select! {
        biased;
        _ = task.cancellation.cancelled() => {
            remove_curseforge_staged_file(&task.download_path).await;
            return Err(ErrorKind::OtherError(
                "CurseForge materialization canceled".to_string(),
            ).into());
        }
        lock = state.lock_instance_content(&task.instance_id) => lock,
    };
    let previous_path = match crate::state::materialize_project_download(
        &task.download_path,
        &task.full_path,
    )
    .await
    {
        Ok(previous_path) => previous_path,
        Err(error) => {
            drop(instance_lock);
            remove_curseforge_staged_file(&task.download_path).await;
            return Err(error);
        }
    };
    drop(instance_lock);
    remove_curseforge_staged_file(&task.download_path).await;

    let project_id = CurseForgeProjectId::new(task.file.mod_id)?;
    let file_id = CurseForgeFileId::new(task.file.id)?;
    let record = crate::state::instances::commands::ProjectFileRecord {
        relative_path: task.relative_path.clone(),
        sha1: verified.sha1,
        size: verified.size,
        project_type: task.project_type,
        source_kind: ContentSourceKind::CurseForge,
        ownership_kind: task.ownership_kind,
        provider_ref: Some(ContentProviderRef::CurseForge {
            project_id,
            file_id: Some(file_id),
        }),
        origin: true,
        known_modrinth_project_id: None,
        known_modrinth_version_id: None,
    };
    let verified_pending = match verified.pending_completion {
        CurseForgePendingCompletionProof::None => None,
        CurseForgePendingCompletionProof::AuthoritativeSha1
        | CurseForgePendingCompletionProof::AuthoritativeFingerprint => {
            Some((project_id, file_id))
        }
    };
    let database_task = CurseForgeDatabaseTask {
        instance_id: task.instance_id.clone(),
        record,
        verified_pending,
        expected_bytes: task.expected_bytes,
        full_path: task.full_path.clone(),
        previous_path,
    };
    let restore_full_path = database_task.full_path.clone();
    let restore_previous_path = database_task.previous_path.clone();
    let send_result = tokio::select! {
        biased;
        _ = task.cancellation.cancelled() => Err(ErrorKind::OtherError(
            "CurseForge modpack database registration canceled".to_string(),
        ).into()),
        result = database_tx.send(database_task) => result.map_err(|_| ErrorKind::OtherError(
            "CurseForge modpack database worker stopped".to_string(),
        ).into()),
    };
    if let Err(error) = send_result {
        restore_curseforge_materialization(
            &task.instance_id,
            &restore_full_path,
            restore_previous_path.as_deref(),
        )
        .await;
        return Err(error);
    }
    Ok(())
}

fn spawn_curseforge_verification_worker(
    mut receiver: mpsc::Receiver<CurseForgeVerificationTask>,
    context: CurseForgeVerificationContext,
    database_tx: mpsc::Sender<CurseForgeDatabaseTask>,
) -> tokio::task::JoinHandle<crate::Result<()>> {
    tokio::spawn(async move {
        let cancellation = context.cancellation.clone();
        let worker_database_tx = database_tx.clone();
        let worker_reporter = context.reporter.clone();
        let result = crate::install::try_for_each_concurrent_draining(
            stream::poll_fn(move |cx| receiver.poll_recv(cx)),
            Some(MODPACK_VERIFICATION_CONCURRENCY),
            cancellation.clone(),
            move |task| {
                let reporter = worker_reporter.clone();
                let database_tx = worker_database_tx.clone();
                async move {
                    let result = verify_and_record_curseforge_modpack_file(
                        task,
                        database_tx,
                    )
                    .await;
                    if let (Some(reporter), Err(error)) = (&reporter, &result) {
                        reporter.fail_pipeline(error);
                    }
                    result
                }
            },
        )
        .await;
        if result.is_err() {
            // Stop network transfers and peers at the first verifier/SQLite
            // failure. This prevents a closed queue from turning every
            // remaining file into an independent retry/error storm.
            if let (Some(reporter), Err(error)) = (&context.reporter, &result) {
                reporter.fail_pipeline(error);
            }
            cancellation.cancel();
        }
        result
    })
}

async fn persist_curseforge_database_batch(
    batch: &[CurseForgeDatabaseTask],
    context: &CurseForgeVerificationContext,
) -> crate::Result<()> {
    if batch.is_empty() {
        return Ok(());
    }
    let instance_id = &batch[0].instance_id;
    if batch.iter().any(|task| task.instance_id != *instance_id) {
        return Err(ErrorKind::OtherError(
            "CurseForge database batch mixed multiple instances".to_string(),
        )
        .into());
    }
    let state = State::get().await?;
    if context.cancellation.is_cancelled() {
        restore_curseforge_materializations(instance_id, batch).await;
        return Err(ErrorKind::OtherError(
            "CurseForge modpack database registration canceled".to_string(),
        )
        .into());
    }
    let records = batch
        .iter()
        .map(|task| task.record.clone())
        .collect::<Vec<_>>();
    let verified_pending = batch
        .iter()
        .filter_map(|task| task.verified_pending)
        .collect::<Vec<_>>();
    // The permit wait is cancelable, but once the transaction starts we must
    // observe whether it committed before deciding to finalize or restore the
    // corresponding files. Dropping a COMMIT future on cancellation leaves
    // the filesystem/database outcome ambiguous.
    let database_permit = tokio::select! {
        biased;
        _ = context.cancellation.cancelled() => {
            restore_curseforge_materializations(instance_id, batch).await;
            return Err(ErrorKind::OtherError(
                "CurseForge modpack database registration canceled".to_string(),
            ).into());
        }
        permit = state.acquire_install_db_permit() => permit?,
    };
    let write_result: crate::Result<()> =
        crate::state::instances::commands::record_project_files_atomic_with_permit(
            instance_id,
            &records,
            &verified_pending,
            Some(database_permit),
            &state,
        )
        .await;
    if let Err(error) = write_result {
        restore_curseforge_materializations(instance_id, batch).await;
        return Err(error);
    }
    finalize_curseforge_materializations(batch).await;

    if context.cancellation.is_cancelled() {
        return Err(ErrorKind::OtherError(
            "CurseForge modpack database registration canceled".to_string(),
        )
        .into());
    }

    let batch_bytes = batch.iter().map(|task| task.expected_bytes).sum::<u64>();
    let batch_files = batch.len() as u64;
    let current_files =
        context.files_done.fetch_add(batch_files, Ordering::Relaxed)
            + batch_files;
    let current_bytes =
        context.bytes_done.fetch_add(batch_bytes, Ordering::Relaxed)
            + batch_bytes;
    let active = context.active_downloads.load(Ordering::Relaxed);
    let message = if context.total_bytes > 0 {
        format!(
            "{current_files}/{} files · {} / {} · {active} downloading in parallel",
            context.total_files,
            format_bytes(current_bytes.min(context.total_bytes)),
            format_bytes(context.total_bytes),
        )
    } else {
        format!(
            "{current_files}/{} files · {active} downloading in parallel",
            context.total_files,
        )
    };
    if let Some(loading_bar) = context.loading_bar.as_deref() {
        emit_loading(loading_bar, batch_files as f64, Some(&message))?;
    }
    if let Some(reporter) = context.reporter.as_ref() {
        reporter
            .update_with_events(
                InstallPhaseId::DownloadingContent,
                Some(InstallProgress {
                    current: current_files,
                    total: context.total_files,
                    secondary: Some(InstallProgressSecondary {
                        current: current_bytes.min(context.total_bytes),
                        total: context.total_bytes,
                    }),
                }),
                context.details.clone(),
                batch
                    .iter()
                    .map(|task| InstallJobEventKind::ContentFileCompleted {
                        path: task.record.relative_path.clone(),
                        bytes: task.expected_bytes,
                    })
                    .collect(),
            )
            .await?;
    }
    Ok(())
}

fn spawn_curseforge_database_worker(
    mut receiver: mpsc::Receiver<CurseForgeDatabaseTask>,
    context: CurseForgeVerificationContext,
) -> tokio::task::JoinHandle<crate::Result<()>> {
    tokio::spawn(async move {
        let cancellation = context.cancellation.clone();
        let result: crate::Result<()> = async {
            while let Some(batch) = receive_curseforge_database_batch(
                &mut receiver,
                &context.cancellation,
            )
            .await?
            {
                persist_curseforge_database_batch(&batch, &context).await?;
            }
            Ok(())
        }
        .await;
        if result.is_err() {
            if let (Some(reporter), Err(error)) = (&context.reporter, &result) {
                reporter.fail_pipeline(error);
            }
            while let Ok(task) = receiver.try_recv() {
                restore_curseforge_materialization(
                    &task.instance_id,
                    &task.full_path,
                    task.previous_path.as_deref(),
                )
                .await;
            }
            cancellation.cancel();
        }
        result
    })
}

async fn receive_curseforge_database_batch<T>(
    receiver: &mut mpsc::Receiver<T>,
    cancellation: &CancellationToken,
) -> crate::Result<Option<Vec<T>>> {
    let first = tokio::select! {
        biased;
        _ = cancellation.cancelled() => {
            return Err(ErrorKind::OtherError(
                "CurseForge modpack database worker canceled".to_string(),
            ).into());
        }
        task = receiver.recv() => match task {
            Some(task) => task,
            None => return Ok(None),
        },
    };
    let mut batch = Vec::with_capacity(MODPACK_DATABASE_BATCH_SIZE);
    batch.push(first);
    let deadline =
        tokio::time::Instant::now() + MODPACK_DATABASE_FLUSH_INTERVAL;
    while batch.len() < MODPACK_DATABASE_BATCH_SIZE {
        tokio::select! {
            biased;
            _ = cancellation.cancelled() => {
                // Return the partial batch so the persistence layer can
                // restore every file that was already materialized.
                break;
            }
            task = receiver.recv() => match task {
                Some(task) => batch.push(task),
                None => break,
            },
            _ = tokio::time::sleep_until(deadline) => break,
        }
    }
    Ok(Some(batch))
}

async fn download_installed_file(
    request: DownloadInstalledFileRequest<'_>,
) -> crate::Result<DownloadedCurseForgeFile> {
    let DownloadInstalledFileRequest {
        instance_id,
        url,
        file,
        project_type,
        world_name,
        project_id,
        file_id,
        project_slug,
        ownership_kind,
        download_metrics,
        defer_persistence,
        verification_tx,
        pre_resolved_relative_path,
        expected_file_name,
    } = request;
    if file.mod_id != project_id || file.id != file_id {
        return Err(ErrorKind::InputError(
            "CurseForge returned metadata for a different project or file"
                .to_string(),
        )
        .into());
    }
    let state = State::get().await?;
    let file_name =
        crate::util::io::sanitize_download_file_name(&file.file_name)?;
    let folder = content_target_folder(project_type, world_name)?;
    let localized_candidate = if project_type == ProjectType::Mod {
        None
    } else {
        chinese_file_title_for_curseforge_slug(project_slug)
            .and_then(|title| localized_content_file_name(&file_name, &title))
            .map(|file_name| format!("{folder}/{file_name}"))
    };
    let relative_path = match pre_resolved_relative_path {
        Some(path) => path.to_string(),
        None => {
            crate::state::resolve_content_install_relative_path(
                instance_id,
                format!("{folder}/{file_name}"),
                localized_candidate,
                &state.pool,
            )
            .await?
        }
    };
    let full_path = crate::api::instance::get_full_path(instance_id)
        .await?
        .join(&relative_path);
    if project_type == ProjectType::Mod
        && let Some(expected_file_name) = expected_file_name
        && Path::new(&relative_path).file_name()
            != Some(std::ffi::OsStr::new(
                &crate::util::io::sanitize_download_file_name(
                    expected_file_name,
                )?,
            ))
    {
        return Err(ErrorKind::OtherError(format!(
            "CurseForge install context mismatch before download: expected_file={expected_file_name} installed_path={relative_path}",
        ))
        .into());
    }
    let mut download_path = full_path.as_os_str().to_os_string();
    download_path.push(".installing.download");
    let download_path = Path::new(&download_path);
    let result = download_curseforge_path(
        url,
        file,
        download_path,
        curseforge_content_validation(&file.file_name),
        None,
        download_metrics
            .and_then(|metrics| metrics.reporter.as_ref())
            .map(|reporter| (reporter, relative_path.as_str())),
        None,
        false,
    )
    .await?;
    if let Some(download_metrics) = download_metrics {
        download_metrics.record(&result);
    }
    if defer_persistence {
        if let Some(sender) = verification_tx {
            let cancellation = download_metrics
                .and_then(|metrics| metrics.reporter.as_ref())
                .map(InstallProgressReporter::cancellation_token)
                .unwrap_or_default();
            let verification_task = CurseForgeVerificationTask {
                instance_id: instance_id.to_string(),
                relative_path: relative_path.clone(),
                download_path: download_path.to_path_buf(),
                full_path: full_path.clone(),
                file: file.clone(),
                project_type,
                ownership_kind,
                expected_bytes: file.file_length,
                download_result: Some(result.clone()),
                cancellation: cancellation.clone(),
            };
            tokio::select! {
                biased;
                _ = cancellation.cancelled() => {
                    return Err(ErrorKind::OtherError(
                        "curseforge verification enqueue canceled".to_string(),
                    ).into());
                }
                result = sender.send(verification_task) => {
                    result.map_err(|_| ErrorKind::OtherError(
                        "curseforge verification worker stopped".to_string(),
                    ))?;
                }
            }
        }
        return Ok(DownloadedCurseForgeFile { relative_path });
    }
    let verified = verify_installed_curseforge_file(
        download_path,
        file,
        None,
        Some(&result),
    )
    .await?;
    // Acquire the writer before the instance lock so a progress checkpoint
    // cannot leave this materialization holding the lock indefinitely.
    let database_permit = Some(state.acquire_install_db_permit().await?);
    let _instance_lock = state.lock_instance_content(instance_id).await;
    let previous_path =
        crate::state::materialize_project_download(download_path, &full_path)
            .await?;
    crate::util::io::remove_file(download_path).await?;
    let record_result = record_verified_curseforge_file_with_permit(
        instance_id,
        &relative_path,
        file,
        project_type,
        ownership_kind,
        database_permit,
        &state,
        verified,
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
    Ok(DownloadedCurseForgeFile { relative_path })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CurseForgePendingCompletionProof {
    None,
    AuthoritativeSha1,
    AuthoritativeFingerprint,
}

struct VerifiedInstalledCurseForgeFile {
    size: u64,
    sha1: String,
    pending_completion: CurseForgePendingCompletionProof,
}

async fn read_curseforge_verification_chunk(
    file: &mut tokio::fs::File,
    buffer: &mut [u8],
    cancellation: Option<&CancellationToken>,
) -> crate::Result<usize> {
    match cancellation {
        Some(cancellation) => tokio::select! {
            biased;
            _ = cancellation.cancelled() => Err(ErrorKind::OtherError(
                "CurseForge fingerprint verification canceled".to_string(),
            ).into()),
            result = file.read(buffer) => Ok(result?),
        },
        None => Ok(file.read(buffer).await?),
    }
}

async fn fingerprint_and_sha1_file(
    path: &Path,
    cancellation: Option<&CancellationToken>,
) -> crate::Result<(u64, String, u32)> {
    #[cfg(test)]
    crate::util::download::verified_file::record_scan();
    let started = std::time::Instant::now();
    let acquire = crate::util::fetch::acquire_native_validation_permit();
    let _permit = if let Some(cancellation) = cancellation {
        tokio::select! { biased; _ = cancellation.cancelled() => return Err(ErrorKind::OtherError("fingerprint verification canceled while waiting for resources".into()).into()), permit = acquire => permit? }
    } else {
        acquire.await?
    };

    use crate::util::download::constants::CURSEFORGE_VERIFY_BUFFER_SIZE as BUFFER_SIZE;
    let mut file = tokio::fs::File::open(path).await?;
    let mut buffer = vec![0_u8; BUFFER_SIZE];
    let mut sha1 = sha1_smol::Sha1::new();
    let mut size = 0_u64;
    let mut normalized_size = 0_u64;
    loop {
        let read = read_curseforge_verification_chunk(
            &mut file,
            &mut buffer,
            cancellation,
        )
        .await?;
        if read == 0 {
            break;
        }
        let bytes = &buffer[..read];
        sha1.update(bytes);
        size += read as u64;
        normalized_size += bytes
            .iter()
            .filter(|byte| !is_curseforge_fingerprint_whitespace(**byte))
            .count() as u64;
    }

    let mut file = tokio::fs::File::open(path).await?;
    let mut fingerprint =
        CurseForgeFingerprintHasher::new(normalized_size as u32);
    loop {
        let read = read_curseforge_verification_chunk(
            &mut file,
            &mut buffer,
            cancellation,
        )
        .await?;
        if read == 0 {
            break;
        }
        fingerprint.update(&buffer[..read]);
    }
    tracing::debug!(path = %path.display(), bytes = size, verification_ms = started.elapsed().as_millis(), "Completed CurseForge fingerprint verification");
    Ok((size, sha1.digest().to_string(), fingerprint.finish()))
}

async fn verify_installed_curseforge_file(
    path: &Path,
    file: &CurseForgeFile,
    cancellation: Option<&CancellationToken>,
    download: Option<&crate::util::fetch::DownloadResult>,
) -> crate::Result<VerifiedInstalledCurseForgeFile> {
    if cancellation.is_some_and(CancellationToken::is_cancelled) {
        return Err(ErrorKind::OtherError(
            "CurseForge verification canceled".into(),
        )
        .into());
    }
    if let Some(expected_sha1) = file
        .hashes
        .iter()
        .find(|hash| hash.algo == 1 && !hash.value.trim().is_empty())
        .map(|hash| hash.value.as_str())
    {
        if let Some(download) = download
            && download.path == path
            && download
                .verified_sha1
                .as_deref()
                .is_some_and(|hash| hash.eq_ignore_ascii_case(expected_sha1))
            && let Some(proof) = &download.verified_file
            && proof.matches(path, download.size).await
        {
            return Ok(VerifiedInstalledCurseForgeFile {
                size: download.size,
                sha1: expected_sha1.to_string(),
                pending_completion:
                    CurseForgePendingCompletionProof::AuthoritativeSha1,
            });
        }
        let (size, sha1) = match cancellation {
            Some(cancellation) => {
                sha1_file_cancellable(path, cancellation).await?
            }
            None => sha1_file_async(path).await?,
        };
        if !sha1.eq_ignore_ascii_case(expected_sha1) {
            return Err(
                ErrorKind::HashError(expected_sha1.to_string(), sha1).into()
            );
        }
        return Ok(VerifiedInstalledCurseForgeFile {
            size,
            sha1,
            pending_completion:
                CurseForgePendingCompletionProof::AuthoritativeSha1,
        });
    }

    if file.file_fingerprint != 0 {
        let (size, sha1, fingerprint) =
            fingerprint_and_sha1_file(path, cancellation).await?;
        if fingerprint as u64 != file.file_fingerprint {
            return Err(ErrorKind::InputError(
                "The downloaded file does not match the required CurseForge fingerprint"
                    .to_string(),
            )
            .into());
        }
        return Ok(VerifiedInstalledCurseForgeFile {
            size,
            sha1,
            pending_completion:
                CurseForgePendingCompletionProof::AuthoritativeFingerprint,
        });
    }

    let (size, sha1) = match cancellation {
        Some(cancellation) => sha1_file_cancellable(path, cancellation).await?,
        None => sha1_file_async(path).await?,
    };
    Ok(VerifiedInstalledCurseForgeFile {
        size,
        sha1,
        pending_completion: CurseForgePendingCompletionProof::None,
    })
}

async fn record_installed_curseforge_file_with_permit(
    instance_id: &str,
    relative_path: &str,
    full_path: &Path,
    file: &CurseForgeFile,
    project_type: ProjectType,
    ownership_kind: crate::state::instances::ContentOwnershipKind,
    database_permit: Option<tokio::sync::SemaphorePermit<'_>>,
    state: &State,
) -> crate::Result<()> {
    let verified =
        verify_installed_curseforge_file(full_path, file, None, None).await?;
    record_verified_curseforge_file_with_permit(
        instance_id,
        relative_path,
        file,
        project_type,
        ownership_kind,
        database_permit,
        state,
        verified,
    )
    .await
}

async fn record_verified_curseforge_file_with_permit(
    instance_id: &str,
    relative_path: &str,
    file: &CurseForgeFile,
    project_type: ProjectType,
    ownership_kind: crate::state::instances::ContentOwnershipKind,
    database_permit: Option<tokio::sync::SemaphorePermit<'_>>,
    state: &State,
    verified: VerifiedInstalledCurseForgeFile,
) -> crate::Result<()> {
    match verified.pending_completion {
        CurseForgePendingCompletionProof::None => {
            let provider_ref = ContentProviderRef::CurseForge {
                project_id: CurseForgeProjectId::new(file.mod_id)?,
                file_id: Some(CurseForgeFileId::new(file.id)?),
            };
            crate::state::instances::commands::record_project_files_atomic_with_permit(
                instance_id,
                &[crate::state::instances::commands::ProjectFileRecord {
                    relative_path: relative_path.to_string(),
                    sha1: verified.sha1,
                    size: verified.size,
                    project_type,
                    source_kind: ContentSourceKind::CurseForge,
                    ownership_kind,
                    provider_ref: Some(provider_ref),
                    origin: true,
                    known_modrinth_project_id: None,
                    known_modrinth_version_id: None,
                }],
                &[],
                database_permit,
                state,
            )
            .await
        }
        CurseForgePendingCompletionProof::AuthoritativeSha1
        | CurseForgePendingCompletionProof::AuthoritativeFingerprint => {
            crate::state::instances::commands::record_project_files_atomic_with_permit(
                instance_id,
                &[crate::state::instances::commands::ProjectFileRecord {
                    relative_path: relative_path.to_string(),
                    sha1: verified.sha1,
                    size: verified.size,
                    project_type,
                    source_kind: ContentSourceKind::CurseForge,
                    ownership_kind,
                    provider_ref: Some(ContentProviderRef::CurseForge {
                        project_id: CurseForgeProjectId::new(file.mod_id)?,
                        file_id: Some(CurseForgeFileId::new(file.id)?),
                    }),
                    origin: true,
                    known_modrinth_project_id: None,
                    known_modrinth_version_id: None,
                }],
                &[(CurseForgeProjectId::new(file.mod_id)?, CurseForgeFileId::new(file.id)?)],
                database_permit,
                state,
            )
            .await
        }
    }
}

fn is_curseforge_fingerprint_whitespace(byte: u8) -> bool {
    matches!(byte, 9 | 10 | 13 | 32)
}

pub(crate) struct CurseForgeFingerprintHasher {
    hash: u32,
    tail: [u8; 4],
    tail_len: usize,
}

impl CurseForgeFingerprintHasher {
    pub(crate) fn new(normalized_size: u32) -> Self {
        Self {
            hash: 1 ^ normalized_size,
            tail: [0; 4],
            tail_len: 0,
        }
    }

    pub(crate) fn update(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            if is_curseforge_fingerprint_whitespace(byte) {
                continue;
            }
            self.tail[self.tail_len] = byte;
            self.tail_len += 1;
            if self.tail_len == self.tail.len() {
                self.mix_word(u32::from_le_bytes(self.tail));
                self.tail_len = 0;
            }
        }
    }

    fn mix_word(&mut self, mut value: u32) {
        const M: u32 = 0x5bd1e995;
        const R: u32 = 24;
        value = value.wrapping_mul(M);
        value ^= value >> R;
        value = value.wrapping_mul(M);
        self.hash = self.hash.wrapping_mul(M);
        self.hash ^= value;
    }

    pub(crate) fn finish(mut self) -> u32 {
        const M: u32 = 0x5bd1e995;
        match self.tail[..self.tail_len] {
            [a, b, c] => {
                self.hash ^= (c as u32) << 16;
                self.hash ^= (b as u32) << 8;
                self.hash ^= a as u32;
                self.hash = self.hash.wrapping_mul(M);
            }
            [a, b] => {
                self.hash ^= (b as u32) << 8;
                self.hash ^= a as u32;
                self.hash = self.hash.wrapping_mul(M);
            }
            [a] => {
                self.hash ^= a as u32;
                self.hash = self.hash.wrapping_mul(M);
            }
            [] => {}
            _ => unreachable!(),
        }
        self.hash ^= self.hash >> 13;
        self.hash = self.hash.wrapping_mul(M);
        self.hash ^= self.hash >> 15;
        self.hash
    }
}

pub fn compute_fingerprint(data: &[u8]) -> u32 {
    let normalized_size = data
        .iter()
        .filter(|byte| !is_curseforge_fingerprint_whitespace(**byte))
        .count() as u32;
    let mut hasher = CurseForgeFingerprintHasher::new(normalized_size);
    hasher.update(data);
    hasher.finish()
}

impl From<CurseForgeProject> for UnifiedSearchHit {
    fn from(project: CurseForgeProject) -> Self {
        let mut versions = Vec::new();
        let mut seen_versions = HashSet::new();
        let mut loaders = HashSet::new();
        for index in &project.latest_files_indexes {
            if seen_versions.insert(index.game_version.clone()) {
                versions.push(index.game_version.clone());
            }
            if let Some(mod_loader) = index.mod_loader {
                let loader_slug = mod_loader_to_slug(mod_loader);
                loaders.insert(loader_slug);
            }
        }

        let project_type = project_type_for_class(project.class_id);
        let mut categories: Vec<String> = project
            .categories
            .iter()
            .map(|category| category.slug.clone())
            .collect();
        categories.extend(loaders.iter().map(|s| s.to_string()));

        Self {
            provider: ContentProvider::CurseForge,
            project_id: project.id.to_string(),
            slug: Some(project.slug),
            author: project
                .authors
                .first()
                .map(|author| author.name.clone())
                .unwrap_or_default(),
            author_url: project
                .authors
                .first()
                .map(|author| author.url.clone()),
            title: project.name,
            description: project.summary,
            project_type: project_type.to_string(),
            categories,
            versions,
            downloads: project.download_count,
            icon_url: project.logo.map(|logo| logo.url),
            date_created: project.date_created,
            date_modified: project.date_modified,
            latest_version: project
                .latest_files
                .first()
                .map(|file| file.id.to_string()),
            gallery: project
                .screenshots
                .into_iter()
                .map(|screenshot| screenshot.url)
                .collect(),
            website_url: project.links.website_url,
            source_url: project.links.source_url,
            allow_mod_distribution: project.allow_mod_distribution,
        }
    }
}

pub(crate) fn api_key() -> Option<String> {
    api_key_with_source().0
}

/// A key set in the launcher wins over `AXOLOTL_CURSEFORGE_API_KEY`, which in
/// turn wins over the one this build was given. Either of the launcher sources
/// can be absent: builds without `CURSEFORGE_API_KEY` stay valid, they just
/// cannot reach the official API until a key is provided.
fn api_key_with_source() -> (Option<String>, CurseForgeApiKeySource) {
    let launcher_key = std::env::var("AXOLOTL_CURSEFORGE_API_KEY")
        .ok()
        .or_else(|| option_env!("CURSEFORGE_API_KEY").map(str::to_string));

    resolve_api_key(curseforge_api_key_override_if_ready(), launcher_key)
}

fn resolve_api_key(
    override_key: Option<String>,
    launcher_key: Option<String>,
) -> (Option<String>, CurseForgeApiKeySource) {
    if let Some(key) = curseforge_settings::normalize(override_key) {
        return (Some(key), CurseForgeApiKeySource::Override);
    }

    match curseforge_settings::normalize(launcher_key) {
        Some(key) => (Some(key), CurseForgeApiKeySource::Launcher),
        None => (None, CurseForgeApiKeySource::Missing),
    }
}

fn api_base_url() -> String {
    #[cfg(debug_assertions)]
    if let Ok(value) = std::env::var("AXOLOTL_CURSEFORGE_API_BASE_URL")
        && value.starts_with("http://127.0.0.1:")
    {
        return value.trim_end_matches('/').to_string();
    }

    API_BASE_URL.to_string()
}

fn request_client(
    state: &State,
    _url: &str,
    use_system_proxy: bool,
) -> reqwest::Client {
    #[cfg(debug_assertions)]
    if _url.starts_with("http://127.0.0.1:") {
        return LOCAL_CLIENT.clone();
    }

    if use_system_proxy {
        state.download_clients().system
    } else {
        CLIENT.clone()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MirrorPolicy {
    MirrorFirst,
    OfficialOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RequestRouteSource {
    Official,
    Mirror,
}

struct RequestRoute {
    url: String,
    use_api_key: bool,
    use_system_proxy: bool,
    source: RequestRouteSource,
}

#[cfg(test)]
fn request_routes(
    path: &str,
    mirror_policy: MirrorPolicy,
) -> Vec<RequestRoute> {
    let mode = match mirror_policy {
        MirrorPolicy::MirrorFirst => DownloadSourceMode::MirrorPreferred,
        MirrorPolicy::OfficialOnly => DownloadSourceMode::OfficialOnly,
    };
    request_routes_with_mode(path, mode)
}

fn request_routes_with_mode(
    path: &str,
    mode: DownloadSourceMode,
) -> Vec<RequestRoute> {
    let base_url = api_base_url();
    if base_url != API_BASE_URL {
        return vec![RequestRoute {
            url: format!("{base_url}{path}"),
            use_api_key: true,
            use_system_proxy: false,
            source: RequestRouteSource::Official,
        }];
    }

    resolve_download_routes_for(
        &format!("{API_BASE_URL}{path}"),
        ResourceClass::CurseForge,
        mode,
    )
    .into_iter()
    .map(|route| RequestRoute {
        use_api_key: route.allow_sensitive_headers,
        use_system_proxy: route.proxy == ProxyPolicy::System,
        source: match route.source {
            DownloadRouteSource::Bmclapi
            | DownloadRouteSource::Mcim
            | DownloadRouteSource::Aliyun => RequestRouteSource::Mirror,
            DownloadRouteSource::Official | DownloadRouteSource::Alternate => {
                RequestRouteSource::Official
            }
        },
        url: route.url,
    })
    .collect()
}

async fn request_json<T: DeserializeOwned>(
    method: Method,
    path: &str,
    query: Vec<(String, String)>,
    body: Option<Value>,
    mirror_policy: MirrorPolicy,
) -> crate::Result<T> {
    let key = api_key();
    let key_generation = api_key_generation();
    let state = State::get().await?;
    let source_mode = if method != Method::GET
        || mirror_policy == MirrorPolicy::OfficialOnly
    {
        DownloadSourceMode::OfficialOnly
    } else {
        state.curseforge_source()
    };
    let routes = request_routes_with_mode(path, source_mode);
    let mut last_error = None;

    for (route_index, route) in routes.iter().enumerate() {
        let started = Instant::now();
        tracing::info!(
            source = ?route.source,
            method = %method,
            url = %route.url,
            route = route_index + 1,
            use_system_proxy = route.use_system_proxy,
            "Attempting CurseForge API request"
        );
        let permit = state.api_semaphore.0.acquire().await?;
        let mut request =
            request_client(&state, &route.url, route.use_system_proxy)
                .request(method.clone(), &route.url)
                .header("accept", "application/json")
                .query(&query);
        if route.use_api_key {
            let Some(key) = key.as_ref() else {
                drop(permit);
                let error: crate::Error = ErrorKind::InputError(
                    "CurseForge integration is waiting for an API key"
                        .to_string(),
                )
                .into();
                if route_index + 1 < routes.len() {
                    last_error = Some(error);
                    continue;
                }
                return Err(error);
            };
            request = request.header("x-api-key", key);
        }
        if let Some(body) = &body {
            request = request.json(body);
        }
        let response = match request.send().await {
            Ok(response) => response,
            Err(error) if route_index + 1 < routes.len() => {
                drop(permit);
                tracing::warn!(
                    url = %route.url,
                    route = route_index + 1,
                    %error,
                    "CurseForge request failed, retrying with another route"
                );
                last_error = Some(error.into());
                continue;
            }
            Err(error) => return Err(error.into()),
        };
        drop(permit);

        let status = response.status();
        let retry_after = response
            .headers()
            .get("retry-after")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok())
            .map(|seconds| Duration::from_secs(seconds.min(30)));
        let bytes = response.bytes().await?;

        tracing::info!(
            source = ?route.source,
            method = %method,
            url = %route.url,
            route = route_index + 1,
            status = status.as_u16(),
            response_bytes = bytes.len(),
            elapsed_ms = started.elapsed().as_millis(),
            "Completed CurseForge API request"
        );

        if status.is_success() {
            match serde_json::from_slice(&bytes) {
                Ok(value) => {
                    record_key_response(key_generation, false);
                    return Ok(value);
                }
                Err(error)
                    if route.source == RequestRouteSource::Mirror
                        && route_index + 1 < routes.len() =>
                {
                    tracing::warn!(
                        url = %route.url,
                        route = route_index + 1,
                        %error,
                        "CurseForge mirror returned incompatible response data; falling back to official source"
                    );
                    last_error = Some(error.into());
                    continue;
                }
                Err(error) => return Err(error.into()),
            }
        }

        if status == StatusCode::UNAUTHORIZED {
            record_key_response(key_generation, true);
        }

        let message = response_error_message(status, &bytes);
        let route_error = ErrorKind::OtherError(format!(
            "CurseForge request to {} failed with HTTP {}: {message}",
            route.url,
            status.as_u16()
        ));

        if should_try_next_route(route, status, route_index + 1 < routes.len())
        {
            if let Some(delay) = retry_after {
                tokio::time::sleep(delay).await;
            }
            tracing::warn!(
                url = %route.url,
                route = route_index + 1,
                status = status.as_u16(),
                "CurseForge route rejected the request, trying another route"
            );
            last_error = Some(route_error.into());
            continue;
        }

        return Err(route_error.into());
    }

    Err(last_error.unwrap_or_else(|| {
        ErrorKind::OtherError("CurseForge request exhausted routes".to_string())
            .into()
    }))
}

fn should_try_next_route(
    route: &RequestRoute,
    status: StatusCode,
    has_next_route: bool,
) -> bool {
    has_next_route
        && (route.source == RequestRouteSource::Mirror
            || status == StatusCode::TOO_MANY_REQUESTS
            || status == StatusCode::FORBIDDEN
            || status.is_server_error())
}

fn response_error_message(status: StatusCode, bytes: &[u8]) -> String {
    serde_json::from_slice::<Value>(bytes)
        .ok()
        .and_then(|value| {
            value
                .get("description")
                .or_else(|| value.get("message"))
                .or_else(|| value.get("error"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| {
            status
                .canonical_reason()
                .unwrap_or("request failed")
                .to_string()
        })
}

#[cfg(test)]
fn murmur2(data: &[u8], seed: u32) -> u32 {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn modpack_manifest_worlds_use_the_save_importer_instead_of_the_mod_file_installer()
     {
        assert_eq!(project_type_for_class(Some(17)), "world");
        assert_eq!(modpack_target_folder("world").unwrap(), "saves");
        assert_eq!(modpack_target_folder("mod").unwrap(), "mods");
        assert_eq!(
            modpack_target_folder("resourcepack").unwrap(),
            "resourcepacks"
        );
        assert!(modpack_target_folder("modpack").is_err());
        assert!(managed_project_type("world").is_err());
    }

    #[cfg(not(feature = "tauri"))]
    #[tokio::test]
    async fn modpack_manifest_imports_world_archives_and_preserves_existing_saves_on_retry()
     {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let (state, instance_id) =
            create_stage6_instance("manifest world").await;
        let mut archive = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        archive
            .start_file(
                "Wrapped world/level.dat",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        archive.write_all(b"world level data").unwrap();
        archive
            .start_file(
                "Wrapped world/region/r.0.0.mca",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        archive.write_all(b"region bytes").unwrap();
        let bytes = archive.finish().unwrap().into_inner();
        let server =
            tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/world.zip", server.local_addr().unwrap());
        let response_bytes = bytes.clone();
        let server_task = tokio::spawn(async move {
            loop {
                let (mut stream, _) = server.accept().await.unwrap();
                let bytes = response_bytes.clone();
                tokio::spawn(async move {
                    let mut request = [0; 8192];
                    let n = stream.read(&mut request).await.unwrap();
                    let head = String::from_utf8_lossy(&request[..n])
                        .starts_with("HEAD ");
                    stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/zip\r\nConnection: close\r\n\r\n", bytes.len()).as_bytes()).await.unwrap();
                    if !head {
                        stream.write_all(&bytes).await.unwrap();
                    }
                });
            }
        });
        let project: CurseForgeProject = serde_json::from_value(json!({
            "id": 1733396, "gameId": 432, "name": "Pack world", "slug": "pack-world", "summary": "World bundled in a pack",
            "status": 4, "downloadCount": 0, "isFeatured": false, "primaryCategoryId": 0,
            "classId": 17, "mainFileId": 9206584, "dateCreated": "", "dateModified": "", "dateReleased": "", "isAvailable": true,
        })).unwrap();
        let mut file = stage8_curseforge_file(
            project.id,
            9206584,
            "Pack world.zip",
            bytes.len() as u64,
            vec![CurseForgeFileHash {
                algo: 1,
                value: sha1_smol::Sha1::from(&bytes).hexdigest(),
            }],
            0,
        );
        file.download_url = Some(url);
        let manifest_file = CurseForgeManifestFile {
            project_id: project.id,
            file_id: file.id,
            required: true,
        };
        let installed = tokio::time::timeout(Duration::from_secs(10), retry_modpack_file_install(
            &instance_id, &manifest_file, "world", "1.21.1", Some(6),
            crate::state::instances::ManualDownloadOperationKind::PackInstall,
            None, &file.file_name, CancellationToken::new(), None, None, Some((project.clone(), file.clone())),
        )).await.unwrap();
        server_task.abort();
        let result = installed.0.unwrap_or_else(|| panic!("{}", installed.2));
        assert_eq!(result.installed[0].relative_path, "saves/Pack world");
        let root = crate::api::instance::get_full_path(&instance_id)
            .await
            .unwrap();
        assert_eq!(
            std::fs::read(root.join("saves/Pack world/level.dat")).unwrap(),
            b"world level data"
        );
        assert_eq!(
            std::fs::read(root.join("saves/Pack world/region/r.0.0.mca"))
                .unwrap(),
            b"region bytes"
        );
        std::fs::write(
            root.join("saves/Pack world/level.dat"),
            b"player progress",
        )
        .unwrap();
        install_modpack_world_file(
            &instance_id,
            &project,
            &file,
            None,
            crate::state::instances::ManualDownloadOperationKind::PackUpdate,
        )
        .await
        .unwrap();
        assert_eq!(
            std::fs::read(root.join("saves/Pack world/level.dat")).unwrap(),
            b"player progress"
        );
        assert!(
            crate::state::get_instance(&instance_id, &state.pool)
                .await
                .unwrap()
                .is_some()
        );
        crate::api::instance::remove(&instance_id).await.unwrap();
    }

    #[test]
    fn content_updates_only_resolve_required_dependencies() {
        use crate::state::instances::ManualDownloadOperationKind;

        let operation = ManualDownloadOperationKind::ContentUpdate;
        assert!(should_resolve_dependency(
            DEPENDENCY_RELATION_REQUIRED,
            operation
        ));
        for relation in [
            DEPENDENCY_RELATION_OPTIONAL,
            DEPENDENCY_RELATION_INCLUDE,
            DEPENDENCY_RELATION_EMBEDDED,
            DEPENDENCY_RELATION_TOOL,
            DEPENDENCY_RELATION_INCOMPATIBLE,
        ] {
            assert!(!should_resolve_dependency(relation, operation));
        }
        assert_eq!(
            dependency_policy(operation),
            DependencyPolicy::RequiredOnly
        );
    }

    #[test]
    fn content_installs_still_resolve_optional_dependencies_for_selection() {
        use crate::state::instances::ManualDownloadOperationKind;

        for operation in [
            ManualDownloadOperationKind::ContentInstall,
            ManualDownloadOperationKind::PackInstall,
            ManualDownloadOperationKind::PackUpdate,
        ] {
            for relation in [
                DEPENDENCY_RELATION_REQUIRED,
                DEPENDENCY_RELATION_OPTIONAL,
                DEPENDENCY_RELATION_INCLUDE,
            ] {
                assert!(should_resolve_dependency(relation, operation));
            }
            assert_eq!(
                dependency_policy(operation),
                DependencyPolicy::IncludeOptional
            );
        }
    }

    fn write_override_test_archive(path: &Path, entries: &[(&str, &[u8])]) {
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

    #[test]
    fn modpack_metadata_ids_are_deduplicated_before_batching() {
        let mut ids = (1..=120).collect::<Vec<_>>();
        ids.extend([1, 50, 120]);
        let chunks = unique_metadata_chunks(ids);

        assert_eq!(
            chunks.iter().map(Vec::len).collect::<Vec<_>>(),
            vec![50, 50, 20]
        );
        assert_eq!(
            chunks
                .iter()
                .flatten()
                .copied()
                .collect::<HashSet<_>>()
                .len(),
            120
        );
    }

    #[test]
    fn modpack_archives_use_bounded_ranges_only_for_large_files() {
        assert_eq!(
            curseforge_modpack_h2_range_concurrency(128 * 1024 * 1024),
            Some(4)
        );
        assert_eq!(curseforge_modpack_h2_range_concurrency(1024 * 1024), None);
    }

    #[tokio::test]
    async fn staged_digest_proof_is_reused_only_for_the_unchanged_file() {
        let directory = tempfile::tempdir().unwrap();
        let staged = directory.path().join("content.installing.download");
        let other = directory.path().join("other.installing.download");
        tokio::fs::write(&staged, b"verified staged bytes")
            .await
            .unwrap();
        let sha1 = sha1_smol::Sha1::from(b"verified staged bytes").hexdigest();
        let file = stage8_curseforge_file(
            1,
            2,
            "content.jar",
            21,
            vec![CurseForgeFileHash {
                value: sha1.clone(),
                algo: 1,
            }],
            0,
        );
        let download = crate::util::fetch::DownloadResult {
            path: staged.clone(),
            url: "https://staged-proof.invalid/file".into(),
            source: crate::util::fetch::DownloadRouteSource::Official,
            size: 21,
            attempts: 1,
            fallback_count: 0,
            verified_sha1: Some(sha1.clone()),
            verified_sha512: None,
            verified_file:
                crate::util::download::verified_file::VerifiedFile::capture(
                    &staged, 21,
                )
                .await,
        };
        assert_eq!(
            download
                .verified_file
                .as_ref()
                .unwrap()
                .matches(&staged, 21)
                .await,
            cfg!(unix)
        );
        let (verified, scans) =
            crate::util::download::verified_file::track_scans(
                verify_installed_curseforge_file(
                    &staged,
                    &file,
                    None,
                    Some(&download),
                ),
            )
            .await;
        let verified = verified.unwrap();
        assert_eq!(scans, usize::from(!cfg!(unix)));
        assert_eq!(verified.sha1, sha1);
        assert_eq!(
            verified.pending_completion,
            CurseForgePendingCompletionProof::AuthoritativeSha1
        );
        let serialized = serde_json::to_value(&download).unwrap();
        let restored: crate::util::fetch::DownloadResult =
            serde_json::from_value(serialized).unwrap();
        assert!(restored.verified_file.is_none());
        let (rescanned, scans) =
            crate::util::download::verified_file::track_scans(
                verify_installed_curseforge_file(
                    &staged,
                    &file,
                    None,
                    Some(&restored),
                ),
            )
            .await;
        assert!(rescanned.is_ok());
        assert_eq!(scans, 1);
        tokio::fs::write(&other, b"corrupted other bytes")
            .await
            .unwrap();
        assert!(
            verify_installed_curseforge_file(
                &other,
                &file,
                None,
                Some(&download)
            )
            .await
            .is_err()
        );
        tokio::fs::write(&staged, b"corrupted staged file")
            .await
            .unwrap();
        let handle =
            std::fs::File::options().write(true).open(&staged).unwrap();
        handle
            .set_times(std::fs::FileTimes::new().set_modified(
                std::time::SystemTime::UNIX_EPOCH
                    + std::time::Duration::from_secs(123456),
            ))
            .unwrap();
        assert!(
            verify_installed_curseforge_file(
                &staged,
                &file,
                None,
                Some(&download)
            )
            .await
            .is_err()
        );
        assert!(
            verify_installed_curseforge_file(
                &staged,
                &file,
                None,
                Some(&restored)
            )
            .await
            .is_err()
        );
    }

    #[tokio::test]
    async fn staged_digest_proof_never_skips_fingerprint_verification() {
        let directory = tempfile::tempdir().unwrap();
        let staged = directory.path().join("content.installing.download");
        tokio::fs::write(&staged, b"verified staged bytes")
            .await
            .unwrap();
        let file = stage8_curseforge_file(1, 2, "content.jar", 21, vec![], 1);
        let download = crate::util::fetch::DownloadResult {
            path: staged.clone(),
            url: "https://staged-proof.invalid/file".into(),
            source: crate::util::fetch::DownloadRouteSource::Official,
            size: 21,
            attempts: 1,
            fallback_count: 0,
            verified_sha1: Some("trusted-sha1".into()),
            verified_sha512: None,
            verified_file:
                crate::util::download::verified_file::VerifiedFile::capture(
                    &staged, 21,
                )
                .await,
        };
        let result = verify_installed_curseforge_file(
            &staged,
            &file,
            None,
            Some(&download),
        )
        .await;
        assert!(result.err().unwrap().to_string().contains("fingerprint"));
    }

    #[test]
    fn curseforge_overrides_publish_after_all_entries_validate() {
        let root = tempfile::tempdir().unwrap();
        let archive_path = root.path().join("pack.zip");
        let instance_path = root.path().join("instance");
        let first = instance_path.join("config/first.txt");
        let second = instance_path.join("config/second.txt");
        std::fs::create_dir_all(first.parent().unwrap()).unwrap();
        std::fs::write(&first, b"old-first").unwrap();
        std::fs::write(&second, b"old-second").unwrap();
        let corrupt_payload = b"unique-corrupt-override-payload";
        write_override_test_archive(
            &archive_path,
            &[
                ("config/first.txt", b"new-first"),
                ("config/second.txt", corrupt_payload),
            ],
        );
        let mut bytes = std::fs::read(&archive_path).unwrap();
        let offset = bytes
            .windows(corrupt_payload.len())
            .position(|window| window == corrupt_payload)
            .unwrap();
        bytes[offset] ^= 0xff;
        std::fs::write(&archive_path, bytes).unwrap();

        let error =
            extract_modpack_overrides(&archive_path, &instance_path, None)
                .unwrap_err();

        assert!(
            error.to_string().contains("CRC")
                || error.to_string().contains("checksum")
        );
        assert_eq!(std::fs::read(&first).unwrap(), b"old-first");
        assert_eq!(std::fs::read(&second).unwrap(), b"old-second");
        assert!(
            !PathBuf::from(format!("{}.installing", first.display())).exists()
        );
        assert!(
            !PathBuf::from(format!("{}.installing", second.display())).exists()
        );
    }

    fn write_override_size_test_archive(path: &Path, entries: &[(&str, u32)]) {
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
            bytes[offset + 24..offset + 28]
                .copy_from_slice(&size.to_le_bytes());
        }
        std::fs::write(path, bytes).unwrap();
    }

    #[test]
    fn curseforge_overrides_accept_sizes_above_two_gib_up_to_shared_limit() {
        let root = tempfile::tempdir().unwrap();
        let archive_path = root.path().join("pack.zip");
        for sizes in [
            [3 * 1024 * 1024 * 1024, 0, 0],
            [u32::MAX - 1, u32::MAX - 1, 4],
        ] {
            write_override_size_test_archive(
                &archive_path,
                &[("a", sizes[0]), ("b", sizes[1]), ("c", sizes[2])],
            );
            let mut archive = zip::ZipArchive::new(
                std::fs::File::open(&archive_path).unwrap(),
            )
            .unwrap();
            let manifest = read_modpack_manifest(&mut archive).unwrap();
            let tasks = collect_modpack_override_tasks(
                &mut archive,
                &manifest,
                root.path(),
                None,
            )
            .unwrap();
            assert_eq!(tasks.len(), 3);
            assert_eq!(
                tasks.iter().map(|task| task.size).sum::<u64>(),
                sizes.into_iter().map(u64::from).sum::<u64>(),
            );
        }
    }

    #[test]
    fn curseforge_overrides_reject_above_shared_limit_before_writing() {
        let root = tempfile::tempdir().unwrap();
        let archive_path = root.path().join("pack.zip");
        let instance_path = root.path().join("instance");
        write_override_size_test_archive(
            &archive_path,
            &[("a", u32::MAX - 1), ("b", u32::MAX - 1), ("c", 5)],
        );
        let error =
            materialize_modpack_overrides(&archive_path, &instance_path, None)
                .err()
                .unwrap()
                .to_string();
        assert!(error.contains("overrides exceed the extraction limit"));
        assert!(error.contains("8589934593 unpacked bytes"));
        assert!(error.contains("limit is 8589934592 bytes"));
        assert!(!instance_path.exists());
    }

    #[test]
    fn curseforge_overrides_size_counts_only_final_duplicate_targets() {
        let root = tempfile::tempdir().unwrap();
        let archive_path = root.path().join("pack.zip");
        write_override_size_test_archive(
            &archive_path,
            &[
                ("config/a", u32::MAX - 1),
                ("config\\a", 1),
                ("config/b", u32::MAX - 1),
                ("config/c", 5),
            ],
        );
        let mut archive =
            zip::ZipArchive::new(std::fs::File::open(&archive_path).unwrap())
                .unwrap();
        let manifest = read_modpack_manifest(&mut archive).unwrap();
        let tasks = collect_modpack_override_tasks(
            &mut archive,
            &manifest,
            root.path(),
            None,
        )
        .unwrap();
        assert_eq!(tasks.len(), 3);
        let winner = tasks
            .iter()
            .find(|task| task.target == root.path().join("config/a"))
            .unwrap();
        assert_eq!(winner.index, 2);
        assert_eq!(winner.size, 1);
    }

    #[test]
    fn curseforge_overrides_commit_successful_parallel_staging() {
        let root = tempfile::tempdir().unwrap();
        let archive_path = root.path().join("pack.zip");
        let instance_path = root.path().join("instance");
        let target = instance_path.join("config/example.txt");
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(&target, b"old").unwrap();
        write_override_test_archive(
            &archive_path,
            &[("config/example.txt", b"replacement")],
        );

        let written =
            extract_modpack_overrides(&archive_path, &instance_path, None)
                .unwrap();

        assert_eq!(written, 1);
        assert_eq!(std::fs::read(&target).unwrap(), b"replacement");
        assert!(
            !PathBuf::from(format!("{}.installing", target.display())).exists()
        );
        assert!(
            !PathBuf::from(format!("{}.installing.previous", target.display()))
                .exists()
        );
    }

    #[test]
    fn curseforge_overrides_restore_after_later_stage_failure() {
        let root = tempfile::tempdir().unwrap();
        let archive_path = root.path().join("pack.zip");
        let instance_path = root.path().join("instance");
        let existing = instance_path.join("config/existing.txt");
        let created = instance_path.join("config/created.txt");
        std::fs::create_dir_all(existing.parent().unwrap()).unwrap();
        std::fs::write(&existing, b"old-existing").unwrap();
        write_override_test_archive(
            &archive_path,
            &[
                ("config/existing.txt", b"new-existing"),
                ("config/created.txt", b"new-created"),
            ],
        );

        let (written, replacements) =
            materialize_modpack_overrides(&archive_path, &instance_path, None)
                .unwrap();
        assert_eq!(written, 2);
        assert_eq!(std::fs::read(&existing).unwrap(), b"new-existing");
        assert_eq!(std::fs::read(&created).unwrap(), b"new-created");

        replacements.rollback().unwrap();

        assert_eq!(std::fs::read(&existing).unwrap(), b"old-existing");
        assert!(!created.exists());
        assert!(
            !PathBuf::from(format!(
                "{}.installing.previous",
                existing.display()
            ))
            .exists()
        );
    }

    #[tokio::test]
    async fn curseforge_database_worker_cancels_while_queue_is_empty() {
        let cancellation = CancellationToken::new();
        let context = CurseForgeVerificationContext {
            reporter: None,
            loading_bar: None,
            details: InstallPhaseDetails::Empty,
            files_done: Arc::new(AtomicU64::new(0)),
            bytes_done: Arc::new(AtomicU64::new(0)),
            active_downloads: Arc::new(AtomicU64::new(0)),
            total_files: 1,
            total_bytes: 1,
            cancellation: cancellation.clone(),
        };
        let (_sender, receiver) = mpsc::channel(1);
        let worker = spawn_curseforge_database_worker(receiver, context);

        cancellation.cancel();
        let error = tokio::time::timeout(Duration::from_millis(100), worker)
            .await
            .expect("an idle database worker should stop immediately")
            .unwrap()
            .unwrap_err();

        assert!(error.to_string().contains("canceled"));
    }

    #[tokio::test]
    async fn curseforge_database_batch_is_retained_when_canceled_mid_batch() {
        let cancellation = CancellationToken::new();
        let (sender, mut receiver) = mpsc::channel(2);
        sender.send(7_u8).await.unwrap();
        let trigger = cancellation.clone();
        let cancel_task = tokio::spawn(async move {
            tokio::task::yield_now().await;
            trigger.cancel();
        });

        let batch = tokio::time::timeout(
            Duration::from_millis(100),
            receive_curseforge_database_batch(&mut receiver, &cancellation),
        )
        .await
        .expect("partial database batch should be returned on cancellation")
        .unwrap()
        .unwrap();
        cancel_task.await.unwrap();

        assert_eq!(batch, vec![7]);
    }

    #[test]
    fn active_curseforge_download_counter_is_restored_on_drop() {
        let counter = Arc::new(AtomicU64::new(0));
        {
            let _active = ActiveCurseForgeDownload::start(counter.clone());
            assert_eq!(counter.load(Ordering::Relaxed), 1);
        }
        assert_eq!(counter.load(Ordering::Relaxed), 0);

        let mut active = ActiveCurseForgeDownload::start(counter.clone());
        active.finish();
        active.finish();
        assert_eq!(counter.load(Ordering::Relaxed), 0);
    }

    #[cfg(not(feature = "tauri"))]
    #[tokio::test]
    async fn failed_curseforge_verification_never_replaces_existing_file() {
        let _state = stage6_state().await;
        let directory = tempfile::tempdir().unwrap();
        let download_path = directory.path().join("new.installing.download");
        let full_path = directory.path().join("existing.jar");
        crate::util::io::write(&download_path, b"invalid replacement")
            .await
            .unwrap();
        crate::util::io::write(&full_path, b"existing content")
            .await
            .unwrap();
        let expected_sha1 =
            sha1_smol::Sha1::from(b"expected replacement").hexdigest();
        let file = stage8_curseforge_file(
            1,
            2,
            "existing.jar",
            19,
            vec![CurseForgeFileHash {
                value: expected_sha1,
                algo: 1,
            }],
            0,
        );
        let (database_tx, _database_rx) = mpsc::channel(1);

        let error = verify_and_record_curseforge_modpack_file(
            CurseForgeVerificationTask {
                instance_id: "verification-order-test".to_string(),
                relative_path: "mods/existing.jar".to_string(),
                download_path: download_path.clone(),
                full_path: full_path.clone(),
                file,
                project_type: ProjectType::Mod,
                ownership_kind:
                    crate::state::instances::ContentOwnershipKind::PackManaged,
                expected_bytes: 19,
                download_result: None,
                cancellation: CancellationToken::new(),
            },
            database_tx,
        )
        .await
        .unwrap_err();

        assert!(error.to_string().to_ascii_lowercase().contains("hash"));
        assert_eq!(
            crate::util::io::read(&full_path).await.unwrap(),
            b"existing content"
        );
        assert!(!download_path.exists());
    }

    #[cfg(not(feature = "tauri"))]
    #[tokio::test]
    async fn canceled_curseforge_database_batch_restores_previous_file() {
        let _state = stage6_state().await;
        let directory = tempfile::tempdir().unwrap();
        let full_path = directory.path().join("content.jar");
        let previous_path =
            directory.path().join("content.jar.installing.previous");
        crate::util::io::write(&full_path, b"new content")
            .await
            .unwrap();
        crate::util::io::write(&previous_path, b"old content")
            .await
            .unwrap();
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        let context = CurseForgeVerificationContext {
            reporter: None,
            loading_bar: None,
            details: InstallPhaseDetails::Empty,
            files_done: Arc::new(AtomicU64::new(0)),
            bytes_done: Arc::new(AtomicU64::new(0)),
            active_downloads: Arc::new(AtomicU64::new(0)),
            total_files: 1,
            total_bytes: 11,
            cancellation,
        };
        let batch = vec![CurseForgeDatabaseTask {
            instance_id: "database-rollback-test".to_string(),
            record: crate::state::instances::commands::ProjectFileRecord {
                relative_path: "mods/content.jar".to_string(),
                sha1: String::new(),
                size: 11,
                project_type: ProjectType::Mod,
                source_kind: ContentSourceKind::CurseForge,
                ownership_kind:
                    crate::state::instances::ContentOwnershipKind::PackManaged,
                provider_ref: None,
                origin: true,
                known_modrinth_project_id: None,
                known_modrinth_version_id: None,
            },
            verified_pending: None,
            expected_bytes: 11,
            full_path: full_path.clone(),
            previous_path: Some(previous_path.clone()),
        }];

        persist_curseforge_database_batch(&batch, &context)
            .await
            .unwrap_err();

        assert_eq!(
            crate::util::io::read(&full_path).await.unwrap(),
            b"old content"
        );
        assert!(!previous_path.exists());
    }

    #[test]
    fn dependency_fallback_requires_the_target_game_and_loader() {
        let mut target_file =
            stage8_curseforge_file(1, 101, "target.jar", 0, Vec::new(), 0);
        target_file.game_versions =
            vec!["1.21.1".to_string(), "NeoForge".to_string()];

        let mut newer_game_file =
            stage8_curseforge_file(1, 102, "newer-game.jar", 0, Vec::new(), 0);
        newer_game_file.game_versions =
            vec!["26.1.2".to_string(), "NeoForge".to_string()];

        let mut wrong_loader_file = stage8_curseforge_file(
            1,
            103,
            "wrong-loader.jar",
            0,
            Vec::new(),
            0,
        );
        wrong_loader_file.game_versions =
            vec!["1.21.1".to_string(), "Fabric".to_string()];

        assert!(file_matches_dependency_target(
            &target_file,
            Some("1.21.1"),
            Some(6),
        ));
        assert!(!file_matches_dependency_target(
            &newer_game_file,
            Some("1.21.1"),
            Some(6),
        ));
        assert!(!file_matches_dependency_target(
            &wrong_loader_file,
            Some("1.21.1"),
            Some(6),
        ));
    }

    #[test]
    fn dependency_fallback_rejects_conflicting_game_version_metadata() {
        let mut conflicting_file =
            stage8_curseforge_file(1, 101, "conflicting.jar", 0, Vec::new(), 0);
        conflicting_file.game_versions =
            vec!["1.21.1".to_string(), "NeoForge".to_string()];
        conflicting_file.sortable_game_versions =
            vec![CurseForgeSortableGameVersion {
                game_version_name: "26.1.2".to_string(),
                game_version_padded: None,
                game_version: Some("26.1.2".to_string()),
                game_version_release_date: None,
                game_version_type_id: None,
            }];

        assert!(!file_matches_dependency_target(
            &conflicting_file,
            Some("1.21.1"),
            Some(6),
        ));
    }

    #[test]
    fn dependency_selection_requires_a_minecraft_target() {
        assert_eq!(dependency_target_game_version(None), None);
        assert_eq!(dependency_target_game_version(Some("")), None);
        assert_eq!(dependency_target_game_version(Some("  ")), None);
        assert_eq!(
            dependency_target_game_version(Some("1.21.1")),
            Some("1.21.1"),
        );
    }

    #[test]
    fn dependency_selection_prefers_a_stable_target_match_over_newer_beta() {
        let mut stable =
            stage8_curseforge_file(1, 101, "stable.jar", 0, Vec::new(), 0);
        stable.game_versions =
            vec!["1.21.1".to_string(), "NeoForge".to_string()];
        stable.release_type = 1;
        stable.file_date = "2026-08-01T00:00:00Z".to_string();

        let mut newer_beta =
            stage8_curseforge_file(1, 102, "newer-beta.jar", 0, Vec::new(), 0);
        newer_beta.game_versions =
            vec!["1.21.1".to_string(), "NeoForge".to_string()];
        newer_beta.release_type = 2;
        newer_beta.file_date = "2026-08-02T00:00:00Z".to_string();

        let selected = select_best_dependency_file(
            vec![newer_beta, stable],
            Some("1.21.1"),
            Some(6),
        )
        .unwrap();

        assert_eq!(selected.id, 101);
    }

    #[test]
    fn waystones_fixture_selects_neoforge_balm_for_minecraft_1_21_1() {
        let mut balm_1_21_1 = stage8_curseforge_file(
            531_761,
            21_065,
            "balm-neoforge-1.21.1-21.0.65.jar",
            0,
            Vec::new(),
            0,
        );
        balm_1_21_1.game_versions =
            vec!["1.21.1".to_string(), "NeoForge".to_string()];
        balm_1_21_1.release_type = 1;

        let mut balm_26_1_2 = stage8_curseforge_file(
            531_761,
            26_102,
            "balm-neoforge-26.1.2-26.1.2.jar",
            0,
            Vec::new(),
            0,
        );
        balm_26_1_2.game_versions =
            vec!["26.1.2".to_string(), "NeoForge".to_string()];
        balm_26_1_2.release_type = 1;

        let selected = select_best_dependency_file(
            vec![balm_26_1_2, balm_1_21_1],
            Some("1.21.1"),
            Some(6),
        )
        .unwrap();
        assert_eq!(selected.file_name, "balm-neoforge-1.21.1-21.0.65.jar");
    }

    fn skipped_curseforge_manual_item(
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

    fn waiting_manual_job_record(
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

    fn apply_recovered_events(
        items: &mut [crate::install::model::DownloadItemSnapshot],
        events: &[InstallJobEventKind],
    ) {
        for event in events {
            let InstallJobEventKind::ContentFileRecovered { path, bytes } =
                event
            else {
                continue;
            };
            let item = items.iter_mut().find(|item| item.id == *path).unwrap();
            item.status = crate::install::model::DownloadItemStatus::Completed;
            item.bytes_downloaded = *bytes;
        }
    }

    fn reconcile_and_simulate_resume(
        items: &mut [crate::install::model::DownloadItemSnapshot],
        pending: &HashSet<(String, String)>,
        materialized: &HashSet<(String, String)>,
        resume_count: &mut usize,
    ) -> CurseForgeManualDownloadReconciliation {
        let result = curseforge_manual_download_reconciliation(
            items,
            pending,
            materialized,
        );
        apply_recovered_events(items, &result.recovered);
        let latest = curseforge_manual_download_reconciliation(
            items,
            pending,
            materialized,
        );
        if latest.should_resume() {
            *resume_count += 1;
        }
        result
    }

    #[cfg(not(feature = "tauri"))]
    async fn stage6_state() -> std::sync::Arc<State> {
        crate::event::EventState::init().await.unwrap();
        let state_root = tempfile::tempdir().unwrap().keep();
        State::init_for_test(state_root.to_string_lossy().to_string())
            .await
            .unwrap()
    }

    #[cfg(not(feature = "tauri"))]
    async fn create_stage6_instance(
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
    fn stage6_manual_download(
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

    fn stage8_legacy_manual_download(
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

    fn stage8_integrity_metadata(
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

    fn stage8_curseforge_file(
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
    async fn stage8_pending_keys(
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
    async fn import_stage6_manual_download(
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
    fn stage6_waiting_manual_job_state(
        instance_id: &str,
        items: &[(&str, &str, &str)],
    ) -> crate::install::model::InstallJobState {
        let mut state = crate::install::model::InstallJobState::new(
            crate::install::model::InstallRequest::InstallPackToExistingInstance {
                instance_id: instance_id.to_string(),
                location: crate::api::pack::install_from::CreatePackLocation::FromFile {
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
                manual_url: Some(
                    "https://www.curseforge.com/download".to_string(),
                ),
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
    async fn insert_stage6_waiting_manual_job(
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
    async fn assert_stage6_job_is_unresolved(
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

    #[test]
    fn curseforge_final_manual_completion_recovers_and_resumes() {
        let mut items =
            vec![skipped_curseforge_manual_item("mods/one.jar", "1", "10")];
        let materialized = HashSet::from([("1".to_string(), "10".to_string())]);
        let mut resume_count = 0;

        let result = reconcile_and_simulate_resume(
            &mut items,
            &HashSet::new(),
            &materialized,
            &mut resume_count,
        );

        assert_eq!(result.recovered.len(), 1);
        assert_eq!(result.materialized_exact_match_count, 1);
        assert_eq!(resume_count, 1);
        assert_eq!(
            items[0].status,
            crate::install::model::DownloadItemStatus::Completed
        );
    }

    #[test]
    fn curseforge_missing_pending_without_materialization_does_not_resume() {
        let mut items =
            vec![skipped_curseforge_manual_item("mods/one.jar", "1", "10")];
        let mut resume_count = 0;

        let result = reconcile_and_simulate_resume(
            &mut items,
            &HashSet::new(),
            &HashSet::new(),
            &mut resume_count,
        );

        assert!(result.recovered.is_empty());
        assert_eq!(result.inconsistent.len(), 1);
        assert_eq!(
            result.inconsistent[0].reason,
            "pending_missing_but_not_materialized"
        );
        assert_eq!(resume_count, 0);
        assert_eq!(
            items[0].status,
            crate::install::model::DownloadItemStatus::Skipped
        );
    }

    #[test]
    fn curseforge_pending_still_exists_does_not_resume() {
        let mut items =
            vec![skipped_curseforge_manual_item("mods/one.jar", "1", "10")];
        let pending = HashSet::from([("1".to_string(), "10".to_string())]);
        let mut resume_count = 0;

        let result = reconcile_and_simulate_resume(
            &mut items,
            &pending,
            &HashSet::new(),
            &mut resume_count,
        );

        assert!(result.recovered.is_empty());
        assert_eq!(result.unresolved_pending_count, 1);
        assert!(result.inconsistent.is_empty());
        assert_eq!(resume_count, 0);
    }

    #[test]
    fn curseforge_partial_manual_completion_does_not_resume() {
        let mut items = vec![
            skipped_curseforge_manual_item("mods/a.jar", "1", "10"),
            skipped_curseforge_manual_item("mods/b.jar", "2", "20"),
            skipped_curseforge_manual_item("mods/c.jar", "3", "30"),
        ];
        let keys = [
            ("1".to_string(), "10".to_string()),
            ("2".to_string(), "20".to_string()),
            ("3".to_string(), "30".to_string()),
        ];
        let mut pending = keys.iter().cloned().collect::<HashSet<_>>();
        let mut materialized = HashSet::new();
        let mut resume_count = 0;

        for (index, key) in keys.into_iter().enumerate() {
            assert!(pending.remove(&key));
            assert!(materialized.insert(key));
            let result = reconcile_and_simulate_resume(
                &mut items,
                &pending,
                &materialized,
                &mut resume_count,
            );

            assert_eq!(result.recovered.len(), 1);
            assert!(items[..=index].iter().all(|item| {
                item.status
                    == crate::install::model::DownloadItemStatus::Completed
            }));
            assert!(items[index + 1..].iter().all(|item| {
                item.status
                    == crate::install::model::DownloadItemStatus::Skipped
            }));
            assert_eq!(resume_count, usize::from(index == 2));
        }
    }

    #[test]
    fn curseforge_repeated_reconciliation_is_idempotent() {
        let mut items =
            vec![skipped_curseforge_manual_item("mods/one.jar", "1", "10")];
        let materialized = HashSet::from([("1".to_string(), "10".to_string())]);
        let mut resume_count = 0;
        let mut waiting_for_user = true;
        let mut recovered_count = 0;

        for _ in 0..3 {
            if !waiting_for_user {
                continue;
            }
            let result = reconcile_and_simulate_resume(
                &mut items,
                &HashSet::new(),
                &materialized,
                &mut resume_count,
            );
            recovered_count += result.recovered.len();
            if resume_count > 0 {
                waiting_for_user = false;
            }
        }

        assert_eq!(recovered_count, 1);
        assert_eq!(resume_count, 1);
    }

    #[test]
    fn curseforge_startup_selects_only_reconcilable_jobs() {
        let curseforge_job = waiting_manual_job_record(
            "curseforge-instance",
            crate::install::model::InstallRequest::InstallCurseForgeContent {
                request: CurseForgeInstallRequest {
                    instance_id: "curseforge-instance".to_string(),
                    project_id: 1,
                    file_id: 10,
                    project_type: "mod".to_string(),
                    ownership_kind:
                        crate::state::instances::ContentOwnershipKind::PackManaged,
                    manual_operation_kind: crate::state::instances::ManualDownloadOperationKind::PackInstall,
                    game_version: None,
                    mod_loader_type: None,
                    world_name: None,
                    install_dependencies: true,
                    excluded_dependency_project_ids: Vec::new(),
					force_dependency_project_ids: Vec::new(),
                    dependency_plan_id: None,
                    defer_persistence: false,
                    verification_tx: None,
                    pre_resolved_relative_path: None,
        expected_file_name: None,
                },
                display_title: "CurseForge".to_string(),
                display_icon: None,
            },
        );
        let modrinth_job = waiting_manual_job_record(
            "modrinth-instance",
            crate::install::model::InstallRequest::InstallPackToExistingInstance {
                instance_id: "modrinth-instance".to_string(),
                location: crate::api::pack::install_from::CreatePackLocation::FromVersionId {
                    project_id: "project".to_string(),
                    version_id: "version".to_string(),
                    title: "Modrinth".to_string(),
                    icon_url: None,
                },
                post_install_edit: None,
            },
        );
        assert_eq!(
            curseforge_waiting_job_instance_ids(&[
                curseforge_job,
                modrinth_job,
            ]),
            vec!["curseforge-instance".to_string()]
        );

        let mut items =
            vec![skipped_curseforge_manual_item("mods/one.jar", "1", "10")];
        let materialized = HashSet::from([("1".to_string(), "10".to_string())]);
        let mut resume_count = 0;

        let result = reconcile_and_simulate_resume(
            &mut items,
            &HashSet::new(),
            &materialized,
            &mut resume_count,
        );

        assert_eq!(result.recovered.len(), 1);
        assert_eq!(resume_count, 1);
    }

    #[test]
    fn curseforge_wrong_instance_or_release_never_recovers() {
        let mut items =
            vec![skipped_curseforge_manual_item("mods/one.jar", "1", "10")];
        let other_instance_or_release = HashSet::from([
            ("1".to_string(), "11".to_string()),
            ("2".to_string(), "10".to_string()),
        ]);
        let mut resume_count = 0;

        let result = reconcile_and_simulate_resume(
            &mut items,
            &HashSet::new(),
            &other_instance_or_release,
            &mut resume_count,
        );

        assert!(result.recovered.is_empty());
        assert_eq!(result.inconsistent.len(), 1);
        assert_eq!(resume_count, 0);
    }

    #[cfg(not(feature = "tauri"))]
    #[tokio::test]
    async fn curseforge_pending_is_scoped_by_instance() {
        let (state, instance_a) = create_stage6_instance("scope A").await;
        let (_, instance_b) = create_stage6_instance("scope B").await;
        let completed = stage6_manual_download(
            101,
            1001,
            "scope-one.jar",
            b"scope-one-content",
        );
        let other_release = stage6_manual_download(
            101,
            1002,
            "scope-two.jar",
            b"scope-two-content",
        );
        persist_manual_download(&instance_a, &completed)
            .await
            .unwrap();
        persist_manual_download(&instance_a, &other_release)
            .await
            .unwrap();
        persist_manual_download(&instance_b, &completed)
            .await
            .unwrap();
        let job_a = insert_stage6_waiting_manual_job(
            &state,
            &instance_a,
            &[
                ("mods/scope-one.jar", "101", "1001"),
                ("mods/scope-two.jar", "101", "1002"),
            ],
        )
        .await;
        let job_b = insert_stage6_waiting_manual_job(
            &state,
            &instance_b,
            &[("mods/scope-one.jar", "101", "1001")],
        )
        .await;

        import_stage6_manual_download(
            &instance_a,
            &completed,
            b"scope-one-content",
        )
        .await;

        let pending_a = crate::state::instances::adapters::sqlite::content_rows::get_pending_manual_downloads(
            &instance_a,
            &state.pool,
        )
        .await
        .unwrap();
        let pending_b = crate::state::instances::adapters::sqlite::content_rows::get_pending_manual_downloads(
            &instance_b,
            &state.pool,
        )
        .await
        .unwrap();
        assert_eq!(pending_a.len(), 1);
        assert_eq!(pending_a[0].provider_project_id, "101");
        assert_eq!(pending_a[0].provider_release_id, "1002");
        assert_eq!(pending_b.len(), 1);
        assert_eq!(pending_b[0].provider_project_id, "101");
        assert_eq!(pending_b[0].provider_release_id, "1001");

        let reconciled_a = crate::install::store::get_required(job_a, &state)
            .await
            .unwrap();
        assert_eq!(
            reconciled_a.status,
            crate::install::model::InstallJobStatus::WaitingForUser
        );
        let items_a = reconciled_a.snapshot().items;
        assert_eq!(
            items_a
                .iter()
                .find(|item| item.id == "mods/scope-one.jar")
                .unwrap()
                .status,
            crate::install::model::DownloadItemStatus::Completed
        );
        assert_eq!(
            items_a
                .iter()
                .find(|item| item.id == "mods/scope-two.jar")
                .unwrap()
                .status,
            crate::install::model::DownloadItemStatus::Skipped
        );
        assert_eq!(
            reconciled_a
                .state
                .events
                .iter()
                .filter(|event| matches!(
                    event.kind,
                    InstallJobEventKind::ContentFileRecovered { .. }
                ))
                .count(),
            1
        );
        assert_stage6_job_is_unresolved(&state, job_b, "mods/scope-one.jar")
            .await;
        crate::install::store::dismiss(job_a, &state).await.unwrap();
        crate::install::store::dismiss(job_b, &state).await.unwrap();
        crate::api::instance::remove(&instance_a).await.unwrap();
        crate::api::instance::remove(&instance_b).await.unwrap();
    }

    #[cfg(not(feature = "tauri"))]
    #[tokio::test]
    async fn curseforge_missing_pending_without_materialization_keeps_real_job_waiting()
     {
        let (state, instance_id) =
            create_stage6_instance("missing materialization").await;
        let job_id = insert_stage6_waiting_manual_job(
            &state,
            &instance_id,
            &[("mods/missing.jar", "201", "2001")],
        )
        .await;

        reconcile_curseforge_waiting_jobs_for_instance_with_state(
            &instance_id,
            &state,
        )
        .await
        .unwrap();

        assert_stage6_job_is_unresolved(&state, job_id, "mods/missing.jar")
            .await;
        crate::install::store::dismiss(job_id, &state)
            .await
            .unwrap();
        crate::api::instance::remove(&instance_id).await.unwrap();
    }

    #[cfg(not(feature = "tauri"))]
    #[tokio::test]
    async fn curseforge_pending_still_exists_keeps_real_job_waiting() {
        let (state, instance_id) = create_stage6_instance("pending").await;
        let download = stage6_manual_download(
            301,
            3001,
            "pending.jar",
            b"pending-content",
        );
        persist_manual_download(&instance_id, &download)
            .await
            .unwrap();
        let job_id = insert_stage6_waiting_manual_job(
            &state,
            &instance_id,
            &[("mods/pending.jar", "301", "3001")],
        )
        .await;

        reconcile_curseforge_waiting_jobs_for_instance_with_state(
            &instance_id,
            &state,
        )
        .await
        .unwrap();

        assert_stage6_job_is_unresolved(&state, job_id, "mods/pending.jar")
            .await;
        crate::install::store::dismiss(job_id, &state)
            .await
            .unwrap();
        crate::api::instance::remove(&instance_id).await.unwrap();
    }

    #[cfg(not(feature = "tauri"))]
    #[tokio::test]
    async fn curseforge_wrong_release_keeps_real_job_waiting() {
        let (state, instance_id) =
            create_stage6_instance("wrong release").await;
        let other_release = stage6_manual_download(
            401,
            4002,
            "other-release.jar",
            b"other-release-content",
        );
        persist_manual_download(&instance_id, &other_release)
            .await
            .unwrap();
        import_stage6_manual_download(
            &instance_id,
            &other_release,
            b"other-release-content",
        )
        .await;
        let job_id = insert_stage6_waiting_manual_job(
            &state,
            &instance_id,
            &[("mods/target-release.jar", "401", "4001")],
        )
        .await;

        reconcile_curseforge_waiting_jobs_for_instance_with_state(
            &instance_id,
            &state,
        )
        .await
        .unwrap();

        assert_stage6_job_is_unresolved(
            &state,
            job_id,
            "mods/target-release.jar",
        )
        .await;
        crate::install::store::dismiss(job_id, &state)
            .await
            .unwrap();
        crate::api::instance::remove(&instance_id).await.unwrap();
    }

    #[cfg(not(feature = "tauri"))]
    #[tokio::test]
    async fn curseforge_startup_reconciliation_recovers_and_resumes_once() {
        let (state, instance_id) = create_stage6_instance("startup").await;
        let download = stage6_manual_download(
            501,
            5001,
            "startup.jar",
            b"startup-content",
        );
        persist_manual_download(&instance_id, &download)
            .await
            .unwrap();
        import_stage6_manual_download(
            &instance_id,
            &download,
            b"startup-content",
        )
        .await;
        let job_id = insert_stage6_waiting_manual_job(
            &state,
            &instance_id,
            &[("mods/startup.jar", "501", "5001")],
        )
        .await;
        let resume_count =
            std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let state_for_resume = std::sync::Arc::clone(&state);
        let resume_count_for_callback = std::sync::Arc::clone(&resume_count);
        let mut resume_job = move |job_id| {
            let state = std::sync::Arc::clone(&state_for_resume);
            let resume_count =
                std::sync::Arc::clone(&resume_count_for_callback);
            async move {
                let current =
                    crate::install::store::get_required(job_id, &state).await?;
                let claimed = crate::install::store::update_status_if(
                    job_id,
                    crate::install::model::InstallJobStatus::WaitingForUser,
                    crate::install::model::InstallJobStatus::Queued,
                    &current.state,
                    &state,
                )
                .await?;
                assert!(
                    claimed.is_some(),
                    "resume status claim must succeed once"
                );
                resume_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(())
            }
        };

        for _ in 0..3 {
            reconcile_persisted_curseforge_waiting_jobs_with_resume(
                &state,
                &mut resume_job,
            )
            .await
            .unwrap();
        }

        let reconciled = crate::install::store::get_required(job_id, &state)
            .await
            .unwrap();
        assert_eq!(
            reconciled.status,
            crate::install::model::InstallJobStatus::Queued
        );
        assert_eq!(
            reconciled
                .snapshot()
                .items
                .iter()
                .find(|item| item.id == "mods/startup.jar")
                .unwrap()
                .status,
            crate::install::model::DownloadItemStatus::Completed
        );
        assert_eq!(
            reconciled
                .state
                .events
                .iter()
                .filter(|event| matches!(
                    event.kind,
                    InstallJobEventKind::ContentFileRecovered { .. }
                ))
                .count(),
            1
        );
        assert_eq!(resume_count.load(std::sync::atomic::Ordering::SeqCst), 1);
        crate::install::store::dismiss(job_id, &state)
            .await
            .unwrap();
        crate::api::instance::remove(&instance_id).await.unwrap();
    }

    #[test]
    fn unified_search_hit_accepts_null_gallery() {
        let hit: UnifiedSearchHit = serde_json::from_value(serde_json::json!({
            "provider": "curseforge",
            "project_id": "250419",
            "slug": null,
            "author": "DarkhaxDev",
            "author_url": null,
            "title": "Enchantment Descriptions",
            "description": "description",
            "project_type": "mod",
            "categories": [],
            "versions": [],
            "downloads": 0,
            "icon_url": null,
            "date_created": "2026-01-01T00:00:00Z",
            "date_modified": "2026-01-01T00:00:00Z",
            "latest_version": null,
            "gallery": null,
            "website_url": null,
            "source_url": null,
            "allow_mod_distribution": null,
        }))
        .unwrap();

        assert!(hit.gallery.is_empty());
    }

    #[test]
    fn curseforge_file_accepts_null_modules() {
        let file: CurseForgeFile = serde_json::from_value(serde_json::json!({
            "id": 4031925,
            "gameId": 432,
            "modId": 250419,
            "isAvailable": true,
            "displayName": "old.jar",
            "fileName": "old.jar",
            "releaseType": 1,
            "fileStatus": 4,
            "fileDate": "2026-01-01T00:00:00Z",
            "fileLength": 1,
            "downloadCount": 0,
            "fileFingerprint": 0,
            "modules": null,
        }))
        .unwrap();

        assert!(file.modules.is_empty());
    }

    #[test]
    fn fingerprint_ignores_curseforge_whitespace() {
        assert_eq!(
            compute_fingerprint(b"abc\r\n def\t"),
            compute_fingerprint(b"abcdef")
        );
    }

    #[test]
    fn streaming_fingerprint_matches_reference_across_chunk_boundaries() {
        let data = (0..4099)
            .map(|index| match index % 31 {
                0 => b' ',
                1 => b'\n',
                2 => b'\r',
                3 => b'\t',
                _ => (index % 251) as u8,
            })
            .collect::<Vec<_>>();
        let normalized = data
            .iter()
            .copied()
            .filter(|byte| !is_curseforge_fingerprint_whitespace(*byte))
            .collect::<Vec<_>>();
        let expected = murmur2(&normalized, 1);

        for chunk_size in [1, 2, 3, 4, 5, 31, 256, 1024] {
            let mut hasher =
                CurseForgeFingerprintHasher::new(normalized.len() as u32);
            for chunk in data.chunks(chunk_size) {
                hasher.update(chunk);
            }
            assert_eq!(hasher.finish(), expected, "chunk size {chunk_size}");
        }
        assert_eq!(compute_fingerprint(&data), expected);
    }

    #[tokio::test]
    async fn file_fingerprint_streams_sha1_and_supports_cancellation() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("large-fingerprint.jar");
        let mut data = vec![b'a'; 256 * 1024 + 17];
        data[3] = b' ';
        data[256 * 1024 - 1] = b'\n';
        crate::util::io::write(&path, &data).await.unwrap();

        let (size, sha1, fingerprint) =
            fingerprint_and_sha1_file(&path, None).await.unwrap();
        assert_eq!(size, data.len() as u64);
        assert_eq!(sha1, sha1_smol::Sha1::from(&data).hexdigest());
        assert_eq!(fingerprint, compute_fingerprint(&data));

        let cancellation = CancellationToken::new();
        cancellation.cancel();
        let error = fingerprint_and_sha1_file(&path, Some(&cancellation))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("canceled"));
    }

    #[test]
    fn project_types_are_provider_qualified() {
        assert_eq!(project_type_for_class(Some(6)), "mod");
        assert_eq!(project_type_for_class(Some(4471)), "modpack");
        assert_eq!(project_type_for_class(Some(6552)), "shader");
        assert_eq!(project_type_for_class(Some(6945)), "datapack");
    }

    #[test]
    fn official_only_requests_exclude_mirror_routes() {
        let routes =
            request_routes("/v1/mods/search", MirrorPolicy::OfficialOnly);

        assert!(
            routes
                .iter()
                .all(|route| route.source == RequestRouteSource::Official)
        );
    }

    #[test]
    fn mirror_not_found_tries_next_route() {
        let route = RequestRoute {
            url: String::new(),
            use_api_key: false,
            use_system_proxy: false,
            source: RequestRouteSource::Mirror,
        };

        assert!(should_try_next_route(&route, StatusCode::NOT_FOUND, true));
    }

    #[test]
    fn official_forbidden_response_tries_proxy_route() {
        let route = RequestRoute {
            url: String::new(),
            use_api_key: true,
            use_system_proxy: false,
            source: RequestRouteSource::Official,
        };

        assert!(should_try_next_route(&route, StatusCode::FORBIDDEN, true));
    }

    #[test]
    fn projects_accept_negative_popularity_ranks() {
        let project = serde_json::from_value::<CurseForgeProject>(json!({
            "id": 1,
            "gameId": MINECRAFT_GAME_ID,
            "name": "Fixture",
            "slug": "fixture",
            "links": {},
            "summary": "Fixture project",
            "status": 4,
            "downloadCount": 0,
            "isFeatured": false,
            "primaryCategoryId": 6,
            "categories": [],
            "classId": 6,
            "authors": [],
            "logo": null,
            "screenshots": [],
            "mainFileId": 0,
            "latestFiles": [],
            "latestFilesIndexes": [],
            "dateCreated": "2026-01-01T00:00:00Z",
            "dateModified": "2026-01-01T00:00:00Z",
            "dateReleased": "2026-01-01T00:00:00Z",
            "allowModDistribution": true,
            "gamePopularityRank": -10,
            "isAvailable": true
        }))
        .unwrap();

        assert_eq!(project.game_popularity_rank, Some(-10));
    }

    #[test]
    fn category_cache_can_be_filtered_for_each_project_class() {
        let categories = vec![
            category(6, None, true),
            category(406, Some(6), false),
            category(4471, None, true),
            category(4481, Some(4471), false),
        ];

        let mods = filter_categories(categories.clone(), Some(6));
        assert_eq!(
            mods.iter().map(|category| category.id).collect::<Vec<_>>(),
            vec![6, 406]
        );

        let modpacks = filter_categories(categories, Some(4471));
        assert_eq!(
            modpacks
                .iter()
                .map(|category| category.id)
                .collect::<Vec<_>>(),
            vec![4471, 4481]
        );
    }

    fn category(
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

    #[test]
    fn manual_downloads_open_the_official_download_page() {
        assert_eq!(
            curseforge_file_page_url(
                Some("https://www.curseforge.com/minecraft/mc-mods/example"),
                12345,
            ),
            Some(
                "https://www.curseforge.com/minecraft/mc-mods/example/download/12345"
                    .to_string()
            )
        );
        assert_eq!(
            curseforge_file_page_url(
                Some("https://www.curseforge.com/minecraft/mc-mods/example/files/12345?tab=files"),
                12345,
            ),
            Some(
                "https://www.curseforge.com/minecraft/mc-mods/example/download/12345"
                    .to_string()
            )
        );
        assert_eq!(
            curseforge_file_page_url(
                Some("https://example.com/project"),
                12345
            ),
            Some("https://example.com/project".to_string())
        );
    }

    #[test]
    fn browser_duplicate_download_names_match_the_expected_file() {
        assert!(crate::util::downloads::browser_download_file_name_matches(
            "example-mod (1).jar",
            "example-mod.jar"
        ));
        assert!(crate::util::downloads::browser_download_file_name_matches(
            "EXAMPLE-MOD.JAR",
            "example-mod.jar"
        ));
        assert!(!crate::util::downloads::browser_download_file_name_matches(
            "example-mod-fabric.jar",
            "example-mod.jar"
        ));
        assert!(!crate::util::downloads::browser_download_file_name_matches(
            "example-mod.jar.crdownload",
            "example-mod.jar"
        ));
    }

    #[tokio::test]
    async fn curseforge_metadata_less_exact_name_is_not_trusted() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("example-mod.jar");
        crate::util::io::write(&path, b"wrong exact-name jar")
            .await
            .unwrap();
        let download = stage8_legacy_manual_download(1, 2, "example-mod.jar");

        let error = verify_manual_download_candidate_with_integrity(
            &path, &download, true,
        )
        .await
        .expect_err("filename alone must not verify a candidate");

        assert!(matches!(
            error.raw.as_ref(),
            ErrorKind::InputError(message)
                if message
                    == "The required CurseForge file has no usable integrity metadata"
        ));
    }

    #[tokio::test]
    async fn curseforge_hydration_is_scoped_to_exact_release() {
        let download =
            stage8_legacy_manual_download(11, 101, "exact-release.jar");
        let error = ensure_manual_download_integrity_metadata_with(
            &download,
            |project_id, file_id| async move {
                assert_eq!(project_id, 11);
                assert_eq!(file_id, 101);
                Ok(CurseForgeManualDownloadIntegrityMetadata {
                    project_id,
                    file_id: 102,
                    hashes: vec![CurseForgeFileHash {
                        value: "wrong-release-sha1".to_string(),
                        algo: 1,
                    }],
                    file_length: 1,
                    file_fingerprint: 0,
                })
            },
        )
        .await
        .expect_err("metadata for another file must be rejected");

        assert!(matches!(
            error.raw.as_ref(),
            ErrorKind::InputError(message)
                if message
                    == "CurseForge returned metadata for a different project or file"
        ));
    }

    #[tokio::test]
    async fn curseforge_existing_integrity_skips_hydration() {
        let mut sha1_download =
            stage8_legacy_manual_download(12, 201, "existing-sha1.jar");
        sha1_download.hashes = vec![CurseForgeFileHash {
            value: "existing-sha1".to_string(),
            algo: 1,
        }];
        let hydrated = ensure_manual_download_integrity_metadata_with(
            &sha1_download,
            |_, _| async {
                Err(ErrorKind::InputError(
                    "metadata resolver must not run".to_string(),
                )
                .into())
            },
        )
        .await
        .unwrap();
        assert_eq!(hydrated.hashes[0].value, "existing-sha1");

        let mut fingerprint_download =
            stage8_legacy_manual_download(12, 202, "existing-fingerprint.jar");
        fingerprint_download.file_fingerprint = 12345;
        let hydrated = ensure_manual_download_integrity_metadata_with(
            &fingerprint_download,
            |_, _| async {
                Err(ErrorKind::InputError(
                    "metadata resolver must not run".to_string(),
                )
                .into())
            },
        )
        .await
        .unwrap();
        assert_eq!(hydrated.file_fingerprint, 12345);
    }

    #[tokio::test]
    async fn curseforge_hydrated_fingerprint_verifies_candidate() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("fingerprint.jar");
        let bytes = b"legacy fingerprint content";
        crate::util::io::write(&path, bytes).await.unwrap();
        let download =
            stage8_legacy_manual_download(13, 301, "fingerprint.jar");
        let metadata = stage8_integrity_metadata(
            &download,
            Vec::new(),
            bytes.len() as u64,
            compute_fingerprint(bytes) as u64,
        );
        let hydrated = ensure_manual_download_integrity_metadata_with(
            &download,
            move |project_id, file_id| async move {
                assert_eq!((project_id, file_id), (13, 301));
                Ok(metadata)
            },
        )
        .await
        .unwrap();

        assert!(
            verify_manual_download_candidate_with_integrity(
                &path, &hydrated, true,
            )
            .await
            .unwrap()
            .is_some()
        );
    }

    #[tokio::test]
    async fn curseforge_manual_materialization_is_copy_isolated() {
        let source_directory = tempfile::tempdir().unwrap();
        let destination_directory = tempfile::tempdir().unwrap();
        let source = source_directory.path().join("manual.jar");
        let destination = destination_directory.path().join("manual.jar");
        let verified_bytes = b"verified manual bytes";
        crate::util::io::write(&source, verified_bytes)
            .await
            .unwrap();
        let (verified_size, verified_sha1) =
            sha1_file_async(&source).await.unwrap();

        crate::state::materialize_verified_project_download_copy(
            &source,
            &destination,
            verified_size,
            &verified_sha1,
        )
        .await
        .unwrap();
        crate::util::io::write(&source, b"mutated source content")
            .await
            .unwrap();

        assert_eq!(
            crate::util::io::read(&destination).await.unwrap(),
            verified_bytes
        );
        assert_eq!(
            sha1_file_async(&destination).await.unwrap(),
            (verified_size, verified_sha1)
        );
    }

    #[tokio::test]
    async fn curseforge_fingerprint_recognition_rechecks_current_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("recognized.jar");
        let matched_bytes = b"matched fingerprint bytes";
        let expected_fingerprint = compute_fingerprint(matched_bytes) as u64;
        crate::util::io::write(&path, matched_bytes).await.unwrap();
        assert!(
            verify_recognized_curseforge_file(&path, expected_fingerprint)
                .await
                .unwrap()
                .is_some()
        );

        crate::util::io::write(&path, b"changed after fingerprint match")
            .await
            .unwrap();
        assert!(
            verify_recognized_curseforge_file(&path, expected_fingerprint)
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn curseforge_localized_candidate_still_requires_integrity() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("[测试]example-mod (1).jar");
        crate::util::io::write(&path, b"expected curseforge file")
            .await
            .unwrap();
        let (_, sha1) = sha1_file_async(&path).await.unwrap();
        let download = CurseForgeManualDownload {
            project_id: 1,
            file_id: 2,
            file_name: "example-mod.jar".to_string(),
            ownership_kind:
                crate::state::instances::ContentOwnershipKind::PackManaged,
            operation_kind:
                crate::state::instances::ManualDownloadOperationKind::PackInstall,
            website_url: None,
            project_type: "mod".to_string(),
            project_slug: String::new(),
            target_folder: "mods".to_string(),
            hashes: vec![CurseForgeFileHash {
                value: sha1,
                algo: 1,
            }],
            file_length: 24,
            file_fingerprint: 0,
        };

        assert!(
            find_manual_download_candidate(directory.path(), &download)
                .await
                .unwrap()
                .is_some()
        );
        crate::util::io::write(&path, b"same-length-wrong-bytes!")
            .await
            .unwrap();
        assert!(
            find_manual_download_candidate(directory.path(), &download)
                .await
                .unwrap()
                .is_none()
        );
    }

    #[cfg(not(feature = "tauri"))]
    #[tokio::test]
    async fn curseforge_legacy_pending_imports_after_exact_metadata_hydration()
    {
        let (state, instance_id) =
            create_stage6_instance("legacy hydration").await;
        let download =
            stage8_legacy_manual_download(14, 401, "legacy-hydration.jar");
        persist_manual_download(&instance_id, &download)
            .await
            .unwrap();
        let source_directory = tempfile::tempdir().unwrap();
        let source = source_directory.path().join(&download.file_name);
        let bytes = b"legacy hydrated content";
        crate::util::io::write(&source, bytes).await.unwrap();
        let metadata = stage8_integrity_metadata(
            &download,
            vec![CurseForgeFileHash {
                value: sha1_smol::Sha1::from(bytes).hexdigest(),
                algo: 1,
            }],
            bytes.len() as u64,
            0,
        );

        let imported =
            import_pending_manual_download_file_with_integrity_resolver(
                &instance_id,
                download.project_id,
                download.file_id,
                source.clone(),
                move |project_id, file_id| async move {
                    assert_eq!((project_id, file_id), (14, 401));
                    Ok(metadata)
                },
            )
            .await
            .unwrap();

        assert!(source.exists());
        assert_eq!(imported.relative_path, "mods/legacy-hydration.jar");
        assert!(
            crate::state::instances::adapters::sqlite::content_rows::get_pending_manual_downloads(
                &instance_id,
                &state.pool,
            )
            .await
            .unwrap()
            .is_empty()
        );
        let snapshot = crate::api::instance::get_content_snapshot(&instance_id)
            .await
            .unwrap();
        assert!(snapshot.items.iter().any(|item| {
            item.provider_project_id.as_deref() == Some("14")
                && item.provider_release_id.as_deref() == Some("401")
                && item.materialization_state
                    == crate::state::instances::PackMemberMaterializationState::Present
                && item.content.is_some()
        }));
        crate::api::instance::remove(&instance_id).await.unwrap();
    }

    #[cfg(not(feature = "tauri"))]
    #[tokio::test]
    async fn curseforge_missing_integrity_after_hydration_keeps_pending_unresolved()
     {
        let (state, instance_id) =
            create_stage6_instance("missing hydrated integrity").await;
        let download =
            stage8_legacy_manual_download(15, 501, "missing-integrity.jar");
        persist_manual_download(&instance_id, &download)
            .await
            .unwrap();
        let job_id = insert_stage6_waiting_manual_job(
            &state,
            &instance_id,
            &[("mods/missing-integrity.jar", "15", "501")],
        )
        .await;
        let source_directory = tempfile::tempdir().unwrap();
        let source = source_directory.path().join(&download.file_name);
        crate::util::io::write(&source, b"unverifiable content")
            .await
            .unwrap();
        let metadata = stage8_integrity_metadata(&download, Vec::new(), 20, 0);

        let fetch_error =
            import_pending_manual_download_file_with_integrity_resolver(
                &instance_id,
                download.project_id,
                download.file_id,
                source.clone(),
                |project_id, file_id| async move {
                    assert_eq!((project_id, file_id), (15, 501));
                    Err(ErrorKind::InputError(
                        "simulated CurseForge metadata fetch failure"
                            .to_string(),
                    )
                    .into())
                },
            )
            .await
            .expect_err("metadata fetch failures must block import");
        assert!(matches!(
            fetch_error.raw.as_ref(),
            ErrorKind::InputError(message)
                if message
                    == "simulated CurseForge metadata fetch failure"
        ));

        let error =
            import_pending_manual_download_file_with_integrity_resolver(
                &instance_id,
                download.project_id,
                download.file_id,
                source.clone(),
                move |project_id, file_id| async move {
                    assert_eq!((project_id, file_id), (15, 501));
                    Ok(metadata)
                },
            )
            .await
            .expect_err("missing authoritative integrity must block import");

        assert!(matches!(
            error.raw.as_ref(),
            ErrorKind::InputError(message)
                if message
                    == "The required CurseForge file has no usable integrity metadata"
        ));
        assert!(source.exists());
        assert_eq!(
            crate::state::instances::adapters::sqlite::content_rows::get_pending_manual_downloads(
                &instance_id,
                &state.pool,
            )
            .await
            .unwrap()
            .len(),
            1
        );
        let target = crate::api::instance::get_full_path(&instance_id)
            .await
            .unwrap()
            .join("mods/missing-integrity.jar");
        assert!(!target.exists());
        assert_stage6_job_is_unresolved(
            &state,
            job_id,
            "mods/missing-integrity.jar",
        )
        .await;
        let job = crate::install::store::get_required(job_id, &state)
            .await
            .unwrap();
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
        crate::install::store::dismiss(job_id, &state)
            .await
            .unwrap();
        crate::api::instance::remove(&instance_id).await.unwrap();
    }

    #[cfg(not(feature = "tauri"))]
    #[tokio::test]
    async fn curseforge_generic_record_does_not_complete_pending() {
        let (state, instance_id) =
            create_stage6_instance("generic record provenance").await;
        let download =
            stage8_legacy_manual_download(16, 601, "generic-record.jar");
        persist_manual_download(&instance_id, &download)
            .await
            .unwrap();
        let relative_path = "mods/generic-record.jar";
        let full_path = crate::api::instance::get_full_path(&instance_id)
            .await
            .unwrap()
            .join(relative_path);
        let bytes = b"generic recorded bytes";
        crate::util::io::write(&full_path, bytes).await.unwrap();
        let (_, sha1) = sha1_file_async(&full_path).await.unwrap();
        let provider_ref = ContentProviderRef::CurseForge {
            project_id: CurseForgeProjectId::new(16).unwrap(),
            file_id: Some(CurseForgeFileId::new(601).unwrap()),
        };

        crate::state::record_project_file_atomic(
            &instance_id,
            relative_path,
            &sha1,
            bytes.len() as u64,
            ProjectType::Mod,
            ContentSourceKind::CurseForge,
            crate::state::instances::ContentOwnershipKind::PackManaged,
            Some(&provider_ref),
            true,
            None,
            &state,
        )
        .await
        .unwrap();

        assert!(
            stage8_pending_keys(&state, &instance_id)
                .await
                .contains(&("16".to_string(), "601".to_string()))
        );
        let snapshot = crate::api::instance::get_content_snapshot(&instance_id)
            .await
            .unwrap();
        assert!(snapshot.items.iter().any(|item| {
            item.provider_project_id.as_deref() == Some("16")
                && item.provider_release_id.as_deref() == Some("601")
                && item.materialization_state
                    == crate::state::instances::PackMemberMaterializationState::Present
                && item.content.is_some()
        }));
        crate::api::instance::remove(&instance_id).await.unwrap();
    }

    #[cfg(not(feature = "tauri"))]
    #[tokio::test]
    async fn curseforge_verified_record_completes_only_exact_pending() {
        let (state, instance_a) =
            create_stage6_instance("verified record A").await;
        let (_, instance_b) = create_stage6_instance("verified record B").await;
        let file_1 = stage8_legacy_manual_download(17, 701, "one.jar");
        let file_2 = stage8_legacy_manual_download(17, 702, "two.jar");
        persist_manual_download(&instance_a, &file_1).await.unwrap();
        persist_manual_download(&instance_a, &file_2).await.unwrap();
        persist_manual_download(&instance_b, &file_1).await.unwrap();
        let relative_path = "mods/one.jar";
        let full_path = crate::api::instance::get_full_path(&instance_a)
            .await
            .unwrap()
            .join(relative_path);
        let bytes = b"verified exact record";
        crate::util::io::write(&full_path, bytes).await.unwrap();
        let (_, sha1) = sha1_file_async(&full_path).await.unwrap();

        crate::state::record_verified_curseforge_project_file_atomic(
            &instance_a,
            relative_path,
            &sha1,
            bytes.len() as u64,
            ProjectType::Mod,
            ContentSourceKind::CurseForge,
            crate::state::instances::ContentOwnershipKind::PackManaged,
            CurseForgeProjectId::new(17).unwrap(),
            CurseForgeFileId::new(701).unwrap(),
            true,
            &state,
        )
        .await
        .unwrap();

        assert_eq!(
            stage8_pending_keys(&state, &instance_a).await,
            HashSet::from([("17".to_string(), "702".to_string())])
        );
        assert_eq!(
            stage8_pending_keys(&state, &instance_b).await,
            HashSet::from([("17".to_string(), "701".to_string())])
        );
        crate::api::instance::remove(&instance_a).await.unwrap();
        crate::api::instance::remove(&instance_b).await.unwrap();
    }

    #[cfg(not(feature = "tauri"))]
    #[tokio::test]
    async fn curseforge_batch_completes_only_authoritatively_verified_pending()
    {
        let (state, instance_id) =
            create_stage6_instance("verified record batch").await;
        let verified = stage8_legacy_manual_download(170, 701, "verified.jar");
        let generic = stage8_legacy_manual_download(170, 702, "generic.jar");
        persist_manual_download(&instance_id, &verified)
            .await
            .unwrap();
        persist_manual_download(&instance_id, &generic)
            .await
            .unwrap();
        let records = [
            crate::state::instances::commands::ProjectFileRecord {
                relative_path: "mods/verified.jar".to_string(),
                sha1: "verified-sha1".to_string(),
                size: 10,
                project_type: ProjectType::Mod,
                source_kind: ContentSourceKind::CurseForge,
                ownership_kind:
                    crate::state::instances::ContentOwnershipKind::PackManaged,
                provider_ref: Some(ContentProviderRef::CurseForge {
                    project_id: CurseForgeProjectId::new(170).unwrap(),
                    file_id: Some(CurseForgeFileId::new(701).unwrap()),
                }),
                origin: true,
                known_modrinth_project_id: None,
                known_modrinth_version_id: None,
            },
            crate::state::instances::commands::ProjectFileRecord {
                relative_path: "mods/generic.jar".to_string(),
                sha1: "generic-sha1".to_string(),
                size: 11,
                project_type: ProjectType::Mod,
                source_kind: ContentSourceKind::CurseForge,
                ownership_kind:
                    crate::state::instances::ContentOwnershipKind::PackManaged,
                provider_ref: Some(ContentProviderRef::CurseForge {
                    project_id: CurseForgeProjectId::new(170).unwrap(),
                    file_id: Some(CurseForgeFileId::new(702).unwrap()),
                }),
                origin: true,
                known_modrinth_project_id: None,
                known_modrinth_version_id: None,
            },
        ];

        crate::state::instances::commands::record_project_files_with_verified_curseforge_atomic(
            &instance_id,
            &records,
            &[(
                CurseForgeProjectId::new(170).unwrap(),
                CurseForgeFileId::new(701).unwrap(),
            )],
            &state,
        )
        .await
        .unwrap();

        assert_eq!(
            stage8_pending_keys(&state, &instance_id).await,
            HashSet::from([("170".to_string(), "702".to_string())])
        );
        crate::api::instance::remove(&instance_id).await.unwrap();
    }

    #[cfg(not(feature = "tauri"))]
    #[tokio::test]
    async fn curseforge_manual_materialization_detects_source_change() {
        let (state, instance_id) =
            create_stage6_instance("manual copy TOCTOU").await;
        let download = stage8_legacy_manual_download(18, 801, "toctou.jar");
        persist_manual_download(&instance_id, &download)
            .await
            .unwrap();
        let source_directory = tempfile::tempdir().unwrap();
        let source = source_directory.path().join("toctou.jar");
        crate::util::io::write(&source, b"original verified bytes")
            .await
            .unwrap();
        let (verified_size, verified_sha1) =
            sha1_file_async(&source).await.unwrap();
        crate::util::io::write(&source, b"changed after verification")
            .await
            .unwrap();
        let destination = crate::api::instance::get_full_path(&instance_id)
            .await
            .unwrap()
            .join("mods/toctou.jar");
        let previous_bytes = b"previous destination";
        crate::util::io::write(&destination, previous_bytes)
            .await
            .unwrap();

        crate::state::materialize_verified_project_download_copy(
            &source,
            &destination,
            verified_size,
            &verified_sha1,
        )
        .await
        .expect_err("changed source must fail copied-byte identity");

        assert_eq!(
            crate::util::io::read(&destination).await.unwrap(),
            previous_bytes
        );
        assert!(
            stage8_pending_keys(&state, &instance_id)
                .await
                .contains(&("18".to_string(), "801".to_string()))
        );
        let snapshot = crate::api::instance::get_content_snapshot(&instance_id)
            .await
            .unwrap();
        assert!(!snapshot.items.iter().any(|item| {
            item.provider_project_id.as_deref() == Some("18")
                && item.provider_release_id.as_deref() == Some("801")
                && item.materialization_state
                    == crate::state::instances::PackMemberMaterializationState::Present
                && item.content.is_some()
        }));
        crate::api::instance::remove(&instance_id).await.unwrap();
    }

    #[cfg(not(feature = "tauri"))]
    #[tokio::test]
    async fn curseforge_automatic_sha1_record_can_complete_pending() {
        let (state, instance_id) =
            create_stage6_instance("automatic SHA1").await;
        let download =
            stage8_legacy_manual_download(19, 901, "automatic-sha1.jar");
        persist_manual_download(&instance_id, &download)
            .await
            .unwrap();
        let relative_path = "mods/automatic-sha1.jar";
        let full_path = crate::api::instance::get_full_path(&instance_id)
            .await
            .unwrap()
            .join(relative_path);
        let bytes = b"automatic SHA1 bytes";
        crate::util::io::write(&full_path, bytes).await.unwrap();
        let (_, sha1) = sha1_file_async(&full_path).await.unwrap();
        let file = stage8_curseforge_file(
            19,
            901,
            "automatic-sha1.jar",
            bytes.len() as u64,
            vec![CurseForgeFileHash {
                value: sha1,
                algo: 1,
            }],
            0,
        );

        record_installed_curseforge_file_with_permit(
            &instance_id,
            relative_path,
            &full_path,
            &file,
            ProjectType::Mod,
            crate::state::instances::ContentOwnershipKind::PackManaged,
            None,
            &state,
        )
        .await
        .unwrap();

        assert!(stage8_pending_keys(&state, &instance_id).await.is_empty());
        crate::api::instance::remove(&instance_id).await.unwrap();
    }

    #[cfg(not(feature = "tauri"))]
    #[tokio::test]
    async fn curseforge_automatic_fingerprint_record_can_complete_pending() {
        let (state, instance_id) =
            create_stage6_instance("automatic fingerprint").await;
        let download = stage8_legacy_manual_download(
            20,
            1001,
            "automatic-fingerprint.jar",
        );
        persist_manual_download(&instance_id, &download)
            .await
            .unwrap();
        let relative_path = "mods/automatic-fingerprint.jar";
        let full_path = crate::api::instance::get_full_path(&instance_id)
            .await
            .unwrap()
            .join(relative_path);
        let bytes = b"automatic fingerprint bytes";
        crate::util::io::write(&full_path, bytes).await.unwrap();
        let file = stage8_curseforge_file(
            20,
            1001,
            "automatic-fingerprint.jar",
            bytes.len() as u64,
            Vec::new(),
            compute_fingerprint(bytes) as u64,
        );

        record_installed_curseforge_file_with_permit(
            &instance_id,
            relative_path,
            &full_path,
            &file,
            ProjectType::Mod,
            crate::state::instances::ContentOwnershipKind::PackManaged,
            None,
            &state,
        )
        .await
        .unwrap();

        assert!(stage8_pending_keys(&state, &instance_id).await.is_empty());
        crate::api::instance::remove(&instance_id).await.unwrap();
    }

    #[cfg(not(feature = "tauri"))]
    #[tokio::test]
    async fn curseforge_automatic_weak_record_keeps_pending() {
        let (state, instance_id) =
            create_stage6_instance("automatic weak metadata").await;
        let download =
            stage8_legacy_manual_download(21, 1101, "automatic-md5.jar");
        persist_manual_download(&instance_id, &download)
            .await
            .unwrap();
        let relative_path = "mods/automatic-md5.jar";
        let full_path = crate::api::instance::get_full_path(&instance_id)
            .await
            .unwrap()
            .join(relative_path);
        let bytes = b"automatic MD5-only bytes";
        crate::util::io::write(&full_path, bytes).await.unwrap();
        let file = stage8_curseforge_file(
            21,
            1101,
            "automatic-md5.jar",
            bytes.len() as u64,
            vec![CurseForgeFileHash {
                value: "validated-md5".to_string(),
                algo: 2,
            }],
            0,
        );

        record_installed_curseforge_file_with_permit(
            &instance_id,
            relative_path,
            &full_path,
            &file,
            ProjectType::Mod,
            crate::state::instances::ContentOwnershipKind::PackManaged,
            None,
            &state,
        )
        .await
        .unwrap();

        assert!(
            stage8_pending_keys(&state, &instance_id)
                .await
                .contains(&("21".to_string(), "1101".to_string()))
        );
        let snapshot = crate::api::instance::get_content_snapshot(&instance_id)
            .await
            .unwrap();
        assert!(snapshot.items.iter().any(|item| {
            item.provider_project_id.as_deref() == Some("21")
                && item.provider_release_id.as_deref() == Some("1101")
                && item.materialization_state
                    == crate::state::instances::PackMemberMaterializationState::Present
        }));
        crate::api::instance::remove(&instance_id).await.unwrap();
    }

    #[cfg(not(feature = "tauri"))]
    #[tokio::test]
    async fn curseforge_rollback_identity_record_does_not_complete_pending() {
        let (state, instance_id) =
            create_stage6_instance("rollback provenance").await;
        let download = stage8_legacy_manual_download(22, 1201, "rollback.jar");
        persist_manual_download(&instance_id, &download)
            .await
            .unwrap();
        let relative_path = "mods/rollback.jar";
        let full_path = crate::api::instance::get_full_path(&instance_id)
            .await
            .unwrap()
            .join(relative_path);
        let bytes = b"historical rollback bytes";
        crate::util::io::write(&full_path, bytes).await.unwrap();
        let (_, sha1) = sha1_file_async(&full_path).await.unwrap();
        let provider_ref = ContentProviderRef::CurseForge {
            project_id: CurseForgeProjectId::new(22).unwrap(),
            file_id: Some(CurseForgeFileId::new(1201).unwrap()),
        };

        crate::state::record_project_file_atomic(
            &instance_id,
            relative_path,
            &sha1,
            bytes.len() as u64,
            ProjectType::Mod,
            ContentSourceKind::CurseForge,
            crate::state::instances::ContentOwnershipKind::PackManaged,
            Some(&provider_ref),
            true,
            None,
            &state,
        )
        .await
        .unwrap();

        assert!(
            stage8_pending_keys(&state, &instance_id)
                .await
                .contains(&("22".to_string(), "1201".to_string()))
        );
        crate::api::instance::remove(&instance_id).await.unwrap();
    }

    #[cfg(not(feature = "tauri"))]
    #[tokio::test]
    async fn curseforge_non_pending_import_fails_before_verification() {
        let (state, instance_id) =
            create_stage6_instance("non-pending import").await;
        let source_directory = tempfile::tempdir().unwrap();
        let source = source_directory.path().join("invalid.jar");
        crate::util::io::write(&source, b"not the required file")
            .await
            .unwrap();

        let error = import_pending_manual_download_file(
            &instance_id,
            601,
            6001,
            source.clone(),
        )
        .await
        .expect_err("missing pending identity must be rejected");

        assert!(matches!(
            error.raw.as_ref(),
            ErrorKind::InputError(message)
                if message
                    == "The selected CurseForge file is not pending for this instance"
        ));
        assert!(source.exists());
        assert!(
            crate::state::instances::adapters::sqlite::content_rows::get_pending_manual_downloads(
                &instance_id,
                &state.pool,
            )
            .await
            .unwrap()
            .is_empty()
        );
        let target = crate::api::instance::get_full_path(&instance_id)
            .await
            .unwrap()
            .join("mods/invalid.jar");
        assert!(!target.exists());
        crate::api::instance::remove(&instance_id).await.unwrap();
    }

    #[cfg(not(feature = "tauri"))]
    #[tokio::test]
    async fn curseforge_wrong_pending_file_fails_integrity() {
        let (state, instance_id) =
            create_stage6_instance("wrong pending file").await;
        let download = stage6_manual_download(
            701,
            7001,
            "expected.jar",
            b"required pending bytes",
        );
        persist_manual_download(&instance_id, &download)
            .await
            .unwrap();
        let source_directory = tempfile::tempdir().unwrap();
        let source = source_directory.path().join("selected.jar");
        crate::util::io::write(&source, b"wrong pending bytes")
            .await
            .unwrap();

        let error = import_pending_manual_download_file(
            &instance_id,
            download.project_id,
            download.file_id,
            source.clone(),
        )
        .await
        .expect_err("wrong pending file must fail integrity");

        assert!(matches!(
            error.raw.as_ref(),
            ErrorKind::InputError(message)
                if message
                    == "The selected file does not match the required CurseForge file"
        ));
        assert!(source.exists());
        let pending = crate::state::instances::adapters::sqlite::content_rows::get_pending_manual_downloads(
            &instance_id,
            &state.pool,
        )
        .await
        .unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].provider_project_id, "701");
        assert_eq!(pending[0].provider_release_id, "7001");
        let snapshot = crate::api::instance::get_content_snapshot(&instance_id)
            .await
            .unwrap();
        assert!(!snapshot.items.iter().any(|item| {
            item.provider_project_id.as_deref() == Some("701")
                && item.provider_release_id.as_deref() == Some("7001")
                && item.materialization_state
                    == crate::state::instances::PackMemberMaterializationState::Present
                && item.content.is_some()
        }));
        crate::api::instance::remove(&instance_id).await.unwrap();
    }

    #[cfg(not(feature = "tauri"))]
    #[tokio::test]
    async fn manual_import_completes_persisted_pack_member() {
        crate::event::EventState::init().await.unwrap();
        let state_root = tempfile::tempdir().unwrap().keep();
        let state =
            State::init_for_test(state_root.to_string_lossy().to_string())
                .await
                .unwrap();
        let created = crate::api::instance::create(
            format!("CurseForge drag {}", uuid::Uuid::new_v4()),
            "1.12.2".to_string(),
            ModLoader::Forge,
            Some("14.23.5.2860".to_string()),
            None,
            InstanceLink::Unmanaged,
            None,
            None,
        )
        .await
        .unwrap();
        let source_directory = tempfile::tempdir().unwrap();
        let source = source_directory.path().join("example-mod.jar");
        let bytes = b"verified curseforge drag";
        crate::util::io::write(&source, bytes).await.unwrap();
        let (_, sha1) = sha1_file_async(&source).await.unwrap();
        let download = CurseForgeManualDownload {
            project_id: 123,
            file_id: 456,
            file_name: "example-mod.jar".to_string(),
            ownership_kind:
                crate::state::instances::ContentOwnershipKind::PackManaged,
            operation_kind:
                crate::state::instances::ManualDownloadOperationKind::PackInstall,
            website_url: None,
            project_type: "mod".to_string(),
            project_slug: "example-mod".to_string(),
            target_folder: "mods".to_string(),
            hashes: vec![CurseForgeFileHash {
                value: sha1,
                algo: 1,
            }],
            file_length: bytes.len() as u64,
            file_fingerprint: 0,
        };
        persist_manual_download(&created.instance.id, &download)
            .await
            .unwrap();

        crate::util::io::write(&source, b"same-length-wrong-bytes!")
            .await
            .unwrap();
        assert!(
            import_pending_manual_download_from_path(
                &created.instance.id,
                &source,
            )
            .await
            .is_err()
        );
        assert_eq!(
            crate::state::instances::adapters::sqlite::content_rows::get_pending_manual_downloads(
                &created.instance.id,
                &state.pool,
            )
            .await
            .unwrap()
            .len(),
            1
        );
        let selected = source_directory.path().join("renamed-local-file.jar");
        crate::util::io::write(&selected, bytes).await.unwrap();

        let relative_path = import_pending_manual_download_file(
            &created.instance.id,
            123,
            456,
            selected.clone(),
        )
        .await
        .unwrap()
        .relative_path;
        let snapshot =
            crate::api::instance::get_content_snapshot(&created.instance.id)
                .await
                .unwrap();
        let imported = snapshot
            .items
            .iter()
            .find(|item| {
                item.provider_project_id.as_deref() == Some("123")
                    && item.provider_release_id.as_deref() == Some("456")
            })
            .unwrap();

        assert!(snapshot.pending_manual_downloads.is_empty());
        assert_eq!(
            imported.ownership_kind,
            crate::state::instances::ContentOwnershipKind::PackManaged
        );
        assert_eq!(
            imported.materialization_state,
            crate::state::instances::PackMemberMaterializationState::Present
        );
        assert_eq!(imported.expected_relative_path, relative_path);
        let instance_path = state
            .directories
            .instance_game_dir(&created.instance)
            .join(relative_path);
        assert_eq!(crate::util::io::read(&instance_path).await.unwrap(), bytes);
        assert!(source.exists());
        assert!(selected.exists());
        crate::util::io::write(&selected, b"mutated selected source")
            .await
            .unwrap();
        assert_eq!(crate::util::io::read(instance_path).await.unwrap(), bytes);
    }

    #[test]
    fn archive_paths_stay_inside_the_instance() {
        assert_eq!(
            safe_archive_relative_path("config/example.toml").unwrap(),
            "config/example.toml"
        );
        assert!(safe_archive_relative_path("../options.txt").is_err());
        assert!(safe_archive_relative_path("/options.txt").is_err());
    }

    #[test]
    fn curseforge_override_content_joins_the_pack_group() {
        assert_eq!(
            curseforge_override_content("mods/cc-tweaked.jar").unwrap(),
            Some(CurseForgePackExpectedOverride {
                project_type: ProjectType::Mod,
                expected_relative_path: "mods/cc-tweaked.jar".to_string(),
            })
        );
        assert_eq!(
            curseforge_override_content("datapacks/sawmill.zip").unwrap(),
            Some(CurseForgePackExpectedOverride {
                project_type: ProjectType::DataPack,
                expected_relative_path: "datapacks/sawmill.zip".to_string(),
            })
        );
        assert_eq!(
            curseforge_override_content("config/example.toml").unwrap(),
            None
        );
    }

    #[test]
    fn cdn_urls_are_restricted_to_forgecdn() {
        assert!(
            validate_cdn_url(
                &reqwest::Url::parse(
                    "https://edge.forgecdn.net/files/1/2/a.jar"
                )
                .unwrap()
            )
            .is_ok()
        );
        assert!(
            validate_cdn_url(
                &reqwest::Url::parse("https://forgecdn.net.evil.test/a.jar")
                    .unwrap()
            )
            .is_err()
        );
    }

    #[test]
    fn blank_download_urls_are_treated_as_unavailable() {
        assert_eq!(normalized_download_url(None), None);
        assert_eq!(normalized_download_url(Some(String::new())), None);
        assert_eq!(normalized_download_url(Some("  \t".to_string())), None);
        assert_eq!(
            normalized_download_url(Some(
                " https://edge.forgecdn.net/files/1/2/a.jar ".to_string(),
            )),
            Some("https://edge.forgecdn.net/files/1/2/a.jar".to_string()),
        );
    }

    #[test]
    fn derived_download_urls_follow_curseforge_file_id_layout() {
        assert_eq!(
            derived_curseforge_download_url(
                6_767_951,
                "jei 1.21.5+neoforge.jar"
            )
            .unwrap(),
            "https://edge.forgecdn.net/files/6767/951/jei%201.21.5%2Bneoforge.jar"
        );
        assert_eq!(
            derived_curseforge_download_url(1_234_005, "example.jar").unwrap(),
            "https://edge.forgecdn.net/files/1234/5/example.jar"
        );
        assert!(
            derived_curseforge_download_url(1_234_005, "../example.jar")
                .is_err()
        );
    }

    #[test]
    fn curseforge_loader_ids_map_to_instance_loaders() {
        assert_eq!(loader_family("forge-47.4.0"), "forge");
        assert_eq!(loader_family("fabric-0.16.10"), "fabric");
        assert_eq!(loader_type("cleanroom"), Some(1));
        assert_eq!(loader_type("Fabric"), Some(4));
        assert_eq!(loader_type("neoforge"), Some(6));
    }

    #[test]
    fn curseforge_game_versions_ignore_loader_labels() {
        assert!(looks_like_game_version("1.21.8"));
        assert!(!looks_like_game_version("Fabric"));
        assert!(!looks_like_game_version("forge"));
    }

    #[test]
    fn modpack_target_uses_primary_forge_loader_and_version() {
        let manifest = CurseForgeModpackManifest {
            minecraft: CurseForgeManifestMinecraft {
                version: "1.12.2".to_string(),
                mod_loaders: vec![CurseForgeManifestLoader {
                    id: "forge-14.23.5.2860".to_string(),
                    primary: true,
                }],
            },
            files: Vec::new(),
            overrides: "overrides".to_string(),
            name: None,
            version: None,
        };

        assert_eq!(
            modpack_target(&manifest).unwrap(),
            CurseForgeModpackTarget {
                game_version: "1.12.2".to_string(),
                loader: ModLoader::Forge,
                loader_version: Some("14.23.5.2860".to_string()),
            }
        );
    }

    #[test]
    fn modpack_target_recognizes_pcl_cleanroom_version_convention() {
        let manifest = CurseForgeModpackManifest {
            minecraft: CurseForgeManifestMinecraft {
                version: "1.12.2".to_string(),
                mod_loaders: vec![CurseForgeManifestLoader {
                    id: "forge-0.3.0-alpha".to_string(),
                    primary: true,
                }],
            },
            files: Vec::new(),
            overrides: "overrides".to_string(),
            name: None,
            version: None,
        };

        assert_eq!(
            modpack_target(&manifest).unwrap(),
            CurseForgeModpackTarget {
                game_version: "1.12.2".to_string(),
                loader: ModLoader::Cleanroom,
                loader_version: Some("0.3.0-alpha".to_string()),
            }
        );
    }

    #[test]
    fn modpack_target_without_loader_is_vanilla() {
        let manifest = CurseForgeModpackManifest {
            minecraft: CurseForgeManifestMinecraft {
                version: "1.20.1".to_string(),
                mod_loaders: Vec::new(),
            },
            files: Vec::new(),
            overrides: "overrides".to_string(),
            name: None,
            version: None,
        };

        assert_eq!(
            modpack_target(&manifest).unwrap(),
            CurseForgeModpackTarget {
                game_version: "1.20.1".to_string(),
                loader: ModLoader::Vanilla,
                loader_version: None,
            }
        );
    }

    #[test]
    fn recognized_project_types_map_curseforge_classes() {
        assert_eq!(recognized_project_type(Some(6)), Some(ProjectType::Mod));
        assert_eq!(
            recognized_project_type(Some(12)),
            Some(ProjectType::ResourcePack),
        );
        assert_eq!(
            recognized_project_type(Some(6552)),
            Some(ProjectType::ShaderPack),
        );
        assert_eq!(
            recognized_project_type(Some(6945)),
            Some(ProjectType::DataPack),
        );
        assert_eq!(
            recognized_project_type(Some(17)),
            Some(ProjectType::WorldSave),
        );
        assert_eq!(recognized_project_type(Some(4471)), None);
        assert_eq!(recognized_project_type(None), None);
    }

    #[test]
    fn world_archives_must_use_a_zip_file_name() {
        assert!(validate_world_archive_name("world.zip").is_ok());
        assert!(validate_world_archive_name("WORLD.ZIP").is_ok());
        assert!(validate_world_archive_name("world.rar").is_err());
        assert!(validate_world_archive_name("../world.zip").is_err());
    }

    #[test]
    fn a_key_set_in_the_launcher_wins_over_the_launcher_key() {
        assert_eq!(
            resolve_api_key(Some("override".into()), Some("launcher".into())),
            (
                Some("override".to_string()),
                CurseForgeApiKeySource::Override
            ),
        );
    }

    #[test]
    fn a_blank_override_falls_back_to_the_launcher_key() {
        assert_eq!(
            resolve_api_key(Some("   ".into()), Some("launcher".into())),
            (
                Some("launcher".to_string()),
                CurseForgeApiKeySource::Launcher
            ),
        );
        assert_eq!(
            resolve_api_key(None, Some("launcher".into())),
            (
                Some("launcher".to_string()),
                CurseForgeApiKeySource::Launcher
            ),
        );
    }

    #[test]
    fn keys_are_trimmed_before_they_are_used() {
        assert_eq!(
            resolve_api_key(Some(" override\n".into()), None),
            (
                Some("override".to_string()),
                CurseForgeApiKeySource::Override
            ),
        );
    }

    #[test]
    fn a_build_without_any_key_reports_no_key() {
        assert_eq!(
            resolve_api_key(None, None),
            (None, CurseForgeApiKeySource::Missing),
        );
        assert_eq!(
            resolve_api_key(Some(" ".into()), Some("\t".into())),
            (None, CurseForgeApiKeySource::Missing),
        );
    }
}

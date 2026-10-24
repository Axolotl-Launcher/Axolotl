//! CurseForge integration: shared constants, HTTP clients, and submodules.

mod auth;
mod catalog;
mod content_change;
mod download;
mod dto;
mod install;
mod manual;
mod mapping;
mod metrics;
mod modpack;
mod preview;
mod selection;
mod transport;
mod update;
mod validation;
mod verify;

#[cfg(test)]
mod tests;

pub use self::auth::*;
pub use self::catalog::*;
pub use self::dto::*;
pub use self::install::*;
pub use self::manual::*;
pub use self::modpack::*;
pub use self::preview::*;
pub use self::selection::*;
pub use self::update::*;

pub(crate) use self::catalog::{
    derived_curseforge_download_url, resource_request,
};
pub(crate) use self::content_change::prepare_curseforge_content_change_action;
pub(crate) use self::dto::StagedCurseForgeUpgrade;
pub(crate) use self::dto::{
    CurseForgeManifestFile, CurseForgePackExpectedContent,
    CurseForgePackExpectedMember, CurseForgePackExpectedOverride,
    get_modpack_expected_members,
};
pub(crate) use self::dto::{
    apply_staged_curseforge_upgrade_file,
    apply_staged_curseforge_upgrade_file_with_state,
    stage_curseforge_upgrade_file,
};
pub(crate) use self::manual::{
    import_pending_manual_download_from_path,
    reconcile_persisted_curseforge_waiting_jobs,
    scan_pending_manual_downloads_in,
};
pub(crate) use self::modpack::install_local_manifest_files;
pub(crate) use self::transport::api_key;
pub(crate) use self::update::loader_family;
pub(crate) use self::update::update_managed_modpack_with_reporter;
pub(crate) use self::verify::{
    CurseForgeFingerprintHasher, compute_fingerprint,
};

use crate::state::DependencyResolutionPlan;
use crate::util::download::constants::MODPACK_METADATA_CONCURRENCY;
use dashmap::DashMap;
use futures::StreamExt;
use futures::stream;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{LazyLock, RwLock};
use std::time::{Duration, Instant};

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

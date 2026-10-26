//! Dependency file selection and resolution plan storage.

use crate::State;
use crate::api::curseforge::catalog::get_files;
use crate::api::curseforge::constants::{DEPENDENCY_PLAN_TTL, MAX_PAGE_SIZE};
use crate::api::curseforge::dto::{CurseForgeFile, CurseForgeFilesRequest};
use crate::api::curseforge::mapping::mod_loader_to_slug;
use crate::api::curseforge::plans::{
    CachedDependencyResolutionPlan, DEPENDENCY_RESOLUTION_PLANS,
};
use crate::state::{
    ContentProviderRef, CurseForgeFileId, CurseForgeProjectId,
    DependencyResolutionPlan, DependencySelectionReason,
};
use serde::{Deserialize, Serialize};
use std::time::Instant;

/// Why a dependency file was selected. This makes fallback provenance
/// explicit rather than allowing it to look like a native declaration.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CurseForgeDependencySelectionReason {
    NativeStrictMatch,
    Sha1VerifiedModrinthFallback,
}

/// A CurseForge file selected for a dependency after local target validation.
pub(crate) struct SelectedDependencyFile {
    pub(crate) file: CurseForgeFile,
    pub(crate) reason: CurseForgeDependencySelectionReason,
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

pub(crate) fn dependency_target_game_version(
    game_version: Option<&str>,
) -> Option<&str> {
    game_version.filter(|version| !version.trim().is_empty())
}

pub(crate) async fn select_dependency_file(
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

pub(crate) fn curseforge_content_ref(
    project_id: u32,
    file_id: u32,
) -> crate::Result<ContentProviderRef> {
    Ok(ContentProviderRef::CurseForge {
        project_id: CurseForgeProjectId::new(project_id)?,
        file_id: Some(CurseForgeFileId::new(file_id)?),
    })
}

pub(crate) fn curseforge_file_sha1(file: &CurseForgeFile) -> Option<String> {
    file.hashes
        .iter()
        .find(|hash| hash.algo == 1)
        .map(|hash| hash.value.clone())
}

pub(crate) async fn instance_content_revision(
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

pub(crate) fn store_dependency_resolution_plan(plan: DependencyResolutionPlan) {
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

pub(crate) fn select_best_dependency_file(
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
pub(crate) fn file_matches_dependency_target(
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

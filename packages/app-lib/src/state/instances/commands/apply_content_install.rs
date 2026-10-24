use super::sync_content_files::{instance_content_root, join_content_path};
use crate::api::content_search::{
    chinese_file_title_for_modrinth_slug, localized_content_file_name,
};
use crate::state::instances::{
    ContentOwnershipKind, ContentRequirement, ContentSourceKind, Instance,
    InstanceFile, PackMember, PackMemberMaterializationState,
    PackMemberOverrideKind,
    adapters::{
        filesystem::project_type_from_relative_path,
        sqlite::{content_rows, instance_rows},
    },
};
use crate::state::{
    CacheBehaviour, CacheValue, CachedEntry, CachedFileHash, ContentProvider,
    ContentProviderRef, CurseForgeFileId, CurseForgeProjectId, Dependency,
    DependencyType, KnownModrinthFile, ModLoader, ModrinthProjectId,
    ModrinthVersionId, Project, ProjectType, State, Version, cache_file_hash,
};
use crate::util::fetch::{
    self, ContentValidation, DownloadMeta, DownloadReason, DownloadRequest,
    Integrity, ResourceClass, download_to_path,
};
use crate::util::io;
use crate::util::io::io_error_with_lock_info;
use async_trait::async_trait;
use bytes::Bytes;
use content_management::{
    ContentMetadataProvider, ContentType, DependencyPolicy,
    Error as ResolveError, ResolutionPreferences, ResolveContentPlan,
    ResolveContentRequest, ResolvedContent, SkippedReason,
};
use futures::{StreamExt, TryStreamExt};
use std::collections::HashMap;
use std::future::Future;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use tracing::warn;
use zip::CompressionMethod;
use zip::write::SimpleFileOptions;

pub(crate) struct ContentScope {
    pub instance: Instance,
    pub content_set_id: String,
}

#[derive(Clone, Debug)]
pub(crate) struct ToggledContentEntry {
    pub content_id: String,
    pub path: String,
    pub enabled: bool,
}

struct ContentToggleOperation {
    content_id: String,
    current_path: String,
    new_path: String,
    enabled: bool,
    project_type: ProjectType,
}

pub(crate) struct InstalledContentFile {
    pub relative_path: String,
    pub provider_refs: Vec<ContentProviderRef>,
    pub enabled: bool,
}

#[derive(Clone)]
pub(crate) struct DownloadedProjectVersion {
    pub file_name: String,
    pub path: PathBuf,
    pub sha1: String,
    pub size: u64,
    pub project_type: ProjectType,
    pub project_id: String,
    pub version_id: String,
}

#[derive(Clone, Debug)]
pub(crate) struct ProjectFileRecord {
    pub relative_path: String,
    pub sha1: String,
    pub size: u64,
    pub project_type: ProjectType,
    pub source_kind: ContentSourceKind,
    pub ownership_kind: ContentOwnershipKind,
    pub provider_ref: Option<ContentProviderRef>,
    pub origin: bool,
    pub known_modrinth_project_id: Option<String>,
    pub known_modrinth_version_id: Option<String>,
}

pub(crate) struct InstanceInstallProjectRequest {
    pub project_id: String,
    pub version_id: Option<String>,
    pub content_type: ContentType,
    pub selected: ResolutionPreferences,
    pub excluded_project_ids: Vec<String>,
    pub force_project_ids: Vec<String>,
}

struct CachedEntryContentProvider<'a> {
    state: &'a State,
    cache_behaviour: Option<CacheBehaviour>,
}

#[async_trait]
impl ContentMetadataProvider for CachedEntryContentProvider<'_> {
    async fn get_version(
        &mut self,
        version_id: &str,
    ) -> Result<Option<content_management::Version>, ResolveError> {
        let version = CachedEntry::get_version(
            &ModrinthVersionId::new(version_id.to_string())
                .map_err(resolve_provider_error)?,
            self.cache_behaviour,
            &self.state.pool,
            &self.state.api_semaphore,
        )
        .await
        .map_err(resolve_provider_error)?;

        Ok(version.map(version_to_resolver))
    }

    async fn get_project_versions(
        &mut self,
        project_id: &str,
    ) -> Result<Vec<content_management::Version>, ResolveError> {
        let versions = CachedEntry::get_project_versions(
            &ModrinthProjectId::new(project_id.to_string())
                .map_err(resolve_provider_error)?,
            self.cache_behaviour,
            &self.state.pool,
            &self.state.api_semaphore,
        )
        .await
        .map_err(resolve_provider_error)?;

        Ok(versions
            .unwrap_or_default()
            .into_iter()
            .map(version_to_resolver)
            .collect())
    }
}

fn resolve_provider_error(error: crate::Error) -> ResolveError {
    ResolveError::Provider(error.to_string())
}

fn resolver_error(error: ResolveError) -> crate::Error {
    crate::ErrorKind::InputError(error.to_string()).into()
}

fn version_to_resolver(version: Version) -> content_management::Version {
    let (filename, sha1) = version
        .files
        .first()
        .map(|f| (Some(f.filename.clone()), f.hashes.get("sha1").cloned()))
        .unwrap_or_default();

    content_management::Version {
        id: version.id,
        project_id: version.project_id,
        date_published: version.date_published,
        version_type: version.version_type,
        dependencies: version
            .dependencies
            .into_iter()
            .map(dependency_to_resolver)
            .collect(),
        game_versions: version.game_versions,
        loaders: version.loaders,
        version_number: Some(version.version_number),
        filename,
        sha1,
    }
}

fn dependency_to_resolver(
    dependency: Dependency,
) -> content_management::Dependency {
    content_management::Dependency {
        version_id: dependency.version_id,
        project_id: dependency.project_id,
        file_name: dependency.file_name,
        dependency_type: match dependency.dependency_type {
            DependencyType::Required => {
                content_management::DependencyType::Required
            }
            DependencyType::Optional => {
                content_management::DependencyType::Optional
            }
            DependencyType::Incompatible => {
                content_management::DependencyType::Incompatible
            }
            DependencyType::Embedded => {
                content_management::DependencyType::Embedded
            }
        },
    }
}

fn target_preferences(
    game_version: String,
    loader: ModLoader,
    content_type: ContentType,
) -> ResolutionPreferences {
    let loader = match content_type {
        ContentType::DataPack => "datapack".to_string(),
        ContentType::ResourcePack => "minecraft".to_string(),
        ContentType::Shader => "iris".to_string(),
        _ => loader.as_str().to_string(),
    };

    ResolutionPreferences {
        game_versions: if content_type == ContentType::ResourcePack {
            Vec::new()
        } else {
            vec![game_version]
        },
        loaders: vec![loader],
    }
}

pub(crate) async fn resolve_install_plan(
    instance_id: &str,
    request: InstanceInstallProjectRequest,
    state: &State,
) -> crate::Result<ResolveContentPlan> {
    resolve_install_plan_with_cache(
        instance_id,
        request,
        Some(CacheBehaviour::MustRevalidate),
        DependencyPolicy::IncludeOptional,
        state,
    )
    .await
}

pub(crate) async fn resolve_update_plan(
    instance_id: &str,
    request: InstanceInstallProjectRequest,
    state: &State,
) -> crate::Result<ResolveContentPlan> {
    resolve_install_plan_with_cache(
        instance_id,
        request,
        Some(CacheBehaviour::MustRevalidate),
        DependencyPolicy::RequiredOnly,
        state,
    )
    .await
}

async fn resolve_install_plan_with_cache(
    instance_id: &str,
    request: InstanceInstallProjectRequest,
    cache_behaviour: Option<CacheBehaviour>,
    dependency_policy: DependencyPolicy,
    state: &State,
) -> crate::Result<ResolveContentPlan> {
    let content_set =
        content_rows::get_applied_content_set(instance_id, &state.pool)
            .await?
            .ok_or_else(|| {
                crate::ErrorKind::InputError(format!(
                    "Instance {instance_id} has no applied content set"
                ))
            })?;
    let existing_project_ids =
        crate::state::get_installed_project_ids_for_instance(
            instance_id,
            None,
            state,
        )
        .await?;
    let provider = CachedEntryContentProvider {
        state,
        cache_behaviour,
    };
    let content_type = request.content_type;
    let request = ResolveContentRequest {
        project_id: request.project_id,
        version_id: request.version_id,
        content_type,
        selected: request.selected,
        target: target_preferences(
            content_set.game_version,
            content_set.loader,
            content_type,
        ),
        existing_project_ids,
        excluded_project_ids: request.excluded_project_ids,
        force_project_ids: request.force_project_ids,
    };

    let mut plan = content_management::resolve_content_with_policy(
        provider,
        request,
        dependency_policy,
    )
    .await
    .map_err(resolver_error)?;

    // Enrich plan with project-level metadata from cache
    {
        let project_ids: Vec<ModrinthProjectId> =
            std::iter::once(&plan.primary.project_id)
                .chain(plan.dependencies.iter().map(|d| &d.project_id))
                .filter_map(|id| ModrinthProjectId::new(id.clone()).ok())
                .collect();

        if !project_ids.is_empty() {
            let projects = CachedEntry::get_project_many(
                &project_ids,
                cache_behaviour,
                &state.pool,
                &state.api_semaphore,
            )
            .await?;

            let project_map: HashMap<String, &Project> =
                projects.iter().map(|p| (p.id.clone(), p)).collect();

            let enrich = |rc: &mut ResolvedContent| {
                let meta = rc.metadata.get_or_insert_with(Default::default);
                if let Some(project) = project_map.get(&rc.project_id) {
                    meta.title = Some(project.title.clone());
                    meta.icon_url = project.icon_url.clone();
                }
            };

            enrich(&mut plan.primary);
            for dep in &mut plan.dependencies {
                enrich(dep);
            }
        }
    }

    Ok(plan)
}

pub(crate) async fn resolve_install_plan_for_target_with_policy(
    request: InstanceInstallProjectRequest,
    game_version: String,
    loader: ModLoader,
    dependency_policy: DependencyPolicy,
    state: &State,
) -> crate::Result<ResolveContentPlan> {
    let cache_behaviour = Some(CacheBehaviour::MustRevalidate);
    let provider = CachedEntryContentProvider {
        state,
        cache_behaviour,
    };
    let content_type = request.content_type;
    let request = ResolveContentRequest {
        project_id: request.project_id,
        version_id: request.version_id,
        content_type,
        selected: request.selected,
        target: target_preferences(game_version, loader, content_type),
        existing_project_ids: Vec::new(),
        excluded_project_ids: request.excluded_project_ids,
        force_project_ids: request.force_project_ids,
    };

    let mut plan = content_management::resolve_content_with_policy(
        provider,
        request,
        dependency_policy,
    )
    .await
    .map_err(resolver_error)?;

    // Enrich plan with project-level metadata from cache
    {
        let project_ids: Vec<ModrinthProjectId> =
            std::iter::once(&plan.primary.project_id)
                .chain(plan.dependencies.iter().map(|d| &d.project_id))
                .filter_map(|id| ModrinthProjectId::new(id.clone()).ok())
                .collect();

        if !project_ids.is_empty() {
            let projects = CachedEntry::get_project_many(
                &project_ids,
                cache_behaviour,
                &state.pool,
                &state.api_semaphore,
            )
            .await?;

            let project_map: HashMap<String, &Project> =
                projects.iter().map(|p| (p.id.clone(), p)).collect();

            let enrich = |rc: &mut ResolvedContent| {
                let meta = rc.metadata.get_or_insert_with(Default::default);
                if let Some(project) = project_map.get(&rc.project_id) {
                    meta.title = Some(project.title.clone());
                    meta.icon_url = project.icon_url.clone();
                }
            };

            enrich(&mut plan.primary);
            for dep in &mut plan.dependencies {
                enrich(dep);
            }
        }
    }

    Ok(plan)
}

pub(crate) async fn install_resolved_content_plan_with_reporter(
    instance_id: &str,
    plan: &ResolveContentPlan,
    reporter: Option<crate::install::InstallProgressReporter>,
    state: &State,
) -> crate::Result<Vec<String>> {
    let total_bytes = resolved_plan_total_bytes(plan, state).await?;
    let file_count = (plan.dependencies.len() + 1) as u64;
    let planned_contents = std::iter::once(&plan.primary)
        .chain(plan.dependencies.iter())
        .cloned()
        .enumerate()
        .collect::<Vec<_>>();
    let mut downloads = futures::stream::iter(planned_contents)
        .map(|(index, content)| {
            let reporter = reporter.clone();
            async move {
                let downloaded = download_project_version_with_reporting(
                    instance_id,
                    &content.version_id,
                    if index == 0 {
                        DownloadReason::Standalone
                    } else {
                        DownloadReason::Dependency
                    },
                    content.dependent_on_version_id.clone(),
                    None,
                    reporter,
                    state,
                )
                .await?;
                Ok::<_, crate::Error>((index, downloaded))
            }
        })
        .buffer_unordered(
            state
                .download_concurrency()
                .clamp(1, crate::util::download::constants::MAX_PACK_FILE_DOWNLOAD_CONCURRENCY),
        )
        .try_collect::<Vec<_>>()
        .await?;
    downloads.sort_by_key(|(index, _)| *index);
    let paths = materialize_project_download_batch(
        instance_id,
        downloads.into_iter().map(|(_, file)| file).collect(),
        Some(plan),
        state,
    )
    .await?;
    if let Some(reporter) = reporter {
        reporter
            .update(
                crate::install::InstallPhaseId::DownloadingContent,
                Some(crate::install::InstallProgress {
                    current: file_count,
                    total: file_count,
                    secondary: Some(crate::install::InstallProgressSecondary {
                        current: total_bytes,
                        total: total_bytes,
                    }),
                }),
                crate::install::InstallPhaseDetails::Empty,
            )
            .await?;
    }
    Ok(paths)
}

async fn resolved_plan_total_bytes(
    plan: &ResolveContentPlan,
    state: &State,
) -> crate::Result<u64> {
    let mut total = resolved_content_file_size(&plan.primary, state).await?;
    for dependency in &plan.dependencies {
        total += resolved_content_file_size(dependency, state).await?;
    }
    Ok(total.max(1))
}

async fn resolved_content_file_size(
    content: &ResolvedContent,
    state: &State,
) -> crate::Result<u64> {
    let version = CachedEntry::get_version(
        &ModrinthVersionId::new(content.version_id.clone())?,
        None,
        &state.pool,
        &state.api_semaphore,
    )
    .await?
    .ok_or_else(|| {
        crate::ErrorKind::InputError(format!(
            "Unable to resolve version {} for progress",
            content.version_id
        ))
    })?;
    let file = version
        .files
        .iter()
        .find(|file| file.primary)
        .or_else(|| version.files.first());
    Ok(file.map_or(1, |file| (file.size as u64).max(1)))
}

pub(crate) async fn switch_project_version_with_dependencies_preserving_name(
    instance_id: &str,
    project_path: &str,
    current_version_id: Option<&str>,
    version_id: &str,
    reporter: Option<crate::install::InstallProgressReporter>,
    state: &State,
) -> crate::Result<String> {
    let version = CachedEntry::get_version(
        &ModrinthVersionId::new(version_id.to_string())?,
        Some(CacheBehaviour::MustRevalidate),
        &state.pool,
        &state.api_semaphore,
    )
    .await?
    .ok_or_else(|| {
        crate::ErrorKind::InputError(format!(
            "Unable to install version id {version_id}. Not found."
        ))
    })?;
    let content_type = ProjectType::get_from_loaders(version.loaders.clone())
        .map(ContentType::from)
        .unwrap_or(ContentType::Mod);
    let plan = resolve_update_plan(
        instance_id,
        InstanceInstallProjectRequest {
            project_id: version.project_id.clone(),
            version_id: Some(version_id.to_string()),
            content_type,
            selected: ResolutionPreferences::default(),
            excluded_project_ids: Vec::new(),
            force_project_ids: Vec::new(),
        },
        state,
    )
    .await?;
    let new_provider_file_name = primary_version_file_name(&version)?;
    let old_provider_file_name = match current_version_id {
        Some(current_version_id) => CachedEntry::get_version(
            &ModrinthVersionId::new(current_version_id.to_string())?,
            Some(CacheBehaviour::MustRevalidate),
            &state.pool,
            &state.api_semaphore,
        )
        .await?
        .map(|version| primary_version_file_name(&version))
        .transpose()?,
        None => None,
    };

    let was_disabled = project_path.ends_with(".disabled");
    let ownership_kind =
        content_ownership_for_path(instance_id, project_path, state).await?;
    let total_bytes = resolved_plan_total_bytes(&plan, state).await?;
    let file_count = (plan.dependencies.len() + 1) as u64;
    let mut base_bytes = 0_u64;
    let primary_progress =
        reporter
            .as_ref()
            .map(|reporter| ResolvedContentDownloadProgress {
                reporter: reporter.clone(),
                file_index: 0,
                file_count,
                base_bytes,
                total_bytes,
            });
    let mut new_path = add_project_from_version_with_progress(
        instance_id,
        &plan.primary.version_id,
        DownloadReason::Update,
        None,
        ContentSourceKind::Local,
        ownership_kind,
        primary_progress,
        state,
    )
    .await?;
    base_bytes += resolved_content_file_size(&plan.primary, state).await?;

    if was_disabled {
        new_path =
            toggle_disable_project(instance_id, &new_path, Some(false), state)
                .await?;
    }

    let mut installed_paths = Vec::with_capacity(plan.dependencies.len() + 1);
    installed_paths.push(new_path.clone());
    for (index, dependency) in plan.dependencies.iter().enumerate() {
        let progress =
            reporter
                .as_ref()
                .map(|reporter| ResolvedContentDownloadProgress {
                    reporter: reporter.clone(),
                    file_index: (index + 1) as u64,
                    file_count,
                    base_bytes,
                    total_bytes,
                });
        installed_paths.push(
            add_resolved_content_with_progress(
                instance_id,
                dependency,
                DownloadReason::Dependency,
                true,
                progress,
                state,
            )
            .await?,
        );
        base_bytes += resolved_content_file_size(dependency, state).await?;
    }
    persist_resolved_plan_dependency_edges(
        instance_id,
        &installed_paths,
        &plan,
        state,
    )
    .await?;

    finalize_updated_project_path(
        instance_id,
        project_path,
        &new_path,
        old_provider_file_name.as_deref(),
        &new_provider_file_name,
        state,
    )
    .await
}

pub(crate) fn primary_version_file_name(
    version: &crate::state::Version,
) -> crate::Result<String> {
    version
        .files
        .iter()
        .find(|file| file.primary)
        .or_else(|| version.files.first())
        .map(|file| file.filename.clone())
        .ok_or_else(|| {
            crate::ErrorKind::InputError(format!(
                "Version {} has no downloadable file",
                version.id
            ))
            .into()
        })
}

pub(crate) async fn prepare_modrinth_content_change_action(
    instance_id: &str,
    action: &mut crate::install::ContentChangeAction,
    state: &State,
) -> crate::Result<()> {
    let version = CachedEntry::get_version(
        &ModrinthVersionId::new(action.target_release_id.clone())?,
        None,
        &state.pool,
        &state.api_semaphore,
    )
    .await?
    .ok_or_else(|| {
        crate::ErrorKind::InputError(format!(
            "Unable to install version id {}. Not found.",
            action.target_release_id
        ))
    })?;
    let content_type = ProjectType::get_from_loaders(version.loaders.clone())
        .map(ContentType::from)
        .unwrap_or(ContentType::Mod);
    let plan = resolve_install_plan_with_cache(
        instance_id,
        InstanceInstallProjectRequest {
            project_id: version.project_id.clone(),
            version_id: Some(version.id.clone()),
            content_type,
            selected: ResolutionPreferences::default(),
            excluded_project_ids: Vec::new(),
            force_project_ids: Vec::new(),
        },
        None,
        DependencyPolicy::RequiredOnly,
        state,
    )
    .await?;
    let target_provider_file_name = primary_version_file_name(&version)?;
    let current_provider_file_name = match action.expected_release_id.as_deref()
    {
        Some(version_id) => CachedEntry::get_version(
            &ModrinthVersionId::new(version_id.to_string())?,
            None,
            &state.pool,
            &state.api_semaphore,
        )
        .await?
        .map(|version| primary_version_file_name(&version))
        .transpose()?,
        None => None,
    };
    let current_path = action.relative_path.as_deref().ok_or_else(|| {
        crate::ErrorKind::InputError(format!(
            "Content {} is not present on disk",
            action.content_id
        ))
    })?;
    action.final_relative_path = Some(
        configured_update_relative_path(
            current_path,
            current_provider_file_name.as_deref(),
            &target_provider_file_name,
            &state.pool,
        )
        .await?,
    );
    action.current_provider_file_name = current_provider_file_name;
    action.target_provider_file_name = Some(target_provider_file_name);
    action.files.clear();
    action.dependencies.clear();

    for (index, resolved) in std::iter::once(&plan.primary)
        .chain(plan.dependencies.iter())
        .enumerate()
    {
        let prepared = prepare_version_download(
            instance_id,
            &resolved.version_id,
            if index == 0 {
                DownloadReason::Update
            } else {
                DownloadReason::Dependency
            },
            resolved.dependent_on_version_id.clone(),
            state,
        )
        .await?;
        let id = format!("modrinth:{}", resolved.version_id);
        action.files.push(crate::install::ContentChangeFile {
            id: id.clone(),
            role: if index == 0 {
                crate::install::ContentChangeFileRole::Primary
            } else {
                crate::install::ContentChangeFileRole::Dependency
            },
            provider: ContentProvider::Modrinth,
            project_id: resolved.project_id.clone(),
            release_id: resolved.version_id.clone(),
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
        if let Some(parent) = resolved.dependent_on_version_id.as_deref() {
            action
                .dependencies
                .push(crate::install::ContentChangeDependency {
                    parent_file_id: format!("modrinth:{parent}"),
                    child_file_id: id,
                    provider: ContentProvider::Modrinth,
                    project_id: resolved.project_id.clone(),
                    release_id: resolved.version_id.clone(),
                    kind: Some(if resolved.required {
                        crate::state::instances::ContentDependencyKind::Required
                    } else {
                        crate::state::instances::ContentDependencyKind::Include
                    }),
                });
        }
    }
    action.modrinth_plan = Some(plan);
    action.set_status(crate::install::ContentChangeActionStatus::Prepared);
    Ok(())
}

pub(crate) fn preserved_update_relative_path(
    current_path: &str,
    old_provider_file_name: Option<&str>,
    new_provider_file_name: &str,
) -> crate::Result<String> {
    let new_provider_file_name =
        io::sanitize_download_file_name(new_provider_file_name)?;
    let old_provider_file_name = old_provider_file_name
        .map(io::sanitize_download_file_name)
        .transpose()?;
    let current_name = Path::new(current_path)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            crate::ErrorKind::InputError(
                "The current content filename is invalid".to_string(),
            )
        })?;
    let disabled = current_name.ends_with(".disabled");
    let current_base = current_name
        .strip_suffix(".disabled")
        .unwrap_or(current_name);
    let replacement = match old_provider_file_name.as_deref() {
        Some(old_name) if current_base == old_name => {
            new_provider_file_name.to_string()
        }
        Some(old_name)
            if !old_name.is_empty()
                && current_base.match_indices(old_name).count() == 1 =>
        {
            current_base.replacen(old_name, &new_provider_file_name, 1)
        }
        _ => current_base.to_string(),
    };
    let file_name = if disabled {
        format!("{replacement}.disabled")
    } else {
        replacement
    };
    if file_name.is_empty()
        || Path::new(&file_name).components().count() != 1
        || file_name == "."
        || file_name == ".."
    {
        return Err(crate::ErrorKind::InputError(
            "The preserved content filename is invalid".to_string(),
        )
        .into());
    }
    let parent = Path::new(current_path)
        .parent()
        .and_then(|parent| parent.to_str())
        .unwrap_or_default();
    let relative_path = if parent.is_empty() {
        file_name
    } else {
        format!("{parent}/{file_name}")
    };
    path_util::SafeRelativeUtf8UnixPathBuf::try_from(relative_path.clone())?;
    Ok(relative_path)
}

async fn configured_update_relative_path(
    current_path: &str,
    old_name: Option<&str>,
    new_name: &str,
    pool: &sqlx::SqlitePool,
) -> crate::Result<String> {
    let settings = crate::state::Settings::get(pool).await?;
    let current_name = current_path.rsplit('/').next().unwrap_or(current_path);
    let current_name = current_name
        .strip_suffix(".disabled")
        .unwrap_or(current_name);
    preserved_update_relative_path(
        current_path,
        if settings.resource_preserve_filenames {
            old_name
        } else {
            Some(current_name)
        },
        new_name,
    )
}

pub(crate) async fn finalize_updated_project_path(
    instance_id: &str,
    old_path: &str,
    installed_path: &str,
    old_provider_file_name: Option<&str>,
    new_provider_file_name: &str,
    state: &State,
) -> crate::Result<String> {
    let desired_path = configured_update_relative_path(
        old_path,
        old_provider_file_name,
        new_provider_file_name,
        &state.pool,
    )
    .await?;
    let mut old_archived = false;
    if desired_path == old_path && installed_path != old_path {
        if archive_project_file(instance_id, old_path, installed_path, state)
            .await?
            .is_none()
        {
            remove_project(instance_id, old_path, state).await?;
        }
        old_archived = true;
    }
    let final_path = if desired_path != installed_path {
        match rename_project_file(
            instance_id,
            installed_path,
            &desired_path,
            state,
        )
        .await
        {
            Ok(()) => desired_path,
            Err(error) => {
                return match remove_project(
                    instance_id,
                    installed_path,
                    state,
                )
                .await
                {
                    Ok(()) => Err(error),
                    Err(cleanup_error) => Err(
                        crate::ErrorKind::OtherError(format!(
                            "Could not finalize the updated content path: {error}; cleanup also failed: {cleanup_error}"
                        ))
                        .into(),
                    ),
                };
            }
        }
    } else {
        installed_path.to_string()
    };
    if !old_archived
        && final_path != old_path
        && archive_project_file(instance_id, old_path, &final_path, state)
            .await?
            .is_none()
    {
        remove_project(instance_id, old_path, state).await?;
    }
    Ok(final_path)
}

async fn rename_project_file(
    instance_id: &str,
    current_path: &str,
    new_path: &str,
    state: &State,
) -> crate::Result<()> {
    let _instance_lock = state.lock_instance_content(instance_id).await;
    let scope = resolve_content_scope(instance_id, None, state).await?;
    let base = instance_full_path(state, &scope.instance);
    let target = join_content_path(&base, new_path);
    if target.exists() {
        return Err(crate::ErrorKind::InputError(format!(
            "Cannot preserve the content filename because {new_path} already exists"
        ))
        .into());
    }
    let source = join_content_path(&base, current_path);
    io::rename_or_move(&source, &target).await?;
    if let Err(error) = rename_indexed_file(
        &scope,
        current_path,
        current_path,
        new_path,
        !new_path.ends_with(".disabled"),
        state,
    )
    .await
    {
        return match io::rename_or_move(&target, &source).await {
            Ok(()) => Err(error),
            Err(rollback_error) => Err(crate::ErrorKind::OtherError(format!(
                "Could not update the content index after moving {current_path} to {new_path}: {error}; restoring the file also failed: {rollback_error}"
            ))
            .into()),
        };
    }
    Ok(())
}

async fn add_resolved_content_with_progress(
    instance_id: &str,
    content: &ResolvedContent,
    reason: DownloadReason,
    auto_dependency: bool,
    progress: Option<ResolvedContentDownloadProgress>,
    state: &State,
) -> crate::Result<String> {
    let path = add_project_from_version_with_progress(
        instance_id,
        &content.version_id,
        reason,
        content.dependent_on_version_id.clone(),
        ContentSourceKind::Local,
        ContentOwnershipKind::UserAdded,
        progress,
        state,
    )
    .await?;
    if auto_dependency {
        let scope = resolve_content_scope(instance_id, None, state).await?;
        if let Some(entry) = content_rows::get_content_entry_by_relative_path(
            &scope.content_set_id,
            &path,
            &state.pool,
        )
        .await?
        {
            let _database_permit = state.acquire_install_db_permit().await?;
            content_rows::set_content_entry_auto_dependency(
                &entry.id,
                true,
                &state.pool,
            )
            .await?;
        }
    }
    Ok(path)
}

/// Materialize one already-resolved Modrinth dependency without also
/// installing the plan's primary. CurseForge uses this for a SHA-1 verified
/// cross-source fallback whose primary is already installed from CurseForge.
pub(crate) async fn install_resolved_dependency(
    instance_id: &str,
    content: &ResolvedContent,
    state: &State,
) -> crate::Result<String> {
    add_resolved_content_with_progress(
        instance_id,
        content,
        DownloadReason::Dependency,
        true,
        None,
        state,
    )
    .await
}

pub(crate) async fn persist_resolved_plan_dependency_edges(
    instance_id: &str,
    paths: &[String],
    plan: &ResolveContentPlan,
    state: &State,
) -> crate::Result<()> {
    if plan.dependencies.is_empty() {
        return Ok(());
    }
    let database_permit = state.acquire_install_db_permit().await?;
    let _instance_lock = state.lock_instance_content(instance_id).await;
    let scope = resolve_content_scope(instance_id, None, state).await?;
    let planned = std::iter::once(&plan.primary)
        .chain(plan.dependencies.iter())
        .collect::<Vec<_>>();

    let mut tx = state.pool.begin().await?;
    for (index, dependency) in plan.dependencies.iter().enumerate() {
        let Some(parent) = dependency
            .dependent_on_version_id
            .as_deref()
            .and_then(|parent_version_id| {
                planned
                    .iter()
                    .find(|planned| planned.version_id == parent_version_id)
            })
        else {
            continue;
        };
        let parent_index = planned
            .iter()
            .position(|planned| planned.version_id == parent.version_id)
            .unwrap_or(0);
        let Some(parent_path) = paths.get(parent_index) else {
            continue;
        };
        let Some(parent_entry) =
            content_rows::get_content_entry_by_relative_path(
                &scope.content_set_id,
                parent_path,
                &state.pool,
            )
            .await?
        else {
            continue;
        };
        let Some(child_entry) =
            content_rows::get_content_entry_by_relative_path(
                &scope.content_set_id,
                &paths[index + 1],
                &state.pool,
            )
            .await?
        else {
            continue;
        };
        let now = chrono::Utc::now();
        content_rows::upsert_content_dependency_edge_in_transaction(
            &crate::state::instances::ContentDependencyEdge {
                id: format!("content-dependency:{}", uuid::Uuid::new_v4()),
                content_set_id: scope.content_set_id.clone(),
                parent_entry_id: parent_entry.id,
                child_entry_id: child_entry.id,
                evidence_provider: crate::state::ContentProvider::Modrinth,
                parent_provider: crate::state::ContentProvider::Modrinth,
                child_provider: crate::state::ContentProvider::Modrinth,
                dependency_kind: if dependency.required {
                    crate::state::instances::ContentDependencyKind::Required
                } else {
                    crate::state::instances::ContentDependencyKind::Include
                },
                parent_project_id: parent.project_id.clone(),
                parent_release_id: parent.version_id.clone(),
                child_project_id: dependency.project_id.clone(),
                child_release_id: dependency.version_id.clone(),
                created_at: now,
                modified_at: now,
            },
            &mut tx,
        )
        .await?;
    }
    for skipped in &plan.skipped {
        if skipped.reason != SkippedReason::AlreadyInstalled {
            continue;
        }
        let Some(parent_version_id) =
            skipped.dependent_on_version_id.as_deref()
        else {
            continue;
        };
        let Some(parent) = planned
            .iter()
            .find(|planned| planned.version_id == parent_version_id)
        else {
            continue;
        };
        let parent_index = planned
            .iter()
            .position(|planned| planned.version_id == parent_version_id)
            .unwrap_or(0);
        let Some(parent_path) = paths.get(parent_index) else {
            continue;
        };
        let Some(parent_entry) =
            content_rows::get_content_entry_by_relative_path(
                &scope.content_set_id,
                parent_path,
                &state.pool,
            )
            .await?
        else {
            continue;
        };
        let Some(child_release_id) = skipped.version_id.as_deref() else {
            continue;
        };
        let Some(child_entry) =
            content_rows::get_content_entry_by_provider_ref(
                &scope.content_set_id,
                crate::state::ContentProvider::Modrinth,
                &skipped.project_id,
                child_release_id,
                &state.pool,
            )
            .await?
        else {
            continue;
        };
        let now = chrono::Utc::now();
        content_rows::upsert_content_dependency_edge_in_transaction(
            &crate::state::instances::ContentDependencyEdge {
                id: format!("content-dependency:{}", uuid::Uuid::new_v4()),
                content_set_id: scope.content_set_id.clone(),
                parent_entry_id: parent_entry.id,
                child_entry_id: child_entry.id,
                evidence_provider: crate::state::ContentProvider::Modrinth,
                parent_provider: crate::state::ContentProvider::Modrinth,
                child_provider: crate::state::ContentProvider::Modrinth,
                dependency_kind:
                    crate::state::instances::ContentDependencyKind::Required,
                parent_project_id: parent.project_id.clone(),
                parent_release_id: parent.version_id.clone(),
                child_project_id: skipped.project_id.clone(),
                child_release_id: child_release_id.to_string(),
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

pub(crate) async fn resolve_content_scope(
    instance_id: &str,
    content_set_id: Option<&str>,
    state: &State,
) -> crate::Result<ContentScope> {
    let instance = instance_rows::get_instance_by_id(instance_id, &state.pool)
        .await?
        .ok_or_else(|| {
            crate::ErrorKind::InputError("Unknown instance".to_string())
        })?;
    let content_set_id = match content_set_id {
        Some(id) => id.to_string(),
        None => instance.applied_content_set_id.clone().ok_or_else(|| {
            crate::ErrorKind::InputError(format!(
                "Instance {} has no applied content set",
                instance.id
            ))
        })?,
    };

    Ok(ContentScope {
        instance,
        content_set_id,
    })
}

pub(crate) async fn add_project_from_version(
    instance_id: &str,
    version_id: &str,
    reason: DownloadReason,
    dependent_on_version_id: Option<String>,
    source_kind: ContentSourceKind,
    ownership_kind: ContentOwnershipKind,
    state: &State,
) -> crate::Result<String> {
    add_project_from_version_with_progress(
        instance_id,
        version_id,
        reason,
        dependent_on_version_id,
        source_kind,
        ownership_kind,
        None,
        state,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn add_project_from_version_with_progress(
    instance_id: &str,
    version_id: &str,
    reason: DownloadReason,
    dependent_on_version_id: Option<String>,
    source_kind: ContentSourceKind,
    ownership_kind: ContentOwnershipKind,
    progress: Option<ResolvedContentDownloadProgress>,
    state: &State,
) -> crate::Result<String> {
    let downloaded = download_project_version_with_progress(
        instance_id,
        version_id,
        reason,
        dependent_on_version_id,
        progress,
        state,
    )
    .await?;

    add_downloaded_project_version(
        instance_id,
        downloaded,
        source_kind,
        ownership_kind,
        state,
    )
    .await
}

pub(crate) async fn download_project_version(
    instance_id: &str,
    version_id: &str,
    reason: DownloadReason,
    dependent_on_version_id: Option<String>,
    state: &State,
) -> crate::Result<DownloadedProjectVersion> {
    download_project_version_with_progress(
        instance_id,
        version_id,
        reason,
        dependent_on_version_id,
        None,
        state,
    )
    .await
}

/// Progress context for one file inside a multi-file content install.
#[derive(Clone)]
pub(crate) struct ResolvedContentDownloadProgress {
    pub reporter: crate::install::InstallProgressReporter,
    pub file_index: u64,
    pub file_count: u64,
    pub base_bytes: u64,
    pub total_bytes: u64,
}

pub(crate) async fn download_project_version_with_progress(
    instance_id: &str,
    version_id: &str,
    reason: DownloadReason,
    dependent_on_version_id: Option<String>,
    progress: Option<ResolvedContentDownloadProgress>,
    state: &State,
) -> crate::Result<DownloadedProjectVersion> {
    download_project_version_with_reporting(
        instance_id,
        version_id,
        reason,
        dependent_on_version_id,
        progress,
        None,
        state,
    )
    .await
}

pub(crate) async fn download_project_version_with_reporter(
    instance_id: &str,
    version_id: &str,
    reason: DownloadReason,
    dependent_on_version_id: Option<String>,
    reporter: crate::install::InstallProgressReporter,
    state: &State,
) -> crate::Result<DownloadedProjectVersion> {
    download_project_version_with_reporting(
        instance_id,
        version_id,
        reason,
        dependent_on_version_id,
        None,
        Some(reporter),
        state,
    )
    .await
}

async fn download_project_version_with_reporting(
    instance_id: &str,
    version_id: &str,
    reason: DownloadReason,
    dependent_on_version_id: Option<String>,
    progress: Option<ResolvedContentDownloadProgress>,
    reporter: Option<crate::install::InstallProgressReporter>,
    state: &State,
) -> crate::Result<DownloadedProjectVersion> {
    let prepared = prepare_version_download(
        instance_id,
        version_id,
        reason,
        dependent_on_version_id,
        state,
    )
    .await?;
    let integrity = prepared.integrity.clone();
    let mut request =
        DownloadRequest::new(&prepared.url, ResourceClass::Modrinth)
            .with_integrity(integrity.clone())
            .with_download_meta(prepared.download_meta);
    let tracking_reporter = progress
        .as_ref()
        .map(|progress| progress.reporter.clone())
        .or(reporter);
    if let Some(reporter) = tracking_reporter {
        request = request.with_install_tracking(
            reporter,
            prepared.path.display().to_string(),
            &prepared.file_name,
        );
    }

    let mut last_reported_bytes = 0_u64;
    let mut progress_callback = progress.map(|progress| {
        let reporter = progress.reporter.clone();
        let file_index = progress.file_index;
        let file_count = progress.file_count;
        let base_bytes = progress.base_bytes;
        let total_bytes = progress.total_bytes.max(1);
        move |current: u64,
              total: u64|
              -> std::pin::Pin<
            Box<dyn Future<Output = crate::Result<()>> + Send + '_>,
        > {
            let min_delta = (total / 200).max(256 * 1024);
            if current < total
                && current.saturating_sub(last_reported_bytes) < min_delta
            {
                return Box::pin(async { Ok(()) });
            }
            last_reported_bytes = current;
            let current_total = (base_bytes + current).min(total_bytes);
            let secondary = crate::install::InstallProgressSecondary {
                current: current_total,
                total: total_bytes,
            };
            let reporter = reporter.clone();
            Box::pin(async move {
                reporter
                    .update(
                        crate::install::InstallPhaseId::DownloadingContent,
                        Some(crate::install::InstallProgress {
                            current: file_index,
                            total: file_count,
                            secondary: Some(secondary),
                        }),
                        crate::install::InstallPhaseDetails::Empty,
                    )
                    .await?;
                Ok(())
            })
        }
    });
    let progress = progress_callback
        .as_mut()
        .map(|progress| progress as &mut fetch::FetchProgressFn<'_>);

    let download = download_to_path(
        request,
        &prepared.path,
        &state.download_semaphore,
        &state.pool,
        progress,
    )
    .await?;

    let mut content_path = prepared.path.clone();
    if let Some(sha512) = integrity.sha512.as_deref() {
        content_path = crate::state::content_store::publish_verified(
            &prepared.path,
            &state.directories,
            sha512,
        )
        .await?;
        crate::state::content_store::record_published(
            &state.pool,
            sha512,
            download.size,
        )
        .await?;
    }
    let sha1 = if let Some(hash) = &prepared.sha1 {
        hash.clone()
    } else {
        fetch::sha1_file_async(&prepared.path).await?.1
    };
    let project_type = ProjectType::get_from_loaders(prepared.loaders.clone())
        .ok_or_else(|| {
            crate::ErrorKind::InputError(format!(
                "Unable to infer project type for version {version_id}"
            ))
        })?;

    Ok(DownloadedProjectVersion {
        file_name: prepared.file_name,
        path: content_path,
        sha1,
        size: download.size,
        project_type,
        project_id: prepared.project_id,
        version_id: prepared.version_id,
    })
}

/// Everything needed to download a version file, resolved before the actual
/// network transfer.
pub(crate) struct PreparedVersionDownload {
    pub(crate) url: String,
    pub(crate) path: PathBuf,
    pub(crate) download_meta: DownloadMeta,
    pub(crate) integrity: Integrity,
    pub(crate) file_name: String,
    pub(crate) sha1: Option<String>,
    pub(crate) loaders: Vec<String>,
    pub(crate) project_id: String,
    pub(crate) version_id: String,
}

/// Resolves the content scope and version metadata for a download and
/// validates the target path, without touching the network.
pub(crate) async fn prepare_version_download(
    instance_id: &str,
    version_id: &str,
    reason: DownloadReason,
    dependent_on_version_id: Option<String>,
    state: &State,
) -> crate::Result<PreparedVersionDownload> {
    let scope = resolve_content_scope(instance_id, None, state).await?;
    let content_set =
        content_rows::get_content_set(&scope.content_set_id, &state.pool)
            .await?
            .ok_or_else(|| {
                crate::ErrorKind::InputError(format!(
                    "Unknown content set {}",
                    scope.content_set_id
                ))
            })?;
    let version = CachedEntry::get_version(
        &ModrinthVersionId::new(version_id.to_string())?,
        None,
        &state.pool,
        &state.api_semaphore,
    )
    .await?
    .ok_or_else(|| {
        crate::ErrorKind::InputError(format!(
            "Unable to install version id {version_id}. Not found."
        ))
    })?;
    let file = version
        .files
        .iter()
        .find(|file| file.primary)
        .or_else(|| version.files.first())
        .ok_or_else(|| {
            crate::ErrorKind::InputError(
                "No files for input version present!".to_string(),
            )
        })?;
    let download_meta = DownloadMeta {
        reason,
        game_version: content_set.game_version,
        loader: content_set.loader.as_str().to_string(),
        dependent_on: dependent_on_version_id,
    };
    let file_name = io::sanitize_download_file_name(&file.filename)?;
    let file_name_path = Path::new(&file_name);
    let path = state
        .directories
        .caches_dir()
        .join("content")
        .join("modrinth")
        .join(&version.id)
        .join(file_name_path);
    let content = if matches!(
        file_name_path.extension().and_then(|value| value.to_str()),
        Some("jar" | "zip" | "mrpack")
    ) {
        ContentValidation::Jar
    } else {
        ContentValidation::None
    };
    let integrity = Integrity {
        size: Some(file.size as u64),
        sha1: file.hashes.get("sha1").cloned(),
        sha512: file.hashes.get("sha512").cloned(),
        content,
        ..Integrity::default()
    };

    Ok(PreparedVersionDownload {
        url: file.url.clone(),
        path,
        download_meta,
        integrity,
        file_name,
        sha1: file.hashes.get("sha1").cloned(),
        loaders: version.loaders.clone(),
        project_id: version.project_id.clone(),
        version_id: version.id.clone(),
    })
}

/// Chooses the instance-relative path for a content file that is about to be
/// installed. Paths already recorded for this instance always win, so repeat
/// installs never duplicate a project across naming preference changes.
pub(crate) async fn resolve_content_install_relative_path(
    instance_id: &str,
    original_relative_path: String,
    localized_candidate: Option<String>,
    pool: &sqlx::SqlitePool,
) -> crate::Result<String> {
    if content_rows::get_instance_file_by_relative_path(
        instance_id,
        &original_relative_path,
        pool,
    )
    .await?
    .is_some()
    {
        return Ok(original_relative_path);
    }
    let Some(localized_candidate) = localized_candidate else {
        return Ok(original_relative_path);
    };
    if content_rows::get_instance_file_by_relative_path(
        instance_id,
        &localized_candidate,
        pool,
    )
    .await?
    .is_some()
    {
        return Ok(localized_candidate);
    }
    if crate::state::Settings::get(pool)
        .await?
        .resource_chinese_filenames
    {
        return Ok(localized_candidate);
    }
    Ok(original_relative_path)
}

/// Builds the `[中文名]` file-name candidate for a Modrinth project file by
/// resolving the project's slug from cache. Any failure disables the naming.
async fn modrinth_chinese_file_name_candidate(
    project_id: &str,
    file_name: &str,
    state: &State,
) -> Option<String> {
    if !crate::state::Settings::get(&state.pool)
        .await
        .ok()?
        .resource_chinese_filenames
    {
        return None;
    }
    let project = CachedEntry::get_project(
        &ModrinthProjectId::new(project_id.to_string()).ok()?,
        None,
        &state.pool,
        &state.api_semaphore,
    )
    .await
    .ok()??;
    let title = chinese_file_title_for_modrinth_slug(&project.slug?)?;
    localized_content_file_name(file_name, &title)
}

pub(crate) async fn add_downloaded_project_version(
    instance_id: &str,
    downloaded: DownloadedProjectVersion,
    source_kind: ContentSourceKind,
    ownership_kind: ContentOwnershipKind,
    state: &State,
) -> crate::Result<String> {
    let database_permit = Some(state.acquire_install_db_permit().await?);
    let _instance_lock = state
        .lock_instance_content_with_timeout(
            instance_id,
            std::time::Duration::from_secs(20),
        )
        .await?;

    let DownloadedProjectVersion {
        file_name,
        path,
        sha1,
        size,
        project_type,
        project_id,
        version_id,
    } = downloaded;
    let scope = resolve_content_scope(instance_id, None, state).await?;
    let localized_candidate = if project_type == ProjectType::Mod {
        None
    } else {
        modrinth_chinese_file_name_candidate(&project_id, &file_name, state)
            .await
            .map(|file_name| {
                format!("{}/{}", project_type.get_folder(), file_name)
            })
    };
    let relative_path = resolve_content_install_relative_path(
        instance_id,
        format!("{}/{}", project_type.get_folder(), file_name),
        localized_candidate,
        &state.pool,
    )
    .await?;
    let full_path = join_content_path(
        &instance_full_path(state, &scope.instance),
        &relative_path,
    );
    let previous_path = materialize_project_download(&path, &full_path).await?;
    let provider_ref = ContentProviderRef::Modrinth {
        project_id: ModrinthProjectId::new(project_id.clone())?,
        version_id: Some(ModrinthVersionId::new(version_id.clone())?),
    };
    let record_result = record_project_files_atomic_with_permit(
        instance_id,
        &[ProjectFileRecord {
            relative_path: relative_path.clone(),
            sha1: sha1.clone(),
            size,
            project_type,
            source_kind,
            ownership_kind,
            provider_ref: Some(provider_ref),
            origin: true,
            known_modrinth_project_id: Some(project_id.clone()),
            known_modrinth_version_id: Some(version_id.clone()),
        }],
        &[],
        database_permit,
        state,
    )
    .await;
    match record_result {
        Ok(()) => {
            finalize_project_materialization(previous_path.as_deref()).await?
        }
        Err(error) => {
            restore_project_materialization(
                &full_path,
                previous_path.as_deref(),
            )
            .await?;
            return Err(error);
        }
    }
    Ok(relative_path)
}

pub(crate) async fn apply_downloaded_project_version_at_path(
    instance_id: &str,
    relative_path: &str,
    downloaded: DownloadedProjectVersion,
    source_kind: ContentSourceKind,
    ownership_kind: ContentOwnershipKind,
    state: &State,
) -> crate::Result<String> {
    let database_permit = Some(state.acquire_install_db_permit().await?);
    let _instance_lock = state
        .lock_instance_content_with_timeout(
            instance_id,
            std::time::Duration::from_secs(20),
        )
        .await?;
    let DownloadedProjectVersion {
        path,
        sha1,
        size,
        project_type,
        project_id,
        version_id,
        ..
    } = downloaded;
    let scope = resolve_content_scope(instance_id, None, state).await?;
    let full_path = join_content_path(
        &instance_full_path(state, &scope.instance),
        relative_path,
    );
    let previous_path = materialize_project_download(&path, &full_path).await?;
    let provider_ref = ContentProviderRef::Modrinth {
        project_id: ModrinthProjectId::new(project_id.clone())?,
        version_id: Some(ModrinthVersionId::new(version_id.clone())?),
    };
    let record_result = record_project_files_atomic_with_permit(
        instance_id,
        &[ProjectFileRecord {
            relative_path: relative_path.to_string(),
            sha1: sha1.clone(),
            size,
            project_type,
            source_kind,
            ownership_kind,
            provider_ref: Some(provider_ref),
            origin: true,
            known_modrinth_project_id: Some(project_id.clone()),
            known_modrinth_version_id: Some(version_id.clone()),
        }],
        &[],
        database_permit,
        state,
    )
    .await;
    match record_result {
        Ok(()) => {
            finalize_project_materialization(previous_path.as_deref()).await?
        }
        Err(error) => {
            restore_project_materialization(
                &full_path,
                previous_path.as_deref(),
            )
            .await?;
            return Err(error);
        }
    }
    Ok(relative_path.to_string())
}

pub(crate) async fn content_ownership_for_path(
    instance_id: &str,
    project_path: &str,
    state: &State,
) -> crate::Result<ContentOwnershipKind> {
    let ownership = sqlx::query_scalar::<_, String>(
        "SELECT entry.ownership_kind
         FROM instance_files file
         INNER JOIN instance_content_entries entry ON entry.file_id = file.id
         INNER JOIN instances instance ON instance.id = entry.instance_id
         WHERE file.instance_id = ? AND file.relative_path = ?
            AND entry.content_set_id = instance.applied_content_set_id
         ORDER BY entry.modified_at DESC
         LIMIT 1",
    )
    .bind(instance_id)
    .bind(project_path)
    .fetch_optional(&state.pool)
    .await?;

    ownership
        .as_deref()
        .map(ContentOwnershipKind::from_str)
        .transpose()
        .map(|ownership| ownership.unwrap_or_default())
}

pub(crate) async fn materialize_project_download(
    source: &Path,
    destination: &Path,
) -> crate::Result<Option<PathBuf>> {
    if let Some(parent) = destination.parent() {
        io::create_dir_all(parent).await?;
    }
    let mut temporary = destination.as_os_str().to_os_string();
    temporary.push(".installing");
    let temporary = PathBuf::from(temporary);
    if temporary.exists() {
        io::remove_file(&temporary).await?;
    }
    io::copy(source, &temporary).await?;
    let mut backup = destination.as_os_str().to_os_string();
    backup.push(".installing.previous");
    let backup = PathBuf::from(backup);
    if backup.exists() {
        io::remove_file(&backup).await?;
    }
    if destination.exists() {
        tokio::fs::rename(destination, &backup).await?;
    }
    if let Err(error) = tokio::fs::rename(&temporary, destination).await {
        if backup.exists() {
            let _ = tokio::fs::rename(&backup, destination).await;
        }
        warn!(
            "Failed to rename temporary file to destination — checking file lock"
        );
        return Err(crate::Error::from(io_error_with_lock_info(
            error,
            destination,
        )));
    }
    Ok(destination
        .exists()
        .then_some(backup)
        .filter(|path| path.exists()))
}

pub(crate) async fn materialize_project_download_batch(
    instance_id: &str,
    downloads: Vec<DownloadedProjectVersion>,
    plan: Option<&ResolveContentPlan>,
    state: &State,
) -> crate::Result<Vec<String>> {
    let database_permit = state.acquire_install_db_permit().await?;
    let _instance_lock = state.lock_instance_content(instance_id).await;
    let scope = resolve_content_scope(instance_id, None, state).await?;
    let base = instance_full_path(state, &scope.instance);
    let mut records = Vec::with_capacity(downloads.len());
    let mut sources = Vec::with_capacity(downloads.len());
    let mut destinations = std::collections::HashSet::new();
    for download in downloads {
        let localized = if download.project_type == ProjectType::Mod {
            None
        } else {
            modrinth_chinese_file_name_candidate(
                &download.project_id,
                &download.file_name,
                state,
            )
            .await
            .map(|name| {
                format!("{}/{}", download.project_type.get_folder(), name)
            })
        };
        let relative_path = resolve_content_install_relative_path(
            instance_id,
            format!(
                "{}/{}",
                download.project_type.get_folder(),
                download.file_name
            ),
            localized,
            &state.pool,
        )
        .await?;
        if !destinations.insert(materialization_path_key(&relative_path)) {
            return Err(crate::ErrorKind::InputError(format!(
                "Multiple content files target {relative_path}"
            ))
            .into());
        }
        sources.push((download.path, join_content_path(&base, &relative_path)));
        records.push(ProjectFileRecord {
            relative_path,
            sha1: download.sha1,
            size: download.size,
            project_type: download.project_type,
            source_kind: ContentSourceKind::Local,
            ownership_kind: ContentOwnershipKind::UserAdded,
            provider_ref: Some(ContentProviderRef::Modrinth {
                project_id: ModrinthProjectId::new(
                    download.project_id.clone(),
                )?,
                version_id: Some(ModrinthVersionId::new(
                    download.version_id.clone(),
                )?),
            }),
            origin: true,
            known_modrinth_project_id: Some(download.project_id),
            known_modrinth_version_id: Some(download.version_id),
        });
    }
    let _critical = crate::install::critical_section::enter();
    let mut materialized = Vec::with_capacity(sources.len());
    let result: crate::Result<()> = async {
        for (source, destination) in &sources {
            let backup =
                materialize_project_download(source, destination).await?;
            materialized.push((destination.clone(), backup));
        }
        record_project_files_atomic_with_plan(
            instance_id,
            &records,
            Some(database_permit),
            plan,
            state,
        )
        .await
    }
    .await;
    if let Err(error) = result {
        let mut rollback_errors = Vec::new();
        for (destination, backup) in materialized.iter().rev() {
            if let Err(error) =
                restore_project_materialization(destination, backup.as_deref())
                    .await
            {
                rollback_errors.push(error.to_string());
            }
        }
        if !rollback_errors.is_empty() {
            return Err(crate::ErrorKind::OtherError(format!(
                "{error}; file rollback failed: {}",
                rollback_errors.join("; ")
            ))
            .into());
        }
        return Err(error);
    }
    for (_, backup) in &materialized {
        if let Err(error) =
            finalize_project_materialization(backup.as_deref()).await
        {
            tracing::warn!(%error, "Could not remove committed content backup");
        }
    }
    Ok(records
        .into_iter()
        .map(|record| record.relative_path)
        .collect())
}

fn materialization_path_key(path: &str) -> String {
    if cfg!(any(windows, target_os = "macos")) {
        path.replace('\\', "/").to_lowercase()
    } else {
        path.to_string()
    }
}

async fn record_project_files_atomic_with_plan(
    instance_id: &str,
    records: &[ProjectFileRecord],
    database_permit: Option<tokio::sync::SemaphorePermit<'_>>,
    plan: Option<&ResolveContentPlan>,
    state: &State,
) -> crate::Result<()> {
    record_project_files_transaction(
        instance_id,
        records,
        &[],
        database_permit,
        plan,
        state,
    )
    .await
}

pub(crate) async fn materialize_verified_project_download_copy(
    source: &Path,
    destination: &Path,
    verified_size: u64,
    verified_sha1: &str,
) -> crate::Result<Option<PathBuf>> {
    if let Some(parent) = destination.parent() {
        io::create_dir_all(parent).await?;
    }
    let mut temporary = destination.as_os_str().to_os_string();
    temporary.push(".installing");
    let temporary = PathBuf::from(temporary);
    if temporary.exists() {
        io::remove_file(&temporary).await?;
    }
    if let Err(error) = tokio::fs::copy(source, &temporary).await {
        if temporary.exists() {
            let _ = io::remove_file(&temporary).await;
        }
        return Err(error.into());
    }
    if let Err(error) = verify_project_download_identity(
        &temporary,
        verified_size,
        verified_sha1,
    )
    .await
    {
        let _ = io::remove_file(&temporary).await;
        return Err(error);
    }

    let mut backup = destination.as_os_str().to_os_string();
    backup.push(".installing.previous");
    let backup = PathBuf::from(backup);
    if backup.exists() {
        io::remove_file(&backup).await?;
    }
    if destination.exists() {
        tokio::fs::rename(destination, &backup).await?;
    }
    if let Err(error) = tokio::fs::rename(&temporary, destination).await {
        if backup.exists() {
            let _ = tokio::fs::rename(&backup, destination).await;
        }
        let _ = io::remove_file(&temporary).await;
        return Err(crate::Error::from(io_error_with_lock_info(
            error,
            destination,
        )));
    }

    let previous = destination
        .exists()
        .then_some(backup)
        .filter(|path| path.exists());
    if let Err(error) = verify_project_download_identity(
        destination,
        verified_size,
        verified_sha1,
    )
    .await
    {
        restore_project_materialization(destination, previous.as_deref())
            .await?;
        return Err(error);
    }
    Ok(previous)
}

async fn verify_project_download_identity(
    path: &Path,
    verified_size: u64,
    verified_sha1: &str,
) -> crate::Result<()> {
    let (actual_size, actual_sha1) = fetch::sha1_file_async(path).await?;
    if actual_size != verified_size {
        return Err(crate::ErrorKind::OtherError(format!(
            "Copied manual download size changed: {verified_size} != {actual_size}"
        ))
        .into());
    }
    if !actual_sha1.eq_ignore_ascii_case(verified_sha1) {
        return Err(crate::ErrorKind::HashError(
            verified_sha1.to_string(),
            actual_sha1,
        )
        .into());
    }
    Ok(())
}

pub(crate) async fn finalize_project_materialization(
    previous_path: Option<&Path>,
) -> crate::Result<()> {
    if let Some(previous_path) = previous_path
        && previous_path.exists()
    {
        io::remove_file(previous_path).await?;
    }
    Ok(())
}

pub(crate) async fn restore_project_materialization(
    destination: &Path,
    previous_path: Option<&Path>,
) -> crate::Result<()> {
    if destination.exists() {
        io::remove_file(destination).await?;
    }
    if let Some(previous_path) = previous_path
        && previous_path.exists()
    {
        tokio::fs::rename(previous_path, destination).await?;
    }
    Ok(())
}

pub(crate) async fn add_project_from_path(
    instance_id: &str,
    path: &Path,
    project_type: Option<ProjectType>,
    inner_base: Option<&str>,
    state: &State,
) -> crate::Result<String> {
    if let Some(inner_base) = inner_base {
        let temp_dir = if is_zip_path(path) {
            Some(tempfile::tempdir().map_err(|e| {
                crate::ErrorKind::InputError(format!(
                    "Failed to create temporary directory: {e}"
                ))
            })?)
        } else {
            None
        };

        let source = if let Some(temp_dir) = &temp_dir {
            extract_zip_to_dir_async(path, temp_dir.path()).await?;
            crate::util::io::join_within_root(temp_dir.path(), inner_base)?
        } else {
            crate::util::io::join_within_root(path, inner_base)?
        };

        let result = if source.is_dir() {
            let bytes = zip_directory(&source)?;
            let file_name = source
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
                + ".zip";
            add_project_bytes(
                instance_id,
                &file_name,
                bytes,
                None,
                project_type,
                ContentSourceKind::Local,
                state,
            )
            .await
        } else {
            let file = io::read(&source).await?;
            let file_name = source
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            add_project_bytes(
                instance_id,
                &file_name,
                Bytes::from(file),
                None,
                project_type,
                ContentSourceKind::Local,
                state,
            )
            .await
        };
        return result;
    }

    let file = io::read(path).await?;
    let file_name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    add_project_bytes(
        instance_id,
        &file_name,
        Bytes::from(file),
        None,
        project_type,
        ContentSourceKind::Local,
        state,
    )
    .await
}

fn is_zip_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            ext.eq_ignore_ascii_case("zip")
                || ext.eq_ignore_ascii_case("mrpack")
        })
}

fn zip_directory(path: &Path) -> crate::Result<Bytes> {
    let Some(root_name) = path.file_name() else {
        return Err(crate::ErrorKind::InputError(
            "Cannot determine folder name for zipped pack".to_string(),
        )
        .into());
    };
    let mut files = Vec::new();
    collect_zip_files(path, path, &mut files)?;

    let mut cursor = Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut cursor);
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated);
        for (absolute, relative) in files {
            let name = format!(
                "{}/{}",
                root_name.to_string_lossy(),
                relative.to_string_lossy().replace('\\', "/")
            );
            zip.start_file(name.as_str(), options).map_err(|e| {
                crate::Error::from(crate::ErrorKind::InputError(format!(
                    "Failed to start zip entry {name}: {e}"
                )))
            })?;
            let bytes = std::fs::read(&absolute).map_err(|e| {
                crate::Error::from(crate::ErrorKind::InputError(format!(
                    "Failed to read {}: {e}",
                    absolute.display()
                )))
            })?;
            zip.write_all(&bytes).map_err(|e| {
                crate::Error::from(crate::ErrorKind::InputError(format!(
                    "Failed to write zip entry {name}: {e}"
                )))
            })?;
        }
        zip.finish().map_err(|e| {
            crate::Error::from(crate::ErrorKind::InputError(format!(
                "Failed to finalize zip: {e}"
            )))
        })?;
    }
    Ok(Bytes::from(cursor.into_inner()))
}

fn collect_zip_files(
    root: &Path,
    dir: &Path,
    out: &mut Vec<(PathBuf, PathBuf)>,
) -> crate::Result<()> {
    let entries = std::fs::read_dir(dir).map_err(|e| {
        crate::Error::from(crate::ErrorKind::InputError(format!(
            "Failed to read {}: {e}",
            dir.display()
        )))
    })?;
    for entry in entries.flatten() {
        let path = entry.path();
        let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
        let Ok(metadata) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            collect_zip_files(root, &path, out)?;
        } else {
            out.push((path, relative));
        }
    }
    Ok(())
}

pub(crate) async fn install_datapack_to_world(
    instance_id: &str,
    world_path: &str,
    path: &Path,
    inner_base: Option<&str>,
    state: &State,
) -> crate::Result<String> {
    let scope = resolve_content_scope(instance_id, None, state).await?;
    let world_path = sanitize_world_path(world_path)?;
    let datapacks_dir = instance_full_path(state, &scope.instance)
        .join("saves")
        .join(&world_path)
        .join("datapacks");
    io::create_dir_all(&datapacks_dir).await?;

    let temp_dir = if path.is_dir() {
        None
    } else if is_zip_path(path) {
        Some(tempfile::tempdir().map_err(|e| {
            crate::ErrorKind::InputError(format!(
                "Failed to create temporary directory: {e}"
            ))
        })?)
    } else {
        None
    };

    let source = if let Some(temp_dir) = &temp_dir {
        extract_zip_to_dir_async(path, temp_dir.path()).await?;
        match inner_base {
            Some(base) => {
                crate::util::io::join_within_root(temp_dir.path(), base)?
            }
            None => temp_dir.path().to_path_buf(),
        }
    } else if let Some(inner_base) = inner_base {
        crate::util::io::join_within_root(path, inner_base)?
    } else {
        path.to_path_buf()
    };

    let (bytes, file_name) = if source.is_dir() {
        let bytes = zip_directory(&source)?;
        let name = source
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        (bytes, format!("{name}.zip"))
    } else {
        let bytes = Bytes::from(io::read(&source).await?);
        let name = source
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        (bytes, name)
    };

    let file_name = sanitize_file_name(&file_name);
    let full_path = datapacks_dir.join(&file_name);
    fetch::write(&full_path, &bytes, &state.io_semaphore).await?;
    Ok(format!("saves/{world_path}/datapacks/{file_name}"))
}

pub(crate) async fn install_datapack_bytes_to_world(
    instance_id: &str,
    world_path: &str,
    file_name: &str,
    bytes: &[u8],
    state: &State,
) -> crate::Result<String> {
    let scope = resolve_content_scope(instance_id, None, state).await?;
    let world_path = sanitize_world_path(world_path)?;
    let file_name = sanitize_file_name(file_name);
    let datapacks_dir = instance_full_path(state, &scope.instance)
        .join("saves")
        .join(&world_path)
        .join("datapacks");
    io::create_dir_all(&datapacks_dir).await?;
    let full_path = datapacks_dir.join(&file_name);
    fetch::write(&full_path, bytes, &state.io_semaphore).await?;
    Ok(format!("saves/{world_path}/datapacks/{file_name}"))
}

fn sanitize_world_path(world_path: &str) -> crate::Result<String> {
    let normalized = world_path.replace('\\', "/");
    let mut safe = Vec::new();
    for segment in normalized.split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }
        if segment == ".." || segment.contains(':') {
            return Err(crate::ErrorKind::InputError(
                "Invalid world path".to_string(),
            )
            .into());
        }
        safe.push(segment);
    }
    if safe.is_empty() {
        return Err(crate::ErrorKind::InputError(
            "World path is empty".to_string(),
        )
        .into());
    }
    Ok(safe.join("/"))
}

fn sanitize_file_name(file_name: &str) -> String {
    let name = Path::new(file_name)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "datapack.zip".to_string());
    if name.is_empty() {
        "datapack.zip".to_string()
    } else {
        name
    }
}

pub(crate) async fn add_project_bytes(
    instance_id: &str,
    file_name: &str,
    bytes: Bytes,
    hash: Option<&str>,
    project_type: Option<ProjectType>,
    source_kind: ContentSourceKind,
    state: &State,
) -> crate::Result<String> {
    add_project_bytes_with_provider(
        instance_id,
        file_name,
        bytes,
        hash,
        project_type,
        source_kind,
        None,
        state,
    )
    .await
}

pub(crate) async fn add_project_bytes_from_provider(
    instance_id: &str,
    file_name: &str,
    bytes: Bytes,
    hash: Option<&str>,
    project_type: ProjectType,
    source_kind: ContentSourceKind,
    provider_ref: &ContentProviderRef,
    state: &State,
) -> crate::Result<String> {
    add_project_bytes_with_provider(
        instance_id,
        file_name,
        bytes,
        hash,
        Some(project_type),
        source_kind,
        Some(provider_ref),
        state,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn add_project_bytes_with_provider(
    instance_id: &str,
    file_name: &str,
    bytes: Bytes,
    hash: Option<&str>,
    project_type: Option<ProjectType>,
    source_kind: ContentSourceKind,
    provider_ref: Option<&ContentProviderRef>,
    state: &State,
) -> crate::Result<String> {
    let _instance_lock = state.lock_instance_content(instance_id).await;

    let scope = resolve_content_scope(instance_id, None, state).await?;
    let file_name = sanitize_file_name(file_name);
    let project_type = match project_type {
        Some(project_type) => project_type,
        None => infer_project_type(&bytes)?,
    };
    // Minecraft only loads resource/shader pack ZIPs whose markers sit at
    // the archive root. Archives produced by zipping a pack folder wrap it
    // in one folder; extract the pack folder(s) directly so the result is
    // usable as-is — no re-packing, no recompression.
    if let Some(plan) = wrapped_pack_plan(&bytes, project_type) {
        if provider_ref.is_some() {
            return Err(crate::ErrorKind::InputError(
                "Provider-managed wrapped archives are not supported"
                    .to_string(),
            )
            .into());
        }
        let install_path = install_wrapped_pack(
            &bytes,
            &plan,
            project_type,
            &scope.instance,
            state,
        )
        .await?;
        // Let the instance content scanner index the extracted files.
        crate::event::emit::emit_instance(
            &scope.instance.id,
            crate::event::InstancePayloadType::Synced,
        )
        .await?;
        return Ok(install_path);
    }
    let relative_path = format!("{}/{}", project_type.get_folder(), file_name);
    let full_path = join_content_path(
        &instance_full_path(state, &scope.instance),
        &relative_path,
    );
    let sha1 = match hash {
        Some(hash) => hash.to_string(),
        None => fetch::sha1_async(bytes.clone()).await?,
    };

    cache_file_hash(
        bytes.clone(),
        &scope.instance.path,
        &relative_path,
        Some(&sha1),
        Some(project_type),
        None,
        &state.pool,
    )
    .await?;
    fetch::write(&full_path, &bytes, &state.io_semaphore).await?;

    let local_mod_data = if project_type == ProjectType::Mod {
        crate::mod_metadata::extract_mod_metadata(&bytes)
            .and_then(|meta| serde_json::to_string(&meta).ok())
    } else {
        None
    };

    let file = content_rows::upsert_instance_file_from_parts(
        content_rows::UpsertInstanceFile {
            instance_id: &scope.instance.id,
            relative_path: &relative_path,
            file_name: &file_name,
            enabled: !relative_path.ends_with(".disabled"),
            sha1: &sha1,
            size: bytes.len() as u64,
            missing: false,
            local_mod_data: local_mod_data.as_deref(),
            icon_path: None,
        },
        &state.pool,
    )
    .await?;
    upsert_entry_for_file(
        &scope,
        &file,
        project_type,
        source_kind,
        ContentOwnershipKind::UserAdded,
        provider_ref,
        provider_ref.is_some(),
        state,
    )
    .await?;

    Ok(relative_path)
}

/// Install plan for a resource/shader pack ZIP wrapped in a single root
/// folder: either the wrapper folder itself is the pack, or its direct
/// children are one or more packs (a collection).
#[derive(Debug, PartialEq)]
enum WrappedPackPlan {
    /// The wrapper folder is the pack; install it as a whole folder.
    Whole { wrapper: String },
    /// Direct children of the wrapper are packs; install each one.
    Children { children: Vec<String> },
}

/// Detect whether `bytes` is a resource/data/shader pack ZIP wrapped in a
/// single top-level folder. Returns the install plan, or `None` for flat
/// archives, other content types and mixed layouts (which are installed
/// as-is).
fn wrapped_pack_plan(
    bytes: &[u8],
    project_type: ProjectType,
) -> Option<WrappedPackPlan> {
    use std::io::Cursor;

    if !matches!(
        project_type,
        ProjectType::ResourcePack
            | ProjectType::DataPack
            | ProjectType::ShaderPack
    ) {
        return None;
    }

    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).ok()?;
    let mut names: Vec<String> = Vec::with_capacity(archive.len());
    for i in 0..archive.len() {
        let entry = archive.by_index_raw(i).ok()?;
        names.push(
            crate::api::pack::detect::decode_zip_entry_name(entry.name_raw())
                .replace('\\', "/"),
        );
    }

    let has_marker_at = |prefix: &str| -> bool {
        let pack_mcmeta = format!("{prefix}pack.mcmeta");
        let shaders = format!("{prefix}shaders/");
        match project_type {
            ProjectType::ResourcePack | ProjectType::DataPack => {
                names.iter().any(|name| name == &pack_mcmeta)
            }
            ProjectType::ShaderPack => names.iter().any(|name| {
                let Some(rest) = name.strip_prefix(&shaders) else {
                    return false;
                };
                if rest.is_empty() {
                    return false;
                }
                let lower = rest.to_ascii_lowercase();
                lower.ends_with(".fsh")
                    || lower.ends_with(".vsh")
                    || lower.ends_with(".glsl")
            }),
            _ => false,
        }
    };

    // Every non-directory entry must share a single top-level folder.
    let mut wrapper: Option<&str> = None;
    for name in &names {
        if name.is_empty() || name.ends_with('/') {
            continue;
        }
        let Some((first, _)) = name.split_once('/') else {
            return None; // flat archive
        };
        match wrapper {
            None => wrapper = Some(first),
            Some(existing) if existing != first => return None,
            _ => {}
        }
    }
    let wrapper = wrapper?;
    let wrapper_prefix = format!("{wrapper}/");

    // The wrapper itself is the pack when its marker sits at its root.
    if has_marker_at(&wrapper_prefix) {
        return Some(WrappedPackPlan::Whole {
            wrapper: wrapper.to_string(),
        });
    }

    // Otherwise its direct children are packs (single pack or a collection).
    let mut children: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for name in &names {
        let Some(rest) = name.strip_prefix(&wrapper_prefix) else {
            continue;
        };
        if rest.is_empty() {
            continue;
        }
        let Some((child, _)) = rest.split_once('/') else {
            // Direct file under the wrapper: a zipped pack.
            if rest.to_lowercase().ends_with(".zip")
                && seen.insert(rest.to_string())
            {
                children.push(rest.to_string());
            }
            continue;
        };
        if seen.insert(child.to_string()) {
            let child_base = format!("{wrapper_prefix}{child}/");
            if has_marker_at(&child_base) {
                children.push(child.to_string());
            }
        }
    }
    if children.is_empty() {
        return None;
    }
    Some(WrappedPackPlan::Children { children })
}

/// Materialize a wrapped resource/data/shader pack into the instance's
/// `resourcepacks/` / `datapacks/` / `shaderpacks/` folder by extracting it
/// (no recompression). Returns the relative path of the installed pack.
async fn install_wrapped_pack(
    bytes: &Bytes,
    plan: &WrappedPackPlan,
    project_type: ProjectType,
    instance: &Instance,
    state: &State,
) -> crate::Result<String> {
    let folder = project_type.get_folder();
    let dest = instance_full_path(state, instance).join(folder);
    tokio::fs::create_dir_all(&dest).await?;

    let temp_dir = tempfile::tempdir().map_err(|e| {
        crate::ErrorKind::InputError(format!(
            "Failed to create temporary directory: {e}"
        ))
    })?;
    let zip_path = temp_dir.path().join("pack.zip");
    tokio::fs::write(&zip_path, &bytes[..]).await?;

    match plan {
        WrappedPackPlan::Whole { wrapper } => {
            // Extract straight into the pack folder: dest/<wrapper>/.
            extract_zip_to_dir_async(&zip_path, &dest).await?;
            Ok(format!("{folder}/{wrapper}"))
        }
        WrappedPackPlan::Children { children } => {
            let extracted = temp_dir.path().join("extracted");
            extract_zip_to_dir_async(&zip_path, &extracted).await?;
            let mut installed = String::new();
            for child in children {
                let src = extracted.join(child);
                let dst = dest.join(child);
                if dst.exists() {
                    continue;
                }
                move_or_copy(&src, &dst).await?;
                if installed.is_empty() {
                    installed = format!("{folder}/{child}");
                }
            }
            Ok(installed)
        }
    }
}

/// Zip-slip-safe extraction of a ZIP into a directory, run off the async
/// runtime.
async fn extract_zip_to_dir_async(
    zip_path: &Path,
    out: &Path,
) -> crate::Result<()> {
    let zip_path = zip_path.to_path_buf();
    let out = out.to_path_buf();
    tokio::task::spawn_blocking(move || {
        crate::api::drop_classifier::extract_zip_to_dir(&zip_path, &out)
    })
    .await
    .map_err(|e| {
        crate::ErrorKind::InputError(format!("Extraction task panicked: {e}"))
    })?
    .map_err(|e| crate::Error::from(crate::ErrorKind::InputError(e)))
}

/// Move a file or folder into place, falling back to a copy when the source
/// and destination live on different volumes.
async fn move_or_copy(src: &Path, dst: &Path) -> crate::Result<()> {
    match tokio::fs::rename(src, dst).await {
        Ok(()) => Ok(()),
        Err(_) => {
            if src.is_dir() {
                io::copy_dir(src, dst).await?;
            } else {
                tokio::fs::copy(src, dst).await?;
            }
            Ok(())
        }
    }
}

pub(crate) async fn record_project_file_atomic(
    instance_id: &str,
    relative_path: &str,
    sha1: &str,
    size: u64,
    project_type: ProjectType,
    source_kind: ContentSourceKind,
    ownership_kind: ContentOwnershipKind,
    provider_ref: Option<&ContentProviderRef>,
    origin: bool,
    known_modrinth_file: Option<KnownModrinthFile<'_>>,
    state: &State,
) -> crate::Result<()> {
    let record = ProjectFileRecord {
        relative_path: relative_path.to_string(),
        sha1: sha1.to_string(),
        size,
        project_type,
        source_kind,
        ownership_kind,
        provider_ref: provider_ref.cloned(),
        origin,
        known_modrinth_project_id: known_modrinth_file
            .map(|file| file.project_id.to_string()),
        known_modrinth_version_id: known_modrinth_file
            .map(|file| file.version_id.to_string()),
    };
    record_project_files_atomic_with_pending_completion(
        instance_id,
        std::slice::from_ref(&record),
        &[],
        None,
        state,
    )
    .await
}

pub(crate) async fn record_verified_curseforge_project_file_atomic(
    instance_id: &str,
    relative_path: &str,
    sha1: &str,
    size: u64,
    project_type: ProjectType,
    source_kind: ContentSourceKind,
    ownership_kind: ContentOwnershipKind,
    project_id: CurseForgeProjectId,
    file_id: CurseForgeFileId,
    origin: bool,
    state: &State,
) -> crate::Result<()> {
    let provider_ref = ContentProviderRef::CurseForge {
        project_id,
        file_id: Some(file_id),
    };
    let record = ProjectFileRecord {
        relative_path: relative_path.to_string(),
        sha1: sha1.to_string(),
        size,
        project_type,
        source_kind,
        ownership_kind,
        provider_ref: Some(provider_ref),
        origin,
        known_modrinth_project_id: None,
        known_modrinth_version_id: None,
    };
    let verified_pending = [(project_id, file_id)];
    record_project_files_atomic_with_pending_completion(
        instance_id,
        std::slice::from_ref(&record),
        &verified_pending,
        None,
        state,
    )
    .await
}

pub(crate) async fn record_project_files_with_verified_curseforge_atomic(
    instance_id: &str,
    records: &[ProjectFileRecord],
    verified_pending: &[(CurseForgeProjectId, CurseForgeFileId)],
    state: &State,
) -> crate::Result<()> {
    record_project_files_atomic_with_pending_completion(
        instance_id,
        records,
        verified_pending,
        None,
        state,
    )
    .await
}

pub(crate) async fn record_project_files_atomic_with_permit(
    instance_id: &str,
    records: &[ProjectFileRecord],
    verified_pending: &[(CurseForgeProjectId, CurseForgeFileId)],
    database_permit: Option<tokio::sync::SemaphorePermit<'_>>,
    state: &State,
) -> crate::Result<()> {
    record_project_files_atomic_with_pending_completion(
        instance_id,
        records,
        verified_pending,
        database_permit,
        state,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn record_project_files_atomic_with_pending_completion(
    instance_id: &str,
    records: &[ProjectFileRecord],
    verified_pending: &[(CurseForgeProjectId, CurseForgeFileId)],
    database_permit: Option<tokio::sync::SemaphorePermit<'_>>,
    state: &State,
) -> crate::Result<()> {
    record_project_files_transaction(
        instance_id,
        records,
        verified_pending,
        database_permit,
        None,
        state,
    )
    .await
}

async fn record_project_files_transaction(
    instance_id: &str,
    records: &[ProjectFileRecord],
    verified_pending: &[(CurseForgeProjectId, CurseForgeFileId)],
    database_permit: Option<tokio::sync::SemaphorePermit<'_>>,
    plan: Option<&ResolveContentPlan>,
    state: &State,
) -> crate::Result<()> {
    if records.is_empty() {
        return Ok(());
    }
    let verified_artifacts = crate::resources::packs::artifacts(
        state,
        records,
        &tokio_util::sync::CancellationToken::new(),
    )
    .await;
    // Always acquire the global writer before the per-instance lock. This
    // avoids holding an instance lock while waiting for a progress checkpoint
    // or another install to release the database writer.
    let _database_permit = match database_permit {
        Some(permit) => permit,
        None => state.acquire_install_db_permit().await?,
    };
    let _instance_lock = state.lock_instance_content(instance_id).await;

    let scope = resolve_content_scope(instance_id, None, state).await?;
    let mut tx = begin_content_write(&state.pool).await?;
    content_rows::ensure_content_write_parents(
        &scope.instance.id,
        &scope.content_set_id,
        &mut tx,
    )
    .await?;
    let mut hash_cache_entries = Vec::with_capacity(records.len());
    for record in records {
        let file_name = Path::new(&record.relative_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let file =
            content_rows::upsert_instance_file_from_parts_in_transaction(
                content_rows::UpsertInstanceFile {
                    instance_id: &scope.instance.id,
                    relative_path: &record.relative_path,
                    file_name: &file_name,
                    enabled: !record.relative_path.ends_with(".disabled"),
                    sha1: &record.sha1,
                    size: record.size,
                    missing: false,
                    local_mod_data: None,
                    icon_path: None,
                },
                &mut tx,
            )
            .await?;
        let entry =
            content_rows::upsert_content_entry_from_parts_in_transaction(
                content_rows::UpsertContentEntry {
                    instance_id: &scope.instance.id,
                    content_set_id: &scope.content_set_id,
                    file_id: Some(&file.id),
                    project_type: record.project_type,
                    source_kind: record.source_kind,
                    ownership_kind: record.ownership_kind,
                    auto_dependency: false,
                    server_requirement: ContentRequirement::Required,
                    client_requirement: ContentRequirement::Required,
                    enabled: file.enabled,
                },
                &mut tx,
            )
            .await?;
        if let Some(provider_ref) = record.provider_ref.as_ref() {
            content_rows::upsert_content_provider_ref_in_transaction(
                &entry.id,
                provider_ref,
                record.origin,
                &mut tx,
            )
            .await?;
        }
        if let Some(artifact) = verified_artifacts.get(&record.relative_path) {
            let parts: Vec<_> = record.relative_path.split('/').collect();
            let resource_scope = if parts.len() >= 4
                && parts[0] == "saves"
                && parts[2] == "datapacks"
            {
                crate::resources::InstallScope::World {
                    relative_path: format!("saves/{}", parts[1]),
                }
            } else {
                crate::resources::InstallScope::Instance
            };
            sqlx::query("INSERT INTO resource_installed_artifacts(entry_id, artifact, scope) VALUES (?, ?, ?) ON CONFLICT(entry_id) DO UPDATE SET artifact = excluded.artifact, scope = excluded.scope")
                .bind(&entry.id).bind(serde_json::to_string(artifact)?).bind(serde_json::to_string(&resource_scope)?).execute(&mut *tx).await?;
        } else {
            sqlx::query(
                "DELETE FROM resource_installed_artifacts WHERE entry_id = ?",
            )
            .bind(&entry.id)
            .execute(&mut *tx)
            .await?;
        }
        if record.ownership_kind == ContentOwnershipKind::PackManaged {
            let now = chrono::Utc::now();
            let member_key = record.provider_ref.as_ref().map_or_else(
                || format!("path:{}", record.relative_path.to_lowercase()),
                |provider_ref| {
                    format!(
                        "{}:{}:{}",
                        provider_ref.provider().as_str(),
                        provider_ref.database_project_id(),
                        record.project_type.get_name(),
                    )
                },
            );
            content_rows::upsert_pack_member_in_transaction(
                &PackMember {
                    id: format!("pack-member:{}", entry.id),
                    content_set_id: scope.content_set_id.clone(),
                    content_entry_id: Some(entry.id.clone()),
                    member_key,
                    project_type: record.project_type,
                    expected_relative_path: record.relative_path.clone(),
                    provider: record
                        .provider_ref
                        .as_ref()
                        .map(ContentProviderRef::provider),
                    provider_project_id: record
                        .provider_ref
                        .as_ref()
                        .map(ContentProviderRef::database_project_id),
                    provider_release_id: record
                        .provider_ref
                        .as_ref()
                        .and_then(ContentProviderRef::database_release_id),
                    required: true,
                    expected_sha1: Some(record.sha1.clone()),
                    expected_size: Some(record.size),
                    expected_fingerprint: None,
                    materialization_state:
                        PackMemberMaterializationState::Present,
                    override_kind: if !file.enabled {
                        PackMemberOverrideKind::Disabled
                    } else if record.source_kind == ContentSourceKind::Local {
                        PackMemberOverrideKind::Version
                    } else {
                        PackMemberOverrideKind::None
                    },
                    reconciled: true,
                    created_at: now,
                    modified_at: now,
                },
                &mut tx,
            )
            .await?;
        }
        if let Some((project_id, file_id)) =
            record.provider_ref.as_ref().and_then(|provider_ref| {
                let ContentProviderRef::CurseForge {
                    project_id,
                    file_id: Some(file_id),
                } = provider_ref
                else {
                    return None;
                };
                verified_pending
                    .contains(&(*project_id, *file_id))
                    .then_some((*project_id, *file_id))
            })
        {
            content_rows::complete_pending_manual_download(
                instance_id,
                &project_id.get().to_string(),
                &file_id.get().to_string(),
                Some(&entry.id),
                &mut tx,
            )
            .await?;
        }
        hash_cache_entries.push(
            CacheValue::FileHash(CachedFileHash {
                path: format!(
                    "{}/{}",
                    scope.instance.path, record.relative_path
                ),
                size: record.size,
                hash: record.sha1.clone(),
                project_type: Some(record.project_type),
                project_id: record.known_modrinth_project_id.clone(),
                version_id: record.known_modrinth_version_id.clone(),
            })
            .get_entry(),
        );
    }
    CachedEntry::upsert_many(&hash_cache_entries, &mut *tx).await?;
    if let Some(plan) = plan {
        let paths = records
            .iter()
            .map(|record| record.relative_path.clone())
            .collect::<Vec<_>>();
        record_resolved_plan_edges(
            &scope.content_set_id,
            &paths,
            plan,
            &mut tx,
        )
        .await?;
    }
    content_rows::bump_content_set_revision_in_transaction(
        &scope.content_set_id,
        &mut tx,
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

async fn record_resolved_plan_edges(
    content_set_id: &str,
    paths: &[String],
    plan: &ResolveContentPlan,
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
) -> crate::Result<()> {
    let planned = std::iter::once(&plan.primary)
        .chain(plan.dependencies.iter())
        .collect::<Vec<_>>();
    let mut ids = Vec::with_capacity(paths.len());
    for path in paths {
        ids.push(sqlx::query_scalar::<_, String>("SELECT entry.id FROM instance_content_entries entry JOIN instance_files file ON file.id = entry.file_id WHERE entry.content_set_id = ? AND file.relative_path = ? ORDER BY entry.modified_at DESC LIMIT 1")
			.bind(content_set_id).bind(path).fetch_one(&mut **tx).await?);
    }
    for id in ids.iter().skip(1) {
        content_rows::set_content_entry_auto_dependency_in_transaction(
            id, true, tx,
        )
        .await?;
    }
    let mut edges = Vec::new();
    for (index, dependency) in plan.dependencies.iter().enumerate() {
        if let Some(parent) = dependency
            .dependent_on_version_id
            .as_deref()
            .and_then(|version| {
                planned.iter().position(|item| item.version_id == version)
            })
        {
            edges.push((
                parent,
                ids[index + 1].clone(),
                dependency.project_id.clone(),
                dependency.version_id.clone(),
            ));
        }
    }
    for skipped in &plan.skipped {
        if skipped.reason != SkippedReason::AlreadyInstalled {
            continue;
        }
        let Some(parent) =
            skipped
                .dependent_on_version_id
                .as_deref()
                .and_then(|version| {
                    planned.iter().position(|item| item.version_id == version)
                })
        else {
            continue;
        };
        let Some(version) = skipped.version_id.as_deref() else {
            continue;
        };
        if let Some(child) = sqlx::query_scalar::<_, String>("SELECT entry.id FROM instance_content_entries entry JOIN instance_content_provider_refs ref ON ref.content_entry_id = entry.id WHERE entry.content_set_id = ? AND ref.provider = 'modrinth' AND ref.provider_project_id = ? AND ref.provider_release_id = ? ORDER BY entry.modified_at DESC LIMIT 1")
			.bind(content_set_id).bind(&skipped.project_id).bind(version).fetch_optional(&mut **tx).await? {
			edges.push((parent, child, skipped.project_id.clone(), version.to_string()));
		}
    }
    for (parent, child, project, version) in edges {
        let now = chrono::Utc::now();
        content_rows::upsert_content_dependency_edge_in_transaction(
            &crate::state::instances::ContentDependencyEdge {
                id: format!("content-dependency:{}", uuid::Uuid::new_v4()),
                content_set_id: content_set_id.to_string(),
                parent_entry_id: ids[parent].clone(),
                child_entry_id: child,
                evidence_provider: ContentProvider::Modrinth,
                parent_provider: ContentProvider::Modrinth,
                child_provider: ContentProvider::Modrinth,
                dependency_kind:
                    crate::state::instances::ContentDependencyKind::Required,
                parent_project_id: planned[parent].project_id.clone(),
                parent_release_id: planned[parent].version_id.clone(),
                child_project_id: project,
                child_release_id: version,
                created_at: now,
                modified_at: now,
            },
            tx,
        )
        .await?;
    }
    Ok(())
}

async fn begin_content_write(
    pool: &sqlx::SqlitePool,
) -> crate::Result<sqlx::Transaction<'static, sqlx::Sqlite>> {
    Ok(pool.begin_with("BEGIN IMMEDIATE").await?)
}

pub(crate) async fn toggle_disable_project(
    instance_id: &str,
    project_path: &str,
    desired_enabled: Option<bool>,
    state: &State,
) -> crate::Result<String> {
    let _instance_lock = state.lock_instance_content(instance_id).await;
    let scope = resolve_content_scope(instance_id, None, state).await?;
    let base = instance_full_path(state, &scope.instance);
    let (current_path, enabled, new_path) =
        resolve_toggle_paths(&base, project_path, desired_enabled)?;

    if current_path != new_path {
        io::rename_or_move(
            &join_content_path(&base, &current_path),
            &join_content_path(&base, &new_path),
        )
        .await?;
    }

    let file = rename_indexed_file(
        &scope,
        project_path,
        &current_path,
        &new_path,
        enabled,
        state,
    )
    .await?;
    let mut tx = begin_content_write(&state.pool).await?;
    let modified_at = chrono::Utc::now().timestamp();
    let updated_entry = sqlx::query(
        "UPDATE instance_content_entries
         SET enabled = ?, modified_at = ?
         WHERE content_set_id = ? AND file_id = ?",
    )
    .bind(i64::from(enabled))
    .bind(modified_at)
    .bind(&scope.content_set_id)
    .bind(&file.id)
    .execute(&mut *tx)
    .await?;
    if updated_entry.rows_affected() == 0 {
        let project_type = project_type_from_relative_path(&new_path)
            .ok_or_else(|| {
                crate::ErrorKind::InputError(format!(
                    "Unable to infer project type from {new_path}"
                ))
            })?;
        content_rows::upsert_content_entry_from_parts_in_transaction(
            content_rows::UpsertContentEntry {
                instance_id: &scope.instance.id,
                content_set_id: &scope.content_set_id,
                file_id: Some(&file.id),
                project_type,
                source_kind: ContentSourceKind::Local,
                ownership_kind: ContentOwnershipKind::UserAdded,
                auto_dependency: false,
                server_requirement: ContentRequirement::Required,
                client_requirement: ContentRequirement::Required,
                enabled,
            },
            &mut tx,
        )
        .await?;
    }
    sqlx::query(
        "UPDATE instance_pack_members
         SET override_kind = ?, materialization_state = 'present',
             modified_at = ?
         WHERE content_entry_id IN (
            SELECT id FROM instance_content_entries
            WHERE content_set_id = ? AND file_id = ?
         )",
    )
    .bind(if enabled { "none" } else { "disabled" })
    .bind(modified_at)
    .bind(&scope.content_set_id)
    .bind(&file.id)
    .execute(&mut *tx)
    .await?;
    content_rows::bump_content_set_revision_in_transaction(
        &scope.content_set_id,
        &mut tx,
    )
    .await?;
    tx.commit().await?;

    Ok(new_path)
}

pub(crate) async fn toggle_content_entries(
    instance_id: &str,
    content_ids: &[String],
    desired_enabled: Option<bool>,
    state: &State,
) -> crate::Result<Vec<ToggledContentEntry>> {
    if content_ids.is_empty() {
        return Ok(Vec::new());
    }

    let _instance_lock = state
        .lock_instance_content_with_timeout(
            instance_id,
            std::time::Duration::from_secs(20),
        )
        .await?;
    let scope = resolve_content_scope(instance_id, None, state).await?;
    let base = instance_full_path(state, &scope.instance);
    let mut seen_ids = std::collections::HashSet::new();
    let mut seen_paths = std::collections::HashSet::new();
    let mut operations = Vec::new();

    for content_id in content_ids {
        if !seen_ids.insert(content_id.as_str()) {
            continue;
        }
        let target = content_rows::get_content_mutation_target(
            instance_id,
            content_id,
            &state.pool,
        )
        .await?
        .ok_or_else(|| {
            crate::ErrorKind::InputError(
                "The selected content no longer exists".to_string(),
            )
        })?;
        let project_path = target.relative_path.ok_or_else(|| {
            crate::ErrorKind::InputError(
                "The selected content is not present on disk".to_string(),
            )
        })?;
        let (current_path, enabled, new_path) =
            resolve_toggle_paths(&base, &project_path, desired_enabled)?;

        if !seen_paths.insert(current_path.clone()) {
            continue;
        }
        operations.push(ContentToggleOperation {
            content_id: content_id.clone(),
            current_path,
            new_path,
            enabled,
            project_type: target.project_type,
        });
    }

    let indexed_paths =
        content_rows::get_instance_files(&scope.instance.id, &state.pool)
            .await?
            .into_iter()
            .map(|file| file.relative_path)
            .collect::<std::collections::HashSet<_>>();
    for operation in &operations {
        if !indexed_paths.contains(&operation.current_path) {
            return Err(crate::ErrorKind::InputError(format!(
                "The selected content is no longer indexed: {}",
                operation.current_path
            ))
            .into());
        }
    }

    let mut destinations = std::collections::HashSet::new();
    for operation in &operations {
        if operation.current_path != operation.new_path
            && !destinations.insert(operation.new_path.as_str())
        {
            return Err(crate::ErrorKind::InputError(
                "The selected content has conflicting destination paths"
                    .to_string(),
            )
            .into());
        }
    }

    let mut renamed: Vec<&ContentToggleOperation> = Vec::new();
    for operation in &operations {
        if operation.current_path == operation.new_path {
            continue;
        }
        if let Err(error) = io::rename_or_move(
            &join_content_path(&base, &operation.current_path),
            &join_content_path(&base, &operation.new_path),
        )
        .await
        {
            for previous in renamed.into_iter().rev() {
                let _ = io::rename_or_move(
                    &join_content_path(&base, &previous.new_path),
                    &join_content_path(&base, &previous.current_path),
                )
                .await;
            }
            return Err(error.into());
        }
        renamed.push(operation);
    }

    let database_result = async {
        let mut tx = begin_content_write(&state.pool).await?;
        let modified_at = chrono::Utc::now().timestamp();
        let mut results = Vec::with_capacity(operations.len());

        for operation in &operations {
            let file_name = Path::new(&operation.new_path)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let file = content_rows::rename_instance_file_in_transaction(
                &scope.instance.id,
                &operation.current_path,
                &operation.new_path,
                &file_name,
                operation.enabled,
                &mut tx,
            )
            .await?
            .ok_or_else(|| {
                crate::ErrorKind::InputError(format!(
                    "The selected content is no longer indexed: {}",
                    operation.current_path
                ))
            })?;
            let updated_entry = sqlx::query(
                "UPDATE instance_content_entries
                 SET enabled = ?, modified_at = ?
                 WHERE content_set_id = ? AND file_id = ?",
            )
            .bind(i64::from(operation.enabled))
            .bind(modified_at)
            .bind(&scope.content_set_id)
            .bind(&file.id)
            .execute(&mut *tx)
            .await?;
            if updated_entry.rows_affected() == 0 {
                content_rows::upsert_content_entry_from_parts_in_transaction(
                    content_rows::UpsertContentEntry {
                        instance_id: &scope.instance.id,
                        content_set_id: &scope.content_set_id,
                        file_id: Some(&file.id),
                        project_type: operation.project_type,
                        source_kind: ContentSourceKind::Local,
                        ownership_kind: ContentOwnershipKind::UserAdded,
                        auto_dependency: false,
                        server_requirement: ContentRequirement::Required,
                        client_requirement: ContentRequirement::Required,
                        enabled: operation.enabled,
                    },
                    &mut tx,
                )
                .await?;
            }
            sqlx::query(
                "UPDATE instance_pack_members
                 SET override_kind = ?, materialization_state = 'present',
                     modified_at = ?
                 WHERE content_entry_id IN (
                    SELECT id FROM instance_content_entries
                    WHERE content_set_id = ? AND file_id = ?
                 )",
            )
            .bind(if operation.enabled {
                "none"
            } else {
                "disabled"
            })
            .bind(modified_at)
            .bind(&scope.content_set_id)
            .bind(&file.id)
            .execute(&mut *tx)
            .await?;
            results.push(ToggledContentEntry {
                content_id: operation.content_id.clone(),
                path: operation.new_path.clone(),
                enabled: operation.enabled,
            });
        }

        content_rows::bump_content_set_revision_in_transaction(
            &scope.content_set_id,
            &mut tx,
        )
        .await?;
        tx.commit().await?;
        Ok(results)
    }
    .await;

    match database_result {
        Ok(results) => Ok(results),
        Err(error) => {
            for operation in renamed.into_iter().rev() {
                let _ = io::rename_or_move(
                    &join_content_path(&base, &operation.new_path),
                    &join_content_path(&base, &operation.current_path),
                )
                .await;
            }
            Err(error)
        }
    }
}

/// Resolves which of `project_path` / `{trimmed}.disabled` currently exists
/// and which path the toggle should end up at.
fn resolve_toggle_paths(
    base: &Path,
    project_path: &str,
    desired_enabled: Option<bool>,
) -> crate::Result<(String, bool, String)> {
    let trimmed = project_path.trim_end_matches(".disabled");
    let current_path = if join_content_path(base, project_path).exists() {
        project_path.to_string()
    } else if join_content_path(base, &format!("{trimmed}.disabled")).exists() {
        format!("{trimmed}.disabled")
    } else if join_content_path(base, trimmed).exists() {
        trimmed.to_string()
    } else {
        return Err(crate::ErrorKind::FSError(format!(
            "Could not find project file for '{project_path}' in instance"
        ))
        .into());
    };
    let current_enabled = !current_path.ends_with(".disabled");
    let enabled = desired_enabled.unwrap_or(!current_enabled);
    let new_path = if enabled {
        trimmed.to_string()
    } else {
        format!("{trimmed}.disabled")
    };
    // An enabled and a disabled copy of the same name can coexist on disk, for
    // example an imported file next to a disabled sibling. Renaming onto that
    // sibling would replace it, so refuse before anything is moved.
    if new_path != current_path && join_content_path(base, &new_path).exists() {
        return Err(crate::ErrorKind::InputError(format!(
            "Cannot {} '{project_path}' because '{new_path}' already exists",
            if enabled { "enable" } else { "disable" }
        ))
        .into());
    }
    Ok((current_path, enabled, new_path))
}

/// Renames the instance-file DB row to match the new on-disk path, falling
/// back to indexing the file when no row matches either name.
async fn rename_indexed_file(
    scope: &ContentScope,
    project_path: &str,
    current_path: &str,
    new_path: &str,
    enabled: bool,
    state: &State,
) -> crate::Result<InstanceFile> {
    let file_name = Path::new(&new_path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let file = match content_rows::rename_instance_file(
        &scope.instance.id,
        current_path,
        new_path,
        &file_name,
        enabled,
        &state.pool,
    )
    .await?
    {
        Some(file) => file,
        None if current_path != project_path => {
            match content_rows::rename_instance_file(
                &scope.instance.id,
                project_path,
                new_path,
                &file_name,
                enabled,
                &state.pool,
            )
            .await?
            {
                Some(file) => file,
                None => index_existing_file(scope, new_path, state).await?,
            }
        }
        None => index_existing_file(scope, new_path, state).await?,
    };
    Ok(file)
}

pub(crate) async fn remove_project(
    instance_id: &str,
    project_path: &str,
    state: &State,
) -> crate::Result<()> {
    let _instance_lock = state.lock_instance_content(instance_id).await;

    let scope = resolve_content_scope(instance_id, None, state).await?;
    let base = instance_full_path(state, &scope.instance);
    let file = content_rows::get_instance_file_by_relative_path(
        &scope.instance.id,
        project_path,
        &state.pool,
    )
    .await?;

    let full_path = join_content_path(&base, project_path);
    let staged_path =
        full_path.with_extension(format!("{}.removing", uuid::Uuid::new_v4()));
    if full_path.exists() {
        io::rename_or_move(&full_path, &staged_path).await?;
    }

    let db_result = async {
        let mut tx = begin_content_write(&state.pool).await?;
        if let Some(file) = file {
            let modified_at = chrono::Utc::now().timestamp();
            sqlx::query(
                "UPDATE instance_pack_members
                 SET content_entry_id = NULL,
                     materialization_state = 'removed',
                     override_kind = 'removed',
                     modified_at = ?
                 WHERE content_entry_id IN (
                    SELECT id FROM instance_content_entries
                    WHERE content_set_id = ? AND file_id = ?
                 )",
            )
            .bind(modified_at)
            .bind(&scope.content_set_id)
            .bind(&file.id)
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                "DELETE FROM instance_content_entries
                 WHERE content_set_id = ? AND file_id = ?",
            )
            .bind(&scope.content_set_id)
            .bind(&file.id)
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                "DELETE FROM instance_files
                 WHERE instance_id = ? AND id = ?",
            )
            .bind(&scope.instance.id)
            .bind(&file.id)
            .execute(&mut *tx)
            .await?;
        }
        content_rows::bump_content_set_revision_in_transaction(
            &scope.content_set_id,
            &mut tx,
        )
        .await?;
        tx.commit().await?;
        Ok::<(), crate::Error>(())
    }
    .await;
    if let Err(error) = db_result {
        if staged_path.exists() {
            io::rename_or_move(&staged_path, &full_path).await?;
        }
        return Err(error);
    }
    if staged_path.exists() {
        io::remove_file(staged_path).await?;
    }

    Ok(())
}

pub(crate) fn backup_relative_path_for_update(
    old_path: &str,
    new_path: &str,
) -> Option<String> {
    let old_name = Path::new(old_path).file_name()?.to_str()?;
    let new_name = Path::new(new_path).file_name()?.to_str()?;
    let old_base = old_name.trim_end_matches(".disabled");
    let new_base = new_name.trim_end_matches(".disabled");
    if old_base.is_empty() || new_base.is_empty() || old_base == new_base {
        return None;
    }
    let backup_name = format!("{new_base}_{old_base}.old");
    let directory = Path::new(old_path).parent()?.to_str()?;
    Some(format!("{directory}/{backup_name}"))
}
pub(crate) async fn archive_project_file(
    instance_id: &str,
    old_path: &str,
    new_path: &str,
    state: &State,
) -> crate::Result<Option<String>> {
    let Some(backup_relative_path) =
        backup_relative_path_for_update(old_path, new_path)
    else {
        return Ok(None);
    };
    let _instance_lock = state.lock_instance_content(instance_id).await;

    let scope = resolve_content_scope(instance_id, None, state).await?;
    let base = instance_full_path(state, &scope.instance);
    let full_old = join_content_path(&base, old_path);
    if !full_old.exists() {
        return Ok(None);
    }
    let full_backup = join_content_path(&base, &backup_relative_path);
    if full_backup.exists() {
        return Err(crate::ErrorKind::InputError(
            "Existing backup must be reviewed before it can be replaced".into(),
        )
        .into());
    }
    io::rename_or_move(&full_old, &full_backup).await?;
    touch_file_modified_time(&full_backup);

    if let Some(file) = content_rows::get_instance_file_by_relative_path(
        &scope.instance.id,
        old_path,
        &state.pool,
    )
    .await?
    {
        content_rows::remove_content_entries_for_file(
            &scope.content_set_id,
            &file.id,
            &state.pool,
        )
        .await?;
        content_rows::remove_instance_file_by_relative_path(
            &scope.instance.id,
            old_path,
            &state.pool,
        )
        .await?;
    }

    Ok(Some(backup_relative_path))
}

pub(crate) async fn rollback_project(
    instance_id: &str,
    project_path: &str,
    state: &std::sync::Arc<State>,
) -> crate::Result<String> {
    let instance_id = instance_id.to_owned();
    let project_path = project_path.to_owned();
    let state = state.clone();
    tokio::spawn(async move {
        rollback_project_inner(&instance_id, &project_path, &state).await
    })
    .await?
}

async fn rollback_project_inner(
    instance_id: &str,
    project_path: &str,
    state: &State,
) -> crate::Result<String> {
    let _writer = state.acquire_install_db_permit().await?;
    let _instance_lock =
        state.lock_instance_content_exclusive(instance_id).await;

    let scope = resolve_content_scope(instance_id, None, state).await?;
    let base = instance_content_root(&state.directories, &scope.instance)?;
    recover_legacy_rollback_for_instance(instance_id, &base, state).await?;
    content_rows::get_instance_file_by_relative_path(
        &scope.instance.id,
        project_path,
        &state.pool,
    )
    .await?
    .ok_or_else(|| {
        crate::ErrorKind::InputError(format!(
            "Project file '{project_path}' not found"
        ))
    })?;

    let active_name = Path::new(project_path)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let active_base = active_name.trim_end_matches(".disabled");
    let active_dir = Path::new(project_path)
        .parent()
        .map(|parent| parent.to_string_lossy().into_owned())
        .unwrap_or_default();
    let prefix = format!("{active_base}_");
    let folder = base.join(&active_dir);
    let backups = scan_folder_backups(&folder, &prefix)?;
    let Some((backup_name, _)) = backups.into_iter().max_by(|left, right| {
        left.1.cmp(&right.1).then_with(|| left.0.cmp(&right.0))
    }) else {
        return Err(crate::ErrorKind::InputError(format!(
            "No backup found for '{project_path}'"
        ))
        .into());
    };
    let old_base = backup_name
        .strip_prefix(&prefix)
        .and_then(|rest| rest.strip_suffix(".old"))
        .ok_or_else(|| {
            crate::ErrorKind::InputError(format!(
                "Invalid backup name '{backup_name}'"
            ))
        })?;
    if old_base.is_empty() || old_base == active_base {
        return Err(crate::ErrorKind::InputError(format!(
            "Invalid backup name '{backup_name}'"
        ))
        .into());
    }

    let disabled = active_name.ends_with(".disabled");
    let full_active =
        crate::resources::execution::checked_path(&base, project_path)?;
    let full_backup = crate::resources::execution::checked_path(
        &base,
        &format!("{active_dir}/{backup_name}"),
    )?;
    let restored_name = old_base.to_string();
    let restored_relative_path = format!("{active_dir}/{restored_name}");
    let full_restored = crate::resources::execution::checked_path(
        &base,
        &restored_relative_path,
    )?;
    let archive_name = format!("{old_base}_{active_base}.old");
    let full_archive = crate::resources::execution::checked_path(
        &base,
        &format!("{active_dir}/{archive_name}"),
    )?;
    let final_path = if disabled {
        format!("{restored_relative_path}.disabled")
    } else {
        restored_relative_path.clone()
    };
    let full_final =
        crate::resources::execution::checked_path(&base, &final_path)?;
    if full_restored.exists() || full_final.exists() {
        return Err(crate::ErrorKind::InputError(
            "Rollback destination already exists".into(),
        )
        .into());
    }

    let (restored_size, restored_sha1) =
        fetch::sha1_file_async(&full_backup).await?;

    if full_archive.exists() {
        return Err(crate::ErrorKind::InputError("Existing rollback archive must be reviewed before it can be replaced".into()).into());
    }
    let (active_size, active_sha1) =
        fetch::sha1_file_async(&full_active).await?;
    let journal = LegacyRollbackJournal {
        active: project_path.into(),
        backup: format!("{active_dir}/{backup_name}"),
        restored: restored_relative_path.clone(),
        final_path: final_path.clone(),
        archive: format!("{active_dir}/{archive_name}"),
        active_size,
        active_sha1,
        backup_size: restored_size,
        backup_sha1: restored_sha1.clone(),
    };
    sqlx::query("INSERT INTO resource_legacy_rollback_journals(instance_id, payload) VALUES (?, ?)").bind(instance_id).bind(serde_json::to_string(&journal)?).execute(&state.pool).await?;
    let _critical = crate::install::critical_section::enter();
    let result: crate::Result<String> = async {
    io::rename_or_move(&full_active, &full_archive).await?;
    touch_file_modified_time(&full_archive);
    io::rename_or_move(&full_backup, &full_restored).await?;

    let final_relative_path = if disabled {
        let disabled_path = format!("{restored_relative_path}.disabled");
        io::rename_or_move(
            &full_restored,
            &join_content_path(&base, &disabled_path),
        )
        .await?;
        disabled_path
    } else {
        restored_relative_path
    };
    let final_name = Path::new(&final_relative_path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let local_mod_data = if project_type_for_file_path(&final_relative_path)
        == Some(ProjectType::Mod)
    {
        tokio::fs::read(join_content_path(&base, &final_relative_path))
            .await
            .ok()
            .and_then(|data| {
                crate::mod_metadata::extract_mod_metadata(&bytes::Bytes::from(
                    data,
                ))
                .and_then(|meta| serde_json::to_string(&meta).ok())
            })
    } else {
        None
    };

    let mut tx = state.pool.begin().await?;
    sqlx::query("UPDATE instance_files SET relative_path = ?, file_name = ?, enabled = ?, local_mod_data = NULL, icon_path = NULL WHERE instance_id = ? AND relative_path = ?").bind(&final_relative_path).bind(&final_name).bind(!disabled).bind(&scope.instance.id).bind(project_path).execute(&mut *tx).await?;
    content_rows::upsert_instance_file_from_parts_in_transaction(
        content_rows::UpsertInstanceFile {
            instance_id: &scope.instance.id,
            relative_path: &final_relative_path,
            file_name: &final_name,
            enabled: !disabled,
            sha1: &restored_sha1,
            size: restored_size,
            missing: false,
            local_mod_data: local_mod_data.as_deref(),
            icon_path: None,
        },
        &mut tx,
    )
    .await?;

    let entry_id: Option<String> = sqlx::query_scalar("SELECT entry.id FROM instance_content_entries entry JOIN instance_files file ON file.id = entry.file_id WHERE entry.content_set_id = ? AND file.instance_id = ? AND file.relative_path = ?")
        .bind(&scope.content_set_id).bind(&scope.instance.id).bind(&final_relative_path).fetch_optional(&mut *tx).await?;
    if let Some(entry_id) = entry_id {
        sqlx::query("DELETE FROM instance_content_provider_refs WHERE content_entry_id = ?").bind(&entry_id).execute(&mut *tx).await?;
        sqlx::query(
            "DELETE FROM resource_installed_artifacts WHERE entry_id = ?",
        )
        .bind(&entry_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query("DELETE FROM resource_entry_graphs WHERE entry_id = ?")
            .bind(&entry_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM instance_content_dependencies WHERE parent_entry_id = ?").bind(&entry_id).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM instance_content_update_checks WHERE content_entry_id = ?").bind(&entry_id).execute(&mut *tx).await?;
    }
    sqlx::query("DELETE FROM store_instance_files WHERE instance_id = ? AND relative_path IN (?, ?)").bind(instance_id).bind(project_path).bind(&final_relative_path).execute(&mut *tx).await?;
    let hash_cache = CacheValue::FileHash(CachedFileHash {
        path: format!("{}/{}", scope.instance.path, final_relative_path),
        size: restored_size,
        hash: restored_sha1,
        project_type: project_type_for_file_path(&final_relative_path),
        project_id: None,
        version_id: None,
    })
    .get_entry();
    CachedEntry::upsert_many(&[hash_cache], &mut *tx).await?;
    content_rows::bump_content_set_revision_in_transaction(
        &scope.content_set_id,
        &mut tx,
    )
    .await?;
    sqlx::query("DELETE FROM resource_legacy_rollback_journals WHERE instance_id = ?").bind(instance_id).execute(&mut *tx).await?;
    tx.commit().await?;

    Ok(final_relative_path)
    }.await;
    if result.is_err() {
        recover_legacy_rollback_for_instance(instance_id, &base, state).await?;
    }
    result
}

#[derive(serde::Serialize, serde::Deserialize)]
struct LegacyRollbackJournal {
    active: String,
    backup: String,
    restored: String,
    final_path: String,
    archive: String,
    active_size: u64,
    active_sha1: String,
    backup_size: u64,
    backup_sha1: String,
}

async fn recover_legacy_rollback_for_instance(
    instance_id: &str,
    root: &Path,
    state: &State,
) -> crate::Result<()> {
    let payload: Option<String> = sqlx::query_scalar("SELECT payload FROM resource_legacy_rollback_journals WHERE instance_id = ?").bind(instance_id).fetch_optional(&state.pool).await?;
    let Some(payload) = payload else {
        return Ok(());
    };
    let journal: LegacyRollbackJournal = serde_json::from_str(&payload)?;
    let path = |relative: &str| {
        crate::resources::execution::checked_path(root, relative)
    };
    let active = path(&journal.active)?;
    let backup = path(&journal.backup)?;
    let restored = path(&journal.restored)?;
    let final_path = path(&journal.final_path)?;
    let archive = path(&journal.archive)?;
    let matches = |size, hash: &str| crate::resources::ArtifactIntegrity {
        size: Some(size),
        hashes: vec![crate::resources::ContentHash {
            algorithm: crate::resources::HashAlgorithm::Sha1,
            value: hash.into(),
        }],
    };
    let _critical = crate::install::critical_section::enter();
    if !backup.exists() {
        let candidate = if final_path.exists() {
            &final_path
        } else {
            &restored
        };
        if !crate::resources::execution::matches_file(
            candidate,
            &matches(journal.backup_size, &journal.backup_sha1),
        )
        .await?
        {
            return Err(crate::ErrorKind::InputError(
                "Rollback backup could not be recovered".into(),
            )
            .into());
        }
        io::rename_or_move(candidate, &backup).await?;
    } else if !crate::resources::execution::matches_file(
        &backup,
        &matches(journal.backup_size, &journal.backup_sha1),
    )
    .await?
    {
        return Err(crate::ErrorKind::InputError(
            "Rollback backup changed during recovery".into(),
        )
        .into());
    }
    if !active.exists() {
        if !crate::resources::execution::matches_file(
            &archive,
            &matches(journal.active_size, &journal.active_sha1),
        )
        .await?
        {
            return Err(crate::ErrorKind::InputError(
                "Rollback active file could not be recovered".into(),
            )
            .into());
        }
        io::rename_or_move(&archive, &active).await?;
    } else if !crate::resources::execution::matches_file(
        &active,
        &matches(journal.active_size, &journal.active_sha1),
    )
    .await?
    {
        return Err(crate::ErrorKind::InputError(
            "Rollback active file changed during recovery".into(),
        )
        .into());
    }
    sqlx::query("DELETE FROM resource_legacy_rollback_journals WHERE instance_id = ? AND payload = ?").bind(instance_id).bind(payload).execute(&state.pool).await?;
    Ok(())
}

pub(crate) async fn recover_legacy_rollbacks(
    state: &State,
) -> crate::Result<()> {
    let instances: Vec<String> = sqlx::query_scalar(
        "SELECT instance_id FROM resource_legacy_rollback_journals",
    )
    .fetch_all(&state.pool)
    .await?;
    for instance_id in instances {
        let _writer = state.acquire_install_db_permit().await?;
        let _lock = state.lock_instance_content_exclusive(&instance_id).await;
        let scope = resolve_content_scope(&instance_id, None, state).await?;
        let root = instance_content_root(&state.directories, &scope.instance)?;
        recover_legacy_rollback_for_instance(&instance_id, &root, state)
            .await?;
    }
    Ok(())
}

fn scan_folder_backups(
    folder: &Path,
    prefix: &str,
) -> crate::Result<Vec<(String, i64)>> {
    let mut backups = Vec::new();
    for entry in std::fs::read_dir(folder)
        .map_err(|err| io::IOError::with_path(err, folder))?
    {
        let path = entry.map_err(io::IOError::from)?.path();
        if !path.is_file() {
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|value| value.to_str())
        else {
            continue;
        };
        if !file_name.starts_with(prefix) || !file_name.ends_with(".old") {
            continue;
        }
        let modified = path
            .metadata()
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs() as i64)
            .unwrap_or_default();
        backups.push((file_name.to_string(), modified));
    }
    Ok(backups)
}

fn touch_file_modified_time(path: &Path) {
    let Ok(file) = std::fs::File::options().write(true).open(path) else {
        return;
    };
    let _ = file.set_modified(std::time::SystemTime::now());
}

fn project_type_for_file_path(relative_path: &str) -> Option<ProjectType> {
    crate::state::instances::adapters::filesystem::project_type_from_relative_path(
        relative_path,
    )
}

pub(crate) async fn list_project_files(
    instance_id: &str,
    state: &State,
) -> crate::Result<Vec<InstalledContentFile>> {
    let scope = resolve_content_scope(instance_id, None, state).await?;
    let entries =
        content_rows::get_content_entries(&scope.content_set_id, &state.pool)
            .await?;
    let files =
        content_rows::get_instance_files(&scope.instance.id, &state.pool)
            .await?
            .into_iter()
            .map(|file| (file.id.clone(), file))
            .collect::<std::collections::HashMap<_, _>>();

    let mut output = Vec::new();
    for entry in entries {
        let Some(file_id) = entry.file_id.as_deref() else {
            continue;
        };
        let Some(file) = files.get(file_id) else {
            continue;
        };
        output.push(InstalledContentFile {
            relative_path: file.relative_path.clone(),
            provider_refs: content_rows::get_content_provider_refs(
                &entry.id,
                &state.pool,
            )
            .await?,
            enabled: entry.enabled && file.enabled,
        });
    }

    Ok(output)
}

pub(crate) fn instance_full_path(
    state: &State,
    instance: &Instance,
) -> PathBuf {
    crate::launcher::linked_game_dir(instance)
        .or_else(|| instance.linked_dot_minecraft.as_deref().map(PathBuf::from))
        .unwrap_or_else(|| state.directories.instance_game_dir(instance))
}

async fn index_existing_file(
    scope: &ContentScope,
    relative_path: &str,
    state: &State,
) -> crate::Result<InstanceFile> {
    let full_path = join_content_path(
        &instance_full_path(state, &scope.instance),
        relative_path,
    );
    // Reuse the size-keyed hash cache (same key format the content scanner
    // uses) before hashing from disk: batch enable/disable of many untracked
    // files otherwise re-reads every file, which is both slow and CPU-heavy.
    let size = tokio::fs::metadata(&full_path)
        .await
        .map_err(|e| io::IOError::with_path(e, &full_path))?
        .len();
    let cache_key = format!("{size}-{}/{}", scope.instance.path, relative_path);
    let (size, sha1) = match CachedEntry::get_file_hash_many(
        &[&cache_key],
        None,
        &state.pool,
        &state.api_semaphore,
    )
    .await?
    .into_iter()
    .next()
    {
        Some(cached) => (cached.size, cached.hash),
        None => fetch::sha1_file_async(&full_path).await?,
    };
    let file_name = Path::new(relative_path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let project_type = project_type_from_relative_path(relative_path)
        .ok_or_else(|| {
            crate::ErrorKind::InputError(format!(
                "Unable to infer project type from {relative_path}"
            ))
        })?;

    let local_mod_data = if project_type == ProjectType::Mod {
        // Read the file bytes, extract mod metadata
        match tokio::fs::read(&full_path).await {
            Ok(data) => {
                let bytes = bytes::Bytes::from(data);
                crate::mod_metadata::extract_mod_metadata(&bytes)
                    .and_then(|meta| serde_json::to_string(&meta).ok())
            }
            Err(_) => None,
        }
    } else {
        None
    };

    // The upserts below insert foreign-keyed rows. Take the instance lock and
    // re-resolve the scope so a concurrent instance deletion fails with a
    // clean error instead of a raw foreign-key violation; hashing above stays
    // unlocked so parallel batch operations are not serialized by it.
    let _instance_lock = state.lock_instance_content(&scope.instance.id).await;
    let scope = resolve_content_scope(&scope.instance.id, None, state).await?;

    let file = content_rows::upsert_instance_file_from_parts(
        content_rows::UpsertInstanceFile {
            instance_id: &scope.instance.id,
            relative_path,
            file_name: &file_name,
            enabled: !relative_path.ends_with(".disabled"),
            sha1: &sha1,
            size,
            missing: false,
            local_mod_data: local_mod_data.as_deref(),
            icon_path: None,
        },
        &state.pool,
    )
    .await?;
    upsert_entry_for_file(
        &scope,
        &file,
        project_type,
        ContentSourceKind::Local,
        ContentOwnershipKind::UserAdded,
        None,
        false,
        state,
    )
    .await?;

    Ok(file)
}

async fn upsert_entry_for_file(
    scope: &ContentScope,
    file: &InstanceFile,
    project_type: ProjectType,
    source_kind: ContentSourceKind,
    ownership_kind: ContentOwnershipKind,
    provider_ref: Option<&ContentProviderRef>,
    origin: bool,
    state: &State,
) -> crate::Result<()> {
    // Serialize the foreign-keyed entry insert with instance deletion and
    // re-validate the parent rows under the lock.
    let _instance_lock = state.lock_instance_content(&scope.instance.id).await;
    let scope = resolve_content_scope(&scope.instance.id, None, state).await?;
    let pool = &state.pool;

    let entry = content_rows::upsert_content_entry_from_parts(
        content_rows::UpsertContentEntry {
            instance_id: &scope.instance.id,
            content_set_id: &scope.content_set_id,
            file_id: Some(&file.id),
            project_type,
            source_kind,
            ownership_kind,
            auto_dependency: false,
            server_requirement: ContentRequirement::Required,
            client_requirement: ContentRequirement::Required,
            enabled: file.enabled,
        },
        pool,
    )
    .await?;

    if let Some(provider_ref) = provider_ref {
        content_rows::upsert_content_provider_ref(
            &entry.id,
            provider_ref,
            origin,
            pool,
        )
        .await?;
    }

    Ok(())
}

fn infer_project_type(bytes: &Bytes) -> crate::Result<ProjectType> {
    let cursor = std::io::Cursor::new(&**bytes);
    let mut archive = zip::ZipArchive::new(cursor).map_err(|_| {
        crate::ErrorKind::InputError(
            "Unable to infer project type for input file".to_string(),
        )
    })?;

    if has_any_entry(
        &mut archive,
        &[
            "fabric.mod.json",
            "quilt.mod.json",
            "META-INF/neoforge.mods.toml",
            "META-INF/mods.toml",
            "mcmod.info",
        ],
    ) {
        return Ok(ProjectType::Mod);
    }
    if archive.by_name("pack.mcmeta").is_ok() {
        return classify_pack_archive(&mut archive);
    }
    if archive
        .file_names()
        .any(|name| name.starts_with("shaders/"))
    {
        return Ok(ProjectType::ShaderPack);
    }
    if archive.file_names().any(|name| name == "Metadata") {
        // .litematic files (Litematica schematic format) contain a root-level
        // "Metadata" NBT entry, which is unique among Minecraft content types.
        return Ok(ProjectType::Schematic);
    }
    Err(crate::ErrorKind::InputError(
        "Unable to infer project type for input file".to_string(),
    )
    .into())
}

/// Whether the archive contains any of the given marker files.
fn has_any_entry(
    archive: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>,
    names: &[&str],
) -> bool {
    names.iter().any(|name| archive.by_name(name).is_ok())
}

/// Distinguishes a data pack from a resource pack by the presence of a
/// top-level `data/` directory.
fn classify_pack_archive(
    archive: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>,
) -> crate::Result<ProjectType> {
    if archive.file_names().any(|name| name.starts_with("data/")) {
        Ok(ProjectType::DataPack)
    } else {
        Ok(ProjectType::ResourcePack)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::time::Duration;

    #[test]
    fn materialization_keys_reject_case_collisions_on_insensitive_platforms() {
        assert_eq!(
            materialization_path_key("mods/Foo.jar")
                == materialization_path_key("mods/foo.jar"),
            cfg!(any(windows, target_os = "macos"))
        );
    }

    #[test]
    fn toggle_refuses_to_replace_an_existing_sibling_file() {
        let directory = tempfile::tempdir().unwrap();
        let mods = directory.path().join("mods");
        std::fs::create_dir_all(&mods).unwrap();
        std::fs::write(mods.join("sibling.jar"), b"enabled").unwrap();
        std::fs::write(mods.join("sibling.jar.disabled"), b"disabled").unwrap();

        let error = resolve_toggle_paths(
            directory.path(),
            "mods/sibling.jar",
            Some(false),
        )
        .unwrap_err();
        assert!(error.to_string().contains("already exists"));

        let error = resolve_toggle_paths(
            directory.path(),
            "mods/sibling.jar.disabled",
            Some(true),
        )
        .unwrap_err();
        assert!(error.to_string().contains("already exists"));

        assert_eq!(
            std::fs::read(mods.join("sibling.jar")).unwrap(),
            b"enabled"
        );
        assert_eq!(
            std::fs::read(mods.join("sibling.jar.disabled")).unwrap(),
            b"disabled"
        );
    }

    #[test]
    fn toggle_renames_when_the_destination_is_free() {
        let directory = tempfile::tempdir().unwrap();
        let mods = directory.path().join("mods");
        std::fs::create_dir_all(&mods).unwrap();
        std::fs::write(mods.join("single.jar"), b"enabled").unwrap();
        std::fs::write(mods.join("already.jar.disabled"), b"disabled").unwrap();

        let (current, enabled, next) = resolve_toggle_paths(
            directory.path(),
            "mods/single.jar",
            Some(false),
        )
        .unwrap();
        assert_eq!(current, "mods/single.jar");
        assert!(!enabled);
        assert_eq!(next, "mods/single.jar.disabled");

        // Disabling a file that is already disabled keeps its current path.
        let (current, enabled, next) = resolve_toggle_paths(
            directory.path(),
            "mods/already.jar.disabled",
            Some(false),
        )
        .unwrap();
        assert_eq!(current, "mods/already.jar.disabled");
        assert!(!enabled);
        assert_eq!(next, "mods/already.jar.disabled");
    }

    #[tokio::test]
    async fn materialized_instances_do_not_share_writable_inodes() {
        let directory = tempfile::tempdir().unwrap();
        let shared = directory.path().join("shared");
        let first = directory.path().join("first/mod.jar");
        let second = directory.path().join("second/mod.jar");
        tokio::fs::write(&shared, b"original").await.unwrap();
        materialize_project_download(&shared, &first).await.unwrap();
        materialize_project_download(&shared, &second)
            .await
            .unwrap();
        tokio::fs::write(&first, b"modified").await.unwrap();
        assert_eq!(tokio::fs::read(shared).await.unwrap(), b"original");
        assert_eq!(tokio::fs::read(second).await.unwrap(), b"original");
    }

    #[test]
    fn resource_pack_target_preferences_ignore_game_version() {
        assert_eq!(
            target_preferences(
                "26.2".to_string(),
                ModLoader::NeoForge,
                ContentType::ResourcePack,
            ),
            ResolutionPreferences {
                game_versions: Vec::new(),
                loaders: vec!["minecraft".to_string()],
            },
        );
    }

    #[test]
    fn backup_relative_path_for_update_naming() {
        assert_eq!(
            backup_relative_path_for_update(
                "mods/Mod-1.0.jar",
                "mods/Mod-1.1.jar"
            )
            .as_deref(),
            Some("mods/Mod-1.1.jar_Mod-1.0.jar.old")
        );
        assert_eq!(
            backup_relative_path_for_update(
                "mods/Mod-1.0.jar.disabled",
                "mods/Mod-1.1.jar.disabled"
            )
            .as_deref(),
            Some("mods/Mod-1.1.jar_Mod-1.0.jar.old")
        );
        assert_eq!(
            backup_relative_path_for_update(
                "schematics/del/Old.litematic",
                "schematics/del/New.litematic"
            )
            .as_deref(),
            Some("schematics/del/New.litematic_Old.litematic.old")
        );
        assert_eq!(
            backup_relative_path_for_update("mods/Mod.jar", "mods/Mod.jar"),
            None
        );
    }

    fn zip_bytes(entries: &[(&str, &[u8])]) -> Vec<u8> {
        use std::io::Write as _;
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut buf);
            for (name, bytes) in entries {
                zip.start_file(*name, zip::write::FileOptions::<()>::default())
                    .expect("start entry");
                zip.write_all(bytes).expect("write entry");
            }
            zip.finish().expect("finish zip");
        }
        buf.into_inner()
    }

    #[test]
    fn wrapped_resource_pack_zip_plans_whole() {
        let bytes = zip_bytes(&[
            ("My Pack/pack.mcmeta", br#"{"pack":{"pack_format":15}}"#),
            ("My Pack/assets/minecraft/lang/en_us.json", b"{}"),
            ("My Pack/", b""),
        ]);
        assert!(
            wrapped_pack_plan(&bytes, ProjectType::ResourcePack)
                == Some(WrappedPackPlan::Whole {
                    wrapper: "My Pack".to_string()
                })
        );
    }

    #[test]
    fn wrapped_shader_pack_zip_plans_whole() {
        let bytes = zip_bytes(&[
            ("My Shader/shaders/a.fsh", b"v"),
            ("My Shader/shaders/b.vsh", b"v"),
        ]);
        assert!(
            wrapped_pack_plan(&bytes, ProjectType::ShaderPack)
                == Some(WrappedPackPlan::Whole {
                    wrapper: "My Shader".to_string()
                })
        );
    }

    #[test]
    fn flat_or_other_archives_have_no_plan() {
        let flat_rp = zip_bytes(&[
            ("pack.mcmeta", br#"{"pack":{"pack_format":15}}"#),
            ("assets/x.txt", b"x"),
        ]);
        assert_eq!(
            wrapped_pack_plan(&flat_rp, ProjectType::ResourcePack),
            None
        );
        let mod_zip = zip_bytes(&[("fabric.mod.json", b"{}")]);
        assert_eq!(wrapped_pack_plan(&mod_zip, ProjectType::Mod), None);
        let mixed = zip_bytes(&[
            ("My Pack/pack.mcmeta", br#"{"pack":{"pack_format":15}}"#),
            ("loose.txt", b"x"),
        ]);
        assert_eq!(wrapped_pack_plan(&mixed, ProjectType::ResourcePack), None);
        let no_marker = zip_bytes(&[("My Pack/assets/x.txt", b"x")]);
        assert_eq!(
            wrapped_pack_plan(&no_marker, ProjectType::ResourcePack),
            None
        );
    }

    #[test]
    fn wrapper_with_pack_children_plans_each_child() {
        let bytes = zip_bytes(&[
            ("packs/A/pack.mcmeta", br#"{"pack":{"pack_format":15}}"#),
            ("packs/A/assets/a.txt", b"x"),
            ("packs/B/pack.mcmeta", br#"{"pack":{"pack_format":15}}"#),
            ("packs/B/assets/b.txt", b"x"),
        ]);
        assert!(
            wrapped_pack_plan(&bytes, ProjectType::ResourcePack)
                == Some(WrappedPackPlan::Children {
                    children: vec!["A".to_string(), "B".to_string()]
                })
        );
    }

    #[test]
    fn shaderpack_collection_with_direct_shaders_plans_whole() {
        // A zipped shaderpacks folder that itself contains a pack folder
        // named "shaders" plus other packs: the wrapper is treated as one
        // pack (its marker sits at the wrapper root).
        let bytes = zip_bytes(&[
            ("shaderpacks/shaders/a.fsh", b"v"),
            ("shaderpacks/Other Pack/shaders/b.vsh", b"v"),
            ("shaderpacks/Another.zip", b"zip"),
        ]);
        assert!(
            wrapped_pack_plan(&bytes, ProjectType::ShaderPack)
                == Some(WrappedPackPlan::Whole {
                    wrapper: "shaderpacks".to_string()
                })
        );
    }

    #[tokio::test]
    async fn content_writes_wait_for_the_existing_sqlite_writer() {
        let directory = tempfile::tempdir().unwrap();
        let options = SqliteConnectOptions::new()
            .filename(directory.path().join("content-write.db"))
            .create_if_missing(true)
            .busy_timeout(Duration::from_secs(2));
        let pool = SqlitePoolOptions::new()
            .max_connections(2)
            .connect_with(options)
            .await
            .unwrap();

        let first = begin_content_write(&pool).await.unwrap();
        let second_pool = pool.clone();
        let second =
            tokio::spawn(
                async move { begin_content_write(&second_pool).await },
            );
        tokio::time::sleep(Duration::from_millis(50)).await;

        assert!(!second.is_finished());
        first.commit().await.unwrap();

        let second = tokio::time::timeout(Duration::from_secs(2), second)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        second.rollback().await.unwrap();
    }

    #[test]
    fn update_filename_replaces_the_official_name_inside_custom_names() {
        assert_eq!(
            preserved_update_relative_path(
                "mods/【世界生成】Terralith_v2.5.1.jar",
                Some("Terralith_v2.5.1.jar"),
                "Terralith_v2.6.1.jar",
            )
            .unwrap(),
            "mods/【世界生成】Terralith_v2.6.1.jar"
        );
        assert_eq!(
            preserved_update_relative_path(
                "mods/prefix-old.jar-suffix.disabled",
                Some("old.jar"),
                "new.jar",
            )
            .unwrap(),
            "mods/prefix-new.jar-suffix.disabled"
        );
    }

    #[test]
    fn update_filename_preserves_unmatched_or_ambiguous_custom_names() {
        assert_eq!(
            preserved_update_relative_path(
                "mods/my-custom-name.jar",
                Some("official-old.jar"),
                "official-new.jar",
            )
            .unwrap(),
            "mods/my-custom-name.jar"
        );
        assert_eq!(
            preserved_update_relative_path(
                "mods/old.jar-old.jar",
                Some("old.jar"),
                "new.jar",
            )
            .unwrap(),
            "mods/old.jar-old.jar"
        );
    }
}

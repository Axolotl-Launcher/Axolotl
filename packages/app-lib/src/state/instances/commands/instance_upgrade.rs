use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};
use std::time::SystemTime;

use chrono::{DateTime, Utc};

use crate::State;
use crate::state::instances::DependencyRelation;
use crate::state::instances::{
    ContentItemCapabilities, ContentOwnershipKind, InstanceContentSnapshot,
    InstanceContentSnapshotItem, InstanceLink, LoaderComponentKind,
    PackMemberMaterializationState, PackMemberOverrideKind,
};
use crate::state::{
    ContentItem, ContentProvider, ContentProviderRef, ContentSourceKind,
    DependencyType, InstanceUpgradeAction, InstanceUpgradeDependencyChange,
    InstanceUpgradeDependencyChangeKind, InstanceUpgradeDependencyRequirement,
    InstanceUpgradeEnvironment, InstanceUpgradeFixedConstraint,
    InstanceUpgradeIssue, InstanceUpgradeIssueCode, InstanceUpgradeItem,
    InstanceUpgradeItemStatus, InstanceUpgradePlan, InstanceUpgradeResolution,
    InstanceUpgradeSelection, InstanceUpgradeSolution,
    InstanceUpgradeSolutionKind, InstanceUpgradeSourceFile, ModrinthProjectId,
    ProjectType, ShaderRuntime, Version,
};

const MAX_CANDIDATES_PER_PROJECT: usize = 6;
// TODO(stage 3): Custom UI candidate listing needs an independent backend API.
// This solver bound is not, and must not become, the UI version-list limit.
const MAX_SEARCH_STATES: usize = 10_000;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct NodeKey {
    provider: ContentProvider,
    project_id: String,
}

impl NodeKey {
    fn new(provider: ContentProvider, project_id: impl Into<String>) -> Self {
        Self {
            provider,
            project_id: project_id.into(),
        }
    }

    fn label(&self) -> String {
        format!("{}:{}", self.provider.as_str(), self.project_id)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CandidateChannel {
    Release,
    Beta,
    Alpha,
}

impl CandidateChannel {
    fn rank(self) -> u8 {
        match self {
            Self::Release => 3,
            Self::Beta => 2,
            Self::Alpha => 1,
        }
    }

    fn is_prerelease(self) -> bool {
        self != Self::Release
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CandidateDependencyKind {
    Required,
    Optional,
    Incompatible,
    Embedded,
}

#[derive(Clone, Debug)]
struct CandidateDependency {
    key: NodeKey,
    version_id: Option<String>,
    kind: CandidateDependencyKind,
}

#[derive(Clone, Debug)]
struct UpgradeCandidate {
    key: NodeKey,
    version_id: String,
    published: DateTime<Utc>,
    channel: CandidateChannel,
    compatible: bool,
    installed_current: bool,
    dependencies: Vec<CandidateDependency>,
}

#[derive(Clone, Debug, Default)]
struct CandidatePool {
    candidates: Vec<UpgradeCandidate>,
    exploration_limited: bool,
    has_target_game_version_release: bool,
}

type UpgradeCatalog = HashMap<NodeKey, CandidatePool>;

#[derive(Clone, Debug)]
struct InstalledAlias {
    key: NodeKey,
    current_release_id: String,
}

#[derive(Clone, Debug)]
struct InstalledNode {
    content_id: String,
    key: NodeKey,
    current_release_id: String,
    project_type: ProjectType,
    enabled: bool,
    auto_dependency: bool,
    user_owned: bool,
    migratable: bool,
    aliases: Vec<InstalledAlias>,
}

#[derive(Clone, Debug)]
struct RootRequest {
    content_id: String,
    key: NodeKey,
    current_release_id: String,
    enabled: bool,
    action: InstanceUpgradeAction,
    allow_prerelease: bool,
}

#[derive(Clone, Debug)]
struct Requirement {
    key: NodeKey,
    version_id: Option<String>,
    explicit_prerelease: bool,
    preserve_unsafe: bool,
    root_content_id: String,
    root_key: NodeKey,
    origins: Vec<InstanceUpgradeDependencyRequirement>,
}

#[derive(Clone, Debug)]
struct SolverResult {
    assignments: HashMap<NodeKey, UpgradeCandidate>,
    physical_assignments: HashMap<String, NodeKey>,
    preserved_unsafe: HashSet<NodeKey>,
}

#[derive(Default)]
struct SearchState {
    visited: usize,
    limit_reached: bool,
    first_issue: Option<InstanceUpgradeIssue>,
}

#[derive(Clone)]
pub(crate) struct ReadOnlyUpgradeSource {
    snapshot: InstanceContentSnapshot,
    source_files: Vec<InstanceUpgradeSourceFile>,
    instance_path: String,
    game_dir: std::path::PathBuf,
    file_states: Vec<UpgradeSourceFileState>,
    catalog: Option<UpgradeCatalog>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct UpgradeSourceFileState {
    relative_path: String,
    size: u64,
    enabled: bool,
    modified: Option<SystemTime>,
}

pub(crate) struct UpgradePlanRuntimeValidation {
    source: ReadOnlyUpgradeSource,
    watcher_epoch: Option<u64>,
    validated_generation: Option<u64>,
    #[cfg(test)]
    full_hash_validations: usize,
    #[cfg(test)]
    incremental_hashes: usize,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum PhysicalNodeIdentity {
    Installed(String),
    Project(NodeKey),
}

#[derive(Default)]
struct InstalledAliasIndex {
    content_id_by_alias: HashMap<NodeKey, String>,
    aliases_by_content_id: HashMap<String, HashMap<NodeKey, String>>,
}

impl InstalledAliasIndex {
    fn new(installed: &[InstalledNode]) -> Self {
        let mut index = Self::default();
        for node in installed {
            for alias in &node.aliases {
                index
                    .content_id_by_alias
                    .insert(alias.key.clone(), node.content_id.clone());
                index
                    .aliases_by_content_id
                    .entry(node.content_id.clone())
                    .or_default()
                    .insert(
                        alias.key.clone(),
                        alias.current_release_id.clone(),
                    );
            }
        }
        index
    }

    fn content_id(&self, key: &NodeKey) -> Option<&str> {
        self.content_id_by_alias.get(key).map(String::as_str)
    }

    fn same_physical_content(&self, left: &NodeKey, right: &NodeKey) -> bool {
        self.content_id(left).is_some()
            && self.content_id(left) == self.content_id(right)
    }

    fn current_release(&self, content_id: &str, key: &NodeKey) -> Option<&str> {
        self.aliases_by_content_id
            .get(content_id)
            .and_then(|aliases| aliases.get(key))
            .map(String::as_str)
    }

    fn physical_identity(&self, key: &NodeKey) -> PhysicalNodeIdentity {
        self.content_id(key).map_or_else(
            || PhysicalNodeIdentity::Project(key.clone()),
            |content_id| {
                PhysicalNodeIdentity::Installed(content_id.to_string())
            },
        )
    }
}

pub(crate) async fn create_instance_upgrade_plan_with_source(
    instance_id: &str,
    target_environment: InstanceUpgradeEnvironment,
    state: &State,
) -> crate::Result<(InstanceUpgradePlan, ReadOnlyUpgradeSource)> {
    let metadata = super::get_instance_metadata(instance_id, &state.pool)
        .await?
        .ok_or_else(|| {
            crate::ErrorKind::InputError("Unknown instance".to_string())
        })?;
    if !matches!(metadata.link, InstanceLink::Unmanaged) {
        return Err(crate::ErrorKind::InputError(
            "Instance upgrade planner only supports unmanaged instances"
                .to_string(),
        )
        .into());
    }
    if metadata.applied_content_set.source_kind != ContentSourceKind::Local {
        return Err(crate::ErrorKind::InputError(
            "Instance upgrade planner only supports local content sets"
                .to_string(),
        )
        .into());
    }

    let mut source = read_only_upgrade_source(instance_id, state).await?;
    let snapshot = source.snapshot.clone();
    if snapshot.revision != metadata.applied_content_set.revision {
        return Err(crate::ErrorKind::InputError(
            "Instance content changed while the upgrade plan was being created; retry planning"
                .to_string(),
        )
        .into());
    }
    let source_environment = InstanceUpgradeEnvironment {
        game_version: metadata.applied_content_set.game_version.clone(),
        mod_loader: metadata.applied_content_set.loader,
        mod_loader_version: metadata.applied_content_set.loader_version.clone(),
        shader_runtime: source_shader_runtime(
            &metadata.loader_components,
            &snapshot,
        ),
    };
    let (mut items, installed) = snapshot_upgrade_items(&snapshot);
    let root_types = installed
        .iter()
        .filter(|node| !node.auto_dependency && node.migratable)
        .map(|node| (node.key.clone(), node.project_type))
        .collect::<HashMap<_, _>>();
    let fixed = FixedRootConstraints::default();
    let catalog = load_upgrade_catalog(
        &root_types,
        &installed,
        &fixed,
        &target_environment,
        state,
    )
    .await?;
    classify_items(&mut items, &installed, &catalog, &target_environment);

    let roots = roots_from_items(&items, &installed);
    let outcome = solve_upgrade_with_fixed_roots(
        &roots,
        &installed,
        &catalog,
        &fixed,
        &confirmed_prereleases(&items),
    );
    apply_solver_issues_to_items(&mut items, &outcome.issues);
    let mut blocking_issues = outcome.issues;
    for item in &items {
        if let Some(issue) = blocking_issue_for_item(item, false) {
            blocking_issues.push(issue);
        }
    }
    deduplicate_issues(&mut blocking_issues);
    let warnings = item_warnings(&items);
    let newest_solution = outcome
        .solutions
        .iter()
        .max_by(|left, right| compare_newest(left, right, &roots))
        .map(|solution| {
            materialize_solution(
                InstanceUpgradeSolutionKind::Newest,
                solution,
                &roots,
                &installed,
            )
        });
    let minimal_change_solution = outcome
        .solutions
        .iter()
        .min_by(|left, right| compare_minimal(left, right, &roots, &installed))
        .map(|solution| {
            materialize_solution(
                InstanceUpgradeSolutionKind::MinimalChange,
                solution,
                &roots,
                &installed,
            )
        });
    let selected_solution = newest_solution.clone();
    let dependency_changes = selected_solution
        .as_ref()
        .map(|solution| solution.dependency_changes.clone())
        .unwrap_or_default();

    let plan = InstanceUpgradePlan {
        id: format!("instance-upgrade-plan:{}", uuid::Uuid::new_v4()),
        instance_id: instance_id.to_string(),
        source_revision: snapshot.revision,
        source_files: source.source_files.clone(),
        source_environment,
        target_environment,
        items,
        dependency_changes,
        warnings,
        blocking_issues,
        newest_solution,
        minimal_change_solution,
        selected_solution,
        custom_constraints: Vec::new(),
    };
    source.catalog = Some(catalog);
    Ok((plan, source))
}

async fn read_only_upgrade_source(
    instance_id: &str,
    state: &State,
) -> crate::Result<ReadOnlyUpgradeSource> {
    let mut snapshot =
        super::get_cached_content_snapshot(instance_id, state).await?;
    let instance = crate::state::instances::adapters::sqlite::instance_rows::get_instance_by_id(
        instance_id,
        &state.pool,
    )
    .await?
    .ok_or_else(|| crate::ErrorKind::InputError("Unknown instance".to_string()))?;
    let instance_dir = upgrade_source_game_dir(&instance, &state.directories)?;
    let scanned =
        crate::state::instances::adapters::filesystem::scan_content_files_from(
            &instance_dir,
            &instance.path,
        )?;
    let file_states = scanned
        .iter()
        .map(|file| UpgradeSourceFileState {
            relative_path: file.relative_path.clone(),
            size: file.size,
            enabled: file.enabled,
            modified: file.modified,
        })
        .collect();
    let mut scanned_by_path = HashMap::new();
    let mut source_files = Vec::new();
    for file in scanned {
        let (_, sha1) = crate::util::fetch::sha1_file_async(
            instance_dir.join(&file.relative_path),
        )
        .await?;
        source_files.push(InstanceUpgradeSourceFile {
            relative_path: file.relative_path.clone(),
            sha1: sha1.clone(),
            size: file.size,
            enabled: file.enabled,
        });
        scanned_by_path.insert(file.relative_path.clone(), (file, sha1));
    }
    snapshot.items.retain(|item| {
        if !crate::state::instances::adapters::filesystem::is_scannable_project_path(
            item.project_type,
            &item.expected_relative_path,
        ) {
            return true;
        }
        scanned_by_path
            .get(&item.expected_relative_path.replace('\\', "/"))
            .is_some_and(|(_, sha1)| {
                item.content.as_ref().is_some_and(|content| {
                    content.id.eq_ignore_ascii_case(sha1)
                })
            })
    });
    let represented = snapshot
        .items
        .iter()
        .map(|item| item.expected_relative_path.replace('\\', "/"))
        .collect::<HashSet<_>>();
    for (_, (file, sha1)) in scanned_by_path {
        if represented.contains(&file.relative_path) {
            continue;
        }
        let Some(project_type) = crate::state::instances::adapters::filesystem::project_type_from_relative_path(
            &file.relative_path,
        ) else {
            continue;
        };
        snapshot.items.push(InstanceContentSnapshotItem {
            artifact_ref: None,
            compatibility: None,
            resource_id: None,
            pack_resource_id: None,
            file_id: None,
            entry_id: None,
            member_id: None,
            ownership_kind: ContentOwnershipKind::LocalDiscovered,
            materialization_state: PackMemberMaterializationState::Present,
            override_kind: PackMemberOverrideKind::None,
            expected_relative_path: file.relative_path.clone(),
            required: false,
            project_type,
            provider: None,
            provider_project_id: None,
            provider_release_id: None,
            content: Some(ContentItem {
                file_name: file.file_name,
                file_path: file.relative_path,
                id: sha1,
                size: file.size,
                enabled: file.enabled,
                project_type,
                project: None,
                version: None,
                owner: None,
                update: None,
                date_added: None,
                provider_refs: Vec::new(),
                origin_provider: None,
                rollback: None,
                environment: None,
                source_kind: None,
                external: true,
                loader: None,
            }),
            capabilities: ContentItemCapabilities::default(),
            dependency: None,
        });
    }
    source_files
        .sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(ReadOnlyUpgradeSource {
        snapshot,
        source_files,
        instance_path: instance.path,
        game_dir: instance_dir,
        file_states,
        catalog: None,
    })
}

fn upgrade_source_game_dir(
    instance: &crate::state::Instance,
    directories: &crate::state::DirectoryInfo,
) -> crate::Result<std::path::PathBuf> {
    Ok(
        crate::launcher::instance_runtime::InstanceRuntimeAdapter::for_instance(
            instance,
            directories,
        )?
        .game_dir(),
    )
}

pub(crate) async fn validate_instance_upgrade_plan_source(
    plan: &InstanceUpgradePlan,
    state: &State,
) -> crate::Result<ReadOnlyUpgradeSource> {
    let current = read_only_upgrade_source(&plan.instance_id, state).await?;
    ensure_upgrade_source_files_match(
        &plan.instance_id,
        &plan.source_files,
        &current.source_files,
    )?;
    Ok(current)
}

pub(crate) async fn scan_instance_upgrade_source_files(
    instance_id: &str,
    state: &State,
) -> crate::Result<Vec<InstanceUpgradeSourceFile>> {
    Ok(read_only_upgrade_source(instance_id, state)
        .await?
        .source_files)
}

impl UpgradePlanRuntimeValidation {
    pub(crate) async fn new(
        source: ReadOnlyUpgradeSource,
        instance_id: &str,
        creation_watch: Option<
            crate::state::instances::watcher::InstanceContentWatchSnapshot,
        >,
        state: &State,
    ) -> Self {
        let watch = state
            .file_watcher
            .track_upgrade_source(
                instance_id,
                source
                    .source_files
                    .iter()
                    .map(|file| file.relative_path.clone()),
            )
            .await;
        let validated_generation = creation_watch
            .as_ref()
            .filter(|creation| {
                watch
                    .as_ref()
                    .is_some_and(|current| current.epoch == creation.epoch)
            })
            .map(|creation| creation.generation);
        Self {
            source,
            watcher_epoch: watch.as_ref().map(|watch| watch.epoch),
            validated_generation,
            #[cfg(test)]
            full_hash_validations: 0,
            #[cfg(test)]
            incremental_hashes: 0,
        }
    }

    pub(crate) async fn validate(
        &mut self,
        plan: &InstanceUpgradePlan,
        state: &State,
    ) -> crate::Result<ReadOnlyUpgradeSource> {
        let Some(mut before) = state
            .file_watcher
            .content_watch_snapshot(&plan.instance_id)
            .await
        else {
            // Unwatched game directories still support metadata and hash
            // validation through the resolved source directory.
            self.incremental_validate(plan, None, state).await?;
            return Ok(self.source.clone());
        };
        if self.watcher_epoch != Some(before.epoch)
            || self.validated_generation.is_none()
            || self.validated_generation > Some(before.generation)
        {
            return self.authoritative_validate(plan, state).await;
        }

        for _ in 0..2 {
            self.incremental_validate(plan, Some(&before), state)
                .await?;
            let Some(after) = state
                .file_watcher
                .content_watch_snapshot(&plan.instance_id)
                .await
            else {
                return self.authoritative_validate(plan, state).await;
            };
            if after.epoch == before.epoch
                && after.generation == before.generation
            {
                self.watcher_epoch = Some(after.epoch);
                self.validated_generation = Some(after.generation);
                return Ok(self.source.clone());
            }
            before = after;
        }

        self.authoritative_validate(plan, state).await
    }

    async fn incremental_validate(
        &mut self,
        plan: &InstanceUpgradePlan,
        watch: Option<
            &crate::state::instances::watcher::InstanceContentWatchSnapshot,
        >,
        _state: &State,
    ) -> crate::Result<()> {
        let scanned = crate::state::instances::adapters::filesystem::scan_content_files_from(
            &self.source.game_dir,
            &self.source.instance_path,
        )?;
        let planned = self
            .source
            .source_files
            .iter()
            .map(|file| (file.relative_path.as_str(), file))
            .collect::<HashMap<_, _>>();
        let previous = self
            .source
            .file_states
            .iter()
            .map(|file| (file.relative_path.as_str(), file))
            .collect::<HashMap<_, _>>();
        if scanned.len() != planned.len()
            || scanned
                .iter()
                .any(|file| !planned.contains_key(file.relative_path.as_str()))
        {
            return stale_upgrade_source(&plan.instance_id);
        }

        let watcher_changed = watch.is_some_and(|watch| {
            self.validated_generation != Some(watch.generation)
        });
        let instance_dir = &self.source.game_dir;
        for file in &scanned {
            let planned_file = planned[&file.relative_path.as_str()];
            if file.size != planned_file.size
                || file.enabled != planned_file.enabled
            {
                return stale_upgrade_source(&plan.instance_id);
            }
            let metadata_changed = previous
                .get(file.relative_path.as_str())
                .is_none_or(|previous| previous.modified != file.modified);
            let watcher_marked = watcher_changed
                && watch.is_some_and(|watch| {
                    watch.dirty_paths.contains(&file.relative_path)
                });
            if metadata_changed || watcher_marked {
                let (_, sha1) = crate::util::fetch::sha1_file_async(
                    instance_dir.join(&file.relative_path),
                )
                .await?;
                #[cfg(test)]
                {
                    self.incremental_hashes += 1;
                }
                if !planned_file.sha1.eq_ignore_ascii_case(&sha1) {
                    return stale_upgrade_source(&plan.instance_id);
                }
            }
        }
        self.source.file_states = scanned
            .into_iter()
            .map(|file| UpgradeSourceFileState {
                relative_path: file.relative_path,
                size: file.size,
                enabled: file.enabled,
                modified: file.modified,
            })
            .collect();
        Ok(())
    }

    async fn authoritative_validate(
        &mut self,
        plan: &InstanceUpgradePlan,
        state: &State,
    ) -> crate::Result<ReadOnlyUpgradeSource> {
        #[cfg(test)]
        {
            self.full_hash_validations += 1;
        }
        let current =
            validate_instance_upgrade_plan_source(plan, state).await?;
        let watch = state
            .file_watcher
            .track_upgrade_source(
                &plan.instance_id,
                current
                    .source_files
                    .iter()
                    .map(|file| file.relative_path.clone()),
            )
            .await;
        self.watcher_epoch = watch.as_ref().map(|watch| watch.epoch);
        self.validated_generation =
            watch.as_ref().map(|watch| watch.generation);
        let catalog = self.source.catalog.clone();
        self.source = current;
        self.source.catalog = catalog;
        Ok(self.source.clone())
    }
}

fn stale_upgrade_source<T>(instance_id: &str) -> crate::Result<T> {
    Err(crate::ErrorKind::StaleInstanceUpgradePlanSource {
        instance_id: instance_id.to_string(),
    }
    .into())
}

fn ensure_upgrade_source_files_match(
    instance_id: &str,
    planned: &[InstanceUpgradeSourceFile],
    current: &[InstanceUpgradeSourceFile],
) -> crate::Result<()> {
    if current != planned {
        return Err(crate::ErrorKind::StaleInstanceUpgradePlanSource {
            instance_id: instance_id.to_string(),
        }
        .into());
    }
    Ok(())
}

fn source_shader_runtime(
    components: &[crate::state::LoaderComponent],
    snapshot: &InstanceContentSnapshot,
) -> ShaderRuntime {
    if snapshot.items.iter().any(item_has_iris_identity) {
        return ShaderRuntime::Iris;
    }
    if components
        .iter()
        .any(|component| component.kind == LoaderComponentKind::OptiFine)
    {
        return ShaderRuntime::OptiFine;
    }
    if snapshot.items.iter().any(|item| {
        item.project_type == ProjectType::Mod
            && item
                .content
                .as_ref()
                .is_none_or(|content| content.provider_refs.is_empty())
    }) {
        ShaderRuntime::Unknown
    } else {
        ShaderRuntime::None
    }
}

fn item_has_iris_identity(item: &InstanceContentSnapshotItem) -> bool {
    (item.provider == Some(ContentProvider::Modrinth)
        && item.provider_project_id.as_deref() == Some("YL57xq9U"))
        || item.content.as_ref().is_some_and(|content| {
            content.provider_refs.iter().any(|reference| {
                matches!(reference, ContentProviderRef::Modrinth { project_id, .. } if project_id.as_str() == "YL57xq9U")
            })
        })
}

fn snapshot_upgrade_items(
    snapshot: &InstanceContentSnapshot,
) -> (Vec<InstanceUpgradeItem>, Vec<InstalledNode>) {
    let mut items = Vec::new();
    let mut installed = Vec::new();
    for item in &snapshot.items {
        let content_id = stable_content_id(item);
        let current_enabled =
            item.content.as_ref().is_none_or(|content| content.enabled);
        let auto_dependency = item
            .dependency
            .as_ref()
            .is_some_and(|dependency| dependency.auto_dependency);
        let unsupported = is_world_datapack(&item.expected_relative_path)
            || matches!(
                item.project_type,
                ProjectType::Schematic | ProjectType::WorldSave
            );
        let installed_identity = installed_identity(item);
        let recognized = installed_identity
            .as_ref()
            .map(|(key, _, _)| (key.provider, key.project_id.clone()))
            .or_else(|| item.provider.zip(item.provider_project_id.clone()));
        let planner_provider = installed_identity
            .as_ref()
            .map(|(key, _, _)| key.provider)
            .or(item.provider);
        let planner_project_id = installed_identity
            .as_ref()
            .map(|(key, _, _)| key.project_id.clone())
            .or_else(|| item.provider_project_id.clone());
        let planner_release_id = installed_identity
            .as_ref()
            .map(|(_, release_id, _)| release_id.clone())
            .or_else(|| item.provider_release_id.clone());
        let status = if unsupported {
            InstanceUpgradeItemStatus::UnsupportedContentType
        } else if recognized.is_none() {
            InstanceUpgradeItemStatus::Unidentified
        } else {
            InstanceUpgradeItemStatus::NoCompatibleRelease
        };
        let action = if unsupported || recognized.is_none() {
            InstanceUpgradeAction::Keep
        } else {
            InstanceUpgradeAction::Upgrade
        };
        items.push(InstanceUpgradeItem {
            content_id: content_id.clone(),
            relative_path: item.expected_relative_path.clone(),
            project_type: item.project_type,
            provider: planner_provider,
            project_id: planner_project_id,
            current_release_id: planner_release_id,
            current_enabled,
            auto_dependency,
            status,
            resolution: InstanceUpgradeResolution {
                content_id: content_id.clone(),
                action,
                allow_prerelease: false,
                confirmed_prerelease_dependencies: Vec::new(),
            },
            candidate_release_ids: Vec::new(),
        });
        if let Some((key, current_release_id, aliases)) = installed_identity {
            installed.push(InstalledNode {
                content_id,
                key,
                current_release_id: current_release_id.clone(),
                project_type: item.project_type,
                enabled: current_enabled,
                auto_dependency,
                user_owned: item.ownership_kind
                    == ContentOwnershipKind::UserAdded
                    && !auto_dependency,
                migratable: !unsupported,
                aliases,
            });
        }
    }
    (items, installed)
}

fn installed_identity(
    item: &InstanceContentSnapshotItem,
) -> Option<(NodeKey, String, Vec<InstalledAlias>)> {
    let mut aliases = item
        .content
        .as_ref()
        .into_iter()
        .flat_map(|content| &content.provider_refs)
        .filter_map(|reference| {
            Some(InstalledAlias {
                key: NodeKey::new(
                    reference.provider(),
                    reference.database_project_id(),
                ),
                current_release_id: reference.database_release_id()?,
            })
        })
        .collect::<Vec<_>>();
    if let (Some(provider), Some(project_id), Some(release_id)) = (
        item.provider,
        item.provider_project_id.as_deref(),
        item.provider_release_id.as_deref(),
    ) {
        let primary = InstalledAlias {
            key: NodeKey::new(provider, project_id),
            current_release_id: release_id.to_string(),
        };
        aliases.retain(|alias| alias.key != primary.key);
        aliases.insert(0, primary);
    }
    let mut seen = HashSet::new();
    aliases.retain(|alias| seen.insert(alias.key.clone()));
    let primary_index = item
        .provider
        .zip(item.provider_project_id.as_deref())
        .and_then(|(provider, project_id)| {
            aliases.iter().position(|alias| {
                alias.key.provider == provider
                    && alias.key.project_id == project_id
            })
        })
        .unwrap_or(0);
    let primary = aliases.get(primary_index)?.clone();
    if primary_index != 0 {
        aliases.swap(0, primary_index);
    }
    Some((primary.key, primary.current_release_id, aliases))
}

fn stable_content_id(item: &InstanceContentSnapshotItem) -> String {
    item.entry_id
        .as_deref()
        .or(item.member_id.as_deref())
        .or(item.file_id.as_deref())
        .unwrap_or(&item.expected_relative_path)
        .to_string()
}

fn is_world_datapack(path: &str) -> bool {
    let components = path
        .replace('\\', "/")
        .split('/')
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();
    components.len() >= 4
        && components[0] == "saves"
        && components[2] == "datapacks"
}

struct SolveOutcome {
    solutions: Vec<SolverResult>,
    issues: Vec<InstanceUpgradeIssue>,
    #[cfg(test)]
    visited_states: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SolveStrategy {
    Newest,
    MinimalChange,
}

#[derive(Clone, Debug)]
struct RootCandidateOptions {
    content_id: String,
    key: NodeKey,
    current_release_id: String,
    fixed: bool,
    exploration_limited: bool,
    candidates: Vec<Option<UpgradeCandidate>>,
}

#[derive(Clone, Debug, Default)]
struct FixedRootConstraints {
    by_content_id: HashMap<String, (NodeKey, String)>,
    versions_by_project: HashMap<NodeKey, HashSet<String>>,
}

impl FixedRootConstraints {
    fn from_constraints(
        constraints: &[InstanceUpgradeFixedConstraint],
    ) -> Self {
        let mut fixed = Self::default();
        for constraint in constraints {
            let key = NodeKey::new(constraint.provider, &constraint.project_id);
            fixed.by_content_id.insert(
                constraint.content_id.clone(),
                (key.clone(), constraint.version_id.clone()),
            );
            fixed
                .versions_by_project
                .entry(key)
                .or_default()
                .insert(constraint.version_id.clone());
        }
        fixed
    }

    #[cfg(test)]
    fn from_project_versions(
        roots: &[RootRequest],
        versions: &HashMap<NodeKey, String>,
    ) -> Self {
        let constraints = roots
            .iter()
            .filter_map(|root| {
                versions.get(&root.key).map(|version_id| {
                    InstanceUpgradeFixedConstraint {
                        content_id: root.content_id.clone(),
                        provider: root.key.provider,
                        project_id: root.key.project_id.clone(),
                        version_id: version_id.clone(),
                    }
                })
            })
            .collect::<Vec<_>>();
        Self::from_constraints(&constraints)
    }

    fn version_for_root<'a>(&'a self, root: &RootRequest) -> Option<&'a str> {
        self.by_content_id
            .get(&root.content_id)
            .filter(|(key, _)| key == &root.key)
            .map(|(_, version_id)| version_id.as_str())
    }

    fn is_fixed_root(&self, root: &RootRequest) -> bool {
        self.version_for_root(root).is_some()
    }

    fn contains_content(&self, content_id: &str) -> bool {
        self.by_content_id.contains_key(content_id)
    }

    fn versions_for_project(&self, key: &NodeKey) -> Option<&HashSet<String>> {
        self.versions_by_project.get(key)
    }

    fn is_empty(&self) -> bool {
        self.by_content_id.is_empty()
    }
}

#[derive(Clone, Debug)]
struct ConflictSet {
    involved_root_content_ids: HashSet<String>,
    involved_parent_projects: HashSet<NodeKey>,
    dependency_project: Option<NodeKey>,
    reason: InstanceUpgradeIssueCode,
    candidate_limit_roots: HashSet<String>,
}

impl ConflictSet {
    fn can_branch_root(&self, root: &RootCandidateOptions) -> bool {
        let relevant_reason = matches!(
            self.reason,
            InstanceUpgradeIssueCode::DependencyConflict
                | InstanceUpgradeIssueCode::IncompatibleDependency
                | InstanceUpgradeIssueCode::MissingRequiredDependency
                | InstanceUpgradeIssueCode::NoCompatibleRelease
        );
        let is_dependency_target_only = self.dependency_project.as_ref()
            == Some(&root.key)
            && !self.involved_root_content_ids.contains(&root.content_id)
            && !self.involved_parent_projects.contains(&root.key);
        relevant_reason
            && !is_dependency_target_only
            && (self.involved_root_content_ids.contains(&root.content_id)
                || self.involved_parent_projects.contains(&root.key))
    }

    fn same_scope(&self, other: &Self) -> bool {
        self.involved_root_content_ids == other.involved_root_content_ids
            && self.involved_parent_projects == other.involved_parent_projects
            && self.dependency_project == other.dependency_project
            && self.reason == other.reason
    }

    fn merge_candidate_limit_evidence(&mut self, other: &Self) {
        if self.same_scope(other) {
            self.candidate_limit_roots
                .extend(other.candidate_limit_roots.iter().cloned());
        }
    }

    fn candidate_search_incomplete(&self) -> bool {
        !self.candidate_limit_roots.is_empty()
    }
}

#[derive(Clone, Debug)]
struct ConflictFailure {
    issue: InstanceUpgradeIssue,
    conflict: ConflictSet,
}

fn retain_best_failure(
    best_failure: &mut Option<ConflictFailure>,
    failure: ConflictFailure,
) {
    let Some(best) = best_failure.as_mut() else {
        *best_failure = Some(failure);
        return;
    };
    if best.conflict.same_scope(&failure.conflict) {
        best.conflict
            .merge_candidate_limit_evidence(&failure.conflict);
        if failure.issue.dependency_requirements.len()
            > best.issue.dependency_requirements.len()
        {
            best.issue = failure.issue;
        }
    } else if best.conflict.candidate_search_incomplete()
        && !failure.conflict.candidate_search_incomplete()
    {
        *best = failure;
    }
}

struct StrategySolveOutcome {
    solution: Option<SolverResult>,
    issue: Option<InstanceUpgradeIssue>,
    #[cfg(test)]
    visited_states: usize,
}

#[derive(Eq, PartialEq)]
struct FrontierState {
    selected: Vec<usize>,
    primary_score: i64,
    freshness_score: i64,
}

impl Ord for FrontierState {
    fn cmp(&self, other: &Self) -> Ordering {
        self.primary_score
            .cmp(&other.primary_score)
            .then_with(|| self.freshness_score.cmp(&other.freshness_score))
            .then_with(|| other.selected.cmp(&self.selected))
    }
}

impl PartialOrd for FrontierState {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub(crate) async fn recompute_instance_upgrade_plan_from_source(
    plan: &mut InstanceUpgradePlan,
    fixed_constraints: &[InstanceUpgradeFixedConstraint],
    selected_kind: InstanceUpgradeSolutionKind,
    source: ReadOnlyUpgradeSource,
    state: &State,
) -> crate::Result<()> {
    let started_at = std::time::Instant::now();
    ensure_upgrade_source_files_match(
        &plan.instance_id,
        &plan.source_files,
        &source.source_files,
    )?;
    let snapshot = source.snapshot;
    let cached_catalog = source.catalog;
    let (_, installed) = snapshot_upgrade_items(&snapshot);
    let root_types = installed
        .iter()
        .filter(|node| !node.auto_dependency && node.migratable)
        .map(|node| (node.key.clone(), node.project_type))
        .collect::<HashMap<_, _>>();
    let fixed = FixedRootConstraints::from_constraints(fixed_constraints);
    let catalog = if fixed_constraints.is_empty() {
        cached_catalog.ok_or_else(|| {
            crate::ErrorKind::OtherError(
                "Upgrade plan source is missing its catalog".to_string(),
            )
        })?
    } else {
        match cached_catalog {
            Some(catalog)
                if catalog_supports_fixed_constraints(&catalog, &fixed) =>
            {
                catalog
            }
            _ => {
                load_upgrade_catalog(
                    &root_types,
                    &installed,
                    &fixed,
                    &plan.target_environment,
                    state,
                )
                .await?
            }
        }
    };
    tracing::debug!(
        elapsed_ms = started_at.elapsed().as_millis(),
        catalog_reused = fixed_constraints.is_empty()
            || catalog_supports_fixed_constraints(&catalog, &fixed),
        "[upgrade-plan-bench] recompute catalog"
    );
    classify_items(
        &mut plan.items,
        &installed,
        &catalog,
        &plan.target_environment,
    );
    tracing::debug!(
        elapsed_ms = started_at.elapsed().as_millis(),
        "[upgrade-plan-bench] recompute classified items"
    );
    let roots = roots_from_items(&plan.items, &installed);
    let outcome = solve_upgrade_with_fixed_roots(
        &roots,
        &installed,
        &catalog,
        &fixed,
        &confirmed_prereleases(&plan.items),
    );
    tracing::debug!(
        elapsed_ms = started_at.elapsed().as_millis(),
        "[upgrade-plan-bench] recompute solver"
    );
    apply_solver_issues_to_items(&mut plan.items, &outcome.issues);
    let mut blocking_issues = outcome.issues;
    for item in &plan.items {
        let fixed_prerelease = fixed.contains_content(&item.content_id);
        if let Some(issue) = blocking_issue_for_item(item, fixed_prerelease) {
            blocking_issues.push(issue);
        }
    }
    deduplicate_issues(&mut blocking_issues);
    plan.warnings = item_warnings_with_fixed(
        &plan.items,
        &fixed.by_content_id.keys().cloned().collect::<HashSet<_>>(),
    );
    plan.blocking_issues = blocking_issues;
    plan.newest_solution = outcome
        .solutions
        .iter()
        .max_by(|left, right| compare_newest(left, right, &roots))
        .map(|solution| {
            materialize_solution(
                InstanceUpgradeSolutionKind::Newest,
                solution,
                &roots,
                &installed,
            )
        });
    plan.minimal_change_solution = outcome
        .solutions
        .iter()
        .min_by(|left, right| compare_minimal(left, right, &roots, &installed))
        .map(|solution| {
            materialize_solution(
                InstanceUpgradeSolutionKind::MinimalChange,
                solution,
                &roots,
                &installed,
            )
        });
    plan.selected_solution = match selected_kind {
        InstanceUpgradeSolutionKind::Newest => plan.newest_solution.clone(),
        InstanceUpgradeSolutionKind::MinimalChange => {
            plan.minimal_change_solution.clone()
        }
        InstanceUpgradeSolutionKind::Custom => {
            outcome.solutions.first().map(|solution| {
                materialize_solution(
                    InstanceUpgradeSolutionKind::Custom,
                    solution,
                    &roots,
                    &installed,
                )
            })
        }
    };
    plan.dependency_changes = plan
        .selected_solution
        .as_ref()
        .map(|solution| solution.dependency_changes.clone())
        .unwrap_or_default();
    Ok(())
}

fn catalog_supports_fixed_constraints(
    catalog: &UpgradeCatalog,
    fixed: &FixedRootConstraints,
) -> bool {
    fixed.versions_by_project.iter().all(|(key, versions)| {
        catalog.get(key).is_some_and(|pool| {
            versions.iter().all(|version_id| {
                pool.candidates.iter().any(|candidate| {
                    candidate.version_id == *version_id && candidate.compatible
                })
            })
        })
    })
}

async fn load_upgrade_catalog(
    root_types: &HashMap<NodeKey, ProjectType>,
    installed: &[InstalledNode],
    fixed: &FixedRootConstraints,
    target: &InstanceUpgradeEnvironment,
    state: &State,
) -> crate::Result<UpgradeCatalog> {
    let current_versions = installed
        .iter()
        .flat_map(|node| {
            node.aliases.iter().map(|alias| {
                (alias.key.clone(), alias.current_release_id.clone())
            })
        })
        .collect::<HashMap<_, _>>();
    let mut catalog = HashMap::new();
    let mut queue = root_types
        .iter()
        .map(|(key, project_type)| (key.clone(), *project_type))
        .collect::<VecDeque<_>>();
    let mut seen = HashSet::new();
    let mut exact_versions = fixed.versions_by_project.clone();
    while !queue.is_empty() {
        let mut batch = Vec::new();
        while let Some((key, project_type)) = queue.pop_front() {
            if seen.insert(key.clone()) {
                batch.push((key, project_type));
            }
        }
        let results = futures::future::try_join_all(batch.into_iter().map(
            |(key, project_type)| {
                let current = current_versions.get(&key).cloned();
                let exact =
                    exact_versions.get(&key).cloned().unwrap_or_default();
                let custom_fixed_versions = fixed
                    .versions_for_project(&key)
                    .cloned()
                    .unwrap_or_default();
                async move {
                    let candidates = match key.provider {
                        ContentProvider::Modrinth => {
                            load_modrinth_candidates(
                                &key,
                                project_type,
                                current.as_deref(),
                                &exact,
                                &custom_fixed_versions,
                                target,
                                state,
                            )
                            .await?
                        }
                        ContentProvider::CurseForge => {
                            load_curseforge_candidates(
                                &key,
                                project_type,
                                current.as_deref(),
                                &exact,
                                &custom_fixed_versions,
                                target,
                                state,
                            )
                            .await?
                        }
                        ContentProvider::McArchive | ContentProvider::Local => {
                            CandidatePool::default()
                        }
                    };
                    Ok::<_, crate::Error>((key, candidates))
                }
            },
        ))
        .await?;
        for (key, candidates) in results {
            for candidate in &candidates.candidates {
                for dependency in &candidate.dependencies {
                    if dependency.kind == CandidateDependencyKind::Required {
                        if let Some(version_id) = dependency.version_id.as_ref()
                        {
                            let added = exact_versions
                                .entry(dependency.key.clone())
                                .or_default()
                                .insert(version_id.clone());
                            if added {
                                seen.remove(&dependency.key);
                            }
                        }
                        queue.push_back((
                            dependency.key.clone(),
                            ProjectType::Mod,
                        ));
                    }
                }
            }
            catalog.insert(key, candidates);
        }
    }
    for node in installed {
        for alias in &node.aliases {
            let pool = catalog.entry(alias.key.clone()).or_default();
            if let Some(candidate) =
                pool.candidates.iter_mut().find(|candidate| {
                    candidate.version_id == alias.current_release_id
                })
            {
                candidate.installed_current = true;
            } else {
                pool.candidates.push(UpgradeCandidate {
                    key: alias.key.clone(),
                    version_id: alias.current_release_id.clone(),
                    published: DateTime::<Utc>::MIN_UTC,
                    channel: CandidateChannel::Release,
                    compatible: false,
                    installed_current: true,
                    dependencies: Vec::new(),
                });
            }
        }
    }
    Ok(catalog)
}

/// The environment solver reads provider protocol projections from the shared
/// resource catalog. Source failure remains visible instead of becoming an
/// empty list or an apparently fresh legacy cache hit.
async fn load_planner_project_versions(
    project_id: &ModrinthProjectId,
    target: &InstanceUpgradeEnvironment,
    state: &State,
) -> crate::Result<(Vec<Version>, bool)> {
    crate::resources::legacy_catalog::page(
        crate::resources::SourceId::modrinth(),
        project_id.as_str().into(),
        crate::resources::ipc::ResourceTarget {
            game_versions: vec![target.game_version.clone()],
            loaders: vec![],
            channels: vec![],
        },
        state,
    )
    .await
}
async fn planner_version(
    id: &str,
    state: &State,
) -> crate::Result<Option<Version>> {
    crate::resources::legacy_catalog::exact(
        crate::resources::SourceId::modrinth(),
        None,
        id.into(),
        state,
    )
    .await
}
async fn planner_cf_file(
    project_id: u32,
    file_id: u32,
    state: &State,
) -> crate::Result<Option<crate::api::curseforge::CurseForgeFile>> {
    crate::resources::legacy_catalog::exact(
        crate::resources::SourceId::curseforge(),
        Some(project_id.to_string()),
        file_id.to_string(),
        state,
    )
    .await
}

async fn load_modrinth_candidates(
    key: &NodeKey,
    project_type: ProjectType,
    current_release_id: Option<&str>,
    exact_versions: &HashSet<String>,
    custom_fixed_versions: &HashSet<String>,
    target: &InstanceUpgradeEnvironment,
    state: &State,
) -> crate::Result<CandidatePool> {
    let project_id = ModrinthProjectId::new(key.project_id.clone())?;
    let (mut versions, more_releases) =
        load_planner_project_versions(&project_id, target, state).await?;
    let has_target_game_version_release = versions.iter().any(|version| {
        version
            .game_versions
            .iter()
            .any(|game_version| game_version == &target.game_version)
    });
    versions.sort_by(compare_modrinth_version);
    let (mut selected, exploration_limited) =
        bounded_compatible_candidates(&versions, |version| {
            modrinth_version_matches(version, project_type, target)
        });
    if let Some(current_release_id) = current_release_id
        && !selected
            .iter()
            .any(|version| version.id == current_release_id)
    {
        let current = versions
            .iter()
            .find(|version| version.id == current_release_id)
            .cloned()
            .or(planner_version(current_release_id, state).await?);
        if let Some(current) = current {
            selected.push(current);
        }
    }
    for exact_version in exact_versions {
        let already_selected =
            selected.iter().any(|version| version.id == *exact_version);
        if already_selected && !custom_fixed_versions.contains(exact_version) {
            continue;
        }
        let exact = selected
            .iter()
            .find(|version| version.id == *exact_version)
            .cloned()
            .or_else(|| {
                versions
                    .iter()
                    .find(|version| version.id == *exact_version)
                    .cloned()
            })
            .or(planner_version(&(exact_version.clone()), state).await?);
        if let Some(exact) = exact {
            if custom_fixed_versions.contains(exact_version) {
                validate_modrinth_custom_fixed(
                    key,
                    &exact,
                    project_type,
                    target,
                )?;
            } else if exact.project_id != key.project_id {
                continue;
            }
            if !already_selected {
                selected.push(exact);
            }
        } else if custom_fixed_versions.contains(exact_version) {
            return Err(crate::ErrorKind::InputError(format!(
                "Unknown custom fixed Modrinth version {exact_version}"
            ))
            .into());
        }
    }

    let mut candidates = Vec::new();
    for version in selected {
        let installed_current = current_release_id == Some(version.id.as_str());
        let mut dependencies = Vec::new();
        for dependency in &version.dependencies {
            let project_id = match dependency.project_id.clone() {
                Some(project_id) => project_id,
                None => match dependency.version_id.as_deref() {
                    Some(version_id) => planner_version(version_id, state)
                        .await?
                        .map(|version| version.project_id)
                        .unwrap_or_else(|| {
                            format!("missing-version:{version_id}")
                        }),
                    None => continue,
                },
            };
            dependencies.push(CandidateDependency {
                key: NodeKey::new(ContentProvider::Modrinth, project_id),
                version_id: dependency.version_id.clone(),
                kind: match dependency.dependency_type {
                    DependencyType::Required => {
                        CandidateDependencyKind::Required
                    }
                    DependencyType::Optional => {
                        CandidateDependencyKind::Optional
                    }
                    DependencyType::Incompatible => {
                        CandidateDependencyKind::Incompatible
                    }
                    DependencyType::Embedded => {
                        CandidateDependencyKind::Embedded
                    }
                },
            });
        }
        let compatible =
            modrinth_version_matches(&version, project_type, target);
        candidates.push(UpgradeCandidate {
            key: key.clone(),
            version_id: version.id,
            published: version.date_published,
            channel: modrinth_channel(&version.version_type),
            compatible,
            installed_current,
            dependencies,
        });
    }
    sort_candidates(&mut candidates);
    Ok(CandidatePool {
        candidates,
        exploration_limited: exploration_limited || more_releases,
        has_target_game_version_release,
    })
}

fn validate_modrinth_custom_fixed(
    key: &NodeKey,
    version: &Version,
    project_type: ProjectType,
    target: &InstanceUpgradeEnvironment,
) -> crate::Result<()> {
    if version.project_id != key.project_id {
        return Err(crate::ErrorKind::InputError(format!(
            "Custom fixed Modrinth version {} belongs to project {}, not {}",
            version.id, version.project_id, key.project_id
        ))
        .into());
    }
    if !modrinth_version_matches(version, project_type, target) {
        return Err(crate::ErrorKind::InputError(format!(
            "Custom fixed Modrinth version {} is not compatible with the target environment",
            version.id
        ))
        .into());
    }
    Ok(())
}

async fn load_curseforge_candidates(
    key: &NodeKey,
    project_type: ProjectType,
    current_release_id: Option<&str>,
    exact_versions: &HashSet<String>,
    custom_fixed_versions: &HashSet<String>,
    target: &InstanceUpgradeEnvironment,
    state: &State,
) -> crate::Result<CandidatePool> {
    let project_id = key.project_id.parse::<u32>().map_err(|_| {
        crate::ErrorKind::InputError(format!(
            "Invalid CurseForge project ID {}",
            key.project_id
        ))
    })?;
    let (mut files, more_releases): (
        Vec<crate::api::curseforge::CurseForgeFile>,
        bool,
    ) = crate::resources::legacy_catalog::page(
        crate::resources::SourceId::curseforge(),
        project_id.to_string(),
        crate::resources::ipc::ResourceTarget {
            game_versions: vec![target.game_version.clone()],
            loaders: if project_type == ProjectType::Mod {
                match target.mod_loader.as_str() {
                    "forge" | "fabric" | "quilt" | "neoforge" => {
                        vec![target.mod_loader.as_str().into()]
                    }
                    "lite_loader" => vec!["liteloader".into()],
                    _ => vec![],
                }
            } else {
                vec![]
            },
            channels: vec![],
        },
        state,
    )
    .await?;
    let has_target_game_version_release = files.iter().any(|file| {
        file.is_available && curseforge_game_version_matches(file, target)
    });
    files.sort_by(|left, right| {
        curseforge_channel(right.release_type)
            .rank()
            .cmp(&curseforge_channel(left.release_type).rank())
            .then_with(|| right.file_date.cmp(&left.file_date))
            .then_with(|| right.id.cmp(&left.id))
    });
    let (mut selected, exploration_limited) =
        bounded_compatible_candidates(&files, |file| {
            file.is_available
                && curseforge_file_matches(file, project_type, target)
        });
    if let Some(current_release_id) = current_release_id
        && !selected
            .iter()
            .any(|file| file.id.to_string() == current_release_id)
    {
        let current = files
            .iter()
            .find(|file| file.id.to_string() == current_release_id)
            .cloned();
        let current = match current {
            Some(current) => Some(current),
            None => match current_release_id.parse::<u32>() {
                Ok(file_id) => {
                    planner_cf_file(project_id, file_id, state).await?
                }
                Err(_) => None,
            },
        };
        if let Some(current) = current {
            selected.push(current);
        }
    }
    for exact_version in exact_versions {
        let already_selected = selected
            .iter()
            .any(|file| file.id.to_string() == *exact_version);
        if already_selected && !custom_fixed_versions.contains(exact_version) {
            continue;
        }
        let file_id = exact_version.parse::<u32>().map_err(|_| {
            crate::ErrorKind::InputError(format!(
                "Invalid CurseForge file ID {exact_version}"
            ))
        })?;
        let exact =
            match selected.iter().find(|file| file.id == file_id).cloned() {
                Some(file) => file,
                None => planner_cf_file(project_id, file_id, state)
                    .await?
                    .ok_or_else(|| {
                        crate::ErrorKind::InputError(format!(
                            "Unknown CurseForge file ID {exact_version}"
                        ))
                    })?,
            };
        if exact.mod_id != project_id {
            return Err(crate::ErrorKind::InputError(format!(
                "Fixed or exact CurseForge file {exact_version} belongs to project {}, not {project_id}",
                exact.mod_id
            ))
            .into());
        }
        if custom_fixed_versions.contains(exact_version)
            && !curseforge_file_matches(&exact, project_type, target)
        {
            return Err(crate::ErrorKind::InputError(format!(
                "Custom fixed CurseForge file {exact_version} is not compatible with the target environment"
            ))
            .into());
        }
        if !already_selected {
            selected.push(exact);
        }
    }
    let mut candidates = selected
        .into_iter()
        .map(|file| {
            let installed_current = current_release_id
                .is_some_and(|current| current == file.id.to_string());
            UpgradeCandidate {
                key: key.clone(),
                version_id: file.id.to_string(),
                published: DateTime::parse_from_rfc3339(&file.file_date)
                    .map(|date| date.with_timezone(&Utc))
                    .unwrap_or(DateTime::<Utc>::MIN_UTC),
                channel: curseforge_channel(file.release_type),
                compatible: curseforge_file_matches(
                    &file,
                    project_type,
                    target,
                ),
                installed_current,
                dependencies: file
                    .dependencies
                    .into_iter()
                    .filter_map(|dependency| {
                        let kind = match dependency.relation() {
                            DependencyRelation::Required
                            | DependencyRelation::Include => {
                                CandidateDependencyKind::Required
                            }
                            DependencyRelation::Optional => {
                                CandidateDependencyKind::Optional
                            }
                            DependencyRelation::Incompatible => {
                                CandidateDependencyKind::Incompatible
                            }
                            DependencyRelation::Embedded
                            | DependencyRelation::Tool => {
                                CandidateDependencyKind::Embedded
                            }
                            DependencyRelation::Unknown(_) => return None,
                        };
                        Some(CandidateDependency {
                            key: NodeKey::new(
                                ContentProvider::CurseForge,
                                dependency.mod_id.to_string(),
                            ),
                            version_id: None,
                            kind,
                        })
                    })
                    .collect(),
            }
        })
        .collect::<Vec<_>>();
    sort_candidates(&mut candidates);
    Ok(CandidatePool {
        candidates,
        exploration_limited: exploration_limited || more_releases,
        has_target_game_version_release,
    })
}

fn bounded_compatible_candidates<T: Clone>(
    candidates: &[T],
    mut compatible: impl FnMut(&T) -> bool,
) -> (Vec<T>, bool) {
    let mut selected = Vec::new();
    let mut compatible_count = 0;
    for candidate in candidates {
        if !compatible(candidate) {
            continue;
        }
        compatible_count += 1;
        if selected.len() < MAX_CANDIDATES_PER_PROJECT {
            selected.push(candidate.clone());
        }
    }
    (selected, compatible_count > MAX_CANDIDATES_PER_PROJECT)
}

fn compare_modrinth_version(left: &Version, right: &Version) -> Ordering {
    modrinth_channel(&right.version_type)
        .rank()
        .cmp(&modrinth_channel(&left.version_type).rank())
        .then_with(|| right.date_published.cmp(&left.date_published))
        .then_with(|| right.id.cmp(&left.id))
}

fn modrinth_channel(version_type: &str) -> CandidateChannel {
    if version_type.eq_ignore_ascii_case("beta") {
        CandidateChannel::Beta
    } else if version_type.eq_ignore_ascii_case("alpha") {
        CandidateChannel::Alpha
    } else {
        CandidateChannel::Release
    }
}

fn curseforge_channel(release_type: u32) -> CandidateChannel {
    match release_type {
        1 => CandidateChannel::Release,
        2 => CandidateChannel::Beta,
        _ => CandidateChannel::Alpha,
    }
}

fn sort_candidates(candidates: &mut [UpgradeCandidate]) {
    candidates.sort_by(|left, right| {
        right
            .channel
            .rank()
            .cmp(&left.channel.rank())
            .then_with(|| right.published.cmp(&left.published))
            .then_with(|| right.version_id.cmp(&left.version_id))
    });
}

fn modrinth_version_matches(
    version: &Version,
    project_type: ProjectType,
    target: &InstanceUpgradeEnvironment,
) -> bool {
    if !version
        .game_versions
        .iter()
        .any(|game_version| game_version == &target.game_version)
    {
        return false;
    }
    match project_type {
        ProjectType::Mod => version.loaders.iter().any(|loader| {
            loader.eq_ignore_ascii_case(target.mod_loader.as_str())
        }),
        ProjectType::ShaderPack => shader_loader_matches(
            version.loaders.iter().map(String::as_str),
            target.shader_runtime,
        ),
        ProjectType::DataPack => version
            .loaders
            .iter()
            .any(|loader| loader.eq_ignore_ascii_case("datapack")),
        ProjectType::ResourcePack => version.loaders.iter().any(|loader| {
            loader.eq_ignore_ascii_case("minecraft")
                || loader.eq_ignore_ascii_case("vanilla")
        }),
        ProjectType::Schematic | ProjectType::WorldSave => false,
    }
}

fn curseforge_file_matches(
    file: &crate::api::curseforge::CurseForgeFile,
    project_type: ProjectType,
    target: &InstanceUpgradeEnvironment,
) -> bool {
    let game_version_matches = curseforge_game_version_matches(file, target);
    if !game_version_matches {
        return false;
    }
    match project_type {
        ProjectType::Mod => file.game_versions.iter().any(|value| {
            value.eq_ignore_ascii_case(target.mod_loader.as_str())
        }),
        ProjectType::ShaderPack => shader_loader_matches(
            file.game_versions.iter().map(String::as_str),
            target.shader_runtime,
        ),
        ProjectType::DataPack | ProjectType::ResourcePack => true,
        ProjectType::Schematic | ProjectType::WorldSave => false,
    }
}

fn curseforge_game_version_matches(
    file: &crate::api::curseforge::CurseForgeFile,
    target: &InstanceUpgradeEnvironment,
) -> bool {
    file.game_versions
        .iter()
        .any(|value| value == &target.game_version)
        || file.sortable_game_versions.iter().any(|value| {
            value.game_version.as_deref() == Some(&target.game_version)
                || value.game_version_name == target.game_version
        })
}

fn shader_loader_matches<'a>(
    loaders: impl Iterator<Item = &'a str>,
    runtime: ShaderRuntime,
) -> bool {
    let expected = match runtime {
        ShaderRuntime::Iris => "iris",
        ShaderRuntime::OptiFine => "optifine",
        ShaderRuntime::None | ShaderRuntime::Unknown => return false,
    };
    loaders.into_iter().any(|loader| {
        loader.eq_ignore_ascii_case(expected)
            || runtime == ShaderRuntime::OptiFine
                && loader.eq_ignore_ascii_case("optifine")
    })
}

fn classify_items(
    items: &mut [InstanceUpgradeItem],
    installed: &[InstalledNode],
    catalog: &UpgradeCatalog,
    target: &InstanceUpgradeEnvironment,
) {
    for item in items {
        if matches!(
            item.status,
            InstanceUpgradeItemStatus::Unidentified
                | InstanceUpgradeItemStatus::UnsupportedContentType
        ) {
            continue;
        }
        if item.project_type == ProjectType::ShaderPack {
            match target.shader_runtime {
                ShaderRuntime::None => {
                    item.status =
                        InstanceUpgradeItemStatus::ShaderRuntimeMissing;
                    if item.resolution.action == InstanceUpgradeAction::Upgrade
                    {
                        item.resolution.action = InstanceUpgradeAction::Keep;
                    }
                    continue;
                }
                ShaderRuntime::Unknown => {
                    item.status =
                        InstanceUpgradeItemStatus::ShaderRuntimeUnknown;
                    if item.resolution.action == InstanceUpgradeAction::Upgrade
                    {
                        item.resolution.action = InstanceUpgradeAction::Keep;
                    }
                    continue;
                }
                ShaderRuntime::Iris | ShaderRuntime::OptiFine => {}
            }
        }
        let Some(node) = installed
            .iter()
            .find(|node| node.content_id == item.content_id)
        else {
            continue;
        };
        let pool = catalog.get(&node.key);
        let candidates =
            pool.map(|pool| pool.candidates.as_slice()).unwrap_or(&[]);
        let compatible = candidates
            .iter()
            .filter(|candidate| candidate.compatible)
            .collect::<Vec<_>>();
        item.candidate_release_ids = compatible
            .iter()
            .map(|candidate| candidate.version_id.clone())
            .collect();
        let has_stable = compatible
            .iter()
            .any(|candidate| candidate.channel == CandidateChannel::Release);
        let current_compatible = compatible
            .iter()
            .any(|candidate| candidate.version_id == node.current_release_id);
        item.status = if has_stable {
            if current_compatible {
                InstanceUpgradeItemStatus::AlreadyCompatible
            } else {
                InstanceUpgradeItemStatus::UpgradeAvailable
            }
        } else if !compatible.is_empty() {
            InstanceUpgradeItemStatus::PrereleaseOnly
        } else if item.project_type == ProjectType::ShaderPack
            && pool.is_some_and(|pool| pool.has_target_game_version_release)
        {
            InstanceUpgradeItemStatus::NoCompatibleShaderRuntime
        } else {
            InstanceUpgradeItemStatus::NoCompatibleRelease
        };
    }
}

fn roots_from_items(
    items: &[InstanceUpgradeItem],
    installed: &[InstalledNode],
) -> Vec<RootRequest> {
    items
        .iter()
        .filter_map(|item| {
            let node = installed.iter().find(|node| {
                node.content_id == item.content_id
                    && !node.auto_dependency
                    && node.migratable
            })?;
            Some(RootRequest {
                content_id: item.content_id.clone(),
                key: node.key.clone(),
                current_release_id: node.current_release_id.clone(),
                enabled: node.enabled,
                action: item.resolution.action,
                allow_prerelease: item.resolution.allow_prerelease,
            })
        })
        .collect()
}

fn confirmed_prereleases(
    items: &[InstanceUpgradeItem],
) -> HashSet<(NodeKey, String)> {
    items
        .iter()
        .flat_map(|item| {
            item.resolution.confirmed_prerelease_dependencies.iter()
        })
        .map(|confirmation| {
            (
                NodeKey::new(confirmation.provider, &confirmation.project_id),
                confirmation.version_id.clone(),
            )
        })
        .collect()
}

#[cfg(test)]
fn solve_upgrade(
    roots: &[RootRequest],
    installed: &[InstalledNode],
    catalog: &UpgradeCatalog,
    fixed: &HashMap<NodeKey, String>,
    confirmed_prereleases: &HashSet<(NodeKey, String)>,
) -> SolveOutcome {
    let fixed = FixedRootConstraints::from_project_versions(roots, fixed);
    solve_upgrade_with_fixed_roots(
        roots,
        installed,
        catalog,
        &fixed,
        confirmed_prereleases,
    )
}

fn solve_upgrade_with_fixed_roots(
    roots: &[RootRequest],
    installed: &[InstalledNode],
    catalog: &UpgradeCatalog,
    fixed: &FixedRootConstraints,
    confirmed_prereleases: &HashSet<(NodeKey, String)>,
) -> SolveOutcome {
    let aliases = InstalledAliasIndex::new(installed);
    let newest = solve_for_strategy_with_fixed_roots(
        SolveStrategy::Newest,
        roots,
        catalog,
        fixed,
        confirmed_prereleases,
        &aliases,
    );
    let minimal = solve_for_strategy_with_fixed_roots(
        SolveStrategy::MinimalChange,
        roots,
        catalog,
        fixed,
        confirmed_prereleases,
        &aliases,
    );
    let candidate_issues = [newest.issue.clone(), minimal.issue.clone()];
    let mut solutions = Vec::new();
    if fixed.is_empty() {
        if let Some(solution) = newest.solution {
            push_unique_solution(&mut solutions, solution);
        }
        if let Some(solution) = minimal.solution {
            push_unique_solution(&mut solutions, solution);
        }
    } else {
        if let Some(solution) = minimal.solution {
            push_unique_solution(&mut solutions, solution);
        }
        if let Some(solution) = newest.solution {
            push_unique_solution(&mut solutions, solution);
        }
    }
    let issues = if solutions.is_empty() {
        vec![select_upgrade_failure_issue(candidate_issues, fixed)]
    } else {
        Vec::new()
    };
    SolveOutcome {
        solutions,
        issues,
        #[cfg(test)]
        visited_states: newest.visited_states + minimal.visited_states,
    }
}

fn select_upgrade_failure_issue(
    candidate_issues: [Option<InstanceUpgradeIssue>; 2],
    fixed: &FixedRootConstraints,
) -> InstanceUpgradeIssue {
    candidate_issues
        .iter()
        .flatten()
        .find(|issue| proven_fixed_exact_conflict(issue, fixed))
        .cloned()
        .or_else(|| {
            candidate_issues
                .iter()
                .flatten()
                .find(|issue| {
                    issue.code == InstanceUpgradeIssueCode::SearchLimitReached
                })
                .cloned()
        })
        .or_else(|| candidate_issues.into_iter().flatten().next())
        .unwrap_or_else(|| {
            issue(
                InstanceUpgradeIssueCode::DependencyConflict,
                "No globally compatible dependency solution exists",
                None,
                None,
                None,
            )
        })
}

fn proven_fixed_exact_conflict(
    issue: &InstanceUpgradeIssue,
    fixed: &FixedRootConstraints,
) -> bool {
    if issue.code != InstanceUpgradeIssueCode::DependencyConflict {
        return false;
    }
    let exact_requirements = issue
        .dependency_requirements
        .iter()
        .filter(|requirement| requirement.required_release_id.is_some())
        .collect::<Vec<_>>();
    if exact_requirements.is_empty()
        || exact_requirements.iter().any(|requirement| {
            !fixed.contains_content(&requirement.root_content_id)
        })
    {
        return false;
    }
    let mut versions_by_dependency = HashMap::<NodeKey, HashSet<&str>>::new();
    for requirement in exact_requirements {
        versions_by_dependency
            .entry(NodeKey::new(
                requirement.dependency_provider,
                &requirement.dependency_project_id,
            ))
            .or_default()
            .insert(requirement.required_release_id.as_deref().unwrap());
    }
    versions_by_dependency
        .values()
        .any(|versions| versions.len() > 1)
}

fn selected_root_exact_dependency_conflict(
    roots: &[RootRequest],
    options: &[RootCandidateOptions],
    selected: &[usize],
    fixed: &FixedRootConstraints,
) -> Option<InstanceUpgradeIssue> {
    let mut requirements_by_dependency =
        HashMap::<NodeKey, Vec<InstanceUpgradeDependencyRequirement>>::new();
    for ((root, options), candidate_index) in
        roots.iter().zip(options).zip(selected)
    {
        let Some(candidate) = options.candidates[*candidate_index].as_ref()
        else {
            continue;
        };
        for dependency in &candidate.dependencies {
            let Some(required_release_id) = dependency.version_id.clone()
            else {
                continue;
            };
            if dependency.kind != CandidateDependencyKind::Required {
                continue;
            }
            requirements_by_dependency
                .entry(dependency.key.clone())
                .or_default()
                .push(InstanceUpgradeDependencyRequirement {
                    root_content_id: root.content_id.clone(),
                    root_provider: root.key.provider,
                    root_project_id: root.key.project_id.clone(),
                    parent_provider: candidate.key.provider,
                    parent_project_id: candidate.key.project_id.clone(),
                    parent_release_id: candidate.version_id.clone(),
                    dependency_provider: dependency.key.provider,
                    dependency_project_id: dependency.key.project_id.clone(),
                    required_release_id: Some(required_release_id),
                    candidate_release_id: None,
                });
        }
    }
    for (dependency, requirements) in requirements_by_dependency {
        let versions = requirements
            .iter()
            .filter_map(|requirement| requirement.required_release_id.as_ref())
            .collect::<HashSet<_>>();
        if versions.len() < 2 {
            continue;
        }
        if requirements.iter().any(|requirement| {
            !fixed.contains_content(&requirement.root_content_id)
        }) {
            continue;
        }
        let mut versions =
            versions.into_iter().map(String::as_str).collect::<Vec<_>>();
        versions.sort_unstable();
        return Some(issue_with_requirements(
            InstanceUpgradeIssueCode::DependencyConflict,
            format!(
                "Contradictory exact requirements for {}: {}",
                dependency.label(),
                versions.join(", ")
            ),
            Some(&dependency),
            Some(&dependency.project_id),
            None,
            requirements,
        ));
    }
    None
}

#[cfg(test)]
fn solve_for_strategy(
    strategy: SolveStrategy,
    roots: &[RootRequest],
    catalog: &UpgradeCatalog,
    fixed: &HashMap<NodeKey, String>,
    confirmed_prereleases: &HashSet<(NodeKey, String)>,
    aliases: &InstalledAliasIndex,
) -> StrategySolveOutcome {
    let fixed = FixedRootConstraints::from_project_versions(roots, fixed);
    solve_for_strategy_with_fixed_roots(
        strategy,
        roots,
        catalog,
        &fixed,
        confirmed_prereleases,
        aliases,
    )
}

fn solve_for_strategy_with_fixed_roots(
    strategy: SolveStrategy,
    roots: &[RootRequest],
    catalog: &UpgradeCatalog,
    fixed: &FixedRootConstraints,
    confirmed_prereleases: &HashSet<(NodeKey, String)>,
    aliases: &InstalledAliasIndex,
) -> StrategySolveOutcome {
    solve_for_strategy_with_fixed_roots_and_limit(
        strategy,
        roots,
        catalog,
        fixed,
        confirmed_prereleases,
        aliases,
        MAX_SEARCH_STATES,
    )
}

#[cfg(test)]
fn solve_for_strategy_with_limit(
    strategy: SolveStrategy,
    roots: &[RootRequest],
    catalog: &UpgradeCatalog,
    fixed: &HashMap<NodeKey, String>,
    confirmed_prereleases: &HashSet<(NodeKey, String)>,
    aliases: &InstalledAliasIndex,
    max_search_states: usize,
) -> StrategySolveOutcome {
    let fixed = FixedRootConstraints::from_project_versions(roots, fixed);
    solve_for_strategy_with_fixed_roots_and_limit(
        strategy,
        roots,
        catalog,
        &fixed,
        confirmed_prereleases,
        aliases,
        max_search_states,
    )
}

fn solve_for_strategy_with_fixed_roots_and_limit(
    strategy: SolveStrategy,
    roots: &[RootRequest],
    catalog: &UpgradeCatalog,
    fixed: &FixedRootConstraints,
    confirmed_prereleases: &HashSet<(NodeKey, String)>,
    aliases: &InstalledAliasIndex,
    max_search_states: usize,
) -> StrategySolveOutcome {
    let options = roots
        .iter()
        .map(|root| {
            root_candidate_options(
                root,
                strategy,
                catalog,
                fixed,
                confirmed_prereleases,
            )
        })
        .collect::<Vec<_>>();
    if let Some((root, _)) = roots
        .iter()
        .zip(&options)
        .find(|(_, options)| options.candidates.is_empty())
    {
        return StrategySolveOutcome {
            solution: None,
            issue: Some(unavailable_root_issue(root, catalog)),
            #[cfg(test)]
            visited_states: 0,
        };
    }

    let initial = vec![0; options.len()];
    let mut frontier =
        BinaryHeap::from([frontier_state(initial.clone(), &options, strategy)]);
    let mut queued = HashSet::from([initial]);
    let mut total_visited = 0;
    let mut best_failure: Option<ConflictFailure> = None;

    while let Some(FrontierState { selected, .. }) = frontier.pop() {
        let mut requirements = roots
            .iter()
            .zip(&options)
            .zip(&selected)
            .map(|((root, options), candidate_index)| Requirement {
                key: root.key.clone(),
                version_id: options.candidates[*candidate_index]
                    .as_ref()
                    .map(|candidate| candidate.version_id.clone()),
                explicit_prerelease: options.fixed,
                preserve_unsafe: root.action != InstanceUpgradeAction::Upgrade,
                root_content_id: root.content_id.clone(),
                root_key: root.key.clone(),
                origins: Vec::new(),
            })
            .collect::<Vec<_>>();
        requirements.sort_by_key(|requirement| {
            std::cmp::Reverse(requirement.preserve_unsafe)
        });
        let mut state = SearchState {
            visited: total_visited,
            ..SearchState::default()
        };
        let mut solutions = Vec::new();
        if let Some(conflict) = selected_root_exact_dependency_conflict(
            roots, &options, &selected, fixed,
        ) {
            record_issue(&mut state, conflict);
        } else {
            search_solutions(
                requirements,
                HashMap::new(),
                HashMap::new(),
                HashMap::new(),
                HashSet::new(),
                HashSet::new(),
                roots,
                catalog,
                confirmed_prereleases,
                aliases,
                strategy,
                max_search_states,
                &mut state,
                &mut solutions,
            );
        }
        total_visited = state.visited;
        if let Some(solution) = solutions.into_iter().next() {
            return StrategySolveOutcome {
                solution: Some(solution),
                issue: None,
                #[cfg(test)]
                visited_states: total_visited,
            };
        }
        if state.limit_reached {
            return StrategySolveOutcome {
                solution: None,
                issue: Some(search_limit_issue(true)),
                #[cfg(test)]
                visited_states: total_visited,
            };
        }
        let branch_issue = state.first_issue.unwrap_or_else(|| {
            issue(
                InstanceUpgradeIssueCode::DependencyConflict,
                "No globally compatible dependency solution exists",
                None,
                None,
                None,
            )
        });
        let mut conflict = conflict_set_from_issue(&branch_issue, roots);
        let mut branched = false;
        for (index, root_options) in options.iter().enumerate() {
            if root_options.fixed || !conflict.can_branch_root(root_options) {
                continue;
            }
            let next_index = selected[index] + 1;
            if next_index < root_options.candidates.len() {
                let mut alternative = selected.clone();
                alternative[index] = next_index;
                if queued.insert(alternative.clone()) {
                    frontier.push(frontier_state(
                        alternative,
                        &options,
                        strategy,
                    ));
                    branched = true;
                }
            } else if root_options.exploration_limited {
                conflict
                    .candidate_limit_roots
                    .insert(root_options.content_id.clone());
            }
        }
        retain_best_failure(
            &mut best_failure,
            ConflictFailure {
                issue: branch_issue,
                conflict,
            },
        );
        if !branched && frontier.is_empty() {
            break;
        }
    }

    StrategySolveOutcome {
        solution: None,
        issue: Some(match best_failure {
            Some(failure) if failure.conflict.candidate_search_incomplete() => {
                search_limit_issue(false)
            }
            Some(failure) => failure.issue,
            None => issue(
                InstanceUpgradeIssueCode::DependencyConflict,
                "No globally compatible dependency solution exists",
                None,
                None,
                None,
            ),
        }),
        #[cfg(test)]
        visited_states: total_visited,
    }
}

fn unavailable_root_issue(
    root: &RootRequest,
    catalog: &UpgradeCatalog,
) -> InstanceUpgradeIssue {
    let prerelease_only = catalog
        .get(&root.key)
        .into_iter()
        .flat_map(|pool| &pool.candidates)
        .any(|candidate| {
            candidate.compatible && candidate.channel.is_prerelease()
        });
    issue(
        if prerelease_only {
            InstanceUpgradeIssueCode::PrereleaseOnly
        } else {
            InstanceUpgradeIssueCode::NoCompatibleRelease
        },
        format!(
            "No compatible release satisfies root project {}",
            root.key.label()
        ),
        Some(&root.key),
        Some(&root.key.project_id),
        None,
    )
}

fn root_candidate_options(
    root: &RootRequest,
    strategy: SolveStrategy,
    catalog: &UpgradeCatalog,
    fixed: &FixedRootConstraints,
    confirmed_prereleases: &HashSet<(NodeKey, String)>,
) -> RootCandidateOptions {
    let requirement = Requirement {
        key: root.key.clone(),
        version_id: fixed.version_for_root(root).map(str::to_string),
        explicit_prerelease: fixed.is_fixed_root(root),
        preserve_unsafe: root.action != InstanceUpgradeAction::Upgrade,
        root_content_id: root.content_id.clone(),
        root_key: root.key.clone(),
        origins: Vec::new(),
    };
    let mut candidates = candidates_for_requirement(
        &requirement,
        std::slice::from_ref(root),
        catalog,
        confirmed_prereleases,
    )
    .into_iter()
    .cloned()
    .map(Some)
    .collect::<Vec<_>>();
    if strategy == SolveStrategy::MinimalChange {
        candidates.sort_by_key(|candidate| {
            std::cmp::Reverse(candidate.as_ref().is_some_and(|candidate| {
                candidate.version_id == root.current_release_id
            }))
        });
    }
    if candidates.is_empty() && requirement.preserve_unsafe {
        candidates.push(None);
    }
    RootCandidateOptions {
        content_id: root.content_id.clone(),
        key: root.key.clone(),
        current_release_id: root.current_release_id.clone(),
        fixed: fixed.is_fixed_root(root),
        exploration_limited: !requirement.preserve_unsafe
            && requirement.version_id.is_none()
            && catalog
                .get(&root.key)
                .is_some_and(|pool| pool.exploration_limited),
        candidates,
    }
}

fn frontier_state(
    selected: Vec<usize>,
    options: &[RootCandidateOptions],
    strategy: SolveStrategy,
) -> FrontierState {
    let candidates = options
        .iter()
        .zip(&selected)
        .filter_map(|(options, index)| options.candidates[*index].as_ref());
    let mut minimum_channel = None;
    let mut freshness_score = 0;
    for candidate in candidates {
        minimum_channel = Some(
            minimum_channel.map_or(candidate.channel.rank(), |rank: u8| {
                rank.min(candidate.channel.rank())
            }),
        );
        freshness_score += candidate.published.timestamp();
    }
    let primary_score = match strategy {
        SolveStrategy::Newest => i64::from(minimum_channel.unwrap_or(0)),
        SolveStrategy::MinimalChange => -i64::try_from(
            options
                .iter()
                .zip(&selected)
                .filter(|(options, index)| {
                    options.candidates[**index].as_ref().is_some_and(
                        |candidate| {
                            candidate.version_id != options.current_release_id
                        },
                    )
                })
                .count(),
        )
        .unwrap_or(i64::MIN),
    };
    FrontierState {
        selected,
        primary_score,
        freshness_score,
    }
}

fn conflict_set_from_issue(
    issue: &InstanceUpgradeIssue,
    roots: &[RootRequest],
) -> ConflictSet {
    let mut involved_root_content_ids = issue
        .dependency_requirements
        .iter()
        .filter(|requirement| is_causal_dependency_requirement(requirement))
        .map(|requirement| requirement.root_content_id.clone())
        .collect::<HashSet<_>>();
    if let Some(content_id) = &issue.content_id {
        involved_root_content_ids.insert(content_id.clone());
    }
    let involved_parent_projects = issue
        .dependency_requirements
        .iter()
        .filter(|requirement| is_causal_dependency_requirement(requirement))
        .map(|requirement| {
            NodeKey::new(
                requirement.parent_provider,
                &requirement.parent_project_id,
            )
        })
        .collect();
    let dependency_project = issue
        .provider
        .zip(issue.project_id.as_ref())
        .map(|(provider, project_id)| NodeKey::new(provider, project_id));
    if involved_root_content_ids.is_empty() {
        for root in roots {
            if dependency_project.as_ref() == Some(&root.key)
                || issue.project_id.as_deref()
                    == Some(root.key.project_id.as_str())
                || issue.conflicting_project_id.as_deref()
                    == Some(root.key.project_id.as_str())
            {
                involved_root_content_ids.insert(root.content_id.clone());
            }
        }
    }
    ConflictSet {
        involved_root_content_ids,
        involved_parent_projects,
        dependency_project,
        reason: issue.code,
        candidate_limit_roots: HashSet::new(),
    }
}

fn is_causal_dependency_requirement(
    requirement: &InstanceUpgradeDependencyRequirement,
) -> bool {
    requirement.parent_provider != requirement.dependency_provider
        || requirement.parent_project_id != requirement.dependency_project_id
}

fn contradictory_exact_requirement_issue(
    requirement: &Requirement,
    requirements: &[Requirement],
) -> Option<InstanceUpgradeIssue> {
    if requirement.origins.is_empty() || requirement.version_id.is_none() {
        return None;
    }
    let matching_requirements = std::iter::once(requirement)
        .chain(requirements)
        .filter(|exact_requirement| {
            exact_requirement.key == requirement.key
                && !exact_requirement.origins.is_empty()
                && exact_requirement.version_id.is_some()
        })
        .collect::<Vec<_>>();
    let has_safe_requirement = matching_requirements
        .iter()
        .any(|requirement| !requirement.preserve_unsafe);
    let mut exact_versions = HashSet::new();
    let mut details = Vec::new();
    for exact_requirement in matching_requirements {
        if has_safe_requirement && exact_requirement.preserve_unsafe {
            continue;
        }
        let Some(version_id) = exact_requirement.version_id.as_ref() else {
            continue;
        };
        exact_versions.insert(version_id.clone());
        for origin in &exact_requirement.origins {
            if !details.contains(origin) {
                details.push(origin.clone());
            }
        }
    }
    if exact_versions.len() < 2 {
        return None;
    }
    let mut exact_versions = exact_versions.into_iter().collect::<Vec<_>>();
    exact_versions.sort();
    Some(issue_with_requirements(
        InstanceUpgradeIssueCode::DependencyConflict,
        format!(
            "Contradictory exact requirements for {}: {}",
            requirement.key.label(),
            exact_versions.join(", ")
        ),
        Some(&requirement.key),
        Some(&requirement.key.project_id),
        None,
        details,
    ))
}

fn search_limit_issue(global_limit: bool) -> InstanceUpgradeIssue {
    issue(
        InstanceUpgradeIssueCode::SearchLimitReached,
        if global_limit {
            "Upgrade dependency search reached its global state limit"
        } else {
            "Upgrade dependency search exhausted its bounded candidate exploration and cannot prove the plan is unsatisfiable"
        },
        None,
        None,
        None,
    )
}

fn push_unique_solution(
    solutions: &mut Vec<SolverResult>,
    candidate: SolverResult,
) {
    let duplicate = solutions.iter().any(|solution| {
        solution.assignments.len() == candidate.assignments.len()
            && solution.assignments.iter().all(|(key, selected)| {
                candidate.assignments.get(key).is_some_and(|other| {
                    other.version_id == selected.version_id
                })
            })
            && solution.preserved_unsafe == candidate.preserved_unsafe
    });
    if !duplicate {
        solutions.push(candidate);
    }
}

fn search_solutions(
    mut requirements: Vec<Requirement>,
    assignments: HashMap<NodeKey, UpgradeCandidate>,
    physical_assignments: HashMap<String, NodeKey>,
    assignment_origins: HashMap<
        NodeKey,
        Vec<InstanceUpgradeDependencyRequirement>,
    >,
    expanded_origins: HashSet<(PhysicalNodeIdentity, String)>,
    preserved_unsafe: HashSet<NodeKey>,
    roots: &[RootRequest],
    catalog: &UpgradeCatalog,
    confirmed_prereleases: &HashSet<(NodeKey, String)>,
    aliases: &InstalledAliasIndex,
    strategy: SolveStrategy,
    max_search_states: usize,
    state: &mut SearchState,
    solutions: &mut Vec<SolverResult>,
) {
    if !solutions.is_empty() {
        return;
    }
    if state.visited >= max_search_states {
        state.limit_reached = true;
        return;
    }
    state.visited += 1;
    let Some(mut requirement) = requirements.pop() else {
        solutions.push(SolverResult {
            assignments,
            physical_assignments,
            preserved_unsafe,
        });
        return;
    };
    coalesce_requirement_origins(&mut requirement, &mut requirements, aliases);
    if let Some(conflict) =
        contradictory_exact_requirement_issue(&requirement, &requirements)
    {
        record_issue(state, conflict);
        return;
    }
    if let Some(selected_key) = assigned_key_for_requirement(
        &requirement.key,
        &assignments,
        &physical_assignments,
        aliases,
    ) {
        let selected = &assignments[selected_key];
        let exact_matches = exact_requirement_matches_assignment(
            &requirement,
            selected_key,
            selected,
            aliases,
        );
        let safe_assignment_satisfies_unsafe = requirement.preserve_unsafe
            && !preserved_unsafe.contains(selected_key);
        if exact_matches || safe_assignment_satisfies_unsafe {
            let mut next_origins = assignment_origins;
            next_origins
                .entry(selected_key.clone())
                .or_default()
                .extend(requirement.origins.clone());
            let mut next_expanded_origins = expanded_origins;
            extend_required_requirements(
                &mut requirements,
                selected,
                &requirement,
                &mut next_expanded_origins,
                aliases,
            );
            search_solutions(
                requirements,
                assignments,
                physical_assignments,
                next_origins,
                next_expanded_origins,
                preserved_unsafe,
                roots,
                catalog,
                confirmed_prereleases,
                aliases,
                strategy,
                max_search_states,
                state,
                solutions,
            );
        } else {
            let mut details = assignment_origins
                .get(selected_key)
                .cloned()
                .unwrap_or_default();
            details.extend(requirement.origins.clone());
            for detail in &mut details {
                if detail.candidate_release_id.is_none() {
                    detail.candidate_release_id =
                        Some(selected.version_id.clone());
                }
            }
            let selected_root = roots.iter().find(|root| {
                &root.key == selected_key
                    || aliases.same_physical_content(&root.key, selected_key)
            });
            let mut conflict = issue_with_requirements(
                InstanceUpgradeIssueCode::DependencyConflict,
                format!(
                    "{} requires an exact provider release that cannot be proven equivalent to selected {}",
                    requirement.key.label(),
                    selected_key.label()
                ),
                Some(&requirement.key),
                Some(&requirement.key.project_id),
                None,
                details,
            );
            conflict.content_id =
                selected_root.map(|root| root.content_id.clone());
            record_issue(state, conflict);
        }
        return;
    }

    let candidates = candidates_for_requirement(
        &requirement,
        roots,
        catalog,
        confirmed_prereleases,
    );
    let mut candidates = candidates;
    if strategy == SolveStrategy::MinimalChange
        && !roots.iter().any(|root| root.key == requirement.key)
    {
        candidates.sort_by_key(|candidate| {
            std::cmp::Reverse(candidate.installed_current)
        });
    }
    if candidates.is_empty() {
        let prerelease_candidate = catalog
            .get(&requirement.key)
            .into_iter()
            .flat_map(|pool| &pool.candidates)
            .find(|candidate| {
                candidate.compatible
                    && candidate.channel.is_prerelease()
                    && requirement.version_id.as_ref().is_none_or(
                        |version_id| version_id == &candidate.version_id,
                    )
            });
        if requirement.preserve_unsafe {
            search_solutions(
                requirements,
                assignments,
                physical_assignments,
                assignment_origins,
                expanded_origins,
                preserved_unsafe,
                roots,
                catalog,
                confirmed_prereleases,
                aliases,
                strategy,
                max_search_states,
                state,
                solutions,
            );
            return;
        }
        let root_requirement =
            roots.iter().any(|root| root.key == requirement.key);
        let code = if prerelease_candidate.is_some() {
            InstanceUpgradeIssueCode::PrereleaseOnly
        } else if root_requirement {
            InstanceUpgradeIssueCode::NoCompatibleRelease
        } else {
            InstanceUpgradeIssueCode::MissingRequiredDependency
        };
        let mut details = requirement.origins.clone();
        if let Some(candidate) = prerelease_candidate {
            for detail in &mut details {
                detail.candidate_release_id =
                    Some(candidate.version_id.clone());
            }
        }
        record_issue(
            state,
            issue_with_requirements(
                code,
                format!(
                    "No compatible release satisfies required project {}",
                    requirement.key.label()
                ),
                Some(&requirement.key),
                Some(&requirement.key.project_id),
                None,
                details,
            ),
        );
        return;
    }
    for candidate in candidates {
        if let Some(conflict) = incompatible_with_assignments(
            candidate,
            &assignments,
            &physical_assignments,
            aliases,
        ) {
            let mut details = requirement.origins.clone();
            details.extend(
                assignment_origins
                    .get(&conflict)
                    .cloned()
                    .unwrap_or_default(),
            );
            details.push(InstanceUpgradeDependencyRequirement {
                root_content_id: requirement.root_content_id.clone(),
                root_provider: requirement.root_key.provider,
                root_project_id: requirement.root_key.project_id.clone(),
                parent_provider: candidate.key.provider,
                parent_project_id: candidate.key.project_id.clone(),
                parent_release_id: candidate.version_id.clone(),
                dependency_provider: conflict.provider,
                dependency_project_id: conflict.project_id.clone(),
                required_release_id: None,
                candidate_release_id: assignments
                    .get(&conflict)
                    .map(|selected| selected.version_id.clone()),
            });
            if let Some(conflicting_root) =
                roots.iter().find(|root| root.key == conflict)
                && let Some(selected) = assignments.get(&conflict)
            {
                details.push(InstanceUpgradeDependencyRequirement {
                    root_content_id: conflicting_root.content_id.clone(),
                    root_provider: conflicting_root.key.provider,
                    root_project_id: conflicting_root.key.project_id.clone(),
                    parent_provider: selected.key.provider,
                    parent_project_id: selected.key.project_id.clone(),
                    parent_release_id: selected.version_id.clone(),
                    dependency_provider: candidate.key.provider,
                    dependency_project_id: candidate.key.project_id.clone(),
                    required_release_id: None,
                    candidate_release_id: Some(candidate.version_id.clone()),
                });
            }
            record_issue(
                state,
                issue_with_requirements(
                    InstanceUpgradeIssueCode::IncompatibleDependency,
                    format!(
                        "{} is incompatible with {}",
                        candidate.key.label(),
                        conflict.label()
                    ),
                    Some(&candidate.key),
                    Some(&candidate.key.project_id),
                    Some(&conflict.project_id),
                    details,
                ),
            );
            continue;
        }
        let mut next_assignments = assignments.clone();
        next_assignments.insert(candidate.key.clone(), candidate.clone());
        let mut next_physical_assignments = physical_assignments.clone();
        if let Some(content_id) = aliases.content_id(&candidate.key) {
            next_physical_assignments
                .insert(content_id.to_string(), candidate.key.clone());
        }
        let mut next_origins = assignment_origins.clone();
        next_origins.insert(candidate.key.clone(), requirement.origins.clone());
        let mut next_expanded_origins = expanded_origins.clone();
        let mut next_preserved = preserved_unsafe.clone();
        if requirement.preserve_unsafe {
            next_preserved.insert(candidate.key.clone());
        }
        let mut next_requirements = requirements.clone();
        extend_required_requirements(
            &mut next_requirements,
            candidate,
            &requirement,
            &mut next_expanded_origins,
            aliases,
        );
        search_solutions(
            next_requirements,
            next_assignments,
            next_physical_assignments,
            next_origins,
            next_expanded_origins,
            next_preserved,
            roots,
            catalog,
            confirmed_prereleases,
            aliases,
            strategy,
            max_search_states,
            state,
            solutions,
        );
        if !solutions.is_empty() {
            return;
        }
    }
}

fn candidates_for_requirement<'a>(
    requirement: &Requirement,
    roots: &[RootRequest],
    catalog: &'a UpgradeCatalog,
    confirmed_prereleases: &HashSet<(NodeKey, String)>,
) -> Vec<&'a UpgradeCandidate> {
    let root = roots.iter().find(|root| root.key == requirement.key);

    catalog
        .get(&requirement.key)
        .into_iter()
        .flat_map(|pool| &pool.candidates)
        .filter(|candidate| {
            if let Some(version_id) = requirement.version_id.as_deref()
                && candidate.version_id != version_id
            {
                return false;
            }
            match root {
                Some(root)
                    if matches!(
                        root.action,
                        InstanceUpgradeAction::Keep
                            | InstanceUpgradeAction::Disable
                    ) =>
                {
                    candidate.version_id == root.current_release_id
                }
                Some(root) => {
                    candidate.compatible
                        && (!candidate.channel.is_prerelease()
                            || root.allow_prerelease
                            || requirement.explicit_prerelease)
                }
                None => {
                    if requirement.preserve_unsafe {
                        candidate.installed_current
                    } else {
                        candidate.compatible
                            && (!candidate.channel.is_prerelease()
                                || confirmed_prereleases.contains(&(
                                    candidate.key.clone(),
                                    candidate.version_id.clone(),
                                )))
                    }
                }
            }
        })
        .collect::<Vec<_>>()
}

fn assigned_key_for_requirement<'a>(
    key: &NodeKey,
    assignments: &'a HashMap<NodeKey, UpgradeCandidate>,
    physical_assignments: &'a HashMap<String, NodeKey>,
    aliases: &InstalledAliasIndex,
) -> Option<&'a NodeKey> {
    if let Some((selected_key, _)) = assignments.get_key_value(key) {
        return Some(selected_key);
    }
    aliases
        .content_id(key)
        .and_then(|content_id| physical_assignments.get(content_id))
}

fn exact_requirement_matches_assignment(
    requirement: &Requirement,
    selected_key: &NodeKey,
    selected: &UpgradeCandidate,
    aliases: &InstalledAliasIndex,
) -> bool {
    provider_constraint_matches_assignment(
        &requirement.key,
        requirement.version_id.as_deref(),
        selected_key,
        selected,
        aliases,
    )
}

fn provider_constraint_matches_assignment(
    constraint_key: &NodeKey,
    exact_version: Option<&str>,
    assigned_key: &NodeKey,
    assigned: &UpgradeCandidate,
    aliases: &InstalledAliasIndex,
) -> bool {
    let Some(exact_version) = exact_version else {
        return constraint_key == assigned_key
            || aliases.same_physical_content(constraint_key, assigned_key);
    };
    if constraint_key == assigned_key {
        return assigned.version_id == exact_version;
    }
    if !aliases.same_physical_content(constraint_key, assigned_key)
        || !assigned.installed_current
    {
        return false;
    }
    let Some(content_id) = aliases.content_id(constraint_key) else {
        return false;
    };
    aliases.current_release(content_id, constraint_key) == Some(exact_version)
}

fn coalesce_requirement_origins(
    requirement: &mut Requirement,
    requirements: &mut Vec<Requirement>,
    aliases: &InstalledAliasIndex,
) {
    let mut index = 0;
    while index < requirements.len() {
        let other = &requirements[index];
        let same_requirement = requirement.version_id == other.version_id
            && (requirement.key == other.key
                || requirement.version_id.is_none()
                    && aliases
                        .same_physical_content(&requirement.key, &other.key));
        if !same_requirement {
            index += 1;
            continue;
        }
        let other = requirements.remove(index);
        requirement.origins.extend(other.origins);
        requirement.explicit_prerelease |= other.explicit_prerelease;
        requirement.preserve_unsafe &= other.preserve_unsafe;
    }
}

fn extend_required_requirements(
    requirements: &mut Vec<Requirement>,
    candidate: &UpgradeCandidate,
    requirement: &Requirement,
    expanded_origins: &mut HashSet<(PhysicalNodeIdentity, String)>,
    aliases: &InstalledAliasIndex,
) {
    let root_origins = if requirement.origins.is_empty() {
        vec![(
            requirement.root_content_id.clone(),
            requirement.root_key.clone(),
        )]
    } else {
        requirement
            .origins
            .iter()
            .map(|origin| {
                (
                    origin.root_content_id.clone(),
                    NodeKey::new(origin.root_provider, &origin.root_project_id),
                )
            })
            .collect::<Vec<_>>()
    }
    .into_iter()
    .filter(|(root_content_id, _)| {
        expanded_origins.insert((
            aliases.physical_identity(&candidate.key),
            root_content_id.clone(),
        ))
    })
    .collect::<Vec<_>>();
    if root_origins.is_empty() {
        return;
    }
    for dependency in &candidate.dependencies {
        if dependency.kind != CandidateDependencyKind::Required {
            continue;
        }
        let origins = root_origins
            .iter()
            .map(|(root_content_id, root_key)| {
                InstanceUpgradeDependencyRequirement {
                    root_content_id: root_content_id.clone(),
                    root_provider: root_key.provider,
                    root_project_id: root_key.project_id.clone(),
                    parent_provider: candidate.key.provider,
                    parent_project_id: candidate.key.project_id.clone(),
                    parent_release_id: candidate.version_id.clone(),
                    dependency_provider: dependency.key.provider,
                    dependency_project_id: dependency.key.project_id.clone(),
                    required_release_id: dependency.version_id.clone(),
                    candidate_release_id: None,
                }
            })
            .collect::<Vec<_>>();
        requirements.insert(
            0,
            Requirement {
                key: dependency.key.clone(),
                version_id: dependency.version_id.clone(),
                explicit_prerelease: false,
                preserve_unsafe: requirement.preserve_unsafe,
                root_content_id: requirement.root_content_id.clone(),
                root_key: requirement.root_key.clone(),
                origins,
            },
        );
    }
}

fn incompatible_with_assignments(
    candidate: &UpgradeCandidate,
    assignments: &HashMap<NodeKey, UpgradeCandidate>,
    physical_assignments: &HashMap<String, NodeKey>,
    aliases: &InstalledAliasIndex,
) -> Option<NodeKey> {
    for dependency in &candidate.dependencies {
        let assigned_key = assigned_key_for_requirement(
            &dependency.key,
            assignments,
            physical_assignments,
            aliases,
        );
        if dependency.kind == CandidateDependencyKind::Incompatible
            && assigned_key.is_some_and(|assigned_key| {
                let selected = &assignments[assigned_key];
                provider_constraint_matches_assignment(
                    &dependency.key,
                    dependency.version_id.as_deref(),
                    assigned_key,
                    selected,
                    aliases,
                )
            })
        {
            return assigned_key.cloned();
        }
    }
    assignments.values().find_map(|selected| {
        selected.dependencies.iter().find_map(|dependency| {
            if dependency.kind == CandidateDependencyKind::Incompatible
                && provider_constraint_matches_assignment(
                    &dependency.key,
                    dependency.version_id.as_deref(),
                    &candidate.key,
                    candidate,
                    aliases,
                )
            {
                Some(selected.key.clone())
            } else {
                None
            }
        })
    })
}

fn record_issue(state: &mut SearchState, issue: InstanceUpgradeIssue) {
    if state.first_issue.is_none() {
        state.first_issue = Some(issue);
    }
}

fn compare_newest(
    left: &SolverResult,
    right: &SolverResult,
    roots: &[RootRequest],
) -> Ordering {
    let left_score = newest_score(left, roots);
    let right_score = newest_score(right, roots);
    left_score.cmp(&right_score)
}

fn newest_score(
    solution: &SolverResult,
    roots: &[RootRequest],
) -> (u8, i64, i64, std::cmp::Reverse<usize>) {
    let root_keys = roots
        .iter()
        .map(|root| root.key.clone())
        .collect::<HashSet<_>>();
    let stable = solution
        .assignments
        .values()
        .map(|candidate| candidate.channel.rank())
        .min()
        .unwrap_or(0);
    let root_freshness = solution
        .assignments
        .iter()
        .filter(|(key, _)| root_keys.contains(*key))
        .map(|(_, candidate)| candidate.published.timestamp())
        .sum();
    let dependency_freshness = solution
        .assignments
        .iter()
        .filter(|(key, _)| !root_keys.contains(*key))
        .map(|(_, candidate)| candidate.published.timestamp())
        .sum();
    let changed = roots
        .iter()
        .filter(|root| {
            solution
                .assignments
                .get(&root.key)
                .is_some_and(|candidate| {
                    candidate.version_id != root.current_release_id
                })
        })
        .count();
    (
        stable,
        root_freshness,
        dependency_freshness,
        std::cmp::Reverse(changed),
    )
}

fn compare_minimal(
    left: &SolverResult,
    right: &SolverResult,
    roots: &[RootRequest],
    installed: &[InstalledNode],
) -> Ordering {
    minimal_score(left, roots, installed)
        .cmp(&minimal_score(right, roots, installed))
}

fn minimal_score(
    solution: &SolverResult,
    roots: &[RootRequest],
    installed: &[InstalledNode],
) -> (usize, usize, usize, usize, std::cmp::Reverse<i64>) {
    let root_keys = roots
        .iter()
        .map(|root| root.key.clone())
        .collect::<HashSet<_>>();
    let installed_by_key = installed
        .iter()
        .flat_map(|node| {
            node.aliases.iter().map(move |alias| {
                (alias.key.clone(), (node, alias.current_release_id.as_str()))
            })
        })
        .collect::<HashMap<_, _>>();
    let root_replacements = roots
        .iter()
        .filter(|root| {
            solution
                .assignments
                .get(&root.key)
                .is_some_and(|candidate| {
                    candidate.version_id != root.current_release_id
                })
        })
        .count();
    let dependency_replacements = solution
        .assignments
        .iter()
        .filter(|(key, candidate)| {
            !root_keys.contains(*key)
                && installed_by_key.get(*key).is_some_and(|(_, current)| {
                    *current != candidate.version_id
                })
        })
        .count();
    let dependency_additions = solution
        .assignments
        .keys()
        .filter(|key| {
            !root_keys.contains(*key) && !installed_by_key.contains_key(*key)
        })
        .count();
    let auto_removals = installed
        .iter()
        .filter(|node| {
            node.auto_dependency
                && node.migratable
                && !node
                    .aliases
                    .iter()
                    .any(|alias| solution.assignments.contains_key(&alias.key))
                && !installed
                    .iter()
                    .any(|other| other.key == node.key && other.user_owned)
        })
        .count();
    let freshness = solution
        .assignments
        .values()
        .map(|candidate| candidate.published.timestamp())
        .sum();
    (
        root_replacements,
        dependency_replacements,
        dependency_additions,
        auto_removals,
        std::cmp::Reverse(freshness),
    )
}

fn materialize_solution(
    kind: InstanceUpgradeSolutionKind,
    solution: &SolverResult,
    roots: &[RootRequest],
    installed: &[InstalledNode],
) -> InstanceUpgradeSolution {
    let aliases = InstalledAliasIndex::new(installed);
    let root_keys = roots
        .iter()
        .map(|root| root.key.clone())
        .collect::<HashSet<_>>();
    let enabled = solution_enabled_nodes(solution, roots, &aliases);
    let installed_by_key = installed
        .iter()
        .flat_map(|node| {
            node.aliases.iter().map(move |alias| {
                (alias.key.clone(), (node, alias.current_release_id.as_str()))
            })
        })
        .collect::<HashMap<_, _>>();
    let mut selections = roots
        .iter()
        .map(|root| {
            let candidate = solution.assignments.get(&root.key);
            let target_release_id =
                candidate.map(|value| value.version_id.clone());
            let action = if root.action == InstanceUpgradeAction::Disable {
                InstanceUpgradeAction::Disable
            } else if target_release_id.as_deref()
                == Some(root.current_release_id.as_str())
            {
                InstanceUpgradeAction::Keep
            } else {
                InstanceUpgradeAction::Upgrade
            };
            InstanceUpgradeSelection {
                content_id: root.content_id.clone(),
                provider: Some(root.key.provider),
                project_id: Some(root.key.project_id.clone()),
                current_release_id: Some(root.current_release_id.clone()),
                target_release_id,
                action,
                enabled: root.enabled
                    && action != InstanceUpgradeAction::Disable,
            }
        })
        .collect::<Vec<_>>();
    selections.sort_by(|left, right| left.content_id.cmp(&right.content_id));

    let mut dependency_changes = solution
        .assignments
        .iter()
        .filter(|(key, _)| !root_keys.contains(*key))
        .map(|(key, candidate)| {
            let current_release_id = installed_by_key
                .get(key)
                .map(|(_, release_id)| (*release_id).to_string());
            let kind = match current_release_id.as_deref() {
                None => InstanceUpgradeDependencyChangeKind::Add,
                Some(current) if current == candidate.version_id => {
                    InstanceUpgradeDependencyChangeKind::Keep
                }
                Some(_) => InstanceUpgradeDependencyChangeKind::Upgrade,
            };
            InstanceUpgradeDependencyChange {
                existing_content_id: installed_by_key
                    .get(key)
                    .map(|(node, _)| node.content_id.clone()),
                provider: key.provider,
                project_id: key.project_id.clone(),
                current_release_id,
                target_release_id: Some(candidate.version_id.clone()),
                kind,
                enabled: key_is_enabled(key, &enabled, &aliases),
            }
        })
        .collect::<Vec<_>>();
    for node in installed
        .iter()
        .filter(|node| node.auto_dependency && node.migratable)
    {
        if node
            .aliases
            .iter()
            .any(|alias| solution.assignments.contains_key(&alias.key))
            || installed
                .iter()
                .any(|other| other.key == node.key && other.user_owned)
        {
            continue;
        }
        dependency_changes.push(InstanceUpgradeDependencyChange {
            existing_content_id: Some(node.content_id.clone()),
            provider: node.key.provider,
            project_id: node.key.project_id.clone(),
            current_release_id: Some(node.current_release_id.clone()),
            target_release_id: None,
            kind: InstanceUpgradeDependencyChangeKind::Remove,
            enabled: false,
        });
    }
    dependency_changes.sort_by(|left, right| {
        left.provider
            .as_str()
            .cmp(right.provider.as_str())
            .then_with(|| left.project_id.cmp(&right.project_id))
    });
    let mut warnings = roots
        .iter()
        .filter_map(|root| {
            let candidate = solution.assignments.get(&root.key)?;
            (root.action == InstanceUpgradeAction::Keep
                && !candidate.compatible)
                .then(|| {
                    issue(
                        InstanceUpgradeIssueCode::KeepIncompatible,
                        format!(
                            "{} remains incompatible with target environment",
                            root.key.label()
                        ),
                        Some(&root.key),
                        Some(&root.key.project_id),
                        None,
                    )
                })
        })
        .collect::<Vec<_>>();
    for key in &solution.preserved_unsafe {
        let Some(candidate) = solution.assignments.get(key) else {
            continue;
        };
        if candidate.compatible
            || !key_is_enabled(key, &enabled, &aliases)
            || warnings.iter().any(|warning| {
                warning.provider == Some(key.provider)
                    && warning.project_id.as_deref()
                        == Some(key.project_id.as_str())
            })
        {
            continue;
        }
        warnings.push(issue(
            InstanceUpgradeIssueCode::KeepIncompatible,
            format!(
                "{} is preserved despite target incompatibility",
                key.label()
            ),
            Some(key),
            Some(&key.project_id),
            None,
        ));
    }
    for (key, candidate) in &solution.assignments {
        if !candidate.channel.is_prerelease()
            || solution.preserved_unsafe.contains(key)
        {
            continue;
        }
        warnings.push(issue(
            InstanceUpgradeIssueCode::PrereleaseOnly,
            format!(
                "{} uses explicitly confirmed prerelease {}",
                key.label(),
                candidate.version_id
            ),
            Some(key),
            Some(&key.project_id),
            None,
        ));
    }
    InstanceUpgradeSolution {
        kind,
        selections,
        dependency_changes,
        warnings,
    }
}

fn solution_enabled_nodes(
    solution: &SolverResult,
    roots: &[RootRequest],
    aliases: &InstalledAliasIndex,
) -> HashSet<NodeKey> {
    let mut enabled = HashSet::new();
    let mut queue = roots
        .iter()
        .filter(|root| {
            root.enabled && root.action != InstanceUpgradeAction::Disable
        })
        .map(|root| root.key.clone())
        .collect::<VecDeque<_>>();
    while let Some(key) = queue.pop_front() {
        if !enabled.insert(key.clone()) {
            continue;
        }
        let assigned_key = assigned_key_for_requirement(
            &key,
            &solution.assignments,
            &solution.physical_assignments,
            aliases,
        );
        if let Some(candidate) = assigned_key
            .and_then(|assigned_key| solution.assignments.get(assigned_key))
        {
            for dependency in &candidate.dependencies {
                if dependency.kind == CandidateDependencyKind::Required {
                    queue.push_back(dependency.key.clone());
                }
            }
        }
    }
    enabled
}

fn key_is_enabled(
    key: &NodeKey,
    enabled: &HashSet<NodeKey>,
    aliases: &InstalledAliasIndex,
) -> bool {
    enabled.contains(key)
        || aliases.content_id(key).is_some_and(|content_id| {
            aliases.aliases_by_content_id.get(content_id).is_some_and(
                |physical_aliases| {
                    physical_aliases.keys().any(|alias| enabled.contains(alias))
                },
            )
        })
}

fn blocking_issue_for_item(
    item: &InstanceUpgradeItem,
    fixed_prerelease: bool,
) -> Option<InstanceUpgradeIssue> {
    if item.resolution.action != InstanceUpgradeAction::Upgrade {
        return None;
    }
    let code = match item.status {
        InstanceUpgradeItemStatus::PrereleaseOnly
            if !item.resolution.allow_prerelease && !fixed_prerelease =>
        {
            InstanceUpgradeIssueCode::PrereleaseOnly
        }
        InstanceUpgradeItemStatus::NoCompatibleRelease => {
            InstanceUpgradeIssueCode::NoCompatibleRelease
        }
        InstanceUpgradeItemStatus::NoCompatibleShaderRuntime => {
            InstanceUpgradeIssueCode::NoCompatibleShaderRuntime
        }
        InstanceUpgradeItemStatus::ShaderRuntimeMissing => {
            InstanceUpgradeIssueCode::ShaderRuntimeMissing
        }
        InstanceUpgradeItemStatus::ShaderRuntimeUnknown => {
            InstanceUpgradeIssueCode::ShaderRuntimeUnknown
        }
        _ => return None,
    };
    Some(InstanceUpgradeIssue {
        code,
        message: format!(
            "{} cannot be upgraded without user resolution",
            item.relative_path
        ),
        content_id: Some(item.content_id.clone()),
        provider: item.provider,
        project_id: item.project_id.clone(),
        conflicting_project_id: None,
        dependency_requirements: Vec::new(),
    })
}

fn item_warnings(items: &[InstanceUpgradeItem]) -> Vec<InstanceUpgradeIssue> {
    item_warnings_with_fixed(items, &HashSet::new())
}

fn item_warnings_with_fixed(
    items: &[InstanceUpgradeItem],
    fixed_content_ids: &HashSet<String>,
) -> Vec<InstanceUpgradeIssue> {
    items
        .iter()
        .filter_map(|item| {
            let code = match item.status {
                InstanceUpgradeItemStatus::Unidentified => {
                    InstanceUpgradeIssueCode::Unidentified
                }
                InstanceUpgradeItemStatus::UnsupportedContentType => {
                    InstanceUpgradeIssueCode::UnsupportedContentType
                }
                InstanceUpgradeItemStatus::PrereleaseOnly => {
                    InstanceUpgradeIssueCode::PrereleaseOnly
                }
                InstanceUpgradeItemStatus::NoCompatibleRelease
                    if item.resolution.action
                        == InstanceUpgradeAction::Keep =>
                {
                    InstanceUpgradeIssueCode::KeepIncompatible
                }
                InstanceUpgradeItemStatus::ShaderRuntimeMissing
                    if item.resolution.action
                        == InstanceUpgradeAction::Keep =>
                {
                    InstanceUpgradeIssueCode::ShaderRuntimeMissing
                }
                InstanceUpgradeItemStatus::ShaderRuntimeUnknown
                    if item.resolution.action
                        == InstanceUpgradeAction::Keep =>
                {
                    InstanceUpgradeIssueCode::ShaderRuntimeUnknown
                }
                _ => return None,
            };
            let message = match code {
                InstanceUpgradeIssueCode::PrereleaseOnly
                    if item.resolution.allow_prerelease
                        || fixed_content_ids.contains(&item.content_id) =>
                {
                    format!(
                        "{} uses a confirmed prerelease",
                        item.relative_path
                    )
                }
                InstanceUpgradeIssueCode::PrereleaseOnly => format!(
                    "{} requires explicit prerelease confirmation",
                    item.relative_path
                ),
                _ => format!("{} will be preserved", item.relative_path),
            };
            Some(InstanceUpgradeIssue {
                code,
                message,
                content_id: Some(item.content_id.clone()),
                provider: item.provider,
                project_id: item.project_id.clone(),
                conflicting_project_id: None,
                dependency_requirements: Vec::new(),
            })
        })
        .collect()
}

fn apply_solver_issues_to_items(
    items: &mut [InstanceUpgradeItem],
    issues: &[InstanceUpgradeIssue],
) {
    for issue in issues {
        let status = match issue.code {
            InstanceUpgradeIssueCode::DependencyConflict => {
                InstanceUpgradeItemStatus::DependencyConflict
            }
            InstanceUpgradeIssueCode::MissingRequiredDependency => {
                InstanceUpgradeItemStatus::MissingRequiredDependency
            }
            InstanceUpgradeIssueCode::IncompatibleDependency => {
                InstanceUpgradeItemStatus::IncompatibleDependency
            }
            _ => continue,
        };
        for item in items.iter_mut().filter(|item| {
            issue.project_id.as_deref() == item.project_id.as_deref()
                || issue.conflicting_project_id.as_deref()
                    == item.project_id.as_deref()
                || issue.dependency_requirements.iter().any(|requirement| {
                    requirement.root_content_id == item.content_id
                })
        }) {
            item.status = status;
        }
    }
}

fn issue(
    code: InstanceUpgradeIssueCode,
    message: impl Into<String>,
    key: Option<&NodeKey>,
    project_id: Option<&str>,
    conflicting_project_id: Option<&str>,
) -> InstanceUpgradeIssue {
    InstanceUpgradeIssue {
        code,
        message: message.into(),
        content_id: None,
        provider: key.map(|key| key.provider),
        project_id: project_id.map(str::to_string),
        conflicting_project_id: conflicting_project_id.map(str::to_string),
        dependency_requirements: Vec::new(),
    }
}

fn issue_with_requirements(
    code: InstanceUpgradeIssueCode,
    message: impl Into<String>,
    key: Option<&NodeKey>,
    project_id: Option<&str>,
    conflicting_project_id: Option<&str>,
    dependency_requirements: Vec<InstanceUpgradeDependencyRequirement>,
) -> InstanceUpgradeIssue {
    InstanceUpgradeIssue {
        dependency_requirements,
        ..issue(code, message, key, project_id, conflicting_project_id)
    }
}

fn deduplicate_issues(issues: &mut Vec<InstanceUpgradeIssue>) {
    let mut seen = HashSet::new();
    issues.retain(|issue| {
        seen.insert((
            format!("{:?}", issue.code),
            issue.content_id.clone(),
            issue.project_id.clone(),
            issue.conflicting_project_id.clone(),
        ))
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::path::Path;

    fn key(project_id: &str) -> NodeKey {
        NodeKey::new(ContentProvider::Modrinth, project_id)
    }

    fn candidate(
        project_id: &str,
        version_id: &str,
        published: i64,
    ) -> UpgradeCandidate {
        UpgradeCandidate {
            key: key(project_id),
            version_id: version_id.to_string(),
            published: Utc.timestamp_opt(published, 0).single().unwrap(),
            channel: CandidateChannel::Release,
            compatible: true,
            installed_current: false,
            dependencies: Vec::new(),
        }
    }

    fn candidate_for_key(
        key: &NodeKey,
        version_id: &str,
        published: i64,
    ) -> UpgradeCandidate {
        UpgradeCandidate {
            key: key.clone(),
            version_id: version_id.to_string(),
            published: Utc.timestamp_opt(published, 0).single().unwrap(),
            channel: CandidateChannel::Release,
            compatible: true,
            installed_current: false,
            dependencies: Vec::new(),
        }
    }

    fn required(
        project_id: &str,
        version_id: Option<&str>,
    ) -> CandidateDependency {
        CandidateDependency {
            key: key(project_id),
            version_id: version_id.map(str::to_string),
            kind: CandidateDependencyKind::Required,
        }
    }

    fn incompatible(project_id: &str) -> CandidateDependency {
        CandidateDependency {
            key: key(project_id),
            version_id: None,
            kind: CandidateDependencyKind::Incompatible,
        }
    }

    fn root(project_id: &str, current: &str, enabled: bool) -> RootRequest {
        RootRequest {
            content_id: format!("entry-{project_id}"),
            key: key(project_id),
            current_release_id: current.to_string(),
            enabled,
            action: InstanceUpgradeAction::Upgrade,
            allow_prerelease: false,
        }
    }

    fn installed(
        project_id: &str,
        current: &str,
        auto_dependency: bool,
        user_owned: bool,
    ) -> InstalledNode {
        InstalledNode {
            content_id: format!("entry-{project_id}"),
            key: key(project_id),
            current_release_id: current.to_string(),
            project_type: ProjectType::Mod,
            enabled: true,
            auto_dependency,
            user_owned,
            migratable: true,
            aliases: vec![InstalledAlias {
                key: key(project_id),
                current_release_id: current.to_string(),
            }],
        }
    }

    fn installed_with_aliases(
        content_id: &str,
        primary_key: &NodeKey,
        current: &str,
        auto_dependency: bool,
        user_owned: bool,
        extra_aliases: Vec<(&NodeKey, &str)>,
    ) -> InstalledNode {
        let mut aliases = vec![InstalledAlias {
            key: primary_key.clone(),
            current_release_id: current.to_string(),
        }];
        aliases.extend(extra_aliases.into_iter().map(|(key, release)| {
            InstalledAlias {
                key: key.clone(),
                current_release_id: release.to_string(),
            }
        }));
        InstalledNode {
            content_id: content_id.to_string(),
            key: primary_key.clone(),
            current_release_id: current.to_string(),
            project_type: ProjectType::Mod,
            enabled: true,
            auto_dependency,
            user_owned,
            migratable: true,
            aliases,
        }
    }

    fn catalog<const N: usize>(
        entries: [(NodeKey, Vec<UpgradeCandidate>); N],
    ) -> UpgradeCatalog {
        entries
            .into_iter()
            .map(|(key, candidates)| {
                (
                    key,
                    CandidatePool {
                        candidates,
                        exploration_limited: false,
                        has_target_game_version_release: true,
                    },
                )
            })
            .collect()
    }

    fn solve(roots: &[RootRequest], catalog: &UpgradeCatalog) -> SolveOutcome {
        solve_upgrade(roots, &[], catalog, &HashMap::new(), &HashSet::new())
    }

    fn solve_with_installed(
        roots: &[RootRequest],
        installed: &[InstalledNode],
        catalog: &UpgradeCatalog,
    ) -> SolveOutcome {
        solve_upgrade(
            roots,
            installed,
            catalog,
            &HashMap::new(),
            &HashSet::new(),
        )
    }

    #[test]
    fn single_mod_cross_minecraft_upgrade_selects_target_candidate() {
        let roots = vec![root("a", "a-old", true)];
        let catalog = catalog([(key("a"), vec![candidate("a", "a-new", 2)])]);
        let outcome = solve(&roots, &catalog);
        assert_eq!(outcome.solutions.len(), 1);
        assert_eq!(
            outcome.solutions[0].assignments[&key("a")].version_id,
            "a-new"
        );
    }

    #[test]
    fn minimal_change_keeps_current_compatible_version() {
        let roots = vec![root("a", "a-old", true)];
        let catalog = catalog([(
            key("a"),
            vec![candidate("a", "a-new", 2), candidate("a", "a-old", 1)],
        )]);
        let outcome = solve(&roots, &catalog);
        let minimal = outcome
            .solutions
            .iter()
            .min_by(|left, right| compare_minimal(left, right, &roots, &[]))
            .unwrap();
        assert_eq!(minimal.assignments[&key("a")].version_id, "a-old");
    }

    #[test]
    fn newest_solution_uses_higher_stable_version() {
        let roots = vec![root("a", "a-old", true)];
        let catalog = catalog([(
            key("a"),
            vec![candidate("a", "a-new", 2), candidate("a", "a-old", 1)],
        )]);
        let outcome = solve(&roots, &catalog);
        let newest = outcome
            .solutions
            .iter()
            .max_by(|left, right| compare_newest(left, right, &roots))
            .unwrap();
        assert_eq!(newest.assignments[&key("a")].version_id, "a-new");
    }

    #[test]
    fn twenty_independent_roots_do_not_form_cartesian_search() {
        let mut roots = Vec::new();
        let mut catalog = UpgradeCatalog::new();
        for root_index in 0..20 {
            let project_id = format!("root-{root_index}");
            let current = format!("{project_id}-current");
            roots.push(root(&project_id, &current, true));
            let mut candidates = (0..5)
                .map(|candidate_index| {
                    candidate(
                        &project_id,
                        &format!("{project_id}-new-{candidate_index}"),
                        100 - candidate_index,
                    )
                })
                .collect::<Vec<_>>();
            candidates.push(candidate(&project_id, &current, 1));
            catalog.insert(
                key(&project_id),
                CandidatePool {
                    candidates,
                    exploration_limited: false,
                    has_target_game_version_release: true,
                },
            );
        }

        let outcome = solve(&roots, &catalog);
        assert_eq!(outcome.solutions.len(), 2);
        assert!(outcome.issues.is_empty());
        assert!(outcome.visited_states < 100);
        let newest = outcome
            .solutions
            .iter()
            .max_by(|left, right| compare_newest(left, right, &roots))
            .unwrap();
        let minimal = outcome
            .solutions
            .iter()
            .min_by(|left, right| compare_minimal(left, right, &roots, &[]))
            .unwrap();
        assert!(roots.iter().all(|root| {
            newest.assignments[&root.key].version_id != root.current_release_id
        }));
        assert!(roots.iter().all(|root| {
            minimal.assignments[&root.key].version_id == root.current_release_id
        }));
    }

    #[test]
    fn real_world_root_candidate_counts_solve_without_search_limit() {
        let counts = [6, 6, 6, 6, 6, 6, 3];
        let mut roots = Vec::new();
        let mut catalog = UpgradeCatalog::new();
        for (root_index, count) in counts.into_iter().enumerate() {
            let project_id = format!("root-{root_index}");
            roots.push(root(&project_id, "old", true));
            let candidates = (0..count)
                .map(|candidate_index| {
                    candidate(
                        &project_id,
                        &format!("{project_id}-{candidate_index}"),
                        100 - candidate_index as i64,
                    )
                })
                .collect();
            catalog.insert(
                key(&project_id),
                CandidatePool {
                    candidates,
                    exploration_limited: false,
                    has_target_game_version_release: true,
                },
            );
        }

        let outcome = solve(&roots, &catalog);
        assert!(!outcome.solutions.is_empty());
        assert!(outcome.issues.iter().all(|issue| {
            issue.code != InstanceUpgradeIssueCode::SearchLimitReached
        }));
        assert!(outcome.visited_states < 50);
    }

    #[test]
    fn duplicate_physical_shader_roots_remain_separate_without_cartesian_search()
     {
        let mut roots = (0..3)
            .map(|index| RootRequest {
                content_id: format!("shader-entry-{index}"),
                key: key("shader"),
                current_release_id: "shader-old".to_string(),
                enabled: true,
                action: InstanceUpgradeAction::Upgrade,
                allow_prerelease: false,
            })
            .collect::<Vec<_>>();
        roots[1].enabled = false;
        let candidates = (0..6)
            .map(|index| {
                candidate("shader", &format!("shader-new-{index}"), 10 - index)
            })
            .collect();
        let catalog = catalog([(key("shader"), candidates)]);

        let outcome = solve(&roots, &catalog);
        assert!(outcome.issues.is_empty());
        assert!(outcome.visited_states < 20);
        let materialized = materialize_solution(
            InstanceUpgradeSolutionKind::Newest,
            &outcome.solutions[0],
            &roots,
            &[],
        );
        assert_eq!(materialized.selections.len(), 3);
        assert_eq!(
            materialized
                .selections
                .iter()
                .map(|selection| selection.content_id.as_str())
                .collect::<HashSet<_>>()
                .len(),
            3
        );
    }

    #[test]
    fn conflict_fallback_does_not_change_independent_roots() {
        let mut a_latest = candidate("a", "a-latest", 10);
        a_latest.dependencies.push(required("x", Some("x-two")));
        let mut a_previous = candidate("a", "a-previous", 9);
        a_previous.dependencies.push(required("x", Some("x-three")));
        let mut b_latest = candidate("b", "b-latest", 10);
        b_latest.dependencies.push(required("x", Some("x-three")));
        let roots = vec![
            root("a", "a-old", true),
            root("b", "b-old", true),
            root("c", "c-old", true),
            root("d", "d-old", true),
            root("e", "e-old", true),
        ];
        let catalog = catalog([
            (key("a"), vec![a_latest, a_previous]),
            (key("b"), vec![b_latest]),
            (key("c"), vec![candidate("c", "c-preferred", 5)]),
            (key("d"), vec![candidate("d", "d-preferred", 5)]),
            (key("e"), vec![candidate("e", "e-preferred", 5)]),
            (
                key("x"),
                vec![candidate("x", "x-three", 3), candidate("x", "x-two", 2)],
            ),
        ]);

        let outcome = solve(&roots, &catalog);
        let newest = outcome
            .solutions
            .iter()
            .max_by(|left, right| compare_newest(left, right, &roots))
            .unwrap();
        assert_eq!(newest.assignments[&key("a")].version_id, "a-previous");
        assert_eq!(newest.assignments[&key("b")].version_id, "b-latest");
        for project_id in ["c", "d", "e"] {
            assert_eq!(
                newest.assignments[&key(project_id)].version_id,
                format!("{project_id}-preferred")
            );
        }
    }

    #[test]
    fn minimal_change_keeps_many_compatible_current_roots_directly() {
        let mut roots = Vec::new();
        let mut catalog = UpgradeCatalog::new();
        for index in 0..30 {
            let project_id = format!("root-{index}");
            let current = format!("{project_id}-current");
            roots.push(root(&project_id, &current, true));
            catalog.insert(
                key(&project_id),
                CandidatePool {
                    candidates: vec![
                        candidate(&project_id, &format!("{project_id}-new"), 2),
                        candidate(&project_id, &current, 1),
                    ],
                    exploration_limited: false,
                    has_target_game_version_release: true,
                },
            );
        }

        let outcome = solve(&roots, &catalog);
        let minimal = outcome
            .solutions
            .iter()
            .min_by(|left, right| compare_minimal(left, right, &roots, &[]))
            .unwrap();
        assert!(roots.iter().all(|root| {
            minimal.assignments[&root.key].version_id == root.current_release_id
        }));
        assert!(outcome.visited_states < 150);
    }

    #[test]
    fn custom_fixed_root_stays_fixed_when_other_roots_conflict() {
        let mut b_latest = candidate("b", "b-latest", 5);
        b_latest.dependencies.push(required("x", Some("x-two")));
        let mut b_previous = candidate("b", "b-previous", 4);
        b_previous.dependencies.push(required("x", Some("x-three")));
        let mut c = candidate("c", "c-latest", 5);
        c.dependencies.push(required("x", Some("x-three")));
        let roots = vec![
            root("a", "a-old", true),
            root("b", "b-old", true),
            root("c", "c-old", true),
        ];
        let catalog = catalog([
            (
                key("a"),
                vec![
                    candidate("a", "a-newest", 10),
                    candidate("a", "a-fixed", 9),
                ],
            ),
            (key("b"), vec![b_latest, b_previous]),
            (key("c"), vec![c]),
            (
                key("x"),
                vec![candidate("x", "x-three", 3), candidate("x", "x-two", 2)],
            ),
        ]);
        let fixed = HashMap::from([(key("a"), "a-fixed".to_string())]);

        let outcome =
            solve_upgrade(&roots, &[], &catalog, &fixed, &HashSet::new());
        assert!(!outcome.solutions.is_empty());
        assert!(outcome.solutions.iter().all(|solution| {
            solution.assignments[&key("a")].version_id == "a-fixed"
        }));
        assert!(outcome.solutions.iter().any(|solution| {
            solution.assignments[&key("b")].version_id == "b-previous"
                && solution.assignments[&key("c")].version_id == "c-latest"
        }));
    }

    #[test]
    fn complex_conflict_alternative_search_still_honors_state_limit() {
        let roots = vec![root("a", "a-old", true), root("b", "b-old", true)];
        let a_candidates = (0..6)
            .map(|index| {
                let mut candidate =
                    candidate("a", &format!("a-{index}"), 20 - index);
                candidate
                    .dependencies
                    .push(required("x", Some(&format!("x-a-{index}"))));
                candidate
            })
            .collect::<Vec<_>>();
        let b_candidates = (0..6)
            .map(|index| {
                let mut candidate =
                    candidate("b", &format!("b-{index}"), 20 - index);
                candidate
                    .dependencies
                    .push(required("x", Some(&format!("x-b-{index}"))));
                candidate
            })
            .collect::<Vec<_>>();
        let x_candidates = (0..6)
            .flat_map(|index| {
                [
                    candidate("x", &format!("x-a-{index}"), 10 - index),
                    candidate("x", &format!("x-b-{index}"), 10 - index),
                ]
            })
            .collect::<Vec<_>>();
        let catalog = catalog([
            (key("a"), a_candidates),
            (key("b"), b_candidates),
            (key("x"), x_candidates),
        ]);

        let outcome = solve_for_strategy_with_limit(
            SolveStrategy::Newest,
            &roots,
            &catalog,
            &HashMap::new(),
            &HashSet::new(),
            &InstalledAliasIndex::default(),
            20,
        );
        assert!(outcome.solution.is_none());
        assert_eq!(outcome.visited_states, 20);
        assert_eq!(
            outcome.issue.unwrap().code,
            InstanceUpgradeIssueCode::SearchLimitReached
        );
    }

    #[test]
    fn prerelease_only_is_not_selected_without_confirmation() {
        let roots = vec![root("a", "a-old", true)];
        let mut beta = candidate("a", "a-beta", 2);
        beta.channel = CandidateChannel::Beta;
        let catalog = catalog([(key("a"), vec![beta])]);
        let outcome = solve(&roots, &catalog);
        assert!(outcome.solutions.is_empty());
    }

    #[test]
    fn required_dependency_is_added() {
        let roots = vec![root("a", "a-old", true)];
        let mut a = candidate("a", "a-new", 2);
        a.dependencies.push(required("x", None));
        let catalog = catalog([
            (key("a"), vec![a]),
            (key("x"), vec![candidate("x", "x-one", 1)]),
        ]);
        let outcome = solve(&roots, &catalog);
        let solution = materialize_solution(
            InstanceUpgradeSolutionKind::Newest,
            &outcome.solutions[0],
            &roots,
            &[],
        );
        assert_eq!(
            solution.dependency_changes[0].kind,
            InstanceUpgradeDependencyChangeKind::Add
        );
    }

    #[test]
    fn transitive_dependency_closure_is_resolved() {
        let roots = vec![root("a", "a-old", true)];
        let mut a = candidate("a", "a-new", 3);
        a.dependencies.push(required("x", None));
        let mut x = candidate("x", "x-one", 2);
        x.dependencies.push(required("y", None));
        let catalog = catalog([
            (key("a"), vec![a]),
            (key("x"), vec![x]),
            (key("y"), vec![candidate("y", "y-one", 1)]),
        ]);
        assert_eq!(solve(&roots, &catalog).solutions[0].assignments.len(), 3);
    }

    #[test]
    fn required_dependency_cycle_terminates_with_solution() {
        let roots = vec![root("a", "a-old", true)];
        let mut a = candidate("a", "a-new", 2);
        a.dependencies.push(required("b", None));
        let mut b = candidate("b", "b-one", 1);
        b.dependencies.push(required("a", None));
        let catalog = catalog([(key("a"), vec![a]), (key("b"), vec![b])]);
        let outcome = solve(&roots, &catalog);
        assert_eq!(outcome.solutions.len(), 1);
        assert!(outcome.issues.is_empty());
    }

    #[test]
    fn self_required_dependency_cycle_terminates_with_solution() {
        let roots = vec![root("a", "a-old", true)];
        let mut a = candidate("a", "a-new", 1);
        a.dependencies.push(required("a", None));
        let catalog = catalog([(key("a"), vec![a])]);
        let outcome = solve(&roots, &catalog);
        assert_eq!(outcome.solutions.len(), 1);
        assert!(outcome.issues.is_empty());
    }

    #[test]
    fn multiple_roots_share_one_dependency_assignment() {
        let roots = vec![root("a", "a-old", true), root("b", "b-old", true)];
        let mut a = candidate("a", "a-new", 3);
        a.dependencies.push(required("x", Some("x-one")));
        let mut b = candidate("b", "b-new", 3);
        b.dependencies.push(required("x", Some("x-one")));
        let catalog = catalog([
            (key("a"), vec![a]),
            (key("b"), vec![b]),
            (key("x"), vec![candidate("x", "x-one", 1)]),
        ]);
        assert_eq!(solve(&roots, &catalog).solutions[0].assignments.len(), 3);
    }

    #[test]
    fn latest_conflict_backtracks_to_older_root_candidate() {
        let roots = vec![root("a", "a-old", true), root("b", "b-old", true)];
        let mut newest = candidate("a", "a-two", 3);
        newest.dependencies.push(incompatible("b"));
        let catalog = catalog([
            (key("a"), vec![newest, candidate("a", "a-one", 2)]),
            (key("b"), vec![candidate("b", "b-one", 1)]),
        ]);
        let outcome = solve(&roots, &catalog);
        assert_eq!(outcome.solutions.len(), 1);
        assert_eq!(
            outcome.solutions[0].assignments[&key("a")].version_id,
            "a-one"
        );
    }

    #[test]
    fn complete_dependency_conflict_has_no_solution() {
        let roots = vec![root("a", "a-old", true), root("b", "b-old", true)];
        let mut a = candidate("a", "a-one", 1);
        a.dependencies.push(incompatible("b"));
        let catalog = catalog([
            (key("a"), vec![a]),
            (key("b"), vec![candidate("b", "b-one", 1)]),
        ]);
        assert!(solve(&roots, &catalog).solutions.is_empty());
    }

    fn shared_exact_root_fixture() -> (Vec<RootRequest>, UpgradeCatalog) {
        let roots = vec![
            root("a", "a-old", true),
            root("b", "b-old", true),
            root("x", "x-zero", true),
        ];
        let mut a = candidate("a", "a-one", 3);
        a.dependencies.push(required("x", Some("x-one")));
        let mut b = candidate("b", "b-one", 3);
        b.dependencies.push(required("x", Some("x-one")));
        let mut x_zero = candidate("x", "x-zero", 1);
        x_zero.installed_current = true;
        let x_one = candidate("x", "x-one", 2);
        let catalog = catalog([
            (key("a"), vec![a]),
            (key("b"), vec![b]),
            (key("x"), vec![x_one, x_zero]),
        ]);
        (roots, catalog)
    }

    #[test]
    fn flexible_root_uses_shared_exact_dependency_assignment() {
        let (roots, catalog) = shared_exact_root_fixture();
        let outcome = solve(&roots, &catalog);

        assert!(outcome.issues.is_empty());
        assert!(outcome.solutions.iter().all(|solution| {
            solution.assignments[&key("x")].version_id == "x-one"
        }));
    }

    #[test]
    fn minimal_change_obeys_shared_exact_dependency_assignment() {
        let (roots, catalog) = shared_exact_root_fixture();
        let outcome = solve_for_strategy(
            SolveStrategy::MinimalChange,
            &roots,
            &catalog,
            &HashMap::new(),
            &HashSet::new(),
            &InstalledAliasIndex::new(&[]),
        );

        assert!(outcome.issue.is_none());
        assert_eq!(
            outcome.solution.unwrap().assignments[&key("x")].version_id,
            "x-one"
        );
    }

    #[test]
    fn fixed_root_conflicts_with_shared_exact_dependency_assignment() {
        let (roots, catalog) = shared_exact_root_fixture();
        let fixed = HashMap::from([(key("x"), "x-zero".to_string())]);
        let outcome =
            solve_upgrade(&roots, &[], &catalog, &fixed, &HashSet::new());

        assert!(outcome.solutions.is_empty());
        assert_eq!(
            outcome.issues[0].code,
            InstanceUpgradeIssueCode::DependencyConflict
        );
        assert_eq!(outcome.issues[0].dependency_requirements.len(), 2);
        assert!(outcome.issues[0].dependency_requirements.iter().all(
            |detail| {
                detail.parent_project_id != detail.dependency_project_id
                    && detail.candidate_release_id.as_deref() == Some("x-zero")
            }
        ));
    }

    #[test]
    fn kept_root_conflicts_with_shared_exact_dependency_assignment() {
        let (mut roots, catalog) = shared_exact_root_fixture();
        roots
            .iter_mut()
            .find(|root| root.key == key("x"))
            .unwrap()
            .action = InstanceUpgradeAction::Keep;
        let outcome = solve(&roots, &catalog);

        assert!(outcome.solutions.is_empty());
        assert_eq!(
            outcome.issues[0].code,
            InstanceUpgradeIssueCode::DependencyConflict
        );
        assert!(
            outcome.issues[0]
                .dependency_requirements
                .iter()
                .all(|detail| detail.parent_project_id != "x")
        );
    }

    #[test]
    fn root_assignment_follows_dependency_after_causal_parent_fallback() {
        let roots = vec![
            root("a", "a-old", true),
            root("b", "b-old", true),
            root("x", "x-zero", true),
        ];
        let mut a_latest = candidate("a", "a-latest", 5);
        a_latest.dependencies.push(required("x", Some("x-two")));
        let mut a_previous = candidate("a", "a-previous", 4);
        a_previous.dependencies.push(required("x", Some("x-three")));
        let mut b = candidate("b", "b-latest", 5);
        b.dependencies.push(required("x", Some("x-three")));
        let mut x_zero = candidate("x", "x-zero", 1);
        x_zero.installed_current = true;
        let catalog = catalog([
            (key("a"), vec![a_latest, a_previous]),
            (key("b"), vec![b]),
            (
                key("x"),
                vec![
                    candidate("x", "x-three", 3),
                    candidate("x", "x-two", 2),
                    x_zero,
                ],
            ),
        ]);
        let outcome = solve(&roots, &catalog);

        assert!(outcome.issues.is_empty());
        assert!(outcome.solutions.iter().all(|solution| {
            solution.assignments[&key("a")].version_id == "a-previous"
                && solution.assignments[&key("x")].version_id == "x-three"
        }));
    }

    #[test]
    fn reconciled_user_owned_root_is_not_materialized_as_dependency() {
        let (roots, catalog) = shared_exact_root_fixture();
        let installed = vec![installed("x", "x-zero", false, true)];
        let outcome = solve_with_installed(&roots, &installed, &catalog);
        let solution = materialize_solution(
            InstanceUpgradeSolutionKind::MinimalChange,
            &outcome.solutions[0],
            &roots,
            &installed,
        );

        let x_selection = solution
            .selections
            .iter()
            .find(|selection| selection.project_id.as_deref() == Some("x"))
            .unwrap();
        assert_eq!(x_selection.target_release_id.as_deref(), Some("x-one"));
        assert!(solution.dependency_changes.iter().all(|change| {
            change.project_id != "x"
                && change.existing_content_id.as_deref() != Some("entry-x")
        }));
    }

    #[test]
    fn missing_required_dependency_is_blocking() {
        let roots = vec![root("a", "a-old", true)];
        let mut a = candidate("a", "a-one", 1);
        a.dependencies.push(required("missing", None));
        let catalog = catalog([(key("a"), vec![a])]);
        let outcome = solve(&roots, &catalog);
        assert_eq!(
            outcome.issues[0].code,
            InstanceUpgradeIssueCode::MissingRequiredDependency
        );
    }

    #[test]
    fn incompatible_dependency_edge_is_blocking() {
        let roots = vec![root("a", "a-old", true), root("x", "x-old", true)];
        let mut a = candidate("a", "a-one", 1);
        a.dependencies.push(incompatible("x"));
        let catalog = catalog([
            (key("a"), vec![a]),
            (key("x"), vec![candidate("x", "x-one", 1)]),
        ]);
        let outcome = solve(&roots, &catalog);
        assert!(outcome.solutions.is_empty());
        assert_eq!(
            outcome.issues[0].code,
            InstanceUpgradeIssueCode::IncompatibleDependency
        );
    }

    #[test]
    fn orphaned_auto_dependency_is_suggested_for_removal() {
        let roots = vec![root("a", "a-old", true)];
        let catalog = catalog([(key("a"), vec![candidate("a", "a-one", 1)])]);
        let outcome = solve(&roots, &catalog);
        let nodes = vec![installed("x", "x-old", true, false)];
        let solution = materialize_solution(
            InstanceUpgradeSolutionKind::Newest,
            &outcome.solutions[0],
            &roots,
            &nodes,
        );
        assert_eq!(
            solution.dependency_changes[0].kind,
            InstanceUpgradeDependencyChangeKind::Remove
        );
    }

    #[test]
    fn user_owned_dependency_identity_is_never_removed() {
        let roots = vec![root("a", "a-old", true)];
        let catalog = catalog([(key("a"), vec![candidate("a", "a-one", 1)])]);
        let outcome = solve(&roots, &catalog);
        let nodes = vec![
            installed("x", "x-auto", true, false),
            installed("x", "x-user", false, true),
        ];
        let solution = materialize_solution(
            InstanceUpgradeSolutionKind::Newest,
            &outcome.solutions[0],
            &roots,
            &nodes,
        );
        assert!(solution.dependency_changes.is_empty());
    }

    #[test]
    fn disabled_root_remains_disabled_after_upgrade() {
        let roots = vec![root("a", "a-old", false)];
        let catalog = catalog([(key("a"), vec![candidate("a", "a-one", 1)])]);
        let outcome = solve(&roots, &catalog);
        let solution = materialize_solution(
            InstanceUpgradeSolutionKind::Newest,
            &outcome.solutions[0],
            &roots,
            &[],
        );
        assert!(!solution.selections[0].enabled);
    }

    #[test]
    fn disabled_only_dependency_remains_disabled() {
        let roots = vec![root("a", "a-old", false)];
        let mut a = candidate("a", "a-one", 2);
        a.dependencies.push(required("x", None));
        let catalog = catalog([
            (key("a"), vec![a]),
            (key("x"), vec![candidate("x", "x-one", 1)]),
        ]);
        let outcome = solve(&roots, &catalog);
        let solution = materialize_solution(
            InstanceUpgradeSolutionKind::Newest,
            &outcome.solutions[0],
            &roots,
            &[],
        );
        assert!(!solution.dependency_changes[0].enabled);
    }

    #[test]
    fn unidentified_local_jar_is_preserved_with_warning() {
        let item = test_item(InstanceUpgradeItemStatus::Unidentified);
        let warnings = item_warnings(&[item]);
        assert_eq!(warnings[0].code, InstanceUpgradeIssueCode::Unidentified);
    }

    #[test]
    fn optifine_to_iris_shader_accepts_iris_release() {
        let version = test_version(vec!["iris"], vec!["26.1"]);
        assert!(modrinth_version_matches(
            &version,
            ProjectType::ShaderPack,
            &test_environment(ShaderRuntime::Iris)
        ));
    }

    #[test]
    fn optifine_to_iris_shader_rejects_optifine_only_release() {
        let version = test_version(vec!["optifine"], vec!["26.1"]);
        assert!(!modrinth_version_matches(
            &version,
            ProjectType::ShaderPack,
            &test_environment(ShaderRuntime::Iris)
        ));
    }

    #[test]
    fn shader_without_target_minecraft_release_reports_no_release() {
        assert_eq!(
            classified_shader_status(ShaderRuntime::Iris, false),
            InstanceUpgradeItemStatus::NoCompatibleRelease
        );
    }

    #[test]
    fn shader_with_target_minecraft_but_wrong_runtime_reports_runtime() {
        assert_eq!(
            classified_shader_status(ShaderRuntime::Iris, true),
            InstanceUpgradeItemStatus::NoCompatibleShaderRuntime
        );
    }

    #[test]
    fn shader_without_target_runtime_reports_missing_only() {
        let (item, roots) = classified_shader(ShaderRuntime::None, true);
        assert_eq!(
            item.status,
            InstanceUpgradeItemStatus::ShaderRuntimeMissing
        );
        assert_eq!(roots[0].action, InstanceUpgradeAction::Keep);
        assert!(solve(&roots, &HashMap::new()).issues.is_empty());
        assert_eq!(
            item_warnings(std::slice::from_ref(&item))[0].code,
            InstanceUpgradeIssueCode::ShaderRuntimeMissing
        );
    }

    #[test]
    fn shader_with_unknown_target_runtime_reports_unknown_only() {
        let (item, roots) = classified_shader(ShaderRuntime::Unknown, true);
        assert_eq!(
            item.status,
            InstanceUpgradeItemStatus::ShaderRuntimeUnknown
        );
        assert_eq!(roots[0].action, InstanceUpgradeAction::Keep);
        assert!(solve(&roots, &HashMap::new()).issues.is_empty());
        assert_eq!(
            item_warnings(std::slice::from_ref(&item))[0].code,
            InstanceUpgradeIssueCode::ShaderRuntimeUnknown
        );
    }

    #[test]
    fn missing_shader_runtime_preserves_explicit_disable() {
        assert_disabled_shader_selection(ShaderRuntime::None);
    }

    #[test]
    fn unknown_shader_runtime_preserves_explicit_disable() {
        assert_disabled_shader_selection(ShaderRuntime::Unknown);
    }

    #[test]
    fn source_shader_runtime_uses_trusted_modrinth_alias_for_curseforge_item() {
        let mut item = snapshot_item_for_test(
            Some(ContentProvider::CurseForge),
            Some("123"),
            Some("123"),
        );
        item.content.as_mut().unwrap().provider_refs = vec![
            ContentProviderRef::from_database(
                "curseforge",
                "123",
                Some("123"),
                None,
            )
            .unwrap(),
            ContentProviderRef::from_database(
                "modrinth",
                "YL57xq9U",
                Some("iris-version"),
                None,
            )
            .unwrap(),
        ];
        let snapshot = InstanceContentSnapshot {
            instance_id: "instance".to_string(),
            revision: 1,
            pack: None,
            items: vec![item],
            pending_manual_downloads: Vec::new(),
            warnings: Vec::new(),
        };
        assert_eq!(source_shader_runtime(&[], &snapshot), ShaderRuntime::Iris);
        let (_, installed) = snapshot_upgrade_items(&snapshot);
        assert!(installed[0].aliases.iter().any(|alias| {
            alias.key.provider == ContentProvider::Modrinth
                && alias.key.project_id == "YL57xq9U"
        }));
    }

    #[test]
    fn verified_persistent_identity_supplies_planner_target_candidates() {
        let item = snapshot_item_for_test(
            Some(ContentProvider::Modrinth),
            Some("5LTBDHXu"),
            Some("eKsF3BdO"),
        );
        let snapshot = InstanceContentSnapshot {
            instance_id: "instance".to_string(),
            revision: 2,
            pack: None,
            items: vec![item],
            pending_manual_downloads: Vec::new(),
            warnings: Vec::new(),
        };
        let (mut items, installed) = snapshot_upgrade_items(&snapshot);
        let target = test_environment(ShaderRuntime::None);
        let catalog = catalog([(
            NodeKey::new(ContentProvider::Modrinth, "5LTBDHXu"),
            vec![candidate_for_key(
                &NodeKey::new(ContentProvider::Modrinth, "5LTBDHXu"),
                "wf4Vw5gN",
                2,
            )],
        )]);

        classify_items(&mut items, &installed, &catalog, &target);

        assert_eq!(items[0].provider, Some(ContentProvider::Modrinth));
        assert_eq!(items[0].project_id.as_deref(), Some("5LTBDHXu"));
        assert_eq!(items[0].current_release_id.as_deref(), Some("eKsF3BdO"));
        assert_eq!(
            items[0].status,
            InstanceUpgradeItemStatus::UpgradeAvailable
        );
        assert_eq!(items[0].candidate_release_ids, vec!["wf4Vw5gN"]);
    }

    #[test]
    fn exact_provider_ref_reconciles_existing_user_owned_dependency_root() {
        let solution = reconciled_existing_dependency_solution(false, "old");
        let fabric = solution
            .selections
            .iter()
            .find(|selection| {
                selection.project_id.as_deref() == Some("P7dR8mSH")
            })
            .unwrap();
        assert_eq!(fabric.content_id, "fabric-entry");
        assert_eq!(fabric.target_release_id.as_deref(), Some("new"));
        assert_eq!(fabric.action, InstanceUpgradeAction::Upgrade);
        assert!(solution.dependency_changes.iter().all(|change| {
            change.project_id != "P7dR8mSH"
                && change.existing_content_id.as_deref() != Some("fabric-entry")
        }));
    }

    #[test]
    fn exact_provider_ref_reconciles_existing_auto_dependency() {
        let solution = reconciled_existing_dependency_solution(true, "old");
        let changes = solution
            .dependency_changes
            .iter()
            .filter(|change| change.project_id == "P7dR8mSH")
            .collect::<Vec<_>>();
        assert_eq!(changes.len(), 1);
        assert_eq!(
            changes[0].existing_content_id.as_deref(),
            Some("fabric-entry")
        );
        assert_eq!(
            changes[0].kind,
            InstanceUpgradeDependencyChangeKind::Upgrade
        );
    }

    #[test]
    fn exact_provider_ref_reuses_matching_existing_dependency_release() {
        let solution = reconciled_existing_dependency_solution(true, "new");
        let changes = solution
            .dependency_changes
            .iter()
            .filter(|change| change.project_id == "P7dR8mSH")
            .collect::<Vec<_>>();
        assert_eq!(changes.len(), 1);
        assert_eq!(
            changes[0].existing_content_id.as_deref(),
            Some("fabric-entry")
        );
        assert_eq!(changes[0].kind, InstanceUpgradeDependencyChangeKind::Keep);
    }

    #[test]
    fn shared_path_semantics_do_not_change_dependency_reconciliation() {
        let local = reconciled_existing_dependency_solution(false, "old");
        let shared_junction =
            reconciled_existing_dependency_solution(false, "old");
        assert_eq!(
            serde_json::to_value(local).unwrap(),
            serde_json::to_value(shared_junction).unwrap()
        );
    }

    fn reconciled_existing_dependency_solution(
        auto_dependency: bool,
        current_release: &str,
    ) -> InstanceUpgradeSolution {
        let mut app = snapshot_item_for_test(
            Some(ContentProvider::Modrinth),
            Some("app"),
            Some("app-old"),
        );
        app.entry_id = Some("app-entry".to_string());
        app.file_id = Some("app-file".to_string());
        app.expected_relative_path = "mods/app.jar".to_string();
        let mut fabric = snapshot_item_for_test(
            Some(ContentProvider::Modrinth),
            Some("P7dR8mSH"),
            None,
        );
        fabric.entry_id = Some("fabric-entry".to_string());
        fabric.file_id = Some("fabric-file".to_string());
        fabric.expected_relative_path = "mods/fabric-api.jar".to_string();
        fabric.ownership_kind = if auto_dependency {
            ContentOwnershipKind::LocalDiscovered
        } else {
            ContentOwnershipKind::UserAdded
        };
        fabric.dependency = Some(crate::state::ContentDependencyInfo {
            auto_dependency,
            ..Default::default()
        });
        fabric.content.as_mut().unwrap().provider_refs = vec![
            ContentProviderRef::Modrinth {
                project_id: ModrinthProjectId::new("P7dR8mSH").unwrap(),
                version_id: None,
            },
            ContentProviderRef::Modrinth {
                project_id: ModrinthProjectId::new("P7dR8mSH").unwrap(),
                version_id: Some(
                    crate::state::ModrinthVersionId::new(current_release)
                        .unwrap(),
                ),
            },
        ];
        let snapshot = InstanceContentSnapshot {
            instance_id: "instance".to_string(),
            revision: 1,
            pack: None,
            items: vec![app, fabric],
            pending_manual_downloads: Vec::new(),
            warnings: Vec::new(),
        };
        let (items, installed) = snapshot_upgrade_items(&snapshot);
        let roots = roots_from_items(&items, &installed);
        let app_key = NodeKey::new(ContentProvider::Modrinth, "app");
        let fabric_key = NodeKey::new(ContentProvider::Modrinth, "P7dR8mSH");
        let mut app_candidate = candidate_for_key(&app_key, "app-new", 2);
        app_candidate.dependencies.push(CandidateDependency {
            key: fabric_key.clone(),
            version_id: Some("new".to_string()),
            kind: CandidateDependencyKind::Required,
        });
        let catalog = catalog([
            (app_key, vec![app_candidate]),
            (
                fabric_key,
                vec![candidate_for_key(
                    &NodeKey::new(ContentProvider::Modrinth, "P7dR8mSH"),
                    "new",
                    2,
                )],
            ),
        ]);
        let outcome = solve_with_installed(&roots, &installed, &catalog);
        assert!(outcome.issues.is_empty());
        materialize_solution(
            InstanceUpgradeSolutionKind::Newest,
            &outcome.solutions[0],
            &roots,
            &installed,
        )
    }

    #[test]
    fn unverified_local_mod_makes_shader_runtime_unknown() {
        let snapshot = InstanceContentSnapshot {
            instance_id: "instance".to_string(),
            revision: 1,
            pack: None,
            items: vec![snapshot_item_for_test(None, None, None)],
            pending_manual_downloads: Vec::new(),
            warnings: Vec::new(),
        };
        assert_eq!(
            source_shader_runtime(&[], &snapshot),
            ShaderRuntime::Unknown
        );
    }

    #[test]
    fn valid_custom_fixed_version_is_selected() {
        let roots = vec![root("a", "old", true)];
        let catalog = catalog([(
            key("a"),
            vec![candidate("a", "two", 2), candidate("a", "one", 1)],
        )]);
        let fixed = HashMap::from([(key("a"), "one".to_string())]);
        let outcome =
            solve_upgrade(&roots, &[], &catalog, &fixed, &HashSet::new());
        assert_eq!(
            outcome.solutions[0].assignments[&key("a")].version_id,
            "one"
        );
    }

    #[test]
    fn cached_catalog_supports_fixed_candidate_already_in_plan() {
        let catalog = catalog([(
            key("a"),
            vec![candidate("a", "newest", 2), candidate("a", "fixed", 1)],
        )]);
        let fixed = FixedRootConstraints::from_constraints(&[
            InstanceUpgradeFixedConstraint {
                content_id: "physical-a".to_string(),
                provider: ContentProvider::Modrinth,
                project_id: "a".to_string(),
                version_id: "fixed".to_string(),
            },
        ]);

        assert!(catalog_supports_fixed_constraints(&catalog, &fixed));
    }

    #[test]
    fn cached_catalog_reloads_when_fixed_candidate_is_missing_or_incompatible()
    {
        let catalog =
            catalog([(key("a"), vec![candidate("a", "compatible", 1)])]);
        let missing = FixedRootConstraints::from_constraints(&[
            InstanceUpgradeFixedConstraint {
                content_id: "physical-a".to_string(),
                provider: ContentProvider::Modrinth,
                project_id: "a".to_string(),
                version_id: "missing".to_string(),
            },
        ]);
        assert!(!catalog_supports_fixed_constraints(&catalog, &missing));

        let mut incompatible_catalog = catalog;
        incompatible_catalog.get_mut(&key("a")).unwrap().candidates[0]
            .compatible = false;
        let incompatible = FixedRootConstraints::from_constraints(&[
            InstanceUpgradeFixedConstraint {
                content_id: "physical-a".to_string(),
                provider: ContentProvider::Modrinth,
                project_id: "a".to_string(),
                version_id: "compatible".to_string(),
            },
        ]);
        assert!(!catalog_supports_fixed_constraints(
            &incompatible_catalog,
            &incompatible
        ));
    }

    #[test]
    fn custom_fixed_constraint_binds_only_matching_physical_root() {
        let mut first = root("duplicate", "first-old", true);
        first.content_id = "physical-first".to_string();
        let mut second = root("duplicate", "second-old", true);
        second.content_id = "physical-second".to_string();
        let roots = [first, second];
        let catalog = catalog([(
            key("duplicate"),
            vec![
                candidate("duplicate", "fixed", 2),
                candidate("duplicate", "flexible", 1),
            ],
        )]);
        let fixed = FixedRootConstraints::from_constraints(&[
            InstanceUpgradeFixedConstraint {
                content_id: "physical-first".to_string(),
                provider: ContentProvider::Modrinth,
                project_id: "duplicate".to_string(),
                version_id: "fixed".to_string(),
            },
        ]);

        let first_options = root_candidate_options(
            &roots[0],
            SolveStrategy::Newest,
            &catalog,
            &fixed,
            &HashSet::new(),
        );
        let second_options = root_candidate_options(
            &roots[1],
            SolveStrategy::Newest,
            &catalog,
            &fixed,
            &HashSet::new(),
        );

        assert!(first_options.fixed);
        assert_eq!(first_options.candidates.len(), 1);
        assert_eq!(
            first_options.candidates[0]
                .as_ref()
                .map(|candidate| candidate.version_id.as_str()),
            Some("fixed")
        );
        assert!(!second_options.fixed);
        assert_eq!(second_options.candidates.len(), 2);
    }

    #[test]
    fn proven_fixed_exact_conflict_takes_priority_over_search_limit() {
        let fixed = FixedRootConstraints::from_constraints(&[
            InstanceUpgradeFixedConstraint {
                content_id: "entry-a".to_string(),
                provider: ContentProvider::Modrinth,
                project_id: "a".to_string(),
                version_id: "a-fixed".to_string(),
            },
            InstanceUpgradeFixedConstraint {
                content_id: "entry-b".to_string(),
                provider: ContentProvider::Modrinth,
                project_id: "b".to_string(),
                version_id: "b-fixed".to_string(),
            },
        ]);
        let exact_conflict = issue_with_requirements(
            InstanceUpgradeIssueCode::DependencyConflict,
            "fixed exact conflict",
            Some(&key("x")),
            Some("x"),
            None,
            vec![
                InstanceUpgradeDependencyRequirement {
                    root_content_id: "entry-a".to_string(),
                    root_provider: ContentProvider::Modrinth,
                    root_project_id: "a".to_string(),
                    parent_provider: ContentProvider::Modrinth,
                    parent_project_id: "a".to_string(),
                    parent_release_id: "a-fixed".to_string(),
                    dependency_provider: ContentProvider::Modrinth,
                    dependency_project_id: "x".to_string(),
                    required_release_id: Some("x-one".to_string()),
                    candidate_release_id: None,
                },
                InstanceUpgradeDependencyRequirement {
                    root_content_id: "entry-b".to_string(),
                    root_provider: ContentProvider::Modrinth,
                    root_project_id: "b".to_string(),
                    parent_provider: ContentProvider::Modrinth,
                    parent_project_id: "b".to_string(),
                    parent_release_id: "b-fixed".to_string(),
                    dependency_provider: ContentProvider::Modrinth,
                    dependency_project_id: "x".to_string(),
                    required_release_id: Some("x-two".to_string()),
                    candidate_release_id: None,
                },
            ],
        );

        let selected = select_upgrade_failure_issue(
            [Some(search_limit_issue(false)), Some(exact_conflict)],
            &fixed,
        );

        assert_eq!(selected.code, InstanceUpgradeIssueCode::DependencyConflict);
        assert_eq!(selected.dependency_requirements.len(), 2);
    }

    #[test]
    fn custom_uses_minimal_order_for_unfixed_roots() {
        let roots =
            vec![root("a", "a-old", true), root("b", "b-current", true)];
        let catalog = catalog([
            (
                key("a"),
                vec![
                    candidate("a", "a-newest", 3),
                    candidate("a", "a-fixed", 2),
                ],
            ),
            (
                key("b"),
                vec![
                    candidate("b", "b-newest", 3),
                    candidate("b", "b-current", 2),
                ],
            ),
        ]);
        let fixed = HashMap::from([(key("a"), "a-fixed".to_string())]);

        let outcome =
            solve_upgrade(&roots, &[], &catalog, &fixed, &HashSet::new());
        assert_eq!(
            outcome.solutions[0].assignments[&key("a")].version_id,
            "a-fixed"
        );
        assert_eq!(
            outcome.solutions[0].assignments[&key("b")].version_id,
            "b-current"
        );
    }

    #[test]
    fn custom_fixed_version_conflict_returns_detail() {
        let roots = vec![root("a", "old", true), root("b", "old", true)];
        let mut a = candidate("a", "two", 2);
        a.dependencies.push(incompatible("b"));
        let catalog = catalog([
            (key("a"), vec![a, candidate("a", "one", 1)]),
            (key("b"), vec![candidate("b", "one", 1)]),
        ]);
        let fixed = HashMap::from([(key("a"), "two".to_string())]);
        let outcome =
            solve_upgrade(&roots, &[], &catalog, &fixed, &HashSet::new());
        assert!(outcome.solutions.is_empty());
        assert_eq!(
            outcome.issues[0].conflicting_project_id.as_deref(),
            Some("b")
        );
        assert!(
            outcome.issues[0]
                .dependency_requirements
                .iter()
                .any(|detail| detail.root_project_id == "a")
        );
        assert!(
            outcome.issues[0]
                .dependency_requirements
                .iter()
                .any(|detail| detail.root_project_id == "b")
        );
    }

    #[test]
    fn compatible_current_outside_exploration_limit_remains_minimal() {
        let roots = vec![root("a", "current", true)];
        let mut candidates = (1..=6)
            .map(|index| candidate("a", &format!("new-{index}"), 20 - index))
            .collect::<Vec<_>>();
        let mut current = candidate("a", "current", 1);
        current.installed_current = true;
        candidates.push(current);
        let catalog = catalog([(key("a"), candidates)]);
        let outcome = solve(&roots, &catalog);
        let minimal = outcome
            .solutions
            .iter()
            .min_by(|left, right| compare_minimal(left, right, &roots, &[]))
            .unwrap();
        assert_eq!(minimal.assignments[&key("a")].version_id, "current");
    }

    #[test]
    fn custom_fixed_candidate_outside_exploration_limit_is_not_truncated() {
        let roots = vec![root("a", "old", true)];
        let mut candidates = (1..=6)
            .map(|index| candidate("a", &format!("new-{index}"), 20 - index))
            .collect::<Vec<_>>();
        candidates.push(candidate("a", "fixed-seven", 1));
        let catalog = catalog([(key("a"), candidates)]);
        let fixed = HashMap::from([(key("a"), "fixed-seven".to_string())]);
        let outcome =
            solve_upgrade(&roots, &[], &catalog, &fixed, &HashSet::new());
        assert_eq!(
            outcome.solutions[0].assignments[&key("a")].version_id,
            "fixed-seven"
        );
    }

    #[test]
    fn custom_fixed_version_from_another_project_is_rejected() {
        let version = test_version(vec!["fabric"], vec!["26.1"]);
        let error = validate_modrinth_custom_fixed(
            &key("different-project"),
            &version,
            ProjectType::Mod,
            &test_environment(ShaderRuntime::Iris),
        )
        .unwrap_err();
        assert!(error.to_string().contains("belongs to project"));
    }

    #[test]
    fn custom_fixed_version_incompatible_with_target_is_rejected() {
        let version = test_version(vec!["fabric"], vec!["1.20.1"]);
        let error = validate_modrinth_custom_fixed(
            &key("project"),
            &version,
            ProjectType::Mod,
            &test_environment(ShaderRuntime::Iris),
        )
        .unwrap_err();
        assert!(error.to_string().contains("not compatible"));
    }

    #[test]
    fn candidate_limit_returns_limit_issue_instead_of_false_conflict() {
        let roots = vec![root("a", "old", true)];
        let mut pool = CandidatePool {
            candidates: vec![candidate("a", "one", 1)],
            exploration_limited: true,
            has_target_game_version_release: true,
        };
        pool.candidates[0]
            .dependencies
            .push(required("missing", None));
        let catalog = HashMap::from([(key("a"), pool)]);
        let outcome = solve(&roots, &catalog);
        assert_eq!(
            outcome.issues[0].code,
            InstanceUpgradeIssueCode::SearchLimitReached
        );
    }

    #[test]
    fn failed_truncated_branch_does_not_block_valid_solution() {
        let roots = vec![root("a", "old-a", true), root("b", "old-b", true)];
        let mut conflicting = candidate("a", "a-conflict", 3);
        conflicting.dependencies.push(incompatible("b"));
        let catalog = HashMap::from([
            (
                key("a"),
                CandidatePool {
                    candidates: vec![conflicting, candidate("a", "a-valid", 2)],
                    exploration_limited: true,
                    has_target_game_version_release: true,
                },
            ),
            (
                key("b"),
                CandidatePool {
                    candidates: vec![candidate("b", "b-one", 1)],
                    exploration_limited: false,
                    has_target_game_version_release: true,
                },
            ),
        ]);
        let outcome = solve(&roots, &catalog);
        assert!(!outcome.solutions.is_empty());
        assert!(outcome.issues.iter().all(|issue| {
            issue.code != InstanceUpgradeIssueCode::SearchLimitReached
        }));
    }

    #[test]
    fn exact_conflict_is_not_hidden_by_exact_pool_candidate_limit() {
        let roots = vec![root("a", "old-a", true), root("b", "old-b", true)];
        let mut a = candidate("a", "a-one", 2);
        a.dependencies.push(required("x", Some("x-two")));
        let mut b = candidate("b", "b-one", 2);
        b.dependencies.push(required("x", Some("x-three")));
        let catalog = HashMap::from([
            (
                key("a"),
                CandidatePool {
                    candidates: vec![a],
                    exploration_limited: false,
                    has_target_game_version_release: true,
                },
            ),
            (
                key("b"),
                CandidatePool {
                    candidates: vec![b],
                    exploration_limited: false,
                    has_target_game_version_release: true,
                },
            ),
            (
                key("x"),
                CandidatePool {
                    candidates: vec![
                        candidate("x", "x-two", 1),
                        candidate("x", "x-three", 1),
                    ],
                    exploration_limited: true,
                    has_target_game_version_release: true,
                },
            ),
        ]);
        let outcome = solve(&roots, &catalog);
        assert_eq!(
            outcome.issues[0].code,
            InstanceUpgradeIssueCode::DependencyConflict
        );
    }

    #[test]
    fn provider_candidate_filter_finds_compatible_item_after_first_fifty() {
        let provider_files = (0..=50).collect::<Vec<_>>();
        let (selected, limited) =
            bounded_compatible_candidates(&provider_files, |file| *file == 50);
        assert_eq!(selected, vec![50]);
        assert!(!limited);
    }

    #[test]
    fn exact_dependency_conflict_contains_both_root_provenances() {
        let roots = vec![root("a", "old-a", true), root("b", "old-b", true)];
        let mut a = candidate("a", "a-one", 2);
        a.dependencies.push(required("x", Some("x-two")));
        let mut b = candidate("b", "b-one", 2);
        b.dependencies.push(required("x", Some("x-three")));
        let catalog = catalog([
            (key("a"), vec![a]),
            (key("b"), vec![b]),
            (
                key("x"),
                vec![candidate("x", "x-two", 1), candidate("x", "x-three", 1)],
            ),
        ]);
        let outcome = solve(&roots, &catalog);
        let details = &outcome.issues[0].dependency_requirements;
        assert_eq!(details.len(), 2);
        assert!(details.iter().any(|detail| {
            detail.root_project_id == "a"
                && detail.required_release_id.as_deref() == Some("x-two")
        }));
        assert!(details.iter().any(|detail| {
            detail.root_project_id == "b"
                && detail.required_release_id.as_deref() == Some("x-three")
        }));
    }

    fn fixed_exact_conflict_fixture(
        target_exploration_limited: bool,
    ) -> (Vec<RootRequest>, UpgradeCatalog, HashMap<NodeKey, String>) {
        let roots = vec![
            root("a", "a-old", true),
            root("b", "b-old", true),
            root("x", "x-old", true),
        ];
        let mut a = candidate("a", "a-fixed", 3);
        a.dependencies.push(required("x", Some("x-one")));
        let mut b = candidate("b", "b-fixed", 3);
        b.dependencies.push(required("x", Some("x-two")));
        let catalog = HashMap::from([
            (
                key("a"),
                CandidatePool {
                    candidates: vec![a],
                    exploration_limited: false,
                    has_target_game_version_release: true,
                },
            ),
            (
                key("b"),
                CandidatePool {
                    candidates: vec![b],
                    exploration_limited: false,
                    has_target_game_version_release: true,
                },
            ),
            (
                key("x"),
                CandidatePool {
                    candidates: vec![
                        candidate("x", "x-one", 2),
                        candidate("x", "x-two", 1),
                    ],
                    exploration_limited: target_exploration_limited,
                    has_target_game_version_release: true,
                },
            ),
        ]);
        let fixed = HashMap::from([
            (key("a"), "a-fixed".to_string()),
            (key("b"), "b-fixed".to_string()),
        ]);

        (roots, catalog, fixed)
    }

    fn fixed_exact_conflict_with_flexible_target(
        target_exploration_limited: bool,
    ) -> SolveOutcome {
        let (roots, catalog, fixed) =
            fixed_exact_conflict_fixture(target_exploration_limited);
        solve_upgrade(&roots, &[], &catalog, &fixed, &HashSet::new())
    }

    #[test]
    fn fixed_exact_dependency_conflict_ignores_flexible_target_root() {
        let outcome = fixed_exact_conflict_with_flexible_target(false);
        assert!(outcome.solutions.is_empty());
        assert_eq!(
            outcome.issues[0].code,
            InstanceUpgradeIssueCode::DependencyConflict
        );
        let details = &outcome.issues[0].dependency_requirements;
        assert_eq!(details.len(), 2);
        assert!(details.iter().any(|detail| {
            detail.root_project_id == "a"
                && detail.parent_release_id == "a-fixed"
                && detail.dependency_project_id == "x"
                && detail.required_release_id.as_deref() == Some("x-one")
        }));
        assert!(details.iter().any(|detail| {
            detail.root_project_id == "b"
                && detail.parent_release_id == "b-fixed"
                && detail.dependency_project_id == "x"
                && detail.required_release_id.as_deref() == Some("x-two")
        }));
    }

    #[test]
    fn fixed_exact_dependency_conflict_ignores_truncated_target_root() {
        let outcome = fixed_exact_conflict_with_flexible_target(true);
        assert!(outcome.solutions.is_empty());
        assert_eq!(
            outcome.issues[0].code,
            InstanceUpgradeIssueCode::DependencyConflict
        );
        assert_eq!(outcome.issues[0].dependency_requirements.len(), 2);
    }

    #[test]
    fn custom_fixed_exact_conflict_does_not_branch_flexible_physical_target() {
        let all_roots = [
            root("iris", "iris-old", true),
            root("voxy", "voxy-old", true),
            root("sodium", "sodium-current", true),
        ];
        let mut iris = candidate("iris", "iris-fixed", 10);
        iris.dependencies
            .push(required("sodium", Some("sodium-for-iris")));
        let mut voxy = candidate("voxy", "voxy-fixed", 9);
        voxy.dependencies
            .push(required("sodium", Some("sodium-for-voxy")));
        let catalog = HashMap::from([
            (
                key("iris"),
                CandidatePool {
                    candidates: vec![iris],
                    exploration_limited: false,
                    has_target_game_version_release: true,
                },
            ),
            (
                key("voxy"),
                CandidatePool {
                    candidates: vec![voxy],
                    exploration_limited: false,
                    has_target_game_version_release: true,
                },
            ),
            (
                key("sodium"),
                CandidatePool {
                    candidates: vec![
                        candidate("sodium", "sodium-current", 8),
                        candidate("sodium", "sodium-for-iris", 7),
                        candidate("sodium", "sodium-for-voxy", 6),
                        candidate("sodium", "sodium-other", 5),
                    ],
                    exploration_limited: true,
                    has_target_game_version_release: true,
                },
            ),
        ]);
        let fixed = HashMap::from([
            (key("iris"), "iris-fixed".to_string()),
            (key("voxy"), "voxy-fixed".to_string()),
        ]);

        for order in [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ] {
            let roots = order
                .map(|index| all_roots[index].clone())
                .into_iter()
                .collect::<Vec<_>>();
            let outcome =
                solve_upgrade(&roots, &[], &catalog, &fixed, &HashSet::new());

            assert!(outcome.solutions.is_empty(), "root order {order:?}");
            assert_eq!(outcome.visited_states, 0, "root order {order:?}");
            assert_eq!(outcome.issues.len(), 1, "root order {order:?}");
            let issue = &outcome.issues[0];
            assert_eq!(
                issue.code,
                InstanceUpgradeIssueCode::DependencyConflict,
                "root order {order:?}"
            );
            assert_eq!(issue.project_id.as_deref(), Some("sodium"));
            assert_eq!(issue.dependency_requirements.len(), 2);
            assert!(issue.dependency_requirements.iter().any(|requirement| {
                requirement.root_project_id == "iris"
                    && requirement.parent_release_id == "iris-fixed"
                    && requirement.required_release_id.as_deref()
                        == Some("sodium-for-iris")
            }));
            assert!(issue.dependency_requirements.iter().any(|requirement| {
                requirement.root_project_id == "voxy"
                    && requirement.parent_release_id == "voxy-fixed"
                    && requirement.required_release_id.as_deref()
                        == Some("sodium-for-voxy")
            }));
            assert!(issue.dependency_requirements.iter().all(|requirement| {
                requirement.root_project_id != "sodium"
                    && requirement.parent_project_id != "sodium"
            }));
        }
    }

    #[test]
    fn fixed_exact_dependency_conflict_is_identical_for_both_strategies() {
        let (roots, catalog, fixed) = fixed_exact_conflict_fixture(false);
        let aliases = InstalledAliasIndex::new(&[]);

        for strategy in [SolveStrategy::Newest, SolveStrategy::MinimalChange] {
            let outcome = solve_for_strategy(
                strategy,
                &roots,
                &catalog,
                &fixed,
                &HashSet::new(),
                &aliases,
            );
            let issue = outcome.issue.expect("strategy should report conflict");
            assert!(outcome.solution.is_none());
            assert_eq!(
                issue.code,
                InstanceUpgradeIssueCode::DependencyConflict
            );
            assert_eq!(issue.dependency_requirements.len(), 2);
            assert!(issue.dependency_requirements.iter().any(|detail| {
                detail.root_project_id == "a"
                    && detail.parent_release_id == "a-fixed"
                    && detail.dependency_project_id == "x"
                    && detail.required_release_id.as_deref() == Some("x-one")
            }));
            assert!(issue.dependency_requirements.iter().any(|detail| {
                detail.root_project_id == "b"
                    && detail.parent_release_id == "b-fixed"
                    && detail.dependency_project_id == "x"
                    && detail.required_release_id.as_deref() == Some("x-two")
            }));
        }
    }

    #[test]
    fn flexible_root_falls_back_to_fixed_roots_exact_dependency() {
        let roots = vec![
            root("a", "a-old", true),
            root("b", "b-old", true),
            root("x", "x-old", true),
        ];
        let mut a = candidate("a", "a-fixed", 4);
        a.dependencies.push(required("x", Some("x-one")));
        let mut b_latest = candidate("b", "b-latest", 4);
        b_latest.dependencies.push(required("x", Some("x-two")));
        let mut b_previous = candidate("b", "b-previous", 3);
        b_previous.dependencies.push(required("x", Some("x-one")));
        let catalog = catalog([
            (key("a"), vec![a]),
            (key("b"), vec![b_latest, b_previous]),
            (
                key("x"),
                vec![candidate("x", "x-one", 2), candidate("x", "x-two", 1)],
            ),
        ]);
        let fixed = HashMap::from([(key("a"), "a-fixed".to_string())]);

        let outcome =
            solve_upgrade(&roots, &[], &catalog, &fixed, &HashSet::new());
        assert!(outcome.issues.is_empty());
        assert_eq!(
            outcome.solutions[0].assignments[&key("a")].version_id,
            "a-fixed"
        );
        assert_eq!(
            outcome.solutions[0].assignments[&key("b")].version_id,
            "b-previous"
        );
    }

    #[test]
    fn truncated_flexible_conflict_root_reports_search_limit() {
        let roots = vec![
            root("a", "a-old", true),
            root("b", "b-old", true),
            root("x", "x-old", true),
        ];
        let mut a = candidate("a", "a-fixed", 10);
        a.dependencies.push(required("x", Some("x-one")));
        let b_candidates = (0..MAX_CANDIDATES_PER_PROJECT)
            .map(|index| {
                let mut candidate =
                    candidate("b", &format!("b-{index}"), 9 - index as i64);
                candidate.dependencies.push(required("x", Some("x-two")));
                candidate
            })
            .collect::<Vec<_>>();
        let catalog = HashMap::from([
            (
                key("a"),
                CandidatePool {
                    candidates: vec![a],
                    exploration_limited: false,
                    has_target_game_version_release: true,
                },
            ),
            (
                key("b"),
                CandidatePool {
                    candidates: b_candidates,
                    exploration_limited: true,
                    has_target_game_version_release: true,
                },
            ),
            (
                key("x"),
                CandidatePool {
                    candidates: vec![
                        candidate("x", "x-one", 2),
                        candidate("x", "x-two", 1),
                    ],
                    exploration_limited: false,
                    has_target_game_version_release: true,
                },
            ),
        ]);
        let fixed = HashMap::from([(key("a"), "a-fixed".to_string())]);

        let outcome =
            solve_upgrade(&roots, &[], &catalog, &fixed, &HashSet::new());
        assert!(outcome.solutions.is_empty());
        assert_eq!(
            outcome.issues[0].code,
            InstanceUpgradeIssueCode::SearchLimitReached
        );
    }

    fn conflict_scope(
        dependency_project: &str,
        causal_root: &str,
        candidate_limit_root: Option<&str>,
    ) -> ConflictSet {
        ConflictSet {
            involved_root_content_ids: HashSet::from([causal_root.to_string()]),
            involved_parent_projects: HashSet::from([key(causal_root)]),
            dependency_project: Some(key(dependency_project)),
            reason: InstanceUpgradeIssueCode::DependencyConflict,
            candidate_limit_roots: candidate_limit_root
                .map(str::to_string)
                .into_iter()
                .collect(),
        }
    }

    #[test]
    fn earlier_unrelated_candidate_limit_does_not_taint_best_conflict() {
        let mut best = None;
        retain_best_failure(
            &mut best,
            ConflictFailure {
                issue: issue(
                    InstanceUpgradeIssueCode::DependencyConflict,
                    "earlier truncated conflict",
                    None,
                    None,
                    None,
                ),
                conflict: conflict_scope("y", "c", Some("c")),
            },
        );
        retain_best_failure(
            &mut best,
            ConflictFailure {
                issue: issue(
                    InstanceUpgradeIssueCode::DependencyConflict,
                    "later fixed exact conflict",
                    None,
                    None,
                    None,
                ),
                conflict: conflict_scope("x", "a", None),
            },
        );

        let best = best.expect("best conflict should exist");
        assert_eq!(best.conflict.dependency_project, Some(key("x")));
        assert!(!best.conflict.candidate_search_incomplete());
    }

    #[test]
    fn later_unrelated_candidate_limit_does_not_taint_best_conflict() {
        let mut best = None;
        retain_best_failure(
            &mut best,
            ConflictFailure {
                issue: issue(
                    InstanceUpgradeIssueCode::DependencyConflict,
                    "earlier fixed exact conflict",
                    None,
                    None,
                    None,
                ),
                conflict: conflict_scope("x", "a", None),
            },
        );
        retain_best_failure(
            &mut best,
            ConflictFailure {
                issue: issue(
                    InstanceUpgradeIssueCode::DependencyConflict,
                    "later truncated conflict",
                    None,
                    None,
                    None,
                ),
                conflict: conflict_scope("y", "c", Some("c")),
            },
        );

        let best = best.expect("best conflict should exist");
        assert_eq!(best.conflict.dependency_project, Some(key("x")));
        assert!(!best.conflict.candidate_search_incomplete());
    }

    #[test]
    fn same_conflict_candidate_limit_marks_search_incomplete() {
        let mut best = conflict_scope("x", "a", None);
        let same_conflict = conflict_scope("x", "a", Some("a"));

        best.merge_candidate_limit_evidence(&same_conflict);

        assert!(best.candidate_search_incomplete());
    }

    #[test]
    fn unrelated_truncated_root_does_not_hide_fixed_exact_conflict() {
        let roots = vec![
            root("a", "a-old", true),
            root("b", "b-old", true),
            root("c", "c-old", true),
            root("x", "x-old", true),
        ];
        let mut a = candidate("a", "a-fixed", 4);
        a.dependencies.push(required("x", Some("x-one")));
        let mut b = candidate("b", "b-fixed", 4);
        b.dependencies.push(required("x", Some("x-two")));
        let catalog = HashMap::from([
            (
                key("a"),
                CandidatePool {
                    candidates: vec![a],
                    exploration_limited: false,
                    has_target_game_version_release: true,
                },
            ),
            (
                key("b"),
                CandidatePool {
                    candidates: vec![b],
                    exploration_limited: false,
                    has_target_game_version_release: true,
                },
            ),
            (
                key("c"),
                CandidatePool {
                    candidates: vec![candidate("c", "c-latest", 3)],
                    exploration_limited: true,
                    has_target_game_version_release: true,
                },
            ),
            (
                key("x"),
                CandidatePool {
                    candidates: vec![
                        candidate("x", "x-one", 2),
                        candidate("x", "x-two", 1),
                    ],
                    exploration_limited: true,
                    has_target_game_version_release: true,
                },
            ),
        ]);
        let fixed = HashMap::from([
            (key("a"), "a-fixed".to_string()),
            (key("b"), "b-fixed".to_string()),
        ]);

        let outcome =
            solve_upgrade(&roots, &[], &catalog, &fixed, &HashSet::new());
        assert!(outcome.solutions.is_empty());
        assert_eq!(
            outcome.issues[0].code,
            InstanceUpgradeIssueCode::DependencyConflict
        );
        assert_eq!(outcome.issues[0].dependency_requirements.len(), 2);
    }

    #[test]
    fn transitive_missing_dependency_reports_root_and_direct_parent() {
        let roots = vec![root("a", "old", true)];
        let mut a = candidate("a", "a-one", 3);
        a.dependencies.push(required("x", None));
        let mut x = candidate("x", "x-one", 2);
        x.dependencies
            .push(required("missing", Some("missing-one")));
        let catalog = catalog([(key("a"), vec![a]), (key("x"), vec![x])]);
        let outcome = solve(&roots, &catalog);
        let detail = &outcome.issues[0].dependency_requirements[0];
        assert_eq!(detail.root_project_id, "a");
        assert_eq!(detail.parent_project_id, "x");
        assert_eq!(detail.parent_release_id, "x-one");
        assert_eq!(detail.dependency_project_id, "missing");
    }

    #[test]
    fn shared_transitive_missing_dependency_reports_every_root() {
        let roots = vec![root("a", "old-a", true), root("b", "old-b", true)];
        let mut a = candidate("a", "a-one", 4);
        a.dependencies.push(required("x", None));
        let mut b = candidate("b", "b-one", 4);
        b.dependencies.push(required("x", None));
        let mut x = candidate("x", "x-one", 3);
        x.dependencies.push(required("y", Some("y-one")));
        let catalog = catalog([
            (key("a"), vec![a]),
            (key("b"), vec![b]),
            (key("x"), vec![x]),
        ]);
        let outcome = solve(&roots, &catalog);
        assert_eq!(
            outcome.issues[0].code,
            InstanceUpgradeIssueCode::MissingRequiredDependency
        );
        let details = &outcome.issues[0].dependency_requirements;
        assert_eq!(details.len(), 2);
        assert!(details.iter().any(|detail| detail.root_project_id == "a"));
        assert!(details.iter().any(|detail| detail.root_project_id == "b"));
        assert!(details.iter().all(|detail| {
            detail.parent_project_id == "x"
                && detail.parent_release_id == "x-one"
                && detail.dependency_project_id == "y"
        }));
    }

    #[test]
    fn version_scoped_incompatible_edge_only_rejects_exact_version() {
        let roots = vec![root("a", "old-a", true), root("x", "old-x", true)];
        let mut a = candidate("a", "a-one", 2);
        a.dependencies.push(CandidateDependency {
            key: key("x"),
            version_id: Some("x-two".to_string()),
            kind: CandidateDependencyKind::Incompatible,
        });
        let compatible_catalog = catalog([
            (key("a"), vec![a.clone()]),
            (key("x"), vec![candidate("x", "x-three", 1)]),
        ]);
        assert!(!solve(&roots, &compatible_catalog).solutions.is_empty());
        let incompatible_catalog = catalog([
            (key("a"), vec![a]),
            (key("x"), vec![candidate("x", "x-two", 1)]),
        ]);
        assert!(solve(&roots, &incompatible_catalog).solutions.is_empty());
    }

    #[test]
    fn exact_incompatible_matches_proven_current_cross_provider_alias() {
        let mr_x = key("x-mr");
        let cf_x = NodeKey::new(ContentProvider::CurseForge, "x-cf");
        let installed_x = installed_with_aliases(
            "entry-x",
            &mr_x,
            "current-mr",
            false,
            true,
            vec![(&cf_x, "current-cf")],
        );
        let aliases = InstalledAliasIndex::new(&[installed_x]);
        let mut current = candidate("x-mr", "current-mr", 1);
        current.installed_current = true;
        let mut incompatible_candidate = candidate("a", "a-one", 2);
        incompatible_candidate
            .dependencies
            .push(CandidateDependency {
                key: cf_x,
                version_id: Some("current-cf".to_string()),
                kind: CandidateDependencyKind::Incompatible,
            });
        let assignments = HashMap::from([(mr_x.clone(), current)]);
        let physical_assignments =
            HashMap::from([("entry-x".to_string(), mr_x.clone())]);
        assert_eq!(
            incompatible_with_assignments(
                &incompatible_candidate,
                &assignments,
                &physical_assignments,
                &aliases,
            ),
            Some(mr_x)
        );
    }

    #[test]
    fn reverse_exact_incompatible_matches_proven_current_alias_only() {
        let mr_x = key("x-mr");
        let cf_x = NodeKey::new(ContentProvider::CurseForge, "x-cf");
        let installed_x = installed_with_aliases(
            "entry-x",
            &mr_x,
            "current-mr",
            false,
            true,
            vec![(&cf_x, "current-cf")],
        );
        let aliases = InstalledAliasIndex::new(&[installed_x]);
        let mut selected = candidate("a", "a-one", 2);
        selected.dependencies.push(CandidateDependency {
            key: cf_x,
            version_id: Some("current-cf".to_string()),
            kind: CandidateDependencyKind::Incompatible,
        });
        let assignments = HashMap::from([(key("a"), selected)]);
        let mut current = candidate("x-mr", "current-mr", 1);
        current.installed_current = true;
        assert_eq!(
            incompatible_with_assignments(
                &current,
                &assignments,
                &HashMap::new(),
                &aliases,
            ),
            Some(key("a"))
        );

        let target = candidate("x-mr", "target-mr", 3);
        assert_eq!(
            incompatible_with_assignments(
                &target,
                &assignments,
                &HashMap::new(),
                &aliases,
            ),
            None
        );
    }

    #[test]
    fn keep_incompatible_root_preserves_unsafe_dependency_closure() {
        let mut keep = root("a", "a-old", true);
        keep.action = InstanceUpgradeAction::Keep;
        let mut a = candidate("a", "a-old", 2);
        a.compatible = false;
        a.installed_current = true;
        a.dependencies.push(required("x", Some("x-old")));
        let mut x = candidate("x", "x-old", 1);
        x.compatible = false;
        x.installed_current = true;
        let catalog = catalog([(key("a"), vec![a]), (key("x"), vec![x])]);
        let nodes = vec![installed("x", "x-old", true, false)];
        let outcome = solve(&[keep.clone()], &catalog);
        assert_eq!(outcome.solutions.len(), 1);
        let solution = materialize_solution(
            InstanceUpgradeSolutionKind::Newest,
            &outcome.solutions[0],
            &[keep],
            &nodes,
        );
        assert!(solution.warnings.len() >= 2);
        assert!(solution.dependency_changes.iter().all(|change| {
            change.kind != InstanceUpgradeDependencyChangeKind::Remove
        }));
    }

    #[test]
    fn disable_incompatible_root_keeps_old_dependency_disabled() {
        let mut disabled = root("a", "a-old", true);
        disabled.action = InstanceUpgradeAction::Disable;
        let mut a = candidate("a", "a-old", 2);
        a.compatible = false;
        a.installed_current = true;
        a.dependencies.push(required("x", Some("x-old")));
        let mut x = candidate("x", "x-old", 1);
        x.compatible = false;
        x.installed_current = true;
        let catalog = catalog([(key("a"), vec![a]), (key("x"), vec![x])]);
        let outcome = solve(&[disabled.clone()], &catalog);
        let solution = materialize_solution(
            InstanceUpgradeSolutionKind::Newest,
            &outcome.solutions[0],
            &[disabled],
            &[],
        );
        assert!(!solution.selections[0].enabled);
        assert!(!solution.dependency_changes[0].enabled);
        assert!(solution.warnings.is_empty());
    }

    #[test]
    fn enabled_root_dependency_wins_over_disabled_preserved_dependency() {
        let mut disabled = root("d", "d-old", true);
        disabled.action = InstanceUpgradeAction::Disable;
        let enabled = root("a", "a-old", true);
        let mut d = candidate("d", "d-old", 3);
        d.installed_current = true;
        d.compatible = false;
        d.dependencies.push(required("x", Some("x-old")));
        let mut a = candidate("a", "a-new", 3);
        a.dependencies.push(required("x", Some("x-new")));
        let mut old_x = candidate("x", "x-old", 1);
        old_x.installed_current = true;
        old_x.compatible = false;
        let catalog = catalog([
            (key("d"), vec![d]),
            (key("a"), vec![a]),
            (key("x"), vec![candidate("x", "x-new", 2), old_x]),
        ]);
        let roots = vec![disabled, enabled];
        let outcome = solve(&roots, &catalog);
        let selected = &outcome.solutions[0];
        assert_eq!(selected.assignments[&key("x")].version_id, "x-new");
        let solution = materialize_solution(
            InstanceUpgradeSolutionKind::Newest,
            selected,
            &roots,
            &[],
        );
        assert!(
            solution
                .dependency_changes
                .iter()
                .find(|change| { change.project_id == "x" })
                .unwrap()
                .enabled
        );
    }

    #[test]
    fn prerelease_dependency_requires_and_records_exact_confirmation() {
        let roots = vec![root("a", "old", true)];
        let mut a = candidate("a", "a-one", 2);
        a.dependencies.push(required("x", Some("x-beta")));
        let mut beta = candidate("x", "x-beta", 1);
        beta.channel = CandidateChannel::Beta;
        let catalog = catalog([(key("a"), vec![a]), (key("x"), vec![beta])]);
        let blocked = solve(&roots, &catalog);
        assert_eq!(
            blocked.issues[0].code,
            InstanceUpgradeIssueCode::PrereleaseOnly
        );
        assert_eq!(
            blocked.issues[0].dependency_requirements[0]
                .candidate_release_id
                .as_deref(),
            Some("x-beta")
        );
        let confirmations = HashSet::from([(key("x"), "x-beta".to_string())]);
        let allowed = solve_upgrade(
            &roots,
            &[],
            &catalog,
            &HashMap::new(),
            &confirmations,
        );
        assert_eq!(allowed.solutions.len(), 1);
    }

    #[test]
    fn custom_fixed_beta_root_is_an_explicit_confirmation() {
        let roots = vec![root("a", "old", true)];
        let mut beta = candidate("a", "a-beta", 1);
        beta.channel = CandidateChannel::Beta;
        let catalog = catalog([(key("a"), vec![beta])]);
        let fixed = HashMap::from([(key("a"), "a-beta".to_string())]);
        assert_eq!(
            solve_upgrade(&roots, &[], &catalog, &fixed, &HashSet::new(),)
                .solutions
                .len(),
            1
        );
    }

    #[test]
    fn trusted_cross_provider_alias_prevents_duplicate_dependency_add() {
        let cf_root = NodeKey::new(ContentProvider::CurseForge, "root-cf");
        let cf_dep = NodeKey::new(ContentProvider::CurseForge, "dep-cf");
        let roots = vec![RootRequest {
            content_id: "root".to_string(),
            key: cf_root.clone(),
            current_release_id: "root-old".to_string(),
            enabled: true,
            action: InstanceUpgradeAction::Upgrade,
            allow_prerelease: false,
        }];
        let root_candidate = UpgradeCandidate {
            key: cf_root.clone(),
            version_id: "root-new".to_string(),
            published: Utc.timestamp_opt(2, 0).single().unwrap(),
            channel: CandidateChannel::Release,
            compatible: true,
            installed_current: false,
            dependencies: vec![CandidateDependency {
                key: cf_dep.clone(),
                version_id: Some("dep-file".to_string()),
                kind: CandidateDependencyKind::Required,
            }],
        };
        let dep_candidate = UpgradeCandidate {
            key: cf_dep.clone(),
            version_id: "dep-file".to_string(),
            published: Utc.timestamp_opt(1, 0).single().unwrap(),
            channel: CandidateChannel::Release,
            compatible: true,
            installed_current: true,
            dependencies: Vec::new(),
        };
        let catalog = catalog([
            (cf_root, vec![root_candidate]),
            (cf_dep.clone(), vec![dep_candidate]),
        ]);
        let installed_dep = InstalledNode {
            content_id: "dep-entry".to_string(),
            key: key("dep-mr"),
            current_release_id: "dep-mr-version".to_string(),
            project_type: ProjectType::Mod,
            enabled: true,
            auto_dependency: true,
            user_owned: false,
            migratable: true,
            aliases: vec![
                InstalledAlias {
                    key: key("dep-mr"),
                    current_release_id: "dep-mr-version".to_string(),
                },
                InstalledAlias {
                    key: cf_dep,
                    current_release_id: "dep-file".to_string(),
                },
            ],
        };
        let outcome = solve(&roots, &catalog);
        let solution = materialize_solution(
            InstanceUpgradeSolutionKind::Newest,
            &outcome.solutions[0],
            &roots,
            &[installed_dep],
        );
        assert_eq!(
            solution.dependency_changes[0].kind,
            InstanceUpgradeDependencyChangeKind::Keep
        );
    }

    #[test]
    fn trusted_alias_root_and_cross_provider_requirement_share_one_assignment()
    {
        let mr_x = key("x-mr");
        let cf_x = NodeKey::new(ContentProvider::CurseForge, "x-cf");
        let cf_b = NodeKey::new(ContentProvider::CurseForge, "b-cf");
        let roots = vec![
            root("x-mr", "x-mr-old", true),
            RootRequest {
                content_id: "entry-b".to_string(),
                key: cf_b.clone(),
                current_release_id: "b-old".to_string(),
                enabled: true,
                action: InstanceUpgradeAction::Upgrade,
                allow_prerelease: false,
            },
        ];
        let mut b = candidate_for_key(&cf_b, "b-new", 3);
        b.dependencies.push(CandidateDependency {
            key: cf_x.clone(),
            version_id: None,
            kind: CandidateDependencyKind::Required,
        });
        let installed_x = installed_with_aliases(
            "entry-x",
            &mr_x,
            "x-mr-old",
            false,
            true,
            vec![(&cf_x, "x-cf-old")],
        );
        let catalog = catalog([
            (mr_x.clone(), vec![candidate("x-mr", "x-mr-new", 4)]),
            (cf_b, vec![b]),
            (cf_x.clone(), vec![candidate_for_key(&cf_x, "x-cf-new", 2)]),
        ]);
        let outcome =
            solve_with_installed(&roots, &[installed_x.clone()], &catalog);
        let selected = &outcome.solutions[0];
        assert!(selected.assignments.contains_key(&mr_x));
        assert!(!selected.assignments.contains_key(&cf_x));
        let solution = materialize_solution(
            InstanceUpgradeSolutionKind::Newest,
            selected,
            &roots,
            &[installed_x],
        );
        assert!(solution.dependency_changes.iter().all(|change| {
            change.existing_content_id.as_deref() != Some("entry-x")
        }));
    }

    #[test]
    fn shared_auto_dependency_aliases_materialize_one_change() {
        let mr_x = key("x-mr");
        let cf_x = NodeKey::new(ContentProvider::CurseForge, "x-cf");
        let cf_b = NodeKey::new(ContentProvider::CurseForge, "b-cf");
        let mut a = candidate("a", "a-new", 4);
        a.dependencies.push(required("x-mr", None));
        let mut b = candidate_for_key(&cf_b, "b-new", 4);
        b.dependencies.push(CandidateDependency {
            key: cf_x.clone(),
            version_id: None,
            kind: CandidateDependencyKind::Required,
        });
        let roots = vec![
            root("a", "a-old", true),
            RootRequest {
                content_id: "entry-b".to_string(),
                key: cf_b.clone(),
                current_release_id: "b-old".to_string(),
                enabled: true,
                action: InstanceUpgradeAction::Upgrade,
                allow_prerelease: false,
            },
        ];
        let installed_x = installed_with_aliases(
            "entry-x",
            &mr_x,
            "x-mr-old",
            true,
            false,
            vec![(&cf_x, "x-cf-old")],
        );
        let catalog = catalog([
            (key("a"), vec![a]),
            (cf_b, vec![b]),
            (mr_x, vec![candidate("x-mr", "x-mr-new", 3)]),
            (cf_x.clone(), vec![candidate_for_key(&cf_x, "x-cf-new", 2)]),
        ]);
        let outcome =
            solve_with_installed(&roots, &[installed_x.clone()], &catalog);
        let solution = materialize_solution(
            InstanceUpgradeSolutionKind::Newest,
            &outcome.solutions[0],
            &roots,
            &[installed_x],
        );
        let x_changes = solution
            .dependency_changes
            .iter()
            .filter(|change| {
                change.existing_content_id.as_deref() == Some("entry-x")
            })
            .collect::<Vec<_>>();
        assert_eq!(x_changes.len(), 1);
    }

    #[test]
    fn cross_provider_exact_alias_requirement_is_not_guessed() {
        let mr_x = key("x-mr");
        let cf_x = NodeKey::new(ContentProvider::CurseForge, "x-cf");
        let cf_b = NodeKey::new(ContentProvider::CurseForge, "b-cf");
        let mut b = candidate_for_key(&cf_b, "b-new", 3);
        b.dependencies.push(CandidateDependency {
            key: cf_x.clone(),
            version_id: Some("x-cf-new".to_string()),
            kind: CandidateDependencyKind::Required,
        });
        let roots = vec![
            root("x-mr", "x-mr-old", true),
            RootRequest {
                content_id: "entry-b".to_string(),
                key: cf_b.clone(),
                current_release_id: "b-old".to_string(),
                enabled: true,
                action: InstanceUpgradeAction::Upgrade,
                allow_prerelease: false,
            },
        ];
        let installed_x = installed_with_aliases(
            "entry-x",
            &mr_x,
            "x-mr-old",
            false,
            true,
            vec![(&cf_x, "x-cf-old")],
        );
        let catalog = catalog([
            (mr_x, vec![candidate("x-mr", "x-mr-new", 4)]),
            (cf_b, vec![b]),
            (cf_x.clone(), vec![candidate_for_key(&cf_x, "x-cf-new", 2)]),
        ]);
        let outcome = solve_with_installed(&roots, &[installed_x], &catalog);
        assert!(outcome.solutions.is_empty());
        assert_eq!(
            outcome.issues[0].code,
            InstanceUpgradeIssueCode::DependencyConflict
        );
        assert!(
            outcome.issues[0]
                .message
                .contains("cannot be proven equivalent")
        );
    }

    #[test]
    fn planner_snapshot_analysis_does_not_mutate_instance_state() {
        let snapshot = InstanceContentSnapshot {
            instance_id: "instance".to_string(),
            revision: 7,
            pack: None,
            items: vec![InstanceContentSnapshotItem {
                artifact_ref: None,
                compatibility: None,
                resource_id: None,
                pack_resource_id: None,
                file_id: Some("file".to_string()),
                entry_id: Some("entry".to_string()),
                member_id: None,
                ownership_kind: ContentOwnershipKind::UserAdded,
                materialization_state:
                    crate::state::PackMemberMaterializationState::Present,
                override_kind: crate::state::PackMemberOverrideKind::None,
                expected_relative_path: "mods/a.jar".to_string(),
                required: true,
                project_type: ProjectType::Mod,
                provider: Some(ContentProvider::Modrinth),
                provider_project_id: Some("a".to_string()),
                provider_release_id: Some("old".to_string()),
                content: None,
                capabilities: crate::state::ContentItemCapabilities::default(),
                dependency: Some(crate::state::ContentDependencyInfo {
                    auto_dependency: false,
                    ..Default::default()
                }),
            }],
            pending_manual_downloads: Vec::new(),
            warnings: Vec::new(),
        };
        let before = serde_json::to_value(&snapshot).unwrap();
        let _ = snapshot_upgrade_items(&snapshot);
        assert_eq!(serde_json::to_value(&snapshot).unwrap(), before);
    }

    #[test]
    fn recompute_source_gate_rejects_identity_not_pinned_to_plan() {
        let source_a = vec![InstanceUpgradeSourceFile {
            relative_path: "mods/a.jar".to_string(),
            sha1: "source-a".to_string(),
            size: 1,
            enabled: true,
        }];
        let source_b = vec![InstanceUpgradeSourceFile {
            relative_path: "mods/a.jar".to_string(),
            sha1: "source-b".to_string(),
            size: 1,
            enabled: true,
        }];
        let error =
            ensure_upgrade_source_files_match("instance", &source_a, &source_b)
                .unwrap_err();
        assert!(matches!(
            error.raw.as_ref(),
            crate::ErrorKind::StaleInstanceUpgradePlanSource { instance_id }
                if instance_id == "instance"
        ));
    }

    #[derive(Debug, Eq, PartialEq)]
    struct PlannerStateDigest {
        metadata: String,
        revision: u64,
        instance_files: String,
        content_entries: String,
        provider_refs: String,
        dependency_edges: String,
        disk_files: Vec<(String, Vec<u8>)>,
    }

    #[tokio::test]
    async fn real_planner_sees_untracked_disk_file_without_mutating_instance() {
        let temp = tempfile::tempdir().unwrap();
        let directories = crate::state::DirectoryInfo {
            settings_dir: temp.path().to_path_buf(),
            config_dir: temp.path().to_path_buf(),
            app_identifier: "upgrade-planner-test".to_string(),
        };
        std::fs::create_dir_all(directories.instances_dir()).unwrap();
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(temp.path().join("state.db"))
                    .create_if_missing(true)
                    .foreign_keys(true),
            )
            .await
            .unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();
        let state = crate::state::test_state(directories, pool.clone())
            .await
            .unwrap();
        let instance = crate::state::instances::create_instance(
            crate::state::instances::CreateInstance {
                name: "Planner Dry Run".to_string(),
                path: Some("planner-dry-run".to_string()),
                game_version: "1.21.4".to_string(),
                loader: crate::state::ModLoader::Vanilla,
                loader_version: None,
                icon_path: None,
                link: InstanceLink::Unmanaged,
                symlink_target: None,
                game_dir_override: None,
            },
            &state,
        )
        .await
        .unwrap();
        let instance_dir =
            state.directories.instances_dir().join(&instance.path);
        let mods = instance_dir.join("mods");
        std::fs::create_dir_all(&mods).unwrap();
        std::fs::write(mods.join("new.jar"), b"not-a-real-jar").unwrap();

        let before =
            planner_state_digest(&pool, &instance_dir, &instance.id).await;
        let creation_watch = state
            .file_watcher
            .content_watch_snapshot(&instance.id)
            .await;
        let (plan, source) = create_instance_upgrade_plan_with_source(
            &instance.id,
            InstanceUpgradeEnvironment {
                game_version: "1.21.5".to_string(),
                mod_loader: crate::state::ModLoader::Vanilla,
                mod_loader_version: None,
                shader_runtime: ShaderRuntime::None,
            },
            &state,
        )
        .await
        .unwrap();
        let after =
            planner_state_digest(&pool, &instance_dir, &instance.id).await;

        assert!(plan.items.iter().any(|item| {
            item.relative_path == "mods/new.jar"
                && item.status == InstanceUpgradeItemStatus::Unidentified
        }));
        assert_eq!(after, before);

        let mut validation = UpgradePlanRuntimeValidation::new(
            source,
            &instance.id,
            creation_watch,
            &state,
        )
        .await;
        validation.validate(&plan, &state).await.unwrap();
        validation.validate(&plan, &state).await.unwrap();
        assert_eq!(validation.full_hash_validations, 0);
        assert_eq!(validation.incremental_hashes, 0);

        let source = validation.validate(&plan, &state).await.unwrap();
        let mut custom = plan.clone();
        recompute_instance_upgrade_plan_from_source(
            &mut custom,
            &[],
            InstanceUpgradeSolutionKind::Custom,
            source,
            &state,
        )
        .await
        .unwrap();
        assert_eq!(validation.full_hash_validations, 0);

        std::fs::write(mods.join("new.jar"), b"yes-a-real-jar").unwrap();
        state
            .file_watcher
            .record_upgrade_content_change(&instance.id, "mods/new.jar")
            .await;
        assert!(validation.validate(&plan, &state).await.is_err());
        assert_eq!(validation.incremental_hashes, 1);
        std::fs::write(mods.join("new.jar"), b"not-a-real-jar").unwrap();
        state
            .file_watcher
            .record_upgrade_content_change(&instance.id, "mods/new.jar")
            .await;
        validation.validate(&plan, &state).await.unwrap();
        assert_eq!(validation.incremental_hashes, 2);

        std::fs::write(mods.join("added.jar"), b"added").unwrap();
        assert!(validation.validate(&plan, &state).await.is_err());
        std::fs::remove_file(mods.join("added.jar")).unwrap();
        validation.validate(&plan, &state).await.unwrap();

        std::fs::create_dir_all(instance_dir.join("config")).unwrap();
        std::fs::write(instance_dir.join("config/options.txt"), b"ignored")
            .unwrap();
        state
            .file_watcher
            .record_upgrade_content_change(&instance.id, "config/options.txt")
            .await;
        validation.validate(&plan, &state).await.unwrap();
        assert_eq!(validation.incremental_hashes, 2);

        std::fs::remove_file(mods.join("new.jar")).unwrap();
        assert!(validation.validate(&plan, &state).await.is_err());
        std::fs::write(mods.join("new.jar"), b"not-a-real-jar").unwrap();
        validation.validate(&plan, &state).await.unwrap();

        validation.watcher_epoch = Some(u64::MAX);
        validation.validate(&plan, &state).await.unwrap();
        assert_eq!(validation.full_hash_validations, 1);
        let metadata = super::super::get_instance_metadata(&instance.id, &pool)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(metadata.applied_content_set.revision, plan.source_revision);
    }

    #[tokio::test]
    async fn direct_link_upgrade_source_resolves_external_game_directory() {
        let temp = tempfile::tempdir().unwrap();
        let directories = crate::state::DirectoryInfo {
            settings_dir: temp.path().to_path_buf(),
            config_dir: temp.path().to_path_buf(),
            app_identifier: "direct-upgrade-planner-test".to_string(),
        };
        std::fs::create_dir_all(directories.instances_dir()).unwrap();
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(temp.path().join("state.db"))
                    .create_if_missing(true)
                    .foreign_keys(true),
            )
            .await
            .unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();
        let state = crate::state::test_state(directories, pool).await.unwrap();

        let dot_minecraft = temp.path().join("external-minecraft");
        let version_dir = dot_minecraft.join("versions/direct-upgrade");
        std::fs::create_dir_all(version_dir.join("mods")).unwrap();
        std::fs::write(
            version_dir.join("direct-upgrade.json"),
            br#"{
                "id": "direct-upgrade",
                "inheritsFrom": "1.21.4",
                "mainClass": "net.minecraft.client.main.Main"
            }"#,
        )
        .unwrap();
        std::fs::write(version_dir.join("mods/new.jar"), b"not-a-real-jar")
            .unwrap();

        let instance = crate::state::create_direct_link_instance(
            crate::state::CreateDirectLinkInstance {
                name: Some("Direct Upgrade".to_string()),
                launcher_type:
                    crate::api::pack::import::ImportLauncherType::Generic,
                base_path: dot_minecraft,
                instance_folder: "versions/direct-upgrade".to_string(),
                instance_path: None,
                game_dir_mode: Some(
                    crate::launcher::ExternalGameDirMode::Isolated,
                ),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(
            !state
                .directories
                .instances_dir()
                .join(&instance.path)
                .exists()
        );
        let source_dir =
            upgrade_source_game_dir(&instance, &state.directories).unwrap();
        assert_eq!(
            source_dir.canonicalize().unwrap(),
            version_dir.canonicalize().unwrap()
        );
        let scanned = crate::state::instances::adapters::filesystem::scan_content_files_from(
            &source_dir,
            &instance.path,
        )
        .unwrap();
        assert!(
            scanned
                .iter()
                .any(|file| file.relative_path == "mods/new.jar")
        );

        let creation_watch = state
            .file_watcher
            .content_watch_snapshot(&instance.id)
            .await;
        let source = read_only_upgrade_source(&instance.id, &state)
            .await
            .unwrap();
        let environment = InstanceUpgradeEnvironment {
            game_version: "1.21.4".to_string(),
            mod_loader: crate::state::ModLoader::Vanilla,
            mod_loader_version: None,
            shader_runtime: ShaderRuntime::None,
        };
        let plan = InstanceUpgradePlan {
            id: "direct-upgrade-validation".to_string(),
            instance_id: instance.id.clone(),
            source_revision: source.snapshot.revision,
            source_files: source.source_files.clone(),
            source_environment: environment.clone(),
            target_environment: environment,
            items: Vec::new(),
            dependency_changes: Vec::new(),
            warnings: Vec::new(),
            blocking_issues: Vec::new(),
            newest_solution: None,
            minimal_change_solution: None,
            selected_solution: None,
            custom_constraints: Vec::new(),
        };
        let mut validation = UpgradePlanRuntimeValidation::new(
            source,
            &instance.id,
            creation_watch,
            &state,
        )
        .await;
        validation.validate(&plan, &state).await.unwrap();
        validation.validate(&plan, &state).await.unwrap();
        assert_eq!(validation.full_hash_validations, 0);
        assert_eq!(validation.incremental_hashes, 0);

        std::fs::write(version_dir.join("mods/added.jar"), b"added").unwrap();
        assert!(validation.validate(&plan, &state).await.is_err());
        assert_eq!(validation.full_hash_validations, 0);
    }

    async fn planner_state_digest(
        pool: &sqlx::SqlitePool,
        instance_dir: &Path,
        instance_id: &str,
    ) -> PlannerStateDigest {
        let metadata = super::super::get_instance_metadata(instance_id, pool)
            .await
            .unwrap()
            .unwrap();
        let content_set_id = metadata.applied_content_set.id.clone();
        PlannerStateDigest {
            metadata: serde_json::to_string(&metadata).unwrap(),
            revision: metadata.applied_content_set.revision,
            instance_files: snapshot_table(
                pool,
                "instance_files",
                "instance_id = ?",
                instance_id,
            )
            .await,
            content_entries: snapshot_table(
                pool,
                "instance_content_entries",
                "content_set_id = ?",
                &content_set_id,
            )
            .await,
            provider_refs: snapshot_table(
                pool,
                "instance_content_provider_refs",
                "content_entry_id IN (SELECT id FROM instance_content_entries WHERE content_set_id = ?)",
                &content_set_id,
            )
            .await,
            dependency_edges: snapshot_table(
                pool,
                "instance_content_dependencies",
                "content_set_id = ?",
                &content_set_id,
            )
            .await,
            disk_files: disk_file_snapshot(instance_dir),
        }
    }

    async fn snapshot_table(
        pool: &sqlx::SqlitePool,
        table: &str,
        predicate: &str,
        value: &str,
    ) -> String {
        let columns = sqlx::query_scalar::<_, String>(&format!(
            "SELECT name FROM pragma_table_info('{table}') ORDER BY cid"
        ))
        .fetch_all(pool)
        .await
        .unwrap();
        let json_columns = columns
            .iter()
            .map(|column| format!("\"{}\"", column.replace('"', "\"\"")))
            .collect::<Vec<_>>()
            .join(", ");
        let query = format!(
            "SELECT COALESCE(json_group_array(json_array({json_columns})), '[]') FROM (SELECT * FROM \"{table}\" WHERE {predicate} ORDER BY rowid)"
        );
        sqlx::query_scalar::<_, String>(&query)
            .bind(value)
            .fetch_one(pool)
            .await
            .unwrap()
    }

    fn disk_file_snapshot(root: &Path) -> Vec<(String, Vec<u8>)> {
        fn visit(
            root: &Path,
            current: &Path,
            files: &mut Vec<(String, Vec<u8>)>,
        ) {
            for entry in std::fs::read_dir(current).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    visit(root, &path, files);
                } else {
                    files.push((
                        path.strip_prefix(root)
                            .unwrap()
                            .to_string_lossy()
                            .replace('\\', "/"),
                        std::fs::read(path).unwrap(),
                    ));
                }
            }
        }
        let mut files = Vec::new();
        visit(root, root, &mut files);
        files.sort_by(|left, right| left.0.cmp(&right.0));
        files
    }

    fn test_item(status: InstanceUpgradeItemStatus) -> InstanceUpgradeItem {
        InstanceUpgradeItem {
            content_id: "local".to_string(),
            relative_path: "mods/local.jar".to_string(),
            project_type: ProjectType::Mod,
            provider: None,
            project_id: None,
            current_release_id: None,
            current_enabled: true,
            auto_dependency: false,
            status,
            resolution: InstanceUpgradeResolution {
                content_id: "local".to_string(),
                action: InstanceUpgradeAction::Keep,
                allow_prerelease: false,
                confirmed_prerelease_dependencies: Vec::new(),
            },
            candidate_release_ids: Vec::new(),
        }
    }

    fn snapshot_item_for_test(
        provider: Option<ContentProvider>,
        project_id: Option<&str>,
        release_id: Option<&str>,
    ) -> InstanceContentSnapshotItem {
        InstanceContentSnapshotItem {
            artifact_ref: None,
            compatibility: None,
            resource_id: None,
            pack_resource_id: None,
            file_id: Some("file".to_string()),
            entry_id: Some("entry".to_string()),
            member_id: None,
            ownership_kind: ContentOwnershipKind::UserAdded,
            materialization_state: PackMemberMaterializationState::Present,
            override_kind: PackMemberOverrideKind::None,
            expected_relative_path: "mods/runtime.jar".to_string(),
            required: false,
            project_type: ProjectType::Mod,
            provider,
            provider_project_id: project_id.map(str::to_string),
            provider_release_id: release_id.map(str::to_string),
            content: Some(ContentItem {
                file_name: "runtime.jar".to_string(),
                file_path: "mods/runtime.jar".to_string(),
                id: "hash".to_string(),
                size: 1,
                enabled: true,
                project_type: ProjectType::Mod,
                project: None,
                version: None,
                owner: None,
                update: None,
                date_added: None,
                provider_refs: Vec::new(),
                origin_provider: provider,
                rollback: None,
                environment: None,
                source_kind: Some(ContentSourceKind::Local),
                external: provider.is_none(),
                loader: None,
            }),
            capabilities: ContentItemCapabilities::default(),
            dependency: Some(crate::state::ContentDependencyInfo {
                auto_dependency: false,
                ..Default::default()
            }),
        }
    }

    fn classified_shader_status(
        runtime: ShaderRuntime,
        has_target_game_version_release: bool,
    ) -> InstanceUpgradeItemStatus {
        classified_shader(runtime, has_target_game_version_release)
            .0
            .status
    }

    fn classified_shader(
        runtime: ShaderRuntime,
        has_target_game_version_release: bool,
    ) -> (InstanceUpgradeItem, Vec<RootRequest>) {
        classified_shader_with_action(
            runtime,
            has_target_game_version_release,
            InstanceUpgradeAction::Upgrade,
        )
    }

    fn classified_shader_with_action(
        runtime: ShaderRuntime,
        has_target_game_version_release: bool,
        action: InstanceUpgradeAction,
    ) -> (InstanceUpgradeItem, Vec<RootRequest>) {
        let mut item = test_item(InstanceUpgradeItemStatus::UpgradeAvailable);
        item.content_id = "entry-shader".to_string();
        item.relative_path = "shaderpacks/shader.zip".to_string();
        item.project_type = ProjectType::ShaderPack;
        item.provider = Some(ContentProvider::Modrinth);
        item.project_id = Some("shader".to_string());
        item.current_release_id = Some("shader-old".to_string());
        item.resolution.content_id = item.content_id.clone();
        item.resolution.action = action;
        let mut node = installed("shader", "shader-old", false, true);
        node.content_id = item.content_id.clone();
        node.project_type = ProjectType::ShaderPack;
        let catalog = HashMap::from([(
            key("shader"),
            CandidatePool {
                candidates: Vec::new(),
                exploration_limited: false,
                has_target_game_version_release,
            },
        )]);
        classify_items(
            std::slice::from_mut(&mut item),
            std::slice::from_ref(&node),
            &catalog,
            &test_environment(runtime),
        );
        let roots = roots_from_items(
            std::slice::from_ref(&item),
            std::slice::from_ref(&node),
        );
        (item, roots)
    }

    fn assert_disabled_shader_selection(runtime: ShaderRuntime) {
        let (item, roots) = classified_shader_with_action(
            runtime,
            true,
            InstanceUpgradeAction::Disable,
        );
        assert_eq!(item.resolution.action, InstanceUpgradeAction::Disable);
        let outcome = solve(&roots, &HashMap::new());
        assert!(outcome.issues.is_empty());
        let solution = materialize_solution(
            InstanceUpgradeSolutionKind::Newest,
            &outcome.solutions[0],
            &roots,
            &[],
        );
        assert_eq!(
            solution.selections[0].action,
            InstanceUpgradeAction::Disable
        );
        assert!(!solution.selections[0].enabled);
    }

    fn test_environment(
        shader_runtime: ShaderRuntime,
    ) -> InstanceUpgradeEnvironment {
        InstanceUpgradeEnvironment {
            game_version: "26.1".to_string(),
            mod_loader: crate::state::ModLoader::Fabric,
            mod_loader_version: None,
            shader_runtime,
        }
    }

    fn test_version(loaders: Vec<&str>, game_versions: Vec<&str>) -> Version {
        Version {
            id: "version".to_string(),
            project_id: "project".to_string(),
            author_id: "author".to_string(),
            featured: false,
            name: "Version".to_string(),
            version_number: "1".to_string(),
            changelog: None,
            changelog_url: None,
            date_published: Utc.timestamp_opt(1, 0).single().unwrap(),
            downloads: 0,
            version_type: "release".to_string(),
            files: Vec::new(),
            dependencies: Vec::new(),
            game_versions: game_versions
                .into_iter()
                .map(str::to_string)
                .collect(),
            loaders: loaders.into_iter().map(str::to_string).collect(),
        }
    }
}

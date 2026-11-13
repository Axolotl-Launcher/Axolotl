use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::SystemTime;

use chrono::{DateTime, Utc};
use tokio_util::sync::CancellationToken;

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
    InstanceUpgradeItemProgress, InstanceUpgradeItemStatus,
    InstanceUpgradePlan, InstanceUpgradePlanningPhase,
    InstanceUpgradePlanningProgress, InstanceUpgradeResolution,
    InstanceUpgradeSelection, InstanceUpgradeSolution,
    InstanceUpgradeSolutionKind, InstanceUpgradeSourceFile, ModrinthProjectId,
    ProjectType, ShaderRuntime, Version,
};

const MAX_CANDIDATES_PER_PROJECT: usize = 6;
// TODO(stage 3): Custom UI candidate listing needs an independent backend API.
// This solver bound is not, and must not become, the UI version-list limit.
const MAX_SEARCH_STATES: usize = 10_000;

mod solver;

pub(crate) use self::solver::recompute_instance_upgrade_plan_from_source;
use self::solver::*;

mod source;

use self::source::*;
pub(crate) use self::source::{
    ReadOnlyUpgradeSource, UpgradePlanRuntimeValidation,
    scan_instance_upgrade_source_files, validate_instance_upgrade_plan_source,
};

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
    canceled: bool,
    cancel: Option<CancellationToken>,
    first_issue: Option<InstanceUpgradeIssue>,
}

pub(crate) type UpgradePlanningReporter =
    Arc<dyn Fn(InstanceUpgradePlanningProgress) + Send + Sync>;

/// Lets the creator of an upgrade plan stop it and follow its progress.
/// Cancellation is checked between files, while candidate releases are
/// fetched and inside the dependency solver.
#[derive(Clone, Default)]
pub(crate) struct UpgradePlanningObserver {
    cancel: CancellationToken,
    reporter: Option<UpgradePlanningReporter>,
}

impl UpgradePlanningObserver {
    pub(crate) fn new(
        cancel: CancellationToken,
        reporter: Option<UpgradePlanningReporter>,
    ) -> Self {
        Self { cancel, reporter }
    }

    pub(crate) fn report(&self, progress: InstanceUpgradePlanningProgress) {
        if let Some(reporter) = &self.reporter
            && !self.cancel.is_cancelled()
        {
            reporter(progress);
        }
    }

    pub(crate) fn phase(
        &self,
        phase: InstanceUpgradePlanningPhase,
        completed: usize,
        total: usize,
    ) {
        self.report(InstanceUpgradePlanningProgress::Phase {
            phase,
            completed,
            total,
        });
    }

    pub(crate) fn check(&self) -> crate::Result<()> {
        if self.cancel.is_cancelled() {
            return Err(upgrade_planning_canceled());
        }
        Ok(())
    }

    pub(crate) fn token(&self) -> &CancellationToken {
        &self.cancel
    }

    /// The same cancellation without progress, for work that must not show
    /// up as a planning phase of its own, such as rereading files to validate.
    pub(crate) fn silent(&self) -> Self {
        Self {
            cancel: self.cancel.clone(),
            reporter: None,
        }
    }
}

pub(crate) fn upgrade_planning_canceled() -> crate::Error {
    crate::ErrorKind::OtherError("Upgrade planning was canceled".to_string())
        .into()
}

fn item_progress(
    items: &[InstanceUpgradeItem],
    checked: bool,
) -> InstanceUpgradePlanningProgress {
    InstanceUpgradePlanningProgress::Items {
        items: items
            .iter()
            .map(|item| InstanceUpgradeItemProgress {
                content_id: item.content_id.clone(),
                relative_path: item.relative_path.clone(),
                project_type: item.project_type,
                provider: item.provider,
                project_id: item.project_id.clone(),
                status: (checked
                    || matches!(
                        item.status,
                        InstanceUpgradeItemStatus::Unidentified
                            | InstanceUpgradeItemStatus::UnsupportedContentType
                    ))
                .then_some(item.status),
            })
            .collect(),
    }
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
    observer: &UpgradePlanningObserver,
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

    let mut source =
        read_only_upgrade_source(instance_id, state, observer).await?;
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
    observer.report(item_progress(&items, false));
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
        observer,
    )
    .await?;
    classify_items(&mut items, &installed, &catalog, &target_environment);
    observer.report(item_progress(&items, true));

    let roots = roots_from_items(&items, &installed);
    observer.phase(InstanceUpgradePlanningPhase::Solving, 0, 0);
    let (outcome, catalog, installed, roots) =
        solve_upgrade_off_thread(roots, installed, catalog, &items, observer)
            .await?;
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

#[cfg(test)]
#[path = "instance_upgrade_tests.rs"]
mod tests;

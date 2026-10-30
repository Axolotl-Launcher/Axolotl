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
mod tests {
    use super::*;
    use chrono::TimeZone;

    thread_local! {
        /// Cancels the solver's token once it reaches this search state, so a
        /// test can cancel a search that is already under way.
        static CANCEL_ON_VISIT: std::cell::Cell<Option<usize>> =
            const { std::cell::Cell::new(None) };
    }

    pub(super) fn cancel_on_visit(state: &SearchState) {
        if CANCEL_ON_VISIT.with(std::cell::Cell::get) == Some(state.visited)
            && let Some(cancel) = &state.cancel
        {
            cancel.cancel();
        }
    }
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
    fn canceled_solver_stops_without_a_solution_or_issue() {
        let roots = vec![root("a", "a-old", true)];
        let catalog = catalog([(key("a"), vec![candidate("a", "a-new", 2)])]);
        let cancel = CancellationToken::new();
        cancel.cancel();
        let outcome = solve_upgrade_with_fixed_roots_cancellable(
            &roots,
            &[],
            &catalog,
            &FixedRootConstraints::default(),
            &HashSet::new(),
            Some(&cancel),
        );
        assert!(outcome.canceled);
        assert!(outcome.solutions.is_empty());
        assert!(outcome.issues.is_empty());
        assert!(!solve(&roots, &catalog).canceled);
    }

    #[test]
    fn solver_canceled_during_a_search_stops_before_finding_a_solution() {
        let mut roots = Vec::new();
        let mut catalog = UpgradeCatalog::new();
        for index in 0..20 {
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
        let solve = |cancel: &CancellationToken| {
            solve_upgrade_with_fixed_roots_cancellable(
                &roots,
                &[],
                &catalog,
                &FixedRootConstraints::default(),
                &HashSet::new(),
                Some(cancel),
            )
        };
        let finished = solve(&CancellationToken::new());
        assert!(!finished.canceled);
        assert!(!finished.solutions.is_empty());

        // Twenty roots need at least twenty search states; cancel at the fifth.
        CANCEL_ON_VISIT.with(|visit| visit.set(Some(5)));
        let cancel = CancellationToken::new();
        let outcome = solve(&cancel);
        CANCEL_ON_VISIT.with(|visit| visit.set(None));
        assert!(cancel.is_cancelled());
        assert!(outcome.canceled);
        assert!(outcome.solutions.is_empty());
        assert!(outcome.issues.is_empty());
    }

    #[test]
    fn search_stops_at_the_next_state_once_canceled() {
        let roots = vec![root("a", "a-old", true)];
        let catalog = catalog([(key("a"), vec![candidate("a", "a-new", 2)])]);
        let cancel = CancellationToken::new();
        cancel.cancel();
        let mut state = SearchState {
            cancel: Some(cancel),
            ..SearchState::default()
        };
        let mut solutions = Vec::new();
        search_solutions(
            vec![Requirement {
                key: key("a"),
                version_id: Some("a-new".to_string()),
                explicit_prerelease: false,
                preserve_unsafe: false,
                root_content_id: "a".to_string(),
                root_key: key("a"),
                origins: Vec::new(),
            }],
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            HashSet::new(),
            HashSet::new(),
            &roots,
            &catalog,
            &HashSet::new(),
            &InstalledAliasIndex::new(&[]),
            SolveStrategy::Newest,
            MAX_SEARCH_STATES,
            &mut state,
            &mut solutions,
        );
        assert!(state.canceled);
        assert_eq!(state.visited, 0);
        assert!(solutions.is_empty());
    }

    #[tokio::test]
    async fn off_thread_solve_returns_as_soon_as_planning_is_canceled() {
        let roots = vec![root("a", "a-old", true)];
        let catalog = catalog([(key("a"), vec![candidate("a", "a-new", 2)])]);
        let observer = UpgradePlanningObserver::default();
        let solved = solve_upgrade_off_thread(
            roots.clone(),
            Vec::new(),
            catalog.clone(),
            &[],
            &observer,
        )
        .await
        .unwrap();
        assert_eq!(solved.0.solutions.len(), 1);

        observer.token().cancel();
        let canceled = solve_upgrade_off_thread(
            roots,
            Vec::new(),
            catalog,
            &[],
            &observer,
        )
        .await;
        assert!(canceled.is_err());
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
                resource_update: None,
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
            &UpgradePlanningObserver::default(),
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
    async fn real_planner_reports_progress_and_stops_when_canceled() {
        let temp = tempfile::tempdir().unwrap();
        let directories = crate::state::DirectoryInfo {
            settings_dir: temp.path().to_path_buf(),
            config_dir: temp.path().to_path_buf(),
            app_identifier: "upgrade-planner-progress-test".to_string(),
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
                name: "Planner Progress".to_string(),
                path: Some("planner-progress".to_string()),
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
        let target = InstanceUpgradeEnvironment {
            game_version: "1.21.5".to_string(),
            mod_loader: crate::state::ModLoader::Vanilla,
            mod_loader_version: None,
            shader_runtime: ShaderRuntime::None,
        };

        let events = Arc::new(std::sync::Mutex::new(Vec::new()));
        let reporter = {
            let events = events.clone();
            Arc::new(move |progress| events.lock().unwrap().push(progress))
                as UpgradePlanningReporter
        };
        create_instance_upgrade_plan_with_source(
            &instance.id,
            target.clone(),
            &state,
            &UpgradePlanningObserver::new(
                CancellationToken::new(),
                Some(reporter.clone()),
            ),
        )
        .await
        .unwrap();
        let events = std::mem::take(&mut *events.lock().unwrap());
        assert!(events.contains(&InstanceUpgradePlanningProgress::Phase {
            phase: InstanceUpgradePlanningPhase::ReadingContent,
            completed: 1,
            total: 1,
        }));
        let identified = events
            .iter()
            .find_map(|event| match event {
                InstanceUpgradePlanningProgress::Items { items } => Some(items),
                _ => None,
            })
            .unwrap();
        assert!(identified.iter().any(|item| {
            item.relative_path == "mods/new.jar"
                && item.status == Some(InstanceUpgradeItemStatus::Unidentified)
        }));
        assert!(events.contains(&InstanceUpgradePlanningProgress::Phase {
            phase: InstanceUpgradePlanningPhase::Solving,
            completed: 0,
            total: 0,
        }));

        let creation_watch = state
            .file_watcher
            .content_watch_snapshot(&instance.id)
            .await;
        let (plan, source) = create_instance_upgrade_plan_with_source(
            &instance.id,
            target.clone(),
            &state,
            &UpgradePlanningObserver::default(),
        )
        .await
        .unwrap();
        let mut validation = UpgradePlanRuntimeValidation::new(
            source,
            &instance.id,
            creation_watch,
            &state,
        )
        .await;
        // Forces the full rehash that runs when the watcher cannot vouch for the files.
        validation.watcher_epoch = Some(u64::MAX);
        let stopped = UpgradePlanningObserver::default();
        stopped.token().cancel();
        assert!(
            validation
                .validate_observed(&plan, &state, &stopped)
                .await
                .is_err()
        );
        validation.validate(&plan, &state).await.unwrap();

        let before =
            planner_state_digest(&pool, &instance_dir, &instance.id).await;
        // Cancel once the planner reports that it has started reading files.
        let started = CancellationToken::new();
        let stop_on_start = {
            let started = started.clone();
            Arc::new(move |progress| {
                if matches!(
                    progress,
                    InstanceUpgradePlanningProgress::Phase {
                        phase: InstanceUpgradePlanningPhase::ReadingContent,
                        ..
                    }
                ) {
                    started.cancel();
                }
            }) as UpgradePlanningReporter
        };
        let stopped_while_reading = create_instance_upgrade_plan_with_source(
            &instance.id,
            target.clone(),
            &state,
            &UpgradePlanningObserver::new(started.clone(), Some(stop_on_start)),
        )
        .await;
        assert!(started.is_cancelled());
        assert!(stopped_while_reading.is_err());
        assert_eq!(
            planner_state_digest(&pool, &instance_dir, &instance.id).await,
            before
        );

        let cancel = CancellationToken::new();
        cancel.cancel();
        let canceled = create_instance_upgrade_plan_with_source(
            &instance.id,
            target,
            &state,
            &UpgradePlanningObserver::new(cancel, Some(reporter)),
        )
        .await;
        assert!(canceled.is_err());
        assert_eq!(
            planner_state_digest(&pool, &instance_dir, &instance.id).await,
            before
        );
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
        let source = read_only_upgrade_source(
            &instance.id,
            &state,
            &UpgradePlanningObserver::default(),
        )
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
            resource_update: None,
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

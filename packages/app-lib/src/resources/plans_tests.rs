use super::*;
use crate::resources::plans::OperationDraft;
use std::collections::HashMap;

#[tokio::test]
async fn switching_a_missing_entry_to_its_current_artifact_restores_the_file() {
    let current = release("root0000", "1", 'a');
    let (_temp, service) = service(vec![current.clone()]).await;
    let mut entry = installed(&current, "entry", "mods/current.jar");
    entry.missing = true;
    let built = service
        .build_operation(
            draft(
                vec![selection("root", &current, Some("entry"))],
                ResourceOperationIntent::SwitchVersion,
            ),
            &target(),
            &[entry],
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(built.plan.actions.len(), 1);
    assert_eq!(built.plan.actions[0].kind, ResourceActionKind::Replace);
    service.cache().close().await;
}

#[tokio::test]
async fn source_confirmation_during_a_switch_resolves_the_installed_identity_redirect()
 {
    let mut mr = release("dep00000", "2", 'b');
    mr.reference.release_id = SourceReleaseId::new("depver02").unwrap();
    mr.artifacts[0].reference.release = mr.reference.clone();
    let mut cf = release("123", "2", 'b');
    cf.reference.project.source = SourceId::curseforge();
    cf.reference.release_id = SourceReleaseId::new("101").unwrap();
    cf.artifacts[0].reference.release = cf.reference.clone();
    let mut old = cf.clone();
    old.reference.release_id = SourceReleaseId::new("100").unwrap();
    old.artifacts[0].reference.release = old.reference.clone();
    old.artifacts[0].integrity.hashes[0].value = "a".repeat(40);
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();
    sqlx::query("INSERT INTO resources(id) VALUES ('a'), ('z')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO resource_source_projects(source, project_id, resource_id) VALUES ('modrinth', 'dep00000', 'a'), ('curseforge', '123', 'z')").execute(&pool).await.unwrap();
    let (_temp, cache) = cache().await;
    let service = ResourceService::with_store(
        cache.clone(),
        vec![
            Arc::new(PlanProvider {
                source: SourceId::modrinth(),
                releases: vec![mr],
                requests: Mutex::new(vec![]),
            }),
            Arc::new(PlanProvider {
                source: SourceId::curseforge(),
                releases: vec![old.clone(), cf.clone()],
                requests: Mutex::new(vec![]),
            }),
        ],
        crate::state::resources::ResourceStore::new(pool.clone()),
    )
    .unwrap();
    let mut current = installed(&old, "entry", "mods/current.jar");
    current.resource_id = Some(ResourceId::new("z").unwrap());
    let built = service
        .build_operation(
            draft(
                vec![selection("root", &cf, Some("entry"))],
                ResourceOperationIntent::SwitchVersion,
            ),
            &target(),
            &[current],
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(built.plan.issues.is_empty(), "{:?}", built.plan.issues);
    assert_eq!(
        service
            .store()
            .unwrap()
            .resolve(&ResourceId::new("z").unwrap())
            .await
            .unwrap()
            .unwrap()
            .as_str(),
        "a"
    );
    assert_eq!(built.plan.actions.len(), 1);
    cache.close().await;
    pool.close().await;
}

#[path = "plans_graph_tests.rs"]
mod graph_tests;

#[tokio::test]
async fn dependency_analysis_failure_does_not_block_installing_valid_roots() {
    let mut root = release("root0000", "1", 'a');
    let absent = release("absent00", "1", 'b');
    requires(&mut root, &absent, ResourceDependencyKind::Required);
    let (_temp, service) = service(vec![root.clone()]).await;
    for intent in [
        ResourceOperationIntent::Install,
        ResourceOperationIntent::Update,
    ] {
        let built = service
            .build_operation(
                draft(vec![selection("root", &root, None)], intent),
                &target(),
                &[],
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(built.plan.actions.len(), 1);
        assert_eq!(
            built.plan.actions[0].target.reference,
            root.artifacts[0].reference
        );
        assert_eq!(
            built.plan.issues.iter().any(|issue| issue.blocking),
            intent != ResourceOperationIntent::Install
        );
        if intent == ResourceOperationIntent::Install {
            assert!(built.plan.issues.iter().any(|issue| issue.code
                == ResourceIssueCode::DependencyAnalysisFailed));
        }
    }
    service.cache().close().await;
}

#[tokio::test]
async fn dependency_analysis_failure_keeps_root_validation_and_other_resolved_dependencies()
 {
    let mut root = release("root0000", "1", 'a');
    let valid = release("valid000", "1", 'b');
    let absent = release("absent00", "1", 'c');
    requires(&mut root, &valid, ResourceDependencyKind::Required);
    requires(&mut root, &absent, ResourceDependencyKind::Required);
    let (_temp, service) = service(vec![root.clone(), valid.clone()]).await;
    let built = service
        .build_operation(
            draft(
                vec![selection("root", &root, None)],
                ResourceOperationIntent::Install,
            ),
            &target(),
            &[],
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(built.plan.actions.len(), 2);
    assert!(!built.plan.issues.iter().any(|issue| issue.blocking));
    let selected_only = service
        .revise_operation(
            built,
            vec![ResourcePlanDecision::IgnoreDependencies],
            &target(),
            &[],
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(selected_only.plan.actions.len(), 1);
    assert_eq!(selected_only.plan.nodes.len(), 1);
    assert!(selected_only.plan.edges.is_empty());
    assert!(!selected_only.plan.issues.iter().any(|issue| issue.blocking));
    assert_eq!(
        selected_only.plan.actions[0].target.reference,
        root.artifacts[0].reference
    );
    let mut request = draft(
        vec![selection("root", &absent, None)],
        ResourceOperationIntent::Install,
    );
    request
        .plan
        .selections
        .push(selection("valid", &valid, None));
    request
        .plan
        .active_roots
        .push(SelectionId::new("valid").unwrap());
    let failed = service
        .build_operation(request, &target(), &[], &CancellationToken::new())
        .await
        .unwrap();
    assert!(failed.plan.issues.iter().any(|issue| issue.blocking));
    let forced = service
        .revise_operation(
            failed,
            vec![ResourcePlanDecision::IgnoreDependencies],
            &target(),
            &[],
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(forced.plan.issues.iter().any(|issue| issue.blocking
        && issue.code == ResourceIssueCode::SourceUnavailable));
    service.cache().close().await;
}

#[tokio::test]
async fn dependency_source_lookup_failure_warns_but_keeps_the_root_installable()
{
    let mut root = release("root0000", "1", 'a');
    root.artifacts[0].dependencies.push(ArtifactDependency {
        target: DependencyTarget::SourceRelease {
            source: SourceId::modrinth(),
            release_id: SourceReleaseId::new("unavailable-version").unwrap(),
        },
        kind: ResourceDependencyKind::Required,
        version_constraint: None,
    });
    let (_temp, service) = service(vec![root.clone()]).await;
    let built = service
        .build_operation(
            draft(
                vec![selection("root", &root, None)],
                ResourceOperationIntent::Install,
            ),
            &target(),
            &[],
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(built.plan.actions.len(), 1);
    assert!(!built.plan.issues.iter().any(|issue| issue.blocking));
    assert!(
        built
            .plan
            .issues
            .iter()
            .any(|issue| issue.code
                == ResourceIssueCode::DependencyAnalysisFailed)
    );
    service.cache().close().await;
}

#[tokio::test]
async fn retained_dependency_lookup_failure_does_not_block_a_new_install() {
    let root = release("root0000", "1", 'a');
    let offline = release("offline0", "1", 'b');
    let (_temp, service) = service(vec![root.clone()]).await;
    let built = service
        .build_operation(
            draft(
                vec![selection("root", &root, None)],
                ResourceOperationIntent::Install,
            ),
            &target(),
            &[installed(&offline, "offline", "mods/offline.jar")],
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(built.plan.actions.len(), 1);
    assert!(!built.plan.issues.iter().any(|issue| issue.blocking));
    assert!(
        built
            .plan
            .issues
            .iter()
            .any(|issue| issue.code
                == ResourceIssueCode::DependencyAnalysisFailed)
    );
    let forced = service
        .revise_operation(
            built,
            vec![ResourcePlanDecision::IgnoreDependencies],
            &target(),
            &[installed(&offline, "offline", "mods/offline.jar")],
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(forced.plan.issues.is_empty(), "{:?}", forced.plan.issues);
    assert!(forced.retained_requirements.is_empty());
    service.cache().close().await;
}

#[tokio::test]
async fn entry_changes_can_skip_dependency_resolution_without_skipping_artifact_validation()
 {
    let old = release("root0000", "1", 'a');
    let mut new = release("root0000", "2", 'b');
    let absent = release("missing0", "1", 'c');
    requires(&mut new, &absent, ResourceDependencyKind::Required);
    let (_directory, service) = service(vec![old.clone(), new.clone()]).await;
    let mut request = draft(
        vec![selection("root", &new, Some("entry"))],
        ResourceOperationIntent::SwitchVersion,
    );
    request.resolve_dependencies = false;
    let built = service
        .build_operation(
            request.clone(),
            &target(),
            &[installed(&old, "entry", "mods/custom.jar.disabled")],
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(built.plan.issues.is_empty(), "{:?}", built.plan.issues);
    assert_eq!(built.plan.actions.len(), 1);
    assert!(built.plan.edges.is_empty());
    assert!(built.candidates.is_empty());
    assert_eq!(built.plan.nodes.len(), 1);
    assert_eq!(
        built.plan.actions[0].target_relative_path,
        "mods/custom.jar.disabled"
    );
    request.plan.selections[0].release = ReleaseSelection::Artifact {
        reference: SourceArtifactRef {
            artifact_id: SourceArtifactId::new("foreign.jar").unwrap(),
            ..new.artifacts[0].reference.clone()
        },
    };
    let invalid = service
        .build_operation(
            request,
            &target(),
            &[installed(&old, "entry", "mods/custom.jar.disabled")],
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(invalid.plan.actions.is_empty());
    assert!(invalid.plan.issues.iter().any(|issue| issue.blocking));
    service.cache().close().await;
}

#[tokio::test]
async fn operation_progress_reports_each_node_before_resolving_its_children() {
    let mut root = release("root0000", "1", 'a');
    let required = release("required", "1", 'b');
    let optional = release("optional", "1", 'c');
    requires(&mut root, &required, ResourceDependencyKind::Required);
    requires(&mut root, &optional, ResourceDependencyKind::Optional);
    let (_directory, cache) = cache().await;
    let provider = Arc::new(PlanProvider {
        source: SourceId::modrinth(),
        releases: vec![root.clone(), required.clone(), optional],
        requests: Mutex::new(vec![]),
    });
    let service = ResourceService::new(cache, vec![provider.clone()]).unwrap();
    let events = Mutex::new(vec![]);
    let progress =
        |event: crate::resources::plans::ResourceOperationProgress| {
            if event.node.id.as_str() == "root" {
                assert!(!provider.requests.lock().unwrap().iter().any(|query| {
                matches!(query, ProviderQuery::Release { release_id, .. }
                    if release_id == &required.reference.release_id)
            }));
            }
            events.lock().unwrap().push(event);
        };
    let plan = service
        .build_operation_with_progress(
            draft(
                vec![selection("root", &root, None)],
                ResourceOperationIntent::Install,
            ),
            &target(),
            &[],
            &CancellationToken::new(),
            Some(&progress),
        )
        .await
        .unwrap()
        .plan;
    let events = events.lock().unwrap();
    assert_eq!(events.len(), 3);
    assert_eq!(events[0].node.id.as_str(), "root");
    assert!(events[0].metadata.is_some());
    assert!(events.iter().all(
        |event| event.plan_id == plan.id && event.revision == plan.revision
    ));
    assert_eq!(events[2].node.state, DependencyNodeState::OptionalCandidate);
    assert_eq!(plan.nodes.len(), events.len());
    assert_eq!(plan.actions.len(), 2);
    drop(events);
    service.cache().close().await;
}

#[tokio::test]
async fn canceling_after_a_progress_node_does_not_publish_a_complete_plan() {
    let mut root = release("root0000", "1", 'a');
    let child = release("required", "1", 'b');
    requires(&mut root, &child, ResourceDependencyKind::Required);
    let (_directory, service) = service(vec![root.clone(), child]).await;
    let cancel = CancellationToken::new();
    let events = Mutex::new(vec![]);
    let progress = |event| {
        events.lock().unwrap().push(event);
        cancel.cancel();
    };
    let result = service
        .build_operation_with_progress(
            draft(
                vec![selection("root", &root, None)],
                ResourceOperationIntent::Install,
            ),
            &target(),
            &[],
            &cancel,
            Some(&progress),
        )
        .await;
    assert!(matches!(result, Err(ResourceError::Canceled)));
    assert_eq!(events.lock().unwrap().len(), 1);
    service.cache().close().await;
}

#[tokio::test]
async fn automatic_catalog_without_compatible_candidates_is_a_normal_no_update_result()
 {
    let mut old = release("root0000", "1", 'a');
    old.compatibility.game_versions = vec!["1.12.2".into()];
    let (_directory, service) = service(vec![old.clone()]).await;
    let mut automatic = selection("root", &old, Some("entry"));
    automatic.release = ReleaseSelection::Automatic;
    let result = service
        .selected_release_for_current(
            &automatic,
            &target(),
            &SourceId::modrinth(),
            &CancellationToken::new(),
            Some(&old),
        )
        .await;
    assert!(matches!(result, Err(ResourceError::NoCompatibleRelease)));
    service.cache().close().await;
}

#[tokio::test]
async fn complete_updates_keep_more_than_one_hundred_entry_actions() {
    let old = release("root0000", "1", 'a');
    let new = release("root0000", "2", 'b');
    let (_directory, service) = service(vec![old.clone(), new.clone()]).await;
    let entries: Vec<_> = (0..101)
        .map(|index| {
            installed(
                &old,
                &format!("copy-{index}"),
                &format!("mods/copy-{index}.jar"),
            )
        })
        .collect();
    let selections = entries
        .iter()
        .map(|entry| {
            selection(
                entry.entry_id.as_str(),
                &new,
                Some(entry.entry_id.as_str()),
            )
        })
        .collect();
    let plan = service
        .build_operation(
            draft(selections, ResourceOperationIntent::Update),
            &target(),
            &entries,
            &CancellationToken::new(),
        )
        .await
        .unwrap()
        .plan;
    assert!(plan.issues.is_empty(), "{:?}", plan.issues);
    assert_eq!(plan.actions.len(), 101);
    assert_eq!(
        plan.actions
            .iter()
            .map(|action| &action.entry_id)
            .collect::<std::collections::HashSet<_>>()
            .len(),
        101
    );
    service.cache().close().await;
}

#[tokio::test]
async fn the_same_datapack_in_two_worlds_produces_two_scoped_actions() {
    let mut old = release("pack0000", "1", 'a');
    let mut new = release("pack0000", "2", 'b');
    for release in [&mut old, &mut new] {
        release.artifacts[0].file_name =
            Some(format!("{}.zip", release.version));
    }
    let (_directory, service) = service(vec![old.clone(), new.clone()]).await;
    let mut roots = Vec::new();
    let mut entries = Vec::new();
    for world in ["one", "two"] {
        let scope = InstallScope::World {
            relative_path: format!("saves/{world}"),
        };
        let mut root = selection(world, &new, Some(world));
        root.scope = scope.clone();
        root.resource_type = ResourceType::DataPack;
        let mut entry = installed(
            &old,
            world,
            &format!("saves/{world}/datapacks/custom.zip"),
        );
        entry.scope = scope;
        entry.resource_type = ResourceType::DataPack;
        roots.push(root);
        entries.push(entry);
    }
    let plan = service
        .build_operation(
            draft(roots, ResourceOperationIntent::Update),
            &target(),
            &entries,
            &CancellationToken::new(),
        )
        .await
        .unwrap()
        .plan;
    assert!(plan.issues.is_empty(), "{:?}", plan.issues);
    assert_eq!(plan.actions.len(), 2);
    assert_eq!(
        plan.actions[0].target_relative_path,
        "saves/one/datapacks/custom.zip"
    );
    assert_eq!(
        plan.actions[1].target_relative_path,
        "saves/two/datapacks/custom.zip"
    );
    service.cache().close().await;
}

#[tokio::test]
async fn confirmed_cross_source_dependency_preserves_both_parent_requirements()
{
    cross_source_dependency_plan(true, false, None).await;
}

#[tokio::test]
async fn updates_preview_renamed_paths_and_report_collisions_when_name_preservation_is_disabled()
 {
    let old = release("root0000", "1", 'a');
    let new = release("root0000", "2", 'b');
    let (_directory, service) = service(vec![old.clone(), new.clone()]).await;
    let entry = installed(
        &old,
        "entry",
        "mods/[用途] root0000-1.jar-copy.jar.disabled",
    );
    let plan = service
        .build_operation(
            draft(
                vec![selection("root", &new, Some("entry"))],
                ResourceOperationIntent::Update,
            ),
            &target(),
            &[entry.clone()],
            &CancellationToken::new(),
        )
        .await
        .unwrap()
        .plan;
    assert_eq!(
        plan.actions[0].target_relative_path,
        "mods/[用途] root0000-2.jar-copy.jar.disabled"
    );
    let mut disabled = draft(
        vec![selection("root", &new, Some("entry"))],
        ResourceOperationIntent::SwitchVersion,
    );
    disabled.naming.preserve = false;
    let plan = service
        .build_operation(
            disabled.clone(),
            &target(),
            &[entry.clone()],
            &CancellationToken::new(),
        )
        .await
        .unwrap()
        .plan;
    assert_eq!(
        plan.actions[0].target_relative_path,
        "mods/root0000-2.jar.disabled"
    );
    let other = installed(&old, "other", "mods/root0000-2.jar.disabled");
    let plan = service
        .build_operation(
            disabled,
            &target(),
            &[entry, other],
            &CancellationToken::new(),
        )
        .await
        .unwrap()
        .plan;
    assert!(
        plan.issues
            .iter()
            .any(|issue| issue.code == ResourceIssueCode::InvalidPath
                && issue.blocking)
    );
    service.cache().close().await;
}

#[tokio::test]
async fn newly_verified_cross_source_dependency_merges_nodes_and_retains_both_parents()
 {
    cross_source_dependency_plan(false, false, None).await;
}

#[tokio::test]
async fn confirmed_cross_source_dependency_with_different_required_bytes_is_a_conflict()
 {
    cross_source_dependency_plan(true, true, None).await;
}

#[tokio::test]
async fn curseforge_requirement_reuses_the_confirmed_modrinth_installed_file_and_rejects_disabled_copy()
 {
    cross_source_dependency_plan(true, false, Some(true)).await;
    cross_source_dependency_plan(true, false, Some(false)).await;
}

async fn cross_source_dependency_plan(
    preconfirmed: bool,
    conflict: bool,
    installed_enabled: Option<bool>,
) {
    let mut a = release("root0000", "1", 'a');
    let mut mr_dep = release("dep00000", "1", 'c');
    let mut cf_dep = release("123", "1", 'c');
    let mut cf_root = release("456", "1", 'd');
    for release in [&mut cf_dep, &mut cf_root] {
        release.reference.project.source = SourceId::curseforge();
        release.reference.release_id = SourceReleaseId::new(
            if release.reference.project.project_id.as_str() == "123" {
                "101"
            } else {
                "102"
            },
        )
        .unwrap();
        release.artifacts[0].reference.release = release.reference.clone();
    }
    mr_dep.reference.release_id = SourceReleaseId::new("depver01").unwrap();
    mr_dep.artifacts[0].reference.release = mr_dep.reference.clone();
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();
    let store = crate::state::resources::ResourceStore::new(pool.clone());
    let ProviderValue::Project(left) =
        project_value(SourceId::modrinth(), "dep00000", "Dependency")
    else {
        panic!()
    };
    let ProviderValue::Project(right) =
        project_value(SourceId::curseforge(), "123", "Dependency")
    else {
        panic!()
    };
    if preconfirmed {
        store
            .confirm_projects(
                &left.metadata,
                &right.metadata,
                &mr_dep.artifacts[0],
                &cf_dep.artifacts[0],
            )
            .await
            .unwrap();
    }
    if conflict {
        cf_dep.artifacts[0].integrity.hashes[0].value = "e".repeat(40);
    }
    requires(&mut a, &mr_dep, ResourceDependencyKind::Required);
    requires(&mut cf_root, &cf_dep, ResourceDependencyKind::Required);
    let mut entries = vec![];
    if let Some(enabled) = installed_enabled {
        let mut entry =
            installed(&mr_dep, "already-installed", "mods/dependency.jar");
        entry.resource_id =
            store.lookup(&mr_dep.reference.project).await.unwrap();
        entry.enabled = enabled;
        entries.push(entry);
    }
    let (directory, cache) = cache().await;
    let service = ResourceService::with_store(
        cache.clone(),
        vec![
            Arc::new(PlanProvider {
                source: SourceId::modrinth(),
                releases: vec![a.clone(), mr_dep],
                requests: Mutex::new(vec![]),
            }),
            Arc::new(PlanProvider {
                source: SourceId::curseforge(),
                releases: vec![cf_root.clone(), cf_dep],
                requests: Mutex::new(vec![]),
            }),
        ],
        store,
    )
    .unwrap();
    let plan = service
        .build_operation(
            draft(
                vec![
                    selection("mr", &a, None),
                    selection("cf", &cf_root, None),
                ],
                ResourceOperationIntent::Install,
            ),
            &target(),
            &entries,
            &CancellationToken::new(),
        )
        .await
        .unwrap()
        .plan;
    if conflict || installed_enabled == Some(false) {
        assert!(
            plan.issues
                .iter()
                .any(|issue| issue.code == ResourceIssueCode::VersionConflict
                    && issue.blocking)
        );
    } else {
        assert!(plan.issues.is_empty(), "{:?}", plan.issues);
    }
    assert_eq!(
        plan.actions.len(),
        if installed_enabled.is_some() { 2 } else { 3 }
    );
    assert_eq!(plan.edges.len(), 2);
    assert_eq!(plan.edges[0].child, plan.edges[1].child);
    cache.close().await;
    pool.close().await;
    drop(directory);
}

struct PlanProvider {
    source: SourceId,
    releases: Vec<ResourceRelease>,
    requests: Mutex<Vec<ProviderQuery>>,
}

#[tokio::test]
async fn incompatible_roots_are_blocked_in_both_selection_orders() {
    let mut a = release("root0000", "1", 'a');
    let b = release("root1111", "1", 'b');
    requires(&mut a, &b, ResourceDependencyKind::Incompatible);
    let (_directory, service) = service(vec![a.clone(), b.clone()]).await;
    for roots in [
        vec![selection("a", &a, None), selection("b", &b, None)],
        vec![selection("b", &b, None), selection("a", &a, None)],
    ] {
        let plan = service
            .build_operation(
                draft(roots, ResourceOperationIntent::Install),
                &target(),
                &[],
                &CancellationToken::new(),
            )
            .await
            .unwrap()
            .plan;
        assert!(
            plan.issues
                .iter()
                .any(|issue| issue.code == ResourceIssueCode::VersionConflict
                    && issue.blocking)
        );
        let edge = plan
            .edges
            .iter()
            .find(|edge| edge.kind == ResourceDependencyKind::Incompatible)
            .unwrap();
        assert_eq!(edge.child.as_str(), "b");
    }
    let b2 = release("root1111", "2", 'c');
    let (_directory2, other) = self::service(vec![a.clone(), b2.clone()]).await;
    let plan = other
        .build_operation(
            draft(
                vec![selection("a", &a, None), selection("b", &b2, None)],
                ResourceOperationIntent::Install,
            ),
            &target(),
            &[],
            &CancellationToken::new(),
        )
        .await
        .unwrap()
        .plan;
    assert!(plan.issues.is_empty(), "{:?}", plan.issues);
    let plan = service
        .build_operation(
            draft(
                vec![selection("a", &a, None)],
                ResourceOperationIntent::Install,
            ),
            &target(),
            &[installed(&b, "existing", "mods/existing.jar")],
            &CancellationToken::new(),
        )
        .await
        .unwrap()
        .plan;
    assert!(
        plan.issues
            .iter()
            .any(|issue| issue.code == ResourceIssueCode::VersionConflict
                && issue.blocking)
    );
    let plan = service
        .build_operation(
            draft(
                vec![selection("a", &a, None)],
                ResourceOperationIntent::Install,
            ),
            &target(),
            &[installed(&b, "existing", "mods/existing.jar.disabled")],
            &CancellationToken::new(),
        )
        .await
        .unwrap()
        .plan;
    assert!(plan.issues.is_empty(), "{:?}", plan.issues);
    service.cache().close().await;
    other.cache().close().await;
}

#[tokio::test]
async fn automatic_non_mod_releases_do_not_filter_by_the_instances_mod_loader()
{
    for (kind, loader) in [
        (ResourceType::ResourcePack, "minecraft"),
        (ResourceType::Shader, "iris"),
        (ResourceType::DataPack, "datapack"),
    ] {
        let mut release = release("root0000", "1", 'a');
        release.compatibility.loaders = vec![loader.into()];
        release.artifacts[0].file_name = Some("file.zip".into());
        let (_directory, service) = service(vec![release.clone()]).await;
        let mut root = selection("root", &release, None);
        root.release = ReleaseSelection::Automatic;
        root.resource_type = kind;
        if kind == ResourceType::DataPack {
            root.scope = InstallScope::World {
                relative_path: "saves/world".into(),
            };
        }
        let plan = service
            .build_operation(
                draft(vec![root], ResourceOperationIntent::Install),
                &target(),
                &[],
                &CancellationToken::new(),
            )
            .await
            .unwrap()
            .plan;
        assert!(plan.issues.is_empty(), "{:?}", plan.issues);
        assert_eq!(plan.actions.len(), 1);
        service.cache().close().await;
    }
}

#[tokio::test]
async fn disabled_dependency_completion_requires_explicit_roots_and_keeps_installed_dependencies()
 {
    let mut a = release("root0000", "1", 'a');
    let b = release("dep00000", "1", 'b');
    requires(&mut a, &b, ResourceDependencyKind::Required);
    let (_directory, service) = service(vec![a.clone(), b.clone()]).await;
    let mut request = draft(
        vec![selection("a", &a, None)],
        ResourceOperationIntent::Install,
    );
    request.install_dependencies = false;
    let missing = service
        .build_operation(
            request.clone(),
            &target(),
            &[],
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(missing.plan.actions.len(), 1);
    assert_eq!(missing.candidates.len(), 1);
    assert!(
        missing
            .plan
            .nodes
            .iter()
            .any(|node| node.state == DependencyNodeState::Missing)
    );
    assert!(missing.plan.issues.iter().any(|issue| issue.blocking
        && issue.code == ResourceIssueCode::MissingDependency));
    let mut explicit = missing;
    let candidate = explicit.candidates[0].clone();
    explicit.plan.active_roots.push(candidate.id.clone());
    explicit.plan.selections.push(candidate);
    let explicit = service
        .build_operation(explicit, &target(), &[], &CancellationToken::new())
        .await
        .unwrap();
    assert!(
        explicit.plan.issues.is_empty(),
        "{:?}",
        explicit.plan.issues
    );
    assert_eq!(explicit.plan.actions.len(), 2);
    let satisfied = service
        .build_operation(
            request,
            &target(),
            &[installed(&b, "installed", "mods/dep.jar")],
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(
        satisfied.plan.issues.is_empty(),
        "{:?}",
        satisfied.plan.issues
    );
    assert_eq!(satisfied.plan.actions.len(), 1);
    service.cache().close().await;
}
#[async_trait]
impl ResourceProvider for PlanProvider {
    fn source(&self) -> ResourceSource {
        ResourceSource {
            id: self.source.clone(),
            types: vec![
                ResourceType::Mod,
                ResourceType::DataPack,
                ResourceType::ResourcePack,
                ResourceType::Shader,
            ],
            capabilities: vec![
                SourceCapability::Project,
                SourceCapability::Releases,
                SourceCapability::IdentifyHash,
            ],
            available: true,
            supported_search_sorts: vec![],
        }
    }
    fn namespace(&self) -> String {
        "plan-fixture".into()
    }
    fn batch_limit(&self, _: &ProviderQuery) -> usize {
        100
    }
    async fn fetch_many(
        &self,
        queries: &[ProviderQuery],
    ) -> Vec<ResourceResult<ProviderValue>> {
        self.requests.lock().unwrap().extend_from_slice(queries);
        queries
            .iter()
            .map(|query| match query {
                ProviderQuery::Project { id, .. } => {
                    let mut value = project_value(
                        self.source.clone(),
                        id.as_str(),
                        id.as_str(),
                    );
                    if let ProviderValue::Project(document) = &mut value {
                        document.metadata.types.push(ResourceType::DataPack);
                        document.metadata.types.extend([
                            ResourceType::ResourcePack,
                            ResourceType::Shader,
                        ]);
                    }
                    Ok(value)
                }
                ProviderQuery::Releases {
                    project_id, target, ..
                } => Ok(ProviderValue::Releases(SourcePage {
                    entries: self
                        .releases
                        .iter()
                        .filter(|release| {
                            &release.reference.project.project_id == project_id
                                && (target.loaders.is_empty()
                                    || release
                                        .compatibility
                                        .loaders
                                        .iter()
                                        .any(|loader| {
                                            target.loaders.contains(loader)
                                        }))
                        })
                        .cloned()
                        .collect(),
                    cursor: None,
                    total: ipc::ResourceTotal {
                        value: None,
                        precision: ipc::TotalPrecision::Unknown,
                    },
                    window_exhausted: false,
                })),
                ProviderQuery::Release { release_id, .. } => self
                    .releases
                    .iter()
                    .find(|release| &release.reference.release_id == release_id)
                    .cloned()
                    .map(|release| {
                        ProviderValue::Release(ReleaseDocument {
                            release,
                            changelog: None,
                            protocol: None,
                        })
                    })
                    .ok_or(ResourceError::Unavailable),
                ProviderQuery::Identify { integrity, .. } => {
                    Ok(ProviderValue::Matches(
                        self.releases
                            .iter()
                            .flat_map(|release| &release.artifacts)
                            .filter(|artifact| {
                                identity::exact_integrity(
                                    integrity,
                                    &artifact.integrity,
                                )
                            })
                            .cloned()
                            .collect(),
                    ))
                }
                _ => Err(ResourceError::Unsupported("fixture query".into())),
            })
            .collect()
    }
}
fn release(project: &str, version: &str, byte: char) -> ResourceRelease {
    let reference = SourceReleaseRef {
        project: SourceProjectRef {
            source: SourceId::modrinth(),
            project_id: SourceProjectId::new(project).unwrap(),
        },
        release_id: SourceReleaseId::new(format!("{project}-{version}"))
            .unwrap(),
    };
    ResourceRelease {
        reference: reference.clone(),
        version: version.into(),
        channel: ReleaseChannel::Release,
        published_at: Some(
            chrono::DateTime::from_timestamp(
                version.parse::<i64>().unwrap() * 1000,
                0,
            )
            .unwrap(),
        ),
        compatibility: ReleaseCompatibility {
            game_versions: vec!["1.20.1".into()],
            loaders: vec!["fabric".into()],
        },
        artifacts: vec![ResourceArtifact {
            reference: SourceArtifactRef {
                release: reference,
                artifact_id: SourceArtifactId::new(format!(
                    "{project}-{version}.jar"
                ))
                .unwrap(),
            },
            file_name: Some(format!("{project}-{version}.jar")),
            role: ArtifactRole::Primary,
            available: Some(true),
            integrity: ArtifactIntegrity {
                size: Some(100),
                hashes: vec![ContentHash {
                    algorithm: HashAlgorithm::Sha1,
                    value: byte.to_string().repeat(40),
                }],
            },
            acquisition: ArtifactAcquisition::PublicDownload {
                urls: vec![format!(
                    "https://example.com/{project}-{version}.jar"
                )],
            },
            dependencies: vec![],
        }],
        equivalent_releases: vec![],
    }
}
fn requires(
    parent: &mut ResourceRelease,
    child: &ResourceRelease,
    kind: ResourceDependencyKind,
) {
    parent.artifacts[0].dependencies.push(ArtifactDependency {
        target: DependencyTarget::Release {
            reference: child.reference.clone(),
        },
        kind,
        version_constraint: None,
    });
}
fn selection(
    id: &str,
    release: &ResourceRelease,
    entry: Option<&str>,
) -> ResourceSelection {
    ResourceSelection {
        id: SelectionId::new(id).unwrap(),
        resource: ResourceRef::Source {
            project: release.reference.project.clone(),
        },
        release: ReleaseSelection::Artifact {
            reference: release.artifacts[0].reference.clone(),
        },
        resource_type: ResourceType::Mod,
        scope: InstallScope::Instance,
        entry_id: entry.map(|id| InstalledEntryId::new(id).unwrap()),
    }
}
fn installed(
    release: &ResourceRelease,
    id: &str,
    path: &str,
) -> InstalledResource {
    InstalledResource {
        entry_id: InstalledEntryId::new(id).unwrap(),
        instance_id: "instance".into(),
        resource_id: None,
        artifact: Some(release.artifacts[0].reference.clone()),
        known_artifacts: vec![release.artifacts[0].reference.clone()],
        integrity: release.artifacts[0].integrity.clone(),
        relative_path: path.into(),
        original_file_name: release.artifacts[0].file_name.clone(),
        resource_type: ResourceType::Mod,
        scope: InstallScope::Instance,
        enabled: !path.ends_with(".disabled"),
        missing: false,
        ownership: ResourceOwnership::UserAdded,
    }
}
fn draft(
    selections: Vec<ResourceSelection>,
    intent: ResourceOperationIntent,
) -> OperationDraft {
    OperationDraft {
        local_metadata: Default::default(),
        retained_requirements: Vec::new(),
        resolve_dependencies: true,
        skip_unwritable_files: false,
        import_files: Default::default(),
        naming: Default::default(),
        plan: ResourceOperationPlan {
            schema_version: ResourceContractVersion,
            id: PlanId::new("plan").unwrap(),
            revision: 1,
            instance_id: "instance".into(),
            instance_revision: u32::MAX as u64 + 1,
            intent,
            active_roots: selections
                .iter()
                .map(|selection| selection.id.clone())
                .collect(),
            selections,
            nodes: vec![],
            edges: vec![],
            actions: vec![],
            issues: vec![],
            exceptions: vec![],
            selection_reasons: vec![],
        },
        candidates: vec![],
        install_dependencies: true,
        local_files: HashMap::new(),
        restore_history: None,
    }
}
fn target() -> ipc::ResourceTarget {
    ipc::ResourceTarget {
        game_versions: vec!["1.20.1".into()],
        loaders: vec!["fabric".into()],
        channels: vec![ReleaseChannel::Release],
    }
}

#[tokio::test]
async fn connector_exact_releases_allow_hash_bound_unknown_sizes_but_require_compatibility_review()
 {
    let mut release = release("connector", "1", 'a');
    let source = SourceId::new("planet_minecraft").unwrap();
    release.reference.project.source = source.clone();
    release.artifacts[0].reference.release = release.reference.clone();
    release.artifacts[0].integrity.size = None;
    release.compatibility.game_versions.clear();
    release.compatibility.loaders.clear();
    let (_directory, cache) = cache().await;
    let resources = ResourceService::new(
        cache,
        vec![Arc::new(PlanProvider {
            source,
            releases: vec![release.clone()],
            requests: Mutex::new(vec![]),
        })],
    )
    .unwrap();
    let mut selected = selection("root", &release, None);
    selected.resource = ResourceRef::Source {
        project: release.reference.project.clone(),
    };
    selected.release = ReleaseSelection::Artifact {
        reference: release.artifacts[0].reference.clone(),
    };
    let draft = resources
        .build_operation(
            draft(vec![selected], ResourceOperationIntent::Install),
            &target(),
            &[],
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(draft.plan.actions.len(), 1);
    assert!(draft.plan.issues.iter().any(|issue| issue.blocking
        && issue.code == ResourceIssueCode::CompatibilityUnknown));
    assert!(draft.plan.actions[0].target.integrity.size.is_none());
}
async fn service(
    releases: Vec<ResourceRelease>,
) -> (tempfile::TempDir, Arc<ResourceService>) {
    let (directory, cache) = cache().await;
    let provider = Arc::new(PlanProvider {
        source: SourceId::modrinth(),
        releases,
        requests: Mutex::new(vec![]),
    });
    (
        directory,
        ResourceService::new(cache, vec![provider]).unwrap(),
    )
}

#[tokio::test]
async fn duplicate_updates_preserve_each_entry_and_merge_shared_dependency() {
    let old = release("root0000", "1", 'a');
    let mut new = release("root0000", "2", 'b');
    let dependency = release("dep00000", "1", 'c');
    requires(&mut new, &dependency, ResourceDependencyKind::Required);
    let (_directory, service) =
        service(vec![old.clone(), new.clone(), dependency.clone()]).await;
    let selections = vec![
        selection("first", &new, Some("a")),
        selection("second", &new, Some("b")),
    ];
    let current = vec![
        installed(&old, "a", "mods/custom-prefix.jar.disabled"),
        installed(&old, "b", "mods/copy.jar"),
    ];
    let plan = service
        .build_operation(
            draft(selections, ResourceOperationIntent::Update),
            &target(),
            &current,
            &CancellationToken::new(),
        )
        .await
        .unwrap()
        .plan;
    assert!(plan.issues.is_empty(), "{:?}", plan.issues);
    assert_eq!(plan.actions.len(), 3);
    assert_eq!(plan.actions[0].entry_id.as_str(), "a");
    assert_eq!(
        plan.actions[0].target_relative_path,
        "mods/custom-prefix.jar.disabled"
    );
    assert_eq!(plan.actions[1].entry_id.as_str(), "b");
    assert_eq!(plan.actions[1].target_relative_path, "mods/copy.jar");
    assert_eq!(
        plan.nodes
            .iter()
            .filter(|node| node.artifact.as_ref().is_some_and(|artifact| {
                artifact.reference.release.project.project_id.as_str()
                    == "dep00000"
            }))
            .count(),
        1
    );
    assert_eq!(plan.edges.len(), 2);
    service.cache().close().await;
}

#[tokio::test]
async fn optional_dependency_closure_only_appears_after_activation_and_can_be_removed()
 {
    let mut a = release("root0000", "1", 'a');
    let mut b = release("opt00000", "1", 'b');
    let c = release("dep00000", "1", 'c');
    requires(&mut b, &c, ResourceDependencyKind::Required);
    requires(&mut a, &b, ResourceDependencyKind::Optional);
    let (_directory, service) = service(vec![a.clone(), b, c]).await;
    let first = service
        .build_operation(
            draft(
                vec![selection("root", &a, None)],
                ResourceOperationIntent::Install,
            ),
            &target(),
            &[],
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(first.plan.actions.len(), 1);
    assert_eq!(first.candidates.len(), 1);
    assert_eq!(first.plan.nodes.len(), 2);
    let mut active = first.clone();
    active.plan.selections.push(first.candidates[0].clone());
    active
        .plan
        .active_roots
        .push(first.candidates[0].id.clone());
    let active = service
        .build_operation(active, &target(), &[], &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(active.plan.actions.len(), 3);
    assert!(active.plan.issues.is_empty());
    let mut removed = active;
    removed.plan.selections.truncate(1);
    removed.plan.active_roots.truncate(1);
    let removed = service
        .build_operation(removed, &target(), &[], &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(removed.plan.actions.len(), 1);
    assert_eq!(removed.plan.nodes.len(), 2);
    service.cache().close().await;
}

#[tokio::test]
async fn duplicate_with_target_bytes_does_not_satisfy_the_entry_being_updated()
{
    let old = release("root0000", "1", 'a');
    let new = release("root0000", "2", 'b');
    let (_directory, service) = service(vec![old.clone(), new.clone()]).await;
    let mut already_new = installed(&new, "copy", "mods/copy.jar");
    already_new.enabled = true;
    let plan = service
        .build_operation(
            draft(
                vec![selection("root", &new, Some("old"))],
                ResourceOperationIntent::Update,
            ),
            &target(),
            &[installed(&old, "old", "mods/original.jar"), already_new],
            &CancellationToken::new(),
        )
        .await
        .unwrap()
        .plan;
    assert!(plan.issues.is_empty(), "{:?}", plan.issues);
    assert_eq!(plan.actions.len(), 1);
    assert_eq!(plan.actions[0].entry_id.as_str(), "old");
    assert_eq!(plan.nodes[0].state, DependencyNodeState::Update);
    assert!(plan.nodes[0].installed_entries.is_empty());
    service.cache().close().await;
}

#[tokio::test]
async fn optional_dependency_of_a_required_dependency_stays_a_candidate() {
    let mut a = release("root0000", "1", 'a');
    let mut c = release("dep00000", "1", 'c');
    let e = release("opt00000", "1", 'e');
    requires(&mut a, &c, ResourceDependencyKind::Required);
    requires(&mut c, &e, ResourceDependencyKind::Optional);
    let (_directory, service) =
        service(vec![a.clone(), c.clone(), e.clone()]).await;
    let draft = service
        .build_operation(
            draft(
                vec![selection("a", &a, None)],
                ResourceOperationIntent::Install,
            ),
            &target(),
            &[],
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(draft.plan.issues.is_empty(), "{:?}", draft.plan.issues);
    assert_eq!(draft.plan.actions.len(), 2);
    assert_eq!(draft.candidates.len(), 1);
    assert!(matches!(
        &draft.candidates[0].resource,
        ResourceRef::Source { project }
            if project.project_id.as_str() == "opt00000"
    ));
    let candidate = &draft.candidates[0];
    assert!(draft.plan.nodes.iter().any(|node| {
        node.id == candidate.id
            && node.state == DependencyNodeState::OptionalCandidate
    }));
    let parent = draft.plan.nodes.iter().find(|node| {
        node.artifact.as_ref().is_some_and(|artifact| {
            artifact.reference.release.project.project_id.as_str() == "dep00000"
        })
    });
    let parent = parent.expect("required dependency node");
    assert!(draft.plan.edges.iter().any(|edge| {
        edge.parent == parent.id
            && edge.child == candidate.id
            && edge.kind == ResourceDependencyKind::Optional
    }));
    service.cache().close().await;
}

#[tokio::test]
async fn curseforge_automatic_roots_and_optional_candidates_resolve_under_channel_targets()
 {
    fn file_for(id: u64) -> Value {
        let mut file: Value = serde_json::from_str(fixtures::CF_IRIS).unwrap();
        if id == 5425379 {
            file["id"] = json!(5425379);
            file["modId"] = json!(394468);
            file["fileName"] = json!("optional-dependency-1.0.jar");
            file["displayName"] = json!("Optional Dependency 1.0");
            file["dependencies"] = json!([]);
        } else {
            file["dependencies"] =
                json!([{"modId": 394468, "relationType": 2}]);
        }
        file
    }
    let transport = FixtureTransport::new(|_, request| {
        match request.path.as_str() {
            "/v1/mods/455508/files" | "/v1/mods/394468/files" => {
                assert!(
                    request
                        .query
                        .contains(&("gameVersion".into(), "1.20.1".into()))
                );
                assert!(
                    request
                        .query
                        .contains(&("modLoaderType".into(), "4".into()))
                );
                let id = if request.path.starts_with("/v1/mods/394468") {
                    5425379
                } else {
                    5425378
                };
                Ok(json!({
                    "data": [file_for(id)],
                    "pagination": {"index": 0, "resultCount": 1, "totalCount": 1}
                }))
            }
            "/v1/mods/files" => {
                let ids = request.body.as_ref().unwrap()["fileIds"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter_map(Value::as_u64)
                    .collect::<Vec<_>>();
                Ok(json!({
                    "data": ids.into_iter().map(file_for).collect::<Vec<_>>()
                }))
            }
            "/v1/mods" => {
                let ids = request.body.as_ref().unwrap()["modIds"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter_map(Value::as_u64)
                    .collect::<Vec<_>>();
                Ok(json!({"data": ids.into_iter().map(|id| json!({
                    "id": id,
                    "name": format!("Project {id}"),
                    "slug": format!("project-{id}"),
                    "classId": 6
                })).collect::<Vec<_>>()}))
            }
            path => panic!("unexpected endpoint {path}"),
        }
    });
    let (_directory, cache) = cache().await;
    let service = ResourceService::new(
        cache.clone(),
        vec![Arc::new(Available(Arc::new(CurseForgeProvider::new(
            transport.clone(),
        ))))],
    )
    .unwrap();
    let root = ResourceSelection {
        id: SelectionId::new("root").unwrap(),
        resource: ResourceRef::Source {
            project: SourceProjectRef {
                source: SourceId::curseforge(),
                project_id: SourceProjectId::new("455508").unwrap(),
            },
        },
        release: ReleaseSelection::Automatic,
        resource_type: ResourceType::Mod,
        scope: InstallScope::Instance,
        entry_id: None,
    };
    let first = service
        .build_operation(
            draft(vec![root], ResourceOperationIntent::Install),
            &target(),
            &[],
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(first.plan.issues.is_empty(), "{:?}", first.plan.issues);
    assert_eq!(first.plan.actions.len(), 1);
    assert_eq!(first.candidates.len(), 1);
    assert!(first.plan.nodes.iter().any(|node| {
        node.state == DependencyNodeState::OptionalCandidate
            && matches!(&node.resource, ResourceRef::Source { project } if project.project_id.as_str() == "394468")
    }));
    let mut active = first;
    let candidate = active.candidates[0].clone();
    active.plan.selections.push(candidate.clone());
    active.plan.active_roots.push(candidate.id.clone());
    let active = service
        .build_operation(active, &target(), &[], &CancellationToken::new())
        .await
        .unwrap();
    assert!(active.plan.issues.is_empty(), "{:?}", active.plan.issues);
    assert_eq!(active.plan.actions.len(), 2);
    assert_eq!(active.plan.edges.len(), 1);
    assert!(active.plan.nodes.iter().any(|node| {
        node.artifact.as_ref().is_some_and(|artifact| {
            artifact.reference.release.project.project_id.as_str() == "394468"
        })
    }));
    service.cache().close().await;
}

#[tokio::test]
async fn dependency_conflicts_and_foreign_entry_ownership_block_a_plan() {
    let mut a = release("root0000", "1", 'a');
    let mut d = release("root1111", "1", 'd');
    let c1 = release("dep00000", "1", 'c');
    let c2 = release("dep00000", "2", 'e');
    requires(&mut a, &c1, ResourceDependencyKind::Required);
    requires(&mut d, &c2, ResourceDependencyKind::Required);
    let (_directory, service) =
        service(vec![a.clone(), d.clone(), c1, c2]).await;
    let plan = service
        .build_operation(
            draft(
                vec![selection("a", &a, None), selection("d", &d, None)],
                ResourceOperationIntent::Install,
            ),
            &target(),
            &[],
            &CancellationToken::new(),
        )
        .await
        .unwrap()
        .plan;
    assert!(
        plan.issues
            .iter()
            .any(|issue| issue.code == ResourceIssueCode::VersionConflict
                && issue.blocking)
    );
    let current = vec![installed(&d, "entry", "mods/entry.jar")];
    let plan = service
        .build_operation(
            draft(
                vec![selection("foreign", &a, Some("entry"))],
                ResourceOperationIntent::SwitchVersion,
            ),
            &target(),
            &current,
            &CancellationToken::new(),
        )
        .await
        .unwrap()
        .plan;
    assert!(
        plan.issues
            .iter()
            .any(|issue| issue.code == ResourceIssueCode::ArtifactMismatch
                && issue.blocking)
    );
    service.cache().close().await;
}

#[tokio::test]
async fn an_existing_root_does_not_swallow_a_different_exact_dependency_artifact()
 {
    let mut a = release("root0000", "1", 'a');
    let d1 = release("dep00000", "1", 'c');
    let d2 = release("dep00000", "2", 'e');
    requires(&mut a, &d2, ResourceDependencyKind::Required);
    let (_directory, service) = service(vec![a.clone(), d1.clone(), d2]).await;
    let plan = service
        .build_operation(
            draft(
                vec![
                    selection("dependency-root", &d1, None),
                    selection("parent", &a, None),
                ],
                ResourceOperationIntent::Install,
            ),
            &target(),
            &[],
            &CancellationToken::new(),
        )
        .await
        .unwrap()
        .plan;
    assert!(
        plan.issues
            .iter()
            .any(|issue| issue.code == ResourceIssueCode::VersionConflict
                && issue.blocking)
    );
    service.cache().close().await;
}

#[tokio::test]
async fn disabled_unchanged_file_does_not_trigger_reinstall_and_unknown_requires_explicit_exception()
 {
    let release = release("root0000", "1", 'a');
    let (_directory, service) = service(vec![release.clone()]).await;
    let current =
        vec![installed(&release, "entry", "mods/custom.jar.disabled")];
    let plan = service
        .build_operation(
            draft(
                vec![selection("root", &release, Some("entry"))],
                ResourceOperationIntent::Update,
            ),
            &target(),
            &current,
            &CancellationToken::new(),
        )
        .await
        .unwrap()
        .plan;
    assert!(plan.actions.is_empty());
    let mut environment = target();
    environment.game_versions.clear();
    let plan = service
        .build_operation(
            draft(
                vec![selection("root", &release, None)],
                ResourceOperationIntent::Install,
            ),
            &environment,
            &[],
            &CancellationToken::new(),
        )
        .await
        .unwrap()
        .plan;
    assert!(plan.issues.iter().any(|issue| issue.code
        == ResourceIssueCode::CompatibilityUnknown
        && issue.blocking));
    service.cache().close().await;
}

#[tokio::test]
async fn fixed_compatibility_exceptions_are_dimension_release_and_selection_scoped()
 {
    let mut root = release("root0000", "1", 'a');
    root.compatibility = ReleaseCompatibility {
        game_versions: vec!["1.12.2".into()],
        loaders: vec!["forge".into()],
    };
    let mut child = release("child000", "1", 'b');
    child.compatibility.game_versions = vec!["1.12.2".into()];
    requires(&mut root, &child, ResourceDependencyKind::Required);
    let (_directory, service) =
        service(vec![root.clone(), child.clone()]).await;
    let environment = target();
    let cancel = CancellationToken::new();
    let mut initial = draft(
        vec![selection("root", &root, None)],
        ResourceOperationIntent::Install,
    );
    initial.plan.exceptions.push(CompatibilityException {
        selection_id: SelectionId::new("root").unwrap(),
        release: root.reference.clone(),
        dimensions: vec![CompatibilityDimension::GameVersion],
    });
    let assessed = service
        .build_operation(initial.clone(), &environment, &[], &cancel)
        .await
        .unwrap()
        .plan;
    assert!(assessed.issues.iter().any(|issue| {
        issue
            .selection_id
            .as_ref()
            .is_some_and(|id| id.as_str() == "root")
            && issue.code == ResourceIssueCode::CompatibilityMismatch
    }));
    assert!(
        assessed
            .nodes
            .iter()
            .find(|node| node.id.as_str() == "root")
            .unwrap()
            .compatibility
            .game_version
            == CompatibilityState::Incompatible
    );
    initial.plan.exceptions[0]
        .dimensions
        .push(CompatibilityDimension::Loader);
    let accepted = service
        .build_operation(initial.clone(), &environment, &[], &cancel)
        .await
        .unwrap()
        .plan;
    assert!(!accepted.issues.iter().any(|issue| {
        issue
            .selection_id
            .as_ref()
            .is_some_and(|id| id.as_str() == "root")
            && issue.code == ResourceIssueCode::CompatibilityMismatch
    }));
    assert!(accepted.issues.iter().any(|issue| {
        issue
            .selection_id
            .as_ref()
            .is_some_and(|id| id.as_str() != "root")
            && issue.code == ResourceIssueCode::CompatibilityMismatch
    }));
    assert_eq!(
        accepted
            .actions
            .iter()
            .find(|action| action.node_id.as_str() == "root")
            .unwrap()
            .target,
        root.artifacts[0]
    );
    initial.plan.exceptions[0].release.release_id =
        SourceReleaseId::new("another-release").unwrap();
    let changed = service
        .build_operation(initial, &environment, &[], &cancel)
        .await
        .unwrap()
        .plan;
    assert!(changed.issues.iter().any(|issue| {
        issue
            .selection_id
            .as_ref()
            .is_some_and(|id| id.as_str() == "root")
            && issue.code == ResourceIssueCode::CompatibilityMismatch
    }));
    service.cache().close().await;
}

#[tokio::test]
async fn automatic_selection_does_not_use_explicit_compatibility_exceptions() {
    let compatible = release("root0000", "1", 'a');
    let mut incompatible = release("root0000", "2", 'b');
    incompatible.compatibility.game_versions = vec!["1.12.2".into()];
    let (_directory, service) =
        service(vec![compatible.clone(), incompatible.clone()]).await;
    let mut automatic = selection("root", &incompatible, None);
    automatic.release = ReleaseSelection::Automatic;
    let mut initial = draft(vec![automatic], ResourceOperationIntent::Install);
    initial.plan.exceptions.push(CompatibilityException {
        selection_id: SelectionId::new("root").unwrap(),
        release: incompatible.reference,
        dimensions: vec![CompatibilityDimension::GameVersion],
    });
    let plan = service
        .build_operation(initial, &target(), &[], &CancellationToken::new())
        .await
        .unwrap()
        .plan;
    assert_eq!(
        plan.actions[0].target.reference,
        compatible.artifacts[0].reference
    );
    service.cache().close().await;
}

#[tokio::test]
async fn compatibility_exceptions_cannot_authorize_an_unsafe_artifact_path() {
    let mut root = release("root0000", "1", 'a');
    root.compatibility.game_versions = vec!["1.12.2".into()];
    root.artifacts[0].file_name = Some("../unsafe.jar".into());
    let (_directory, service) = service(vec![root.clone()]).await;
    let mut initial = draft(
        vec![selection("root", &root, None)],
        ResourceOperationIntent::Install,
    );
    initial.plan.exceptions.push(CompatibilityException {
        selection_id: SelectionId::new("root").unwrap(),
        release: root.reference,
        dimensions: vec![
            CompatibilityDimension::GameVersion,
            CompatibilityDimension::Loader,
        ],
    });
    let plan = service
        .build_operation(initial, &target(), &[], &CancellationToken::new())
        .await
        .unwrap()
        .plan;
    assert!(plan.actions.is_empty());
    assert!(plan.issues.iter().any(|issue| matches!(
        issue.code,
        ResourceIssueCode::InvalidPath
            | ResourceIssueCode::SourceUnavailable
            | ResourceIssueCode::MissingDependency
    ) && issue.blocking));
    service.cache().close().await;
}

#[tokio::test]
async fn optional_activity_preserves_shared_independent_and_installed_requirements()
 {
    let mut a = release("root0000", "1", 'a');
    let mut b = release("option00", "1", 'b');
    let c = release("shared00", "1", 'c');
    let mut d = release("parent00", "1", 'd');
    let mut e = release("nested00", "1", 'e');
    let f = release("leaf0000", "1", 'f');
    requires(&mut a, &b, ResourceDependencyKind::Optional);
    requires(&mut b, &c, ResourceDependencyKind::Required);
    requires(&mut b, &e, ResourceDependencyKind::Optional);
    requires(&mut d, &c, ResourceDependencyKind::Required);
    requires(&mut e, &f, ResourceDependencyKind::Required);
    let (_directory, service) = service(vec![
        a.clone(),
        b.clone(),
        c.clone(),
        d.clone(),
        e.clone(),
        f,
    ])
    .await;
    let initial = service
        .build_operation(
            draft(
                vec![selection("a", &a, None), selection("d", &d, None)],
                ResourceOperationIntent::Install,
            ),
            &target(),
            &[],
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(initial.plan.actions.len(), 3);
    assert!(!initial.plan.nodes.iter().any(|node| {
        node.artifact.as_ref().is_some_and(|artifact| {
            artifact.reference.release.project == e.reference.project
        })
    }));
    let mut activated = initial;
    let optional = activated.candidates[0].clone();
    activated.plan.active_roots.push(optional.id.clone());
    activated.plan.selections.push(optional);
    let activated = service
        .build_operation(activated, &target(), &[], &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(activated.plan.actions.len(), 4);
    let shared = activated
        .plan
        .nodes
        .iter()
        .find(|node| {
            node.artifact.as_ref().is_some_and(|artifact| {
                artifact.reference.release.project == c.reference.project
            })
        })
        .unwrap();
    assert_eq!(
        activated
            .plan
            .edges
            .iter()
            .filter(|edge| edge.child == shared.id
                && edge.kind == ResourceDependencyKind::Required)
            .count(),
        2
    );
    assert!(
        activated
            .plan
            .nodes
            .iter()
            .any(|node| node.state == DependencyNodeState::OptionalCandidate)
    );
    let mut removed = activated;
    removed.plan.selections.retain(|selection| {
        selection.id.as_str() == "a" || selection.id.as_str() == "d"
    });
    removed.plan.active_roots = vec![
        SelectionId::new("a").unwrap(),
        SelectionId::new("d").unwrap(),
    ];
    let removed = service
        .build_operation(
            removed,
            &target(),
            &[installed(&c, "existing", "mods/custom.jar")],
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(removed.plan.actions.len(), 2);
    assert!(
        removed
            .plan
            .nodes
            .iter()
            .any(|node| node.state == DependencyNodeState::Satisfied)
    );
    service.cache().close().await;
}

#[tokio::test]
async fn unpinned_required_dependency_reuses_an_installed_compatible_release() {
    let mut a = release("root0000", "1", 'a');
    let old = release("depend00", "1", 'b');
    let newer = release("depend00", "2", 'c');
    a.artifacts[0].dependencies.push(ArtifactDependency {
        target: DependencyTarget::Resource {
            reference: ResourceRef::Source {
                project: old.reference.project.clone(),
            },
        },
        kind: ResourceDependencyKind::Required,
        version_constraint: None,
    });
    let (_directory, service) =
        service(vec![a.clone(), old.clone(), newer]).await;
    let mut request = draft(
        vec![selection("a", &a, None)],
        ResourceOperationIntent::Install,
    );
    request.install_dependencies = false;
    let plan = service
        .build_operation(
            request,
            &target(),
            &[installed(&old, "present", "mods/custom.jar")],
            &CancellationToken::new(),
        )
        .await
        .unwrap()
        .plan;
    assert!(plan.issues.is_empty(), "{:?}", plan.issues);
    assert_eq!(plan.actions.len(), 1);
    let dependency = plan
        .nodes
        .iter()
        .find(|node| node.state == DependencyNodeState::Satisfied)
        .unwrap();
    assert_eq!(
        dependency.artifact.as_ref().unwrap().reference,
        old.artifacts[0].reference
    );
    service.cache().close().await;
}

#[tokio::test]
async fn optional_installed_status_requires_the_declared_release_enabled_file_and_scope()
 {
    let mut a = release("root0000", "1", 'a');
    let old = release("option00", "1", 'b');
    let current = release("option00", "2", 'c');
    requires(&mut a, &current, ResourceDependencyKind::Optional);
    let (_directory, service) =
        service(vec![a.clone(), old.clone(), current.clone()]).await;
    let mut disabled =
        installed(&current, "disabled", "mods/current.jar.disabled");
    disabled.enabled = false;
    let mut missing = installed(&current, "missing", "mods/current.jar");
    missing.missing = true;
    let mut wrong_scope =
        installed(&current, "world", "saves/a/datapacks/current.zip");
    wrong_scope.scope = InstallScope::World {
        relative_path: "saves/a".into(),
    };
    for entries in [
        vec![installed(&old, "old", "mods/old.jar")],
        vec![disabled],
        vec![missing],
        vec![wrong_scope],
        vec![installed(&current, "valid", "mods/current.jar")],
    ] {
        let valid = entries[0].entry_id.as_str() == "valid";
        let result = service
            .build_operation(
                draft(
                    vec![selection("a", &a, None)],
                    ResourceOperationIntent::Install,
                ),
                &target(),
                &entries,
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(result.plan.actions.len(), 1);
        assert_eq!(result.candidates.is_empty(), valid);
        assert!(result.plan.nodes.iter().any(|node| node.state
            == if valid {
                DependencyNodeState::Satisfied
            } else {
                DependencyNodeState::OptionalCandidate
            }));
        if valid {
            let dependency = result
                .plan
                .nodes
                .iter()
                .find(|node| node.state == DependencyNodeState::Satisfied)
                .unwrap();
            assert_eq!(
                dependency.compatibility.game_version,
                CompatibilityState::Compatible
            );
            assert_eq!(
                dependency.compatibility.loader,
                CompatibilityState::Compatible
            );
            assert_eq!(
                dependency.declared_compatibility.as_ref(),
                Some(&current.compatibility)
            );
        }
    }
    service.cache().close().await;
}

#[tokio::test]
async fn merged_explicit_roots_preserve_both_selection_ids_and_frozen_artifacts()
 {
    let a = release("root0000", "1", 'a');
    let (_directory, service) = service(vec![a.clone()]).await;
    let result = service
        .build_operation(
            draft(
                vec![
                    selection("first", &a, None),
                    selection("second", &a, None),
                ],
                ResourceOperationIntent::Install,
            ),
            &target(),
            &[],
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(result.plan.nodes.len(), 1);
    assert_eq!(result.plan.actions.len(), 1);
    assert_eq!(result.plan.active_roots.len(), 2);
    assert_eq!(result.plan.nodes[0].root_selection_ids.len(), 2);
    assert!(result.plan.selections.iter().all(|selection| matches!(
        selection.release,
        ReleaseSelection::Artifact { .. }
    )));
    let revised = service
        .revise_operation(
            result,
            vec![ResourcePlanDecision::RemoveRoot {
                selection_id: SelectionId::new("second").unwrap(),
            }],
            &target(),
            &[],
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(revised.plan.actions.len(), 1);
    assert_eq!(
        revised.plan.active_roots,
        vec![SelectionId::new("first").unwrap()]
    );
    service.cache().close().await;
}

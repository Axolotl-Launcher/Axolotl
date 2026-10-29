use super::*;
use crate::resources::plans::{
    OperationDraft, ResourceOperationPhase, ResourceOperationProgress,
};
use std::collections::HashMap;

#[path = "../state/store/tests/content_identity_confirmation.rs"]
mod content_identity_confirmation;

struct MissingReleaseProvider(PlanProvider, Option<ResourceError>);

#[async_trait]
impl ResourceProvider for MissingReleaseProvider {
    fn source(&self) -> ResourceSource {
        self.0.source()
    }
    fn namespace(&self) -> String {
        self.0.namespace()
    }
    fn batch_limit(&self, query: &ProviderQuery) -> usize {
        self.0.batch_limit(query)
    }
    async fn fetch_many(
        &self,
        queries: &[ProviderQuery],
    ) -> Vec<ResourceResult<ProviderValue>> {
        self.0
            .fetch_many(queries)
            .await
            .into_iter()
            .zip(queries)
            .map(|(result, query)| {
                if matches!(query, ProviderQuery::Release { .. })
                    && matches!(result, Err(ResourceError::Unavailable))
                {
                    self.1.clone().map_or(Ok(ProviderValue::NotFound), Err)
                } else {
                    result
                }
            })
            .collect()
    }
}

async fn missing_release_service(
    releases: Vec<ResourceRelease>,
) -> (tempfile::TempDir, Arc<ResourceService>) {
    let (directory, cache) = cache().await;
    let service = ResourceService::new(
        cache,
        vec![Arc::new(MissingReleaseProvider(
            PlanProvider {
                source: SourceId::modrinth(),
                releases,
                requests: Mutex::new(vec![]),
            },
            None,
        ))],
    )
    .unwrap();
    (directory, service)
}

#[tokio::test]
async fn actual_metadata_timeout_downgrades_dependency_lookup_but_keeps_internal_errors_blocking()
 {
    let timeout: crate::Result<()> = crate::api::curseforge::metadata_deadline(
        tokio::time::Instant::now(),
        std::future::pending(),
    )
    .await;
    let timeout = ResourceError::from(timeout.unwrap_err());
    assert_eq!(timeout, ResourceError::Unavailable);
    for error in [
        timeout,
        ResourceError::Database("broken store".into()),
        ResourceError::Canceled,
    ] {
        let mut root = release("root0000", "1", 'a');
        root.artifacts[0].dependencies.push(ArtifactDependency {
            target: DependencyTarget::SourceRelease {
                source: SourceId::modrinth(),
                release_id: SourceReleaseId::new("missing1").unwrap(),
            },
            kind: ResourceDependencyKind::Required,
            version_constraint: None,
        });
        let (_temp, cache) = cache().await;
        let service = ResourceService::new(
            cache.clone(),
            vec![Arc::new(MissingReleaseProvider(
                PlanProvider {
                    source: SourceId::modrinth(),
                    releases: vec![root.clone()],
                    requests: Mutex::new(vec![]),
                },
                Some(error.clone()),
            ))],
        )
        .unwrap();
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
            .await;
        match error {
            ResourceError::Unavailable => {
                let built = built.unwrap();
                assert_eq!(built.plan.actions.len(), 1);
                assert!(!built.plan.issues.iter().any(|issue| issue.blocking));
                assert!(built.plan.issues.iter().any(|issue| issue.code
                    == ResourceIssueCode::DependencyAnalysisFailed));
            }
            ResourceError::Canceled => {
                assert!(matches!(built, Err(ResourceError::Canceled)))
            }
            _ => {
                if let Ok(built) = built {
                    assert!(
                        built.plan.issues.iter().any(|issue| issue.blocking)
                    );
                }
            }
        }
        cache.close().await;
    }
}

#[tokio::test]
async fn real_not_found_dependencies_warn_without_discarding_known_parent_constraints()
 {
    let mut root = release("root0000", "1", 'a');
    let missing = release("missing0", "1", 'b');
    root.artifacts[0].dependencies.push(ArtifactDependency {
        target: DependencyTarget::SourceRelease {
            source: SourceId::modrinth(),
            release_id: missing.reference.release_id.clone(),
        },
        kind: ResourceDependencyKind::Required,
        version_constraint: None,
    });
    let mut parent = release("parent00", "1", 'c');
    parent.artifacts[0].dependencies.push(ArtifactDependency {
        target: DependencyTarget::Resource {
            reference: ResourceRef::Source {
                project: root.reference.project.clone(),
            },
        },
        kind: ResourceDependencyKind::Required,
        version_constraint: Some("[2,)".into()),
    });
    let (_temp, service) =
        missing_release_service(vec![root.clone(), parent.clone()]).await;
    for known_parent in [false, true] {
        let mut entries =
            vec![installed(&missing, "missing", "mods/missing.jar")];
        if known_parent {
            entries.push(installed(&parent, "parent", "mods/parent.jar"));
        }
        let built = service
            .build_operation(
                draft(
                    vec![selection("root", &root, None)],
                    ResourceOperationIntent::Install,
                ),
                &target(),
                &entries,
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        assert!(
            built.plan.issues.iter().any(|issue| issue.code
                == ResourceIssueCode::DependencyAnalysisFailed)
        );
        assert_eq!(
            built.plan.issues.iter().any(|issue| issue.blocking),
            known_parent
        );
        if !known_parent {
            assert_eq!(built.plan.actions.len(), 1);
        } else {
            assert!(!built.retained_requirements.is_empty());
        }
    }
    service.cache().close().await;
}

#[tokio::test]
async fn a_missing_sibling_keeps_the_same_parents_known_constraints_in_either_order()
 {
    for missing_first in [false, true] {
        for exact_missing in [false, true] {
            let root = release("root0000", "1", 'a');
            let missing = release("missing0", "1", 'b');
            let mut parent = release("parent00", "1", 'c');
            let known = ArtifactDependency {
                target: DependencyTarget::Resource {
                    reference: ResourceRef::Source {
                        project: root.reference.project.clone(),
                    },
                },
                kind: ResourceDependencyKind::Required,
                version_constraint: Some("[2,)".into()),
            };
            let unknown = ArtifactDependency {
                target: if exact_missing {
                    DependencyTarget::Release {
                        reference: missing.reference.clone(),
                    }
                } else {
                    DependencyTarget::SourceRelease {
                        source: SourceId::modrinth(),
                        release_id: missing.reference.release_id.clone(),
                    }
                },
                kind: ResourceDependencyKind::Required,
                version_constraint: None,
            };
            parent.artifacts[0].dependencies = if missing_first {
                vec![unknown, known]
            } else {
                vec![known, unknown]
            };
            let (_temp, service) =
                missing_release_service(vec![root.clone(), parent.clone()])
                    .await;
            let built = service
                .build_operation(
                    draft(
                        vec![selection("root", &root, None)],
                        ResourceOperationIntent::Install,
                    ),
                    &target(),
                    &[installed(&parent, "parent", "mods/parent.jar")],
                    &CancellationToken::new(),
                )
                .await
                .unwrap();
            assert!(built.plan.issues.iter().any(|issue| issue.code
                == ResourceIssueCode::DependencyAnalysisFailed));
            assert!(built.plan.issues.iter().any(|issue| issue.blocking
                && issue.code == ResourceIssueCode::VersionConflict));
            assert!(built.plan.actions.is_empty());
            assert_eq!(built.retained_requirements.len(), 1);
            assert_eq!(
                built.retained_requirements[0].version_constraint.as_deref(),
                Some("[2,)")
            );
            service.cache().close().await;
        }
    }
}

#[tokio::test]
async fn unavailable_installed_incompatible_version_is_unknown_in_the_final_plan()
 {
    let mut root = release("root0000", "1", 'a');
    let missing = release("missing0", "1", 'b');
    root.artifacts[0].dependencies.push(ArtifactDependency {
        target: DependencyTarget::Resource {
            reference: ResourceRef::Source {
                project: missing.reference.project.clone(),
            },
        },
        kind: ResourceDependencyKind::Incompatible,
        version_constraint: Some("[1,)".into()),
    });
    let (_temp, service) = missing_release_service(vec![root.clone()]).await;
    let built = service
        .build_operation(
            draft(
                vec![selection("root", &root, None)],
                ResourceOperationIntent::Install,
            ),
            &target(),
            &[installed(&missing, "missing", "mods/missing.jar")],
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
async fn resource_operation_futures_can_cross_runtime_threads() {
    fn require_send<T: Send>(value: T) -> T {
        value
    }

    let release = release("root0000", "1", 'a');
    let (_directory, service) = service(vec![release.clone()]).await;
    let target = target();
    let cancel = CancellationToken::new();
    let memo = plans::analysis::AnalysisMemo::default();
    let plan = require_send(service.build_operation_with_memo(
        draft(
            vec![selection("root", &release, None)],
            ResourceOperationIntent::Install,
        ),
        &target,
        &[],
        &cancel,
        None,
        Some(&memo),
    ))
    .await
    .unwrap();
    assert_eq!(plan.plan.actions.len(), 1);

    let session_id = SessionId::new("send-contract").unwrap();
    // Tauri runs these futures on its multithreaded runtime. Enforce the
    // contract without initializing the global application state in a test.
    drop(require_send(plans::analysis::update(
        &session_id,
        1,
        vec![],
        true,
        &cancel,
        None,
        false,
        None,
    )));
    drop(require_send(plans::prepare_with_progress(
        "instance".into(),
        ResourceOperationIntent::Install,
        vec![],
        true,
        &cancel,
        None,
        true,
    )));
    service.cache().close().await;
}

#[tokio::test]
async fn cart_analysis_reuses_unchanged_resolutions_and_removes_only_unused_dependencies()
 {
    let shared = release("shared00", "1", 'c');
    let mut first = release("first000", "1", 'a');
    let mut second = release("second00", "1", 'b');
    requires(&mut first, &shared, ResourceDependencyKind::Required);
    requires(&mut second, &shared, ResourceDependencyKind::Required);
    let (directory, cache) = cache().await;
    let provider = Arc::new(PlanProvider {
        source: SourceId::modrinth(),
        releases: vec![first.clone(), second.clone(), shared.clone()],
        requests: Mutex::new(vec![]),
    });
    let service = ResourceService::new(cache, vec![provider.clone()]).unwrap();
    let memo = plans::analysis::AnalysisMemo::default();
    let cancel = CancellationToken::new();
    let one = draft(
        vec![selection("first", &first, None)],
        ResourceOperationIntent::Install,
    );
    service
        .build_operation_with_memo(
            one,
            &target(),
            &[],
            &cancel,
            None,
            Some(&memo),
        )
        .await
        .unwrap();

    // Clear the source cache so a repeated network request cannot hide behind
    // HTTP/cache reuse. The cart memo must retain the unchanged resolution.
    service.cache().clear().await.unwrap();
    provider.requests.lock().unwrap().clear();
    let both = draft(
        vec![
            selection("first", &first, None),
            selection("second", &second, None),
        ],
        ResourceOperationIntent::Install,
    );
    let result = service
        .build_operation_with_memo(
            both,
            &target(),
            &[],
            &cancel,
            None,
            Some(&memo),
        )
        .await
        .unwrap();
    assert_eq!(result.plan.actions.len(), 3);
    assert_eq!(
        result
            .plan
            .nodes
            .iter()
            .filter(|node| node.artifact.as_ref().is_some_and(|artifact| {
                artifact.reference.release == shared.reference
            }))
            .count(),
        1
    );
    assert!(!provider.requests.lock().unwrap().iter().any(|query| matches!(query,
        ProviderQuery::Release { release_id, .. } if release_id == &first.reference.release_id || release_id == &shared.reference.release_id
    )));

    let remaining = draft(
        vec![selection("second", &second, None)],
        ResourceOperationIntent::Install,
    );
    let result = service
        .build_operation_with_memo(
            remaining,
            &target(),
            &[],
            &cancel,
            None,
            Some(&memo),
        )
        .await
        .unwrap();
    assert_eq!(result.plan.actions.len(), 2);
    assert!(
        result.plan.actions.iter().all(|action| action
            .target
            .reference
            .release
            != first.reference)
    );
    assert!(
        result.plan.actions.iter().any(|action| action
            .target
            .reference
            .release
            == shared.reference)
    );
    service.cache().close().await;
    drop(directory);
}

#[tokio::test]
async fn cart_analysis_rechecks_shared_constraints_after_adding_and_removing_a_conflicting_root()
 {
    let old = release("shared00", "1", 'a');
    let new = release("shared00", "2", 'b');
    let mut first = release("first000", "1", 'c');
    let mut second = release("second00", "1", 'd');
    requires(&mut first, &old, ResourceDependencyKind::Required);
    requires(&mut second, &new, ResourceDependencyKind::Required);
    let (_directory, service) = service(vec![
        first.clone(),
        second.clone(),
        old.clone(),
        new.clone(),
    ])
    .await;
    let memo = plans::analysis::AnalysisMemo::default();
    let cancel = CancellationToken::new();
    for (roots, conflict, expected) in [
        (
            vec![selection("first", &first, None)],
            false,
            old.reference.clone(),
        ),
        (
            vec![
                selection("first", &first, None),
                selection("second", &second, None),
            ],
            true,
            old.reference.clone(),
        ),
        (
            vec![selection("second", &second, None)],
            false,
            new.reference.clone(),
        ),
    ] {
        let result = service
            .build_operation_with_memo(
                draft(roots, ResourceOperationIntent::Install),
                &target(),
                &[],
                &cancel,
                None,
                Some(&memo),
            )
            .await
            .unwrap();
        assert_eq!(
            result.plan.issues.iter().any(|issue| issue.blocking
                && issue.code == ResourceIssueCode::VersionConflict),
            conflict
        );
        if !conflict {
            assert!(
                result.plan.actions.iter().any(|action| action
                    .target
                    .reference
                    .release
                    == expected)
            );
        }
    }
    service.cache().close().await;
}

#[tokio::test]
async fn remote_artifact_file_names_are_sanitized_for_install_and_update() {
    let mut root = release("root0000", "1", 'a');
    root.artifacts[0].file_name = Some("mod:version?\n.jar".into());
    let (_directory, service) = service(vec![root.clone()]).await;
    let plan = service
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
        .unwrap()
        .plan;
    assert_eq!(plan.actions.len(), 1);
    assert_eq!(
        plan.actions[0].target_relative_path,
        "mods/mod_version__.jar"
    );
    assert_eq!(
        plan.actions[0].target.file_name.as_deref(),
        Some("mod:version?\n.jar")
    );

    let mut current =
        installed(&root, "entry", "mods/[自定义]mod_old.jar.disabled");
    current.original_file_name = Some("mod:old.jar".into());
    assert_eq!(
        super::naming::replacement_path(&current, "mod:new.jar", true).unwrap(),
        "mods/[自定义]mod_new.jar.disabled"
    );
    assert_eq!(
        super::naming::replacement_path(&current, "mod:new.jar", false)
            .unwrap(),
        "mods/mod_new.jar.disabled"
    );
    service.cache().close().await;
}

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
    let context = failed
        .plan
        .issues
        .iter()
        .find(|issue| issue.blocking)
        .unwrap()
        .context
        .as_ref()
        .unwrap();
    assert!(!context.dependency);
    assert_eq!(context.source, Some(SourceId::modrinth()));
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
async fn direct_update_ignores_dependencies_declared_by_both_updated_and_retained_mods()
 {
    let old = release("root0000", "1", 'a');
    let mut new = release("root0000", "2", 'b');
    let absent = release("missing0", "1", 'c');
    let mut parent = release("parent00", "1", 'd');
    requires(&mut parent, &old, ResourceDependencyKind::Required);
    requires(&mut new, &absent, ResourceDependencyKind::Required);
    let (_temp, service) = service(vec![old.clone(), new.clone()]).await;
    let mut request = draft(
        vec![selection("root", &new, Some("entry"))],
        ResourceOperationIntent::Update,
    );
    request.resolve_dependencies = false;
    let built = service
        .build_operation(
            request,
            &target(),
            &[
                installed(&old, "entry", "mods/custom.jar.disabled"),
                installed(&parent, "parent", "mods/parent.jar"),
            ],
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(built.plan.issues.is_empty(), "{:?}", built.plan.issues);
    assert!(built.retained_requirements.is_empty());
    assert!(built.plan.edges.is_empty());
    assert_eq!(built.plan.nodes.len(), 1);
    assert_eq!(built.plan.actions.len(), 1);
    assert_eq!(built.plan.actions[0].kind, ResourceActionKind::Replace);
    assert_eq!(
        built.plan.actions[0].target_relative_path,
        "mods/custom.jar.disabled"
    );
    service.cache().close().await;
}

#[tokio::test]
async fn switching_an_explicit_entry_replaces_its_own_path_but_cannot_overwrite_other_files()
 {
    let old = release("root0000", "1", 'a');
    let mut new = release("root0000", "2", 'b');
    new.artifacts[0].file_name = Some("custom.jar".into());
    let foreign = release("foreign0", "1", 'c');
    let (_temp, service) =
        service(vec![old.clone(), new.clone(), foreign.clone()]).await;
    for disabled in [false, true] {
        let path = if disabled {
            "mods/custom.jar.disabled"
        } else {
            "mods/custom.jar"
        };
        let current = installed(&old, "entry", path);
        let mut request = draft(
            vec![selection("root", &new, Some("entry"))],
            ResourceOperationIntent::SwitchVersion,
        );
        request.resolve_dependencies = false;
        let mut built = service
            .build_operation(
                request.clone(),
                &target(),
                &[current.clone()],
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        assert!(built.plan.issues.is_empty(), "{:?}", built.plan.issues);
        assert_eq!(built.plan.actions.len(), 1);
        assert_eq!(built.plan.actions[0].kind, ResourceActionKind::Replace);
        assert_eq!(
            built.plan.actions[0].current.as_ref().unwrap().entry_id,
            current.entry_id
        );
        assert_eq!(built.plan.actions[0].target_relative_path, path);
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir(directory.path().join("mods")).unwrap();
        std::fs::write(directory.path().join(path), b"current bytes").unwrap();
        super::write_access::review_at_root(&mut built, directory.path())
            .unwrap();
        assert!(built.plan.issues.is_empty(), "{:?}", built.plan.issues);
        let occupied = installed(&foreign, "foreign", path);
        let blocked = service
            .build_operation(
                request.clone(),
                &target(),
                &[current, occupied],
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        assert!(blocked.plan.actions.is_empty());
        assert!(blocked.plan.issues.iter().any(|issue| issue.blocking
            && issue.code == ResourceIssueCode::InvalidPath));
        request.plan.selections[0].resource = ResourceRef::Source {
            project: foreign.reference.project.clone(),
        };
        request.plan.selections[0].release = ReleaseSelection::Artifact {
            reference: foreign.artifacts[0].reference.clone(),
        };
        let rejected = service
            .build_operation(
                request,
                &target(),
                &[installed(&old, "entry", path)],
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        assert!(rejected.plan.issues.iter().any(|issue| issue.blocking
            && issue.code == ResourceIssueCode::ArtifactMismatch));
    }
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
    let progress = |event: ResourceOperationProgress| {
        if let ResourceOperationProgress::Node { node, .. } = &event
            && node.id.as_str() == "root"
        {
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
    assert!(matches!(
        events[0],
        ResourceOperationProgress::Phase {
            phase: ResourceOperationPhase::CollectingConstraints
        }
    ));
    assert!(matches!(
        &events[1],
        ResourceOperationProgress::RoundStarted { plan_id, revision, round: 0 }
            if *plan_id == plan.id && *revision == plan.revision
    ));
    let nodes = node_events(&events);
    assert_eq!(nodes.len(), 3);
    assert_eq!(nodes[0].0.id.as_str(), "root");
    assert!(nodes[0].1);
    assert_eq!(nodes[2].0.state, DependencyNodeState::OptionalCandidate);
    assert!(events.iter().all(|event| match event {
        ResourceOperationProgress::Node {
            plan_id, revision, ..
        } => *plan_id == plan.id && *revision == plan.revision,
        _ => true,
    }));
    assert_eq!(replay(&events), plan.nodes);
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
    let progress = |event: ResourceOperationProgress| {
        if matches!(event, ResourceOperationProgress::Node { .. }) {
            events.lock().unwrap().push(event);
            cancel.cancel();
        }
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

/// The node events in order, with whether each carried metadata.
fn node_events(
    events: &[ResourceOperationProgress],
) -> Vec<(&ResourceDependencyNode, bool)> {
    events
        .iter()
        .filter_map(|event| match event {
            ResourceOperationProgress::Node { node, metadata, .. } => {
                Some((node.as_ref(), metadata.is_some()))
            }
            _ => None,
        })
        .collect()
}

/// Applies node updates and removals in order, as a page showing them would.
fn replay(events: &[ResourceOperationProgress]) -> Vec<ResourceDependencyNode> {
    let mut nodes: Vec<ResourceDependencyNode> = Vec::new();
    for event in events {
        match event {
            ResourceOperationProgress::Node { node, .. } => {
                match nodes.iter_mut().find(|shown| shown.id == node.id) {
                    Some(shown) => *shown = node.as_ref().clone(),
                    None => nodes.push(node.as_ref().clone()),
                }
            }
            ResourceOperationProgress::NodeRemoved { node_id, .. } => {
                nodes.retain(|shown| &shown.id != node_id);
            }
            _ => {}
        }
    }
    nodes
}

#[tokio::test]
async fn a_revised_plan_removes_the_nodes_it_no_longer_has() {
    let mut root = release("root0000", "1", 'a');
    let child = release("required", "1", 'b');
    requires(&mut root, &child, ResourceDependencyKind::Required);
    let (_directory, service) = service(vec![root.clone(), child]).await;
    let first = service
        .build_operation_with_progress(
            draft(
                vec![selection("root", &root, None)],
                ResourceOperationIntent::Install,
            ),
            &target(),
            &[],
            &CancellationToken::new(),
            None,
        )
        .await
        .unwrap();
    assert_eq!(first.plan.nodes.len(), 2);

    // Revising without dependencies drops the child the shown plan had.
    let mut revised = first.clone();
    revised.resolve_dependencies = false;
    revised.install_dependencies = false;
    let events = Mutex::new(vec![]);
    let progress = |event: ResourceOperationProgress| {
        events.lock().unwrap().push(event);
    };
    let plan = service
        .build_operation_with_progress(
            revised,
            &target(),
            &[],
            &CancellationToken::new(),
            Some(&progress),
        )
        .await
        .unwrap()
        .plan;
    let events = events.lock().unwrap();
    let child_id = first
        .plan
        .nodes
        .iter()
        .find(|node| node.id.as_str() != "root")
        .unwrap()
        .id
        .clone();
    assert!(!plan.nodes.iter().any(|node| node.id == child_id));
    assert!(events.iter().any(|event| matches!(
        event,
        ResourceOperationProgress::NodeRemoved { node_id, .. } if *node_id == child_id
    )));
    let mut shown = first.plan.nodes.clone();
    for node in replay(&events) {
        match shown.iter_mut().find(|item| item.id == node.id) {
            Some(item) => *item = node,
            None => shown.push(node),
        }
    }
    shown.retain(|node| plan.nodes.iter().any(|kept| kept.id == node.id));
    assert_eq!(shown, plan.nodes);
    drop(events);
    service.cache().close().await;
}

#[tokio::test]
async fn a_later_round_removes_and_updates_what_an_earlier_round_sent() {
    use crate::resources::plans::OperationReporter;

    let mut root = release("root0000", "1", 'a');
    let child = release("required", "1", 'b');
    requires(&mut root, &child, ResourceDependencyKind::Required);
    let (_directory, service) = service(vec![root.clone(), child]).await;
    let mut plan = service
        .build_operation_with_progress(
            draft(
                vec![selection("root", &root, None)],
                ResourceOperationIntent::Install,
            ),
            &target(),
            &[],
            &CancellationToken::new(),
            None,
        )
        .await
        .unwrap()
        .plan;
    assert_eq!(plan.nodes.len(), 2);
    let events = Mutex::new(vec![]);
    let sink = |event: ResourceOperationProgress| {
        events.lock().unwrap().push(event);
    };
    let mut reporter = OperationReporter::new(Some(&sink), &[]);

    reporter.round_started(&plan, 0);
    for node in plan.nodes.clone() {
        reporter.node(&plan, &node, None);
    }
    reporter.reconcile(&plan);
    assert_eq!(events.lock().unwrap().len(), 3);

    // The next round resolves the root differently and drops the child.
    reporter.round_started(&plan, 1);
    plan.nodes.truncate(1);
    plan.nodes[0].state = DependencyNodeState::Conflict;
    reporter.reconcile(&plan);
    let events = events.into_inner().unwrap();
    assert!(matches!(
        &events[3],
        ResourceOperationProgress::RoundStarted { round: 1, .. }
    ));
    assert!(matches!(
        &events[4],
        ResourceOperationProgress::NodeRemoved { node_id, round: 1, .. }
            if node_id.as_str() != "root"
    ));
    assert!(matches!(
        &events[5],
        ResourceOperationProgress::Node { node, round: 1, .. }
            if node.id.as_str() == "root"
                && node.state == DependencyNodeState::Conflict
    ));
    assert_eq!(events.len(), 6);
    assert_eq!(replay(&events), plan.nodes);
    service.cache().close().await;
}

#[test]
fn progress_events_are_tagged_by_kind() {
    let value = serde_json::to_value(ResourceOperationProgress::NodeRemoved {
        plan_id: PlanId::new("plan".to_owned()).unwrap(),
        revision: 2,
        round: 1,
        node_id: SelectionId::new("node".to_owned()).unwrap(),
    })
    .unwrap();
    assert_eq!(
        value,
        serde_json::json!({
            "kind": "nodeRemoved",
            "planId": "plan",
            "revision": 2,
            "round": 1,
            "nodeId": "node",
        })
    );
    assert_eq!(
        serde_json::to_value(ResourceOperationProgress::Phase {
            phase: ResourceOperationPhase::CollectingConstraints,
        })
        .unwrap(),
        serde_json::json!({ "kind": "phase", "phase": "collectingConstraints" })
    );
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
        let details = plan
            .issues
            .iter()
            .find_map(|issue| issue.conflict.as_ref())
            .unwrap();
        assert_eq!(
            details.reason,
            ResourceConflictReason::IncompatibleResource
        );
        assert_eq!(
            details.related.as_deref(),
            b.artifacts[0].file_name.as_deref()
        );
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
async fn connector_exact_releases_allow_hash_bound_unknown_sizes_and_compatibility()
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
    assert!(!draft.plan.issues.iter().any(|issue| issue.blocking));
    assert_eq!(
        draft.plan.nodes[0].compatibility.game_version,
        CompatibilityState::Unknown
    );
    assert_eq!(
        draft.plan.nodes[0].compatibility.loader,
        CompatibilityState::Unknown
    );
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
        vec![Arc::new(Available(Arc::new(
            CurseForgeProvider::for_tests(transport.clone()),
        )))],
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
async fn disabled_unchanged_file_does_not_trigger_reinstall_and_unknown_is_allowed()
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
    assert!(!plan.issues.iter().any(|issue| issue.blocking));
    assert_eq!(
        plan.nodes[0].compatibility.game_version,
        CompatibilityState::Unknown
    );
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

#[tokio::test]
async fn whole_instance_update_checks_run_concurrently_with_source_limits() {
    struct GatedProvider {
        inner: PlanProvider,
        gate: Arc<Semaphore>,
        slow_first: Arc<Semaphore>,
        active: AtomicUsize,
        maximum: AtomicUsize,
    }
    #[async_trait]
    impl ResourceProvider for GatedProvider {
        fn source(&self) -> ResourceSource {
            self.inner.source()
        }
        fn namespace(&self) -> String {
            self.inner.namespace()
        }
        fn batch_limit(&self, _query: &ProviderQuery) -> usize {
            // Exercise source concurrency independently of batch aggregation.
            1
        }
        async fn fetch_many(
            &self,
            queries: &[ProviderQuery],
        ) -> Vec<ResourceResult<ProviderValue>> {
            let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
            self.maximum.fetch_max(active, Ordering::SeqCst);
            self.gate.acquire().await.unwrap().forget();
            if queries.iter().any(|query| matches!(query, ProviderQuery::Release { release_id, .. } if release_id.as_str() == "p0000000-1")) {
                self.slow_first.acquire().await.unwrap().forget();
            }
            let result = self.inner.fetch_many(queries).await;
            self.active.fetch_sub(1, Ordering::SeqCst);
            result
        }
    }
    let mut releases = Vec::new();
    let mut entries = Vec::new();
    for index in 0..70 {
        let project = format!("p{index:07}");
        let old = release(&project, "1", 'a');
        entries.push(installed(&old, &project, &format!("mods/{project}.jar")));
        releases.extend([old, release(&project, "2", 'b')]);
    }
    let (_temp, cache) = cache().await;
    let provider = Arc::new(GatedProvider {
        inner: PlanProvider {
            source: SourceId::modrinth(),
            releases,
            requests: Mutex::new(vec![]),
        },
        gate: Arc::new(Semaphore::new(0)),
        slow_first: Arc::new(Semaphore::new(0)),
        active: AtomicUsize::new(0),
        maximum: AtomicUsize::new(0),
    });
    let service =
        ResourceService::new(cache.clone(), vec![provider.clone()]).unwrap();
    let task = tokio::spawn(async move {
        super::super::updates::check_entries(
            &service,
            entries,
            super::super::updates::ResourceUpdateCheck {
                entries: vec![],
                releases: Default::default(),
                unresolved: vec![],
                failures: vec![],
                failed_entries: vec![],
            },
            &target(),
            &SourceId::modrinth(),
            |_| {},
            &CancellationToken::new(),
        )
        .await
        .unwrap()
    });
    wait_until(|| provider.active.load(Ordering::SeqCst) == 3).await;
    provider.gate.add_permits(1000);
    wait_until(|| provider.inner.requests.lock().unwrap().iter().any(|query| {
        matches!(query, ProviderQuery::Releases { project_id, .. } if project_id.as_str() == "p0000069")
    })).await;
    assert!(!task.is_finished());
    provider.slow_first.add_permits(1);
    let result = task.await.unwrap();
    assert_eq!(result.entries.len(), 70, "{:?}", result.failures);
    assert_eq!(result.releases.len(), 70);
    assert!(result.failed_entries.is_empty());
    assert_eq!(provider.maximum.load(Ordering::SeqCst), 3);
    cache.close().await;
}

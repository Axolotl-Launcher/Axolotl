use super::*;

#[tokio::test]
async fn retained_parents_share_dependency_lookups_without_a_disk_cache() {
    struct BatchRecorder {
        provider: Arc<PlanProvider>,
        sizes: Mutex<Vec<usize>>,
    }
    #[async_trait]
    impl ResourceProvider for BatchRecorder {
        fn source(&self) -> ResourceSource {
            self.provider.source()
        }
        fn namespace(&self) -> String {
            self.provider.namespace()
        }
        fn batch_limit(&self, query: &ProviderQuery) -> usize {
            self.provider.batch_limit(query)
        }
        async fn fetch_many(
            &self,
            queries: &[ProviderQuery],
        ) -> Vec<ResourceResult<ProviderValue>> {
            self.sizes.lock().unwrap().push(queries.len());
            self.provider.fetch_many(queries).await
        }
    }
    let directory = tempfile::tempdir().unwrap();
    // Opening a directory as SQLite disables the rebuildable disk cache.
    let cache = Arc::new(
        cache::ResourceCache::open(directory.path().to_path_buf()).await,
    );
    let child = release("shared00", "1", 'c');
    let mut parents = Vec::new();
    for index in 0..40 {
        let mut parent = release(&format!("parent{index:02}"), "1", 'a');
        requires(&mut parent, &child, ResourceDependencyKind::Required);
        parents.push(parent);
    }
    let provider = Arc::new(PlanProvider {
        source: SourceId::modrinth(),
        releases: [parents.clone(), vec![child.clone()]].concat(),
        requests: Mutex::new(Vec::new()),
    });
    let recorder = Arc::new(BatchRecorder {
        provider: provider.clone(),
        sizes: Mutex::new(Vec::new()),
    });
    let service = ResourceService::new(cache, vec![recorder.clone()]).unwrap();
    let entries: Vec<_> = parents
        .iter()
        .enumerate()
        .map(|(index, parent)| {
            installed(
                parent,
                &format!("entry-{index}"),
                &format!("mods/parent-{index}.jar"),
            )
        })
        .collect();
    let requirements = service
        .retained_requirements(
            &[],
            &entries,
            &HashMap::new(),
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(requirements.len(), 40);
    assert!(requirements.iter().all(|requirement| requirement.resource
        == ResourceRef::Source {
            project: child.reference.project.clone(),
        }));
    let requests = provider.requests.lock().unwrap();
    assert_eq!(requests.iter().filter(|query| matches!(query, ProviderQuery::Project { id, .. } if id == &child.reference.project.project_id)).count(), 1);
    assert_eq!(
        requests
            .iter()
            .filter(|query| matches!(query, ProviderQuery::Release { .. }))
            .count(),
        41
    );
    drop(requests);
    assert_eq!(*recorder.sizes.lock().unwrap(), vec![40, 1, 1]);
    service.cache().close().await;
}

#[tokio::test]
async fn constraints_from_deeper_parents_reselect_the_shared_dependency_in_both_root_orders()
 {
    let mut a = release("root0000", "1", 'a');
    let mut b = release("parent00", "1", 'b');
    let mut c = release("child000", "1", 'c');
    let versions = vec![
        release("depend00", "1", 'd'),
        release("depend00", "2", 'e'),
        release("depend00", "3", 'f'),
    ];
    requires(&mut b, &c, ResourceDependencyKind::Required);
    for (root, constraint) in [(&mut a, ">=1"), (&mut c, "<3")] {
        root.artifacts[0].dependencies.push(ArtifactDependency {
            target: DependencyTarget::Resource {
                reference: ResourceRef::Source {
                    project: versions[0].reference.project.clone(),
                },
            },
            kind: ResourceDependencyKind::Required,
            version_constraint: Some(constraint.into()),
        });
    }
    for roots in [
        vec![selection("a", &a, None), selection("b", &b, None)],
        vec![selection("b", &b, None), selection("a", &a, None)],
    ] {
        let (_temp, service) = service(
            [vec![a.clone(), b.clone(), c.clone()], versions.clone()].concat(),
        )
        .await;
        let built = service
            .build_operation(
                draft(roots, ResourceOperationIntent::Install),
                &target(),
                &[],
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        assert!(built.plan.issues.is_empty(), "{:?}", built.plan.issues);
        let d = built
            .plan
            .nodes
            .iter()
            .find(|node| {
                node.artifact.as_ref().is_some_and(|artifact| {
                    artifact.reference.release.project
                        == versions[0].reference.project
                })
            })
            .unwrap();
        assert_eq!(
            d.artifact.as_ref().unwrap().reference,
            versions[1].artifacts[0].reference
        );
        assert_eq!(
            built
                .plan
                .edges
                .iter()
                .filter(|edge| edge.child == d.id)
                .count(),
            2
        );
        service.cache().close().await;
    }
}

#[tokio::test]
async fn disabled_and_missing_dependencies_require_review_but_outdated_dependency_is_reused()
 {
    let mut a = release("root0000", "1", 'a');
    let old = release("depend00", "1", 'b');
    let current = release("depend00", "2", 'c');
    requires(&mut a, &current, ResourceDependencyKind::Required);
    let (_directory, service) =
        service(vec![a.clone(), old.clone(), current.clone()]).await;
    let mut disabled =
        installed(&current, "disabled", "mods/custom.jar.disabled");
    disabled.enabled = false;
    let mut missing = installed(&current, "missing", "mods/custom.jar");
    missing.missing = true;
    for entry in [disabled, missing] {
        let plan = service
            .build_operation(
                draft(
                    vec![selection("a", &a, None)],
                    ResourceOperationIntent::Install,
                ),
                &target(),
                &[entry.clone()],
                &CancellationToken::new(),
            )
            .await
            .unwrap()
            .plan;
        assert_eq!(plan.actions.len(), 1);
        assert!(
            plan.issues
                .iter()
                .any(|issue| issue.code == ResourceIssueCode::VersionConflict
                    && issue.blocking)
        );
        assert!(plan.nodes.iter().any(|node| {
            node.installed_entries.contains(&entry.entry_id)
                && matches!(
                    node.state,
                    DependencyNodeState::Disabled
                        | DependencyNodeState::Conflict
                )
        }));
    }
    let outdated = installed(&old, "outdated", "mods/custom.jar");
    let plan = service
        .build_operation(
            draft(
                vec![selection("a", &a, None)],
                ResourceOperationIntent::Install,
            ),
            &target(),
            &[outdated.clone()],
            &CancellationToken::new(),
        )
        .await
        .unwrap()
        .plan;
    assert!(plan.issues.is_empty(), "{:?}", plan.issues);
    assert_eq!(plan.actions.len(), 1);
    assert!(plan.nodes.iter().any(|node| {
        node.installed_entries.contains(&outdated.entry_id)
            && node.state == DependencyNodeState::Satisfied
    }));
    let disabled = installed(&old, "disabled-root", "mods/custom.jar.disabled");
    let plan = service
        .build_operation(
            draft(
                vec![
                    selection("a", &a, None),
                    selection("disabled-root", &current, Some("disabled-root")),
                ],
                ResourceOperationIntent::Update,
            ),
            &target(),
            &[disabled],
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
async fn install_reuses_local_dependency_across_parent_release_requirements() {
    let mut first = release("first000", "1", 'a');
    let mut second = release("second00", "1", 'b');
    let old = release("depend00", "1", 'c');
    let required_by_first = release("depend00", "2", 'd');
    let required_by_second = release("depend00", "3", 'e');
    requires(
        &mut first,
        &required_by_first,
        ResourceDependencyKind::Required,
    );
    requires(
        &mut second,
        &required_by_second,
        ResourceDependencyKind::Required,
    );
    let (_directory, service) = service(vec![
        first.clone(),
        second.clone(),
        old.clone(),
        required_by_first,
        required_by_second,
    ])
    .await;

    let existing = installed(&old, "existing", "mods/dependency.jar");
    let plan = service
        .build_operation(
            draft(
                vec![
                    selection("first", &first, None),
                    selection("second", &second, None),
                ],
                ResourceOperationIntent::Install,
            ),
            &target(),
            &[existing.clone()],
            &CancellationToken::new(),
        )
        .await
        .unwrap()
        .plan;

    assert!(plan.issues.is_empty(), "{:?}", plan.issues);
    assert_eq!(plan.actions.len(), 2);
    assert!(plan.nodes.iter().any(|node| {
        node.state == DependencyNodeState::Satisfied
            && node.installed_entries.contains(&existing.entry_id)
    }));
    service.cache().close().await;
}

#[tokio::test]
async fn install_does_not_revalidate_a_satisfied_dependency_against_retained_parents()
 {
    let mut root = release("root0000", "1", 'a');
    let mut retained_parent = release("parent00", "1", 'b');
    let installed_dependency = release("depend00", "1", 'c');
    let requested_dependency = release("depend00", "2", 'd');
    let retained_requirement = release("depend00", "3", 'e');
    requires(
        &mut root,
        &requested_dependency,
        ResourceDependencyKind::Required,
    );
    requires(
        &mut retained_parent,
        &retained_requirement,
        ResourceDependencyKind::Required,
    );
    let (_directory, service) = service(vec![
        root.clone(),
        retained_parent.clone(),
        installed_dependency.clone(),
        requested_dependency,
        retained_requirement,
    ])
    .await;

    let entries = [
        installed(&retained_parent, "parent", "mods/parent.jar"),
        installed(&installed_dependency, "dependency", "mods/dependency.jar"),
    ];
    let plan = service
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
        .unwrap()
        .plan;

    assert!(plan.issues.is_empty(), "{:?}", plan.issues);
    assert_eq!(plan.actions.len(), 1);
    service.cache().close().await;
}

#[tokio::test]
async fn install_reuses_a_locally_satisfied_dependency_with_unknown_compatibility()
 {
    let mut root = release("root0000", "1", 'a');
    let mut installed_dependency = release("depend00", "1", 'b');
    installed_dependency.compatibility.game_versions.clear();
    installed_dependency.compatibility.loaders.clear();
    let requested_dependency = release("depend00", "2", 'c');
    requires(
        &mut root,
        &requested_dependency,
        ResourceDependencyKind::Required,
    );
    let (_directory, service) = service(vec![
        root.clone(),
        installed_dependency.clone(),
        requested_dependency,
    ])
    .await;

    let existing =
        installed(&installed_dependency, "existing", "mods/dependency.jar");
    let plan = service
        .build_operation(
            draft(
                vec![selection("root", &root, None)],
                ResourceOperationIntent::Install,
            ),
            &target(),
            &[existing.clone()],
            &CancellationToken::new(),
        )
        .await
        .unwrap()
        .plan;

    assert!(plan.issues.is_empty(), "{:?}", plan.issues);
    assert!(plan.nodes.iter().any(|node| {
        node.state == DependencyNodeState::Satisfied
            && node.installed_entries.contains(&existing.entry_id)
    }));
    service.cache().close().await;
}

#[tokio::test]
async fn active_cycles_terminate_and_inactive_optional_closures_stay_unresolved()
 {
    let mut a = release("root0000", "1", 'a');
    let mut b = release("cycle000", "1", 'b');
    let mut c = release("option00", "1", 'c');
    let absent = release("absent00", "1", 'd');
    requires(&mut a, &b, ResourceDependencyKind::Required);
    requires(&mut b, &a, ResourceDependencyKind::Required);
    requires(&mut a, &c, ResourceDependencyKind::Optional);
    requires(&mut c, &absent, ResourceDependencyKind::Required);
    let (_directory, service) = service(vec![a.clone(), b, c]).await;
    let initial = service
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
    assert_eq!(initial.plan.actions.len(), 2);
    assert!(initial.plan.issues.is_empty());
    assert_eq!(initial.plan.nodes.len(), 3);
    let candidate = initial.candidates[0].clone();
    let revised = service
        .revise_operation(
            initial.clone(),
            vec![ResourcePlanDecision::AddOptional {
                node_id: candidate.id,
            }],
            &target(),
            &[],
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(revised.plan.actions.len(), 3);
    assert_eq!(revised.plan.active_roots.len(), 2);
    assert!(!revised.plan.issues.iter().any(|issue| issue.blocking));
    assert!(revised.plan.issues.iter().any(|issue| {
        issue.code == ResourceIssueCode::DependencyAnalysisFailed
    }));
    assert_eq!(initial.plan.active_roots.len(), 1);
    assert_eq!(initial.plan.actions.len(), 2);
    service.cache().close().await;
}

#[tokio::test]
async fn optional_revisions_keep_independent_roots_and_cancellation_leaves_previous_choices()
 {
    let mut a = release("root0000", "1", 'a');
    let mut b = release("option00", "1", 'b');
    let c = release("shared00", "1", 'c');
    requires(&mut a, &b, ResourceDependencyKind::Optional);
    requires(&mut b, &c, ResourceDependencyKind::Required);
    let (_directory, service) = service(vec![a.clone(), b, c.clone()]).await;
    let initial = service
        .build_operation(
            draft(
                vec![
                    selection("a", &a, None),
                    selection("independent-c", &c, None),
                ],
                ResourceOperationIntent::Install,
            ),
            &target(),
            &[],
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    let id = initial.candidates[0].id.clone();
    let canceled = CancellationToken::new();
    canceled.cancel();
    assert!(matches!(
        service
            .revise_operation(
                initial.clone(),
                vec![ResourcePlanDecision::AddOptional {
                    node_id: id.clone()
                }],
                &target(),
                &[],
                &canceled
            )
            .await,
        Err(ResourceError::Canceled)
    ));
    assert_eq!(initial.plan.revision, 1);
    let activated = service
        .revise_operation(
            initial,
            vec![ResourcePlanDecision::AddOptional {
                node_id: id.clone(),
            }],
            &target(),
            &[],
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(activated.plan.revision, 2);
    assert_eq!(activated.plan.actions.len(), 3);
    let removed = service
        .revise_operation(
            activated,
            vec![ResourcePlanDecision::RemoveOptional { node_id: id }],
            &target(),
            &[],
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(removed.plan.actions.len(), 2);
    assert_eq!(removed.plan.revision, 3);
    assert!(
        removed
            .plan
            .active_roots
            .iter()
            .any(|id| id.as_str() == "independent-c")
    );
    assert!(
        removed
            .plan
            .actions
            .iter()
            .any(|action| action.target.reference == c.artifacts[0].reference)
    );
    service.cache().close().await;
}

#[tokio::test]
async fn multiple_parent_constraints_select_a_common_release_and_report_empty_intersections()
 {
    let mut a = release("root0000", "1", 'a');
    let mut b = release("parent00", "1", 'b');
    let versions = vec![
        release("depend00", "1", 'c'),
        release("depend00", "2", 'd'),
        release("depend00", "3", 'e'),
    ];
    for (root, constraint) in [(&mut a, ">=1"), (&mut b, "<3")] {
        root.artifacts[0].dependencies.push(ArtifactDependency {
            target: DependencyTarget::Resource {
                reference: ResourceRef::Source {
                    project: versions[0].reference.project.clone(),
                },
            },
            kind: ResourceDependencyKind::Required,
            version_constraint: Some(constraint.into()),
        });
    }
    let (_directory, first_service) =
        service([vec![a.clone(), b.clone()], versions.clone()].concat()).await;
    let plan = first_service
        .build_operation(
            draft(
                vec![selection("a", &a, None), selection("b", &b, None)],
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
    let dependency = plan
        .nodes
        .iter()
        .find(|node| node.id.as_str().starts_with("dependency:"))
        .unwrap();
    assert_eq!(
        dependency.artifact.as_ref().unwrap().reference,
        versions[1].artifacts[0].reference
    );
    first_service.cache().close().await;
    b.artifacts[0].dependencies[0].version_constraint = Some("<1".into());
    let (_directory, service) =
        service([vec![a.clone(), b.clone()], versions].concat()).await;
    let plan = service
        .build_operation(
            draft(
                vec![selection("a", &a, None), selection("b", &b, None)],
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
async fn fixed_and_project_only_parent_requirements_share_the_fixed_release_in_both_orders()
 {
    let mut a = release("root0000", "1", 'a');
    let mut b = release("parent00", "1", 'b');
    let old = release("depend00", "1", 'c');
    let new = release("depend00", "2", 'd');
    a.artifacts[0].dependencies.push(ArtifactDependency {
        target: DependencyTarget::Resource {
            reference: ResourceRef::Source {
                project: old.reference.project.clone(),
            },
        },
        kind: ResourceDependencyKind::Required,
        version_constraint: None,
    });
    requires(&mut b, &old, ResourceDependencyKind::Required);
    let (_directory, service) =
        service(vec![a.clone(), b.clone(), old.clone(), new]).await;
    for selections in [
        vec![selection("a", &a, None), selection("b", &b, None)],
        vec![selection("b", &b, None), selection("a", &a, None)],
    ] {
        let plan = service
            .build_operation(
                draft(selections, ResourceOperationIntent::Install),
                &target(),
                &[],
                &CancellationToken::new(),
            )
            .await
            .unwrap()
            .plan;
        assert!(plan.issues.is_empty(), "{:?}", plan.issues);
        assert_eq!(plan.actions.len(), 3);
        assert!(plan.actions.iter().any(
            |action| action.target.reference == old.artifacts[0].reference
        ));
    }
    service.cache().close().await;
}

#[tokio::test]
async fn updates_resolve_new_required_dependencies_and_respect_manual_policy() {
    let old = release("root0000", "1", 'a');
    let mut new = release("root0000", "2", 'b');
    let dependency = release("depend00", "1", 'c');
    requires(&mut new, &dependency, ResourceDependencyKind::Required);
    let (_temp, service) =
        service(vec![old.clone(), new.clone(), dependency]).await;
    for automatic in [false, true] {
        let mut request = draft(
            vec![selection("root", &new, Some("root-entry"))],
            ResourceOperationIntent::Update,
        );
        request.install_dependencies = automatic;
        let plan = service
            .build_operation(
                request,
                &target(),
                &[installed(&old, "root-entry", "mods/root.jar")],
                &CancellationToken::new(),
            )
            .await
            .unwrap()
            .plan;
        assert_eq!(plan.actions.len(), if automatic { 2 } else { 1 });
        assert_eq!(
            plan.issues.iter().any(|issue| issue.blocking
                && issue.code == ResourceIssueCode::MissingDependency),
            !automatic
        );
        assert_eq!(plan.edges.len(), 1);
    }
    service.cache().close().await;
}

#[tokio::test]
async fn updates_choose_the_newest_release_allowed_by_retained_parents() {
    let mut parent = release("parent00", "1", 'a');
    let old = release("depend00", "1", 'b');
    let compatible = release("depend00", "2", 'c');
    let latest = release("depend00", "3", 'd');
    parent.artifacts[0].dependencies.push(ArtifactDependency {
        target: DependencyTarget::Resource {
            reference: ResourceRef::Source {
                project: old.reference.project.clone(),
            },
        },
        kind: ResourceDependencyKind::Required,
        version_constraint: Some("<3".into()),
    });
    let (_temp, service) = service(vec![
        parent.clone(),
        old.clone(),
        compatible.clone(),
        latest.clone(),
    ])
    .await;
    let entries = [
        installed(&parent, "parent", "mods/parent.jar"),
        installed(&old, "child", "mods/child.jar"),
    ];
    let mut automatic = selection("child", &old, Some("child"));
    automatic.release = ReleaseSelection::Automatic;
    let plan = service
        .build_operation(
            draft(vec![automatic], ResourceOperationIntent::Update),
            &target(),
            &entries,
            &CancellationToken::new(),
        )
        .await
        .unwrap()
        .plan;
    assert!(plan.issues.is_empty(), "{:?}", plan.issues);
    assert_eq!(
        plan.actions[0].target.reference,
        compatible.artifacts[0].reference
    );
    let explicit = service
        .build_operation(
            draft(
                vec![selection("child", &latest, Some("child"))],
                ResourceOperationIntent::SwitchVersion,
            ),
            &target(),
            &entries,
            &CancellationToken::new(),
        )
        .await
        .unwrap()
        .plan;
    assert!(explicit.issues.iter().any(|issue| issue.blocking
        && issue.code == ResourceIssueCode::VersionConflict));
    service.cache().close().await;
}

#[tokio::test]
async fn exact_retained_requirements_prevent_updates_but_replaced_parents_do_not_pin_old_releases()
 {
    let mut parent = release("parent00", "1", 'a');
    let mut new_parent = release("parent00", "2", 'd');
    let old = release("depend00", "1", 'b');
    let new = release("depend00", "2", 'c');
    requires(&mut parent, &old, ResourceDependencyKind::Required);
    requires(&mut new_parent, &new, ResourceDependencyKind::Required);
    let (_temp, service) = service(vec![
        parent.clone(),
        new_parent.clone(),
        old.clone(),
        new.clone(),
    ])
    .await;
    let entries = [
        installed(&parent, "parent", "mods/parent.jar"),
        installed(&old, "child", "mods/child.jar"),
    ];
    let mut automatic = selection("child", &old, Some("child"));
    automatic.release = ReleaseSelection::Automatic;
    let plan = service
        .build_operation(
            draft(vec![automatic.clone()], ResourceOperationIntent::Update),
            &target(),
            &entries,
            &CancellationToken::new(),
        )
        .await
        .unwrap()
        .plan;
    assert!(plan.actions.is_empty());
    let plan = service
        .build_operation(
            draft(
                vec![
                    selection("parent", &new_parent, Some("parent")),
                    automatic,
                ],
                ResourceOperationIntent::Update,
            ),
            &target(),
            &entries,
            &CancellationToken::new(),
        )
        .await
        .unwrap()
        .plan;
    assert!(plan.issues.is_empty(), "{:?}", plan.issues);
    assert_eq!(plan.actions.len(), 2);
    service.cache().close().await;
}

#[tokio::test]
async fn retained_local_jar_ranges_filter_updates_even_without_online_parent_identity()
 {
    let parent = release("parent00", "1", 'a');
    let old = release("depend00", "1", 'b');
    let mut compatible = release("depend00", "2", 'c');
    let mut latest = release("depend00", "3", 'd');
    compatible.version = "Library Forge build two".into();
    latest.version = "Library Forge build three".into();
    let versions = [
        (old.artifacts[0].reference.clone(), "1"),
        (compatible.artifacts[0].reference.clone(), "2"),
        (latest.artifacts[0].reference.clone(), "3"),
    ]
    .into_iter()
    .map(|(reference, version)| (reference, version.to_string()))
    .collect();
    let (_temp, mut service) =
        service(vec![old.clone(), compatible.clone(), latest]).await;
    Arc::get_mut(&mut service).unwrap().artifact_metadata_reader =
        Arc::new(FixtureMetadataReader(versions));
    let mut local_parent = installed(&parent, "parent", "mods/local.jar");
    local_parent.artifact = None;
    local_parent.known_artifacts.clear();
    let child = installed(&old, "child", "mods/child.jar");
    let mut automatic = selection("child", &old, Some("child"));
    automatic.release = ReleaseSelection::Automatic;
    let mut request = draft(vec![automatic], ResourceOperationIntent::Update);
    for (id, mod_id, dependencies) in [
        (
            "parent",
            "parent",
            serde_json::json!([{"mod_id":"child", "version_range":"<3"}]),
        ),
        ("child", "child", serde_json::json!([])),
    ] {
        request.local_metadata.insert(
            InstalledEntryId::new(id).unwrap(),
            serde_json::from_value(serde_json::json!({
                "mod_id":mod_id,"version":"1","dependencies":dependencies
            }))
            .unwrap(),
        );
    }
    let plan = service
        .build_operation(
            request,
            &target(),
            &[local_parent, child],
            &CancellationToken::new(),
        )
        .await
        .unwrap()
        .plan;
    assert!(plan.issues.is_empty(), "{:?}", plan.issues);
    assert_eq!(
        plan.actions[0].target.reference,
        compatible.artifacts[0].reference
    );
    service.cache().close().await;
}

#[tokio::test]
async fn filename_dependencies_match_unrecognized_files_and_keep_disabled_missing_and_ambiguous_files_blocked()
 {
    let mut parent = release("parent00", "1", 'a');
    let file = release("depend00", "1", 'b');
    parent.artifacts[0].dependencies.push(ArtifactDependency {
        target: DependencyTarget::FileName {
            name: "local.jar".into(),
        },
        kind: ResourceDependencyKind::Required,
        version_constraint: None,
    });
    let (_temp, service) = service(vec![parent.clone()]).await;
    let mut local = installed(&file, "local", "mods/local.jar");
    local.artifact = None;
    local.known_artifacts.clear();
    for scenario in
        ["present", "disabled", "missing", "ambiguous", "other-scope"]
    {
        let mut entry = local.clone();
        match scenario {
            "disabled" => {
                entry.enabled = false;
                entry.relative_path.push_str(".disabled");
            }
            "missing" => entry.missing = true,
            "other-scope" => {
                entry.scope = InstallScope::World {
                    relative_path: "saves/world".into(),
                }
            }
            _ => {}
        }
        let mut entries = vec![entry];
        if scenario == "ambiguous" {
            let mut other = local.clone();
            other.entry_id = InstalledEntryId::new("duplicate").unwrap();
            entries.push(other);
        }
        let plan = service
            .build_operation(
                draft(
                    vec![selection("parent", &parent, None)],
                    ResourceOperationIntent::Install,
                ),
                &target(),
                &entries,
                &CancellationToken::new(),
            )
            .await
            .unwrap()
            .plan;
        assert_eq!(
            plan.issues.iter().any(|issue| issue.blocking),
            scenario != "present",
            "{scenario}: {:?}",
            plan.issues
        );
        if scenario == "present" {
            assert_eq!(plan.actions.len(), 1);
            let node = plan
                .nodes
                .iter()
                .find(|node| node.state == DependencyNodeState::Satisfied)
                .unwrap();
            assert!(node.installed_entries.contains(&local.entry_id));
            assert_eq!(plan.edges.len(), 1);
        }
    }
    service.cache().close().await;
}

#[tokio::test]
async fn install_ignores_filename_dependency_version_for_an_existing_local_mod()
{
    let mut parent = release("parent00", "1", 'a');
    let dependency = release("depend00", "1", 'b');
    parent.artifacts[0].dependencies.push(ArtifactDependency {
        target: DependencyTarget::FileName {
            name: "local.jar".into(),
        },
        kind: ResourceDependencyKind::Required,
        version_constraint: Some(">=2".into()),
    });
    let (_directory, service) = service(vec![parent.clone()]).await;
    let mut local = installed(&dependency, "local", "mods/local.jar");
    local.artifact = None;
    local.known_artifacts.clear();
    let mut request = draft(
        vec![selection("parent", &parent, None)],
        ResourceOperationIntent::Install,
    );
    request.local_metadata.insert(
        local.entry_id.clone(),
        serde_json::from_value(serde_json::json!({
            "mod_id": "local",
            "version": "1",
            "dependencies": []
        }))
        .unwrap(),
    );
    let plan = service
        .build_operation(
            request,
            &target(),
            &[local.clone()],
            &CancellationToken::new(),
        )
        .await
        .unwrap()
        .plan;

    assert!(plan.issues.is_empty(), "{:?}", plan.issues);
    assert_eq!(plan.actions.len(), 1);
    assert!(plan.nodes.iter().any(|node| {
        node.state == DependencyNodeState::Satisfied
            && node.installed_entries.contains(&local.entry_id)
    }));
    service.cache().close().await;
}

#[test]
fn local_mod_version_ranges_use_fabric_and_forge_semantics() {
    for (range, version, expected) in [
        ("~1.2", "1.2.4", true),
        ("~1.2", "1.3.0", false),
        ("^0.2.1", "0.2.9", true),
        ("^0.2.1", "0.3.0", false),
        (">=1.0 <2.0", "1.9.0", true),
        ("[1.0,2.0)", "2.0", false),
        (">=1.0 || ~0.9", "0.9.2", true),
        ("1.2", "1.3.0", false),
        ("1.0.0-beta.2", "1.0.0-beta.2", true),
        ("unknown", "1.0.0", false),
    ] {
        assert_eq!(
            plans::requirements::mod_version_matches(range, version),
            expected,
            "{range}: {version}"
        );
    }
}

#[tokio::test]
async fn updating_a_parent_can_review_and_replace_an_outdated_installed_dependency()
 {
    let old_parent = release("parent00", "1", 'a');
    let mut new_parent = release("parent00", "2", 'b');
    let old_child = release("depend00", "1", 'c');
    let new_child = release("depend00", "2", 'd');
    requires(
        &mut new_parent,
        &new_child,
        ResourceDependencyKind::Required,
    );
    let (_temp, service) = service(vec![
        old_parent.clone(),
        new_parent.clone(),
        old_child.clone(),
        new_child.clone(),
    ])
    .await;
    for automatic in [false, true] {
        let mut request = draft(
            vec![selection("parent", &new_parent, Some("parent"))],
            ResourceOperationIntent::Update,
        );
        request.install_dependencies = automatic;
        let entries = [
            installed(&old_parent, "parent", "mods/parent.jar"),
            installed(&old_child, "child", "mods/custom-child.jar"),
        ];
        let plan = service
            .build_operation(
                request,
                &target(),
                &entries,
                &CancellationToken::new(),
            )
            .await
            .unwrap()
            .plan;
        if automatic {
            assert!(plan.issues.is_empty(), "{:?}", plan.issues);
            assert_eq!(plan.actions.len(), 2);
            let child = plan
                .actions
                .iter()
                .find(|action| action.entry_id.as_str() == "child")
                .unwrap();
            assert_eq!(child.kind, ResourceActionKind::Replace);
            assert_eq!(
                child.target.reference,
                new_child.artifacts[0].reference
            );
            assert_eq!(
                child.current.as_ref().unwrap().relative_path,
                "mods/custom-child.jar"
            );
        } else {
            assert!(plan.issues.iter().any(|issue| issue.blocking));
            assert_eq!(plan.actions.len(), 1);
        }
    }
    service.cache().close().await;
}

struct FixtureMetadataReader(HashMap<SourceArtifactRef, String>);

#[async_trait]
impl plans::artifact_metadata::ArtifactMetadataReader
    for FixtureMetadataReader
{
    async fn read(
        &self,
        artifact: &ResourceArtifact,
        _cancel: &CancellationToken,
    ) -> ResourceResult<Option<crate::mod_metadata::LocalModMetadata>> {
        Ok(self.0.get(&artifact.reference).map(|version| {
            serde_json::from_value(serde_json::json!({
                "mod_id":"child", "version":version, "dependencies":[]
            }))
            .unwrap()
        }))
    }
}

#[tokio::test]
async fn cross_type_dependencies_use_the_child_type_and_world_scope_is_never_invented()
 {
    struct TypedProvider(Vec<ResourceRelease>);
    #[async_trait]
    impl ResourceProvider for TypedProvider {
        fn source(&self) -> ResourceSource {
            ResourceSource {
                id: SourceId::modrinth(),
                types: vec![
                    ResourceType::Mod,
                    ResourceType::ResourcePack,
                    ResourceType::DataPack,
                ],
                capabilities: vec![
                    SourceCapability::Project,
                    SourceCapability::Releases,
                ],
                available: true,
                supported_search_sorts: vec![],
            }
        }
        fn namespace(&self) -> String {
            "typed-dependencies".into()
        }
        fn batch_limit(&self, _: &ProviderQuery) -> usize {
            50
        }
        async fn fetch_many(
            &self,
            queries: &[ProviderQuery],
        ) -> Vec<ResourceResult<ProviderValue>> {
            queries
                .iter()
                .map(|query| match query {
                    ProviderQuery::Project { id, .. } => {
                        let mut value = project_value(
                            SourceId::modrinth(),
                            id.as_str(),
                            id.as_str(),
                        );
                        if let ProviderValue::Project(document) = &mut value {
                            document.metadata.types = vec![match id.as_str() {
                                "pack0000" => ResourceType::ResourcePack,
                                "data0000" => ResourceType::DataPack,
                                _ => ResourceType::Mod,
                            }];
                        }
                        Ok(value)
                    }
                    ProviderQuery::Release { release_id, .. } => self
                        .0
                        .iter()
                        .find(|release| {
                            &release.reference.release_id == release_id
                        })
                        .cloned()
                        .map(|release| {
                            ProviderValue::Release(ReleaseDocument {
                                release,
                                protocol: None,
                                changelog: None,
                            })
                        })
                        .ok_or(ResourceError::Unavailable),
                    _ => Err(ResourceError::Unavailable),
                })
                .collect()
        }
    }
    let mut pack = release("pack0000", "1", 'a');
    let child = release("mod00000", "1", 'b');
    let mut data = release("data0000", "1", 'c');
    pack.artifacts[0].file_name = Some("pack.zip".into());
    data.artifacts[0].file_name = Some("data.zip".into());
    requires(&mut pack, &child, ResourceDependencyKind::Required);
    requires(&mut data, &child, ResourceDependencyKind::Required);
    let (_temp, cache) = cache().await;
    let service = ResourceService::new(
        cache.clone(),
        vec![Arc::new(TypedProvider(vec![
            pack.clone(),
            child.clone(),
            data.clone(),
        ]))],
    )
    .unwrap();
    for (root, kind, scope, path) in [
        (
            &pack,
            ResourceType::ResourcePack,
            InstallScope::Instance,
            "resourcepacks/pack.zip",
        ),
        (
            &data,
            ResourceType::DataPack,
            InstallScope::World {
                relative_path: "saves/world".into(),
            },
            "saves/world/datapacks/data.zip",
        ),
    ] {
        let mut root_selection = selection("root", root, None);
        root_selection.resource_type = kind;
        root_selection.scope = scope;
        let plan = service
            .build_operation(
                draft(vec![root_selection], ResourceOperationIntent::Install),
                &target(),
                &[],
                &CancellationToken::new(),
            )
            .await
            .unwrap()
            .plan;
        assert!(plan.issues.is_empty(), "{:?}", plan.issues);
        assert_eq!(plan.actions.len(), 2);
        assert!(
            plan.actions
                .iter()
                .any(|action| action.target_relative_path == path)
        );
        assert!(
            plan.actions
                .iter()
                .any(|action| action.target_relative_path
                    == "mods/mod00000-1.jar")
        );
        let mod_node = plan
            .nodes
            .iter()
            .find(|node| node.resource_type == Some(ResourceType::Mod))
            .unwrap();
        assert_eq!(mod_node.scope, InstallScope::Instance);
    }
    let mut mod_parent = child.clone();
    requires(&mut mod_parent, &data, ResourceDependencyKind::Required);
    let child_selection = selection("root", &mod_parent, None);
    let reverse = ResourceService::new(
        cache.clone(),
        vec![Arc::new(TypedProvider(vec![
            mod_parent.clone(),
            data.clone(),
        ]))],
    )
    .unwrap();
    cache.clear().await.unwrap();
    let plan = reverse
        .build_operation(
            draft(vec![child_selection], ResourceOperationIntent::Install),
            &target(),
            &[],
            &CancellationToken::new(),
        )
        .await
        .unwrap()
        .plan;
    assert!(
        plan.issues.iter().any(|issue| issue.blocking
            && issue.message.contains("explicit world target")),
        "{:?}",
        plan.issues
    );
    cache.close().await;
}

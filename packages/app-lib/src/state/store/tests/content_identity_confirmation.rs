use super::*;

struct ProjectTypeProvider {
    inner: PlanProvider,
    types: Vec<ResourceType>,
    project_error: Option<ResourceError>,
}

#[async_trait]
impl ResourceProvider for ProjectTypeProvider {
    fn source(&self) -> ResourceSource {
        self.inner.source()
    }

    fn namespace(&self) -> String {
        self.inner.namespace()
    }

    fn batch_limit(&self, query: &ProviderQuery) -> usize {
        self.inner.batch_limit(query)
    }

    async fn fetch_many(
        &self,
        queries: &[ProviderQuery],
    ) -> Vec<ResourceResult<ProviderValue>> {
        self.inner
            .fetch_many(queries)
            .await
            .into_iter()
            .zip(queries)
            .map(|(result, query)| {
                if matches!(query, ProviderQuery::Project { .. })
                    && let Some(error) = &self.project_error
                {
                    return Err(error.clone());
                }
                result.map(|mut value| {
                    if let ProviderValue::Project(document) = &mut value {
                        document.metadata.types = self.types.clone();
                    }
                    value
                })
            })
            .collect()
    }
}

#[tokio::test]
async fn optional_counterpart_metadata_failures_preserve_primary_install_but_not_internal_errors()
 {
    for primary_fails in [false, true] {
        for error in [
            ResourceError::Network { status: Some(503) },
            ResourceError::Unavailable,
            ResourceError::InvalidResponse(
                "counterpart metadata unavailable".into(),
            ),
            ResourceError::Canceled,
            ResourceError::Database("database failed".into()),
            ResourceError::Cache("cache failed".into()),
            ResourceError::InvalidRequest("unexpected contract failure".into()),
            ResourceError::Unsupported("unexpected project operation".into()),
        ] {
            let mut cf = release("123", "1", 'a');
            cf.reference.project.source = SourceId::curseforge();
            cf.reference.release_id = SourceReleaseId::new("101").unwrap();
            cf.artifacts[0].reference.release = cf.reference.clone();
            let mut mr = release("other000", "1", 'a');
            mr.reference.release_id = SourceReleaseId::new("otherv01").unwrap();
            mr.artifacts[0].reference.release = mr.reference.clone();
            let pool = sqlx::sqlite::SqlitePoolOptions::new()
                .max_connections(1)
                .connect("sqlite::memory:")
                .await
                .unwrap();
            sqlx::migrate!().run(&pool).await.unwrap();
            let (_temp, cache) = cache().await;
            let counterpart = Arc::new(ProjectTypeProvider {
                inner: PlanProvider {
                    source: SourceId::modrinth(),
                    releases: vec![mr.clone()],
                    requests: Mutex::new(vec![]),
                },
                types: vec![ResourceType::Mod],
                project_error: (!primary_fails).then(|| error.clone()),
            });
            let service = ResourceService::with_store(
                cache.clone(),
                vec![
                    counterpart.clone(),
                    Arc::new(ProjectTypeProvider {
                        inner: PlanProvider {
                            source: SourceId::curseforge(),
                            releases: vec![cf.clone()],
                            requests: Mutex::new(vec![]),
                        },
                        types: vec![ResourceType::Mod],
                        project_error: primary_fails.then(|| error.clone()),
                    }),
                ],
                crate::state::resources::ResourceStore::new(pool.clone()),
            )
            .unwrap();
            let optional_source_failure = !primary_fails
                && matches!(
                    error,
                    ResourceError::Network { .. }
                        | ResourceError::Unavailable
                        | ResourceError::InvalidResponse(_)
                );
            for resolve_dependencies in [true, false] {
                let mut request = draft(
                    vec![selection("root", &cf, None)],
                    ResourceOperationIntent::Install,
                );
                request.resolve_dependencies = resolve_dependencies;
                let result = service
                    .build_operation(
                        request,
                        &target(),
                        &[],
                        &CancellationToken::new(),
                    )
                    .await;
                if optional_source_failure {
                    let built = result.unwrap();
                    assert!(
                        !built.plan.issues.iter().any(|issue| issue.blocking)
                    );
                    assert_eq!(built.plan.actions.len(), 1);
                    assert_eq!(
                        built.plan.actions[0].target.reference,
                        cf.artifacts[0].reference
                    );
                    let forced = service
                        .revise_operation(
                            built,
                            vec![ResourcePlanDecision::IgnoreDependencies],
                            &target(),
                            &[],
                            &CancellationToken::new(),
                        )
                        .await
                        .unwrap();
                    assert!(
                        !forced.plan.issues.iter().any(|issue| issue.blocking)
                    );
                    assert_eq!(forced.plan.actions.len(), 1);
                    assert_eq!(
                        forced.plan.actions[0].target.reference,
                        cf.artifacts[0].reference
                    );
                } else if primary_fails {
                    if let Ok(built) = result {
                        assert!(
                            built
                                .plan
                                .issues
                                .iter()
                                .any(|issue| issue.blocking),
                            "{error:?}"
                        );
                        assert!(built.plan.actions.is_empty());
                    }
                } else {
                    assert!(
                        matches!(result, Err(actual) if actual == error),
                        "{error:?}"
                    );
                }
            }
            if !primary_fails {
                assert!(counterpart.inner.requests.lock().unwrap().iter().any(|query|
                    matches!(query, ProviderQuery::Project { id, .. } if id == &mr.reference.project.project_id)
                ), "the optional metadata branch must actually be reached");
            }
            assert!(
                service
                    .store()
                    .unwrap()
                    .lookup(&mr.reference.project)
                    .await
                    .unwrap()
                    .is_none()
            );
            cache.close().await;
            pool.close().await;
        }
    }
}

#[tokio::test]
async fn optional_counterpart_type_mismatch_keeps_curseforge_install_and_force_plan()
 {
    let mut cf = release("123", "1", 'a');
    cf.reference.project.source = SourceId::curseforge();
    cf.reference.release_id = SourceReleaseId::new("101").unwrap();
    cf.artifacts[0].reference.release = cf.reference.clone();
    let mut mr = release("other000", "1", 'a');
    mr.reference.release_id = SourceReleaseId::new("otherv01").unwrap();
    mr.artifacts[0].reference.release = mr.reference.clone();
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();
    let (_temp, cache) = cache().await;
    let service = ResourceService::with_store(
        cache.clone(),
        vec![
            Arc::new(ProjectTypeProvider {
                inner: PlanProvider {
                    source: SourceId::modrinth(),
                    releases: vec![mr.clone()],
                    requests: Mutex::new(vec![]),
                },
                types: vec![ResourceType::Shader],
                project_error: None,
            }),
            Arc::new(ProjectTypeProvider {
                inner: PlanProvider {
                    source: SourceId::curseforge(),
                    releases: vec![cf.clone()],
                    requests: Mutex::new(vec![]),
                },
                types: vec![ResourceType::Mod],
                project_error: None,
            }),
        ],
        crate::state::resources::ResourceStore::new(pool.clone()),
    )
    .unwrap();
    let built = service
        .build_operation(
            draft(
                vec![selection("root", &cf, None)],
                ResourceOperationIntent::Install,
            ),
            &target(),
            &[],
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(!built.plan.issues.iter().any(|issue| issue.blocking));
    assert_eq!(built.plan.actions.len(), 1);
    assert_eq!(
        built.plan.actions[0].target.reference,
        cf.artifacts[0].reference
    );
    let forced = service
        .revise_operation(
            built,
            vec![ResourcePlanDecision::IgnoreDependencies],
            &target(),
            &[],
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(!forced.plan.issues.iter().any(|issue| issue.blocking));
    assert_eq!(forced.plan.actions.len(), 1);
    assert_eq!(
        forced.plan.actions[0].target.reference,
        cf.artifacts[0].reference
    );
    let store = service.store().unwrap();
    assert!(store.lookup(&cf.reference.project).await.unwrap().is_some());
    assert!(store.lookup(&mr.reference.project).await.unwrap().is_none());
    cache.close().await;
    pool.close().await;
}

#[tokio::test]
async fn unrelated_installed_counterpart_mismatch_does_not_block_new_or_forced_install()
 {
    let (_temp, state, original, mut old, _) =
        crate::resources::execution::tests::fixture().await;
    old.reference.release.release_id =
        SourceReleaseId::new("oldver01").unwrap();
    sqlx::query("DELETE FROM resource_installed_artifacts WHERE entry_id = ?")
        .bind(original.entry_id.as_str())
        .execute(&state.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE instance_content_provider_refs SET provider_release_id = 'oldver01' WHERE content_entry_id = ?")
        .bind(original.entry_id.as_str()).execute(&state.pool).await.unwrap();
    let mut installed_release = release("root0000", "1", 'a');
    installed_release.reference = old.reference.release.clone();
    installed_release.artifacts = vec![old.clone()];
    let mut counterpart = installed_release.clone();
    counterpart.reference.project.source = SourceId::curseforge();
    counterpart.reference.project.project_id =
        SourceProjectId::new("123").unwrap();
    counterpart.reference.release_id = SourceReleaseId::new("101").unwrap();
    counterpart.artifacts[0].reference.release = counterpart.reference.clone();
    let new = release("newroot0", "1", 'b');
    let (_cache_temp, cache) = cache().await;
    let service = ResourceService::with_store(
        cache.clone(),
        vec![
            Arc::new(PlanProvider {
                source: SourceId::modrinth(),
                releases: vec![installed_release, new.clone()],
                requests: Mutex::new(vec![]),
            }),
            Arc::new(ProjectTypeProvider {
                inner: PlanProvider {
                    source: SourceId::curseforge(),
                    releases: vec![counterpart.clone()],
                    requests: Mutex::new(vec![]),
                },
                types: vec![ResourceType::World],
                project_error: None,
            }),
        ],
        crate::state::resources::ResourceStore::new(state.pool.clone()),
    )
    .unwrap();
    let hydrated = crate::resources::installed::hydrate_selected_with_service(
        &state,
        "instance",
        &CancellationToken::new(),
        None,
        &service,
    )
    .await
    .unwrap();
    assert!(hydrated.failures.is_empty(), "{:?}", hydrated.failures);
    assert_eq!(hydrated.entries.len(), 1);
    assert_eq!(hydrated.entries[0].artifact.as_ref(), Some(&old.reference));
    assert_eq!(
        hydrated.entries[0].known_artifacts,
        vec![old.reference.clone()]
    );
    assert!(
        service
            .store()
            .unwrap()
            .lookup(&counterpart.reference.project)
            .await
            .unwrap()
            .is_none()
    );
    let built = service
        .build_operation(
            draft(
                vec![selection("new", &new, None)],
                ResourceOperationIntent::Install,
            ),
            &target(),
            &hydrated.entries,
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(!built.plan.issues.iter().any(|issue| issue.blocking));
    assert_eq!(built.plan.actions.len(), 1);
    assert_eq!(
        built.plan.actions[0].target.reference,
        new.artifacts[0].reference
    );
    let forced = service
        .revise_operation(
            built,
            vec![ResourcePlanDecision::IgnoreDependencies],
            &target(),
            &hydrated.entries,
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(!forced.plan.issues.iter().any(|issue| issue.blocking));
    assert_eq!(forced.plan.actions.len(), 1);
    assert_eq!(
        forced.plan.actions[0].target.reference,
        new.artifacts[0].reference
    );
    cache.close().await;
    state.pool.close().await;
}

#[tokio::test]
async fn recognition_falls_back_by_type_without_replacing_a_recorded_identity()
{
    let mut cf = release("123", "1", 'a');
    cf.reference.project.source = SourceId::curseforge();
    cf.reference.release_id = SourceReleaseId::new("101").unwrap();
    cf.artifacts[0].reference.release = cf.reference.clone();
    let mr = release("other000", "1", 'a');
    let (_temp, cache) = cache().await;
    let service = ResourceService::new(
        cache.clone(),
        vec![
            Arc::new(ProjectTypeProvider {
                inner: PlanProvider {
                    source: SourceId::curseforge(),
                    releases: vec![cf.clone()],
                    requests: Mutex::new(vec![]),
                },
                types: vec![ResourceType::World],
                project_error: None,
            }),
            Arc::new(ProjectTypeProvider {
                inner: PlanProvider {
                    source: SourceId::modrinth(),
                    releases: vec![mr.clone()],
                    requests: Mutex::new(vec![]),
                },
                types: vec![ResourceType::Mod],
                project_error: None,
            }),
        ],
    )
    .unwrap();
    let matches = vec![cf.artifacts[0].clone(), mr.artifacts[0].clone()];
    let cancel = CancellationToken::new();
    let selected = service
        .select_recognized_artifact(
            &matches,
            None,
            None,
            &SourceId::curseforge(),
            ResourceType::Mod,
            false,
            CachePolicy::UseFresh,
            &cancel,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(selected.0.reference, mr.artifacts[0].reference);
    for (existing, origin) in [
        (Some(&cf.artifacts[0].reference), None),
        (None, Some(SourceId::curseforge())),
    ] {
        assert!(
            service
                .select_recognized_artifact(
                    &matches,
                    existing,
                    origin.as_ref(),
                    &SourceId::modrinth(),
                    ResourceType::Mod,
                    false,
                    CachePolicy::UseFresh,
                    &cancel
                )
                .await
                .unwrap()
                .is_none()
        );
    }
    cache.close().await;
}

struct FailedProjectProvider(PlanProvider, ResourceError);
#[async_trait]
impl ResourceProvider for FailedProjectProvider {
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
        vec![Err(self.1.clone()); queries.len()]
    }
}

#[tokio::test]
async fn recognition_falls_back_only_after_expected_source_failures() {
    let mut cf = release("123", "1", 'a');
    cf.reference.project.source = SourceId::curseforge();
    cf.reference.release_id = SourceReleaseId::new("101").unwrap();
    cf.artifacts[0].reference.release = cf.reference.clone();
    let mr = release("other000", "1", 'a');
    let matches = vec![cf.artifacts[0].clone(), mr.artifacts[0].clone()];
    for error in [
        ResourceError::Unavailable,
        ResourceError::Cache("cache failed".into()),
        ResourceError::InvalidRequest("invalid caller".into()),
        ResourceError::Database("db failed".into()),
        ResourceError::Canceled,
    ] {
        let (_temp, cache) = cache().await;
        let service = ResourceService::new(
            cache.clone(),
            vec![
                Arc::new(FailedProjectProvider(
                    PlanProvider {
                        source: SourceId::curseforge(),
                        releases: vec![],
                        requests: Mutex::new(vec![]),
                    },
                    error.clone(),
                )),
                Arc::new(PlanProvider {
                    source: SourceId::modrinth(),
                    releases: vec![mr.clone()],
                    requests: Mutex::new(vec![]),
                }),
            ],
        )
        .unwrap();
        let result = service
            .select_recognized_artifact(
                &matches,
                None,
                None,
                &SourceId::curseforge(),
                ResourceType::Mod,
                false,
                CachePolicy::UseFresh,
                &CancellationToken::new(),
            )
            .await;
        if error == ResourceError::Unavailable {
            assert_eq!(
                result.unwrap().unwrap().0.reference,
                mr.artifacts[0].reference
            );
        } else {
            assert!(matches!(result, Err(actual) if actual == error));
        }
        cache.close().await;
    }
}

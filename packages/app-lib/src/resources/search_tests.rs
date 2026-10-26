use super::*;
use crate::resources::ipc::*;
use std::collections::HashSet;

#[tokio::test]
async fn provider_qualified_hidden_entries_do_not_disable_a_source_or_hide_another_provider()
 {
    let (_temp, cache) = cache().await;
    let cf = SearchProvider::new(
        SourceId::curseforge(),
        &["CF 1".into(), "CF 2".into()],
    );
    let service = ResourceService::new(cache.clone(), vec![cf]).unwrap();
    let mut query = query(20);
    query.filter = Some(ResourceSearchExpression::Predicate {
        field: "projectId".into(),
        values: vec!["curseforge:1".into(), "modrinth:2".into()],
        excluded: true,
    });
    let page = service
        .search(
            query,
            None,
            RequestId::new("hidden").unwrap(),
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(page.entries.len(), 1);
    assert_eq!(
        page.entries[0].entry.default_project.project_id.as_str(),
        "2"
    );
    assert!(page.source_failures.is_empty());
    assert_ne!(page.total.precision, TotalPrecision::Exact);
    cache.close().await;
}

struct SearchProvider {
    source: SourceId,
    entries: Vec<SourceProjectMetadata>,
    calls: Mutex<Vec<ProviderQuery>>,
    fail: AtomicBool,
    gate: Option<Arc<Semaphore>>,
}

fn metadata(
    source: SourceId,
    index: usize,
    title: &str,
) -> SourceProjectMetadata {
    let id = if source == SourceId::curseforge() {
        (index + 1).to_string()
    } else {
        format!("{index:08}")
    };
    let ProviderValue::Project(document) = project_value(source, &id, title)
    else {
        unreachable!()
    };
    document.metadata
}

impl SearchProvider {
    fn new(source: SourceId, titles: &[String]) -> Arc<Self> {
        Arc::new(Self {
            entries: titles
                .iter()
                .enumerate()
                .map(|(index, title)| metadata(source.clone(), index, title))
                .collect(),
            source,
            calls: Mutex::new(vec![]),
            fail: AtomicBool::new(false),
            gate: None,
        })
    }
}

#[async_trait]
impl ResourceProvider for SearchProvider {
    fn source(&self) -> ResourceSource {
        ResourceSource {
            id: self.source.clone(),
            types: vec![ResourceType::Mod],
            available: true,
            capabilities: vec![
                SourceCapability::Search,
                SourceCapability::Project,
                SourceCapability::Description,
            ],
            supported_search_sorts: vec![
                ResourceSearchSort::Relevance,
                ResourceSearchSort::Created,
                ResourceSearchSort::FirstPublished,
            ],
        }
    }
    fn namespace(&self) -> String {
        format!("search-test:{}", self.source.as_str())
    }
    fn batch_limit(&self, _: &ProviderQuery) -> usize {
        100
    }
    async fn fetch_many(
        &self,
        queries: &[ProviderQuery],
    ) -> Vec<ResourceResult<ProviderValue>> {
        self.calls.lock().unwrap().extend_from_slice(queries);
        if let Some(gate) = &self.gate {
            gate.acquire().await.unwrap().forget();
        }
        queries
            .iter()
            .map(|query| {
                if self.fail.load(Ordering::Acquire) {
                    return Err(ResourceError::Network { status: Some(503) });
                }
                match query {
                    ProviderQuery::Search { query, cursor } => {
                        Ok(ProviderValue::Search(slice_page(
                            self.entries.clone(),
                            offset(cursor)?,
                            query.page_size,
                        )?))
                    }
                    ProviderQuery::Project { id, .. } => self
                        .entries
                        .iter()
                        .find(|entry| entry.project.project_id == *id)
                        .map(|metadata| {
                            ProviderValue::Project(ProjectDocument {
                                metadata: metadata.clone(),
                                description: ResourceText {
                                    source: self.source.clone(),
                                    format: TextFormat::Plain,
                                    body: "description".into(),
                                },
                                protocol: Some(json!({"id": id.as_str()})),
                            })
                        })
                        .ok_or(ResourceError::Unavailable),
                    ProviderQuery::Description { .. } => {
                        Ok(ProviderValue::Description(ResourceText {
                            source: self.source.clone(),
                            format: TextFormat::Plain,
                            body: self.source.as_str().into(),
                        }))
                    }
                    _ => Err(ResourceError::Unsupported("test query".into())),
                }
            })
            .collect()
    }
}

fn query(page_size: u32) -> ResourceSearchQuery {
    ResourceSearchQuery {
        filter: None,
        text: String::new(),
        types: vec![ResourceType::Mod],
        sources: vec![],
        target: ResourceTarget {
            game_versions: vec![],
            loaders: vec![],
            channels: vec![],
        },
        categories: vec![],
        sort: ResourceSearchSort::Relevance,
        page_size,
    }
}
async fn main_pool() -> sqlx::SqlitePool {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();
    pool
}

#[tokio::test]
async fn one_item_relevance_pages_alternate_sources() {
    let (_temp, cache) = cache().await;
    let providers: Vec<Arc<dyn ResourceProvider>> = vec![
        SearchProvider::new(
            SourceId::modrinth(),
            &["MR A".into(), "MR B".into(), "MR C".into()],
        ),
        SearchProvider::new(
            SourceId::curseforge(),
            &["CF A".into(), "CF B".into(), "CF C".into()],
        ),
    ];
    let service = ResourceService::new(cache.clone(), providers).unwrap();
    let mut cursor = None;
    let mut sources = Vec::new();
    for _ in 0..6 {
        let page = service
            .search(
                query(1),
                cursor,
                RequestId::new("alternate").unwrap(),
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        sources.push(page.entries[0].entry.default_project.source.clone());
        cursor = page.cursor;
    }
    assert!(sources.windows(2).all(|pair| pair[0] != pair[1]));
    cache.close().await;
}

#[tokio::test]
async fn ordered_pages_wait_for_a_source_head_after_filtered_batches() {
    let (_temp, cache) = cache().await;
    let mr = SearchProvider::new(
        SourceId::modrinth(),
        &["hidden A".into(), "hidden B".into(), "Newer".into()],
    );
    let cf = SearchProvider::new(SourceId::curseforge(), &["Older".into()]);
    let mut mr = Arc::try_unwrap(mr).ok().unwrap();
    mr.entries[2].created_at = Some("2026-10-14T00:00:00Z".parse().unwrap());
    let mut cf = Arc::try_unwrap(cf).ok().unwrap();
    cf.entries[0].created_at = Some("2020-01-01T00:00:00Z".parse().unwrap());
    let service =
        ResourceService::new(cache.clone(), vec![Arc::new(mr), Arc::new(cf)])
            .unwrap();
    let mut query = query(1);
    query.sort = ResourceSearchSort::Created;
    query.filter = Some(ResourceSearchExpression::Predicate {
        field: "projectId".into(),
        values: vec!["modrinth:00000000".into(), "modrinth:00000001".into()],
        excluded: true,
    });
    let first = service
        .search(
            query.clone(),
            None,
            RequestId::new("sorted1").unwrap(),
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(first.entries.is_empty());
    assert!(first.has_more);
    let second = service
        .search(
            query.clone(),
            first.cursor,
            RequestId::new("sorted2").unwrap(),
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(second.entries[0].projects[0].title, "Newer");
    let third = service
        .search(
            query,
            second.cursor,
            RequestId::new("sorted3").unwrap(),
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(third.entries[0].projects[0].title, "Older");
    cache.close().await;
}

#[tokio::test]
async fn confirmed_distinct_resources_never_merge_by_title() {
    let pool = main_pool().await;
    let (_temp, cache) = cache().await;
    sqlx::query("INSERT INTO resources(id) VALUES ('mr'), ('cf')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO resource_source_projects(source, project_id, resource_id) VALUES ('modrinth', '00000000', 'mr'), ('curseforge', '1', 'cf')").execute(&pool).await.unwrap();
    let service = ResourceService::with_store(
        cache.clone(),
        vec![
            SearchProvider::new(SourceId::modrinth(), &["Same name".into()]),
            SearchProvider::new(SourceId::curseforge(), &["Same name".into()]),
        ],
        crate::state::resources::ResourceStore::new(pool.clone()),
    )
    .unwrap();
    let page = service
        .search(
            query(10),
            None,
            RequestId::new("distinct").unwrap(),
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(page.entries.len(), 2);
    assert!(
        page.entries
            .iter()
            .all(|entry| entry.entry.status == IdentityStatus::Confirmed
                && entry.entry.resource_id.is_some())
    );
    cache.close().await;
    pool.close().await;
}

#[tokio::test]
async fn modrinth_primary_type_exclusions_do_not_reject_server_filtered_mixed_projects()
 {
    let (_temp, cache) = cache().await;
    let provider = SearchProvider::new(
        SourceId::modrinth(),
        &["Pack containing mods".into()],
    );
    let mut provider = Arc::try_unwrap(provider).ok().unwrap();
    provider.entries[0].types = vec![ResourceType::ModPack, ResourceType::Mod];
    let service =
        ResourceService::new(cache.clone(), vec![Arc::new(provider)]).unwrap();
    let mut query = query(10);
    query.filter = Some(ResourceSearchExpression::Predicate {
        field: "type".into(),
        values: vec!["mod".into()],
        excluded: true,
    });
    let page = service
        .search(
            query,
            None,
            RequestId::new("mixed").unwrap(),
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(page.entries.len(), 1);
    cache.close().await;
}

#[tokio::test]
async fn search_keeps_all_provider_remainders_and_has_no_fabricated_mixed_total()
 {
    let (_directory, cache) = cache().await;
    let mr = SearchProvider::new(
        SourceId::modrinth(),
        &(0..31)
            .map(|index| format!("MR {index}"))
            .collect::<Vec<_>>(),
    );
    let cf = SearchProvider::new(
        SourceId::curseforge(),
        &(0..27)
            .map(|index| format!("CF {index}"))
            .collect::<Vec<_>>(),
    );
    let service =
        ResourceService::new(cache.clone(), vec![mr.clone(), cf.clone()])
            .unwrap();
    let cancel = CancellationToken::new();
    let mut cursor = None;
    let mut returned = std::collections::HashSet::new();
    loop {
        let before = [
            mr.calls.lock().unwrap().len(),
            cf.calls.lock().unwrap().len(),
        ];
        let page = service
            .search(
                query(10),
                cursor,
                RequestId::new(uuid::Uuid::new_v4().to_string()).unwrap(),
                &cancel,
            )
            .await
            .unwrap();
        assert_eq!(page.total.precision, TotalPrecision::Unknown);
        assert_eq!(page.total.value, None);
        for entry in page.entries {
            assert!(returned.insert(entry.entry.default_project));
        }
        assert!(mr.calls.lock().unwrap().len() - before[0] <= 2);
        assert!(cf.calls.lock().unwrap().len() - before[1] <= 2);
        if !page.has_more {
            break;
        }
        cursor = page.cursor;
    }
    assert_eq!(returned.len(), 58);
    assert!(
        service
            .search(
                query(11),
                Some("invented".into()),
                RequestId::new("invalid").unwrap(),
                &cancel
            )
            .await
            .is_err()
    );
    cache.close().await;
}

#[tokio::test]
async fn search_candidates_preserve_ambiguity_variants_and_do_not_create_business_ids()
 {
    let pool = main_pool().await;
    let (_directory, cache) = cache().await;
    let mr = SearchProvider::new(
        SourceId::modrinth(),
        &[
            "Shared".into(),
            "Shared".into(),
            "Sodium Lite".into(),
            "Cafe\u{301}".into(),
        ],
    );
    let cf = SearchProvider::new(
        SourceId::curseforge(),
        &["Shared".into(), "Sodium".into(), "Café".into()],
    );
    let service = ResourceService::with_store(
        cache.clone(),
        vec![mr, cf],
        crate::state::resources::ResourceStore::new(pool.clone()),
    )
    .unwrap();
    let page = service
        .search(
            query(20),
            None,
            RequestId::new("candidates").unwrap(),
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(page.entries.len(), 6);
    let accent = page
        .entries
        .iter()
        .find(|entry| entry.projects.len() == 2)
        .unwrap();
    assert_eq!(accent.entry.status, IdentityStatus::Unconfirmed);
    assert_eq!(accent.entry.basis, vec![CandidateBasis::ExactTitle]);
    assert!(
        page.entries
            .iter()
            .filter(|entry| entry.projects[0].title == "Shared")
            .all(|entry| entry.projects.len() == 1)
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM resources")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    let mut changed = query(20);
    changed.text = "another query".into();
    if let Some(cursor) = page.cursor {
        assert!(
            service
                .search(
                    changed,
                    Some(cursor),
                    RequestId::new("changed").unwrap(),
                    &CancellationToken::new()
                )
                .await
                .is_err()
        );
    }
    cache.close().await;
}

#[tokio::test]
async fn search_retains_success_when_one_source_fails_and_cancel_does_not_commit_cursor()
 {
    let (_directory, cache) = cache().await;
    let mr = SearchProvider::new(SourceId::modrinth(), &["Working".into()]);
    let cf = SearchProvider::new(SourceId::curseforge(), &["Failed".into()]);
    cf.fail.store(true, Ordering::Release);
    let service = ResourceService::new(cache.clone(), vec![mr, cf]).unwrap();
    let page = service
        .search(
            query(10),
            None,
            RequestId::new("partial").unwrap(),
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(page.entries.len(), 1);
    assert_eq!(page.source_failures.len(), 1);
    let canceled = CancellationToken::new();
    canceled.cancel();
    assert_eq!(
        service
            .search(
                query(10),
                None,
                RequestId::new("canceled").unwrap(),
                &canceled
            )
            .await
            .unwrap_err(),
        ResourceError::Canceled
    );
    cache.close().await;
}

fn artifact(project: SourceProjectRef) -> ResourceArtifact {
    let release = SourceReleaseRef {
        release_id: SourceReleaseId::new(
            if project.source == SourceId::curseforge() {
                "10"
            } else {
                "release1"
            },
        )
        .unwrap(),
        project,
    };
    ResourceArtifact {
        reference: SourceArtifactRef {
            artifact_id: SourceArtifactId::new("file.jar").unwrap(),
            release,
        },
        file_name: Some("file.jar".into()),
        role: ArtifactRole::Primary,
        available: Some(true),
        integrity: ArtifactIntegrity {
            size: Some(100),
            hashes: vec![ContentHash {
                algorithm: HashAlgorithm::Sha1,
                value: "a".repeat(40),
            }],
        },
        acquisition: ArtifactAcquisition::PublicDownload {
            urls: vec!["https://example.com/file.jar".into()],
        },
        dependencies: vec![],
    }
}

#[tokio::test]
async fn confirmed_project_details_fetch_sources_concurrently_and_keep_preferred_order()
 {
    let pool = main_pool().await;
    let (_directory, cache) = cache().await;
    let mut mr = SearchProvider::new(SourceId::modrinth(), &["MR".into()]);
    let mut cf = SearchProvider::new(SourceId::curseforge(), &["CF".into()]);
    let mr_gate = Arc::new(Semaphore::new(0));
    let cf_gate = Arc::new(Semaphore::new(0));
    Arc::get_mut(&mut mr).unwrap().gate = Some(mr_gate.clone());
    Arc::get_mut(&mut cf).unwrap().gate = Some(cf_gate.clone());
    let left = mr.entries[0].clone();
    let right = cf.entries[0].clone();
    let store = crate::state::resources::ResourceStore::new(pool.clone());
    store
        .confirm_projects(
            &left,
            &right,
            &artifact(left.project.clone()),
            &artifact(right.project.clone()),
        )
        .await
        .unwrap();
    store
        .set_preferred_source(&SourceId::curseforge())
        .await
        .unwrap();
    let service = ResourceService::with_store(
        cache.clone(),
        vec![mr.clone(), cf.clone()],
        store,
    )
    .unwrap();
    let request = tokio::spawn(async move {
        service
            .get(
                &ResourceRef::Source {
                    project: left.project,
                },
                None,
                None,
                &CancellationToken::new(),
            )
            .await
    });
    wait_until(|| {
        !mr.calls.lock().unwrap().is_empty()
            && !cf.calls.lock().unwrap().is_empty()
    })
    .await;
    assert!(!request.is_finished());
    mr_gate.add_permits(1);
    cf_gate.add_permits(1);
    let resource = request.await.unwrap().unwrap();
    assert_eq!(resource.projects.len(), 2);
    assert_eq!(resource.projects[0].project.source, SourceId::curseforge());
    assert_eq!(resource.default_project.source, SourceId::curseforge());
    cache.close().await;
    pool.close().await;
}

#[tokio::test]
async fn confirmed_favorites_preferences_and_descriptions_survive_cache_clear()
{
    let pool = main_pool().await;
    let (_directory, cache) = cache().await;
    let mr = SearchProvider::new(
        SourceId::modrinth(),
        &["Different MR title".into()],
    );
    let cf = SearchProvider::new(
        SourceId::curseforge(),
        &["Different CF title".into()],
    );
    let left = mr.entries[0].clone();
    let right = cf.entries[0].clone();
    let store = crate::state::resources::ResourceStore::new(pool.clone());
    let service = ResourceService::with_store(
        cache.clone(),
        vec![mr.clone(), cf.clone()],
        store.clone(),
    )
    .unwrap();
    service
        .favorite_add(&pool, left.project.clone(), ResourceType::Mod)
        .await
        .unwrap();
    service
        .favorite_add(&pool, right.project.clone(), ResourceType::Mod)
        .await
        .unwrap();
    sqlx::query("UPDATE resource_favorites SET saved_at = 123")
        .execute(&pool)
        .await
        .unwrap();
    store
        .confirm_projects(
            &left,
            &right,
            &artifact(left.project.clone()),
            &artifact(right.project.clone()),
        )
        .await
        .unwrap();
    store
        .set_preferred_source(&SourceId::curseforge())
        .await
        .unwrap();
    cache.clear().await.unwrap();
    let favorites = service.favorites(&pool).await.unwrap();
    assert_eq!(favorites.len(), 1);
    assert_eq!(favorites[0].saved_at, 123);
    assert_eq!(favorites[0].projects.len(), 2);
    assert_eq!(favorites[0].project.source, SourceId::curseforge());
    let reference = ResourceRef::Source {
        project: left.project.clone(),
    };
    let resource = service
        .get(&reference, None, None, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(resource.default_project.source, SourceId::curseforge());
    let description = service
        .description(
            &reference,
            &SourceId::modrinth(),
            None,
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(description.source, SourceId::modrinth());
    assert_eq!(
        service
            .get(&reference, None, None, &CancellationToken::new())
            .await
            .unwrap()
            .default_project,
        resource.default_project
    );
    let page = service
        .search(
            query(10),
            None,
            RequestId::new("confirmed").unwrap(),
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(page.entries.len(), 1);
    assert_eq!(page.entries[0].entry.status, IdentityStatus::Confirmed);
    cf.fail.store(true, Ordering::Release);
    cache.clear().await.unwrap();
    assert_eq!(
        service
            .get(&reference, None, None, &CancellationToken::new())
            .await
            .unwrap()
            .default_project
            .source,
        SourceId::modrinth()
    );
    service
        .favorite_remove(&pool, &right.project)
        .await
        .unwrap();
    assert!(service.favorites(&pool).await.unwrap().is_empty());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM content_favorites")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert!(
        sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&pool)
            .await
            .unwrap()
            .is_empty()
    );
    cache.close().await;
}

#[tokio::test]
async fn connector_favorites_use_stable_ids_without_the_legacy_provider_constraint()
 {
    let pool = main_pool().await;
    let (_directory, cache) = cache().await;
    let store = crate::state::resources::ResourceStore::new(pool.clone());
    let service =
        ResourceService::with_store(cache.clone(), vec![], store).unwrap();
    let project = SourceProjectRef {
        source: SourceId::planet_minecraft(),
        project_id: SourceProjectId::new("connector-project").unwrap(),
    };
    service
        .favorite_add(&pool, project.clone(), ResourceType::Mod)
        .await
        .unwrap();
    assert_eq!(service.favorites(&pool).await.unwrap()[0].project, project);
    service.favorite_remove(&pool, &project).await.unwrap();
    assert!(service.favorites(&pool).await.unwrap().is_empty());
    cache.close().await;
}

#[tokio::test]
async fn long_search_chains_retry_failed_sources_without_losing_their_cursor() {
    let (_directory, cache) = cache().await;
    let mr = SearchProvider::new(
        SourceId::modrinth(),
        &(0..150)
            .map(|index| format!("MR {index}"))
            .collect::<Vec<_>>(),
    );
    let cf =
        SearchProvider::new(SourceId::curseforge(), &["CF recovery".into()]);
    cf.fail.store(true, Ordering::Release);
    let service =
        ResourceService::new(cache.clone(), vec![mr.clone(), cf.clone()])
            .unwrap();
    let mut cursor = None;
    let mut returned = std::collections::HashSet::new();
    let cancel = CancellationToken::new();
    for index in 0..151 {
        if index == 1 {
            cf.fail.store(false, Ordering::Release);
        }
        let page = service
            .search(
                query(1),
                cursor,
                RequestId::new(format!("page-{index}")).unwrap(),
                &cancel,
            )
            .await
            .unwrap();
        if index == 0 {
            assert_eq!(page.source_failures.len(), 1);
        }
        for entry in page.entries {
            assert!(returned.insert(entry.entry.default_project));
        }
        if !page.has_more {
            break;
        }
        cursor = page.cursor;
    }
    assert_eq!(returned.len(), 151);
    assert!(service.searches.len() <= 2);
    cache.close().await;
}

#[tokio::test]
async fn hundred_item_deep_pages_keep_dates_ties_duplicates_and_unequal_source_remainders()
 {
    let (_directory, cache) = cache().await;
    let mut mr = SearchProvider::new(
        SourceId::modrinth(),
        &(0..713)
            .map(|index| format!("MR {index}"))
            .collect::<Vec<_>>(),
    );
    let mut cf = SearchProvider::new(
        SourceId::curseforge(),
        &(0..457)
            .map(|index| format!("CF {index}"))
            .collect::<Vec<_>>(),
    );
    for provider in [&mut mr, &mut cf] {
        let data = Arc::get_mut(provider).unwrap();
        for (index, entry) in data.entries.iter_mut().enumerate() {
            entry.created_at = chrono::DateTime::from_timestamp(
                100000 - (index / 2) as i64,
                0,
            );
            entry.first_published_at = chrono::DateTime::from_timestamp(
                500000 - (index / 2) as i64,
                0,
            );
        }
        let duplicate = data.entries[49].clone();
        data.entries.insert(50, duplicate);
    }
    let service =
        ResourceService::new(cache.clone(), vec![mr.clone(), cf.clone()])
            .unwrap();
    let mut search = query(100);
    search.sort = ResourceSearchSort::Created;
    search.target.game_versions = vec!["1.21.1".into()];
    search.target.loaders = vec!["neoforge".into()];
    let mut cursor = None;
    let mut seen = HashSet::new();
    let mut previous = None;
    let mut page_count = 0;
    loop {
        let before = [
            mr.calls.lock().unwrap().len(),
            cf.calls.lock().unwrap().len(),
        ];
        let page = service
            .search(
                search.clone(),
                cursor,
                RequestId::new(format!("deep-{page_count}")).unwrap(),
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        page_count += 1;
        for result in page.entries {
            let metadata = &result.projects[0];
            assert!(seen.insert(metadata.project.clone()));
            if let Some(previous) = previous {
                assert!(metadata.created_at <= previous);
            }
            previous = Some(metadata.created_at);
            assert_ne!(metadata.created_at, metadata.first_published_at);
        }
        assert!(mr.calls.lock().unwrap().len() - before[0] <= 2);
        assert!(cf.calls.lock().unwrap().len() - before[1] <= 2);
        if !page.has_more {
            break;
        }
        cursor = page.cursor;
        assert!(page_count < 30);
    }
    assert_eq!(seen.len(), 1170);
    assert!(page_count > 10);
    cache.close().await;
}

#[tokio::test]
async fn unavailable_sort_is_reported_without_querying_or_widening_the_source()
{
    let (_directory, cache) = cache().await;
    let mr = SearchProvider::new(SourceId::modrinth(), &["Ignored".into()]);
    let service =
        ResourceService::new(cache.clone(), vec![mr.clone()]).unwrap();
    let mut search = query(100);
    search.sort = ResourceSearchSort::LatestPublished;
    let page = service
        .search(
            search,
            None,
            RequestId::new("unsupported-sort").unwrap(),
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(page.entries.is_empty());
    assert!(!page.has_more);
    assert_eq!(page.source_failures.len(), 1);
    assert!(!page.source_failures[0].retryable);
    assert!(mr.calls.lock().unwrap().is_empty());
    cache.close().await;
}

#[tokio::test]
async fn modrinth_boolean_filters_preserve_live_v3_shapes_and_unsupported_sources_report_difference()
 {
    let transport = FixtureTransport::new(|_, request| {
        assert_eq!(request.path, "/v3/search");
        let filters = &request
            .query
            .iter()
            .find(|(key, _)| key == "new_filters")
            .unwrap()
            .1;
        assert!(filters.contains("environment = \"server_only\""));
        assert!(filters.contains("license = \"MIT\""));
        assert!(filters.contains("categories != \"unofficial\""));
        assert!(!request.query.iter().any(|(key, _)| key == "facets"));
        Ok(
            json!({"hits":[{"project_id":"abcdefgh","slug":"sample","name":"Sample","summary":"Summary","project_types":["mod"],"all_project_types":["mod"],"environment":["server_only"],"loaders":["neoforge"],"game_versions":["1.21.1"],"categories":["technology"],"license":"MIT"}],"page":2,"hits_per_page":1,"total_hits":10}),
        )
    });
    let mut search = query(1);
    search.filter = Some(ResourceSearchExpression::All {
        terms: vec![
            ResourceSearchExpression::Predicate {
                field: "environment".into(),
                values: vec!["server_only".into()],
                excluded: false,
            },
            ResourceSearchExpression::Predicate {
                field: "license".into(),
                values: vec!["MIT".into()],
                excluded: false,
            },
            ResourceSearchExpression::Predicate {
                field: "category".into(),
                values: vec![
                    "modrinth:unofficial".into(),
                    "curseforge:10".into(),
                ],
                excluded: true,
            },
        ],
    });
    let result = ModrinthProvider::new(transport)
        .fetch_many(&[ProviderQuery::Search {
            query: search.clone(),
            cursor: Some("1".into()),
        }])
        .await
        .remove(0)
        .unwrap();
    let ProviderValue::Search(page) = result else {
        panic!("wrong search")
    };
    assert_eq!(page.entries[0].environments, vec!["server_only"]);
    assert_eq!(page.entries[0].types, vec![ResourceType::Mod]);
    assert_eq!(page.entries[0].loaders, vec!["neoforge"]);
    let cf = CurseForgeProvider::for_tests(FixtureTransport::new(|_, _| {
        panic!("unsupported filters must not request a broadened search")
    }));
    assert!(matches!(
        cf.fetch_many(&[ProviderQuery::Search {
            query: search,
            cursor: None
        }])
        .await
        .remove(0),
        Err(ResourceError::Unsupported(_))
    ));
}

#[tokio::test]
async fn cached_only_projection_keeps_expired_metadata_without_requesting_a_source()
 {
    let (_directory, cache) = cache().await;
    let provider = MockProvider::new(100, None);
    let service =
        ResourceService::new(cache.clone(), vec![provider.clone()]).unwrap();
    let query = project_query("abcdefgh");
    let key =
        CacheKey::new("mock-v1".into(), SourceId::modrinth(), &query).unwrap();
    cache
        .put(
            &key,
            &project_value(SourceId::modrinth(), "abcdefgh", "Expired"),
            -1,
            cache.generation().await,
            chrono::Utc::now().timestamp_micros(),
        )
        .await;
    assert!(
        service
            .query(
                &SourceId::modrinth(),
                query.clone(),
                CachePolicy::OnlyCached,
                &CancellationToken::new()
            )
            .await
            .unwrap()
            .from_cache
    );
    assert_eq!(provider.calls(), 0);
    assert!(
        service
            .query(
                &SourceId::modrinth(),
                project_query("missing1"),
                CachePolicy::OnlyCached,
                &CancellationToken::new()
            )
            .await
            .is_err()
    );
    assert_eq!(provider.calls(), 0);
    service
        .query(
            &SourceId::modrinth(),
            query.clone(),
            CachePolicy::UseFresh,
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    service
        .query(
            &SourceId::modrinth(),
            query,
            CachePolicy::ForceRefresh,
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(provider.calls(), 2);
    cache.close().await;
}

#[tokio::test]
async fn untyped_connector_results_report_capability_difference_without_broadening_search()
 {
    let (_directory, cache) = cache().await;
    let transport = FixtureTransport::new(|_, request| {
        assert_eq!(request.path, "/projects");
        Ok(json!([{"id":"untyped", "title":"Unknown project"}]))
    });
    let connector =
        Arc::new(Available(Arc::new(PlanetMinecraftProvider::new(transport))));
    let mr = SearchProvider::new(SourceId::modrinth(), &["Working".into()]);
    let service =
        ResourceService::new(cache.clone(), vec![mr, connector]).unwrap();
    let page = service
        .search(
            query(10),
            None,
            RequestId::new("combined").unwrap(),
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(page.entries.len(), 1);
    assert_eq!(page.source_failures.len(), 1);
    let mut explicit = query(10);
    explicit.sources = vec![SourceId::planet_minecraft()];
    let page = service
        .search(
            explicit,
            None,
            RequestId::new("explicit").unwrap(),
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(page.entries.is_empty());
    assert_eq!(page.source_failures.len(), 1);
    cache.close().await;
}

#[test]
fn compatibility_projection_does_not_cache_credentials_or_signed_download_urls()
{
    let cleaned = protocol_projection(
        json!({"apiKey":"private", "downloadUrl":"https://example.com/file?token=private", "nested":{"headers":{"authorization":"private"},"url":"https://example.com/public"}}),
    );
    assert!(cleaned.get("apiKey").is_none());
    assert!(cleaned["downloadUrl"].is_null());
    assert!(cleaned["nested"].get("headers").is_none());
    assert_eq!(cleaned["nested"]["url"], "https://example.com/public");
}

#[tokio::test]
async fn typed_connector_search_filters_the_complete_collection_before_paging()
{
    let (_directory, cache) = cache().await;
    let transport = FixtureTransport::new(|_, request| {
        assert_eq!(request.path, "/projects");
        Ok(json!([
            {"id":"first-mod", "title":"First mod", "projectType":"mod"},
            {"id":"world", "title":"World", "projectType":"world"},
            {"id":"second-mod", "title":"Second mod", "projectType":"mod"}
        ]))
    });
    let connector =
        Arc::new(Available(Arc::new(PlanetMinecraftProvider::new(transport))));
    let service = ResourceService::new(cache.clone(), vec![connector]).unwrap();
    let mut query = query(1);
    query.sources = vec![SourceId::planet_minecraft()];
    query.filter = Some(ResourceSearchExpression::All {
        terms: vec![
            ResourceSearchExpression::Predicate {
                field: "type".into(),
                values: vec!["mod".into()],
                excluded: false,
            },
            ResourceSearchExpression::Predicate {
                field: "allTypes".into(),
                values: vec!["modpack".into()],
                excluded: true,
            },
        ],
    });
    let token = CancellationToken::new();
    let first = service
        .search(
            query.clone(),
            None,
            RequestId::new("first").unwrap(),
            &token,
        )
        .await
        .unwrap();
    assert_eq!(
        first.entries[0].entry.default_project.project_id.as_str(),
        "first-mod"
    );
    assert!(first.has_more);
    let second = service
        .search(
            query,
            first.cursor,
            RequestId::new("second").unwrap(),
            &token,
        )
        .await
        .unwrap();
    assert_eq!(
        second.entries[0].entry.default_project.project_id.as_str(),
        "second-mod"
    );
    assert!(!second.has_more);
    assert!(
        first.source_failures.is_empty() && second.source_failures.is_empty()
    );
    cache.close().await;
}

#[tokio::test]
async fn v3_project_display_projection_keeps_server_fields_in_the_independent_cache()
 {
    let transport = FixtureTransport::new(|_, request| {
        assert_eq!(request.path, "/v3/projects");
        Ok(
            json!([{"id":"abcdefgh","slug":"sample","name":"Sample","summary":"Summary","body":"Body","project_types":["minecraft_java_server"],"minecraft_java_server":{"address":"example.com"},"apiKey":"secret"}]),
        )
    });
    let (_directory, cache) = cache().await;
    let service = ResourceService::new(
        cache.clone(),
        vec![Arc::new(ModrinthProvider::new(transport.clone()))],
    )
    .unwrap();
    let queries = vec![ProviderQuery::ProjectProtocol {
        locator: "abcdefgh".into(),
        version: "v3".into(),
    }];
    let first = service
        .presentation_many(
            &SourceId::modrinth(),
            queries.clone(),
            &CancellationToken::new(),
        )
        .await
        .remove(0)
        .unwrap();
    let ProviderValue::Project(document) = first.value else {
        panic!("wrong protocol")
    };
    assert_eq!(
        document.protocol.as_ref().unwrap()["minecraft_java_server"]["address"],
        "example.com"
    );
    assert!(document.protocol.as_ref().unwrap().get("apiKey").is_none());
    assert!(
        service
            .presentation_many(
                &SourceId::modrinth(),
                queries,
                &CancellationToken::new()
            )
            .await
            .remove(0)
            .unwrap()
            .from_cache
    );
    assert_eq!(transport.requests.lock().unwrap().len(), 1);
    cache.close().await;
}

#[tokio::test]
async fn eight_character_slug_links_use_locator_and_cached_reads_stay_offline()
{
    let transport = FixtureTransport::new(|_, request| {
        assert_eq!(request.path, "/project/eightchr");
        Ok(
            json!({"id":"abcdefgh","slug":"eightchr","title":"Sample","description":"Summary","body":"Body","project_type":"mod"}),
        )
    });
    let (_directory, cache) = cache().await;
    let service = ResourceService::new(
        cache.clone(),
        vec![Arc::new(ModrinthProvider::new(transport.clone()))],
    )
    .unwrap();
    let cancel = CancellationToken::new();
    let project = service
        .locate(&SourceId::modrinth(), "eightchr".into(), &cancel)
        .await
        .unwrap();
    assert_eq!(project.metadata.project.project_id.as_str(), "abcdefgh");
    assert_eq!(
        service
            .locate_with_policy(
                &SourceId::modrinth(),
                "eightchr".into(),
                &cancel,
                CachePolicy::OnlyCached
            )
            .await
            .unwrap(),
        project
    );
    assert!(
        service
            .locate_with_policy(
                &SourceId::modrinth(),
                "missing8".into(),
                &cancel,
                CachePolicy::OnlyCached
            )
            .await
            .is_err()
    );
    assert_eq!(transport.requests.lock().unwrap().len(), 1);
    cache.close().await;
}

#[tokio::test]
async fn all_sources_skip_archive_when_instance_filters_are_not_applicable() {
    let (_directory, cache) = cache().await;
    let archive =
        Arc::new(McArchiveProvider::new(FixtureTransport::new(|_, _| {
            panic!("inapplicable archive search must not perform a request")
        })));
    let mr = SearchProvider::new(SourceId::modrinth(), &["Compatible".into()]);
    let service =
        ResourceService::new(cache.clone(), vec![mr, archive]).unwrap();
    let mut query = query(10);
    query.target.loaders = vec!["forge".into()];
    let page = service
        .search(
            query.clone(),
            None,
            RequestId::new("all-with-loader").unwrap(),
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(page.entries.len(), 1);
    assert!(page.source_failures.is_empty());
    query.sources = vec![SourceId::mcarchive()];
    let page = service
        .search(
            query,
            None,
            RequestId::new("archive-with-loader").unwrap(),
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(page.entries.is_empty());
    assert_eq!(page.source_failures.len(), 1);
    cache.close().await;
}

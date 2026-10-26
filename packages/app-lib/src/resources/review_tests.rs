use super::*;
use crate::resources::ipc::*;

fn search_query() -> ResourceSearchQuery {
    ResourceSearchQuery {
        text: String::new(),
        sources: vec![],
        types: vec![ResourceType::Mod],
        target: ResourceTarget {
            game_versions: vec![],
            loaders: vec![],
            channels: vec![],
        },
        categories: vec![],
        filter: None,
        sort: ResourceSearchSort::Relevance,
        page_size: 20,
    }
}
fn predicate(
    field: &str,
    values: &[&str],
    excluded: bool,
) -> ResourceSearchExpression {
    ResourceSearchExpression::Predicate {
        field: field.into(),
        values: values.iter().map(|value| (*value).into()).collect(),
        excluded,
    }
}
fn mr_release() -> Value {
    let mut value: Value = serde_json::from_str(fixtures::MR_IRIS).unwrap();
    value["author_id"] = json!("author00");
    value["name"] = json!("Iris fixture");
    value["downloads"] = json!(0);
    value["featured"] = json!(false);
    value
}

#[test]
fn malformed_json_is_a_response_error_and_export_failures_keep_their_category()
{
    let error: crate::Error =
        serde_json::from_str::<Value>("invalid").unwrap_err().into();
    assert!(matches!(
        ResourceError::from(error),
        ResourceError::InvalidResponse(_)
    ));
    let error: crate::Error =
        ResourceError::Network { status: Some(503) }.into();
    assert!(matches!(
        error.raw.as_ref(),
        crate::ErrorKind::ResourceError(ResourceError::Network {
            status: Some(503)
        })
    ));
}

#[tokio::test]
async fn curseforge_filter_only_constraints_reach_the_request_and_conflicts_fail()
 {
    let transport = FixtureTransport::new(|_, request| {
        for (key, value) in [
            ("gameVersion", "1.21.1"),
            ("modLoaderType", "6"),
            ("classId", "6"),
            ("categoryIds", "[10]"),
        ] {
            assert!(
                request.query.contains(&(key.into(), value.into())),
                "{:?}",
                request.query
            );
        }
        Ok(
            json!({"data":[], "pagination":{"index":0,"resultCount":0,"totalCount":0}}),
        )
    });
    let provider = CurseForgeProvider::for_tests(transport.clone());
    let mut query = search_query();
    query.filter = Some(ResourceSearchExpression::All {
        terms: vec![
            predicate("type", &["mod"], false),
            predicate("gameVersion", &["1.21.1"], false),
            predicate("loader", &["neoforge"], false),
            predicate("category", &["curseforge:10"], false),
        ],
    });
    assert!(
        provider
            .fetch_many(&[ProviderQuery::Search {
                query: query.clone(),
                cursor: None
            }])
            .await[0]
            .is_ok()
    );
    query.target.game_versions = vec!["1.20.1".into()];
    assert!(matches!(
        provider
            .fetch_many(&[ProviderQuery::Search {
                query,
                cursor: None
            }])
            .await[0],
        Err(ResourceError::Unsupported(_))
    ));
    assert_eq!(transport.requests.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn modrinth_keeps_category_conjunctions_with_boolean_filters() {
    let transport = FixtureTransport::new(|_, request| {
        let filters = request
            .query
            .iter()
            .find(|(key, _)| key == "new_filters")
            .unwrap()
            .1
            .clone();
        assert!(filters.contains("categories = \"adventure\""));
        assert!(filters.contains("categories = \"technology\""));
        assert!(filters.contains("environment = \"client_only\""));
        Ok(json!({"hits":[],"total_hits":0}))
    });
    let mut query = search_query();
    query.categories = vec!["adventure".into(), "technology".into()];
    query.filter = Some(predicate("environment", &["client_only"], false));
    assert!(
        ModrinthProvider::new(transport)
            .fetch_many(&[ProviderQuery::Search {
                query,
                cursor: None
            }])
            .await[0]
            .is_ok()
    );
}

#[tokio::test]
async fn archive_filter_game_version_reaches_the_request() {
    let transport = FixtureTransport::new(|_, request| {
        assert!(
            request
                .query
                .contains(&("game_version".into(), "1.7.10".into()))
        );
        Ok(json!([]))
    });
    let mut query = search_query();
    query.filter = Some(predicate("gameVersion", &["1.7.10"], false));
    assert!(
        McArchiveProvider::new(transport.clone())
            .fetch_many(&[ProviderQuery::Search {
                query: query.clone(),
                cursor: None
            }])
            .await[0]
            .is_ok()
    );
    query.target.game_versions = vec!["1.6.4".into()];
    assert!(matches!(
        McArchiveProvider::new(transport)
            .fetch_many(&[ProviderQuery::Search {
                query,
                cursor: None
            }])
            .await[0],
        Err(ResourceError::Unsupported(_))
    ));
}

#[tokio::test]
async fn planet_minecraft_multi_type_search_is_a_union() {
    let transport = FixtureTransport::new(|_, _| {
        Ok(
            json!([{ "id":"a", "title":"Mod", "projectType":"mod" }, {"id":"b", "title":"Pack", "projectType":"resourcepack"}]),
        )
    });
    let mut query = search_query();
    query.types.push(ResourceType::ResourcePack);
    let result = PlanetMinecraftProvider::new(transport)
        .fetch_many(&[ProviderQuery::Search {
            query,
            cursor: None,
        }])
        .await
        .remove(0)
        .unwrap();
    let ProviderValue::Search(page) = result else {
        panic!("page");
    };
    assert_eq!(page.entries.len(), 2);
}

#[test]
fn excluding_other_source_categories_is_no_constraint() {
    let filter = providers::search_filters::for_source(
        &predicate("category", &["curseforge:10"], true),
        &SourceId::modrinth(),
    )
    .unwrap();
    assert_eq!(filter, ResourceSearchExpression::All { terms: vec![] });
}

#[tokio::test]
async fn stale_metadata_returns_before_a_blocked_refresh() {
    let (_temp, cache) = cache().await;
    let gate = Arc::new(Semaphore::new(0));
    let provider = MockProvider::new(100, Some(gate.clone()));
    let query = project_query("abcdefgh");
    let key = CacheKey::new(
        "mock-v1".into(),
        SourceId::modrinth(),
        &query.cache_identity(),
    )
    .unwrap();
    cache
        .put(
            &key,
            &project_value(SourceId::modrinth(), "abcdefgh", "Saved"),
            -1,
            cache.generation().await,
            chrono::Utc::now().timestamp_micros(),
        )
        .await;
    let service =
        ResourceService::new(cache.clone(), vec![provider.clone()]).unwrap();
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        service.query(
            &SourceId::modrinth(),
            query,
            CachePolicy::StaleWhileRevalidate,
            &CancellationToken::new(),
        ),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(result.from_cache);
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        while provider.calls() == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    gate.add_permits(1);
    while service.subscriber_count() > 0 {
        tokio::task::yield_now().await;
    }
    cache.close().await;
}

#[tokio::test]
async fn disabled_cache_cleanup_is_a_noop() {
    let temp = tempfile::tempdir().unwrap();
    let parent = temp.path().join("file");
    tokio::fs::write(&parent, b"file").await.unwrap();
    let cache = ResourceCache::open(parent.join("resource-cache.db")).await;
    assert!(!cache.available());
    cache.clear().await.unwrap();
}

#[tokio::test]
async fn stale_background_refresh_preserves_provider_batching() {
    let (_temp, cache) = cache().await;
    let gate = Arc::new(Semaphore::new(0));
    let provider = MockProvider::new(100, Some(gate.clone()));
    let queries: Vec<_> = (0..101)
        .map(|index| project_query(&format!("p{index:07}")))
        .collect();
    for query in &queries {
        let ProviderQuery::Project { id, .. } = query else {
            unreachable!()
        };
        let key = CacheKey::new(
            "mock-v1".into(),
            SourceId::modrinth(),
            &query.cache_identity(),
        )
        .unwrap();
        cache
            .put(
                &key,
                &project_value(SourceId::modrinth(), id.as_str(), "Saved"),
                -1,
                cache.generation().await,
                chrono::Utc::now().timestamp_micros(),
            )
            .await;
    }
    let service =
        ResourceService::new(cache.clone(), vec![provider.clone()]).unwrap();
    let results = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        service.query_many(
            &SourceId::modrinth(),
            queries,
            CachePolicy::StaleWhileRevalidate,
            &CancellationToken::new(),
        ),
    )
    .await
    .unwrap();
    assert!(
        results
            .iter()
            .all(|result| result.as_ref().unwrap().from_cache)
    );
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while provider.calls() < 2 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(provider.calls(), 2);
    gate.add_permits(2);
    while service.subscriber_count() > 0 {
        tokio::task::yield_now().await;
    }
    cache.close().await;
}

#[tokio::test]
async fn raw_protocol_queries_do_not_persist_secrets() {
    let transport = FixtureTransport::new(|_, request| {
        if request.path == "/v3/versions" {
            let mut value = mr_release();
            value["token"] = json!("secret");
            value["private_url"] =
                json!("https://cdn.example/a.jar?access_token=secret");
            Ok(json!([value]))
        } else {
            Ok(
                json!({"hits":[],"total_hits":0,"authorization":"secret", "url":"https://cdn.example/file?signature=secret"}),
            )
        }
    });
    let (_temp, cache) = cache().await;
    let service = ResourceService::new(
        cache.clone(),
        vec![Arc::new(ModrinthProvider::new(transport))],
    )
    .unwrap();
    for query in [
        ProviderQuery::ReleaseProtocol {
            release_id: SourceReleaseId::new("G5dd9TM4").unwrap(),
            version: "v3".into(),
        },
        ProviderQuery::SearchProtocol {
            query: "query=example".into(),
            version: "v3".into(),
        },
    ] {
        let key = CacheKey::new(
            "modrinth-v2-dates".into(),
            SourceId::modrinth(),
            &query.cache_identity(),
        )
        .unwrap();
        let result = service
            .query(
                &SourceId::modrinth(),
                query,
                CachePolicy::UseFresh,
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        assert!(
            !serde_json::to_string(&result.value)
                .unwrap()
                .contains("secret")
        );
        assert!(
            !serde_json::to_string(&cache.get_any(&key).await.unwrap())
                .unwrap()
                .contains("secret")
        );
    }
    cache.close().await;
}

#[tokio::test]
async fn full_lightweight_catalog_includes_late_stable_releases_and_works_offline()
 {
    let fail = Arc::new(AtomicBool::new(false));
    let flag = fail.clone();
    let transport = FixtureTransport::new(move |_, request| {
        assert_eq!(request.path, "/project/YL57xq9U/version");
        assert!(
            request
                .query
                .contains(&("include_changelog".into(), "false".into()))
        );
        if flag.load(Ordering::Acquire) {
            return Err(ResourceError::Network { status: Some(503) });
        }
        Ok(Value::Array(
            (0..80)
                .map(|index| {
                    let mut value = mr_release();
                    value["id"] = json!(format!("v{index:07}"));
                    value["game_versions"] = json!(["1.20.1"]);
                    value["loaders"] =
                        json!([if index < 50 { "forge" } else { "fabric" }]);
                    value["version_type"] =
                        json!(if index < 50 { "alpha" } else { "release" });
                    value
                })
                .collect(),
        ))
    });
    let (_temp, cache) = cache().await;
    let service = ResourceService::new(
        cache.clone(),
        vec![Arc::new(ModrinthProvider::new(transport.clone()))],
    )
    .unwrap();
    let (values, limited): (Vec<crate::state::Version>, bool) =
        legacy_catalog::page_with_service(
            SourceId::modrinth(),
            "YL57xq9U".into(),
            ResourceTarget {
                game_versions: vec!["1.20.1".into()],
                loaders: vec!["fabric".into()],
                channels: vec![ReleaseChannel::Release],
            },
            &service,
        )
        .await
        .unwrap();
    assert_eq!(values.len(), 30);
    assert!(!limited);
    fail.store(true, Ordering::Release);
    let key = CacheKey::new(
        "modrinth-v2-dates".into(),
        SourceId::modrinth(),
        &ProviderQuery::ReleasesProtocol {
            project_id: SourceProjectId::new("YL57xq9U").unwrap(),
        }
        .cache_identity(),
    )
    .unwrap();
    let value = cache.get_any(&key).await.unwrap();
    cache
        .put(
            &key,
            &value,
            -1,
            cache.generation().await,
            chrono::Utc::now().timestamp_micros() + 1,
        )
        .await;
    let (cached, _): (Vec<crate::state::Version>, bool) =
        legacy_catalog::page_with_service(
            SourceId::modrinth(),
            "YL57xq9U".into(),
            ResourceTarget {
                game_versions: vec!["1.20.1".into()],
                loaders: vec!["fabric".into()],
                channels: vec![ReleaseChannel::Release],
            },
            &service,
        )
        .await
        .unwrap();
    assert_eq!(cached.len(), 30);
    cache.close().await;
}

#[tokio::test]
async fn legacy_pack_indexes_survive_expiry_and_are_validated_during_import() {
    let main = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!().run(&main).await.unwrap();
    let (_temp, cache) = cache().await;
    let valid = json!({"version_id":"release1", "file_hashes":["a".repeat(40)], "project_ids":["project1"], "token":"discard me"});
    let invalid = json!({"version_id":"release2", "file_hashes":["not a hash"], "project_ids":[]});
    for (id, value) in [("release1", valid), ("release2", invalid)] {
        sqlx::query("INSERT INTO cache(id, data_type, data, expires) VALUES (?, 'modpack_files', ?, 0)").bind(id).bind(value.to_string()).execute(&main).await.unwrap();
    }
    cache.import_legacy(&main).await.unwrap();
    let index = packs::index(&cache, "release1").await.unwrap();
    assert_eq!(index.project_ids, vec!["project1"]);
    assert!(packs::index(&cache, "release2").await.is_none());
    let key = packs::index_key("release1").unwrap();
    assert!(
        !serde_json::to_string(&cache.get_any(&key).await.unwrap())
            .unwrap()
            .contains("discard me")
    );
    cache.close().await;
    main.close().await;
}

#[tokio::test]
async fn upgrade_catalog_reuses_the_previous_normalized_cache_when_protocol_list_is_absent()
 {
    let (_temp, cache) = cache().await;
    let document = providers::legacy::release(mr_release()).unwrap();
    let query = ProviderQuery::Releases {
        project_id: document.release.reference.project.project_id.clone(),
        slug: None,
        target: target(),
        cursor: None,
        page_size: 0,
    };
    let key = CacheKey::new(
        "modrinth-v2-dates".into(),
        SourceId::modrinth(),
        &query.cache_identity(),
    )
    .unwrap();
    cache
        .put(
            &key,
            &ProviderValue::Releases(SourcePage {
                entries: vec![document.release.clone()],
                cursor: None,
                total: ResourceTotal {
                    value: Some(1),
                    precision: TotalPrecision::Exact,
                },
                window_exhausted: false,
            }),
            -1,
            cache.generation().await,
            chrono::Utc::now().timestamp_micros(),
        )
        .await;
    let exact = ProviderQuery::Release {
        project_id: Some(document.release.reference.project.project_id.clone()),
        release_id: document.release.reference.release_id.clone(),
        slug: None,
    };
    let key = CacheKey::new(
        "modrinth-v2-dates".into(),
        SourceId::modrinth(),
        &exact.cache_identity(),
    )
    .unwrap();
    cache
        .put(
            &key,
            &ProviderValue::Release(document),
            -1,
            cache.generation().await,
            chrono::Utc::now().timestamp_micros(),
        )
        .await;
    let transport = FixtureTransport::new(|_, _| {
        Err(ResourceError::Network { status: Some(503) })
    });
    let service = ResourceService::new(
        cache.clone(),
        vec![Arc::new(ModrinthProvider::new(transport.clone()))],
    )
    .unwrap();
    let (values, limited): (Vec<crate::state::Version>, bool) =
        legacy_catalog::page_with_service(
            SourceId::modrinth(),
            "YL57xq9U".into(),
            target(),
            &service,
        )
        .await
        .unwrap();
    assert_eq!(values.len(), 1);
    assert!(!limited);
    assert_eq!(transport.requests.lock().unwrap().len(), 1);
    cache.close().await;
}

#[tokio::test]
async fn corruption_in_the_migration_table_rebuilds_the_cache() {
    use sqlx::Row;
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("resource-cache.db");
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(&path)
                .create_if_missing(true),
        )
        .await
        .unwrap();
    sqlx::query("CREATE TABLE _sqlx_migrations(version INTEGER PRIMARY KEY, description TEXT, installed_on TEXT, success INTEGER, checksum BLOB, execution_time INTEGER)").execute(&pool).await.unwrap();
    let row = sqlx::query(
        "SELECT rootpage FROM sqlite_master WHERE name = '_sqlx_migrations'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let root: i64 = row.get("rootpage");
    let page_size: i64 = sqlx::query_scalar("PRAGMA page_size")
        .fetch_one(&pool)
        .await
        .unwrap();
    pool.close().await;
    let mut bytes = tokio::fs::read(&path).await.unwrap();
    bytes[((root - 1) * page_size) as usize] = 0;
    tokio::fs::write(&path, bytes).await.unwrap();
    let rebuilt = ResourceCache::open(path).await;
    assert!(rebuilt.available());
    assert_eq!(rebuilt.info().await.unwrap().entry_count, 0);
    rebuilt.close().await;
}

#[tokio::test]
async fn catalog_cursor_keeps_its_first_source_and_rejects_changed_targets() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();
    sqlx::query("INSERT INTO resources(id) VALUES ('shared')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO resource_source_projects(source, project_id, resource_id) VALUES ('modrinth', 'YL57xq9U', 'shared'), ('curseforge', '1', 'shared')").execute(&pool).await.unwrap();
    let transport = FixtureTransport::new(|_, request| {
        if request.path == "/projects" {
            return Ok(
                json!([{"id":"YL57xq9U", "slug":"iris", "title":"Iris", "project_type":"mod", "description":"Iris"}]),
            );
        }
        assert_eq!(request.path, "/project/YL57xq9U/version");
        Ok(Value::Array(
            (0..60)
                .map(|index| {
                    let mut value = mr_release();
                    value["id"] = json!(format!("v{index:07}"));
                    value
                })
                .collect(),
        ))
    });
    let (_temp, cache) = cache().await;
    let mut cf = Arc::try_unwrap(MockProvider::new(50, None)).ok().unwrap();
    cf.source = SourceId::curseforge();
    let service = ResourceService::with_store(
        cache.clone(),
        vec![Arc::new(ModrinthProvider::new(transport)), Arc::new(cf)],
        crate::state::resources::ResourceStore::new(pool.clone()),
    )
    .unwrap();
    let reference = ResourceRef::Resource {
        id: ResourceId::new("shared").unwrap(),
    };
    let target = ResourceTarget {
        game_versions: vec![],
        loaders: vec![],
        channels: vec![],
    };
    let first = service
        .releases(
            &reference,
            target.clone(),
            None,
            RequestId::new("first").unwrap(),
            None,
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(first.entries.len(), 50);
    service
        .store()
        .unwrap()
        .set_preferred_source(&SourceId::curseforge())
        .await
        .unwrap();
    let token = first.cursor.unwrap();
    let second = service
        .releases(
            &reference,
            target.clone(),
            Some(token.clone()),
            RequestId::new("second").unwrap(),
            None,
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(second.entries.len(), 10);
    assert!(second.entries.iter().all(
        |release| release.reference.project.source == SourceId::modrinth()
    ));
    let mut changed = target;
    changed.game_versions = vec!["1.21.1".into()];
    assert!(matches!(
        service
            .releases(
                &reference,
                changed,
                Some(token),
                RequestId::new("changed").unwrap(),
                None,
                &CancellationToken::new()
            )
            .await,
        Err(ResourceError::CursorExpired)
    ));
    cache.close().await;
    pool.close().await;
}

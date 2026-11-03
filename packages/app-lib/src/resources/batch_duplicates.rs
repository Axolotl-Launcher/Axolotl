use super::*;

fn project(id: u32) -> Value {
    json!({"id": id, "name": format!("Project {id}"), "slug": format!("project-{id}"), "classId": 6})
}

fn file(id: u32, project: u32) -> Value {
    let mut file: Value = serde_json::from_str(fixtures::CF_IRIS).unwrap();
    file["id"] = json!(id);
    file["modId"] = json!(project);
    file
}

fn release_query(id: u32, project: Option<u32>) -> ProviderQuery {
    ProviderQuery::Release {
        project_id: project
            .map(|id| SourceProjectId::new(id.to_string()).unwrap()),
        release_id: SourceReleaseId::new(id.to_string()).unwrap(),
        slug: None,
    }
}

fn project_query(id: u32) -> ProviderQuery {
    ProviderQuery::Project {
        id: SourceProjectId::new(id.to_string()).unwrap(),
        slug: None,
    }
}

#[tokio::test]
async fn release_ids_are_unique_on_the_wire_and_fan_out_in_the_original_order()
{
    let transport = FixtureTransport::new(|_, request| {
        assert_eq!(request.path, "/v1/mods/files");
        assert_eq!(
            request.body.as_ref().unwrap()["fileIds"],
            json!([100, 200, 300])
        );
        // Missing and out-of-order rows must not change result positions.
        Ok(json!({"data": [file(200, 20), file(100, 10), file(100, 10)]}))
    });
    let results = CurseForgeProvider::for_tests(transport.clone())
        .fetch_many(&[
            release_query(100, None),
            release_query(200, Some(20)),
            release_query(100, Some(10)),
            release_query(300, None),
            release_query(100, Some(99)),
            release_query(200, None),
        ])
        .await;
    assert_eq!(results.len(), 6);
    for (index, id) in [(0, "100"), (1, "200"), (2, "100"), (5, "200")] {
        let ProviderValue::Release(document) = results[index].as_ref().unwrap()
        else {
            panic!("missing release");
        };
        assert_eq!(document.release.reference.release_id.as_str(), id);
    }
    assert_eq!(results[3], Ok(ProviderValue::NotFound));
    assert!(
        matches!(&results[4], Err(ResourceError::InvalidResponse(message)) if message.contains("another project"))
    );
    assert_eq!(transport.requests.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn project_ids_are_unique_and_identical_response_duplicates_are_idempotent()
 {
    let transport = FixtureTransport::new(|_, request| {
        assert_eq!(request.path, "/v1/mods");
        assert_eq!(
            request.body.as_ref().unwrap()["modIds"],
            json!([10, 20, 30])
        );
        Ok(json!({"data": [project(20), project(10), project(10)]}))
    });
    let results = CurseForgeProvider::for_tests(transport.clone())
        .fetch_many(&[
            project_query(10),
            project_query(20),
            project_query(10),
            project_query(30),
        ])
        .await;
    assert_eq!(results.len(), 4);
    assert_eq!(results[0], results[2]);
    for (index, id) in [(0, "10"), (1, "20")] {
        let ProviderValue::Project(document) = results[index].as_ref().unwrap()
        else {
            panic!("missing project");
        };
        assert_eq!(document.metadata.project.project_id.as_str(), id);
    }
    assert_eq!(results[3], Ok(ProviderValue::NotFound));
}

#[tokio::test]
async fn conflicting_or_unrequested_batch_records_remain_errors() {
    for project_batch in [false, true] {
        for scenario in ["conflict", "unexpected", "invalid-hash"] {
            if project_batch && scenario == "invalid-hash" {
                continue;
            }
            let transport = FixtureTransport::new(move |_, _| {
                let first = if project_batch {
                    project(10)
                } else {
                    file(100, 10)
                };
                let mut second = first.clone();
                match scenario {
                    "unexpected" => second["id"] = json!(999),
                    "invalid-hash" => {
                        second["hashes"][0]["value"] = json!("bad")
                    }
                    _ if project_batch => {
                        second["name"] = json!("conflicting title")
                    }
                    _ => second["modId"] = json!(99),
                }
                Ok(json!({"data": [first, second]}))
            });
            let query = if project_batch {
                project_query(10)
            } else {
                release_query(100, Some(10))
            };
            let results = CurseForgeProvider::for_tests(transport)
                .fetch_many(&[query.clone(), query])
                .await;
            assert_eq!(results.len(), 2);
            assert!(results.iter().all(|result| matches!(
                result,
                Err(ResourceError::InvalidResponse(_))
            )));
        }
    }
}

fn update_query(hash: char, game: &str) -> ProviderQuery {
    ProviderQuery::UpdateCandidates {
        project_id: SourceProjectId::new("10").unwrap(),
        sha1: hash.to_string().repeat(40),
        target: ipc::ResourceTarget {
            game_versions: vec![game.into()],
            loaders: vec!["fabric".into()],
            channels: vec![ReleaseChannel::Release],
        },
    }
}

#[tokio::test]
async fn repeated_update_projects_keep_each_hash_and_target_result() {
    let transport = FixtureTransport::new(|_, request| {
        match request.path.as_str() {
            "/v1/mods" => {
                assert_eq!(
                    request.body.as_ref().unwrap()["modIds"],
                    json!([10])
                );
                let mut row = project(10);
                row["latestFilesIndexes"] = json!([
                    {"gameVersion":"1.20.1","modLoader":4,"fileId":100,"releaseType":1},
                    {"gameVersion":"1.20.1","modLoader":4,"fileId":100,"releaseType":1},
                    {"gameVersion":"1.21.1","modLoader":4,"fileId":200,"releaseType":1}
                ]);
                Ok(json!({"data": [row.clone(), row]}))
            }
            "/v1/mods/files" => {
                assert_eq!(
                    request.body.as_ref().unwrap()["fileIds"],
                    json!([100, 200])
                );
                Ok(
                    json!({"data": [file(200, 10), file(100, 10), file(100, 10)]}),
                )
            }
            _ => panic!("unexpected endpoint"),
        }
    });
    let results = CurseForgeProvider::for_tests(transport.clone())
        .fetch_many(&[
            update_query('a', "1.20.1"),
            update_query('b', "1.21.1"),
            update_query('c', "1.20.1"),
        ])
        .await;
    assert_eq!(results.len(), 3);
    for (result, id) in results.iter().zip(["100", "200", "100"]) {
        let ProviderValue::UpdateCandidates(documents) =
            result.as_ref().unwrap()
        else {
            panic!("wrong response");
        };
        assert_eq!(documents.len(), 1);
        assert_eq!(documents[0].release.reference.release_id.as_str(), id);
    }
    assert_eq!(transport.requests.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn repeated_fingerprints_share_one_lookup_for_identification_and_export()
{
    let transport = FixtureTransport::new(|_, request| {
        assert_eq!(request.path, "/v1/fingerprints/432");
        assert_eq!(
            request.body.as_ref().unwrap()["fingerprints"],
            json!([123])
        );
        let mut row = file(100, 10);
        row["fileFingerprint"] = json!(123);
        Ok(json!({"data": {"exactMatches": [{"id":10,"file": row}]}}))
    });
    let identity = ArtifactIntegrity {
        size: Some(2684113),
        hashes: vec![],
    };
    let results = CurseForgeProvider::for_tests(transport)
        .fetch_many(&[
            ProviderQuery::Identify {
                integrity: identity.clone(),
                fingerprint: Some(123),
            },
            ProviderQuery::IdentifyExport {
                integrity: identity,
                fingerprint: Some(123),
            },
        ])
        .await;
    assert_eq!(results.len(), 2);
    assert!(
        matches!(&results[0], Ok(ProviderValue::Matches(values)) if values.len() == 1)
    );
    assert!(
        matches!(&results[1], Ok(ProviderValue::Identified(values)) if values.len() == 1)
    );
}

#[tokio::test]
async fn service_coalesces_alias_queries_and_failed_batches_can_be_retried() {
    let fail = Arc::new(AtomicBool::new(true));
    let setting = fail.clone();
    let transport = FixtureTransport::new(move |_, request| {
        assert_eq!(request.path, "/v1/mods/files");
        assert_eq!(request.body.as_ref().unwrap()["fileIds"], json!([100]));
        let row = file(100, 10);
        let mut duplicate = row.clone();
        if setting.load(Ordering::Acquire) {
            duplicate["modId"] = json!(99);
        }
        Ok(json!({"data": [row, duplicate]}))
    });
    let (_temp, cache) = cache().await;
    let service = ResourceService::new(
        cache.clone(),
        vec![Arc::new(Available(Arc::new(
            CurseForgeProvider::for_tests(transport.clone()),
        )))],
    )
    .unwrap();
    let queries = vec![
        release_query(100, None),
        release_query(100, Some(10)),
        release_query(100, None),
    ];
    let cancel = CancellationToken::new();
    let failed = service
        .query_many(
            &SourceId::curseforge(),
            queries.clone(),
            CachePolicy::UseFresh,
            &cancel,
        )
        .await;
    assert_eq!(failed.len(), 3);
    assert!(failed.iter().all(Result::is_err));
    assert_eq!(transport.requests.lock().unwrap().len(), 1);
    fail.store(false, Ordering::Release);
    let source = SourceId::curseforge();
    let (first, second) = tokio::join!(
        service.query_many(
            &source,
            queries.clone(),
            CachePolicy::UseFresh,
            &cancel
        ),
        service.query_many(&source, queries, CachePolicy::UseFresh, &cancel),
    );
    assert!(first.iter().chain(&second).all(Result::is_ok));
    assert_eq!(first.len(), 3);
    assert_eq!(second.len(), 3);
    assert_eq!(transport.requests.lock().unwrap().len(), 2);
    let cached = service
        .query(
            &SourceId::curseforge(),
            release_query(100, Some(10)),
            CachePolicy::UseFresh,
            &cancel,
        )
        .await
        .unwrap();
    assert!(cached.from_cache);
    assert_eq!(transport.requests.lock().unwrap().len(), 2);
    cache.close().await;
}

#[tokio::test]
async fn an_empty_batch_does_not_contact_the_source() {
    let transport =
        FixtureTransport::new(|_, _| panic!("empty batch made a request"));
    assert!(
        CurseForgeProvider::for_tests(transport.clone())
            .fetch_many(&[])
            .await
            .is_empty()
    );
    assert!(transport.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn service_keeps_wire_batches_bounded_while_fanning_out_many_release_aliases()
 {
    let transport = FixtureTransport::new(|_, request| {
        assert_eq!(request.path, "/v1/mods/files");
        let ids = request.body.as_ref().unwrap()["fileIds"]
            .as_array()
            .unwrap();
        assert!(ids.len() <= 50);
        assert_eq!(
            ids.iter()
                .map(|id| id.as_u64().unwrap())
                .collect::<std::collections::HashSet<_>>()
                .len(),
            ids.len()
        );
        Ok(
            json!({"data": ids.iter().rev().map(|id| file(id.as_u64().unwrap() as u32, 10)).collect::<Vec<_>>()}),
        )
    });
    let (_temp, cache) = cache().await;
    let service = ResourceService::new(
        cache.clone(),
        vec![Arc::new(Available(Arc::new(
            CurseForgeProvider::for_tests(transport.clone()),
        )))],
    )
    .unwrap();
    let queries = (100..160)
        .flat_map(|id| {
            [
                release_query(id, None),
                release_query(id, Some(10)),
                release_query(id, None),
            ]
        })
        .collect();
    let results = service
        .query_many(
            &SourceId::curseforge(),
            queries,
            CachePolicy::ForceRefresh,
            &CancellationToken::new(),
        )
        .await;
    assert_eq!(results.len(), 180);
    for (index, result) in results.iter().enumerate() {
        let ProviderValue::Release(document) = &result.as_ref().unwrap().value
        else {
            panic!("missing release");
        };
        assert_eq!(
            document.release.reference.release_id.as_str(),
            (100 + index / 3).to_string()
        );
    }
    assert_eq!(transport.requests.lock().unwrap().len(), 3);
    cache.close().await;
}

#[tokio::test]
async fn update_batches_reject_conflicting_duplicate_projects_and_files() {
    for conflicting_project in [false, true] {
        let transport = FixtureTransport::new(move |_, request| match request
            .path
            .as_str()
        {
            "/v1/mods" => {
                let mut first = project(10);
                first["latestFilesIndexes"] = json!([{"gameVersion":"1.20.1","modLoader":4,"fileId":100,"releaseType":1}]);
                let mut second = first.clone();
                if conflicting_project {
                    second["slug"] = json!("another-project");
                }
                Ok(json!({"data": [first, second]}))
            }
            "/v1/mods/files" => {
                let first = file(100, 10);
                let mut second = first.clone();
                second["fileLength"] = json!(1);
                Ok(json!({"data": [first, second]}))
            }
            _ => panic!("unexpected endpoint"),
        });
        let results = CurseForgeProvider::for_tests(transport)
            .fetch_many(&[
                update_query('a', "1.20.1"),
                update_query('b', "1.20.1"),
            ])
            .await;
        assert!(results.iter().all(|result| matches!(result, Err(ResourceError::InvalidResponse(message)) if message.contains("conflicting"))));
    }
}

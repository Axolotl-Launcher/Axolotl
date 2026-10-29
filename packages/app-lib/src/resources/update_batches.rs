use super::*;

#[tokio::test]
async fn requests_accumulate_into_one_batch_while_source_capacity_is_busy() {
    let (_temp, cache) = cache().await;
    let gate = Arc::new(Semaphore::new(0));
    let provider = MockProvider::new(100, Some(gate.clone()));
    let service =
        ResourceService::new(cache.clone(), vec![provider.clone()]).unwrap();
    let mut tasks = Vec::new();
    for index in 0..3 {
        let service = service.clone();
        tasks.push(tokio::spawn(async move {
            service
                .query(
                    &SourceId::modrinth(),
                    project_query(&format!("block-{index}")),
                    CachePolicy::UseFresh,
                    &CancellationToken::new(),
                )
                .await
        }));
        wait_until(|| provider.calls() == index + 1).await;
    }
    for group in 0..2 {
        for index in 0..10 {
            let service = service.clone();
            tasks.push(tokio::spawn(async move {
                service
                    .query(
                        &SourceId::modrinth(),
                        project_query(&format!("queued-{group}-{index}")),
                        CachePolicy::UseFresh,
                        &CancellationToken::new(),
                    )
                    .await
            }));
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert_eq!(provider.calls(), 3);
    gate.add_permits(30);
    for task in tasks {
        task.await.unwrap().unwrap();
    }
    let batches = provider.batches.lock().unwrap();
    assert_eq!(batches.len(), 4);
    assert_eq!(batches[3].len(), 20);
    assert_eq!(provider.max_in_flight.load(Ordering::Acquire), 3);
    drop(batches);
    cache.close().await;
}

fn update_query(
    project: &str,
    hash: String,
    game: &str,
    loader: &str,
) -> ProviderQuery {
    ProviderQuery::UpdateCandidates {
        project_id: SourceProjectId::new(project).unwrap(),
        sha1: hash,
        target: ipc::ResourceTarget {
            game_versions: vec![game.into()],
            loaders: vec![loader.into()],
            channels: vec![ReleaseChannel::Release],
        },
    }
}

#[tokio::test]
async fn refresh_round_reuses_release_documents_seeded_by_update_batches() {
    let transport = FixtureTransport::new(|_, request| {
        assert_eq!(request.path, "/version_files/update");
        let release: Value = serde_json::from_str(fixtures::MR_IRIS).unwrap();
        Ok(json!({ "574992b5760f3bf2f9aa0b70984f19fb1952b748": release }))
    });
    let (_temp, cache) = cache().await;
    let service = ResourceService::new(
        cache.clone(),
        vec![Arc::new(ModrinthProvider::new(transport.clone()))],
    )
    .unwrap();
    let cancel = CancellationToken::new();
    let policy =
        CachePolicy::RefreshSince(chrono::Utc::now().timestamp_micros());
    let outcome = service
        .query(
            &SourceId::modrinth(),
            update_query(
                "YL57xq9U",
                "574992b5760f3bf2f9aa0b70984f19fb1952b748".into(),
                "1.20.1",
                "fabric",
            ),
            policy,
            &cancel,
        )
        .await
        .unwrap();
    let ProviderValue::UpdateCandidates(documents) = outcome.value else {
        panic!("wrong response");
    };
    let expected = &documents[0].release;
    let release = service
        .release_with_policy(&expected.reference, None, &cancel, policy)
        .await
        .unwrap();
    assert_eq!(&release, expected);
    assert_eq!(transport.requests.lock().unwrap().len(), 1);
    cache.close().await;
}

#[tokio::test]
async fn modrinth_batches_respect_limits_and_split_targets_without_losing_hashes()
 {
    let transport = FixtureTransport::new(|_, request| {
        assert_eq!(request.path, "/version_files/update");
        let body = request.body.as_ref().unwrap();
        let hashes = body["hashes"].as_array().unwrap();
        assert!(hashes.len() <= 100);
        assert_eq!(body["algorithm"], "sha1");
        assert_eq!(body["version_types"], json!(["release"]));
        let mut release: Value =
            serde_json::from_str(fixtures::MR_IRIS).unwrap();
        release["game_versions"] = body["game_versions"].clone();
        release["loaders"] = body["loaders"].clone();
        Ok(Value::Object(
            hashes
                .iter()
                .map(|hash| (hash.as_str().unwrap().into(), release.clone()))
                .collect(),
        ))
    });
    let (_temp, cache) = cache().await;
    let service = ResourceService::new(
        cache.clone(),
        vec![Arc::new(ModrinthProvider::new(transport.clone()))],
    )
    .unwrap();
    let queries = (0..213)
        .map(|index| {
            update_query(
                "YL57xq9U",
                format!("{index:040x}"),
                "1.20.1",
                "fabric",
            )
        })
        .collect();
    let results = service
        .query_many(
            &SourceId::modrinth(),
            queries,
            CachePolicy::ForceRefresh,
            &CancellationToken::new(),
        )
        .await;
    assert_eq!(results.len(), 213);
    assert!(results.iter().all(|result| matches!(result, Ok(QueryOutcome { value: ProviderValue::UpdateCandidates(documents), .. }) if documents.len() == 1)));
    assert_eq!(transport.requests.lock().unwrap().len(), 3);
    transport.requests.lock().unwrap().clear();
    let provider = ModrinthProvider::new(transport.clone());
    let results = provider
        .fetch_many(&[
            update_query("YL57xq9U", "a".repeat(40), "1.20.1", "fabric"),
            update_query("YL57xq9U", "b".repeat(40), "1.21", "quilt"),
        ])
        .await;
    assert!(results.iter().all(Result::is_ok));
    assert_eq!(transport.requests.lock().unwrap().len(), 2);
    cache.close().await;
}

#[tokio::test]
async fn independent_identification_calls_are_coalesced() {
    let transport = FixtureTransport::new(|_, request| {
        assert_eq!(request.path, "/version_files");
        Ok(json!({}))
    });
    let (_temp, cache) = cache().await;
    let service = ResourceService::new(
        cache.clone(),
        vec![Arc::new(ModrinthProvider::new(transport.clone()))],
    )
    .unwrap();
    let cancel = CancellationToken::new();
    let source = SourceId::modrinth();
    let results = futures::future::join_all((0..20).map(|index| {
        service.query(
            &source,
            ProviderQuery::Identify {
                integrity: ArtifactIntegrity {
                    size: Some(1),
                    hashes: vec![ContentHash {
                        algorithm: HashAlgorithm::Sha1,
                        value: format!("{index:040x}"),
                    }],
                },
                fingerprint: None,
            },
            CachePolicy::ForceRefresh,
            &cancel,
        )
    }))
    .await;
    assert!(results.iter().all(Result::is_ok));
    let requests = transport.requests.lock().unwrap();
    assert!(
        requests.len() < 20,
        "independent requests must share source batches"
    );
    let mut hashes = Vec::new();
    for request in requests.iter() {
        let batch =
            request.body.as_ref().unwrap()["hashes"].as_array().unwrap();
        assert!(batch.len() <= 100);
        hashes
            .extend(batch.iter().map(|hash| hash.as_str().unwrap().to_owned()));
    }
    assert_eq!(hashes.len(), 20);
    hashes.sort();
    hashes.dedup();
    assert_eq!(hashes.len(), 20);
    drop(requests);
    cache.close().await;
}

#[tokio::test]
async fn curseforge_uses_project_indexes_and_verifies_files_in_batches_of_fifty()
 {
    let transport = FixtureTransport::new(|_, request| {
        match request.path.as_str() {
            "/v1/mods" => {
                let projects = request.body.as_ref().unwrap()["modIds"]
                    .as_array()
                    .unwrap();
                assert!(projects.len() <= 50);
                Ok(
                    json!({ "data": projects.iter().map(|id| json!({ "id": id, "name": "project", "slug": "project", "classId": 6,
                "latestFilesIndexes": [{ "gameVersion": "1.20.1", "modLoader": 4, "fileId": id.as_u64().unwrap() + 1000, "releaseType": 1 }] })).collect::<Vec<_>>() }),
                )
            }
            "/v1/mods/files" => {
                let ids = request.body.as_ref().unwrap()["fileIds"]
                    .as_array()
                    .unwrap();
                assert!(ids.len() <= 50);
                Ok(json!({ "data": ids.iter().map(|id| {
                let mut file: Value = serde_json::from_str(fixtures::CF_IRIS).unwrap();
                file["id"] = id.clone(); file["modId"] = json!(id.as_u64().unwrap() - 1000);
                file
            }).collect::<Vec<_>>() }))
            }
            _ => panic!("unexpected endpoint {}", request.path),
        }
    });
    let (_temp, cache) = cache().await;
    let service = ResourceService::new(
        cache.clone(),
        vec![Arc::new(Available(Arc::new(
            CurseForgeProvider::for_tests(transport.clone()),
        )))],
    )
    .unwrap();
    let results = service
        .query_many(
            &SourceId::curseforge(),
            (1..=103)
                .map(|id| {
                    update_query(
                        &id.to_string(),
                        "a".repeat(40),
                        "1.20.1",
                        "fabric",
                    )
                })
                .collect(),
            CachePolicy::ForceRefresh,
            &CancellationToken::new(),
        )
        .await;
    assert_eq!(results.len(), 103);
    assert!(results.iter().all(|result| matches!(result, Ok(QueryOutcome { value: ProviderValue::UpdateCandidates(documents), .. }) if documents.len() == 1)));
    let requests = transport.requests.lock().unwrap();
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.path == "/v1/mods")
            .count(),
        3
    );
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.path == "/v1/mods/files")
            .count(),
        3
    );
    drop(requests);
    cache.close().await;
}

#[tokio::test]
async fn modrinth_batches_keep_different_channel_policies_separate() {
    let transport = FixtureTransport::new(|_, request| {
        assert_eq!(request.path, "/version_files/update");
        let body = request.body.as_ref().unwrap();
        let channels = body["version_types"].as_array().unwrap();
        assert_eq!(channels.len(), 1);
        let mut release: Value =
            serde_json::from_str(fixtures::MR_IRIS).unwrap();
        release["version_type"] = channels[0].clone();
        Ok(Value::Object(
            body["hashes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|hash| (hash.as_str().unwrap().into(), release.clone()))
                .collect(),
        ))
    });
    let release = update_query("YL57xq9U", "a".repeat(40), "1.20.1", "fabric");
    let mut alpha = release.clone();
    if let ProviderQuery::UpdateCandidates { target, .. } = &mut alpha {
        target.channels = vec![ReleaseChannel::Alpha];
    }
    let results = ModrinthProvider::new(transport.clone())
        .fetch_many(&[release, alpha])
        .await;
    for (result, channel) in results
        .iter()
        .zip([ReleaseChannel::Release, ReleaseChannel::Alpha])
    {
        let ProviderValue::UpdateCandidates(documents) =
            result.as_ref().unwrap()
        else {
            panic!("wrong response");
        };
        assert_eq!(documents[0].release.channel, channel);
    }
    assert_eq!(transport.requests.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn curseforge_indexes_fetch_only_files_in_allowed_channels() {
    let transport = FixtureTransport::new(|_, request| {
        match request.path.as_str() {
            "/v1/mods" => Ok(
                json!({"data":[{"id":455508,"name":"Iris","slug":"iris","classId":6,"latestFilesIndexes":[
                    {"gameVersion":"1.20.1","modLoader":4,"releaseType":3,"fileId":9999999},
                    {"gameVersion":"1.20.1","modLoader":4,"releaseType":1,"fileId":5425378}
                ]}]}),
            ),
            "/v1/mods/files" => {
                assert_eq!(
                    request.body.as_ref().unwrap()["fileIds"],
                    json!([5425378])
                );
                Ok(
                    json!({"data":[serde_json::from_str::<Value>(fixtures::CF_IRIS).unwrap()]}),
                )
            }
            _ => panic!("unexpected endpoint {}", request.path),
        }
    });
    let results = CurseForgeProvider::for_tests(transport.clone())
        .fetch_many(&[update_query(
            "455508",
            "a".repeat(40),
            "1.20.1",
            "fabric",
        )])
        .await;
    let ProviderValue::UpdateCandidates(documents) =
        results[0].as_ref().unwrap()
    else {
        panic!("wrong response");
    };
    assert_eq!(documents.len(), 1);
    assert_eq!(documents[0].release.channel, ReleaseChannel::Release);
    assert_eq!(transport.requests.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn missing_batch_rows_and_wrong_project_candidates_are_never_successful_no_update()
 {
    let transport = FixtureTransport::new(|_, request| {
        assert_eq!(request.path, "/version_files/update");
        let mut release: Value =
            serde_json::from_str(fixtures::MR_IRIS).unwrap();
        release["project_id"] = json!("wrong000");
        Ok(json!({ "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa": release }))
    });
    let (_temp, cache) = cache().await;
    let service = ResourceService::new(
        cache.clone(),
        vec![Arc::new(ModrinthProvider::new(transport))],
    )
    .unwrap();
    let result = service
        .query(
            &SourceId::modrinth(),
            update_query("YL57xq9U", "a".repeat(40), "1.20.1", "fabric"),
            CachePolicy::ForceRefresh,
            &CancellationToken::new(),
        )
        .await;
    assert!(matches!(result, Err(ResourceError::InvalidResponse(_))));
    cache.close().await;
}

fn installed_fixture() -> InstalledResource {
    let artifact =
        legacy::release(serde_json::from_str(fixtures::MR_IRIS).unwrap())
            .unwrap()
            .release
            .artifacts
            .remove(0);
    InstalledResource {
        entry_id: InstalledEntryId::new("entry").unwrap(),
        instance_id: "instance".into(),
        resource_id: None,
        artifact: Some(artifact.reference.clone()),
        known_artifacts: vec![artifact.reference.clone()],
        integrity: artifact.integrity,
        relative_path: "mods/iris.jar".into(),
        original_file_name: None,
        resource_type: ResourceType::Mod,
        scope: InstallScope::Instance,
        enabled: true,
        missing: false,
        ownership: ResourceOwnership::UserAdded,
    }
}

#[path = "../state/store/tests/content_update_batches.rs"]
mod persisted_update_batches;

#[tokio::test]
async fn a_hundred_numeric_checks_reuse_two_http_batches_without_histories() {
    let transport =
        FixtureTransport::new(|_, request| match request.path.as_str() {
            "/versions" => {
                Ok(json!([
                    serde_json::from_str::<Value>(fixtures::MR_IRIS).unwrap()
                ]))
            }
            "/version_files/update" => {
                let mut next: Value =
                    serde_json::from_str(fixtures::MR_IRIS).unwrap();
                next["id"] = json!("next0000");
                next["version_number"] = json!("2.0");
                next["date_published"] = json!("2025-01-01T00:00:00Z");
                next["files"][0]["hashes"]["sha1"] = json!("b".repeat(40));
                let hashes = request.body.as_ref().unwrap()["hashes"]
                    .as_array()
                    .unwrap();
                assert_eq!(hashes.len(), 100);
                Ok(Value::Object(
                    hashes
                        .iter()
                        .map(|hash| {
                            (hash.as_str().unwrap().into(), next.clone())
                        })
                        .collect(),
                ))
            }
            _ => panic!("unexpected scalar request {}", request.path),
        });
    let (_temp, cache) = cache().await;
    let service = ResourceService::new(
        cache.clone(),
        vec![Arc::new(ModrinthProvider::new(transport.clone()))],
    )
    .unwrap();
    let entries = (0..100)
        .map(|index| {
            let mut entry = installed_fixture();
            entry.entry_id =
                InstalledEntryId::new(format!("entry-{index}")).unwrap();
            entry.integrity.hashes = vec![ContentHash {
                algorithm: HashAlgorithm::Sha1,
                value: format!("{index:040x}"),
            }];
            entry
        })
        .collect::<Vec<_>>();
    let target = ipc::ResourceTarget {
        game_versions: vec!["1.20.1".into()],
        loaders: vec!["fabric".into()],
        channels: vec![ReleaseChannel::Release],
    };
    let cancel = CancellationToken::new();
    let preferred = SourceId::modrinth();
    let policy =
        CachePolicy::RefreshSince(chrono::Utc::now().timestamp_micros());
    let candidates = service
        .query_many(
            &preferred,
            entries
                .iter()
                .map(|entry| {
                    update_query(
                        "YL57xq9U",
                        entry.integrity.hashes[0].value.clone(),
                        "1.20.1",
                        "fabric",
                    )
                })
                .collect(),
            policy,
            &cancel,
        )
        .await;
    assert!(candidates.iter().all(Result::is_ok));
    let results = futures::future::join_all(entries.iter().map(|entry| {
        updates::check_entry_with_policy(
            &service, entry, &target, &preferred, &cancel, policy,
        )
    }))
    .await;
    assert!(results.iter().all(|result| matches!(result, Ok(Some(reference)) if reference.release_id.as_str() == "next0000")));
    assert_eq!(transport.requests.lock().unwrap().len(), 2);
    cache.close().await;
}

#[tokio::test]
async fn batch_candidate_selection_falls_back_only_for_incomplete_or_invalid_responses()
 {
    for scenario in [
        "missing",
        "wrong-channel",
        "wrong-project",
        "network",
        "numeric",
        "direct",
        "current",
        "older-numeric",
        "invalid-primary",
    ] {
        let transport = FixtureTransport::new(move |_, request| {
            let old: Value = serde_json::from_str(fixtures::MR_IRIS).unwrap();
            let mut old = old;
            if scenario == "older-numeric" {
                old["version_number"] = json!("9.0");
            }
            let mut next = old.clone();
            next["id"] = json!("next0000");
            next["version_number"] = json!("1.8+mc1.20.1");
            next["date_published"] = json!("2025-01-01T00:00:00Z");
            next["files"][0]["hashes"]["sha1"] = json!("b".repeat(40));
            match request.path.as_str() {
                "/versions" => Ok(json!([old])),
                "/version_files/update" => {
                    if scenario == "network" {
                        return Err(ResourceError::Unavailable);
                    }
                    if scenario == "missing" {
                        return Ok(json!({}));
                    }
                    if scenario == "wrong-channel" {
                        next["version_type"] = json!("alpha");
                    }
                    if scenario == "numeric" {
                        next["version_number"] = json!("1.8");
                    }
                    if scenario == "older-numeric" {
                        next["version_number"] = json!("8.0");
                    }
                    if scenario == "current" {
                        return Ok(
                            json!({ "574992b5760f3bf2f9aa0b70984f19fb1952b748": old }),
                        );
                    }
                    if scenario == "wrong-project" {
                        next["project_id"] = json!("wrong000");
                    }
                    if scenario == "invalid-primary" {
                        next["files"][0]["primary"] = json!(false);
                    }
                    Ok(
                        json!({ "574992b5760f3bf2f9aa0b70984f19fb1952b748": next }),
                    )
                }
                "/project/YL57xq9U/version" => {
                    assert!(
                        request
                            .query
                            .iter()
                            .any(|(key, value)| key == "game_versions"
                                && value.contains("1.20.1"))
                    );
                    assert!(
                        request
                            .query
                            .iter()
                            .any(|(key, value)| key == "loaders"
                                && value.contains("fabric"))
                    );
                    Ok(json!([old, next]))
                }
                _ => panic!("unexpected request {}", request.path),
            }
        });
        let (_temp, cache) = cache().await;
        let service = ResourceService::new(
            cache.clone(),
            vec![Arc::new(ModrinthProvider::new(transport.clone()))],
        )
        .unwrap();
        let target = ipc::ResourceTarget {
            game_versions: vec!["1.20.1".into()],
            loaders: vec!["fabric".into()],
            channels: vec![ReleaseChannel::Release],
        };
        let selected = updates::check_entry_with_policy(
            &service,
            &installed_fixture(),
            &target,
            &SourceId::modrinth(),
            &CancellationToken::new(),
            CachePolicy::ForceRefresh,
        )
        .await
        .unwrap();
        if matches!(scenario, "current" | "older-numeric") {
            assert!(selected.is_none(), "{scenario}");
        } else {
            assert_eq!(
                selected.unwrap().release_id.as_str(),
                "next0000",
                "{scenario}"
            );
        }
        let requests = transport.requests.lock().unwrap();
        assert_eq!(
            requests
                .iter()
                .filter(|request| request.path == "/project/YL57xq9U/version")
                .count(),
            usize::from(!matches!(
                scenario,
                "direct" | "numeric" | "current" | "older-numeric"
            )),
            "{scenario}"
        );
        drop(requests);
        cache.close().await;
    }
}

#[tokio::test]
async fn absent_candidates_and_failed_filtered_lookup_return_error_instead_of_no_update()
 {
    let transport =
        FixtureTransport::new(|_, request| match request.path.as_str() {
            "/versions" => {
                Ok(json!([
                    serde_json::from_str::<Value>(fixtures::MR_IRIS).unwrap()
                ]))
            }
            "/version_files/update" => Ok(json!({})),
            _ => Err(ResourceError::Unavailable),
        });
    let (_temp, cache) = cache().await;
    let service = ResourceService::new(
        cache.clone(),
        vec![Arc::new(ModrinthProvider::new(transport))],
    )
    .unwrap();
    let target = ipc::ResourceTarget {
        game_versions: vec!["1.20.1".into()],
        loaders: vec!["fabric".into()],
        channels: vec![ReleaseChannel::Release],
    };
    assert!(
        updates::check_entry_with_policy(
            &service,
            &installed_fixture(),
            &target,
            &SourceId::modrinth(),
            &CancellationToken::new(),
            CachePolicy::ForceRefresh
        )
        .await
        .is_err()
    );
    cache.close().await;
}

#[tokio::test]
async fn update_candidate_cache_is_short_lived_and_force_ignores_empty_and_positive_results()
 {
    let empty = Arc::new(AtomicBool::new(true));
    let transport = FixtureTransport::new({
        let empty = empty.clone();
        move |_, request| {
            assert_eq!(request.path, "/version_files/update");
            if empty.load(Ordering::SeqCst) {
                return Ok(json!({}));
            }
            Ok(
                json!({ "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa": serde_json::from_str::<Value>(fixtures::MR_IRIS).unwrap() }),
            )
        }
    });
    let (_temp, cache) = cache().await;
    let service = ResourceService::new(
        cache.clone(),
        vec![Arc::new(ModrinthProvider::new(transport.clone()))],
    )
    .unwrap();
    let query = update_query("YL57xq9U", "a".repeat(40), "1.20.1", "fabric");
    assert_eq!(query.ttl(&ProviderValue::UpdateCandidates(vec![])), 300);
    let cancel = CancellationToken::new();
    service
        .query(
            &SourceId::modrinth(),
            query.clone(),
            CachePolicy::UseFresh,
            &cancel,
        )
        .await
        .unwrap();
    empty.store(false, Ordering::SeqCst);
    let cached = service
        .query(
            &SourceId::modrinth(),
            query.clone(),
            CachePolicy::UseFresh,
            &cancel,
        )
        .await
        .unwrap();
    assert!(
        matches!(cached.value, ProviderValue::UpdateCandidates(ref documents) if documents.is_empty())
    );
    let fresh = service
        .query(
            &SourceId::modrinth(),
            query.clone(),
            CachePolicy::ForceRefresh,
            &cancel,
        )
        .await
        .unwrap();
    assert!(
        matches!(fresh.value, ProviderValue::UpdateCandidates(ref documents) if documents.len() == 1)
    );
    empty.store(true, Ordering::SeqCst);
    service
        .query(
            &SourceId::modrinth(),
            query,
            CachePolicy::ForceRefresh,
            &cancel,
        )
        .await
        .unwrap();
    assert_eq!(transport.requests.lock().unwrap().len(), 3);
    cache.close().await;
}

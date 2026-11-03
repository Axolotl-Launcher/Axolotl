use super::*;

#[tokio::test]
async fn confirmed_sources_use_batch_candidates_without_loading_project_histories()
 {
    let old = legacy::release(serde_json::from_str(fixtures::MR_IRIS).unwrap())
        .unwrap();
    let mut other = old.clone();
    other.release.reference.project = SourceProjectRef {
        source: SourceId::curseforge(),
        project_id: SourceProjectId::new("455508").unwrap(),
    };
    other.release.reference.release_id =
        SourceReleaseId::new("5425378").unwrap();
    for artifact in &mut other.release.artifacts {
        artifact.reference.release = other.release.reference.clone();
    }
    let cf_old = other.release.artifacts[0].clone();
    let fail_cf = Arc::new(AtomicBool::new(false));
    let failure = fail_cf.clone();
    let transport = FixtureTransport::new(move |source, request| {
        if *source == SourceId::modrinth() {
            match request.path.as_str() {
                "/versions" => Ok(json!([serde_json::from_str::<Value>(
                    fixtures::MR_IRIS
                )
                .unwrap()])),
                "/version_files/update" => {
                    assert_eq!(
                        request.body.as_ref().unwrap()["version_types"],
                        json!(["release"])
                    );
                    Ok(
                        json!({ "574992b5760f3bf2f9aa0b70984f19fb1952b748": serde_json::from_str::<Value>(fixtures::MR_IRIS).unwrap() }),
                    )
                }
                _ => panic!("unexpected MR history request {}", request.path),
            }
        } else {
            if failure.load(Ordering::SeqCst) {
                return Err(ResourceError::Unavailable);
            }
            match request.path.as_str() {
                "/v1/mods" => Ok(
                    json!({"data":[{"id":455508,"name":"Iris","slug":"iris","classId":6,"latestFilesIndexes":[{"gameVersion":"1.20.1","modLoader":4,"releaseType":1,"fileId":5425379}]}]}),
                ),
                "/v1/mods/files" => {
                    let mut file: Value =
                        serde_json::from_str(fixtures::CF_IRIS).unwrap();
                    file["id"] = json!(5425379);
                    file["displayName"] = json!("2.0");
                    file["fileDate"] = json!("2025-01-01T00:00:00Z");
                    file["hashes"] = json!([{"algo":1,"value":"b".repeat(40)}]);
                    Ok(json!({"data":[file]}))
                }
                _ => panic!("unexpected CF history request {}", request.path),
            }
        }
    });
    let (_temp, cache) = cache().await;
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();
    let store = crate::state::resources::ResourceStore::new(pool.clone());
    let ProviderValue::Project(mr) =
        project_value(SourceId::modrinth(), "YL57xq9U", "Iris")
    else {
        unreachable!()
    };
    let ProviderValue::Project(cf) =
        project_value(SourceId::curseforge(), "455508", "Iris")
    else {
        unreachable!()
    };
    let id = store
        .confirm_projects(
            &mr.metadata,
            &cf.metadata,
            &old.release.artifacts[0],
            &cf_old,
        )
        .await
        .unwrap();
    let service = ResourceService::with_store(
        cache.clone(),
        vec![
            Arc::new(ModrinthProvider::new(transport.clone())),
            Arc::new(Available(Arc::new(CurseForgeProvider::for_tests(
                transport.clone(),
            )))),
        ],
        store,
    )
    .unwrap();
    let mut installed = installed_fixture();
    installed.resource_id = Some(id);
    let target = ipc::ResourceTarget {
        game_versions: vec!["1.20.1".into()],
        loaders: vec!["fabric".into()],
        channels: vec![ReleaseChannel::Release],
    };
    let selected = updates::check_entry_with_policy(
        &service,
        &installed,
        &target,
        &SourceId::modrinth(),
        &CancellationToken::new(),
        CachePolicy::ForceRefresh,
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(selected.project.source, SourceId::curseforge());
    assert_eq!(selected.release_id.as_str(), "5425379");
    assert_eq!(transport.requests.lock().unwrap().len(), 4);
    fail_cf.store(true, Ordering::SeqCst);
    let incomplete = updates::check_entry_with_policy(
        &service,
        &installed,
        &target,
        &SourceId::modrinth(),
        &CancellationToken::new(),
        CachePolicy::ForceRefresh,
    )
    .await;
    assert!(
        matches!(incomplete, Err(ResourceError::InvalidResponse(message)) if message.contains("incomplete"))
    );
    cache.close().await;
    pool.close().await;
}

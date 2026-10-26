use super::*;
use crate::state::{CacheBehaviour, CacheValueType};

fn project_protocol(id: &str) -> Value {
    json!({
        "id": id, "slug": id, "project_type": "mod", "team": "team0000", "organization": null,
        "title": "Example", "description": "Example mod", "body": "", "published": "2024-01-01T00:00:00Z",
        "updated": "2024-01-01T00:00:00Z", "approved": null, "status": "approved",
        "license": {"id": "MIT", "name": "MIT", "url": null}, "client_side": "optional", "server_side": "optional",
        "downloads": 1, "followers": 0, "categories": [], "additional_categories": [], "game_versions": ["1.21.1"],
        "loaders": ["neoforge"], "versions": [], "icon_url": null, "issues_url": null, "source_url": null,
        "wiki_url": null, "discord_url": null, "donation_urls": [
            {"id": "patreon", "platform": "Patreon", "url": "https://patreon.com/example"},
            {"id": "other", "platform": "Other", "url": "https://example.com/donate?key=private-donation-key"}
        ],
        "gallery": [{"url": "https://example.com/gallery.png", "raw_url": "https://example.com/original.png", "featured": false,
            "title": null, "description": null, "created": "2024-01-01T00:00:00Z", "ordering": 0}], "color": null
    })
}

#[tokio::test]
async fn project_projection_accepts_nullable_fields_and_redacted_links_online_and_from_cache()
 {
    let transport =
        FixtureTransport::new(|_, request| match request.path.as_str() {
            "/projects" => Ok(json!([project_protocol("joEfVgkn")])),
            "/teams" => Ok(json!([])),
            _ => panic!("unexpected endpoint"),
        });
    let (_directory, cache) = cache().await;
    let service = ResourceService::new(
        cache,
        vec![Arc::new(ModrinthProvider::new(transport.clone()))],
    )
    .unwrap();
    for behaviour in [None, Some(CacheBehaviour::CacheOnly)] {
        let entries = protocol::many(
            &service,
            CacheValueType::Project,
            &["joEfVgkn"],
            behaviour,
            &CancellationToken::new(),
        )
        .await
        .unwrap();
        assert_eq!(entries.len(), 1);
        let entry = serde_json::to_value(&entries[0]).unwrap();
        let project: crate::state::Project =
            serde_json::from_value(entry["data"]["Project"].clone()).unwrap();
        assert!(project.organization.is_none());
        assert!(project.wiki_url.is_none());
        assert!(project.license.url.is_none());
        let links = project.donation_urls.as_ref().unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].url, "https://patreon.com/example");
        assert_eq!(
            project.gallery[0].raw_url,
            "https://example.com/original.png"
        );
        assert!(
            !serde_json::to_string(&entries)
                .unwrap()
                .contains("private-donation-key")
        );
    }
    assert_eq!(transport.requests.lock().unwrap().len(), 2);
    let query = ProviderQuery::Project {
        id: SourceProjectId::new("joEfVgkn").unwrap(),
        slug: None,
    };
    let cached = service
        .query(
            &SourceId::modrinth(),
            query,
            CachePolicy::OnlyCached,
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    let ProviderValue::Project(document) = cached.value else {
        panic!("project expected");
    };
    let links = document.protocol.unwrap()["donation_urls"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(links.len(), 1);
    assert_eq!(links[0]["url"], "https://patreon.com/example");
    service.cache().close().await;
}

#[tokio::test]
async fn project_projection_preserves_projects_without_teams_online_and_from_cache()
 {
    let transport = FixtureTransport::new(|_, request| {
        assert_eq!(request.path, "/projects");
        let mut nullable = project_protocol("nullable");
        nullable["team"] = Value::Null;
        let mut missing = project_protocol("missing0");
        missing.as_object_mut().unwrap().remove("team");
        Ok(json!([nullable, missing]))
    });
    let (_directory, cache) = cache().await;
    let service = ResourceService::new(
        cache,
        vec![Arc::new(ModrinthProvider::new(transport.clone()))],
    )
    .unwrap();
    for behaviour in [None, Some(CacheBehaviour::CacheOnly)] {
        let entries = protocol::many(
            &service,
            CacheValueType::Project,
            &["nullable", "missing0"],
            behaviour,
            &CancellationToken::new(),
        )
        .await
        .unwrap();
        assert_eq!(entries.len(), 2);
        for entry in entries {
            let entry = serde_json::to_value(entry).unwrap();
            assert_eq!(entry["data"]["Project"]["team"], "");
            assert_eq!(entry["data"]["Project"]["title"], "Example");
        }
    }
    let cached = service
        .query(
            &SourceId::modrinth(),
            ProviderQuery::Project {
                id: SourceProjectId::new("nullable").unwrap(),
                slug: None,
            },
            CachePolicy::OnlyCached,
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    let ProviderValue::Project(document) = cached.value else {
        panic!("project expected");
    };
    assert_eq!(document.protocol.unwrap()["team"], "");
    // Projects without teams must not trigger an author lookup.
    assert_eq!(transport.requests.lock().unwrap().len(), 1);
    service.cache().close().await;
}

#[tokio::test]
async fn project_projection_reads_legacy_cached_null_and_missing_teams_without_network()
 {
    let (_directory, cache) = cache().await;
    for id in ["nullable", "missing0"] {
        let mut raw = project_protocol(id);
        raw["team"] = Value::Null;
        if id == "missing0" {
            raw.as_object_mut().unwrap().remove("team");
        }
        let mut document =
            providers::legacy::project("project", raw.clone()).unwrap();
        // Reproduce a document cached before provider-side normalization.
        document.protocol = Some(providers::protocol_projection(raw));
        let query = ProviderQuery::Project {
            id: SourceProjectId::new(id).unwrap(),
            slug: None,
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
                &ProviderValue::Project(document),
                -1,
                cache.generation().await,
                chrono::Utc::now().timestamp_micros(),
            )
            .await;
    }
    let transport = FixtureTransport::new(|_, _| {
        panic!("cached projection must not access the network")
    });
    let service = ResourceService::new(
        cache,
        vec![Arc::new(ModrinthProvider::new(transport.clone()))],
    )
    .unwrap();
    let entries = protocol::many(
        &service,
        CacheValueType::Project,
        &["nullable", "missing0"],
        Some(CacheBehaviour::CacheOnly),
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(entries.len(), 2);
    for entry in entries {
        let entry = serde_json::to_value(entry).unwrap();
        assert_eq!(entry["data"]["Project"]["team"], "");
        assert_eq!(
            entry["data"]["Project"]["donation_urls"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }
    assert!(transport.requests.lock().unwrap().is_empty());
    service.cache().close().await;
}

#[tokio::test]
async fn malformed_project_projection_does_not_discard_other_content_metadata()
{
    let transport =
        FixtureTransport::new(|_, request| match request.path.as_str() {
            "/projects" => {
                let valid = project_protocol("joEfVgkn");
                let mut broken = project_protocol("broken00");
                broken["license"]["name"] = Value::Null;
                Ok(json!([broken, valid]))
            }
            "/teams" => Ok(json!([])),
            _ => panic!("unexpected endpoint"),
        });
    let (_directory, cache) = cache().await;
    let service = ResourceService::new(
        cache,
        vec![Arc::new(ModrinthProvider::new(transport))],
    )
    .unwrap();
    for behaviour in [None, Some(CacheBehaviour::CacheOnly)] {
        let entries = protocol::many(
            &service,
            CacheValueType::Project,
            &["broken00", "joEfVgkn"],
            behaviour,
            &CancellationToken::new(),
        )
        .await
        .unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(
            serde_json::to_value(&entries[0]).unwrap()["id"],
            "joEfVgkn"
        );
    }
    service.cache().close().await;
}

fn mr_protocol() -> Value {
    let mut value: Value = serde_json::from_str(fixtures::MR_IRIS).unwrap();
    value["author_id"] = json!("author00");
    value["name"] = json!("Iris fixture");
    value["downloads"] = json!(0);
    value["featured"] = json!(false);
    value
}

#[tokio::test]
async fn offline_update_queries_use_expired_identification_and_release_lists_but_bypass_does_not()
 {
    let mut current = mr_protocol();
    current["version_number"] = json!("1.0.0");
    current["game_versions"] = json!(["1.20.1"]);
    current["loaders"] = json!(["fabric"]);
    current["date_published"] = json!("2020-01-01T00:00:00Z");
    let mut newer = current.clone();
    newer["id"] = json!("new00000");
    newer["version_number"] = json!("2.0.0");
    newer["date_published"] = json!("2021-01-01T00:00:00Z");
    newer["files"][0]["hashes"]["sha1"] = json!("b".repeat(40));
    newer["files"][0]["hashes"]["sha512"] = json!("b".repeat(128));
    let hash = current["files"][0]["hashes"]["sha1"]
        .as_str()
        .unwrap()
        .to_owned();
    let (_temp, cache) = cache().await;
    let identified = ProviderQuery::IdentifyExport {
        integrity: ArtifactIntegrity {
            size: None,
            hashes: vec![ContentHash {
                algorithm: HashAlgorithm::Sha1,
                value: hash.clone(),
            }],
        },
        fingerprint: None,
    };
    let list = ProviderQuery::ReleasesProtocol {
        project_id: SourceProjectId::new(
            current["project_id"].as_str().unwrap(),
        )
        .unwrap(),
    };
    for (query, value) in [
        (
            identified,
            ProviderValue::Identified(vec![
                providers::legacy::release(current.clone()).unwrap(),
            ]),
        ),
        (list, ProviderValue::Protocol(json!([current, newer]))),
    ] {
        let key = CacheKey::new(
            "modrinth-v2-dates".into(),
            SourceId::modrinth(),
            &query.cache_identity(),
        )
        .unwrap();
        cache
            .put(
                &key,
                &value,
                -1,
                cache.generation().await,
                chrono::Utc::now().timestamp_micros(),
            )
            .await;
    }
    let transport = FixtureTransport::new(|_, _| {
        Err(ResourceError::Network { status: Some(503) })
    });
    let service = ResourceService::new(
        cache.clone(),
        vec![Arc::new(ModrinthProvider::new(transport))],
    )
    .unwrap();
    let key = format!("{hash}-fabric-release-1.20.1");
    let result = protocol::many(
        &service,
        CacheValueType::FileUpdate,
        &[&key],
        Some(CacheBehaviour::MustRevalidate),
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(result.len(), 1);
    assert!(
        protocol::many(
            &service,
            CacheValueType::FileUpdate,
            &[&key],
            Some(CacheBehaviour::Bypass),
            &CancellationToken::new()
        )
        .await
        .is_err()
    );
    cache.close().await;
}

#[tokio::test]
async fn shared_pack_queries_use_independent_cache_and_force_refresh() {
    let transport = FixtureTransport::new(|_, request| {
        assert_eq!(request.path, "/versions");
        Ok(json!([mr_protocol()]))
    });
    let (_directory, cache) = cache().await;
    let service = ResourceService::new(
        cache.clone(),
        vec![Arc::new(ModrinthProvider::new(transport.clone()))],
    )
    .unwrap();
    let cancel = CancellationToken::new();
    for behaviour in [None, None, Some(CacheBehaviour::Bypass)] {
        let entries = protocol::many(
            &service,
            CacheValueType::Version,
            &["G5dd9TM4"],
            behaviour,
            &cancel,
        )
        .await
        .unwrap();
        assert_eq!(
            serde_json::to_value(&entries[0]).unwrap()["id"],
            "G5dd9TM4"
        );
    }
    assert_eq!(transport.requests.lock().unwrap().len(), 2);
    cache.clear().await.unwrap();
    assert!(
        protocol::many(
            &service,
            CacheValueType::Version,
            &["G5dd9TM4"],
            Some(CacheBehaviour::CacheOnly),
            &cancel
        )
        .await
        .unwrap()
        .is_empty()
    );
    assert_eq!(transport.requests.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn pack_hash_lookup_reuses_fresh_release_without_requery() {
    let release = mr_protocol();
    let hash = release["files"][0]["hashes"]["sha1"]
        .as_str()
        .unwrap()
        .to_owned();
    let hash_response = hash.clone();
    let transport = FixtureTransport::new(move |_, request| {
        assert_eq!(request.path, "/version_files");
        Ok(json!({hash_response.clone(): release.clone()}))
    });
    let (_directory, cache) = cache().await;
    let service = ResourceService::new(
        cache,
        vec![Arc::new(ModrinthProvider::new(transport.clone()))],
    )
    .unwrap();
    let cancel = CancellationToken::new();
    let entries =
        protocol::many(&service, CacheValueType::File, &[&hash], None, &cancel)
            .await
            .unwrap();
    assert_eq!(serde_json::to_value(&entries[0]).unwrap()["id"], hash);
    let entries = protocol::many(
        &service,
        CacheValueType::Version,
        &["G5dd9TM4"],
        Some(CacheBehaviour::CacheOnly),
        &cancel,
    )
    .await
    .unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(transport.requests.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn pack_release_project_mismatch_is_rejected() {
    let transport = FixtureTransport::new(|_, _| {
        Ok(json!([
            serde_json::from_str::<Value>(fixtures::MR_IRIS).unwrap()
        ]))
    });
    let (_directory, cache) = cache().await;
    let service = ResourceService::new(
        cache,
        vec![Arc::new(ModrinthProvider::new(transport))],
    )
    .unwrap();
    let result = service
        .query(
            &SourceId::modrinth(),
            ProviderQuery::Release {
                project_id: Some(SourceProjectId::new("abcdefgh").unwrap()),
                release_id: SourceReleaseId::new("G5dd9TM4").unwrap(),
                slug: None,
            },
            CachePolicy::ForceRefresh,
            &CancellationToken::new(),
        )
        .await;
    assert!(matches!(result, Err(ResourceError::InvalidResponse(_))));
}

#[tokio::test]
async fn pack_indexes_are_validated_and_clearable() {
    let (_directory, cache) = cache().await;
    let mut index = crate::state::CachedModpackFiles {
        version_id: "G5dd9TM4".into(),
        file_hashes: vec!["a".repeat(40)],
        project_ids: vec!["abcdefgh".into()],
    };
    packs::save_index(&cache, &index).await.unwrap();
    assert_eq!(
        packs::index(&cache, "G5dd9TM4").await.unwrap().project_ids,
        index.project_ids
    );
    index.project_ids.push("not-a-stable-id".into());
    assert!(packs::save_index(&cache, &index).await.is_err());
    cache.clear().await.unwrap();
    assert!(packs::index(&cache, "G5dd9TM4").await.is_none());
}

#[tokio::test]
async fn force_refresh_never_falls_back_to_saved_protocol_metadata() {
    let transport = FixtureTransport::new(|_, _| {
        Err(ResourceError::Network { status: Some(503) })
    });
    let (_directory, cache) = cache().await;
    let query = ProviderQuery::Release {
        project_id: None,
        release_id: SourceReleaseId::new("G5dd9TM4").unwrap(),
        slug: None,
    };
    let key =
        CacheKey::new("modrinth-v2-dates".into(), SourceId::modrinth(), &query)
            .unwrap();
    cache
        .put(
            &key,
            &ProviderValue::Release(
                providers::legacy::release(mr_protocol()).unwrap(),
            ),
            -1,
            cache.generation().await,
            chrono::Utc::now().timestamp_micros(),
        )
        .await;
    let service = ResourceService::new(
        cache,
        vec![Arc::new(ModrinthProvider::new(transport.clone()))],
    )
    .unwrap();
    let cancel = CancellationToken::new();
    assert_eq!(
        protocol::many(
            &service,
            CacheValueType::Version,
            &["G5dd9TM4"],
            Some(CacheBehaviour::MustRevalidate),
            &cancel
        )
        .await
        .unwrap()
        .len(),
        1
    );
    assert!(
        protocol::many(
            &service,
            CacheValueType::Version,
            &["G5dd9TM4"],
            Some(CacheBehaviour::Bypass),
            &cancel
        )
        .await
        .is_err()
    );
    assert_eq!(transport.requests.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn protocol_search_preserves_filters_and_query_encoding() {
    let transport = FixtureTransport::new(|_, request| {
        assert_eq!(request.path, "/v3/search");
        assert!(request.query.contains(&("query".into(), "a+b".into())));
        assert!(request.query.contains(&("limit".into(), "100".into())));
        Ok(json!({"hits":[],"total_hits":0,"offset":0,"limit":100}))
    });
    let (_directory, cache) = cache().await;
    let service = ResourceService::new(
        cache,
        vec![Arc::new(ModrinthProvider::new(transport.clone()))],
    )
    .unwrap();
    let values = protocol::many(
        &service,
        CacheValueType::SearchResultsV3,
        &["?query=a%2Bb&limit=100"],
        None,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(transport.requests.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn pack_artifact_snapshot_requires_exact_size_and_hash() {
    let transport = FixtureTransport::new(|_, _| {
        Ok(json!([
            serde_json::from_str::<Value>(fixtures::MR_IRIS).unwrap()
        ]))
    });
    let (_directory, cache) = cache().await;
    let service = ResourceService::new(
        cache.clone(),
        vec![Arc::new(ModrinthProvider::new(transport.clone()))],
    )
    .unwrap();
    let cancel = CancellationToken::new();
    let value = service
        .query(
            &SourceId::modrinth(),
            ProviderQuery::Release {
                project_id: None,
                release_id: SourceReleaseId::new("G5dd9TM4").unwrap(),
                slug: None,
            },
            CachePolicy::ForceRefresh,
            &cancel,
        )
        .await
        .unwrap();
    let ProviderValue::Release(document) = value.value else {
        panic!("release expected");
    };
    let artifact = document.release.artifacts.first().unwrap();
    let hash = artifact
        .integrity
        .hashes
        .iter()
        .find(|hash| hash.algorithm == HashAlgorithm::Sha1)
        .unwrap()
        .value
        .clone();
    let local = ArtifactIntegrity {
        size: artifact.integrity.size,
        hashes: vec![ContentHash {
            algorithm: HashAlgorithm::Sha1,
            value: hash,
        }],
    };
    assert!(identity::exact_integrity(&local, &artifact.integrity));
    let mut wrong_size = local.clone();
    wrong_size.size = Some(local.size.unwrap() + 1);
    assert!(!identity::exact_integrity(&wrong_size, &artifact.integrity));
    let mut changed_bytes = local;
    changed_bytes.hashes[0].value = "0".repeat(40);
    assert!(!identity::exact_integrity(
        &changed_bytes,
        &artifact.integrity
    ));
    assert_eq!(transport.requests.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn imported_pack_member_persists_exact_artifact_and_identity_after_cache_clear()
 {
    use crate::state::instances::{
        ContentOwnershipKind, adapters::sqlite::content_rows,
        commands::ProjectFileRecord,
    };
    use crate::state::{
        ContentProviderRef, ContentSourceKind, DirectoryInfo,
        ModrinthProjectId, ModrinthVersionId, ProjectType,
    };
    let directory = tempfile::tempdir().unwrap();
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();
    sqlx::raw_sql("INSERT INTO instances(id, path, install_stage, launcher_feature_version, name, created, modified) VALUES ('instance', 'example', 'installed', '1', 'Example', 1, 1); INSERT INTO instance_content_sets(id, instance_id, name, source_kind, status, game_version, loader, created, modified) VALUES ('set', 'instance', 'Example', 'imported_modpack', 'available', '1.20.1', 'fabric', 1, 1); UPDATE instances SET applied_content_set_id = 'set' WHERE id = 'instance';").execute(&pool).await.unwrap();
    let state = crate::state::test_state(
        DirectoryInfo {
            settings_dir: directory.path().into(),
            config_dir: directory.path().into(),
            app_identifier: "pack-resource-test".into(),
        },
        pool.clone(),
    )
    .await
    .unwrap();
    let service = state.resources().await.unwrap();
    let document = providers::legacy::release(mr_protocol()).unwrap();
    let artifact = document.release.artifacts.first().unwrap();
    let sha1 = artifact
        .integrity
        .hashes
        .iter()
        .find(|hash| hash.algorithm == HashAlgorithm::Sha1)
        .unwrap()
        .value
        .clone();
    let query = ProviderQuery::Release {
        project_id: Some(document.release.reference.project.project_id.clone()),
        release_id: document.release.reference.release_id.clone(),
        slug: None,
    };
    let key =
        CacheKey::new("modrinth-v2-dates".into(), SourceId::modrinth(), &query)
            .unwrap();
    service
        .cache()
        .put(
            &key,
            &ProviderValue::Release(document.clone()),
            86400,
            service.cache().generation().await,
            chrono::Utc::now().timestamp_micros(),
        )
        .await;
    let record = ProjectFileRecord {
        relative_path: "mods/iris.jar".into(),
        sha1: sha1.clone(),
        size: artifact.integrity.size.unwrap(),
        project_type: ProjectType::Mod,
        source_kind: ContentSourceKind::ImportedModpack,
        ownership_kind: ContentOwnershipKind::PackManaged,
        provider_ref: Some(ContentProviderRef::Modrinth {
            project_id: ModrinthProjectId::new("YL57xq9U").unwrap(),
            version_id: Some(ModrinthVersionId::new("G5dd9TM4").unwrap()),
        }),
        origin: false,
        known_modrinth_project_id: Some("YL57xq9U".into()),
        known_modrinth_version_id: Some("G5dd9TM4".into()),
    };
    crate::state::instances::commands::record_project_files_atomic_with_permit(
        "instance",
        &[record.clone()],
        &[],
        None,
        &state,
    )
    .await
    .unwrap();
    service.cache().clear().await.unwrap();
    let entries = content_rows::get_content_entries("set", &pool)
        .await
        .unwrap();
    assert_eq!(entries.len(), 1);
    let saved: String = sqlx::query_scalar(
        "SELECT artifact FROM resource_installed_artifacts WHERE entry_id = ?",
    )
    .bind(&entries[0].id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        serde_json::from_str::<ResourceArtifact>(&saved).unwrap(),
        *artifact
    );
    let links: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM resource_entry_links WHERE entry_id = ? AND source = 'modrinth' AND project_id = 'YL57xq9U'").bind(&entries[0].id).fetch_one(&pool).await.unwrap();
    assert_eq!(links, 1);
    let members = content_rows::get_pack_members("set", &pool).await.unwrap();
    assert_eq!(members.len(), 1);
    assert_eq!(
        members[0].content_entry_id.as_deref(),
        Some(entries[0].id.as_str())
    );
    let main_cache: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM cache WHERE data_type IN ('project','version','file')").fetch_one(&pool).await.unwrap();
    assert_eq!(main_cache, 0);
    assert!(
        sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&pool)
            .await
            .unwrap()
            .is_empty()
    );
    let mut changed = record;
    changed.size += 1;
    crate::state::instances::commands::record_project_files_atomic_with_permit(
        "instance",
        &[changed],
        &[],
        None,
        &state,
    )
    .await
    .unwrap();
    let artifacts: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM resource_installed_artifacts WHERE entry_id = ?",
    )
    .bind(&entries[0].id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        artifacts, 0,
        "replacement without current exact evidence must drop the previous artifact snapshot"
    );
}

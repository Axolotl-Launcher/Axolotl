use super::{cache::*, identity::*, providers::*, *};
use async_trait::async_trait;
use serde_json::{Value, json};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use tokio::sync::{Notify, Semaphore};
use tokio_util::sync::CancellationToken;

#[test]
fn modrinth_orphaned_embedded_dependency_does_not_hide_release() {
    let mut release: Value = serde_json::from_str(fixtures::MR_IRIS).unwrap();
    release["dependencies"] = json!([
        {"project_id": null, "version_id": null, "file_name": null,
         "dependency_type": "embedded"},
        {"project_id": "P7dR8mSH", "version_id": null, "file_name": null,
         "dependency_type": "required"}
    ]);
    let document = legacy::release(release).unwrap();
    for artifact in document.release.artifacts {
        assert_eq!(artifact.dependencies.len(), 1);
        assert_eq!(
            artifact.dependencies[0].kind,
            ResourceDependencyKind::Required
        );
    }
    assert_eq!(
        document.protocol.unwrap()["dependencies"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn modrinth_missing_required_dependency_target_is_still_rejected() {
    let mut release: Value = serde_json::from_str(fixtures::MR_IRIS).unwrap();
    release["dependencies"] = json!([
        {"project_id": null, "version_id": null, "file_name": null,
         "dependency_type": "required"}
    ]);
    assert!(matches!(
        legacy::release(release),
        Err(ResourceError::InvalidResponse(message)) if message == "dependency has no target"
    ));
}

#[tokio::test]
async fn modrinth_catalog_pages_share_one_complete_response() {
    let transport = FixtureTransport::new(|_, request| {
        assert_eq!(request.path, "/project/YL57xq9U/version");
        let release: Value = serde_json::from_str(fixtures::MR_IRIS).unwrap();
        Ok(Value::Array(
            (0..120)
                .map(|index| {
                    let mut value = release.clone();
                    value["id"] = json!(format!("v{index:07}"));
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
    let reference = ResourceRef::Source {
        project: source_project(SourceId::modrinth(), "YL57xq9U").unwrap(),
    };
    let cancel = CancellationToken::new();
    let mut cursor = None;
    let mut ids = std::collections::HashSet::new();
    loop {
        let page = service
            .releases(
                &reference,
                target(),
                cursor,
                RequestId::new("page").unwrap(),
                None,
                &cancel,
            )
            .await
            .unwrap();
        for release in page.entries {
            assert!(ids.insert(release.reference.release_id));
        }
        if !page.has_more {
            break;
        }
        cursor = page.cursor;
    }
    assert_eq!(ids.len(), 120);
    assert_eq!(transport.requests.lock().unwrap().len(), 1);
    cache.close().await;
}

#[tokio::test]
async fn presentation_failure_uses_expired_metadata_without_masking_force_refresh()
 {
    let (_temp, cache) = cache().await;
    let provider = MockProvider::new(100, None);
    let query = project_query("abcdefgh");
    let key =
        CacheKey::new("mock-v1".into(), SourceId::modrinth(), &query).unwrap();
    cache
        .put(
            &key,
            &project_value(SourceId::modrinth(), "abcdefgh", "Saved"),
            -1,
            cache.generation().await,
            chrono::Utc::now().timestamp_micros(),
        )
        .await;
    provider.fail.store(true, Ordering::Relaxed);
    let service = ResourceService::new(cache.clone(), vec![provider]).unwrap();
    let project = source_project(SourceId::modrinth(), "abcdefgh").unwrap();
    assert_eq!(
        service
            .project_document(&project, None, &CancellationToken::new())
            .await
            .unwrap()
            .metadata
            .title,
        "Saved"
    );
    assert!(
        service
            .query(
                &project.source,
                query,
                CachePolicy::ForceRefresh,
                &CancellationToken::new()
            )
            .await
            .is_err()
    );
    cache.close().await;
}

#[path = "../../../content-management/tests/fixtures/mod.rs"]
mod fixtures;

#[tokio::test]
async fn presentation_keeps_incomplete_protocol_cache_when_offline_without_retrying_twice()
 {
    for ttl in [-1, 60] {
        let (_temp, cache) = cache().await;
        let provider = MockProvider::new(100, None);
        let query = project_query("abcdefgh");
        let value = project_value(SourceId::modrinth(), "abcdefgh", "Saved");
        let key = CacheKey::new("mock-v1".into(), SourceId::modrinth(), &query)
            .unwrap();
        cache
            .put(
                &key,
                &value,
                ttl,
                cache.generation().await,
                chrono::Utc::now().timestamp_micros(),
            )
            .await;
        provider.fail.store(true, Ordering::Relaxed);
        let service =
            ResourceService::new(cache.clone(), vec![provider.clone()])
                .unwrap();
        let outcome = service
            .presentation_query(
                &SourceId::modrinth(),
                query.clone(),
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(outcome.value, value);
        assert!(outcome.from_cache);
        assert_eq!(provider.calls(), 1);
        assert!(
            service
                .presentation_many_with_policy(
                    &SourceId::modrinth(),
                    vec![query],
                    &CancellationToken::new(),
                    CachePolicy::ForceRefresh
                )
                .await[0]
                .is_err()
        );
        cache.close().await;
    }
}

#[tokio::test]
async fn revalidation_refreshes_fresh_project_indexes_and_keeps_offline_cache()
{
    let (_temp, cache) = cache().await;
    let provider = MockProvider::new(100, None);
    let service =
        ResourceService::new(cache.clone(), vec![provider.clone()]).unwrap();
    let query = project_query("abcdefgh");
    let saved = project_value(SourceId::modrinth(), "abcdefgh", "Saved");
    let key =
        CacheKey::new("mock-v1".into(), SourceId::modrinth(), &query).unwrap();
    cache
        .put(
            &key,
            &saved,
            1800,
            cache.generation().await,
            chrono::Utc::now().timestamp_micros(),
        )
        .await;
    let cancel = CancellationToken::new();
    let current = service
        .presentation_many_with_policy(
            &SourceId::modrinth(),
            vec![query.clone()],
            &cancel,
            CachePolicy::Revalidate,
        )
        .await
        .remove(0)
        .unwrap();
    assert!(!current.from_cache);
    assert_ne!(current.value, saved);
    assert_eq!(provider.calls(), 1);
    provider.fail.store(true, Ordering::Relaxed);
    let offline = service
        .presentation_many_with_policy(
            &SourceId::modrinth(),
            vec![query.clone()],
            &cancel,
            CachePolicy::Revalidate,
        )
        .await
        .remove(0)
        .unwrap();
    assert!(offline.from_cache);
    assert_eq!(offline.value, current.value);
    assert_eq!(provider.calls(), 2);
    provider.fail.store(false, Ordering::Relaxed);
    provider.missing.store(true, Ordering::Relaxed);
    let missing = service
        .presentation_many_with_policy(
            &SourceId::modrinth(),
            vec![query],
            &cancel,
            CachePolicy::Revalidate,
        )
        .await
        .remove(0)
        .unwrap();
    assert_eq!(missing.value, ProviderValue::NotFound);
    cache.close().await;
}

#[path = "plans_tests.rs"]
mod plans_tests;
#[path = "protocol_tests.rs"]
mod protocol_tests;
#[path = "review_tests.rs"]
mod review_tests;
#[path = "search_tests.rs"]
mod search_tests;

#[tokio::test]
async fn installed_compatibility_uses_cached_exact_bytes_without_network_or_name_guesses()
 {
    let (_temp, cache) = cache().await;
    let provider = MockProvider::new(100, None);
    provider.fail.store(true, Ordering::Relaxed);
    let service =
        ResourceService::new(cache.clone(), vec![provider.clone()]).unwrap();
    let mut release = crate::resources::providers::legacy::release(
        serde_json::from_str(fixtures::MR_IRIS).unwrap(),
    )
    .unwrap()
    .release;
    release.compatibility = ReleaseCompatibility {
        game_versions: vec!["1.21.1".into()],
        loaders: vec!["neoforge".into()],
    };
    let artifact = release
        .artifacts
        .iter()
        .find(|artifact| artifact.role == ArtifactRole::Primary)
        .unwrap()
        .clone();
    let hash = artifact
        .integrity
        .hashes
        .iter()
        .find(|hash| hash.algorithm == HashAlgorithm::Sha1)
        .unwrap()
        .clone();
    let lookup = ProviderQuery::IdentifyExport {
        integrity: ArtifactIntegrity {
            size: None,
            hashes: vec![hash],
        },
        fingerprint: None,
    };
    let key =
        CacheKey::new("mock-v1".into(), SourceId::modrinth(), &lookup).unwrap();
    cache
        .put(
            &key,
            &ProviderValue::Identified(vec![ReleaseDocument {
                release: release.clone(),
                changelog: None,
                protocol: None,
            }]),
            -1,
            cache.generation().await,
            chrono::Utc::now().timestamp_micros(),
        )
        .await;
    let release_query = ProviderQuery::Release {
        project_id: Some(release.reference.project.project_id.clone()),
        release_id: release.reference.release_id.clone(),
        slug: None,
    };
    let release_key =
        CacheKey::new("mock-v1".into(), SourceId::modrinth(), &release_query)
            .unwrap();
    cache
        .put(
            &release_key,
            &ProviderValue::Release(ReleaseDocument {
                release: release.clone(),
                changelog: None,
                protocol: None,
            }),
            -1,
            cache.generation().await,
            chrono::Utc::now().timestamp_micros(),
        )
        .await;
    let mut wrong_size = artifact.integrity.clone();
    wrong_size.size = wrong_size.size.map(|size| size + 1);
    let mut wrong_hash = artifact.integrity.clone();
    wrong_hash.hashes = vec![ContentHash {
        algorithm: HashAlgorithm::Sha1,
        value: "f".repeat(40),
    }];
    let declarations = super::installed::cached_file_compatibility(
        &service,
        &[
            (artifact.integrity.clone(), vec![]),
            (wrong_size, vec![]),
            (wrong_hash, vec![release.reference.clone()]),
        ],
    )
    .await;
    assert_eq!(declarations[0], Some(release.compatibility.clone()));
    assert!(declarations[1].is_none());
    assert!(declarations[2].is_none());
    assert_eq!(provider.calls(), 0);
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    for schema in [
        "CREATE TABLE instance_content_entries(id TEXT, file_id TEXT, instance_id TEXT)",
        "CREATE TABLE instance_files(id TEXT, sha1 TEXT, size INTEGER, local_mod_data TEXT, missing INTEGER)",
        "CREATE TABLE resource_installed_artifacts(entry_id TEXT, artifact TEXT)",
    ] {
        sqlx::query(schema).execute(&pool).await.unwrap();
    }
    let local = json!({"metadata_version": 2, "mod_id": "example", "minecraft_version": "[1.21,1.21.1)", "loader": "neoforge"}).to_string();
    let sha1 = artifact
        .integrity
        .hashes
        .iter()
        .find(|hash| hash.algorithm == HashAlgorithm::Sha1)
        .unwrap()
        .value
        .clone();
    let mut items = Vec::new();
    for (id, hash, local) in [
        ("missing", sha1.clone(), None),
        ("incorrect", sha1, Some(local.clone())),
        ("modified", "f".repeat(40), Some(local)),
    ] {
        sqlx::query(
            "INSERT INTO instance_content_entries VALUES (?, ?, 'instance')",
        )
        .bind(id)
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO instance_files VALUES (?, ?, ?, ?, 0)")
            .bind(id)
            .bind(hash)
            .bind(artifact.integrity.size.unwrap() as i64)
            .bind(local)
            .execute(&pool)
            .await
            .unwrap();
        items.push(crate::state::InstanceContentSnapshotItem {
            artifact_ref: None,
            resource_id: None,
            pack_resource_id: None,
            compatibility: None,
            file_id: Some(id.into()),
            entry_id: Some(id.into()),
            member_id: None,
            ownership_kind: crate::state::ContentOwnershipKind::UserAdded,
            materialization_state:
                crate::state::PackMemberMaterializationState::Present,
            override_kind: crate::state::PackMemberOverrideKind::None,
            expected_relative_path: format!("mods/{id}.jar"),
            required: false,
            project_type: crate::state::ProjectType::Mod,
            provider: None,
            provider_project_id: None,
            provider_release_id: None,
            content: None,
            capabilities: Default::default(),
            dependency: None,
        });
    }
    super::installed::attach_compatibility(
        &pool,
        "instance",
        "1.21.1",
        "neoforge",
        &mut items,
        Some(&service),
    )
    .await
    .unwrap();
    assert_eq!(
        items[0].compatibility.as_ref().unwrap().game_version,
        CompatibilityState::Compatible
    );
    assert_eq!(
        items[1].compatibility.as_ref().unwrap().game_version,
        CompatibilityState::Compatible
    );
    assert_eq!(
        items[2].compatibility.as_ref().unwrap().game_version,
        CompatibilityState::Incompatible
    );
    pool.close().await;
    let mut attachment = release.clone();
    for artifact in &mut attachment.artifacts {
        artifact.role = ArtifactRole::Attachment;
    }
    cache
        .put(
            &key,
            &ProviderValue::Identified(vec![ReleaseDocument {
                release: attachment,
                changelog: None,
                protocol: None,
            }]),
            -1,
            cache.generation().await,
            chrono::Utc::now().timestamp_micros(),
        )
        .await;
    assert!(
        super::installed::cached_file_compatibility(
            &service,
            &[(artifact.integrity.clone(), vec![])]
        )
        .await[0]
            .is_none()
    );
    let mut other = release.clone();
    other.reference.release_id = SourceReleaseId::new("otherver").unwrap();
    for artifact in &mut other.artifacts {
        artifact.reference.release = other.reference.clone();
    }
    cache
        .put(
            &key,
            &ProviderValue::Identified(vec![
                ReleaseDocument {
                    release: release.clone(),
                    changelog: None,
                    protocol: None,
                },
                ReleaseDocument {
                    release: other,
                    changelog: None,
                    protocol: None,
                },
            ]),
            -1,
            cache.generation().await,
            chrono::Utc::now().timestamp_micros(),
        )
        .await;
    assert!(
        super::installed::cached_file_compatibility(
            &service,
            &[(artifact.integrity, vec![])]
        )
        .await[0]
            .is_none()
    );
    assert_eq!(provider.calls(), 0);
    cache.close().await;
}

#[tokio::test]
async fn changelog_catalog_pages_omit_bodies_and_reject_foreign_releases() {
    let release: Value = serde_json::from_str(fixtures::MR_IRIS).unwrap();
    let project = release["project_id"].as_str().unwrap().to_string();
    let transport = FixtureTransport::new(move |_, request| {
        assert!(
            request
                .query
                .contains(&("include_changelog".into(), "false".into()))
        );
        let mut second = release.clone();
        second["id"] = json!("next1234");
        Ok(json!([release, second]))
    });
    let (_temp, cache) = cache().await;
    let service = ResourceService::new(
        cache.clone(),
        vec![Arc::new(ModrinthProvider::new(transport))],
    )
    .unwrap();
    let target = ipc::ResourceTarget {
        game_versions: vec![],
        loaders: vec![],
        channels: vec![],
    };
    let reference = ResourceRef::Source {
        project: SourceProjectRef {
            source: SourceId::modrinth(),
            project_id: SourceProjectId::new(&project).unwrap(),
        },
    };
    let page = service
        .releases(
            &reference,
            target.clone(),
            None,
            RequestId::new("logs-page").unwrap(),
            None,
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(page.entries.len(), 2);
    assert!(!page.has_more);
    assert!(matches!(
        service
            .releases(
                &ResourceRef::Source {
                    project: SourceProjectRef {
                        source: SourceId::modrinth(),
                        project_id: SourceProjectId::new("wrong123").unwrap()
                    }
                },
                target,
                None,
                RequestId::new("wrong-page").unwrap(),
                None,
                &CancellationToken::new()
            )
            .await,
        Err(ResourceError::InvalidResponse(_))
    ));
    cache.close().await;
}

#[tokio::test]
async fn changelog_modrinth_uses_exact_release_notes_and_rejects_other_projects()
 {
    for body in ["## Changes\nFixed **this release**", ""] {
        let mut release: Value =
            serde_json::from_str(fixtures::MR_IRIS).unwrap();
        let project = release["project_id"].as_str().unwrap().to_string();
        let id = release["id"].as_str().unwrap().to_string();
        release["changelog"] = json!(body);
        let expected_id = id.clone();
        let transport = FixtureTransport::new(move |_, request| {
            assert_eq!(request.path, format!("/version/{expected_id}"));
            Ok(release.clone())
        });
        let provider = ModrinthProvider::new(transport);
        let query = ProviderQuery::Changelog {
            project_id: SourceProjectId::new(project).unwrap(),
            release_id: SourceReleaseId::new(&id).unwrap(),
            slug: None,
        };
        let results = provider.fetch_many(&[query]).await;
        let ProviderValue::Changelog(text) = results[0].as_ref().unwrap()
        else {
            panic!("wrong changelog");
        };
        assert_eq!(text.source, SourceId::modrinth());
        assert_eq!(text.format, TextFormat::Markdown);
        assert_eq!(text.body, body);
        let wrong = ProviderQuery::Changelog {
            project_id: SourceProjectId::new("wrong123").unwrap(),
            release_id: SourceReleaseId::new(id).unwrap(),
            slug: None,
        };
        assert!(matches!(
            provider.fetch_many(&[wrong]).await[0],
            Err(ResourceError::InvalidResponse(_))
        ));
    }
}

#[tokio::test]
async fn changelog_curseforge_uses_release_html_and_rejects_other_projects() {
    for body in ["<p>Fixed <strong>this file</strong></p>", ""] {
        let file: Value = serde_json::from_str(fixtures::CF_IRIS).unwrap();
        let project = file["modId"].as_u64().unwrap().to_string();
        let id = file["id"].as_u64().unwrap().to_string();
        let path = format!("/v1/mods/{project}/files/{id}");
        let transport = FixtureTransport::new(move |_, request| {
            if request.path.ends_with("/changelog") {
                assert_eq!(request.path, format!("{path}/changelog"));
                Ok(json!({"data": body}))
            } else {
                Ok(json!({"data": file}))
            }
        });
        let provider = CurseForgeProvider::new(transport.clone());
        let query = ProviderQuery::Changelog {
            project_id: SourceProjectId::new(project).unwrap(),
            release_id: SourceReleaseId::new(&id).unwrap(),
            slug: None,
        };
        let results = provider.fetch_many(&[query]).await;
        let ProviderValue::Changelog(text) = results[0].as_ref().unwrap()
        else {
            panic!("wrong changelog");
        };
        assert_eq!(text.source, SourceId::curseforge());
        assert_eq!(text.format, TextFormat::Html);
        assert_eq!(text.body, body);
        let count = transport.requests.lock().unwrap().len();
        let wrong = ProviderQuery::Changelog {
            project_id: SourceProjectId::new("99999").unwrap(),
            release_id: SourceReleaseId::new(id).unwrap(),
            slug: None,
        };
        assert!(matches!(
            provider.fetch_many(&[wrong]).await[0],
            Err(ResourceError::InvalidResponse(_))
        ));
        assert_eq!(transport.requests.lock().unwrap().len(), count + 1);
    }
}

#[tokio::test]
async fn legacy_cache_import_is_offline_resumable_conservative_and_clear_safe()
{
    let main = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!().run(&main).await.unwrap();
    let (_directory, cache) = cache().await;
    let now = chrono::Utc::now().timestamp();
    let legacy_ttl = 100 * 365 * 24 * 60 * 60;
    let project = json!({
        "id": "abcdefgh", "slug": "locator", "project_type": "mod", "title": "Legacy",
        "description": "Summary", "body": "Body"
    });
    let cf_project = json!({
        "id": 456, "gameId": 432, "name": "CF Legacy", "slug": "legacy", "summary": "Summary",
        "status": 4, "downloadCount": 100, "isFeatured": false, "primaryCategoryId": 6,
        "categories": [], "classId": 6, "authors": [], "logo": null,
        "mainFileId": 5425378, "latestFiles": [{
            "id": 5425378, "modId": 456, "fileName": "mod.jar", "fileLength": 100,
            "gameId": 432, "displayName": "Mod", "fileStatus": 4,
            "fileDate": "2020-01-03T00:00:00Z", "downloadCount": 100,
            "hashes": [{"algo": 1, "value": "a".repeat(40)}], "releaseType": 1,
            "fileFingerprint": 123, "isAvailable": true,
            "downloadUrl": "https://example.com/mod.jar"
        }], "dateCreated": "2020-01-01T00:00:00Z", "dateModified": "2020-01-02T00:00:00Z",
        "dateReleased": "2020-01-03T00:00:00Z", "isAvailable": true
    });
    for (id, kind, data, age) in [
        ("abcdefgh", "project", project.to_string(), 0),
        ("456", "curseforge_project", cf_project.to_string(), 0),
        ("bad-json", "project", "not json".into(), 0),
        ("wrong-id", "project", project.to_string(), 0),
        ("minecraft", "minecraft_manifest", "{}".into(), 0),
        ("stale123", "project", json!({"id":"stale123","project_type":"mod","title":"Stale","description":"Summary"}).to_string(), 3600),
        ("G5dd9TM4", "version", fixtures::MR_IRIS.into(), 0),
        ("large123", "project", json!({"id":"large123", "project_type":"mod", "title":"Large", "description":"", "body":"a".repeat(1024 * 1024 + 1)}).to_string(), 0),
    ] {
        sqlx::query("INSERT INTO cache(id, data_type, data, expires) VALUES (?, ?, ?, ?)")
            .bind(id).bind(kind).bind(data).bind(now + legacy_ttl - age).execute(&main).await.unwrap();
    }
    // A full extra page verifies progress beyond the import's memory bound.
    for index in 0..101 {
        let id = format!("{index:08}");
        let mut version: Value =
            serde_json::from_str(fixtures::MR_IRIS).unwrap();
        version["id"] = json!(id);
        sqlx::query("INSERT INTO cache(id, data_type, data, expires) VALUES (?, 'version', ?, ?)")
            .bind(id).bind(version.to_string()).bind(now + legacy_ttl).execute(&main).await.unwrap();
    }
    let key = CacheKey::new(
        "modrinth-v2-dates".into(),
        SourceId::modrinth(),
        &project_query("abcdefgh"),
    )
    .unwrap();
    let current =
        project_value(SourceId::modrinth(), "abcdefgh", "Fetched recently");
    cache
        .put(
            &key,
            &current,
            1800,
            cache.generation().await,
            chrono::Utc::now().timestamp_micros(),
        )
        .await;
    cache.import_legacy(&main).await.unwrap();
    assert_eq!(cache.get(&key).await, Some(current.clone()));
    assert!(
        cache
            .get(
                &CacheKey::new(
                    "modrinth-v2-dates".into(),
                    SourceId::modrinth(),
                    &project_query("large123")
                )
                .unwrap()
            )
            .await
            .is_none()
    );
    assert!(
        cache
            .get(
                &CacheKey::new(
                    "modrinth-v2-dates".into(),
                    SourceId::modrinth(),
                    &project_query("stale123")
                )
                .unwrap()
            )
            .await
            .is_none()
    );
    let description_query = ProviderQuery::Description {
        id: SourceProjectId::new("abcdefgh").unwrap(),
        slug: None,
    };
    assert!(
        matches!(cache.get(&CacheKey::new("modrinth-v2-dates".into(), SourceId::modrinth(), &description_query).unwrap()).await, Some(ProviderValue::Description(text)) if text.body == "Body")
    );
    let cf_query = ProviderQuery::Release {
        project_id: Some(SourceProjectId::new("456").unwrap()),
        release_id: SourceReleaseId::new("5425378").unwrap(),
        slug: None,
    };
    assert!(matches!(
        cache
            .get(
                &CacheKey::new(
                    "curseforge-v1-dates".into(),
                    SourceId::curseforge(),
                    &cf_query
                )
                .unwrap()
            )
            .await,
        Some(ProviderValue::Release(_))
    ));
    let last_query = ProviderQuery::Release {
        project_id: None,
        release_id: SourceReleaseId::new("00000100").unwrap(),
        slug: None,
    };
    assert!(matches!(
        cache
            .get(
                &CacheKey::new(
                    "modrinth-v2-dates".into(),
                    SourceId::modrinth(),
                    &last_query
                )
                .unwrap()
            )
            .await,
        Some(ProviderValue::Release(_))
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM resource_source_projects"
        )
        .fetch_one(&main)
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM cache")
            .fetch_one(&main)
            .await
            .unwrap(),
        109
    );
    let count = cache.info().await.unwrap().entry_count;
    cache.import_legacy(&main).await.unwrap();
    assert_eq!(cache.info().await.unwrap().entry_count, count);
    let probe = sqlx::sqlite::SqlitePoolOptions::new()
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new().filename(cache.path()),
        )
        .await
        .unwrap();
    // Model interruption after cache writes but before the durable checkpoint.
    sqlx::query(
        "UPDATE resource_legacy_import SET completed = 0, last_rowid = 0",
    )
    .execute(&probe)
    .await
    .unwrap();
    let path = cache.path().to_path_buf();
    probe.close().await;
    cache.close().await;
    let reopened = ResourceCache::open(path).await;
    reopened.import_legacy(&main).await.unwrap();
    assert_eq!(reopened.info().await.unwrap().entry_count, count);
    assert_eq!(reopened.get(&key).await, Some(current));
    let probe = sqlx::sqlite::SqlitePoolOptions::new()
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new().filename(reopened.path()),
        )
        .await
        .unwrap();
    sqlx::query(
        "UPDATE resource_legacy_import SET completed = 0, last_rowid = 0",
    )
    .execute(&probe)
    .await
    .unwrap();
    reopened.clear().await.unwrap();
    reopened.import_legacy(&main).await.unwrap();
    assert_eq!(reopened.info().await.unwrap().entry_count, 0);
    probe.close().await;
    reopened.close().await;
}

fn project_query(id: &str) -> ProviderQuery {
    ProviderQuery::Project {
        id: SourceProjectId::new(id).unwrap(),
        slug: None,
    }
}

fn project_value(source: SourceId, id: &str, title: &str) -> ProviderValue {
    ProviderValue::Project(ProjectDocument {
        protocol: None,
        metadata: SourceProjectMetadata {
            environments: vec![],
            categories: vec![],
            game_versions: vec![],
            loaders: vec![],
            gallery: vec![],
            project: SourceProjectRef {
                source: source.clone(),
                project_id: SourceProjectId::new(id).unwrap(),
            },
            slug: Some("locator".into()),
            title: title.into(),
            summary: "summary".into(),
            icon_url: None,
            types: vec![ResourceType::Mod],
            authors: vec![],
            license: None,
            downloads: None,
            followers: None,
            created_at: None,
            project_published_at: None,
            updated_at: None,
            first_published_at: None,
            latest_published_at: None,
        },
        description: ResourceText {
            source,
            format: TextFormat::Plain,
            body: "description".into(),
        },
    })
}

async fn cache() -> (tempfile::TempDir, Arc<ResourceCache>) {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("caches/resources/beta/resource-cache.db");
    let cache = Arc::new(ResourceCache::open(path).await);
    assert!(cache.available());
    (temp, cache)
}

struct MockProvider {
    source: SourceId,
    batches: Mutex<Vec<Vec<ProviderQuery>>>,
    in_flight: AtomicUsize,
    max_in_flight: AtomicUsize,
    canceled: AtomicUsize,
    gate: Option<Arc<Semaphore>>,
    started: Notify,
    fail: AtomicBool,
    missing: AtomicBool,
    limit: usize,
}

impl MockProvider {
    fn new(limit: usize, gate: Option<Arc<Semaphore>>) -> Arc<Self> {
        Arc::new(Self {
            source: SourceId::modrinth(),
            batches: Mutex::new(vec![]),
            in_flight: AtomicUsize::new(0),
            max_in_flight: AtomicUsize::new(0),
            canceled: AtomicUsize::new(0),
            gate,
            started: Notify::new(),
            fail: AtomicBool::new(false),
            missing: AtomicBool::new(false),
            limit,
        })
    }
    fn calls(&self) -> usize {
        self.batches.lock().unwrap().len()
    }
}

struct Activity<'a> {
    provider: &'a MockProvider,
    completed: bool,
}
impl Drop for Activity<'_> {
    fn drop(&mut self) {
        self.provider.in_flight.fetch_sub(1, Ordering::AcqRel);
        if !self.completed {
            self.provider.canceled.fetch_add(1, Ordering::AcqRel);
        }
    }
}

#[async_trait]
impl ResourceProvider for MockProvider {
    fn source(&self) -> ResourceSource {
        ResourceSource {
            id: self.source.clone(),
            available: true,
            types: vec![ResourceType::Mod],
            capabilities: vec![
                SourceCapability::Project,
                SourceCapability::IdentifyHash,
            ],
            supported_search_sorts: vec![],
        }
    }
    fn namespace(&self) -> String {
        "mock-v1".into()
    }
    fn batch_limit(&self, _: &ProviderQuery) -> usize {
        self.limit
    }
    async fn fetch_many(
        &self,
        queries: &[ProviderQuery],
    ) -> Vec<ResourceResult<ProviderValue>> {
        self.batches.lock().unwrap().push(queries.to_vec());
        let active = self.in_flight.fetch_add(1, Ordering::AcqRel) + 1;
        self.max_in_flight.fetch_max(active, Ordering::AcqRel);
        let mut activity = Activity {
            provider: self,
            completed: false,
        };
        self.started.notify_waiters();
        if let Some(gate) = &self.gate {
            gate.acquire().await.unwrap().forget();
        }
        activity.completed = true;
        queries
            .iter()
            .map(|query| {
                if self.fail.load(Ordering::Acquire) {
                    return Err(ResourceError::Network { status: Some(503) });
                }
                if self.missing.load(Ordering::Acquire) {
                    return Ok(ProviderValue::NotFound);
                }
                match query {
                    ProviderQuery::Project { id, .. } => Ok(project_value(
                        self.source.clone(),
                        id.as_str(),
                        "current",
                    )),
                    ProviderQuery::Identify { .. } => {
                        Ok(ProviderValue::Matches(vec![]))
                    }
                    _ => {
                        Err(ResourceError::Unsupported("mock operation".into()))
                    }
                }
            })
            .collect()
    }
}

async fn wait_until(mut predicate: impl FnMut() -> bool) {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while !predicate() {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn cache_survives_close_and_reopen_with_a_separate_wal_database() {
    let (_temp, first) = cache().await;
    let key = CacheKey::new(
        "v1".into(),
        SourceId::modrinth(),
        &project_query("project"),
    )
    .unwrap();
    let value = project_value(SourceId::modrinth(), "project", "persistent");
    first
        .put(&key, &value, 1800, first.generation().await, 1)
        .await;
    assert!(first.info().await.unwrap().size_bytes > 0);
    let path = first.path().to_path_buf();
    first.close().await;
    let reopened = ResourceCache::open(path).await;
    assert_eq!(reopened.get(&key).await, Some(value));
    let probe = sqlx::sqlite::SqlitePoolOptions::new()
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new().filename(reopened.path()),
        )
        .await
        .unwrap();
    let mode: String = sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(&probe)
        .await
        .unwrap();
    assert_eq!(mode, "wal");
    assert_eq!(reopened.info().await.unwrap().schema_version, 2);
    probe.close().await;
    reopened.close().await;
}

#[tokio::test]
async fn cache_expiry_malformed_json_and_unknown_payload_version_are_misses() {
    let (_temp, cache) = cache().await;
    let key = CacheKey::new(
        "v1".into(),
        SourceId::modrinth(),
        &project_query("project"),
    )
    .unwrap();
    cache
        .put(
            &key,
            &ProviderValue::NotFound,
            -1,
            cache.generation().await,
            1,
        )
        .await;
    assert!(cache.get(&key).await.is_none());
    cache
        .put(
            &key,
            &ProviderValue::NotFound,
            100,
            cache.generation().await,
            2,
        )
        .await;
    let probe = sqlx::sqlite::SqlitePoolOptions::new()
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new().filename(cache.path()),
        )
        .await
        .unwrap();
    sqlx::query("UPDATE resource_cache_entries SET payload = 'broken JSON'")
        .execute(&probe)
        .await
        .unwrap();
    assert!(cache.get(&key).await.is_none());
    cache
        .put(
            &key,
            &ProviderValue::NotFound,
            100,
            cache.generation().await,
            3,
        )
        .await;
    sqlx::query("UPDATE resource_cache_entries SET value_version = 99")
        .execute(&probe)
        .await
        .unwrap();
    assert!(cache.get(&key).await.is_none());
    probe.close().await;
    cache.close().await;
}

#[tokio::test]
async fn clearing_resource_cache_releases_database_file_space() {
    let (_temp, cache) = cache().await;
    let probe = sqlx::SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new().filename(cache.path()),
    )
    .await
    .unwrap();
    sqlx::query("INSERT INTO resource_cache_entries(namespace, source, query_key, value_version, payload, retrieved_at, expires_at) VALUES ('v1','modrinth','large',1,randomblob(2097152),0,99999999999)").execute(&probe).await.unwrap();
    let before = cache.info().await.unwrap().size_bytes;
    cache.clear().await.unwrap();
    cache.compact().await.unwrap();
    let after = cache.info().await.unwrap();
    assert_eq!(after.entry_count, 0);
    assert!(before > after.size_bytes + 1024 * 1024);
    probe.close().await;
    cache.close().await;
}

#[tokio::test]
async fn cache_clear_rejects_old_generation_and_late_older_responses() {
    let (_temp, cache) = cache().await;
    let key = CacheKey::new(
        "v1".into(),
        SourceId::modrinth(),
        &project_query("project"),
    )
    .unwrap();
    let old_generation = cache.generation().await;
    cache.clear().await.unwrap();
    cache
        .put(&key, &ProviderValue::NotFound, 100, old_generation, 1)
        .await;
    assert!(cache.get(&key).await.is_none());
    let new = project_value(SourceId::modrinth(), "project", "new");
    cache
        .put(&key, &new, 100, cache.generation().await, 3)
        .await;
    cache
        .put(
            &key,
            &ProviderValue::NotFound,
            100,
            cache.generation().await,
            2,
        )
        .await;
    assert_eq!(cache.get(&key).await, Some(new));
    cache.close().await;
}

#[tokio::test]
async fn corrupt_cache_rebuilds_and_creation_failure_degrades_to_live_queries()
{
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("resource-cache.db");
    tokio::fs::write(&path, b"not a SQLite database")
        .await
        .unwrap();
    let rebuilt = ResourceCache::open(path.clone()).await;
    assert!(rebuilt.available());
    assert_eq!(rebuilt.info().await.unwrap().entry_count, 0);
    rebuilt.close().await;
    let parent = temp.path().join("not-a-directory");
    tokio::fs::write(&parent, b"file").await.unwrap();
    let disabled =
        Arc::new(ResourceCache::open(parent.join("resource-cache.db")).await);
    assert!(!disabled.available());
    let provider = MockProvider::new(100, None);
    let service =
        ResourceService::new(disabled, vec![provider.clone()]).unwrap();
    assert!(
        service
            .query(
                &SourceId::modrinth(),
                project_query("project"),
                CachePolicy::UseFresh,
                &CancellationToken::new()
            )
            .await
            .is_ok()
    );
    assert_eq!(provider.calls(), 1);
}

#[tokio::test]
async fn unknown_cache_migration_is_preserved_and_disabled() {
    let (_temp, cache) = cache().await;
    let path = cache.path().to_path_buf();
    let probe = sqlx::sqlite::SqlitePoolOptions::new()
        .connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(&path))
        .await
        .unwrap();
    sqlx::query("INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time) VALUES (20271011090000, 'future schema', 1, x'00', 1)").execute(&probe).await.unwrap();
    probe.close().await;
    cache.close().await;
    let next = ResourceCache::open(path.clone()).await;
    assert!(!next.available());
    let probe = sqlx::sqlite::SqlitePoolOptions::new()
        .connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(path))
        .await
        .unwrap();
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM _sqlx_migrations WHERE version = 20271011090000)").fetch_one(&probe).await.unwrap();
    assert!(exists);
    probe.close().await;
}

#[tokio::test]
async fn service_uses_hot_cache_and_force_refresh_ignores_positive_and_negative_cache()
 {
    let (_temp, cache) = cache().await;
    let provider = MockProvider::new(100, None);
    let service =
        ResourceService::new(cache.clone(), vec![provider.clone()]).unwrap();
    let token = CancellationToken::new();
    let query = project_query("project");
    let source = SourceId::modrinth();
    assert!(
        !service
            .query(&source, query.clone(), CachePolicy::UseFresh, &token)
            .await
            .unwrap()
            .from_cache
    );
    assert!(
        service
            .query(&source, query.clone(), CachePolicy::UseFresh, &token)
            .await
            .unwrap()
            .from_cache
    );
    assert_eq!(provider.calls(), 1);
    provider.fail.store(true, Ordering::Release);
    assert!(matches!(
        service
            .query(&source, query.clone(), CachePolicy::ForceRefresh, &token)
            .await,
        Err(ResourceError::Network { .. })
    ));
    assert_eq!(provider.calls(), 2);
    provider.fail.store(false, Ordering::Release);
    provider.missing.store(true, Ordering::Release);
    assert_eq!(
        service
            .query(&source, query.clone(), CachePolicy::ForceRefresh, &token)
            .await
            .unwrap()
            .value,
        ProviderValue::NotFound
    );
    provider.missing.store(false, Ordering::Release);
    assert_eq!(
        service
            .query(&source, query.clone(), CachePolicy::UseFresh, &token)
            .await
            .unwrap()
            .value,
        ProviderValue::NotFound
    );
    assert!(matches!(
        service
            .query(&source, query, CachePolicy::ForceRefresh, &token)
            .await
            .unwrap()
            .value,
        ProviderValue::Project(_)
    ));
    cache.close().await;
}

#[tokio::test]
async fn batch_queries_deduplicate_keys_preserve_order_and_limit_source_concurrency()
 {
    let (_temp, cache) = cache().await;
    let gate = Arc::new(Semaphore::new(0));
    let provider = MockProvider::new(50, Some(gate.clone()));
    let service =
        ResourceService::new(cache.clone(), vec![provider.clone()]).unwrap();
    let mut queries: Vec<_> =
        (0..251).map(|id| project_query(&id.to_string())).collect();
    queries.push(project_query("0"));
    let task = tokio::spawn({
        let service = service.clone();
        async move {
            service
                .query_many(
                    &SourceId::modrinth(),
                    queries,
                    CachePolicy::UseFresh,
                    &CancellationToken::new(),
                )
                .await
        }
    });
    wait_until(|| provider.calls() == 3).await;
    assert_eq!(provider.max_in_flight.load(Ordering::Acquire), 3);
    gate.add_permits(6);
    let results = task.await.unwrap();
    assert_eq!(provider.calls(), 6);
    assert!(
        provider
            .batches
            .lock()
            .unwrap()
            .iter()
            .all(|batch| batch.len() <= 50)
    );
    assert_eq!(results.len(), 252);
    assert_eq!(results[0], results[251]);
    for (id, result) in results.into_iter().take(251).enumerate() {
        let ProviderValue::Project(document) = result.unwrap().value else {
            panic!("wrong result");
        };
        assert_eq!(
            document.metadata.project.project_id.as_str(),
            id.to_string()
        );
    }
    cache.close().await;
}

#[tokio::test]
async fn canceling_one_subscriber_keeps_shared_request_alive() {
    let (_temp, cache) = cache().await;
    let gate = Arc::new(Semaphore::new(0));
    let provider = MockProvider::new(100, Some(gate.clone()));
    let service =
        ResourceService::new(cache.clone(), vec![provider.clone()]).unwrap();
    let first_cancel = CancellationToken::new();
    let first = tokio::spawn({
        let service = service.clone();
        let token = first_cancel.clone();
        async move {
            service
                .query(
                    &SourceId::modrinth(),
                    project_query("shared"),
                    CachePolicy::UseFresh,
                    &token,
                )
                .await
        }
    });
    wait_until(|| provider.calls() == 1).await;
    let second = tokio::spawn({
        let service = service.clone();
        async move {
            service
                .query(
                    &SourceId::modrinth(),
                    project_query("shared"),
                    CachePolicy::UseFresh,
                    &CancellationToken::new(),
                )
                .await
        }
    });
    wait_until(|| service.subscriber_count() == 2).await;
    first_cancel.cancel();
    assert_eq!(first.await.unwrap(), Err(ResourceError::Canceled));
    gate.add_permits(1);
    assert!(second.await.unwrap().is_ok());
    assert_eq!(provider.calls(), 1);
    assert_eq!(provider.canceled.load(Ordering::Acquire), 0);
    cache.close().await;
}

#[tokio::test]
async fn last_subscriber_cancellation_stops_work_and_does_not_fill_cache() {
    let (_temp, cache) = cache().await;
    let provider = MockProvider::new(100, Some(Arc::new(Semaphore::new(0))));
    let service =
        ResourceService::new(cache.clone(), vec![provider.clone()]).unwrap();
    let token = CancellationToken::new();
    let task = tokio::spawn({
        let service = service.clone();
        let token = token.clone();
        async move {
            service
                .query(
                    &SourceId::modrinth(),
                    project_query("cancel"),
                    CachePolicy::ForceRefresh,
                    &token,
                )
                .await
        }
    });
    wait_until(|| provider.calls() == 1).await;
    token.cancel();
    assert_eq!(task.await.unwrap(), Err(ResourceError::Canceled));
    wait_until(|| provider.canceled.load(Ordering::Acquire) == 1).await;
    assert_eq!(cache.info().await.unwrap().entry_count, 0);
    cache.close().await;
}

type ResponseFactory =
    dyn Fn(&SourceId, &ProtocolRequest) -> ResourceResult<Value> + Send + Sync;
struct FixtureTransport {
    requests: Mutex<Vec<ProtocolRequest>>,
    response: Box<ResponseFactory>,
}

impl FixtureTransport {
    fn new(
        response: impl Fn(&SourceId, &ProtocolRequest) -> ResourceResult<Value>
        + Send
        + Sync
        + 'static,
    ) -> Arc<Self> {
        Arc::new(Self {
            requests: Mutex::new(vec![]),
            response: Box::new(response),
        })
    }
}

#[async_trait]
impl ProtocolTransport for FixtureTransport {
    async fn request(
        &self,
        source: &SourceId,
        request: ProtocolRequest,
    ) -> ResourceResult<Value> {
        self.requests.lock().unwrap().push(request.clone());
        (self.response)(source, &request)
    }
}

struct Available(Arc<dyn ResourceProvider>);
#[tokio::test]
async fn export_identification_reuses_fresh_multifile_release_and_rejects_fingerprint_collision()
 {
    let mr = FixtureTransport::new(|_, request| {
        assert_eq!(request.path, "/version_files");
        let release: Value = serde_json::from_str(fixtures::MR_IRIS).unwrap();
        Ok(json!({"574992b5760f3bf2f9aa0b70984f19fb1952b748": release}))
    });
    let provider = ModrinthProvider::new(mr.clone());
    let results = provider
        .fetch_many(&[ProviderQuery::IdentifyExport {
            integrity: ArtifactIntegrity {
                size: Some(2684113),
                hashes: vec![ContentHash {
                    algorithm: HashAlgorithm::Sha1,
                    value: "574992b5760f3bf2f9aa0b70984f19fb1952b748".into(),
                }],
            },
            fingerprint: None,
        }])
        .await;
    let ProviderValue::Identified(documents) = results[0].as_ref().unwrap()
    else {
        panic!("wrong export result");
    };
    assert_eq!(documents.len(), 1);
    assert!(documents[0].protocol.is_some());
    assert_eq!(mr.requests.lock().unwrap().len(), 1);
    let cf = FixtureTransport::new(|_, request| {
        assert_eq!(request.path, "/v1/fingerprints/432");
        Ok(
            json!({"data":{"exactMatches":[{"id":455508,"file": serde_json::from_str::<Value>(fixtures::CF_IRIS).unwrap()}]}}),
        )
    });
    let (_temp, cache) = cache().await;
    let service = ResourceService::new(
        cache,
        vec![Arc::new(Available(Arc::new(CurseForgeProvider::new(cf))))],
    )
    .unwrap();
    let mut draft = crate::resources::export::refresh::ExportDraft::new(
        ResourceExportScan {
            session_id: SessionId::new("fixture").unwrap(),
            instance_id: "instance".into(),
            instance_revision: 1,
            files: vec![],
            categories: vec![],
            excluded_paths: vec![],
            directories: vec![],
            requirements: ExportGameRequirements {
                game_version: "1.20.1".into(),
                loader: None,
                loader_version: None,
            },
        },
    );
    let file = ExportFileSnapshot {
        relative_path: "mods/iris.jar".into(),
        integrity: ArtifactIntegrity {
            size: Some(2684113),
            hashes: vec![ContentHash {
                algorithm: HashAlgorithm::Sha1,
                value: "b".repeat(40),
            }],
        },
        revision: 1,
        identifiable: true,
        modified_at: None,
        category_id: "mods".into(),
        category_selected: true,
    };
    let results = crate::resources::export::refresh::resolve_source(
        &service,
        SourceId::curseforge(),
        &[(file, 1967997505)],
        &std::collections::HashMap::new(),
        &mut draft,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert!(matches!(
        &results[0],
        ExportSourceResolution::NoMatch { .. }
    ));
    assert!(draft.projects.is_empty());
}
#[async_trait]
impl ResourceProvider for Available {
    fn source(&self) -> ResourceSource {
        let mut source = self.0.source();
        source.available = true;
        source
    }
    fn namespace(&self) -> String {
        self.0.namespace()
    }
    fn batch_limit(&self, query: &ProviderQuery) -> usize {
        self.0.batch_limit(query)
    }
    fn validate_query(&self, query: &ProviderQuery) -> ResourceResult<()> {
        self.0.validate_query(query)
    }
    async fn fetch_many(
        &self,
        queries: &[ProviderQuery],
    ) -> Vec<ResourceResult<ProviderValue>> {
        self.0.fetch_many(queries).await
    }
}

#[tokio::test]
async fn adapters_normalize_real_iris_and_balm_without_merging_different_bytes()
{
    let mr_transport = FixtureTransport::new(|_, _| {
        Ok(json!([
            serde_json::from_str::<Value>(fixtures::MR_IRIS).unwrap(),
            serde_json::from_str::<Value>(fixtures::MR_BALM_43).unwrap(),
            serde_json::from_str::<Value>(fixtures::MR_BALM_44).unwrap(),
        ]))
    });
    let cf_transport = FixtureTransport::new(|_, _| {
        Ok(json!({"data": [
            serde_json::from_str::<Value>(fixtures::CF_IRIS).unwrap(),
            serde_json::from_str::<Value>(fixtures::CF_BALM_43).unwrap(),
            serde_json::from_str::<Value>(fixtures::CF_BALM_44).unwrap(),
        ]}))
    });
    let release_query = |id: &str| ProviderQuery::Release {
        project_id: None,
        release_id: SourceReleaseId::new(id).unwrap(),
        slug: None,
    };
    let mr = ModrinthProvider::new(mr_transport);
    let cf = CurseForgeProvider::new(cf_transport);
    let mr_results = mr
        .fetch_many(&[
            release_query("G5dd9TM4"),
            release_query("1VlYVa3k"),
            release_query("dDN38ae7"),
        ])
        .await;
    let cf_results = cf
        .fetch_many(&[
            release_query("5425378"),
            release_query("8862257"),
            release_query("8969708"),
        ])
        .await;
    let artifact = |result: &ResourceResult<ProviderValue>| {
        let ProviderValue::Release(document) = result.as_ref().unwrap() else {
            panic!("wrong release");
        };
        document.release.artifacts[0].clone()
    };
    let mr_iris = artifact(&mr_results[0]);
    let cf_iris = artifact(&cf_results[0]);
    assert!(verify_artifact_pair(&mr_iris, &cf_iris).is_some());
    assert!(mr_iris.dependencies.is_empty());
    assert_eq!(
        cf_iris.dependencies[0].kind,
        ResourceDependencyKind::Required
    );
    assert!(
        verify_artifact_pair(
            &artifact(&mr_results[1]),
            &artifact(&cf_results[1])
        )
        .is_some()
    );
    assert!(
        verify_artifact_pair(
            &artifact(&mr_results[2]),
            &artifact(&cf_results[2])
        )
        .is_none()
    );
}

#[tokio::test]
async fn modrinth_identification_matches_the_actual_file_in_a_multifile_release()
 {
    let mut release: Value = serde_json::from_str(fixtures::MR_IRIS).unwrap();
    let mut attachment = release["files"][0].clone();
    attachment["id"] = json!("attachment");
    attachment["filename"] = json!("iris-sources.jar");
    attachment["hashes"]["sha1"] = json!("a".repeat(40));
    attachment["primary"] = json!(false);
    attachment["file_type"] = json!("sources-jar");
    release["files"]
        .as_array_mut()
        .unwrap()
        .insert(0, attachment);
    let hash = "574992b5760f3bf2f9aa0b70984f19fb1952b748";
    let transport = FixtureTransport::new(move |_, request| {
        assert_eq!(request.path, "/version_files");
        Ok(json!({hash: release}))
    });
    let provider = ModrinthProvider::new(transport.clone());
    let results = provider
        .fetch_many(&[ProviderQuery::Identify {
            integrity: ArtifactIntegrity {
                size: Some(2684113),
                hashes: vec![ContentHash {
                    algorithm: HashAlgorithm::Sha1,
                    value: hash.into(),
                }],
            },
            fingerprint: None,
        }])
        .await;
    let ProviderValue::Matches(matches) = results[0].as_ref().unwrap() else {
        panic!("wrong matches");
    };
    assert_eq!(matches.len(), 1);
    assert_eq!(
        matches[0].file_name.as_deref(),
        Some("iris-1.7.1+mc1.20.1.jar")
    );
    assert_eq!(matches[0].role, ArtifactRole::Primary);
    assert_eq!(transport.requests.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn fingerprint_collision_and_unknown_size_do_not_confirm_identity() {
    let file: Value = serde_json::from_str(fixtures::CF_IRIS).unwrap();
    let transport = FixtureTransport::new(move |_, request| {
        assert_eq!(request.path, "/v1/fingerprints/432");
        Ok(json!({"data": {"exactMatches": [{"id": 455508, "file": file}]}}))
    });
    let (_temp, cache) = cache().await;
    let service = ResourceService::new(
        cache.clone(),
        vec![Arc::new(Available(Arc::new(CurseForgeProvider::new(
            transport,
        ))))],
    )
    .unwrap();
    let local = LocalArtifactIdentity {
        integrity: ArtifactIntegrity {
            size: Some(2684113),
            hashes: vec![ContentHash {
                algorithm: HashAlgorithm::Sha1,
                value: "b".repeat(40),
            }],
        },
        fingerprint: Some(1967997505),
    };
    let result = service
        .identify_many(
            &[SourceId::curseforge()],
            &[local],
            CachePolicy::ForceRefresh,
            &CancellationToken::new(),
        )
        .await;
    assert!(result[0].matches.is_empty());
    assert_eq!(result[0].unresolved_sources, [SourceId::curseforge()]);
    assert!(result[0].failures.is_empty());
    let known = ArtifactIntegrity {
        size: Some(1),
        hashes: vec![ContentHash {
            algorithm: HashAlgorithm::Sha1,
            value: "a".repeat(40),
        }],
    };
    let mut unknown = known.clone();
    unknown.size = None;
    assert!(!exact_integrity(&known, &unknown));
    cache.close().await;
}

#[tokio::test]
async fn mcarchive_stable_uuid_uses_verified_slug_locator_and_unknown_size() {
    let transport = FixtureTransport::new(|_, request| {
        assert_eq!(request.path, "/mods/by_slug/modloader");
        assert!(request.fields.as_ref().unwrap().contains("uuid"));
        Ok(
            json!({"uuid":"project-uuid", "slug":"modloader", "name":"Modloader", "mod_versions":[{"uuid":"release-uuid", "name":"1.6.2", "game_versions":[{"name":"1.6.2"}], "files":[{"uuid":"file-uuid", "name":"ModLoader.zip", "sha256":"a".repeat(64), "archive_url":"https://b2.mcarchive.net/file/modloader.zip"}]}]}),
        )
    });
    let provider = McArchiveProvider::new(transport);
    let query = ProviderQuery::Release {
        project_id: Some(SourceProjectId::new("project-uuid").unwrap()),
        release_id: SourceReleaseId::new("release-uuid").unwrap(),
        slug: Some("modloader".into()),
    };
    let results = provider.fetch_many(&[query]).await;
    let ProviderValue::Release(document) = results[0].as_ref().unwrap() else {
        panic!("wrong release");
    };
    assert!(document.release.published_at.is_none());
    assert!(document.release.artifacts[0].integrity.size.is_none());
    assert_eq!(
        document.release.reference.project.project_id.as_str(),
        "project-uuid"
    );
    assert!(matches!(
        document.release.artifacts[0].acquisition,
        ArtifactAcquisition::PublicDownload { .. }
    ));
    let result = provider
        .fetch_many(&[ProviderQuery::Project {
            id: SourceProjectId::new("wrong-project").unwrap(),
            slug: Some("modloader".into()),
        }])
        .await;
    assert!(matches!(result[0], Err(ResourceError::InvalidResponse(_))));
}

#[tokio::test]
async fn planet_minecraft_missing_metadata_remains_unknown_and_downloads_need_hashes()
 {
    let transport = FixtureTransport::new(|_, request| {
        assert_eq!(request.path, "/projects/project");
        Ok(
            json!({"id":"project", "title":"Project", "pageUrl":"https://www.planetminecraft.com/project/example", "versions":[{"id":"release", "name":"1", "gameVersions":["1.20.1"], "download":{"pageUrl":"https://www.planetminecraft.com/project/example", "directUrl":"https://cdn.example/file.zip"}}]}),
        )
    });
    let provider = PlanetMinecraftProvider::new(transport);
    let results = provider
        .fetch_many(&[ProviderQuery::Release {
            project_id: Some(SourceProjectId::new("project").unwrap()),
            release_id: SourceReleaseId::new("release").unwrap(),
            slug: None,
        }])
        .await;
    let ProviderValue::Release(document) = results[0].as_ref().unwrap() else {
        panic!("wrong release");
    };
    assert!(document.release.published_at.is_none());
    assert!(document.release.artifacts[0].file_name.is_none());
    assert!(document.release.artifacts[0].integrity.size.is_none());
    assert!(matches!(
        document.release.artifacts[0].acquisition,
        ArtifactAcquisition::ManualDownload { .. }
    ));
    assert!(
        !provider
            .source()
            .capabilities
            .contains(&SourceCapability::IdentifyHash)
    );
    assert!(
        !provider
            .source()
            .capabilities
            .contains(&SourceCapability::Changelog)
    );
}

#[test]
fn probes_have_bounded_history_and_md5_is_not_identity_evidence() {
    let mut budget = ProbeBudget::default();
    assert_eq!(budget.take_initial(20), 3);
    assert_eq!(budget.take_initial(20), 0);
    assert!(budget.take_history_page());
    assert_eq!(budget.take_history_artifacts(6), 6);
    assert!(budget.take_history_page());
    assert_eq!(budget.take_history_artifacts(6), 4);
    assert!(!budget.take_history_page());
    let md5 = ArtifactIntegrity {
        size: Some(10),
        hashes: vec![ContentHash {
            algorithm: HashAlgorithm::Md5,
            value: "a".repeat(32),
        }],
    };
    assert!(!exact_integrity(&md5, &md5));
}

#[test]
fn cache_paths_are_channel_scoped_and_reject_path_traversal() {
    let directories = crate::state::DirectoryInfo {
        settings_dir: "settings".into(),
        config_dir: "custom".into(),
        app_identifier: "test".into(),
    };
    assert_eq!(
        directories.resource_cache_db("beta").unwrap(),
        std::path::Path::new("custom/caches/resources/beta/resource-cache.db")
    );
    assert_ne!(
        directories.resource_cache_db("beta").unwrap(),
        directories.resource_cache_db("release").unwrap()
    );
    assert!(directories.resource_cache_db("../outside").is_err());
}

fn target() -> ipc::ResourceTarget {
    ipc::ResourceTarget {
        game_versions: vec![],
        loaders: vec![],
        channels: vec![],
    }
}

#[tokio::test]
async fn concurrent_cache_initialization_does_not_block_the_runtime() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("resource-cache.db");
    let (left, right) =
        tokio::time::timeout(std::time::Duration::from_secs(8), async {
            tokio::join!(
                ResourceCache::open(path.clone()),
                ResourceCache::open(path.clone())
            )
        })
        .await
        .unwrap();
    assert!(left.available());
    assert!(right.available());
    left.close().await;
    right.close().await;
}

#[tokio::test]
async fn modrinth_project_batches_keep_public_team_members_and_ignore_pending_invites()
 {
    let transport = FixtureTransport::new(|_, request| {
        match request.path.as_str() {
            "/projects" => Ok(
                json!([{"id":"YL57xq9U","slug":"iris","title":"Iris","description":"","project_type":"mod","team":"team0001"}]),
            ),
            "/teams" => Ok(
                json!([[{"team_id":"team0001","accepted":true,"user":{"username":"public-author"}}, {"team_id":"team0001","accepted":false,"user":{"username":"pending-invite"}}]]),
            ),
            _ => panic!("unexpected endpoint"),
        }
    });
    let results = ModrinthProvider::new(transport.clone())
        .fetch_many(&[project_query("YL57xq9U")])
        .await;
    let ProviderValue::Project(document) = results[0].as_ref().unwrap() else {
        panic!("wrong project");
    };
    assert_eq!(document.metadata.authors, ["public-author"]);
    assert_eq!(transport.requests.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn curseforge_auto_download_derives_missing_urls_but_keeps_public_and_unavailable_files()
 {
    let transport = FixtureTransport::new(|_, request| {
        assert_eq!(request.path, "/v1/mods/files");
        let files: Vec<_> = [
            Value::Null,
            json!(""),
            json!("  "),
            json!("https://example.com/original.jar"),
            Value::Null,
        ]
        .into_iter()
        .enumerate()
        .map(|(index, url)| {
            let mut file: Value =
                serde_json::from_str(fixtures::CF_IRIS).unwrap();
            file["id"] = json!(5425378 + index);
            file["downloadUrl"] = url;
            file["fileName"] = json!("chest dna: 1.jar");
            file["isAvailable"] = json!(index != 4);
            file
        })
        .collect();
        Ok(json!({"data": files}))
    });
    let provider = CurseForgeProvider::with_download_restriction_policy(
        transport.clone(),
        Arc::new(|| true),
    );
    let queries = (5425378..=5425382)
        .map(|id| ProviderQuery::Release {
            project_id: None,
            release_id: SourceReleaseId::new(id.to_string()).unwrap(),
            slug: None,
        })
        .collect::<Vec<_>>();
    let results = provider.fetch_many(&queries).await;
    for (index, result) in results.into_iter().enumerate() {
        let ProviderValue::Release(document) = result.unwrap() else {
            panic!("wrong release");
        };
        let acquisition = &document.release.artifacts[0].acquisition;
        let expected = match index {
            0..=2 => ArtifactAcquisition::PublicDownload {
                urls: vec![format!(
                    "https://edge.forgecdn.net/files/5425/{}/chest%20dna%3A%201.jar",
                    378 + index
                )],
            },
            3 => ArtifactAcquisition::PublicDownload {
                urls: vec!["https://example.com/original.jar".into()],
            },
            _ => ArtifactAcquisition::Unavailable,
        };
        assert_eq!(acquisition, &expected);
    }
    assert_eq!(transport.requests.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn curseforge_auto_download_applies_to_automatic_release_catalogs() {
    let transport = FixtureTransport::new(|_, request| {
        assert_eq!(request.path, "/v1/mods/455508/files");
        let mut file: Value = serde_json::from_str(fixtures::CF_IRIS).unwrap();
        file["downloadUrl"] = Value::Null;
        Ok(
            json!({"data": [file], "pagination": {"index":0,"pageSize":50,"resultCount":1,"totalCount":1}}),
        )
    });
    let provider = CurseForgeProvider::with_download_restriction_policy(
        transport,
        Arc::new(|| true),
    );
    let results = provider
        .fetch_many(&[ProviderQuery::Releases {
            project_id: SourceProjectId::new("455508").unwrap(),
            slug: None,
            target: ipc::ResourceTarget {
                game_versions: vec![],
                loaders: vec![],
                channels: vec![],
            },
            cursor: None,
            page_size: 50,
        }])
        .await;
    let ProviderValue::Releases(page) = results[0].as_ref().unwrap() else {
        panic!("wrong catalog");
    };
    assert!(matches!(
        page.entries[0].artifacts[0].acquisition,
        ArtifactAcquisition::PublicDownload { .. }
    ));
}

#[tokio::test]
async fn curseforge_download_policy_changes_do_not_reuse_the_other_policy_cache()
 {
    let transport = FixtureTransport::new(|_, request| {
        match request.path.as_str() {
            "/v1/mods/files" => {
                let mut file: Value =
                    serde_json::from_str(fixtures::CF_IRIS).unwrap();
                file["downloadUrl"] = Value::Null;
                Ok(json!({"data": [file]}))
            }
            "/v1/mods" => Ok(
                json!({"data":[{"id":455508,"slug":"irisshaders","name":"Iris","classId":6,"links":{"websiteUrl":"https://www.curseforge.com/minecraft/mc-mods/irisshaders"}}]}),
            ),
            _ => panic!("unexpected endpoint"),
        }
    });
    let enabled = Arc::new(AtomicBool::new(false));
    let setting = Arc::clone(&enabled);
    let provider = CurseForgeProvider::with_download_restriction_policy(
        transport.clone(),
        Arc::new(move || setting.load(Ordering::Relaxed)),
    );
    let (_directory, cache) = cache().await;
    let service = ResourceService::new(
        cache,
        vec![Arc::new(Available(Arc::new(provider)))],
    )
    .unwrap();
    let query = ProviderQuery::Release {
        project_id: None,
        release_id: SourceReleaseId::new("5425378").unwrap(),
        slug: None,
    };
    for (bypass, cached) in
        [(false, false), (true, false), (false, true), (true, true)]
    {
        enabled.store(bypass, Ordering::Relaxed);
        let result = service
            .query(
                &SourceId::curseforge(),
                query.clone(),
                CachePolicy::UseFresh,
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(result.from_cache, cached);
        let ProviderValue::Release(document) = result.value else {
            panic!("wrong release");
        };
        assert_eq!(
            matches!(
                document.release.artifacts[0].acquisition,
                ArtifactAcquisition::PublicDownload { .. }
            ),
            bypass
        );
        assert_eq!(
            matches!(
                document.release.artifacts[0].acquisition,
                ArtifactAcquisition::ManualDownload { .. }
            ),
            !bypass
        );
    }
    assert_eq!(transport.requests.lock().unwrap().len(), 3);
    service.cache().close().await;
}

#[tokio::test]
async fn curseforge_policy_change_during_a_request_cannot_poison_the_cache() {
    let enabled = Arc::new(AtomicBool::new(false));
    let setting = Arc::clone(&enabled);
    let transport = FixtureTransport::new(move |_, request| {
        match request.path.as_str() {
            "/v1/mods/files" => {
                setting.store(true, Ordering::Relaxed);
                let mut file: Value =
                    serde_json::from_str(fixtures::CF_IRIS).unwrap();
                file["downloadUrl"] = Value::Null;
                Ok(json!({"data": [file]}))
            }
            "/v1/mods" => Ok(
                json!({"data":[{"id":455508,"slug":"irisshaders","name":"Iris","classId":6,"links":{"websiteUrl":"https://www.curseforge.com/minecraft/mc-mods/irisshaders"}}]}),
            ),
            _ => panic!("unexpected endpoint"),
        }
    });
    let setting = Arc::clone(&enabled);
    let provider = CurseForgeProvider::with_download_restriction_policy(
        transport,
        Arc::new(move || setting.load(Ordering::Relaxed)),
    );
    let (_directory, cache) = cache().await;
    let service = ResourceService::new(
        cache,
        vec![Arc::new(Available(Arc::new(provider)))],
    )
    .unwrap();
    let query = ProviderQuery::Release {
        project_id: None,
        release_id: SourceReleaseId::new("5425378").unwrap(),
        slug: None,
    };
    let cancel = CancellationToken::new();
    assert_eq!(
        service
            .query(
                &SourceId::curseforge(),
                query.clone(),
                CachePolicy::UseFresh,
                &cancel
            )
            .await
            .unwrap_err(),
        ResourceError::Canceled
    );
    enabled.store(false, Ordering::Relaxed);
    assert_eq!(
        service
            .query(
                &SourceId::curseforge(),
                query,
                CachePolicy::OnlyCached,
                &cancel
            )
            .await
            .unwrap_err(),
        ResourceError::Unavailable
    );
    service.cache().close().await;
}

#[tokio::test]
async fn curseforge_manual_links_use_project_urls_and_unavailable_files_remain_unavailable()
 {
    let transport = FixtureTransport::new(|_, request| {
        match request.path.as_str() {
            "/v1/mods/files" => {
                let mut file: Value =
                    serde_json::from_str(fixtures::CF_IRIS).unwrap();
                file["downloadUrl"] = Value::Null;
                file["isServerPack"] = Value::Null;
                let mut unavailable = file.clone();
                unavailable["id"] = json!(5425379);
                unavailable["isAvailable"] = json!(false);
                Ok(json!({"data":[file, unavailable]}))
            }
            "/v1/mods" => Ok(
                json!({"data":[{"id":455508,"slug":"irisshaders","name":"Iris","classId":6,"links":{"websiteUrl":"https://www.curseforge.com/minecraft/mc-mods/irisshaders"}}]}),
            ),
            _ => panic!("unexpected endpoint"),
        }
    });
    let queries = [5425378, 5425379].map(|id| ProviderQuery::Release {
        project_id: Some(SourceProjectId::new("455508").unwrap()),
        release_id: SourceReleaseId::new(id.to_string()).unwrap(),
        slug: None,
    });
    let results = CurseForgeProvider::new(transport.clone())
        .fetch_many(&queries)
        .await;
    let ProviderValue::Release(document) = results[0].as_ref().unwrap() else {
        panic!("wrong release");
    };
    assert_eq!(document.release.artifacts[0].acquisition, ArtifactAcquisition::ManualDownload { page_url: "https://www.curseforge.com/minecraft/mc-mods/irisshaders/files/5425378".into() });
    let ProviderValue::Release(document) = results[1].as_ref().unwrap() else {
        panic!("wrong release");
    };
    assert_eq!(document.release.artifacts[0].available, Some(false));
    assert_eq!(
        document.release.artifacts[0].acquisition,
        ArtifactAcquisition::Unavailable
    );
    assert_eq!(transport.requests.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn source_search_projections_preserve_variants_and_date_semantics() {
    let mr_transport = FixtureTransport::new(|_, _| {
        let mut response: Value =
            serde_json::from_str(fixtures::MR_SEARCH).unwrap();
        response["total_hits"] =
            json!(response["hits"].as_array().unwrap().len());
        response["hits"][0]["date_created"] = json!("2024-01-01T00:00:00Z");
        response["hits"][0]["date_modified"] = json!("2025-01-01T00:00:00Z");
        Ok(response)
    });
    let cf_transport = FixtureTransport::new(|_, _| {
        let mut response: Value =
            serde_json::from_str(fixtures::CF_SEARCH).unwrap();
        for project in response["data"].as_array_mut().unwrap() {
            project["classId"] = json!(6);
        }
        Ok(response)
    });
    let query = ProviderQuery::Search {
        query: ipc::ResourceSearchQuery {
            filter: None,
            text: "itemphysic".into(),
            types: vec![ResourceType::Mod],
            sources: vec![],
            target: target(),
            categories: vec![],
            sort: ipc::ResourceSearchSort::Relevance,
            page_size: 20,
        },
        cursor: None,
    };
    let mr = ModrinthProvider::new(mr_transport);
    let cf = CurseForgeProvider::new(cf_transport);
    let mr_result = mr
        .fetch_many(std::slice::from_ref(&query))
        .await
        .remove(0)
        .unwrap();
    let cf_result = cf.fetch_many(&[query]).await.remove(0).unwrap();
    let ProviderValue::Search(mr_page) = mr_result else {
        panic!("wrong search");
    };
    let ProviderValue::Search(cf_page) = cf_result else {
        panic!("wrong search");
    };
    assert!(mr_page.entries[0].created_at.is_none());
    assert!(mr_page.entries[0].project_published_at.is_some());
    assert!(mr_page.entries[0].updated_at.is_some());
    assert!(mr_page.entries[0].first_published_at.is_none());
    assert!(mr_page.entries[0].latest_published_at.is_none());
    assert!(
        mr_page
            .entries
            .iter()
            .any(|project| project.title == "ItemPhysic Lite")
    );
    assert!(
        cf_page
            .entries
            .iter()
            .any(|project| project.title == "ItemPhysic Full")
    );
    assert!(
        cf_page
            .entries
            .iter()
            .any(|project| project.title == "ItemPhysic Legacy Unofficial")
    );
    assert_eq!(cf_page.total.precision, ipc::TotalPrecision::Unknown);
}

#[tokio::test]
async fn modrinth_search_publication_matches_detail_approval_without_reordering_by_creation()
 {
    let transport = FixtureTransport::new(|_, request| {
        if request.path == "/search" {
            assert!(request.query.contains(&("index".into(), "newest".into())));
            assert!(request.query.contains(&("limit".into(), "100".into())));
            Ok(
                json!({"hits":[{"project_id":"kgA13GJP", "slug":"sample", "title":"Sample", "project_type":"mod", "description":"", "date_created":"2026-10-13T05:50:26.298835+08:00"}], "total_hits":1}),
            )
        } else {
            assert_eq!(request.path, "/projects");
            Ok(
                json!([{"id":"kgA13GJP", "slug":"sample", "title":"Sample", "project_type":"mod", "description":"", "published":"2026-09-09T19:39:43.551169Z", "approved":"2026-10-12T21:50:26.298835Z"}]),
            )
        }
    });
    let provider = ModrinthProvider::new(transport);
    let mut query = ipc::ResourceSearchQuery {
        filter: None,
        text: String::new(),
        types: vec![ResourceType::Mod],
        sources: vec![],
        target: ipc::ResourceTarget {
            game_versions: vec!["1.21.1".into()],
            loaders: vec!["neoforge".into()],
            channels: vec![],
        },
        categories: vec![],
        sort: ipc::ResourceSearchSort::ProjectPublished,
        page_size: 100,
    };
    let ProviderValue::Search(page) = provider
        .fetch_many(&[ProviderQuery::Search {
            query: query.clone(),
            cursor: None,
        }])
        .await
        .remove(0)
        .unwrap()
    else {
        panic!("search page missing");
    };
    let ProviderValue::Project(project) = provider
        .fetch_many(&[ProviderQuery::Project {
            id: SourceProjectId::new("kgA13GJP").unwrap(),
            slug: None,
        }])
        .await
        .remove(0)
        .unwrap()
    else {
        panic!("project missing");
    };
    assert_eq!(
        page.entries[0].project_published_at,
        project.metadata.project_published_at
    );
    assert_ne!(
        page.entries[0].project_published_at,
        project.metadata.created_at
    );
    assert!(page.entries[0].created_at.is_none());
    query.sort = ipc::ResourceSearchSort::Created;
    assert!(matches!(
        provider
            .fetch_many(&[ProviderQuery::Search {
                query,
                cursor: None
            }])
            .await
            .remove(0),
        Err(ResourceError::Unsupported(_))
    ));
}

#[tokio::test]
async fn curseforge_last_window_page_is_bounded_and_does_not_claim_a_complete_total()
 {
    let transport = FixtureTransport::new(|_, request| {
        assert!(request.query.contains(&("index".into(), "9990".into())));
        assert!(request.query.contains(&("pageSize".into(), "10".into())));
        Ok(
            json!({"data":(1..=10).map(|id| json!({"id":id,"name":"Project","slug":"project","classId":6})).collect::<Vec<_>>(), "pagination":{"index":9990,"resultCount":10,"totalCount":10000}}),
        )
    });
    let query = ProviderQuery::Search {
        query: ipc::ResourceSearchQuery {
            filter: None,
            text: "".into(),
            types: vec![ResourceType::Mod],
            sources: vec![],
            target: target(),
            categories: vec![],
            sort: ipc::ResourceSearchSort::Relevance,
            page_size: 50,
        },
        cursor: Some("9990".into()),
    };
    let result = CurseForgeProvider::new(transport)
        .fetch_many(&[query])
        .await
        .remove(0)
        .unwrap();
    let ProviderValue::Search(page) = result else {
        panic!("wrong search");
    };
    assert!(page.window_exhausted);
    assert!(page.cursor.is_none());
    assert_eq!(page.entries.len(), 10);
    assert_eq!(page.total.precision, ipc::TotalPrecision::Estimate);
}

#[tokio::test]
async fn curseforge_search_never_uses_game_version_sort_for_publication_dates()
{
    let transport = FixtureTransport::new(|_, request| {
        assert!(request.query.contains(&("sortField".into(), "3".into())));
        assert!(
            request
                .query
                .contains(&("gameVersion".into(), "1.21.1".into()))
        );
        assert!(
            request
                .query
                .contains(&("modLoaderType".into(), "6".into()))
        );
        Ok(
            json!({"data":[{"id":238222,"name":"Sample","slug":"sample","classId":6,"dateCreated":"2015-11-23T23:00:41.923Z","dateReleased":"2026-10-12T14:05:52.453Z"}],"pagination":{"index":0,"resultCount":1,"totalCount":1}}),
        )
    });
    let provider = CurseForgeProvider::new(transport);
    let mut query = ipc::ResourceSearchQuery {
        filter: None,
        text: String::new(),
        types: vec![ResourceType::Mod],
        sources: vec![],
        target: ipc::ResourceTarget {
            game_versions: vec!["1.21.1".into()],
            loaders: vec!["neoforge".into()],
            channels: vec![],
        },
        categories: vec![],
        sort: ipc::ResourceSearchSort::ProjectUpdated,
        page_size: 50,
    };
    let ProviderValue::Search(page) = provider
        .fetch_many(&[ProviderQuery::Search {
            query: query.clone(),
            cursor: None,
        }])
        .await
        .remove(0)
        .unwrap()
    else {
        panic!("search page missing");
    };
    assert!(page.entries[0].first_published_at.is_none());
    assert!(page.entries[0].latest_published_at > page.entries[0].created_at);
    query.sort = ipc::ResourceSearchSort::Created;
    assert!(matches!(
        provider
            .fetch_many(&[ProviderQuery::Search {
                query,
                cursor: None
            }])
            .await
            .remove(0),
        Err(ResourceError::Unsupported(_))
    ));
}

#[tokio::test]
async fn curseforge_release_pages_filter_channels_client_side_without_losing_the_source_window()
 {
    let transport = FixtureTransport::new(|_, request| {
        assert_eq!(request.path, "/v1/mods/455508/files");
        assert!(request.query.contains(&("index".into(), "0".into())));
        assert!(request.query.contains(&("pageSize".into(), "50".into())));
        let release: Value = serde_json::from_str(fixtures::CF_IRIS).unwrap();
        let mut beta = release.clone();
        beta["id"] = json!(5425379);
        beta["releaseType"] = json!(2);
        let mut alpha = release.clone();
        alpha["id"] = json!(5425380);
        alpha["releaseType"] = json!(3);
        Ok(json!({
            "data": [release, beta, alpha],
            "pagination": {"index": 0, "resultCount": 3, "totalCount": 3}
        }))
    });
    let query = |channels: Vec<ReleaseChannel>| ProviderQuery::Releases {
        project_id: SourceProjectId::new("455508").unwrap(),
        slug: None,
        target: ipc::ResourceTarget {
            game_versions: vec![],
            loaders: vec![],
            channels,
        },
        cursor: None,
        page_size: 50,
    };
    let provider = CurseForgeProvider::new(transport);
    let results = provider
        .fetch_many(&[query(vec![ReleaseChannel::Release])])
        .await;
    let ProviderValue::Releases(stable) = results[0].as_ref().unwrap() else {
        panic!("wrong page: {results:?}");
    };
    assert_eq!(stable.entries.len(), 1);
    assert_eq!(stable.entries[0].channel, ReleaseChannel::Release);
    assert_eq!(stable.total.value, Some(3));
    assert_eq!(stable.total.precision, ipc::TotalPrecision::Exact);
    assert!(stable.cursor.is_none());
    assert!(!stable.window_exhausted);

    let results = provider
        .fetch_many(&[query(vec![
            ReleaseChannel::Release,
            ReleaseChannel::Beta,
        ])])
        .await;
    let ProviderValue::Releases(prerelease) = results[0].as_ref().unwrap()
    else {
        panic!("wrong page: {results:?}");
    };
    assert_eq!(prerelease.entries.len(), 2);
    assert!(prerelease.entries.iter().all(|release| matches!(
        release.channel,
        ReleaseChannel::Release | ReleaseChannel::Beta
    )));
    assert_eq!(prerelease.total.value, Some(3));
}

#[tokio::test]
async fn clear_leaves_main_database_facts_and_source_namespaces_intact() {
    let (temp, cache) = cache().await;
    let main = sqlx::sqlite::SqlitePoolOptions::new()
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(temp.path().join("app.db"))
                .create_if_missing(true),
        )
        .await
        .unwrap();
    sqlx::query("CREATE TABLE business_facts (id TEXT PRIMARY KEY)")
        .execute(&main)
        .await
        .unwrap();
    sqlx::query("INSERT INTO business_facts VALUES ('installed-entry')")
        .execute(&main)
        .await
        .unwrap();
    let mr =
        CacheKey::new("v1".into(), SourceId::modrinth(), &project_query("42"))
            .unwrap();
    let cf = CacheKey::new(
        "v1".into(),
        SourceId::curseforge(),
        &project_query("42"),
    )
    .unwrap();
    let other_namespace =
        CacheKey::new("v2".into(), SourceId::modrinth(), &project_query("42"))
            .unwrap();
    cache
        .put(
            &mr,
            &project_value(SourceId::modrinth(), "42", "MR"),
            100,
            cache.generation().await,
            1,
        )
        .await;
    assert!(cache.get(&cf).await.is_none());
    assert!(cache.get(&other_namespace).await.is_none());
    cache.clear().await.unwrap();
    assert_eq!(cache.info().await.unwrap().entry_count, 0);
    let fact: String = sqlx::query_scalar("SELECT id FROM business_facts")
        .fetch_one(&main)
        .await
        .unwrap();
    assert_eq!(fact, "installed-entry");
    main.close().await;
    cache.close().await;
}

#[tokio::test]
async fn source_validation_rejects_one_bad_id_without_poisoning_peer_results() {
    let transport = FixtureTransport::new(|_, request| {
        assert_eq!(request.body.as_ref().unwrap()["modIds"], json!([42]));
        Ok(
            json!({"data": [{"id":42,"name":"Valid", "slug":"valid", "classId":6}]}),
        )
    });
    let (_temp, cache) = cache().await;
    let service = ResourceService::new(
        cache.clone(),
        vec![Arc::new(Available(Arc::new(CurseForgeProvider::new(
            transport.clone(),
        ))))],
    )
    .unwrap();
    let results = service
        .query_many(
            &SourceId::curseforge(),
            vec![project_query("not-numeric"), project_query("42")],
            CachePolicy::ForceRefresh,
            &CancellationToken::new(),
        )
        .await;
    assert!(matches!(results[0], Err(ResourceError::InvalidRequest(_))));
    assert!(results[1].is_ok());
    assert_eq!(transport.requests.lock().unwrap().len(), 1);
    cache.close().await;
}

#[tokio::test]
async fn project_probe_persists_historical_balm_confirmation_with_a_bounded_budget()
 {
    let mr_transport = FixtureTransport::new(|_, request| {
        match request.path.as_str() {
            "/projects" => Ok(
                json!([{"id":"MBAkmtvl","slug":"balm","title":"Balm","description":"","project_type":"mod","loaders":["forge"]}]),
            ),
            "/version_files" => {
                let requested = request.body.as_ref().unwrap()["hashes"]
                    .as_array()
                    .unwrap();
                let common = "4248e3b8fb20e9b6432d2225325c605f4ad7b211";
                if requested.iter().any(|value| value == common) {
                    Ok(
                        json!({common: serde_json::from_str::<Value>(fixtures::MR_BALM_43).unwrap()}),
                    )
                } else {
                    Ok(json!({}))
                }
            }
            _ => panic!("unexpected MR endpoint"),
        }
    });
    let cf_transport = FixtureTransport::new(|_, request| {
        match request.path.as_str() {
            "/v1/mods" => Ok(
                json!({"data":[{"id":531761,"name":"Balm","slug":"balm","classId":6}]}),
            ),
            "/v1/mods/531761/files" => {
                let current: Value =
                    serde_json::from_str(fixtures::CF_BALM_44).unwrap();
                let mut files = vec![current];
                for id in [8000000, 8000001] {
                    let mut file = files[0].clone();
                    file["id"] = json!(id);
                    file["hashes"][0]["value"] = json!("b".repeat(40));
                    files.push(file);
                }
                files.push(serde_json::from_str(fixtures::CF_BALM_43).unwrap());
                Ok(json!({"data":files,"pagination":{"totalCount":4}}))
            }
            _ => panic!("unexpected CF endpoint"),
        }
    });
    let (_temp, cache) = cache().await;
    let main = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!().run(&main).await.unwrap();
    let store = crate::state::resources::ResourceStore::new(main.clone());
    let service = ResourceService::with_store(
        cache.clone(),
        vec![
            Arc::new(ModrinthProvider::new(mr_transport.clone())),
            Arc::new(Available(Arc::new(CurseForgeProvider::new(
                cf_transport.clone(),
            )))),
        ],
        store.clone(),
    )
    .unwrap();
    let mr = SourceProjectRef {
        source: SourceId::modrinth(),
        project_id: SourceProjectId::new("MBAkmtvl").unwrap(),
    };
    let cf = SourceProjectRef {
        source: SourceId::curseforge(),
        project_id: SourceProjectId::new("531761").unwrap(),
    };
    assert!(store.lookup(&mr).await.unwrap().is_none());
    assert!(store.lookup(&cf).await.unwrap().is_none());
    let result = service
        .probe_projects(
            &mr,
            None,
            &cf,
            None,
            target(),
            &CancellationToken::new(),
        )
        .await;
    assert_eq!(result.status, IdentityStatus::Confirmed);
    assert_eq!(result.history_pages, 1);
    assert_eq!(result.probed_artifacts, 4);
    assert_eq!(result.evidence.len(), 1);
    assert_eq!(
        store.lookup(&mr).await.unwrap(),
        store.lookup(&cf).await.unwrap()
    );
    let proof = &result.evidence[0];
    assert!(
        service
            .artifact_evidence(&proof.left, &proof.right)
            .await
            .is_some()
    );
    let before = mr_transport.requests.lock().unwrap().len()
        + cf_transport.requests.lock().unwrap().len();
    let repeated = service
        .probe_projects(
            &mr,
            None,
            &cf,
            None,
            target(),
            &CancellationToken::new(),
        )
        .await;
    assert_eq!(repeated.status, IdentityStatus::Confirmed);
    assert_eq!(
        before,
        mr_transport.requests.lock().unwrap().len()
            + cf_transport.requests.lock().unwrap().len()
    );
    cache.clear().await.unwrap();
    assert!(
        service
            .artifact_evidence(&proof.left, &proof.right)
            .await
            .is_none()
    );
    let durable = service
        .probe_projects(
            &mr,
            None,
            &cf,
            None,
            target(),
            &CancellationToken::new(),
        )
        .await;
    assert_eq!(durable.status, IdentityStatus::Confirmed);
    assert_eq!(durable.probed_artifacts, 0);
    assert!(durable.evidence.is_empty());
    assert_eq!(
        before,
        mr_transport.requests.lock().unwrap().len()
            + cf_transport.requests.lock().unwrap().len()
    );
    cache.close().await;
}

struct HttpTransport {
    base: String,
    client: reqwest::Client,
}

#[async_trait]
impl ProtocolTransport for HttpTransport {
    async fn request(
        &self,
        _: &SourceId,
        request: ProtocolRequest,
    ) -> ResourceResult<Value> {
        let mut builder = self
            .client
            .request(request.method, format!("{}{}", self.base, request.path))
            .query(&request.query);
        if let Some(body) = request.body {
            builder = builder.json(&body);
        }
        let response =
            builder
                .send()
                .await
                .map_err(|error| ResourceError::Network {
                    status: error.status().map(|status| status.as_u16()),
                })?;
        if !response.status().is_success() {
            return Err(ResourceError::Network {
                status: Some(response.status().as_u16()),
            });
        }
        response.json().await.map_err(|_| {
            ResourceError::InvalidResponse("invalid HTTP JSON".into())
        })
    }
}

#[tokio::test]
async fn real_http_batches_and_reopened_sqlite_cache_obey_force_refresh() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let bytes = Arc::new(AtomicUsize::new(0));
    let fail = Arc::new(AtomicBool::new(false));
    let server = tokio::spawn({
        let calls = calls.clone();
        let bytes = bytes.clone();
        let fail = fail.clone();
        async move {
            loop {
                let (mut stream, _) = listener.accept().await.unwrap();
                let calls = calls.clone();
                let bytes = bytes.clone();
                let fail = fail.clone();
                tokio::spawn(async move {
                    let mut header = vec![];
                    loop {
                        let mut buffer = [0u8; 2048];
                        let count = stream.read(&mut buffer).await.unwrap();
                        if count == 0 {
                            return;
                        }
                        header.extend_from_slice(&buffer[..count]);
                        if header.windows(4).any(|window| window == b"\r\n\r\n")
                        {
                            break;
                        }
                        assert!(header.len() < 32768);
                    }
                    calls.fetch_add(1, Ordering::AcqRel);
                    let header = String::from_utf8(header).unwrap();
                    let path = header.split_whitespace().nth(1).unwrap();
                    let url =
                        url::Url::parse(&format!("http://127.0.0.1{path}"))
                            .unwrap();
                    let ids: Vec<String> = serde_json::from_str(
                        &url.query_pairs()
                            .find(|(key, _)| key == "ids")
                            .unwrap()
                            .1,
                    )
                    .unwrap();
                    assert!(ids.len() <= 100);
                    let failed = fail.load(Ordering::Acquire);
                    let payload = if failed {
                        json!({"error":"unavailable"})
                    } else {
                        Value::Array(ids.iter().map(|id| json!({"id":id,"slug":id,"title":id,"description":"","body":"body","project_type":"mod"})).collect())
                    };
                    let payload = serde_json::to_vec(&payload).unwrap();
                    bytes.fetch_add(payload.len(), Ordering::AcqRel);
                    let response = format!(
                        "HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        if failed {
                            "503 Service Unavailable"
                        } else {
                            "200 OK"
                        },
                        payload.len()
                    );
                    stream.write_all(response.as_bytes()).await.unwrap();
                    stream.write_all(&payload).await.unwrap();
                });
            }
        }
    });
    let transport = Arc::new(HttpTransport {
        base: format!("http://{address}"),
        client: reqwest::Client::builder().no_proxy().build().unwrap(),
    });
    let (_temp, cache) = cache().await;
    let service = ResourceService::new(
        cache.clone(),
        vec![Arc::new(ModrinthProvider::new(transport.clone()))],
    )
    .unwrap();
    let queries: Vec<_> = (0..101)
        .map(|id| project_query(&format!("p{id:07}")))
        .collect();
    let token = CancellationToken::new();
    assert!(
        service
            .query_many(
                &SourceId::modrinth(),
                queries.clone(),
                CachePolicy::UseFresh,
                &token
            )
            .await
            .iter()
            .all(Result::is_ok)
    );
    assert_eq!(calls.load(Ordering::Acquire), 2);
    assert!(
        service
            .query_many(
                &SourceId::modrinth(),
                queries.clone(),
                CachePolicy::UseFresh,
                &token
            )
            .await
            .iter()
            .all(|result| result.as_ref().unwrap().from_cache)
    );
    assert_eq!(calls.load(Ordering::Acquire), 2);
    let path = cache.path().to_path_buf();
    cache.close().await;
    let reopened = Arc::new(ResourceCache::open(path).await);
    let new_service = ResourceService::new(
        reopened.clone(),
        vec![Arc::new(ModrinthProvider::new(transport))],
    )
    .unwrap();
    assert!(
        new_service
            .query_many(
                &SourceId::modrinth(),
                queries.clone(),
                CachePolicy::UseFresh,
                &token
            )
            .await
            .iter()
            .all(|result| result.as_ref().unwrap().from_cache)
    );
    assert_eq!(calls.load(Ordering::Acquire), 2);
    assert!(
        new_service
            .query_many(
                &SourceId::modrinth(),
                queries,
                CachePolicy::ForceRefresh,
                &token
            )
            .await
            .iter()
            .all(Result::is_ok)
    );
    assert_eq!(calls.load(Ordering::Acquire), 4);
    fail.store(true, Ordering::Release);
    assert_eq!(
        new_service
            .query(
                &SourceId::modrinth(),
                project_query("p0000000"),
                CachePolicy::ForceRefresh,
                &token
            )
            .await,
        Err(ResourceError::Network { status: Some(503) })
    );
    assert_eq!(calls.load(Ordering::Acquire), 5);
    assert!(bytes.load(Ordering::Acquire) > 0);
    reopened.close().await;
    server.abort();
}

#[tokio::test]
async fn selected_counterparts_trust_curated_pairs_without_artifact_probes() {
    struct CounterpartProvider {
        source: SourceId,
        project: ProjectDocument,
        releases: Vec<ResourceRelease>,
        identified: ResourceRelease,
        matches: bool,
        requests: Arc<Mutex<Vec<ProviderQuery>>>,
    }
    #[async_trait]
    impl ResourceProvider for CounterpartProvider {
        fn source(&self) -> ResourceSource {
            ResourceSource {
                id: self.source.clone(),
                types: vec![ResourceType::Mod],
                capabilities: vec![
                    SourceCapability::Project,
                    SourceCapability::Search,
                    SourceCapability::Releases,
                    if self.source == SourceId::curseforge() {
                        SourceCapability::IdentifyFingerprint
                    } else {
                        SourceCapability::IdentifyHash
                    },
                ],
                available: true,
                supported_search_sorts: vec![
                    ipc::ResourceSearchSort::Relevance,
                ],
            }
        }
        fn namespace(&self) -> String {
            format!("counterpart-{}", self.source.as_str())
        }
        fn batch_limit(&self, _: &ProviderQuery) -> usize {
            50
        }
        async fn fetch_many(
            &self,
            queries: &[ProviderQuery],
        ) -> Vec<ResourceResult<ProviderValue>> {
            self.requests.lock().unwrap().extend_from_slice(queries);
            queries
                .iter()
                .map(|query| match query {
                    ProviderQuery::Project { .. }
                    | ProviderQuery::Locate { .. } => {
                        Ok(ProviderValue::Project(self.project.clone()))
                    }
                    ProviderQuery::Search { .. } => {
                        Ok(ProviderValue::Search(SourcePage {
                            entries: vec![self.project.metadata.clone()],
                            cursor: None,
                            window_exhausted: false,
                            total: ipc::ResourceTotal {
                                value: Some(1),
                                precision: ipc::TotalPrecision::Exact,
                            },
                        }))
                    }
                    ProviderQuery::Releases { .. } => {
                        Ok(ProviderValue::Releases(SourcePage {
                            entries: self.releases.clone(),
                            cursor: None,
                            window_exhausted: false,
                            total: ipc::ResourceTotal {
                                value: Some(self.releases.len() as u64),
                                precision: ipc::TotalPrecision::Exact,
                            },
                        }))
                    }
                    ProviderQuery::Identify { integrity, .. } => {
                        Ok(ProviderValue::Matches(
                            if self.matches
                                && exact_integrity(
                                    integrity,
                                    &self.identified.artifacts[0].integrity,
                                )
                            {
                                self.identified.artifacts.clone()
                            } else {
                                vec![]
                            },
                        ))
                    }
                    _ => Err(ResourceError::Unsupported(
                        "counterpart fixture".into(),
                    )),
                })
                .collect()
        }
    }
    for origin in [SourceId::modrinth(), SourceId::curseforge()] {
        for same_bytes in [true, false] {
            let mut mr_doc =
                match project_value(SourceId::modrinth(), "abcdefgh", "AE2") {
                    ProviderValue::Project(doc) => doc,
                    _ => unreachable!(),
                };
            mr_doc.metadata.slug = Some("ae2".into());
            let mut cf_doc =
                match project_value(SourceId::curseforge(), "42", "AE2") {
                    ProviderValue::Project(doc) => doc,
                    _ => unreachable!(),
                };
            cf_doc.metadata.slug = Some("applied-energistics-2".into());
            let mut mr_value: Value =
                serde_json::from_str(fixtures::MR_BALM_43).unwrap();
            mr_value["id"] = json!("release1");
            mr_value["project_id"] = json!("abcdefgh");
            let mr = legacy::release(mr_value).unwrap().release;
            let mut cf_value: Value =
                serde_json::from_str(fixtures::CF_BALM_43).unwrap();
            cf_value["id"] = json!(123);
            cf_value["modId"] = json!(42);
            let mut historical =
                legacy::curseforge_file(cf_value).unwrap().release;
            historical.artifacts[0].integrity =
                mr.artifacts[0].integrity.clone();
            let mut latest = historical.clone();
            latest.reference.release_id = SourceReleaseId::new("124").unwrap();
            latest.artifacts[0].reference.release = latest.reference.clone();
            latest.artifacts[0].integrity.hashes = vec![ContentHash {
                algorithm: HashAlgorithm::Sha1,
                value: "b".repeat(40),
            }];
            let mut latest_releases = Vec::new();
            for id in [124, 125, 126] {
                let mut release = latest.clone();
                release.reference.release_id =
                    SourceReleaseId::new(id.to_string()).unwrap();
                release.artifacts[0].reference.release =
                    release.reference.clone();
                latest_releases.push(release);
            }
            latest_releases.push(historical);
            let requests = Arc::new(Mutex::new(vec![]));
            let (_temp, cache) = cache().await;
            let pool = sqlx::sqlite::SqlitePoolOptions::new()
                .max_connections(1)
                .connect("sqlite::memory:")
                .await
                .unwrap();
            sqlx::migrate!().run(&pool).await.unwrap();
            let store =
                crate::state::resources::ResourceStore::new(pool.clone());
            store
                .ensure_project(&mr_doc.metadata.project, &[ResourceType::Mod])
                .await
                .unwrap();
            store
                .ensure_project(&cf_doc.metadata.project, &[ResourceType::Mod])
                .await
                .unwrap();
            let service = ResourceService::with_store(
                cache.clone(),
                vec![
                    Arc::new(CounterpartProvider {
                        source: SourceId::modrinth(),
                        project: mr_doc.clone(),
                        releases: vec![mr.clone()],
                        identified: mr.clone(),
                        matches: same_bytes,
                        requests: requests.clone(),
                    }),
                    Arc::new(CounterpartProvider {
                        source: SourceId::curseforge(),
                        project: cf_doc.clone(),
                        releases: latest_releases,
                        identified: mr,
                        matches: same_bytes,
                        requests: requests.clone(),
                    }),
                ],
                store.clone(),
            )
            .unwrap();
            let metadata = if origin == SourceId::modrinth() {
                &mr_doc.metadata
            } else {
                &cf_doc.metadata
            };
            service
                .confirm_counterpart(metadata, &CancellationToken::new())
                .await
                .unwrap();
            assert!(
                store.lookup(&mr_doc.metadata.project).await.unwrap()
                    == store.lookup(&cf_doc.metadata.project).await.unwrap(),
                "origin={origin:?}, requests={:?}",
                requests.lock().unwrap().clone(),
            );
            assert!(!requests.lock().unwrap().iter().any(|query| matches!(
                query,
                ProviderQuery::Releases { .. } | ProviderQuery::Identify { .. }
            )));
            if same_bytes {
                let count = requests.lock().unwrap().len();
                cache.clear().await.unwrap();
                service
                    .confirm_counterpart(metadata, &CancellationToken::new())
                    .await
                    .unwrap();
                assert_eq!(requests.lock().unwrap().len(), count);
            }
            assert!(pool.acquire().await.is_ok());
            cache.close().await;
            pool.close().await;
        }
    }
}

#[tokio::test]
async fn curated_project_confirmation_rejects_different_mcmod_subvariants() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();
    let store = crate::state::resources::ResourceStore::new(pool.clone());
    let metadata = |source, id, slug: &str| {
        let ProviderValue::Project(mut document) =
            project_value(source, id, slug)
        else {
            unreachable!()
        };
        document.metadata.slug = Some(slug.to_string());
        document.metadata
    };
    let full = metadata(SourceId::modrinth(), "full0000", "itemphysic");
    let lite = metadata(SourceId::curseforge(), "123", "itemphysic-lite");
    assert!(store.confirm_curated_projects(&full, &lite).await.is_err());
    assert!(store.lookup(&full.project).await.unwrap().is_none());
    assert!(store.lookup(&lite.project).await.unwrap().is_none());
    let counterpart = metadata(SourceId::curseforge(), "456", "itemphysic");
    let id = store
        .confirm_curated_projects(&full, &counterpart)
        .await
        .unwrap();
    assert_eq!(store.lookup(&full.project).await.unwrap(), Some(id.clone()));
    assert_eq!(store.lookup(&counterpart.project).await.unwrap(), Some(id));
    assert!(store.lookup(&lite.project).await.unwrap().is_none());
    let evidence: String =
        sqlx::query_scalar("SELECT payload FROM resource_identity_evidence")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(evidence.contains("mcmod-source-pair"));
    assert!(!evidence.contains("leftArtifact"));
    pool.close().await;
}

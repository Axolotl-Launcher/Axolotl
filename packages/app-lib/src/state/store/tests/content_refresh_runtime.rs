use crate::resources::providers::*;
use crate::resources::refresh::*;
use crate::state::store::{
    Collection as _, content_refresh::*, content_refresh_checks::*,
    content_snapshot,
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use tokio::sync::Notify;

struct Provider {
    slow: Notify,
    hold_slow: AtomicBool,
    fail: AtomicBool,
    queries: AtomicUsize,
    identify_queries: AtomicUsize,
    batches: AtomicUsize,
    largest_batch: AtomicUsize,
}

fn reference(project: &str, version: &str) -> SourceReleaseRef {
    SourceReleaseRef {
        project: SourceProjectRef {
            source: SourceId::modrinth(),
            project_id: SourceProjectId::new(project).unwrap(),
        },
        release_id: SourceReleaseId::new(version).unwrap(),
    }
}

fn artifact(project: &str, version: &str, sha1: &str) -> ResourceArtifact {
    ResourceArtifact {
        reference: SourceArtifactRef {
            release: reference(project, version),
            artifact_id: SourceArtifactId::new(format!("{version}-file"))
                .unwrap(),
        },
        file_name: Some(format!("{project}.zip")),
        role: ArtifactRole::Primary,
        available: Some(true),
        integrity: ArtifactIntegrity {
            size: Some(3),
            hashes: vec![ContentHash {
                algorithm: HashAlgorithm::Sha1,
                value: sha1.into(),
            }],
        },
        acquisition: ArtifactAcquisition::PublicDownload {
            urls: vec!["https://example.com/file.zip".into()],
        },
        dependencies: vec![],
    }
}

fn release(project: &str, version: &str) -> ResourceRelease {
    ResourceRelease {
        reference: reference(project, version),
        version: if version == "old" { "1.0" } else { "2.0" }.into(),
        channel: ReleaseChannel::Release,
        published_at: Some(
            chrono::DateTime::from_timestamp(
                if version == "old" { 100 } else { 200 },
                0,
            )
            .unwrap(),
        ),
        compatibility: ReleaseCompatibility {
            game_versions: vec!["1.21".into()],
            loaders: vec!["fabric".into()],
        },
        artifacts: vec![artifact(
            project,
            version,
            &if version == "old" {
                sha1_smol::Sha1::from(b"old").hexdigest()
            } else {
                "b".repeat(40)
            },
        )],
        equivalent_releases: vec![],
    }
}

#[async_trait::async_trait]
impl ResourceProvider for Provider {
    fn source(&self) -> ResourceSource {
        ResourceSource {
            id: SourceId::modrinth(),
            types: vec![
                ResourceType::Mod,
                ResourceType::ResourcePack,
                ResourceType::Shader,
                ResourceType::DataPack,
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
        "content-refresh-test".into()
    }
    fn batch_limit(&self, query: &ProviderQuery) -> usize {
        if matches!(query, ProviderQuery::Releases { .. }) {
            1
        } else {
            100
        }
    }
    async fn fetch_many(
        &self,
        queries: &[ProviderQuery],
    ) -> Vec<ResourceResult<ProviderValue>> {
        self.batches.fetch_add(1, Ordering::SeqCst);
        self.largest_batch
            .fetch_max(queries.len(), Ordering::SeqCst);
        let mut results = Vec::new();
        for query in queries {
            self.queries.fetch_add(1, Ordering::SeqCst);
            if matches!(query, ProviderQuery::Identify { .. }) {
                self.identify_queries.fetch_add(1, Ordering::SeqCst);
            }
            if self.fail.load(Ordering::SeqCst) {
                results.push(Err(ResourceError::Unavailable));
                continue;
            }
            let result = match query {
                ProviderQuery::Release {
                    project_id: Some(project),
                    release_id,
                    ..
                } => Ok(ProviderValue::Release(ReleaseDocument {
                    release: release(project.as_str(), release_id.as_str()),
                    changelog: None,
                    protocol: None,
                })),
                ProviderQuery::Releases { project_id, .. } => {
                    if project_id.as_str() == "slow"
                        && self.hold_slow.load(Ordering::SeqCst)
                    {
                        self.slow.notified().await;
                    }
                    Ok(ProviderValue::Releases(SourcePage {
                        entries: vec![release(project_id.as_str(), "new")],
                        cursor: None,
                        total: ipc::ResourceTotal {
                            value: Some(1),
                            precision: ipc::TotalPrecision::Exact,
                        },
                        window_exhausted: false,
                    }))
                }
                ProviderQuery::Project { id, .. } => {
                    Ok(ProviderValue::Project(ProjectDocument {
                        owner: Some(ProjectOwner {
                            id: "author000".into(),
                            name: "ExampleAuthor".into(),
                            avatar_url: Some(
                                "https://cdn.modrinth.com/avatars/author.png"
                                    .into(),
                            ),
                            organization: false,
                        }),
                        metadata: SourceProjectMetadata {
                            project: reference(id.as_str(), "old").project,
                            environments: vec![],
                            categories: vec![],
                            game_versions: vec![],
                            loaders: vec![],
                            gallery: vec![],
                            slug: Some(id.as_str().into()),
                            title: format!("Title {}", id.as_str()),
                            summary: String::new(),
                            icon_url: None,
                            types: vec![
                                ResourceType::Mod,
                                ResourceType::ResourcePack,
                                ResourceType::Shader,
                                ResourceType::DataPack,
                            ],
                            authors: vec!["ExampleAuthor".into()],
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
                            source: SourceId::modrinth(),
                            format: TextFormat::Plain,
                            body: String::new(),
                        },
                        protocol: None,
                    }))
                }
                _ => Err(ResourceError::Unsupported("test query".into())),
            };
            results.push(result);
        }
        results
    }
}

#[tokio::test]
async fn scan_failures_without_an_entry_survive_and_retry_without_rechecking_successful_files()
 {
    use crate::state::store::content_refresh_scan_failures::*;
    let (_temp, state, provider) = fixture().await;
    state
        .content_refresh
        .start(state.clone(), "instance".into(), RefreshMode::Normal)
        .await
        .unwrap();
    finished(&state).await;
    let successful =
        ContentRefreshChecks::get_scoped(&state.data, &"instance".into())
            .await
            .unwrap();
    let records = content_snapshot::get(&state.data, "instance")
        .await
        .unwrap();
    let root = crate::state::instances::commands::instance_content_root(
        &state.directories,
        &records.instance,
    )
    .unwrap();
    std::fs::write(root.join("resourcepacks/recovered.zip"), b"new").unwrap();
    let key = ScanFailureKey {
        instance_id: "instance".into(),
        location_key: serde_json::to_string(&(
            location_key(&records.instance),
            root.to_string_lossy(),
        ))
        .unwrap(),
        relative_path: "resourcepacks/recovered.zip".into(),
    };
    ContentRefreshScanFailures::upsert(
        &state.derived,
        &key,
        &ScanFailureRecord {
            attempted_at: chrono::Utc::now().timestamp(),
            error: "file temporarily unavailable".into(),
        },
    )
    .await
    .unwrap();
    state
        .content_refresh
        .start(state.clone(), "instance".into(), RefreshMode::Normal)
        .await
        .unwrap();
    finished(&state).await;
    assert!(
        ContentRefreshScanFailures::failed(&state.derived, "instance")
            .await
            .unwrap()
            .iter()
            .any(|(stored, _)| stored == &key)
    );
    assert_eq!(
        state
            .content_refresh
            .state(&state, "instance")
            .await
            .unwrap()
            .task
            .unwrap()
            .failed,
        1
    );
    provider.queries.store(0, Ordering::SeqCst);
    state
        .content_refresh
        .start(state.clone(), "instance".into(), RefreshMode::RetryFailed)
        .await
        .unwrap();
    finished(&state).await;
    assert!(
        !ContentRefreshScanFailures::failed(&state.derived, "instance")
            .await
            .unwrap()
            .iter()
            .any(|(stored, _)| stored == &key)
    );
    let after =
        ContentRefreshChecks::get_scoped(&state.data, &"instance".into())
            .await
            .unwrap();
    assert_eq!(after.len(), successful.len() + 1);
    for (key, record) in successful {
        assert_eq!(
            ContentRefreshChecks::get(&state.data, &key)
                .await
                .unwrap()
                .unwrap(),
            record
        );
    }
    assert!(after.iter().any(|(_, check)| check.context.sha1
        == sha1_smol::Sha1::from(b"new").hexdigest()));
    assert_eq!(provider.queries.load(Ordering::SeqCst), 1);
    state.content_refresh.shutdown().await;
    state.pool.close().await;
}

#[tokio::test]
async fn verified_install_seeds_a_baseline_without_a_second_scan_read_and_rejects_changed_attributes()
 {
    use crate::state::store::content_file_fingerprints::ContentFileFingerprints;
    let (temp, state, _) = fixture().await;
    let records = content_snapshot::get(&state.data, "instance")
        .await
        .unwrap();
    let root = crate::state::instances::commands::instance_content_root(
        &state.directories,
        &records.instance,
    )
    .unwrap();
    let source = temp.path().join("verified.zip");
    std::fs::write(&source, b"new").unwrap();
    let sha1 = sha1_smol::Sha1::from(b"new").hexdigest();
    let destination = root.join("resourcepacks/fast.zip");
    let (_, modified) =
        crate::state::materialize_verified_project_download_copy(
            &source,
            &destination,
            3,
            &sha1,
        )
        .await
        .unwrap();
    sqlx::query("UPDATE instance_files SET sha1=? WHERE id='fast'")
        .bind(&sha1)
        .execute(&state.pool)
        .await
        .unwrap();
    let integrity = ArtifactIntegrity {
        size: Some(3),
        hashes: vec![ContentHash {
            algorithm: HashAlgorithm::Sha1,
            value: sha1.clone(),
        }],
    };
    state
        .file_fingerprints
        .seed_verified(
            &state,
            "instance",
            &root,
            "resourcepacks/fast.zip",
            integrity.clone(),
            modified.clone(),
        )
        .await
        .unwrap();
    let saved = ContentFileFingerprints::get_current(
        &state.data,
        "instance",
        "resourcepacks/fast.zip",
    )
    .await
    .unwrap()
    .unwrap();
    let reads = state.file_fingerprints.read_count.load(Ordering::SeqCst);
    let resolved = state
        .file_fingerprints
        .resolve_scan(
            &root,
            "resourcepacks/fast.zip",
            Some(&saved),
            false,
            &tokio_util::sync::CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(resolved.record.unwrap().sha1(), sha1);
    assert_eq!(
        state.file_fingerprints.read_count.load(Ordering::SeqCst),
        reads
    );
    let file = std::fs::File::options()
        .write(true)
        .open(&destination)
        .unwrap();
    file.set_times(std::fs::FileTimes::new().set_modified(
        std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(100),
    ))
    .unwrap();
    ContentFileFingerprints::set_scoped(&state.data, &"instance".into(), &[])
        .await
        .unwrap();
    state
        .file_fingerprints
        .seed_verified(
            &state,
            "instance",
            &root,
            "resourcepacks/fast.zip",
            integrity,
            modified,
        )
        .await
        .unwrap();
    assert!(
        ContentFileFingerprints::get_current(
            &state.data,
            "instance",
            "resourcepacks/fast.zip"
        )
        .await
        .unwrap()
        .is_none()
    );
    state.content_refresh.shutdown().await;
    state.pool.close().await;
}

#[tokio::test]
async fn exact_saved_provider_references_skip_unnecessary_hash_identification()
{
    let (_temp, state, provider) = fixture().await;
    sqlx::query("DELETE FROM resource_installed_artifacts")
        .execute(&state.pool)
        .await
        .unwrap();
    for project in ["fast", "slow"] {
        sqlx::query("INSERT INTO instance_content_provider_refs(content_entry_id,provider,provider_project_id,provider_release_id,is_origin) VALUES (?,'modrinth',?,'old',1)")
            .bind(project).bind(project).execute(&state.pool).await.unwrap();
    }
    state
        .content_refresh
        .start(state.clone(), "instance".into(), RefreshMode::Normal)
        .await
        .unwrap();
    finished(&state).await;
    let checks =
        ContentRefreshChecks::get_scoped(&state.data, &"instance".into())
            .await
            .unwrap();
    assert_eq!(checks.len(), 2);
    assert!(
        checks
            .iter()
            .all(|(_, check)| check.attempt == CheckAttempt::Succeeded
                && check.last_success.as_ref().unwrap().candidate.is_some())
    );
    assert_eq!(provider.identify_queries.load(Ordering::SeqCst), 0);
    state.content_refresh.shutdown().await;
    state.pool.close().await;
}

#[tokio::test]
async fn all_refresh_content_kinds_recover_candidates_and_reuse_hashes() {
    for (folder, extension, kind) in [
        ("mods", "jar", "mod"),
        ("resourcepacks", "zip", "resourcepack"),
        ("shaderpacks", "zip", "shaderpack"),
        ("saves/Test World/datapacks", "zip", "datapack"),
    ] {
        let (_temp, state, _) = fixture().await;
        let records = content_snapshot::get(&state.data, "instance")
            .await
            .unwrap();
        let root = crate::state::instances::commands::instance_content_root(
            &state.directories,
            &records.instance,
        )
        .unwrap();
        std::fs::create_dir_all(root.join(folder)).unwrap();
        if folder != "resourcepacks" {
            for project in ["fast", "slow"] {
                let path = format!("{folder}/{project}.{extension}");
                std::fs::rename(
                    root.join(format!("resourcepacks/{project}.zip")),
                    root.join(&path),
                )
                .unwrap();
                sqlx::query("UPDATE instance_files SET relative_path=?,file_name=? WHERE id=?").bind(&path).bind(format!("{project}.{extension}")).bind(project).execute(&state.pool).await.unwrap();
                sqlx::query("UPDATE instance_content_entries SET project_type=? WHERE id=?").bind(kind).bind(project).execute(&state.pool).await.unwrap();
            }
        }
        sqlx::query("DELETE FROM resource_installed_artifacts")
            .execute(&state.pool)
            .await
            .unwrap();
        for project in ["fast", "slow"] {
            sqlx::query("INSERT INTO instance_content_provider_refs(content_entry_id,provider,provider_project_id,provider_release_id,is_origin) VALUES (?,'modrinth',?,'old',1)")
                .bind(project).bind(project).execute(&state.pool).await.unwrap();
        }
        state
            .content_refresh
            .start(state.clone(), "instance".into(), RefreshMode::Normal)
            .await
            .unwrap();
        finished(&state).await;
        let recovered = state
            .content_refresh
            .state(&state, "instance")
            .await
            .unwrap();
        assert_eq!(recovered.snapshot.items.len(), 2, "{folder}");
        assert_eq!(
            recovered
                .snapshot
                .items
                .iter()
                .filter(|item| item.resource_update.is_some())
                .count(),
            2,
            "{folder}"
        );
        let reads = state.file_fingerprints.read_count.load(Ordering::SeqCst);
        if folder.starts_with("saves/") {
            let scope: String = sqlx::query_scalar("SELECT scope FROM resource_installed_artifacts WHERE entry_id='fast'").fetch_one(&state.pool).await.unwrap();
            assert_eq!(
                serde_json::from_str::<InstallScope>(&scope).unwrap(),
                InstallScope::World {
                    relative_path: "saves/Test World".into()
                }
            );
        }
        for _ in 0..10 {
            state
                .content_refresh
                .start(
                    state.clone(),
                    "instance".into(),
                    RefreshMode::ForceRemote,
                )
                .await
                .unwrap();
            finished(&state).await;
        }
        assert_eq!(
            state.file_fingerprints.read_count.load(Ordering::SeqCst),
            reads,
            "{folder}"
        );
        state
            .content_refresh
            .slots
            .lock()
            .await
            .get_mut("instance")
            .unwrap()
            .background_failed = true;
        state
            .content_refresh
            .start(state.clone(), "instance".into(), RefreshMode::RetryFailed)
            .await
            .unwrap();
        finished(&state).await;
        assert!(
            !state
                .content_refresh
                .slots
                .lock()
                .await
                .get("instance")
                .unwrap()
                .background_failed
        );
        let checks =
            ContentRefreshChecks::get_scoped(&state.data, &"instance".into())
                .await
                .unwrap();
        assert_eq!(checks.len(), 2);
        assert_eq!(
            state
                .content_refresh
                .state(&state, "instance")
                .await
                .unwrap()
                .task
                .unwrap()
                .checked,
            0
        );
        state.content_refresh.shutdown().await;
        state.pool.close().await;
    }
}

#[tokio::test]
async fn refresh_rows_fill_source_batches_without_increasing_network_concurrency()
 {
    let (_temp, state, provider) = fixture().await;
    let records = content_snapshot::get(&state.data, "instance")
        .await
        .unwrap();
    let root = crate::state::instances::commands::instance_content_root(
        &state.directories,
        &records.instance,
    )
    .unwrap();
    let hash = sha1_smol::Sha1::from(b"old").hexdigest();
    for number in 0..62 {
        let project = format!("project-{number}");
        let path = format!("resourcepacks/{project}.zip");
        std::fs::write(root.join(&path), b"old").unwrap();
        sqlx::query("INSERT INTO instance_files(id,instance_id,relative_path,file_name,enabled,sha1,size,missing,added_at,modified_at) VALUES (?,'instance',?,?,1,?,3,0,1,1)")
            .bind(&project).bind(&path).bind(format!("{project}.zip")).bind(&hash).execute(&state.pool).await.unwrap();
        sqlx::query("INSERT INTO instance_content_entries(id,instance_id,content_set_id,file_id,project_type,source_kind,server_requirement,client_requirement,enabled,added_at,modified_at) VALUES (?,'instance','set',?,'resourcepack','local','optional','optional',1,1,1)")
            .bind(&project).bind(&project).execute(&state.pool).await.unwrap();
        sqlx::query("INSERT INTO resource_installed_artifacts(entry_id,artifact,scope) VALUES (?,?,'{\"kind\":\"instance\"}')")
            .bind(&project).bind(serde_json::to_string(&artifact(&project,"old",&hash)).unwrap()).execute(&state.pool).await.unwrap();
    }
    state
        .content_refresh
        .start(state.clone(), "instance".into(), RefreshMode::Normal)
        .await
        .unwrap();
    finished(&state).await;
    let snapshot = state
        .content_refresh
        .state(&state, "instance")
        .await
        .unwrap();
    assert_eq!(
        snapshot
            .snapshot
            .items
            .iter()
            .filter(|item| item.resource_update.is_some())
            .count(),
        64
    );
    assert!(provider.largest_batch.load(Ordering::SeqCst) >= 8);
    // Each fallback catalog is intentionally scalar in this fixture; release
    // and project requests must be shared across the rows instead of adding 128 more calls.
    assert!(provider.batches.load(Ordering::SeqCst) < 128);
    state.content_refresh.shutdown().await;
    state.pool.close().await;
}

#[tokio::test]
async fn failed_database_commit_never_publishes_and_retry_revisits_only_unsaved_entries()
 {
    let (_temp, state, _) = fixture().await;
    let task = state
        .content_refresh
        .start(state.clone(), "instance".into(), RefreshMode::Normal)
        .await
        .unwrap();
    finished(&state).await;
    let key = RefreshCheckKey {
        instance_id: "instance".into(),
        entry_id: "fast".into(),
    };
    let peer = RefreshCheckKey {
        instance_id: "instance".into(),
        entry_id: "slow".into(),
    };
    let previous = ContentRefreshChecks::get(&state.data, &key)
        .await
        .unwrap()
        .unwrap();
    let previous_peer = ContentRefreshChecks::get(&state.data, &peer)
        .await
        .unwrap()
        .unwrap();
    let mut no_update = previous.clone();
    no_update.succeeded(chrono::Utc::now().timestamp(), None);
    let write = RefreshWrite {
        entry_id: "fast".into(),
        file_id: "fast".into(),
        context: previous.context.clone(),
        check: Some(no_update),
        presentation: None,
        artifact: None,
    };
    sqlx::query("CREATE TRIGGER reject_refresh_result BEFORE INSERT ON content_refresh_checks BEGIN SELECT RAISE(FAIL, 'simulated write failure'); END").execute(&state.pool).await.unwrap();
    let mut events = state.content_refresh.subscribe();
    assert!(
        state
            .content_refresh
            .save(&state, "instance", &task.id, vec![write])
            .await
            .is_err()
    );
    assert!(matches!(
        events.try_recv(),
        Err(tokio::sync::broadcast::error::TryRecvError::Empty)
    ));
    assert_eq!(
        ContentRefreshChecks::get(&state.data, &key)
            .await
            .unwrap()
            .unwrap(),
        previous
    );
    assert!(
        state
            .content_refresh
            .slots
            .lock()
            .await
            .get("instance")
            .unwrap()
            .unsaved_entries
            .contains("fast")
    );
    sqlx::query("DROP TRIGGER reject_refresh_result")
        .execute(&state.pool)
        .await
        .unwrap();
    state
        .content_refresh
        .start(state.clone(), "instance".into(), RefreshMode::RetryFailed)
        .await
        .unwrap();
    finished(&state).await;
    let current = state
        .content_refresh
        .state(&state, "instance")
        .await
        .unwrap();
    assert_eq!(current.task.unwrap().checked, 1);
    assert!(
        state
            .content_refresh
            .slots
            .lock()
            .await
            .get("instance")
            .unwrap()
            .unsaved_entries
            .is_empty()
    );
    assert_eq!(
        ContentRefreshChecks::get(&state.data, &peer)
            .await
            .unwrap()
            .unwrap(),
        previous_peer
    );
    assert!(
        current
            .snapshot
            .items
            .iter()
            .find(|item| item.entry_id.as_deref() == Some("fast"))
            .unwrap()
            .resource_update
            .is_some()
    );
    state.content_refresh.shutdown().await;
    state.pool.close().await;
}

async fn fixture() -> (tempfile::TempDir, Arc<crate::State>, Arc<Provider>) {
    let temp = tempfile::tempdir().unwrap();
    let directories = crate::state::DirectoryInfo {
        settings_dir: temp.path().into(),
        config_dir: temp.path().into(),
        app_identifier: "content-refresh-test".into(),
    };
    let root = directories.instances_dir().join("example");
    std::fs::create_dir_all(root.join("resourcepacks")).unwrap();
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(temp.path().join("app.db"))
                .create_if_missing(true)
                .foreign_keys(true),
        )
        .await
        .unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();
    sqlx::raw_sql("INSERT INTO instances(id, path, name, install_stage, launcher_feature_version, created, modified) VALUES ('instance', 'example', 'Example', 'installed', '1', 1, 1); INSERT INTO instance_content_sets(id, instance_id, name, source_kind, status, game_version, loader, created, modified) VALUES ('set', 'instance', 'Main', 'local', 'available', '1.21', 'fabric', 1, 1); UPDATE instances SET applied_content_set_id = 'set' WHERE id = 'instance';").execute(&pool).await.unwrap();
    for project in ["fast", "slow"] {
        std::fs::write(
            root.join(format!("resourcepacks/{project}.zip")),
            b"old",
        )
        .unwrap();
        let hash = sha1_smol::Sha1::from(b"old").hexdigest();
        sqlx::query("INSERT INTO instance_files(id, instance_id, relative_path, file_name, enabled, sha1, size, added_at, modified_at) VALUES (?, 'instance', ?, ?, 1, ?, 3, 1, 1)").bind(project).bind(format!("resourcepacks/{project}.zip")).bind(format!("{project}.zip")).bind(&hash).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO instance_content_entries(id, instance_id, content_set_id, file_id, project_type, source_kind, server_requirement, client_requirement, enabled, added_at, modified_at) VALUES (?, 'instance', 'set', ?, 'resourcepack', 'local', 'optional', 'optional', 1, 1, 1)").bind(project).bind(project).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO resource_installed_artifacts(entry_id, artifact, scope) VALUES (?, ?, '{\"kind\":\"instance\"}')").bind(project).bind(serde_json::to_string(&artifact(project, "old", &hash)).unwrap()).execute(&pool).await.unwrap();
    }
    let state = crate::state::test_state(directories, pool).await.unwrap();
    let provider = Arc::new(Provider {
        slow: Notify::new(),
        hold_slow: AtomicBool::new(false),
        fail: AtomicBool::new(false),
        queries: AtomicUsize::new(0),
        identify_queries: AtomicUsize::new(0),
        batches: AtomicUsize::new(0),
        largest_batch: AtomicUsize::new(0),
    });
    let cache = Arc::new(
        crate::resources::cache::ResourceCache::open(
            temp.path().join("resources.db"),
        )
        .await,
    );
    let service = ResourceService::new(cache, vec![provider.clone()]).unwrap();
    state.set_resource_service_for_test(service);
    (temp, state, provider)
}

async fn finished(state: &crate::State) {
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        loop {
            if state
                .content_refresh
                .state(state, "instance")
                .await
                .unwrap()
                .task
                .is_some_and(|task| !task.running)
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn fast_results_are_saved_before_slow_work_finishes_and_survive_unsubscribe()
 {
    let (_temp, state, provider) = fixture().await;
    provider.hold_slow.store(true, Ordering::SeqCst);
    let mut events = state.content_refresh.subscribe();
    state
        .content_refresh
        .start(state.clone(), "instance".into(), RefreshMode::Normal)
        .await
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let event = events.recv().await.unwrap();
            if matches!(event.patch, RefreshPatch::Entry { ref entry_id, update: Some(ref update), .. } if entry_id == "fast" && update.candidate.is_some()) { break; }
        }
    }).await.unwrap();
    let current = state
        .content_refresh
        .state(&state, "instance")
        .await
        .unwrap();
    assert!(current.task.unwrap().running);
    assert!(
        current
            .snapshot
            .items
            .iter()
            .find(|item| item.entry_id.as_deref() == Some("fast"))
            .unwrap()
            .resource_update
            .is_some()
    );
    drop(events);
    provider.hold_slow.store(false, Ordering::SeqCst);
    provider.slow.notify_waiters();
    finished(&state).await;
    let saved =
        ContentRefreshChecks::get_scoped(&state.data, &"instance".into())
            .await
            .unwrap();
    assert_eq!(saved.len(), 2);
    assert!(saved.iter().all(|(_, check)| {
        check.last_success.as_ref().unwrap().candidate.is_some()
    }));
    state.content_refresh.shutdown().await;
    state.pool.close().await;
}

#[tokio::test]
async fn repeat_requests_join_and_force_remote_bypasses_recent_success_without_rehashing()
 {
    let (_temp, state, provider) = fixture().await;
    provider.hold_slow.store(true, Ordering::SeqCst);
    let first = state
        .content_refresh
        .start(state.clone(), "instance".into(), RefreshMode::Normal)
        .await
        .unwrap();
    for _ in 0..10 {
        let next = state
            .content_refresh
            .start(state.clone(), "instance".into(), RefreshMode::ForceRemote)
            .await
            .unwrap();
        assert_eq!(next.id, first.id);
    }
    provider.hold_slow.store(false, Ordering::SeqCst);
    provider.slow.notify_waiters();
    finished(&state).await;
    let before = provider.queries.load(Ordering::SeqCst);
    let hashes = state.file_fingerprints.read_count.load(Ordering::SeqCst);
    let records = content_snapshot::get(&state.data, "instance")
        .await
        .unwrap();
    assert!(records.presentations.values().all(|presentation| {
        presentation.content.owner.as_ref().is_some_and(|owner| {
            owner.name == "ExampleAuthor"
                && owner.id == "author000"
                && owner.avatar_url.as_deref()
                    == Some("https://cdn.modrinth.com/avatars/author.png")
        })
    }));
    let mut aged = records;
    for (_, check) in &mut aged.checks {
        check.last_success.as_mut().unwrap().checked_at -=
            REMOTE_CACHE_SECONDS - 60;
    }
    for presentation in aged.presentations.values_mut() {
        presentation.saved_at -= REMOTE_CACHE_SECONDS - 60;
    }
    assert!(
        entry::work(&aged, ["fast", "slow"].into_iter(), RefreshMode::Normal)
            .is_empty()
    );
    assert_eq!(
        entry::work(
            &aged,
            ["fast", "slow"].into_iter(),
            RefreshMode::ForceRemote
        )
        .len(),
        2
    );
    for (_, check) in &mut aged.checks {
        check.last_success.as_mut().unwrap().checked_at -= 120;
    }
    for presentation in aged.presentations.values_mut() {
        presentation.saved_at -= 120;
    }
    assert_eq!(
        entry::work(&aged, ["fast", "slow"].into_iter(), RefreshMode::Normal)
            .len(),
        2
    );
    state
        .content_refresh
        .start(state.clone(), "instance".into(), RefreshMode::Normal)
        .await
        .unwrap();
    finished(&state).await;
    assert_eq!(provider.queries.load(Ordering::SeqCst), before);
    assert_eq!(
        state.file_fingerprints.read_count.load(Ordering::SeqCst),
        hashes
    );
    provider.fail.store(true, Ordering::SeqCst);
    state
        .content_refresh
        .start(state.clone(), "instance".into(), RefreshMode::ForceRemote)
        .await
        .unwrap();
    finished(&state).await;
    assert!(provider.queries.load(Ordering::SeqCst) > before);
    let saved =
        ContentRefreshChecks::get_scoped(&state.data, &"instance".into())
            .await
            .unwrap();
    assert!(
        saved
            .iter()
            .all(|(_, check)| check.attempt == CheckAttempt::Failed
                && check.last_success.as_ref().unwrap().candidate.is_some())
    );
    state.content_refresh.shutdown().await;
    state.pool.close().await;
}

#[tokio::test]
async fn conditional_result_writes_reject_changed_files_and_changed_target() {
    let (_temp, state, _) = fixture().await;
    let records = content_snapshot::get(&state.data, "instance")
        .await
        .unwrap();
    let mut context = RefreshCheckContext {
        location_key: location_key(&records.instance),
        content_root: "root".into(),
        sha1: sha1_smol::Sha1::from(b"old").hexdigest(),
        size: 3,
        current_release: Some(reference("fast", "old")),
        target: crate::resources::ipc::ResourceTarget {
            game_versions: vec!["1.21".into()],
            loaders: vec!["fabric".into()],
            channels: plans::update_channels(records.instance.update_channel),
        },
        preferred_source: records.preferred_source,
    };
    let mut check = RefreshCheckRecord::for_context(
        None,
        "fast".into(),
        context.clone(),
        1,
    );
    check.succeeded(1, Some(reference("fast", "new")));
    let write = RefreshWrite {
        entry_id: "fast".into(),
        file_id: "fast".into(),
        context: context.clone(),
        check: Some(check),
        presentation: None,
        artifact: None,
    };
    assert_eq!(
        set_results(&state.data, "instance", vec![write.clone()])
            .await
            .unwrap()
            .len(),
        1
    );
    context.target.game_versions = vec!["other".into()];
    let invalid = RefreshWrite {
        context,
        ..write.clone()
    };
    assert!(
        set_results(&state.data, "instance", vec![invalid])
            .await
            .unwrap()
            .is_empty()
    );
    sqlx::query("UPDATE instance_files SET sha1 = ? WHERE id = 'fast'")
        .bind("c".repeat(40))
        .execute(&state.pool)
        .await
        .unwrap();
    assert!(
        set_results(&state.data, "instance", vec![write])
            .await
            .unwrap()
            .is_empty()
    );
    state.pool.close().await;
}

#[tokio::test]
async fn shutdown_cancels_the_active_check_and_preserves_already_saved_results()
{
    let (_temp, state, provider) = fixture().await;
    provider.hold_slow.store(true, Ordering::SeqCst);
    let mut events = state.content_refresh.subscribe();
    state
        .content_refresh
        .start(state.clone(), "instance".into(), RefreshMode::Normal)
        .await
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let event = events.recv().await.unwrap();
            if matches!(event.patch, RefreshPatch::Entry { ref entry_id, update: Some(ref update), .. } if entry_id == "fast" && update.candidate.is_some()) { break; }
        }
    }).await.unwrap();
    state.content_refresh.shutdown().await;
    assert!(
        !state
            .content_refresh
            .state(&state, "instance")
            .await
            .unwrap()
            .task
            .unwrap()
            .running
    );
    let saved =
        ContentRefreshChecks::get_scoped(&state.data, &"instance".into())
            .await
            .unwrap();
    assert!(saved.iter().any(|(key, check)| key.entry_id == "fast"
        && check.last_success.as_ref().unwrap().candidate.is_some()));
    assert!(!saved.iter().any(|(key, _)| key.entry_id == "slow"));
    assert!(
        state
            .content_refresh
            .start(state.clone(), "instance".into(), RefreshMode::Normal)
            .await
            .is_err()
    );
    state.pool.close().await;
}

#[tokio::test]
async fn a_disk_edit_during_network_work_cannot_publish_a_candidate_for_the_old_bytes()
 {
    let (_temp, state, provider) = fixture().await;
    provider.hold_slow.store(true, Ordering::SeqCst);
    let mut events = state.content_refresh.subscribe();
    state
        .content_refresh
        .start(state.clone(), "instance".into(), RefreshMode::Normal)
        .await
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let event = events.recv().await.unwrap();
            if matches!(event.patch, RefreshPatch::Entry { ref entry_id, update: Some(ref update), .. } if entry_id == "fast" && update.candidate.is_some()) { break; }
        }
    }).await.unwrap();
    let path = state
        .directories
        .instances_dir()
        .join("example/resourcepacks/slow.zip");
    let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
    std::fs::write(&path, b"new").unwrap();
    std::fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(modified + std::time::Duration::from_secs(1))
        .unwrap();
    provider.hold_slow.store(false, Ordering::SeqCst);
    provider.slow.notify_waiters();
    finished(&state).await;
    let saved =
        ContentRefreshChecks::get_scoped(&state.data, &"instance".into())
            .await
            .unwrap();
    assert!(!saved.iter().any(|(key, _)| key.entry_id == "slow"));
    state.content_refresh.shutdown().await;
    state.pool.close().await;
}

#[tokio::test]
async fn retry_failed_entries_does_not_requery_successful_peers() {
    let (_temp, state, provider) = fixture().await;
    state
        .content_refresh
        .start(state.clone(), "instance".into(), RefreshMode::Normal)
        .await
        .unwrap();
    finished(&state).await;
    let fast = ContentRefreshChecks::get(
        &state.data,
        &RefreshCheckKey {
            instance_id: "instance".into(),
            entry_id: "fast".into(),
        },
    )
    .await
    .unwrap()
    .unwrap();
    let mut slow = ContentRefreshChecks::get(
        &state.data,
        &RefreshCheckKey {
            instance_id: "instance".into(),
            entry_id: "slow".into(),
        },
    )
    .await
    .unwrap()
    .unwrap();
    slow.failed(chrono::Utc::now().timestamp(), "offline".into());
    ContentRefreshChecks::set(
        &state.data,
        &RefreshCheckKey {
            instance_id: "instance".into(),
            entry_id: "slow".into(),
        },
        Some(&slow),
    )
    .await
    .unwrap();
    provider.queries.store(0, Ordering::SeqCst);
    state
        .content_refresh
        .start(state.clone(), "instance".into(), RefreshMode::RetryFailed)
        .await
        .unwrap();
    finished(&state).await;
    assert_eq!(
        ContentRefreshChecks::get(
            &state.data,
            &RefreshCheckKey {
                instance_id: "instance".into(),
                entry_id: "fast".into()
            }
        )
        .await
        .unwrap()
        .unwrap(),
        fast
    );
    assert_eq!(
        ContentRefreshChecks::get(
            &state.data,
            &RefreshCheckKey {
                instance_id: "instance".into(),
                entry_id: "slow".into()
            }
        )
        .await
        .unwrap()
        .unwrap()
        .attempt,
        CheckAttempt::Succeeded
    );
    assert!(provider.queries.load(Ordering::SeqCst) < 6);
    state.content_refresh.shutdown().await;
    state.pool.close().await;
}

#[tokio::test]
async fn pack_failures_survive_reload_and_retry_preserves_success_until_recovered()
 {
    use crate::state::store::content_refresh_pack::*;
    let (_temp, state, _) = fixture().await;
    let records = content_snapshot::get_with_saved_pack(
        &state.data,
        &state.derived,
        "instance",
    )
    .await
    .unwrap();
    let location = location_key(&records.instance);
    let link = serde_json::to_string(&records.link).unwrap();
    let key = PackKey {
        instance_id: "instance".into(),
        content_set_id: "set".into(),
    };
    let pack = crate::state::InstanceContentPack {
        resource_id: None,
        name: "Saved pack".into(),
        icon_path: None,
        provider: None,
        project_id: None,
        version_id: None,
        reconciled: true,
        can_update: false,
        metadata: None,
    };
    let success = pack_attempt(
        None,
        location.clone(),
        link.clone(),
        100,
        &Ok(Some(pack)),
    );
    let failed = pack_attempt(
        Some(success),
        location.clone(),
        link.clone(),
        110,
        &Err(crate::ErrorKind::InputError("offline".into()).into()),
    );
    ContentRefreshPacks::upsert(&state.derived, &key, &failed)
        .await
        .unwrap();
    let restored = content_snapshot::get_with_saved_pack(
        &state.data,
        &state.derived,
        "instance",
    )
    .await
    .unwrap()
    .pack_presentation
    .unwrap();
    assert_eq!(restored.saved_at, 100);
    assert_eq!(restored.pack.as_ref().unwrap().name, "Saved pack");
    assert!(restored.last_error.is_some());
    assert!(pack_needs_refresh(
        Some(&restored),
        RefreshMode::RetryFailed,
        111
    ));
    assert!(!pack_needs_refresh(
        Some(&restored),
        RefreshMode::Normal,
        110 + REMOTE_CACHE_SECONDS - 1
    ));
    assert!(pack_needs_refresh(
        Some(&restored),
        RefreshMode::Normal,
        110 + REMOTE_CACHE_SECONDS
    ));
    let recovered =
        pack_attempt(Some(restored), location, link, 120, &Ok(None));
    ContentRefreshPacks::upsert(&state.derived, &key, &recovered)
        .await
        .unwrap();
    let restored = ContentRefreshPacks::saved(&state.derived, &key)
        .await
        .unwrap()
        .unwrap();
    assert!(restored.last_error.is_none());
    assert!(restored.pack.is_none());
    assert!(!pack_needs_refresh(
        Some(&restored),
        RefreshMode::RetryFailed,
        121
    ));
    assert!(!pack_needs_refresh(None, RefreshMode::RetryFailed, 121));
    assert!(pack_needs_refresh(
        Some(&restored),
        RefreshMode::ForceRemote,
        121
    ));
    state.content_refresh.shutdown().await;
    state.pool.close().await;
}

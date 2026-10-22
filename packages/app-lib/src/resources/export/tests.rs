use super::{plan::*, refresh::*};
use crate::resources::{cache::ResourceCache, providers::*, *};
use async_trait::async_trait;
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio_util::sync::CancellationToken;

struct Provider {
    source: SourceId,
    calls: Mutex<Vec<ProviderQuery>>,
    failed: AtomicBool,
    matched: bool,
}

#[tokio::test]
async fn cleanroom_is_only_supported_by_the_generic_index() {
    let (_, service, _, _) = setup(false).await;
    let mut draft = draft(vec![]);
    draft.scan.requirements.loader = Some("cleanroom".into());
    draft.scan.requirements.loader_version = Some("0.3.0".into());
    draft.selection_revision = 1;
    let info = ExportPackageInfo {
        name: "Test".into(),
        version: "1".into(),
        summary: "".into(),
        author: None,
    };
    assert!(
        build(
            &draft,
            1,
            ResourceExportFormat::Modrinth,
            info.clone(),
            vec![],
            &SourceId::modrinth()
        )
        .is_err()
    );
    let generic = build(
        &draft,
        1,
        ResourceExportFormat::GenericZip,
        info,
        vec![],
        &SourceId::modrinth(),
    )
    .unwrap();
    let (_, manifest) = super::formats::manifest(&generic).unwrap();
    assert_eq!(manifest["dependencies"]["cleanroom"], "0.3.0");
    service.cache().close().await;
}

#[tokio::test]
async fn identified_connector_artifact_with_failed_release_refresh_is_a_failure()
 {
    struct Connector;
    #[async_trait]
    impl ResourceProvider for Connector {
        fn source(&self) -> ResourceSource {
            ResourceSource {
                id: SourceId::mcarchive(),
                available: true,
                types: vec![ResourceType::Mod],
                capabilities: vec![
                    SourceCapability::IdentifyHash,
                    SourceCapability::Releases,
                ],
                supported_search_sorts: vec![],
            }
        }
        fn namespace(&self) -> String {
            "connector-failure".into()
        }
        fn batch_limit(&self, _: &ProviderQuery) -> usize {
            50
        }
        async fn fetch_many(
            &self,
            queries: &[ProviderQuery],
        ) -> Vec<ResourceResult<ProviderValue>> {
            queries.iter().map(|query| match query {
                ProviderQuery::Identify { integrity, .. } => Ok(ProviderValue::Matches(vec![ResourceArtifact { reference: SourceArtifactRef { release: SourceReleaseRef { project: SourceProjectRef { source: SourceId::mcarchive(), project_id: SourceProjectId::new("b319f41b-13f0-4c78-b95b-439640bf4606").unwrap() }, release_id: SourceReleaseId::new("release1").unwrap() }, artifact_id: SourceArtifactId::new("artifact1").unwrap() }, file_name: Some("file.jar".into()), role: ArtifactRole::Unknown, available: Some(true), integrity: integrity.clone(), acquisition: ArtifactAcquisition::Unavailable, dependencies: vec![] }])),
                ProviderQuery::Release { .. } => Err(ResourceError::Network { status: Some(503) }),
                _ => panic!("unexpected query"),
            }).collect()
        }
    }
    let temp = tempfile::tempdir().unwrap();
    let cache =
        Arc::new(ResourceCache::open(temp.path().join("cache.db")).await);
    let service =
        ResourceService::new(cache.clone(), vec![Arc::new(Connector)]).unwrap();
    let mut file = file("file.jar");
    file.integrity.hashes.push(ContentHash {
        algorithm: HashAlgorithm::Sha256,
        value: "a".repeat(64),
    });
    let mut draft = draft(vec![file.clone()]);
    let resolutions = resolve_source(
        &service,
        SourceId::mcarchive(),
        &[(file, 0)],
        &HashMap::new(),
        &mut draft,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert!(matches!(
        resolutions[0],
        ExportSourceResolution::Failed { .. }
    ));
    cache.close().await;
}

#[tokio::test]
async fn explicit_refresh_rehashes_bytes_even_when_file_size_and_mtime_are_preserved()
 {
    let (root, service, _, _) = setup(false).await;
    let path = root.path().join("file.jar");
    std::fs::write(&path, b"old bytes").unwrap();
    let mut draft = draft(vec![file("file.jar")]);
    run(
        &service,
        &mut draft,
        root.path(),
        vec!["file.jar".into()],
        1,
    )
    .await;
    let old = draft.results["file.jar"].snapshot.integrity.clone();
    let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
    std::fs::write(&path, b"new bytes").unwrap();
    let handle = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
    handle
        .set_times(std::fs::FileTimes::new().set_modified(modified))
        .unwrap();
    drop(handle);
    run(
        &service,
        &mut draft,
        root.path(),
        vec!["file.jar".into()],
        2,
    )
    .await;
    assert_ne!(draft.results["file.jar"].snapshot.integrity, old);
    assert_eq!(draft.results["file.jar"].snapshot.revision, 2);
}

#[tokio::test]
async fn export_reference_reads_are_local_and_do_not_create_business_identity()
{
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();
    assert!(known_references(&pool, "unknown").await.unwrap().is_empty());
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM resources")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    pool.close().await;
}

#[tokio::test]
async fn session_addition_cache_clear_and_archive_signature_do_not_expand_the_file_set()
 {
    let (root, service, mr, _) = setup(false).await;
    std::fs::write(root.path().join("a.jar"), "one").unwrap();
    std::fs::write(root.path().join("b.bin"), b"PK\x03\x04test").unwrap();
    let mut draft = draft(vec![file("a.jar"), file("b.bin")]);
    run(&service, &mut draft, root.path(), vec!["a.jar".into()], 1).await;
    let before = mr.calls.lock().unwrap().len();
    service.cache().clear().await.unwrap();
    let result = run(
        &service,
        &mut draft,
        root.path(),
        vec!["a.jar".into(), "b.bin".into()],
        2,
    )
    .await;
    assert_eq!(before + 1, mr.calls.lock().unwrap().len());
    assert_eq!(result.files.len(), 2);
    assert!(result.files[1].snapshot.identifiable);
    std::fs::write(root.path().join("new.jar"), "new").unwrap();
    let result =
        run(&service, &mut draft, root.path(), vec!["a.jar".into()], 3).await;
    assert_eq!(result.files.len(), 1);
    assert_eq!(before + 1, mr.calls.lock().unwrap().len());
    let bad = refresh_local(
        &service,
        &mut draft,
        root.path(),
        vec!["new.jar".into()],
        4,
        RequestId::new("new").unwrap(),
        HashMap::new(),
        &CancellationToken::new(),
        &|_| {},
    )
    .await;
    assert!(matches!(bad, Err(ResourceError::InvalidRequest(_))));
}
#[async_trait]
impl ResourceProvider for Provider {
    fn source(&self) -> ResourceSource {
        ResourceSource {
            id: self.source.clone(),
            available: true,
            types: vec![ResourceType::Mod],
            capabilities: vec![
                SourceCapability::IdentifyHash,
                SourceCapability::Project,
                SourceCapability::Releases,
            ],
            supported_search_sorts: vec![],
        }
    }
    fn namespace(&self) -> String {
        "export-test".into()
    }
    fn batch_limit(&self, _: &ProviderQuery) -> usize {
        50
    }
    async fn fetch_many(
        &self,
        queries: &[ProviderQuery],
    ) -> Vec<ResourceResult<ProviderValue>> {
        self.calls.lock().unwrap().extend_from_slice(queries);
        queries.iter().map(|query| {
            if self.failed.load(Ordering::SeqCst) { return Err(ResourceError::Network { status: Some(503) }); }
            match query {
                ProviderQuery::IdentifyExport { integrity, .. } => {
                    if !self.matched { return Ok(ProviderValue::Identified(vec![])); }
                    let reference = SourceReleaseRef { project: SourceProjectRef { source: self.source.clone(), project_id: SourceProjectId::new("12345678").unwrap() }, release_id: SourceReleaseId::new("87654321").unwrap() };
                    let artifact = ResourceArtifact { reference: SourceArtifactRef { release: reference.clone(), artifact_id: SourceArtifactId::new("file.jar").unwrap() }, file_name: Some("file.jar".into()), role: ArtifactRole::Primary, available: Some(true), integrity: integrity.clone(), acquisition: ArtifactAcquisition::PublicDownload { urls: vec!["https://cdn.modrinth.com/file.jar".into()] }, dependencies: vec![] };
                    Ok(ProviderValue::Identified(vec![ReleaseDocument { release: ResourceRelease { reference, version: "1".into(), channel: ReleaseChannel::Release, published_at: None, compatibility: ReleaseCompatibility { game_versions: vec![], loaders: vec![] }, artifacts: vec![artifact], equivalent_releases: vec![] }, changelog: None, protocol: None }]))
                },
                ProviderQuery::Project { id, .. } => Ok(ProviderValue::Project(ProjectDocument { metadata: SourceProjectMetadata { project: SourceProjectRef { source: self.source.clone(), project_id: id.clone() }, slug: None, title: "Test".into(), summary: String::new(), icon_url: None, types: vec![ResourceType::Mod], authors: vec![], license: None, downloads: None, followers: None, created_at: None, project_published_at: None, updated_at: None, first_published_at: None, latest_published_at: None, categories: vec![], game_versions: vec![], loaders: vec![], gallery: vec![], environments: vec![] }, description: ResourceText { source: self.source.clone(), format: TextFormat::Plain, body: String::new() }, protocol: None })),
                _ => panic!("export must not traverse releases or download: {query:?}"),
            }
        }).collect()
    }
}
fn file(path: &str) -> ExportFileSnapshot {
    ExportFileSnapshot {
        relative_path: path.into(),
        integrity: ArtifactIntegrity {
            size: Some(4),
            hashes: vec![],
        },
        revision: 1,
        identifiable: path.ends_with(".jar"),
        modified_at: None,
        category_id: "mods".into(),
        category_selected: true,
    }
}
fn draft(files: Vec<ExportFileSnapshot>) -> ExportDraft {
    ExportDraft::new(ResourceExportScan {
        session_id: SessionId::new(uuid::Uuid::new_v4().to_string()).unwrap(),
        instance_id: "instance".into(),
        instance_revision: 1,
        files,
        categories: vec![],
        excluded_paths: vec![],
        directories: vec![],
        requirements: ExportGameRequirements {
            game_version: "1.21.1".into(),
            loader: None,
            loader_version: None,
        },
    })
}
fn info() -> ExportPackageInfo {
    ExportPackageInfo {
        name: "My package".into(),
        version: "2".into(),
        summary: "Summary".into(),
        author: None,
    }
}
async fn setup(
    matched: bool,
) -> (
    tempfile::TempDir,
    Arc<ResourceService>,
    Arc<Provider>,
    Arc<Provider>,
) {
    let temp = tempfile::tempdir().unwrap();
    let cache = Arc::new(
        ResourceCache::open(temp.path().join("cache/resource-cache.db")).await,
    );
    let mr = Arc::new(Provider {
        source: SourceId::modrinth(),
        calls: Mutex::new(vec![]),
        failed: AtomicBool::new(false),
        matched,
    });
    let cf = Arc::new(Provider {
        source: SourceId::curseforge(),
        calls: Mutex::new(vec![]),
        failed: AtomicBool::new(false),
        matched: false,
    });
    let service =
        ResourceService::new(cache, vec![mr.clone(), cf.clone()]).unwrap();
    (temp, service, mr, cf)
}
async fn run(
    service: &Arc<ResourceService>,
    draft: &mut ExportDraft,
    root: &std::path::Path,
    selected: Vec<String>,
    revision: u32,
) -> ExportResolutionSession {
    refresh_local(
        service,
        draft,
        root,
        selected,
        revision,
        RequestId::new(uuid::Uuid::new_v4().to_string()).unwrap(),
        HashMap::new(),
        &CancellationToken::new(),
        &|_| {},
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn force_refresh_all_selected_unknown_files_bypasses_warm_negative_cache_and_reuses_session_evidence()
 {
    let (root, service, mr, cf) = setup(true).await;
    let mut files = vec![];
    for index in 0..105 {
        let path = format!("file-{index}.jar");
        std::fs::write(root.path().join(&path), format!("{index:04}")).unwrap();
        files.push(file(&path));
    }
    std::fs::write(root.path().join("unselected.jar"), "data").unwrap();
    files.push(file("unselected.jar"));
    std::fs::write(root.path().join("options.txt"), "data").unwrap();
    files.push(file("options.txt"));
    let selected = files
        .iter()
        .filter(|f| f.relative_path != "unselected.jar")
        .map(|f| f.relative_path.clone())
        .collect::<Vec<_>>();
    let mut draft = draft(files);
    let (frozen, fingerprint) =
        freeze(root.path(), &draft.scan.files[0], &CancellationToken::new())
            .await
            .unwrap();
    mr.failed.store(false, Ordering::SeqCst);
    service
        .query(
            &SourceId::modrinth(),
            ProviderQuery::IdentifyExport {
                integrity: frozen.integrity,
                fingerprint: Some(fingerprint),
            },
            CachePolicy::UseFresh,
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    mr.calls.lock().unwrap().clear();
    let (frozen, fingerprint) =
        freeze(root.path(), &draft.scan.files[0], &CancellationToken::new())
            .await
            .unwrap();
    service
        .query(
            &SourceId::curseforge(),
            ProviderQuery::IdentifyExport {
                integrity: frozen.integrity,
                fingerprint: Some(fingerprint),
            },
            CachePolicy::UseFresh,
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    cf.calls.lock().unwrap().clear();
    let result =
        run(&service, &mut draft, root.path(), selected.clone(), 1).await;
    assert_eq!(result.state, ExportResolutionState::Ready);
    assert_eq!(
        mr.calls
            .lock()
            .unwrap()
            .iter()
            .filter(|q| matches!(q, ProviderQuery::IdentifyExport { .. }))
            .count(),
        105
    );
    assert_eq!(cf.calls.lock().unwrap().len(), 105);
    assert!(!mr.calls.lock().unwrap().iter().any(|q| matches!(
        q,
        ProviderQuery::Release { .. } | ProviderQuery::Releases { .. }
    )));
    let before = mr.calls.lock().unwrap().len();
    run(&service, &mut draft, root.path(), selected.clone(), 2).await;
    assert_eq!(before, mr.calls.lock().unwrap().len());
    std::fs::write(root.path().join(&selected[0]), "edit").unwrap();
    run(&service, &mut draft, root.path(), selected.clone(), 3).await;
    assert_eq!(before + 1, mr.calls.lock().unwrap().len());
    let plan = build(
        &draft,
        draft.selection_revision,
        ResourceExportFormat::Modrinth,
        info(),
        vec![],
        &SourceId::modrinth(),
    )
    .unwrap();
    let pack: crate::pack::install_from::PackFormat =
        serde_json::from_value(super::formats::manifest(&plan).unwrap().1)
            .unwrap();
    assert_eq!(pack.files.len(), 105);
    assert_eq!(pack.name, "My package");
    assert!(
        pack.files
            .iter()
            .all(|file| file.env.is_none() && file.hashes.len() == 2)
    );
    let mut new = ExportDraft::new(draft.scan.clone());
    run(&service, &mut new, root.path(), selected, 1).await;
    assert_eq!(before + 107, mr.calls.lock().unwrap().len());
}

#[tokio::test]
async fn source_failure_is_not_cached_success_and_embedding_requires_exact_snapshot()
 {
    let (root, service, mr, cf) = setup(false).await;
    std::fs::write(root.path().join("file.jar"), "data").unwrap();
    let mut draft = draft(vec![file("file.jar")]);
    let first = run(
        &service,
        &mut draft,
        root.path(),
        vec!["file.jar".into()],
        1,
    )
    .await;
    assert_eq!(first.state, ExportResolutionState::Ready);
    let mut reopened = ExportDraft::new(draft.scan.clone());
    cf.failed.store(true, Ordering::SeqCst);
    let result = run(
        &service,
        &mut reopened,
        root.path(),
        vec!["file.jar".into()],
        1,
    )
    .await;
    assert_eq!(result.state, ExportResolutionState::NeedsDecision);
    assert!(validate_choices(&reopened, 1, &[]).is_err());
    let choice = ExportEmbeddedFailureChoice {
        snapshot: result.files[0].snapshot.clone(),
        source: SourceId::curseforge(),
    };
    validate_choices(&reopened, 1, &[choice.clone()]).unwrap();
    let mr_before = mr.calls.lock().unwrap().len();
    cf.failed.store(false, Ordering::SeqCst);
    run(
        &service,
        &mut reopened,
        root.path(),
        vec!["file.jar".into()],
        2,
    )
    .await;
    assert_eq!(mr_before, mr.calls.lock().unwrap().len());
    assert!(validate_choices(&reopened, 2, &[choice]).is_err());
    std::fs::remove_file(root.path().join("file.jar")).unwrap();
    let result = run(
        &service,
        &mut reopened,
        root.path(),
        vec!["file.jar".into()],
        3,
    )
    .await;
    assert!(result.files[0].local_error.is_some());
    assert!(validate_choices(&reopened, 3, &[]).is_err());
}

#[tokio::test]
async fn fingerprint_and_hashes_use_current_bytes_and_cancel_before_network() {
    let (root, service, mr, _) = setup(false).await;
    let bytes = b"file \n\r\tbytes";
    std::fs::write(root.path().join("file.jar"), bytes).unwrap();
    let (_, fingerprint) =
        freeze(root.path(), &file("file.jar"), &CancellationToken::new())
            .await
            .unwrap();
    assert_eq!(
        fingerprint,
        crate::api::curseforge::compute_fingerprint(bytes)
    );
    let cancel = CancellationToken::new();
    cancel.cancel();
    let mut draft = draft(vec![file("file.jar")]);
    let result = refresh_local(
        &service,
        &mut draft,
        root.path(),
        vec!["file.jar".into()],
        1,
        RequestId::new("cancel").unwrap(),
        HashMap::new(),
        &cancel,
        &|_| {},
    )
    .await;
    assert!(matches!(result, Err(ResourceError::Canceled)));
    assert!(mr.calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn three_formats_freeze_current_artifacts_paths_and_package_info() {
    let (root, service, mr, cf) = setup(true).await;
    for path in [
        "mods/file.jar",
        "mods/[中文]file.jar",
        "mods/file.jar.disabled",
        "config/settings.txt",
    ] {
        std::fs::create_dir_all(root.path().join(path).parent().unwrap())
            .unwrap();
        std::fs::write(root.path().join(path), "data").unwrap();
    }
    let paths: Vec<String> = [
        "mods/file.jar",
        "mods/[中文]file.jar",
        "mods/file.jar.disabled",
        "config/settings.txt",
    ]
    .map(String::from)
    .into();
    let mut draft = draft(paths.iter().map(|p| file(p)).collect());
    run(&service, &mut draft, root.path(), paths.clone(), 1).await;
    let original = draft.results[&paths[0]].clone();
    let ExportSourceResolution::Matched { artifacts, .. } =
        &original.resolutions[0]
    else {
        panic!()
    };
    let mut cf_artifact = artifacts[0].clone();
    cf_artifact.reference.release.project.source = SourceId::curseforge();
    let mut project = draft.projects.values().next().unwrap().clone();
    project.metadata.project = cf_artifact.reference.release.project.clone();
    draft
        .projects
        .insert(project.metadata.project.clone(), project);
    for path in [&paths[0], &paths[1]] {
        draft.results.get_mut(path).unwrap().resolutions[1] =
            ExportSourceResolution::Matched {
                source: SourceId::curseforge(),
                artifacts: vec![cf_artifact.clone()],
            };
    }
    let mut info = info();
    info.author = Some("作者".into());
    draft.scan.requirements.loader = Some("neoforge".into());
    draft.scan.requirements.loader_version = Some("21.1.1".into());
    let count = mr.calls.lock().unwrap().len() + cf.calls.lock().unwrap().len();
    for format in [
        ResourceExportFormat::Modrinth,
        ResourceExportFormat::GenericZip,
        ResourceExportFormat::CurseForge,
    ] {
        let plan = build(
            &draft,
            1,
            format,
            info.clone(),
            vec![],
            &SourceId::curseforge(),
        )
        .unwrap();
        assert_eq!(plan.files.len(), paths.len());
        assert!(matches!(plan.files[2], ExportFileAction::Embed { .. }));
        assert!(matches!(plan.files[3], ExportFileAction::Embed { .. }));
        let (_, manifest) = super::formats::manifest(&plan).unwrap();
        assert_eq!(manifest["name"], "My package");
        assert_eq!(manifest["author"], "作者");
        match format {
            ResourceExportFormat::CurseForge => {
                assert!(matches!(
                    plan.files[0],
                    ExportFileAction::CurseForgeReference { .. }
                ));
                assert!(matches!(
                    plan.files[1],
                    ExportFileAction::Embed { .. }
                ));
                assert_eq!(
                    manifest["minecraft"]["modLoaders"][0]["id"],
                    "neoforge-21.1.1"
                );
                assert_eq!(manifest["files"][0]["projectID"], 12345678);
                assert_eq!(manifest["files"][0]["fileID"], 87654321);
            }
            _ => {
                let ExportFileAction::Download { artifact, .. } =
                    &plan.files[0]
                else {
                    panic!()
                };
                assert_eq!(
                    artifact.reference.release.project.source,
                    if format == ResourceExportFormat::Modrinth {
                        SourceId::modrinth()
                    } else {
                        SourceId::curseforge()
                    }
                );
                assert_eq!(manifest["files"][1]["path"], "mods/[中文]file.jar");
                assert!(manifest["files"][0].get("env").is_none());
                let parsed: crate::pack::install_from::PackFormat =
                    serde_json::from_value(manifest).unwrap();
                assert_eq!(parsed.files.len(), 2);
            }
        }
        let output = root.path().join(format!("out-{format:?}.zip"));
        super::writer::write(
            root.path(),
            &plan,
            &output,
            &CancellationToken::new(),
            &|_, _| {},
        )
        .await
        .unwrap();
        let detected =
            crate::api::pack::detect::detect_local_pack_sync(&output).unwrap();
        if format == ResourceExportFormat::CurseForge {
            let (name, target) =
                crate::api::curseforge::get_local_modpack_target(&output)
                    .await
                    .unwrap();
            assert_eq!(name, "My package");
            assert_eq!(target.loader, crate::state::ModLoader::NeoForge);
            assert_eq!(target.loader_version.as_deref(), Some("21.1.1"));
        }
        assert_eq!(
            detected.format,
            if format == ResourceExportFormat::CurseForge {
                crate::api::pack::detect::LocalPackFormat::CurseForge
            } else {
                crate::api::pack::detect::LocalPackFormat::Mrpack
            }
        );
        let mut archive =
            zip::ZipArchive::new(std::fs::File::open(output).unwrap()).unwrap();
        assert!(archive.by_name("overrides/config/settings.txt").is_ok());
        assert!(archive.by_name("overrides/mods/file.jar.disabled").is_ok());
        assert!(archive.by_name("overrides/mods/file.jar").is_err());
        assert_eq!(archive.file_names().filter(|n| *n == "manifest.json" || *n == "modrinth.index.json").count(), 1);
    }
    assert_eq!(
        count,
        mr.calls.lock().unwrap().len() + cf.calls.lock().unwrap().len()
    );
    draft.results.get_mut(&paths[0]).unwrap().resolutions[0] =
        ExportSourceResolution::NoMatch {
            source: SourceId::modrinth(),
        };
    let standard = build(
        &draft,
        1,
        ResourceExportFormat::Modrinth,
        info.clone(),
        vec![],
        &SourceId::modrinth(),
    )
    .unwrap();
    assert!(matches!(standard.files[0], ExportFileAction::Embed { .. }));
    let generic = build(
        &draft,
        1,
        ResourceExportFormat::GenericZip,
        info.clone(),
        vec![],
        &SourceId::modrinth(),
    )
    .unwrap();
    assert!(matches!(
        generic.files[0],
        ExportFileAction::Download { .. }
    ));
    let ExportSourceResolution::Matched { artifacts, .. } =
        &mut draft.results.get_mut(&paths[0]).unwrap().resolutions[1]
    else {
        panic!()
    };
    artifacts[0].acquisition = ArtifactAcquisition::Unavailable;
    let generic = build(
        &draft,
        1,
        ResourceExportFormat::GenericZip,
        info.clone(),
        vec![],
        &SourceId::modrinth(),
    )
    .unwrap();
    assert!(matches!(generic.files[0], ExportFileAction::Embed { .. }));
    let cf = build(
        &draft,
        1,
        ResourceExportFormat::CurseForge,
        info,
        vec![],
        &SourceId::modrinth(),
    )
    .unwrap();
    assert!(matches!(
        cf.files[0],
        ExportFileAction::CurseForgeReference { .. }
    ));
}

#[tokio::test]
async fn export_failure_cancel_and_file_changes_preserve_existing_output() {
    let (root, service, _, _) = setup(false).await;
    std::fs::write(root.path().join("settings.txt"), "old settings").unwrap();
    let mut draft = draft(vec![file("settings.txt")]);
    run(
        &service,
        &mut draft,
        root.path(),
        vec!["settings.txt".into()],
        1,
    )
    .await;
    let plan = build(
        &draft,
        1,
        ResourceExportFormat::GenericZip,
        info(),
        vec![],
        &SourceId::modrinth(),
    )
    .unwrap();
    let output = root.path().join("out.zip");
    std::fs::write(&output, "previous output").unwrap();
    let token = CancellationToken::new();
    token.cancel();
    assert!(matches!(
        super::writer::write(root.path(), &plan, &output, &token, &|_, _| {})
            .await,
        Err(ResourceError::Canceled)
    ));
    let token = CancellationToken::new();
    assert!(
        super::writer::write(root.path(), &plan, &output, &token, &|_, _| {
            token.cancel();
        })
        .await
        .is_err()
    );
    assert_eq!(std::fs::read(&output).unwrap(), b"previous output");
    assert!(!std::fs::read_dir(root.path()).unwrap().any(|e| {
        e.unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".axolotl-export-")
    }));
    let token = CancellationToken::new();
    assert!(
        super::writer::write(root.path(), &plan, &output, &token, &|_, _| {
            std::fs::write(root.path().join("settings.txt"), "mutated bytes")
                .unwrap();
        })
        .await
        .is_err()
    );
    assert_eq!(std::fs::read(&output).unwrap(), b"previous output");
    assert!(
        super::writer::write(
            root.path(),
            &plan,
            &output,
            &CancellationToken::new(),
            &|_, _| {}
        )
        .await
        .is_err()
    );
    assert_eq!(std::fs::read(&output).unwrap(), b"previous output");
    assert!(
        super::writer::write(
            root.path(),
            &plan,
            &root.path().join("settings.txt"),
            &CancellationToken::new(),
            &|_, _| {}
        )
        .await
        .is_err()
    );
}

#[tokio::test]
async fn export_uses_exact_bytes_and_valid_public_urls_and_keeps_real_environment()
 {
    let (root, service, _, _) = setup(true).await;
    std::fs::write(root.path().join("file.jar"), "data").unwrap();
    let mut draft = draft(vec![file("file.jar")]);
    run(
        &service,
        &mut draft,
        root.path(),
        vec!["file.jar".into()],
        1,
    )
    .await;
    for p in draft.projects.values_mut() {
        p.protocol = Some(
            serde_json::json!({"client_side": "optional", "server_side": "unsupported"}),
        );
    }
    let plan = build(
        &draft,
        1,
        ResourceExportFormat::Modrinth,
        info(),
        vec![],
        &SourceId::modrinth(),
    )
    .unwrap();
    let (_, value) = super::formats::manifest(&plan).unwrap();
    assert_eq!(
        value["files"][0]["env"],
        serde_json::json!({"client": "optional", "server": "unsupported"})
    );
    let ExportSourceResolution::Matched { artifacts, .. } =
        &mut draft.results.get_mut("file.jar").unwrap().resolutions[0]
    else {
        panic!()
    };
    artifacts[0].acquisition = ArtifactAcquisition::PublicDownload {
        urls: vec!["https://cdn.example/file?token=secret".into()],
    };
    let plan = build(
        &draft,
        1,
        ResourceExportFormat::GenericZip,
        info(),
        vec![],
        &SourceId::modrinth(),
    )
    .unwrap();
    assert!(matches!(plan.files[0], ExportFileAction::Embed { .. }));
    let ExportSourceResolution::Matched { artifacts, .. } =
        &mut draft.results.get_mut("file.jar").unwrap().resolutions[0]
    else {
        panic!()
    };
    artifacts[0].acquisition = ArtifactAcquisition::PublicDownload {
        urls: vec!["https://cdn.example/file".into()],
    };
    artifacts[0].integrity.hashes[0].value = "0".repeat(40);
    let plan = build(
        &draft,
        1,
        ResourceExportFormat::Modrinth,
        info(),
        vec![],
        &SourceId::modrinth(),
    )
    .unwrap();
    assert!(matches!(plan.files[0], ExportFileAction::Embed { .. }));
    assert!(
        build(
            &draft,
            2,
            ResourceExportFormat::Modrinth,
            info(),
            vec![],
            &SourceId::modrinth()
        )
        .is_err()
    );
}

#[tokio::test]
async fn export_revalidates_requirements_before_publish_and_cleans_interrupted_files()
 {
    let (root, service, _, _) = setup(false).await;
    std::fs::write(root.path().join("settings.txt"), "settings").unwrap();
    let mut draft = draft(vec![file("settings.txt")]);
    run(
        &service,
        &mut draft,
        root.path(),
        vec!["settings.txt".into()],
        1,
    )
    .await;
    let plan = build(
        &draft,
        1,
        ResourceExportFormat::Modrinth,
        info(),
        vec![],
        &SourceId::modrinth(),
    )
    .unwrap();
    let output = root.path().join("out.mrpack");
    std::fs::write(&output, "old output").unwrap();
    let validations = std::sync::atomic::AtomicUsize::new(0);
    let result = super::writer::write_checked(
        root.path(),
        &plan,
        &output,
        &CancellationToken::new(),
        &|_, _| {},
        || async {
            if validations.fetch_add(1, Ordering::SeqCst) == 0 {
                Ok(())
            } else {
                Err(invalid("requirements changed"))
            }
        },
    )
    .await;
    assert!(result.is_err());
    assert_eq!(std::fs::read(&output).unwrap(), b"old output");
    let temporary = super::writer::temporary_path(&plan, &output).unwrap();
    std::fs::write(&temporary, "partial archive").unwrap();
    super::writer::cleanup_partial(&plan, &output)
        .await
        .unwrap();
    assert!(!temporary.exists());
    assert_eq!(std::fs::read(&output).unwrap(), b"old output");
}

#[cfg(not(feature = "tauri"))]
#[tokio::test]
async fn frozen_export_job_survives_session_close_and_preserves_instance_stage()
{
    use crate::install::model::{
        InstallCleanup, InstallJobState, InstallJobStatus, InstallRequest,
    };
    let state = if crate::State::initialized() {
        crate::State::get().await.unwrap()
    } else {
        crate::State::init_for_test(
            tempfile::tempdir()
                .unwrap()
                .keep()
                .to_string_lossy()
                .to_string(),
        )
        .await
        .unwrap();
        crate::State::get().await.unwrap()
    };
    let instance = crate::api::instance::create(
        "Export task".into(),
        "1.21.1".into(),
        crate::state::ModLoader::Vanilla,
        None,
        None,
        crate::state::InstanceLink::Unmanaged,
        None,
        None,
    )
    .await
    .unwrap();
    let root = crate::api::instance::get_full_path(&instance.instance.id)
        .await
        .unwrap();
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("settings.txt"), "task settings").unwrap();
    let (_provider_root, service, mr, cf) = setup(false).await;
    let mut draft = draft(vec![file("settings.txt")]);
    draft.scan.instance_id = instance.instance.id.clone();
    run(&service, &mut draft, &root, vec!["settings.txt".into()], 1).await;
    let session = draft.scan.session_id.clone();
    service.register_export(draft.scan.clone()).unwrap();
    *service.export_draft(&session).unwrap().lock().await = draft;
    let plan = prepare(
        &service,
        session.clone(),
        1,
        ResourceExportFormat::GenericZip,
        info(),
        vec![],
    )
    .await
    .unwrap();
    let output_dir = tempfile::tempdir().unwrap();
    let output = output_dir.path().join("task.zip");
    let request = InstallRequest::ExportResource {
        plan: plan.clone(),
        output_path: output.clone(),
        resolution: None,
    };
    assert!(!request.completes_instance_install_stage());
    let decoded: InstallJobState = serde_json::from_value(
        serde_json::to_value(InstallJobState::new(request)).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        decoded.request,
        InstallRequest::ExportResource { .. }
    ));
    assert_eq!(decoded.cleanup, InstallCleanup::None);
    crate::state::instances::commands::set_instance_install_stage(
        &instance.instance.id,
        crate::state::InstanceInstallStage::PackInstalling,
        &state.pool,
    )
    .await
    .unwrap();
    let job = queue(&service, plan.id.clone(), 1, output.clone())
        .await
        .unwrap();
    let again = queue(&service, plan.id.clone(), 1, output).await.unwrap();
    assert_eq!(job.job_id, again.job_id);
    service.close_export(&session);
    service.cache().clear().await.unwrap();
    let job = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let job =
                crate::install::runner::get_job(job.job_id).await.unwrap();
            if matches!(
                job.status,
                InstallJobStatus::Succeeded
                    | InstallJobStatus::Failed
                    | InstallJobStatus::Canceled
            ) {
                break job;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(job.status, InstallJobStatus::Succeeded, "{:?}", job.error);
    let output = output_dir.path().join("task.zip");
    let existing = std::fs::read(&output).unwrap();
    let permits = state.install_job_semaphore.available_permits();
    let permit = state
        .install_job_semaphore
        .acquire_many(permits as u32)
        .await
        .unwrap();
    let queued = crate::install::runner::export_resource(plan, output.clone())
        .await
        .unwrap();
    let canceled = crate::install::runner::cancel_job(queued.job_id)
        .await
        .unwrap();
    drop(permit);
    assert_eq!(canceled.status, InstallJobStatus::Canceled);
    assert_eq!(std::fs::read(output).unwrap(), existing);
    let mut pending = self::draft(vec![file("settings.txt")]);
    pending.scan.instance_id = instance.instance.id.clone();
    let session = pending.scan.session_id.clone();
    service.register_export(pending.scan).unwrap();
    let permit = state.install_job_semaphore.acquire().await.unwrap();
    let output = output_dir.path().join("pending.zip");
    let job = super::job::queue(
        &service,
        session.clone(),
        vec!["settings.txt".into()],
        ResourceExportFormat::GenericZip,
        info(),
        vec![],
        output.clone(),
    )
    .await
    .unwrap();
    assert_eq!(job.phase, crate::install::InstallPhaseId::Verifying);
    let repeated = super::job::queue(
        &service,
        session.clone(),
        vec!["settings.txt".into()],
        ResourceExportFormat::GenericZip,
        info(),
        vec![],
        output.clone(),
    )
    .await
    .unwrap();
    assert_eq!(repeated.job_id, job.job_id);
    assert!(
        super::job::queue(
            &service,
            session.clone(),
            vec!["settings.txt".into()],
            ResourceExportFormat::GenericZip,
            info(),
            vec![],
            output_dir.path().join("other.zip")
        )
        .await
        .is_err()
    );
    let record = crate::install::store::get_required(job.job_id, &state)
        .await
        .unwrap();
    let restored: InstallJobState =
        serde_json::from_value(serde_json::to_value(&record.state).unwrap())
            .unwrap();
    assert!(matches!(
        restored.request,
        InstallRequest::ExportResource {
            resolution: Some(_),
            ..
        }
    ));
    service.close_export(&session);
    drop(permit);
    let completed =
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            loop {
                let job =
                    crate::install::runner::get_job(job.job_id).await.unwrap();
                if matches!(
                    job.status,
                    InstallJobStatus::Succeeded
                        | InstallJobStatus::Failed
                        | InstallJobStatus::Canceled
                ) {
                    break job;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
    assert_eq!(
        completed.status,
        InstallJobStatus::Succeeded,
        "{:?}",
        completed.error
    );
    let persisted = crate::install::store::get_required(job.job_id, &state)
        .await
        .unwrap();
    assert!(matches!(
        persisted.state.request,
        InstallRequest::ExportResource {
            resolution: None,
            ..
        }
    ));
    assert!(output.exists());
    assert_eq!(
        crate::state::get_instance(&instance.instance.id, &state.pool)
            .await
            .unwrap()
            .unwrap()
            .instance
            .install_stage,
        crate::state::InstanceInstallStage::PackInstalling
    );
    assert_eq!(
        mr.calls.lock().unwrap().len() + cf.calls.lock().unwrap().len(),
        0
    );
}

#[test]
fn large_index_file_sizes_round_trip_without_truncation() {
    let pack: crate::pack::install_from::PackFormat = serde_json::from_value(serde_json::json!({
        "game": "minecraft", "formatVersion": 1, "versionId": "1", "name": "Large", "dependencies": {"minecraft": "1.21.1"},
        "files": [{"path": "mods/large.jar", "hashes": {"sha1": "0".repeat(40), "sha512": "0".repeat(128)}, "downloads": ["https://cdn.example/large.jar"], "fileSize": 5_000_000_000u64}]
    })).unwrap();
    assert_eq!(pack.files[0].file_size, 5_000_000_000);
}

#[tokio::test]
async fn export_reference_verification_reports_bytes_without_repeating_identification()
 {
    use sha2::{Digest, Sha256, Sha512};
    let (root, _service, mr, cf) = setup(false).await;
    let bytes = vec![42; 16 * 1024 * 1024];
    std::fs::write(root.path().join("reference.jar"), &bytes).unwrap();
    let mut frozen = file("reference.jar");
    frozen.integrity = ArtifactIntegrity {
        size: Some(bytes.len() as u64),
        hashes: vec![
            ContentHash {
                algorithm: HashAlgorithm::Sha1,
                value: sha1_smol::Sha1::from(&bytes).digest().to_string(),
            },
            ContentHash {
                algorithm: HashAlgorithm::Sha256,
                value: format!("{:x}", Sha256::digest(&bytes)),
            },
            ContentHash {
                algorithm: HashAlgorithm::Sha512,
                value: format!("{:x}", Sha512::digest(&bytes)),
            },
        ],
    };
    let artifact = ResourceArtifact {
        reference: SourceArtifactRef {
            release: SourceReleaseRef {
                project: SourceProjectRef {
                    source: SourceId::modrinth(),
                    project_id: SourceProjectId::new("project").unwrap(),
                },
                release_id: SourceReleaseId::new("release").unwrap(),
            },
            artifact_id: SourceArtifactId::new("reference.jar").unwrap(),
        },
        file_name: Some("reference.jar".into()),
        role: ArtifactRole::Primary,
        available: Some(true),
        integrity: frozen.integrity.clone(),
        acquisition: ArtifactAcquisition::PublicDownload {
            urls: vec!["https://cdn.example/reference.jar".into()],
        },
        dependencies: vec![],
    };
    let mut draft = draft(vec![frozen.clone()]);
    draft.selection_revision = 1;
    draft.selected = vec!["reference.jar".into()];
    draft.results.insert(
        "reference.jar".into(),
        ExportResolvedFile {
            snapshot: frozen,
            resolutions: vec![
                ExportSourceResolution::Matched {
                    source: SourceId::modrinth(),
                    artifacts: vec![artifact],
                },
                ExportSourceResolution::NoMatch {
                    source: SourceId::curseforge(),
                },
            ],
            local_error: None,
        },
    );
    let plan = build(
        &draft,
        1,
        ResourceExportFormat::Modrinth,
        info(),
        vec![],
        &SourceId::modrinth(),
    )
    .unwrap();
    let progress = Mutex::new(Vec::new());
    let output = root.path().join("out.mrpack");
    let started = std::time::Instant::now();
    super::writer::write(
        root.path(),
        &plan,
        &output,
        &CancellationToken::new(),
        &|current, total| {
            progress.lock().unwrap().push((current, total));
        },
    )
    .await
    .unwrap();
    eprintln!("Verified 16 MiB reference in {:?}", started.elapsed());
    let progress = progress.lock().unwrap();
    let total = bytes.len() as u64 + 1;
    assert_eq!(super::writer::work_size(&plan), total);
    assert_eq!(progress.first(), Some(&(0, total)));
    assert_eq!(progress.last(), Some(&(total, total)));
    assert!(
        progress
            .iter()
            .any(|(current, _)| *current > 0 && *current < total)
    );
    assert!(progress.windows(2).all(|pair| pair[0].0 <= pair[1].0));
    assert!(
        mr.calls.lock().unwrap().is_empty()
            && cf.calls.lock().unwrap().is_empty()
    );
    assert!(std::fs::metadata(output).unwrap().len() < 1024 * 1024);
    drop(progress);
    let cancel = CancellationToken::new();
    let result = super::writer::write(
        root.path(),
        &plan,
        &root.path().join("cancel.mrpack"),
        &cancel,
        &|current, _| {
            if current > 0 {
                cancel.cancel();
            }
        },
    )
    .await;
    assert!(matches!(result, Err(ResourceError::Canceled)));
    assert!(!root.path().join("cancel.mrpack").exists());
    let source = root.path().join("reference.jar");
    let modified = std::fs::metadata(&source).unwrap().modified().unwrap();
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .open(&source)
        .unwrap();
    std::io::Write::write_all(&mut file, b"changed").unwrap();
    file.set_times(std::fs::FileTimes::new().set_modified(modified))
        .unwrap();
    drop(file);
    let previous = std::fs::read(root.path().join("out.mrpack")).unwrap();
    assert!(
        super::writer::write(
            root.path(),
            &plan,
            &root.path().join("out.mrpack"),
            &CancellationToken::new(),
            &|_, _| {}
        )
        .await
        .is_err()
    );
    assert_eq!(
        std::fs::read(root.path().join("out.mrpack")).unwrap(),
        previous
    );
}

#[tokio::test]
async fn queued_export_resumes_partial_fresh_evidence_and_never_implicitly_embeds_failures()
 {
    use super::job::{ExportResolutionCheckpoint, resolve};
    let (root, service, mr, cf) = setup(true).await;
    std::fs::write(root.path().join("a.jar"), "data").unwrap();
    std::fs::write(root.path().join("b.jar"), "data").unwrap();
    std::fs::write(root.path().join("c.jar"), "data").unwrap();
    let mut draft = draft(vec![file("a.jar"), file("b.jar"), file("c.jar")]);
    run(&service, &mut draft, root.path(), vec!["a.jar".into()], 1).await;
    let mut plan = build(
        &draft,
        1,
        ResourceExportFormat::Modrinth,
        info(),
        vec![],
        &SourceId::modrinth(),
    )
    .unwrap();
    plan.files.push(ExportFileAction::Embed {
        snapshot: file("b.jar"),
    });
    let checkpoint: ExportResolutionCheckpoint = serde_json::from_value(
        serde_json::to_value(ExportResolutionCheckpoint::capture(
            &draft,
            SourceId::modrinth(),
        ))
        .unwrap(),
    )
    .unwrap();
    let before_mr = mr.calls.lock().unwrap().len();
    let before_cf = cf.calls.lock().unwrap().len();
    cf.failed.store(true, Ordering::SeqCst);
    let mut restored = checkpoint.restore();
    let progress = Mutex::new(Vec::new());
    let failed = resolve(
        &service,
        &mut restored,
        &plan,
        root.path(),
        &checkpoint.preferred_source,
        HashMap::new(),
        &CancellationToken::new(),
        &|value| {
            progress.lock().unwrap().push(value);
        },
    )
    .await;
    assert!(failed.is_err());
    let updates = progress.lock().unwrap();
    assert!(
        updates
            .windows(2)
            .all(|pair| pair[0].overall_completed <= pair[1].overall_completed)
    );
    assert_eq!(updates.last().unwrap().overall_completed, 6);
    assert_eq!(updates.last().unwrap().overall_total, 6);
    drop(updates);
    assert_eq!(restored.selected, vec!["a.jar", "b.jar"]);
    assert_eq!(mr.calls.lock().unwrap().len(), before_mr + 1);
    assert_eq!(cf.calls.lock().unwrap().len(), before_cf + 1);
    assert!(matches!(
        restored.results["b.jar"].resolutions[1],
        ExportSourceResolution::Failed { .. }
    ));
    let checkpoint: ExportResolutionCheckpoint = serde_json::from_value(
        serde_json::to_value(ExportResolutionCheckpoint::capture(
            &restored,
            SourceId::modrinth(),
        ))
        .unwrap(),
    )
    .unwrap();
    let mut explicit = plan.clone();
    explicit.embedded_failure_choices = vec![ExportEmbeddedFailureChoice {
        snapshot: restored.results["b.jar"].snapshot.clone(),
        source: SourceId::curseforge(),
    }];
    let embedded = resolve(
        &service,
        &mut checkpoint.restore(),
        &explicit,
        root.path(),
        &checkpoint.preferred_source,
        HashMap::new(),
        &CancellationToken::new(),
        &|_| {},
    )
    .await
    .unwrap();
    assert!(matches!(embedded.files[1], ExportFileAction::Embed { .. }));
    assert_eq!(cf.calls.lock().unwrap().len(), before_cf + 1);
    cf.failed.store(false, Ordering::SeqCst);
    let mut expanded = explicit;
    expanded.files.push(ExportFileAction::Embed {
        snapshot: file("c.jar"),
    });
    let resolved = resolve(
        &service,
        &mut checkpoint.restore(),
        &expanded,
        root.path(),
        &checkpoint.preferred_source,
        HashMap::new(),
        &CancellationToken::new(),
        &|_| {},
    )
    .await
    .unwrap();
    assert!(matches!(resolved.files[1], ExportFileAction::Embed { .. }));
    assert!(matches!(
        resolved.files[2],
        ExportFileAction::Download { .. }
    ));
    let completed = resolve(
        &service,
        &mut checkpoint.restore(),
        &plan,
        root.path(),
        &checkpoint.preferred_source,
        HashMap::new(),
        &CancellationToken::new(),
        &|_| {},
    )
    .await
    .unwrap();
    assert_eq!(completed.id, plan.id);
    assert_eq!(completed.files.len(), 2);
    assert!(
        completed
            .files
            .iter()
            .all(|action| matches!(action, ExportFileAction::Download { .. }))
    );
    assert_eq!(mr.calls.lock().unwrap().len(), before_mr + 2);
    assert_eq!(cf.calls.lock().unwrap().len(), before_cf + 3);
}

#[tokio::test]
async fn frozen_file_hashing_reports_progress_before_the_file_finishes() {
    let root = tempfile::tempdir().unwrap();
    let bytes = vec![42; 1024 * 1024];
    std::fs::write(root.path().join("large.jar"), &bytes).unwrap();
    let progress = Mutex::new(Vec::new());
    let (snapshot, fingerprint) = freeze_with_progress(
        root.path(),
        &file("large.jar"),
        &CancellationToken::new(),
        &|current, total| {
            progress.lock().unwrap().push((current, total));
        },
    )
    .await
    .unwrap();
    let progress = progress.lock().unwrap();
    assert_eq!(snapshot.integrity.size, Some(bytes.len() as u64));
    assert_eq!(
        fingerprint,
        crate::api::curseforge::compute_fingerprint(&bytes)
    );
    let total = bytes.len() as u64 * 2;
    assert_eq!(progress.first(), Some(&(0, total)));
    assert_eq!(progress.last(), Some(&(total, total)));
    assert!(
        progress
            .iter()
            .any(|(current, _)| *current > 0 && *current < bytes.len() as u64)
    );
    assert!(progress.windows(2).all(|pair| pair[0].0 <= pair[1].0));
}

#[tokio::test]
async fn source_verification_announces_the_provider_before_starting_its_requests()
 {
    let (root, service, mr, cf) = setup(false).await;
    std::fs::write(root.path().join("file.jar"), "data").unwrap();
    let mut draft = draft(vec![file("file.jar")]);
    let phases = Mutex::new(Vec::new());
    let result = refresh_local(
        &service,
        &mut draft,
        root.path(),
        vec!["file.jar".into()],
        1,
        RequestId::new("progress").unwrap(),
        HashMap::new(),
        &CancellationToken::new(),
        &|value| {
            phases.lock().unwrap().push((
                value.phase,
                mr.calls.lock().unwrap().len(),
                cf.calls.lock().unwrap().len(),
            ));
        },
    )
    .await
    .unwrap();
    assert_eq!(result.state, ExportResolutionState::Ready);
    let phases = phases.lock().unwrap();
    assert!(
        phases
            .iter()
            .any(|(phase, mr_calls, _)| phase == "modrinth" && *mr_calls == 0)
    );
    assert!(phases.iter().any(|(phase, _, cf_calls)| phase == "curseforge" && *cf_calls == 0));
    assert_eq!(phases.last().unwrap().0, "complete");
}

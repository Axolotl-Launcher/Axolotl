use super::*;
use crate::state::{DirectoryInfo, ModrinthProjectId, ModrinthVersionId};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::sync::Arc;

#[tokio::test]
async fn pack_overrides_keep_membership_across_reconciliation() {
    use crate::api::curseforge::{
        CurseForgePackExpectedContent, CurseForgePackExpectedOverride,
    };
    use crate::state::instances::commands::reconcile_curseforge_members;

    let (_temp, state, original, _, _) = fixture().await;
    let mut tx = state.pool.begin().await.unwrap();
    crate::state::instances::adapters::sqlite::instance_rows::upsert_instance_link(
        "instance",
        &crate::state::InstanceLink::CurseForgeModpack {
            project_id: "1".into(),
            version_id: "2".into(),
        },
        &mut tx,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let expected = CurseForgePackExpectedContent {
        members: vec![],
        overrides: vec![CurseForgePackExpectedOverride {
            project_type: crate::state::ProjectType::ResourcePack,
            expected_relative_path: original
                .relative_path
                .trim_end_matches(".disabled")
                .to_owned(),
        }],
    };
    reconcile_curseforge_members("instance", "set", &expected, &state)
        .await
        .unwrap();
    let entries = plans::installed(&state.pool, "instance").await.unwrap();
    let ResourceOwnership::PackManaged { pack_member_id } =
        &entries[0].ownership
    else {
        panic!("pack override lost its ownership");
    };
    let snapshot =
        crate::state::instances::commands::get_local_content_snapshot(
            "instance", &state,
        )
        .await
        .unwrap();
    let pack = snapshot.pack.unwrap();
    assert!(pack.reconciled && pack.can_update);
    sqlx::query("UPDATE instance_pack_members SET required = 0, reconciled = 0 WHERE id = ?")
        .bind(pack_member_id).execute(&state.pool).await.unwrap();
    reconcile_curseforge_members("instance", "set", &expected, &state)
        .await
        .unwrap();
    let repaired: (String, i64, i64) = sqlx::query_as(
        "SELECT id, required, reconciled FROM instance_pack_members",
    )
    .fetch_one(&state.pool)
    .await
    .unwrap();
    assert_eq!(repaired, (pack_member_id.clone(), 0, 1));
    // Existing releases used path keys; keep their identity and user choices.
    sqlx::query("UPDATE instance_pack_members SET member_key = ?, expected_relative_path = ?, override_kind = 'disabled', required = 0, reconciled = 0 WHERE id = ?")
        .bind(format!("path:{}", expected.overrides[0].expected_relative_path))
        .bind(original.relative_path.replace('/', "\\").to_ascii_uppercase().replace(".DISABLED", ".disabled"))
        .bind(pack_member_id).execute(&state.pool).await.unwrap();
    reconcile_curseforge_members("instance", "set", &expected, &state)
        .await
        .unwrap();
    let member: (String, String, i64, i64) = sqlx::query_as(
        "SELECT id, override_kind, required, reconciled FROM instance_pack_members",
    )
    .fetch_one(&state.pool)
    .await
    .unwrap();
    assert_eq!(member, (pack_member_id.clone(), "disabled".into(), 0, 1));
    let snapshot =
        crate::state::instances::commands::get_local_content_snapshot(
            "instance", &state,
        )
        .await
        .unwrap();
    assert!(snapshot.pack.unwrap().can_update);
    plans::installed(&state.pool, "instance").await.unwrap();
    state.pool.close().await;
}

#[tokio::test]
async fn content_operation_repairs_legacy_pack_entries_without_members() {
    let (_temp, state, original, _, _) = fixture().await;
    sqlx::query("UPDATE instance_content_entries SET ownership_kind = 'pack_managed' WHERE id = ?")
        .bind(original.entry_id.as_str()).execute(&state.pool).await.unwrap();
    assert!(plans::installed(&state.pool, "instance").await.is_err());
    let cancel = CancellationToken::new();
    super::installed::refresh_install_snapshot(&state, "instance", &cancel)
        .await
        .unwrap();
    let entries = plans::installed(&state.pool, "instance").await.unwrap();
    assert!(matches!(
        entries[0].ownership,
        ResourceOwnership::PackManaged { .. }
    ));
    let member = content_rows::get_pack_members("set", &state.pool)
        .await
        .unwrap()
        .remove(0);
    assert!(!member.reconciled);
    assert!(member.provider.is_none());
    assert!(member.expected_sha1.is_none() && member.expected_size.is_none());
    let revision = plans::instance_revision(&state.pool, "instance")
        .await
        .unwrap();
    super::installed::refresh_install_snapshot(&state, "instance", &cancel)
        .await
        .unwrap();
    assert_eq!(
        plans::instance_revision(&state.pool, "instance")
            .await
            .unwrap(),
        revision
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM instance_pack_members"
        )
        .fetch_one(&state.pool)
        .await
        .unwrap(),
        1
    );
    state.pool.close().await;
}

#[tokio::test]
async fn legacy_pack_path_collisions_keep_all_members_and_stable_revisions() {
    for existing_member in [false, true] {
        let (_temp, state, original, _, _) = fixture().await;
        sqlx::query("UPDATE instance_content_entries SET ownership_kind = 'pack_managed' WHERE id = ?")
            .bind(original.entry_id.as_str()).execute(&state.pool).await.unwrap();
        let missing_path = original.relative_path.trim_end_matches(".disabled");
        let file = content_rows::upsert_instance_file_from_parts(
            content_rows::UpsertInstanceFile {
                instance_id: "instance",
                relative_path: missing_path,
                file_name: "prefix-old.zip",
                enabled: true,
                sha1: "0000000000000000000000000000000000000000",
                size: 1,
                missing: true,
                local_mod_data: None,
                icon_path: None,
            },
            &state.pool,
        )
        .await
        .unwrap();
        let missing = content_rows::upsert_content_entry_from_parts(
            content_rows::UpsertContentEntry {
                instance_id: "instance",
                content_set_id: "set",
                file_id: Some(&file.id),
                project_type: ProjectType::ResourcePack,
                source_kind: ContentSourceKind::Local,
                ownership_kind: ContentOwnershipKind::PackManaged,
                auto_dependency: false,
                server_requirement: ContentRequirement::Required,
                client_requirement: ContentRequirement::Required,
                enabled: true,
            },
            &state.pool,
        )
        .await
        .unwrap();
        let existing_id = format!("pack-member:{}", missing.id);
        if existing_member {
            // Reproduce a member already reassigned by an older path repair.
            sqlx::query("INSERT INTO instance_pack_members(id, content_set_id, content_entry_id, member_key, project_type, expected_relative_path, required, override_kind, expected_sha1, reconciled, created_at, modified_at) VALUES (?, 'set', ?, ?, 'resourcepack', ?, 0, 'disabled', 'expected-hash', 1, 1, 1)")
                .bind(&existing_id).bind(original.entry_id.as_str())
                .bind(format!("path:{missing_path}"))
                .bind(missing_path).execute(&state.pool).await.unwrap();
        }
        let cancel = CancellationToken::new();
        super::installed::refresh_install_snapshot(&state, "instance", &cancel)
            .await
            .unwrap();
        let members = content_rows::get_pack_members("set", &state.pool)
            .await
            .unwrap();
        assert_eq!(members.len(), 2);
        let associations: std::collections::HashMap<_, _> = members
            .iter()
            .map(|member| {
                (member.content_entry_id.clone().unwrap(), member.id.clone())
            })
            .collect();
        assert_eq!(associations.len(), 2);
        assert!(associations.contains_key(original.entry_id.as_str()));
        assert!(associations.contains_key(&missing.id));
        if existing_member {
            let member = members
                .iter()
                .find(|member| member.id == existing_id)
                .unwrap();
            assert_eq!(
                member.content_entry_id.as_deref(),
                Some(original.entry_id.as_str())
            );
            assert!(!member.required);
            assert!(member.reconciled);
            assert_eq!(member.expected_sha1.as_deref(), Some("expected-hash"));
            assert_eq!(
                member.override_kind,
                crate::state::PackMemberOverrideKind::Disabled
            );
        }
        let revision = plans::instance_revision(&state.pool, "instance")
            .await
            .unwrap();
        for _ in 0..3 {
            super::installed::refresh_install_snapshot(
                &state, "instance", &cancel,
            )
            .await
            .unwrap();
            let entries =
                plans::installed(&state.pool, "instance").await.unwrap();
            assert_eq!(entries.len(), 2);
            assert!(entries.iter().all(|entry| matches!(
                entry.ownership,
                ResourceOwnership::PackManaged { .. }
            )));
            let current: std::collections::HashMap<_, _> =
                content_rows::get_pack_members("set", &state.pool)
                    .await
                    .unwrap()
                    .into_iter()
                    .map(|member| (member.content_entry_id.unwrap(), member.id))
                    .collect();
            assert_eq!(current, associations);
            assert_eq!(
                plans::instance_revision(&state.pool, "instance")
                    .await
                    .unwrap(),
                revision
            );
        }
        let violations = sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&state.pool)
            .await
            .unwrap();
        assert!(violations.is_empty());
        state.pool.close().await;
    }
}

#[tokio::test]
async fn install_snapshot_reuses_hashes_and_only_rehashes_changed_files() {
    use crate::util::download::verified_file::track_scans;
    let (_temp, state, original, _, root) = fixture().await;
    let cancel = CancellationToken::new();
    let (result, scans) = track_scans(
        super::installed::refresh_install_snapshot(&state, "instance", &cancel),
    )
    .await;
    result.unwrap();
    assert_eq!(
        scans, 0,
        "an unchanged tracked file must not be rehashed even with a cold hash cache"
    );
    std::fs::write(root.join(&original.relative_path), b"changed and longer")
        .unwrap();
    let (result, scans) = track_scans(
        super::installed::refresh_install_snapshot(&state, "instance", &cancel),
    )
    .await;
    result.unwrap();
    assert_eq!(scans, 1);
    let (result, scans) = track_scans(
        super::installed::refresh_install_snapshot(&state, "instance", &cancel),
    )
    .await;
    result.unwrap();
    assert_eq!(scans, 0);
    std::fs::write(root.join("resourcepacks/new.zip"), b"new file").unwrap();
    let (result, scans) = track_scans(
        super::installed::refresh_install_snapshot(&state, "instance", &cancel),
    )
    .await;
    result.unwrap();
    assert_eq!(scans, 1, "only the new file is hashed");
    std::fs::remove_file(root.join("resourcepacks/new.zip")).unwrap();
    let (result, scans) = track_scans(
        super::installed::refresh_install_snapshot(&state, "instance", &cancel),
    )
    .await;
    result.unwrap();
    assert_eq!(scans, 0);
    let (result, scans) = track_scans(super::installed::refresh_snapshot(
        &state, "instance", &cancel,
    ))
    .await;
    result.unwrap();
    assert_eq!(
        scans, 1,
        "explicit verification still reads unchanged bytes"
    );
    state.pool.close().await;
}

#[tokio::test]
async fn unchanged_installed_artifacts_do_not_fetch_project_presentations() {
    use crate::resources::providers::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct OfflineProvider(AtomicUsize);
    #[async_trait::async_trait]
    impl ResourceProvider for OfflineProvider {
        fn source(&self) -> ResourceSource {
            ResourceSource {
                id: SourceId::modrinth(),
                types: vec![ResourceType::ResourcePack],
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
            "offline-hydration-test".into()
        }
        fn batch_limit(&self, _: &ProviderQuery) -> usize {
            100
        }
        async fn fetch_many(
            &self,
            queries: &[ProviderQuery],
        ) -> Vec<ResourceResult<ProviderValue>> {
            self.0.fetch_add(queries.len(), Ordering::Relaxed);
            vec![Err(ResourceError::Network { status: None }); queries.len()]
        }
    }

    let (_temp, state, original, mut saved, _) = fixture().await;
    let provider = Arc::new(OfflineProvider(AtomicUsize::new(0)));
    let cache = Arc::new(
        cache::ResourceCache::open(
            state.directories.caches_dir().join("test-resources.db"),
        )
        .await,
    );
    let service = ResourceService::new(cache, vec![provider.clone()]).unwrap();
    let result = super::installed::hydrate_selected_with_service(
        &state,
        "instance",
        &CancellationToken::new(),
        None,
        &service,
    )
    .await
    .unwrap();
    assert!(result.failures.is_empty());
    assert_eq!(result.entries.len(), 1);
    assert_eq!(result.entries[0].artifact, original.artifact);
    assert_eq!(provider.0.load(Ordering::Relaxed), 0);

    let batch = super::installed::hydrate_update_batch_with_service(
        &state,
        "instance",
        &CancellationToken::new(),
        &[original.entry_id.clone()],
        &service,
    )
    .await
    .unwrap();
    assert_eq!(batch.entries.len(), 1);
    assert!(batch.failures.is_empty());
    assert_eq!(provider.0.load(Ordering::Relaxed), 0);

    // Even an uncached installed entry must stay untouched outside the batch.
    sqlx::query("DELETE FROM resource_installed_artifacts WHERE entry_id = ?")
        .bind(original.entry_id.as_str())
        .execute(&state.pool)
        .await
        .unwrap();
    for ids in [vec![], vec![InstalledEntryId::new("not-visible").unwrap()]] {
        let batch = super::installed::hydrate_update_batch_with_service(
            &state,
            "instance",
            &CancellationToken::new(),
            &ids,
            &service,
        )
        .await
        .unwrap();
        assert!(batch.entries.is_empty());
        assert!(batch.failures.is_empty());
        assert_eq!(provider.0.load(Ordering::Relaxed), 0);
    }
    sqlx::query("INSERT INTO resource_installed_artifacts(entry_id, artifact, scope) VALUES (?, ?, ?)")
        .bind(original.entry_id.as_str()).bind(serde_json::to_string(&saved).unwrap())
        .bind(serde_json::to_string(&original.scope).unwrap()).execute(&state.pool).await.unwrap();

    let child = SourceProjectRef {
        source: SourceId::modrinth(),
        project_id: SourceProjectId::new("child000").unwrap(),
    };
    saved.dependencies.push(ArtifactDependency {
        target: DependencyTarget::Resource {
            reference: ResourceRef::Source {
                project: child.clone(),
            },
        },
        kind: ResourceDependencyKind::Required,
        version_constraint: Some(">=2".into()),
    });
    sqlx::query("UPDATE resource_installed_artifacts SET artifact = ? WHERE entry_id = ?")
        .bind(serde_json::to_string(&saved).unwrap()).bind(original.entry_id.as_str()).execute(&state.pool).await.unwrap();
    sqlx::query(
        "UPDATE instance_files SET enabled = 1 WHERE instance_id = 'instance'",
    )
    .execute(&state.pool)
    .await
    .unwrap();
    let store = crate::state::resources::ResourceStore::new(state.pool.clone());
    store
        .ensure_project(
            &saved.reference.release.project,
            &[ResourceType::ResourcePack],
        )
        .await
        .unwrap();
    store
        .ensure_project(&child, &[ResourceType::Mod])
        .await
        .unwrap();
    let service = ResourceService::with_store(
        service.cache().clone(),
        vec![provider.clone()],
        store,
    )
    .unwrap();
    let entries = plans::installed(&state.pool, "instance").await.unwrap();
    let requirements = service
        .retained_requirements(
            &[],
            &entries,
            &HashMap::new(),
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(requirements.len(), 1);
    assert_eq!(requirements[0].version_constraint.as_deref(), Some(">=2"));
    assert_eq!(
        provider.0.load(Ordering::Relaxed),
        0,
        "saved declarations and dependency types must work without API calls"
    );

    saved.integrity.size = Some(123456);
    sqlx::query("UPDATE resource_installed_artifacts SET artifact = ? WHERE entry_id = ?")
        .bind(serde_json::to_string(&saved).unwrap()).bind(original.entry_id.as_str()).execute(&state.pool).await.unwrap();
    assert!(
        service
            .retained_requirements(
                &[],
                &entries,
                &HashMap::new(),
                &CancellationToken::new()
            )
            .await
            .is_err()
    );
    assert!(
        provider.0.load(Ordering::Relaxed) > 0,
        "mismatched saved artifacts must be refreshed rather than trusted"
    );
    service.cache().close().await;
    state.pool.close().await;
}

#[tokio::test]
async fn missing_backup_is_only_idempotent_when_original_bytes_are_restored() {
    let temp = tempfile::tempdir().unwrap();
    let old =
        artifact(SourceId::modrinth(), "old00000", "release1", b"old bytes")
            .integrity;
    let target =
        artifact(SourceId::modrinth(), "new00000", "release2", b"new bytes")
            .integrity;
    let file = JournalFile {
        path: "new.jar".into(),
        previous_path: Some("old.jar".into()),
        backup: Some("backup.old".into()),
        old: Some(old),
        target,
    };
    assert!(recover_files(temp.path(), &[file.clone()]).await.is_err());
    std::fs::write(temp.path().join("new.jar"), b"new bytes").unwrap();
    assert!(recover_files(temp.path(), &[file.clone()]).await.is_err());
    std::fs::write(temp.path().join("old.jar"), b"old bytes").unwrap();
    recover_files(temp.path(), &[file]).await.unwrap();
    assert!(!temp.path().join("new.jar").exists());
}

#[tokio::test]
async fn restoring_a_missing_current_artifact_executes_without_inventing_a_backup()
 {
    let (_temp, state, mut original, old, root) = fixture().await;
    std::fs::remove_file(root.join(&original.relative_path)).unwrap();
    original.missing = true;
    sqlx::query(
        "UPDATE instance_files SET missing = 1 WHERE instance_id = 'instance'",
    )
    .execute(&state.pool)
    .await
    .unwrap();
    std::fs::write(root.join("download.zip"), b"old bytes").unwrap();
    let plan =
        local_plan(&state, original.clone(), old, "download.zip", None).await;
    execute("instance", &plan, None, &state).await.unwrap();
    assert_eq!(
        std::fs::read(root.join(original.relative_path)).unwrap(),
        b"old bytes"
    );
    let current = plans::installed(&state.pool, "instance")
        .await
        .unwrap()
        .remove(0);
    assert!(!current.missing);
    let history = history::list_with_state("instance", None, &state)
        .await
        .unwrap();
    assert!(!history[0].backup_available);
    state.pool.close().await;
}

#[tokio::test]
async fn selected_installed_entries_verify_disk_even_with_saved_exact_artifacts()
 {
    let (_temp, state, original, old, root) = fixture().await;
    sqlx::query("INSERT INTO resource_installed_artifacts(entry_id, artifact, scope) VALUES (?, ?, ?) ON CONFLICT(entry_id) DO UPDATE SET artifact = excluded.artifact, scope = excluded.scope").bind(original.entry_id.as_str()).bind(serde_json::to_string(&old).unwrap()).bind(serde_json::to_string(&InstallScope::Instance).unwrap()).execute(&state.pool).await.unwrap();
    std::fs::write(root.join(&original.relative_path), b"modified").unwrap();
    let selected = super::installed::hydrate_selected(
        &state,
        "instance",
        &CancellationToken::new(),
        Some(&[original.entry_id.clone()]),
    )
    .await
    .unwrap();
    assert_eq!(selected.failures.len(), 1);
    assert!(selected.failures[0].1.contains("installed files changed"));
    std::fs::remove_file(root.join(&original.relative_path)).unwrap();
    let selected = super::installed::hydrate_selected(
        &state,
        "instance",
        &CancellationToken::new(),
        Some(&[original.entry_id]),
    )
    .await
    .unwrap();
    assert_eq!(selected.failures.len(), 1);
    state.pool.close().await;
}

#[tokio::test]
async fn frozen_switch_jobs_report_switch_version_and_match_ipc_snapshot_contracts()
 {
    use crate::install::{model::*, store};
    let (_temp, state, original, _, _) = fixture().await;
    let mut plan = local_plan(
        &state,
        original,
        artifact(SourceId::curseforge(), "123", "456", b"new bytes"),
        "unused",
        None,
    )
    .await;
    plan.intent = ResourceOperationIntent::SwitchVersion;
    let job_state = InstallJobState::new(InstallRequest::InstallContentBatch {
        instance_id: "instance".into(),
        items: vec![InstallContentBatchItem::FrozenResource { plan }],
        display_title: "Switch".into(),
        display_icon: None,
    });
    let record = store::insert(
        uuid::Uuid::new_v4(),
        &job_state,
        InstallJobStatus::Queued,
        &state,
    )
    .await
    .unwrap();
    let snapshot = record.snapshot();
    let empty_export = ResourceExportPlan {
        schema_version: ResourceContractVersion,
        id: PlanId::new(uuid::Uuid::new_v4().to_string()).unwrap(),
        revision: 1,
        session_id: SessionId::new("export").unwrap(),
        selection_revision: 1,
        instance_id: "instance".into(),
        instance_revision: 1,
        format: ResourceExportFormat::GenericZip,
        package_info: ExportPackageInfo {
            name: "Test".into(),
            version: "1".into(),
            summary: "".into(),
            author: None,
        },
        requirements: ExportGameRequirements {
            game_version: "1.20.1".into(),
            loader: None,
            loader_version: None,
        },
        files: vec![],
        embedded_failure_choices: vec![],
    };
    let export = super::export::plan::PreparedExport {
        plan: empty_export,
        queued: Some(snapshot.clone()),
        output: Some(std::path::PathBuf::from("A.zip")),
    };
    assert_eq!(
        export
            .queued_for(Path::new("A.zip"))
            .unwrap()
            .unwrap()
            .job_id,
        snapshot.job_id
    );
    assert!(export.queued_for(Path::new("B.zip")).is_err());
    assert!(matches!(
        snapshot.content_change.as_ref().unwrap().intent,
        ContentChangeIntent::SwitchVersion { .. }
    ));
    for command in [
        "resource_queue_plan",
        "resource_rollback",
        "resource_export_execute",
    ] {
        let response: crate::resources::ipc::ResourceResponse =
            serde_json::from_value(
                serde_json::json!({"command":command, "result": snapshot}),
            )
            .unwrap();
        assert_eq!(
            serde_json::to_value(response).unwrap()["result"]["job_id"],
            snapshot.job_id.to_string()
        );
    }
    state.pool.close().await;
}

#[tokio::test]
async fn committed_resource_cleanup_failure_does_not_block_other_interrupted_jobs()
 {
    use crate::install::{model::*, store};
    let (_temp, state, original, _, root) = fixture().await;
    std::fs::create_dir_all(
        state.directories.caches_dir().join("resource-staging"),
    )
    .unwrap();
    let mut plan = local_plan(
        &state,
        original,
        artifact(SourceId::curseforge(), "123", "456", b"new bytes"),
        "unused",
        None,
    )
    .await;
    plan.id = PlanId::new("invalid-staging-id").unwrap();
    sqlx::query("INSERT INTO resource_operation_plans(id, instance_id, revision, payload, completed) VALUES (?, 'instance', 1, '{}', 1)").bind(plan.id.as_str()).execute(&state.pool).await.unwrap();
    let committed = store::insert(
        uuid::Uuid::new_v4(),
        &InstallJobState::new(InstallRequest::InstallContentBatch {
            instance_id: "instance".into(),
            items: vec![InstallContentBatchItem::FrozenResource { plan }],
            display_title: "Committed".into(),
            display_icon: None,
        }),
        InstallJobStatus::Running,
        &state,
    )
    .await
    .unwrap();
    let interrupted = store::insert(
        uuid::Uuid::new_v4(),
        &InstallJobState::new(InstallRequest::DownloadJava {
            vendor: "test".into(),
            version: 21,
        }),
        InstallJobStatus::Running,
        &state,
    )
    .await
    .unwrap();
    crate::install::recovery::recover_interrupted_jobs(&state)
        .await
        .unwrap();
    assert_eq!(
        store::get_required(committed.id, &state)
            .await
            .unwrap()
            .status,
        InstallJobStatus::Succeeded
    );
    assert_ne!(
        store::get_required(interrupted.id, &state)
            .await
            .unwrap()
            .status,
        InstallJobStatus::Running
    );
    assert!(root.exists());
    state.pool.close().await;
}

#[tokio::test]
async fn dropped_legacy_rollback_is_recovered_from_its_durable_journal() {
    let (_temp, state, original, _, root) = fixture().await;
    let backup = "resourcepacks/prefix-old.zip_legacy.zip.old";
    let archive = "resourcepacks/legacy.zip_prefix-old.zip.old";
    let restored = "resourcepacks/legacy.zip.disabled";
    std::fs::write(root.join(backup), b"legacy bytes").unwrap();
    let journal = serde_json::json!({"active":original.relative_path,"backup":backup,"restored":"resourcepacks/legacy.zip","final_path":restored,"archive":archive,"active_size":9,"active_sha1":sha1_smol::Sha1::from(b"old bytes").digest().to_string(),"backup_size":12,"backup_sha1":sha1_smol::Sha1::from(b"legacy bytes").digest().to_string()});
    let pool = state.pool.clone();
    let base = root.clone();
    let active = original.relative_path.clone();
    let moved = Arc::new(tokio::sync::Notify::new());
    let signal = moved.clone();
    let task = tokio::spawn(async move {
        sqlx::query("INSERT INTO resource_legacy_rollback_journals(instance_id, payload) VALUES ('instance', ?)").bind(journal.to_string()).execute(&pool).await.unwrap();
        tokio::fs::rename(base.join(active), base.join(archive))
            .await
            .unwrap();
        tokio::fs::rename(base.join(backup), base.join(restored))
            .await
            .unwrap();
        signal.notify_one();
        std::future::pending::<()>().await;
    });
    moved.notified().await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    crate::state::instances::commands::recover_legacy_rollbacks(&state)
        .await
        .unwrap();
    assert_eq!(
        std::fs::read(root.join(&original.relative_path)).unwrap(),
        b"old bytes"
    );
    assert_eq!(std::fs::read(root.join(backup)).unwrap(), b"legacy bytes");
    assert!(!root.join(restored).exists());
    assert_eq!(
        plans::installed(&state.pool, "instance").await.unwrap()[0],
        original
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM resource_legacy_rollback_journals"
        )
        .fetch_one(&state.pool)
        .await
        .unwrap(),
        0
    );
    state.pool.close().await;
}

#[tokio::test]
async fn replacing_import_files_restores_manual_requirements_and_increments_the_plan_revision()
 {
    let (_temp, state, original, _, root) = fixture().await;
    let source = root.join("download.zip");
    std::fs::write(&source, b"new bytes").unwrap();
    let mut target =
        artifact(SourceId::modrinth(), "project1", "release1", b"new bytes");
    target.acquisition = ArtifactAcquisition::ManualDownload {
        page_url: "https://example.com/download".into(),
    };
    let plan = local_plan(&state, original, target, "download.zip", None).await;
    let id = plan.actions[0].node_id.clone();
    let added = plans::prepare_import_files_with_pool(
        &state.pool,
        plan.clone(),
        HashMap::from([(id, source.to_string_lossy().to_string())]),
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(added.revision, plan.revision + 1);
    assert!(
        !added.issues.iter().any(
            |issue| issue.code == ResourceIssueCode::ManualDownloadRequired
        )
    );
    assert!(
        plans::prepare_import_files_with_pool(
            &state.pool,
            plan,
            HashMap::new(),
            &CancellationToken::new()
        )
        .await
        .is_err()
    );
    let cancel = CancellationToken::new();
    let (first, second) = tokio::join!(
        plans::prepare_import_files_with_pool(
            &state.pool,
            added.clone(),
            HashMap::new(),
            &cancel
        ),
        plans::prepare_import_files_with_pool(
            &state.pool,
            added.clone(),
            HashMap::new(),
            &cancel
        ),
    );
    assert_ne!(first.is_ok(), second.is_ok());
    let removed = first.or(second).unwrap();
    assert_eq!(removed.revision, added.revision + 1);
    assert!(removed.issues.iter().any(|issue| issue.code
        == ResourceIssueCode::ManualDownloadRequired
        && issue.blocking));
    state.pool.close().await;
}

#[tokio::test]
async fn entry_graph_stores_only_the_entry_dependency_closure() {
    let (_temp, state, original, _, _) = fixture().await;
    let mut plan = local_plan(
        &state,
        original,
        artifact(SourceId::curseforge(), "123", "456", b"new bytes"),
        "unused",
        None,
    )
    .await;
    let root = plan.nodes[0].id.clone();
    let mut unrelated = plan.nodes[0].clone();
    unrelated.id = SelectionId::new("unrelated").unwrap();
    plan.nodes.push(unrelated);
    let (nodes, edges) = entry_graph(&plan, &root);
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].id, root);
    assert!(edges.is_empty());
    state.pool.close().await;
}

#[tokio::test]
async fn connector_download_with_hash_and_unknown_size_commits_measured_bytes_and_exact_source()
 {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let (_temp, state, original, _, root) = fixture().await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let data = b"new bytes";
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = vec![0u8; 4096];
        let _bytes_read = socket.read(&mut request).await.unwrap();
        socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 9\r\nConnection: close\r\n\r\nnew bytes").await.unwrap();
    });
    let mut target = artifact(
        SourceId::new("planet_minecraft").unwrap(),
        "opaque-project",
        "release",
        data,
    );
    target.integrity.size = None;
    target.acquisition = ArtifactAcquisition::PublicDownload {
        urls: vec![format!("http://{address}/new.zip")],
    };
    let plan =
        local_plan(&state, original.clone(), target, "unused", None).await;
    let payload: String = sqlx::query_scalar(
        "SELECT payload FROM resource_operation_plans WHERE id = ?",
    )
    .bind(plan.id.as_str())
    .fetch_one(&state.pool)
    .await
    .unwrap();
    let mut draft: plans::OperationDraft =
        serde_json::from_str(&payload).unwrap();
    draft.local_files.clear();
    sqlx::query("UPDATE resource_operation_plans SET payload = ? WHERE id = ?")
        .bind(serde_json::to_string(&draft).unwrap())
        .bind(plan.id.as_str())
        .execute(&state.pool)
        .await
        .unwrap();
    execute("instance", &plan, None, &state).await.unwrap();
    server.await.unwrap();
    let current = plans::installed(&state.pool, "instance")
        .await
        .unwrap()
        .remove(0);
    assert_eq!(
        current
            .artifact
            .as_ref()
            .unwrap()
            .release
            .project
            .source
            .as_str(),
        "planet_minecraft"
    );
    assert_eq!(current.integrity.size, Some(9));
    assert_eq!(
        std::fs::read(root.join(&original.relative_path)).unwrap(),
        data
    );
    let histories = history::list_with_state("instance", None, &state)
        .await
        .unwrap();
    assert_eq!(histories[0].target.integrity.size, Some(9));
    assert!(histories[0].backup_available);
    state.pool.close().await;
}

#[tokio::test]
async fn verified_manual_import_rejects_changed_bytes_and_preserves_connector_source_and_size()
 {
    let (_temp, state, original, _, root) = fixture().await;
    let source = root.join("download.zip");
    std::fs::write(&source, b"new bytes").unwrap();
    let mut artifact = artifact(
        SourceId::new("planet_minecraft").unwrap(),
        "opaque-project",
        "opaque-release",
        b"new bytes",
    );
    artifact.integrity.size = None;
    artifact.acquisition = ArtifactAcquisition::ManualDownload {
        page_url: "https://www.planetminecraft.com/project/example".into(),
    };
    let plan =
        local_plan(&state, original.clone(), artifact, "download.zip", None)
            .await;
    let payload: String = sqlx::query_scalar(
        "SELECT payload FROM resource_operation_plans WHERE id = ?",
    )
    .bind(plan.id.as_str())
    .fetch_one(&state.pool)
    .await
    .unwrap();
    let mut draft: plans::OperationDraft =
        serde_json::from_str(&payload).unwrap();
    draft.local_files.clear();
    draft.import_files.insert(
        draft.plan.actions[0].node_id.clone(),
        source.to_string_lossy().into_owned(),
    );
    draft.plan.issues.push(ResourcePlanIssue {
        code: ResourceIssueCode::ManualDownloadRequired,
        selection_id: Some(draft.plan.actions[0].node_id.clone()),
        entry_id: None,
        blocking: true,
        message: "manual".into(),
    });
    plans::verify_import_files(&mut draft).await.unwrap();
    assert!(draft.plan.issues.is_empty());
    assert_eq!(draft.plan.actions[0].target.integrity.size, Some(9));
    sqlx::query("UPDATE resource_operation_plans SET payload = ? WHERE id = ?")
        .bind(serde_json::to_string(&draft).unwrap())
        .bind(plan.id.as_str())
        .execute(&state.pool)
        .await
        .unwrap();
    std::fs::write(&source, b"changed bytes").unwrap();
    assert!(
        execute("instance", &draft.plan, None, &state)
            .await
            .is_err()
    );
    assert_eq!(
        std::fs::read(root.join(&original.relative_path)).unwrap(),
        b"old bytes"
    );
    std::fs::write(&source, b"new bytes").unwrap();
    execute("instance", &draft.plan, None, &state)
        .await
        .unwrap();
    let current = plans::installed(&state.pool, "instance")
        .await
        .unwrap()
        .remove(0);
    assert_eq!(
        current
            .artifact
            .as_ref()
            .unwrap()
            .release
            .project
            .source
            .as_str(),
        "planet_minecraft"
    );
    assert_eq!(current.integrity.size, Some(9));
    assert!(
        matches_file(&root.join(&current.relative_path), &current.integrity)
            .await
            .unwrap()
    );
    let history = history::list_with_state("instance", None, &state)
        .await
        .unwrap()
        .remove(0);
    assert_eq!(history.target.integrity.size, Some(9));
    assert!(
        sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&state.pool)
            .await
            .unwrap()
            .is_empty()
    );
    state.pool.close().await;
}

#[tokio::test]
async fn externally_replaced_file_does_not_reuse_local_metadata_or_icon() {
    let (_temp, state, original, _, root) = fixture().await;
    sqlx::query("UPDATE instance_files SET local_mod_data = ?, icon_path = 'old-icon.png' WHERE instance_id = 'instance'")
        .bind(r#"{"mod_id":"old","name":"Old title","dependencies":[]}"#).execute(&state.pool).await.unwrap();
    std::fs::write(
        root.join(&original.relative_path),
        b"externally changed content",
    )
    .unwrap();
    let cache =
        crate::state::CacheValue::FileHash(crate::state::CachedFileHash {
            path: format!("example/{}", original.relative_path),
            size: b"externally changed content".len() as u64,
            hash: sha1_smol::Sha1::from(b"externally changed content")
                .hexdigest(),
            project_type: Some(crate::state::ProjectType::ResourcePack),
            project_id: None,
            version_id: None,
        })
        .get_entry();
    let mut tx = state.pool.begin().await.unwrap();
    crate::state::CachedEntry::upsert_many(&[cache], &mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    crate::state::sync_content_files("instance", &state)
        .await
        .unwrap();
    let (metadata, icon): (Option<String>, Option<String>) = sqlx::query_as("SELECT local_mod_data, icon_path FROM instance_files WHERE instance_id = 'instance' AND relative_path = ?")
        .bind(&original.relative_path).fetch_one(&state.pool).await.unwrap();
    assert!(metadata.is_none());
    assert_ne!(icon.as_deref(), Some("old-icon.png"));
}

#[tokio::test]
async fn display_snapshot_keeps_local_entries_without_verifying_history_backups()
 {
    let (_temp, state, original, _, root) = fixture().await;
    std::fs::write(root.join("new.zip"), b"new bytes").unwrap();
    let plan = local_plan(
        &state,
        original.clone(),
        artifact(SourceId::curseforge(), "123", "456", b"new bytes"),
        "new.zip",
        None,
    )
    .await;
    execute("instance", &plan, None, &state).await.unwrap();
    let snapshot =
        crate::state::instances::commands::get_local_content_snapshot(
            "instance", &state,
        )
        .await
        .unwrap();
    assert_eq!(snapshot.items.len(), 1);
    assert_eq!(
        snapshot.items[0].entry_id.as_deref(),
        Some(original.entry_id.as_str())
    );
    assert_eq!(
        snapshot.items[0].expected_relative_path,
        original.relative_path
    );
    assert!(
        snapshot.items[0]
            .content
            .as_ref()
            .unwrap()
            .rollback
            .is_none()
    );
    assert!(snapshot.warnings.is_empty());
}

#[tokio::test]
async fn update_restart_cache_clear_unlink_rollback_and_cleanup_keep_business_facts()
 {
    let (temp, state, original, old, root) = fixture().await;
    pack_fixture(&state, &original).await;
    sqlx::query("UPDATE settings SET resource_chinese_filenames = 0, resource_preserve_filenames = 0 WHERE id = 0").execute(&state.pool).await.unwrap();
    let managed = plans::installed(&state.pool, "instance")
        .await
        .unwrap()
        .remove(0);
    std::fs::write(root.join("new.zip"), b"new bytes").unwrap();
    let plan = local_plan(
        &state,
        managed,
        artifact(SourceId::curseforge(), "123", "456", b"new bytes"),
        "new.zip",
        None,
    )
    .await;
    let plan = change_target_path(
        &state,
        plan,
        "resourcepacks/custom-456.zip.disabled",
    )
    .await;
    let payload: String = sqlx::query_scalar(
        "SELECT payload FROM resource_operation_plans WHERE id = ?",
    )
    .bind(plan.id.as_str())
    .fetch_one(&state.pool)
    .await
    .unwrap();
    let mut old_payload: serde_json::Value =
        serde_json::from_str(&payload).unwrap();
    old_payload.as_object_mut().unwrap().remove("naming");
    let legacy: plans::OperationDraft =
        serde_json::from_value(old_payload.clone()).unwrap();
    assert!(legacy.naming.chinese && legacy.naming.preserve);
    sqlx::query("UPDATE resource_operation_plans SET payload = ? WHERE id = ?")
        .bind(old_payload.to_string())
        .bind(plan.id.as_str())
        .execute(&state.pool)
        .await
        .unwrap();
    let request = crate::install::InstallRequest::InstallContentBatch {
        instance_id: "instance".into(),
        items: vec![crate::install::InstallContentBatchItem::FrozenResource {
            plan: plan.clone(),
        }],
        display_title: "Reviewed update".into(),
        display_icon: None,
    };
    let job_id = uuid::Uuid::new_v4();
    crate::install::store::insert(
        job_id,
        &crate::install::model::InstallJobState::new(request.clone()),
        crate::install::InstallJobStatus::Queued,
        &state,
    )
    .await
    .unwrap();
    let directories = DirectoryInfo {
        settings_dir: state.directories.settings_dir.clone(),
        config_dir: state.directories.config_dir.clone(),
        app_identifier: state.directories.app_identifier.clone(),
    };
    state.pool.close().await;
    drop(state);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(temp.path().join("state.db"))
                .foreign_keys(true),
        )
        .await
        .unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();
    let state = crate::state::test_state(directories, pool).await.unwrap();
    let cache = super::cache::ResourceCache::open(
        temp.path().join("resource-cache.db"),
    )
    .await;
    cache.clear().await.unwrap();
    cache.close().await;
    let recovered = crate::install::store::get_required(job_id, &state)
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(&recovered.state.request).unwrap(),
        serde_json::to_value(&request).unwrap()
    );
    assert_eq!(recovered.status, crate::install::InstallJobStatus::Queued);
    let settings = crate::state::Settings::get(&state.pool).await.unwrap();
    assert!(
        !settings.resource_chinese_filenames
            && !settings.resource_preserve_filenames
    );
    execute("instance", &plan, None, &state).await.unwrap();
    assert!(
        plans::frozen_request_completed(&request, &state.pool)
            .await
            .unwrap()
    );
    sqlx::query("DELETE FROM install_jobs WHERE id = ?")
        .bind(job_id.to_string())
        .execute(&state.pool)
        .await
        .unwrap();
    let changed = plans::installed(&state.pool, "instance")
        .await
        .unwrap()
        .remove(0);
    assert_eq!(changed.entry_id, original.entry_id);
    assert_eq!(
        changed.artifact.as_ref().unwrap().release.project.source,
        SourceId::curseforge()
    );
    assert_eq!(
        changed.relative_path,
        "resourcepacks/custom-456.zip.disabled"
    );
    let before_unlink = history::list_with_state("instance", None, &state)
        .await
        .unwrap()
        .remove(0);
    crate::state::edit_instance(
        "instance",
        crate::state::EditInstance {
            link: Some(crate::state::InstanceLink::Unmanaged),
            ..Default::default()
        },
        &state.pool,
    )
    .await
    .unwrap();
    let draft = history::prepare_rollback(
        "instance".into(),
        original.entry_id.clone(),
        before_unlink.id,
        &state,
    )
    .await
    .unwrap();
    assert_eq!(draft.plan.actions[0].target.reference, old.reference);
    sqlx::query("INSERT INTO resource_operation_plans(id, instance_id, revision, payload) VALUES (?, 'instance', 1, ?)").bind(draft.plan.id.as_str()).bind(serde_json::to_string(&draft).unwrap()).execute(&state.pool).await.unwrap();
    execute("instance", &draft.plan, None, &state)
        .await
        .unwrap();
    let restored = plans::installed(&state.pool, "instance")
        .await
        .unwrap()
        .remove(0);
    assert_eq!(restored.entry_id, original.entry_id);
    assert_eq!(restored.resource_id, original.resource_id);
    assert_eq!(restored.artifact, original.artifact);
    assert_eq!(restored.relative_path, original.relative_path);
    assert_eq!(restored.known_artifacts, original.known_artifacts);
    assert_eq!(restored.ownership, ResourceOwnership::UserAdded);
    assert_eq!(
        std::fs::read(root.join(&original.relative_path)).unwrap(),
        b"old bytes"
    );
    assert!(!root.join(&changed.relative_path).exists());
    crate::state::instances::commands::repair_recorded_pack_unlink(
        "instance",
        &state.pool,
    )
    .await
    .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM instance_pack_members"
        )
        .fetch_one(&state.pool)
        .await
        .unwrap(),
        0
    );
    let backups = super::history_cleanup::scan_with_state("instance", &state)
        .await
        .unwrap();
    let selected: Vec<_> = backups
        .iter()
        .filter(|file| file.recognized && file.exists)
        .map(|file| file.id.clone())
        .collect();
    assert!(!selected.is_empty());
    let cleanup = super::history_cleanup::prepare_with_state(
        "instance", selected, &state,
    )
    .await
    .unwrap();
    let result = super::history_cleanup::execute_with_state(
        cleanup.id,
        cleanup.revision,
        &state,
    )
    .await
    .unwrap();
    assert!(result.failures.is_empty());
    assert!(!result.removed.is_empty());
    assert!(
        history::list_with_state("instance", None, &state)
            .await
            .unwrap()
            .iter()
            .all(|entry| !entry.backup_available)
    );
    assert_eq!(
        std::fs::read(root.join(&original.relative_path)).unwrap(),
        b"old bytes"
    );
    assert!(
        sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&state.pool)
            .await
            .unwrap()
            .is_empty()
    );
    state.pool.close().await;
}

fn artifact(
    source: SourceId,
    project: &str,
    release: &str,
    bytes: &[u8],
) -> ResourceArtifact {
    ResourceArtifact {
        reference: SourceArtifactRef {
            release: SourceReleaseRef {
                project: SourceProjectRef {
                    source,
                    project_id: SourceProjectId::new(project).unwrap(),
                },
                release_id: SourceReleaseId::new(release).unwrap(),
            },
            artifact_id: SourceArtifactId::new(release).unwrap(),
        },
        file_name: Some(format!("{release}.zip")),
        role: ArtifactRole::Primary,
        available: Some(true),
        integrity: ArtifactIntegrity {
            size: Some(bytes.len() as u64),
            hashes: vec![ContentHash {
                algorithm: HashAlgorithm::Sha1,
                value: sha1_smol::Sha1::from(bytes).hexdigest(),
            }],
        },
        acquisition: ArtifactAcquisition::PublicDownload {
            urls: vec!["https://invalid.example/frozen.zip".into()],
        },
        dependencies: vec![],
    }
}

async fn fixture() -> (
    tempfile::TempDir,
    std::sync::Arc<crate::State>,
    InstalledResource,
    ResourceArtifact,
    PathBuf,
) {
    let temp = tempfile::tempdir().unwrap();
    let directories = DirectoryInfo {
        settings_dir: temp.path().to_path_buf(),
        config_dir: temp.path().to_path_buf(),
        app_identifier: "resource-execution-test".into(),
    };
    let root = directories.instances_dir().join("example");
    std::fs::create_dir_all(root.join("resourcepacks")).unwrap();
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(temp.path().join("state.db"))
                .create_if_missing(true)
                .foreign_keys(true),
        )
        .await
        .unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();
    sqlx::raw_sql("INSERT INTO instances(id, path, install_stage, launcher_feature_version, name, created, modified) VALUES ('instance', 'example', 'installed', '1', 'Example', 1, 1); INSERT INTO instance_content_sets(id, instance_id, name, source_kind, status, game_version, loader, created, modified) VALUES ('set', 'instance', 'Example', 'local', 'available', '1.20.1', 'fabric', 1, 1); UPDATE instances SET applied_content_set_id = 'set' WHERE id = 'instance';").execute(&pool).await.unwrap();
    let state = crate::state::test_state(directories, pool.clone())
        .await
        .unwrap();
    let old = artifact(SourceId::modrinth(), "root0000", "old", b"old bytes");
    let current_path = "resourcepacks/prefix-old.zip.disabled";
    std::fs::write(root.join(current_path), b"old bytes").unwrap();
    let file = content_rows::upsert_instance_file_from_parts(
        content_rows::UpsertInstanceFile {
            instance_id: "instance",
            relative_path: current_path,
            file_name: "prefix-old.zip.disabled",
            enabled: false,
            sha1: &old.integrity.hashes[0].value,
            size: 9,
            missing: false,
            local_mod_data: None,
            icon_path: None,
        },
        &pool,
    )
    .await
    .unwrap();
    let entry = content_rows::upsert_content_entry_from_parts(
        content_rows::UpsertContentEntry {
            instance_id: "instance",
            content_set_id: "set",
            file_id: Some(&file.id),
            project_type: ProjectType::ResourcePack,
            source_kind: ContentSourceKind::Local,
            ownership_kind: ContentOwnershipKind::UserAdded,
            auto_dependency: false,
            server_requirement: ContentRequirement::Optional,
            client_requirement: ContentRequirement::Required,
            enabled: false,
        },
        &pool,
    )
    .await
    .unwrap();
    let reference = ContentProviderRef::Modrinth {
        project_id: ModrinthProjectId::new("root0000").unwrap(),
        version_id: Some(ModrinthVersionId::new("old").unwrap()),
    };
    content_rows::upsert_content_provider_ref(
        &entry.id, &reference, true, &pool,
    )
    .await
    .unwrap();
    sqlx::query("INSERT INTO resource_installed_artifacts(entry_id, artifact, scope) VALUES (?, ?, ?)").bind(&entry.id).bind(serde_json::to_string(&old).unwrap()).bind(serde_json::to_string(&InstallScope::Instance).unwrap()).execute(&pool).await.unwrap();
    let installed =
        plans::installed(&pool, "instance").await.unwrap().remove(0);
    (temp, state, installed, old, root)
}

async fn local_plan(
    state: &crate::State,
    current: InstalledResource,
    target: ResourceArtifact,
    source_path: &str,
    restore: Option<HistoryId>,
) -> ResourceOperationPlan {
    let id = PlanId::new(uuid::Uuid::new_v4().to_string()).unwrap();
    let node_id = SelectionId::new("root").unwrap();
    let reference = ResourceRef::Source {
        project: target.reference.release.project.clone(),
    };
    let plan = ResourceOperationPlan {
        schema_version: ResourceContractVersion,
        id: id.clone(),
        revision: 1,
        instance_id: "instance".into(),
        instance_revision: plans::instance_revision(&state.pool, "instance")
            .await
            .unwrap(),
        intent: ResourceOperationIntent::SwitchVersion,
        selections: vec![ResourceSelection {
            id: node_id.clone(),
            resource: reference.clone(),
            release: ReleaseSelection::Artifact {
                reference: target.reference.clone(),
            },
            resource_type: ResourceType::ResourcePack,
            scope: current.scope.clone(),
            entry_id: Some(current.entry_id.clone()),
        }],
        active_roots: vec![node_id.clone()],
        nodes: vec![ResourceDependencyNode {
            id: node_id.clone(),
            root_selection_ids: vec![node_id.clone()],
            resource_type: Some(ResourceType::ResourcePack),
            resource: reference,
            scope: current.scope.clone(),
            state: DependencyNodeState::Update,
            artifact: Some(target.clone()),
            installed_entries: vec![],
            compatibility: CompatibilityAssessment {
                game_version: CompatibilityState::Compatible,
                loader: CompatibilityState::Compatible,
                reasons: vec![],
            },
            declared_compatibility: Some(ReleaseCompatibility {
                game_versions: vec!["1.20.1".into()],
                loaders: vec![],
            }),
        }],
        edges: vec![],
        actions: vec![ResourceEntryAction {
            entry_id: current.entry_id.clone(),
            node_id,
            kind: ResourceActionKind::Replace,
            target,
            target_relative_path: current.relative_path.clone(),
            current: Some(current.clone()),
        }],
        issues: vec![],
        exceptions: vec![],
        selection_reasons: vec![],
    };
    let draft = plans::OperationDraft {
        local_metadata: Default::default(),
        retained_requirements: Vec::new(),
        resolve_dependencies: true,
        skip_unwritable_files: false,
        import_files: Default::default(),
        naming: Default::default(),
        plan: plan.clone(),
        install_dependencies: false,
        candidates: vec![],
        local_files: HashMap::from([(
            current.entry_id.clone(),
            source_path.into(),
        )]),
        restore_history: restore,
    };
    sqlx::query("INSERT INTO resource_operation_plans(id, instance_id, revision, payload) VALUES (?, 'instance', 1, ?)").bind(id.as_str()).bind(serde_json::to_string(&draft).unwrap()).execute(&state.pool).await.unwrap();
    plan
}

#[tokio::test]
async fn forced_writes_skip_readonly_files_without_changing_their_database_or_history()
 {
    for readonly_during_review in [false, true] {
        let (_temp, state, original, _, root) = fixture().await;
        std::fs::write(root.join("new.zip"), b"new bytes").unwrap();
        let target =
            artifact(SourceId::modrinth(), "root0000", "new", b"new bytes");
        let plan = local_plan(
            &state,
            original.clone(),
            target.clone(),
            "new.zip",
            None,
        )
        .await;
        let mut draft = plans::load(&state.pool, &plan.id, plan.revision)
            .await
            .unwrap();
        let mut added = plan.actions[0].clone();
        added.entry_id = InstalledEntryId::new("added-entry").unwrap();
        added.node_id = SelectionId::new("added-root").unwrap();
        added.kind = ResourceActionKind::Install;
        added.current = None;
        added.target =
            artifact(SourceId::modrinth(), "added000", "new", b"new bytes");
        added.target_relative_path = "resourcepacks/added.zip".into();
        let mut node = plan.nodes[0].clone();
        node.id = added.node_id.clone();
        node.root_selection_ids = vec![node.id.clone()];
        node.resource = ResourceRef::Source {
            project: added.target.reference.release.project.clone(),
        };
        node.artifact = Some(added.target.clone());
        draft.plan.nodes.push(node);
        draft.plan.active_roots.push(added.node_id.clone());
        draft
            .local_files
            .insert(added.entry_id.clone(), "new.zip".into());
        draft.plan.actions.push(added);
        let current_path = root.join(&original.relative_path);
        let permissions =
            std::fs::metadata(&current_path).unwrap().permissions();
        let mut readonly = permissions.clone();
        readonly.set_readonly(true);
        if readonly_during_review {
            std::fs::set_permissions(&current_path, readonly.clone()).unwrap();
        }
        super::super::write_access::review_at_root(&mut draft, &root).unwrap();
        assert_eq!(
            draft
                .plan
                .issues
                .iter()
                .any(|issue| issue.code == ResourceIssueCode::FileNotWritable
                    && issue.blocking),
            readonly_during_review
        );
        draft.skip_unwritable_files = true;
        super::super::write_access::review_at_root(&mut draft, &root).unwrap();
        assert_eq!(
            draft.plan.actions.len(),
            if readonly_during_review { 1 } else { 2 }
        );
        sqlx::query(
            "UPDATE resource_operation_plans SET payload = ? WHERE id = ?",
        )
        .bind(serde_json::to_string(&draft).unwrap())
        .bind(plan.id.as_str())
        .execute(&state.pool)
        .await
        .unwrap();
        std::fs::set_permissions(&current_path, readonly).unwrap();
        let result = execute("instance", &draft.plan, None, &state).await;
        std::fs::set_permissions(&current_path, permissions).unwrap();
        result.unwrap();
        assert_eq!(std::fs::read(&current_path).unwrap(), b"old bytes");
        assert_eq!(
            std::fs::read(root.join("resourcepacks/added.zip")).unwrap(),
            b"new bytes"
        );
        let installed =
            plans::installed(&state.pool, "instance").await.unwrap();
        assert_eq!(installed.len(), 2);
        assert_eq!(
            installed
                .iter()
                .find(|entry| entry.entry_id == original.entry_id)
                .unwrap(),
            &original
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM resource_history"
            )
            .fetch_one(&state.pool)
            .await
            .unwrap(),
            0
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT completed FROM resource_operation_plans WHERE id = ?"
            )
            .bind(plan.id.as_str())
            .fetch_one(&state.pool)
            .await
            .unwrap(),
            1
        );
        let mut invalid = draft.clone();
        invalid.plan.actions[0].target_relative_path = "../outside.zip".into();
        super::super::write_access::review_at_root(&mut invalid, &root)
            .unwrap();
        assert!(
            invalid
                .plan
                .issues
                .iter()
                .any(|issue| issue.code == ResourceIssueCode::InvalidPath
                    && issue.blocking)
        );
        assert!(!root.parent().unwrap().join("outside.zip").exists());
        assert!(
            sqlx::query("PRAGMA foreign_key_check")
                .fetch_all(&state.pool)
                .await
                .unwrap()
                .is_empty()
        );
        state.pool.close().await;
    }
}

#[cfg(windows)]
#[tokio::test]
async fn forced_writes_detect_windows_files_open_without_delete_sharing() {
    use std::os::windows::fs::OpenOptionsExt;
    let (_temp, state, original, _, root) = fixture().await;
    let target =
        artifact(SourceId::modrinth(), "root0000", "new", b"new bytes");
    let plan =
        local_plan(&state, original.clone(), target, "new.zip", None).await;
    let mut draft = plans::load(&state.pool, &plan.id, plan.revision)
        .await
        .unwrap();
    let locked = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(root.join(&original.relative_path))
        .unwrap();
    super::super::write_access::review_at_root(&mut draft, &root).unwrap();
    assert!(
        draft
            .plan
            .issues
            .iter()
            .any(|issue| issue.code == ResourceIssueCode::FileNotWritable
                && issue.blocking)
    );
    draft.skip_unwritable_files = true;
    super::super::write_access::review_at_root(&mut draft, &root).unwrap();
    assert!(draft.plan.actions.is_empty());
    assert!(
        draft
            .plan
            .issues
            .iter()
            .any(|issue| issue.code == ResourceIssueCode::FileWriteSkipped
                && !issue.blocking)
    );
    assert_eq!(
        std::fs::read(root.join(&original.relative_path)).unwrap(),
        b"old bytes"
    );
    drop(locked);
    assert_eq!(
        plans::installed(&state.pool, "instance").await.unwrap(),
        vec![original]
    );
    state.pool.close().await;
}

#[tokio::test]
async fn exact_cross_source_replace_and_rollback_preserve_entry_disabled_path_and_metadata()
 {
    let (_temp, state, original, old, root) = fixture().await;
    std::fs::write(root.join("new.zip"), b"new bytes").unwrap();
    let mut target =
        artifact(SourceId::curseforge(), "123", "456", b"new bytes");
    use sha2::Digest;
    target.integrity.hashes = vec![ContentHash {
        algorithm: HashAlgorithm::Sha512,
        value: format!("{:x}", sha2::Sha512::digest(b"new bytes")),
    }];
    let plan =
        local_plan(&state, original.clone(), target.clone(), "new.zip", None)
            .await;
    tokio::time::timeout(
        std::time::Duration::from_secs(10),
        execute("instance", &plan, None, &state),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        std::fs::read(root.join(&original.relative_path)).unwrap(),
        b"new bytes"
    );
    let changed = plans::installed(&state.pool, "instance")
        .await
        .unwrap()
        .remove(0);
    assert_eq!(changed.entry_id, original.entry_id);
    assert_eq!(changed.relative_path, original.relative_path);
    assert!(!changed.enabled);
    assert_eq!(changed.artifact, Some(target.reference));
    let mut snapshot = vec![crate::state::InstanceContentSnapshotItem {
        artifact_ref: None,
        compatibility: None,
        resource_id: None,
        pack_resource_id: None,
        file_id: None,
        entry_id: Some(original.entry_id.as_str().into()),
        member_id: None,
        ownership_kind: ContentOwnershipKind::UserAdded,
        materialization_state: crate::state::PackMemberMaterializationState::Present,
        override_kind: crate::state::PackMemberOverrideKind::None,
        expected_relative_path: original.relative_path.clone(),
        required: false,
        project_type: ProjectType::ResourcePack,
        provider: None,
        provider_project_id: None,
        provider_release_id: None,
        content: Some(crate::state::ContentItem {
            file_name: "prefix-old.zip.disabled".into(),
            ..serde_json::from_value(serde_json::json!({
                "file_name": "prefix-old.zip.disabled", "project_type": "resourcepack",
                "file_path": original.relative_path, "id": "test", "size": 9,
                "enabled": false, "provider_refs": [], "external": false
            })).unwrap()
        }),
        capabilities: Default::default(),
        dependency: None,
    }];
    history::attach_to_snapshot("instance", &mut snapshot, &state)
        .await
        .unwrap();
    super::installed::attach_compatibility(
        &state.pool,
        "instance",
        "1.21.1",
        "neoforge",
        &mut snapshot,
        None,
    )
    .await
    .unwrap();
    assert_eq!(
        snapshot[0].compatibility.as_ref().unwrap().game_version,
        CompatibilityState::Incompatible
    );
    assert_eq!(
        snapshot[0].compatibility.as_ref().unwrap().loader,
        CompatibilityState::Compatible
    );
    super::installed::attach_compatibility(
        &state.pool,
        "instance",
        "1.20.1",
        "fabric",
        &mut snapshot,
        None,
    )
    .await
    .unwrap();
    assert_eq!(
        snapshot[0].compatibility.as_ref().unwrap().game_version,
        CompatibilityState::Compatible
    );
    let saved: String = sqlx::query_scalar(
        "SELECT artifact FROM resource_installed_artifacts WHERE entry_id = ?",
    )
    .bind(original.entry_id.as_str())
    .fetch_one(&state.pool)
    .await
    .unwrap();
    let record: super::installed::InstalledArtifactRecord =
        serde_json::from_str(&saved).unwrap();
    assert_eq!(
        record
            .declared_compatibility
            .as_ref()
            .unwrap()
            .game_versions,
        vec!["1.20.1"]
    );
    let legacy: ResourceArtifact = serde_json::from_str(&saved).unwrap();
    assert_eq!(legacy.reference, record.artifact.reference);
    let historical: super::installed::InstalledArtifactRecord =
        serde_json::from_str(&serde_json::to_string(&legacy).unwrap()).unwrap();
    assert!(historical.declared_compatibility.is_none());
    assert!(snapshot[0].content.as_ref().unwrap().rollback.is_some());
    sqlx::query("UPDATE resource_history SET backup_available = 0 WHERE instance_id = 'instance'").execute(&state.pool).await.unwrap();
    history::attach_to_snapshot("instance", &mut snapshot, &state)
        .await
        .unwrap();
    assert!(snapshot[0].content.as_ref().unwrap().rollback.is_none());
    sqlx::query("UPDATE resource_history SET backup_available = 1 WHERE instance_id = 'instance'").execute(&state.pool).await.unwrap();
    history::attach_to_snapshot("instance", &mut snapshot, &state)
        .await
        .unwrap();
    assert!(snapshot[0].content.as_ref().unwrap().rollback.is_some());
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM instance_content_entries")
            .fetch_one(&state.pool)
            .await
            .unwrap();
    assert_eq!(count, 1);
    let history_json: String = sqlx::query_scalar(
        "SELECT payload FROM resource_history WHERE entry_id = ?",
    )
    .bind(original.entry_id.as_str())
    .fetch_one(&state.pool)
    .await
    .unwrap();
    let history: ResourceHistoryEntry =
        serde_json::from_str(&history_json).unwrap();
    assert!(
        matches_file(
            &root.join(history.backup_relative_path.as_ref().unwrap()),
            &original.integrity
        )
        .await
        .unwrap()
    );
    assert!(serde_json::from_str::<serde_json::Value>(&history_json).unwrap()["previousArtifact"].is_object());
    execute("instance", &plan, None, &state).await.unwrap();
    let request = crate::install::InstallRequest::InstallContentBatch {
        instance_id: "instance".into(),
        items: vec![crate::install::InstallContentBatchItem::FrozenResource {
            plan: plan.clone(),
        }],
        display_title: "Test".into(),
        display_icon: None,
    };
    assert!(
        plans::frozen_request_completed(&request, &state.pool)
            .await
            .unwrap()
    );
    let mut job = crate::install::model::InstallJobState::new(request);
    job.record_event(crate::install::InstallJobEventKind::JobSucceeded {
        instance_id: Some("instance".into()),
    });
    let job_id = uuid::Uuid::new_v4();
    crate::install::store::insert(
        job_id,
        &job,
        crate::install::InstallJobStatus::Canceling,
        &state,
    )
    .await
    .unwrap();
    let completed =
        crate::install::store::complete_running_job(job_id, &job, &state)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(
        completed.status,
        crate::install::InstallJobStatus::Succeeded
    );
    let restart_id = uuid::Uuid::new_v4();
    crate::install::store::insert(
        restart_id,
        &job,
        crate::install::InstallJobStatus::Running,
        &state,
    )
    .await
    .unwrap();
    crate::install::recovery::recover_interrupted_jobs(&state)
        .await
        .unwrap();
    assert_eq!(
        crate::install::store::get_required(restart_id, &state)
            .await
            .unwrap()
            .status,
        crate::install::InstallJobStatus::Succeeded
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM resource_history")
            .fetch_one(&state.pool)
            .await
            .unwrap(),
        1
    );
    let rollback = local_plan(
        &state,
        changed,
        old,
        history.backup_relative_path.as_ref().unwrap(),
        Some(history.id),
    )
    .await;
    let backup = root.join(history.backup_relative_path.as_ref().unwrap());
    let backup_bytes = std::fs::read(&backup).unwrap();
    std::fs::remove_file(&backup).unwrap();
    history::attach_to_snapshot("instance", &mut snapshot, &state)
        .await
        .unwrap();
    assert!(snapshot[0].content.as_ref().unwrap().rollback.is_none());
    std::fs::write(&backup, backup_bytes).unwrap();
    execute("instance", &rollback, None, &state).await.unwrap();
    assert_eq!(
        std::fs::read(root.join(&original.relative_path)).unwrap(),
        b"old bytes"
    );
    let restored = plans::installed(&state.pool, "instance")
        .await
        .unwrap()
        .remove(0);
    assert_eq!(restored.entry_id, original.entry_id);
    assert_eq!(restored.artifact, original.artifact);
    assert_eq!(restored.ownership, original.ownership);
    assert!(!restored.enabled);
    let histories =
        history::list_with_state("instance", Some(&original.entry_id), &state)
            .await
            .unwrap();
    assert_eq!(histories.len(), 2);
    let consumed = histories
        .iter()
        .find(|history| !history.backup_available)
        .unwrap();
    assert!(consumed.backup_retired);
    assert!(consumed.backup_relative_path.is_some());
    assert!(consumed.backup_integrity.is_some());
    assert!(histories.iter().any(|history| history.backup_available));
    let mut legacy_payload = serde_json::to_value(consumed).unwrap();
    legacy_payload
        .as_object_mut()
        .unwrap()
        .remove("backupRetired");
    sqlx::query("UPDATE resource_history SET payload = ? WHERE id = ?")
        .bind(legacy_payload.to_string())
        .bind(consumed.id.as_str())
        .execute(&state.pool)
        .await
        .unwrap();
    let backups = super::history_cleanup::scan_with_state("instance", &state)
        .await
        .unwrap();
    let retired = backups
        .iter()
        .find(|backup| backup.id == consumed.id)
        .unwrap();
    assert_eq!(retired.history_id.as_ref(), Some(&consumed.id));
    assert!(retired.exists);
    assert!(!retired.rollback_available);
    let cleanup = super::history_cleanup::prepare_with_state(
        "instance",
        vec![consumed.id.clone()],
        &state,
    )
    .await
    .unwrap();
    super::history_cleanup::execute_with_state(
        cleanup.id,
        cleanup.revision,
        &state,
    )
    .await
    .unwrap();
    assert!(
        !root
            .join(consumed.backup_relative_path.as_ref().unwrap())
            .exists()
    );
    let retained: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM store_retained_refs WHERE reason = ?",
    )
    .bind(format!("resource-history:{}", consumed.id.as_str()))
    .fetch_one(&state.pool)
    .await
    .unwrap();
    assert_eq!(retained, 0);
    assert_eq!(
        std::fs::read(root.join(&original.relative_path)).unwrap(),
        b"old bytes"
    );
    let entry = content_rows::get_content_entry_by_id(
        original.entry_id.as_str(),
        &state.pool,
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(entry.server_requirement, ContentRequirement::Optional);
    assert_eq!(entry.client_requirement, ContentRequirement::Required);
    assert!(
        sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&state.pool)
            .await
            .unwrap()
            .is_empty()
    );
    state.pool.close().await;
}

#[tokio::test]
async fn install_cleanup_restores_resource_journal_without_reselection() {
    let (_temp, state, original, _old, root) = fixture().await;
    std::fs::write(root.join("new.zip"), b"new bytes").unwrap();
    let target = artifact(SourceId::curseforge(), "123", "456", b"new bytes");
    let plan =
        local_plan(&state, original.clone(), target.clone(), "new.zip", None)
            .await;
    let backup = "resourcepacks/interrupted.old";
    std::fs::rename(root.join(&original.relative_path), root.join(backup))
        .unwrap();
    let journal = vec![JournalFile {
        previous_path: None,
        path: original.relative_path.clone(),
        backup: Some(backup.into()),
        old: Some(original.integrity),
        target: target.integrity,
    }];
    sqlx::query("INSERT INTO resource_execution_journals(plan_id, payload) VALUES (?, ?)").bind(plan.id.as_str()).bind(serde_json::to_string(&journal).unwrap()).execute(&state.pool).await.unwrap();
    let mut job = crate::install::model::InstallJobState::new(
        crate::install::InstallRequest::InstallContentBatch {
            instance_id: "instance".into(),
            items: vec![
                crate::install::InstallContentBatchItem::FrozenResource {
                    plan,
                },
            ],
            display_title: "Recover".into(),
            display_icon: None,
        },
    );
    crate::install::recovery::apply_cleanup(&mut job, &state)
        .await
        .unwrap();
    assert_eq!(
        std::fs::read(root.join(&original.relative_path)).unwrap(),
        b"old bytes"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM resource_execution_journals"
        )
        .fetch_one(&state.pool)
        .await
        .unwrap(),
        0
    );
    state.pool.close().await;
}

#[tokio::test]
async fn stale_review_and_changed_backup_leave_disk_and_database_untouched() {
    let (_temp, state, original, _old, root) = fixture().await;
    std::fs::write(root.join("new.zip"), b"new bytes").unwrap();
    let plan = local_plan(
        &state,
        original.clone(),
        artifact(SourceId::curseforge(), "123", "456", b"new bytes"),
        "new.zip",
        None,
    )
    .await;
    sqlx::query("UPDATE instance_content_sets SET revision = revision + 1 WHERE id = 'set'").execute(&state.pool).await.unwrap();
    assert!(execute("instance", &plan, None, &state).await.is_err());
    assert_eq!(
        std::fs::read(root.join(&original.relative_path)).unwrap(),
        b"old bytes"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM resource_history")
            .fetch_one(&state.pool)
            .await
            .unwrap(),
        0
    );
    let new_plan = local_plan(
        &state,
        original.clone(),
        artifact(SourceId::curseforge(), "123", "456", b"new bytes"),
        "new.zip",
        None,
    )
    .await;
    std::fs::write(root.join("new.zip"), b"changed!").unwrap();
    assert!(execute("instance", &new_plan, None, &state).await.is_err());
    assert_eq!(
        std::fs::read(root.join(&original.relative_path)).unwrap(),
        b"old bytes"
    );
    state.pool.close().await;
}

#[tokio::test]
async fn interrupted_materialization_recovers_exact_backup_and_rejects_external_changes()
 {
    let temp = tempfile::tempdir().unwrap();
    let old = artifact(SourceId::modrinth(), "root0000", "old", b"old bytes");
    let new = artifact(SourceId::curseforge(), "123", "456", b"new bytes");
    std::fs::write(temp.path().join("backup.old"), b"old bytes").unwrap();
    std::fs::write(temp.path().join("active.zip"), b"new bytes").unwrap();
    let journal = vec![JournalFile {
        previous_path: None,
        path: "active.zip".into(),
        backup: Some("backup.old".into()),
        old: Some(old.integrity),
        target: new.integrity,
    }];
    recover_files(temp.path(), &journal).await.unwrap();
    assert_eq!(
        std::fs::read(temp.path().join("active.zip")).unwrap(),
        b"old bytes"
    );
    std::fs::write(temp.path().join("backup.old"), b"old bytes").unwrap();
    std::fs::write(temp.path().join("active.zip"), b"user changes").unwrap();
    assert!(recover_files(temp.path(), &journal).await.is_err());
    assert_eq!(
        std::fs::read(temp.path().join("active.zip")).unwrap(),
        b"user changes"
    );
}

async fn change_target_path(
    state: &crate::State,
    mut plan: ResourceOperationPlan,
    path: &str,
) -> ResourceOperationPlan {
    plan.actions[0].target_relative_path = path.into();
    let payload: String = sqlx::query_scalar(
        "SELECT payload FROM resource_operation_plans WHERE id = ?",
    )
    .bind(plan.id.as_str())
    .fetch_one(&state.pool)
    .await
    .unwrap();
    let mut draft: plans::OperationDraft =
        serde_json::from_str(&payload).unwrap();
    draft.plan = plan.clone();
    sqlx::query("UPDATE resource_operation_plans SET payload = ? WHERE id = ?")
        .bind(serde_json::to_string(&draft).unwrap())
        .bind(plan.id.as_str())
        .execute(&state.pool)
        .await
        .unwrap();
    plan
}

#[tokio::test]
async fn renamed_update_and_rollback_restore_exact_path_source_and_file_id() {
    let (_temp, state, original, old, root) = fixture().await;
    std::fs::write(root.join("new.zip"), b"new bytes").unwrap();
    let target = artifact(SourceId::curseforge(), "123", "456", b"new bytes");
    let plan =
        local_plan(&state, original.clone(), target.clone(), "new.zip", None)
            .await;
    let path =
        super::naming::replacement_path(&original, "456.zip", true).unwrap();
    assert_eq!(path, "resourcepacks/prefix-456.zip.disabled");
    let plan = change_target_path(&state, plan, &path).await;
    let original_file: String = sqlx::query_scalar(
        "SELECT file_id FROM instance_content_entries WHERE id = ?",
    )
    .bind(original.entry_id.as_str())
    .fetch_one(&state.pool)
    .await
    .unwrap();
    execute("instance", &plan, None, &state).await.unwrap();
    assert!(!root.join(&original.relative_path).exists());
    assert_eq!(std::fs::read(root.join(&path)).unwrap(), b"new bytes");
    let changed = plans::installed(&state.pool, "instance")
        .await
        .unwrap()
        .remove(0);
    assert_eq!(changed.relative_path, path);
    assert_eq!(changed.original_file_name.as_deref(), Some("456.zip"));
    let histories =
        history::list_with_state("instance", Some(&original.entry_id), &state)
            .await
            .unwrap();
    let entry = &histories[0];
    assert_eq!(entry.previous.relative_path, original.relative_path);
    assert_eq!(entry.target.relative_path, path);
    let draft = history::prepare_rollback(
        "instance".into(),
        changed.entry_id,
        entry.id.clone(),
        &state,
    )
    .await
    .unwrap();
    let rollback = draft.plan.clone();
    assert_eq!(rollback.actions[0].target.reference, old.reference);
    sqlx::query("INSERT INTO resource_operation_plans(id, instance_id, revision, payload) VALUES (?, 'instance', 1, ?)").bind(rollback.id.as_str()).bind(serde_json::to_string(&draft).unwrap()).execute(&state.pool).await.unwrap();
    execute("instance", &rollback, None, &state).await.unwrap();
    assert!(!root.join(&path).exists());
    assert_eq!(
        std::fs::read(root.join(&original.relative_path)).unwrap(),
        b"old bytes"
    );
    let restored = plans::installed(&state.pool, "instance")
        .await
        .unwrap()
        .remove(0);
    assert_eq!(restored, original);
    let file: String = sqlx::query_scalar(
        "SELECT file_id FROM instance_content_entries WHERE id = ?",
    )
    .bind(original.entry_id.as_str())
    .fetch_one(&state.pool)
    .await
    .unwrap();
    assert_eq!(file, original_file);
    assert!(
        sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&state.pool)
            .await
            .unwrap()
            .is_empty()
    );
    state.pool.close().await;
}

#[tokio::test]
async fn renamed_update_collision_and_sql_failure_preserve_original_bytes_and_identity()
 {
    let (_temp, state, original, _, root) = fixture().await;
    std::fs::write(root.join("new.zip"), b"new bytes").unwrap();
    let plan = local_plan(
        &state,
        original.clone(),
        artifact(SourceId::curseforge(), "123", "456", b"new bytes"),
        "new.zip",
        None,
    )
    .await;
    let destination = "resourcepacks/new.zip.disabled";
    let plan = change_target_path(&state, plan, destination).await;
    std::fs::write(root.join(destination), b"user file").unwrap();
    assert!(execute("instance", &plan, None, &state).await.is_err());
    assert_eq!(std::fs::read(root.join(destination)).unwrap(), b"user file");
    std::fs::remove_file(root.join(destination)).unwrap();
    sqlx::raw_sql("CREATE TRIGGER fail_resource_write BEFORE INSERT ON resource_installed_artifacts BEGIN SELECT RAISE(ABORT, 'test failure'); END;").execute(&state.pool).await.unwrap();
    assert!(execute("instance", &plan, None, &state).await.is_err());
    assert_eq!(
        std::fs::read(root.join(&original.relative_path)).unwrap(),
        b"old bytes"
    );
    assert!(!root.join(destination).exists());
    assert_eq!(
        plans::installed(&state.pool, "instance").await.unwrap()[0],
        original
    );
    state.pool.close().await;
}

#[tokio::test]
async fn backup_cleanup_checks_selection_snapshot_tasks_references_and_rollback_loss()
 {
    let (_temp, state, original, _, root) = fixture().await;
    std::fs::write(root.join("new.zip"), b"new bytes").unwrap();
    let plan = local_plan(
        &state,
        original.clone(),
        artifact(SourceId::curseforge(), "123", "456", b"new bytes"),
        "new.zip",
        None,
    )
    .await;
    execute("instance", &plan, None, &state).await.unwrap();
    std::fs::write(root.join("resourcepacks/unknown.old"), b"unknown").unwrap();
    std::fs::write(root.join("resourcepacks/new.zip_old.zip.old"), b"orphan")
        .unwrap();
    let files = super::history_cleanup::scan_with_state("instance", &state)
        .await
        .unwrap();
    assert_eq!(files.len(), 3);
    let valid = files.iter().find(|file| file.rollback_available).unwrap();
    let orphan = files
        .iter()
        .find(|file| file.history_id.is_none() && file.recognized)
        .unwrap();
    let unknown = files.iter().find(|file| !file.recognized).unwrap();
    assert!(
        super::history_cleanup::prepare_with_state(
            "instance",
            vec![unknown.id.clone()],
            &state
        )
        .await
        .is_err()
    );
    let review = super::history_cleanup::prepare_with_state(
        "instance",
        vec![valid.id.clone(), orphan.id.clone()],
        &state,
    )
    .await
    .unwrap();
    assert!(root.join(&valid.relative_path).is_file());
    sqlx::query("UPDATE instance_content_sets SET revision = revision + 1 WHERE id = 'set'").execute(&state.pool).await.unwrap();
    assert!(
        super::history_cleanup::execute_with_state(
            review.id,
            review.revision,
            &state
        )
        .await
        .is_err()
    );
    let review = super::history_cleanup::prepare_with_state(
        "instance",
        vec![valid.id.clone(), orphan.id.clone()],
        &state,
    )
    .await
    .unwrap();
    std::fs::write(root.join(&orphan.relative_path), b"modified").unwrap();
    assert!(
        super::history_cleanup::execute_with_state(
            review.id,
            review.revision,
            &state
        )
        .await
        .is_err()
    );
    assert!(root.join(&valid.relative_path).exists());
    let review = super::history_cleanup::prepare_with_state(
        "instance",
        vec![valid.id.clone(), orphan.id.clone()],
        &state,
    )
    .await
    .unwrap();
    let job = crate::install::model::InstallJobState::new(
        crate::install::InstallRequest::InstallContentBatch {
            instance_id: "instance".into(),
            items: vec![],
            display_title: "Busy".into(),
            display_icon: None,
        },
    );
    let id = uuid::Uuid::new_v4();
    crate::install::store::insert(
        id,
        &job,
        crate::install::InstallJobStatus::Queued,
        &state,
    )
    .await
    .unwrap();
    assert!(
        super::history_cleanup::execute_with_state(
            review.id.clone(),
            review.revision,
            &state
        )
        .await
        .is_err()
    );
    sqlx::query("DELETE FROM install_jobs WHERE id = ?")
        .bind(id.to_string())
        .execute(&state.pool)
        .await
        .unwrap();
    let result = super::history_cleanup::execute_with_state(
        review.id.clone(),
        review.revision,
        &state,
    )
    .await
    .unwrap();
    assert_eq!(result.removed.len(), 2);
    assert!(result.failures.is_empty());
    assert!(
        super::history_cleanup::execute_with_state(
            review.id,
            review.revision,
            &state
        )
        .await
        .is_err()
    );
    assert!(!root.join(&valid.relative_path).exists());
    assert!(!root.join(&orphan.relative_path).exists());
    assert!(root.join(&unknown.relative_path).exists());
    assert_eq!(
        std::fs::read(root.join(&original.relative_path)).unwrap(),
        b"new bytes"
    );
    assert!(
        !history::list_with_state("instance", Some(&original.entry_id), &state)
            .await
            .unwrap()[0]
            .backup_available
    );
    let retained: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM store_retained_refs WHERE reason = ?",
    )
    .bind(format!("resource-history:{}", valid.id.as_str()))
    .fetch_one(&state.pool)
    .await
    .unwrap();
    assert_eq!(retained, 0);
    state.pool.close().await;
}

#[tokio::test]
async fn naming_preferences_cover_unknown_ambiguous_custom_disabled_names() {
    let (_temp, state, mut current, old, _) = fixture().await;
    for (path, original, preserve, expected) in [
        (
            "resourcepacks/[用途] prefix-old.zip suffix.zip.disabled",
            Some("old.zip"),
            true,
            "resourcepacks/[用途] prefix-new.zip suffix.zip.disabled",
        ),
        (
            "resourcepacks/old.zip-old.zip.disabled",
            Some("old.zip"),
            true,
            "resourcepacks/old.zip-old.zip.disabled",
        ),
        (
            "resourcepacks/custom.zip",
            None,
            true,
            "resourcepacks/custom.zip",
        ),
        (
            "resourcepacks/custom.zip.disabled",
            Some("old.zip"),
            true,
            "resourcepacks/custom.zip.disabled",
        ),
        (
            "resourcepacks/prefix-old.zip.disabled",
            Some("old.zip"),
            false,
            "resourcepacks/new.zip.disabled",
        ),
    ] {
        current.relative_path = path.into();
        current.original_file_name = original.map(str::to_owned);
        assert_eq!(
            super::naming::replacement_path(&current, "new.zip", preserve)
                .unwrap(),
            expected
        );
    }
    let store = crate::state::resources::ResourceStore::new(state.pool.clone());
    store
        .add_alias(&old.reference.release.project, "ae2")
        .await
        .unwrap();
    let service = state.resources().await.unwrap();
    let path = service
        .named_path("resourcepacks/old.zip".into(), &old, &Default::default())
        .await
        .unwrap();
    let title =
        crate::api::content_search::chinese_file_title_for_modrinth_slug("ae2")
            .unwrap();
    assert_eq!(path, format!("resourcepacks/[{title}]old.zip"));
    let prefs = super::naming::NamingPreferences {
        chinese: false,
        preserve: true,
    };
    assert_eq!(
        service
            .named_path("resourcepacks/old.zip".into(), &old, &prefs)
            .await
            .unwrap(),
        "resourcepacks/old.zip"
    );
    state.pool.close().await;
}

async fn pack_fixture(state: &crate::State, entry: &InstalledResource) {
    sqlx::query("UPDATE instance_content_entries SET ownership_kind = 'pack_managed', source_kind = 'imported_modpack' WHERE id = ?").bind(entry.entry_id.as_str()).execute(&state.pool).await.unwrap();
    sqlx::query("INSERT INTO instance_pack_members(id, content_set_id, content_entry_id, member_key, project_type, expected_relative_path, provider, provider_project_id, provider_release_id, created_at, modified_at) VALUES ('member', 'set', ?, 'member', 'resourcepack', ?, 'modrinth', 'root0000', 'old', 1, 1)").bind(entry.entry_id.as_str()).bind(&entry.relative_path).execute(&state.pool).await.unwrap();
    sqlx::raw_sql("INSERT INTO instance_pack_members(id, content_set_id, member_key, project_type, expected_relative_path, materialization_state, created_at, modified_at) VALUES ('missing', 'set', 'missing', 'mod', 'mods/missing.jar', 'missing', 1, 1);").execute(&state.pool).await.unwrap();
}

#[tokio::test]
async fn explicit_unlink_preserves_files_sources_and_repairs_only_recorded_unlinks()
 {
    let (_temp, state, original, _, root) = fixture().await;
    pack_fixture(&state, &original).await;
    crate::state::instances::commands::repair_recorded_pack_unlink(
        "instance",
        &state.pool,
    )
    .await
    .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM instance_pack_members"
        )
        .fetch_one(&state.pool)
        .await
        .unwrap(),
        2
    );
    let patch = crate::state::EditInstance {
        link: Some(crate::state::InstanceLink::Unmanaged),
        ..Default::default()
    };
    crate::state::edit_instance("instance", patch, &state.pool)
        .await
        .unwrap();
    assert!(root.join(&original.relative_path).exists());
    let installed = plans::installed(&state.pool, "instance").await.unwrap();
    assert_eq!(installed[0].artifact, original.artifact);
    assert_eq!(installed[0].ownership, ResourceOwnership::UserAdded);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM instance_pack_members"
        )
        .fetch_one(&state.pool)
        .await
        .unwrap(),
        0
    );
    pack_fixture(&state, &original).await;
    crate::state::instances::commands::repair_recorded_pack_unlink(
        "instance",
        &state.pool,
    )
    .await
    .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM instance_pack_members"
        )
        .fetch_one(&state.pool)
        .await
        .unwrap(),
        0
    );
    std::fs::remove_file(root.join(&original.relative_path)).unwrap();
    crate::state::instances::commands::repair_recorded_pack_unlink(
        "instance",
        &state.pool,
    )
    .await
    .unwrap();
    assert!(
        sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&state.pool)
            .await
            .unwrap()
            .is_empty()
    );
    state.pool.close().await;
}

#[tokio::test]
async fn unlink_task_conflict_and_sql_failure_leave_pack_metadata_intact() {
    let (_temp, state, original, _, _) = fixture().await;
    pack_fixture(&state, &original).await;
    let patch = crate::state::EditInstance {
        link: Some(crate::state::InstanceLink::Unmanaged),
        ..Default::default()
    };
    let job = crate::install::model::InstallJobState::new(
        crate::install::InstallRequest::InstallContentBatch {
            instance_id: "instance".into(),
            items: vec![],
            display_title: "Busy".into(),
            display_icon: None,
        },
    );
    let id = uuid::Uuid::new_v4();
    crate::install::store::insert(
        id,
        &job,
        crate::install::InstallJobStatus::Running,
        &state,
    )
    .await
    .unwrap();
    assert!(
        crate::state::edit_instance("instance", patch.clone(), &state.pool)
            .await
            .is_err()
    );
    sqlx::query("DELETE FROM install_jobs WHERE id = ?")
        .bind(id.to_string())
        .execute(&state.pool)
        .await
        .unwrap();
    sqlx::raw_sql("CREATE TRIGGER fail_unlink BEFORE DELETE ON instance_pack_members BEGIN SELECT RAISE(ABORT, 'test failure'); END;").execute(&state.pool).await.unwrap();
    assert!(
        crate::state::edit_instance("instance", patch, &state.pool)
            .await
            .is_err()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM instance_pack_members"
        )
        .fetch_one(&state.pool)
        .await
        .unwrap(),
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM resource_pack_unlinks"
        )
        .fetch_one(&state.pool)
        .await
        .unwrap(),
        0
    );
    assert!(matches!(
        plans::installed(&state.pool, "instance").await.unwrap()[0].ownership,
        ResourceOwnership::PackManaged { .. }
    ));
    state.pool.close().await;
}

#[tokio::test]
async fn explicit_unlink_repairs_legacy_residue_and_deleted_content_does_not_reappear()
 {
    let (_temp, state, original, _, root) = fixture().await;
    pack_fixture(&state, &original).await;
    let before = crate::state::instances::commands::get_local_content_snapshot(
        "instance", &state,
    )
    .await
    .unwrap();
    assert!(before.warnings.is_empty());
    assert!(before.pack.is_some());
    assert_eq!(before.items.len(), 2);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM resource_pack_unlinks"
        )
        .fetch_one(&state.pool)
        .await
        .unwrap(),
        0
    );

    crate::state::edit_instance(
        "instance",
        crate::state::EditInstance {
            link: Some(crate::state::InstanceLink::Unmanaged),
            ..Default::default()
        },
        &state.pool,
    )
    .await
    .unwrap();
    let after = crate::state::instances::commands::get_local_content_snapshot(
        "instance", &state,
    )
    .await
    .unwrap();
    assert!(after.warnings.is_empty());
    assert!(after.pack.is_none());
    assert_eq!(after.items.len(), 1);
    assert_eq!(
        after.items[0].ownership_kind,
        crate::state::ContentOwnershipKind::UserAdded
    );
    assert!(after.items[0].member_id.is_none());
    assert_eq!(
        std::fs::read(root.join(&original.relative_path)).unwrap(),
        b"old bytes"
    );
    assert_eq!(
        plans::installed(&state.pool, "instance").await.unwrap()[0].artifact,
        original.artifact
    );

    crate::state::instances::commands::remove_project(
        "instance",
        &original.relative_path,
        &state,
    )
    .await
    .unwrap();
    let deleted =
        crate::state::instances::commands::get_local_content_snapshot(
            "instance", &state,
        )
        .await
        .unwrap();
    assert!(deleted.items.is_empty());
    assert!(!root.join(&original.relative_path).exists());
    assert!(
        sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&state.pool)
            .await
            .unwrap()
            .is_empty()
    );
    state.pool.close().await;
}

#[tokio::test]
async fn local_pack_without_unlink_history_stays_managed_without_a_warning() {
    let (_temp, state, original, _, root) = fixture().await;
    pack_fixture(&state, &original).await;
    for link in [
        crate::state::InstanceLink::Unmanaged,
        crate::state::InstanceLink::ImportedModpack {
            project_id: None,
            version_id: None,
            name: Some("Local pack".into()),
            version_number: None,
            filename: None,
        },
    ] {
        let mut tx = state.pool.begin().await.unwrap();
        crate::state::instances::adapters::sqlite::instance_rows::upsert_instance_link("instance", &link, &mut tx).await.unwrap();
        tx.commit().await.unwrap();
        for _ in 0..2 {
            let snapshot =
                crate::state::instances::commands::get_local_content_snapshot(
                    "instance", &state,
                )
                .await
                .unwrap();
            assert!(snapshot.warnings.is_empty());
            assert!(snapshot.pack.is_some());
            assert_eq!(snapshot.items.len(), 2);
            assert!(snapshot.items.iter().all(|item| item.ownership_kind
                == crate::state::ContentOwnershipKind::PackManaged));
            assert_eq!(
                sqlx::query_scalar::<_, i64>(
                    "SELECT COUNT(*) FROM instance_pack_members"
                )
                .fetch_one(&state.pool)
                .await
                .unwrap(),
                2
            );
            assert_eq!(
                sqlx::query_scalar::<_, i64>(
                    "SELECT COUNT(*) FROM resource_pack_unlinks"
                )
                .fetch_one(&state.pool)
                .await
                .unwrap(),
                0
            );
            assert!(root.join(&original.relative_path).exists());
        }
    }
    state.pool.close().await;
}

#[tokio::test]
async fn cleanup_sql_failure_recovers_backups_and_external_roots_are_refused() {
    let (temp, state, original, _, root) = fixture().await;
    std::fs::write(root.join("new.zip"), b"new bytes").unwrap();
    let plan = local_plan(
        &state,
        original.clone(),
        artifact(SourceId::curseforge(), "123", "456", b"new bytes"),
        "new.zip",
        None,
    )
    .await;
    execute("instance", &plan, None, &state).await.unwrap();
    let files = super::history_cleanup::scan_with_state("instance", &state)
        .await
        .unwrap();
    let file = &files[0];
    let review = super::history_cleanup::prepare_with_state(
        "instance",
        vec![file.id.clone()],
        &state,
    )
    .await
    .unwrap();
    sqlx::raw_sql("CREATE TRIGGER fail_cleanup BEFORE UPDATE ON resource_history BEGIN SELECT RAISE(ABORT, 'test failure'); END;").execute(&state.pool).await.unwrap();
    assert!(
        super::history_cleanup::execute_with_state(
            review.id,
            review.revision,
            &state
        )
        .await
        .is_err()
    );
    assert!(
        matches_file(&root.join(&file.relative_path), &file.integrity)
            .await
            .unwrap()
    );
    assert!(
        history::list_with_state("instance", None, &state)
            .await
            .unwrap()[0]
            .backup_available
    );
    let external = temp.path().join("external");
    std::fs::create_dir_all(&external).unwrap();
    std::fs::write(external.join("mod.jar_old.jar.old"), b"external").unwrap();
    sqlx::query(
        "UPDATE instances SET game_dir_override = ? WHERE id = 'instance'",
    )
    .bind(external.to_string_lossy().as_ref())
    .execute(&state.pool)
    .await
    .unwrap();
    assert!(
        super::history_cleanup::scan_with_state("instance", &state)
            .await
            .is_err()
    );
    assert!(external.join("mod.jar_old.jar.old").exists());
    state.pool.close().await;
}

#[tokio::test]
async fn legacy_backup_rollback_clears_stale_release_and_recovers_database_failure()
 {
    let (_temp, state, original, _, root) = fixture().await;
    let backup = "resourcepacks/prefix-old.zip_legacy.zip.old";
    sqlx::query("UPDATE instance_files SET local_mod_data = 'stale metadata', icon_path = 'stale icon' WHERE instance_id = 'instance'").execute(&state.pool).await.unwrap();
    std::fs::write(root.join(backup), b"legacy bytes").unwrap();
    sqlx::raw_sql("CREATE TRIGGER fail_legacy_rollback BEFORE DELETE ON instance_content_provider_refs BEGIN SELECT RAISE(ABORT, 'test failure'); END;").execute(&state.pool).await.unwrap();
    assert!(
        crate::state::instances::commands::rollback_project(
            "instance",
            &original.relative_path,
            &state
        )
        .await
        .is_err()
    );
    assert_eq!(
        std::fs::read(root.join(&original.relative_path)).unwrap(),
        b"old bytes"
    );
    assert_eq!(std::fs::read(root.join(backup)).unwrap(), b"legacy bytes");
    assert_eq!(
        plans::installed(&state.pool, "instance").await.unwrap()[0],
        original
    );
    sqlx::query("DROP TRIGGER fail_legacy_rollback")
        .execute(&state.pool)
        .await
        .unwrap();
    let restored = crate::state::instances::commands::rollback_project(
        "instance",
        &original.relative_path,
        &state,
    )
    .await
    .unwrap();
    assert_eq!(restored, "resourcepacks/legacy.zip.disabled");
    let metadata: (Option<String>, Option<String>) = sqlx::query_as("SELECT local_mod_data, icon_path FROM instance_files WHERE instance_id = 'instance' AND relative_path = 'resourcepacks/legacy.zip.disabled'").fetch_one(&state.pool).await.unwrap();
    assert_eq!(metadata, (None, None));
    assert_eq!(std::fs::read(root.join(restored)).unwrap(), b"legacy bytes");
    let entry = plans::installed(&state.pool, "instance")
        .await
        .unwrap()
        .remove(0);
    assert_eq!(entry.entry_id, original.entry_id);
    assert!(entry.artifact.is_none());
    assert!(entry.known_artifacts.is_empty());
    assert!(
        sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&state.pool)
            .await
            .unwrap()
            .is_empty()
    );
    state.pool.close().await;
}

#[tokio::test]
async fn rollback_after_unlink_does_not_restore_pack_ownership_or_missing_members()
 {
    let (_temp, state, original, old, root) = fixture().await;
    pack_fixture(&state, &original).await;
    let managed = plans::installed(&state.pool, "instance")
        .await
        .unwrap()
        .remove(0);
    std::fs::write(root.join("new.zip"), b"new bytes").unwrap();
    let plan = local_plan(
        &state,
        managed,
        artifact(SourceId::curseforge(), "123", "456", b"new bytes"),
        "new.zip",
        None,
    )
    .await;
    execute("instance", &plan, None, &state).await.unwrap();
    let history = history::list_with_state("instance", None, &state)
        .await
        .unwrap()
        .remove(0);
    crate::state::edit_instance(
        "instance",
        crate::state::EditInstance {
            link: Some(crate::state::InstanceLink::Unmanaged),
            ..Default::default()
        },
        &state.pool,
    )
    .await
    .unwrap();
    let current = plans::installed(&state.pool, "instance")
        .await
        .unwrap()
        .remove(0);
    let rollback = local_plan(
        &state,
        current,
        old,
        history.backup_relative_path.as_ref().unwrap(),
        Some(history.id),
    )
    .await;
    execute("instance", &rollback, None, &state).await.unwrap();
    let entry = plans::installed(&state.pool, "instance")
        .await
        .unwrap()
        .remove(0);
    assert_eq!(entry.artifact, original.artifact);
    assert_eq!(entry.ownership, ResourceOwnership::UserAdded);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM instance_pack_members"
        )
        .fetch_one(&state.pool)
        .await
        .unwrap(),
        0
    );
    let mut tx = state.pool.begin().await.unwrap();
    crate::state::instances::adapters::sqlite::instance_rows::upsert_instance_link("instance", &crate::state::InstanceLink::ImportedModpack { project_id: None, version_id: None, name: Some("Local".into()), version_number: None, filename: None }, &mut tx).await.unwrap();
    tx.commit().await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM resource_pack_unlinks"
        )
        .fetch_one(&state.pool)
        .await
        .unwrap(),
        0
    );
    pack_fixture(&state, &original).await;
    crate::state::instances::commands::repair_recorded_pack_unlink(
        "instance",
        &state.pool,
    )
    .await
    .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM instance_pack_members"
        )
        .fetch_one(&state.pool)
        .await
        .unwrap(),
        2
    );
    state.pool.close().await;
}

#[tokio::test]
async fn cleanup_recovery_restores_uncommitted_moves_and_finishes_committed_deletion()
 {
    let (_temp, state, _, _, root) = fixture().await;
    let path = "resourcepacks/current.zip_previous.zip.old";
    std::fs::write(root.join(path), b"old bytes").unwrap();
    let files = super::history_cleanup::scan_with_state("instance", &state)
        .await
        .unwrap();
    let review = super::history_cleanup::prepare_with_state(
        "instance",
        vec![files[0].id.clone()],
        &state,
    )
    .await
    .unwrap();
    let temporary = "resourcepacks/.cleanup-test.old";
    let mut draft = super::history_cleanup::CleanupDraft {
        plan: review.clone(),
        moves: vec![temporary.into()],
        committed: false,
    };
    sqlx::query(
        "UPDATE resource_history_cleanup_plans SET payload = ? WHERE id = ?",
    )
    .bind(serde_json::to_string(&draft).unwrap())
    .bind(review.id.as_str())
    .execute(&state.pool)
    .await
    .unwrap();
    std::fs::rename(root.join(path), root.join(temporary)).unwrap();
    let recovered = super::history_cleanup::scan_with_state("instance", &state)
        .await
        .unwrap();
    assert_eq!(recovered[0].relative_path, path);
    assert!(root.join(path).exists());
    assert!(!root.join(temporary).exists());
    let review = super::history_cleanup::prepare_with_state(
        "instance",
        vec![files[0].id.clone()],
        &state,
    )
    .await
    .unwrap();
    draft.plan = review.clone();
    draft.committed = true;
    sqlx::query("UPDATE resource_history_cleanup_plans SET payload = ?, consumed = 1 WHERE id = ?").bind(serde_json::to_string(&draft).unwrap()).bind(review.id.as_str()).execute(&state.pool).await.unwrap();
    std::fs::rename(root.join(path), root.join(temporary)).unwrap();
    assert!(
        super::history_cleanup::scan_with_state("instance", &state)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(!root.join(temporary).exists());
    state.pool.close().await;
}

#[tokio::test]
async fn backup_cleanup_refuses_current_files_and_shared_history_references() {
    let (_temp, state, original, _, root) = fixture().await;
    let path = "resourcepacks/current.zip_previous.zip.old";
    std::fs::write(root.join(path), b"old bytes").unwrap();
    sqlx::query("UPDATE instance_files SET relative_path = ? WHERE id = (SELECT file_id FROM instance_content_entries WHERE id = ?)").bind(path).bind(original.entry_id.as_str()).execute(&state.pool).await.unwrap();
    let files = super::history_cleanup::scan_with_state("instance", &state)
        .await
        .unwrap();
    assert!(
        super::history_cleanup::prepare_with_state(
            "instance",
            vec![files[0].id.clone()],
            &state
        )
        .await
        .is_err()
    );
    sqlx::query("UPDATE instance_files SET relative_path = ? WHERE id = (SELECT file_id FROM instance_content_entries WHERE id = ?)").bind(&original.relative_path).bind(original.entry_id.as_str()).execute(&state.pool).await.unwrap();
    let history = history::history_entry(
        "instance",
        &original,
        original.clone(),
        &local_plan(
            &state,
            original.clone(),
            artifact(SourceId::curseforge(), "123", "456", b"new bytes"),
            path,
            None,
        )
        .await,
        Some(path.into()),
    );
    let mut second = history.clone();
    second.id = HistoryId::new(uuid::Uuid::new_v4().to_string()).unwrap();
    for entry in [&history, &second] {
        sqlx::query("INSERT INTO resource_history(id, instance_id, entry_id, recorded_at, payload, backup_relative_path, backup_available) VALUES (?, 'instance', ?, 1, ?, ?, 1)").bind(entry.id.as_str()).bind(original.entry_id.as_str()).bind(serde_json::to_string(entry).unwrap()).bind(path).execute(&state.pool).await.unwrap();
    }
    assert!(
        super::history_cleanup::prepare_with_state(
            "instance",
            vec![history.id.clone()],
            &state
        )
        .await
        .is_err()
    );
    let review = super::history_cleanup::prepare_with_state(
        "instance",
        vec![history.id, second.id],
        &state,
    )
    .await
    .unwrap();
    assert_eq!(review.files.len(), 1);
    assert_eq!(review.entries.len(), 2);
    let result =
        super::history_cleanup::execute_with_state(review.id, 1, &state)
            .await
            .unwrap();
    assert_eq!(result.removed.len(), 2);
    assert!(!root.join(path).exists());
    state.pool.close().await;
}

#[tokio::test]
async fn a_satisfied_filename_dependency_is_rechecked_before_any_file_is_replaced()
 {
    let (_temp, state, original, old, root) = fixture().await;
    std::fs::create_dir_all(root.join("mods")).unwrap();
    let local =
        artifact(SourceId::modrinth(), "local000", "localver", b"local bytes");
    std::fs::write(root.join("mods/local.jar"), b"local bytes").unwrap();
    let file = content_rows::upsert_instance_file_from_parts(
        content_rows::UpsertInstanceFile {
            instance_id: "instance",
            relative_path: "mods/local.jar",
            file_name: "local.jar",
            enabled: true,
            sha1: &local.integrity.hashes[0].value,
            size: 11,
            missing: false,
            local_mod_data: None,
            icon_path: None,
        },
        &state.pool,
    )
    .await
    .unwrap();
    let entry = content_rows::upsert_content_entry_from_parts(
        content_rows::UpsertContentEntry {
            instance_id: "instance",
            content_set_id: "set",
            file_id: Some(&file.id),
            project_type: ProjectType::Mod,
            source_kind: ContentSourceKind::Local,
            ownership_kind: ContentOwnershipKind::UserAdded,
            auto_dependency: false,
            server_requirement: ContentRequirement::Unknown,
            client_requirement: ContentRequirement::Unknown,
            enabled: true,
        },
        &state.pool,
    )
    .await
    .unwrap();
    std::fs::write(root.join("download.zip"), b"old bytes").unwrap();
    let mut plan =
        local_plan(&state, original.clone(), old.clone(), "download.zip", None)
            .await;
    let node_id = SelectionId::new("local-file").unwrap();
    plan.nodes.push(ResourceDependencyNode {
        id: node_id.clone(),
        root_selection_ids: vec![],
        resource_type: Some(ResourceType::Mod),
        resource: ResourceRef::Source {
            project: crate::resources::providers::source_project(
                SourceId::new("local").unwrap(),
                "local.jar",
            )
            .unwrap(),
        },
        scope: InstallScope::Instance,
        state: DependencyNodeState::Satisfied,
        artifact: None,
        installed_entries: vec![InstalledEntryId::new(entry.id).unwrap()],
        compatibility: CompatibilityAssessment {
            game_version: CompatibilityState::Unknown,
            loader: CompatibilityState::Unknown,
            reasons: vec![],
        },
        declared_compatibility: None,
    });
    plan.edges.push(ResourceDependencyEdge {
        parent: plan.actions[0].node_id.clone(),
        child: node_id,
        declared_by: old.reference.clone(),
        kind: ResourceDependencyKind::Required,
        release_selection: None,
        version_constraint: None,
    });
    let payload: String = sqlx::query_scalar(
        "SELECT payload FROM resource_operation_plans WHERE id = ?",
    )
    .bind(plan.id.as_str())
    .fetch_one(&state.pool)
    .await
    .unwrap();
    let mut draft: plans::OperationDraft =
        serde_json::from_str(&payload).unwrap();
    draft.plan = plan.clone();
    sqlx::query("UPDATE resource_operation_plans SET payload = ? WHERE id = ?")
        .bind(serde_json::to_string(&draft).unwrap())
        .bind(plan.id.as_str())
        .execute(&state.pool)
        .await
        .unwrap();
    std::fs::write(root.join("mods/local.jar"), b"other bytes").unwrap();
    let error = execute("instance", &plan, None, &state).await.unwrap_err();
    assert!(
        error.to_string().contains("satisfied dependency changed"),
        "{error}"
    );
    assert_eq!(
        std::fs::read(root.join(&original.relative_path)).unwrap(),
        b"old bytes"
    );
    std::fs::write(root.join("mods/local.jar"), b"local bytes").unwrap();
    execute("instance", &plan, None, &state).await.unwrap();
    assert_eq!(
        std::fs::read(root.join("mods/local.jar")).unwrap(),
        b"local bytes"
    );
    state.pool.close().await;
}

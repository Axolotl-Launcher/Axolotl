use super::*;

#[test]
fn historical_content_recovery_requires_its_saved_release() {
    assert_eq!(
        request::require_historical_release(Some("original".into())).unwrap(),
        "original"
    );
    assert!(request::require_historical_release(None).is_err());
    assert!(request::require_historical_release(Some(String::new())).is_err());
}

fn update_one(content_id: &str) -> crate::install::ContentChangeIntent {
    crate::install::ContentChangeIntent::UpdateOne {
        content_id: content_id.to_string(),
        target_release_id: Some(format!("target-{content_id}")),
    }
}

#[test]
fn content_change_conflicts_only_for_overlapping_scopes() {
    use crate::install::{ContentChangeIntent, ContentChangeTarget};

    let first = update_one("first");
    let second = update_one("second");
    let selected = ContentChangeIntent::UpdateSelected {
        targets: vec![ContentChangeTarget {
            content_id: "first".to_string(),
            target_release_id: "target-first".to_string(),
        }],
    };

    assert!(!content_change_intents_overlap(&first, &second));
    assert!(content_change_intents_overlap(&first, &selected));
    assert!(content_change_intents_overlap(
        &ContentChangeIntent::UpdateAllUserAdded,
        &second
    ));
}

fn physical_test_item(
    content_id: &str,
    relative_path: &str,
    status: crate::state::InstanceUpgradeItemStatus,
    action: InstanceUpgradeAction,
    current_enabled: bool,
) -> crate::state::InstanceUpgradeItem {
    crate::state::InstanceUpgradeItem {
        content_id: content_id.to_string(),
        relative_path: relative_path.to_string(),
        project_type: crate::state::ProjectType::Mod,
        provider: None,
        project_id: None,
        current_release_id: None,
        current_enabled,
        auto_dependency: false,
        status,
        resolution: crate::state::InstanceUpgradeResolution {
            content_id: content_id.to_string(),
            action,
            allow_prerelease: false,
            confirmed_prerelease_dependencies: Vec::new(),
        },
        candidate_release_ids: Vec::new(),
    }
}

fn physical_test_execution(
    items: Vec<crate::state::InstanceUpgradeItem>,
    selections: Vec<crate::state::InstanceUpgradeSelection>,
) -> InstanceUpgradeExecution {
    let environment = crate::state::InstanceUpgradeEnvironment {
        game_version: "1.21.9".to_string(),
        mod_loader: ModLoader::Fabric,
        mod_loader_version: Some("0.18.5".to_string()),
        shader_runtime: crate::state::ShaderRuntime::Iris,
    };
    InstanceUpgradeExecution {
        source_revision: 1,
        source_files: Vec::new(),
        source_environment: environment.clone(),
        target_environment: environment,
        items,
        solution: crate::state::InstanceUpgradeSolution {
            kind: crate::state::InstanceUpgradeSolutionKind::Custom,
            selections,
            dependency_changes: Vec::new(),
            warnings: Vec::new(),
        },
        warnings: Vec::new(),
        source_watch: None,
    }
}

#[test]
fn final_physical_decision_uses_solution_then_item_resolution() {
    let solver_item = physical_test_item(
        "solver",
        "mods/solver.jar",
        crate::state::InstanceUpgradeItemStatus::UpgradeAvailable,
        InstanceUpgradeAction::Disable,
        true,
    );
    let local_disable = physical_test_item(
        "local-disable",
        "mods/local-disable.jar",
        crate::state::InstanceUpgradeItemStatus::Unidentified,
        InstanceUpgradeAction::Disable,
        true,
    );
    let local_keep_disabled = physical_test_item(
        "local-keep",
        "mods/local-keep.jar.disabled",
        crate::state::InstanceUpgradeItemStatus::Unidentified,
        InstanceUpgradeAction::Keep,
        false,
    );
    let execution = physical_test_execution(
        vec![
            solver_item.clone(),
            local_disable.clone(),
            local_keep_disabled.clone(),
        ],
        vec![crate::state::InstanceUpgradeSelection {
            content_id: "solver".to_string(),
            provider: Some(ContentProvider::Modrinth),
            project_id: Some("project".to_string()),
            current_release_id: Some("old".to_string()),
            target_release_id: None,
            action: InstanceUpgradeAction::Keep,
            enabled: true,
        }],
    );

    assert_eq!(
        execution.final_physical_decision(&solver_item),
        (InstanceUpgradeAction::Keep, true)
    );
    assert_eq!(
        execution.final_physical_decision(&local_disable),
        (InstanceUpgradeAction::Disable, false)
    );
    assert_eq!(
        execution.final_physical_decision(&local_keep_disabled),
        (InstanceUpgradeAction::Keep, false)
    );
}

#[test]
fn generated_upgrade_name_uses_exact_resolved_loader_version() {
    assert_eq!(
        default_upgrade_instance_name(
            &crate::state::InstanceUpgradeEnvironment {
                game_version: "1.21.9".to_string(),
                mod_loader: ModLoader::Fabric,
                mod_loader_version: Some("0.18.5".to_string()),
                shader_runtime: crate::state::ShaderRuntime::Iris,
            },
        ),
        "1.21.9-Fabric 0.18.5"
    );
}

#[test]
fn compatibility_warning_details_recover_unique_exact_content_identity() {
    let environment = crate::state::InstanceUpgradeEnvironment {
        game_version: "1.21.9".to_string(),
        mod_loader: ModLoader::Fabric,
        mod_loader_version: Some("0.18.5".to_string()),
        shader_runtime: crate::state::ShaderRuntime::Iris,
    };
    let execution = InstanceUpgradeExecution {
        source_revision: 1,
        source_files: Vec::new(),
        source_environment: environment.clone(),
        target_environment: environment,
        items: vec![crate::state::InstanceUpgradeItem {
            content_id: "content".to_string(),
            relative_path: "resourcepacks/foo.zip".to_string(),
            project_type: crate::state::ProjectType::ResourcePack,
            provider: Some(crate::state::ContentProvider::Modrinth),
            project_id: Some("project".to_string()),
            current_release_id: Some("release".to_string()),
            current_enabled: true,
            auto_dependency: false,
            status:
                crate::state::InstanceUpgradeItemStatus::NoCompatibleRelease,
            resolution: crate::state::InstanceUpgradeResolution {
                content_id: "content".to_string(),
                action: crate::state::InstanceUpgradeAction::Keep,
                allow_prerelease: false,
                confirmed_prerelease_dependencies: Vec::new(),
            },
            candidate_release_ids: Vec::new(),
        }],
        solution: crate::state::InstanceUpgradeSolution {
            kind: crate::state::InstanceUpgradeSolutionKind::Custom,
            selections: Vec::new(),
            dependency_changes: Vec::new(),
            warnings: Vec::new(),
        },
        warnings: vec![crate::state::InstanceUpgradeIssue {
            code: crate::state::InstanceUpgradeIssueCode::KeepIncompatible,
            message: "preserved".to_string(),
            content_id: None,
            provider: Some(crate::state::ContentProvider::Modrinth),
            project_id: Some("project".to_string()),
            conflicting_project_id: None,
            dependency_requirements: Vec::new(),
        }],
        source_watch: None,
    };

    let details = upgrade_compatibility_warning_details(&execution);
    assert_eq!(details.len(), 1);
    assert_eq!(details[0].content_id.as_deref(), Some("content"));
    assert_eq!(
        details[0].relative_path.as_deref(),
        Some("resourcepacks/foo.zip")
    );
}

#[test]
fn local_path_only_preserve_warnings_create_physical_notice_details() {
    let environment = crate::state::InstanceUpgradeEnvironment {
        game_version: "1.21.9".to_string(),
        mod_loader: ModLoader::Fabric,
        mod_loader_version: Some("0.18.5".to_string()),
        shader_runtime: crate::state::ShaderRuntime::Iris,
    };
    let item = |content_id: &str, relative_path: &str| {
        crate::state::InstanceUpgradeItem {
            content_id: content_id.to_string(),
            relative_path: relative_path.to_string(),
            project_type: crate::state::ProjectType::ResourcePack,
            provider: None,
            project_id: None,
            current_release_id: None,
            current_enabled: true,
            auto_dependency: false,
            status:
                crate::state::InstanceUpgradeItemStatus::NoCompatibleRelease,
            resolution: crate::state::InstanceUpgradeResolution {
                content_id: content_id.to_string(),
                action: crate::state::InstanceUpgradeAction::Keep,
                allow_prerelease: false,
                confirmed_prerelease_dependencies: Vec::new(),
            },
            candidate_release_ids: Vec::new(),
        }
    };
    let execution = InstanceUpgradeExecution {
        source_revision: 1,
        source_files: Vec::new(),
        source_environment: environment.clone(),
        target_environment: environment,
        items: vec![
            item("resource-pack", "resourcepacks/foo.zip"),
            item("shader-pack", "shaderpacks/bar.zip"),
        ],
        solution: crate::state::InstanceUpgradeSolution {
            kind: crate::state::InstanceUpgradeSolutionKind::Custom,
            selections: Vec::new(),
            dependency_changes: Vec::new(),
            warnings: Vec::new(),
        },
        warnings: vec![crate::state::InstanceUpgradeIssue {
            code: crate::state::InstanceUpgradeIssueCode::KeepIncompatible,
            message: "local content preserved".to_string(),
            content_id: None,
            provider: None,
            project_id: None,
            conflicting_project_id: None,
            dependency_requirements: Vec::new(),
        }],
        source_watch: None,
    };

    let details = upgrade_compatibility_warning_details(&execution);
    assert_eq!(details.len(), 2);
    assert!(details.iter().any(|detail| {
        detail.relative_path.as_deref() == Some("resourcepacks/foo.zip")
            && detail.provider.is_none()
            && detail.project_id.is_none()
    }));
    assert_eq!(
        details
            .iter()
            .filter(|detail| detail.relative_path.is_some())
            .count(),
        2
    );
}

fn upgrade_job_state() -> InstallJobState {
    let environment = crate::state::InstanceUpgradeEnvironment {
        game_version: "1.21.1".to_string(),
        mod_loader: ModLoader::Fabric,
        mod_loader_version: Some("0.16.0".to_string()),
        shader_runtime: crate::state::ShaderRuntime::Iris,
    };
    InstallJobState::new(InstallRequest::UpgradeUnmanagedInstance {
        instance_id: "instance".to_string(),
        plan_id: "plan".to_string(),
        execution: InstanceUpgradeExecution {
            source_revision: 1,
            source_files: Vec::new(),
            source_environment: environment.clone(),
            target_environment: environment,
            items: Vec::new(),
            solution: crate::state::InstanceUpgradeSolution {
                kind: crate::state::InstanceUpgradeSolutionKind::Custom,
                selections: Vec::new(),
                dependency_changes: Vec::new(),
                warnings: Vec::new(),
            },
            warnings: Vec::new(),
            source_watch: None,
        },
        create_full_backup: false,
        shared_upgrade_mode: SharedUpgradeMode::Direct,
        display_names: InstanceUpgradeDisplayNames::default(),
    })
}

fn components(
    primary: ModLoader,
    adjuncts: &[LoaderComponentKind],
) -> Vec<LoaderComponent> {
    std::iter::once(LoaderComponent::new_primary("", primary, None))
        .chain(adjuncts.iter().map(|kind| LoaderComponent {
            instance_id: String::new(),
            kind: *kind,
            version: None,
            role: LoaderComponentRole::Adjunct,
            provider_metadata: None,
        }))
        .collect()
}

fn curseforge_file(
    id: u32,
    is_available: bool,
    game_versions: &[&str],
) -> crate::api::curseforge::CurseForgeFile {
    crate::api::curseforge::CurseForgeFile {
        id,
        game_id: 432,
        mod_id: OPTIFABRIC_CURSEFORGE_PROJECT_ID,
        is_available,
        display_name: String::new(),
        file_name: String::new(),
        release_type: 1,
        file_status: 4,
        hashes: Vec::new(),
        file_date: String::new(),
        file_length: 0,
        download_count: 0,
        file_size_on_disk: None,
        download_url: None,
        game_versions: game_versions.iter().map(ToString::to_string).collect(),
        sortable_game_versions: Vec::new(),
        dependencies: Vec::new(),
        expose_as_alternative: None,
        parent_project_file_id: None,
        alternate_file_id: None,
        is_server_pack: None,
        server_pack_file_id: None,
        is_early_access_content: None,
        early_access_end_date: None,
        file_fingerprint: 0,
        modules: Vec::new(),
    }
}

#[test]
fn loader_component_preflight_accepts_verified_combinations() {
    for components in [
        components(ModLoader::Vanilla, &[LoaderComponentKind::OptiFine]),
        components(ModLoader::Vanilla, &[LoaderComponentKind::LiteLoader]),
        components(ModLoader::Forge, &[LoaderComponentKind::OptiFine]),
        components(ModLoader::NeoForge, &[LoaderComponentKind::OptiFine]),
        components(ModLoader::Forge, &[LoaderComponentKind::LiteLoader]),
        components(
            ModLoader::Fabric,
            &[
                LoaderComponentKind::OptiFine,
                LoaderComponentKind::OptiFabric,
            ],
        ),
    ] {
        validate_loader_components(&components).unwrap();
    }
}

#[test]
fn loader_component_preflight_rejects_unverified_combinations() {
    for components in [
        components(ModLoader::Quilt, &[LoaderComponentKind::OptiFine]),
        components(ModLoader::Cleanroom, &[LoaderComponentKind::OptiFine]),
        components(ModLoader::LegacyFabric, &[LoaderComponentKind::LiteLoader]),
        components(ModLoader::Fabric, &[LoaderComponentKind::LiteLoader]),
        components(ModLoader::Fabric, &[LoaderComponentKind::OptiFine]),
        components(
            ModLoader::Forge,
            &[
                LoaderComponentKind::OptiFine,
                LoaderComponentKind::OptiFabric,
            ],
        ),
    ] {
        assert!(validate_loader_components(&components).is_err());
    }
}

#[test]
fn optifabric_selection_requires_an_available_exact_game_version() {
    let files = vec![
        curseforge_file(1, true, &["1.19.2"]),
        curseforge_file(2, false, &["1.20.1"]),
        curseforge_file(3, true, &["1.20.1"]),
    ];

    assert_eq!(select_optifabric_file_id(&files, "1.20.1"), Some(3));
    assert_eq!(select_optifabric_file_id(&files, "1.20.2"), None);
}

#[test]
fn stalled_downloads_are_reported_as_network_errors() {
    let error: crate::Error = crate::ErrorKind::NetworkError(
        "no data received for 60 seconds".to_string(),
    )
    .into();

    assert_eq!(
        install_error_code(InstallPhaseId::DownloadingMinecraft, &error),
        "network_error"
    );
}

#[test]
fn export_resolution_errors_keep_network_database_and_parse_categories() {
    use crate::resources::ResourceError;
    for (error, expected) in [
        (
            ResourceError::Network { status: Some(503) },
            "network_error",
        ),
        (
            ResourceError::Database("query failed".into()),
            "database_error",
        ),
        (ResourceError::InvalidResponse("JSON".into()), "parse_error"),
        (
            ResourceError::InvalidRequest("selection".into()),
            "export_error",
        ),
    ] {
        assert_eq!(
            install_error_code(InstallPhaseId::Verifying, &error.into()),
            expected
        );
    }
}

#[test]
fn cache_read_errors_have_repair_context_but_generic_sqlx_does_not() {
    let cache_error: crate::Error = crate::ErrorKind::CacheReadError {
        cache_type: "curseforge_project".to_string(),
        message: "malformed cache row".to_string(),
        sqlite_code: Some("11".to_string()),
    }
    .into();
    let view =
        install_error_view(InstallPhaseId::ResolvingPack, &cache_error, None);
    assert_eq!(view.code, "cache_repair_required");
    let context = view.context.unwrap();
    assert_eq!(context.cache_types, vec!["curseforge_project"]);
    assert_eq!(context.sqlite_code.as_deref(), Some("11"));

    let database_error: crate::Error =
        crate::ErrorKind::Sqlx(sqlx::Error::RowNotFound).into();
    assert_eq!(
        install_error_code(InstallPhaseId::ResolvingPack, &database_error),
        "database_error"
    );
}

#[test]
fn cache_repair_validation_rejects_old_or_unknown_context() {
    let old_context = InstallErrorContext::new("read cache").build();
    let old_error = InstallErrorView::from_message(
        "cache_repair_required",
        InstallPhaseId::ResolvingPack,
        "cache failed",
    );
    assert!(
        validated_cache_repair_types_for(
            InstallJobStatus::Failed,
            Some(&InstallErrorView {
                context: Some(old_context),
                ..old_error.clone()
            }),
        )
        .is_err()
    );

    let mut unknown_context = InstallErrorContext::new("read cache").build();
    unknown_context.cache_types = vec!["install_jobs".to_string()];
    assert!(
        validated_cache_repair_types_for(
            InstallJobStatus::Failed,
            Some(&InstallErrorView {
                context: Some(unknown_context),
                ..old_error
            }),
        )
        .is_err()
    );
}

#[test]
fn cache_repair_validation_accepts_only_whitelisted_terminal_jobs() {
    let mut context = InstallErrorContext::new("read cache").build();
    context.cache_types = vec![
        "curseforge_project".to_string(),
        "curseforge_project".to_string(),
    ];
    let error = InstallErrorView {
        code: "cache_repair_required".to_string(),
        phase: Some(InstallPhaseId::ResolvingPack),
        message: "cache failed".to_string(),
        api: None,
        context: Some(context),
    };
    assert_eq!(
        validated_cache_repair_types_for(
            InstallJobStatus::Interrupted,
            Some(&error),
        )
        .unwrap(),
        vec![crate::state::CacheValueType::CurseForgeProject]
    );
    assert!(
        validated_cache_repair_types_for(
            InstallJobStatus::Running,
            Some(&error),
        )
        .is_err()
    );
}

#[test]
fn missing_required_content_pauses_without_starting_rollback() {
    let mut job_state = InstallJobState::new(InstallRequest::DownloadJava {
        vendor: "test".to_string(),
        version: 21,
    });
    job_state.progress.phase = InstallPhaseId::DownloadingContent;
    job_state.cleanup = InstallCleanup::DeleteNewInstance {
        instance_id: Some("same-instance".to_string()),
    };
    let cleanup = job_state.cleanup.clone();
    let reason = InstallPauseReason::MissingRequiredContent {
        failed_files: 2,
        paths: vec!["mods/a.jar".to_string(), "mods/b.jar".to_string()],
    };

    begin_waiting_for_user(&mut job_state, reason.clone());

    assert_eq!(job_state.progress.phase, InstallPhaseId::DownloadingContent);
    assert_eq!(job_state.pause_reason, Some(reason));
    assert_eq!(job_state.cleanup, cleanup);
    assert!(job_state.error.is_none());
    assert!(job_state.rollback.is_none());
    assert!(job_state.events.iter().any(|event| matches!(
        &event.kind,
        InstallJobEventKind::WaitingForUser { .. }
    )));
    assert!(!job_state.events.iter().any(|event| matches!(
        &event.kind,
        InstallJobEventKind::RollbackStarted { .. }
    )));
}

#[test]
fn curseforge_manual_downloads_create_a_recoverable_pause() {
    let manual_download = crate::api::curseforge::CurseForgeManualDownload {
        project_id: 123,
        file_id: 456,
        file_name: "mods/manual.jar".to_string(),
        ownership_kind:
            crate::state::instances::ContentOwnershipKind::PackManaged,
        operation_kind:
            crate::state::instances::ManualDownloadOperationKind::PackInstall,
        website_url: Some(
            "https://www.curseforge.com/minecraft/mc-mods/example/download/456"
                .to_string(),
        ),
        project_type: "mod".to_string(),
        project_slug: "example".to_string(),
        target_folder: "mods".to_string(),
        hashes: Vec::new(),
        file_length: 12,
        file_fingerprint: 34,
    };
    let result = crate::api::curseforge::CurseForgeModpackInstallResult {
        content: crate::api::curseforge::CurseForgeInstallResult {
            manual_downloads: vec![manual_download],
            ..Default::default()
        },
        ..Default::default()
    };

    assert_eq!(
        curseforge_manual_download_pause(&result, &[]),
        Some(InstallPauseReason::MissingRequiredContent {
            failed_files: 1,
            paths: vec!["mods/manual.jar".to_string()],
        })
    );
    assert_eq!(
        curseforge_manual_download_pause(
            &result,
            &["mods/manual.jar".to_string()],
        ),
        None
    );
}

#[test]
fn recovered_manual_world_download_does_not_run_again() {
    let request = crate::api::curseforge::CurseForgeWorldInstallRequest {
        instance_id: "instance".to_string(),
        project_id: 123,
        file_id: 456,
    };
    let mut job_state =
        InstallJobState::new(InstallRequest::InstallCurseForgeWorld {
            request: request.clone(),
            display_title: "World".to_string(),
            display_icon: None,
        });
    job_state.record_event(InstallJobEventKind::ContentFileSkipped {
        path: "saves/world.zip".to_string(),
        reason: "manual download required".to_string(),
        project_id: Some(request.project_id.to_string()),
        version_id: Some(request.file_id.to_string()),
        manual_url: Some("https://www.curseforge.com/download".to_string()),
    });
    job_state.record_event(InstallJobEventKind::ContentFileRecovered {
        path: "saves/world.zip".to_string(),
        bytes: 42,
    });

    assert!(curseforge_world_was_imported_manually(&job_state, &request));
}

#[test]
fn resume_preserves_instance_cleanup_and_existing_pack_checkpoint() {
    let mut job_state = InstallJobState::new(InstallRequest::DownloadJava {
        vendor: "test".to_string(),
        version: 21,
    });
    job_state.target = InstallTarget::NewInstance {
        instance_id: Some("same-instance".to_string()),
    };
    job_state.cleanup = InstallCleanup::DeleteNewInstance {
        instance_id: Some("same-instance".to_string()),
    };
    job_state.continuation =
        Some(InstallContinuationState::InstallingPackToExistingInstance {
            disabled_project_ids: vec!["project-a".to_string()],
        });
    job_state.pause_reason = Some(InstallPauseReason::MissingRequiredContent {
        failed_files: 1,
        paths: vec!["mods/a.jar".to_string()],
    });
    let target = job_state.target.clone();
    let cleanup = job_state.cleanup.clone();
    let continuation = job_state.continuation.clone();

    prepare_resumed_job(&mut job_state);

    assert_eq!(job_state.target, target);
    assert_eq!(job_state.cleanup, cleanup);
    assert_eq!(job_state.continuation, continuation);
    assert!(job_state.pause_reason.is_none());
    assert!(job_state.events.iter().any(|event| matches!(
        &event.kind,
        InstallJobEventKind::JobQueued { .. }
    )));
}

#[test]
fn resumed_job_can_pause_again_without_rollback() {
    let mut job_state = InstallJobState::new(InstallRequest::DownloadJava {
        vendor: "test".to_string(),
        version: 21,
    });
    job_state.pause_reason = Some(InstallPauseReason::MissingRequiredContent {
        failed_files: 1,
        paths: vec!["mods/first.jar".to_string()],
    });
    prepare_resumed_job(&mut job_state);
    begin_waiting_for_user(
        &mut job_state,
        InstallPauseReason::MissingRequiredContent {
            failed_files: 1,
            paths: vec!["mods/still-missing.jar".to_string()],
        },
    );

    assert!(matches!(
        job_state.pause_reason,
        Some(InstallPauseReason::MissingRequiredContent {
            failed_files: 1,
            ..
        })
    ));
    assert!(!job_state.events.iter().any(|event| matches!(
        &event.kind,
        InstallJobEventKind::RollbackStarted { .. }
    )));
}

#[test]
fn canceling_waiting_jobs_keeps_the_original_cleanup_plan() {
    for cleanup in [
        InstallCleanup::DeleteNewInstance {
            instance_id: Some("new-instance".to_string()),
        },
        InstallCleanup::RestoreExistingInstance {
            instance_id: "existing-instance".to_string(),
        },
    ] {
        let mut job_state =
            InstallJobState::new(InstallRequest::DownloadJava {
                vendor: "test".to_string(),
                version: 21,
            });
        job_state.cleanup = cleanup.clone();
        job_state.pause_reason =
            Some(InstallPauseReason::MissingRequiredContent {
                failed_files: 1,
                paths: vec!["mods/missing.jar".to_string()],
            });

        begin_canceling_job(&mut job_state);

        assert_eq!(job_state.cleanup, cleanup);
        assert!(job_state.pause_reason.is_none());
        assert_eq!(job_state.progress.phase, InstallPhaseId::RollingBack);
        assert!(job_state.events.iter().any(|event| matches!(
            &event.kind,
            InstallJobEventKind::RollbackStarted {
                cleanup: event_cleanup,
            } if event_cleanup == &cleanup
        )));
    }
}

#[test]
fn manifest_and_override_errors_remain_fatal() {
    for (phase, message) in [
        (
            InstallPhaseId::ReadingPackManifest,
            "No pack manifest found in mrpack",
        ),
        (InstallPhaseId::ExtractingOverrides, "Invalid override path"),
    ] {
        let mut job_state =
            InstallJobState::new(InstallRequest::DownloadJava {
                vendor: "test".to_string(),
                version: 21,
            });
        job_state.progress.phase = phase;
        let error: crate::Error =
            crate::ErrorKind::InputError(message.to_string()).into();

        begin_failed_job_rollback(&mut job_state, &error);

        assert!(job_state.pause_reason.is_none());
        assert_eq!(job_state.progress.phase, InstallPhaseId::RollingBack);
        assert!(job_state.events.iter().any(|event| matches!(
            &event.kind,
            InstallJobEventKind::Failed {
                phase: failed_phase,
                ..
            } if *failed_phase == phase
        )));
        assert!(job_state.events.iter().any(|event| matches!(
            &event.kind,
            InstallJobEventKind::RollbackStarted { .. }
        )));
    }
}

/// The launcher state is a process-wide singleton; initialize it once and
/// reuse it so `State::get()` resolves inside these APIs. The state root
/// is intentionally leaked (`.keep()`) because the shared state outlives
/// this function.
async fn global_state() -> std::sync::Arc<State> {
    if !State::initialized() {
        let root = tempfile::tempdir().unwrap().keep();
        let _ = State::init_for_test(root.to_string_lossy().to_string()).await;
    }
    State::get().await.unwrap()
}

#[tokio::test]
async fn direct_link_instances_cannot_be_duplicated() {
    let state = global_state().await;
    let minecraft = tempfile::tempdir().unwrap();
    let version_dir = minecraft.path().join("versions/duplicate-demo");
    std::fs::create_dir_all(&version_dir).unwrap();
    std::fs::write(
        version_dir.join("duplicate-demo.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "id": "duplicate-demo",
            "inheritsFrom": "1.20.1",
            "mainClass": "net.minecraft.client.main.Main"
        }))
        .unwrap(),
    )
    .unwrap();

    let instance = crate::state::create_direct_link_instance(
        crate::state::CreateDirectLinkInstance {
            name: None,
            launcher_type:
                crate::api::pack::import::ImportLauncherType::Generic,
            base_path: minecraft.path().to_path_buf(),
            instance_folder: "versions/duplicate-demo".to_string(),
            instance_path: None,
            game_dir_mode: None,
        },
        &state,
    )
    .await
    .unwrap();

    let error = duplicate_instance(instance.id)
        .await
        .expect_err("directly associated instances must be rejected");

    assert!(
        error.to_string().contains("directly associated"),
        "expected a friendly rejection, got: {error}"
    );
}

#[test]
fn upgrade_failure_preserves_applying_phase_after_successful_rollback() {
    let mut reporter_state = upgrade_job_state();
    reporter_state.set_progress(
        InstallPhaseId::StagingContent,
        None,
        InstallPhaseDetails::Empty,
    );
    let mut execution_state = reporter_state.clone();
    execution_state.set_progress(
        InstallPhaseId::ApplyingContent,
        None,
        InstallPhaseDetails::Empty,
    );
    let failed_phase = latest_failure_phase(&execution_state, &reporter_state);
    let mut terminal_state = reporter_state;
    terminal_state.progress.phase = failed_phase;
    let error: crate::Error = crate::ErrorKind::InputError(
        "Injected unmanaged instance upgrade failure after 1 mutation(s)"
            .to_string(),
    )
    .into();

    begin_failed_job_rollback(&mut terminal_state, &error);
    recovery::finalize_rollback_state(&mut terminal_state, true);
    let status = InstallJobStatus::Failed;

    assert_eq!(status, InstallJobStatus::Failed);
    assert_eq!(terminal_state.progress.phase, InstallPhaseId::Finalizing);
    assert_eq!(
        terminal_state.error.as_ref().and_then(|error| error.phase),
        Some(InstallPhaseId::ApplyingContent)
    );
    assert!(terminal_state.rollback_error.is_none());
}

#[test]
fn interrupted_upgrade_uses_terminal_phase_after_successful_recovery() {
    let mut job_state = upgrade_job_state();
    job_state.progress.phase = InstallPhaseId::RollingBack;
    job_state.error = Some(InstallErrorView::from_message(
        "app_closed",
        InstallPhaseId::ApplyingContent,
        "App closed while install was running",
    ));

    recovery::finalize_rollback_state(&mut job_state, true);
    let status = InstallJobStatus::Interrupted;

    assert_eq!(status, InstallJobStatus::Interrupted);
    assert_eq!(job_state.progress.phase, InstallPhaseId::Finalizing);
    assert!(job_state.rollback_error.is_none());
}

#[test]
fn rollback_failure_keeps_recovery_phase_and_error() {
    let mut job_state = upgrade_job_state();
    job_state.progress.phase = InstallPhaseId::RollingBack;
    job_state.rollback_error = Some(InstallErrorView::from_message(
        "rollback_error",
        InstallPhaseId::RollingBack,
        "rollback failed",
    ));
    job_state.record_event(InstallJobEventKind::RollbackFailed {
        message: "rollback failed".to_string(),
    });

    recovery::finalize_rollback_state(&mut job_state, false);

    assert_eq!(job_state.progress.phase, InstallPhaseId::RollingBack);
    assert_eq!(
        job_state
            .rollback_error
            .as_ref()
            .and_then(|error| error.phase),
        Some(InstallPhaseId::RollingBack)
    );
    assert!(job_state.events.iter().any(|event| matches!(
        event.kind,
        InstallJobEventKind::RollbackFailed { .. }
    )));
    assert!(!job_state.events.iter().any(|event| matches!(
        event.kind,
        InstallJobEventKind::RollbackCompleted
    )));
}

#[test]
fn instance_upgrade_direct_backup_honors_enabled_option() {
    assert!(should_create_upgrade_backup(
        true,
        SharedUpgradeMode::Direct
    ));
}

#[test]
fn instance_upgrade_direct_backup_can_be_disabled() {
    assert!(!should_create_upgrade_backup(
        false,
        SharedUpgradeMode::Direct
    ));
}

#[test]
fn instance_upgrade_copy_never_creates_second_backup() {
    assert!(!should_create_upgrade_backup(
        true,
        SharedUpgradeMode::CopyAndUpgrade
    ));
}

#[cfg(not(feature = "tauri"))]
#[tokio::test]
async fn upgrade_backup_clones_authoritative_content_metadata() {
    crate::event::EventState::init().await.unwrap();
    let root = tempfile::tempdir().unwrap().keep();
    let state = State::init_for_test(root.to_string_lossy().to_string())
        .await
        .unwrap();
    let source = crate::api::instance::create(
        "T13 Source".to_string(),
        "1.21.8".to_string(),
        ModLoader::Fabric,
        Some("0.17.2".to_string()),
        None,
        InstanceLink::Unmanaged,
        None,
        None,
    )
    .await
    .unwrap();
    let source_id = source.instance.id.clone();
    let source_base = state
        .directories
        .instances_dir()
        .join(&source.instance.path);
    crate::util::io::create_dir_all(source_base.join("mods"))
        .await
        .unwrap();
    let content = [
        (
            "mods/sodium.jar",
            b"sodium".as_slice(),
            "AANobbMI",
            "7pwil2dy",
            crate::state::instances::ContentOwnershipKind::UserAdded,
        ),
        (
            "mods/lithium.jar",
            b"lithium".as_slice(),
            "gvQqBUqZ",
            "qxIL7Kb8",
            crate::state::instances::ContentOwnershipKind::PackManaged,
        ),
    ];
    for (relative_path, bytes, project_id, release_id, ownership) in content {
        let path = source_base.join(relative_path);
        crate::util::io::write(&path, bytes).await.unwrap();
        let (_, sha1) =
            crate::util::fetch::sha1_file_async(&path).await.unwrap();
        let provider_ref = ContentProviderRef::Modrinth {
            project_id: crate::state::ModrinthProjectId::new(project_id)
                .unwrap(),
            version_id: Some(
                crate::state::ModrinthVersionId::new(release_id).unwrap(),
            ),
        };
        crate::state::record_project_file_atomic(
            &source_id,
            relative_path,
            &sha1,
            bytes.len() as u64,
            crate::state::ProjectType::Mod,
            crate::state::instances::ContentSourceKind::Local,
            ownership,
            Some(&provider_ref),
            true,
            None,
            &state,
        )
        .await
        .unwrap();
    }
    let source_entries = crate::state::instances::adapters::sqlite::content_rows::get_content_entries(
        &source.applied_content_set.id,
        &state.pool,
    )
    .await
    .unwrap();
    let source_files = crate::state::instances::adapters::sqlite::content_rows::get_instance_files(
        &source_id,
        &state.pool,
    )
    .await
    .unwrap();
    let paths_by_file = source_files
        .iter()
        .map(|file| (file.id.as_str(), file.relative_path.as_str()))
        .collect::<HashMap<_, _>>();
    let entry_by_path = source_entries
        .iter()
        .map(|entry| (paths_by_file[entry.file_id.as_deref().unwrap()], entry))
        .collect::<HashMap<_, _>>();
    let sodium = entry_by_path["mods/sodium.jar"];
    let lithium = entry_by_path["mods/lithium.jar"];
    crate::state::instances::adapters::sqlite::content_rows::upsert_content_provider_ref(
        &sodium.id,
        &ContentProviderRef::CurseForge {
            project_id: crate::state::CurseForgeProjectId::new(394468)
                .unwrap(),
            file_id: Some(
                crate::state::CurseForgeFileId::new(6853381).unwrap(),
            ),
        },
        false,
        &state.pool,
    )
    .await
    .unwrap();
    crate::state::instances::adapters::sqlite::content_rows::set_content_entry_auto_dependency(
        &lithium.id,
        true,
        &state.pool,
    )
    .await
    .unwrap();
    crate::state::instances::commands::toggle_content_entries(
        &source_id,
        std::slice::from_ref(&lithium.id),
        Some(false),
        &state,
    )
    .await
    .unwrap();
    let mut tx = state.pool.begin().await.unwrap();
    let now = chrono::Utc::now();
    crate::state::instances::adapters::sqlite::content_rows::upsert_content_dependency_edge_in_transaction(
        &crate::state::instances::ContentDependencyEdge {
            id: format!("content-dependency:{}", Uuid::new_v4()),
            content_set_id: source.applied_content_set.id.clone(),
            parent_entry_id: sodium.id.clone(),
            child_entry_id: lithium.id.clone(),
            evidence_provider: ContentProvider::Modrinth,
            parent_provider: ContentProvider::Modrinth,
            child_provider: ContentProvider::Modrinth,
            dependency_kind: crate::state::instances::ContentDependencyKind::Required,
            parent_project_id: "AANobbMI".to_string(),
            parent_release_id: "7pwil2dy".to_string(),
            child_project_id: "gvQqBUqZ".to_string(),
            child_release_id: "qxIL7Kb8".to_string(),
            created_at: now,
            modified_at: now,
        },
        &mut tx,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let job_id = Uuid::new_v4();
    let mut job_state = upgrade_job_state();
    job_state.target = InstallTarget::ExistingInstance {
        instance_id: source_id.clone(),
    };
    job_state.cleanup = InstallCleanup::RestoreExistingInstance {
        instance_id: source_id.clone(),
    };
    store::insert(job_id, &job_state, InstallJobStatus::Running, &state)
        .await
        .unwrap();

    let backup_id =
        create_upgrade_backup(job_id, &job_state, &state, &source_id, None)
            .await
            .unwrap();
    let snapshot = crate::state::instances::commands::get_content_snapshot(
        &backup_id, false, &state,
    )
    .await
    .unwrap();
    let sodium_snapshot = snapshot
        .items
        .iter()
        .find(|item| item.provider_project_id.as_deref() == Some("AANobbMI"))
        .unwrap();
    let lithium_snapshot = snapshot
        .items
        .iter()
        .find(|item| item.provider_project_id.as_deref() == Some("gvQqBUqZ"))
        .unwrap();
    assert_eq!(
        sodium_snapshot.provider_release_id.as_deref(),
        Some("7pwil2dy")
    );
    assert_eq!(
        lithium_snapshot.provider_release_id.as_deref(),
        Some("qxIL7Kb8")
    );
    assert_eq!(
        sodium_snapshot.ownership_kind,
        crate::state::instances::ContentOwnershipKind::UserAdded
    );
    assert_eq!(
        lithium_snapshot.ownership_kind,
        crate::state::instances::ContentOwnershipKind::PackManaged
    );
    assert!(
        lithium_snapshot
            .dependency
            .as_ref()
            .unwrap()
            .auto_dependency
    );

    let backup = crate::state::get_instance(&backup_id, &state.pool)
        .await
        .unwrap()
        .unwrap();
    let backup_entries = crate::state::instances::adapters::sqlite::content_rows::get_content_entries(
        &backup.applied_content_set.id,
        &state.pool,
    )
    .await
    .unwrap();
    let backup_files = crate::state::instances::adapters::sqlite::content_rows::get_instance_files(
        &backup_id,
        &state.pool,
    )
    .await
    .unwrap();
    let backup_paths = backup_files
        .iter()
        .map(|file| (file.id.as_str(), file.relative_path.as_str()))
        .collect::<HashMap<_, _>>();
    let backup_by_project = backup_entries
        .iter()
        .map(|entry| {
            let path = backup_paths[entry.file_id.as_deref().unwrap()];
            (path, entry)
        })
        .collect::<HashMap<_, _>>();
    let backup_sodium = backup_by_project["mods/sodium.jar"];
    let backup_lithium = backup_by_project["mods/lithium.jar.disabled"];
    assert!(!backup_lithium.enabled);
    let refs = crate::state::instances::adapters::sqlite::content_rows::get_content_provider_refs_with_origin(
        &backup_sodium.id,
        &state.pool,
    )
    .await
    .unwrap();
    assert_eq!(refs.len(), 2);
    assert!(refs.iter().any(|(provider_ref, origin)| {
        *origin
            && matches!(
                provider_ref,
                ContentProviderRef::Modrinth { project_id, version_id }
                    if project_id.to_string() == "AANobbMI"
                        && version_id.as_ref().map(ToString::to_string).as_deref()
                            == Some("7pwil2dy")
            )
    }));
    assert!(refs.iter().any(|(provider_ref, origin)| {
        !origin
            && matches!(
                provider_ref,
                ContentProviderRef::CurseForge {
                    project_id,
                    file_id,
                } if project_id.get() == 394468
                    && file_id.as_ref().is_some_and(|id| id.get() == 6853381)
            )
    }));
    let edges = crate::state::instances::adapters::sqlite::content_rows::get_content_dependency_edges(
        &backup.applied_content_set.id,
        &state.pool,
    )
    .await
    .unwrap();
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].parent_entry_id, backup_sodium.id);
    assert_eq!(edges[0].child_entry_id, backup_lithium.id);
    assert_eq!(edges[0].parent_release_id, "7pwil2dy");
    assert_eq!(edges[0].child_release_id, "qxIL7Kb8");
}

#[cfg(not(feature = "tauri"))]
#[tokio::test]
async fn upgrade_applies_solver_and_non_solver_physical_actions() {
    crate::event::EventState::init().await.unwrap();
    let state = if State::initialized() {
        State::get().await.unwrap()
    } else {
        let root = tempfile::tempdir().unwrap().keep();
        State::init_for_test(root.to_string_lossy().to_string())
            .await
            .unwrap()
    };
    let instance = crate::api::instance::create(
        format!("Upgrade physical actions {}", Uuid::new_v4()),
        "1.21.8".to_string(),
        ModLoader::Fabric,
        Some("0.17.2".to_string()),
        None,
        InstanceLink::Unmanaged,
        None,
        None,
    )
    .await
    .unwrap();
    let instance_id = instance.instance.id.clone();
    let base = state
        .directories
        .instances_dir()
        .join(&instance.instance.path);
    crate::util::io::create_dir_all(base.join("mods"))
        .await
        .unwrap();

    let recognized = [
        (
            "mods/solver-upgrade.jar",
            b"old-upgrade".as_slice(),
            "AANobbMI",
            "7pwil2dy",
        ),
        (
            "mods/solver-keep.jar",
            b"old-keep".as_slice(),
            "gvQqBUqZ",
            "qxIL7Kb8",
        ),
        (
            "mods/solver-disable.jar",
            b"old-disable".as_slice(),
            "mOgUt4GM",
            "oldRelease",
        ),
    ];
    for (relative_path, bytes, project_id, release_id) in recognized {
        let path = base.join(relative_path);
        crate::util::io::write(&path, bytes).await.unwrap();
        let (_, sha1) =
            crate::util::fetch::sha1_file_async(&path).await.unwrap();
        crate::state::record_project_file_atomic(
            &instance_id,
            relative_path,
            &sha1,
            bytes.len() as u64,
            crate::state::ProjectType::Mod,
            crate::state::instances::ContentSourceKind::Local,
            crate::state::instances::ContentOwnershipKind::UserAdded,
            Some(&ContentProviderRef::Modrinth {
                project_id: crate::state::ModrinthProjectId::new(project_id)
                    .unwrap(),
                version_id: Some(
                    crate::state::ModrinthVersionId::new(release_id).unwrap(),
                ),
            }),
            true,
            None,
            &state,
        )
        .await
        .unwrap();
    }
    for (relative_path, bytes) in [
        ("mods/local-keep.jar.disabled", b"local-keep".as_slice()),
        ("mods/local-disable.jar", b"local-disable".as_slice()),
        (
            "mods/unsupported-disable.jar",
            b"unsupported-disable".as_slice(),
        ),
        (
            "mods/external-disable.jar",
            b"externally-modified".as_slice(),
        ),
    ] {
        crate::util::io::write(&base.join(relative_path), bytes)
            .await
            .unwrap();
    }

    let entries = crate::state::instances::adapters::sqlite::content_rows::get_content_entries(
        &instance.applied_content_set.id,
        &state.pool,
    )
    .await
    .unwrap();
    let files = crate::state::instances::adapters::sqlite::content_rows::get_instance_files(
        &instance_id,
        &state.pool,
    )
    .await
    .unwrap();
    let paths = files
        .iter()
        .map(|file| (file.id.as_str(), file.relative_path.as_str()))
        .collect::<HashMap<_, _>>();
    let content_ids = entries
        .iter()
        .map(|entry| {
            (
                paths[entry.file_id.as_deref().unwrap()].to_string(),
                entry.id.clone(),
            )
        })
        .collect::<HashMap<_, _>>();
    let upgrade_id = content_ids["mods/solver-upgrade.jar"].clone();
    let keep_id = content_ids["mods/solver-keep.jar"].clone();
    let disable_id = content_ids["mods/solver-disable.jar"].clone();

    let items = vec![
        physical_test_item(
            &upgrade_id,
            "mods/solver-upgrade.jar",
            crate::state::InstanceUpgradeItemStatus::UpgradeAvailable,
            InstanceUpgradeAction::Upgrade,
            true,
        ),
        physical_test_item(
            &keep_id,
            "mods/solver-keep.jar",
            crate::state::InstanceUpgradeItemStatus::NoCompatibleRelease,
            InstanceUpgradeAction::Keep,
            true,
        ),
        physical_test_item(
            &disable_id,
            "mods/solver-disable.jar",
            crate::state::InstanceUpgradeItemStatus::NoCompatibleRelease,
            InstanceUpgradeAction::Disable,
            true,
        ),
        physical_test_item(
            "local-keep",
            "mods/local-keep.jar.disabled",
            crate::state::InstanceUpgradeItemStatus::Unidentified,
            InstanceUpgradeAction::Keep,
            false,
        ),
        physical_test_item(
            "local-disable",
            "mods/local-disable.jar",
            crate::state::InstanceUpgradeItemStatus::Unidentified,
            InstanceUpgradeAction::Disable,
            true,
        ),
        physical_test_item(
            "unsupported-disable",
            "mods/unsupported-disable.jar",
            crate::state::InstanceUpgradeItemStatus::UnsupportedContentType,
            InstanceUpgradeAction::Disable,
            true,
        ),
        physical_test_item(
            "external-disable",
            "mods/external-disable.jar",
            crate::state::InstanceUpgradeItemStatus::Unidentified,
            InstanceUpgradeAction::Disable,
            true,
        ),
    ];
    let selections = vec![
        crate::state::InstanceUpgradeSelection {
            content_id: upgrade_id,
            provider: Some(ContentProvider::Modrinth),
            project_id: Some("AANobbMI".to_string()),
            current_release_id: Some("7pwil2dy".to_string()),
            target_release_id: Some("vf7UgZpC".to_string()),
            action: InstanceUpgradeAction::Upgrade,
            enabled: true,
        },
        crate::state::InstanceUpgradeSelection {
            content_id: keep_id,
            provider: Some(ContentProvider::Modrinth),
            project_id: Some("gvQqBUqZ".to_string()),
            current_release_id: Some("qxIL7Kb8".to_string()),
            target_release_id: None,
            action: InstanceUpgradeAction::Keep,
            enabled: true,
        },
        crate::state::InstanceUpgradeSelection {
            content_id: disable_id,
            provider: Some(ContentProvider::Modrinth),
            project_id: Some("mOgUt4GM".to_string()),
            current_release_id: Some("oldRelease".to_string()),
            target_release_id: None,
            action: InstanceUpgradeAction::Disable,
            enabled: false,
        },
    ];
    let execution = physical_test_execution(items, selections);
    let staged_path = state
        .directories
        .caches_dir()
        .join(format!("upgrade-test-{}.jar", Uuid::new_v4()));
    crate::util::io::create_dir_all(
        staged_path.parent().expect("staged path has parent"),
    )
    .await
    .unwrap();
    crate::util::io::write(&staged_path, b"target-upgrade")
        .await
        .unwrap();
    let (_, staged_sha1) = crate::util::fetch::sha1_file_async(&staged_path)
        .await
        .unwrap();
    let staged = vec![StagedUpgradeMutation {
        existing_path: Some("mods/solver-upgrade.jar".to_string()),
        target_path: "mods/solver-upgrade.jar".to_string(),
        ownership: crate::state::instances::ContentOwnershipKind::UserAdded,
        auto_dependency: false,
        enabled: true,
        download: StagedUpgradeDownload::Modrinth(
            crate::state::instances::commands::DownloadedProjectVersion {
                file_name: "solver-upgrade.jar".to_string(),
                path: staged_path,
                sha1: staged_sha1,
                size: b"target-upgrade".len() as u64,
                project_type: crate::state::ProjectType::Mod,
                project_id: "AANobbMI".to_string(),
                version_id: "vf7UgZpC".to_string(),
            },
        ),
    }];

    apply_upgrade_content(
        &instance_id,
        staged,
        &execution,
        &HashSet::from(["mods/external-disable.jar".to_string()]),
        &[],
        &state,
    )
    .await
    .unwrap();

    assert_eq!(
        tokio::fs::read(base.join("mods/solver-upgrade.jar"))
            .await
            .unwrap(),
        b"target-upgrade"
    );
    assert_eq!(
        tokio::fs::read(base.join("mods/solver-keep.jar"))
            .await
            .unwrap(),
        b"old-keep"
    );
    assert!(base.join("mods/solver-disable.jar.disabled").exists());
    assert!(base.join("mods/local-keep.jar.disabled").exists());
    assert!(base.join("mods/local-disable.jar.disabled").exists());
    assert!(base.join("mods/unsupported-disable.jar.disabled").exists());
    assert!(base.join("mods/external-disable.jar").exists());
    assert!(!base.join("mods/external-disable.jar.disabled").exists());
    assert_eq!(
        tokio::fs::read(base.join("mods/external-disable.jar"))
            .await
            .unwrap(),
        b"externally-modified"
    );
}

#[test]
fn instance_upgrade_external_add_is_classified() {
    assert_eq!(
        classify_upgrade_external_change(false, true),
        InstanceUpgradeExternalChangeKind::Added
    );
}

#[test]
fn instance_upgrade_external_remove_is_classified() {
    assert_eq!(
        classify_upgrade_external_change(true, false),
        InstanceUpgradeExternalChangeKind::Removed
    );
}

#[test]
fn instance_upgrade_external_modify_is_classified() {
    assert_eq!(
        classify_upgrade_external_change(true, true),
        InstanceUpgradeExternalChangeKind::Modified
    );
}

#[test]
fn instance_upgrade_full_scan_detects_added_file() {
    let changes = diff_upgrade_source_files(
        &[],
        &[upgrade_source_file("mods/new.jar", "new", true)],
    );
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].kind, InstanceUpgradeExternalChangeKind::Added);
}

#[test]
fn instance_upgrade_full_scan_detects_removed_file() {
    let changes = diff_upgrade_source_files(
        &[upgrade_source_file("mods/old.jar", "old", true)],
        &[],
    );
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].kind, InstanceUpgradeExternalChangeKind::Removed);
}

#[test]
fn instance_upgrade_full_scan_detects_enabled_state_change() {
    let changes = diff_upgrade_source_files(
        &[upgrade_source_file("mods/mod.jar", "same", true)],
        &[upgrade_source_file("mods/mod.jar", "same", false)],
    );
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].kind, InstanceUpgradeExternalChangeKind::Modified);
}

#[test]
fn instance_upgrade_reports_external_edit_after_launcher_mutation() {
    let source = upgrade_source_file("mods/lithium.jar", "old", true);
    let expected = upgrade_source_file("mods/lithium.jar", "target", true);
    let current = upgrade_source_file("mods/lithium.jar", "user", true);
    let changes = final_upgrade_external_changes(
        &[source],
        &[current],
        &HashMap::from([("mods/lithium.jar".to_string(), Some(expected))]),
    );

    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].relative_path, "mods/lithium.jar");
    assert_eq!(changes[0].kind, InstanceUpgradeExternalChangeKind::Modified);
}

#[test]
fn instance_upgrade_reports_external_edit_of_skipped_mutation() {
    let changes = final_upgrade_external_changes(
        &[upgrade_source_file("mods/sodium.jar", "old", true)],
        &[upgrade_source_file("mods/sodium.jar", "user", true)],
        &HashMap::new(),
    );

    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].relative_path, "mods/sodium.jar");
}

#[test]
fn instance_upgrade_reports_external_add_but_not_launcher_write() {
    let changes = final_upgrade_external_changes(
        &[],
        &[
            upgrade_source_file("mods/dependency.jar", "target", true),
            upgrade_source_file("mods/t16-external-added.jar", "user", true),
        ],
        &HashMap::from([(
            "mods/dependency.jar".to_string(),
            Some(upgrade_source_file("mods/dependency.jar", "target", true)),
        )]),
    );

    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].relative_path, "mods/t16-external-added.jar");
    assert_eq!(changes[0].kind, InstanceUpgradeExternalChangeKind::Added);
}

#[test]
fn instance_upgrade_external_changes_coalesce_with_skipped_conflicts() {
    let mut changes = vec![InstanceUpgradeExternalChange {
        relative_path: "mods/sodium.jar".to_string(),
        kind: InstanceUpgradeExternalChangeKind::Modified,
    }];
    merge_upgrade_external_changes(
        &mut changes,
        vec![
            InstanceUpgradeExternalChange {
                relative_path: "mods/sodium.jar".to_string(),
                kind: InstanceUpgradeExternalChangeKind::Modified,
            },
            InstanceUpgradeExternalChange {
                relative_path: "mods/t16-external-added.jar".to_string(),
                kind: InstanceUpgradeExternalChangeKind::Added,
            },
        ],
    );

    assert_eq!(changes.len(), 2);
    assert!(changes.iter().any(|change| {
        change.relative_path == "mods/sodium.jar"
            && change.kind == InstanceUpgradeExternalChangeKind::Modified
    }));
}

#[test]
fn instance_upgrade_external_target_conflict_skips_mutation() {
    let mutation = test_upgrade_mutation(Some("mods/old.jar"), "mods/new.jar");
    assert!(upgrade_mutation_conflicts(
        &mutation,
        &HashSet::from(["mods/new.jar".to_string()])
    ));
}

#[test]
fn instance_upgrade_external_delete_conflict_skips_mutation() {
    let mutation = test_upgrade_mutation(Some("mods/old.jar"), "mods/new.jar");
    assert!(upgrade_mutation_conflicts(
        &mutation,
        &HashSet::from(["mods/old.jar".to_string()])
    ));
}

#[test]
fn instance_upgrade_unrelated_external_change_does_not_skip_mutation() {
    let mutation = test_upgrade_mutation(Some("mods/old.jar"), "mods/new.jar");
    assert!(!upgrade_mutation_conflicts(
        &mutation,
        &HashSet::from(["mods/user.jar".to_string()])
    ));
}

fn test_upgrade_mutation(
    existing_path: Option<&str>,
    target_path: &str,
) -> StagedUpgradeMutation {
    StagedUpgradeMutation {
        existing_path: existing_path.map(ToString::to_string),
        target_path: target_path.to_string(),
        ownership: crate::state::instances::ContentOwnershipKind::UserAdded,
        auto_dependency: false,
        enabled: true,
        download: StagedUpgradeDownload::Modrinth(
            crate::state::instances::commands::DownloadedProjectVersion {
                file_name: "new.jar".to_string(),
                path: PathBuf::from("new.jar"),
                sha1: "sha1".to_string(),
                size: 1,
                project_type: crate::state::ProjectType::Mod,
                project_id: "project".to_string(),
                version_id: "version".to_string(),
            },
        ),
    }
}

#[test]
fn instance_upgrade_staging_populates_persisted_download_summary() {
    let staged = (0..27)
        .map(|index| {
            test_upgrade_mutation(
                Some(&format!("mods/old-{index}.jar")),
                &format!("mods/target-{index}.jar"),
            )
        })
        .collect::<Vec<_>>();
    let mut job = InstallJobState::new(InstallRequest::DownloadJava {
        vendor: "test".to_string(),
        version: 21,
    });
    job.record_event(InstallJobEventKind::ContentDownloadStarted {
        files: staged.len() as u64,
        bytes: Some(staged.len() as u64),
    });
    for mutation in staged {
        job.record_event(InstallJobEventKind::ContentFileCompleted {
            path: mutation.target_path,
            bytes: 1,
        });
    }

    let persisted = serde_json::to_string(&job).unwrap();
    let restored: InstallJobState = serde_json::from_str(&persisted).unwrap();
    let summary = restored.download_summary();
    assert_eq!(summary.files_completed, 27);
    assert_eq!(summary.files_total, Some(27));
    assert_eq!(summary.bytes_downloaded, 27);
    assert_eq!(summary.bytes_total, Some(27));
}

#[tokio::test]
async fn upgrade_staging_scheduler_enters_requests_concurrently() {
    use std::sync::Arc;
    use tokio::sync::Barrier;

    let barrier = Arc::new(Barrier::new(2));
    let mut downloads = (0..2)
        .map(|index| {
            let barrier = barrier.clone();
            async move {
                barrier.wait().await;
                Ok::<_, crate::Error>((index, index))
            }
        })
        .collect::<FuturesUnordered<_>>();

    assert_eq!(
        collect_ordered_upgrade_staging(&mut downloads)
            .await
            .unwrap(),
        vec![0, 1]
    );
}

#[tokio::test]
async fn upgrade_staging_scheduler_restores_request_order() {
    let mut downloads = [2_usize, 0, 1]
        .into_iter()
        .map(|index| async move {
            Ok::<_, crate::Error>((index, format!("mutation-{index}")))
        })
        .collect::<FuturesUnordered<_>>();

    assert_eq!(
        collect_ordered_upgrade_staging(&mut downloads)
            .await
            .unwrap(),
        vec!["mutation-0", "mutation-1", "mutation-2"]
    );
}

#[tokio::test]
async fn upgrade_staging_scheduler_returns_first_error() {
    let mut downloads = [
        Ok((0, "first")),
        Err(crate::ErrorKind::InputError("failed".into()).into()),
    ]
    .into_iter()
    .map(std::future::ready)
    .collect::<FuturesUnordered<_>>();

    assert!(
        collect_ordered_upgrade_staging(&mut downloads)
            .await
            .is_err()
    );
}

#[cfg(debug_assertions)]
#[test]
fn instance_upgrade_debug_hook_ignores_invalid_and_zero_counts() {
    assert_eq!(debug_mutation_count(""), None);
    assert_eq!(debug_mutation_count("invalid"), None);
    assert_eq!(debug_mutation_count("0"), None);
    assert_eq!(debug_mutation_count(" 2 "), Some(2));
}

fn upgrade_source_file(
    relative_path: &str,
    sha1: &str,
    enabled: bool,
) -> crate::state::InstanceUpgradeSourceFile {
    crate::state::InstanceUpgradeSourceFile {
        relative_path: relative_path.to_string(),
        sha1: sha1.to_string(),
        size: 1,
        enabled,
    }
}

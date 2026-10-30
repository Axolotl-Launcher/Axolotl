//! What `install::store` promises on top of the install jobs collection: the
//! completion paths that also touch instances, and the one resume that two
//! callers race for.

use super::support;
use crate::install::model::{
    InstallJobState, InstallJobStatus, InstallPauseReason, InstallRequest,
    InstallTarget, SharedUpgradeMode,
};
use crate::install::store::{
    complete_running_job, get_required, insert, update_status_if,
};
use crate::state::{InstanceInstallStage, State};
use uuid::Uuid;

#[cfg(not(feature = "tauri"))]
async fn create_completion_test_instance(
    label: &str,
) -> (std::sync::Arc<State>, String) {
    crate::event::EventState::init().await.unwrap();
    let root = tempfile::tempdir().unwrap().keep();
    let state = State::init_for_test(root.to_string_lossy().to_string())
        .await
        .unwrap();
    let created = crate::api::instance::create(
        format!("Install completion {label} {}", Uuid::new_v4()),
        "1.20.1".to_string(),
        crate::state::ModLoader::Vanilla,
        None,
        None,
        crate::state::InstanceLink::Unmanaged,
        None,
        None,
    )
    .await
    .unwrap();
    crate::state::instances::commands::set_instance_install_stage(
        &created.instance.id,
        InstanceInstallStage::PackInstalling,
        &state.pool,
    )
    .await
    .unwrap();
    (state, created.instance.id)
}

#[cfg(not(feature = "tauri"))]
fn curseforge_content_request(instance_id: &str) -> InstallRequest {
    InstallRequest::InstallCurseForgeContent {
        request: crate::api::curseforge::CurseForgeInstallRequest {
            instance_id: instance_id.to_string(),
            project_id: 348_025,
            file_id: 4_436_467,
            project_type: "mod".to_string(),
            ownership_kind: Default::default(),
            manual_operation_kind: Default::default(),
            game_version: None,
            mod_loader_type: None,
            world_name: None,
            install_dependencies: false,
            excluded_dependency_project_ids: Vec::new(),
            force_dependency_project_ids: Vec::new(),
            dependency_plan_id: None,
            defer_persistence: false,
            verification_tx: None,
            pre_resolved_relative_path: None,
            expected_file_name: None,
        },
        display_title: "CurseForge content".to_string(),
        display_icon: None,
    }
}

#[cfg(not(feature = "tauri"))]
async fn instance_install_stage(
    instance_id: &str,
    state: &State,
) -> InstanceInstallStage {
    crate::state::get_instance(instance_id, &state.pool)
        .await
        .unwrap()
        .unwrap()
        .instance
        .install_stage
}

#[cfg(not(feature = "tauri"))]
async fn insert_running_job(
    state: &InstallJobState,
    app_state: &State,
) -> crate::install::store::InstallJobRecord {
    insert(Uuid::new_v4(), state, InstallJobStatus::Running, app_state)
        .await
        .unwrap()
}

#[cfg(not(feature = "tauri"))]
fn upgrade_completion_state(
    source_instance_id: &str,
    target_instance_id: &str,
    mode: SharedUpgradeMode,
) -> InstallJobState {
    use crate::install::model::{
        InstanceUpgradeCompatibilityWarning, InstanceUpgradeDisplayNames,
        InstanceUpgradeExecution, InstanceUpgradeResult,
    };
    use crate::state::{
        ContentProvider, InstanceUpgradeEnvironment, InstanceUpgradeIssueCode,
        InstanceUpgradeSolution, InstanceUpgradeSolutionKind, ModLoader,
        ShaderRuntime,
    };

    let source_environment = InstanceUpgradeEnvironment {
        game_version: "1.21.8".to_string(),
        mod_loader: ModLoader::Fabric,
        mod_loader_version: Some("0.18.4".to_string()),
        shader_runtime: ShaderRuntime::Iris,
    };
    let target_environment = InstanceUpgradeEnvironment {
        game_version: "1.21.9".to_string(),
        mod_loader: ModLoader::Fabric,
        mod_loader_version: Some("0.18.5".to_string()),
        shader_runtime: ShaderRuntime::Iris,
    };
    let solution = InstanceUpgradeSolution {
        kind: InstanceUpgradeSolutionKind::Custom,
        selections: Vec::new(),
        dependency_changes: Vec::new(),
        warnings: Vec::new(),
    };
    let execution = InstanceUpgradeExecution {
        source_revision: 1,
        source_files: Vec::new(),
        source_environment: source_environment.clone(),
        target_environment: target_environment.clone(),
        items: Vec::new(),
        solution: solution.clone(),
        warnings: Vec::new(),
        source_watch: None,
    };
    let mut state =
        InstallJobState::new(InstallRequest::UpgradeUnmanagedInstance {
            instance_id: source_instance_id.to_string(),
            plan_id: "plan".to_string(),
            execution,
            create_full_backup: false,
            shared_upgrade_mode: mode,
            display_names: InstanceUpgradeDisplayNames::default(),
        });
    state.target = match mode {
        SharedUpgradeMode::Direct => InstallTarget::ExistingInstance {
            instance_id: target_instance_id.to_string(),
        },
        SharedUpgradeMode::CopyAndUpgrade => InstallTarget::NewInstance {
            instance_id: Some(target_instance_id.to_string()),
        },
    };
    state.upgrade_result = Some(InstanceUpgradeResult {
        plan_id: "plan".to_string(),
        source_instance_id: source_instance_id.to_string(),
        target_instance_id: target_instance_id.to_string(),
        backup_instance_id: None,
        source_environment: Some(source_environment),
        target_environment: Some(target_environment),
        solution,
        compatibility_warnings: Vec::new(),
        compatibility_warning_details: (0..30)
            .map(|index| InstanceUpgradeCompatibilityWarning {
                code: InstanceUpgradeIssueCode::KeepIncompatible,
                relative_path: Some(if index == 0 {
                    "resourcepacks/foo.zip".to_string()
                } else {
                    format!("mods/preserved-{index}.jar")
                }),
                content_id: Some(format!("content-{index}")),
                provider: Some(ContentProvider::Modrinth),
                project_id: Some(format!("project-{index}")),
                conflicting_project_id: None,
            })
            .collect(),
        external_changes: Vec::new(),
        skipped_due_to_external_conflict: Vec::new(),
    });
    state
}

#[cfg(not(feature = "tauri"))]
#[tokio::test]
async fn curseforge_content_completion_preserves_incomplete_instance_stage() {
    let (state, instance_id) =
        create_completion_test_instance("CurseForge content").await;
    let job_state =
        InstallJobState::new(curseforge_content_request(&instance_id));
    let job = insert_running_job(&job_state, &state).await;

    let completed = complete_running_job(job.id, &job_state, &state)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(completed.status, InstallJobStatus::Succeeded);
    assert_eq!(
        instance_install_stage(&instance_id, &state).await,
        InstanceInstallStage::PackInstalling
    );
}

#[cfg(not(feature = "tauri"))]
#[tokio::test]
async fn modrinth_content_completion_preserves_incomplete_instance_stage() {
    let (state, instance_id) =
        create_completion_test_instance("Modrinth content").await;
    let job_state = InstallJobState::new(InstallRequest::InstallContent {
        instance_id: instance_id.clone(),
        project_id: "project".to_string(),
        version_id: Some("version".to_string()),
        content_type: content_management::ContentType::Mod,
        selected: Default::default(),
        excluded_project_ids: Vec::new(),
        display_title: "Modrinth content".to_string(),
        display_icon: None,
    });
    let job = insert_running_job(&job_state, &state).await;

    let completed = complete_running_job(job.id, &job_state, &state)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(completed.status, InstallJobStatus::Succeeded);
    assert_eq!(
        instance_install_stage(&instance_id, &state).await,
        InstanceInstallStage::PackInstalling
    );
}

#[cfg(not(feature = "tauri"))]
#[tokio::test]
async fn lifecycle_completion_marks_instance_installed() {
    let (state, instance_id) =
        create_completion_test_instance("lifecycle owner").await;
    let job_state =
        InstallJobState::new(InstallRequest::InstallExistingInstance {
            instance_id: instance_id.clone(),
            force: false,
        });
    let job = insert_running_job(&job_state, &state).await;

    let completed = complete_running_job(job.id, &job_state, &state)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(completed.status, InstallJobStatus::Succeeded);
    assert_eq!(
        instance_install_stage(&instance_id, &state).await,
        InstanceInstallStage::Installed
    );
}

#[cfg(not(feature = "tauri"))]
#[tokio::test]
async fn direct_upgrade_completion_atomically_persists_target_notice() {
    let (state, instance_id) =
        create_completion_test_instance("direct upgrade notice").await;
    let job_state = upgrade_completion_state(
        &instance_id,
        &instance_id,
        SharedUpgradeMode::Direct,
    );
    let job = insert_running_job(&job_state, &state).await;

    complete_running_job(job.id, &job_state, &state)
        .await
        .unwrap()
        .unwrap();

    let notice =
        crate::state::instances::commands::get_instance_post_upgrade_notice(
            &instance_id,
            &state.pool,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(notice.instance_id, instance_id);
    assert_eq!(notice.upgrade_job_id, job.id.to_string());
    assert_eq!(notice.target_game_version, "1.21.9");
    assert_eq!(notice.consecutive_clean_launches, 0);
    assert_eq!(notice.warnings.len(), 30);
    assert_eq!(
        notice.warnings[0].relative_path.as_deref(),
        Some("resourcepacks/foo.zip")
    );
}

#[cfg(not(feature = "tauri"))]
#[tokio::test]
async fn copy_upgrade_completion_persists_notice_only_for_target() {
    let (state, source_id) =
        create_completion_test_instance("copy upgrade source").await;
    let target = crate::api::instance::create(
        format!("Copy upgrade target {}", Uuid::new_v4()),
        "1.21.8".to_string(),
        crate::state::ModLoader::Fabric,
        Some("0.18.4".to_string()),
        None,
        crate::state::InstanceLink::Unmanaged,
        None,
        None,
    )
    .await
    .unwrap();
    let job_state = upgrade_completion_state(
        &source_id,
        &target.instance.id,
        SharedUpgradeMode::CopyAndUpgrade,
    );
    let job = insert_running_job(&job_state, &state).await;

    complete_running_job(job.id, &job_state, &state)
        .await
        .unwrap()
        .unwrap();

    assert!(
        crate::state::instances::commands::get_instance_post_upgrade_notice(
            &source_id,
            &state.pool,
        )
        .await
        .unwrap()
        .is_none()
    );
    let target_notice =
        crate::state::instances::commands::get_instance_post_upgrade_notice(
            &target.instance.id,
            &state.pool,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(target_notice.instance_id, target.instance.id);
    assert_eq!(target_notice.upgrade_job_id, job.id.to_string());
    assert_eq!(target_notice.target_game_version, "1.21.9");
    assert_eq!(target_notice.warnings.len(), 30);
}

#[cfg(not(feature = "tauri"))]
#[tokio::test]
async fn waiting_lifecycle_job_survives_content_job_completion() {
    let (state, instance_id) =
        create_completion_test_instance("waiting lifecycle").await;
    let mut waiting_state =
        InstallJobState::new(InstallRequest::InstallPackToExistingInstance {
            instance_id: instance_id.clone(),
            location:
                crate::api::pack::install_from::CreatePackLocation::FromFile {
                    path: std::path::PathBuf::from("unused.mrpack"),
                },
            post_install_edit: None,
        });
    waiting_state.pause_reason =
        Some(InstallPauseReason::MissingRequiredContent {
            failed_files: 1,
            paths: vec!["mods/manual.jar".to_string()],
        });
    let waiting_job = insert(
        Uuid::new_v4(),
        &waiting_state,
        InstallJobStatus::WaitingForUser,
        &state,
    )
    .await
    .unwrap();
    let content_state =
        InstallJobState::new(curseforge_content_request(&instance_id));
    let content_job = insert_running_job(&content_state, &state).await;

    let completed =
        complete_running_job(content_job.id, &content_state, &state)
            .await
            .unwrap()
            .unwrap();
    let waiting = get_required(waiting_job.id, &state).await.unwrap();

    assert_eq!(waiting.status, InstallJobStatus::WaitingForUser);
    assert!(waiting.state.pause_reason.is_some());
    assert_eq!(completed.status, InstallJobStatus::Succeeded);
    assert_eq!(
        instance_install_stage(&instance_id, &state).await,
        InstanceInstallStage::PackInstalling
    );
}

#[tokio::test]
async fn only_one_waiting_job_resume_can_claim_the_status() {
    let pool = support::pool().await;
    let temp = tempfile::tempdir().unwrap();
    let directories = crate::state::DirectoryInfo {
        settings_dir: temp.path().to_owned(),
        config_dir: temp.path().to_owned(),
        app_identifier: "install-store-test".into(),
    };
    let state = crate::state::test_state(directories, pool).await.unwrap();

    let job_id = Uuid::new_v4();
    let now = chrono::Utc::now().timestamp();
    sqlx::query(
        "INSERT INTO install_jobs
         (id, kind, status, state, created, modified, dismissed)
         VALUES (?, 'pack', 'waiting_for_user', '{}', ?, ?, 0)",
    )
    .bind(job_id.to_string())
    .bind(now)
    .bind(now)
    .execute(&state.pool)
    .await
    .unwrap();

    let resumed =
        InstallJobState::new(InstallRequest::InstallExistingInstance {
            instance_id: "instance".to_string(),
            force: false,
        });
    let first = update_status_if(
        job_id,
        InstallJobStatus::WaitingForUser,
        InstallJobStatus::Queued,
        &resumed,
        &state,
    );
    let second = update_status_if(
        job_id,
        InstallJobStatus::WaitingForUser,
        InstallJobStatus::Queued,
        &resumed,
        &state,
    );
    let (first, second) = tokio::join!(first, second);

    assert_ne!(first.unwrap().is_some(), second.unwrap().is_some());
    let status: String =
        sqlx::query_scalar("SELECT status FROM install_jobs WHERE id = ?")
            .bind(job_id.to_string())
            .fetch_one(&state.pool)
            .await
            .unwrap();
    assert_eq!(status, "queued");
}

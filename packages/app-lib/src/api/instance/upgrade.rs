use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

use tokio::sync::Mutex;

use crate::state::instances::adapters::sqlite::content_rows;
use crate::state::{
    ContentSourceKind, InstanceLink, InstanceUpgradeAction,
    InstanceUpgradeFixedConstraint, InstanceUpgradePlan,
    InstanceUpgradeResolution, InstanceUpgradeResolutionBatchResult,
    InstanceUpgradeResolutionResult, InstanceUpgradeSolutionChoice,
    InstanceUpgradeSolutionKind, State,
};

use crate::install::{
    InstallJobSnapshot, InstanceUpgradeExecution, InstanceUpgradeWatchBaseline,
    SharedUpgradeMode,
};

struct StoredUpgradePlanState {
    plan: InstanceUpgradePlan,
    validation: crate::state::instances::commands::UpgradePlanRuntimeValidation,
    execution_started: bool,
}

type StoredUpgradePlan = Arc<Mutex<StoredUpgradePlanState>>;

static INSTANCE_UPGRADE_PLANS: LazyLock<
    super::upgrade_plan_registry::PlanRegistry<Mutex<StoredUpgradePlanState>>,
> = LazyLock::new(|| {
    super::upgrade_plan_registry::PlanRegistry::new(
        16,
        std::time::Duration::from_secs(60 * 60),
    )
});
static PLAN_CLEANUP_STARTED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

pub fn discard_instance_upgrade_plan(plan_id: &str) {
    INSTANCE_UPGRADE_PLANS.remove(plan_id);
}

fn start_plan_cleanup() {
    if !PLAN_CLEANUP_STARTED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        tokio::spawn(async {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                INSTANCE_UPGRADE_PLANS.prune();
            }
        });
    }
}

#[tracing::instrument]
pub async fn plan_instance_upgrade(
    instance_id: &str,
    target_environment: crate::state::InstanceUpgradeEnvironment,
) -> crate::Result<InstanceUpgradePlan> {
    let state = State::get().await?;
    let creation_watch =
        state.file_watcher.content_watch_snapshot(instance_id).await;
    let (plan, source) = crate::state::instances::commands::create_instance_upgrade_plan_with_source(
        instance_id,
        target_environment,
        &state,
    )
    .await?;
    let mut validation =
        crate::state::instances::commands::UpgradePlanRuntimeValidation::new(
            source,
            instance_id,
            creation_watch,
            &state,
        )
        .await;
    validation.validate(&plan, &state).await?;
    start_plan_cleanup();
    INSTANCE_UPGRADE_PLANS.insert(
        plan.id.clone(),
        Arc::new(Mutex::new(StoredUpgradePlanState {
            plan: plan.clone(),
            validation,
            execution_started: false,
        })),
    )?;
    Ok(plan)
}

#[tracing::instrument]
pub async fn get_instance_upgrade_plan(
    plan_id: &str,
) -> crate::Result<InstanceUpgradePlan> {
    let state = State::get().await?;
    let handle = stored_plan_handle(plan_id)?;
    let mut stored = handle.lock().await;
    if let Err(error) = ensure_current_revision(&mut stored, &state).await {
        drop(stored);
        INSTANCE_UPGRADE_PLANS.remove(plan_id);
        return Err(error);
    }
    Ok(stored.plan.clone())
}

#[tracing::instrument]
pub async fn update_instance_upgrade_resolution(
    plan_id: &str,
    resolution: InstanceUpgradeResolution,
) -> crate::Result<InstanceUpgradePlan> {
    let started_at = std::time::Instant::now();
    let state = State::get().await?;
    let handle = stored_plan_handle(plan_id)?;
    let mut stored = handle.lock().await;
    let source = ensure_current_revision(&mut stored, &state).await?;
    tracing::debug!(
        elapsed_ms = started_at.elapsed().as_millis(),
        "[upgrade-plan-bench] update-resolution source validation"
    );
    let mut plan = stored.plan.clone();
    let item = plan
        .items
        .iter_mut()
        .find(|item| item.content_id == resolution.content_id)
        .ok_or_else(|| {
            crate::ErrorKind::InputError(format!(
                "Upgrade plan has no content item {}",
                resolution.content_id
            ))
        })?;
    item.resolution = resolution;
    let (kind, constraints) = selected_kind_and_constraints(&plan);
    crate::state::instances::commands::recompute_instance_upgrade_plan_from_source(
        &mut plan,
        &constraints,
        kind,
        source,
        &state,
    )
    .await?;
    tracing::debug!(
        elapsed_ms = started_at.elapsed().as_millis(),
        "[upgrade-plan-bench] update-resolution complete"
    );
    stored.plan = plan.clone();
    Ok(plan)
}

#[tracing::instrument(skip(resolutions))]
pub async fn update_instance_upgrade_resolutions(
    plan_id: &str,
    resolutions: Vec<InstanceUpgradeResolution>,
) -> crate::Result<InstanceUpgradeResolutionBatchResult> {
    let state = State::get().await?;
    let handle = stored_plan_handle(plan_id)?;
    let mut stored = handle.lock().await;
    let source = ensure_current_revision(&mut stored, &state).await?;
    let requested_count = resolutions.len();
    let (requests, mut skipped) = normalize_batch_resolutions(resolutions);
    let mut working_plan = stored.plan.clone();
    let mut applied = Vec::new();
    let mut failed = Vec::new();
    let mut pending = vec![requests];

    while let Some(chunk) = pending.pop() {
        let mut applicable = Vec::new();
        for resolution in chunk {
            if !resolution_is_applicable(&working_plan, &resolution) {
                skipped.push(InstanceUpgradeResolutionResult {
                    content_id: resolution.content_id,
                    code: Some("no_longer_applicable".to_string()),
                    message: Some(
                        "Resolution is no longer applicable".to_string(),
                    ),
                });
            } else {
                applicable.push(resolution);
            }
        }
        if applicable.is_empty() {
            continue;
        }

        let mut trial = working_plan.clone();
        apply_resolutions(&mut trial, &applicable)?;
        let (kind, constraints) = selected_kind_and_constraints(&trial);
        match crate::state::instances::commands::recompute_instance_upgrade_plan_from_source(
            &mut trial,
            &constraints,
            kind,
            source.clone(),
            &state,
        )
        .await
        {
            Ok(()) => {
                applied.extend(applicable.iter().map(|resolution| {
                    InstanceUpgradeResolutionResult {
                        content_id: resolution.content_id.clone(),
                        code: None,
                        message: None,
                    }
                }));
                working_plan = trial;
            }
            Err(error) if applicable.len() == 1 => {
                failed.push(InstanceUpgradeResolutionResult {
                    content_id: applicable[0].content_id.clone(),
                    code: Some("resolution_failed".to_string()),
                    message: Some(error.to_string()),
                });
            }
            Err(_) => {
                let midpoint = applicable.len() / 2;
                let right = applicable[midpoint..].to_vec();
                let left = applicable[..midpoint].to_vec();
                pending.push(right);
                pending.push(left);
            }
        }
    }

    if !applied.is_empty() {
        stored.plan = working_plan.clone();
    }
    Ok(InstanceUpgradeResolutionBatchResult {
        plan: working_plan,
        requested_count,
        applied,
        skipped,
        failed,
    })
}

#[tracing::instrument]
pub async fn reset_instance_upgrade_resolution(
    plan_id: &str,
    content_id: &str,
) -> crate::Result<InstanceUpgradePlan> {
    let state = State::get().await?;
    let handle = stored_plan_handle(plan_id)?;
    let mut stored = handle.lock().await;
    let source = ensure_current_revision(&mut stored, &state).await?;
    let mut plan = stored.plan.clone();
    let item = plan
        .items
        .iter_mut()
        .find(|item| item.content_id == content_id)
        .ok_or_else(|| {
            crate::ErrorKind::InputError(format!(
                "Upgrade plan has no content item {content_id}"
            ))
        })?;
    item.resolution = automatic_resolution(item);
    let (kind, constraints) = selected_kind_and_constraints(&plan);
    crate::state::instances::commands::recompute_instance_upgrade_plan_from_source(
        &mut plan,
        &constraints,
        kind,
        source,
        &state,
    )
    .await?;
    stored.plan = plan.clone();
    Ok(plan)
}

fn normalize_batch_resolutions(
    resolutions: Vec<InstanceUpgradeResolution>,
) -> (
    Vec<InstanceUpgradeResolution>,
    Vec<InstanceUpgradeResolutionResult>,
) {
    let mut by_content_id = HashMap::new();
    let mut skipped = Vec::new();
    for resolution in resolutions {
        if by_content_id
            .insert(resolution.content_id.clone(), resolution.clone())
            .is_some()
        {
            skipped.push(InstanceUpgradeResolutionResult {
                content_id: resolution.content_id,
                code: Some("duplicate_request".to_string()),
                message: Some(
                    "Duplicate request replaced by its last value".to_string(),
                ),
            });
        }
    }
    let mut normalized = by_content_id.into_values().collect::<Vec<_>>();
    normalized.sort_by(|left, right| left.content_id.cmp(&right.content_id));
    (normalized, skipped)
}

fn resolution_is_applicable(
    plan: &InstanceUpgradePlan,
    resolution: &InstanceUpgradeResolution,
) -> bool {
    plan.items.iter().any(|item| {
        item.content_id == resolution.content_id
            && item.resolution != *resolution
    })
}

fn apply_resolutions(
    plan: &mut InstanceUpgradePlan,
    resolutions: &[InstanceUpgradeResolution],
) -> crate::Result<()> {
    for resolution in resolutions {
        let item = plan
            .items
            .iter_mut()
            .find(|item| item.content_id == resolution.content_id)
            .ok_or_else(|| {
                crate::ErrorKind::InputError(format!(
                    "Upgrade plan has no content item {}",
                    resolution.content_id
                ))
            })?;
        item.resolution = resolution.clone();
    }
    Ok(())
}

fn automatic_resolution(
    item: &crate::state::InstanceUpgradeItem,
) -> InstanceUpgradeResolution {
    let action = if matches!(
        item.status,
        crate::state::InstanceUpgradeItemStatus::Unidentified
            | crate::state::InstanceUpgradeItemStatus::UnsupportedContentType
    ) {
        InstanceUpgradeAction::Keep
    } else {
        InstanceUpgradeAction::Upgrade
    };
    InstanceUpgradeResolution {
        content_id: item.content_id.clone(),
        action,
        allow_prerelease: false,
        confirmed_prerelease_dependencies: Vec::new(),
    }
}

#[tracing::instrument]
pub async fn select_instance_upgrade_solution(
    plan_id: &str,
    choice: InstanceUpgradeSolutionChoice,
) -> crate::Result<InstanceUpgradePlan> {
    let state = State::get().await?;
    let handle = stored_plan_handle(plan_id)?;
    let mut stored = handle.lock().await;
    ensure_current_revision(&mut stored, &state).await?;
    let mut plan = stored.plan.clone();
    plan.selected_solution = match choice {
        InstanceUpgradeSolutionChoice::Newest => plan.newest_solution.clone(),
        InstanceUpgradeSolutionChoice::MinimalChange => {
            plan.minimal_change_solution.clone()
        }
        InstanceUpgradeSolutionChoice::Custom => Some(
            plan.selected_solution
                .clone()
                .filter(|solution| {
                    solution.kind == InstanceUpgradeSolutionKind::Custom
                })
                .ok_or_else(|| {
                    crate::ErrorKind::InputError(
                        "No custom upgrade solution has been resolved"
                            .to_string(),
                    )
                })?,
        ),
    };
    plan.dependency_changes = plan
        .selected_solution
        .as_ref()
        .map(|solution| solution.dependency_changes.clone())
        .unwrap_or_default();
    stored.plan = plan.clone();
    Ok(plan)
}

#[tracing::instrument]
pub async fn resolve_custom_instance_upgrade_solution(
    plan_id: &str,
    fixed_constraints: Vec<InstanceUpgradeFixedConstraint>,
) -> crate::Result<InstanceUpgradePlan> {
    let state = State::get().await?;
    let handle = stored_plan_handle(plan_id)?;
    let mut stored = handle.lock().await;
    let source = ensure_current_revision(&mut stored, &state).await?;
    let mut plan = stored.plan.clone();
    validate_fixed_constraints(&plan, &fixed_constraints)?;
    crate::state::instances::commands::recompute_instance_upgrade_plan_from_source(
        &mut plan,
        &fixed_constraints,
        InstanceUpgradeSolutionKind::Custom,
        source,
        &state,
    )
    .await?;
    plan.custom_constraints = fixed_constraints;
    stored.plan = plan.clone();
    Ok(plan)
}

#[tracing::instrument]
pub async fn execute_instance_upgrade(
    plan_id: &str,
    create_full_backup: bool,
    shared_upgrade_mode: SharedUpgradeMode,
    display_names: crate::install::InstanceUpgradeDisplayNames,
) -> crate::Result<InstallJobSnapshot> {
    let state = State::get().await?;
    let handle = stored_plan_handle(plan_id)?;
    let mut stored = handle.lock().await;
    if stored.execution_started {
        return Err(crate::ErrorKind::InputError(
            "Upgrade plan execution has already started".to_string(),
        )
        .into());
    }
    let _execution_guard =
        state.lock_instance_content(&stored.plan.instance_id).await;
    let current_revision = content_rows::get_applied_content_set(
        &stored.plan.instance_id,
        &state.pool,
    )
    .await?
    .ok_or_else(|| {
        crate::ErrorKind::InputError(
            "Instance has no applied content set".to_string(),
        )
    })?
    .revision;
    ensure_instance_upgrade_revision(
        stored.plan.source_revision,
        current_revision,
    )?;
    if !stored.plan.blocking_issues.is_empty() {
        return Err(crate::ErrorKind::InputError(
            "Upgrade plan still has blocking issues".to_string(),
        )
        .into());
    }
    let solution = stored.plan.selected_solution.clone().ok_or_else(|| {
        crate::ErrorKind::InputError(
            "Upgrade plan has no selected solution".to_string(),
        )
    })?;
    let metadata = crate::state::instances::commands::get_instance_metadata(
        &stored.plan.instance_id,
        &state.pool,
    )
    .await?
    .ok_or_else(|| {
        crate::ErrorKind::InputError("Unknown instance".to_string())
    })?;
    if !matches!(metadata.link, InstanceLink::Unmanaged)
        || metadata.applied_content_set.source_kind != ContentSourceKind::Local
    {
        return Err(crate::ErrorKind::InputError(
            "Only Local unmanaged instances can use upgrade execution"
                .to_string(),
        )
        .into());
    }
    if state
        .process_manager
        .get_all()
        .iter()
        .any(|process| process.instance_id == stored.plan.instance_id)
    {
        return Err(crate::ErrorKind::InputError(
            "Instance is currently running".to_string(),
        )
        .into());
    }
    let resolved_target_loader =
        crate::launcher::get_loader_version_from_profile(
            &stored.plan.target_environment.game_version,
            stored.plan.target_environment.mod_loader,
            stored.plan.target_environment.mod_loader_version.as_deref(),
        )
        .await?;
    if stored.plan.target_environment.mod_loader
        != crate::state::ModLoader::Vanilla
        && resolved_target_loader.is_none()
    {
        return Err(crate::ErrorKind::InputError(
            "Target loader version is no longer available".to_string(),
        )
        .into());
    }
    let upgrade_target =
        crate::launcher::instance_runtime::InstanceRuntimeAdapter::for_instance(
            &metadata.instance,
            &state.directories,
        )?
        .game_dir();
    ensure_upgrade_disk_space(
        &upgrade_target,
        &stored.plan.source_files,
        create_full_backup,
        shared_upgrade_mode,
    )?;
    ensure_upgrade_target_writable(&upgrade_target).await?;
    crate::state::instances::commands::validate_instance_upgrade_plan_source(
        &stored.plan,
        &state,
    )
    .await?;
    let source_watch = state
        .file_watcher
        .content_watch_snapshot(&stored.plan.instance_id)
        .await
        .map(|snapshot| InstanceUpgradeWatchBaseline {
            epoch: snapshot.epoch,
            generation: snapshot.generation,
            dirty_paths: snapshot.dirty_paths.into_iter().collect(),
        });
    if crate::install::store::list(false, &state)
        .await?
        .into_iter()
        .any(|job| {
            !job.status.is_finished()
                && job.instance_id.as_deref()
                    == Some(stored.plan.instance_id.as_str())
        })
    {
        return Err(crate::ErrorKind::InputError(
            "Instance already has an active install job".to_string(),
        )
        .into());
    }
    let instance_id = stored.plan.instance_id.clone();
    let mut target_environment = stored.plan.target_environment.clone();
    if let Some(loader) = resolved_target_loader {
        target_environment.mod_loader_version = Some(loader.id);
    }
    let mut warnings = stored.plan.warnings.clone();
    for warning in &solution.warnings {
        if !warnings.contains(warning) {
            warnings.push(warning.clone());
        }
    }
    let execution = InstanceUpgradeExecution {
        source_revision: stored.plan.source_revision,
        source_files: stored.plan.source_files.clone(),
        source_environment: stored.plan.source_environment.clone(),
        target_environment,
        items: stored.plan.items.clone(),
        solution,
        warnings,
        source_watch,
    };
    stored.execution_started = true;
    let result = crate::install::upgrade_unmanaged_instance(
        instance_id,
        plan_id.to_string(),
        execution,
        create_full_backup,
        shared_upgrade_mode,
        display_names,
    )
    .await;
    if result.is_err() {
        stored.execution_started = false;
    } else {
        INSTANCE_UPGRADE_PLANS.remove(plan_id);
    }
    result
}

pub async fn get_instance_post_upgrade_notice(
    instance_id: &str,
) -> crate::Result<Option<crate::state::InstancePostUpgradeNotice>> {
    let state = State::get().await?;
    crate::state::instances::commands::get_instance_post_upgrade_notice(
        instance_id,
        &state.pool,
    )
    .await
}

pub async fn dismiss_instance_post_upgrade_notice(
    instance_id: &str,
) -> crate::Result<()> {
    let state = State::get().await?;
    crate::state::instances::commands::dismiss_instance_post_upgrade_notice(
        instance_id,
        &state.pool,
    )
    .await
}

fn ensure_upgrade_disk_space(
    instance_path: &std::path::Path,
    source_files: &[crate::state::InstanceUpgradeSourceFile],
    create_full_backup: bool,
    shared_upgrade_mode: SharedUpgradeMode,
) -> crate::Result<()> {
    let canonical = crate::util::io::canonicalize(instance_path)?;
    let disks = sysinfo::Disks::new_with_refreshed_list();
    let available = disks
        .iter()
        .filter(|disk| canonical.starts_with(disk.mount_point()))
        .max_by_key(|disk| disk.mount_point().as_os_str().len())
        .map(sysinfo::Disk::available_space);
    let Some(available) = available else {
        return Ok(());
    };
    let source_size = source_files
        .iter()
        .fold(0_u64, |total, file| total.saturating_add(file.size));
    let copies = 1_u64
        + u64::from(
            create_full_backup
                && shared_upgrade_mode == SharedUpgradeMode::Direct,
        );
    let required = source_size
        .saturating_mul(copies)
        .saturating_add(source_size / 10);
    if available < required {
        return Err(crate::ErrorKind::FSError(format!(
            "Not enough free disk space for upgrade staging: need {required} bytes, have {available} bytes"
        ))
        .into());
    }
    Ok(())
}

async fn ensure_upgrade_target_writable(
    path: &std::path::Path,
) -> crate::Result<()> {
    let probe = path.join(format!(
        ".instance-upgrade-write-test-{}",
        uuid::Uuid::new_v4()
    ));
    let file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
        .await
        .map_err(|error| {
            crate::ErrorKind::FSError(format!(
                "Upgrade target is not writable: {error}"
            ))
        })?;
    drop(file);
    tokio::fs::remove_file(&probe).await.map_err(|error| {
        crate::ErrorKind::FSError(format!(
            "Upgrade write probe could not be removed: {error}"
        ))
        .into()
    })
}

fn stored_plan_handle(plan_id: &str) -> crate::Result<StoredUpgradePlan> {
    INSTANCE_UPGRADE_PLANS.get(plan_id).ok_or_else(|| {
        crate::ErrorKind::InputError(
            "The instance upgrade plan has expired".to_string(),
        )
        .into()
    })
}

async fn ensure_current_revision(
    stored: &mut StoredUpgradePlanState,
    state: &State,
) -> crate::Result<crate::state::instances::commands::ReadOnlyUpgradeSource> {
    let current_revision = content_rows::get_applied_content_set(
        &stored.plan.instance_id,
        &state.pool,
    )
    .await?
    .ok_or_else(|| {
        crate::ErrorKind::InputError(
            "Instance has no applied content set".to_string(),
        )
    })?
    .revision;
    ensure_instance_upgrade_revision(
        stored.plan.source_revision,
        current_revision,
    )?;
    stored.validation.validate(&stored.plan, state).await
}

fn ensure_instance_upgrade_revision(
    planned_revision: u64,
    current_revision: u64,
) -> crate::Result<()> {
    if planned_revision == current_revision {
        return Ok(());
    }
    Err(crate::ErrorKind::StaleInstanceUpgradePlan {
        planned_revision,
        current_revision,
    }
    .into())
}

fn selected_kind_and_constraints(
    plan: &InstanceUpgradePlan,
) -> (
    InstanceUpgradeSolutionKind,
    Vec<InstanceUpgradeFixedConstraint>,
) {
    let Some(solution) = plan.selected_solution.as_ref() else {
        return (InstanceUpgradeSolutionKind::Newest, Vec::new());
    };
    if solution.kind != InstanceUpgradeSolutionKind::Custom {
        return (solution.kind, Vec::new());
    }
    (
        InstanceUpgradeSolutionKind::Custom,
        plan.custom_constraints.clone(),
    )
}

fn validate_fixed_constraints(
    plan: &InstanceUpgradePlan,
    constraints: &[InstanceUpgradeFixedConstraint],
) -> crate::Result<()> {
    let mut seen = HashMap::new();
    for constraint in constraints {
        if let Some(previous) = seen.insert(
            constraint.content_id.as_str(),
            constraint.version_id.as_str(),
        ) && previous != constraint.version_id.as_str()
        {
            return Err(crate::ErrorKind::InputError(format!(
                "Custom upgrade constraints select multiple versions for content {}",
                constraint.content_id
            ))
            .into());
        }
        let root_exists = plan.items.iter().any(|item| {
            !item.auto_dependency
                && item.content_id == constraint.content_id
                && item.provider == Some(constraint.provider)
                && item.project_id.as_deref()
                    == Some(constraint.project_id.as_str())
        });
        if !root_exists {
            return Err(crate::ErrorKind::InputError(format!(
				"Custom upgrade constraint does not match root content {} at {}:{}",
				constraint.content_id,
				constraint.provider.as_str(),
				constraint.project_id
			))
			.into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_fixed_constraint_preserves_content_id_through_serde() {
        let input = serde_json::json!({
            "contentId": "physical-iris",
            "provider": "modrinth",
            "projectId": "YL57xq9U",
            "versionId": "Rhzf61g1"
        });

        let constraint: InstanceUpgradeFixedConstraint =
            serde_json::from_value(input).unwrap();
        let output = serde_json::to_value(constraint).unwrap();

        assert_eq!(output["contentId"], "physical-iris");
    }

    #[test]
    fn duplicate_custom_constraints_are_rejected_before_provider_work() {
        let mut plan = empty_plan();
        plan.items.push(crate::state::InstanceUpgradeItem {
            content_id: "root".to_string(),
            relative_path: "mods/root.jar".to_string(),
            project_type: crate::state::ProjectType::Mod,
            provider: Some(crate::state::ContentProvider::Modrinth),
            project_id: Some("root".to_string()),
            current_release_id: Some("old".to_string()),
            current_enabled: true,
            auto_dependency: false,
            status: crate::state::InstanceUpgradeItemStatus::UpgradeAvailable,
            resolution: crate::state::InstanceUpgradeResolution {
                content_id: "root".to_string(),
                action: crate::state::InstanceUpgradeAction::Upgrade,
                allow_prerelease: false,
                confirmed_prerelease_dependencies: Vec::new(),
            },
            candidate_release_ids: vec!["one".to_string(), "two".to_string()],
        });
        let constraints = vec![
            InstanceUpgradeFixedConstraint {
                content_id: "root".to_string(),
                provider: crate::state::ContentProvider::Modrinth,
                project_id: "root".to_string(),
                version_id: "one".to_string(),
            },
            InstanceUpgradeFixedConstraint {
                content_id: "root".to_string(),
                provider: crate::state::ContentProvider::Modrinth,
                project_id: "root".to_string(),
                version_id: "two".to_string(),
            },
        ];
        assert!(validate_fixed_constraints(&plan, &constraints).is_err());

        let wrong_physical_root = vec![InstanceUpgradeFixedConstraint {
            content_id: "different-root".to_string(),
            provider: crate::state::ContentProvider::Modrinth,
            project_id: "root".to_string(),
            version_id: "one".to_string(),
        }];
        assert!(
            validate_fixed_constraints(&plan, &wrong_physical_root).is_err()
        );
    }

    #[test]
    fn stale_upgrade_plan_revision_is_rejected() {
        assert!(ensure_instance_upgrade_revision(4, 4).is_ok());
        let error = ensure_instance_upgrade_revision(4, 5).unwrap_err();
        assert!(error.to_string().contains("planned revision 4"));
        assert!(error.to_string().contains("current revision 5"));
    }

    #[test]
    fn custom_recompute_uses_only_explicitly_stored_constraints() {
        let mut plan = empty_plan();
        plan.custom_constraints = vec![InstanceUpgradeFixedConstraint {
            content_id: "a".to_string(),
            provider: crate::state::ContentProvider::Modrinth,
            project_id: "a".to_string(),
            version_id: "a-fixed".to_string(),
        }];
        plan.selected_solution = Some(crate::state::InstanceUpgradeSolution {
            kind: InstanceUpgradeSolutionKind::Custom,
            selections: vec![
                crate::state::InstanceUpgradeSelection {
                    content_id: "a".to_string(),
                    provider: Some(crate::state::ContentProvider::Modrinth),
                    project_id: Some("a".to_string()),
                    current_release_id: Some("a-old".to_string()),
                    target_release_id: Some("a-fixed".to_string()),
                    action: crate::state::InstanceUpgradeAction::Upgrade,
                    enabled: true,
                },
                crate::state::InstanceUpgradeSelection {
                    content_id: "b".to_string(),
                    provider: Some(crate::state::ContentProvider::Modrinth),
                    project_id: Some("b".to_string()),
                    current_release_id: Some("b-old".to_string()),
                    target_release_id: Some("b-auto".to_string()),
                    action: crate::state::InstanceUpgradeAction::Upgrade,
                    enabled: true,
                },
            ],
            dependency_changes: Vec::new(),
            warnings: Vec::new(),
        });
        let (kind, constraints) = selected_kind_and_constraints(&plan);
        assert_eq!(kind, InstanceUpgradeSolutionKind::Custom);
        assert_eq!(constraints, plan.custom_constraints);
        assert_eq!(constraints.len(), 1);
        assert_eq!(constraints[0].project_id, "a");
    }

    #[test]
    fn batch_requests_dedupe_by_content_with_last_value_and_stable_order() {
        let request = |content_id: &str, action| InstanceUpgradeResolution {
            content_id: content_id.to_string(),
            action,
            allow_prerelease: false,
            confirmed_prerelease_dependencies: Vec::new(),
        };
        let (normalized, skipped) = normalize_batch_resolutions(vec![
            request("b", InstanceUpgradeAction::Keep),
            request("a", InstanceUpgradeAction::Disable),
            request("b", InstanceUpgradeAction::Disable),
        ]);
        assert_eq!(
            normalized
                .iter()
                .map(|item| item.content_id.as_str())
                .collect::<Vec<_>>(),
            ["a", "b"]
        );
        assert_eq!(normalized[1].action, InstanceUpgradeAction::Disable);
        assert_eq!(skipped.len(), 1);
    }

    #[test]
    fn batch_resolution_is_skipped_when_plan_already_has_requested_value() {
        let mut plan = empty_plan();
        plan.items.push(crate::state::InstanceUpgradeItem {
            content_id: "root".to_string(),
            relative_path: "mods/root.jar".to_string(),
            project_type: crate::state::ProjectType::Mod,
            provider: None,
            project_id: None,
            current_release_id: None,
            current_enabled: true,
            auto_dependency: false,
            status: crate::state::InstanceUpgradeItemStatus::Unidentified,
            resolution: InstanceUpgradeResolution {
                content_id: "root".to_string(),
                action: InstanceUpgradeAction::Keep,
                allow_prerelease: false,
                confirmed_prerelease_dependencies: Vec::new(),
            },
            candidate_release_ids: Vec::new(),
        });
        let request = plan.items[0].resolution.clone();
        assert!(!resolution_is_applicable(&plan, &request));
    }

    #[tokio::test]
    async fn per_plan_mutex_serializes_mutations_without_lost_update() {
        let plan = Arc::new(Mutex::new(empty_plan()));
        let acquired = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let first_plan = Arc::clone(&plan);
        let first_acquired = Arc::clone(&acquired);
        let first_release = Arc::clone(&release);
        let first = tokio::spawn(async move {
            let mut stored = first_plan.lock().await;
            first_acquired.notify_one();
            first_release.notified().await;
            stored
                .custom_constraints
                .push(InstanceUpgradeFixedConstraint {
                    content_id: "a".to_string(),
                    provider: crate::state::ContentProvider::Modrinth,
                    project_id: "a".to_string(),
                    version_id: "a-one".to_string(),
                });
        });
        acquired.notified().await;
        let second_plan = Arc::clone(&plan);
        let second = tokio::spawn(async move {
            let mut stored = second_plan.lock().await;
            stored
                .custom_constraints
                .push(InstanceUpgradeFixedConstraint {
                    content_id: "b".to_string(),
                    provider: crate::state::ContentProvider::Modrinth,
                    project_id: "b".to_string(),
                    version_id: "b-one".to_string(),
                });
        });
        tokio::task::yield_now().await;
        release.notify_one();
        first.await.unwrap();
        second.await.unwrap();
        let stored = plan.lock().await;
        assert_eq!(stored.custom_constraints.len(), 2);
    }

    fn empty_plan() -> InstanceUpgradePlan {
        let environment = crate::state::InstanceUpgradeEnvironment {
            game_version: "1.21.1".to_string(),
            mod_loader: crate::state::ModLoader::Fabric,
            mod_loader_version: None,
            shader_runtime: crate::state::ShaderRuntime::Iris,
        };
        InstanceUpgradePlan {
            id: "plan".to_string(),
            instance_id: "instance".to_string(),
            source_revision: 1,
            source_files: Vec::new(),
            source_environment: environment.clone(),
            target_environment: environment,
            items: Vec::new(),
            dependency_changes: Vec::new(),
            warnings: Vec::new(),
            blocking_issues: Vec::new(),
            newest_solution: None,
            minimal_change_solution: None,
            selected_solution: None,
            custom_constraints: Vec::new(),
        }
    }
}

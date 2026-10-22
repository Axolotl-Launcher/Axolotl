use crate::state::{
    CacheBehaviour, ContentFile, ContentItem, ContentItemUpdate,
    ContentOwnershipKind, ContentProvider, ContentSet, ContentUpdatePlan,
    ContentUpdatePlanAction, ContentUpdateResolution,
    ContentUpdateResolutionChoice, ContentUpdateScope, Dependency,
    InstanceContentSnapshot, InstanceInstallCandidate, InstanceInstallTarget,
    LinkedModpackInfo, ProjectType, State,
};
use dashmap::DashMap;
use std::sync::LazyLock;

static CONTENT_UPDATE_PLANS: LazyLock<DashMap<String, ContentUpdatePlan>> =
    LazyLock::new(DashMap::new);

#[tracing::instrument]
pub async fn sync_content_files(
    instance_id: &str,
) -> crate::Result<Vec<crate::state::instances::InstanceFile>> {
    let state = State::get().await?;
    crate::state::sync_content_files(instance_id, &state).await
}

#[tracing::instrument]
pub async fn list_content_sets(
    instance_id: &str,
) -> crate::Result<Vec<ContentSet>> {
    let state = State::get().await?;
    crate::state::list_content_sets(instance_id, &state.pool).await
}

#[tracing::instrument]
pub async fn get_projects(
    instance_id: &str,
    cache_behaviour: Option<CacheBehaviour>,
) -> crate::Result<DashMap<String, ContentFile>> {
    let state = State::get().await?;
    crate::state::get_content_projects(
        instance_id,
        None,
        cache_behaviour,
        &state,
    )
    .await
}

#[tracing::instrument]
pub async fn get_installed_project_ids(
    instance_id: &str,
) -> crate::Result<Vec<String>> {
    let state = State::get().await?;
    crate::state::get_installed_project_ids_for_instance(
        instance_id,
        None,
        &state,
    )
    .await
}

#[tracing::instrument]
pub async fn get_install_candidates(
    project_id: &str,
    project_type: ProjectType,
    targets: Vec<InstanceInstallTarget>,
) -> crate::Result<Vec<InstanceInstallCandidate>> {
    let state = State::get().await?;
    crate::state::get_instance_install_candidates(
        project_id,
        project_type,
        &targets,
        &state.pool,
    )
    .await
}

#[tracing::instrument]
pub async fn get_content_items(
    instance_id: &str,
    cache_behaviour: Option<CacheBehaviour>,
) -> crate::Result<Vec<ContentItem>> {
    let state = State::get().await?;
    crate::state::list_content(instance_id, None, cache_behaviour, &state).await
}

#[tracing::instrument]
pub async fn get_content_items_by_paths(
    instance_id: &str,
    paths: Vec<String>,
    cache_behaviour: Option<CacheBehaviour>,
) -> crate::Result<Vec<ContentItem>> {
    let state = State::get().await?;
    crate::state::list_content_by_paths(
        instance_id,
        &paths,
        cache_behaviour,
        &state,
    )
    .await
}

#[tracing::instrument]
pub async fn get_content_snapshot(
    instance_id: &str,
) -> crate::Result<InstanceContentSnapshot> {
    let state = State::get().await?;
    crate::state::get_content_snapshot(instance_id, false, &state).await
}

pub async fn get_content_snapshot_for_display(
    instance_id: &str,
) -> crate::Result<InstanceContentSnapshot> {
    let state = State::get().await?;
    crate::state::instances::commands::get_local_content_snapshot(
        instance_id,
        &state,
    )
    .await
}

#[tracing::instrument]
pub async fn refresh_content(
    instance_id: &str,
) -> crate::Result<InstanceContentSnapshot> {
    let state = State::get().await?;
    require_installed_instance(instance_id, &state).await?;
    crate::state::get_content_snapshot(instance_id, true, &state).await
}

#[tracing::instrument]
pub async fn plan_content_updates(
    instance_id: &str,
    scope: ContentUpdateScope,
    target: Option<&str>,
) -> crate::Result<ContentUpdatePlan> {
    if scope != ContentUpdateScope::Pack {
        return Err(crate::ErrorKind::InputError(
            "Content updates require a reviewed resource operation plan".into(),
        )
        .into());
    }
    plan_content_updates_with_refresh(instance_id, scope, target, true).await
}

async fn plan_content_updates_with_refresh(
    instance_id: &str,
    scope: ContentUpdateScope,
    target: Option<&str>,
    refresh_remote: bool,
) -> crate::Result<ContentUpdatePlan> {
    let state = State::get().await?;
    let snapshot =
        crate::state::get_content_snapshot(instance_id, refresh_remote, &state)
            .await?;
    let mut actions = Vec::new();

    match scope {
        ContentUpdateScope::UserAdded | ContentUpdateScope::Item => {
            if scope == ContentUpdateScope::Item && target.is_none() {
                return Err(crate::ErrorKind::InputError(
                    "An item update plan requires a stable content ID"
                        .to_string(),
                )
                .into());
            }
            for item in &snapshot.items {
                let content_id = item
                    .entry_id
                    .as_deref()
                    .or(item.member_id.as_deref())
                    .or(item.file_id.as_deref());
                let selected =
                    update_scope_selects_item(scope, target, item, content_id);
                if !selected {
                    continue;
                }
                let Some(content) = item.content.as_ref() else {
                    continue;
                };
                let Some(update) = content.update.as_ref() else {
                    continue;
                };
                let Some(content_id) = content_id else {
                    continue;
                };
                actions.push(update_action(
                    content_id.to_string(),
                    Some(item.expected_relative_path.clone()),
                    item.ownership_kind,
                    update,
                ));
            }
        }
        ContentUpdateScope::Pack => {
            let pack = snapshot.pack.as_ref().ok_or_else(|| {
                crate::ErrorKind::InputError(
                    "This instance is not linked to a managed pack".to_string(),
                )
            })?;
            if !pack.reconciled {
                return Err(crate::ErrorKind::InputError(
					"Pack membership is not calibrated yet; refresh while online before updating the pack"
						.to_string(),
				)
				.into());
            }
            if let Some(target_release_id) = target {
                let provider = pack.provider.ok_or_else(|| {
                    crate::ErrorKind::InputError(
                        "This pack has no managed provider".to_string(),
                    )
                })?;
                actions.push(ContentUpdatePlanAction {
                    content_id: "pack".to_string(),
                    relative_path: None,
                    ownership_kind: ContentOwnershipKind::PackManaged,
                    provider,
                    current_release_id: pack.version_id.clone(),
                    target_release_id: target_release_id.to_string(),
                });
            } else if let Some(update) = pack
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.update.as_ref())
            {
                actions.push(update_action(
                    "pack".to_string(),
                    None,
                    ContentOwnershipKind::PackManaged,
                    update,
                ));
            }
        }
    }

    let plan = ContentUpdatePlan {
        id: format!("content-update-plan:{}", uuid::Uuid::new_v4()),
        instance_id: instance_id.to_string(),
        revision: snapshot.revision,
        scope,
        actions,
        warnings: snapshot
            .warnings
            .into_iter()
            .map(|warning| warning.message)
            .collect(),
    };
    CONTENT_UPDATE_PLANS.insert(plan.id.clone(), plan.clone());
    Ok(plan)
}

#[tracing::instrument]
pub async fn apply_content_update_plan(
    plan_id: &str,
    resolutions: Vec<ContentUpdateResolution>,
) -> crate::Result<InstanceContentSnapshot> {
    let plan = CONTENT_UPDATE_PLANS
        .get(plan_id)
        .map(|entry| entry.clone())
        .ok_or_else(|| {
            crate::ErrorKind::InputError(
                "The content update plan has expired".to_string(),
            )
        })?;
    let state = State::get().await?;
    let current_revision = crate::state::instances::adapters::sqlite::content_rows::get_applied_content_set(
		&plan.instance_id,
		&state.pool,
	)
	.await?
	.ok_or_else(|| {
		crate::ErrorKind::InputError(
			"Instance has no applied content set".to_string(),
		)
	})?
	.revision;
    if let Err(error) =
        ensure_update_plan_revision(plan.revision, current_revision)
    {
        CONTENT_UPDATE_PLANS.remove(plan_id);
        return Err(error);
    }

    for resolution in resolutions {
        if resolution.choice
            == ContentUpdateResolutionChoice::RestorePackDefault
        {
            super::projects::restore_pack_member_default(
                &plan.instance_id,
                &resolution.content_id,
            )
            .await?;
        }
    }

    if plan.scope == ContentUpdateScope::Pack {
        if let Some(action) = plan.actions.first() {
            match action.provider {
                ContentProvider::Modrinth => {
                    super::projects::update_managed_modrinth_version(
                        &plan.instance_id,
                        &action.target_release_id,
                    )
                    .await?;
                }
                ContentProvider::CurseForge => {
                    let file_id = action
                        .target_release_id
                        .parse::<u32>()
                        .map_err(|_| {
                            crate::ErrorKind::InputError(
                                "The planned CurseForge file ID is invalid"
                                    .to_string(),
                            )
                        })?;
                    crate::install::runner::update_managed_curseforge_modpack(
                        plan.instance_id.clone(),
                        file_id,
                    )
                    .await?;
                }
                ContentProvider::McArchive => {
                    return Err(crate::ErrorKind::InputError(
                        "MCArchive does not support automatic pack updates"
                            .to_string(),
                    )
                    .into());
                }
                ContentProvider::Local => {}
            }
        }
    } else {
        for action in &plan.actions {
            super::projects::switch_content_entry_version(
                &plan.instance_id,
                &action.content_id,
                &action.target_release_id,
                None,
            )
            .await?;
        }
    }

    CONTENT_UPDATE_PLANS.remove(plan_id);
    crate::state::get_content_snapshot(&plan.instance_id, false, &state).await
}

pub(crate) async fn resolve_content_change_actions(
    instance_id: &str,
    intent: &crate::install::ContentChangeIntent,
) -> crate::Result<Vec<crate::install::ContentChangeAction>> {
    use crate::install::{
        ContentChangeIntent, ContentChangeOperation, ContentChangeTarget,
    };

    match intent {
        ContentChangeIntent::UpdateOne {
            content_id,
            target_release_id: Some(target_release_id),
        } => Ok(vec![
            resolve_content_change_action(
                instance_id,
                &ContentChangeTarget {
                    content_id: content_id.clone(),
                    target_release_id: target_release_id.clone(),
                },
                ContentChangeOperation::Update,
            )
            .await?,
        ]),
        ContentChangeIntent::UpdateOne {
            content_id,
            target_release_id: None,
        } => {
            let mut plan = plan_content_updates_with_refresh(
                instance_id,
                ContentUpdateScope::Item,
                Some(content_id),
                false,
            )
            .await?;
            if plan.actions.is_empty() {
                CONTENT_UPDATE_PLANS.remove(&plan.id);
                plan = plan_content_updates_with_refresh(
                    instance_id,
                    ContentUpdateScope::Item,
                    Some(content_id),
                    true,
                )
                .await?;
            }
            CONTENT_UPDATE_PLANS.remove(&plan.id);
            let mut actions = Vec::with_capacity(plan.actions.len());
            for action in plan.actions {
                actions.push(
                    resolve_content_change_action(
                        instance_id,
                        &ContentChangeTarget {
                            content_id: action.content_id,
                            target_release_id: action.target_release_id,
                        },
                        ContentChangeOperation::Update,
                    )
                    .await?,
                );
            }
            if actions.is_empty() {
                return Err(crate::ErrorKind::InputError(
                    "The selected content has no available update".to_string(),
                )
                .into());
            }
            Ok(actions)
        }
        ContentChangeIntent::UpdateSelected { targets } => {
            if targets.is_empty() {
                return Err(crate::ErrorKind::InputError(
                    "At least one content item must be selected".to_string(),
                )
                .into());
            }
            let mut seen = std::collections::HashSet::new();
            let mut actions = Vec::with_capacity(targets.len());
            for target in targets {
                if !seen.insert(target.content_id.as_str()) {
                    return Err(crate::ErrorKind::InputError(format!(
                        "Content {} was selected more than once",
                        target.content_id
                    ))
                    .into());
                }
                actions.push(
                    resolve_content_change_action(
                        instance_id,
                        target,
                        ContentChangeOperation::Update,
                    )
                    .await?,
                );
            }
            Ok(actions)
        }
        ContentChangeIntent::UpdateAllUserAdded => {
            let mut plan = plan_content_updates_with_refresh(
                instance_id,
                ContentUpdateScope::UserAdded,
                None,
                false,
            )
            .await?;
            if plan.actions.is_empty() {
                CONTENT_UPDATE_PLANS.remove(&plan.id);
                plan = plan_content_updates_with_refresh(
                    instance_id,
                    ContentUpdateScope::UserAdded,
                    None,
                    true,
                )
                .await?;
            }
            CONTENT_UPDATE_PLANS.remove(&plan.id);
            let mut actions = Vec::with_capacity(plan.actions.len());
            for action in plan.actions {
                actions.push(
                    resolve_content_change_action(
                        instance_id,
                        &ContentChangeTarget {
                            content_id: action.content_id,
                            target_release_id: action.target_release_id,
                        },
                        ContentChangeOperation::Update,
                    )
                    .await?,
                );
            }
            Ok(actions)
        }
        ContentChangeIntent::SwitchVersion {
            content_id,
            target_release_id,
        } => Ok(vec![
            resolve_content_change_action(
                instance_id,
                &ContentChangeTarget {
                    content_id: content_id.clone(),
                    target_release_id: target_release_id.clone(),
                },
                ContentChangeOperation::SwitchVersion,
            )
            .await?,
        ]),
    }
}

async fn resolve_content_change_action(
    instance_id: &str,
    target: &crate::install::ContentChangeTarget,
    operation: crate::install::ContentChangeOperation,
) -> crate::Result<crate::install::ContentChangeAction> {
    let current = super::projects::content_mutation_target(
        instance_id,
        &target.content_id,
    )
    .await?;
    let provider = current.provider.ok_or_else(|| {
        crate::ErrorKind::InputError(format!(
            "Content {} is not linked to a supported provider",
            target.content_id
        ))
    })?;
    if !matches!(
        provider,
        ContentProvider::Modrinth | ContentProvider::CurseForge
    ) {
        return Err(crate::ErrorKind::InputError(format!(
            "Provider {} does not support content version changes",
            provider.as_str()
        ))
        .into());
    }
    let project_id = current.provider_project_id.ok_or_else(|| {
        crate::ErrorKind::InputError(format!(
            "Content {} has no provider project ID",
            target.content_id
        ))
    })?;

    Ok(crate::install::ContentChangeAction {
        content_id: target.content_id.clone(),
        operation,
        provider,
        project_id: Some(project_id),
        expected_release_id: current.provider_release_id,
        target_release_id: target.target_release_id.clone(),
        final_relative_path: current.relative_path.clone(),
        relative_path: current.relative_path,
        current_provider_file_name: None,
        target_provider_file_name: None,
        files: Vec::new(),
        dependencies: Vec::new(),
        modrinth_plan: None,
        status: crate::install::ContentChangeActionStatus::Pending,
        error: None,
        completed: false,
    })
}

fn update_action(
    content_id: String,
    relative_path: Option<String>,
    ownership_kind: ContentOwnershipKind,
    update: &ContentItemUpdate,
) -> ContentUpdatePlanAction {
    match update {
        ContentItemUpdate::Modrinth {
            current_version_id,
            target_version_id,
            ..
        } => ContentUpdatePlanAction {
            content_id,
            relative_path,
            ownership_kind,
            provider: ContentProvider::Modrinth,
            current_release_id: Some(current_version_id.to_string()),
            target_release_id: target_version_id.to_string(),
        },
        ContentItemUpdate::CurseForge {
            current_file_id,
            target_file_id,
            ..
        } => ContentUpdatePlanAction {
            content_id,
            relative_path,
            ownership_kind,
            provider: ContentProvider::CurseForge,
            current_release_id: Some(current_file_id.get().to_string()),
            target_release_id: target_file_id.get().to_string(),
        },
    }
}

fn update_scope_selects_item(
    scope: ContentUpdateScope,
    target: Option<&str>,
    item: &crate::state::InstanceContentSnapshotItem,
    content_id: Option<&str>,
) -> bool {
    match scope {
        ContentUpdateScope::UserAdded => {
            item.ownership_kind == ContentOwnershipKind::UserAdded
        }
        ContentUpdateScope::Item => content_id == target,
        ContentUpdateScope::Pack => false,
    }
}

fn ensure_update_plan_revision(
    planned_revision: u64,
    current_revision: u64,
) -> crate::Result<()> {
    if planned_revision == current_revision {
        return Ok(());
    }
    Err(crate::ErrorKind::InputError(format!(
        "The content update plan is stale (planned revision {planned_revision}, current revision {current_revision})"
    ))
    .into())
}

#[tracing::instrument]
pub async fn get_linked_modpack_content(
    instance_id: &str,
    cache_behaviour: Option<CacheBehaviour>,
) -> crate::Result<Vec<ContentItem>> {
    let state = State::get().await?;
    crate::state::list_linked_modpack_content(
        instance_id,
        None,
        cache_behaviour,
        &state,
    )
    .await
}

#[tracing::instrument]
pub async fn get_dependencies_as_content_items(
    dependencies: Vec<Dependency>,
    cache_behaviour: Option<CacheBehaviour>,
) -> crate::Result<Vec<ContentItem>> {
    let state = State::get().await?;
    crate::state::dependencies_to_content_items(
        &dependencies,
        cache_behaviour,
        &state.pool,
        &state.api_semaphore,
    )
    .await
}

#[tracing::instrument]
pub async fn get_linked_modpack_info(
    instance_id: &str,
    cache_behaviour: Option<CacheBehaviour>,
) -> crate::Result<Option<LinkedModpackInfo>> {
    let state = State::get().await?;
    require_installed_instance(instance_id, &state).await?;
    crate::state::get_linked_modpack_info(
        instance_id,
        None,
        cache_behaviour,
        &state,
    )
    .await
}

async fn require_installed_instance(
    instance_id: &str,
    state: &State,
) -> crate::Result<()> {
    let metadata = crate::state::instances::commands::get_instance_metadata(
        instance_id,
        &state.pool,
    )
    .await?
    .ok_or_else(|| {
        crate::ErrorKind::UnmanagedInstanceError(instance_id.to_string())
    })?;
    let stage = metadata.instance.install_stage;
    if stage != crate::state::InstanceInstallStage::Installed {
        return Err(crate::ErrorKind::InstanceNotReady {
            instance_id: instance_id.to_string(),
            stage: stage.as_str().to_string(),
        }
        .into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::instances::{
        ContentItemCapabilities, InstanceContentSnapshotItem,
        PackMemberMaterializationState, PackMemberOverrideKind,
    };

    fn snapshot_item(
        ownership_kind: ContentOwnershipKind,
        entry_id: Option<&str>,
        member_id: Option<&str>,
        file_id: Option<&str>,
    ) -> InstanceContentSnapshotItem {
        InstanceContentSnapshotItem {
            artifact_ref: None,
            compatibility: None,
            resource_id: None,
            pack_resource_id: None,
            file_id: file_id.map(str::to_string),
            entry_id: entry_id.map(str::to_string),
            member_id: member_id.map(str::to_string),
            ownership_kind,
            materialization_state: PackMemberMaterializationState::Present,
            override_kind: PackMemberOverrideKind::None,
            expected_relative_path: "mods/test.jar".to_string(),
            required: true,
            project_type: ProjectType::Mod,
            provider: None,
            provider_project_id: None,
            provider_release_id: None,
            content: None,
            capabilities: ContentItemCapabilities::default(),
            dependency: None,
        }
    }

    #[test]
    fn user_added_update_scope_excludes_pack_and_discovered_content() {
        let user_added = snapshot_item(
            ContentOwnershipKind::UserAdded,
            Some("user-entry"),
            None,
            Some("user-file"),
        );
        let pack_managed = snapshot_item(
            ContentOwnershipKind::PackManaged,
            Some("pack-entry"),
            Some("pack-member"),
            Some("pack-file"),
        );
        let discovered = snapshot_item(
            ContentOwnershipKind::LocalDiscovered,
            None,
            None,
            Some("discovered-file"),
        );

        assert!(update_scope_selects_item(
            ContentUpdateScope::UserAdded,
            None,
            &user_added,
            user_added.entry_id.as_deref(),
        ));
        assert!(!update_scope_selects_item(
            ContentUpdateScope::UserAdded,
            None,
            &pack_managed,
            pack_managed.entry_id.as_deref(),
        ));
        assert!(!update_scope_selects_item(
            ContentUpdateScope::UserAdded,
            None,
            &discovered,
            discovered.file_id.as_deref(),
        ));
    }

    #[test]
    fn update_plan_revision_rejects_stale_plans() {
        assert!(ensure_update_plan_revision(7, 7).is_ok());
        let error = ensure_update_plan_revision(7, 8).unwrap_err();
        assert!(error.to_string().contains("planned revision 7"));
        assert!(error.to_string().contains("current revision 8"));
    }
}

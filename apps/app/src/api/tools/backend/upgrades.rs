use super::{LauncherBackend, business_error, jobs, page, reviewed, serialize};
use launcher_tools::{BackendContext, Command, Loader, ToolError};
use serde_json::{Value, json};
use theseus::state::{
    InstanceUpgradeAction, InstanceUpgradeEnvironment, InstanceUpgradePlan,
    InstanceUpgradeResolution, InstanceUpgradeSolutionChoice, ModLoader,
    ShaderRuntime,
};

#[derive(Clone)]
pub(super) struct UpgradePlan {
    native_id: String,
    instance_id: String,
    revision: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn review_hides_native_ids_and_hashes_and_scopes_execution_handles() {
        let env = InstanceUpgradeEnvironment {
            game_version: "1.21.1".into(),
            mod_loader: ModLoader::Vanilla,
            mod_loader_version: None,
            shader_runtime: ShaderRuntime::None,
        };
        let plan = InstanceUpgradePlan {
            id: "private-native-id".into(),
            instance_id: "instance".into(),
            source_revision: 7,
            source_files: vec![theseus::state::InstanceUpgradeSourceFile {
                relative_path: "private-file.jar".into(),
                sha1: "private-hash".into(),
                size: 42,
                enabled: true,
            }],
            source_environment: env.clone(),
            target_environment: env,
            items: vec![],
            dependency_changes: vec![],
            warnings: vec![],
            blocking_issues: vec![],
            newest_solution: None,
            minimal_change_solution: None,
            selected_solution: None,
            custom_constraints: vec![],
        };
        let backend = LauncherBackend::default();
        let result = review(&backend, "owner", plan, 3).unwrap();
        assert_eq!(result["executable"], false);
        let encoded = result.to_string();
        for private in ["private-native-id", "private-hash", "private-file.jar"]
        {
            assert!(!encoded.contains(private));
        }
        let handle = serde_json::from_value(result["plan_id"].clone()).unwrap();
        assert!(backend.upgrades.get("another", handle, 3).is_err());
        assert!(backend.upgrades.get("owner", handle, 2).is_err());
        assert_eq!(
            backend.upgrades.take("owner", handle, 3).unwrap().native_id,
            "private-native-id"
        );
        assert!(backend.upgrades.take("owner", handle, 3).is_err());
    }
}

fn issue(issue: &theseus::state::InstanceUpgradeIssue) -> Value {
    json!({"code":issue.code,"content_id":issue.content_id,"provider":issue.provider,"project_id":issue.project_id,"conflicting_project_id":issue.conflicting_project_id,"dependency_requirements":issue.dependency_requirements.iter().take(30).collect::<Vec<_>>(),"requirements_truncated":issue.dependency_requirements.len()>30})
}
fn items(plan: &InstanceUpgradePlan) -> Vec<Value> {
    plan.items.iter().map(|item| json!({"content_id":item.content_id,"relative_path":item.relative_path,"project_type":item.project_type,"provider":item.provider,"project_id":item.project_id,"current_release_id":item.current_release_id,"current_enabled":item.current_enabled,"status":item.status,"resolution":item.resolution,"candidate_release_ids":item.candidate_release_ids.iter().take(30).collect::<Vec<_>>(),"candidates_truncated":item.candidate_release_ids.len()>30})).collect()
}
fn review(
    backend: &LauncherBackend,
    session: &str,
    plan: InstanceUpgradePlan,
    revision: u32,
) -> Result<Value, ToolError> {
    let executable =
        plan.blocking_issues.is_empty() && plan.selected_solution.is_some();
    let handle = backend.upgrades.insert(
        session,
        revision,
        UpgradePlan {
            native_id: plan.id.clone(),
            instance_id: plan.instance_id.clone(),
            revision,
        },
    )?;
    Ok(
        json!({"plan_id":handle,"revision":revision,"instance_id":plan.instance_id,"source_environment":plan.source_environment,"target_environment":plan.target_environment,"executable":executable,
        "item_count":plan.items.len(),"items":items(&plan).into_iter().take(100).collect::<Vec<_>>(),"warning_count":plan.warnings.len(),"warnings":plan.warnings.iter().take(100).map(issue).collect::<Vec<_>>(),"blocking_issue_count":plan.blocking_issues.len(),"blocking_issues":plan.blocking_issues.iter().take(100).map(issue).collect::<Vec<_>>(),
        "selected_solution":plan.selected_solution.as_ref().map(|s|s.kind),"newest_available":plan.newest_solution.is_some(),"minimal_change_available":plan.minimal_change_solution.is_some(),"dependency_change_count":plan.dependency_changes.len(),"dependency_changes":plan.dependency_changes.iter().take(100).collect::<Vec<_>>()}),
    )
}

pub(super) async fn execute(
    backend: &LauncherBackend,
    command: Command,
    context: BackendContext,
) -> Result<Value, ToolError> {
    if let Command::InstancePlanUpgrade {
        instance_id,
        game_version,
        loader,
        loader_version,
        shader_runtime,
    } = command
    {
        let metadata = reviewed::instance(&instance_id).await?;
        if !matches!(metadata.link, theseus::data::InstanceLink::Unmanaged)
            || metadata.applied_content_set.source_kind
                != theseus::state::ContentSourceKind::Local
        {
            return Err(ToolError::new(
                "managed_pack",
                "Use modpack installation/update for a managed pack",
            ));
        }
        let target = InstanceUpgradeEnvironment {
            game_version,
            mod_loader: match loader {
                Loader::Vanilla => ModLoader::Vanilla,
                Loader::Fabric => ModLoader::Fabric,
                Loader::Forge => ModLoader::Forge,
                Loader::Quilt => ModLoader::Quilt,
                Loader::NeoForge => ModLoader::NeoForge,
            },
            mod_loader_version: loader_version,
            shader_runtime: match shader_runtime.as_str() {
                "iris" => ShaderRuntime::Iris,
                "optifine" => ShaderRuntime::OptiFine,
                "none" => ShaderRuntime::None,
                _ => ShaderRuntime::Unknown,
            },
        };
        // Accepted MCP operations finish independently of their connection.
        let plan = theseus::instance::plan_instance_upgrade(
            &instance_id,
            target,
            tokio_util::sync::CancellationToken::new(),
            None,
        )
        .await
        .map_err(business_error)?;
        return review(backend, &context.session_id, plan, 1);
    }
    let (id, revision) = match &command {
        Command::InstanceGetUpgradePlan {
            plan_id, revision, ..
        }
        | Command::InstanceReviseUpgrade {
            plan_id, revision, ..
        }
        | Command::InstanceSelectUpgradeSolution {
            plan_id, revision, ..
        }
        | Command::InstanceExecuteUpgrade {
            plan_id, revision, ..
        } => (*plan_id, *revision),
        _ => {
            return Err(ToolError::new(
                "unsupported_command",
                "Not an upgrade operation",
            ));
        }
    };
    let stored = backend.upgrades.get(&context.session_id, id, revision)?;
    let current =
        theseus::instance::get_instance_upgrade_plan(&stored.native_id)
            .await
            .map_err(business_error)?;
    match command {
        Command::InstanceGetUpgradePlan {
            section,
            offset,
            limit,
            ..
        } => {
            let values = match section.as_str() {
                "items" => items(&current),
                "warnings" => current.warnings.iter().map(issue).collect(),
                "blocking_issues" => {
                    current.blocking_issues.iter().map(issue).collect()
                }
                "dependency_changes" => current
                    .dependency_changes
                    .into_iter()
                    .map(serialize)
                    .collect::<Result<_, _>>()?,
                "selected_selections" => current
                    .selected_solution
                    .map(|s| s.selections)
                    .unwrap_or_default()
                    .into_iter()
                    .map(serialize)
                    .collect::<Result<_, _>>()?,
                _ => {
                    return Err(ToolError::new(
                        "invalid_section",
                        "Unknown upgrade section",
                    ));
                }
            };
            page(values, offset, limit)
        }
        Command::InstanceReviseUpgrade {
            content_id,
            action,
            allow_prerelease,
            ..
        } => {
            let resolution = InstanceUpgradeResolution {
                content_id,
                action: match action.as_str() {
                    "upgrade" => InstanceUpgradeAction::Upgrade,
                    "keep" => InstanceUpgradeAction::Keep,
                    "disable" => InstanceUpgradeAction::Disable,
                    _ => {
                        return Err(ToolError::new(
                            "invalid_action",
                            "Unknown upgrade action",
                        ));
                    }
                },
                allow_prerelease,
                confirmed_prerelease_dependencies: vec![],
            };
            let plan = theseus::instance::update_instance_upgrade_resolution(
                &stored.native_id,
                resolution,
            )
            .await
            .map_err(business_error)?;
            backend.upgrades.take(&context.session_id, id, revision)?;
            review(
                backend,
                &context.session_id,
                plan,
                stored.revision.saturating_add(1),
            )
        }
        Command::InstanceSelectUpgradeSolution { solution, .. } => {
            let choice = match solution.as_str() {
                "newest" => InstanceUpgradeSolutionChoice::Newest,
                "minimal_change" => {
                    InstanceUpgradeSolutionChoice::MinimalChange
                }
                _ => {
                    return Err(ToolError::new(
                        "invalid_solution",
                        "Unknown upgrade solution",
                    ));
                }
            };
            let plan = theseus::instance::select_instance_upgrade_solution(
                &stored.native_id,
                choice,
            )
            .await
            .map_err(business_error)?;
            backend.upgrades.take(&context.session_id, id, revision)?;
            review(
                backend,
                &context.session_id,
                plan,
                stored.revision.saturating_add(1),
            )
        }
        Command::InstanceExecuteUpgrade {
            create_full_backup, ..
        } => {
            reviewed::instance(&stored.instance_id).await?;
            if !current.blocking_issues.is_empty()
                || current.selected_solution.is_none()
            {
                return Err(ToolError::new(
                    "plan_blocked",
                    "Resolve blocking issues and select an upgrade solution first",
                ));
            }
            backend.upgrades.take(&context.session_id, id, revision)?;
            Ok(jobs::summary(
                theseus::instance::execute_instance_upgrade(
                    &stored.native_id,
                    create_full_backup,
                    theseus::install::SharedUpgradeMode::Direct,
                    Default::default(),
                )
                .await
                .map_err(business_error)?,
            ))
        }
        _ => unreachable!(),
    }
}

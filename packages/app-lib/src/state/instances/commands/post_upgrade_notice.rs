use sqlx::{SqliteConnection, SqlitePool};

use crate::state::instances::adapters::sqlite::post_upgrade_notices;

use crate::state::{InstancePostUpgradeNotice, InstancePostUpgradeWarning};

pub(crate) async fn get_instance_post_upgrade_notice(
    instance_id: &str,
    pool: &SqlitePool,
) -> crate::Result<Option<InstancePostUpgradeNotice>> {
    post_upgrade_notices::get_post_upgrade_notice(instance_id, pool).await
}

pub(crate) async fn replace_instance_post_upgrade_notice(
    notice: &InstancePostUpgradeNotice,
    pool: &SqlitePool,
) -> crate::Result<()> {
    post_upgrade_notices::replace_post_upgrade_notice(notice, pool).await
}

pub(crate) async fn replace_instance_post_upgrade_notice_on_connection(
    notice: &InstancePostUpgradeNotice,
    connection: &mut SqliteConnection,
) -> crate::Result<()> {
    post_upgrade_notices::replace_post_upgrade_notice(notice, connection).await
}

pub(crate) async fn dismiss_instance_post_upgrade_notice(
    instance_id: &str,
    pool: &SqlitePool,
) -> crate::Result<()> {
    post_upgrade_notices::dismiss_post_upgrade_notice(instance_id, pool).await
}

pub(crate) fn next_post_upgrade_notice_after_launch(
    mut notice: InstancePostUpgradeNotice,
    clean: bool,
) -> Option<InstancePostUpgradeNotice> {
    if clean {
        notice.consecutive_clean_launches =
            notice.consecutive_clean_launches.saturating_add(1);
        (notice.consecutive_clean_launches < 2).then_some(notice)
    } else {
        notice.consecutive_clean_launches = 0;
        Some(notice)
    }
}

pub(crate) async fn record_instance_post_upgrade_launch(
    instance_id: &str,
    clean: bool,
    pool: &SqlitePool,
) -> crate::Result<()> {
    let Some(notice) =
        get_instance_post_upgrade_notice(instance_id, pool).await?
    else {
        return Ok(());
    };
    match next_post_upgrade_notice_after_launch(notice, clean) {
        Some(notice) => {
            replace_instance_post_upgrade_notice(&notice, pool).await
        }
        None => dismiss_instance_post_upgrade_notice(instance_id, pool).await,
    }
}

pub(crate) fn post_upgrade_warnings_from_result(
    result: &crate::install::InstanceUpgradeResult,
    execution: &crate::install::InstanceUpgradeExecution,
) -> Vec<InstancePostUpgradeWarning> {
    use crate::state::{InstanceUpgradeAction, InstanceUpgradeIssueCode};

    result
        .compatibility_warning_details
        .iter()
        .filter_map(|warning| {
            if warning.content_id.is_none() && warning.relative_path.is_none() {
                return None;
            }
            let action = warning
                .content_id
                .as_ref()
                .and_then(|content_id| {
                    result
                        .solution
                        .selections
                        .iter()
                        .find(|selection| selection.content_id == *content_id)
                })
                .or_else(|| {
                    let provider = warning.provider?;
                    let project_id = warning.project_id.as_deref()?;
                    let mut matches =
                        result.solution.selections.iter().filter(|selection| {
                            selection.provider == Some(provider)
                                && selection.project_id.as_deref()
                                    == Some(project_id)
                        });
                    let selection = matches.next()?;
                    matches.next().is_none().then_some(selection)
                })
                .map(|selection| selection.action);
            let action = action.or_else(|| {
                let item = warning
                    .content_id
                    .as_ref()
                    .and_then(|content_id| {
                        execution
                            .items
                            .iter()
                            .find(|item| item.content_id == *content_id)
                    })
                    .or_else(|| {
                        let relative_path = warning.relative_path.as_deref()?;
                        execution
                            .items
                            .iter()
                            .find(|item| item.relative_path == relative_path)
                    })?;
                Some(execution.final_physical_decision(item).0)
            });
            let code = match action {
                Some(
                    InstanceUpgradeAction::Upgrade
                    | InstanceUpgradeAction::Disable,
                ) => {
                    return None;
                }
                Some(InstanceUpgradeAction::Keep) => match warning.code {
                    InstanceUpgradeIssueCode::PrereleaseOnly
                    | InstanceUpgradeIssueCode::DependencyConflict
                    | InstanceUpgradeIssueCode::MissingRequiredDependency
                    | InstanceUpgradeIssueCode::IncompatibleDependency
                    | InstanceUpgradeIssueCode::SearchLimitReached => {
                        InstanceUpgradeIssueCode::KeepIncompatible
                    }
                    code => code,
                },
                None => match warning.code {
                    InstanceUpgradeIssueCode::Unidentified
                    | InstanceUpgradeIssueCode::UnsupportedContentType
                    | InstanceUpgradeIssueCode::KeepIncompatible => {
                        warning.code
                    }
                    _ => return None,
                },
            };
            Some(InstancePostUpgradeWarning {
                code,
                content_id: warning.content_id.clone(),
                relative_path: warning.relative_path.clone(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install::{
        InstanceUpgradeCompatibilityWarning, InstanceUpgradeExecution,
        InstanceUpgradeResult,
    };
    use crate::state::{
        InstanceUpgradeAction, InstanceUpgradeEnvironment,
        InstanceUpgradeIssueCode, InstanceUpgradeItem,
        InstanceUpgradeItemStatus, InstanceUpgradeResolution,
        InstanceUpgradeSelection, InstanceUpgradeSolution,
        InstanceUpgradeSolutionKind, ModLoader, ProjectType, ShaderRuntime,
    };

    fn empty_execution() -> InstanceUpgradeExecution {
        let environment = InstanceUpgradeEnvironment {
            game_version: "1.21.9".to_string(),
            mod_loader: ModLoader::Fabric,
            mod_loader_version: Some("0.18.5".to_string()),
            shader_runtime: ShaderRuntime::Iris,
        };
        InstanceUpgradeExecution {
            source_revision: 1,
            source_files: Vec::new(),
            source_environment: environment.clone(),
            target_environment: environment,
            items: Vec::new(),
            solution: InstanceUpgradeSolution {
                kind: InstanceUpgradeSolutionKind::Custom,
                selections: Vec::new(),
                dependency_changes: Vec::new(),
                warnings: Vec::new(),
            },
            warnings: Vec::new(),
            source_watch: None,
        }
    }

    fn upgrade_result(
        entries: &[(
            &str,
            InstanceUpgradeIssueCode,
            InstanceUpgradeAction,
            Option<&str>,
        )],
    ) -> InstanceUpgradeResult {
        InstanceUpgradeResult {
            plan_id: "plan".to_string(),
            source_instance_id: "instance".to_string(),
            target_instance_id: "instance".to_string(),
            backup_instance_id: None,
            source_environment: None,
            target_environment: None,
            solution: InstanceUpgradeSolution {
                kind: InstanceUpgradeSolutionKind::Custom,
                selections: entries
                    .iter()
                    .map(|(content_id, _, action, target_release_id)| {
                        InstanceUpgradeSelection {
                            content_id: (*content_id).to_string(),
                            provider: None,
                            project_id: None,
                            current_release_id: None,
                            target_release_id: target_release_id
                                .map(str::to_string),
                            action: *action,
                            enabled: *action != InstanceUpgradeAction::Disable,
                        }
                    })
                    .collect(),
                dependency_changes: Vec::new(),
                warnings: Vec::new(),
            },
            compatibility_warnings: Vec::new(),
            compatibility_warning_details: entries
                .iter()
                .map(|(content_id, code, _, _)| {
                    InstanceUpgradeCompatibilityWarning {
                        code: *code,
                        relative_path: Some(format!("mods/{content_id}.jar")),
                        content_id: Some((*content_id).to_string()),
                        provider: None,
                        project_id: None,
                        conflicting_project_id: None,
                    }
                })
                .collect(),
            external_changes: Vec::new(),
            skipped_due_to_external_conflict: Vec::new(),
        }
    }

    fn notice(clean_launches: u8) -> InstancePostUpgradeNotice {
        InstancePostUpgradeNotice {
            instance_id: "instance".to_string(),
            upgrade_job_id: "job".to_string(),
            target_game_version: "26.2".to_string(),
            consecutive_clean_launches: clean_launches,
            warnings: vec![InstancePostUpgradeWarning {
                code: InstanceUpgradeIssueCode::KeepIncompatible,
                content_id: Some("content".to_string()),
                relative_path: Some("mods/example.jar".to_string()),
            }],
        }
    }

    #[test]
    fn clean_launch_expires_notice_after_two_consecutive_sessions() {
        let first = next_post_upgrade_notice_after_launch(notice(0), true)
            .expect("first launch keeps notice");
        assert_eq!(first.consecutive_clean_launches, 1);
        assert!(next_post_upgrade_notice_after_launch(first, true).is_none());
    }

    #[test]
    fn failed_launch_resets_consecutive_count() {
        let reset = next_post_upgrade_notice_after_launch(notice(1), false)
            .expect("failed launch keeps notice");
        assert_eq!(reset.consecutive_clean_launches, 0);
    }

    #[test]
    fn upgraded_prerelease_history_is_not_a_post_upgrade_risk() {
        let result = upgrade_result(&[(
            "voxy",
            InstanceUpgradeIssueCode::PrereleaseOnly,
            InstanceUpgradeAction::Upgrade,
            Some("target-alpha"),
        )]);

        assert!(
            post_upgrade_warnings_from_result(&result, &empty_execution())
                .is_empty()
        );
        assert_eq!(result.compatibility_warning_details.len(), 1);
    }

    #[test]
    fn kept_prerelease_is_reported_as_incompatible_preserved_content() {
        let result = upgrade_result(&[(
            "voxy",
            InstanceUpgradeIssueCode::PrereleaseOnly,
            InstanceUpgradeAction::Keep,
            None,
        )]);

        let warnings =
            post_upgrade_warnings_from_result(&result, &empty_execution());
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0].code,
            InstanceUpgradeIssueCode::KeepIncompatible
        );
    }

    #[test]
    fn kept_no_release_and_unidentified_content_remain_risks() {
        let result = upgrade_result(&[
            (
                "resource-pack",
                InstanceUpgradeIssueCode::NoCompatibleRelease,
                InstanceUpgradeAction::Keep,
                None,
            ),
            (
                "local-jar",
                InstanceUpgradeIssueCode::Unidentified,
                InstanceUpgradeAction::Keep,
                None,
            ),
        ]);

        let warnings =
            post_upgrade_warnings_from_result(&result, &empty_execution());
        assert_eq!(warnings.len(), 2);
        assert!(warnings.iter().any(|warning| {
            warning.code == InstanceUpgradeIssueCode::NoCompatibleRelease
        }));
        assert!(warnings.iter().any(|warning| {
            warning.code == InstanceUpgradeIssueCode::Unidentified
        }));
    }

    #[test]
    fn disabled_content_is_not_a_post_upgrade_risk() {
        let result = upgrade_result(&[(
            "disabled",
            InstanceUpgradeIssueCode::NoCompatibleRelease,
            InstanceUpgradeAction::Disable,
            None,
        )]);

        assert!(
            post_upgrade_warnings_from_result(&result, &empty_execution())
                .is_empty()
        );
    }

    #[test]
    fn global_history_warning_is_not_a_content_notice() {
        let mut result = upgrade_result(&[(
            "global",
            InstanceUpgradeIssueCode::KeepIncompatible,
            InstanceUpgradeAction::Keep,
            None,
        )]);
        result.compatibility_warning_details[0].content_id = None;
        result.compatibility_warning_details[0].relative_path = None;

        assert!(
            post_upgrade_warnings_from_result(&result, &empty_execution())
                .is_empty()
        );
    }

    #[test]
    fn mixed_final_actions_only_report_kept_content() {
        let result = upgrade_result(&[
            (
                "upgraded",
                InstanceUpgradeIssueCode::PrereleaseOnly,
                InstanceUpgradeAction::Upgrade,
                Some("target-alpha"),
            ),
            (
                "kept",
                InstanceUpgradeIssueCode::NoCompatibleRelease,
                InstanceUpgradeAction::Keep,
                None,
            ),
            (
                "disabled",
                InstanceUpgradeIssueCode::NoCompatibleRelease,
                InstanceUpgradeAction::Disable,
                None,
            ),
        ]);

        let warnings =
            post_upgrade_warnings_from_result(&result, &empty_execution());
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].content_id.as_deref(), Some("kept"));
    }

    #[test]
    fn non_solver_local_notice_uses_execution_item_resolution() {
        let mut result = upgrade_result(&[(
            "local",
            InstanceUpgradeIssueCode::Unidentified,
            InstanceUpgradeAction::Keep,
            None,
        )]);
        result.solution.selections.clear();
        let mut execution = empty_execution();
        execution.items.push(InstanceUpgradeItem {
            content_id: "local".to_string(),
            relative_path: "mods/local.jar".to_string(),
            project_type: ProjectType::Mod,
            provider: None,
            project_id: None,
            current_release_id: None,
            current_enabled: true,
            auto_dependency: false,
            status: InstanceUpgradeItemStatus::Unidentified,
            resolution: InstanceUpgradeResolution {
                content_id: "local".to_string(),
                action: InstanceUpgradeAction::Keep,
                allow_prerelease: false,
                confirmed_prerelease_dependencies: Vec::new(),
            },
            candidate_release_ids: Vec::new(),
        });

        assert_eq!(
            post_upgrade_warnings_from_result(&result, &execution).len(),
            1
        );
        execution.items[0].resolution.action = InstanceUpgradeAction::Disable;
        assert!(
            post_upgrade_warnings_from_result(&result, &execution).is_empty()
        );
    }

    #[tokio::test]
    async fn dismiss_removes_persisted_notice() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query("CREATE TABLE instances (id TEXT PRIMARY KEY NOT NULL)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO instances (id) VALUES ('instance')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE instance_post_upgrade_notices (instance_id TEXT PRIMARY KEY NOT NULL REFERENCES instances(id) ON DELETE CASCADE, upgrade_job_id TEXT NOT NULL, target_game_version TEXT NOT NULL, consecutive_clean_launches INTEGER NOT NULL DEFAULT 0, warnings_json TEXT NOT NULL, created TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, modified TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP)",
        )
        .execute(&pool)
        .await
        .unwrap();

        replace_instance_post_upgrade_notice(&notice(0), &pool)
            .await
            .unwrap();
        assert!(
            get_instance_post_upgrade_notice("instance", &pool)
                .await
                .unwrap()
                .is_some()
        );
        dismiss_instance_post_upgrade_notice("instance", &pool)
            .await
            .unwrap();
        assert!(
            get_instance_post_upgrade_notice("instance", &pool)
                .await
                .unwrap()
                .is_none()
        );
    }
}

use super::{LauncherBackend, business_error, files::paths, page, reviewed};
use launcher_tools::{BackendContext, Command, ToolError};
use serde_json::{Value, json};
use theseus::{data::CoreComponent, instance as api};
fn summary(c: CoreComponent) -> Value {
    json!({"component_id":c.id,"kind":c.kind,"file_name":c.file_name,"enabled":c.enabled,"removed":c.removed,"order":c.order,"sha1":c.sha1,"sha256":c.sha256,"target_game_version":c.target_game_version,"has_failure":c.failure_reason.is_some(),"source":c.source.map(|s|json!({"provider":s.provider,"project_id":s.project_id,"version_id":s.version_id,"file_id":s.file_id}))})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn component_summary_omits_paths_urls_and_native_error_details() {
        let component = CoreComponent {
            id: "id".into(),
            kind: theseus::data::CoreComponentKind::JarMod,
            file_name: "mod.jar".into(),
            relative_path: "private/internal".into(),
            enabled: true,
            removed: false,
            order: 0,
            sha1: None,
            sha256: None,
            source: Some(theseus::data::CoreComponentSource {
                provider: "source".into(),
                project_id: None,
                version_id: None,
                file_id: None,
                page_url: Some("https://example.com/?secret=token".into()),
            }),
            target_game_version: "1.2.5".into(),
            created_at: chrono::Utc::now(),
            modified_at: chrono::Utc::now(),
            failure_reason: Some("private error details".into()),
        };
        let result = summary(component);
        assert_eq!(result["has_failure"], true);
        for private in ["private/", "private error", "secret=token"] {
            assert!(!result.to_string().contains(private));
        }
    }
}
pub(super) async fn execute(
    _: &LauncherBackend,
    command: Command,
    _: BackendContext,
) -> Result<Value, ToolError> {
    let id = match &command {
        Command::CoreListComponents { instance_id, .. }
        | Command::CorePreviewJar { instance_id }
        | Command::CoreAddJarMod { instance_id, .. }
        | Command::CoreReplaceJar { instance_id, .. }
        | Command::CoreSetEnabled { instance_id, .. }
        | Command::CoreMoveComponent { instance_id, .. }
        | Command::CoreRemoveComponent { instance_id, .. }
        | Command::CoreRestoreComponent { instance_id, .. }
        | Command::CoreInstallMcarchiveLoader { instance_id }
        | Command::CoreImportMcarchiveLoader { instance_id, .. } => instance_id,
        _ => return Err(paths::error("command")),
    };
    let current = if matches!(command, Command::CoreListComponents { .. }) {
        api::get(id)
            .await
            .map_err(business_error)?
            .ok_or_else(|| paths::error("instance"))?
    } else {
        reviewed::instance(id).await?
    };
    let instance_root = paths::root("instance", id, false).await?;
    tokio::task::spawn_blocking(move || {
        let dir = paths::open_root(&instance_root)?;
        match dir.symlink_metadata("core-components") {
            Ok(metadata) => {
                if !metadata.is_dir() || metadata.file_type().is_symlink() {
                    return Err(paths::error("linked core components"));
                }
                let _ = paths::directory(&dir, "core-components")?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(paths::error(e)),
        }
        Ok(())
    })
    .await
    .map_err(paths::error)??;
    match command {
        Command::CoreListComponents {
            instance_id,
            offset,
            limit,
        } => page(
            api::list_core_components(&instance_id)
                .await
                .map_err(business_error)?
                .into_iter()
                .map(summary)
                .collect(),
            offset.unwrap_or(0),
            limit.unwrap_or(50),
        ),
        Command::CorePreviewJar { instance_id } => Ok(
            match api::preview_core_jar(&instance_id)
                .await
                .map_err(business_error)?
            {
                Some(p) => {
                    json!({"assembled":true,"component_count":p.component_count,"replacement_component_id":p.replacement_component_id,"entries":p.entries,"sha1":p.sha1,"sha256":p.sha256})
                }
                None => json!({"assembled":false}),
            },
        ),
        c @ (Command::CoreAddJarMod { .. }
        | Command::CoreReplaceJar { .. }
        | Command::CoreImportMcarchiveLoader { .. }) => {
            let add = matches!(c, Command::CoreAddJarMod { .. });
            let import = matches!(c, Command::CoreImportMcarchiveLoader { .. });
            let (Command::CoreAddJarMod {
                instance_id,
                path,
                expected_sha256,
            }
            | Command::CoreReplaceJar {
                instance_id,
                path,
                expected_sha256,
            }
            | Command::CoreImportMcarchiveLoader {
                instance_id,
                path,
                expected_sha256,
            }) = c
            else {
                unreachable!()
            };
            let payload =
                paths::payload(&instance_id, &path, &expected_sha256).await?;
            if import {
                return Ok(mcarchive(
                    api::import_mcarchive_modloader(
                        &instance_id,
                        &current.applied_content_set.game_version,
                        payload.path().to_owned(),
                    )
                    .await
                    .map_err(business_error)?,
                ));
            }
            let c = if add {
                api::add_core_jar_mod(
                    &instance_id,
                    payload.path().to_owned(),
                    current.applied_content_set.game_version,
                    None,
                )
                .await
            } else {
                api::replace_core_jar(
                    &instance_id,
                    payload.path().to_owned(),
                    current.applied_content_set.game_version,
                    None,
                )
                .await
            }
            .map_err(business_error)?;
            Ok(summary(c))
        }
        Command::CoreSetEnabled {
            instance_id,
            component_id,
            enabled,
        } => Ok(summary(
            api::set_core_component_enabled(
                &instance_id,
                &component_id,
                enabled,
            )
            .await
            .map_err(business_error)?,
        )),
        Command::CoreMoveComponent {
            instance_id,
            component_id,
            direction,
        } => Ok(
            json!({"components":api::move_core_component(&instance_id,&component_id,if direction=="up"{-1}else{1}).await.map_err(business_error)?.into_iter().map(summary).collect::<Vec<_>>()}),
        ),
        Command::CoreRemoveComponent {
            instance_id,
            component_id,
        } => {
            api::remove_core_component(&instance_id, &component_id)
                .await
                .map_err(business_error)?;
            Ok(json!({"removed":true,"component_id":component_id}))
        }
        Command::CoreRestoreComponent {
            instance_id,
            component_id,
        } => Ok(summary(
            api::restore_core_component(&instance_id, &component_id)
                .await
                .map_err(business_error)?,
        )),
        Command::CoreInstallMcarchiveLoader { instance_id } => Ok(mcarchive(
            api::install_mcarchive_modloader(
                &instance_id,
                &current.applied_content_set.game_version,
            )
            .await
            .map_err(business_error)?,
        )),
        _ => Err(paths::error("command")),
    }
}
fn mcarchive(result: api::McArchiveCoreInstallResult) -> Value {
    match result {
        api::McArchiveCoreInstallResult::Installed { component } => {
            json!({"state":"installed","component":summary(component)})
        }
        api::McArchiveCoreInstallResult::ManualDownload {
            file_name,
            page_url,
            expected_sha256,
        } => {
            json!({"state":"manual_download","file_name":file_name,"page_url":page_url,"expected_sha256":expected_sha256})
        }
    }
}

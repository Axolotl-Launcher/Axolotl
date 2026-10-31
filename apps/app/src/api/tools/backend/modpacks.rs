use super::{LauncherBackend, business_error, jobs, resources, reviewed};
use launcher_tools::{BackendContext, Command, Source, ToolError};
use serde_json::{Value, json};
use theseus::{
    data::{InstanceLink, ModLoader},
    pack::install_from::CreatePackLocation,
    resources::*,
};
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub(super) struct PackPlan {
    kind: PackKind,
    name: String,
    target: Option<(String, String)>,
}
#[derive(Clone)]
enum PackKind {
    Modrinth {
        project_id: String,
        release_id: String,
    },
    Curseforge {
        project_id: u32,
        file_id: u32,
        game: String,
        loader: ModLoader,
        loader_version: Option<String>,
    },
    Local {
        path: std::path::PathBuf,
        hash: String,
    },
}

async fn target(
    id: Option<String>,
) -> Result<Option<(String, String)>, ToolError> {
    match id {
        Some(id) => {
            let metadata = reviewed::instance(&id).await?;
            Ok(Some((id, reviewed::fingerprint(&metadata)?)))
        }
        None => Ok(None),
    }
}
fn numeric(value: &str) -> Result<u32, ToolError> {
    value
        .parse::<u32>()
        .ok()
        .filter(|id| *id > 0)
        .ok_or_else(|| {
            ToolError::new(
                "invalid_provider_id",
                "CurseForge IDs must be positive numeric IDs",
            )
        })
}

pub(super) async fn execute(
    backend: &LauncherBackend,
    command: Command,
    context: BackendContext,
) -> Result<Value, ToolError> {
    match command {
        Command::ModpackPrepareInstall {
            source,
            project_id,
            release_id,
            name,
            instance_id,
        } => {
            let target = target(instance_id).await?;
            let service = resources::service().await?;
            let project = resources::project(source, project_id.clone())?;
            let cancel = CancellationToken::new();
            let metadata = service
                .get(
                    &ResourceRef::Source {
                        project: project.clone(),
                    },
                    None,
                    None,
                    &cancel,
                )
                .await
                .map_err(resources::error)?;
            if !metadata.projects.iter().any(|p| {
                p.project == project && p.types.contains(&ResourceType::ModPack)
            }) {
                return Err(ToolError::new(
                    "not_modpack",
                    "Project must be a modpack",
                ));
            }
            let reference = SourceReleaseRef {
                project,
                release_id: SourceReleaseId::new(release_id.clone()).map_err(
                    |_| ToolError::new("invalid_release", "Invalid release ID"),
                )?,
            };
            let release = service
                .release(&reference, None, &cancel)
                .await
                .map_err(resources::error)?;
            let mut summary = json!({"source":reference.project.source,"project_id":project_id,"release_id":release_id,"declared_compatibility":release.compatibility});
            let kind = match source {
                Source::Modrinth => {
                    if !release.artifacts.iter().any(|a| {
                        a.available != Some(false)
                            && a.file_name.as_ref().is_some_and(|name| {
                                name.to_ascii_lowercase().ends_with(".mrpack")
                            })
                    }) {
                        return Err(ToolError::new(
                            "pack_archive_missing",
                            "Release has no available mrpack archive",
                        ));
                    }
                    PackKind::Modrinth {
                        project_id,
                        release_id,
                    }
                }
                Source::Curseforge => {
                    let project = numeric(&project_id)?;
                    let file_id = numeric(&release_id)?;
                    if let Some((id, _)) = &target {
                        let current = reviewed::instance(id).await?;
                        if !matches!(&current.link, InstanceLink::CurseForgeModpack { project_id: current, .. } if current == &project_id)
                        {
                            return Err(ToolError::new(
                                "pack_project_mismatch",
                                "Existing CurseForge target must already be managed by this project",
                            ));
                        }
                    }
                    let pack = theseus::curseforge::get_modpack_target(
                        project, file_id,
                    )
                    .await
                    .map_err(business_error)?;
                    summary["game_version"] = json!(pack.game_version);
                    summary["loader"] = json!(pack.loader);
                    summary["loader_version"] = json!(pack.loader_version);
                    PackKind::Curseforge {
                        project_id: project,
                        file_id,
                        game: pack.game_version,
                        loader: pack.loader,
                        loader_version: pack.loader_version,
                    }
                }
                _ => {
                    return Err(ToolError::new(
                        "unsupported_source",
                        "Use Modrinth or CurseForge",
                    ));
                }
            };
            let instance_id = target.as_ref().map(|(id, _)| id.clone());
            let handle = backend.modpacks.insert(
                &context.session_id,
                1,
                PackPlan {
                    kind,
                    name: name.trim().into(),
                    target,
                },
            )?;
            Ok(
                json!({"plan_id":handle,"name":name,"instance_id":instance_id,"creates_instance":instance_id.is_none(),"replaces_pack_content":instance_id.is_some(),"pack":summary}),
            )
        }
        Command::ModpackPrepareLocal {
            path,
            name,
            instance_id,
        } => {
            let target = target(instance_id).await?;
            let path = std::path::PathBuf::from(path);
            let hash = reviewed::archive(&path).await?;
            let detected = theseus::pack::detect::detect_local_pack(&path)
                .await
                .map_err(business_error)?;
            let preview = theseus::pack::install_from::get_instance_from_pack(
                CreatePackLocation::FromFile { path: path.clone() },
            )
            .await
            .map_err(business_error)?;
            if reviewed::archive(&path).await? != hash {
                return Err(ToolError::new(
                    "archive_changed",
                    "Archive changed during preview",
                ));
            }
            let instance_id = target.as_ref().map(|(id, _)| id.clone());
            let summary = json!({"format":detected.format.as_str(),"game_version":preview.game_version,"loader":preview.modloader,"loader_version":preview.loader_version});
            let handle = backend.modpacks.insert(
                &context.session_id,
                1,
                PackPlan {
                    kind: PackKind::Local { path, hash },
                    name: name.trim().into(),
                    target,
                },
            )?;
            Ok(
                json!({"plan_id":handle,"name":name,"instance_id":instance_id,"creates_instance":instance_id.is_none(),"replaces_pack_content":instance_id.is_some(),"pack":summary}),
            )
        }
        Command::ModpackExecuteInstall { plan_id } => {
            let plan = backend.modpacks.get(&context.session_id, plan_id, 1)?;
            if let Some((id, expected)) = &plan.target {
                reviewed::unchanged(id, expected).await?;
            }
            if let PackKind::Local { path, hash } = &plan.kind {
                if &reviewed::archive(path).await? != hash {
                    return Err(ToolError::new(
                        "archive_changed",
                        "Archive changed after preview",
                    ));
                }
            }
            let plan =
                backend.modpacks.take(&context.session_id, plan_id, 1)?;
            let job = match plan.kind {
                PackKind::Curseforge {
                    project_id,
                    file_id,
                    game,
                    loader,
                    loader_version,
                } => {
                    if let Some((id, _)) = plan.target {
                        theseus::install::update_managed_curseforge_modpack(
                            id, file_id,
                        )
                        .await
                    } else {
                        theseus::install::create_instance(
                            plan.name,
                            game,
                            loader,
                            loader_version,
                            None,
                            InstanceLink::CurseForgeModpack {
                                project_id: project_id.to_string(),
                                version_id: file_id.to_string(),
                            },
                        )
                        .await
                    }
                }
                kind => {
                    let location = match kind {
                        PackKind::Modrinth {
                            project_id,
                            release_id,
                        } => CreatePackLocation::FromVersionId {
                            project_id,
                            version_id: release_id,
                            title: plan.name.clone(),
                            icon_url: None,
                        },
                        PackKind::Local { path, .. } => {
                            CreatePackLocation::FromFile { path }
                        }
                        _ => unreachable!(),
                    };
                    let edit = Some(theseus::install::InstallPostInstallEdit {
                        name: Some(plan.name),
                        icon_path: None,
                        link: None,
                    });
                    if let Some((id, _)) = plan.target {
                        theseus::install::install_pack_to_existing_instance(
                            id, location, edit,
                        )
                        .await
                    } else {
                        theseus::install::create_modpack_instance(
                            location, edit,
                        )
                        .await
                    }
                }
            }
            .map_err(business_error)?;
            Ok(jobs::summary(job))
        }
        _ => Err(ToolError::new(
            "unsupported_command",
            "Not a modpack operation",
        )),
    }
}

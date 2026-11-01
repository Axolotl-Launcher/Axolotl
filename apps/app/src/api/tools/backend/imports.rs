use super::{LauncherBackend, business_error, jobs, reviewed};
use launcher_tools::{BackendContext, Command, ToolError};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use theseus::{
    install::ImportPlanRequest,
    pack::import::{self, ImportLauncherType},
};

#[derive(Clone)]
pub(super) struct ImportSource {
    request: ImportPlanRequest,
    identity: String,
}

fn launcher(value: &str) -> Result<ImportLauncherType, ToolError> {
    Ok(match value {
        "axolotl" => ImportLauncherType::Axolotl,
        "multimc" => ImportLauncherType::MultiMC,
        "prism" => ImportLauncherType::PrismLauncher,
        "atlauncher" => ImportLauncherType::ATLauncher,
        "gdlauncher" => ImportLauncherType::GDLauncher,
        "curseforge" => ImportLauncherType::Curseforge,
        "modrinth" => ImportLauncherType::ModrinthApp,
        "pcl2" => ImportLauncherType::PCL2,
        "pcl2ce" => ImportLauncherType::PCL2CE,
        "hmcl" => ImportLauncherType::HMCL,
        "generic" => ImportLauncherType::Generic,
        _ => {
            return Err(ToolError::new(
                "invalid_launcher",
                "Unknown import launcher",
            ));
        }
    })
}

async fn identity(path: &Path) -> Result<String, ToolError> {
    let metadata = tokio::fs::symlink_metadata(path).await.map_err(|_| {
        ToolError::new("source_unavailable", "Import directory is unavailable")
    })?;
    if !path.is_absolute() || !metadata.is_dir() || reviewed::linked(&metadata)
    {
        return Err(ToolError::new(
            "invalid_import_source",
            "Use an absolute real import directory",
        ));
    }
    let canonical = tokio::fs::canonicalize(path).await.map_err(|_| {
        ToolError::new("source_unavailable", "Cannot resolve import directory")
    })?;
    reviewed::fingerprint(
        &json!({"canonical":canonical,"created":metadata.created().ok(),"modified":metadata.modified().ok()}),
    )
}

pub(super) async fn execute(
    backend: &LauncherBackend,
    command: Command,
    context: BackendContext,
) -> Result<Value, ToolError> {
    match command {
        Command::InstanceScanImports {
            launcher: kind,
            base_path,
            offset,
            limit,
        } => {
            let launcher_type = launcher(&kind)?;
            let base = base_path.map(PathBuf::from).or_else(|| import::get_default_launcher_path(launcher_type)).ok_or_else(|| ToolError::new("launcher_path_missing", "Default launcher location not found; supply an absolute base_path"))?;
            identity(&base).await?;
            let mut found =
                import::get_importable_instances(launcher_type, base.clone())
                    .await
                    .map_err(business_error)?;
            found.sort_by(|a, b| a.path.cmp(&b.path).then(a.name.cmp(&b.name)));
            let total = found.len();
            let mut items = Vec::new();
            for candidate in found.into_iter().skip(offset).take(limit) {
                let path = PathBuf::from(
                    candidate.version_path.as_ref().unwrap_or(&candidate.path),
                );
                let source_identity = identity(&path).await?;
                let request = ImportPlanRequest {
                    request_id: uuid::Uuid::new_v4().to_string(),
                    launcher_type,
                    base_path: base.clone(),
                    instance_folder: candidate.name.clone(),
                    instance_path: Some(path.to_string_lossy().into_owned()),
                    game_version: None,
                    loader: None,
                    loader_version: None,
                };
                let id = backend.import_sources.insert(
                    &context.session_id,
                    1,
                    ImportSource {
                        request,
                        identity: source_identity,
                    },
                )?;
                items.push(json!({"source_id":id,"name":candidate.name,"path":candidate.path,"compatible_mode":candidate.compatible_mode}));
            }
            let end = offset.saturating_add(items.len());
            Ok(
                json!({"items":items,"total":total,"next_offset":(end < total).then_some(end)}),
            )
        }
        Command::InstancePrepareImport { source_id } => {
            let source = backend.import_sources.get(
                &context.session_id,
                source_id,
                1,
            )?;
            let path =
                Path::new(source.request.instance_path.as_deref().unwrap());
            if identity(path).await? != source.identity {
                return Err(ToolError::new(
                    "source_changed",
                    "Import source changed; scan again",
                ));
            }
            let snapshot = theseus::install::import_plan::inspect_import_plan(
                &source.request,
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .map_err(business_error)?;
            if identity(path).await? != source.identity {
                return Err(ToolError::new(
                    "source_changed",
                    "Import source changed during analysis; scan again",
                ));
            }
            let handle =
                backend
                    .import_plans
                    .insert(&context.session_id, 1, source)?;
            Ok(
                json!({"plan_id":handle,"game_version":snapshot.game_version,"loader":snapshot.loader,"loader_version":snapshot.loader_version,"mod_count":snapshot.mod_count,"cache":snapshot.cache,"local":snapshot.local,"network":snapshot.network,"migrate":snapshot.migrate,"mode":"copy","source_contents_frozen":false}),
            )
        }
        Command::InstanceExecuteImport { plan_id } => {
            let source =
                backend.import_plans.get(&context.session_id, plan_id, 1)?;
            if identity(Path::new(
                source.request.instance_path.as_deref().unwrap(),
            ))
            .await?
                != source.identity
            {
                return Err(ToolError::new(
                    "source_changed",
                    "Import directory changed after review; scan again",
                ));
            }
            let source =
                backend.import_plans.take(&context.session_id, plan_id, 1)?;
            let request = source.request;
            Ok(jobs::summary(
                theseus::install::import_instance_with_plan(
                    request.launcher_type,
                    request.base_path,
                    request.instance_folder,
                    request.instance_path,
                    false,
                    request.game_version,
                    request.loader,
                    request.loader_version,
                    None,
                )
                .await
                .map_err(business_error)?,
            ))
        }
        _ => Err(ToolError::new(
            "unsupported_command",
            "Not an import operation",
        )),
    }
}

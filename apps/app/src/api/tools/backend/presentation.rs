use super::{LauncherBackend, business_error, modpacks, resources};
use futures::{StreamExt, future::BoxFuture, stream};
use launcher_tools::{Command, ToolError};
use serde_json::{Value, json};
use std::{collections::HashMap, sync::Arc, time::Duration};
use theseus::resources::{
    ResourceOperationPlan, ResourceRef, ResourceService, SourceReleaseRef,
};
use tokio_util::sync::CancellationToken;

pub(super) async fn describe(
    backend: &LauncherBackend,
    name: &str,
    arguments: &Value,
    scope: &str,
    cancel: &CancellationToken,
) -> Result<Option<Value>, ToolError> {
    if !matches!(
        name,
        "resource_execute_plan"
            | "resource_execute_changes"
            | "modpack_execute_install"
    ) {
        return Ok(None);
    }
    let command: Command =
        serde_json::from_value(json!({"name":name,"arguments":arguments}))
            .map_err(|_| {
                ToolError::new(
                    "invalid_arguments",
                    "Invalid installation review arguments",
                )
            })?;
    match command {
        Command::ResourceExecutePlan { plan_id, revision }
        | Command::ResourceExecuteChanges { plan_id, revision } => {
            let plan = backend.plans.get(scope, plan_id, revision)?;
            Ok(Some(resource_plan(&plan, cancel).await?))
        }
        Command::ModpackExecuteInstall { plan_id } => {
            Ok(Some(modpacks::describe(backend, scope, plan_id).await?))
        }
        _ => Ok(None),
    }
}

async fn resource_plan(
    plan: &ResourceOperationPlan,
    cancel: &CancellationToken,
) -> Result<Value, ToolError> {
    let metadata = theseus::instance::get(&plan.instance_id)
        .await
        .map_err(business_error)?
        .ok_or_else(|| {
            ToolError::new(
                "instance_not_found",
                "Review target instance no longer exists",
            )
        })?;
    let (dependencies_checked, install_dependencies) =
        theseus::resources::plans::dependency_options(&plan.id, plan.revision)
            .await
            .map_err(resources::error)?;
    // Labels are optional decoration of the frozen plan. Bound metadata lookup
    // time; a readable exact filename remains available if a provider is offline.
    let mut labels: HashMap<usize, (String, String)> = HashMap::new();
    if let Ok(service) = resources::service().await {
        let lookups = plan
            .actions
            .iter()
            .enumerate()
            .map(|(index, action)| {
                lookup_label(
                    index,
                    action.target.reference.release.clone(),
                    service.clone(),
                    cancel.clone(),
                )
            })
            .collect::<Vec<_>>();
        let mut pending = stream::iter(lookups).buffer_unordered(8);
        let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
        loop {
            tokio::select! {
                _ = cancel.cancelled() => return Err(ToolError::new("cancelled", "Installation review cancelled")),
                next = tokio::time::timeout_at(deadline, pending.next()) => match next {
                    Ok(Some((index, label))) => { labels.insert(index, label); },
                    _ => break,
                }
            }
        }
    }
    let actions = plan.actions.iter().enumerate().map(|(index, action)| {
        let reference = &action.target.reference.release;
        let (title, version) = labels.remove(&index).unwrap_or_default();
        let file = action.target.file_name.clone().unwrap_or_else(|| action.target_relative_path.clone());
        let node = plan.nodes.iter().find(|node| node.id == action.node_id);
        let root = plan.active_roots.contains(&action.node_id) || node.is_some_and(|node| node.root_selection_ids.iter().any(|id| plan.active_roots.contains(id)));
        json!({"title":if title.is_empty() {file.clone()} else {title},"version":version,"kind":action.kind,
            "source":reference.project.source,"project_id":reference.project.project_id,"release_id":reference.release_id,
            "file_name":file,"size":action.target.integrity.size,"target_path":action.target_relative_path,
            "current_path":action.current.as_ref().map(|current| &current.relative_path),"dependency":!root,"node_id":action.node_id,
            "scope":node.map(|node| &node.scope),"compatibility":node.map(|node| &node.compatibility),"declared_compatibility":node.and_then(|node| node.declared_compatibility.as_ref())})
    }).collect::<Vec<_>>();
    Ok(
        json!({"mode":"resources","intent":plan.intent,"instance":{"id":plan.instance_id,"name":metadata.instance.name,
        "game_version":metadata.applied_content_set.game_version,"loader":metadata.applied_content_set.loader},
        "dependencies_checked":dependencies_checked,"install_dependencies":install_dependencies,
        "actions":actions,"issues":plan.issues,"exceptions":plan.exceptions}),
    )
}

// Each queued lookup owns its inputs so the Tauri command future is Send.
fn lookup_label(
    index: usize,
    reference: SourceReleaseRef,
    service: Arc<ResourceService>,
    cancel: CancellationToken,
) -> BoxFuture<'static, (usize, (String, String))> {
    Box::pin(async move {
        let project = ResourceRef::Source {
            project: reference.project.clone(),
        };
        let (metadata, release) = tokio::join!(
            service.get(&project, None, None, &cancel),
            service.release(&reference, None, &cancel)
        );
        let title = metadata
            .ok()
            .and_then(|resource| {
                resource
                    .projects
                    .into_iter()
                    .find(|project| project.project == reference.project)
            })
            .map(|project| project.title.chars().take(256).collect::<String>())
            .unwrap_or_default();
        let version = release
            .ok()
            .filter(|release| release.reference == reference)
            .map(|release| {
                release.version.chars().take(128).collect::<String>()
            })
            .unwrap_or_default();
        (index, (title, version))
    })
}

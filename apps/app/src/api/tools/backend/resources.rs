use super::{LauncherBackend, business_error, page, serialize};
use launcher_tools::{
    BackendContext, Command, InstallKind, ResourceKind, Source, ToolError,
};
use serde_json::{Value, json};
use theseus::resources::{
    self as r,
    ipc::{ResourceSearchQuery, ResourceSearchSort, ResourceTarget},
    *,
};
use tokio_util::sync::CancellationToken;

fn source(value: Source) -> SourceId {
    match value {
        Source::Modrinth => SourceId::modrinth(),
        Source::Curseforge => SourceId::curseforge(),
        Source::Mcarchive => SourceId::mcarchive(),
        Source::PlanetMinecraft => SourceId::planet_minecraft(),
    }
}
pub(super) fn project(
    src: Source,
    id: String,
) -> Result<SourceProjectRef, ToolError> {
    Ok(SourceProjectRef {
        source: source(src),
        project_id: SourceProjectId::new(id).map_err(|_| {
            ToolError::new("invalid_project_id", "Invalid project ID")
        })?,
    })
}
fn kind(value: ResourceKind) -> ResourceType {
    match value {
        ResourceKind::Mod => ResourceType::Mod,
        ResourceKind::Plugin => ResourceType::Plugin,
        ResourceKind::DataPack => ResourceType::DataPack,
        ResourceKind::ResourcePack => ResourceType::ResourcePack,
        ResourceKind::Shader => ResourceType::Shader,
        ResourceKind::ModPack => ResourceType::ModPack,
        ResourceKind::World => ResourceType::World,
    }
}
pub(super) fn error(error: ResourceError) -> ToolError {
    tracing::warn!(%error, "MCP resource operation failed");
    let (code, message) = match error {
        ResourceError::CursorExpired => (
            "cursor_expired",
            "Restart the resource query without a cursor",
        ),
        ResourceError::Unsupported(_) => (
            "unsupported",
            "Resource source does not support this operation or type",
        ),
        ResourceError::Unavailable => (
            "source_unavailable",
            "Resource source is unavailable; inspect resource_list_sources",
        ),
        ResourceError::NoCompatibleRelease => (
            "no_compatible_release",
            "No compatible release found for this instance",
        ),
        ResourceError::Canceled => ("cancelled", "Resource request cancelled"),
        ResourceError::InvalidRequest(_) => (
            "invalid_resource_request",
            "Resource request is invalid, stale, blocked or incompatible; prepare a fresh plan or inspect source/project/release IDs",
        ),
        ResourceError::Network { .. } => {
            ("resource_network_error", "Resource source request failed")
        }
        _ => (
            "resource_error",
            "Resource operation failed; consult launcher diagnostics",
        ),
    };
    ToolError::new(code, message)
}
pub(super) async fn service()
-> Result<std::sync::Arc<ResourceService>, ToolError> {
    Ok(theseus::State::get()
        .await
        .map_err(business_error)?
        .resources()
        .await
        .map_err(error)?
        .clone())
}
async fn target(id: Option<&str>) -> Result<ResourceTarget, ToolError> {
    let Some(id) = id else {
        return Ok(ResourceTarget {
            game_versions: vec![],
            loaders: vec![],
            channels: vec![],
        });
    };
    let instance = theseus::instance::get(id)
        .await
        .map_err(business_error)?
        .ok_or_else(|| ToolError::new("not_found", "Instance not found"))?;
    Ok(ResourceTarget {
        game_versions: vec![instance.applied_content_set.game_version],
        loaders: vec![instance.applied_content_set.loader.as_str().into()],
        channels: vec![],
    })
}
fn clip(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}
fn project_summary(p: SourceProjectMetadata) -> Value {
    json!({"source":p.project.source,"project_id":p.project.project_id,"title":clip(&p.title,256),"summary":clip(&p.summary,1024),
        "types":p.types,"authors":p.authors.into_iter().take(10).map(|s|clip(&s,128)).collect::<Vec<_>>(),
        "game_versions":p.game_versions.into_iter().take(100).collect::<Vec<_>>(),"loaders":p.loaders,"downloads":p.downloads,"updated_at":p.updated_at})
}
fn release_summary(release: ResourceRelease) -> Value {
    let total = release.artifacts.len();
    json!({"source":release.reference.project.source,"project_id":release.reference.project.project_id,"release_id":release.reference.release_id,
        "version":clip(&release.version,256),"channel":release.channel,"published_at":release.published_at,"compatibility":release.compatibility,
        "artifact_count":total,"artifacts_truncated":total > 20,"artifacts":release.artifacts.into_iter().take(20).map(|a|json!({
            "artifact_id":a.reference.artifact_id,"file_name":a.file_name,"role":a.role,"available":a.available,"size":a.integrity.size,
            "acquisition":match a.acquisition { ArtifactAcquisition::PublicDownload { .. } => "public_download", ArtifactAcquisition::ManualDownload { .. } => "manual_download", ArtifactAcquisition::Unavailable => "unavailable" },
            "dependency_count":a.dependencies.len()
        })).collect::<Vec<_>>()})
}

fn executable(plan: &ResourceOperationPlan) -> bool {
    !plan.issues.iter().any(|issue| issue.blocking)
        && !plan.actions.is_empty()
        && plan.actions.len() <= 100
        && plan.issues.len() <= 100
        && plan.nodes.len() <= 100
}

pub(super) fn review(
    backend: &LauncherBackend,
    session: &str,
    plan: ResourceOperationPlan,
) -> Result<Value, ToolError> {
    let executable = executable(&plan);
    let handle = backend.plans.insert(session, plan.revision, plan.clone())?;
    Ok(
        json!({"plan_id":handle,"revision":plan.revision,"instance_id":plan.instance_id,"intent":plan.intent,"executable":executable,
            "actions_truncated":plan.actions.len()>100,"issues_truncated":plan.issues.len()>100,"nodes_truncated":plan.nodes.len()>100,
            "actions":plan.actions.into_iter().take(100).map(|a|json!({"kind":a.kind,"entry_id":a.entry_id,"current_path":a.current.map(|c|c.relative_path),"target_path":a.target_relative_path,"release":a.target.reference.release})).collect::<Vec<_>>(),
            "nodes":plan.nodes.into_iter().take(100).map(|node|json!({"node_id":node.id,"state":node.state,"release":node.artifact.map(|a|a.reference.release),"compatibility":node.compatibility})).collect::<Vec<_>>(),
            "issues":plan.issues.into_iter().take(100).map(|i|json!({"code":i.code,"blocking":i.blocking,"node_id":i.selection_id,"message":clip(&i.message,512)})).collect::<Vec<_>>(),
            "accepted_compatibility_exceptions":plan.exceptions
        }),
    )
}

fn override_decisions(
    plan: &ResourceOperationPlan,
    ignore_dependencies: bool,
    nodes: &[String],
) -> Result<Vec<ResourcePlanDecision>, ToolError> {
    let mut decisions = Vec::new();
    if ignore_dependencies {
        decisions.push(ResourcePlanDecision::IgnoreDependencies);
    }
    let mut seen = std::collections::HashSet::new();
    for id in nodes {
        if !seen.insert(id) {
            continue;
        }
        let node = plan
            .nodes
            .iter()
            .find(|node| node.id.as_str() == id)
            .ok_or_else(|| {
                ToolError::new(
                    "invalid_node",
                    "Compatibility node does not belong to this plan",
                )
            })?;
        let release = node
            .artifact
            .as_ref()
            .map(|a| a.reference.release.clone())
            .ok_or_else(|| {
                ToolError::new(
                    "unresolved_node",
                    "Select a release before accepting its compatibility",
                )
            })?;
        let mut dimensions = Vec::new();
        if node.compatibility.game_version != CompatibilityState::Compatible {
            dimensions.push(CompatibilityDimension::GameVersion);
        }
        if node.compatibility.loader != CompatibilityState::Compatible {
            dimensions.push(CompatibilityDimension::Loader);
        }
        if !dimensions.is_empty() {
            decisions.push(ResourcePlanDecision::AcceptCompatibility {
                exception: CompatibilityException {
                    selection_id: node.id.clone(),
                    release,
                    dimensions,
                },
            });
        }
    }
    if decisions.is_empty() {
        return Err(ToolError::new(
            "no_changes",
            "Specify ignore_dependencies or nodes with unaccepted compatibility problems",
        ));
    }
    Ok(decisions)
}

pub(super) async fn execute(
    backend: &LauncherBackend,
    command: Command,
    context: BackendContext,
) -> Result<Value, ToolError> {
    let cancel = CancellationToken::new();
    match command {
        Command::ResourceListSources {} => {
            serialize(service().await?.sources())
        }
        Command::ResourceSearch {
            source: src,
            query,
            resource_type,
            instance_id,
            cursor,
            limit,
        } => {
            let mut compatibility = target(instance_id.as_deref()).await?;
            if matches!(src, Source::PlanetMinecraft)
                || !matches!(
                    resource_type,
                    ResourceKind::Mod
                        | ResourceKind::Plugin
                        | ResourceKind::ModPack
                )
            {
                compatibility.loaders.clear();
            }
            let query = ResourceSearchQuery {
                filter: None,
                text: query,
                types: vec![kind(resource_type)],
                sources: vec![source(src)],
                target: compatibility,
                categories: vec![],
                sort: ResourceSearchSort::Relevance,
                page_size: limit,
            };
            let page = service()
                .await?
                .search(
                    query,
                    cursor,
                    RequestId::new(uuid::Uuid::new_v4().to_string()).unwrap(),
                    &cancel,
                )
                .await
                .map_err(error)?;
            Ok(
                json!({"items":page.entries.into_iter().flat_map(|entry|entry.projects).map(project_summary).collect::<Vec<_>>(),"next_cursor":page.cursor,"has_more":page.has_more,"total":page.total,"source_failures":page.source_failures}),
            )
        }
        Command::ResourceGet {
            source: src,
            project_id,
        } => {
            let project = project(src, project_id)?;
            let resource = service()
                .await?
                .get(&ResourceRef::Source { project }, None, None, &cancel)
                .await
                .map_err(error)?;
            Ok(
                json!({"projects":resource.projects.into_iter().map(project_summary).collect::<Vec<_>>(),"source_failures":resource.source_failures}),
            )
        }
        Command::ResourceListReleases {
            source: src,
            project_id,
            instance_id,
            cursor,
        } => {
            let project = project(src, project_id)?;
            let resources = service().await?;
            let reference = ResourceRef::Source { project };
            let metadata = resources
                .get(&reference, None, None, &cancel)
                .await
                .map_err(error)?;
            let mut compatibility = target(Some(&instance_id)).await?;
            if matches!(src, Source::PlanetMinecraft)
                || !metadata
                    .projects
                    .iter()
                    .flat_map(|project| &project.types)
                    .any(|kind| {
                        matches!(
                            kind,
                            ResourceType::Mod
                                | ResourceType::Plugin
                                | ResourceType::ModPack
                        )
                    })
            {
                compatibility.loaders.clear();
            }
            let page = resources
                .releases(
                    &reference,
                    compatibility,
                    cursor,
                    RequestId::new(uuid::Uuid::new_v4().to_string()).unwrap(),
                    None,
                    &cancel,
                )
                .await
                .map_err(error)?;
            Ok(
                json!({"items":page.entries.into_iter().map(release_summary).collect::<Vec<_>>(),"next_cursor":page.cursor,"has_more":page.has_more,"total":page.total,"source_failures":page.source_failures}),
            )
        }
        Command::ResourceGetRelease {
            source: src,
            project_id,
            release_id,
        } => {
            let reference = SourceReleaseRef {
                project: project(src, project_id)?,
                release_id: SourceReleaseId::new(release_id).map_err(|_| {
                    ToolError::new("invalid_release_id", "Invalid release ID")
                })?,
            };
            Ok(release_summary(
                service()
                    .await?
                    .release(&reference, None, &cancel)
                    .await
                    .map_err(error)?,
            ))
        }
        Command::ContentList {
            instance_id,
            offset,
            limit,
        } => {
            target(Some(&instance_id)).await?;
            let mut items = r::installed::list(instance_id, &cancel)
                .await
                .map_err(error)?;
            items.sort_by(|a, b| a.entry_id.cmp(&b.entry_id));
            page(items.into_iter().map(|item|json!({"entry_id":item.entry_id,"instance_id":item.instance_id,"resource_type":item.resource_type,
                "relative_path":item.relative_path,"enabled":item.enabled,"missing":item.missing,"ownership":item.ownership,
                "artifact":item.artifact,"known_artifacts":item.known_artifacts,"scope":item.scope})).collect(), offset, limit)
        }
        Command::ContentSetEnabled {
            instance_id,
            entry_id,
            enabled,
        } => {
            let results = theseus::instance::toggle_content_entries(
                &instance_id,
                vec![entry_id],
                Some(enabled),
            )
            .await
            .map_err(business_error)?;
            serialize(results)
        }
        Command::ContentRemove {
            instance_id,
            entry_id,
        } => {
            theseus::instance::remove_content_entry(&instance_id, &entry_id)
                .await
                .map_err(business_error)?;
            Ok(
                json!({"removed":true,"instance_id":instance_id,"entry_id":entry_id}),
            )
        }
        Command::ResourcePrepareInstall {
            instance_id,
            source: src,
            project_id,
            release_id,
            resource_type,
        } => {
            target(Some(&instance_id)).await?;
            let project = project(src, project_id)?;
            let reference = SourceReleaseRef {
                project: project.clone(),
                release_id: SourceReleaseId::new(release_id).map_err(|_| {
                    ToolError::new("invalid_release_id", "Invalid release ID")
                })?,
            };
            let selection = ResourceSelection {
                id: SelectionId::new(uuid::Uuid::new_v4().to_string()).unwrap(),
                resource: ResourceRef::Source { project },
                release: ReleaseSelection::Release { reference },
                resource_type: match resource_type {
                    InstallKind::Mod => ResourceType::Mod,
                    InstallKind::ResourcePack => ResourceType::ResourcePack,
                    InstallKind::Shader => ResourceType::Shader,
                },
                scope: InstallScope::Instance,
                entry_id: None,
            };
            let plan = r::plans::prepare(
                instance_id,
                ResourceOperationIntent::Install,
                vec![selection],
                true,
                &cancel,
            )
            .await
            .map_err(error)?;
            review(backend, &context.session_id, plan)
        }
        command @ (Command::ResourceExecutePlan { .. }
        | Command::ResourceExecuteChanges { .. }) => {
            let changes =
                matches!(command, Command::ResourceExecuteChanges { .. });
            let (Command::ResourceExecutePlan { plan_id, revision }
            | Command::ResourceExecuteChanges { plan_id, revision }) = command
            else {
                unreachable!()
            };
            let checked =
                backend.plans.get(&context.session_id, plan_id, revision)?;
            if (checked.intent != ResourceOperationIntent::Install) != changes {
                return Err(ToolError::new(
                    "wrong_plan_intent",
                    "Use resource_execute_changes for updates/version switches and resource_execute_plan for new installations",
                ));
            }
            super::resource_history::idle(&checked.instance_id).await?;
            let plan =
                backend.plans.take(&context.session_id, plan_id, revision)?;
            if !executable(&plan) {
                return Err(ToolError::new(
                    "plan_blocked",
                    "Plan is blocked, empty or too large; revise it and review before execution",
                ));
            }
            Ok(super::jobs::summary(
                r::plans::queue(plan.id, revision).await.map_err(error)?,
            ))
        }
        Command::ResourceRevisePlan {
            plan_id,
            revision,
            ignore_dependencies,
            accept_compatibility_nodes,
        } => {
            let plan =
                backend.plans.take(&context.session_id, plan_id, revision)?;
            let decisions = override_decisions(
                &plan,
                ignore_dependencies,
                &accept_compatibility_nodes,
            )?;
            let revised =
                r::plans::revise(plan.id, revision, decisions, &cancel)
                    .await
                    .map_err(error)?;
            review(backend, &context.session_id, revised)
        }
        _ => Err(ToolError::new(
            "unsupported_command",
            "Not a resource operation",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan() -> ResourceOperationPlan {
        serde_json::from_value(json!({
            "schemaVersion":1,"id":"private-native-plan","revision":2,"instanceId":"instance","instanceRevision":1,
            "intent":"install","selections":[],"activeRoots":["root"],"edges":[],"actions":[],
            "issues":[{"code":"compatibilityMismatch","selectionId":"root","entryId":null,"blocking":true,"message":"Loader: Incompatible"}],
            "exceptions":[],"selectionReasons":[],"nodes":[{
                "id":"root","rootSelectionIds":["root"],"resourceType":"mod",
                "resource":{"kind":"source","project":{"source":"modrinth","projectId":"project"}},"scope":{"kind":"instance"},
                "state":"selectedRoot","installedEntries":[],"compatibility":{"gameVersion":"unknown","loader":"incompatible","reasons":["loader"]},
                "artifact":{
                    "reference":{"release":{"project":{"source":"modrinth","projectId":"project"},"releaseId":"frozen-release"},"artifactId":"file"},
                    "fileName":"mod.jar","role":"primary","available":true,"integrity":{"size":100,"hashes":[]},
                    "acquisition":{"kind":"publicDownload","urls":["https://example.com/private?token=hidden"]},"dependencies":[]
                }
            }]
        })).unwrap()
    }

    #[test]
    fn overrides_use_exact_frozen_release_and_only_requested_dimensions() {
        let plan = plan();
        let decisions =
            override_decisions(&plan, true, &["root".into(), "root".into()])
                .unwrap();
        assert_eq!(decisions.len(), 2);
        assert!(matches!(
            &decisions[0],
            ResourcePlanDecision::IgnoreDependencies
        ));
        let ResourcePlanDecision::AcceptCompatibility { exception } =
            &decisions[1]
        else {
            panic!("Expected compatibility exception")
        };
        assert_eq!(exception.selection_id.as_str(), "root");
        assert_eq!(exception.release.release_id.as_str(), "frozen-release");
        assert_eq!(
            exception.dimensions,
            vec![
                CompatibilityDimension::GameVersion,
                CompatibilityDimension::Loader
            ]
        );
        assert_eq!(
            override_decisions(&plan, false, &["foreign".into()])
                .unwrap_err()
                .code,
            "invalid_node"
        );
        assert_eq!(
            override_decisions(&plan, false, &[]).unwrap_err().code,
            "no_changes"
        );
    }

    #[test]
    fn blocked_plans_can_be_reviewed_and_revised_but_cannot_execute() {
        let backend = LauncherBackend::default();
        let value = review(&backend, "session", plan()).unwrap();
        assert_eq!(value["executable"], false);
        assert_eq!(value["nodes"][0]["node_id"], "root");
        assert!(!value.to_string().contains("private-native-plan"));
        assert!(!value.to_string().contains("token=hidden"));
        let handle =
            uuid::Uuid::parse_str(value["plan_id"].as_str().unwrap()).unwrap();
        assert_eq!(
            backend.plans.take("foreign", handle, 2).unwrap_err().code,
            "plan_not_found"
        );
        let plan = backend.plans.take("session", handle, 2).unwrap();
        assert!(!executable(&plan));
        assert!(override_decisions(&plan, true, &[]).is_ok());
    }

    #[test]
    fn release_summaries_do_not_expose_download_urls() {
        let artifact = plan().nodes.remove(0).artifact.unwrap();
        let value = release_summary(ResourceRelease {
            reference: artifact.reference.release.clone(),
            version: "1.0".into(),
            channel: ReleaseChannel::Release,
            published_at: None,
            compatibility: ReleaseCompatibility {
                game_versions: vec!["1.21".into()],
                loaders: vec!["fabric".into()],
            },
            artifacts: vec![artifact],
            equivalent_releases: vec![],
        });
        assert_eq!(value["artifacts"][0]["acquisition"], "public_download");
        assert!(!value.to_string().contains("token=hidden"));
        assert!(!value.to_string().contains("https://"));
    }
}

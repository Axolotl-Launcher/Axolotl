use super::{LauncherBackend, resource_history, resources};
use launcher_tools::{BackendContext, Command, ToolError};
use serde_json::{Value, json};
use theseus::resources::{self as r, *};
use tokio_util::sync::CancellationToken;

fn ids(values: Vec<String>) -> Result<Vec<InstalledEntryId>, ToolError> {
    let mut seen = std::collections::HashSet::new();
    values
        .into_iter()
        .map(|value| {
            if !seen.insert(value.clone()) {
                return Err(ToolError::new(
                    "duplicate_entry",
                    "Each entry_id must occur once",
                ));
            }
            InstalledEntryId::new(value).map_err(|_| {
                ToolError::new("invalid_entry", "Invalid entry_id")
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn entry(ownership: Value) -> InstalledResource {
        serde_json::from_value(json!({"entryId":"entry","instanceId":"instance","resourceId":null,
            "artifact":{"release":{"project":{"source":"modrinth","projectId":"project"},"releaseId":"old"},"artifactId":"file"},
            "knownArtifacts":[],"integrity":{"size":1,"hashes":[]},"relativePath":"mods/example.jar","originalFileName":null,
            "resourceType":"mod","scope":{"kind":"instance"},"enabled":true,"missing":false,"ownership":ownership})).unwrap()
    }
    #[test]
    fn update_roots_preserve_exact_entry_and_project_and_reject_pack_content() {
        let selected = selection(
            entry(json!({"kind":"userAdded"})),
            ReleaseSelection::Automatic,
        )
        .unwrap();
        assert_eq!(selected.entry_id.unwrap().as_str(), "entry");
        assert_eq!(selected.scope, InstallScope::Instance);
        assert!(matches!(selected.release, ReleaseSelection::Automatic));
        let error = selection(
            entry(json!({"kind":"packManaged","packMemberId":"member"})),
            ReleaseSelection::Automatic,
        )
        .unwrap_err();
        assert_eq!(error.code, "pack_managed");
        assert_eq!(
            ids(vec!["entry".into(), "entry".into()]).unwrap_err().code,
            "duplicate_entry"
        );
    }
}

fn selection(
    entry: InstalledResource,
    release: ReleaseSelection,
) -> Result<ResourceSelection, ToolError> {
    let project = entry
        .artifact
        .as_ref()
        .or(entry.known_artifacts.first())
        .ok_or_else(|| {
            ToolError::new(
                "unidentified_entry",
                "Identify the installed resource before updating it",
            )
        })?
        .release
        .project
        .clone();
    if matches!(entry.ownership, ResourceOwnership::PackManaged { .. }) {
        return Err(ToolError::new(
            "pack_managed",
            "Use managed modpack operations for pack-owned entries",
        ));
    }
    Ok(ResourceSelection {
        id: SelectionId::new(uuid::Uuid::new_v4().to_string()).unwrap(),
        resource: ResourceRef::Source { project },
        release,
        resource_type: entry.resource_type,
        scope: entry.scope,
        entry_id: Some(entry.entry_id),
    })
}

pub(super) async fn execute(
    backend: &LauncherBackend,
    command: Command,
    context: BackendContext,
) -> Result<Value, ToolError> {
    let cancel = CancellationToken::new();
    let (instance_id, entry_ids, switch) = match command {
        Command::ResourceCheckUpdates {
            instance_id,
            entry_ids,
        } => {
            let entries = ids(entry_ids)?;
            let result =
                r::updates::check(instance_id, Some(&entries), |_| {}, &cancel)
                    .await
                    .map_err(resources::error)?;
            return Ok(
                json!({"updates":result.releases,"unresolved_paths":result.unresolved,"failed_entries":result.failed_entries,"failure_count":result.failures.len()}),
            );
        }
        Command::ResourcePrepareUpdate {
            instance_id,
            entry_ids,
        } => (instance_id, ids(entry_ids)?, None),
        Command::ResourcePrepareSwitch {
            instance_id,
            entry_id,
            source,
            project_id,
            release_id,
        } => {
            let project = resources::project(source, project_id)?;
            let reference = SourceReleaseRef {
                project,
                release_id: SourceReleaseId::new(release_id).map_err(|_| {
                    ToolError::new("invalid_release", "Invalid release_id")
                })?,
            };
            (instance_id, ids(vec![entry_id])?, Some(reference))
        }
        _ => {
            return Err(ToolError::new(
                "unsupported_command",
                "Not a resource update operation",
            ));
        }
    };
    resource_history::idle(&instance_id).await?;
    let installed = r::installed::list(instance_id.clone(), &cancel)
        .await
        .map_err(resources::error)?;
    let mut selections = Vec::new();
    for id in entry_ids {
        let entry = installed
            .iter()
            .find(|entry| entry.entry_id == id)
            .cloned()
            .ok_or_else(|| {
                ToolError::new(
                    "entry_not_found",
                    "Entry does not belong to this instance",
                )
            })?;
        if let Some(reference) = &switch {
            if !entry
                .artifact
                .iter()
                .chain(entry.known_artifacts.iter())
                .any(|a| a.release.project == reference.project)
            {
                return Err(ToolError::new(
                    "project_mismatch",
                    "Target release must belong to this entry's known source-qualified project",
                ));
            }
        }
        let mut selected = selection(
            entry,
            switch
                .clone()
                .map(|reference| ReleaseSelection::Release { reference })
                .unwrap_or(ReleaseSelection::Automatic),
        )?;
        if let Some(reference) = &switch {
            selected.resource = ResourceRef::Source {
                project: reference.project.clone(),
            };
        }
        selections.push(selected);
    }
    let intent = if switch.is_some() {
        ResourceOperationIntent::SwitchVersion
    } else {
        ResourceOperationIntent::Update
    };
    let plan =
        r::plans::prepare(instance_id, intent, selections, true, &cancel)
            .await
            .map_err(resources::error)?;
    resources::review(backend, &context.session_id, plan)
}

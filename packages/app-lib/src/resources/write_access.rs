use super::{plans::OperationDraft, *};
use std::collections::HashSet;
use std::path::Path;

pub(crate) async fn review(
    draft: &mut OperationDraft,
    state: &crate::State,
) -> ResourceResult<()> {
    let instance =
        crate::state::get_instance(&draft.plan.instance_id, &state.pool)
            .await?
            .ok_or_else(|| {
                ResourceError::InvalidRequest("unknown instance".into())
            })?;
    let root = crate::state::instances::commands::instance_content_root(
        &state.directories,
        &instance.instance,
    )?;
    review_at_root(draft, &root)
}

pub(crate) fn review_at_root(
    draft: &mut OperationDraft,
    root: &Path,
) -> ResourceResult<()> {
    draft.plan.issues.retain(|issue| {
        issue.code != ResourceIssueCode::FileNotWritable
            && (issue.code != ResourceIssueCode::FileWriteSkipped
                || !draft.plan.actions.iter().any(|action| {
                    Some(&action.entry_id) == issue.entry_id.as_ref()
                }))
    });
    let mut skipped = HashSet::new();
    for action in &draft.plan.actions {
        let Err(error) = probe(root, action) else {
            continue;
        };
        let writable_error = matches!(error.raw.as_ref(), crate::ErrorKind::StdIOError(io)
            if !matches!(io.kind(), std::io::ErrorKind::NotFound | std::io::ErrorKind::InvalidInput));
        let skip = writable_error && draft.skip_unwritable_files;
        draft.plan.issues.push(ResourcePlanIssue {
            code: if skip {
                ResourceIssueCode::FileWriteSkipped
            } else if writable_error {
                ResourceIssueCode::FileNotWritable
            } else {
                ResourceIssueCode::InvalidPath
            },
            selection_id: Some(action.node_id.clone()),
            entry_id: Some(action.entry_id.clone()),
            blocking: !skip,
            message: action.target_relative_path.clone(),
        });
        if skip {
            skipped.insert(action.entry_id.clone());
        }
    }
    draft
        .plan
        .actions
        .retain(|action| !skipped.contains(&action.entry_id));
    let skipped_nodes: HashSet<_> = draft
        .plan
        .issues
        .iter()
        .filter(|issue| issue.code == ResourceIssueCode::FileWriteSkipped)
        .filter_map(|issue| issue.selection_id.clone())
        .filter(|id| {
            !draft
                .plan
                .actions
                .iter()
                .any(|action| &action.node_id == id)
        })
        .collect();
    for node in &mut draft.plan.nodes {
        if skipped_nodes.contains(&node.id) {
            node.state = DependencyNodeState::Unresolved;
            node.installed_entries.clear();
        }
    }
    draft.plan.edges.retain(|edge| {
        !skipped_nodes.contains(&edge.parent)
            && !skipped_nodes.contains(&edge.child)
    });
    Ok(())
}

fn probe(root: &Path, action: &ResourceEntryAction) -> crate::Result<()> {
    let destination =
        super::execution::checked_path(root, &action.target_relative_path)?;
    if destination.exists()
        && action.current.as_ref().is_none_or(|current| {
            current.missing
                || current.relative_path != action.target_relative_path
        })
    {
        return Err(crate::ErrorKind::InputError(
            "resource destination already exists".into(),
        )
        .into());
    }
    if let Some(current) = &action.current
        && !current.missing
    {
        let previous =
            super::execution::checked_path(root, &current.relative_path)?;
        let metadata = std::fs::metadata(&previous)?;
        if !metadata.is_file() {
            return Err(
                std::io::Error::from(std::io::ErrorKind::InvalidInput).into()
            );
        }
        if metadata.permissions().readonly() {
            return Err(std::io::Error::from(
                std::io::ErrorKind::PermissionDenied,
            )
            .into());
        }
        let mut options = std::fs::OpenOptions::new();
        options.write(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            // Replacement also needs delete access, which detects files held
            // open by a process that does not share deletion on Windows.
            options.access_mode(0x0001_0002);
        }
        drop(options.open(&previous)?);
        probe_parent(previous.parent().unwrap_or(root))?;
    }
    probe_parent(destination.parent().unwrap_or(root))?;
    Ok(())
}

fn probe_parent(mut parent: &Path) -> std::io::Result<()> {
    while !parent.exists() {
        parent = parent.parent().ok_or_else(|| {
            std::io::Error::from(std::io::ErrorKind::NotFound)
        })?;
    }
    let metadata = std::fs::metadata(parent)?;
    if !metadata.is_dir() {
        return Err(std::io::ErrorKind::InvalidInput.into());
    }
    #[cfg(unix)]
    if metadata.permissions().readonly() {
        return Err(std::io::ErrorKind::PermissionDenied.into());
    }
    tempfile::Builder::new()
        .prefix(".axolotl-write-check-")
        .tempfile_in(parent)?
        .close()
}

use super::create_direct_link_instance::create_direct_link_instance;
use crate::api::pack::import::direct_link::{
    detect_direct_link_source, direct_link_group,
    has_minecraft_version_manifest, resolve_direct_link,
};
use crate::event::{InstancePayloadType, emit::emit_instance};
use crate::launcher::ExternalGameDirMode;
use crate::state::instances::{
    CreateDirectLinkInstance, EditInstance, adapters::sqlite::instance_rows,
};
use crate::state::{AppliedContentSetPatch, InstanceInstallStage, State};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalMinecraftRoot {
    pub path: PathBuf,
    #[serde(default = "default_external_root_mode")]
    pub mode: ExternalGameDirMode,
}

fn default_external_root_mode() -> ExternalGameDirMode {
    ExternalGameDirMode::Automatic
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectLinkSyncReport {
    pub imported: u32,
    pub updated: u32,
    pub removed: u32,
    pub missing: u32,
    pub errors: Vec<String>,
}

/// Reconciles configured external `.minecraft` roots with Axolotl's instance
/// records. The external filesystem is authoritative: new version folders are
/// associated, changed JSON metadata is refreshed, and records whose version
/// JSON disappeared are removed without touching any remaining files.
pub(crate) async fn sync_direct_link_instances(
    roots: Vec<ExternalMinecraftRoot>,
    state: &State,
) -> crate::Result<DirectLinkSyncReport> {
    tracing::debug!(roots = ?roots, "starting direct-link game directory sync");
    let mut report = DirectLinkSyncReport::default();
    let mut canonical_roots = Vec::new();
    let mut unavailable_roots = Vec::new();
    for root in &roots {
        match crate::util::io::canonicalize(&root.path) {
            Ok(path) if path.is_dir() => {
                canonical_roots.push((path, root.mode))
            }
            Ok(_) => {
                report.missing += 1;
                report.errors.push(format!(
                    "{} is not a directory",
                    root.path.display()
                ));
            }
            Err(error) => {
                if error.kind() == std::io::ErrorKind::NotFound {
                    report.missing += 1;
                } else {
                    unavailable_roots.push(root.path.clone());
                }
                report
                    .errors
                    .push(format!("{}: {error}", root.path.display()));
            }
        }
    }
    canonical_roots.sort_by(|left, right| left.0.cmp(&right.0));
    canonical_roots.dedup_by(|left, right| left.0 == right.0);

    let existing = crate::state::list_instances(&state.pool)
        .await?
        .into_iter()
        .collect::<Vec<_>>();
    let mut seen_json = Vec::<PathBuf>::new();

    for (root, mode) in &canonical_roots {
        let versions = root.join("versions");
        let entries = match std::fs::read_dir(&versions) {
            Ok(entries) => entries,
            Err(error) => {
                report
                    .errors
                    .push(format!("{}: {error}", versions.display()));
                continue;
            }
        };
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    report
                        .errors
                        .push(format!("{}: {error}", versions.display()));
                    continue;
                }
            };
            let folder = entry.path();
            if !folder.is_dir() {
                continue;
            }
            // A `versions` directory can also contain PCL bookkeeping entries
            // left by an interrupted install. They have no version manifest,
            // so they are not Minecraft instances and should not surface a
            // warning on every root synchronization.
            if !has_minecraft_version_manifest(&folder) {
                continue;
            }
            tracing::debug!(root = %root.display(), version_dir = %folder.display(), mode = %mode.key(), "attempting direct-link instance sync");
            let folder_name = entry.file_name().to_string_lossy().to_string();
            let instance_folder = Path::new("versions").join(&folder_name);
            let source = detect_direct_link_source(root, &folder);
            let resolved = match resolve_direct_link(
                source.launcher,
                source.launcher_root.clone(),
                instance_folder.to_string_lossy().to_string(),
                Some(folder.to_string_lossy().to_string()),
            )
            .await
            {
                Ok(resolved) => resolved,
                Err(error) => {
                    report
                        .errors
                        .push(format!("{}: {error}", folder.display()));
                    continue;
                }
            };
            seen_json.push(resolved.version_json.clone());

            let existing_instance = existing.iter().find(|metadata| {
                metadata
                    .instance
                    .linked_version_json_path
                    .as_deref()
                    .is_some_and(|path| {
                        Path::new(path) == resolved.version_json
                    })
                    || metadata
                        .instance
                        .game_dir_override
                        .as_deref()
                        .is_some_and(|path| Path::new(path) == folder)
            });

            if let Some(metadata) = existing_instance {
                let instance = &metadata.instance;
                if instance.symlink_target.is_some() {
                    // Symlink imports can point at the same external version,
                    // but they are ordinary managed instances rather than
                    // records owned by direct-link reconciliation.
                    continue;
                }
                if !instance.is_direct_linked()
                    && !is_unpromoted_direct_link_candidate(
                        instance.install_stage,
                        instance.symlink_target.as_deref(),
                    )
                {
                    // An import can temporarily have the same isolated
                    // game-dir override before its symlink metadata is
                    // persisted. Reserve the external version for that import
                    // without promoting, duplicating, or deleting its record.
                    continue;
                }
                let fields_changed = instance.linked_version_id.as_deref()
                    != Some(resolved.version_id.as_str())
                    || instance.linked_launcher.as_deref()
                        != Some(resolved.launcher_key())
                    || instance.linked_launcher_root.as_deref()
                        != Some(
                            resolved.launcher_root.to_string_lossy().as_ref(),
                        )
                    || instance.linked_dot_minecraft.as_deref()
                        != Some(
                            resolved.dot_minecraft.to_string_lossy().as_ref(),
                        )
                    || instance.linked_game_dir_mode.as_deref()
                        != Some(mode.key());
                let content_changed = metadata.applied_content_set.game_version
                    != resolved.game_version
                    || metadata.applied_content_set.loader != resolved.loader;
                let groups = direct_link_group(&resolved.dot_minecraft)
                    .into_iter()
                    .collect::<Vec<_>>();
                let groups_changed = metadata.groups != groups;
                if fields_changed || content_changed || groups_changed {
                    if content_changed {
                        crate::state::edit_instance(
                            &instance.id,
                            EditInstance {
                                content_set_patch: Some(
                                    AppliedContentSetPatch {
                                        game_version: Some(
                                            resolved.game_version.clone(),
                                        ),
                                        loader: Some(resolved.loader),
                                        ..AppliedContentSetPatch::default()
                                    },
                                ),
                                ..EditInstance::default()
                            },
                            &state.pool,
                        )
                        .await?;
                    }
                    let (mut tx, tx_diagnostics) =
                        crate::state::store::lock_diagnostics::begin(
                            &state.pool,
                            "BEGIN IMMEDIATE",
                        )
                        .await?;
                    instance_rows::set_direct_link_fields(
                        &instance.id,
                        &instance_rows::DirectLinkFields {
                            launcher: Some(resolved.launcher_key().to_string()),
                            launcher_root: Some(
                                resolved
                                    .launcher_root
                                    .to_string_lossy()
                                    .to_string(),
                            ),
                            dot_minecraft: Some(
                                resolved
                                    .dot_minecraft
                                    .to_string_lossy()
                                    .to_string(),
                            ),
                            version_id: Some(resolved.version_id.clone()),
                            version_json_path: Some(
                                resolved
                                    .version_json
                                    .to_string_lossy()
                                    .to_string(),
                            ),
                            game_dir_mode: Some(mode.key().to_string()),
                        },
                        &mut tx,
                    )
                    .await?;
                    tx_diagnostics.observe("commit_wait", tx.commit()).await?;
                    tx_diagnostics.complete("committed");
                    crate::state::instance_groups::set_instance_groups(
                        &instance.id,
                        &groups,
                    )?;
                    let _ = emit_instance(
                        &instance.id,
                        InstancePayloadType::Edited,
                    )
                    .await;
                    report.updated += 1;
                }
            } else {
                let instance = match create_direct_link_instance(
                    CreateDirectLinkInstance {
                        name: Some(folder_name),
                        launcher_type: source.launcher,
                        base_path: source.launcher_root,
                        instance_folder: instance_folder
                            .to_string_lossy()
                            .to_string(),
                        instance_path: Some(
                            folder.to_string_lossy().to_string(),
                        ),
                        game_dir_mode: Some(*mode),
                    },
                    state,
                )
                .await
                {
                    Ok(instance) => instance,
                    Err(error) => {
                        report
                            .errors
                            .push(format!("{}: {error}", folder.display()));
                        continue;
                    }
                };
                let _ =
                    emit_instance(&instance.id, InstancePayloadType::Created)
                        .await;
                report.imported += 1;
            }
        }
    }

    // Ordinary instances created with a version-isolated game-dir override
    // are also associated with a configured root. If that root is removed
    // from Settings before the next scan promotes the record to a direct
    // link, drop only the Axolotl record here as well.
    for metadata in &existing {
        if metadata.instance.linked_dot_minecraft.is_some() {
            continue;
        }
        if !is_unpromoted_direct_link_candidate(
            metadata.instance.install_stage,
            metadata.instance.symlink_target.as_deref(),
        ) {
            continue;
        }
        let Some(game_dir_override) =
            metadata.instance.game_dir_override.as_deref()
        else {
            continue;
        };
        let Some(root) = version_isolated_root(game_dir_override) else {
            continue;
        };
        if configured_root_matches(&root, &canonical_roots, &unavailable_roots)
        {
            continue;
        }
        crate::state::remove_instance_preserving_external_files(
            &metadata.instance.id,
            state,
        )
        .await?;
        let _ =
            emit_instance(&metadata.instance.id, InstancePayloadType::Removed)
                .await;
        report.removed += 1;
    }

    for metadata in existing {
        if metadata.instance.symlink_target.is_some() {
            continue;
        }
        let Some(json_path) =
            metadata.instance.linked_version_json_path.as_deref()
        else {
            continue;
        };
        let Some(root) = metadata.instance.linked_dot_minecraft.as_deref()
        else {
            continue;
        };
        if !configured_root_matches(
            Path::new(root),
            &canonical_roots,
            &unavailable_roots,
        ) {
            // Configured roots are authoritative. Removing a root from Settings
            // only drops Axolotl's association; the external files remain intact.
            crate::state::remove_instance_preserving_external_files(
                &metadata.instance.id,
                state,
            )
            .await?;
            let _ = emit_instance(
                &metadata.instance.id,
                InstancePayloadType::Removed,
            )
            .await;
            report.removed += 1;
            continue;
        }
        let json_path = PathBuf::from(json_path);
        if !json_path.exists()
            && !seen_json.iter().any(|path| path == &json_path)
        {
            // External deletion is authoritative, but there is nothing left
            // to delete on disk. Only remove the stale Axolotl record.
            crate::state::remove_instance_preserving_external_files(
                &metadata.instance.id,
                state,
            )
            .await?;
            let _ = emit_instance(
                &metadata.instance.id,
                InstancePayloadType::Removed,
            )
            .await;
            report.removed += 1;
        }
    }

    tracing::debug!(report = ?report, "direct-link game directory sync finished");
    Ok(report)
}

fn is_unpromoted_direct_link_candidate(
    install_stage: InstanceInstallStage,
    symlink_target: Option<&str>,
) -> bool {
    install_stage == InstanceInstallStage::Installed && symlink_target.is_none()
}

fn version_isolated_root(path: &str) -> Option<PathBuf> {
    let version_dir = Path::new(path);
    if version_dir
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        != Some("versions")
    {
        return None;
    }
    version_dir.parent()?.parent().map(Path::to_path_buf)
}

/// A configured root with a transient access error retains its records, while
/// a root that is confirmed missing is no longer authoritative.
fn configured_root_matches(
    root: &Path,
    canonical_roots: &[(PathBuf, ExternalGameDirMode)],
    unavailable_roots: &[PathBuf],
) -> bool {
    canonical_roots
        .iter()
        .any(|(candidate, _)| candidate == root)
        || unavailable_roots
            .iter()
            .any(|candidate| paths_match(candidate, root))
}

fn paths_match(left: &Path, right: &Path) -> bool {
    let left = left.components().collect::<PathBuf>();
    let right = right.components().collect::<PathBuf>();

    #[cfg(target_os = "windows")]
    {
        left.to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy())
    }
    #[cfg(not(target_os = "windows"))]
    {
        left == right
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transiently_unavailable_root_keeps_its_association() {
        let root = PathBuf::from("minecraft-root");
        let equivalent = PathBuf::from("minecraft-root").join(".");

        assert!(configured_root_matches(&equivalent, &[], &[root],));
    }

    #[test]
    fn missing_root_does_not_keep_its_association() {
        assert!(!configured_root_matches(
            Path::new("minecraft-root"),
            &[],
            &[],
        ));
    }

    #[test]
    fn installing_override_is_not_treated_as_an_orphaned_direct_link() {
        assert!(!is_unpromoted_direct_link_candidate(
            InstanceInstallStage::NotInstalled,
            None,
        ));
        assert!(!is_unpromoted_direct_link_candidate(
            InstanceInstallStage::MinecraftInstalling,
            None,
        ));
    }

    #[test]
    fn completed_symlink_import_is_not_treated_as_an_orphaned_direct_link() {
        assert!(!is_unpromoted_direct_link_candidate(
            InstanceInstallStage::Installed,
            Some(r"D:\Minecraft\.minecraft\versions\1.20.1"),
        ));
    }

    #[test]
    fn installed_unpromoted_direct_link_remains_eligible_for_cleanup() {
        assert!(is_unpromoted_direct_link_candidate(
            InstanceInstallStage::Installed,
            None,
        ));
    }
}

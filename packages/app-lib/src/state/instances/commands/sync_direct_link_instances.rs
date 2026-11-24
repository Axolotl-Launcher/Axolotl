use super::create_direct_link_instance::create_direct_link_instance;
use crate::api::pack::import::{
    ImportLauncherType,
    direct_link::{
        DirectLinkSource, detect_direct_link_source,
        has_linked_version_manifest, resolve_direct_link,
    },
};
use crate::event::{InstancePayloadType, emit::emit_instance};
use crate::launcher::ExternalGameDirMode;
use crate::state::instances::{
    CreateDirectLinkInstance, EditInstance, adapters::sqlite::instance_rows,
};
use crate::state::{AppliedContentSetPatch, InstanceInstallStage, State};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    path::{Path, PathBuf},
};

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
    let mut canonical_roots =
        BTreeMap::<PathBuf, Vec<&ExternalMinecraftRoot>>::new();
    let mut unavailable_roots = Vec::new();
    let mut unresolved_configured_root = false;
    for root in &roots {
        match crate::util::io::canonicalize(&root.path) {
            Ok(path) if path.is_dir() => {
                canonical_roots.entry(path).or_default().push(root);
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
                    unresolved_configured_root = true;
                    unavailable_roots.push(root.path.clone());
                }
                report
                    .errors
                    .push(format!("{}: {error}", root.path.display()));
            }
        }
    }
    let existing = crate::state::list_instances(&state.pool).await?;
    let existing_paths = existing
        .iter()
        .map(|metadata| {
            let instance = &metadata.instance;
            (
                instance
                    .linked_version_json_path
                    .as_deref()
                    .map(path_identity),
                instance.game_dir_override.as_deref().map(path_identity),
            )
        })
        .collect::<Vec<_>>();
    let mut seen_json = HashSet::new();

    for (root, aliases) in &canonical_roots {
        let Some(mode) = configured_mode(aliases) else {
            unavailable_roots.push(root.clone());
            report.errors.push(format!(
                "{} has conflicting configured game directory modes",
                root.display()
            ));
            continue;
        };
        let versions = root.join("versions");
        let entries = match std::fs::read_dir(&versions) {
            Ok(entries) => entries,
            Err(error) => {
                if error.kind() != std::io::ErrorKind::NotFound {
                    unavailable_roots.push(root.clone());
                }
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
            let Some(source) =
                detect_source_for_aliases(root, aliases, &folder)
            else {
                report.errors.push(format!(
                    "{} has conflicting launcher associations through its configured paths",
                    folder.display()
                ));
                continue;
            };
            if !has_linked_version_manifest(&folder, source.launcher) {
                continue;
            }
            tracing::debug!(root = %root.display(), version_dir = %folder.display(), mode = %mode.key(), "attempting direct-link instance sync");
            let folder_name = entry.file_name().to_string_lossy().to_string();
            let instance_folder = Path::new("versions").join(&folder_name);
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
            if !seen_json.insert(resolved.version_json.clone()) {
                continue;
            }

            let mut matches = existing.iter().zip(&existing_paths).filter(
                |(_, (json, game_dir))| {
                    json.as_ref() == Some(&resolved.version_json)
                        || game_dir.as_ref() == Some(&resolved.version_dir)
                },
            );
            let existing_instance = matches.next();
            if matches.next().is_some() {
                report.errors.push(format!(
                    "Multiple instances refer to {}; their associations were preserved",
                    resolved.version_json.display()
                ));
                continue;
            }

            if let Some((metadata, _)) = existing_instance {
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
                    || instance.linked_version_json_path.as_deref()
                        != Some(
                            resolved.version_json.to_string_lossy().as_ref(),
                        )
                    || instance.linked_game_dir_mode.as_deref()
                        != Some(mode.key());
                let content_changed = metadata.applied_content_set.game_version
                    != resolved.game_version
                    || metadata.applied_content_set.loader != resolved.loader;
                // The library derives game-directory grouping from the link.
                // Its path is not an ID in the user's group document.
                if !fields_changed && !content_changed {
                    continue;
                }
                if content_changed {
                    crate::state::edit_instance(
                        &instance.id,
                        EditInstance {
                            content_set_patch: Some(AppliedContentSetPatch {
                                game_version: Some(
                                    resolved.game_version.clone(),
                                ),
                                loader: Some(resolved.loader),
                                ..AppliedContentSetPatch::default()
                            }),
                            ..EditInstance::default()
                        },
                        &state.pool,
                    )
                    .await?;
                }
                if fields_changed {
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
                }
                let _ =
                    emit_instance(&instance.id, InstancePayloadType::Edited)
                        .await;
                report.updated += 1;
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
                        game_dir_mode: Some(mode),
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
            || unresolved_configured_root
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

    for (metadata, (json_identity, _)) in existing.iter().zip(&existing_paths) {
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
            // An inaccessible configured alias may name this stored physical
            // root. Only remove the association once every alias can be checked.
            if unresolved_configured_root {
                continue;
            }
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
        if !root_is_unavailable(Path::new(root), &unavailable_roots)
            && !json_identity
                .as_ref()
                .is_some_and(|path| seen_json.contains(path))
            && matches!(Path::new(json_path).try_exists(), Ok(false))
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

fn configured_mode(
    aliases: &[&ExternalMinecraftRoot],
) -> Option<ExternalGameDirMode> {
    let mut explicit = aliases
        .iter()
        .map(|root| root.mode)
        .filter(|mode| *mode != ExternalGameDirMode::Automatic);
    let mode = explicit.next().unwrap_or(ExternalGameDirMode::Automatic);
    explicit.all(|other| other == mode).then_some(mode)
}

fn detect_source_for_aliases(
    canonical_root: &Path,
    aliases: &[&ExternalMinecraftRoot],
    version_dir: &Path,
) -> Option<DirectLinkSource> {
    // Launcher configuration can live beside the configured junction, while
    // the version folder and its contents live beneath the target directory.
    let mut sources = aliases
        .iter()
        .map(|root| root.path.as_path())
        .filter(|path| *path != canonical_root)
        .chain(std::iter::once(canonical_root))
        .map(|path| detect_direct_link_source(path, version_dir));
    let Some(source) =
        sources.find(|source| source.launcher != ImportLauncherType::Generic)
    else {
        return Some(DirectLinkSource {
            launcher: ImportLauncherType::Generic,
            launcher_root: canonical_root.to_path_buf(),
        });
    };
    let launcher_root = path_identity(&source.launcher_root);
    sources
        .all(|other| {
            other.launcher == ImportLauncherType::Generic
                || (other.launcher == source.launcher
                    && path_identity(&other.launcher_root) == launcher_root)
        })
        .then_some(source)
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
    canonical_roots: &BTreeMap<PathBuf, Vec<&ExternalMinecraftRoot>>,
    unavailable_roots: &[PathBuf],
) -> bool {
    canonical_roots.contains_key(root)
        || canonical_roots.contains_key(&path_identity(root))
        || root_is_unavailable(root, unavailable_roots)
}

fn path_identity(path: impl AsRef<Path>) -> PathBuf {
    let path = path.as_ref();
    crate::util::io::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn root_is_unavailable(root: &Path, unavailable_roots: &[PathBuf]) -> bool {
    if unavailable_roots.is_empty() {
        return false;
    }
    let identity = path_identity(root);
    unavailable_roots
        .iter()
        .any(|candidate| paths_match(candidate, root) || candidate == &identity)
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

        assert!(configured_root_matches(
            &equivalent,
            &BTreeMap::new(),
            &[root],
        ));
    }

    #[test]
    fn missing_root_does_not_keep_its_association() {
        assert!(!configured_root_matches(
            Path::new("minecraft-root"),
            &BTreeMap::new(),
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

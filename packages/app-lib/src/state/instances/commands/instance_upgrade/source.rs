#![allow(dead_code)]

use super::*;

#[derive(Clone)]
pub(crate) struct ReadOnlyUpgradeSource {
    pub(super) snapshot: InstanceContentSnapshot,
    pub(super) source_files: Vec<InstanceUpgradeSourceFile>,
    pub(super) instance_path: String,
    pub(super) game_dir: std::path::PathBuf,
    pub(super) file_states: Vec<UpgradeSourceFileState>,
    pub(super) catalog: Option<UpgradeCatalog>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct UpgradeSourceFileState {
    pub(super) relative_path: String,
    pub(super) size: u64,
    pub(super) enabled: bool,
    pub(super) modified: Option<SystemTime>,
}

pub(crate) struct UpgradePlanRuntimeValidation {
    pub(super) source: ReadOnlyUpgradeSource,
    pub(super) watcher_epoch: Option<u64>,
    pub(super) validated_generation: Option<u64>,
    #[cfg(test)]
    pub(super) full_hash_validations: usize,
    #[cfg(test)]
    pub(super) incremental_hashes: usize,
}

pub(super) async fn read_only_upgrade_source(
    instance_id: &str,
    state: &State,
    observer: &UpgradePlanningObserver,
) -> crate::Result<ReadOnlyUpgradeSource> {
    let mut snapshot =
        super::super::get_cached_content_snapshot(instance_id, state).await?;
    let instance = crate::state::instances::adapters::sqlite::instance_rows::get_instance_by_id(
        instance_id,
        &state.pool,
    )
    .await?
    .ok_or_else(|| crate::ErrorKind::InputError("Unknown instance".to_string()))?;
    let instance_dir = upgrade_source_game_dir(&instance, &state.directories)?;
    let scanned =
        crate::state::instances::adapters::filesystem::scan_content_files_from(
            &instance_dir,
            &instance.path,
        )?;
    let file_states = scanned
        .iter()
        .map(|file| UpgradeSourceFileState {
            relative_path: file.relative_path.clone(),
            size: file.size,
            enabled: file.enabled,
            modified: file.modified,
        })
        .collect();
    let mut scanned_by_path = HashMap::new();
    let mut source_files = Vec::new();
    let total = scanned.len();
    observer.phase(InstanceUpgradePlanningPhase::ReadingContent, 0, total);
    for (index, file) in scanned.into_iter().enumerate() {
        observer.check()?;
        let (_, sha1) = crate::util::fetch::sha1_file_async(
            instance_dir.join(&file.relative_path),
        )
        .await?;
        observer.phase(
            InstanceUpgradePlanningPhase::ReadingContent,
            index + 1,
            total,
        );
        source_files.push(InstanceUpgradeSourceFile {
            relative_path: file.relative_path.clone(),
            sha1: sha1.clone(),
            size: file.size,
            enabled: file.enabled,
        });
        scanned_by_path.insert(file.relative_path.clone(), (file, sha1));
    }
    snapshot.items.retain(|item| {
        if !crate::state::instances::adapters::filesystem::is_scannable_project_path(
            item.project_type,
            &item.expected_relative_path,
        ) {
            return true;
        }
        scanned_by_path
            .get(&item.expected_relative_path.replace('\\', "/"))
            .is_some_and(|(_, sha1)| {
                item.content.as_ref().is_some_and(|content| {
                    content.id.eq_ignore_ascii_case(sha1)
                })
            })
    });
    let represented = snapshot
        .items
        .iter()
        .map(|item| item.expected_relative_path.replace('\\', "/"))
        .collect::<HashSet<_>>();
    for (_, (file, sha1)) in scanned_by_path {
        if represented.contains(&file.relative_path) {
            continue;
        }
        let Some(project_type) = crate::state::instances::adapters::filesystem::project_type_from_relative_path(
            &file.relative_path,
        ) else {
            continue;
        };
        snapshot.items.push(InstanceContentSnapshotItem {
            resource_update: None,
            artifact_ref: None,
            compatibility: None,
            resource_id: None,
            pack_resource_id: None,
            file_id: None,
            entry_id: None,
            member_id: None,
            ownership_kind: ContentOwnershipKind::LocalDiscovered,
            materialization_state: PackMemberMaterializationState::Present,
            override_kind: PackMemberOverrideKind::None,
            expected_relative_path: file.relative_path.clone(),
            required: false,
            project_type,
            provider: None,
            provider_project_id: None,
            provider_release_id: None,
            content: Some(ContentItem {
                file_name: file.file_name,
                file_path: file.relative_path,
                id: sha1,
                size: file.size,
                enabled: file.enabled,
                project_type,
                project: None,
                version: None,
                owner: None,
                update: None,
                date_added: None,
                provider_refs: Vec::new(),
                origin_provider: None,
                rollback: None,
                environment: None,
                source_kind: None,
                external: true,
                loader: None,
            }),
            capabilities: ContentItemCapabilities::default(),
            dependency: None,
        });
    }
    source_files
        .sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(ReadOnlyUpgradeSource {
        snapshot,
        source_files,
        instance_path: instance.path,
        game_dir: instance_dir,
        file_states,
        catalog: None,
    })
}

pub(super) fn upgrade_source_game_dir(
    instance: &crate::state::Instance,
    directories: &crate::state::DirectoryInfo,
) -> crate::Result<std::path::PathBuf> {
    Ok(
        crate::launcher::instance_runtime::InstanceRuntimeAdapter::for_instance(
            instance,
            directories,
        )?
        .game_dir(),
    )
}

pub(crate) async fn validate_instance_upgrade_plan_source(
    plan: &InstanceUpgradePlan,
    state: &State,
) -> crate::Result<ReadOnlyUpgradeSource> {
    validate_instance_upgrade_plan_source_observed(
        plan,
        state,
        &UpgradePlanningObserver::default(),
    )
    .await
}

pub(super) async fn validate_instance_upgrade_plan_source_observed(
    plan: &InstanceUpgradePlan,
    state: &State,
    observer: &UpgradePlanningObserver,
) -> crate::Result<ReadOnlyUpgradeSource> {
    let current =
        read_only_upgrade_source(&plan.instance_id, state, &observer.silent())
            .await?;
    ensure_upgrade_source_files_match(
        &plan.instance_id,
        &plan.source_files,
        &current.source_files,
    )?;
    Ok(current)
}

pub(crate) async fn scan_instance_upgrade_source_files(
    instance_id: &str,
    state: &State,
) -> crate::Result<Vec<InstanceUpgradeSourceFile>> {
    Ok(read_only_upgrade_source(
        instance_id,
        state,
        &UpgradePlanningObserver::default(),
    )
    .await?
    .source_files)
}

impl UpgradePlanRuntimeValidation {
    pub(crate) async fn new(
        source: ReadOnlyUpgradeSource,
        instance_id: &str,
        creation_watch: Option<
            crate::state::instances::watcher::InstanceContentWatchSnapshot,
        >,
        state: &State,
    ) -> Self {
        let watch = state
            .file_watcher
            .track_upgrade_source(
                instance_id,
                source
                    .source_files
                    .iter()
                    .map(|file| file.relative_path.clone()),
            )
            .await;
        let validated_generation = creation_watch
            .as_ref()
            .filter(|creation| {
                watch
                    .as_ref()
                    .is_some_and(|current| current.epoch == creation.epoch)
            })
            .map(|creation| creation.generation);
        Self {
            source,
            watcher_epoch: watch.as_ref().map(|watch| watch.epoch),
            validated_generation,
            #[cfg(test)]
            full_hash_validations: 0,
            #[cfg(test)]
            incremental_hashes: 0,
        }
    }

    pub(crate) async fn validate(
        &mut self,
        plan: &InstanceUpgradePlan,
        state: &State,
    ) -> crate::Result<ReadOnlyUpgradeSource> {
        self.validate_observed(plan, state, &UpgradePlanningObserver::default())
            .await
    }

    /// Validates like [`Self::validate`], stopping between hashed files once
    /// the observer is canceled.
    pub(crate) async fn validate_observed(
        &mut self,
        plan: &InstanceUpgradePlan,
        state: &State,
        observer: &UpgradePlanningObserver,
    ) -> crate::Result<ReadOnlyUpgradeSource> {
        let Some(mut before) = state
            .file_watcher
            .content_watch_snapshot(&plan.instance_id)
            .await
        else {
            // Unwatched game directories still support metadata and hash
            // validation through the resolved source directory.
            self.incremental_validate(plan, None, state, observer)
                .await?;
            return Ok(self.source.clone());
        };
        if self.watcher_epoch != Some(before.epoch)
            || self.validated_generation.is_none()
            || self.validated_generation > Some(before.generation)
        {
            return self.authoritative_validate(plan, state, observer).await;
        }

        for _ in 0..2 {
            self.incremental_validate(plan, Some(&before), state, observer)
                .await?;
            let Some(after) = state
                .file_watcher
                .content_watch_snapshot(&plan.instance_id)
                .await
            else {
                return self
                    .authoritative_validate(plan, state, observer)
                    .await;
            };
            if after.epoch == before.epoch
                && after.generation == before.generation
            {
                self.watcher_epoch = Some(after.epoch);
                self.validated_generation = Some(after.generation);
                return Ok(self.source.clone());
            }
            before = after;
        }

        self.authoritative_validate(plan, state, observer).await
    }

    pub(super) async fn incremental_validate(
        &mut self,
        plan: &InstanceUpgradePlan,
        watch: Option<
            &crate::state::instances::watcher::InstanceContentWatchSnapshot,
        >,
        _state: &State,
        observer: &UpgradePlanningObserver,
    ) -> crate::Result<()> {
        let scanned = crate::state::instances::adapters::filesystem::scan_content_files_from(
            &self.source.game_dir,
            &self.source.instance_path,
        )?;
        let planned = self
            .source
            .source_files
            .iter()
            .map(|file| (file.relative_path.as_str(), file))
            .collect::<HashMap<_, _>>();
        let previous = self
            .source
            .file_states
            .iter()
            .map(|file| (file.relative_path.as_str(), file))
            .collect::<HashMap<_, _>>();
        if scanned.len() != planned.len()
            || scanned
                .iter()
                .any(|file| !planned.contains_key(file.relative_path.as_str()))
        {
            return stale_upgrade_source(&plan.instance_id);
        }

        let watcher_changed = watch.is_some_and(|watch| {
            self.validated_generation != Some(watch.generation)
        });
        let instance_dir = &self.source.game_dir;
        for file in &scanned {
            let planned_file = planned[&file.relative_path.as_str()];
            if file.size != planned_file.size
                || file.enabled != planned_file.enabled
            {
                return stale_upgrade_source(&plan.instance_id);
            }
            let metadata_changed = previous
                .get(file.relative_path.as_str())
                .is_none_or(|previous| previous.modified != file.modified);
            let watcher_marked = watcher_changed
                && watch.is_some_and(|watch| {
                    watch.dirty_paths.contains(&file.relative_path)
                });
            if metadata_changed || watcher_marked {
                observer.check()?;
                let (_, sha1) = crate::util::fetch::sha1_file_async(
                    instance_dir.join(&file.relative_path),
                )
                .await?;
                #[cfg(test)]
                {
                    self.incremental_hashes += 1;
                }
                if !planned_file.sha1.eq_ignore_ascii_case(&sha1) {
                    return stale_upgrade_source(&plan.instance_id);
                }
            }
        }
        self.source.file_states = scanned
            .into_iter()
            .map(|file| UpgradeSourceFileState {
                relative_path: file.relative_path,
                size: file.size,
                enabled: file.enabled,
                modified: file.modified,
            })
            .collect();
        Ok(())
    }

    pub(super) async fn authoritative_validate(
        &mut self,
        plan: &InstanceUpgradePlan,
        state: &State,
        observer: &UpgradePlanningObserver,
    ) -> crate::Result<ReadOnlyUpgradeSource> {
        #[cfg(test)]
        {
            self.full_hash_validations += 1;
        }
        let current = validate_instance_upgrade_plan_source_observed(
            plan, state, observer,
        )
        .await?;
        let watch = state
            .file_watcher
            .track_upgrade_source(
                &plan.instance_id,
                current
                    .source_files
                    .iter()
                    .map(|file| file.relative_path.clone()),
            )
            .await;
        self.watcher_epoch = watch.as_ref().map(|watch| watch.epoch);
        self.validated_generation =
            watch.as_ref().map(|watch| watch.generation);
        let catalog = self.source.catalog.clone();
        self.source = current;
        self.source.catalog = catalog;
        Ok(self.source.clone())
    }
}

pub(super) fn stale_upgrade_source<T>(instance_id: &str) -> crate::Result<T> {
    Err(crate::ErrorKind::StaleInstanceUpgradePlanSource {
        instance_id: instance_id.to_string(),
    }
    .into())
}

pub(super) fn ensure_upgrade_source_files_match(
    instance_id: &str,
    planned: &[InstanceUpgradeSourceFile],
    current: &[InstanceUpgradeSourceFile],
) -> crate::Result<()> {
    if current != planned {
        return Err(crate::ErrorKind::StaleInstanceUpgradePlanSource {
            instance_id: instance_id.to_string(),
        }
        .into());
    }
    Ok(())
}

pub(super) fn source_shader_runtime(
    components: &[crate::state::LoaderComponent],
    snapshot: &InstanceContentSnapshot,
) -> ShaderRuntime {
    if snapshot.items.iter().any(item_has_iris_identity) {
        return ShaderRuntime::Iris;
    }
    if components
        .iter()
        .any(|component| component.kind == LoaderComponentKind::OptiFine)
    {
        return ShaderRuntime::OptiFine;
    }
    if snapshot.items.iter().any(|item| {
        item.project_type == ProjectType::Mod
            && item
                .content
                .as_ref()
                .is_none_or(|content| content.provider_refs.is_empty())
    }) {
        ShaderRuntime::Unknown
    } else {
        ShaderRuntime::None
    }
}

pub(super) fn item_has_iris_identity(
    item: &InstanceContentSnapshotItem,
) -> bool {
    (item.provider == Some(ContentProvider::Modrinth)
        && item.provider_project_id.as_deref() == Some("YL57xq9U"))
        || item.content.as_ref().is_some_and(|content| {
            content.provider_refs.iter().any(|reference| {
                matches!(reference, ContentProviderRef::Modrinth { project_id, .. } if project_id.as_str() == "YL57xq9U")
            })
        })
}

pub(super) fn snapshot_upgrade_items(
    snapshot: &InstanceContentSnapshot,
) -> (Vec<InstanceUpgradeItem>, Vec<InstalledNode>) {
    let mut items = Vec::new();
    let mut installed = Vec::new();
    for item in &snapshot.items {
        let content_id = stable_content_id(item);
        let current_enabled =
            item.content.as_ref().is_none_or(|content| content.enabled);
        let auto_dependency = item
            .dependency
            .as_ref()
            .is_some_and(|dependency| dependency.auto_dependency);
        let unsupported = is_world_datapack(&item.expected_relative_path)
            || matches!(
                item.project_type,
                ProjectType::Schematic | ProjectType::WorldSave
            );
        let installed_identity = installed_identity(item);
        let recognized = installed_identity
            .as_ref()
            .map(|(key, _, _)| (key.provider, key.project_id.clone()))
            .or_else(|| item.provider.zip(item.provider_project_id.clone()));
        let planner_provider = installed_identity
            .as_ref()
            .map(|(key, _, _)| key.provider)
            .or(item.provider);
        let planner_project_id = installed_identity
            .as_ref()
            .map(|(key, _, _)| key.project_id.clone())
            .or_else(|| item.provider_project_id.clone());
        let planner_release_id = installed_identity
            .as_ref()
            .map(|(_, release_id, _)| release_id.clone())
            .or_else(|| item.provider_release_id.clone());
        let status = if unsupported {
            InstanceUpgradeItemStatus::UnsupportedContentType
        } else if recognized.is_none() {
            InstanceUpgradeItemStatus::Unidentified
        } else {
            InstanceUpgradeItemStatus::NoCompatibleRelease
        };
        let action = if unsupported || recognized.is_none() {
            InstanceUpgradeAction::Keep
        } else {
            InstanceUpgradeAction::Upgrade
        };
        items.push(InstanceUpgradeItem {
            content_id: content_id.clone(),
            relative_path: item.expected_relative_path.clone(),
            project_type: item.project_type,
            provider: planner_provider,
            project_id: planner_project_id,
            current_release_id: planner_release_id,
            current_enabled,
            auto_dependency,
            status,
            resolution: InstanceUpgradeResolution {
                content_id: content_id.clone(),
                action,
                allow_prerelease: false,
                confirmed_prerelease_dependencies: Vec::new(),
            },
            candidate_release_ids: Vec::new(),
        });
        if let Some((key, current_release_id, aliases)) = installed_identity {
            installed.push(InstalledNode {
                content_id,
                key,
                current_release_id: current_release_id.clone(),
                project_type: item.project_type,
                enabled: current_enabled,
                auto_dependency,
                user_owned: item.ownership_kind
                    == ContentOwnershipKind::UserAdded
                    && !auto_dependency,
                migratable: !unsupported,
                aliases,
            });
        }
    }
    (items, installed)
}

pub(super) fn installed_identity(
    item: &InstanceContentSnapshotItem,
) -> Option<(NodeKey, String, Vec<InstalledAlias>)> {
    let mut aliases = item
        .content
        .as_ref()
        .into_iter()
        .flat_map(|content| &content.provider_refs)
        .filter_map(|reference| {
            Some(InstalledAlias {
                key: NodeKey::new(
                    reference.provider(),
                    reference.database_project_id(),
                ),
                current_release_id: reference.database_release_id()?,
            })
        })
        .collect::<Vec<_>>();
    if let (Some(provider), Some(project_id), Some(release_id)) = (
        item.provider,
        item.provider_project_id.as_deref(),
        item.provider_release_id.as_deref(),
    ) {
        let primary = InstalledAlias {
            key: NodeKey::new(provider, project_id),
            current_release_id: release_id.to_string(),
        };
        aliases.retain(|alias| alias.key != primary.key);
        aliases.insert(0, primary);
    }
    let mut seen = HashSet::new();
    aliases.retain(|alias| seen.insert(alias.key.clone()));
    let primary_index = item
        .provider
        .zip(item.provider_project_id.as_deref())
        .and_then(|(provider, project_id)| {
            aliases.iter().position(|alias| {
                alias.key.provider == provider
                    && alias.key.project_id == project_id
            })
        })
        .unwrap_or(0);
    let primary = aliases.get(primary_index)?.clone();
    if primary_index != 0 {
        aliases.swap(0, primary_index);
    }
    Some((primary.key, primary.current_release_id, aliases))
}

pub(super) fn stable_content_id(item: &InstanceContentSnapshotItem) -> String {
    item.entry_id
        .as_deref()
        .or(item.member_id.as_deref())
        .or(item.file_id.as_deref())
        .unwrap_or(&item.expected_relative_path)
        .to_string()
}

pub(super) fn is_world_datapack(path: &str) -> bool {
    let components = path
        .replace('\\', "/")
        .split('/')
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();
    components.len() >= 4
        && components[0] == "saves"
        && components[2] == "datapacks"
}

//! Local projection; no directory scan, remote query or dependency discovery.

use super::*;
use crate::resources::{ArtifactIntegrity, ContentHash, HashAlgorithm};
use crate::state::store::content_snapshot::SnapshotRecords;

pub(crate) async fn get(
    instance_id: &str,
    state: &State,
) -> crate::Result<InstanceContentSnapshot> {
    let started = std::time::Instant::now();
    let records = crate::state::store::content_snapshot::get_with_saved_pack(
        &state.data,
        &state.derived,
        instance_id,
    )
    .await?;
    let result = project(records);
    tracing::debug!(
        instance_id,
        elapsed_us = started.elapsed().as_micros() as u64,
        "Read local content snapshot"
    );
    Ok(result)
}

pub(crate) fn project(records: SnapshotRecords) -> InstanceContentSnapshot {
    let SnapshotRecords {
        pack_presentation,
        fingerprints,
        presentations,
        instance,
        content_set,
        link,
        files,
        entries,
        members,
        pending,
        dependencies,
        references: refs_by_entry,
        origins: origin_by_entry,
        artifacts,
        entry_resources,
        pack_resources,
        pack_resource,
        preferred_source,
        checks,
    } = records;
    let instance_id = instance.id.clone();
    let location_key =
        crate::state::store::content_refresh_checks::location_key(&instance);
    let entries_by_file_id = entries
        .iter()
        .filter_map(|entry| {
            entry.file_id.as_ref().map(|file_id| (file_id, entry))
        })
        .collect::<HashMap<_, _>>();
    let members_by_entry_id = members
        .iter()
        .filter_map(|member| {
            member
                .content_entry_id
                .as_ref()
                .map(|entry_id| (entry_id, member))
        })
        .collect::<HashMap<_, _>>();
    let mut represented_members = HashSet::new();
    let local_declarations = files
        .iter()
        .filter_map(|file| {
            let metadata = serde_json::from_str::<
                crate::mod_metadata::LocalModMetadata,
            >(file.local_mod_data.as_deref()?)
            .ok()?;
            Some((
                file.id.clone(),
                crate::resources::ReleaseCompatibility {
                    game_versions: metadata
                        .minecraft_version
                        .into_iter()
                        .collect(),
                    loaders: metadata.loader.into_iter().collect(),
                },
            ))
        })
        .collect::<HashMap<_, _>>();
    let mut items = Vec::new();

    for file in files.into_iter().filter(|file| !file.missing) {
        let Some(project_type) = super::project_type_for_file(&file) else {
            continue;
        };
        let entry = entries_by_file_id.get(&file.id).copied();
        let member =
            entry.and_then(|entry| members_by_entry_id.get(&entry.id).copied());
        if let Some(member) = member {
            represented_members.insert(member.id.as_str());
        }
        let ownership_kind = entry
            .map_or(ContentOwnershipKind::LocalDiscovered, |entry| {
                entry.ownership_kind
            });
        let provider_refs = entry
            .and_then(|entry| refs_by_entry.get(&entry.id))
            .cloned()
            .unwrap_or_default();
        let origin_provider = entry
            .and_then(|entry| origin_by_entry.get(&entry.id))
            .copied();
        let local_mod = file.local_mod_data.as_ref().and_then(|json| {
            serde_json::from_str::<crate::mod_metadata::LocalModMetadata>(json)
                .ok()
        });
        let title = local_mod
            .as_ref()
            .and_then(|metadata| metadata.name.clone())
            .or_else(|| {
                std::path::Path::new(&file.file_name)
                    .file_stem()
                    .map(|stem| stem.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| file.file_name.clone());
        let external = provider_refs.is_empty();
        let mut content = ContentItem {
            file_name: file.file_name.clone(),
            file_path: file.relative_path.clone(),
            id: file.sha1.clone(),
            size: file.size,
            enabled: entry
                .map_or(file.enabled, |entry| entry.enabled && file.enabled),
            project_type,
            project: Some(ContentItemProject {
                id: local_mod
                    .as_ref()
                    .map(|metadata| format!("local:{}", metadata.mod_id))
                    .unwrap_or_else(|| format!("local:file:{}", file.sha1)),
                slug: local_mod
                    .as_ref()
                    .map(|metadata| metadata.mod_id.clone()),
                title,
                icon_url: file.icon_path.clone(),
                license: None,
            }),
            version: local_mod.as_ref().and_then(|metadata| {
                metadata.version.clone().map(|version| ContentItemVersion {
                    id: format!("local:{}", metadata.mod_id),
                    version_number: version,
                    file_name: file.file_name.clone(),
                    date_published: None,
                })
            }),
            owner: local_mod
                .as_ref()
                .and_then(|metadata| metadata.authors.first())
                .map(|author| crate::state::ContentItemOwner {
                    id: format!("local:{author}"),
                    name: author.clone(),
                    avatar_url: None,
                    owner_type: crate::state::OwnerType::User,
                }),
            update: None,
            date_added: Some(file.modified_at.to_rfc3339()),
            provider_refs,
            origin_provider,
            rollback: None,
            environment: None,
            source_kind: entry.map(|entry| entry.source_kind),
            external,
            loader: local_mod.as_ref().and_then(|meta| meta.loader.clone()),
        };
        if let Some(record) = presentations.get(&file.id)
            && record.location_key == location_key
            && record.content.id.eq_ignore_ascii_case(&content.id)
            && record.content.size == content.size
            && record.content.origin_provider == content.origin_provider
        {
            content.project =
                record.content.project.clone().or(content.project);
            content.version =
                record.content.version.clone().or(content.version);
            content.owner = record.content.owner.clone().or(content.owner);
            content.environment = record.content.environment;
            content.loader = record.content.loader.clone().or(content.loader);
            content.external = record.content.external;
        }
        items.push(super::snapshot_item(
            Some(file.id.clone()),
            entry.map(|entry| entry.id.clone()),
            member,
            ownership_kind,
            project_type,
            file.relative_path,
            Some(content),
        ));
    }

    for member in &members {
        if represented_members.contains(member.id.as_str()) {
            continue;
        }
        if !filesystem::is_scannable_project_path(
            member.project_type,
            &member.expected_relative_path,
        ) {
            continue;
        }
        items.push(super::snapshot_item(
            None,
            member.content_entry_id.clone(),
            Some(member),
            ContentOwnershipKind::PackManaged,
            member.project_type,
            member.expected_relative_path.clone(),
            None,
        ));
    }

    let target = crate::resources::ipc::ResourceTarget {
        game_versions: vec![content_set.game_version.clone()],
        loaders: vec![content_set.loader.as_str().into()],
        channels: crate::resources::plans::update_channels(
            instance.update_channel,
        ),
    };
    let checks = checks
        .into_iter()
        .map(|(key, record)| (key.entry_id, record))
        .collect::<HashMap<_, _>>();
    let mut warnings = Vec::new();
    for item in &mut items {
        item.dependency = item
            .entry_id
            .as_ref()
            .and_then(|id| dependencies.get(id))
            .cloned();
        item.resource_id = item
            .entry_id
            .as_ref()
            .and_then(|id| entry_resources.get(id))
            .cloned();
        item.pack_resource_id = item
            .member_id
            .as_ref()
            .and_then(|id| pack_resources.get(id))
            .cloned();
        let Some((size, sha1)) = item
            .content
            .as_ref()
            .map(|content| (content.size, content.id.clone()))
        else {
            continue;
        };
        let integrity = ArtifactIntegrity {
            size: Some(size),
            hashes: vec![ContentHash {
                algorithm: HashAlgorithm::Sha1,
                value: sha1.clone(),
            }],
        };
        let artifact = item
            .entry_id
            .as_ref()
            .and_then(|id| artifacts.get(id))
            .filter(|record| {
                crate::resources::identity::exact_integrity(
                    &integrity,
                    &record.artifact.integrity,
                )
            });
        if let Some(record) = artifact {
            item.artifact_ref = Some(record.artifact.reference.clone());
            item.capabilities.can_change_version = item.materialization_state
                == PackMemberMaterializationState::Present;
            item.compatibility =
                record.declared_compatibility.as_ref().map(|declared| {
                    let kind = match item.project_type {
                        crate::state::ProjectType::ResourcePack => {
                            crate::resources::ResourceType::ResourcePack
                        }
                        crate::state::ProjectType::DataPack => {
                            crate::resources::ResourceType::DataPack
                        }
                        crate::state::ProjectType::ShaderPack => {
                            crate::resources::ResourceType::Shader
                        }
                        _ => crate::resources::ResourceType::Mod,
                    };
                    crate::resources::assess_declared_compatibility(
                        declared, &target, kind,
                    )
                });
        }
        if let Some(declared) = item
            .file_id
            .as_ref()
            .and_then(|id| local_declarations.get(id))
            && item.project_type == crate::state::ProjectType::Mod
        {
            let local = crate::resources::assess_declared_compatibility(
                declared,
                &target,
                crate::resources::ResourceType::Mod,
            );
            if let Some(compatibility) = &mut item.compatibility {
                use crate::resources::CompatibilityState;
                if compatibility.game_version == CompatibilityState::Unknown {
                    compatibility.game_version = local.game_version;
                }
                if compatibility.loader == CompatibilityState::Unknown {
                    compatibility.loader = local.loader;
                }
                compatibility
                    .reasons
                    .retain(|reason| match reason.as_str() {
                        "gameVersion" => {
                            compatibility.game_version
                                != CompatibilityState::Compatible
                        }
                        "loader" => {
                            compatibility.loader
                                != CompatibilityState::Compatible
                        }
                        _ => true,
                    });
            } else {
                item.compatibility = Some(local);
            }
        }
        item.capabilities.can_update = false;
        let Some(check) = item.entry_id.as_ref().and_then(|id| checks.get(id))
        else {
            continue;
        };
        if check.file_id != item.file_id.as_deref().unwrap_or_default()
            || fingerprints.get(&check.file_id).is_some_and(|fingerprint| {
                fingerprint.content_root != check.context.content_root
            })
            || check.context.location_key != location_key
            || check.context.sha1 != sha1
            || check.context.size != size
            || check.context.target != target
            || check.context.preferred_source != preferred_source
            || check.context.current_release.as_ref()
                != item.artifact_ref.as_ref().map(|artifact| &artifact.release)
        {
            continue;
        }
        item.resource_update = check
            .last_success
            .as_ref()
            .and_then(|success| success.candidate.clone());
        item.capabilities.can_update = item.resource_update.is_some()
            && item.materialization_state
                == PackMemberMaterializationState::Present;
        if let Some(error) = &check.error {
            warnings.push(InstanceContentWarning {
                entry_id: item.entry_id.clone(),
                code: "resource_update_unavailable".into(),
                message: error.clone(),
                provider: item.provider,
            });
        }
    }
    let mut pack = super::pack_snapshot(
        &instance.name,
        instance.icon_path,
        &link,
        &members,
        None,
    );
    if let Some(saved) = pack_presentation
        && saved.location_key == location_key
        && saved.link_key == serde_json::to_string(&link).unwrap_or_default()
    {
        pack = saved.pack.or(pack);
    }
    if let Some(pack) = &mut pack {
        pack.resource_id = pack_resource;
    }
    items.sort_by_key(|item| item.expected_relative_path.to_lowercase());
    InstanceContentSnapshot {
        instance_id,
        revision: content_set.revision,
        pack,
        items,
        pending_manual_downloads: pending,
        warnings,
    }
}

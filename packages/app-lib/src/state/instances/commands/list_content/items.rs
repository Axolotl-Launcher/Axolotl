#![allow(dead_code)]

use super::*;

pub(crate) async fn dependencies_to_content_items(
    dependencies: &[Dependency],
    cache_behaviour: Option<CacheBehaviour>,
    pool: &SqlitePool,
    fetch_semaphore: &FetchSemaphore,
) -> crate::Result<Vec<ContentItem>> {
    let project_ids = dependencies
        .iter()
        .filter_map(|dependency| dependency.project_id.clone())
        .collect::<HashSet<_>>();
    if project_ids.is_empty() {
        return Ok(Vec::new());
    }
    let version_ids = dependencies
        .iter()
        .filter_map(|dependency| dependency.version_id.clone())
        .collect::<HashSet<_>>();
    let meta = resolve_metadata(
        &project_ids,
        &version_ids,
        cache_behaviour,
        pool,
        fetch_semaphore,
    )
    .await?;
    let mut items = dependencies
        .iter()
        .filter_map(|dependency| {
            let project_id = dependency.project_id.as_ref()?;
            let project = meta
                .projects
                .iter()
                .find(|project| &project.id == project_id)?;
            let version =
                dependency.version_id.as_ref().and_then(|version_id| {
                    meta.versions
                        .iter()
                        .find(|version| &version.id == version_id)
                });
            let owner =
                resolve_owner(project, &meta.teams, &meta.organizations);
            let project_type =
                project_type_from_api_name(&project.project_type);

            Some(ContentItem {
                file_name: version
                    .and_then(|version| version.files.first())
                    .map(|file| file.filename.clone())
                    .unwrap_or_else(|| {
                        format!(
                            "{}.jar",
                            project.slug.as_deref().unwrap_or(&project.id)
                        )
                    }),
                file_path: String::new(),
                id: String::new(),
                size: version
                    .and_then(|version| version.files.first())
                    .map(|file| file.size as u64)
                    .unwrap_or(0),
                enabled: true,
                project_type,
                project: Some(ContentItemProject {
                    id: project.id.clone(),
                    slug: project.slug.clone(),
                    title: project.title.clone(),
                    icon_url: project.icon_url.clone(),
                    license: Some(project.license.clone()),
                }),
                version: version.map(|version| ContentItemVersion {
                    id: version.id.clone(),
                    version_number: version.version_number.clone(),
                    file_name: version
                        .files
                        .first()
                        .map(|file| file.filename.clone())
                        .unwrap_or_default(),
                    date_published: Some(version.date_published.to_rfc3339()),
                }),
                owner,
                update: None,
                date_added: None,
                provider_refs: vec![ContentProviderRef::Modrinth {
                    project_id: crate::state::ModrinthProjectId::new(
                        project.id.clone(),
                    )
                    .ok()?,
                    version_id: version
                        .map(|version| {
                            crate::state::ModrinthVersionId::new(
                                version.id.clone(),
                            )
                        })
                        .transpose()
                        .ok()?,
                }],
                origin_provider: Some(ContentProvider::Modrinth),
                rollback: None,
                environment: None,
                source_kind: None,
                external: false,
                loader: version
                    .and_then(|version| version.loaders.first().cloned()),
            })
        })
        .collect::<Vec<_>>();
    sort_content_items(&mut items);

    Ok(items)
}

pub(super) async fn content_projects_for_scope(
    resolved: &ResolvedContentScope,
    cache_behaviour: Option<CacheBehaviour>,
    state: &State,
    filter: ContentFilter<'_>,
    _refresh_file_updates: bool,
) -> crate::Result<DashMap<String, ContentFile>> {
    let files = if cache_behaviour == Some(CacheBehaviour::CacheOnly) {
        sqlite::files::get_instance_files(&resolved.instance.id, &state.pool)
            .await?
    } else {
        sync_instance_content_files(&resolved.instance, state).await?
    };

    content_projects_for_files(resolved, files, cache_behaviour, state, filter)
        .await
}

pub(super) async fn content_projects_for_files(
    resolved: &ResolvedContentScope,
    files: Vec<InstanceFile>,
    cache_behaviour: Option<CacheBehaviour>,
    state: &State,
    filter: ContentFilter<'_>,
) -> crate::Result<DashMap<String, ContentFile>> {
    let mut entry_maps =
        load_entry_maps(&resolved.content_set.id, &state.pool).await?;
    let hashes = files
        .iter()
        .map(|file| file.sha1.as_str())
        .collect::<Vec<_>>();
    let file_info = CachedEntry::get_file_many(
        &hashes,
        cache_behaviour,
        state.cache_pool(),
        &state.api_semaphore,
    )
    .await?;
    let file_info_by_hash = file_info
        .into_iter()
        .map(|file| (file.hash.clone(), file))
        .collect::<HashMap<_, _>>();
    let reconciled = if matches!(filter, ContentFilter::All)
        && cache_behaviour != Some(CacheBehaviour::CacheOnly)
    {
        reconcile_hash_matched_entries(
            resolved,
            &files,
            &file_info_by_hash,
            state,
        )
        .await?
    } else {
        false
    };
    if reconciled {
        entry_maps =
            load_entry_maps(&resolved.content_set.id, &state.pool).await?;
    }
    let output = DashMap::new();

    for file in files {
        if file.missing {
            continue;
        }

        let Some(project_type) = project_type_for_file(&file) else {
            continue;
        };
        let metadata = file_info_by_hash.get(&file.sha1).cloned();
        let entry = entry_maps.entries_by_file_id.get(&file.id);
        let provider_refs = entry_maps
            .provider_refs_by_file_id
            .get(&file.id)
            .cloned()
            .unwrap_or_default();
        let origin_provider =
            entry_maps.origin_provider_by_file_id.get(&file.id).copied();
        let modrinth_metadata = metadata.as_ref().and_then(modrinth_file_match);

        match filter {
            ContentFilter::All => {}
            ContentFilter::ExcludeModpack(ids) => {
                if ids.is_modpack_file(
                    &file.sha1,
                    metadata.as_ref(),
                    provider_refs.iter().find_map(
                        |reference| match reference {
                            ContentProviderRef::Modrinth {
                                project_id, ..
                            } => Some(project_id.as_str()),
                            ContentProviderRef::CurseForge { .. } => None,
                            ContentProviderRef::McArchive { .. } => None,
                        },
                    ),
                ) {
                    continue;
                }
            }
            ContentFilter::ExcludeSourceKind {
                source_kind,
                exclude_untracked,
            } => {
                if entry.is_some_and(|entry| entry.source_kind == source_kind)
                    || (exclude_untracked && entry.is_none())
                {
                    continue;
                }
            }
            ContentFilter::OnlySourceKind {
                source_kind,
                include_untracked,
            } => {
                if !include_untracked && entry.is_none() {
                    continue;
                }
                if entry.is_some_and(|entry| entry.source_kind != source_kind) {
                    continue;
                }
            }
            ContentFilter::OnlyModpack(ids) => {
                if !ids.is_modpack_file(
                    &file.sha1,
                    metadata.as_ref(),
                    provider_refs.iter().find_map(
                        |reference| match reference {
                            ContentProviderRef::Modrinth {
                                project_id, ..
                            } => Some(project_id.as_str()),
                            ContentProviderRef::CurseForge { .. } => None,
                            ContentProviderRef::McArchive { .. } => None,
                        },
                    ),
                ) {
                    continue;
                }
            }
        }

        output.insert(
            file.relative_path.clone(),
            ContentFile {
                update: None,
                hash: file.sha1,
                file_name: file.file_name,
                enabled: entry.map_or(file.enabled, |entry| {
                    entry.enabled && file.enabled
                }),
                size: file.size,
                modrinth: modrinth_metadata,
                provider_refs,
                origin_provider,
                project_type,
                local_mod_data: file.local_mod_data,
                icon_path: file.icon_path,
            },
        );
    }

    Ok(output)
}

pub(super) async fn reconcile_hash_matched_entries(
    resolved: &ResolvedContentScope,
    files: &[InstanceFile],
    file_info_by_hash: &HashMap<String, crate::state::ModrinthHashMatch>,
    state: &State,
) -> crate::Result<bool> {
    let _instance_lock =
        state.lock_instance_content(&resolved.instance.id).await;
    let entry_maps =
        load_entry_maps(&resolved.content_set.id, &state.pool).await?;
    let candidates = files
        .iter()
        .filter_map(|file| {
            if file.missing {
                return None;
            }
            let metadata = file_info_by_hash.get(&file.sha1)?;
            let provider_ref = ContentProviderRef::Modrinth {
                project_id: ModrinthProjectId::new(metadata.project_id.clone())
                    .ok()?,
                version_id: Some(
                    ModrinthVersionId::new(metadata.version_id.clone()).ok()?,
                ),
            };
            let provider_refs = entry_maps
                .provider_refs_by_file_id
                .get(&file.id)
                .map(Vec::as_slice)
                .unwrap_or_default();
            if provider_refs.iter().any(|reference| {
                matches!(
                    reference,
                    ContentProviderRef::Modrinth {
                        project_id,
                        version_id: Some(version_id),
                    } if project_id.as_str() == metadata.project_id
                        && version_id.as_str() == metadata.version_id
                )
            }) {
                return None;
            }
            let project_type = project_type_for_file(file)?;
            let entry = entry_maps.entries_by_file_id.get(&file.id);
            Some((file, project_type, entry, provider_ref))
        })
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return Ok(false);
    }

    let mut tx = state.pool.begin_with("BEGIN IMMEDIATE").await?;
    sqlite::write_guards::ensure_content_write_parents(
        &resolved.instance.id,
        &resolved.content_set.id,
        &mut tx,
    )
    .await?;
    for (file, project_type, existing_entry, provider_ref) in &candidates {
        let entry_id = if let Some(entry) = existing_entry {
            entry.id.clone()
        } else {
            sqlite::content_entries::upsert_content_entry_from_parts_in_transaction(
                sqlite::content_entries::UpsertContentEntry {
                    instance_id: &resolved.instance.id,
                    content_set_id: &resolved.content_set.id,
                    file_id: Some(&file.id),
                    project_type: *project_type,
                    source_kind: ContentSourceKind::Local,
                    ownership_kind: ContentOwnershipKind::UserAdded,
                    auto_dependency: false,
                    server_requirement: ContentRequirement::Required,
                    client_requirement: ContentRequirement::Required,
                    enabled: file.enabled,
                },
                &mut tx,
            )
            .await?
            .id
        };
        sqlite::content_entries::upsert_content_provider_ref_in_transaction(
            &entry_id,
            provider_ref,
            false,
            &mut tx,
        )
        .await?;
    }
    sqlite::content_sets::bump_content_set_revision_in_transaction(
        &resolved.content_set.id,
        &mut tx,
    )
    .await?;
    tx.commit().await?;

    Ok(true)
}

pub(super) async fn content_files_to_content_items(
    instance: &Instance,
    files: &[(String, ContentFile)],
    cache_behaviour: Option<CacheBehaviour>,
    state: &State,
) -> crate::Result<Vec<ContentItem>> {
    let mut provider_refs_by_path =
        HashMap::<String, Vec<ContentProviderRef>>::new();
    let mut origin_provider_by_path = HashMap::<String, ContentProvider>::new();
    let mut source_kind_by_path = HashMap::<String, ContentSourceKind>::new();
    for (path, reference, origin, source_kind) in
        sqlite::content_entries::get_provider_refs_by_path(
            &instance.id,
            &state.pool,
        )
        .await?
    {
        provider_refs_by_path
            .entry(path.clone())
            .or_default()
            .push(reference);
        if let Some(provider) = origin {
            origin_provider_by_path.insert(path.clone(), provider);
        }
        if let Some(source_kind) = source_kind {
            source_kind_by_path.insert(path, source_kind);
        }
    }
    let curseforge_project_ids = provider_refs_by_path
        .values()
        .flatten()
        .filter_map(|reference| match reference {
            ContentProviderRef::CurseForge { project_id, .. } => {
                Some(project_id.get())
            }
            ContentProviderRef::Modrinth { .. } => None,
            ContentProviderRef::McArchive { .. } => None,
        })
        .collect::<HashSet<_>>();
    let curseforge_projects = if cache_behaviour
        == Some(CacheBehaviour::CacheOnly)
        || curseforge_project_ids.is_empty()
        || crate::api::curseforge::capability().status
            != crate::api::curseforge::CurseForgeCapabilityStatus::Ready
    {
        HashMap::new()
    } else {
        crate::api::curseforge::get_projects(
            curseforge_project_ids.into_iter().collect(),
        )
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|project| (project.id.to_string(), project))
        .collect::<HashMap<_, _>>()
    };
    let project_ids = files
        .iter()
        .filter_map(|(_, file)| {
            file.modrinth
                .as_ref()
                .map(|metadata| metadata.project_id.to_string())
        })
        .collect::<HashSet<_>>();
    let mut version_ids = HashSet::new();
    for (_, file) in files {
        if let Some(metadata) = file.modrinth.as_ref() {
            version_ids.insert(metadata.version_id.to_string());
        }
        if let Some(version_id) =
            installed_modrinth_version_id(&file.provider_refs)
        {
            version_ids.insert(version_id.to_string());
        }
    }
    let meta = resolve_metadata(
        &project_ids,
        &version_ids,
        cache_behaviour,
        state.cache_pool(),
        &state.api_semaphore,
    )
    .await?;
    let instance_path = instance_content_root(&state.directories, instance)?;
    let paths = files
        .iter()
        .map(|(path, _)| join_content_path(&instance_path, path))
        .collect::<Vec<_>>();
    let modification_times: Vec<Option<String>> =
        tokio::task::spawn_blocking(move || {
            paths
                .iter()
                .map(|path| {
                    std::fs::metadata(path)
                        .and_then(|metadata| metadata.modified())
                        .ok()
                        .map(|time| {
                            chrono::DateTime::<chrono::Utc>::from(time)
                                .to_rfc3339()
                        })
                })
                .collect()
        })
        .await?;
    // Backups are read from the resolved root: the linked installation for
    // directly associated instances, the `game_dir_override` target (or the
    // profile directory) otherwise.
    let content_backups =
        crate::state::instances::adapters::filesystem::scan_content_backups_from(
            &instance_path,
        )?;
    let mut items = files
        .iter()
        .enumerate()
        .map(|(index, (path, file))| {
            let provider_refs = provider_refs_by_path
                .get(path)
                .cloned()
                .or_else(|| {
                    file.modrinth.as_ref().map(|metadata| {
                        vec![ContentProviderRef::Modrinth {
                            project_id: metadata.project_id.clone(),
                            version_id: Some(metadata.version_id.clone()),
                        }]
                    })
                })
                .unwrap_or_default();
            let origin_provider = origin_provider_by_path.get(path).copied();
            let curseforge_ref = provider_refs.iter().find(|reference| {
                reference.provider() == ContentProvider::CurseForge
            });
            let curseforge_project = curseforge_ref.and_then(|reference| {
                let ContentProviderRef::CurseForge { project_id, .. } =
                    reference
                else {
                    return None;
                };
                curseforge_projects.get(&project_id.get().to_string())
            });
            let curseforge_file = curseforge_ref.and_then(|reference| {
                let ContentProviderRef::CurseForge { file_id, .. } = reference
                else {
                    return None;
                };
                let file_id = file_id.as_ref()?.get();
                curseforge_project?
                    .latest_files
                    .iter()
                    .find(|file| file.id == file_id)
            });
            let project = file.modrinth.as_ref().and_then(|metadata| {
                meta.projects
                    .iter()
                    .find(|project| project.id == metadata.project_id.as_str())
            });
            let installed_version_id =
                installed_modrinth_version_id(&provider_refs);
            let version = installed_version_id
                .as_ref()
                .and_then(|version_id| {
                    meta.versions
                        .iter()
                        .find(|version| version.id == version_id.as_str())
                })
                .or_else(|| {
                    file.modrinth.as_ref().and_then(|metadata| {
                        meta.versions.iter().find(|version| {
                            version.id == metadata.version_id.as_str()
                        })
                    })
                });
            let version_v3 = installed_version_id
                .as_ref()
                .and_then(|version_id| {
                    meta.versions_v3
                        .iter()
                        .find(|version| version.id == version_id.as_str())
                })
                .or_else(|| {
                    file.modrinth.as_ref().and_then(|metadata| {
                        meta.versions_v3.iter().find(|version| {
                            version.id == metadata.version_id.as_str()
                        })
                    })
                });
            let environment =
                version_v3.and_then(|version| version.environment);
            let source_kind = source_kind_by_path.get(path).copied();
            let external = provider_refs.is_empty();
            // Parse local_mod_data for fallback display when Modrinth /
            // CurseForge has no match for this file.
            let local_mod = file.local_mod_data.as_ref().and_then(|json| {
                serde_json::from_str::<crate::mod_metadata::LocalModMetadata>(
                    json,
                )
                .ok()
            });
            // Loader priority: locally parsed mod metadata (most reliable for
            // CurseForge-only files, whose CF game versions often omit the
            // loader token) → installed version loaders → local metadata.
            let local_loader =
                local_mod.as_ref().and_then(|meta| meta.loader.clone());
            let loader = local_loader
                .as_deref()
                .filter(|loader| {
                    matches!(*loader, "fabric" | "forge" | "quilt" | "neoforge")
                })
                .map(str::to_string)
                .or_else(|| {
                    version.and_then(|version| version.loaders.first().cloned())
                })
                .or(local_loader);
            let owner = project.and_then(|project| {
                resolve_owner(project, &meta.teams, &meta.organizations)
            });

            let cached_icon = file
                .icon_path
                .as_ref()
                .filter(|path| !path.is_empty())
                .cloned();

            ContentItem {
                file_name: file.file_name.clone(),
                file_path: path.clone(),
                id: file.hash.clone(),
                size: file.size,
                enabled: file.enabled,
                project_type: file.project_type,
                project: project
                    .map(|project| ContentItemProject {
                        id: project.id.clone(),
                        slug: project.slug.clone(),
                        title: project.title.clone(),
                        icon_url: project.icon_url.clone(),
                        license: Some(project.license.clone()),
                    })
                    .or_else(|| {
                        curseforge_project.map(|project| ContentItemProject {
                            id: project.id.to_string(),
                            slug: Some(project.slug.clone()),
                            title: project.name.clone(),
                            icon_url: project
                                .logo
                                .as_ref()
                                .map(|logo| logo.thumbnail_url.clone()),
                            license: None,
                        })
                    })
                    .or_else(|| {
                        local_mod.as_ref().map(|meta| ContentItemProject {
                            id: local_id(&meta.mod_id),
                            slug: Some(meta.mod_id.clone()),
                            title: meta
                                .name
                                .clone()
                                .unwrap_or_else(|| meta.mod_id.clone()),
                            icon_url: cached_icon.clone(),
                            license: None,
                        })
                    })
                    .or_else(|| {
                        // Unmatched packs without embedded mod metadata still
                        // get a project so rows can render their name and the
                        // cached icon extracted from the archive.
                        (!file.file_name.is_empty()).then(|| {
                            ContentItemProject {
                                id: local_id(format_args!(
                                    "file:{}",
                                    file.hash
                                )),
                                slug: None,
                                title: Path::new(&file.file_name)
                                    .file_stem()
                                    .map(|stem| {
                                        stem.to_string_lossy().into_owned()
                                    })
                                    .unwrap_or_else(|| file.file_name.clone()),
                                icon_url: cached_icon.clone(),
                                license: None,
                            }
                        })
                    }),
                version: version
                    .map(|version| ContentItemVersion {
                        id: version.id.clone(),
                        version_number: version.version_number.clone(),
                        file_name: file.file_name.clone(),
                        date_published: Some(
                            version.date_published.to_rfc3339(),
                        ),
                    })
                    .or_else(|| {
                        curseforge_file.map(|version| ContentItemVersion {
                            id: version.id.to_string(),
                            version_number: version.display_name.clone(),
                            file_name: version.file_name.clone(),
                            date_published: Some(version.file_date.clone()),
                        })
                    })
                    .or_else(|| {
                        local_mod.as_ref().and_then(|meta| {
                            meta.version.clone().map(|v| ContentItemVersion {
                                id: local_id(&meta.mod_id),
                                version_number: v,
                                file_name: file.file_name.clone(),
                                date_published: None,
                            })
                        })
                    }),
                owner: owner
                    .or_else(|| {
                        curseforge_project
                            .and_then(|project| project.authors.first())
                            .map(|author| ContentItemOwner {
                                id: author.id.to_string(),
                                name: author.name.clone(),
                                avatar_url: None,
                                owner_type: OwnerType::User,
                            })
                    })
                    .or_else(|| {
                        local_mod.as_ref().and_then(|meta| {
                            meta.authors.first().map(|author| {
                                ContentItemOwner {
                                    // The author comes from the local mod file's
                                    // own manifest, not from a resolved project:
                                    // the prefix here marks a self-declared
                                    // author, not an unresolved project.
                                    id: local_id(author),
                                    name: author.clone(),
                                    avatar_url: None,
                                    owner_type: OwnerType::User,
                                }
                            })
                        })
                    }),
                update: None,
                date_added: modification_times[index].clone(),
                provider_refs,
                origin_provider,
                rollback: rollback_for_content_file(
                    path,
                    &file.file_name,
                    &content_backups,
                ),
                environment,
                source_kind,
                external,
                loader,
            }
        })
        .collect::<Vec<_>>();
    sort_content_items(&mut items);

    Ok(items)
}

pub(super) fn rollback_for_content_file(
    relative_path: &str,
    file_name: &str,
    backups: &[crate::state::instances::adapters::filesystem::ScannedBackupFile],
) -> Option<ContentItemRollback> {
    let base = file_name.trim_end_matches(".disabled");
    let prefix = format!("{base}_");
    let dir = relative_path
        .rsplit_once('/')
        .map(|(dir, _)| dir)
        .unwrap_or("");
    let backup_name = backups
        .iter()
        .filter(|backup| {
            backup.file_name.starts_with(&prefix)
                && backup.file_name.ends_with(".old")
                && backup
                    .relative_path
                    .rsplit_once('/')
                    .map(|(backup_dir, _)| backup_dir)
                    .unwrap_or("")
                    == dir
        })
        .min_by(|left, right| {
            left.modified
                .cmp(&right.modified)
                .then_with(|| left.file_name.cmp(&right.file_name))
        })
        .map(|backup| backup.file_name.as_str())?;
    let old_base = backup_name.strip_prefix(&prefix)?.strip_suffix(".old")?;
    if old_base.is_empty() {
        return None;
    }
    Some(ContentItemRollback {
        file_name: old_base.to_string(),
    })
}

pub(super) struct ResolvedMetadata {
    pub(super) projects: Vec<Project>,
    pub(super) versions: Vec<Version>,
    pub(super) versions_v3: Vec<VersionV3>,
    pub(super) teams: Vec<Vec<TeamMember>>,
    pub(super) organizations: Vec<Organization>,
}

pub(super) async fn resolve_metadata(
    project_ids: &HashSet<String>,
    version_ids: &HashSet<String>,
    cache_behaviour: Option<CacheBehaviour>,
    pool: &SqlitePool,
    fetch_semaphore: &FetchSemaphore,
) -> crate::Result<ResolvedMetadata> {
    let project_id_refs = project_ids
        .iter()
        .map(|id| ModrinthProjectId::new(id.clone()))
        .collect::<crate::Result<Vec<_>>>()?;
    let version_id_refs = version_ids
        .iter()
        .map(|id| ModrinthVersionId::new(id.clone()))
        .collect::<crate::Result<Vec<_>>>()?;
    let (projects, versions, versions_v3) =
        if !project_ids.is_empty() || !version_ids.is_empty() {
            tokio::try_join!(
                async {
                    if project_ids.is_empty() {
                        Ok(Vec::new())
                    } else {
                        CachedEntry::get_project_many(
                            &project_id_refs,
                            cache_behaviour,
                            pool,
                            fetch_semaphore,
                        )
                        .await
                    }
                },
                async {
                    if version_ids.is_empty() {
                        Ok(Vec::new())
                    } else {
                        CachedEntry::get_version_many(
                            &version_id_refs,
                            cache_behaviour,
                            pool,
                            fetch_semaphore,
                        )
                        .await
                    }
                },
                async {
                    if version_ids.is_empty() {
                        Ok(Vec::new())
                    } else {
                        CachedEntry::get_version_v3_many(
                            &version_id_refs,
                            cache_behaviour,
                            pool,
                            fetch_semaphore,
                        )
                        .await
                    }
                }
            )?
        } else {
            (Vec::new(), Vec::new(), Vec::new())
        };
    let team_ids = projects
        .iter()
        .map(|project| project.team.clone())
        .filter(|team| !team.is_empty())
        .collect::<HashSet<_>>();
    let org_ids = projects
        .iter()
        .filter_map(|project| project.organization.clone())
        .collect::<HashSet<_>>();
    let team_id_refs = team_ids.iter().map(String::as_str).collect::<Vec<_>>();
    let org_id_refs = org_ids.iter().map(String::as_str).collect::<Vec<_>>();
    let (teams, organizations) = if !team_ids.is_empty() || !org_ids.is_empty()
    {
        tokio::try_join!(
            async {
                if team_ids.is_empty() {
                    Ok(Vec::new())
                } else {
                    CachedEntry::get_team_many(
                        &team_id_refs,
                        cache_behaviour,
                        pool,
                        fetch_semaphore,
                    )
                    .await
                }
            },
            async {
                if org_ids.is_empty() {
                    Ok(Vec::new())
                } else {
                    CachedEntry::get_organization_many(
                        &org_id_refs,
                        cache_behaviour,
                        pool,
                        fetch_semaphore,
                    )
                    .await
                }
            }
        )?
    } else {
        (Vec::new(), Vec::new())
    };

    Ok(ResolvedMetadata {
        projects,
        versions,
        versions_v3,
        teams,
        organizations,
    })
}

pub(super) fn resolve_owner(
    project: &Project,
    teams: &[Vec<TeamMember>],
    organizations: &[Organization],
) -> Option<ContentItemOwner> {
    if let Some(org_id) = &project.organization {
        organizations
            .iter()
            .find(|organization| &organization.id == org_id)
            .map(|organization| ContentItemOwner {
                id: organization.id.clone(),
                name: organization.name.clone(),
                avatar_url: organization.icon_url.clone(),
                owner_type: OwnerType::Organization,
            })
    } else {
        teams
            .iter()
            .find(|team| {
                team.first()
                    .is_some_and(|member| member.team_id == project.team)
            })
            .and_then(|team| team.iter().find(|member| member.is_owner))
            .map(|member| ContentItemOwner {
                id: member.user.id.clone(),
                name: member.user.username.clone(),
                avatar_url: member.user.avatar_url.clone(),
                owner_type: OwnerType::User,
            })
    }
}

pub(super) fn modrinth_file_match(
    cached: &crate::state::ModrinthHashMatch,
) -> Option<ModrinthFileMatch> {
    Some(ModrinthFileMatch {
        project_id: crate::state::ModrinthProjectId::new(
            cached.project_id.clone(),
        )
        .ok()?,
        version_id: crate::state::ModrinthVersionId::new(
            cached.version_id.clone(),
        )
        .ok()?,
    })
}

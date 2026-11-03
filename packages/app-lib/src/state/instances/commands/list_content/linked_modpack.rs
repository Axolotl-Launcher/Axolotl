#![allow(dead_code)]

use super::*;

pub(crate) async fn get_linked_modpack_info(
    instance_id: &str,
    content_set_id: Option<&str>,
    cache_behaviour: Option<CacheBehaviour>,
    state: &State,
) -> crate::Result<Option<LinkedModpackInfo>> {
    let resolved = resolve_content_scope_with_instance(
        instance_id,
        content_set_id,
        &state.pool,
    )
    .await?;
    let link =
        sqlite::links::get_instance_link(&resolved.instance.id, &state.pool)
            .await?;
    if let Some((project_id, version_id)) = linked_curseforge_modpack_ids(&link)
    {
        return get_curseforge_linked_modpack_info(
            project_id,
            version_id,
            resolved.instance.update_channel,
            cache_behaviour,
            &state.pool,
            &state.api_semaphore,
        )
        .await;
    }

    let Some((project_id, version_id)) = linked_modrinth_modpack_ids(&link)
    else {
        return Ok(None);
    };

    let project_id_ref = ModrinthProjectId::new(project_id.to_string())?;
    let version_id_ref = ModrinthVersionId::new(version_id.to_string())?;
    let (project, version, all_versions) = tokio::try_join!(
        CachedEntry::get_project(
            &project_id_ref,
            cache_behaviour,
            state.cache_pool(),
            &state.api_semaphore,
        ),
        CachedEntry::get_version(
            &version_id_ref,
            cache_behaviour,
            state.cache_pool(),
            &state.api_semaphore,
        ),
        CachedEntry::get_project_versions(
            &project_id_ref,
            cache_behaviour,
            state.cache_pool(),
            &state.api_semaphore,
        ),
    )?;
    let version = match version {
        Some(version) => version,
        None if matches!(cache_behaviour, Some(CacheBehaviour::CacheOnly)) => {
            return Ok(None);
        }
        None => {
            return Err(crate::ErrorKind::InputError(format!(
                "Linked modpack version {version_id} not found"
            ))
            .into());
        }
    };
    let (project, all_versions) = if version.project_id != project_id {
        let modpack_project_id_ref =
            ModrinthProjectId::new(version.project_id.clone())?;
        let (modpack_project, modpack_versions) = tokio::try_join!(
            CachedEntry::get_project(
                &modpack_project_id_ref,
                cache_behaviour,
                state.cache_pool(),
                &state.api_semaphore,
            ),
            CachedEntry::get_project_versions(
                &modpack_project_id_ref,
                cache_behaviour,
                state.cache_pool(),
                &state.api_semaphore,
            ),
        )?;
        (modpack_project.or(project), modpack_versions)
    } else {
        (project, all_versions)
    };
    let project = project.ok_or_else(|| {
        crate::ErrorKind::InputError(format!(
            "Linked modpack project {project_id} not found"
        ))
    })?;
    let owner = if let Some(org_id) = &project.organization {
        let org = CachedEntry::get_organization(
            org_id,
            cache_behaviour,
            state.cache_pool(),
            &state.api_semaphore,
        )
        .await?;
        org.map(|org| ContentItemOwner {
            id: org.id,
            name: org.name,
            avatar_url: org.icon_url,
            owner_type: OwnerType::Organization,
        })
    } else if !project.team.is_empty() {
        let team = CachedEntry::get_team(
            &project.team,
            cache_behaviour,
            state.cache_pool(),
            &state.api_semaphore,
        )
        .await?;
        team.and_then(|team| {
            team.into_iter()
                .find(|member| member.is_owner)
                .map(|member| ContentItemOwner {
                    id: member.user.id,
                    name: member.user.username,
                    avatar_url: member.user.avatar_url,
                    owner_type: OwnerType::User,
                })
        })
    } else {
        None
    };
    let (_, update_version_id, update_version) = check_modpack_update(
        version_id,
        &version,
        all_versions,
        resolved.instance.update_channel,
    );
    let modpack_project_id = ModrinthProjectId::new(project.id.clone())?;
    let current_version_id = ModrinthVersionId::new(version.id.clone())?;
    let update = update_version_id
        .map(ModrinthVersionId::new)
        .transpose()?
        .map(|target_version_id| ContentItemUpdate::Modrinth {
            project_id: modpack_project_id.clone(),
            current_version_id: current_version_id.clone(),
            target_version_id,
        });

    Ok(Some(LinkedModpackInfo {
        project,
        version,
        owner,
        update,
        update_version,
    }))
}

pub(super) async fn get_curseforge_linked_modpack_info(
    project_id: &str,
    version_id: &str,
    preferred_update_channel: ReleaseChannel,
    cache_behaviour: Option<CacheBehaviour>,
    pool: &SqlitePool,
    fetch_semaphore: &FetchSemaphore,
) -> crate::Result<Option<LinkedModpackInfo>> {
    // A normal content snapshot is explicitly cache-only. CurseForge file
    // metadata has no local cache representation, so attempting to enrich a
    // linked pack here would turn every page open into network I/O. The UI
    // already has an instance-backed fallback card; full metadata is fetched
    // by the explicit refresh path instead.
    if cache_behaviour == Some(CacheBehaviour::CacheOnly) {
        return Ok(None);
    }

    let numeric_project_id = project_id.parse::<u32>().map_err(|_| {
        crate::ErrorKind::InputError(format!(
            "Linked CurseForge project ID {project_id} is invalid"
        ))
    })?;
    let numeric_file_id = version_id.parse::<u32>().map_err(|_| {
        crate::ErrorKind::InputError(format!(
            "Linked CurseForge file ID {version_id} is invalid"
        ))
    })?;

    let (cf_project, cf_file) = tokio::join!(
        crate::api::curseforge::get_project(numeric_project_id),
        crate::api::curseforge::get_file(numeric_project_id, numeric_file_id),
    );
    let cf_project = cf_project.ok();
    let cf_file = cf_file.ok();

    let mr_slug = cf_project.as_ref().map(|project| project.slug.clone());
    let mr_data: Option<(Option<Project>, Option<Vec<Version>>)> =
        if let Some(ref slug) = mr_slug {
            if let Ok(mr_id) = ModrinthProjectId::new(slug.clone()) {
                match tokio::try_join!(
                    CachedEntry::get_project(
                        &mr_id,
                        cache_behaviour,
                        pool,
                        fetch_semaphore,
                    ),
                    CachedEntry::get_project_versions(
                        &mr_id,
                        cache_behaviour,
                        pool,
                        fetch_semaphore,
                    ),
                ) {
                    Ok((project_opt, versions_opt)) => {
                        Some((project_opt, versions_opt))
                    }
                    Err(_) => None,
                }
            } else {
                None
            }
        } else {
            None
        };

    let cf_data = cf_project.zip(cf_file);
    match (mr_data, cf_data) {
        (Some((Some(mr_project), mr_versions)), cf_tuple) => {
            let mr_version_opt = cf_tuple.as_ref().and_then(|(_, cf_file)| {
                mr_versions.as_ref().and_then(|versions| {
                    versions.iter().find(|v| {
                        v.version_number == cf_file.display_name
                            || v.id == cf_file.id.to_string()
                    })
                })
            });
            let version = if let Some(mr_v) = mr_version_opt {
                mr_v.clone()
            } else if let Some((_, cf_file)) = cf_tuple.as_ref() {
                curseforge_file_to_version(cf_file)
            } else {
                return Ok(None);
            };

            // Owner from Modrinth (with avatar)
            let owner = if let Some(org_id) = &mr_project.organization {
                CachedEntry::get_organization(
                    org_id,
                    cache_behaviour,
                    pool,
                    fetch_semaphore,
                )
                .await?
                .map(|org| ContentItemOwner {
                    id: org.id,
                    name: org.name,
                    avatar_url: org.icon_url,
                    owner_type: OwnerType::Organization,
                })
            } else if !mr_project.team.is_empty() {
                CachedEntry::get_team(
                    &mr_project.team,
                    cache_behaviour,
                    pool,
                    fetch_semaphore,
                )
                .await?
                .and_then(|team| {
                    team.into_iter().find(|member| member.is_owner).map(
                        |member| ContentItemOwner {
                            id: member.user.id,
                            name: member.user.username,
                            avatar_url: member.user.avatar_url,
                            owner_type: OwnerType::User,
                        },
                    )
                })
            } else {
                None
            };

            let files = crate::api::curseforge::get_files(
                numeric_project_id,
                crate::api::curseforge::CurseForgeFilesRequest {
                    game_version: None,
                    mod_loader_type: None,
                    game_version_type_id: None,
                    index: 0,
                    page_size: 50,
                },
            )
            .await
            .ok()
            .map(|response| response.files)
            .unwrap_or_default();
            let installed_cf_version = cf_tuple
                .as_ref()
                .map(|(_, cf_file)| curseforge_file_to_version(cf_file));
            let all_versions = files
                .into_iter()
                .filter(|file| file.is_available)
                .map(|file| curseforge_file_to_version(&file))
                .collect::<Vec<_>>();
            let cf_version_count = all_versions.len();
            let (_, update_version_id, update_version) = installed_cf_version
                .as_ref()
                .map_or((false, None, None), |installed| {
                    check_modpack_update(
                        &installed.id,
                        installed,
                        Some(all_versions),
                        preferred_update_channel,
                    )
                });
            let update = update_version_id.and_then(|target| {
                Some(ContentItemUpdate::CurseForge {
                    project_id: crate::state::CurseForgeProjectId::new(
                        numeric_project_id,
                    )
                    .ok()?,
                    current_file_id: crate::state::CurseForgeFileId::new(
                        numeric_file_id,
                    )
                    .ok()?,
                    target_file_id: crate::state::CurseForgeFileId::new(
                        target.parse().ok()?,
                    )
                    .ok()?,
                })
            });

            tracing::debug!(
                project_id,
                version_id,
                mr_project = %mr_project.id,
                cf_versions = cf_version_count,
                update = ?update.as_ref().map(|value| match value {
                    ContentItemUpdate::CurseForge { target_file_id, .. } => {
                        target_file_id.get().to_string()
                    }
                    ContentItemUpdate::Modrinth { target_version_id, .. } => {
                        target_version_id.to_string()
                    }
                }),
                "resolved CurseForge modpack update"
            );

            Ok(Some(LinkedModpackInfo {
                project: mr_project,
                version,
                owner,
                update,
                update_version,
            }))
        }

        (None, Some((cf_proj, cf_file))) => {
            let files = crate::api::curseforge::get_files(
                numeric_project_id,
                crate::api::curseforge::CurseForgeFilesRequest {
                    game_version: None,
                    mod_loader_type: None,
                    game_version_type_id: None,
                    index: 0,
                    page_size: 50,
                },
            )
            .await
            .ok()
            .map(|response| response.files)
            .unwrap_or_default();

            let project_model = curseforge_project_to_project(&cf_proj);
            let version = curseforge_file_to_version(&cf_file);
            let all_versions = files
                .into_iter()
                .filter(|file| file.is_available)
                .map(|file| curseforge_file_to_version(&file))
                .collect::<Vec<_>>();
            let (_, update_version_id, update_version) = check_modpack_update(
                &version.id,
                &version,
                Some(all_versions),
                preferred_update_channel,
            );

            Ok(Some(LinkedModpackInfo {
                project: project_model,
                version,
                owner: None,
                update: update_version_id.and_then(|target| {
                    Some(ContentItemUpdate::CurseForge {
                        project_id: crate::state::CurseForgeProjectId::new(
                            numeric_project_id,
                        )
                        .ok()?,
                        current_file_id: crate::state::CurseForgeFileId::new(
                            numeric_file_id,
                        )
                        .ok()?,
                        target_file_id: crate::state::CurseForgeFileId::new(
                            target.parse().ok()?,
                        )
                        .ok()?,
                    })
                }),
                update_version,
            }))
        }

        _ => Ok(None),
    }
}

pub(super) fn curseforge_project_to_project(
    project: &crate::api::curseforge::CurseForgeProject,
) -> Project {
    let mut game_versions = Vec::new();
    let mut loaders = Vec::new();
    let mut seen_versions = HashSet::new();
    let mut seen_loaders = HashSet::new();
    for index in &project.latest_files_indexes {
        if seen_versions.insert(index.game_version.clone()) {
            game_versions.push(index.game_version.clone());
        }
        if let Some(loader) = index.mod_loader.and_then(curseforge_loader_name)
            && seen_loaders.insert(loader)
        {
            loaders.push(loader.to_string());
        }
    }

    Project {
        id: format!("curseforge:{}", project.id),
        slug: Some(project.slug.clone()),
        project_type: "modpack".to_string(),
        team: String::new(),
        organization: None,
        title: project.name.clone(),
        description: project.summary.clone(),
        body: project.summary.clone(),
        published: parse_curseforge_datetime(&project.date_created),
        updated: parse_curseforge_datetime(&project.date_modified),
        approved: None,
        status: "approved".to_string(),
        license: crate::state::License {
            id: String::new(),
            name: String::new(),
            url: None,
        },
        client_side: crate::state::SideType::Unknown,
        server_side: crate::state::SideType::Unknown,
        downloads: project.download_count.min(u32::MAX as u64) as u32,
        followers: 0,
        categories: project
            .categories
            .iter()
            .map(|category| category.slug.clone())
            .collect(),
        additional_categories: Vec::new(),
        game_versions,
        loaders,
        versions: project
            .latest_files
            .iter()
            .map(|file| file.id.to_string())
            .collect(),
        icon_url: project.logo.as_ref().map(|logo| logo.thumbnail_url.clone()),
        issues_url: project.links.issues_url.clone(),
        source_url: project.links.source_url.clone(),
        wiki_url: project.links.wiki_url.clone(),
        discord_url: None,
        donation_urls: None,
        gallery: project
            .screenshots
            .iter()
            .map(|screenshot| crate::state::GalleryItem {
                url: screenshot.url.clone(),
                raw_url: screenshot.url.clone(),
                featured: false,
                title: Some(screenshot.title.clone()),
                description: Some(screenshot.description.clone()),
                created: parse_curseforge_datetime(&project.date_created),
                ordering: 0,
            })
            .collect(),
        color: None,
    }
}

pub(super) fn curseforge_file_to_version(
    file: &crate::api::curseforge::CurseForgeFile,
) -> Version {
    let loaders = file
        .game_versions
        .iter()
        .filter_map(|value| curseforge_loader_from_name(value))
        .collect::<HashSet<_>>()
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let game_versions = file
        .game_versions
        .iter()
        .filter(|value| curseforge_loader_from_name(value).is_none())
        .cloned()
        .collect::<Vec<_>>();
    let hashes = file
        .hashes
        .iter()
        .filter_map(|hash| {
            let algo = match hash.algo {
                1 => "sha1",
                2 => "md5",
                _ => return None,
            };
            Some((algo.to_string(), hash.value.clone()))
        })
        .collect();

    Version {
        id: file.id.to_string(),
        project_id: format!("curseforge:{}", file.mod_id),
        author_id: String::new(),
        featured: false,
        name: file.display_name.clone(),
        version_number: file.display_name.clone(),
        changelog: None,
        changelog_url: None,
        date_published: parse_curseforge_datetime(&file.file_date),
        downloads: file.download_count.min(u32::MAX as u64) as u32,
        version_type: match file.release_type {
            1 => "release".to_string(),
            2 => "beta".to_string(),
            _ => "alpha".to_string(),
        },
        files: vec![crate::state::VersionFile {
            hashes,
            url: file.download_url.clone().unwrap_or_default(),
            filename: file.file_name.clone(),
            primary: true,
            size: file.file_length.min(u32::MAX as u64) as u32,
            file_type: None,
        }],
        dependencies: Vec::new(),
        game_versions,
        loaders: if loaders.is_empty() {
            vec!["minecraft".to_string()]
        } else {
            loaders
        },
    }
}

pub(super) fn parse_curseforge_datetime(
    value: &str,
) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|datetime| datetime.with_timezone(&chrono::Utc))
        .or_else(|_| {
            chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S")
                .map(|datetime| datetime.and_utc())
        })
        .unwrap_or_else(|_| chrono::Utc::now())
}

pub(super) fn curseforge_loader_name(value: u32) -> Option<&'static str> {
    match value {
        1 => Some("forge"),
        4 => Some("fabric"),
        5 => Some("quilt"),
        6 => Some("neoforge"),
        _ => None,
    }
}

pub(super) fn curseforge_loader_from_name(value: &str) -> Option<&'static str> {
    match value.to_ascii_lowercase().replace(' ', "").as_str() {
        "forge" => Some("forge"),
        "fabric" | "fabricloader" => Some("fabric"),
        "quilt" => Some("quilt"),
        "neoforge" => Some("neoforge"),
        _ => None,
    }
}

pub(super) fn is_imported_modpack_scope(link: &InstanceLink) -> bool {
    matches!(link, InstanceLink::ImportedModpack { .. })
}

pub(super) fn is_curseforge_modpack_scope(link: &InstanceLink) -> bool {
    matches!(link, InstanceLink::CurseForgeModpack { .. })
}

pub(super) fn linked_modrinth_modpack_ids(
    link: &InstanceLink,
) -> Option<(&str, &str)> {
    match link {
        InstanceLink::ModrinthModpack {
            project_id,
            version_id,
        } => Some((project_id, version_id)),
        InstanceLink::ServerProjectModpack {
            content_project_id,
            content_version_id,
            ..
        } => Some((content_project_id, content_version_id)),
        _ => None,
    }
}

pub(super) fn linked_curseforge_modpack_ids(
    link: &InstanceLink,
) -> Option<(&str, &str)> {
    match link {
        InstanceLink::CurseForgeModpack {
            project_id,
            version_id,
        } => Some((project_id, version_id)),
        _ => None,
    }
}

pub(super) fn linked_modpack_source_kind(
    link: &InstanceLink,
) -> Option<ContentSourceKind> {
    match link {
        InstanceLink::ModrinthModpack { .. } => {
            Some(ContentSourceKind::ModrinthModpack)
        }
        InstanceLink::CurseForgeModpack { .. } => {
            Some(ContentSourceKind::CurseForge)
        }
        InstanceLink::ServerProjectModpack { .. } => {
            Some(ContentSourceKind::ServerProject)
        }
        _ => None,
    }
}

pub(super) fn check_modpack_update(
    installed_version_id: &str,
    installed_version: &Version,
    all_versions: Option<Vec<Version>>,
    preferred_update_channel: ReleaseChannel,
) -> (bool, Option<String>, Option<Version>) {
    let Some(versions) = all_versions else {
        return (false, None, None);
    };
    let installed_channel =
        ReleaseChannel::from_version_type(&installed_version.version_type);
    let effective_channel =
        preferred_update_channel.least_stable(installed_channel);

    tracing::debug!(
        installed_version_id,
        installed_version_number = %installed_version.version_number,
        installed_date = %installed_version.date_published,
        installed_channel = ?installed_channel,
        effective_channel = ?effective_channel,
        version_count = versions.len(),
        "checking modpack update"
    );

    for version_types in effective_channel.version_type_fallbacks() {
        if !versions.iter().any(|version| {
            version_types.contains(&version.version_type.as_str())
        }) {
            continue;
        }

        let mut newer_versions = versions
            .iter()
            .filter(|version| {
                version.id != installed_version_id
                    && version.date_published > installed_version.date_published
                    && version_types.contains(&version.version_type.as_str())
            })
            .collect::<Vec<_>>();
        newer_versions
            .sort_by_key(|version| std::cmp::Reverse(version.date_published));

        if let Some(newest) = newer_versions.first() {
            tracing::debug!(
                target_version_id = %newest.id,
                target_version_number = %newest.version_number,
                target_date = %newest.date_published,
                "modpack update found"
            );
            return (true, Some(newest.id.clone()), Some((*newest).clone()));
        }

        tracing::debug!(
            version_types = ?version_types,
            "no newer modpack version in channel fallback"
        );
        return (false, None, None);
    }

    (false, None, None)
}

#[derive(Clone, Debug)]
pub(super) struct ModpackIdentifiers {
    pub(super) hashes: HashSet<String>,
    pub(super) project_ids: HashSet<String>,
}

impl ModpackIdentifiers {
    pub(super) fn is_modpack_file(
        &self,
        hash: &str,
        file: Option<&crate::state::ModrinthHashMatch>,
        entry_project_id: Option<&str>,
    ) -> bool {
        self.hashes.contains(hash)
            || entry_project_id
                .is_some_and(|project_id| self.project_ids.contains(project_id))
            || file
                .is_some_and(|file| self.project_ids.contains(&file.project_id))
    }
}

pub(super) fn project_type_from_api_name(project_type: &str) -> ProjectType {
    match project_type {
        "resourcepack" => ProjectType::ResourcePack,
        "shader" => ProjectType::ShaderPack,
        "datapack" => ProjectType::DataPack,
        _ => ProjectType::Mod,
    }
}

pub(super) fn sort_content_items(items: &mut [ContentItem]) {
    items.sort_by(|left, right| {
        let left_name = left
            .project
            .as_ref()
            .map(|project| project.title.as_str())
            .unwrap_or(&left.file_name);
        let right_name = right
            .project
            .as_ref()
            .map(|project| project.title.as_str())
            .unwrap_or(&right.file_name);

        left_name
            .to_lowercase()
            .cmp(&right_name.to_lowercase())
            .then_with(|| left.file_name.cmp(&right.file_name))
    });
}

pub(super) async fn get_cached_modpack_identifiers(
    version_id: &str,
    handle: &Handle,
) -> crate::Result<Option<ModpackIdentifiers>> {
    let Some(cached) =
        CachedEntry::get_modpack_files(version_id, handle).await?
    else {
        return Ok(None);
    };

    if cached.project_ids.is_empty() {
        return Ok(None);
    }

    Ok(Some(ModpackIdentifiers {
        hashes: cached.file_hashes.into_iter().collect(),
        project_ids: cached.project_ids.into_iter().collect(),
    }))
}

pub(super) async fn get_modpack_identifiers(
    version_id: &str,
    content_set: &ContentSet,
    state: &State,
    fetch_semaphore: &FetchSemaphore,
) -> crate::Result<ModpackIdentifiers> {
    let _cache_usage = crate::storage::usage::lease().await?;
    if let Some(cached) =
        CachedEntry::get_modpack_files(version_id, &state.cache_handle())
            .await?
    {
        if !cached.project_ids.is_empty() {
            return Ok(ModpackIdentifiers {
                hashes: cached.file_hashes.into_iter().collect(),
                project_ids: cached.project_ids.into_iter().collect(),
            });
        }

        let hash_refs = cached
            .file_hashes
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        let files = CachedEntry::get_file_many(
            &hash_refs,
            None,
            state.cache_pool(),
            fetch_semaphore,
        )
        .await?;
        let project_ids = files
            .iter()
            .map(|file| file.project_id.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        CachedEntry::cache_modpack_files(
            version_id,
            cached.file_hashes.clone(),
            project_ids.clone(),
        )
        .await?;

        return Ok(ModpackIdentifiers {
            hashes: cached.file_hashes.into_iter().collect(),
            project_ids: project_ids.into_iter().collect(),
        });
    }

    let version = CachedEntry::get_version(
        &ModrinthVersionId::new(version_id.to_string())?,
        None,
        state.cache_pool(),
        fetch_semaphore,
    )
    .await?
    .ok_or_else(|| {
        crate::ErrorKind::InputError(format!(
            "Modpack version {version_id} not found"
        ))
    })?;
    let primary_file = version
        .files
        .iter()
        .find(|file| file.primary)
        .or_else(|| version.files.first())
        .ok_or_else(|| {
            crate::ErrorKind::InputError(format!(
                "No files found for modpack version {version_id}"
            ))
        })?;
    let download_meta = DownloadMeta {
        reason: DownloadReason::Modpack,
        game_version: content_set.game_version.clone(),
        loader: content_set.loader.as_str().to_string(),
        dependent_on: Some(version_id.to_string()),
    };
    let file_name = Path::new(&primary_file.filename);
    if file_name.components().count() != 1
        || !matches!(file_name.components().next(), Some(Component::Normal(_)))
    {
        return Err(crate::ErrorKind::InputError(
            "Modrinth returned an invalid modpack file name".to_string(),
        )
        .into());
    }
    // Stream the pack to the same cache location the installer uses instead
    // of buffering multi-gigabyte .mrpacks in memory; a later install of this
    // version can then reuse the already-verified file.
    let pack_path = state
        .directories
        .caches_dir()
        .join("modpacks")
        .join(&version.project_id)
        .join(version_id)
        .join(file_name);
    download_to_path(
        DownloadRequest::new(&primary_file.url, ResourceClass::Modpack)
            .with_integrity(Integrity {
                size: Some(primary_file.size as u64),
                sha1: primary_file.hashes.get("sha1").cloned(),
                sha512: primary_file.hashes.get("sha512").cloned(),
                content: ContentValidation::Jar,
                ..Integrity::default()
            })
            .with_download_meta(download_meta),
        &pack_path,
        &state.download_semaphore,
        None,
    )
    .await?;
    let zip_reader = ZipFileReader::new(&pack_path).await.map_err(|_| {
        crate::ErrorKind::InputError("Failed to read modpack zip".to_string())
    })?;
    let manifest_idx = zip_reader
        .file()
        .entries()
        .iter()
        .position(|file| {
            matches!(file.filename().as_str(), Ok("modrinth.index.json"))
        })
        .ok_or_else(|| {
            crate::ErrorKind::InputError(
                "No modrinth.index.json found in mrpack".to_string(),
            )
        })?;
    let mut manifest = String::new();
    let mut entry_reader = zip_reader.reader_with_entry(manifest_idx).await?;
    entry_reader.read_to_string_checked(&mut manifest).await?;
    let pack: PackFormat = serde_json::from_str(&manifest)?;
    let mut hashes = pack
        .files
        .iter()
        .filter_map(|file| file.hashes.get(&PackFileHash::Sha1).cloned())
        .collect::<Vec<_>>();
    let project_ids = pack
        .files
        .iter()
        .filter_map(|file| {
            file.downloads.iter().find_map(|url| {
                let parts = url.split('/').collect::<Vec<_>>();
                let data_idx = parts.iter().position(|part| *part == "data")?;
                parts.get(data_idx + 1).map(|part| part.to_string())
            })
        })
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let override_entries = zip_reader
        .file()
        .entries()
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            let filename = entry.filename().as_str().ok()?;
            let is_override = (filename.starts_with("overrides/")
                || filename.starts_with("client-overrides/")
                || filename.starts_with("server-overrides/"))
                && !filename.ends_with('/');
            is_override.then_some(index)
        })
        .collect::<Vec<_>>();

    for index in override_entries {
        let mut file_bytes = Vec::new();
        let mut entry_reader = zip_reader.reader_with_entry(index).await?;
        entry_reader.read_to_end_checked(&mut file_bytes).await?;
        hashes.push(sha1_async(bytes::Bytes::from(file_bytes)).await?);
    }

    CachedEntry::cache_modpack_files(
        version_id,
        hashes.clone(),
        project_ids.clone(),
    )
    .await?;

    Ok(ModpackIdentifiers {
        hashes: hashes.into_iter().collect(),
        project_ids: project_ids.into_iter().collect(),
    })
}

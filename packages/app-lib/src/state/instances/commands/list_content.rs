use super::sync_content_files::{
    fetch_content_file_updates, installed_modrinth_version_id,
    instance_content_root, join_content_path, modrinth_update_enabled,
    project_type_for_file, sync_instance_content_files,
};
use crate::State;
use crate::pack::install_from::{PackFileHash, PackFormat};
use crate::state::instances::adapters::sqlite;
use crate::state::instances::{
    ContentEntry, ContentOwnershipKind, ContentRequirement, ContentSet,
    ContentSourceKind, Instance, InstanceFile, InstanceInstallCandidate,
    InstanceInstallTarget, InstanceLink,
};
use crate::state::{
    CacheBehaviour, CachedEntry, ContentFile, ContentItem, ContentItemOwner,
    ContentItemProject, ContentItemRollback, ContentItemUpdate,
    ContentItemVersion, ContentProvider, ContentProviderRef, Dependency,
    LinkedModpackInfo, ModLoader, ModrinthFileMatch, ModrinthProjectId,
    ModrinthVersionId, Organization, OwnerType, Project, ProjectType,
    ReleaseChannel, TeamMember, Version, VersionV3,
};
use crate::util::fetch::{
    ContentValidation, DownloadMeta, DownloadReason, DownloadRequest,
    FetchSemaphore, Integrity, ResourceClass, download_to_path, sha1_async,
};
use async_zip::tokio::read::fs::ZipFileReader;
use dashmap::DashMap;
use sqlx::{Row, SqlitePool};
use std::collections::{HashMap, HashSet};
use std::path::{Component, Path};

#[derive(Clone, Debug)]
struct ResolvedContentScope {
    instance: Instance,
    content_set: ContentSet,
}

struct EntryMaps {
    entries_by_file_id: HashMap<String, ContentEntry>,
    provider_refs_by_file_id: HashMap<String, Vec<ContentProviderRef>>,
    origin_provider_by_file_id: HashMap<String, ContentProvider>,
}

async fn load_entry_maps(
    content_set_id: &str,
    pool: &SqlitePool,
) -> crate::Result<EntryMaps> {
    let entries =
        sqlite::content_rows::get_content_entries(content_set_id, pool).await?;
    let entries_by_file_id = entries
        .iter()
        .filter_map(|entry| {
            entry
                .file_id
                .as_ref()
                .map(|file_id| (file_id.clone(), entry.clone()))
        })
        .collect::<HashMap<_, _>>();
    let mut provider_refs_by_file_id = HashMap::new();
    let mut origin_provider_by_file_id = HashMap::new();
    let rows = sqlx::query(
        "SELECT entry.file_id, ref.provider, ref.provider_project_id,
            ref.provider_release_id, ref.provider_file_id, ref.is_origin
         FROM instance_content_entries entry
         JOIN instance_content_provider_refs ref ON ref.content_entry_id = entry.id
         WHERE entry.content_set_id = ? AND entry.file_id IS NOT NULL
         ORDER BY entry.id, ref.is_origin DESC, ref.provider, ref.provider_project_id,
            ref.provider_release_id IS NULL, ref.provider_release_id",
    )
    .bind(content_set_id)
    .fetch_all(pool)
    .await?;
    for row in rows {
        let Some(file_id) = row.try_get::<Option<String>, _>("file_id")? else {
            continue;
        };
        let provider = row.try_get::<String, _>("provider")?;
        let project_id = row.try_get::<String, _>("provider_project_id")?;
        let release_id =
            row.try_get::<Option<String>, _>("provider_release_id")?;
        let file_id_ref =
            row.try_get::<Option<String>, _>("provider_file_id")?;
        let reference = ContentProviderRef::from_database(
            &provider,
            &project_id,
            release_id.as_deref(),
            file_id_ref.as_deref(),
        )?;
        if row.try_get::<i64, _>("is_origin")? != 0 {
            origin_provider_by_file_id
                .insert(file_id.clone(), reference.provider());
        }
        provider_refs_by_file_id
            .entry(file_id)
            .or_insert_with(Vec::new)
            .push(reference);
    }

    Ok(EntryMaps {
        entries_by_file_id,
        provider_refs_by_file_id,
        origin_provider_by_file_id,
    })
}

#[derive(Clone, Copy, Debug)]
enum ContentFilter<'a> {
    All,
    ExcludeModpack(&'a ModpackIdentifiers),
    ExcludeSourceKind {
        source_kind: ContentSourceKind,
        exclude_untracked: bool,
    },
    OnlyModpack(&'a ModpackIdentifiers),
    OnlySourceKind {
        source_kind: ContentSourceKind,
        include_untracked: bool,
    },
}

#[derive(Clone, Copy, Debug)]
enum ContentUpdateLookup {
    Skip,
    Include { refresh: bool },
}

pub(crate) async fn list_content_sets(
    instance_id: &str,
    pool: &SqlitePool,
) -> crate::Result<Vec<ContentSet>> {
    let instance = sqlite::instance_rows::get_instance_by_id(instance_id, pool)
        .await?
        .ok_or_else(|| {
            crate::ErrorKind::InputError("Unknown instance".to_string())
        })?;

    sqlite::content_rows::get_content_sets_for_instance(&instance.id, pool)
        .await
}

pub(crate) async fn get_content_projects(
    instance_id: &str,
    content_set_id: Option<&str>,
    cache_behaviour: Option<CacheBehaviour>,
    state: &State,
) -> crate::Result<DashMap<String, ContentFile>> {
    let resolved = resolve_content_scope_with_instance(
        instance_id,
        content_set_id,
        &state.pool,
    )
    .await?;

    content_projects_for_scope(
        &resolved,
        cache_behaviour,
        state,
        ContentFilter::All,
        false,
    )
    .await
}

pub(crate) async fn get_installed_project_ids_for_instance(
    instance_id: &str,
    content_set_id: Option<&str>,
    state: &State,
) -> crate::Result<Vec<String>> {
    let resolved = resolve_content_scope_with_instance(
        instance_id,
        content_set_id,
        &state.pool,
    )
    .await?;
    let mut project_ids = HashSet::new();
    let provider_rows = sqlx::query(
        "SELECT DISTINCT ref.provider, ref.provider_project_id
         FROM instance_content_entries entry
         INNER JOIN instance_files file ON file.id = entry.file_id
         INNER JOIN instance_content_provider_refs ref
            ON ref.content_entry_id = entry.id
         WHERE entry.content_set_id = ? AND file.missing = 0",
    )
    .bind(&resolved.content_set.id)
    .fetch_all(&state.pool)
    .await?;
    for row in provider_rows {
        let provider = row.try_get::<String, _>("provider")?;
        let project_id = row.try_get::<String, _>("provider_project_id")?;
        if provider == "curseforge" {
            project_ids.insert(format!("curseforge:{project_id}"));
        } else {
            project_ids.insert(project_id);
        }
    }

    Ok(project_ids.into_iter().collect())
}

#[derive(sqlx::FromRow)]
struct InstanceInstallCandidateRow {
    id: String,
    name: String,
    icon_path: Option<String>,
    game_version: String,
    loader: String,
    installed: i64,
}

pub(crate) async fn get_instance_install_candidates(
    project_id: &str,
    project_type: ProjectType,
    targets: &[InstanceInstallTarget],
    pool: &SqlitePool,
) -> crate::Result<Vec<InstanceInstallCandidate>> {
    let rows = sqlx::query_as::<_, InstanceInstallCandidateRow>(
        r#"
		SELECT
			i.id,
			i.name,
			i.icon_path,
			cs.game_version,
			cs.loader,
			CASE
				WHEN EXISTS (
					SELECT 1
					FROM instance_content_entries entry
					INNER JOIN instance_files file
						ON file.id = entry.file_id
					WHERE entry.content_set_id = cs.id
						AND EXISTS (
							SELECT 1
							FROM instance_content_provider_refs ref
							WHERE ref.content_entry_id = entry.id
								AND ref.provider = CASE
									WHEN ? LIKE 'curseforge:%' THEN 'curseforge'
									ELSE 'modrinth'
								END
								AND ref.provider_project_id = CASE
									WHEN ? LIKE 'curseforge:%' THEN substr(?, 12)
									ELSE ?
									END
						)
						AND file.missing = 0
				)
					THEN 1
				ELSE 0
			END AS installed
		FROM instances i
		INNER JOIN instance_content_sets cs
			ON cs.id = i.applied_content_set_id
		LEFT JOIN instance_links link
			ON link.instance_id = i.id
		WHERE COALESCE(link.link_kind, 'unmanaged') NOT IN (
			'server_project',
			'server_project_modpack'
		)
		ORDER BY i.name ASC
		"#,
    )
    .bind(project_id)
    .bind(project_id)
    .bind(project_id)
    .bind(project_id)
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|row| -> crate::Result<_> {
            let loader = ModLoader::try_from_string(&row.loader)?;
            let compatible = instance_matches_targets(
                project_type,
                &row.game_version,
                loader.as_str(),
                targets,
            );

            Ok(InstanceInstallCandidate {
                id: row.id,
                name: row.name,
                icon_path: row.icon_path,
                game_version: row.game_version,
                loader,
                installed: row.installed != 0,
                compatible,
            })
        })
        .collect()
}

fn instance_matches_targets(
    project_type: ProjectType,
    game_version: &str,
    loader: &str,
    targets: &[InstanceInstallTarget],
) -> bool {
    if project_type == ProjectType::ResourcePack {
        return !targets.is_empty();
    }

    targets.iter().any(|target| {
        target.game_version == game_version
            && (project_type != ProjectType::Mod
                || target.loader == loader
                || target.loader == "datapack")
    })
}

pub(crate) async fn list_content(
    instance_id: &str,
    content_set_id: Option<&str>,
    cache_behaviour: Option<CacheBehaviour>,
    state: &State,
) -> crate::Result<Vec<ContentItem>> {
    let resolved = resolve_content_scope_with_instance(
        instance_id,
        content_set_id,
        &state.pool,
    )
    .await?;
    let link = sqlite::instance_rows::get_instance_link(
        &resolved.instance.id,
        &state.pool,
    )
    .await?;
    let imported_modpack_scope = is_imported_modpack_scope(&link);
    let curseforge_modpack_scope = is_curseforge_modpack_scope(&link);
    let linked_modpack_source_kind = linked_modpack_source_kind(&link);
    let modpack_ids = if imported_modpack_scope || curseforge_modpack_scope {
        None
    } else {
        match linked_modrinth_modpack_ids(&link) {
            Some((_, version_id)) => {
                get_cached_modpack_identifiers(
                    version_id,
                    &state.pool,
                    &state.api_semaphore,
                )
                .await?
            }
            None => None,
        }
    };
    let filter = if imported_modpack_scope {
        ContentFilter::ExcludeSourceKind {
            source_kind: ContentSourceKind::ImportedModpack,
            exclude_untracked: resolved.instance.install_stage
                != crate::state::InstanceInstallStage::Installed,
        }
    } else if curseforge_modpack_scope {
        ContentFilter::ExcludeSourceKind {
            source_kind: ContentSourceKind::CurseForge,
            exclude_untracked: resolved.instance.install_stage
                != crate::state::InstanceInstallStage::Installed,
        }
    } else if let Some(ids) = modpack_ids.as_ref() {
        ContentFilter::ExcludeModpack(ids)
    } else if let Some(source_kind) = linked_modpack_source_kind {
        ContentFilter::ExcludeSourceKind {
            source_kind,
            exclude_untracked: true,
        }
    } else {
        ContentFilter::All
    };
    let files = content_projects_for_scope(
        &resolved,
        cache_behaviour,
        state,
        filter,
        false,
    )
    .await?;
    let files = files.into_iter().collect::<Vec<_>>();

    content_files_to_content_items(
        &resolved.instance,
        &files,
        cache_behaviour,
        state,
    )
    .await
}

pub(crate) async fn list_all_content(
    instance_id: &str,
    cache_behaviour: Option<CacheBehaviour>,
    refresh_file_updates: bool,
    state: &State,
) -> crate::Result<Vec<ContentItem>> {
    let resolved =
        resolve_content_scope_with_instance(instance_id, None, &state.pool)
            .await?;
    let files = content_projects_for_scope(
        &resolved,
        cache_behaviour,
        state,
        ContentFilter::All,
        refresh_file_updates,
    )
    .await?
    .into_iter()
    .collect::<Vec<_>>();

    content_files_to_content_items(
        &resolved.instance,
        &files,
        cache_behaviour,
        state,
    )
    .await
}

pub(crate) async fn list_all_content_from_files(
    instance_id: &str,
    files: Vec<InstanceFile>,
    cache_behaviour: Option<CacheBehaviour>,
    refresh_file_updates: bool,
    state: &State,
) -> crate::Result<Vec<ContentItem>> {
    let resolved =
        resolve_content_scope_with_instance(instance_id, None, &state.pool)
            .await?;
    let files = content_projects_for_files(
        &resolved,
        files,
        cache_behaviour,
        state,
        ContentFilter::All,
        ContentUpdateLookup::Include {
            refresh: refresh_file_updates,
        },
    )
    .await?
    .into_iter()
    .collect::<Vec<_>>();

    content_files_to_content_items(
        &resolved.instance,
        &files,
        cache_behaviour,
        state,
    )
    .await
}

pub(crate) async fn list_content_by_paths(
    instance_id: &str,
    paths: &[String],
    cache_behaviour: Option<CacheBehaviour>,
    state: &State,
) -> crate::Result<Vec<ContentItem>> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }

    let resolved =
        resolve_content_scope_with_instance(instance_id, None, &state.pool)
            .await?;

    let path_set = paths.iter().map(String::as_str).collect::<HashSet<_>>();

    let files = sqlite::content_rows::get_instance_files(
        &resolved.instance.id,
        &state.pool,
    )
    .await?
    .into_iter()
    .filter(|file| path_set.contains(file.relative_path.as_str()))
    .collect::<Vec<_>>();

    let files = content_projects_for_files(
        &resolved,
        files,
        cache_behaviour,
        state,
        ContentFilter::All,
        ContentUpdateLookup::Skip,
    )
    .await?
    .into_iter()
    .collect::<Vec<_>>();

    content_files_to_content_items(
        &resolved.instance,
        &files,
        cache_behaviour,
        state,
    )
    .await
}

pub(crate) async fn list_linked_modpack_content(
    instance_id: &str,
    content_set_id: Option<&str>,
    cache_behaviour: Option<CacheBehaviour>,
    state: &State,
) -> crate::Result<Vec<ContentItem>> {
    let resolved = resolve_content_scope_with_instance(
        instance_id,
        content_set_id,
        &state.pool,
    )
    .await?;
    let link = sqlite::instance_rows::get_instance_link(
        &resolved.instance.id,
        &state.pool,
    )
    .await?;
    if is_imported_modpack_scope(&link) {
        let files = content_projects_for_scope(
            &resolved,
            cache_behaviour,
            state,
            ContentFilter::OnlySourceKind {
                source_kind: ContentSourceKind::ImportedModpack,
                include_untracked: false,
            },
            false,
        )
        .await?;
        let files = files.into_iter().collect::<Vec<_>>();

        return content_files_to_content_items(
            &resolved.instance,
            &files,
            cache_behaviour,
            state,
        )
        .await;
    }

    if is_curseforge_modpack_scope(&link) {
        let files = content_projects_for_scope(
            &resolved,
            cache_behaviour,
            state,
            ContentFilter::OnlySourceKind {
                source_kind: ContentSourceKind::CurseForge,
                include_untracked: false,
            },
            false,
        )
        .await?;
        let files = files.into_iter().collect::<Vec<_>>();

        return content_files_to_content_items(
            &resolved.instance,
            &files,
            cache_behaviour,
            state,
        )
        .await;
    }

    let Some((_, version_id)) = linked_modrinth_modpack_ids(&link) else {
        return Ok(Vec::new());
    };
    let ids = match get_modpack_identifiers(
        version_id,
        &resolved.content_set,
        &state.pool,
        &state.api_semaphore,
    )
    .await
    {
        Ok(ids) => ids,
        Err(err) => {
            tracing::warn!("Failed to fetch modpack identifiers: {}", err);
            return Ok(Vec::new());
        }
    };
    let files = content_projects_for_scope(
        &resolved,
        cache_behaviour,
        state,
        ContentFilter::OnlyModpack(&ids),
        false,
    )
    .await?;
    let files = files.into_iter().collect::<Vec<_>>();

    content_files_to_content_items(
        &resolved.instance,
        &files,
        cache_behaviour,
        state,
    )
    .await
}

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
    let link = sqlite::instance_rows::get_instance_link(
        &resolved.instance.id,
        &state.pool,
    )
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
            &state.pool,
            &state.api_semaphore,
        ),
        CachedEntry::get_version(
            &version_id_ref,
            cache_behaviour,
            &state.pool,
            &state.api_semaphore,
        ),
        CachedEntry::get_project_versions(
            &project_id_ref,
            cache_behaviour,
            &state.pool,
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
                &state.pool,
                &state.api_semaphore,
            ),
            CachedEntry::get_project_versions(
                &modpack_project_id_ref,
                cache_behaviour,
                &state.pool,
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
            &state.pool,
            &state.api_semaphore,
        )
        .await?;
        org.map(|org| ContentItemOwner {
            id: org.id,
            name: org.name,
            avatar_url: org.icon_url,
            owner_type: OwnerType::Organization,
        })
    } else {
        let team = CachedEntry::get_team(
            &project.team,
            cache_behaviour,
            &state.pool,
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

async fn get_curseforge_linked_modpack_info(
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
            } else {
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

fn curseforge_project_to_project(
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

fn curseforge_file_to_version(
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

fn parse_curseforge_datetime(value: &str) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|datetime| datetime.with_timezone(&chrono::Utc))
        .or_else(|_| {
            chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S")
                .map(|datetime| datetime.and_utc())
        })
        .unwrap_or_else(|_| chrono::Utc::now())
}

fn curseforge_loader_name(value: u32) -> Option<&'static str> {
    match value {
        1 => Some("forge"),
        4 => Some("fabric"),
        5 => Some("quilt"),
        6 => Some("neoforge"),
        _ => None,
    }
}

fn curseforge_loader_from_name(value: &str) -> Option<&'static str> {
    match value.to_ascii_lowercase().replace(' ', "").as_str() {
        "forge" => Some("forge"),
        "fabric" | "fabricloader" => Some("fabric"),
        "quilt" => Some("quilt"),
        "neoforge" => Some("neoforge"),
        _ => None,
    }
}

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

async fn resolve_content_scope_with_instance(
    instance_id: &str,
    content_set_id: Option<&str>,
    pool: &SqlitePool,
) -> crate::Result<ResolvedContentScope> {
    let instance = sqlite::instance_rows::get_instance_by_id(instance_id, pool)
        .await?
        .ok_or_else(|| {
            crate::ErrorKind::InputError("Unknown instance".to_string())
        })?;
    let content_set = match content_set_id {
        Some(content_set_id) => {
            let content_set =
                sqlite::content_rows::get_content_set(content_set_id, pool)
                    .await?
                    .ok_or_else(|| {
                        crate::ErrorKind::InputError(format!(
                            "Unknown content set {content_set_id}"
                        ))
                    })?;

            if content_set.instance_id != instance.id {
                return Err(crate::ErrorKind::InputError(format!(
					"Content set {content_set_id} does not belong to instance {}",
					instance.id
				))
                .into());
            }

            content_set
        }
        None => {
            sqlite::content_rows::get_applied_content_set(&instance.id, pool)
                .await?
                .ok_or_else(|| {
                    crate::ErrorKind::InputError(format!(
                        "Instance {} has no applied content set",
                        instance.id
                    ))
                })?
        }
    };

    Ok(ResolvedContentScope {
        instance,
        content_set,
    })
}

async fn content_projects_for_scope(
    resolved: &ResolvedContentScope,
    cache_behaviour: Option<CacheBehaviour>,
    state: &State,
    filter: ContentFilter<'_>,
    refresh_file_updates: bool,
) -> crate::Result<DashMap<String, ContentFile>> {
    let files = if cache_behaviour == Some(CacheBehaviour::CacheOnly) {
        sqlite::content_rows::get_instance_files(
            &resolved.instance.id,
            &state.pool,
        )
        .await?
    } else {
        sync_instance_content_files(&resolved.instance, state).await?
    };

    content_projects_for_files(
        resolved,
        files,
        cache_behaviour,
        state,
        filter,
        ContentUpdateLookup::Include {
            refresh: refresh_file_updates,
        },
    )
    .await
}

async fn content_projects_for_files(
    resolved: &ResolvedContentScope,
    files: Vec<InstanceFile>,
    cache_behaviour: Option<CacheBehaviour>,
    state: &State,
    filter: ContentFilter<'_>,
    update_lookup: ContentUpdateLookup,
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
        &state.pool,
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
    // Visible-row metadata must not depend on update catalogs: one invalid
    // candidate release would otherwise discard the whole display batch.
    let file_updates = if let ContentUpdateLookup::Include { refresh } =
        update_lookup
    {
        let installed_version_ids_by_hash = files
            .iter()
            .filter_map(|file| {
                let provider_refs = entry_maps
                    .provider_refs_by_file_id
                    .get(&file.id)
                    .map(Vec::as_slice)
                    .unwrap_or_default();
                installed_modrinth_version_id(provider_refs)
                    .or_else(|| {
                        file_info_by_hash.get(&file.sha1).and_then(|metadata| {
                            ModrinthVersionId::new(metadata.version_id.clone())
                                .ok()
                        })
                    })
                    .map(|version_id| (file.sha1.clone(), version_id))
            })
            .collect::<HashMap<_, _>>();
        let installed_channels = get_installed_update_channels(
            &installed_version_ids_by_hash,
            cache_behaviour,
            &state.pool,
            &state.api_semaphore,
        )
        .await?;
        let update_keys = files
            .iter()
            .filter(|file| file_info_by_hash.contains_key(&file.sha1))
            .filter_map(|file| {
                let project_type = project_type_for_file(file)?;
                let channel = resolved.instance.update_channel.least_stable(
                    installed_channels
                        .get(&file.sha1)
                        .copied()
                        .unwrap_or(resolved.instance.update_channel),
                );
                Some(file_update_cache_key(
                    &file.sha1,
                    project_type,
                    &resolved.content_set,
                    channel,
                ))
            })
            .collect::<Vec<_>>();
        let update_key_refs =
            update_keys.iter().map(String::as_str).collect::<Vec<_>>();
        fetch_content_file_updates(
            &update_key_refs,
            cache_behaviour,
            refresh,
            &state.pool,
            &state.api_semaphore,
        )
        .await?
    } else {
        Vec::new()
    };
    let mut updates_by_hash: HashMap<String, Vec<String>> = HashMap::new();
    for update in file_updates {
        updates_by_hash
            .entry(update.hash)
            .or_default()
            .push(update.update_version_id);
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

        let update = if modrinth_update_enabled(origin_provider, &provider_refs)
        {
            modrinth_metadata.as_ref().and_then(|metadata| {
                let current_version_id =
                    installed_modrinth_version_id(&provider_refs)
                        .unwrap_or_else(|| metadata.version_id.clone());
                let update_ids =
                    updates_by_hash.remove(&file.sha1).unwrap_or_default();
                update_ids
                    .into_iter()
                    .find(|update_id| update_id != current_version_id.as_str())
                    .and_then(|target| {
                        Some(ContentItemUpdate::Modrinth {
                            project_id: metadata.project_id.clone(),
                            current_version_id: current_version_id.clone(),
                            target_version_id: ModrinthVersionId::new(target)
                                .ok()?,
                        })
                    })
            })
        } else {
            None
        };

        output.insert(
            file.relative_path.clone(),
            ContentFile {
                update,
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

async fn reconcile_hash_matched_entries(
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
    sqlite::content_rows::ensure_content_write_parents(
        &resolved.instance.id,
        &resolved.content_set.id,
        &mut tx,
    )
    .await?;
    for (file, project_type, existing_entry, provider_ref) in &candidates {
        let entry_id = if let Some(entry) = existing_entry {
            entry.id.clone()
        } else {
            sqlite::content_rows::upsert_content_entry_from_parts_in_transaction(
                sqlite::content_rows::UpsertContentEntry {
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
        sqlite::content_rows::upsert_content_provider_ref_in_transaction(
            &entry_id,
            provider_ref,
            false,
            &mut tx,
        )
        .await?;
    }
    sqlite::content_rows::bump_content_set_revision_in_transaction(
        &resolved.content_set.id,
        &mut tx,
    )
    .await?;
    tx.commit().await?;

    Ok(true)
}

async fn get_installed_update_channels(
    installed_version_ids_by_hash: &HashMap<String, ModrinthVersionId>,
    cache_behaviour: Option<CacheBehaviour>,
    pool: &SqlitePool,
    fetch_semaphore: &FetchSemaphore,
) -> crate::Result<HashMap<String, ReleaseChannel>> {
    let version_ids = installed_version_ids_by_hash
        .values()
        .map(ModrinthVersionId::as_str)
        .collect::<HashSet<_>>();
    if version_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let version_id_refs = version_ids
        .iter()
        .filter_map(|id| ModrinthVersionId::new((*id).to_string()).ok())
        .collect::<Vec<_>>();
    let versions = CachedEntry::get_version_many(
        &version_id_refs,
        cache_behaviour,
        pool,
        fetch_semaphore,
    )
    .await?;
    let channels_by_version_id = versions
        .into_iter()
        .map(|version| {
            (
                version.id,
                ReleaseChannel::from_version_type(&version.version_type),
            )
        })
        .collect::<HashMap<_, _>>();

    Ok(installed_version_ids_by_hash
        .iter()
        .filter_map(|(hash, version_id)| {
            channels_by_version_id
                .get(version_id.as_str())
                .copied()
                .map(|channel| (hash.clone(), channel))
        })
        .collect())
}

fn file_update_cache_key(
    hash: &str,
    project_type: ProjectType,
    content_set: &ContentSet,
    channel: ReleaseChannel,
) -> String {
    let loader_key = if project_type == ProjectType::Mod {
        content_set.loader.as_str().to_string()
    } else {
        project_type.get_loaders().join("+")
    };

    format!(
        "{}-{}-{}-{}",
        hash,
        loader_key,
        channel.key(),
        content_set.game_version
    )
}

async fn content_files_to_content_items(
    instance: &Instance,
    files: &[(String, ContentFile)],
    cache_behaviour: Option<CacheBehaviour>,
    state: &State,
) -> crate::Result<Vec<ContentItem>> {
    let mut provider_refs_by_path =
        HashMap::<String, Vec<ContentProviderRef>>::new();
    let mut origin_provider_by_path = HashMap::<String, ContentProvider>::new();
    let mut source_kind_by_path = HashMap::<String, ContentSourceKind>::new();
    let provider_rows = sqlx::query(
        "SELECT file.relative_path, ref.provider, ref.provider_project_id,
                ref.provider_release_id, ref.provider_file_id, ref.is_origin, entry.source_kind
         FROM instance_files file
         INNER JOIN instance_content_entries entry ON entry.file_id = file.id
         INNER JOIN instance_content_provider_refs ref
            ON ref.content_entry_id = entry.id
         WHERE file.instance_id = ?
         ORDER BY file.relative_path, ref.is_origin DESC, ref.provider,
            ref.provider_project_id, ref.provider_release_id IS NULL,
            ref.provider_release_id",
    )
    .bind(&instance.id)
    .fetch_all(&state.pool)
    .await?;
    for row in provider_rows {
        let provider = ContentProvider::from_str(row.try_get("provider")?)?;
        provider_refs_by_path
            .entry(row.try_get("relative_path")?)
            .or_default()
            .push(ContentProviderRef::from_database(
                provider.as_str(),
                row.try_get("provider_project_id")?,
                row.try_get::<Option<String>, _>("provider_release_id")?
                    .as_deref(),
                row.try_get::<Option<String>, _>("provider_file_id")?
                    .as_deref(),
            )?);
        if row.try_get::<i64, _>("is_origin")? != 0 {
            origin_provider_by_path
                .insert(row.try_get("relative_path")?, provider);
        }
        if let Some(source_kind) =
            row.try_get::<Option<String>, _>("source_kind")?
            && let Ok(source_kind) = ContentSourceKind::from_str(&source_kind)
        {
            source_kind_by_path
                .insert(row.try_get("relative_path")?, source_kind);
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
    let content_set = sqlite::content_rows::get_applied_content_set(
        &instance.id,
        &state.pool,
    )
    .await?;
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
        &state.pool,
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
            let curseforge_update_id = curseforge_ref.and_then(|reference| {
                if origin_provider != Some(ContentProvider::CurseForge) {
                    return None;
                }
                let ContentProviderRef::CurseForge { file_id, .. } = reference
                else {
                    return None;
                };
                let current_file_id = file_id.as_ref()?.get();
                let content_set = content_set.as_ref()?;
                let loader_type = match content_set.loader.as_str() {
                    "forge" => Some(1),
                    "fabric" => Some(4),
                    "quilt" => Some(5),
                    "neoforge" => Some(6),
                    _ => None,
                };
                let release_type_allowed =
                    |release_type: u32| match instance.update_channel {
                        ReleaseChannel::Release => release_type == 1,
                        ReleaseChannel::Beta => release_type <= 2,
                        ReleaseChannel::Alpha => true,
                    };

                if let Some(index) = curseforge_project?
                    .latest_files_indexes
                    .iter()
                    .find(|index| {
                        index.game_version == content_set.game_version
                            && (file.project_type != ProjectType::Mod
                                || index.mod_loader == loader_type)
                            && release_type_allowed(index.release_type)
                    })
                {
                    return (index.file_id != current_file_id)
                        .then(|| index.file_id.to_string());
                }

                let fallback = curseforge_project?.latest_files.iter().find(
                    |candidate| {
                        if !candidate.is_available
                            || !release_type_allowed(candidate.release_type)
                        {
                            return false;
                        }
                        let has_game_version = candidate
                            .game_versions
                            .iter()
                            .any(|value| value == &content_set.game_version);
                        if !has_game_version {
                            return false;
                        }
                        if file.project_type != ProjectType::Mod {
                            return true;
                        }
                        candidate.game_versions.iter().any(|value| {
                            match curseforge_loader_from_name(value) {
                                Some("forge") => loader_type == Some(1),
                                Some("fabric") => loader_type == Some(4),
                                Some("quilt") => loader_type == Some(5),
                                Some("neoforge") => loader_type == Some(6),
                                _ => false,
                            }
                        })
                    },
                )?;
                (fallback.id != current_file_id)
                    .then(|| fallback.id.to_string())
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
                            id: format!("local:{}", meta.mod_id),
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
                                id: format!("local:file:{}", file.hash),
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
                                id: format!("local:{}", meta.mod_id),
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
                                    id: format!("local:{author}"),
                                    name: author.clone(),
                                    avatar_url: None,
                                    owner_type: OwnerType::User,
                                }
                            })
                        })
                    }),
                update: file.update.clone().or_else(|| {
                    curseforge_update_id.and_then(|target| {
                        let reference =
                            provider_refs.iter().find_map(|reference| {
                                match reference {
                                    ContentProviderRef::CurseForge {
                                        project_id,
                                        file_id: Some(current_file_id),
                                    } => Some((*project_id, *current_file_id)),
                                    _ => None,
                                }
                            })?;
                        Some(ContentItemUpdate::CurseForge {
                            project_id: reference.0,
                            current_file_id: reference.1,
                            target_file_id:
                                crate::state::CurseForgeFileId::new(
                                    target.parse().ok()?,
                                )
                                .ok()?,
                        })
                    })
                }),
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

fn rollback_for_content_file(
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

struct ResolvedMetadata {
    projects: Vec<Project>,
    versions: Vec<Version>,
    versions_v3: Vec<VersionV3>,
    teams: Vec<Vec<TeamMember>>,
    organizations: Vec<Organization>,
}

async fn resolve_metadata(
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

fn resolve_owner(
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

fn modrinth_file_match(
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

fn is_imported_modpack_scope(link: &InstanceLink) -> bool {
    matches!(link, InstanceLink::ImportedModpack { .. })
}

fn is_curseforge_modpack_scope(link: &InstanceLink) -> bool {
    matches!(link, InstanceLink::CurseForgeModpack { .. })
}

fn linked_modrinth_modpack_ids(link: &InstanceLink) -> Option<(&str, &str)> {
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

fn linked_curseforge_modpack_ids(link: &InstanceLink) -> Option<(&str, &str)> {
    match link {
        InstanceLink::CurseForgeModpack {
            project_id,
            version_id,
        } => Some((project_id, version_id)),
        _ => None,
    }
}

fn linked_modpack_source_kind(
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

fn check_modpack_update(
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
struct ModpackIdentifiers {
    hashes: HashSet<String>,
    project_ids: HashSet<String>,
}

impl ModpackIdentifiers {
    fn is_modpack_file(
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

async fn get_cached_modpack_identifiers(
    version_id: &str,
    pool: &SqlitePool,
    fetch_semaphore: &FetchSemaphore,
) -> crate::Result<Option<ModpackIdentifiers>> {
    let Some(cached) =
        CachedEntry::get_modpack_files(version_id, pool, fetch_semaphore)
            .await?
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

async fn get_modpack_identifiers(
    version_id: &str,
    content_set: &ContentSet,
    pool: &SqlitePool,
    fetch_semaphore: &FetchSemaphore,
) -> crate::Result<ModpackIdentifiers> {
    let _cache_usage = crate::storage::usage::lease().await?;
    if let Some(cached) =
        CachedEntry::get_modpack_files(version_id, pool, fetch_semaphore)
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
        let files =
            CachedEntry::get_file_many(&hash_refs, None, pool, fetch_semaphore)
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
            pool,
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
        pool,
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
    let state = State::get().await?;
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
        pool,
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
        pool,
    )
    .await?;

    Ok(ModpackIdentifiers {
        hashes: hashes.into_iter().collect(),
        project_ids: project_ids.into_iter().collect(),
    })
}

fn project_type_from_api_name(project_type: &str) -> ProjectType {
    match project_type {
        "resourcepack" => ProjectType::ResourcePack,
        "shader" => ProjectType::ShaderPack,
        "datapack" => ProjectType::DataPack,
        _ => ProjectType::Mod,
    }
}

fn sort_content_items(items: &mut [ContentItem]) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::instances::{CreateInstance, InstanceLink};
    use chrono::Utc;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    async fn reconciliation_fixture() -> (
        tempfile::TempDir,
        std::sync::Arc<State>,
        ResolvedContentScope,
    ) {
        let temp = tempfile::tempdir().unwrap();
        let directories = crate::state::DirectoryInfo {
            settings_dir: temp.path().to_path_buf(),
            config_dir: temp.path().to_path_buf(),
            app_identifier: "provider-reconciliation-test".to_string(),
        };
        std::fs::create_dir_all(directories.instances_dir()).unwrap();
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(temp.path().join("state.db"))
                    .create_if_missing(true)
                    .foreign_keys(true),
            )
            .await
            .unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();
        let state = crate::state::test_state(directories, pool).await.unwrap();
        let instance = crate::state::instances::create_instance(
            CreateInstance {
                name: "Provider reconciliation".to_string(),
                path: Some("provider-reconciliation".to_string()),
                game_version: "1.21.4".to_string(),
                loader: ModLoader::Vanilla,
                loader_version: None,
                icon_path: None,
                link: InstanceLink::Unmanaged,
                symlink_target: None,
                game_dir_override: None,
            },
            &state,
        )
        .await
        .unwrap();
        let content_set = sqlite::content_rows::get_applied_content_set(
            &instance.id,
            &state.pool,
        )
        .await
        .unwrap()
        .unwrap();
        let resolved = ResolvedContentScope {
            instance,
            content_set,
        };
        (temp, state, resolved)
    }

    async fn insert_test_file(
        resolved: &ResolvedContentScope,
        state: &State,
        id: &str,
        path: &str,
        sha1: &str,
        enabled: bool,
    ) -> InstanceFile {
        let now = Utc::now();
        let file = InstanceFile {
            id: id.to_string(),
            instance_id: resolved.instance.id.clone(),
            relative_path: path.to_string(),
            file_name: path.rsplit('/').next().unwrap().to_string(),
            enabled,
            sha1: sha1.to_string(),
            size: 1,
            missing: false,
            added_at: now,
            modified_at: now,
            local_mod_data: None,
            icon_path: None,
        };
        let mut tx = state.pool.begin().await.unwrap();
        sqlite::content_rows::upsert_instance_file(&file, &mut tx)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        file
    }

    async fn insert_test_entry(
        resolved: &ResolvedContentScope,
        state: &State,
        file: &InstanceFile,
        enabled: bool,
    ) -> ContentEntry {
        let mut tx = state.pool.begin().await.unwrap();
        let entry = sqlite::content_rows::upsert_content_entry_from_parts_in_transaction(
            sqlite::content_rows::UpsertContentEntry {
                instance_id: &resolved.instance.id,
                content_set_id: &resolved.content_set.id,
                file_id: Some(&file.id),
                project_type: ProjectType::Mod,
                source_kind: ContentSourceKind::Local,
                ownership_kind: ContentOwnershipKind::UserAdded,
                auto_dependency: false,
                server_requirement: ContentRequirement::Optional,
                client_requirement: ContentRequirement::Required,
                enabled,
            },
            &mut tx,
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
        entry
    }

    #[test]
    fn resource_pack_install_candidates_ignore_game_version() {
        let targets = vec![InstanceInstallTarget {
            game_version: "1.21.11".to_string(),
            loader: "minecraft".to_string(),
        }];

        assert!(instance_matches_targets(
            ProjectType::ResourcePack,
            "26.2",
            "vanilla",
            &targets,
        ));
        assert!(!instance_matches_targets(
            ProjectType::DataPack,
            "26.2",
            "vanilla",
            &targets,
        ));
        assert!(!instance_matches_targets(
            ProjectType::ResourcePack,
            "26.2",
            "vanilla",
            &[],
        ));
    }

    #[tokio::test]
    async fn installed_ids_use_local_refs_without_hash_or_remote_metadata() {
        let (_temp, state, resolved) = reconciliation_fixture().await;
        for (id, enabled, missing, provider_ref) in [
            (
                "modrinth",
                true,
                false,
                ContentProviderRef::Modrinth {
                    project_id: crate::state::ModrinthProjectId::new(
                        "projectA",
                    )
                    .unwrap(),
                    version_id: Some(
                        crate::state::ModrinthVersionId::new("versionA")
                            .unwrap(),
                    ),
                },
            ),
            (
                "disabled",
                false,
                false,
                ContentProviderRef::CurseForge {
                    project_id: crate::state::CurseForgeProjectId::new(123)
                        .unwrap(),
                    file_id: Some(
                        crate::state::CurseForgeFileId::new(456).unwrap(),
                    ),
                },
            ),
            (
                "missing",
                true,
                true,
                ContentProviderRef::Modrinth {
                    project_id: crate::state::ModrinthProjectId::new(
                        "projectB",
                    )
                    .unwrap(),
                    version_id: None,
                },
            ),
        ] {
            let file = insert_test_file(
                &resolved,
                &state,
                id,
                &format!("mods/{id}.jar"),
                "hash",
                enabled,
            )
            .await;
            let entry =
                insert_test_entry(&resolved, &state, &file, enabled).await;
            let mut tx = state.pool.begin().await.unwrap();
            sqlite::content_rows::upsert_content_provider_ref_in_transaction(
                &entry.id,
                &provider_ref,
                true,
                &mut tx,
            )
            .await
            .unwrap();
            sqlx::query("UPDATE instance_files SET missing = ? WHERE id = ?")
                .bind(missing)
                .bind(&file.id)
                .execute(&mut *tx)
                .await
                .unwrap();
            tx.commit().await.unwrap();
        }
        let mut ids = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            get_installed_project_ids_for_instance(
                &resolved.instance.id,
                None,
                &state,
            ),
        )
        .await
        .expect("installed IDs must not wait for remote metadata")
        .unwrap();
        ids.sort();
        assert_eq!(ids, ["curseforge:123", "projectA"]);

        let maps = load_entry_maps(&resolved.content_set.id, &state.pool)
            .await
            .unwrap();
        for (file_id, entry) in &maps.entries_by_file_id {
            assert_eq!(
                maps.provider_refs_by_file_id
                    .get(file_id)
                    .cloned()
                    .unwrap_or_default(),
                sqlite::content_rows::get_content_provider_refs(
                    &entry.id,
                    &state.pool
                )
                .await
                .unwrap(),
            );
            assert_eq!(
                maps.origin_provider_by_file_id.get(file_id).copied(),
                sqlite::content_rows::get_content_origin_provider(
                    &entry.id,
                    &state.pool
                )
                .await
                .unwrap(),
            );
        }
    }

    #[test]
    fn linked_modpack_ids_stay_provider_qualified() {
        let modrinth = InstanceLink::ModrinthModpack {
            project_id: "mr-project".to_string(),
            version_id: "mr-version".to_string(),
        };
        let curseforge = InstanceLink::CurseForgeModpack {
            project_id: "123".to_string(),
            version_id: "456".to_string(),
        };
        let imported = InstanceLink::ImportedModpack {
            project_id: Some("legacy-project".to_string()),
            version_id: Some("legacy-version".to_string()),
            name: None,
            version_number: None,
            filename: None,
        };

        assert_eq!(
            linked_modrinth_modpack_ids(&modrinth),
            Some(("mr-project", "mr-version")),
        );
        assert_eq!(linked_curseforge_modpack_ids(&modrinth), None);
        assert_eq!(linked_modrinth_modpack_ids(&curseforge), None);
        assert_eq!(
            linked_curseforge_modpack_ids(&curseforge),
            Some(("123", "456")),
        );
        assert_eq!(linked_modrinth_modpack_ids(&imported), None);
        assert_eq!(linked_curseforge_modpack_ids(&imported), None);
    }

    #[tokio::test]
    async fn exact_hash_backfill_preserves_entry_origin_aliases_and_revision() {
        let (_temp, state, resolved) = reconciliation_fixture().await;
        let file = insert_test_file(
            &resolved,
            &state,
            "file-existing",
            "mods/vuurkrabs-tweaks-1.0.0.jar",
            "exact-sha1",
            false,
        )
        .await;
        let entry = insert_test_entry(&resolved, &state, &file, false).await;
        let curseforge_ref = ContentProviderRef::CurseForge {
            project_id: crate::state::CurseForgeProjectId::new(123).unwrap(),
            file_id: Some(crate::state::CurseForgeFileId::new(456).unwrap()),
        };
        let mut tx = state.pool.begin().await.unwrap();
        sqlite::content_rows::upsert_content_provider_ref_in_transaction(
            &entry.id,
            &curseforge_ref,
            true,
            &mut tx,
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
        let revision = sqlite::content_rows::get_applied_content_set(
            &resolved.instance.id,
            &state.pool,
        )
        .await
        .unwrap()
        .unwrap()
        .revision;
        let matches = HashMap::from([(
            file.sha1.clone(),
            crate::state::ModrinthHashMatch {
                hash: file.sha1.clone(),
                project_id: "5LTBDHXu".to_string(),
                version_id: "eKsF3BdO".to_string(),
            },
        )]);

        assert!(
            reconcile_hash_matched_entries(
                &resolved,
                std::slice::from_ref(&file),
                &matches,
                &state,
            )
            .await
            .unwrap()
        );
        let stored_entry = sqlite::content_rows::get_content_entries(
            &resolved.content_set.id,
            &state.pool,
        )
        .await
        .unwrap()
        .into_iter()
        .find(|candidate| candidate.id == entry.id)
        .unwrap();
        assert_eq!(
            stored_entry.ownership_kind,
            ContentOwnershipKind::UserAdded
        );
        assert!(!stored_entry.auto_dependency);
        assert!(!stored_entry.enabled);
        assert_eq!(
            stored_entry.server_requirement,
            ContentRequirement::Optional
        );
        let refs = sqlite::content_rows::get_content_provider_refs_with_origin(
            &entry.id,
            &state.pool,
        )
        .await
        .unwrap();
        assert_eq!(refs.len(), 2);
        assert!(refs.iter().any(|(reference, origin)| {
            reference == &curseforge_ref && *origin
        }));
        assert!(refs.iter().any(|(reference, origin)| {
            matches!(
                reference,
                ContentProviderRef::Modrinth {
                    project_id,
                    version_id: Some(version_id),
                } if project_id.as_str() == "5LTBDHXu"
                    && version_id.as_str() == "eKsF3BdO"
            ) && !origin
        }));
        let bumped_revision = sqlite::content_rows::get_applied_content_set(
            &resolved.instance.id,
            &state.pool,
        )
        .await
        .unwrap()
        .unwrap()
        .revision;
        assert_eq!(bumped_revision, revision + 1);

        assert!(
            !reconcile_hash_matched_entries(
                &resolved,
                std::slice::from_ref(&file),
                &matches,
                &state,
            )
            .await
            .unwrap()
        );
        assert_eq!(
            sqlite::content_rows::get_applied_content_set(
                &resolved.instance.id,
                &state.pool,
            )
            .await
            .unwrap()
            .unwrap()
            .revision,
            bumped_revision
        );
        assert_eq!(
            sqlite::content_rows::get_content_provider_refs_with_origin(
                &entry.id,
                &state.pool,
            )
            .await
            .unwrap()
            .len(),
            2
        );
    }

    #[tokio::test]
    async fn external_file_edit_invalidates_exact_provider_refs() {
        let (_temp, state, resolved) = reconciliation_fixture().await;
        let relative_path = "mods/lithium.jar";
        let file = insert_test_file(
            &resolved,
            &state,
            "file-lithium",
            relative_path,
            "official-sha1",
            true,
        )
        .await;
        let entry = insert_test_entry(&resolved, &state, &file, true).await;
        let modrinth = ContentProviderRef::Modrinth {
            project_id: ModrinthProjectId::new("gvQqBUqZ").unwrap(),
            version_id: Some(ModrinthVersionId::new("Oqq8TOAV").unwrap()),
        };
        let curseforge = ContentProviderRef::CurseForge {
            project_id: crate::state::CurseForgeProjectId::new(123).unwrap(),
            file_id: Some(crate::state::CurseForgeFileId::new(456).unwrap()),
        };
        let mut tx = state.pool.begin().await.unwrap();
        sqlite::content_rows::upsert_content_provider_ref_in_transaction(
            &entry.id, &modrinth, true, &mut tx,
        )
        .await
        .unwrap();
        sqlite::content_rows::upsert_content_provider_ref_in_transaction(
            &entry.id,
            &curseforge,
            false,
            &mut tx,
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
        let mut tx = state.pool.begin().await.unwrap();
        assert!(
            sqlite::content_rows::invalidate_exact_provider_refs_for_file_in_transaction(
                &resolved.content_set.id,
                &file.id,
                &mut tx,
            )
            .await
            .unwrap()
        );
        sqlite::content_rows::bump_content_set_revision_in_transaction(
            &resolved.content_set.id,
            &mut tx,
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();

        let refs = sqlite::content_rows::get_content_provider_refs_with_origin(
            &entry.id,
            &state.pool,
        )
        .await
        .unwrap();
        assert_eq!(refs.len(), 2);
        assert!(refs.iter().any(|(reference, origin)| {
            *origin
                && matches!(
                    reference,
                    ContentProviderRef::Modrinth {
                        project_id,
                        version_id: None,
                    } if project_id.as_str() == "gvQqBUqZ"
                )
        }));
        assert!(refs.iter().any(|(reference, origin)| {
            !origin
                && matches!(
                    reference,
                    ContentProviderRef::CurseForge {
                        project_id,
                        file_id: None,
                    } if project_id.get() == 123
                )
        }));
        let snapshot = super::super::content_snapshot::get_content_snapshot(
            &resolved.instance.id,
            false,
            &state,
        )
        .await
        .unwrap();
        let item = snapshot
            .items
            .iter()
            .find(|item| item.entry_id.as_deref() == Some(&entry.id))
            .unwrap();
        assert_eq!(item.provider_project_id.as_deref(), Some("gvQqBUqZ"));
        assert_eq!(item.provider_release_id, None);
    }

    #[tokio::test]
    async fn exact_hash_creates_local_entry_but_similarity_and_invalid_match_do_not()
     {
        let (_temp, state, resolved) = reconciliation_fixture().await;
        let exact = insert_test_file(
            &resolved,
            &state,
            "file-exact",
            "mods/exact.disabled",
            "exact",
            false,
        )
        .await;
        let similar = insert_test_file(
            &resolved,
            &state,
            "file-similar",
            "mods/5LTBDHXu-eKsF3BdO.jar",
            "similar",
            true,
        )
        .await;
        let invalid = insert_test_file(
            &resolved,
            &state,
            "file-invalid",
            "mods/invalid.jar",
            "invalid",
            true,
        )
        .await;
        let matches = HashMap::from([
            (
                exact.sha1.clone(),
                crate::state::ModrinthHashMatch {
                    hash: exact.sha1.clone(),
                    project_id: "5LTBDHXu".to_string(),
                    version_id: "eKsF3BdO".to_string(),
                },
            ),
            (
                invalid.sha1.clone(),
                crate::state::ModrinthHashMatch {
                    hash: invalid.sha1.clone(),
                    project_id: "5LTBDHXu".to_string(),
                    version_id: String::new(),
                },
            ),
        ]);

        assert!(
            reconcile_hash_matched_entries(
                &resolved,
                &[exact.clone(), similar.clone(), invalid.clone()],
                &matches,
                &state,
            )
            .await
            .unwrap()
        );
        let entries = sqlite::content_rows::get_content_entries(
            &resolved.content_set.id,
            &state.pool,
        )
        .await
        .unwrap();
        let exact_entry = entries
            .iter()
            .find(|entry| entry.file_id.as_deref() == Some(&exact.id))
            .unwrap();
        assert_eq!(exact_entry.ownership_kind, ContentOwnershipKind::UserAdded);
        assert!(!exact_entry.auto_dependency);
        assert!(!exact_entry.enabled);
        assert_eq!(
            sqlite::content_rows::get_content_provider_refs_with_origin(
                &exact_entry.id,
                &state.pool,
            )
            .await
            .unwrap(),
            vec![(
                ContentProviderRef::Modrinth {
                    project_id: ModrinthProjectId::new("5LTBDHXu").unwrap(),
                    version_id: Some(
                        ModrinthVersionId::new("eKsF3BdO").unwrap(),
                    ),
                },
                false,
            )]
        );
        assert!(!entries.iter().any(|entry| {
            entry.file_id.as_deref() == Some(&similar.id)
                || entry.file_id.as_deref() == Some(&invalid.id)
        }));
    }

    #[tokio::test]
    async fn ordinary_snapshot_does_not_backfill_verified_identity() {
        let (_temp, state, resolved) = reconciliation_fixture().await;
        let file = insert_test_file(
            &resolved,
            &state,
            "file-read-only",
            "mods/read-only.jar",
            "read-only-hash",
            true,
        )
        .await;
        let revision = resolved.content_set.revision;

        let snapshot = super::super::content_snapshot::get_content_snapshot(
            &resolved.instance.id,
            false,
            &state,
        )
        .await
        .unwrap();

        assert!(snapshot.items.iter().any(|item| {
            item.file_id.as_deref() == Some(&file.id) && item.provider.is_none()
        }));
        assert!(
            sqlite::content_rows::get_content_entries(
                &resolved.content_set.id,
                &state.pool,
            )
            .await
            .unwrap()
            .is_empty()
        );
        assert_eq!(
            sqlite::content_rows::get_applied_content_set(
                &resolved.instance.id,
                &state.pool,
            )
            .await
            .unwrap()
            .unwrap()
            .revision,
            revision
        );
    }
}

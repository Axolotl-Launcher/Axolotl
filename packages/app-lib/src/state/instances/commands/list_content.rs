use super::sync_content_files::{
    installed_modrinth_version_id, instance_content_root, join_content_path,
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
use crate::state::store::Handle;
use crate::state::{
    CacheBehaviour, CachedEntry, ContentFile, ContentItem, ContentItemOwner,
    ContentItemProject, ContentItemRollback, ContentItemUpdate,
    ContentItemVersion, ContentProvider, ContentProviderRef, Dependency,
    LinkedModpackInfo, ModLoader, ModrinthFileMatch, ModrinthProjectId,
    ModrinthVersionId, Organization, OwnerType, Project, ProjectType,
    ReleaseChannel, TeamMember, Version, VersionV3, local_id,
};
use crate::util::fetch::{
    ContentValidation, DownloadMeta, DownloadReason, DownloadRequest,
    FetchSemaphore, Integrity, ResourceClass, download_to_path, sha1_async,
};
use async_zip::tokio::read::fs::ZipFileReader;
use dashmap::DashMap;
use sqlx::SqlitePool;
use std::collections::{HashMap, HashSet};
use std::path::{Component, Path};

mod linked_modpack;

pub(crate) use self::linked_modpack::get_linked_modpack_info;

mod items;

pub(crate) use self::items::dependencies_to_content_items;
use self::items::*;
use self::linked_modpack::*;

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
        sqlite::content_entries::get_content_entries(content_set_id, pool)
            .await?;
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
    for (file_id, reference, origin) in
        sqlite::content_entries::get_provider_refs_by_file_for_content_set(
            content_set_id,
            pool,
        )
        .await?
    {
        if let Some(provider) = origin {
            origin_provider_by_file_id.insert(file_id.clone(), provider);
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

pub(crate) async fn list_content_sets(
    instance_id: &str,
    pool: &SqlitePool,
) -> crate::Result<Vec<ContentSet>> {
    let instance = sqlite::instance_rows::get_instance_by_id(instance_id, pool)
        .await?
        .ok_or_else(|| {
            crate::ErrorKind::InputError("Unknown instance".to_string())
        })?;

    sqlite::content_sets::get_content_sets_for_instance(&instance.id, pool)
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
    for (provider, project_id) in
        sqlite::content_entries::get_provider_project_keys_for_present_files(
            &resolved.content_set.id,
            &state.pool,
        )
        .await?
    {
        if provider == "curseforge" {
            project_ids.insert(format!("curseforge:{project_id}"));
        } else {
            project_ids.insert(project_id);
        }
    }

    Ok(project_ids.into_iter().collect())
}

pub(crate) async fn get_instance_install_candidates(
    project_id: &str,
    project_type: ProjectType,
    targets: &[InstanceInstallTarget],
    pool: &SqlitePool,
) -> crate::Result<Vec<InstanceInstallCandidate>> {
    let rows =
        sqlite::instance_rows::get_install_candidates(project_id, pool).await?;

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
    let link =
        sqlite::links::get_instance_link(&resolved.instance.id, &state.pool)
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
                    &state.cache_handle(),
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
    _refresh_file_updates: bool,
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

    let files =
        sqlite::files::get_instance_files(&resolved.instance.id, &state.pool)
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
    let link =
        sqlite::links::get_instance_link(&resolved.instance.id, &state.pool)
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
        state,
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
                sqlite::content_sets::get_content_set(content_set_id, pool)
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
            sqlite::content_sets::get_applied_content_set(&instance.id, pool)
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
        let content_set = sqlite::content_sets::get_applied_content_set(
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
        sqlite::files::upsert_instance_file(&file, &mut tx)
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
        let entry = sqlite::content_entries::upsert_content_entry_from_parts_in_transaction(
            sqlite::content_entries::UpsertContentEntry {
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
            sqlite::content_entries::upsert_content_provider_ref_in_transaction(
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
                sqlite::content_entries::get_content_provider_refs(
                    &entry.id,
                    &state.pool
                )
                .await
                .unwrap(),
            );
            assert_eq!(
                maps.origin_provider_by_file_id.get(file_id).copied(),
                sqlite::content_entries::get_content_origin_provider(
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
        sqlite::content_entries::upsert_content_provider_ref_in_transaction(
            &entry.id,
            &curseforge_ref,
            true,
            &mut tx,
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
        let revision = sqlite::content_sets::get_applied_content_set(
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
        let stored_entry = sqlite::content_entries::get_content_entries(
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
        let refs =
            sqlite::content_entries::get_content_provider_refs_with_origin(
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
        let bumped_revision = sqlite::content_sets::get_applied_content_set(
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
            sqlite::content_sets::get_applied_content_set(
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
            sqlite::content_entries::get_content_provider_refs_with_origin(
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
        // The legacy cache projection uses global runtime state even when the
        // content snapshot itself is backed by the isolated fixture below.
        crate::event::EventState::init().await.unwrap();
        let runtime = tempfile::tempdir().unwrap().keep();
        State::init_for_test(runtime.to_string_lossy().into_owned())
            .await
            .unwrap();
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
        sqlite::content_entries::upsert_content_provider_ref_in_transaction(
            &entry.id, &modrinth, true, &mut tx,
        )
        .await
        .unwrap();
        sqlite::content_entries::upsert_content_provider_ref_in_transaction(
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
            sqlite::content_entries::invalidate_exact_provider_refs_for_file_in_transaction(
                &resolved.content_set.id,
                &file.id,
                &mut tx,
            )
            .await
            .unwrap()
        );
        sqlite::content_sets::bump_content_set_revision_in_transaction(
            &resolved.content_set.id,
            &mut tx,
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();

        let refs =
            sqlite::content_entries::get_content_provider_refs_with_origin(
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
        let entries = sqlite::content_entries::get_content_entries(
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
            sqlite::content_entries::get_content_provider_refs_with_origin(
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
            sqlite::content_entries::get_content_entries(
                &resolved.content_set.id,
                &state.pool,
            )
            .await
            .unwrap()
            .is_empty()
        );
        assert_eq!(
            sqlite::content_sets::get_applied_content_set(
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

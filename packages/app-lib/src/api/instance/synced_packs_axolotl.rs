//! Axolotl's shared resource/data-pack projection.
//!
//! The upstream implementation stores a launcher-wide JSON library and decorates
//! the upstream `ContentItem` with sync-only fields. Axolotl keeps content
//! ownership in SQLite, so this adapter stores only the shared catalog here and
//! lets the normal instance watcher materialize ownership in each content set.

use crate::api::instance::synced_servers::DesyncServerMode;
use crate::state::instances::adapters::sqlite::{
    content_entries, files, pack_members,
};
use crate::state::store::Collection as _;
use crate::state::store::atom;
use crate::state::store::synced_pack_catalog::{
    SyncedPackCatalog, SyncedPackRecord,
};
use crate::state::store::synced_pack_instances::{
    SyncedPackInstanceKey, SyncedPackInstanceRecord, SyncedPackInstances,
};
use crate::state::{
    ContentItem, ContentItemOwner, ContentItemProject, ContentItemRollback,
    ContentItemVersion, ContentOwnershipKind, ContentProvider,
    ContentProviderRef, ContentRequirement, ContentSourceKind, ProjectType,
    State,
};
use bytes::Bytes;
use chrono::Utc;
use serde::Serialize;
use std::collections::{BTreeSet, HashSet};
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Debug, Serialize)]
pub struct PackSyncTarget {
    pub instance_id: String,
    pub name: String,
    pub game_version: String,
    pub compatible: bool,
    pub participating: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct PackSyncPreview {
    pub pack: ContentItem,
    pub instances: Vec<PackSyncTarget>,
}

#[derive(Clone, Debug)]
struct PackRow {
    id: String,
    project_type: String,
    file_name: String,
    sha1: String,
    size: i64,
    game_versions_json: String,
    enabled: i64,
}

/// The row shape the catalog's callers work with.
fn pack_row(id: &str, record: &SyncedPackRecord) -> PackRow {
    PackRow {
        id: id.to_string(),
        project_type: record.project_type.clone(),
        file_name: record.file_name.clone(),
        sha1: record.sha1.clone(),
        size: record.size,
        game_versions_json: record.game_versions_json.clone(),
        enabled: i64::from(record.enabled),
    }
}

fn parse_type(value: &str) -> crate::Result<ProjectType> {
    match value {
        "resourcepack" => Ok(ProjectType::ResourcePack),
        "datapack" => Ok(ProjectType::DataPack),
        other => Err(crate::ErrorKind::InputError(format!(
            "Unsupported synced pack type: {other}"
        ))
        .into()),
    }
}

fn validate_type(project_type: ProjectType) -> crate::Result<()> {
    if matches!(
        project_type,
        ProjectType::ResourcePack | ProjectType::DataPack
    ) {
        Ok(())
    } else {
        Err(crate::ErrorKind::InputError(
            "Only resource packs and data packs can be synced.".to_string(),
        )
        .into())
    }
}

async fn globally_enabled(project_type: ProjectType) -> crate::Result<bool> {
    let option = match project_type {
        ProjectType::ResourcePack => crate::state::SyncedOption::ResourcePacks,
        ProjectType::DataPack => crate::state::SyncedOption::DataPacks,
        _ => return Ok(false),
    };
    Ok(crate::api::instance::synced_options::get_global_options()
        .await?
        .get(option))
}

fn cache_dir(state: &State) -> PathBuf {
    state.directories.synced_options_dir().join("packs/files")
}

fn cache_path(state: &State, sha1: &str) -> PathBuf {
    cache_dir(state).join(sha1)
}

fn content_root(
    state: &State,
    metadata: &crate::state::InstanceMetadata,
) -> crate::Result<PathBuf> {
    crate::state::instances::commands::instance_content_root(
        &state.directories,
        &metadata.instance,
    )
}

fn logical_path(
    project_type: ProjectType,
    file_name: &str,
    enabled: bool,
) -> String {
    format!(
        "{}/{}{}",
        project_type.get_folder(),
        file_name.trim_end_matches(".disabled"),
        if enabled { "" } else { ".disabled" }
    )
}

fn game_versions(row: &PackRow) -> Vec<String> {
    serde_json::from_str(&row.game_versions_json).unwrap_or_default()
}

fn resource_pack_option_entry(
    metadata: &crate::state::InstanceMetadata,
    entry: &str,
) -> String {
    let legacy = metadata
        .applied_content_set
        .game_version
        .strip_prefix("1.")
        .and_then(|version| version.split('.').next()?.parse::<u32>().ok())
        .is_some_and(|minor| minor < 13);
    if legacy {
        entry.strip_prefix("file/").unwrap_or(entry).to_string()
    } else {
        entry.to_string()
    }
}

fn version_compatible(
    row: &PackRow,
    metadata: &crate::state::InstanceMetadata,
) -> bool {
    let versions = game_versions(row);
    versions.is_empty()
        || versions.iter().any(|version| {
            version == &metadata.applied_content_set.game_version
        })
}

fn safe_relative_path(path: &str) -> Option<PathBuf> {
    let path = Path::new(path);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir
                    | Component::RootDir
                    | Component::Prefix(_)
            )
        })
    {
        return None;
    }
    Some(path.to_path_buf())
}

async fn tracked_materialized_path(
    state: &State,
    pack_id: &str,
    instance_id: &str,
) -> crate::Result<Option<String>> {
    Ok(SyncedPackInstances::get(
        &state.data,
        &SyncedPackInstanceKey {
            pack_id: pack_id.to_string(),
            instance_id: instance_id.to_string(),
        },
    )
    .await?
    .and_then(|record| record.materialized_path))
}

fn normalize_tracked_path(
    metadata: &crate::state::InstanceMetadata,
    tracked: &str,
) -> Option<String> {
    let normalized = tracked.replace('\\', "/");
    let instance_path = metadata.instance.path.replace('\\', "/");
    let relative = normalized
        .strip_prefix(&format!("{instance_path}/"))
        .unwrap_or(&normalized);
    safe_relative_path(relative)
        .map(|path| path.to_string_lossy().replace('\\', "/"))
}

async fn file_matches_sha1(path: &Path, expected: &str) -> bool {
    match tokio::fs::read(path).await {
        Ok(bytes) => crate::util::fetch::sha1_async(Bytes::from(bytes))
            .await
            .is_ok_and(|sha1| sha1 == expected),
        Err(_) => false,
    }
}

async fn cleanup_materialization(
    state: &State,
    row: &PackRow,
    metadata: &crate::state::InstanceMetadata,
) -> crate::Result<()> {
    let Some(tracked) =
        tracked_materialized_path(state, &row.id, &metadata.instance.id)
            .await?
    else {
        return Ok(());
    };
    let Some(relative_path) = normalize_tracked_path(metadata, &tracked) else {
        tracing::warn!(
            "Ignoring unsafe synced-pack materialized path for {}: {tracked}",
            metadata.instance.id
        );
        return Ok(());
    };
    let absolute = content_root(state, metadata)?.join(&relative_path);
    if file_matches_sha1(&absolute, &row.sha1).await {
        match tokio::fs::remove_file(&absolute).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }

    let mut transaction = state.pool.begin().await?;
    pack_members::delete_shared_member_in_transaction(
        &row.id,
        &metadata.instance.id,
        &mut transaction,
    )
    .await?;
    content_entries::delete_shared_entries_for_path_in_transaction(
        &metadata.instance.id,
        &relative_path,
        &mut transaction,
    )
    .await?;
    files::delete_unreferenced_file_in_transaction(
        &metadata.instance.id,
        &relative_path,
        &mut transaction,
    )
    .await?;
    transaction.commit().await?;
    Ok(())
}

async fn record_materialization(
    state: &State,
    pack_id: &str,
    instance_id: &str,
    excluded: bool,
    materialized_path: Option<&str>,
) -> crate::Result<()> {
    SyncedPackInstances::set(
        &state.data,
        &SyncedPackInstanceKey {
            pack_id: pack_id.to_string(),
            instance_id: instance_id.to_string(),
        },
        Some(&SyncedPackInstanceRecord {
            excluded,
            materialized_path: materialized_path.map(ToString::to_string),
            modified_at: Utc::now().timestamp(),
        }),
    )
    .await?;
    Ok(())
}

fn row_item(row: &PackRow) -> crate::Result<ContentItem> {
    let project_type = parse_type(&row.project_type)?;
    let file_name = row.file_name.trim_end_matches(".disabled").to_string();
    Ok(ContentItem {
        file_name: file_name.clone(),
        file_path: logical_path(project_type, &file_name, row.enabled != 0),
        id: row.id.clone(),
        size: row.size.max(0) as u64,
        enabled: row.enabled != 0,
        project_type,
        project: None::<ContentItemProject>,
        version: None::<ContentItemVersion>,
        owner: None::<ContentItemOwner>,
        update: None,
        date_added: None,
        provider_refs: Vec::<ContentProviderRef>::new(),
        origin_provider: None::<ContentProvider>,
        rollback: None::<ContentItemRollback>,
        environment: None,
        source_kind: Some(ContentSourceKind::SharedInstance),
        external: true,
        loader: None,
    })
}

async fn load_row(id: &str, state: &State) -> crate::Result<PackRow> {
    SyncedPackCatalog::get(&state.data, &id.to_string())
        .await?
        .map(|record| pack_row(id, &record))
        .ok_or_else(|| {
            crate::ErrorKind::InputError("Unknown synced pack".to_string())
                .into()
        })
}

async fn write_cache(
    state: &State,
    bytes: &Bytes,
    sha1: &str,
) -> crate::Result<()> {
    tokio::fs::create_dir_all(cache_dir(state)).await?;
    let path = cache_path(state, sha1);
    if !tokio::fs::try_exists(&path).await? {
        tokio::fs::write(path, bytes).await?;
    }
    Ok(())
}

async fn materialize(
    state: &State,
    row: &PackRow,
    instance_id: &str,
    excluded: bool,
) -> crate::Result<()> {
    let metadata = crate::state::get_instance(instance_id, &state.pool)
        .await?
        .ok_or_else(|| {
            crate::ErrorKind::InputError("Unknown instance".to_string())
        })?;
    let project_type = parse_type(&row.project_type)?;
    let root = content_root(state, &metadata)?;
    let relative_path =
        logical_path(project_type, &row.file_name, row.enabled != 0);
    let destination = root.join(&relative_path);
    if excluded
        || !globally_enabled(project_type).await?
        || !metadata.synced_options_for(project_type)
        || !version_compatible(row, &metadata)
    {
        cleanup_materialization(state, row, &metadata).await?;
        record_materialization(state, &row.id, instance_id, excluded, None)
            .await?;
        return Ok(());
    }

    let tracked =
        tracked_materialized_path(state, &row.id, instance_id).await?;
    let owns_destination = tracked
        .as_deref()
        .and_then(|path| normalize_tracked_path(&metadata, path))
        .is_some_and(|path| path == relative_path);
    if tokio::fs::try_exists(&destination).await?
        && !owns_destination
        && !file_matches_sha1(&destination, &row.sha1).await
    {
        return Err(crate::ErrorKind::InputError(format!(
            "Cannot sync {} because {} already contains a different local file.",
            row.file_name,
            destination.display()
        ))
        .into());
    }
    if tracked.as_deref().is_some_and(|path| {
        normalize_tracked_path(&metadata, path)
            .is_some_and(|path| path != relative_path)
    }) {
        cleanup_materialization(state, row, &metadata).await?;
    }
    if let Some(parent) = destination.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let bytes = tokio::fs::read(cache_path(state, &row.sha1)).await?;
    let destination_matches = tokio::fs::read(&destination)
        .await
        .is_ok_and(|existing| existing == bytes);
    if !destination_matches {
        tokio::fs::write(&destination, &bytes).await?;
    }
    let file = files::upsert_instance_file_from_parts(
        crate::state::instances::adapters::sqlite::files::UpsertInstanceFile {
            instance_id,
            relative_path: &relative_path,
            file_name: &row.file_name,
            enabled: row.enabled != 0,
            sha1: &row.sha1,
            size: row.size.max(0) as u64,
            missing: false,
            local_mod_data: None,
            icon_path: None,
        },
        &state.pool,
    )
    .await?;
    let entry = content_entries::upsert_content_entry_from_parts(
        content_entries::UpsertContentEntry {
            instance_id,
            content_set_id: &metadata.applied_content_set.id,
            file_id: Some(&file.id),
            project_type,
            source_kind: ContentSourceKind::SharedInstance,
            ownership_kind: ContentOwnershipKind::PackManaged,
            auto_dependency: false,
            server_requirement: ContentRequirement::Optional,
            client_requirement: ContentRequirement::Optional,
            enabled: row.enabled != 0,
        },
        &state.pool,
    )
    .await?;
    let mut transaction = state.pool.begin().await?;
    pack_members::upsert_shared_pack_member_in_transaction(
        &row.id,
        &metadata.applied_content_set.id,
        &entry.id,
        project_type.get_name(),
        &relative_path,
        &row.sha1,
        row.size,
        Utc::now().timestamp(),
        &mut transaction,
    )
    .await?;
    transaction.commit().await?;
    record_materialization(
        state,
        &row.id,
        instance_id,
        excluded,
        Some(&relative_path),
    )
    .await?;
    Ok(())
}

trait SyncedPackInstanceOptions {
    fn synced_options_for(&self, project_type: ProjectType) -> bool;
}

impl SyncedPackInstanceOptions for crate::state::InstanceMetadata {
    fn synced_options_for(&self, project_type: ProjectType) -> bool {
        match project_type {
            ProjectType::ResourcePack => self.synced_options.resource_packs,
            ProjectType::DataPack => self.synced_options.data_packs,
            _ => false,
        }
    }
}

pub async fn list_synced_packs(
    project_type: ProjectType,
) -> crate::Result<Vec<ContentItem>> {
    validate_type(project_type)?;
    let state = State::get().await?;
    let rows = SyncedPackCatalog::list_of_project_type(
        &state.data,
        project_type.get_name(),
    )
    .await?;
    rows.iter()
        .map(|(id, record)| row_item(&pack_row(id, record)))
        .collect()
}

pub async fn get_pack_sync_preview(
    instance_id: &str,
    project_path: &str,
) -> crate::Result<PackSyncPreview> {
    let state = State::get().await?;
    let metadata = crate::state::get_instance(instance_id, &state.pool)
        .await?
        .ok_or_else(|| {
            crate::ErrorKind::InputError("Unknown instance".to_string())
        })?;
    let project_type = project_path
        .split('/')
        .next()
        .and_then(|folder| match folder {
            "resourcepacks" => Some(ProjectType::ResourcePack),
            "datapacks" => Some(ProjectType::DataPack),
            _ => None,
        })
        .ok_or_else(|| {
            crate::ErrorKind::InputError("Invalid pack path".to_string())
        })?;
    validate_type(project_type)?;
    let global_enabled = globally_enabled(project_type).await?;
    let source = content_root(&state, &metadata)?.join(project_path);
    let bytes = Bytes::from(tokio::fs::read(&source).await?);
    let sha1 = crate::util::fetch::sha1_async(bytes.clone()).await?;
    let row = SyncedPackCatalog::get_by_sha1(&state.data, &sha1)
        .await?
        .map(|(id, record)| pack_row(&id, &record));
    let preview_row = row.unwrap_or(PackRow {
        id: format!("synced-pack:{sha1}"),
        project_type: project_type.get_name().to_string(),
        file_name: Path::new(project_path)
            .file_name()
            .and_then(|x| x.to_str())
            .unwrap_or("pack.zip")
            .to_string(),
        sha1,
        size: bytes.len() as i64,
        game_versions_json: "[]".to_string(),
        enabled: 1,
    });
    let pack = row_item(&preview_row)?;
    let instances = crate::state::list_instances(&state.pool)
        .await?
        .into_iter()
        .map(|item| {
            let participating =
                global_enabled && item.synced_options_for(project_type);
            let versions = game_versions(&preview_row);
            let compatible = versions.is_empty()
                || versions.iter().any(|version| {
                    version == &item.applied_content_set.game_version
                });
            PackSyncTarget {
                instance_id: item.instance.id,
                name: item.instance.name,
                game_version: item.applied_content_set.game_version,
                compatible,
                participating,
            }
        })
        .collect();
    Ok(PackSyncPreview { pack, instances })
}

pub async fn sync_pack(
    instance_id: &str,
    project_path: &str,
) -> crate::Result<()> {
    let state = State::get().await?;
    let preview = get_pack_sync_preview(instance_id, project_path).await?;
    if !globally_enabled(preview.pack.project_type).await? {
        return Err(crate::ErrorKind::InputError(
            "This sync option is globally disabled.".to_string(),
        )
        .into());
    }
    if !preview
        .instances
        .iter()
        .any(|target| target.instance_id == instance_id && target.participating)
    {
        return Err(crate::ErrorKind::InputError(
            "This sync option is disabled for the source instance.".to_string(),
        )
        .into());
    }
    let source_metadata = crate::state::get_instance(instance_id, &state.pool)
        .await?
        .ok_or_else(|| {
            crate::ErrorKind::InputError("Unknown instance".to_string())
        })?;
    let source = content_root(&state, &source_metadata)?.join(project_path);
    let bytes = Bytes::from(tokio::fs::read(source).await?);
    let sha1 = crate::util::fetch::sha1_async(bytes.clone()).await?;
    write_cache(&state, &bytes, &sha1).await?;
    let now = Utc::now().timestamp();
    let (catalog_id, catalog) = SyncedPackCatalog::record_upload(
        &state.data,
        &preview.pack.id,
        &SyncedPackRecord {
            project_type: preview.pack.project_type.get_name().to_string(),
            file_name: preview.pack.file_name.clone(),
            sha1: sha1.clone(),
            size: bytes.len() as i64,
            game_versions_json: "[]".to_string(),
            enabled: true,
            selected: true,
            selection_order: None,
            created_at: now,
            modified_at: now,
        },
    )
    .await?;
    let catalog = pack_row(&catalog_id, &catalog);
    for target in preview
        .instances
        .into_iter()
        .filter(|target| target.participating)
    {
        materialize(&state, &catalog, &target.instance_id, false).await?;
    }
    Ok(())
}

pub async fn upload_synced_pack(
    path: PathBuf,
    project_type: ProjectType,
    game_versions: Vec<String>,
) -> crate::Result<()> {
    validate_type(project_type)?;
    let state = State::get().await?;
    if !globally_enabled(project_type).await? {
        return Err(crate::ErrorKind::InputError(
            "This sync option is globally disabled.".to_string(),
        )
        .into());
    }
    let bytes = Bytes::from(tokio::fs::read(&path).await?);
    let sha1 = crate::util::fetch::sha1_async(bytes.clone()).await?;
    write_cache(&state, &bytes, &sha1).await?;
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            crate::ErrorKind::InputError("Invalid pack filename".to_string())
        })?
        .to_string();
    let now = Utc::now().timestamp();
    SyncedPackCatalog::record_shared_upload(
        &state.data,
        &format!("synced-pack:{sha1}"),
        &SyncedPackRecord {
            project_type: project_type.get_name().to_string(),
            file_name: name,
            sha1: sha1.clone(),
            size: bytes.len() as i64,
            game_versions_json: serde_json::to_string(&game_versions)?,
            enabled: true,
            selected: true,
            selection_order: None,
            created_at: now,
            modified_at: now,
        },
    )
    .await?;
    Ok(())
}

pub async fn set_synced_pack_enabled(
    pack_id: &str,
    enabled: bool,
) -> crate::Result<()> {
    let state = State::get().await?;
    let mut row = load_row(pack_id, &state).await?;
    if !globally_enabled(parse_type(&row.project_type)?).await? {
        return Err(crate::ErrorKind::InputError(
            "This sync option is globally disabled.".to_string(),
        )
        .into());
    }
    SyncedPackCatalog::set_enabled(
        &state.data,
        pack_id,
        enabled,
        Utc::now().timestamp(),
    )
    .await?;
    row.enabled = i64::from(enabled);
    for (key, record) in
        SyncedPackInstances::get_scoped(&state.data, &pack_id.to_string())
            .await?
    {
        if record.excluded {
            continue;
        }
        materialize(&state, &row, &key.instance_id, false).await?;
    }
    Ok(())
}

pub async fn desync_pack(
    instance_id: &str,
    pack_id: &str,
    mode: DesyncServerMode,
) -> crate::Result<()> {
    let state = State::get().await?;
    let row = load_row(pack_id, &state).await?;
    materialize(&state, &row, instance_id, true).await?;
    if mode == DesyncServerMode::RemoveFromOtherInstances {
        let links =
            SyncedPackInstances::get_scoped(&state.data, &pack_id.to_string())
                .await?;
        let mut kept = Vec::new();
        for (key, record) in links {
            if key.instance_id == instance_id {
                kept.push((key, record));
                continue;
            }
            if let Some(metadata) =
                crate::state::get_instance(&key.instance_id, &state.pool)
                    .await?
            {
                cleanup_materialization(&state, &row, &metadata).await?;
            }
        }
        SyncedPackInstances::set_scoped(
            &state.data,
            &pack_id.to_string(),
            &kept,
        )
        .await?;
    }
    Ok(())
}

pub async fn remove_synced_pack(pack_id: &str) -> crate::Result<()> {
    let state = State::get().await?;
    let row = load_row(pack_id, &state).await?;
    let links =
        SyncedPackInstances::get_scoped(&state.data, &pack_id.to_string())
            .await?;
    for (key, _) in links {
        if let Some(metadata) =
            crate::state::get_instance(&key.instance_id, &state.pool).await?
        {
            cleanup_materialization(&state, &row, &metadata).await?;
        }
    }
    SyncedPackCatalog::set(&state.data, &pack_id.to_string(), None).await?;
    Ok(())
}

pub(crate) async fn detach(
    metadata: &crate::state::InstanceMetadata,
    option: crate::state::SyncedOption,
    state: &State,
) -> crate::Result<()> {
    let project_type = match option {
        crate::state::SyncedOption::ResourcePacks => ProjectType::ResourcePack,
        crate::state::SyncedOption::DataPacks => ProjectType::DataPack,
        _ => return Ok(()),
    };
    let rows = SyncedPackCatalog::list_of_project_type(
        &state.data,
        project_type.get_name(),
    )
    .await?;
    for (id, record) in rows {
        let row = pack_row(&id, &record);
        let excluded = SyncedPackInstances::get(
            &state.data,
            &SyncedPackInstanceKey {
                pack_id: row.id.clone(),
                instance_id: metadata.instance.id.clone(),
            },
        )
        .await?
        .is_some_and(|record| record.excluded);
        cleanup_materialization(state, &row, metadata).await?;
        record_materialization(
            state,
            &row.id,
            &metadata.instance.id,
            excluded,
            None,
        )
        .await?;
    }
    Ok(())
}

pub(crate) async fn prepare_instance_update(
    metadata: &crate::state::InstanceMetadata,
    state: &State,
) -> crate::Result<()> {
    // Remove only files owned by the shared catalog before an install or
    // upgrade writes the instance directory. User-owned files are preserved by
    // cleanup_materialization's content hash check and remain local overrides.
    for project_type in [ProjectType::ResourcePack, ProjectType::DataPack] {
        if !metadata.synced_options_for(project_type) {
            continue;
        }
        let rows = SyncedPackCatalog::list_of_project_type(
            &state.data,
            project_type.get_name(),
        )
        .await?;
        for (id, record) in rows {
            let row = pack_row(&id, &record);
            let excluded = SyncedPackInstances::get(
                &state.data,
                &SyncedPackInstanceKey {
                    pack_id: row.id.clone(),
                    instance_id: metadata.instance.id.clone(),
                },
            )
            .await?
            .is_some_and(|record| record.excluded);
            cleanup_materialization(state, &row, metadata).await?;
            record_materialization(
                state,
                &row.id,
                &metadata.instance.id,
                excluded,
                None,
            )
            .await?;
        }
    }
    Ok(())
}

pub(crate) async fn reconcile(
    metadata: &crate::state::InstanceMetadata,
    option: crate::state::SyncedOption,
    state: &State,
) -> crate::Result<()> {
    let project_type = match option {
        crate::state::SyncedOption::ResourcePacks => ProjectType::ResourcePack,
        crate::state::SyncedOption::DataPacks => ProjectType::DataPack,
        _ => return Ok(()),
    };
    let rows = SyncedPackCatalog::list_of_project_type(
        &state.data,
        project_type.get_name(),
    )
    .await?;
    for (id, record) in rows {
        let row = pack_row(&id, &record);
        let excluded = SyncedPackInstances::get(
            &state.data,
            &SyncedPackInstanceKey {
                pack_id: row.id.clone(),
                instance_id: metadata.instance.id.clone(),
            },
        )
        .await?
        .is_some_and(|record| record.excluded);
        materialize(state, &row, &metadata.instance.id, excluded).await?;
    }
    Ok(())
}

pub(crate) async fn capture_resource_pack_selection_change(
    metadata: &crate::state::InstanceMetadata,
    state: &State,
) -> crate::Result<()> {
    let Some(options) = crate::api::instance::synced_options::game_options::read_resource_pack_entries(metadata, state).await? else {
        return Ok(());
    };
    let global_enabled = globally_enabled(ProjectType::ResourcePack).await?;
    if !global_enabled || !metadata.synced_options.resource_packs {
        return Ok(());
    }
    let rows = SyncedPackCatalog::list_of_project_type(
        &state.data,
        ProjectType::ResourcePack.get_name(),
    )
    .await?;
    let mut managed = BTreeSet::new();
    let mut selected_names = BTreeSet::new();
    let mut unit = atom(&state.data).await?;
    for (order, (id, record)) in rows.iter().enumerate() {
        let file_name = record.file_name.clone();
        let entry = format!("file/{file_name}");
        let legacy = file_name.clone();
        let is_selected = options
            .entries
            .iter()
            .any(|candidate| candidate == &entry || candidate == &legacy);
        managed.insert(entry.clone());
        managed.insert(legacy);
        SyncedPackCatalog::set_selection_in(
            &mut unit,
            id,
            is_selected,
            options
                .entries
                .iter()
                .position(|candidate| {
                    candidate == &entry || candidate == &file_name
                })
                .map(|value| value as i64)
                .or(Some(order as i64)),
        )
        .await?;
        if is_selected {
            selected_names.insert(entry);
        }
    }
    unit.commit().await?;
    let selected = options
        .entries
        .iter()
        .filter(|entry| managed.contains(*entry))
        .filter(|entry| {
            selected_names.contains(*entry)
                || selected_names
                    .contains(entry.strip_prefix("file/").unwrap_or(entry))
        })
        .cloned()
        .collect::<Vec<_>>();

    for target in crate::state::list_instances(&state.pool).await? {
        if target.instance.id == metadata.instance.id
            || !target.synced_options.resource_packs
        {
            continue;
        }
        let target_selected = selected
            .iter()
            .map(|entry| resource_pack_option_entry(&target, entry))
            .collect::<Vec<_>>();
        let _ = crate::api::instance::synced_options::game_options::merge_resource_pack_entries(
            &target,
            &managed,
            &target_selected,
            state,
        )
        .await?;
    }
    Ok(())
}

pub(crate) async fn seed_from_instance(
    metadata: &crate::state::InstanceMetadata,
    option: crate::state::SyncedOption,
    state: &State,
) -> crate::Result<()> {
    let project_type = match option {
        crate::state::SyncedOption::ResourcePacks => ProjectType::ResourcePack,
        crate::state::SyncedOption::DataPacks => ProjectType::DataPack,
        _ => return Ok(()),
    };
    let root = content_root(state, metadata)?;
    let directory = root.join(project_type.get_folder());
    let mut entries = match tokio::fs::read_dir(&directory).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    };
    let game_versions_json =
        serde_json::to_string(&[&metadata.applied_content_set.game_version])?;
    let mut seen_sha1 = HashSet::new();

    while let Some(entry) = entries.next_entry().await? {
        if !entry.file_type().await?.is_file() {
            continue;
        }
        let path = entry.path();
        let Some(file_name) = entry.file_name().to_str().map(ToOwned::to_owned)
        else {
            continue;
        };
        let bytes = Bytes::from(tokio::fs::read(&path).await?);
        let sha1 = crate::util::fetch::sha1_async(bytes.clone()).await?;
        seen_sha1.insert(sha1.clone());
        write_cache(state, &bytes, &sha1).await?;
        let enabled = !file_name.ends_with(".disabled");
        let file_name = file_name.trim_end_matches(".disabled");
        let now = Utc::now().timestamp();
        let catalog = SyncedPackCatalog::list_of_project_type(
            &state.data,
            project_type.get_name(),
        )
        .await?;
        let mut replaced_rows = Vec::new();
        for (id, record) in catalog {
            if record.file_name == file_name && record.sha1 != sha1 {
                replaced_rows.push(pack_row(&id, &record));
            }
        }
        for replaced in &replaced_rows {
            for (key, _) in
                SyncedPackInstances::get_scoped(&state.data, &replaced.id)
                    .await?
            {
                if let Some(target_metadata) =
                    crate::state::get_instance(&key.instance_id, &state.pool)
                        .await?
                {
                    cleanup_materialization(state, replaced, &target_metadata)
                        .await?;
                }
            }
        }
        let mut unit = atom(&state.data).await?;
        SyncedPackCatalog::replace_seeded_in(
            &mut unit,
            &replaced_rows
                .iter()
                .map(|row| row.id.clone())
                .collect::<Vec<_>>(),
            &format!("synced-pack:{sha1}"),
            &SyncedPackRecord {
                project_type: project_type.get_name().to_string(),
                file_name: file_name.to_string(),
                sha1: sha1.clone(),
                size: bytes.len() as i64,
                game_versions_json: game_versions_json.clone(),
                enabled,
                selected: true,
                selection_order: None,
                created_at: now,
                modified_at: now,
            },
        )
        .await?;
        unit.commit().await?;
    }

    // A watcher event can report a directory after a file was removed. Reconcile
    // catalog entries materialized in this source instance so stale files do not
    // remain in other participating instances.
    let tracked = SyncedPackCatalog::materialized_for_instance(
        &state.data,
        project_type.get_name(),
        &metadata.instance.id,
    )
    .await?;
    for (id, record, materialized_path) in tracked {
        let row = pack_row(&id, &record);
        if seen_sha1.contains(&row.sha1) {
            continue;
        }
        let Some(path) = normalize_tracked_path(metadata, &materialized_path)
        else {
            continue;
        };
        if tokio::fs::try_exists(&content_root(state, metadata)?.join(path))
            .await?
        {
            continue;
        }
        for (key, _) in
            SyncedPackInstances::get_scoped(&state.data, &row.id).await?
        {
            if let Some(target_metadata) =
                crate::state::get_instance(&key.instance_id, &state.pool)
                    .await?
            {
                cleanup_materialization(state, &row, &target_metadata).await?;
            }
        }
        SyncedPackCatalog::set(&state.data, &row.id, None).await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logical_paths_preserve_disabled_state() {
        assert_eq!(
            logical_path(ProjectType::ResourcePack, "example.zip", true),
            "resourcepacks/example.zip"
        );
        assert_eq!(
            logical_path(ProjectType::DataPack, "example.zip", false),
            "datapacks/example.zip.disabled"
        );
    }

    #[test]
    fn unsafe_materialized_paths_are_rejected() {
        assert!(safe_relative_path("resourcepacks/example.zip").is_some());
        assert!(safe_relative_path("../outside.zip").is_none());
        assert!(safe_relative_path("/absolute.zip").is_none());
        assert!(
            safe_relative_path("resourcepacks/../../outside.zip").is_none()
        );
    }

    #[test]
    fn malformed_game_version_metadata_is_treated_as_unrestricted() {
        let row = PackRow {
            id: "pack".to_string(),
            project_type: "resourcepack".to_string(),
            file_name: "example.zip".to_string(),
            sha1: "sha1".to_string(),
            size: 1,
            game_versions_json: "not-json".to_string(),
            enabled: 1,
        };
        assert!(game_versions(&row).is_empty());
    }
}

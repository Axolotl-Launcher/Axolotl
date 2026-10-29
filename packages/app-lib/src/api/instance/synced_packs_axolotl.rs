//! Axolotl's shared resource/data-pack projection.
//!
//! The upstream implementation stores a launcher-wide JSON library and decorates
//! the upstream `ContentItem` with sync-only fields. Axolotl keeps content
//! ownership in SQLite, so this adapter stores only the shared catalog here and
//! lets the normal instance watcher materialize ownership in each content set.

use crate::api::instance::synced_servers::DesyncServerMode;
use crate::state::{
    ContentItem, ContentItemOwner, ContentItemProject, ContentItemRollback,
    ContentItemVersion, ContentOwnershipKind, ContentProvider,
    ContentProviderRef, ContentRequirement, ContentSourceKind, ProjectType,
    State,
};
use bytes::Bytes;
use chrono::Utc;
use serde::Serialize;
use sqlx::Row;
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

#[derive(Clone, Debug, sqlx::FromRow)]
struct PackRow {
    id: String,
    project_type: String,
    file_name: String,
    sha1: String,
    size: i64,
    game_versions_json: String,
    enabled: i64,
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
    Ok(sqlx::query_scalar::<_, Option<String>>(
        "SELECT materialized_path FROM synced_pack_instances
         WHERE pack_id = ? AND instance_id = ?",
    )
    .bind(pack_id)
    .bind(instance_id)
    .fetch_optional(&state.pool)
    .await?
    .flatten())
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
    sqlx::query(
        "DELETE FROM instance_pack_members
         WHERE member_key = ?
           AND content_set_id IN (
             SELECT id FROM instance_content_sets WHERE instance_id = ?
           )",
    )
    .bind(format!("shared:{}", row.id))
    .bind(&metadata.instance.id)
    .execute(&mut *transaction)
    .await?;
    sqlx::query(
        "DELETE FROM instance_content_entries
         WHERE instance_id = ?
           AND source_kind = 'shared_instance'
           AND file_id IN (
             SELECT id FROM instance_files
             WHERE instance_id = ? AND relative_path = ?
           )",
    )
    .bind(&metadata.instance.id)
    .bind(&metadata.instance.id)
    .bind(&relative_path)
    .execute(&mut *transaction)
    .await?;
    sqlx::query(
        "DELETE FROM instance_files
         WHERE instance_id = ? AND relative_path = ?
           AND NOT EXISTS (
             SELECT 1 FROM instance_content_entries
             WHERE file_id = instance_files.id
           )",
    )
    .bind(&metadata.instance.id)
    .bind(&relative_path)
    .execute(&mut *transaction)
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
    sqlx::query(
        "INSERT INTO synced_pack_instances
         (pack_id, instance_id, excluded, materialized_path, modified_at)
         VALUES (?, ?, ?, ?, ?)
         ON CONFLICT(pack_id, instance_id) DO UPDATE SET
           excluded=excluded.excluded,
           materialized_path=excluded.materialized_path,
           modified_at=excluded.modified_at",
    )
    .bind(pack_id)
    .bind(instance_id)
    .bind(i64::from(excluded))
    .bind(materialized_path)
    .bind(Utc::now().timestamp())
    .execute(&state.pool)
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
    sqlx::query_as::<_, PackRow>(
        "SELECT id, project_type, file_name, sha1, size, game_versions_json, enabled
         FROM synced_pack_catalog WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| crate::ErrorKind::InputError("Unknown synced pack".to_string()).into())
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
    let file = crate::state::instances::adapters::sqlite::files::upsert_instance_file_from_parts(
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
    let entry = crate::state::instances::adapters::sqlite::content_entries::upsert_content_entry_from_parts(
        crate::state::instances::adapters::sqlite::content_entries::UpsertContentEntry {
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
    sqlx::query(
        "INSERT INTO instance_pack_members
         (id, content_set_id, content_entry_id, member_key, project_type,
          expected_relative_path, required, expected_sha1, expected_size,
          materialization_state, override_kind, reconciled, created_at, modified_at)
         VALUES (?, ?, ?, ?, ?, ?, 0, ?, ?, 'present', 'none', 1, ?, ?)
         ON CONFLICT(content_set_id, member_key) DO UPDATE SET
           content_entry_id=excluded.content_entry_id,
           expected_sha1=excluded.expected_sha1, expected_size=excluded.expected_size,
           materialization_state='present', override_kind='none',
           reconciled=1, modified_at=excluded.modified_at",
    )
    .bind(format!("pack-member:{}", row.id))
    .bind(&metadata.applied_content_set.id)
    .bind(&entry.id)
    .bind(format!("shared:{}", row.id))
    .bind(project_type.get_name())
    .bind(&relative_path)
    .bind(&row.sha1)
    .bind(row.size)
    .bind(Utc::now().timestamp())
    .bind(Utc::now().timestamp())
    .execute(&state.pool)
    .await?;
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
    let rows = sqlx::query_as::<_, PackRow>(
        "SELECT id, project_type, file_name, sha1, size, game_versions_json, enabled
         FROM synced_pack_catalog WHERE project_type = ? ORDER BY modified_at DESC",
    )
    .bind(project_type.get_name())
    .fetch_all(&state.pool)
    .await?;
    rows.iter().map(row_item).collect()
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
    let row = sqlx::query_as::<_, PackRow>(
        "SELECT id, project_type, file_name, sha1, size, game_versions_json, enabled
         FROM synced_pack_catalog WHERE sha1 = ?",
    ).bind(&sha1).fetch_optional(&state.pool).await?;
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
    let row = sqlx::query("INSERT INTO synced_pack_catalog(id, project_type, file_name, sha1, size, game_versions_json, enabled, created_at, modified_at) VALUES (?, ?, ?, ?, ?, '[]', 1, ?, ?) ON CONFLICT(sha1) DO UPDATE SET modified_at=excluded.modified_at RETURNING id, project_type, file_name, sha1, size, game_versions_json, enabled").bind(&preview.pack.id).bind(preview.pack.project_type.get_name()).bind(&preview.pack.file_name).bind(&sha1).bind(bytes.len() as i64).bind(now).bind(now).fetch_one(&state.pool).await?;
    let catalog = PackRow {
        id: row.try_get("id")?,
        project_type: row.try_get("project_type")?,
        file_name: row.try_get("file_name")?,
        sha1: row.try_get("sha1")?,
        size: row.try_get("size")?,
        game_versions_json: row.try_get("game_versions_json")?,
        enabled: row.try_get("enabled")?,
    };
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
    sqlx::query("INSERT INTO synced_pack_catalog(id, project_type, file_name, sha1, size, game_versions_json, enabled, created_at, modified_at) VALUES (?, ?, ?, ?, ?, ?, 1, ?, ?) ON CONFLICT(sha1) DO UPDATE SET file_name=excluded.file_name, game_versions_json=excluded.game_versions_json, modified_at=excluded.modified_at")
        .bind(format!("synced-pack:{sha1}"))
        .bind(project_type.get_name()).bind(name).bind(&sha1).bind(bytes.len() as i64)
        .bind(serde_json::to_string(&game_versions)?).bind(now).bind(now).execute(&state.pool).await?;
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
    sqlx::query("UPDATE synced_pack_catalog SET enabled = ?, modified_at = ? WHERE id = ?").bind(i64::from(enabled)).bind(Utc::now().timestamp()).bind(pack_id).execute(&state.pool).await?;
    row.enabled = i64::from(enabled);
    for target in sqlx::query("SELECT instance_id FROM synced_pack_instances WHERE pack_id = ? AND excluded = 0").bind(pack_id).fetch_all(&state.pool).await? { materialize(&state, &row, target.try_get("instance_id")?, false).await?; }
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
        let targets = sqlx::query(
            "SELECT instance_id FROM synced_pack_instances
             WHERE pack_id = ? AND instance_id != ?",
        )
        .bind(pack_id)
        .bind(instance_id)
        .fetch_all(&state.pool)
        .await?;
        for target in targets {
            let target_id: String = target.try_get("instance_id")?;
            if let Some(metadata) =
                crate::state::get_instance(&target_id, &state.pool).await?
            {
                cleanup_materialization(&state, &row, &metadata).await?;
            }
        }
        sqlx::query(
            "DELETE FROM synced_pack_instances WHERE pack_id = ? AND instance_id != ?",
        )
        .bind(pack_id)
        .bind(instance_id)
        .execute(&state.pool)
        .await?;
    }
    Ok(())
}

pub async fn remove_synced_pack(pack_id: &str) -> crate::Result<()> {
    let state = State::get().await?;
    let row = load_row(pack_id, &state).await?;
    let targets = sqlx::query(
        "SELECT instance_id FROM synced_pack_instances WHERE pack_id = ?",
    )
    .bind(pack_id)
    .fetch_all(&state.pool)
    .await?;
    for target in targets {
        let target_id: String = target.try_get("instance_id")?;
        if let Some(metadata) =
            crate::state::get_instance(&target_id, &state.pool).await?
        {
            cleanup_materialization(&state, &row, &metadata).await?;
        }
    }
    sqlx::query("DELETE FROM synced_pack_catalog WHERE id = ?")
        .bind(pack_id)
        .execute(&state.pool)
        .await?;
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
    let rows = sqlx::query_as::<_, PackRow>(
        "SELECT id, project_type, file_name, sha1, size, game_versions_json, enabled
         FROM synced_pack_catalog WHERE project_type = ?",
    )
    .bind(project_type.get_name())
    .fetch_all(&state.pool)
    .await?;
    for row in rows {
        let excluded = sqlx::query_scalar::<_, i64>(
            "SELECT excluded FROM synced_pack_instances
             WHERE pack_id = ? AND instance_id = ?",
        )
        .bind(&row.id)
        .bind(&metadata.instance.id)
        .fetch_optional(&state.pool)
        .await?
        .unwrap_or(0)
            != 0;
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
        let rows = sqlx::query_as::<_, PackRow>(
            "SELECT id, project_type, file_name, sha1, size, game_versions_json, enabled
             FROM synced_pack_catalog WHERE project_type = ?",
        )
        .bind(project_type.get_name())
        .fetch_all(&state.pool)
        .await?;
        for row in rows {
            let excluded = sqlx::query_scalar::<_, i64>(
                "SELECT excluded FROM synced_pack_instances
                 WHERE pack_id = ? AND instance_id = ?",
            )
            .bind(&row.id)
            .bind(&metadata.instance.id)
            .fetch_optional(&state.pool)
            .await?
            .unwrap_or(0)
                != 0;
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
    let rows = sqlx::query_as::<_, PackRow>(
        "SELECT id, project_type, file_name, sha1, size, game_versions_json, enabled
         FROM synced_pack_catalog WHERE project_type = ?",
    )
    .bind(project_type.get_name())
    .fetch_all(&state.pool)
    .await?;
    for row in rows {
        let excluded = sqlx::query_scalar::<_, i64>(
            "SELECT excluded FROM synced_pack_instances WHERE pack_id = ? AND instance_id = ?",
        )
        .bind(&row.id)
        .bind(&metadata.instance.id)
        .fetch_optional(&state.pool)
        .await?
        .unwrap_or(0)
            != 0;
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
    let rows = sqlx::query(
        "SELECT id, file_name, selected FROM synced_pack_catalog
         WHERE project_type = 'resourcepack'",
    )
    .fetch_all(&state.pool)
    .await?;
    let mut managed = BTreeSet::new();
    let mut selected_names = BTreeSet::new();
    let mut transaction = state.pool.begin().await?;
    for (order, row) in rows.iter().enumerate() {
        let id: String = row.try_get("id")?;
        let file_name: String = row.try_get("file_name")?;
        let entry = format!("file/{file_name}");
        let legacy = file_name.clone();
        let is_selected = options
            .entries
            .iter()
            .any(|candidate| candidate == &entry || candidate == &legacy);
        managed.insert(entry.clone());
        managed.insert(legacy);
        sqlx::query(
            "UPDATE synced_pack_catalog
             SET selected = ?, selection_order = ?
             WHERE id = ?",
        )
        .bind(i64::from(is_selected))
        .bind(
            options
                .entries
                .iter()
                .position(|candidate| {
                    candidate == &entry || candidate == &file_name
                })
                .map(|value| value as i64)
                .or(Some(order as i64)),
        )
        .bind(id)
        .execute(&mut *transaction)
        .await?;
        if is_selected {
            selected_names.insert(entry);
        }
    }
    transaction.commit().await?;
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
        let replaced_rows = sqlx::query_as::<_, PackRow>(
            "SELECT id, project_type, file_name, sha1, size, game_versions_json, enabled
             FROM synced_pack_catalog
             WHERE project_type = ? AND file_name = ? AND sha1 != ?",
        )
        .bind(project_type.get_name())
        .bind(file_name)
        .bind(&sha1)
        .fetch_all(&state.pool)
        .await?;
        for replaced in replaced_rows {
            let targets = sqlx::query(
                "SELECT instance_id FROM synced_pack_instances WHERE pack_id = ?",
            )
            .bind(&replaced.id)
            .fetch_all(&state.pool)
            .await?;
            for target in targets {
                let target_id: String = target.try_get("instance_id")?;
                if let Some(target_metadata) =
                    crate::state::get_instance(&target_id, &state.pool).await?
                {
                    cleanup_materialization(state, &replaced, &target_metadata)
                        .await?;
                }
            }
        }
        let mut transaction = state.pool.begin().await?;
        sqlx::query(
            "DELETE FROM synced_pack_catalog
             WHERE project_type = ? AND file_name = ? AND sha1 != ?",
        )
        .bind(project_type.get_name())
        .bind(file_name)
        .bind(&sha1)
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            "INSERT INTO synced_pack_catalog
             (id, project_type, file_name, sha1, size, game_versions_json,
              enabled, created_at, modified_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(sha1) DO UPDATE SET
               file_name=excluded.file_name,
               game_versions_json=excluded.game_versions_json,
               enabled=excluded.enabled,
               modified_at=excluded.modified_at",
        )
        .bind(format!("synced-pack:{sha1}"))
        .bind(project_type.get_name())
        .bind(file_name)
        .bind(&sha1)
        .bind(bytes.len() as i64)
        .bind(&game_versions_json)
        .bind(i64::from(enabled))
        .bind(now)
        .bind(now)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
    }

    // A watcher event can report a directory after a file was removed. Reconcile
    // catalog entries materialized in this source instance so stale files do not
    // remain in other participating instances.
    let tracked = sqlx::query(
        "SELECT c.id, c.project_type, c.file_name, c.sha1, c.size,
                c.game_versions_json, c.enabled, i.materialized_path
         FROM synced_pack_catalog c
         JOIN synced_pack_instances i ON i.pack_id = c.id
         WHERE c.project_type = ? AND i.instance_id = ?
           AND i.materialized_path IS NOT NULL",
    )
    .bind(project_type.get_name())
    .bind(&metadata.instance.id)
    .fetch_all(&state.pool)
    .await?;
    for tracked_row in tracked {
        let row = PackRow {
            id: tracked_row.try_get("id")?,
            project_type: tracked_row.try_get("project_type")?,
            file_name: tracked_row.try_get("file_name")?,
            sha1: tracked_row.try_get("sha1")?,
            size: tracked_row.try_get("size")?,
            game_versions_json: tracked_row.try_get("game_versions_json")?,
            enabled: tracked_row.try_get("enabled")?,
        };
        if seen_sha1.contains(&row.sha1) {
            continue;
        }
        let Some(path) = tracked_row
            .try_get::<Option<String>, _>("materialized_path")?
            .and_then(|path| normalize_tracked_path(metadata, &path))
        else {
            continue;
        };
        if tokio::fs::try_exists(&content_root(state, metadata)?.join(path))
            .await?
        {
            continue;
        }
        let targets = sqlx::query(
            "SELECT instance_id FROM synced_pack_instances WHERE pack_id = ?",
        )
        .bind(&row.id)
        .fetch_all(&state.pool)
        .await?;
        for target in targets {
            let target_id: String = target.try_get("instance_id")?;
            if let Some(target_metadata) =
                crate::state::get_instance(&target_id, &state.pool).await?
            {
                cleanup_materialization(state, &row, &target_metadata).await?;
            }
        }
        sqlx::query("DELETE FROM synced_pack_catalog WHERE id = ?")
            .bind(&row.id)
            .execute(&state.pool)
            .await?;
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

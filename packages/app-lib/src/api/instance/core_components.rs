use crate::state::{
    CoreComponent, CoreComponentKind, CoreComponentSource, CoreJarPreview,
    State,
};
use crate::util::{fetch, io};
use chrono::Utc;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use uuid::Uuid;
use zip::write::SimpleFileOptions;

const CORE_COMPONENTS_DIR: &str = "core-components";
const MANIFEST_FILE: &str = "manifest.json";
const FILES_DIR: &str = "files";
const TRASH_DIR: &str = "trash";
const ASSEMBLED_FILE: &str = "minecraft.jar";

#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CoreComponentManifest {
    components: Vec<CoreComponent>,
}

pub async fn list_core_components(
    instance_id: &str,
) -> crate::Result<Vec<CoreComponent>> {
    let (_, instance_path) = instance_context(instance_id).await?;
    let mut manifest = load_manifest(&instance_path).await?;
    manifest.components.sort_by_key(|component| component.order);
    Ok(manifest.components)
}

pub async fn add_core_jar_mod(
    instance_id: &str,
    source_path: PathBuf,
    target_game_version: String,
    source: Option<CoreComponentSource>,
) -> crate::Result<CoreComponent> {
    add_component(
        instance_id,
        source_path,
        CoreComponentKind::JarMod,
        target_game_version,
        source,
        None,
    )
    .await
}

pub async fn replace_core_jar(
    instance_id: &str,
    source_path: PathBuf,
    target_game_version: String,
    source: Option<CoreComponentSource>,
) -> crate::Result<CoreComponent> {
    add_component(
        instance_id,
        source_path,
        CoreComponentKind::ReplacementJar,
        target_game_version,
        source,
        None,
    )
    .await
}

pub async fn move_core_component(
    instance_id: &str,
    component_id: &str,
    direction: i32,
) -> crate::Result<Vec<CoreComponent>> {
    if direction != -1 && direction != 1 {
        return Err(crate::ErrorKind::InputError(
            "Core component movement must be -1 or 1".to_string(),
        )
        .into());
    }
    let (state, instance_path) = instance_context(instance_id).await?;
    let _lock = state.lock_instance_content(instance_id).await;
    let mut manifest = load_manifest(&instance_path).await?;
    manifest.components.sort_by_key(|component| component.order);
    let active = manifest
        .components
        .iter()
        .enumerate()
        .filter(|(_, component)| !component.removed)
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let current = active
        .iter()
        .position(|index| manifest.components[*index].id == component_id)
        .ok_or_else(|| {
            crate::ErrorKind::InputError(
                "Unknown active core component".to_string(),
            )
        })?;
    let target = current as i32 + direction;
    if target >= 0 && (target as usize) < active.len() {
        let current_index = active[current];
        let target_index = active[target as usize];
        let order = manifest.components[current_index].order;
        manifest.components[current_index].order =
            manifest.components[target_index].order;
        manifest.components[target_index].order = order;
        manifest.components[current_index].modified_at = Utc::now();
        manifest.components[target_index].modified_at = Utc::now();
    }
    save_manifest(&instance_path, &manifest).await?;
    manifest.components.sort_by_key(|component| component.order);
    Ok(manifest.components)
}

pub async fn set_core_component_enabled(
    instance_id: &str,
    component_id: &str,
    enabled: bool,
) -> crate::Result<CoreComponent> {
    let (state, instance_path) = instance_context(instance_id).await?;
    let _lock = state.lock_instance_content(instance_id).await;
    let mut manifest = load_manifest(&instance_path).await?;
    let index = manifest
        .components
        .iter()
        .position(|component| {
            component.id == component_id && !component.removed
        })
        .ok_or_else(|| {
            crate::ErrorKind::InputError(
                "Unknown active core component".to_string(),
            )
        })?;
    if enabled
        && manifest.components[index].kind == CoreComponentKind::ReplacementJar
    {
        for component in &mut manifest.components {
            if component.id != component_id
                && component.kind == CoreComponentKind::ReplacementJar
            {
                component.enabled = false;
            }
        }
    }
    let component = &mut manifest.components[index];
    component.enabled = enabled;
    component.modified_at = Utc::now();
    component.failure_reason = None;
    let result = component.clone();
    save_manifest(&instance_path, &manifest).await?;
    Ok(result)
}

pub async fn remove_core_component(
    instance_id: &str,
    component_id: &str,
) -> crate::Result<()> {
    let (state, instance_path) = instance_context(instance_id).await?;
    let _lock = state.lock_instance_content(instance_id).await;
    let mut manifest = load_manifest(&instance_path).await?;
    let component = manifest
        .components
        .iter_mut()
        .find(|component| component.id == component_id && !component.removed)
        .ok_or_else(|| {
            crate::ErrorKind::InputError(
                "Unknown active core component".to_string(),
            )
        })?;
    let root = component_root(&instance_path);
    let source = io::join_within_root(&root, &component.relative_path)?;
    let trash_relative = format!("{TRASH_DIR}/{}", component.id);
    let trash = io::join_within_root(&root, &trash_relative)?;
    if source.is_file() {
        if let Some(parent) = trash.parent() {
            io::create_dir_all(parent).await?;
        }
        tokio::fs::rename(&source, &trash).await?;
    }
    component.relative_path = trash_relative;
    component.enabled = false;
    component.removed = true;
    component.modified_at = Utc::now();
    save_manifest(&instance_path, &manifest).await
}

pub async fn restore_core_component(
    instance_id: &str,
    component_id: &str,
) -> crate::Result<CoreComponent> {
    let (state, instance_path) = instance_context(instance_id).await?;
    let _lock = state.lock_instance_content(instance_id).await;
    let mut manifest = load_manifest(&instance_path).await?;
    let component = manifest
        .components
        .iter_mut()
        .find(|component| component.id == component_id && component.removed)
        .ok_or_else(|| {
            crate::ErrorKind::InputError(
                "Unknown deleted core component".to_string(),
            )
        })?;
    let root = component_root(&instance_path);
    let source = io::join_within_root(&root, &component.relative_path)?;
    let target_relative = format!("{FILES_DIR}/{}", component.id);
    let target = io::join_within_root(&root, &target_relative)?;
    if !source.is_file() {
        return Err(crate::ErrorKind::InputError(
            "Deleted core component payload no longer exists".to_string(),
        )
        .into());
    }
    if let Some(parent) = target.parent() {
        io::create_dir_all(parent).await?;
    }
    tokio::fs::rename(&source, &target).await?;
    component.relative_path = target_relative;
    component.removed = false;
    component.modified_at = Utc::now();
    component.failure_reason = None;
    let result = component.clone();
    save_manifest(&instance_path, &manifest).await?;
    Ok(result)
}

pub async fn preview_core_jar(
    instance_id: &str,
) -> crate::Result<Option<CoreJarPreview>> {
    let (state, instance_path) = instance_context(instance_id).await?;
    let instance = crate::state::get_instance(instance_id, &state.pool)
        .await?
        .ok_or_else(|| {
            crate::ErrorKind::InputError("Unknown instance".to_string())
        })?;
    let base_jar = state
        .directories
        .version_dir(&instance.applied_content_set.game_version)
        .join(format!("{}.jar", instance.applied_content_set.game_version));
    if !base_jar.is_file() {
        return Err(crate::ErrorKind::InputError(format!(
            "Minecraft {} is not installed yet",
            instance.applied_content_set.game_version
        ))
        .into());
    }
    let Some(output) = assemble_for_launch(
        &instance_path,
        &instance.applied_content_set.game_version,
        &base_jar,
    )
    .await?
    else {
        return Ok(None);
    };
    let manifest = load_manifest(&instance_path).await?;
    let preview = inspect_assembled(&output, &manifest).await?;
    Ok(Some(preview))
}

pub(crate) async fn assemble_for_launch(
    instance_path: &Path,
    game_version: &str,
    base_jar: &Path,
) -> crate::Result<Option<PathBuf>> {
    let manifest = load_manifest(instance_path).await?;
    let mut enabled = manifest
        .components
        .iter()
        .filter(|component| component.enabled && !component.removed)
        .cloned()
        .collect::<Vec<_>>();
    enabled.sort_by_key(|component| component.order);
    if enabled.is_empty() {
        return Ok(None);
    }
    let component_ids = enabled
        .iter()
        .map(|component| component.id.clone())
        .collect::<HashSet<_>>();
    let result: crate::Result<Option<PathBuf>> = async {
        for component in &enabled {
            if component.target_game_version != game_version {
                return Err(crate::ErrorKind::InputError(format!(
                    "Core component {} targets Minecraft {}, not {}",
                    component.file_name,
                    component.target_game_version,
                    game_version
                ))
                .into());
            }
        }
        let output = assembled_path(instance_path);
        let root = component_root(instance_path);
        let components = enabled
            .into_iter()
            .filter(|component| component.kind != CoreComponentKind::Agent)
            .map(|component| {
                let path =
                    io::join_within_root(&root, &component.relative_path)?;
                Ok((component, path))
            })
            .collect::<crate::Result<Vec<_>>>()?;
        for (component, path) in &components {
            verify_component_payload(component, path).await?;
        }
        let output_for_task = output.clone();
        let base_jar = base_jar.to_path_buf();
        tokio::task::spawn_blocking(move || -> crate::Result<()> {
            assemble_archive(&base_jar, &output_for_task, &components)
        })
        .await??;
        Ok(Some(output))
    }
    .await;

    match result {
        Ok(output) => {
            update_component_assembly_state(
                instance_path,
                &component_ids,
                None,
            )
            .await?;
            Ok(output)
        }
        Err(error) => {
            let failure_reason = error.to_string();
            if let Err(save_error) = update_component_assembly_state(
                instance_path,
                &component_ids,
                Some(&failure_reason),
            )
            .await
            {
                tracing::warn!(
                    %save_error,
                    "Could not persist core component assembly failure"
                );
            }
            Err(error)
        }
    }
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum McArchiveCoreInstallResult {
    Installed {
        component: CoreComponent,
    },
    ManualDownload {
        file_name: String,
        page_url: Option<String>,
        expected_sha256: Option<String>,
    },
}

pub async fn install_mcarchive_modloader(
    instance_id: &str,
    game_version: &str,
) -> crate::Result<McArchiveCoreInstallResult> {
    if !legacy_modloader_game_version(game_version) {
        return Err(crate::ErrorKind::InputError(
            "ModLoader core patches are supported only through Minecraft 1.6.2"
                .to_string(),
        )
        .into());
    }
    let modloader = crate::api::mcarchive::get_mod_by_slug("modloader").await?;
    let (version, file) = resolve_modloader_file(&modloader, game_version)?;
    if !is_supported_modloader_archive(file) || file.needs_manual_download() {
        return Ok(McArchiveCoreInstallResult::ManualDownload {
            file_name: file.name.clone(),
            page_url: file
                .manual_download_url()
                .map(ToString::to_string)
                .or(modloader.page_url.clone()),
            expected_sha256: file.sha256.clone(),
        });
    }
    let bytes = crate::api::mcarchive::download_file(file).await?;
    let state = State::get().await?;
    let cache_dir = state.directories.caches_dir().join("mcarchive-core");
    io::create_dir_all(&cache_dir).await?;
    let temporary =
        cache_dir.join(format!("{}-{}", file.uuid, safe_file_name(&file.name)));
    tokio::fs::write(&temporary, &bytes).await?;
    let result = add_modloader_component(
        instance_id,
        temporary.clone(),
        game_version,
        &modloader,
        version,
        file,
    )
    .await;
    let _ = tokio::fs::remove_file(temporary).await;
    result.map(|component| McArchiveCoreInstallResult::Installed { component })
}

pub async fn import_mcarchive_modloader(
    instance_id: &str,
    game_version: &str,
    source_path: PathBuf,
) -> crate::Result<McArchiveCoreInstallResult> {
    if !legacy_modloader_game_version(game_version) {
        return Err(crate::ErrorKind::InputError(
            "ModLoader core patches are supported only through Minecraft 1.6.2"
                .to_string(),
        )
        .into());
    }
    let modloader = crate::api::mcarchive::get_mod_by_slug("modloader").await?;
    let (version, file) = resolve_modloader_file(&modloader, game_version)?;
    let Some(expected_sha256) = file
        .sha256
        .as_deref()
        .filter(|hash| !hash.trim().is_empty())
    else {
        return Ok(McArchiveCoreInstallResult::ManualDownload {
            file_name: file.name.clone(),
            page_url: file
                .manual_download_url()
                .map(ToString::to_string)
                .or(modloader.page_url.clone()),
            expected_sha256: None,
        });
    };
    let metadata = tokio::fs::metadata(&source_path).await?;
    if !metadata.is_file() {
        return Err(crate::ErrorKind::InputError(
            "The selected ModLoader import must be a regular file".to_string(),
        )
        .into());
    }
    let source_path = io::canonicalize(&source_path)?;
    let actual_sha256 = sha256_file(&source_path).await?;
    if !actual_sha256.eq_ignore_ascii_case(expected_sha256) {
        return Err(crate::ErrorKind::InputError(format!(
            "The selected file does not match MCArchive SHA-256 for {}",
            file.name
        ))
        .into());
    }
    add_modloader_component(
        instance_id,
        source_path,
        game_version,
        &modloader,
        version,
        file,
    )
    .await
    .map(|component| McArchiveCoreInstallResult::Installed { component })
}

fn resolve_modloader_file<'a>(
    modloader: &'a crate::mcarchive::McArchiveMod,
    game_version: &str,
) -> crate::Result<(
    &'a crate::mcarchive::McArchiveModVersion,
    &'a crate::mcarchive::McArchiveFile,
)> {
    let version = modloader
		.mod_versions
		.iter()
		.find(|version| {
			version.name == game_version
				|| version
					.game_versions
					.iter()
					.any(|candidate| candidate.name == game_version)
		})
		.ok_or_else(|| {
			crate::ErrorKind::InputError(format!(
				"MCArchive has no ModLoader release for Minecraft {game_version}"
			))
		})?;
    let file = version
        .files
        .iter()
        .find(|file| {
            is_supported_modloader_archive(file)
                && file.is_automatically_installable()
        })
        .or_else(|| {
            version
                .files
                .iter()
                .find(|file| is_supported_modloader_archive(file))
        })
        .ok_or_else(|| {
            crate::ErrorKind::InputError(format!(
                "MCArchive ModLoader {} has no supported archive",
                version.name
            ))
        })?;
    Ok((version, file))
}

async fn add_modloader_component(
    instance_id: &str,
    source_path: PathBuf,
    game_version: &str,
    modloader: &crate::mcarchive::McArchiveMod,
    version: &crate::mcarchive::McArchiveModVersion,
    file: &crate::mcarchive::McArchiveFile,
) -> crate::Result<CoreComponent> {
    let source = CoreComponentSource {
        provider: "mcarchive".to_string(),
        project_id: Some(modloader.uuid.clone()),
        version_id: Some(version.uuid.clone()),
        file_id: Some(file.uuid.clone()),
        page_url: file
            .manual_download_url()
            .map(ToString::to_string)
            .or(modloader.page_url.clone()),
    };
    add_component(
        instance_id,
        source_path,
        CoreComponentKind::JarMod,
        game_version.to_string(),
        Some(source),
        Some(file.name.clone()),
    )
    .await
}

async fn add_component(
    instance_id: &str,
    source_path: PathBuf,
    kind: CoreComponentKind,
    target_game_version: String,
    source: Option<CoreComponentSource>,
    display_file_name: Option<String>,
) -> crate::Result<CoreComponent> {
    let (state, instance_path) = instance_context(instance_id).await?;
    if target_game_version.is_empty() {
        return Err(crate::ErrorKind::InputError(
            "Core component target Minecraft version is required".to_string(),
        )
        .into());
    }
    let instance = crate::state::get_instance(instance_id, &state.pool)
        .await?
        .ok_or_else(|| {
            crate::ErrorKind::InputError("Unknown instance".to_string())
        })?;
    if instance.applied_content_set.game_version != target_game_version {
        return Err(crate::ErrorKind::InputError(format!(
			"Core component targets Minecraft {target_game_version}, but this instance uses {}",
			instance.applied_content_set.game_version
		))
		.into());
    }
    let metadata = tokio::fs::metadata(&source_path).await?;
    if !metadata.is_file() {
        return Err(crate::ErrorKind::InputError(
            "Core component source must be a regular file".to_string(),
        )
        .into());
    }
    let source_path = io::canonicalize(&source_path)?;
    validate_archive(&source_path).await?;
    let (size, sha1) = fetch::sha1_file_async(&source_path).await?;
    let sha256 = sha256_file(&source_path).await?;
    if size == 0 {
        return Err(crate::ErrorKind::InputError(
            "Core component archive is empty".to_string(),
        )
        .into());
    }
    let _lock = state.lock_instance_content(instance_id).await;
    let mut manifest = load_manifest(&instance_path).await?;
    let id = Uuid::new_v4().to_string();
    let root = component_root(&instance_path);
    let relative_path = format!("{FILES_DIR}/{id}");
    let destination = io::join_within_root(&root, &relative_path)?;
    if let Some(parent) = destination.parent() {
        io::create_dir_all(parent).await?;
    }
    fetch::copy(&source_path, &destination, &state.io_semaphore).await?;
    if kind == CoreComponentKind::ReplacementJar {
        for existing in &mut manifest.components {
            if existing.kind == CoreComponentKind::ReplacementJar
                && !existing.removed
            {
                existing.enabled = false;
            }
        }
    }
    let now = Utc::now();
    let file_name = display_file_name.unwrap_or_else(|| {
        source_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("component.jar")
            .to_string()
    });
    if file_name.trim().is_empty() {
        return Err(crate::ErrorKind::InputError(
            "Core component filename is required".to_string(),
        )
        .into());
    }
    let component = CoreComponent {
        id,
        kind,
        file_name,
        relative_path,
        enabled: true,
        removed: false,
        order: manifest
            .components
            .iter()
            .map(|component| component.order)
            .max()
            .unwrap_or(-1)
            .saturating_add(1),
        sha1: Some(sha1),
        sha256: Some(sha256),
        source,
        target_game_version,
        created_at: now,
        modified_at: now,
        failure_reason: None,
    };
    manifest.components.push(component.clone());
    save_manifest(&instance_path, &manifest).await?;
    Ok(component)
}

async fn instance_context(
    instance_id: &str,
) -> crate::Result<(std::sync::Arc<State>, PathBuf)> {
    let state = State::get().await?;
    let instance = crate::state::get_instance(instance_id, &state.pool)
        .await?
        .ok_or_else(|| {
            crate::ErrorKind::InputError("Unknown instance".to_string())
        })?;
    let path = io::canonicalize(
        state
            .directories
            .instances_dir()
            .join(instance.instance.path),
    )?;
    Ok((state, path))
}

fn component_root(instance_path: &Path) -> PathBuf {
    instance_path.join(CORE_COMPONENTS_DIR)
}

fn manifest_path(instance_path: &Path) -> PathBuf {
    component_root(instance_path).join(MANIFEST_FILE)
}

fn assembled_path(instance_path: &Path) -> PathBuf {
    component_root(instance_path)
        .join("assembled")
        .join(ASSEMBLED_FILE)
}

async fn load_manifest(
    instance_path: &Path,
) -> crate::Result<CoreComponentManifest> {
    let path = manifest_path(instance_path);
    match tokio::fs::read(path).await {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(CoreComponentManifest::default())
        }
        Err(error) => Err(error.into()),
    }
}

async fn save_manifest(
    instance_path: &Path,
    manifest: &CoreComponentManifest,
) -> crate::Result<()> {
    let path = manifest_path(instance_path);
    let bytes = serde_json::to_vec_pretty(manifest)?;
    if let Some(parent) = path.parent() {
        io::create_dir_all(parent).await?;
    }
    let temporary = path.with_extension(format!("json.{}.tmp", Uuid::new_v4()));
    tokio::fs::write(&temporary, bytes).await?;
    let result = atomically_replace_file(&temporary, &path);
    if result.is_err() {
        let _ = tokio::fs::remove_file(&temporary).await;
    }
    result.map_err(Into::into)
}

async fn validate_archive(path: &Path) -> crate::Result<()> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || -> crate::Result<()> {
        let file = std::fs::File::open(&path).map_err(|error| {
            crate::util::io::IOError::with_path(error, &path)
        })?;
        let mut archive = zip::ZipArchive::new(file).map_err(|error| {
            crate::ErrorKind::InputError(format!(
                "Core component is not a valid ZIP/JAR archive: {error}"
            ))
        })?;
        for index in 0..archive.len() {
            let entry = archive.by_index(index).map_err(|error| {
                crate::ErrorKind::InputError(format!(
                    "Core component ZIP entry cannot be read: {error}"
                ))
            })?;
            validate_archive_entry(entry.name())?;
        }
        Ok(())
    })
    .await??;
    Ok(())
}

async fn sha256_file(path: &Path) -> crate::Result<String> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let mut file = std::fs::File::open(&path).map_err(|error| {
            crate::util::io::IOError::with_path(error, &path)
        })?;
        let mut hasher = Sha256::new();
        let mut buffer = [0_u8; 262_144];
        loop {
            let read = file.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
        Ok(format!("{:x}", hasher.finalize()))
    })
    .await?
}

fn assemble_archive(
    base_jar: &Path,
    output: &Path,
    components: &[(CoreComponent, PathBuf)],
) -> crate::Result<()> {
    let replacement = components
        .iter()
        .find(|(component, _)| {
            component.kind == CoreComponentKind::ReplacementJar
        })
        .map(|(_, path)| path.as_path())
        .unwrap_or(base_jar);
    let mut inputs = vec![replacement.to_path_buf()];
    inputs.extend(
        components
            .iter()
            .filter(|(component, _)| {
                component.kind == CoreComponentKind::JarMod
            })
            .map(|(_, path)| path.clone()),
    );
    let temporary =
        output.with_extension(format!("jar.{}.tmp", Uuid::new_v4()));
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let result = (|| -> crate::Result<()> {
        let file = std::fs::File::create(&temporary)?;
        let mut writer = zip::ZipWriter::new(file);
        let mut written = HashSet::new();
        for input in inputs.into_iter().rev() {
            let file = std::fs::File::open(&input).map_err(|error| {
                crate::util::io::IOError::with_path(error, &input)
            })?;
            let mut archive = zip::ZipArchive::new(file).map_err(|error| {
                crate::ErrorKind::InputError(format!(
                    "Cannot open core component archive {}: {error}",
                    input.display()
                ))
            })?;
            for index in 0..archive.len() {
                let mut entry = archive.by_index(index).map_err(|error| {
                    crate::ErrorKind::InputError(format!(
                        "Cannot read core component ZIP entry: {error}"
                    ))
                })?;
                let name = validate_archive_entry(entry.name())?;
                let Some(name) = name else {
                    continue;
                };
                if !written.insert(name.clone()) {
                    continue;
                }
                writer
                    .start_file(name, SimpleFileOptions::default())
                    .map_err(|error| {
                        crate::ErrorKind::OtherError(format!(
                            "Cannot write assembled core JAR entry: {error}"
                        ))
                    })?;
                std::io::copy(&mut entry, &mut writer)?;
            }
        }
        writer.finish().map_err(|error| {
            crate::ErrorKind::OtherError(format!(
                "Cannot finalize assembled core JAR: {error}"
            ))
        })?;
        Ok(())
    })();
    if let Err(error) = result {
        let _ = std::fs::remove_file(&temporary);
        return Err(error);
    }
    if let Err(error) = atomically_replace_file(&temporary, output) {
        let _ = std::fs::remove_file(&temporary);
        return Err(error.into());
    }
    Ok(())
}

fn atomically_replace_file(
    temporary: &Path,
    destination: &Path,
) -> std::io::Result<()> {
    io::atomically_replace_file(temporary, destination)
}

async fn update_component_assembly_state(
    instance_path: &Path,
    component_ids: &HashSet<String>,
    failure_reason: Option<&str>,
) -> crate::Result<()> {
    let mut manifest = load_manifest(instance_path).await?;
    let now = Utc::now();
    let mut changed = false;
    for component in &mut manifest.components {
        if component_ids.contains(&component.id)
            && component.failure_reason.as_deref() != failure_reason
        {
            component.failure_reason = failure_reason.map(ToString::to_string);
            component.modified_at = now;
            changed = true;
        }
    }
    if changed {
        save_manifest(instance_path, &manifest).await?;
    }
    Ok(())
}

fn validate_archive_entry(name: &str) -> crate::Result<Option<String>> {
    if name.is_empty() || name.contains('\0') {
        return Err(crate::ErrorKind::InputError(
            "Core component contains an invalid ZIP entry name".to_string(),
        )
        .into());
    }
    let normalized = name.replace('\\', "/");
    let path = Path::new(&normalized);
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
        return Err(crate::ErrorKind::InputError(format!(
            "Core component ZIP entry escapes the archive: {name}"
        ))
        .into());
    }
    if normalized.ends_with('/') {
        return Ok(None);
    }
    if normalized.to_ascii_uppercase().starts_with("META-INF/") {
        return Ok(None);
    }
    Ok(Some(normalized))
}

async fn verify_component_payload(
    component: &CoreComponent,
    path: &Path,
) -> crate::Result<()> {
    if !path.is_file() {
        return Err(crate::ErrorKind::InputError(format!(
            "Core component {} is missing from instance storage",
            component.file_name
        ))
        .into());
    }
    if let Some(expected) = component.sha256.as_deref() {
        let actual = sha256_file(path).await?;
        if !actual.eq_ignore_ascii_case(expected) {
            return Err(crate::ErrorKind::InputError(format!(
                "Core component {} no longer matches its SHA-256",
                component.file_name
            ))
            .into());
        }
    }
    Ok(())
}

fn legacy_modloader_game_version(game_version: &str) -> bool {
    let numeric = game_version.trim().trim_start_matches('v');
    if numeric.starts_with('a')
        || numeric.starts_with('b')
        || numeric.starts_with("inf-")
    {
        return true;
    }
    let parts = numeric
        .split('.')
        .map(str::parse::<u16>)
        .collect::<Result<Vec<_>, _>>();
    let Ok(parts) = parts else {
        return false;
    };
    match parts.as_slice() {
        [1, minor] => *minor <= 6,
        [1, minor, _] if *minor < 6 => true,
        [1, 6, patch] => *patch <= 2,
        _ => false,
    }
}

fn safe_file_name(name: &str) -> String {
    let value = Path::new(name)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("component.zip");
    let sanitized = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric()
                || matches!(character, '.' | '-' | '_')
            {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    if sanitized.is_empty() {
        "component.zip".to_string()
    } else {
        sanitized
    }
}

fn is_supported_modloader_archive(
    file: &crate::mcarchive::McArchiveFile,
) -> bool {
    let name = file.name.to_ascii_lowercase();
    (name.ends_with(".jar") || name.ends_with(".zip"))
        && !name.contains("javadoc")
        && !name.contains("sources")
}

async fn inspect_assembled(
    path: &Path,
    manifest: &CoreComponentManifest,
) -> crate::Result<CoreJarPreview> {
    if !path.is_file() {
        return Err(crate::ErrorKind::InputError(
            "No assembled core JAR exists yet. Start the instance once first."
                .to_string(),
        )
        .into());
    }
    let entries = {
        let path = path.to_path_buf();
        tokio::task::spawn_blocking(move || {
            let file = std::fs::File::open(&path)?;
            let archive = zip::ZipArchive::new(file).map_err(|error| {
                crate::ErrorKind::InputError(format!(
                    "Cannot inspect assembled core JAR: {error}"
                ))
            })?;
            Ok::<_, crate::Error>(archive.len())
        })
        .await??
    };
    let (_, sha1) = fetch::sha1_file_async(path).await?;
    let sha256 = sha256_file(path).await?;
    Ok(CoreJarPreview {
        output_path: path.to_string_lossy().to_string(),
        component_count: manifest
            .components
            .iter()
            .filter(|component| component.enabled && !component.removed)
            .count(),
        replacement_component_id: manifest
            .components
            .iter()
            .find(|component| {
                component.enabled
                    && !component.removed
                    && component.kind == CoreComponentKind::ReplacementJar
            })
            .map(|component| component.id.clone()),
        entries,
        sha1,
        sha256,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_archive(path: &Path, entries: &[(&str, &[u8])]) {
        let file = std::fs::File::create(path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        for (name, contents) in entries {
            writer
                .start_file(*name, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(contents).unwrap();
        }
        writer.finish().unwrap();
    }

    #[test]
    fn later_component_wins_and_meta_inf_is_filtered() {
        let directory = tempfile::tempdir().unwrap();
        let base = directory.path().join("base.jar");
        let first = directory.path().join("first.jar");
        let second = directory.path().join("second.jar");
        let output = directory.path().join("output.jar");
        write_archive(
            &base,
            &[("value.txt", b"base"), ("META-INF/OLD.SF", b"sig")],
        );
        write_archive(&first, &[("value.txt", b"first"), ("one.txt", b"one")]);
        write_archive(
            &second,
            &[("value.txt", b"second"), ("two.txt", b"two")],
        );
        let component = |id: &str| CoreComponent {
            id: id.to_string(),
            kind: CoreComponentKind::JarMod,
            file_name: format!("{id}.jar"),
            relative_path: String::new(),
            enabled: true,
            removed: false,
            order: 0,
            sha1: None,
            sha256: None,
            source: None,
            target_game_version: "b1.7.3".to_string(),
            created_at: Utc::now(),
            modified_at: Utc::now(),
            failure_reason: None,
        };
        assemble_archive(
            &base,
            &output,
            &[(component("first"), first), (component("second"), second)],
        )
        .unwrap();
        let file = std::fs::File::open(output).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let mut value = String::new();
        archive
            .by_name("value.txt")
            .unwrap()
            .read_to_string(&mut value)
            .unwrap();
        assert_eq!(value, "second");
        assert!(archive.by_name("META-INF/OLD.SF").is_err());
    }

    #[test]
    fn rejects_path_traversal_entries() {
        assert!(validate_archive_entry("../outside.class").is_err());
        assert!(validate_archive_entry("/outside.class").is_err());
        assert!(validate_archive_entry("../outside/").is_err());
    }

    #[test]
    fn modloader_version_gate_excludes_modern_versions() {
        assert!(legacy_modloader_game_version("1.6.2"));
        assert!(legacy_modloader_game_version("1.5.2"));
        assert!(legacy_modloader_game_version("b1.7.3"));
        assert!(!legacy_modloader_game_version("1.6.3"));
        assert!(!legacy_modloader_game_version("1.7.10"));
    }
}

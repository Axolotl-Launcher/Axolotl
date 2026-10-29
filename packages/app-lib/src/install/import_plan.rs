use std::{
    path::{Path, PathBuf},
    sync::LazyLock,
};

use daedalus::minecraft::{
    AssetsIndex, DownloadType, Library, LoggingConfiguration, LoggingSide,
    VersionInfo as GameVersionInfo,
};
use dashmap::DashMap;
use reqwest::Method;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::instance::QuickPlayType;
use crate::launcher::download::{
    ArtifactAvailability, LocalRuntimeSource, classify_local_artifact,
    is_native_library, legacy_library_sha1, library_native_classifier,
    local_asset_index_path, local_asset_object_path, local_client_path,
    local_library_path, local_log_config_path, local_native_library_path,
    native_library_artifact_path, needs_java_artifact,
};
use crate::launcher::parse_rules;
use crate::state::{ModLoader, State};

#[path = "../api/pack/import/instance_json.rs"]
mod instance_json;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPlanRequest {
    pub request_id: String,
    pub launcher_type: crate::api::pack::import::ImportLauncherType,
    pub base_path: PathBuf,
    pub instance_folder: String,
    #[serde(default)]
    pub instance_path: Option<String>,
    #[serde(default)]
    pub game_version: Option<String>,
    #[serde(default)]
    pub loader: Option<crate::state::ModLoader>,
    #[serde(default)]
    pub loader_version: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPlanCounts {
    pub files: u64,
    pub bytes: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ImportPlanStage {
    Resolving,
    Scanning,
    Done,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPlanSnapshot {
    pub request_id: String,
    pub stage: ImportPlanStage,
    pub game_version: Option<String>,
    pub loader: Option<String>,
    pub loader_version: Option<String>,
    pub import_path: String,
    pub minecraft_root: String,
    pub mod_count: u64,
    pub cache: ImportPlanCounts,
    pub local: ImportPlanCounts,
    pub network: ImportPlanCounts,
    pub migrate: ImportPlanCounts,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
struct AxolotlConfigMetadata {
    schema_version: Option<u32>,
    #[serde(default)]
    content_set: AxolotlContentSetMetadata,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
struct AxolotlContentSetMetadata {
    #[serde(default)]
    game_version: Option<String>,
    #[serde(default)]
    loader: Option<String>,
    #[serde(default)]
    loader_version: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct DetectedInstanceInfo {
    vanilla_name: Option<String>,
    loader: Option<String>,
    loader_version: Option<String>,
}

static IMPORT_PLAN_CANCELLATIONS: LazyLock<DashMap<String, CancellationToken>> =
    LazyLock::new(DashMap::new);

pub fn start_import_plan(request: ImportPlanRequest) -> crate::Result<String> {
    let request_id = request.request_id.clone();
    if request_id.trim().is_empty() {
        return Err(crate::ErrorKind::InputError(
            "Import plan request id must not be empty".to_string(),
        )
        .into());
    }

    let cancellation = CancellationToken::new();
    IMPORT_PLAN_CANCELLATIONS.insert(request_id.clone(), cancellation.clone());
    tokio::spawn(async move {
        let result = run_import_plan(&request, &cancellation).await;
        IMPORT_PLAN_CANCELLATIONS.remove(&request.request_id);
        if let Err(error) = result {
            tracing::error!(
                request_id = %request.request_id,
                "Import plan failed: {error}"
            );
        }
    });

    Ok(request_id)
}

pub async fn cancel_import_plan(request_id: &str) -> crate::Result<()> {
    if let Some((_, cancellation)) =
        IMPORT_PLAN_CANCELLATIONS.remove(request_id)
    {
        cancellation.cancel();
    }
    Ok(())
}

async fn run_import_plan(
    request: &ImportPlanRequest,
    cancellation: &CancellationToken,
) -> crate::Result<()> {
    if cancellation.is_cancelled() {
        return Ok(());
    }

    let source = resolve_source(request)?;
    let import_path = source.to_string_lossy().to_string();
    let dotminecraft = if source.join(".minecraft").is_dir() {
        source.join(".minecraft")
    } else {
        source.clone()
    };
    let local_source = LocalRuntimeSource::discover(&dotminecraft);
    let minecraft_root = local_source
        .as_ref()
        .map(|source| source.root.to_string_lossy().to_string())
        .unwrap_or_default();

    let detected = detect_import_plan_info(&source, &dotminecraft);
    let game_version = request
        .game_version
        .clone()
        .filter(|version| !version.trim().is_empty())
        .or_else(|| detected.vanilla_name.clone());
    let mut loader = request
        .loader
        .map(|loader| loader.as_str().to_string())
        .or_else(|| detected.loader.clone());
    let mut loader_version = request
        .loader_version
        .clone()
        .or_else(|| detected.loader_version.clone());
    if let Some(loader_name) = loader.as_deref() {
        let supported = matches!(
            loader_name,
            "forge"
                | "neoforge"
                | "fabric"
                | "quilt"
                | "optifine"
                | "cleanroom"
                | "lite_loader"
                | "legacy_fabric"
        );
        if !supported {
            loader = None;
            loader_version = None;
        }
    }

    emit_import_plan(&snapshot_for(
        request,
        ImportPlanStage::Resolving,
        &import_path,
        &minecraft_root,
        None,
        None,
        None,
        0,
        ImportPlanCountsBySource::default(),
        ImportPlanCounts::default(),
        None,
    ))
    .await?;
    if cancellation.is_cancelled() {
        return Ok(());
    }

    emit_import_plan(&snapshot_for(
        request,
        ImportPlanStage::Scanning,
        &import_path,
        &minecraft_root,
        game_version.clone(),
        loader.clone(),
        loader_version.clone(),
        0,
        ImportPlanCountsBySource::default(),
        ImportPlanCounts::default(),
        None,
    ))
    .await?;
    if cancellation.is_cancelled() {
        return Ok(());
    }

    let result = match scan_import_plan(
        request,
        &source,
        &dotminecraft,
        local_source.as_ref(),
        &detected,
        cancellation,
    )
    .await
    {
        Ok(result) => result,
        Err(error) => {
            if cancellation.is_cancelled() {
                return Ok(());
            }
            let snapshot = snapshot_for(
                request,
                ImportPlanStage::Error,
                &import_path,
                &minecraft_root,
                None,
                None,
                None,
                0,
                ImportPlanCountsBySource::default(),
                ImportPlanCounts::default(),
                Some(error.to_string()),
            );
            emit_import_plan(&snapshot).await?;
            return Err(error);
        }
    };

    if cancellation.is_cancelled() {
        return Ok(());
    }

    for counts in &result.category_progress {
        emit_import_plan(&snapshot_for(
            request,
            ImportPlanStage::Scanning,
            &import_path,
            &minecraft_root,
            game_version.clone(),
            loader.clone(),
            loader_version.clone(),
            result.mod_count,
            *counts,
            result.migrate,
            None,
        ))
        .await?;
        if cancellation.is_cancelled() {
            return Ok(());
        }
    }

    emit_import_plan(&snapshot_for(
        request,
        ImportPlanStage::Done,
        &import_path,
        &minecraft_root,
        game_version,
        loader,
        loader_version,
        result.mod_count,
        result.counts,
        result.migrate,
        None,
    ))
    .await?;

    Ok(())
}

#[derive(Debug, Clone, Copy, Default)]
struct ImportPlanCountsBySource {
    cache: ImportPlanCounts,
    local: ImportPlanCounts,
    network: ImportPlanCounts,
}

impl std::ops::AddAssign for ImportPlanCountsBySource {
    fn add_assign(&mut self, rhs: Self) {
        self.cache.files += rhs.cache.files;
        self.cache.bytes += rhs.cache.bytes;
        self.local.files += rhs.local.files;
        self.local.bytes += rhs.local.bytes;
        self.network.files += rhs.network.files;
        self.network.bytes += rhs.network.bytes;
    }
}

#[derive(Debug, Clone, Default)]
struct ImportPlanScanResult {
    mod_count: u64,
    migrate: ImportPlanCounts,
    counts: ImportPlanCountsBySource,
    category_progress: Vec<ImportPlanCountsBySource>,
}

#[derive(Debug, Clone)]
struct RequiredArtifact {
    relative_path: PathBuf,
    destination: PathBuf,
    expected_sha1: Option<String>,
    expected_size: Option<u64>,
}

fn library_classifier_coordinate_is_native(library: &Library) -> bool {
    library
        .name
        .split(':')
        .nth(3)
        .and_then(|classifier| classifier.split('@').next())
        .is_some_and(|classifier| classifier.starts_with("natives-"))
}

fn snapshot_for(
    request: &ImportPlanRequest,
    stage: ImportPlanStage,
    import_path: &str,
    minecraft_root: &str,
    game_version: Option<String>,
    loader: Option<String>,
    loader_version: Option<String>,
    mod_count: u64,
    counts: ImportPlanCountsBySource,
    migrate: ImportPlanCounts,
    error: Option<String>,
) -> ImportPlanSnapshot {
    ImportPlanSnapshot {
        request_id: request.request_id.clone(),
        stage,
        game_version,
        loader,
        loader_version,
        import_path: import_path.to_string(),
        minecraft_root: minecraft_root.to_string(),
        mod_count,
        cache: counts.cache,
        local: counts.local,
        network: counts.network,
        migrate,
        error,
    }
}

fn resolve_source(request: &ImportPlanRequest) -> crate::Result<PathBuf> {
    let Some(instance_path) = &request.instance_path else {
        return Err(crate::ErrorKind::InputError(
            "Import source path is required".to_string(),
        )
        .into());
    };
    let source = PathBuf::from(instance_path);
    if !source.is_dir() {
        return Err(crate::ErrorKind::InputError(format!(
            "Import source path does not exist: {}",
            source.display()
        ))
        .into());
    }
    Ok(source)
}

fn detect_import_plan_info(
    source: &Path,
    dotminecraft: &Path,
) -> DetectedInstanceInfo {
    let json_info = instance_json::detect(dotminecraft);
    if let Some(info) = &json_info {
        tracing::debug!(
            adjunct_count = info.adjuncts.len(),
            "Detected imported loader adjuncts"
        );
    }
    let config_info = read_axolotl_config(source);
    DetectedInstanceInfo {
        vanilla_name: config_info
            .as_ref()
            .and_then(|config| {
                config
                    .content_set
                    .game_version
                    .clone()
                    .filter(|version| !version.trim().is_empty())
            })
            .or_else(|| {
                json_info.as_ref().map(|info| info.vanilla_name.clone())
            }),
        loader: config_info
            .as_ref()
            .and_then(|config| {
                config
                    .content_set
                    .loader
                    .clone()
                    .filter(|loader| !loader.trim().is_empty())
            })
            .or_else(|| {
                json_info.as_ref().and_then(|info| info.loader.clone())
            }),
        loader_version: config_info
            .as_ref()
            .and_then(|config| {
                config
                    .content_set
                    .loader_version
                    .clone()
                    .filter(|version| !version.trim().is_empty())
            })
            .or_else(|| {
                json_info
                    .as_ref()
                    .and_then(|info| info.loader_version.clone())
            }),
    }
}

fn read_axolotl_config(source: &Path) -> Option<AxolotlConfigMetadata> {
    let path = source.join("axolotl_config.json");
    if !path.is_file() {
        return None;
    }
    let content = std::fs::read_to_string(&path).ok()?;
    let config: AxolotlConfigMetadata = serde_json::from_str(&content).ok()?;
    (config.schema_version == Some(1)).then_some(config)
}

async fn scan_import_plan(
    request: &ImportPlanRequest,
    source: &Path,
    dotminecraft: &Path,
    local_source: Option<&LocalRuntimeSource>,
    detected: &DetectedInstanceInfo,
    cancellation: &CancellationToken,
) -> crate::Result<ImportPlanScanResult> {
    let import_path = source.to_string_lossy().to_string();
    let minecraft_root = local_source
        .map(|source| source.root.to_string_lossy().to_string())
        .unwrap_or_default();
    let state = State::get().await?;
    let game_version = request
        .game_version
        .clone()
        .filter(|version| !version.trim().is_empty())
        .or_else(|| detected.vanilla_name.clone())
        .ok_or_else(|| {
            crate::ErrorKind::InputError(
                "Could not detect a Minecraft version; provide one to continue"
                    .to_string(),
            )
        })?;
    let detected_loader =
        detected.loader.as_deref().map(str::to_ascii_lowercase);
    if detected_loader.as_deref().is_some_and(|loader| {
        loader == "labymod" || loader.starts_with("labymod-")
    }) {
        return Err(crate::ErrorKind::InputError(
            "Unsupported loader LabyMod: Axolotl does not install, update, or repair LabyMod instances"
                .to_string(),
        )
        .into());
    }
    let loader = request
        .loader
        .or_else(|| {
            detected_loader
                .as_deref()
                .and_then(|loader_name| match loader_name {
                    "forge" => Some(ModLoader::Forge),
                    "neoforge" | "neo_forge" => Some(ModLoader::NeoForge),
                    "fabric" => Some(ModLoader::Fabric),
                    "quilt" => Some(ModLoader::Quilt),
                    "optifine" => Some(ModLoader::OptiFine),
                    "cleanroom" => Some(ModLoader::Cleanroom),
                    "lite_loader" | "liteloader" => Some(ModLoader::LiteLoader),
                    "legacy_fabric" | "legacyfabric" => {
                        Some(ModLoader::LegacyFabric)
                    }
                    _ => None,
                })
        })
        .unwrap_or(ModLoader::Vanilla);
    let loader_version = request
        .loader_version
        .clone()
        .or_else(|| detected.loader_version.clone());
    if let Some(detected_loader) = detected_loader
        && loader == ModLoader::Vanilla
        && detected_loader != "vanilla"
    {
        return Err(crate::ErrorKind::InputError(format!(
            "Unsupported loader {detected_loader}: the instance was not planned as Vanilla"
        ))
        .into());
    }
    let requested_loader_version = loader_version
        .as_deref()
        .filter(|version| !version.is_empty() && *version != "latest");
    let loader_name = loader.as_str().to_string();
    let game_version_option = Some(game_version.clone());
    let loader_option = Some(loader_name);
    let (mod_count, migrate) = scan_counts_emitting(
        request,
        dotminecraft,
        &import_path,
        &minecraft_root,
        &game_version_option,
        &loader_option,
        &loader_version,
        cancellation,
    )
    .await?;
    if cancellation.is_cancelled() {
        return Ok(ImportPlanScanResult::default());
    }

    let categories = build_runtime_artifact_categories(
        &state,
        local_source,
        &game_version,
        loader,
        requested_loader_version,
    )
    .await?;
    if cancellation.is_cancelled() {
        return Ok(ImportPlanScanResult::default());
    }

    let mut counts = ImportPlanCountsBySource::default();
    let mut category_progress = Vec::with_capacity(categories.len());
    for artifacts in categories {
        if cancellation.is_cancelled() {
            break;
        }
        counts +=
            classify_artifacts(local_source, &artifacts, cancellation).await?;
        category_progress.push(counts);
    }

    Ok(ImportPlanScanResult {
        mod_count,
        migrate,
        counts,
        category_progress,
    })
}

async fn build_runtime_artifact_categories(
    state: &State,
    local_source: Option<&LocalRuntimeSource>,
    game_version: &str,
    loader: ModLoader,
    requested_loader_version: Option<&str>,
) -> crate::Result<Vec<Vec<RequiredArtifact>>> {
    let (manifest, version_index) =
        crate::launcher::resolve_minecraft_manifest(game_version, state)
            .await?;
    let version = &manifest.versions[version_index];
    let minecraft_updated = version_index
        <= manifest
            .versions
            .iter()
            .position(|entry| entry.id == "22w16a")
            .unwrap_or(0);
    let loader_version = if loader == ModLoader::Vanilla {
        None
    } else {
        crate::launcher::get_loader_version_from_profile(
            game_version,
            loader,
            requested_loader_version,
        )
        .await?
    };
    let version_info = crate::launcher::download::download_version_info(
        state,
        version,
        loader,
        loader_version.as_ref(),
        None,
        None,
        None,
    )
    .await?;
    let java_arch = std::env::consts::ARCH;

    let mut categories = Vec::with_capacity(6);
    categories.push(client_artifacts(state, game_version, &version_info)?);
    categories.push(asset_index_artifacts(state, &version_info)?);
    categories.push(log_config_artifacts(state, &version_info)?);
    let (libraries, natives) =
        library_artifacts(state, &version_info, java_arch, minecraft_updated)?;
    categories.push(libraries);
    categories.push(natives);
    categories.push(
        asset_object_artifacts(state, local_source, &version_info).await?,
    );
    Ok(categories)
}

fn client_artifacts(
    state: &State,
    game_version: &str,
    version_info: &GameVersionInfo,
) -> crate::Result<Vec<RequiredArtifact>> {
    let client = version_info
        .downloads
        .get(&DownloadType::Client)
        .ok_or_else(|| {
            crate::ErrorKind::LauncherError(format!(
                "No client downloads exist for version {}",
                version_info.id
            ))
        })?;
    let version_id = &version_info.id;
    Ok(vec![RequiredArtifact {
        relative_path: local_client_path(game_version),
        destination: state
            .directories
            .version_dir(version_id)
            .join(format!("{version_id}.jar")),
        expected_sha1: Some(client.sha1.clone()),
        expected_size: Some(client.size as u64),
    }])
}

fn asset_index_artifacts(
    state: &State,
    version_info: &GameVersionInfo,
) -> crate::Result<Vec<RequiredArtifact>> {
    let index = &version_info.asset_index;
    Ok(vec![RequiredArtifact {
        relative_path: local_asset_index_path(&index.id),
        destination: state
            .directories
            .assets_index_dir()
            .join(format!("{}.json", index.id)),
        expected_sha1: Some(index.sha1.clone()),
        expected_size: Some(index.size as u64),
    }])
}

fn log_config_artifacts(
    state: &State,
    version_info: &GameVersionInfo,
) -> crate::Result<Vec<RequiredArtifact>> {
    let Some(logging) = version_info
        .logging
        .as_ref()
        .and_then(|logging| logging.get(&LoggingSide::Client))
    else {
        return Ok(Vec::new());
    };
    let LoggingConfiguration::Log4j2Xml { file, .. } = logging;
    Ok(vec![RequiredArtifact {
        relative_path: local_log_config_path(&file.id),
        destination: state.directories.log_configs_dir().join(&file.id),
        expected_sha1: Some(file.sha1.clone()),
        expected_size: Some(file.size as u64),
    }])
}

fn library_artifacts(
    state: &State,
    version_info: &GameVersionInfo,
    java_arch: &str,
    minecraft_updated: bool,
) -> crate::Result<(Vec<RequiredArtifact>, Vec<RequiredArtifact>)> {
    let mut libraries = Vec::new();
    let mut natives = Vec::new();

    for library in &version_info.libraries {
        if let Some(rules) = &library.rules
            && !parse_rules(
                rules,
                java_arch,
                &QuickPlayType::None,
                minecraft_updated,
            )
        {
            continue;
        }
        if !library.downloadable {
            continue;
        }

        // Native library artifact for this platform, if any.
        if is_native_library(library)
            && let Some(classifier) =
                library_native_classifier(library, java_arch)
        {
            let native = library
                .downloads
                .as_ref()
                .and_then(|downloads| downloads.classifiers.as_ref())
                .and_then(|classifiers| classifiers.get(&classifier));
            if let Some(native) = native {
                natives.push(RequiredArtifact {
                    relative_path: local_native_library_path(
                        library,
                        native,
                        &classifier,
                    )?,
                    destination: state
                        .directories
                        .caches_dir()
                        .join("minecraft-natives")
                        .join(format!("{}.jar", native.sha1)),
                    expected_sha1: Some(native.sha1.clone()),
                    expected_size: Some(native.size as u64),
                });
            } else if library_classifier_coordinate_is_native(library) {
                // Forge and newer manifests may represent a native as a
                // four-part coordinate (group:artifact:version:natives-*),
                // with its metadata in downloads.artifact rather than the
                // legacy downloads.classifiers map. Keep that artifact in
                // libraries/ so native preparation can consume it directly.
                if let Some(artifact) = library
                    .downloads
                    .as_ref()
                    .and_then(|downloads| downloads.artifact.as_ref())
                    .filter(|artifact| !artifact.url.is_empty())
                {
                    let artifact_path =
                        native_library_artifact_path(library, &classifier)?;
                    natives.push(RequiredArtifact {
                        relative_path: Path::new("libraries")
                            .join(&artifact_path),
                        destination: state
                            .directories
                            .libraries_dir()
                            .join(&artifact_path),
                        expected_sha1: Some(artifact.sha1.clone()),
                        expected_size: Some(artifact.size as u64),
                    });
                }
            }
        }

        // Java artifact (regular JAR). Mixed libraries carry both.
        if needs_java_artifact(library) {
            let artifact_path =
                daedalus::get_path_from_artifact(&library.name)?;
            let (expected_sha1, expected_size) = if let Some(artifact) = library
                .downloads
                .as_ref()
                .and_then(|downloads| downloads.artifact.as_ref())
                .filter(|artifact| !artifact.url.is_empty())
            {
                (Some(artifact.sha1.clone()), Some(artifact.size as u64))
            } else {
                (legacy_library_sha1(library).map(str::to_string), None)
            };
            libraries.push(RequiredArtifact {
                relative_path: local_library_path(&library.name)?,
                destination: state
                    .directories
                    .libraries_dir()
                    .join(&artifact_path),
                expected_sha1,
                expected_size,
            });
        }
    }

    Ok((libraries, natives))
}

async fn asset_object_artifacts(
    state: &State,
    local_source: Option<&LocalRuntimeSource>,
    version_info: &GameVersionInfo,
) -> crate::Result<Vec<RequiredArtifact>> {
    let index = load_assets_index(state, local_source, version_info).await?;
    let mut artifacts = Vec::with_capacity(index.objects.len());
    for asset in index.objects.values() {
        artifacts.push(RequiredArtifact {
            relative_path: local_asset_object_path(&asset.hash),
            destination: state.directories.object_dir(&asset.hash),
            expected_sha1: Some(asset.hash.clone()),
            expected_size: Some(asset.size as u64),
        });
    }
    Ok(artifacts)
}

async fn load_assets_index(
    state: &State,
    local_source: Option<&LocalRuntimeSource>,
    version_info: &GameVersionInfo,
) -> crate::Result<AssetsIndex> {
    let index_path = state
        .directories
        .assets_index_dir()
        .join(format!("{}.json", version_info.asset_index.id));
    if index_path.is_file() {
        let bytes = crate::util::io::read(&index_path).await?;
        return Ok(serde_json::from_slice(&bytes)?);
    }

    if let Some(local) = local_source {
        let relative = local_asset_index_path(&version_info.asset_index.id);
        let candidate = local.root.join(&relative);
        if candidate.is_file()
            && matches!(
                classify_local_artifact(
                    Some(local),
                    &index_path,
                    &relative,
                    Some(&version_info.asset_index.sha1),
                    Some(version_info.asset_index.size as u64),
                )
                .await?,
                ArtifactAvailability::LocalReusable
            )
        {
            let bytes = crate::util::io::read(&candidate).await?;
            return Ok(serde_json::from_slice(&bytes)?);
        }
    }

    crate::util::fetch::fetch_json(
        Method::GET,
        &version_info.asset_index.url,
        Some(&version_info.asset_index.sha1),
        None,
        None,
        &state.api_semaphore,
    )
    .await
}

async fn classify_artifacts(
    local: Option<&LocalRuntimeSource>,
    artifacts: &[RequiredArtifact],
    cancellation: &CancellationToken,
) -> crate::Result<ImportPlanCountsBySource> {
    let mut counts = ImportPlanCountsBySource::default();
    for artifact in artifacts {
        if cancellation.is_cancelled() {
            break;
        }
        let availability = classify_local_artifact(
            local,
            &artifact.destination,
            &artifact.relative_path,
            artifact.expected_sha1.as_deref(),
            artifact.expected_size,
        )
        .await?;
        let bytes = match artifact.expected_size {
            Some(size) => size,
            None => match availability {
                ArtifactAvailability::Cached => {
                    crate::util::io::metadata(&artifact.destination)
                        .await
                        .map(|metadata| metadata.len())
                        .unwrap_or(0)
                }
                ArtifactAvailability::LocalReusable => match local {
                    Some(source) => crate::util::io::metadata(
                        &source.root.join(&artifact.relative_path),
                    )
                    .await
                    .map(|metadata| metadata.len())
                    .unwrap_or(0),
                    None => 0,
                },
                ArtifactAvailability::NetworkRequired => 0,
            },
        };
        match availability {
            ArtifactAvailability::Cached => {
                counts.cache.files += 1;
                counts.cache.bytes += bytes;
            }
            ArtifactAvailability::LocalReusable => {
                counts.local.files += 1;
                counts.local.bytes += bytes;
            }
            ArtifactAvailability::NetworkRequired => {
                counts.network.files += 1;
                counts.network.bytes += bytes;
            }
        }
    }
    Ok(counts)
}

async fn scan_counts_emitting(
    request: &ImportPlanRequest,
    dotminecraft: &Path,
    import_path: &str,
    minecraft_root: &str,
    game_version: &Option<String>,
    loader: &Option<String>,
    loader_version: &Option<String>,
    cancellation: &CancellationToken,
) -> crate::Result<(u64, ImportPlanCounts)> {
    // TODO(B2): when overrides change, recompute cache/local/network
    // incrementally without rescanning the full .minecraft tree.
    let excluded_roots = ["assets", "libraries", "versions"];
    let dirname = dotminecraft
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default();
    let skip_json = format!("{dirname}.json");
    let skip_jar = format!("{dirname}.jar");
    let mods_dir = dotminecraft.join("mods");
    let mut mod_count = 0u64;
    let mut migrate = ImportPlanCounts::default();
    let mut stack = vec![dotminecraft.to_path_buf()];
    let mut processed = 0u64;

    while let Some(dir) = stack.pop() {
        if cancellation.is_cancelled() {
            break;
        }
        let mut entries = tokio::fs::read_dir(&dir).await.map_err(|error| {
            crate::util::io::IOError::with_path(error, &dir)
        })?;
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|error| crate::util::io::IOError::with_path(error, &dir))?
        {
            if cancellation.is_cancelled() {
                break;
            }
            let path = entry.path();
            if dir == dotminecraft
                && excluded_roots
                    .iter()
                    .any(|name| entry.file_name().to_string_lossy() == *name)
            {
                continue;
            }
            let metadata =
                tokio::fs::symlink_metadata(&path).await.map_err(|error| {
                    crate::util::io::IOError::with_path(error, &path)
                })?;
            if metadata.is_dir() && !metadata.file_type().is_symlink() {
                stack.push(path);
                continue;
            }
            if dir == dotminecraft {
                let name = entry.file_name().to_string_lossy().to_string();
                if name == skip_json || name == skip_jar {
                    continue;
                }
            }
            migrate.files += 1;
            migrate.bytes += metadata.len();
            if path.strip_prefix(&mods_dir).is_ok()
                && path.extension().map(|ext| ext == "jar").unwrap_or(false)
            {
                mod_count += 1;
            }
            processed += 1;
            if processed <= 64 || processed.is_multiple_of(16) {
                emit_import_plan(&snapshot_for(
                    request,
                    ImportPlanStage::Scanning,
                    import_path,
                    minecraft_root,
                    game_version.clone(),
                    loader.clone(),
                    loader_version.clone(),
                    mod_count,
                    ImportPlanCountsBySource::default(),
                    migrate,
                    None,
                ))
                .await?;
            }
        }
    }

    Ok((mod_count, migrate))
}

#[allow(unused_variables)]
async fn emit_import_plan(snapshot: &ImportPlanSnapshot) -> crate::Result<()> {
    #[cfg(feature = "tauri")]
    {
        use tauri::Emitter;

        let event_state = crate::EventState::get()?;
        event_state
            .app
            .emit("import_plan", snapshot)
            .map_err(crate::event::EventError::from)?;
    }
    Ok(())
}

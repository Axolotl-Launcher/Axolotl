use crate::State;
use crate::state::CachedLoaderManifest;
use crate::util::fetch::{
    FetchSemaphore, fetch, fetch_json, fetch_metadata, fetch_official,
};
use crate::util::io;
use daedalus::minecraft::{
    Argument, ArgumentType, JavaVersion, Library, VersionType,
};
use daedalus::modded::{
    DUMMY_REPLACE_STRING, LoaderProfileSource, LoaderVersion, Manifest,
    PartialVersionInfo, Processor, SidedDataEntry, Version, VersionGroup,
};
use reqwest::Method;
use serde::Deserialize;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::io::{Cursor, Read};
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

const FABRIC_META_URL: &str = "https://meta.fabricmc.net/v2/versions/";
const QUILT_META_URL: &str = "https://meta.quiltmc.org/v3/versions/";
const LEGACY_FABRIC_META_URL: &str =
    "https://meta.legacyfabric.net/v2/versions/";
const BABRIC_META_URL: &str = "https://meta.babric.dev/v2/versions/";
const BABRIC_GAME_VERSION: &str = "b1.7.3";
const BABRIC_MAVEN_URL: &str = "https://maven.glass-launcher.net/babric/";
const BABRIC_LOADER_METADATA_URL: &str =
    "https://maven.glass-launcher.net/babric/babric/fabric-loader/";
const BABRIC_POLYFILL_VERSION_URL: &str =
    "https://babric.github.io/manifest-polyfill/b1.7.3.json";
const BABRIC_FALLBACK_LOADER_VERSION: &str = "0.15.6-babric.2";
const OPTIFINE_META_URL: &str = "https://bmclapi2.bangbang93.com/optifine";
const OPTIFINE_VERSION_LIST_URL: &str =
    "https://bmclapi2.bangbang93.com/optifine/versionList";
const LITELOADER_META_URL: &str =
    "https://dl.liteloader.com/versions/versions.json";
const CLEANROOM_RELEASES_URL: &str =
    "https://api.github.com/repos/CleanroomMC/Cleanroom/releases?per_page=100";
const CLEANROOM_MAVEN_METADATA_URL: &str = "https://repo.cleanroommc.com/releases/com/cleanroommc/cleanroom/maven-metadata.xml";
const CLEANROOM_MAVEN_VERSION_URL: &str =
    "https://repo.cleanroommc.com/releases/com/cleanroommc/cleanroom/";
const FORGE_MAVEN_URL: &str =
    "https://maven.minecraftforge.net/net/minecraftforge/forge/";
const FORGE_PROMOTIONS_URL: &str = "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json";
const NEOFORGE_MAVEN_URL: &str =
    "https://maven.neoforged.net/releases/net/neoforged/neoforge/";
const NEOFORGE_LEGACY_MAVEN_URL: &str =
    "https://maven.neoforged.net/releases/net/neoforged/forge/";
const LOADER_METADATA_TTL: Duration = Duration::from_secs(30 * 60);
const LOADER_METADATA_RETRY_DELAY: Duration = Duration::from_secs(5);
const LOADER_METADATA_TIMEOUT: Duration = Duration::from_secs(15);
const FORGE_PROMOTIONS_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_LOADER_METADATA_ENTRIES: usize = 128;
static NEOFORGE_CURRENT_METADATA: LazyLock<MetadataCache<MavenMetadata>> =
    LazyLock::new(MetadataCache::default);
static NEOFORGE_LEGACY_METADATA: LazyLock<MetadataCache<MavenMetadata>> =
    LazyLock::new(MetadataCache::default);
static LOADER_MANIFESTS: LazyLock<LoaderManifestCache> =
    LazyLock::new(LoaderManifestCache::default);
const INSTALLER_URL_KEY: &str = "AXOLOTL_INSTALLER_URL";
const INSTALLER_ENTRY_KEY_PREFIX: &str = "AXOLOTL_INSTALLER_ENTRY_";

#[derive(Deserialize)]
struct MetaGameVersion {
    version: String,
    stable: bool,
}

#[derive(Deserialize)]
struct FabricLoaderVersion {
    version: String,
    stable: bool,
}

#[derive(Deserialize)]
struct QuiltLoaderVersion {
    version: String,
}

#[derive(Deserialize)]
struct FabricLoaderMetadata {
    loader: FabricLoaderVersion,
}

#[derive(Deserialize)]
struct QuiltLoaderMetadata {
    loader: QuiltLoaderVersion,
}

#[derive(Deserialize)]
struct LegacyFabricMetadata {
    game: Vec<MetaGameVersion>,
    loader: Vec<FabricLoaderVersion>,
}

#[derive(Deserialize)]
struct OptiFineMetadata {
    mcversion: String,
    #[serde(rename = "type")]
    type_: String,
    patch: String,
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    #[serde(default)]
    assets: Vec<GithubReleaseAsset>,
}

#[derive(Deserialize)]
struct GithubReleaseAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Clone, Deserialize, Default)]
struct MavenMetadata {
    versioning: MavenVersioning,
}

#[derive(Clone, Deserialize, Default)]
struct MavenVersioning {
    #[serde(default)]
    release: String,
    versions: MavenVersions,
}

#[derive(Clone, Deserialize, Default)]
struct MavenVersions {
    #[serde(rename = "version", default)]
    values: Vec<String>,
}

struct MetadataCache<T> {
    value: Mutex<Option<(Instant, T)>>,
    failure: Mutex<Option<(Instant, Arc<crate::ErrorKind>)>>,
}

impl<T> Default for MetadataCache<T> {
    fn default() -> Self {
        Self {
            value: Mutex::new(None),
            failure: Mutex::new(None),
        }
    }
}

impl<T: Clone> MetadataCache<T> {
    #[cfg(test)]
    async fn get_or_fetch<F, Fut>(&self, fetch: F) -> crate::Result<T>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = crate::Result<T>>,
    {
        self.get_or_fetch_with_refresh(false, fetch).await
    }

    async fn get_or_fetch_with_refresh<F, Fut>(
        &self,
        force_refresh: bool,
        fetch: F,
    ) -> crate::Result<T>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = crate::Result<T>>,
    {
        let requested_at = Instant::now();
        // Keep the lock through the fetch so full and scoped
        // requests share one download, including during background refresh.
        let mut cached = self.value.lock().await;
        if let Some((fetched_at, metadata)) = cached.as_ref()
            && if force_refresh {
                *fetched_at >= requested_at
            } else {
                fetched_at.elapsed() < LOADER_METADATA_TTL
            }
        {
            return Ok(metadata.clone());
        }

        if let Some((failed_at, error)) = self.failure.lock().await.as_ref()
            && failed_at.elapsed() < LOADER_METADATA_RETRY_DELAY
        {
            if !force_refresh && let Some((_, value)) = cached.as_ref() {
                return Ok(value.clone());
            }
            return Err(crate::Error::from_shared_kind(Arc::clone(error)));
        }

        match fetch().await {
            Ok(metadata) => {
                *cached = Some((Instant::now(), metadata.clone()));
                *self.failure.lock().await = None;
                Ok(metadata)
            }
            Err(error) => {
                *self.failure.lock().await =
                    Some((Instant::now(), Arc::clone(&error.raw)));
                if !force_refresh && let Some((_, metadata)) = cached.as_ref() {
                    tracing::warn!(error = %error, "Loader metadata refresh failed; serving cached data");
                    Ok(metadata.clone())
                } else {
                    Err(error)
                }
            }
        }
    }
}

#[derive(Default)]
struct LoaderManifestCache {
    entries: Mutex<HashMap<String, Arc<MetadataCache<CachedLoaderManifest>>>>,
}

impl LoaderManifestCache {
    #[cfg(test)]
    async fn get_or_fetch<'a, Fut>(
        &self,
        loader: &str,
        game_version: Option<&'a str>,
        fetch: impl FnOnce(Option<&'a str>) -> Fut,
    ) -> crate::Result<CachedLoaderManifest>
    where
        Fut: Future<Output = crate::Result<CachedLoaderManifest>>,
    {
        self.get_or_fetch_with_refresh(loader, game_version, false, fetch)
            .await
    }

    async fn get_or_fetch_with_refresh<'a, Fut>(
        &self,
        loader: &str,
        game_version: Option<&'a str>,
        force_refresh: bool,
        fetch: impl FnOnce(Option<&'a str>) -> Fut,
    ) -> crate::Result<CachedLoaderManifest>
    where
        Fut: Future<Output = crate::Result<CachedLoaderManifest>>,
    {
        let request_scope = manifest_request_scope(loader, game_version);
        let cache_key = if let Some(game_version) = request_scope {
            daedalus::modded::loader_manifest_metadata_for_game(
                loader,
                game_version,
            )
        } else {
            daedalus::modded::loader_manifest_metadata(loader)
        }
        .cache_key;
        let cache = self.entry(cache_key).await;
        let mut result = cache
            .get_or_fetch_with_refresh(force_refresh, || fetch(request_scope))
            .await?;
        if request_scope != game_version
            && let Some(game_version) = game_version
        {
            result.manifest = scope_manifest(result.manifest, game_version);
        }
        Ok(result)
    }

    async fn entry(
        &self,
        key: String,
    ) -> Arc<MetadataCache<CachedLoaderManifest>> {
        let mut entries = self.entries.lock().await;
        if let Some(entry) = entries.get(&key) {
            return Arc::clone(entry);
        }
        if entries.len() >= MAX_LOADER_METADATA_ENTRIES {
            let unused = entries
                .iter()
                .find(|(_, entry)| Arc::strong_count(entry) == 1)
                .map(|(key, _)| key.clone());
            if let Some(unused) = unused {
                entries.remove(&unused);
            } else {
                return Arc::new(MetadataCache::default());
            }
        }
        Arc::clone(entries.entry(key).or_default())
    }
}

fn manifest_request_scope<'a>(
    loader: &str,
    game_version: Option<&'a str>,
) -> Option<&'a str> {
    match loader {
        "forge" | "legacy_fabric" | "babric" | "lite_loader" | "cleanroom" => {
            None
        }
        _ => game_version,
    }
}

#[derive(Deserialize)]
struct BabricLauncherMetadata {
    #[serde(default)]
    libraries: BabricLauncherLibraries,
    #[serde(rename = "mainClass")]
    main_class: BabricMainClass,
    #[serde(default)]
    min_java_version: Option<u32>,
}

#[derive(Deserialize, Default)]
struct BabricLauncherLibraries {
    #[serde(default)]
    common: Vec<Library>,
    #[serde(default)]
    client: Vec<Library>,
}

#[derive(Deserialize)]
struct BabricMainClass {
    client: String,
}

#[derive(Deserialize, Default)]
struct ForgePromotions {
    #[serde(default)]
    promos: HashMap<String, String>,
}

#[derive(Deserialize)]
struct InstallerProfile {
    #[serde(default)]
    minecraft: Option<String>,
    #[serde(default)]
    data: HashMap<String, SidedDataEntry>,
    #[serde(default)]
    processors: Vec<Processor>,
    #[serde(default)]
    libraries: Vec<Library>,
    #[serde(default)]
    install: Option<LegacyInstallerArtifact>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyInstallerArtifact {
    path: String,
    file_path: String,
}

struct InstallerArtifact {
    coordinate: String,
    data: Vec<u8>,
}

struct ParsedInstaller {
    profile: PartialVersionInfo,
    artifacts: Vec<InstallerArtifact>,
}

pub(crate) async fn fetch_loader_manifest_official_first(
    loader: &str,
    game_version: Option<&str>,
    fallback_url: &str,
    fetch_semaphore: &FetchSemaphore,
    force_refresh: bool,
) -> crate::Result<CachedLoaderManifest> {
    if let Some(game_version) = game_version {
        let unsupported = match loader {
            "babric" => game_version != BABRIC_GAME_VERSION,
            "cleanroom" => game_version != "1.12.2",
            "lite_loader" => !liteloader_supports_game_version(game_version),
            _ => false,
        };
        if unsupported {
            return Ok(CachedLoaderManifest {
                loader: loader.to_string(),
                manifest: unsupported_loader_manifest(game_version),
            });
        }
    }
    LOADER_MANIFESTS
        .get_or_fetch_with_refresh(
            loader,
            game_version,
            force_refresh,
            |request_scope| {
                fetch_loader_manifest_uncached(
                    loader,
                    request_scope,
                    fallback_url,
                    fetch_semaphore,
                    force_refresh,
                )
            },
        )
        .await
}

async fn fetch_loader_manifest_uncached(
    loader: &str,
    game_version: Option<&str>,
    fallback_url: &str,
    fetch_semaphore: &FetchSemaphore,
    force_refresh: bool,
) -> crate::Result<CachedLoaderManifest> {
    let official = tokio::time::timeout(LOADER_METADATA_TIMEOUT, async {
        match (loader, game_version) {
            ("fabric", Some(game_version)) => {
                fetch_fabric_manifest_for_game(game_version, fetch_semaphore)
                    .await
            }
            ("quilt", Some(game_version)) => {
                fetch_quilt_manifest_for_game(game_version, fetch_semaphore)
                    .await
            }
            ("legacy_fabric", Some(game_version)) => {
                fetch_legacy_fabric_manifest(fetch_semaphore)
                    .await
                    .map(|manifest| scope_manifest(manifest, game_version))
            }
            ("babric", Some(game_version)) => {
                fetch_babric_manifest_for_game(game_version, fetch_semaphore)
                    .await
            }
            ("optifine", Some(game_version)) => {
                fetch_optifine_manifest(Some(game_version), fetch_semaphore)
                    .await
            }
            ("lite_loader", Some(game_version)) => {
                fetch_liteloader_manifest(fetch_semaphore)
                    .await
                    .map(|manifest| scope_manifest(manifest, game_version))
            }
            ("cleanroom", Some(game_version)) => {
                fetch_cleanroom_manifest(fetch_semaphore)
                    .await
                    .map(|manifest| scope_manifest(manifest, game_version))
            }
            ("forge", Some(game_version)) => {
                fetch_forge_manifest(fetch_semaphore, force_refresh)
                    .await
                    .map(|manifest| scope_manifest(manifest, game_version))
            }
            ("neo", Some(game_version)) => {
                fetch_neoforge_manifest(
                    Some(game_version),
                    fetch_semaphore,
                    force_refresh,
                )
                .await
            }
            ("fabric", None) => fetch_fabric_manifest(fetch_semaphore).await,
            ("forge", None) => {
                fetch_forge_manifest(fetch_semaphore, force_refresh).await
            }
            ("neo", None) => {
                fetch_neoforge_manifest(None, fetch_semaphore, force_refresh)
                    .await
            }
            ("quilt", None) => fetch_quilt_manifest(fetch_semaphore).await,
            ("legacy_fabric", None) => {
                fetch_legacy_fabric_manifest(fetch_semaphore).await
            }
            ("babric", None) => fetch_babric_manifest(fetch_semaphore).await,
            ("optifine", None) => {
                fetch_optifine_manifest(None, fetch_semaphore).await
            }
            ("lite_loader", None) => {
                fetch_liteloader_manifest(fetch_semaphore).await
            }
            ("cleanroom", None) => {
                fetch_cleanroom_manifest(fetch_semaphore).await
            }
            _ => {
                return fetch_fallback_manifest(
                    loader,
                    game_version,
                    fallback_url,
                    fetch_semaphore,
                )
                .await
                .map(|metadata| metadata.manifest);
            }
        }
    })
    .await
    .unwrap_or_else(|_| {
        Err(crate::ErrorKind::OtherError(format!(
            "{loader} metadata request timed out"
        ))
        .as_error())
    });

    let official = official.and_then(|manifest| {
        if let Some(game_version) = game_version {
            validate_scoped_manifest(manifest, game_version)
        } else {
            validate_manifest(manifest)
        }
    });

    match official {
        Ok(manifest) => Ok(CachedLoaderManifest {
            loader: loader.to_string(),
            manifest,
        }),
        Err(error) => {
            tracing::warn!(
                loader,
                fallback_url,
                error = %error,
                "Official loader metadata failed; using launcher-meta fallback"
            );
            fetch_fallback_manifest(
                loader,
                game_version,
                fallback_url,
                fetch_semaphore,
            )
            .await
        }
    }
}

async fn fetch_fallback_manifest(
    loader: &str,
    game_version: Option<&str>,
    fallback_url: &str,
    fetch_semaphore: &FetchSemaphore,
) -> crate::Result<CachedLoaderManifest> {
    let mut manifest: Manifest =
        fetch_loader_json(fallback_url, fetch_semaphore).await?;
    if loader == "lite_loader" {
        manifest
            .game_versions
            .retain(|version| liteloader_supports_game_version(&version.id));
    }
    let manifest = if let Some(game_version) = game_version {
        scope_manifest(manifest, game_version)
    } else {
        manifest
    };
    Ok(CachedLoaderManifest {
        loader: loader.to_string(),
        manifest,
    })
}

fn validate_scoped_manifest(
    manifest: Manifest,
    game_version: &str,
) -> crate::Result<Manifest> {
    if manifest.game_versions.len() != 1
        || manifest.game_versions[0].id != game_version
    {
        return Err(crate::ErrorKind::OtherError(format!(
            "Loader metadata response was not scoped to Minecraft {game_version}"
        ))
        .as_error());
    }
    Ok(manifest)
}

fn validate_manifest(manifest: Manifest) -> crate::Result<Manifest> {
    let loader_count = manifest
        .game_versions
        .iter()
        .map(|version| version.loaders.len())
        .chain(
            manifest
                .version_groups
                .iter()
                .map(|group| group.loaders.len()),
        )
        .sum::<usize>();
    if manifest.game_versions.is_empty() || loader_count == 0 {
        return Err(crate::ErrorKind::OtherError(
            "Official loader metadata returned no installable versions"
                .to_string(),
        )
        .as_error());
    }
    Ok(manifest)
}

async fn fetch_fabric_manifest(
    fetch_semaphore: &FetchSemaphore,
) -> crate::Result<Manifest> {
    let game_url = format!("{FABRIC_META_URL}game");
    let loader_url = format!("{FABRIC_META_URL}loader");
    let (games, loaders) = tokio::try_join!(
        fetch_loader_json::<Vec<MetaGameVersion>>(&game_url, fetch_semaphore),
        fetch_loader_json::<Vec<FabricLoaderVersion>>(
            &loader_url,
            fetch_semaphore
        ),
    )?;
    Ok(meta_api_manifest(
        games,
        loaders
            .into_iter()
            .map(|loader| (loader.version, loader.stable))
            .collect(),
        "fabric",
        FABRIC_META_URL,
        0,
    ))
}

async fn fetch_fabric_manifest_for_game(
    game_version: &str,
    fetch_semaphore: &FetchSemaphore,
) -> crate::Result<Manifest> {
    let url = meta_loader_versions_url(FABRIC_META_URL, game_version);
    let loaders =
        fetch_loader_json::<Vec<FabricLoaderMetadata>>(&url, fetch_semaphore)
            .await?
            .into_iter()
            .map(|metadata| (metadata.loader.version, metadata.loader.stable))
            .collect();
    Ok(meta_api_manifest_for_game(
        game_version,
        loaders,
        "fabric",
        FABRIC_META_URL,
        0,
    ))
}

async fn fetch_quilt_manifest(
    fetch_semaphore: &FetchSemaphore,
) -> crate::Result<Manifest> {
    let game_url = format!("{QUILT_META_URL}game");
    let loader_url = format!("{QUILT_META_URL}loader");
    let (games, loaders) = tokio::try_join!(
        fetch_loader_json::<Vec<MetaGameVersion>>(&game_url, fetch_semaphore),
        fetch_loader_json::<Vec<QuiltLoaderVersion>>(
            &loader_url,
            fetch_semaphore
        ),
    )?;
    Ok(meta_api_manifest(
        games,
        loaders
            .into_iter()
            .map(|loader| {
                let stable = is_stable_version(&loader.version);
                (loader.version, stable)
            })
            .collect(),
        "quilt",
        QUILT_META_URL,
        1,
    ))
}

async fn fetch_quilt_manifest_for_game(
    game_version: &str,
    fetch_semaphore: &FetchSemaphore,
) -> crate::Result<Manifest> {
    let url = meta_loader_versions_url(QUILT_META_URL, game_version);
    let loaders =
        fetch_loader_json::<Vec<QuiltLoaderMetadata>>(&url, fetch_semaphore)
            .await?
            .into_iter()
            .map(|metadata| {
                let version = metadata.loader.version;
                let stable = is_stable_version(&version);
                (version, stable)
            })
            .collect();
    Ok(meta_api_manifest_for_game(
        game_version,
        loaders,
        "quilt",
        QUILT_META_URL,
        1,
    ))
}

async fn fetch_legacy_fabric_manifest(
    fetch_semaphore: &FetchSemaphore,
) -> crate::Result<Manifest> {
    let metadata: LegacyFabricMetadata = fetch_loader_json(
        LEGACY_FABRIC_META_URL.trim_end_matches('/'),
        fetch_semaphore,
    )
    .await?;
    Ok(meta_api_manifest(
        metadata.game,
        metadata
            .loader
            .into_iter()
            .map(|loader| (loader.version, loader.stable))
            .collect(),
        "legacy_fabric",
        LEGACY_FABRIC_META_URL,
        0,
    ))
}

async fn fetch_babric_manifest(
    fetch_semaphore: &FetchSemaphore,
) -> crate::Result<Manifest> {
    fetch_babric_manifest_for_game(BABRIC_GAME_VERSION, fetch_semaphore).await
}

async fn fetch_babric_manifest_for_game(
    game_version: &str,
    fetch_semaphore: &FetchSemaphore,
) -> crate::Result<Manifest> {
    if game_version != BABRIC_GAME_VERSION {
        return Ok(unsupported_loader_manifest(game_version));
    }
    let metadata_url =
        format!("{BABRIC_LOADER_METADATA_URL}maven-metadata.xml");
    let metadata = fetch_maven_metadata(&metadata_url, fetch_semaphore)
        .await
        .ok();
    let versions = metadata
        .map(|metadata| metadata.versioning.versions.values)
        .filter(|versions| !versions.is_empty())
        .unwrap_or_else(|| vec![BABRIC_FALLBACK_LOADER_VERSION.to_string()]);
    Ok(babric_manifest(game_version, versions))
}

fn unsupported_loader_manifest(game_version: &str) -> Manifest {
    Manifest {
        game_versions: vec![Version {
            id: game_version.to_string(),
            stable: true,
            version_group: None,
            loaders: Vec::new(),
        }],
        version_groups: Vec::new(),
    }
}

fn babric_manifest(game_version: &str, versions: Vec<String>) -> Manifest {
    let mut loaders = versions
        .into_iter()
        .map(|version| LoaderVersion {
            id: version.clone(),
            url: format!(
                "{BABRIC_META_URL}loader/{game_version}/{version}/profile/json"
            ),
            stable: version == BABRIC_FALLBACK_LOADER_VERSION,
            profile_source: LoaderProfileSource::Babric,
            fallback_url: None,
        })
        .collect::<Vec<_>>();
    loaders.sort_by(|left, right| compare_versions(&right.id, &left.id));
    Manifest {
        game_versions: vec![Version {
            id: game_version.to_string(),
            stable: true,
            version_group: None,
            loaders,
        }],
        version_groups: Vec::new(),
    }
}

async fn fetch_optifine_manifest(
    game_version: Option<&str>,
    fetch_semaphore: &FetchSemaphore,
) -> crate::Result<Manifest> {
    let url = optifine_metadata_url(game_version);
    let entries: Vec<OptiFineMetadata> =
        fetch_loader_json(&url, fetch_semaphore).await?;
    Ok(optifine_manifest(entries, game_version))
}

fn optifine_metadata_url(game_version: Option<&str>) -> String {
    game_version.map_or_else(
        || OPTIFINE_VERSION_LIST_URL.to_string(),
        |game_version| format!("{OPTIFINE_META_URL}/{game_version}"),
    )
}

fn optifine_manifest(
    entries: Vec<OptiFineMetadata>,
    game_version: Option<&str>,
) -> Manifest {
    let mut grouped = HashMap::<String, Vec<LoaderVersion>>::new();
    for entry in entries {
        if game_version.is_some_and(|version| version != entry.mcversion) {
            continue;
        }
        let id = format!("OptiFine_{}_{}", entry.type_, entry.patch);
        let stable = !entry.patch.to_ascii_lowercase().contains("pre");
        grouped.entry(entry.mcversion.clone()).or_default().push(
            LoaderVersion {
                id,
                url: format!(
                    "{OPTIFINE_META_URL}/{}/{}/{}",
                    entry.mcversion, entry.type_, entry.patch
                ),
                stable,
                profile_source: LoaderProfileSource::Json,
                fallback_url: None,
            },
        );
    }
    let manifest = grouped_manifest(grouped);
    match game_version {
        Some(game_version) => scope_manifest(manifest, game_version),
        None => manifest,
    }
}

async fn fetch_liteloader_manifest(
    fetch_semaphore: &FetchSemaphore,
) -> crate::Result<Manifest> {
    let metadata: serde_json::Value =
        fetch_loader_json(LITELOADER_META_URL, fetch_semaphore).await?;
    liteloader_manifest(&metadata)
}

fn liteloader_manifest(
    metadata: &serde_json::Value,
) -> crate::Result<Manifest> {
    let versions = metadata
        .get("versions")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| {
            crate::ErrorKind::OtherError(
                "LiteLoader metadata has no versions object".to_string(),
            )
            .as_error()
        })?;
    let mut game_versions = Vec::new();
    for (game_version, metadata) in versions {
        if !liteloader_supports_game_version(game_version) {
            continue;
        }
        let Some(entry) = liteloader_latest_entry(metadata) else {
            continue;
        };
        let Some(version) =
            entry.get("version").and_then(|value| value.as_str())
        else {
            continue;
        };
        let stable = entry
            .get("stream")
            .and_then(|value| value.as_str())
            .is_none_or(|stream| !stream.eq_ignore_ascii_case("snapshot"));
        game_versions.push(Version {
            id: game_version.clone(),
            stable: true,
            version_group: None,
            loaders: vec![LoaderVersion {
                id: version.to_string(),
                url: LITELOADER_META_URL.to_string(),
                stable,
                profile_source: LoaderProfileSource::LiteLoader,
                fallback_url: Some(
                    "https://bmclapi2.bangbang93.com/maven/com/mumfrey/liteloader/versions.json"
                        .to_string(),
                ),
            }],
        });
    }
    game_versions.sort_by(|left, right| compare_versions(&right.id, &left.id));
    Ok(Manifest {
        game_versions,
        version_groups: Vec::new(),
    })
}

fn liteloader_supports_game_version(game_version: &str) -> bool {
    !game_version.starts_with("1.5") && !game_version.starts_with("1.6")
}

fn liteloader_latest_entry(
    game_metadata: &serde_json::Value,
) -> Option<&serde_json::Value> {
    game_metadata
        .get("artefacts")
        .or_else(|| game_metadata.get("snapshots"))?
        .get("com.mumfrey:liteloader")?
        .get("latest")
}

async fn fetch_cleanroom_manifest(
    fetch_semaphore: &FetchSemaphore,
) -> crate::Result<Manifest> {
    // CleanroomMC's own Maven is the primary source: it is the same channel
    // their website links for downloads and, unlike the anonymous GitHub
    // releases API, it has no per-IP rate limit to exhaust (which otherwise
    // leaves the loader appearing unsupported). GitHub stays as a fallback.
    match fetch_maven_metadata(CLEANROOM_MAVEN_METADATA_URL, fetch_semaphore)
        .await
    {
        Ok(metadata) => Ok(cleanroom_maven_manifest(metadata)),
        Err(maven_error) => {
            tracing::warn!(
                error = %maven_error,
                "CleanroomMC maven metadata failed; falling back to GitHub releases"
            );
            let releases: Vec<GithubRelease> =
                fetch_loader_json(CLEANROOM_RELEASES_URL, fetch_semaphore)
                    .await?;
            Ok(cleanroom_manifest(releases))
        }
    }
}

fn cleanroom_maven_manifest(metadata: MavenMetadata) -> Manifest {
    let mut loaders = metadata
        .versioning
        .versions
        .values
        .into_iter()
        .map(|version| {
            let url = format!("{CLEANROOM_MAVEN_VERSION_URL}{version}/cleanroom-{version}-installer.jar");
            LoaderVersion {
                id: version.clone(),
                url,
                stable: is_stable_version(&version),
                profile_source: LoaderProfileSource::Installer,
                fallback_url: None,
            }
        })
        .collect::<Vec<_>>();
    loaders.sort_by(|left, right| compare_versions(&right.id, &left.id));
    Manifest {
        game_versions: vec![Version {
            id: "1.12.2".to_string(),
            stable: true,
            version_group: None,
            loaders,
        }],
        version_groups: Vec::new(),
    }
}

fn cleanroom_manifest(releases: Vec<GithubRelease>) -> Manifest {
    let mut loaders = releases
        .into_iter()
        .filter(|release| !release.draft)
        .filter_map(|release| {
            let expected_name =
                format!("cleanroom-{}-installer.jar", release.tag_name);
            let asset = release
                .assets
                .into_iter()
                .find(|asset| asset.name == expected_name)?;
            Some(LoaderVersion {
                id: release.tag_name.clone(),
                url: asset.browser_download_url,
                stable: !release.prerelease
                    && is_stable_version(&release.tag_name),
                profile_source: LoaderProfileSource::Installer,
                fallback_url: None,
            })
        })
        .collect::<Vec<_>>();
    loaders.sort_by(|left, right| compare_versions(&right.id, &left.id));
    Manifest {
        game_versions: vec![Version {
            id: "1.12.2".to_string(),
            stable: true,
            version_group: None,
            loaders,
        }],
        version_groups: Vec::new(),
    }
}

fn meta_loader_versions_url(base_url: &str, game_version: &str) -> String {
    format!("{base_url}loader/{game_version}")
}

fn meta_api_manifest_for_game(
    game_version: &str,
    mut loaders: Vec<(String, bool)>,
    loader: &str,
    official_base_url: &str,
    fallback_format_version: usize,
) -> Manifest {
    loaders.sort_by(|left, right| compare_versions(&right.0, &left.0));
    Manifest {
        game_versions: vec![Version {
            id: game_version.to_string(),
            stable: true,
            version_group: None,
            loaders: loaders
                .into_iter()
                .map(|(version, stable)| LoaderVersion {
                    id: version.clone(),
                    url: format!(
                        "{official_base_url}loader/{game_version}/{version}/profile/json"
                    ),
                    stable,
                    profile_source: LoaderProfileSource::Json,
                    fallback_url: Some(fallback_profile_url(
                        loader,
                        fallback_format_version,
                        &version,
                    )),
                })
                .collect(),
        }],
        version_groups: Vec::new(),
    }
}

fn meta_api_manifest(
    games: Vec<MetaGameVersion>,
    mut loaders: Vec<(String, bool)>,
    loader: &str,
    official_base_url: &str,
    fallback_format_version: usize,
) -> Manifest {
    loaders.sort_by(|left, right| compare_versions(&right.0, &left.0));
    let group_id = format!("{loader}-official");
    let loader_versions = loaders
        .into_iter()
        .map(|(version, stable)| LoaderVersion {
            id: version.clone(),
            url: format!(
                "{official_base_url}loader/{DUMMY_REPLACE_STRING}/{version}/profile/json"
            ),
            stable,
            profile_source: LoaderProfileSource::Json,
            fallback_url: Some(format!(
                "{}{}{}{}{}",
                env!("MODRINTH_LAUNCHER_META_URL"),
                loader,
                "/v",
                fallback_format_version,
                format!("/versions/{version}.json"),
            )),
        })
        .collect();
    Manifest {
        game_versions: games
            .into_iter()
            .map(|game| Version {
                id: game.version,
                stable: game.stable,
                version_group: Some(group_id.clone()),
                loaders: Vec::new(),
            })
            .collect(),
        version_groups: vec![VersionGroup {
            id: group_id,
            loaders: loader_versions,
        }],
    }
}

async fn fetch_forge_manifest(
    fetch_semaphore: &FetchSemaphore,
    official_only: bool,
) -> crate::Result<Manifest> {
    let metadata_url = format!("{FORGE_MAVEN_URL}maven-metadata.xml");
    let (metadata, promotions) = fetch_forge_indexes(
        async {
            if official_only {
                let bytes = fetch_official(
                    &metadata_url,
                    None,
                    None,
                    None,
                    fetch_semaphore,
                )
                .await?;
                parse_maven_metadata(&metadata_url, &bytes)
            } else {
                fetch_maven_metadata(&metadata_url, fetch_semaphore).await
            }
        },
        fetch_loader_json(FORGE_PROMOTIONS_URL, fetch_semaphore),
    )
    .await?;
    Ok(forge_manifest(metadata, promotions))
}

/// An exact-version miss is not a transport error, so it needs an independent
/// catalog lookup even when the primary catalog passed its format validation.
pub(crate) async fn find_forge_version_in_fallback(
    game_version: &str,
    requested: &str,
) -> crate::Result<Option<LoaderVersion>> {
    let state = crate::State::get().await?;
    let url = format!(
        "{}forge/v0/manifest.json",
        env!("MODRINTH_LAUNCHER_META_URL")
    );
    let bytes = tokio::time::timeout(
        LOADER_METADATA_TIMEOUT,
        fetch_official(&url, None, None, None, &state.api_semaphore),
    )
    .await
    .map_err(|_| {
        crate::ErrorKind::OtherError(
            "Forge fallback catalog request timed out".into(),
        )
        .as_error()
    })??;
    let manifest = validate_manifest(serde_json::from_slice(&bytes)?)?;
    Ok(find_exact_forge_version(manifest, game_version, requested))
}

fn find_exact_forge_version(
    manifest: Manifest,
    game_version: &str,
    requested: &str,
) -> Option<LoaderVersion> {
    scope_manifest(manifest, game_version)
        .game_versions
        .into_iter()
        .flat_map(|version| version.loaders)
        .find(|version| version.id == requested)
}

async fn fetch_forge_indexes(
    metadata: impl Future<Output = crate::Result<MavenMetadata>>,
    promotions: impl Future<Output = crate::Result<ForgePromotions>>,
) -> crate::Result<(MavenMetadata, ForgePromotions)> {
    tokio::try_join!(metadata, async {
        Ok(tokio::time::timeout(FORGE_PROMOTIONS_TIMEOUT, promotions)
            .await
            .ok()
            .and_then(Result::ok)
            .unwrap_or_default())
    })
}

fn forge_manifest(
    metadata: MavenMetadata,
    promotions: ForgePromotions,
) -> Manifest {
    let mut grouped: HashMap<String, Vec<LoaderVersion>> = HashMap::new();
    for full_version in metadata.versioning.versions.values {
        let Some((game_version, raw_loader_version)) =
            full_version.split_once('-')
        else {
            continue;
        };

        let trailing_game_version = format!("-{game_version}");
        let loader_version = raw_loader_version
            .strip_suffix(&trailing_game_version)
            .unwrap_or(raw_loader_version);

        let recommended = promotions
            .promos
            .get(&format!("{game_version}-recommended"));
        let stable =
            recommended.is_some_and(|promoted| promoted == loader_version);

        add_forge_loader_version(
            &mut grouped,
            game_version,
            loader_version,
            &full_version,
            stable,
        );
    }

    let mut manifest = grouped_manifest(grouped);
    for game_version in &mut manifest.game_versions {
        let Some(latest) = promotions
            .promos
            .get(&format!("{}-latest", game_version.id))
        else {
            continue;
        };
        let Some(index) = game_version
            .loaders
            .iter()
            .position(|loader| loader.id == *latest)
        else {
            continue;
        };
        if index != 0 {
            let latest = game_version.loaders.remove(index);
            game_version.loaders.insert(0, latest);
        }
    }
    manifest
}

fn add_forge_loader_version(
    grouped: &mut HashMap<String, Vec<LoaderVersion>>,
    game_version: &str,
    loader_version: &str,
    full_version: &str,
    stable: bool,
) {
    let loaders = grouped.entry(game_version.to_string()).or_default();
    if let Some(existing) = loaders
        .iter_mut()
        .find(|version| version.id == loader_version)
    {
        existing.stable |= stable;
        return;
    }

    loaders.push(LoaderVersion {
        id: loader_version.to_string(),
        url: format!(
            "{FORGE_MAVEN_URL}{full_version}/forge-{full_version}-installer.jar"
        ),
        stable,
        profile_source: LoaderProfileSource::Installer,
        fallback_url: Some(fallback_profile_url("forge", 0, loader_version)),
    });
}

async fn fetch_neoforge_manifest(
    game_version: Option<&str>,
    fetch_semaphore: &FetchSemaphore,
    force_refresh: bool,
) -> crate::Result<Manifest> {
    let (current, legacy) = fetch_neoforge_indexes(
        game_version,
        || {
            NEOFORGE_CURRENT_METADATA.get_or_fetch_with_refresh(
                force_refresh,
                || async {
                    let url = format!("{NEOFORGE_MAVEN_URL}maven-metadata.xml");
                    fetch_maven_metadata(&url, fetch_semaphore).await
                },
            )
        },
        || {
            NEOFORGE_LEGACY_METADATA.get_or_fetch_with_refresh(
                force_refresh,
                || async {
                    let url = format!(
                        "{NEOFORGE_LEGACY_MAVEN_URL}maven-metadata.xml"
                    );
                    fetch_maven_metadata(&url, fetch_semaphore).await
                },
            )
        },
    )
    .await?;
    let manifest = neoforge_manifest(current, legacy);
    Ok(if let Some(game_version) = game_version {
        scope_manifest(manifest, game_version)
    } else {
        manifest
    })
}

async fn fetch_neoforge_indexes<C, L>(
    game_version: Option<&str>,
    current: impl FnOnce() -> C,
    legacy: impl FnOnce() -> L,
) -> crate::Result<(MavenMetadata, MavenMetadata)>
where
    C: Future<Output = crate::Result<MavenMetadata>>,
    L: Future<Output = crate::Result<MavenMetadata>>,
{
    match game_version {
        Some("1.20.1") => Ok((MavenMetadata::default(), legacy().await?)),
        Some(_) => Ok((current().await?, MavenMetadata::default())),
        None => tokio::try_join!(current(), legacy()),
    }
}

fn neoforge_manifest(
    current: MavenMetadata,
    legacy: MavenMetadata,
) -> Manifest {
    let current_release = current.versioning.release;
    let legacy_release = legacy.versioning.release;
    let mut grouped: HashMap<String, Vec<LoaderVersion>> = HashMap::new();

    for version in current.versioning.versions.values {
        let Some(game_version) = neoforge_game_version(&version) else {
            continue;
        };
        grouped
            .entry(game_version)
            .or_default()
            .push(LoaderVersion {
                id: version.clone(),
                url: format!(
                    "{NEOFORGE_MAVEN_URL}{version}/neoforge-{version}-installer.jar"
                ),
                stable: version == current_release
                    || is_stable_version(&version),
                profile_source: LoaderProfileSource::Installer,
                fallback_url: Some(fallback_profile_url(
                    "neo", 0, &version,
                )),
            });
    }

    for full_version in legacy.versioning.versions.values {
        let Some((game_version, loader_version)) = full_version.split_once('-')
        else {
            continue;
        };
        grouped
            .entry(game_version.to_string())
            .or_default()
            .push(LoaderVersion {
                id: loader_version.to_string(),
                url: format!(
                    "{NEOFORGE_LEGACY_MAVEN_URL}{full_version}/forge-{full_version}-installer.jar"
                ),
                stable: full_version == legacy_release
                    || is_stable_version(loader_version),
                profile_source: LoaderProfileSource::Installer,
                fallback_url: Some(fallback_profile_url(
                    "neo",
                    0,
                    loader_version,
                )),
            });
    }

    grouped_manifest(grouped)
}

fn grouped_manifest(
    mut grouped: HashMap<String, Vec<LoaderVersion>>,
) -> Manifest {
    let mut game_versions = grouped
        .drain()
        .map(|(game_version, mut loaders)| {
            loaders
                .sort_by(|left, right| compare_versions(&right.id, &left.id));
            Version {
                id: game_version,
                stable: true,
                version_group: None,
                loaders,
            }
        })
        .collect::<Vec<_>>();
    game_versions.sort_by(|left, right| compare_versions(&right.id, &left.id));
    Manifest {
        game_versions,
        version_groups: Vec::new(),
    }
}

fn scope_manifest(manifest: Manifest, game_version: &str) -> Manifest {
    let has_explicit_game_version = manifest
        .game_versions
        .iter()
        .any(|version| version.id == game_version);
    let scoped = manifest
        .game_versions
        .iter()
        .find(|version| version.id == game_version)
        .or_else(|| {
            manifest.game_versions.iter().find(|version| {
                version.id.replace(DUMMY_REPLACE_STRING, game_version)
                    == game_version
            })
        });

    let (stable, loaders) = scoped.map_or((true, Vec::new()), |version| {
        let loaders = if version.id == DUMMY_REPLACE_STRING
            && !has_explicit_game_version
        {
            Vec::new()
        } else if let Some(group_id) = &version.version_group {
            manifest
                .version_groups
                .iter()
                .find(|group| group.id == *group_id)
                .map(|group| group.loaders.clone())
                .unwrap_or_default()
        } else {
            version.loaders.clone()
        };
        (version.stable, loaders)
    });

    Manifest {
        game_versions: vec![Version {
            id: game_version.to_string(),
            stable,
            version_group: None,
            loaders,
        }],
        version_groups: Vec::new(),
    }
}

async fn fetch_maven_metadata(
    url: &str,
    fetch_semaphore: &FetchSemaphore,
) -> crate::Result<MavenMetadata> {
    fetch_metadata(
        url,
        |bytes| parse_maven_metadata(url, bytes),
        fetch_semaphore,
    )
    .await
}

fn parse_maven_metadata(
    url: &str,
    bytes: &[u8],
) -> crate::Result<MavenMetadata> {
    let xml = std::str::from_utf8(bytes).map_err(|error| {
        crate::ErrorKind::OtherError(format!(
            "Maven metadata at {url} was not UTF-8: {error}"
        ))
        .as_error()
    })?;
    let metadata: MavenMetadata =
        quick_xml::de::from_str(xml).map_err(|error| {
            crate::ErrorKind::OtherError(format!(
                "Failed to parse Maven metadata at {url}: {error}"
            ))
            .as_error()
        })?;
    if metadata.versioning.versions.values.is_empty() {
        return Err(crate::ErrorKind::OtherError(format!(
            "Maven metadata at {url} contained no versions"
        ))
        .as_error());
    }
    Ok(metadata)
}

async fn fetch_loader_json<T: serde::de::DeserializeOwned>(
    url: &str,
    fetch_semaphore: &FetchSemaphore,
) -> crate::Result<T> {
    fetch_metadata(
        url,
        |bytes| serde_json::from_slice(bytes).map_err(Into::into),
        fetch_semaphore,
    )
    .await
}

fn fallback_profile_url(
    loader: &str,
    format_version: usize,
    version: &str,
) -> String {
    format!(
        "{}{loader}/v{format_version}/versions/{version}.json",
        env!("MODRINTH_LAUNCHER_META_URL")
    )
}

fn neoforge_game_version(version: &str) -> Option<String> {
    if let Some(snapshot) = version.strip_prefix("0.") {
        let snapshot = snapshot.rsplit_once('.')?.0;
        return Some(format!("1.0.{snapshot}"));
    }

    let components = version.split('.').collect::<Vec<_>>();
    let major = components.first()?.parse::<u32>().ok()?;
    let minor = components.get(1)?.parse::<u32>().ok()?;
    if major >= 26 {
        let patch = components.get(2)?.parse::<u32>().ok()?;
        if patch == 0 {
            Some(format!("{major}.{minor}"))
        } else {
            Some(format!("{major}.{minor}.{patch}"))
        }
    } else if minor == 0 {
        Some(format!("1.{major}"))
    } else {
        Some(format!("1.{major}.{minor}"))
    }
}

fn is_stable_version(version: &str) -> bool {
    let version = version.to_ascii_lowercase();
    !["alpha", "beta", "rc", "snapshot", "pre"]
        .iter()
        .any(|marker| version.contains(marker))
}

fn compare_versions(left: &str, right: &str) -> Ordering {
    let mut left = left.as_bytes().iter().peekable();
    let mut right = right.as_bytes().iter().peekable();
    loop {
        match (left.peek(), right.peek()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Greater,
            (Some(_), None) => return Ordering::Less,
            (Some(left_byte), Some(right_byte)) => {
                let left_is_digit = left_byte.is_ascii_digit();
                let right_is_digit = right_byte.is_ascii_digit();
                if left_is_digit != right_is_digit {
                    return left_byte.cmp(right_byte);
                }

                let mut left_part = Vec::new();
                let mut right_part = Vec::new();
                while left
                    .peek()
                    .is_some_and(|byte| byte.is_ascii_digit() == left_is_digit)
                {
                    left_part.push(**left.peek().unwrap());
                    left.next();
                }
                while right
                    .peek()
                    .is_some_and(|byte| byte.is_ascii_digit() == right_is_digit)
                {
                    right_part.push(**right.peek().unwrap());
                    right.next();
                }

                let ordering = if left_is_digit {
                    left_part
                        .len()
                        .cmp(&right_part.len())
                        .then_with(|| left_part.cmp(&right_part))
                } else {
                    left_part.cmp(&right_part)
                };
                if ordering != Ordering::Equal {
                    return ordering;
                }
            }
        }
    }
}

pub(crate) async fn resolve_loader_profile(
    state: &State,
    game_version: &str,
    loader_version: &LoaderVersion,
) -> crate::Result<PartialVersionInfo> {
    let primary_url = loader_version
        .url
        .replace(DUMMY_REPLACE_STRING, game_version);
    let primary = match loader_version.profile_source {
        LoaderProfileSource::Json => {
            fetch_json(
                Method::GET,
                &primary_url,
                None,
                None,
                None,
                &state.api_semaphore,
            )
            .await
        }
        LoaderProfileSource::Installer => {
            resolve_installer_profile(state, &primary_url).await
        }
        LoaderProfileSource::LiteLoader => {
            resolve_liteloader_profile(
                state,
                game_version,
                &loader_version.id,
                &primary_url,
            )
            .await
        }
        LoaderProfileSource::Babric => {
            resolve_babric_profile(
                state,
                game_version,
                &loader_version.id,
                &primary_url,
            )
            .await
        }
    };

    match primary {
        Ok(profile) => Ok(profile),
        Err(primary_error) => {
            let Some(fallback_url) = &loader_version.fallback_url else {
                return Err(primary_error);
            };
            let fallback_url =
                fallback_url.replace(DUMMY_REPLACE_STRING, game_version);
            tracing::warn!(
                loader_version = loader_version.id,
                primary_url,
                fallback_url,
                error = %primary_error,
                "Official loader profile failed; using launcher-meta fallback"
            );
            match loader_version.profile_source {
                LoaderProfileSource::LiteLoader => {
                    resolve_liteloader_profile(
                        state,
                        game_version,
                        &loader_version.id,
                        &fallback_url,
                    )
                    .await
                }
                LoaderProfileSource::Babric => {
                    resolve_babric_profile(
                        state,
                        game_version,
                        &loader_version.id,
                        &fallback_url,
                    )
                    .await
                }
                _ => {
                    fetch_json(
                        Method::GET,
                        &fallback_url,
                        None,
                        None,
                        None,
                        &state.api_semaphore,
                    )
                    .await
                }
            }
        }
    }
}

async fn resolve_babric_profile(
    state: &State,
    game_version: &str,
    loader_version: &str,
    profile_url: &str,
) -> crate::Result<PartialVersionInfo> {
    if game_version != BABRIC_GAME_VERSION {
        return Err(crate::ErrorKind::InputError(format!(
            "Babric does not support Minecraft {game_version}"
        ))
        .into());
    }

    match fetch_json::<PartialVersionInfo>(
        Method::GET,
        profile_url,
        None,
        None,
        None,
        &state.api_semaphore,
    )
    .await
    {
        Ok(profile) => Ok(profile),
        Err(error) => {
            tracing::warn!(
                game_version,
                loader_version,
                error = %error,
                "Babric profile endpoint is unavailable; synthesizing the profile from Glass Maven"
            );
            build_babric_profile(state, game_version, loader_version).await
        }
    }
}

async fn build_babric_profile(
    state: &State,
    game_version: &str,
    loader_version: &str,
) -> crate::Result<PartialVersionInfo> {
    let loader_metadata_url = format!(
        "{BABRIC_LOADER_METADATA_URL}{loader_version}/fabric-loader-{loader_version}.json"
    );
    let polyfill = fetch_json::<serde_json::Value>(
        Method::GET,
        BABRIC_POLYFILL_VERSION_URL,
        None,
        None,
        None,
        &state.api_semaphore,
    )
    .await?;
    let launcher_metadata = fetch_json::<BabricLauncherMetadata>(
        Method::GET,
        &loader_metadata_url,
        None,
        None,
        None,
        &state.api_semaphore,
    )
    .await?;

    let mut libraries = launcher_metadata.libraries.common;
    libraries.extend(launcher_metadata.libraries.client);
    libraries.push(babric_library(
        "babric:intermediary-upstream:b1.7.3",
        BABRIC_MAVEN_URL,
    ));
    libraries.push(babric_library(
        &format!("babric:fabric-loader:{loader_version}"),
        BABRIC_MAVEN_URL,
    ));
    libraries.extend(babric_logging_libraries());
    libraries.extend(babric_lwjgl_libraries(&polyfill)?);

    let minecraft_arguments = polyfill
        .get("minecraftArguments")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("${auth_player_name} ${auth_session} --gameDir ${game_directory} --assetsDir ${game_assets}");
    let game_arguments = minecraft_arguments
        .split_whitespace()
        .map(|argument| Argument::Normal(argument.to_string()))
        .collect::<Vec<_>>();
    let jvm_arguments = vec![
        Argument::Normal(
            "-DFabricMcEmu= net.minecraft.client.main.Main ".to_string(),
        ),
        Argument::Normal("-cp".to_string()),
        Argument::Normal("${classpath}".to_string()),
        Argument::Normal(
            "-Djava.library.path=${natives_directory}".to_string(),
        ),
    ];
    let now = chrono::Utc::now();

    Ok(PartialVersionInfo {
        id: format!("fabric-loader-{loader_version}-{game_version}"),
        inherits_from: game_version.to_string(),
        release_time: now,
        time: now,
        main_class: Some(launcher_metadata.main_class.client),
        minecraft_arguments: None,
        arguments: Some(HashMap::from([
            (ArgumentType::Game, game_arguments),
            (ArgumentType::Jvm, jvm_arguments),
        ])),
        libraries,
        java_version: Some(JavaVersion {
            component: "jre-legacy".to_string(),
            major_version: launcher_metadata.min_java_version.unwrap_or(8),
        }),
        type_: VersionType::Release,
        data: None,
        processors: None,
    })
}

fn babric_library(name: &str, url: &str) -> Library {
    Library {
        downloads: None,
        extract: None,
        name: name.to_string(),
        url: Some(url.to_string()),
        natives: None,
        rules: None,
        checksums: None,
        include_in_classpath: true,
        downloadable: true,
    }
}

fn babric_logging_libraries() -> Vec<Library> {
    [
        ("babric:log4j-config:1.0.0", BABRIC_MAVEN_URL),
        (
            "net.minecrell:terminalconsoleappender:1.2.0",
            "https://repo1.maven.org/maven2/",
        ),
        (
            "org.slf4j:slf4j-api:1.8.0-beta4",
            "https://libraries.minecraft.net/",
        ),
        (
            "org.apache.logging.log4j:log4j-slf4j18-impl:2.16.0",
            "https://libraries.minecraft.net/",
        ),
        (
            "org.apache.logging.log4j:log4j-api:2.16.0",
            "https://libraries.minecraft.net/",
        ),
        (
            "org.apache.logging.log4j:log4j-core:2.16.0",
            "https://libraries.minecraft.net/",
        ),
        (
            "com.google.code.gson:gson:2.8.9",
            "https://libraries.minecraft.net/",
        ),
        (
            "com.google.guava:guava:31.0.1-jre",
            "https://libraries.minecraft.net/",
        ),
        (
            "com.google.guava:failureaccess:1.0.1",
            "https://libraries.minecraft.net/",
        ),
        (
            "org.apache.commons:commons-lang3:3.12.0",
            "https://libraries.minecraft.net/",
        ),
        (
            "commons-io:commons-io:2.11.0",
            "https://libraries.minecraft.net/",
        ),
        (
            "commons-codec:commons-codec:1.15",
            "https://libraries.minecraft.net/",
        ),
    ]
    .into_iter()
    .map(|(name, url)| babric_library(name, url))
    .collect()
}

fn babric_lwjgl_libraries(
    polyfill: &serde_json::Value,
) -> crate::Result<Vec<Library>> {
    let Some(entries) = polyfill
        .get("libraries")
        .and_then(serde_json::Value::as_array)
    else {
        return Ok(Vec::new());
    };
    entries
        .iter()
        .filter(|library| {
            let Some(name) =
                library.get("name").and_then(serde_json::Value::as_str)
            else {
                return false;
            };
            name.contains("lwjgl") && name.contains("-babric.")
        })
        .cloned()
        .map(serde_json::from_value)
        .collect::<Result<Vec<_>, _>>()
        .map_err(Into::into)
}

async fn resolve_liteloader_profile(
    state: &State,
    game_version: &str,
    loader_version: &str,
    url: &str,
) -> crate::Result<PartialVersionInfo> {
    let metadata: serde_json::Value =
        fetch_json(Method::GET, url, None, None, None, &state.api_semaphore)
            .await?;
    build_liteloader_profile(&metadata, game_version, loader_version)
}

fn build_liteloader_profile(
    metadata: &serde_json::Value,
    game_version: &str,
    loader_version: &str,
) -> crate::Result<PartialVersionInfo> {
    let game_metadata = metadata
        .get("versions")
        .and_then(|versions| versions.get(game_version))
        .ok_or_else(|| {
            crate::ErrorKind::InputError(format!(
                "LiteLoader does not support Minecraft {game_version}"
            ))
            .as_error()
        })?;
    let entry = liteloader_latest_entry(game_metadata).ok_or_else(|| {
        crate::ErrorKind::OtherError(format!(
            "LiteLoader metadata for {game_version} has no latest build"
        ))
        .as_error()
    })?;
    let actual_version = entry
        .get("version")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            crate::ErrorKind::OtherError(
                "LiteLoader metadata has no version".to_string(),
            )
            .as_error()
        })?;
    if actual_version != loader_version {
        return Err(crate::ErrorKind::InputError(format!(
            "LiteLoader version {loader_version} is not available for Minecraft {game_version}"
        ))
        .into());
    }
    let tweak_class = entry
        .get("tweakClass")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            crate::ErrorKind::OtherError(
                "LiteLoader metadata has no tweakClass".to_string(),
            )
            .as_error()
        })?;
    let mut libraries = entry
        .get("libraries")
        .cloned()
        .map(serde_json::from_value::<Vec<Library>>)
        .transpose()?
        .unwrap_or_default();
    libraries.push(Library {
        downloads: None,
        extract: None,
        name: format!("com.mumfrey:liteloader:{actual_version}"),
        url: Some("https://dl.liteloader.com/versions/".to_string()),
        natives: None,
        rules: None,
        checksums: None,
        include_in_classpath: true,
        downloadable: true,
    });
    let timestamp = entry
        .get("timestamp")
        .and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_str()?.parse::<i64>().ok())
        })
        .unwrap_or(0);
    let time = chrono::DateTime::from_timestamp(timestamp, 0)
        .unwrap_or(chrono::DateTime::UNIX_EPOCH);

    Ok(PartialVersionInfo {
        id: format!("{game_version}-LiteLoader"),
        inherits_from: game_version.to_string(),
        release_time: time,
        time,
        main_class: Some("net.minecraft.launchwrapper.Launch".to_string()),
        minecraft_arguments: None,
        arguments: Some(HashMap::from([(
            daedalus::minecraft::ArgumentType::Game,
            vec![
                daedalus::minecraft::Argument::Normal(
                    "--tweakClass".to_string(),
                ),
                daedalus::minecraft::Argument::Normal(tweak_class.to_string()),
            ],
        )])),
        libraries,
        java_version: Some(JavaVersion {
            component: "jre-legacy".to_string(),
            major_version: 8,
        }),
        type_: daedalus::minecraft::VersionType::Release,
        data: None,
        processors: None,
    })
}

async fn resolve_installer_profile(
    state: &State,
    installer_url: &str,
) -> crate::Result<PartialVersionInfo> {
    let bytes =
        fetch(installer_url, None, None, None, &state.api_semaphore).await?;
    let installer_url = installer_url.to_string();
    let parsed = tokio::task::spawn_blocking(move || {
        parse_installer(bytes.to_vec(), &installer_url)
    })
    .await??;
    persist_installer_artifacts(state, &parsed.artifacts).await?;
    Ok(parsed.profile)
}

pub(crate) async fn ensure_installer_artifacts(
    state: &State,
    version: &daedalus::minecraft::VersionInfo,
) -> crate::Result<()> {
    let Some(data) = &version.data else {
        return Ok(());
    };
    let Some(installer_url) = data.get(INSTALLER_URL_KEY) else {
        return Ok(());
    };
    let entries = data
        .iter()
        .filter(|(key, _)| key.starts_with(INSTALLER_ENTRY_KEY_PREFIX))
        .filter_map(|(_, entry)| entry.client.split_once('|'))
        .map(|(coordinate, archive_path)| {
            (coordinate.to_string(), archive_path.to_string())
        })
        .collect::<Vec<_>>();
    let mut missing = Vec::new();
    for (coordinate, archive_path) in entries {
        let relative = daedalus::get_path_from_artifact(&coordinate)?;
        if !state.directories.libraries_dir().join(relative).exists() {
            missing.push((coordinate, archive_path));
        }
    }
    if missing.is_empty() {
        return Ok(());
    }

    let bytes = fetch(
        &installer_url.client,
        None,
        None,
        None,
        &state.api_semaphore,
    )
    .await?;
    let artifacts = tokio::task::spawn_blocking(move || {
        extract_installer_artifacts(bytes.to_vec(), &missing)
    })
    .await??;
    persist_installer_artifacts(state, &artifacts).await
}

fn parse_installer(
    bytes: Vec<u8>,
    installer_url: &str,
) -> crate::Result<ParsedInstaller> {
    let mut archive =
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|error| {
            crate::ErrorKind::OtherError(format!(
                "Failed to open loader installer: {error}"
            ))
            .as_error()
        })?;
    let profile_json = read_zip_entry(&mut archive, "install_profile.json")?;
    let profile_value: serde_json::Value =
        serde_json::from_slice(&profile_json)?;
    let version_json = if let Ok(version_json) =
        read_zip_entry(&mut archive, "version.json")
    {
        version_json
    } else if let Some(path) =
        profile_value.get("json").and_then(|value| value.as_str())
    {
        read_zip_entry(&mut archive, path.trim_start_matches('/'))?
    } else if let Some(version_info) = profile_value.get("versionInfo") {
        serde_json::to_vec(version_info)?
    } else {
        return Err(crate::ErrorKind::OtherError(
            "Loader installer contains no version.json, profile json, or versionInfo"
                .to_string(),
        )
        .as_error());
    };
    let mut installer: InstallerProfile =
        serde_json::from_value(profile_value)?;
    let mut version_value: serde_json::Value =
        serde_json::from_slice(&version_json)?;
    if version_value.get("inheritsFrom").is_none() {
        let minecraft = installer.minecraft.clone().ok_or_else(|| {
            crate::ErrorKind::OtherError(
                "Loader installer version has no inheritsFrom or minecraft version"
                    .to_string(),
            )
            .as_error()
        })?;
        version_value
            .as_object_mut()
            .ok_or_else(|| {
                crate::ErrorKind::OtherError(
                    "Loader installer version is not a JSON object".to_string(),
                )
                .as_error()
            })?
            .insert(
                "inheritsFrom".to_string(),
                serde_json::Value::String(minecraft),
            );
    }
    let mut partial: PartialVersionInfo =
        serde_json::from_value(version_value)?;

    for library in &mut installer.libraries {
        library.include_in_classpath = false;
    }

    let has_processors = !installer.processors.is_empty();

    for library in &mut partial.libraries {
        let library_name = library.name.clone();

        if let Some(artifact) = library
            .downloads
            .as_mut()
            .and_then(|downloads| downloads.artifact.as_mut())
            && artifact.url.is_empty()
        {
            if !has_processors
                && let Some(full_version) =
                    library_name.strip_prefix("net.minecraftforge:forge:")
            {
                artifact.url = format!(
                    "{FORGE_MAVEN_URL}{full_version}/forge-{full_version}-universal.jar"
                );
                library.downloadable = true;
            } else {
                library.downloadable = false;
            }
        }
    }

    let existing = partial
        .libraries
        .iter()
        .map(|library| library.name.clone())
        .collect::<HashSet<_>>();
    partial.libraries.extend(
        installer
            .libraries
            .into_iter()
            .filter(|library| !existing.contains(&library.name)),
    );
    partial.processors = Some(installer.processors);

    let requests = installer
        .data
        .iter()
        .flat_map(|(key, entry)| {
            [("client", &entry.client), ("server", &entry.server)]
                .into_iter()
                .filter_map(move |(side, value)| {
                    value
                        .strip_prefix('/')
                        .map(|path| (key.clone(), side, path.to_string()))
                })
        })
        .collect::<Vec<_>>();
    let mut artifacts = Vec::new();
    for (index, (key, side, archive_path)) in requests.into_iter().enumerate() {
        let artifact_data = read_zip_entry(&mut archive, &archive_path)?;
        let extension = std::path::Path::new(&archive_path)
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("bin");
        let coordinate = format!(
            "com.axolotl.loader-installer:embedded:{}:{}-{}@{}",
            sanitize_coordinate_part(&partial.id),
            sanitize_coordinate_part(&key),
            side,
            sanitize_coordinate_part(extension),
        );
        let entry = installer.data.get_mut(&key).unwrap();
        let replacement = format!("[{coordinate}]");
        if side == "client" {
            entry.client = replacement;
        } else {
            entry.server = replacement;
        }
        installer.data.insert(
            format!("{INSTALLER_ENTRY_KEY_PREFIX}{index}"),
            SidedDataEntry {
                client: format!("{coordinate}|{archive_path}"),
                server: format!("{coordinate}|{archive_path}"),
            },
        );
        partial.libraries.push(local_library(coordinate.clone()));
        artifacts.push(InstallerArtifact {
            coordinate,
            data: artifact_data,
        });
    }
    if let Some(install) = installer.install {
        let archive_path = install.file_path.trim_start_matches('/');
        let artifact_data = read_zip_entry(&mut archive, archive_path)?;
        let index = installer.data.len();
        installer.data.insert(
            format!("{INSTALLER_ENTRY_KEY_PREFIX}{index}"),
            SidedDataEntry {
                client: format!("{}|{archive_path}", install.path),
                server: format!("{}|{archive_path}", install.path),
            },
        );
        if !partial
            .libraries
            .iter()
            .any(|library| library.name == install.path)
        {
            partial.libraries.push(local_library(install.path.clone()));
        }
        artifacts.push(InstallerArtifact {
            coordinate: install.path,
            data: artifact_data,
        });
    }
    let mut embedded_maven_entries = Vec::new();
    for path in archive.file_names() {
        let path = path.map_err(|error| {
            crate::ErrorKind::InputError(format!(
                "Failed to read installer archive entry name: {error}"
            ))
        })?;
        if let Some(coordinate) = coordinate_from_maven_path(&path) {
            embedded_maven_entries.push((coordinate, path.into_owned()));
        }
    }
    for (coordinate, archive_path) in embedded_maven_entries {
        if artifacts
            .iter()
            .any(|artifact| artifact.coordinate == coordinate)
        {
            continue;
        }
        let artifact_data = read_zip_entry(&mut archive, &archive_path)?;
        let index = installer.data.len();
        installer.data.insert(
            format!("{INSTALLER_ENTRY_KEY_PREFIX}{index}"),
            SidedDataEntry {
                client: format!("{coordinate}|{archive_path}"),
                server: format!("{coordinate}|{archive_path}"),
            },
        );
        artifacts.push(InstallerArtifact {
            coordinate,
            data: artifact_data,
        });
    }
    installer.data.insert(
        INSTALLER_URL_KEY.to_string(),
        SidedDataEntry {
            client: installer_url.to_string(),
            server: installer_url.to_string(),
        },
    );
    partial.data = Some(installer.data);

    Ok(ParsedInstaller {
        profile: partial,
        artifacts,
    })
}

fn coordinate_from_maven_path(path: &str) -> Option<String> {
    let path = path.strip_prefix("maven/")?;
    let components = path.split('/').collect::<Vec<_>>();
    if components.len() < 4 {
        return None;
    }
    let file_name = components.last()?;
    let version = components.get(components.len() - 2)?;
    let artifact = components.get(components.len() - 3)?;
    let group = components[..components.len() - 3].join(".");
    let prefix = format!("{artifact}-{version}");
    let suffix = file_name.strip_prefix(&prefix)?;
    let (classifier, extension) = suffix.rsplit_once('.')?;
    let classifier = classifier
        .strip_prefix('-')
        .filter(|value| !value.is_empty());
    Some(match classifier {
        Some(classifier) => {
            format!("{group}:{artifact}:{version}:{classifier}@{extension}")
        }
        None if extension == "jar" => format!("{group}:{artifact}:{version}"),
        None => format!("{group}:{artifact}:{version}@{extension}"),
    })
}

fn extract_installer_artifacts(
    bytes: Vec<u8>,
    entries: &[(String, String)],
) -> crate::Result<Vec<InstallerArtifact>> {
    let mut archive =
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|error| {
            crate::ErrorKind::OtherError(format!(
                "Failed to open cached loader installer: {error}"
            ))
            .as_error()
        })?;
    entries
        .iter()
        .map(|(coordinate, archive_path)| {
            Ok(InstallerArtifact {
                coordinate: coordinate.clone(),
                data: read_zip_entry(&mut archive, archive_path)?,
            })
        })
        .collect()
}

fn read_zip_entry<R: Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    path: &str,
) -> crate::Result<Vec<u8>> {
    let mut entry = archive.by_name(path).map_err(|error| {
        crate::ErrorKind::OtherError(format!(
            "Loader installer is missing {path}: {error}"
        ))
        .as_error()
    })?;
    let mut data = Vec::new();
    entry.read_to_end(&mut data)?;
    Ok(data)
}

async fn persist_installer_artifacts(
    state: &State,
    artifacts: &[InstallerArtifact],
) -> crate::Result<()> {
    for artifact in artifacts {
        let relative = daedalus::get_path_from_artifact(&artifact.coordinate)?;
        let path = state.directories.libraries_dir().join(relative);
        if let Some(parent) = path.parent() {
            io::create_dir_all(parent).await?;
        }
        io::write(path, &artifact.data).await?;
    }
    Ok(())
}

fn local_library(name: String) -> Library {
    Library {
        downloads: None,
        extract: None,
        name,
        url: None,
        natives: None,
        rules: None,
        checksums: None,
        include_in_classpath: false,
        downloadable: false,
    }
}

fn sanitize_coordinate_part(value: &str) -> String {
    value
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
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    #[test]
    fn forge_fallback_requires_exact_game_and_loader_version() {
        let manifest = forge_manifest(
            maven_metadata(
                "",
                &["1.20.1-47.4.28", "1.20.1-47.4.22", "1.21.1-52.1.0"],
            ),
            ForgePromotions::default(),
        );
        assert_eq!(
            find_exact_forge_version(manifest.clone(), "1.20.1", "47.4.28")
                .unwrap()
                .id,
            "47.4.28"
        );
        assert!(
            find_exact_forge_version(manifest.clone(), "1.21.1", "47.4.28")
                .is_none()
        );
        assert!(
            find_exact_forge_version(manifest, "1.20.1", "47.4.29").is_none()
        );
    }

    fn maven_metadata(release: &str, versions: &[&str]) -> MavenMetadata {
        MavenMetadata {
            versioning: MavenVersioning {
                release: release.to_string(),
                versions: MavenVersions {
                    values: versions
                        .iter()
                        .map(|version| version.to_string())
                        .collect(),
                },
            },
        }
    }

    #[tokio::test]
    async fn forced_forge_refresh_replaces_fresh_memory_catalog() {
        let cache = LoaderManifestCache::default();
        let old = || async {
            Ok(CachedLoaderManifest {
                loader: "forge".into(),
                manifest: forge_manifest(
                    maven_metadata("", &["1.20.1-47.4.22"]),
                    ForgePromotions::default(),
                ),
            })
        };
        cache
            .get_or_fetch("forge", Some("1.20.1"), |_| old())
            .await
            .unwrap();
        let fresh = cache
            .get_or_fetch_with_refresh(
                "forge",
                Some("1.20.1"),
                true,
                |scope| async move {
                    assert_eq!(scope, None);
                    Ok(CachedLoaderManifest {
                        loader: "forge".into(),
                        manifest: forge_manifest(
                            maven_metadata(
                                "",
                                &["1.20.1-47.4.28", "1.21.1-52.1.0"],
                            ),
                            ForgePromotions::default(),
                        ),
                    })
                },
            )
            .await
            .unwrap();
        assert_eq!(forge_loaders(&fresh.manifest, "1.20.1")[0].id, "47.4.28");
        let other = cache
            .get_or_fetch("forge", Some("1.21.1"), |_| async {
                panic!(
                    "refreshed full catalog should serve other game versions"
                )
            })
            .await
            .unwrap();
        assert_eq!(forge_loaders(&other.manifest, "1.21.1")[0].id, "52.1.0");
    }

    #[tokio::test]
    async fn forced_metadata_refresh_reports_failure_without_destroying_cache()
    {
        let cache = MetadataCache::default();
        cache.get_or_fetch(|| async { Ok(22) }).await.unwrap();
        let failed = cache
            .get_or_fetch_with_refresh(true, || async {
                Err(crate::ErrorKind::OtherError("refresh failed".into())
                    .as_error())
            })
            .await;
        assert!(failed.unwrap_err().to_string().contains("refresh failed"));
        assert_eq!(
            cache
                .get_or_fetch(|| async { panic!("keep good cache") })
                .await
                .unwrap(),
            22
        );
        let retry = cache
            .get_or_fetch_with_refresh(true, || async {
                panic!("failure backoff should prevent a retry storm")
            })
            .await;
        assert!(retry.is_err());
    }

    #[tokio::test]
    async fn simultaneous_forced_refreshes_share_the_inflight_request() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let cache = MetadataCache::default();
        cache.get_or_fetch(|| async { Ok(22) }).await.unwrap();
        let calls = AtomicUsize::new(0);
        let fetch = || async {
            calls.fetch_add(1, Ordering::SeqCst);
            tokio::task::yield_now().await;
            Ok(28)
        };
        let (first, second) = tokio::join!(
            cache.get_or_fetch_with_refresh(true, fetch),
            cache.get_or_fetch_with_refresh(true, fetch),
        );
        assert_eq!(first.unwrap(), 28);
        assert_eq!(second.unwrap(), 28);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn database_bypass_propagates_through_loader_memory_cache() {
        let entry = LOADER_MANIFESTS
            .entry(
                daedalus::modded::loader_manifest_metadata("forge").cache_key,
            )
            .await;
        *entry.value.lock().await = Some((
            Instant::now(),
            CachedLoaderManifest {
                loader: "forge".into(),
                manifest: forge_manifest(
                    maven_metadata("", &["1.20.1-47.4.22"]),
                    ForgePromotions::default(),
                ),
            },
        ));
        *entry.failure.lock().await = Some((
            Instant::now(),
            Arc::new(crate::ErrorKind::OtherError(
                "forced refresh reached memory cache".into(),
            )),
        ));
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let semaphore = FetchSemaphore(tokio::sync::Semaphore::new(0));
        let result = tokio::time::timeout(
            Duration::from_secs(2),
            crate::state::CachedEntry::get_loader_manifest(
                &daedalus::modded::loader_manifest_metadata_for_game(
                    "forge", "1.20.1",
                )
                .cache_key,
                Some(crate::state::CacheBehaviour::Bypass),
                &pool,
                &semaphore,
            ),
        )
        .await;
        *entry.value.lock().await = None;
        *entry.failure.lock().await = None;
        pool.close().await;
        let error = result.unwrap().unwrap_err();
        assert!(
            error
                .to_string()
                .contains("forced refresh reached memory cache")
        );
    }

    #[test]
    fn scoped_catalog_prefers_explicit_game_over_earlier_placeholder() {
        let mut full = forge_manifest(
            maven_metadata("", &["1.20.1-47.4.28"]),
            ForgePromotions::default(),
        );
        let mut generic = full.game_versions[0].clone();
        generic.id = DUMMY_REPLACE_STRING.into();
        generic.loaders.clear();
        full.game_versions.insert(0, generic);
        let scoped = scope_manifest(full, "1.20.1");
        assert_eq!(forge_loaders(&scoped, "1.20.1")[0].id, "47.4.28");
    }

    #[tokio::test]
    async fn catalog_loaders_share_downloads_without_mixing_game_versions() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let cache = LoaderManifestCache::default();
        for loader in [
            "forge",
            "legacy_fabric",
            "babric",
            "lite_loader",
            "cleanroom",
        ] {
            let calls = AtomicUsize::new(0);
            let fetch = |scope| {
                assert_eq!(scope, None);
                async {
                    calls.fetch_add(1, Ordering::Relaxed);
                    tokio::task::yield_now().await;
                    Ok(CachedLoaderManifest {
                        loader: loader.into(),
                        manifest: forge_manifest(
                            maven_metadata(
                                "",
                                &["1.20.1-47.1.0", "1.21.1-52.1.0"],
                            ),
                            ForgePromotions::default(),
                        ),
                    })
                }
            };
            let (full, first, second) = tokio::join!(
                cache.get_or_fetch(loader, None, fetch),
                cache.get_or_fetch(loader, Some("1.20.1"), fetch),
                cache.get_or_fetch(loader, Some("1.21.1"), fetch),
            );
            assert_eq!(full.unwrap().manifest.game_versions.len(), 2);
            let first = first.unwrap().manifest;
            let second = second.unwrap().manifest;
            assert_eq!(first.game_versions.len(), 1);
            assert_eq!(first.game_versions[0].id, "1.20.1");
            assert_eq!(first.game_versions[0].loaders[0].id, "47.1.0");
            assert_eq!(second.game_versions[0].id, "1.21.1");
            assert_eq!(second.game_versions[0].loaders[0].id, "52.1.0");
            let unsupported = cache
                .get_or_fetch(loader, Some("1.2.3"), fetch)
                .await
                .unwrap();
            assert!(unsupported.manifest.game_versions[0].loaders.is_empty());
            assert_eq!(calls.load(Ordering::Relaxed), 1);
        }
    }

    #[tokio::test]
    async fn known_unsupported_versions_return_without_network_or_database_access()
     {
        let semaphore = FetchSemaphore(tokio::sync::Semaphore::new(0));
        for (loader, game) in [
            ("babric", "1.21.1"),
            ("cleanroom", "1.20.1"),
            ("lite_loader", "1.6.4"),
        ] {
            let result = tokio::time::timeout(
                Duration::from_secs(1),
                fetch_loader_manifest_official_first(
                    loader,
                    Some(game),
                    "https://invalid.test/manifest.json",
                    &semaphore,
                    false,
                ),
            )
            .await
            .unwrap()
            .unwrap();
            assert_eq!(result.manifest.game_versions[0].id, game);
            assert!(result.manifest.game_versions[0].loaders.is_empty());
        }
    }

    #[tokio::test]
    async fn scoped_loader_caches_preserve_compatibility_and_empty_results() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let cache = LoaderManifestCache::default();
        for loader in ["fabric", "quilt", "optifine", "neo"] {
            let calls = AtomicUsize::new(0);
            for (game, versions) in [
                ("1.21.1", vec![("compatible".into(), true)]),
                ("1.2.3", vec![]),
            ] {
                let fetch = |scope| {
                    assert_eq!(scope, Some(game));
                    calls.fetch_add(1, Ordering::Relaxed);
                    let versions = versions.clone();
                    async move {
                        Ok(CachedLoaderManifest {
                            loader: loader.into(),
                            manifest: meta_api_manifest_for_game(
                                game,
                                versions,
                                loader,
                                FABRIC_META_URL,
                                0,
                            ),
                        })
                    }
                };
                let first = cache
                    .get_or_fetch(loader, Some(game), fetch)
                    .await
                    .unwrap();
                let repeated = cache
                    .get_or_fetch(loader, Some(game), |_| async {
                        panic!("scoped cache should avoid repeated downloads")
                    })
                    .await
                    .unwrap();
                assert_eq!(
                    first.manifest.game_versions[0].loaders.len(),
                    versions.len()
                );
                assert_eq!(repeated.manifest.game_versions[0].id, game);
            }
            assert_eq!(calls.load(Ordering::Relaxed), 2);
        }
    }

    #[tokio::test]
    async fn manifest_cache_capacity_preserves_active_requests() {
        let cache = LoaderManifestCache::default();
        let active = cache.entry("active".into()).await;
        for index in 0..MAX_LOADER_METADATA_ENTRIES * 2 {
            cache.entry(format!("key-{index}")).await;
        }
        assert_eq!(
            cache.entries.lock().await.len(),
            MAX_LOADER_METADATA_ENTRIES
        );
        assert!(Arc::ptr_eq(&active, &cache.entry("active".into()).await));
    }

    #[tokio::test]
    async fn metadata_cache_coalesces_failures_and_retries_after_cooldown() {
        let cache = MetadataCache::<MavenMetadata>::default();
        let failed = cache
            .get_or_fetch(|| async {
                Err(crate::ErrorKind::OtherError("offline".into()).as_error())
            })
            .await;
        assert!(failed.is_err());
        assert!(
            cache
                .get_or_fetch(|| async {
                    panic!("failure cooldown must prevent another request")
                })
                .await
                .is_err()
        );
        cache.failure.lock().await.as_mut().unwrap().0 =
            Instant::now() - LOADER_METADATA_RETRY_DELAY;
        assert!(
            cache
                .get_or_fetch(|| async {
                    Ok(maven_metadata("", &["21.1.200"]))
                })
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn forge_fetches_catalog_and_promotions_concurrently() {
        let barrier = tokio::sync::Barrier::new(2);
        let (metadata, promotions) = tokio::time::timeout(
            Duration::from_secs(1),
            fetch_forge_indexes(
                async {
                    barrier.wait().await;
                    Ok(maven_metadata("", &["1.20.1-47.1.0"]))
                },
                async {
                    barrier.wait().await;
                    Ok(ForgePromotions {
                        promos: HashMap::from([(
                            "1.20.1-recommended".into(),
                            "47.1.0".into(),
                        )]),
                    })
                },
            ),
        )
        .await
        .unwrap()
        .unwrap();
        let manifest = forge_manifest(metadata, promotions);
        assert!(manifest.game_versions[0].loaders[0].stable);
    }

    #[tokio::test]
    async fn forge_keeps_catalog_when_promotions_stall_or_fail() {
        let (metadata, promotions) = tokio::time::timeout(
            FORGE_PROMOTIONS_TIMEOUT + Duration::from_secs(1),
            fetch_forge_indexes(
                async { Ok(maven_metadata("", &["1.20.1-47.1.0"])) },
                std::future::pending(),
            ),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(promotions.promos.is_empty());
        assert_eq!(
            forge_manifest(metadata, promotions).game_versions[0]
                .loaders
                .len(),
            1
        );
        assert!(
            fetch_forge_indexes(
                async { Ok(maven_metadata("", &["1.20.1-47.1.0"])) },
                async {
                    Err(crate::ErrorKind::OtherError("offline".into())
                        .as_error())
                },
            )
            .await
            .is_ok()
        );
        assert!(
            fetch_forge_indexes(
                async {
                    Err(crate::ErrorKind::OtherError("invalid XML".into())
                        .as_error())
                },
                async { Ok(ForgePromotions::default()) },
            )
            .await
            .is_err()
        );
    }

    #[tokio::test]
    async fn neoforge_scoped_requests_skip_the_unrelated_index() {
        for game_version in ["1.21.1", "26.3", "1.0.25w14craftmine"] {
            let (current, legacy) = fetch_neoforge_indexes(
                Some(game_version),
                || async { Ok(maven_metadata("21.1.200", &["21.1.200"])) },
                || async {
                    panic!("modern versions must not fetch the legacy index")
                },
            )
            .await
            .unwrap();
            assert_eq!(current.versioning.versions.values.len(), 1);
            assert!(legacy.versioning.versions.values.is_empty());
        }
        let (current, legacy) = fetch_neoforge_indexes(
            Some("1.20.1"),
            || async { panic!("1.20.1 must not fetch the modern index") },
            || async { Ok(maven_metadata("", &["1.20.1-47.1.106"])) },
        )
        .await
        .unwrap();
        assert!(current.versioning.versions.values.is_empty());
        let manifest = neoforge_manifest(current, legacy);
        assert_eq!(manifest.game_versions[0].id, "1.20.1");
        assert_eq!(manifest.game_versions[0].loaders[0].id, "47.1.106");
    }

    #[tokio::test]
    async fn neoforge_full_catalog_and_scoped_requests_share_index_downloads() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let current = MetadataCache::default();
        let legacy = MetadataCache::default();
        let current_calls = AtomicUsize::new(0);
        let legacy_calls = AtomicUsize::new(0);
        let fetch_current = || {
            current.get_or_fetch(|| async {
                current_calls.fetch_add(1, Ordering::Relaxed);
                tokio::task::yield_now().await;
                Ok(maven_metadata("21.1.200", &["21.1.200", "26.3.0.1"]))
            })
        };
        let fetch_legacy = || {
            legacy.get_or_fetch(|| async {
                legacy_calls.fetch_add(1, Ordering::Relaxed);
                tokio::task::yield_now().await;
                Ok(maven_metadata("", &["1.20.1-47.1.106"]))
            })
        };
        let (full, modern, old) = tokio::join!(
            fetch_neoforge_indexes(None, fetch_current, fetch_legacy),
            fetch_neoforge_indexes(Some("1.21.1"), fetch_current, fetch_legacy),
            fetch_neoforge_indexes(Some("1.20.1"), fetch_current, fetch_legacy),
        );
        let (current_metadata, legacy_metadata) = full.unwrap();
        assert_eq!(
            neoforge_manifest(current_metadata, legacy_metadata)
                .game_versions
                .len(),
            3
        );
        assert_eq!(modern.unwrap().0.versioning.versions.values.len(), 2);
        assert_eq!(old.unwrap().1.versioning.versions.values.len(), 1);
        fetch_neoforge_indexes(Some("26.3"), fetch_current, fetch_legacy)
            .await
            .unwrap();
        assert_eq!(current_calls.load(Ordering::Relaxed), 1);
        assert_eq!(legacy_calls.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn neoforge_index_refreshes_after_expiry_and_preserves_data_on_failure()
     {
        let cache = MetadataCache::default();
        *cache.value.lock().await = Some((
            Instant::now() - LOADER_METADATA_TTL,
            maven_metadata("21.1.200", &["21.1.200"]),
        ));
        let updated = cache
            .get_or_fetch(|| async {
                Ok(maven_metadata("21.1.201", &["21.1.200", "21.1.201"]))
            })
            .await
            .unwrap();
        assert_eq!(updated.versioning.release, "21.1.201");
        let cached = cache
            .get_or_fetch(|| async {
                panic!("fresh cache must avoid fetching")
            })
            .await
            .unwrap();
        assert_eq!(cached.versioning.release, "21.1.201");
        cache.value.lock().await.as_mut().unwrap().0 =
            Instant::now() - LOADER_METADATA_TTL;
        let fail = || async {
            Err(crate::ErrorKind::OtherError("offline".into()).as_error())
        };
        let stale = cache.get_or_fetch(fail).await.unwrap();
        assert_eq!(stale.versioning.release, "21.1.201");
        assert!(
            MetadataCache::<MavenMetadata>::default()
                .get_or_fetch(fail)
                .await
                .is_err()
        );
    }

    #[test]
    fn maven_index_validation_rejects_invalid_and_empty_responses() {
        for xml in [
            "not XML",
            "<html><body>upstream unavailable</body></html>",
            "<metadata><versioning><versions/></versioning></metadata>",
        ] {
            assert!(
                parse_maven_metadata(
                    "https://example.test/index.xml",
                    xml.as_bytes()
                )
                .is_err()
            );
        }
        let metadata = parse_maven_metadata(
            "https://example.test/index.xml",
            b"<metadata><versioning><versions><version>21.1.200</version></versions></versioning></metadata>",
        ).unwrap();
        assert_eq!(metadata.versioning.versions.values, ["21.1.200"]);
    }

    fn forge_loaders<'a>(
        manifest: &'a Manifest,
        game_version: &str,
    ) -> &'a [LoaderVersion] {
        &manifest
            .game_versions
            .iter()
            .find(|version| version.id == game_version)
            .unwrap_or_else(|| {
                panic!("missing Forge support for {game_version}")
            })
            .loaders
    }

    #[test]
    fn forge_metadata_groups_versions_and_uses_installer_profiles() {
        let manifest = forge_manifest(
            maven_metadata(
                "1.21.5-55.1.13",
                &["1.21.5-55.1.9", "1.21.5-55.1.13"],
            ),
            ForgePromotions {
                promos: HashMap::from([
                    ("1.21.5-latest".to_string(), "55.1.13".to_string()),
                    ("1.21.5-recommended".to_string(), "55.1.9".to_string()),
                ]),
            },
        );
        let loaders = &manifest.game_versions[0].loaders;
        assert_eq!(loaders[0].id, "55.1.13");
        assert_eq!(loaders[0].profile_source, LoaderProfileSource::Installer);
        assert!(!loaders[0].stable);
        assert!(loaders[1].stable);
        assert!(
            loaders[0].url.ends_with(
                "/1.21.5-55.1.13/forge-1.21.5-55.1.13-installer.jar"
            )
        );
    }

    #[test]
    fn forge_maven_metadata_parses_modern_version_coordinates() {
        let metadata: MavenMetadata = quick_xml::de::from_str(
            r#"<metadata>
                <versioning>
                    <release>26.2-65.1.0</release>
                    <versions>
                        <version>1.18.2-40.3.12</version>
                        <version>1.19-41.1.0</version>
                        <version>1.20.1-47.4.22</version>
                        <version>1.21.1-52.1.16</version>
                        <version>26.1-62.0.9</version>
                        <version>26.2-65.1.0</version>
                    </versions>
                </versioning>
            </metadata>"#,
        )
        .unwrap();
        let manifest = forge_manifest(metadata, ForgePromotions::default());

        for (game_version, loader_prefix) in [
            ("1.18.2", "40."),
            ("1.19", "41."),
            ("1.20.1", "47."),
            ("1.21.1", "52."),
            ("26.1", "62."),
            ("26.2", "65."),
        ] {
            assert!(
                forge_loaders(&manifest, game_version)
                    .iter()
                    .any(|loader| loader.id.starts_with(loader_prefix)),
                "missing Forge {loader_prefix} for Minecraft {game_version}"
            );
        }
    }

    #[test]
    fn forge_preserves_complete_maven_catalog_and_maps_promotions() {
        let metadata: MavenMetadata = quick_xml::de::from_str(
            r#"<metadata>
                <versioning>
                    <release>26.2-65.1.1</release>
                    <versions>
                        <version>26.2-65.1.1</version>
                        <version>26.2-65.1.0</version>
                        <version>26.2-65.0.9</version>
                        <version>26.2-65.0.8</version>
                        <version>26.2-65.0.1</version>
                        <version>26.2-65.0.0</version>
                        <version>26.1-62.0.9</version>
                    </versions>
                </versioning>
            </metadata>"#,
        )
        .unwrap();
        let manifest = forge_manifest(
            metadata,
            ForgePromotions {
                promos: HashMap::from([
                    ("26.2-latest".to_string(), "65.1.1".to_string()),
                    ("26.2-recommended".to_string(), "65.1.0".to_string()),
                ]),
            },
        );
        let loaders = forge_loaders(&manifest, "26.2");

        assert_eq!(
            loaders
                .iter()
                .map(|loader| loader.id.as_str())
                .collect::<Vec<_>>(),
            ["65.1.1", "65.1.0", "65.0.9", "65.0.8", "65.0.1", "65.0.0"]
        );
        assert!(!loaders[0].stable);
        assert!(loaders[1].stable);
        assert!(loaders[2..].iter().all(|loader| !loader.stable));
        assert!(
            forge_loaders(&manifest, "26.1")
                .iter()
                .all(|loader| !loader.id.starts_with("65."))
        );
    }

    #[test]
    fn forge_promotions_do_not_create_catalog_entries() {
        let manifest = forge_manifest(
            maven_metadata("26.2-65.1.0", &["26.2-65.1.0"]),
            ForgePromotions {
                promos: HashMap::from([(
                    "26.2-latest".to_string(),
                    "65.1.1".to_string(),
                )]),
            },
        );

        assert_eq!(forge_loaders(&manifest, "26.2")[0].id, "65.1.0");
    }

    #[test]
    fn forge_scoped_metadata_keeps_empty_and_supported_versions_isolated() {
        let full = forge_manifest(
            maven_metadata(
                "1.20.1-47.4.22",
                &["1.20.1-47.4.21", "1.20.1-47.4.22", "1.21.5-55.1.13"],
            ),
            ForgePromotions::default(),
        );

        let forge_262 = scope_manifest(full.clone(), "26.2");
        let forge_1201 = scope_manifest(full.clone(), "1.20.1");
        let forge_262_again = scope_manifest(full, "26.2");

        assert!(forge_262.game_versions[0].loaders.is_empty());
        assert_eq!(
            forge_1201.game_versions[0]
                .loaders
                .iter()
                .map(|loader| loader.id.as_str())
                .collect::<Vec<_>>(),
            ["47.4.22", "47.4.21"]
        );
        assert!(forge_262_again.game_versions[0].loaders.is_empty());
        assert!(validate_scoped_manifest(forge_262, "26.2").is_ok());
    }

    #[test]
    fn forge_scoped_metadata_preserves_complete_catalog_across_switches() {
        let full = forge_manifest(
            maven_metadata(
                "26.2-65.1.1",
                &[
                    "26.2-65.1.1",
                    "26.2-65.1.0",
                    "26.2-65.0.9",
                    "1.20.1-47.4.22",
                    "1.20.1-47.4.21",
                ],
            ),
            ForgePromotions::default(),
        );

        let forge_262 = scope_manifest(full.clone(), "26.2");
        let forge_1201 = scope_manifest(full.clone(), "1.20.1");
        let forge_262_again = scope_manifest(full, "26.2");
        let ids = |manifest: &Manifest| {
            manifest.game_versions[0]
                .loaders
                .iter()
                .map(|loader| loader.id.clone())
                .collect::<Vec<_>>()
        };

        assert_eq!(ids(&forge_262), ["65.1.1", "65.1.0", "65.0.9"]);
        assert_eq!(ids(&forge_1201), ["47.4.22", "47.4.21"]);
        assert_eq!(ids(&forge_262_again), ids(&forge_262));
    }

    #[test]
    fn meta_api_manifest_uses_official_profiles_and_fallbacks() {
        let manifest = meta_api_manifest(
            vec![MetaGameVersion {
                version: "1.21.1".to_string(),
                stable: true,
            }],
            vec![("0.16.9".to_string(), false), ("0.16.14".to_string(), true)],
            "fabric",
            FABRIC_META_URL,
            0,
        );
        let version = &manifest.game_versions[0];
        let loaders = &manifest.version_groups[0].loaders;
        assert_eq!(version.version_group.as_deref(), Some("fabric-official"));
        assert_eq!(loaders[0].id, "0.16.14");
        assert!(
            loaders[0]
                .url
                .contains("${modrinth.gameVersion}/0.16.14/profile/json")
        );
        assert!(
            loaders[0]
                .fallback_url
                .as_ref()
                .unwrap()
                .ends_with("fabric/v0/versions/0.16.14.json")
        );
    }

    #[test]
    fn scoped_meta_manifest_binds_profile_and_versions_to_requested_game() {
        assert_eq!(
            meta_loader_versions_url(FABRIC_META_URL, "1.20.1"),
            "https://meta.fabricmc.net/v2/versions/loader/1.20.1"
        );
        assert_eq!(
            meta_loader_versions_url(QUILT_META_URL, "26.2"),
            "https://meta.quiltmc.org/v3/versions/loader/26.2"
        );
        let manifest = meta_api_manifest_for_game(
            "1.20.1",
            vec![("0.16.9".to_string(), false), ("0.16.14".to_string(), true)],
            "fabric",
            FABRIC_META_URL,
            0,
        );

        let game = &manifest.game_versions[0];
        assert_eq!(game.id, "1.20.1");
        assert_eq!(game.loaders[0].id, "0.16.14");
        assert!(
            game.loaders[0]
                .url
                .contains("loader/1.20.1/0.16.14/profile/json")
        );
        assert!(!game.loaders[0].url.contains(DUMMY_REPLACE_STRING));
    }

    #[test]
    fn neoforge_scoped_metadata_does_not_mix_build_series() {
        let full = neoforge_manifest(
            maven_metadata("21.2.10", &["21.1.200", "21.1.201", "21.2.10"]),
            maven_metadata("", &[]),
        );

        let mc_1211 = scope_manifest(full.clone(), "1.21.1");
        let mc_1212 = scope_manifest(full.clone(), "1.21.2");
        let mc_1211_again = scope_manifest(full, "1.21.1");

        assert!(
            mc_1211.game_versions[0]
                .loaders
                .iter()
                .all(|loader| loader.id.starts_with("21.1."))
        );
        assert!(
            mc_1212.game_versions[0]
                .loaders
                .iter()
                .all(|loader| loader.id.starts_with("21.2."))
        );
        assert_eq!(
            mc_1211.game_versions[0]
                .loaders
                .iter()
                .map(|loader| loader.id.as_str())
                .collect::<Vec<_>>(),
            mc_1211_again.game_versions[0]
                .loaders
                .iter()
                .map(|loader| loader.id.as_str())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn neoforge_versions_map_to_minecraft_versions() {
        assert_eq!(neoforge_game_version("20.2.93"), Some("1.20.2".into()));
        assert_eq!(neoforge_game_version("21.0.167"), Some("1.21".into()));
        assert_eq!(neoforge_game_version("21.11.45"), Some("1.21.11".into()));
        assert_eq!(
            neoforge_game_version("26.1.0.19-beta"),
            Some("26.1".into())
        );
        assert_eq!(neoforge_game_version("26.1.2.95"), Some("26.1.2".into()));
        assert_eq!(
            neoforge_game_version("0.25w14craftmine.5-beta"),
            Some("1.0.25w14craftmine".into())
        );
    }

    #[test]
    fn installer_parser_merges_profile_and_embedded_artifacts() {
        let cursor = Cursor::new(Vec::new());
        let mut archive = zip::ZipWriter::new(cursor);
        let options = SimpleFileOptions::default();
        archive.start_file("version.json", options).unwrap();
        archive
            .write_all(
                br#"{
                    "id":"1.21.5-forge-55.1.13",
                    "inheritsFrom":"1.21.5",
                    "releaseTime":"2026-08-15T20:37:45+00:00",
                    "time":"2026-08-15T20:37:45+00:00",
                    "mainClass":"example.Main",
                    "libraries":[{
                        "name":"example:generated:1.0",
                        "downloads":{"artifact":{"path":"example/generated/1.0/generated-1.0.jar","sha1":"abc","size":1,"url":""}}
                    }],
                    "type":"release"
                }"#,
            )
            .unwrap();
        archive.start_file("install_profile.json", options).unwrap();
        archive
            .write_all(
                br#"{
                    "data":{"BINPATCH":{"client":"/data/client.lzma","server":"/data/server.lzma"}},
                    "processors":[{"jar":"example:processor:1.0","classpath":[],"args":["{BINPATCH}"]}],
                    "libraries":[{"name":"example:processor:1.0"}]
                }"#,
            )
            .unwrap();
        archive.start_file("data/client.lzma", options).unwrap();
        archive.write_all(b"client-patch").unwrap();
        archive.start_file("data/server.lzma", options).unwrap();
        archive.write_all(b"server-patch").unwrap();
        let bytes = archive.finish().unwrap().into_inner();

        let parsed =
            parse_installer(bytes, "https://example.com/installer.jar")
                .unwrap();
        assert_eq!(parsed.artifacts.len(), 2);
        assert_eq!(parsed.profile.processors.as_ref().unwrap().len(), 1);
        assert!(parsed.profile.libraries.iter().any(|library| {
            library.name == "example:processor:1.0"
                && !library.include_in_classpath
        }));
        assert!(parsed.profile.libraries.iter().any(|library| {
            library.name == "example:generated:1.0" && !library.downloadable
        }));
        let data = parsed.profile.data.unwrap();
        assert!(data["BINPATCH"].client.starts_with("[com.axolotl."));
        assert_eq!(
            data[INSTALLER_URL_KEY].client,
            "https://example.com/installer.jar"
        );
    }

    fn minimal_installer_version(id: &str) -> serde_json::Value {
        serde_json::json!({
            "id": id,
            "inheritsFrom": "1.12.2",
            "releaseTime": "2017-09-18T08:39:46+00:00",
            "time": "2017-09-18T08:39:46+00:00",
            "mainClass": "net.minecraft.launchwrapper.Launch",
            "libraries": [],
            "type": "release"
        })
    }

    fn installer_fixture(
        version_path: Option<&str>,
        profile: serde_json::Value,
    ) -> Vec<u8> {
        let cursor = Cursor::new(Vec::new());
        let mut archive = zip::ZipWriter::new(cursor);
        let options = SimpleFileOptions::default();
        if let Some(version_path) = version_path {
            archive.start_file(version_path, options).unwrap();
            archive
                .write_all(
                    serde_json::to_string(&minimal_installer_version(
                        version_path,
                    ))
                    .unwrap()
                    .as_bytes(),
                )
                .unwrap();
        }
        archive.start_file("install_profile.json", options).unwrap();
        archive
            .write_all(serde_json::to_string(&profile).unwrap().as_bytes())
            .unwrap();
        archive.finish().unwrap().into_inner()
    }

    #[test]
    fn babric_manifest_is_scoped_to_beta_1_7_3_and_uses_babric_profiles() {
        let manifest =
            babric_manifest("b1.7.3", vec!["0.15.6-babric.2".to_string()]);
        assert_eq!(manifest.game_versions[0].id, "b1.7.3");
        let loader = &manifest.game_versions[0].loaders[0];
        assert!(loader.url.contains("meta.babric.dev"));
        assert!(
            loader
                .url
                .contains("loader/b1.7.3/0.15.6-babric.2/profile/json")
        );
        assert_eq!(loader.profile_source, LoaderProfileSource::Babric);
        assert!(loader.fallback_url.is_none());

        let profile: PartialVersionInfo =
            serde_json::from_value(serde_json::json!({
                "id": "babric-loader-0.15.6-babric.2-b1.7.3",
                "inheritsFrom": "b1.7.3",
                "releaseTime": "2026-08-23T00:00:00Z",
                "time": "2026-08-23T00:00:00Z",
                "type": "release",
                "mainClass": "net.fabricmc.loader.impl.launch.knot.KnotClient",
                "libraries": [
                    {"name": "babric:intermediary:b1.7.3"},
                    {"name": "babric:fabric-loader:0.15.6-babric.2"}
                ],
                "arguments": {"game": ["--demo"]}
            }))
            .unwrap();
        assert_eq!(
            profile.main_class.as_deref(),
            Some("net.fabricmc.loader.impl.launch.knot.KnotClient")
        );
        assert_eq!(profile.inherits_from, "b1.7.3");
        assert!(
            profile.libraries.iter().any(|library| {
                library.name == "babric:intermediary:b1.7.3"
            })
        );
        assert_eq!(
            serde_json::to_value(&profile.arguments).unwrap()["game"],
            serde_json::json!(["--demo"])
        );
    }

    #[test]
    fn babric_uses_the_glass_maven_fallback_when_metadata_is_unavailable() {
        let manifest = babric_manifest(
            BABRIC_GAME_VERSION,
            vec![BABRIC_FALLBACK_LOADER_VERSION.to_string()],
        );
        let loader = &manifest.game_versions[0].loaders[0];

        assert_eq!(manifest.game_versions[0].id, BABRIC_GAME_VERSION);
        assert_eq!(loader.id, BABRIC_FALLBACK_LOADER_VERSION);
        assert!(loader.stable);
        assert!(
            loader
                .url
                .contains("loader/b1.7.3/0.15.6-babric.2/profile/json")
        );

        let unsupported = unsupported_loader_manifest("1.7.10");
        assert_eq!(unsupported.game_versions[0].id, "1.7.10");
        assert!(unsupported.game_versions[0].loaders.is_empty());
    }

    #[test]
    fn babric_profile_libraries_copy_only_babric_lwjgl_from_polyfill() {
        let libraries = babric_lwjgl_libraries(&serde_json::json!({
            "libraries": [
                {"name": "org.lwjgl.lwjgl:lwjgl:2.9.4-babric.1"},
                {"name": "org.lwjgl.lwjgl:lwjgl_util:2.9.4-babric.1"},
                {"name": "org.lwjgl.lwjgl:lwjgl:2.9.0"},
                {"name": "net.java.jinput:jinput:2.0.5"}
            ]
        }))
        .unwrap();
        assert_eq!(libraries.len(), 2);
        assert!(libraries.iter().all(|library| {
            library.name.contains("lwjgl") && library.name.contains("-babric.")
        }));
    }

    #[test]
    fn installer_parser_accepts_mid_legacy_profile_json_path() {
        let bytes = installer_fixture(
            Some("install/1.8.9.json"),
            serde_json::json!({
                "json": "/install/1.8.9.json",
                "libraries": []
            }),
        );

        let parsed = parse_installer(bytes, "https://example.com/forge.jar")
            .expect("mid-legacy installer");
        assert_eq!(parsed.profile.id, "install/1.8.9.json");
    }

    #[test]
    fn installer_parser_accepts_old_inline_version_info() {
        let version_info = minimal_installer_version("1.7.10-Forge");
        let bytes = installer_fixture(
            None,
            serde_json::json!({
                "versionInfo": version_info,
                "libraries": []
            }),
        );

        let parsed = parse_installer(bytes, "https://example.com/forge.jar")
            .expect("old inline installer");
        assert_eq!(parsed.profile.id, "1.7.10-Forge");
    }

    #[test]
    fn installer_parser_accepts_legacy_universal_artifact() {
        let cursor = Cursor::new(Vec::new());
        let mut archive = zip::ZipWriter::new(cursor);
        let options = SimpleFileOptions::default();
        archive.start_file("install_profile.json", options).unwrap();
        archive
            .write_all(
                serde_json::json!({
                    "versionInfo": minimal_installer_version("1.7.10-Forge"),
                    "install": {
                        "path": "net.minecraftforge:forge:1.7.10-10.13.4.1614-1.7.10",
                        "filePath": "/forge-universal.jar"
                    }
                })
                .to_string()
                .as_bytes(),
            )
            .unwrap();
        archive.start_file("forge-universal.jar", options).unwrap();
        archive.write_all(b"legacy universal artifact").unwrap();
        let parsed = parse_installer(
            archive.finish().unwrap().into_inner(),
            "https://example.com/forge.jar",
        )
        .unwrap();

        assert!(parsed.profile.libraries.iter().any(|library| {
            library.name
                == "net.minecraftforge:forge:1.7.10-10.13.4.1614-1.7.10"
                && !library.downloadable
        }));
        assert_eq!(parsed.artifacts.len(), 1);
    }

    #[test]
    fn coordinate_from_embedded_maven_path_preserves_classifier_and_extension()
    {
        assert_eq!(
            coordinate_from_maven_path(
                "maven/com/example/tool/1.2.3/tool-1.2.3-all.jar"
            ),
            Some("com.example:tool:1.2.3:all@jar".to_string())
        );
        assert_eq!(
            coordinate_from_maven_path(
                "maven/com/example/tool/1.2.3/tool-1.2.3.zip"
            ),
            Some("com.example:tool:1.2.3@zip".to_string())
        );
        assert_eq!(
            coordinate_from_maven_path(
                "maven/com/example/tool/1.2.3/not-an-artifact.jar"
            ),
            None
        );
    }

    #[test]
    fn installer_profile_uses_profile_minecraft_when_version_has_no_inheritance()
     {
        let cursor = Cursor::new(Vec::new());
        let mut archive = zip::ZipWriter::new(cursor);
        let options = SimpleFileOptions::default();
        archive.start_file("version.json", options).unwrap();
        archive
            .write_all(
                serde_json::json!({
                    "id": "1.12.2-Cleanroom-test",
                    "releaseTime": "2026-08-15T20:37:45+00:00",
                    "time": "2026-08-15T20:37:45+00:00",
                    "mainClass": "top.outlands.foundation.boot.Foundation",
                    "javaVersion": {"component": "java-runtime-epsilon", "majorVersion": 25},
                    "libraries": [],
                    "type": "release"
                })
                .to_string()
                .as_bytes(),
            )
            .unwrap();
        archive.start_file("install_profile.json", options).unwrap();
        archive
            .write_all(br#"{"minecraft":"1.12.2","json":"/version.json"}"#)
            .unwrap();
        let parsed = parse_installer(
            archive.finish().unwrap().into_inner(),
            "https://example.com/cleanroom.jar",
        )
        .unwrap();

        assert_eq!(parsed.profile.inherits_from, "1.12.2");
        assert_eq!(
            parsed
                .profile
                .java_version
                .as_ref()
                .map(|java| java.major_version),
            Some(25)
        );
    }

    #[test]
    fn provider_fixtures_filter_and_parse_new_loaders() {
        assert_eq!(
            optifine_metadata_url(None),
            "https://bmclapi2.bangbang93.com/optifine/versionList"
        );
        assert_eq!(
            optifine_metadata_url(Some("1.20.1")),
            "https://bmclapi2.bangbang93.com/optifine/1.20.1"
        );

        let optifine = optifine_manifest(
            vec![
                OptiFineMetadata {
                    mcversion: "1.12.2".to_string(),
                    type_: "HD_U".to_string(),
                    patch: "G5".to_string(),
                },
                OptiFineMetadata {
                    mcversion: "1.20.1".to_string(),
                    type_: "HD_U".to_string(),
                    patch: "I6_pre1".to_string(),
                },
            ],
            Some("1.12.2"),
        );
        assert_eq!(optifine.game_versions.len(), 1);
        assert_eq!(optifine.game_versions[0].loaders[0].id, "OptiFine_HD_U_G5");

        let unsupported_optifine = optifine_manifest(Vec::new(), Some("26.2"));
        assert_eq!(unsupported_optifine.game_versions.len(), 1);
        assert_eq!(unsupported_optifine.game_versions[0].id, "26.2");
        assert!(unsupported_optifine.game_versions[0].loaders.is_empty());
        assert!(validate_scoped_manifest(unsupported_optifine, "26.2").is_ok());

        let lite_metadata = serde_json::json!({
            "versions": {
                "1.6.4": {
                    "artefacts": {
                        "com.mumfrey:liteloader": {
                            "latest": {
                                "tweakClass": "com.mumfrey.liteloader.launch.LiteLoaderTweaker",
                                "libraries": [{"name": "net.minecraft:launchwrapper:1.8"}],
                                "version": "1.6.4_01",
                                "stream": "RELEASE",
                                "timestamp": "1380567800"
                            }
                        }
                    }
                },
                "1.12.2": {
                    "snapshots": {
                        "com.mumfrey:liteloader": {
                            "latest": {
                                "tweakClass": "com.mumfrey.liteloader.launch.LiteLoaderTweaker",
                                "libraries": [{"name": "net.minecraft:launchwrapper:1.12"}],
                                "version": "1.12.2-SNAPSHOT",
                                "stream": "SNAPSHOT",
                                "timestamp": "1511880271"
                            }
                        }
                    }
                }
            }
        });
        let lite = liteloader_manifest(&lite_metadata).unwrap();
        assert_eq!(lite.game_versions.len(), 1);
        assert_eq!(lite.game_versions[0].id, "1.12.2");
        assert_eq!(lite.game_versions[0].loaders[0].id, "1.12.2-SNAPSHOT");
        let profile = build_liteloader_profile(
            &lite_metadata,
            "1.12.2",
            "1.12.2-SNAPSHOT",
        )
        .unwrap();
        assert_eq!(
            profile.main_class.as_deref(),
            Some("net.minecraft.launchwrapper.Launch")
        );
        assert!(profile.libraries.iter().any(|library| {
            library.name == "com.mumfrey:liteloader:1.12.2-SNAPSHOT"
        }));
        assert_eq!(
            profile.java_version.as_ref().map(|java| java.major_version),
            Some(8)
        );
        assert_eq!(
            profile
                .java_version
                .as_ref()
                .map(|java| java.component.as_str()),
            Some("jre-legacy")
        );

        let legacy_fabric_profile: PartialVersionInfo =
            serde_json::from_value(serde_json::json!({
                "id": "fabric-loader-0.19.3-1.8.9",
                "inheritsFrom": "1.8.9",
                "releaseTime": "2026-08-19T13:03:54+0000",
                "time": "2026-08-19T13:03:54+0000",
                "type": "release",
                "mainClass": "net.fabricmc.loader.impl.launch.knot.KnotClient",
                "libraries": [
                    {"name": "net.legacyfabric:intermediary:1.8.9", "url": "https://maven.legacyfabric.net/"},
                    {"name": "net.fabricmc:fabric-loader:0.19.3", "url": "https://maven.fabricmc.net/"},
                    {"name": "org.lwjgl.lwjgl:lwjgl:2.9.4+legacyfabric.17", "url": "https://maven.legacyfabric.net/"}
                ]
            }))
            .unwrap();
        assert_eq!(
            legacy_fabric_profile.main_class.as_deref(),
            Some("net.fabricmc.loader.impl.launch.knot.KnotClient")
        );
        assert!(legacy_fabric_profile.libraries.iter().any(|library| {
            library.name == "net.legacyfabric:intermediary:1.8.9"
        }));
        assert!(legacy_fabric_profile.arguments.is_none());
        assert!(legacy_fabric_profile.minecraft_arguments.is_none());

        let cleanroom = cleanroom_manifest(vec![GithubRelease {
            tag_name: "0.6.11-alpha".to_string(),
            draft: false,
            prerelease: false,
            assets: vec![GithubReleaseAsset {
                name: "cleanroom-0.6.11-alpha-installer.jar".to_string(),
                browser_download_url: "https://example.com/cleanroom.jar"
                    .to_string(),
            }],
        }]);
        assert_eq!(cleanroom.game_versions[0].id, "1.12.2");
        assert_eq!(
            cleanroom.game_versions[0].loaders[0].profile_source,
            LoaderProfileSource::Installer
        );
    }

    #[test]
    fn cleanroom_maven_metadata_builds_installer_manifest() {
        let metadata: MavenMetadata = quick_xml::de::from_str(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<metadata>
  <groupId>com.cleanroommc</groupId>
  <artifactId>cleanroom</artifactId>
  <versioning>
    <latest>0.6.13-alpha</latest>
    <release>0.6.13-alpha</release>
    <versions>
      <version>0.4.0-alpha</version>
      <version>0.4.0</version>
      <version>0.6.13-alpha</version>
    </versions>
  </versioning>
</metadata>"#,
        )
        .unwrap();

        let manifest = cleanroom_maven_manifest(metadata);
        assert_eq!(manifest.game_versions.len(), 1);
        assert_eq!(manifest.game_versions[0].id, "1.12.2");

        let loaders = &manifest.game_versions[0].loaders;
        // Newest first; the stable 0.4.0 release outranks its alpha twin.
        assert_eq!(
            loaders
                .iter()
                .map(|version| version.id.as_str())
                .collect::<Vec<_>>(),
            ["0.6.13-alpha", "0.4.0", "0.4.0-alpha"]
        );
        assert_eq!(
            loaders[0].url,
            "https://repo.cleanroommc.com/releases/com/cleanroommc/cleanroom/0.6.13-alpha/cleanroom-0.6.13-alpha-installer.jar"
        );
        assert_eq!(loaders[0].profile_source, LoaderProfileSource::Installer);
        assert!(!loaders[0].stable);
        assert!(loaders[1].stable);
        assert!(!loaders[2].stable);
    }

    #[test]
    fn natural_version_order_keeps_latest_first() {
        let mut versions = ["55.1.9", "55.1.13", "55.1.10"];
        versions.sort_by(|left, right| compare_versions(right, left));
        assert_eq!(versions, ["55.1.13", "55.1.10", "55.1.9"]);
    }
}

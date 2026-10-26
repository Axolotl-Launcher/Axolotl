use crate::api::curseforge::CurseForgeProject;
use crate::state::{
    CurseForgeProjectId, ModrinthProjectId, ModrinthVersionId, ProjectType,
};
use crate::util::fetch::{
    FetchSemaphore, fetch_json, fetch_json_nonempty, sha1_async,
};
use chrono::{DateTime, Utc};
use dashmap::DashSet;
use reqwest::Method;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::env;
use std::fmt::Display;
use std::hash::Hash;
use std::path::PathBuf;

// 1 day
const DEFAULT_ID: &str = "0";

/// Most cache expiries are set to 100 years (effectively permanent).
/// Loader manifests expire at the background refresh threshold because newly
/// published loader versions must become selectable without an app restart.
const PERMANENT_CACHE_SECONDS: i64 = 100 * 365 * 24 * 60 * 60;

/// How long before an entry should be asynchronously refreshed in the background, in seconds.
const BACKGROUND_REFRESH_THRESHOLD: i64 = 30 * 60; // 30 minutes

fn encode_json_query_value<T: Serialize>(
    value: &T,
) -> serde_json::Result<String> {
    serde_json::to_string(value).map(|json| {
        url::form_urlencoded::byte_serialize(json.as_bytes()).collect()
    })
}

#[cfg(test)]
mod query_encoding_tests {
    use super::encode_json_query_value;

    #[test]
    fn preserves_plus_signs_in_batched_query_values() {
        assert_eq!(
            encode_json_query_value(&["foo+bar"]).unwrap(),
            "%5B%22foo%2Bbar%22%5D"
        );
    }
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CacheValueType {
    Project,
    ProjectV3,
    CurseForgeProject,
    Version,
    VersionV3,
    User,
    Team,
    Organization,
    File,
    LoaderManifest,
    MinecraftManifest,
    Categories,
    ReportTypes,
    Loaders,
    GameVersions,
    DonationPlatforms,
    FileHash,
    FileUpdate,
    SearchResults,
    SearchResultsV3,
    ModpackFiles,
    /// Cached list of versions for a project (without changelogs for fast loading)
    ProjectVersions,
}

impl CacheValueType {
    pub fn as_str(&self) -> &'static str {
        match self {
            CacheValueType::Project => "project",
            CacheValueType::ProjectV3 => "project_v3",
            CacheValueType::CurseForgeProject => "curseforge_project",
            CacheValueType::Version => "version",
            CacheValueType::VersionV3 => "version_v3",
            CacheValueType::User => "user",
            CacheValueType::Team => "team",
            CacheValueType::Organization => "organization",
            CacheValueType::File => "file",
            CacheValueType::LoaderManifest => "loader_manifest",
            CacheValueType::MinecraftManifest => "minecraft_manifest",
            CacheValueType::Categories => "categories",
            CacheValueType::ReportTypes => "report_types",
            CacheValueType::Loaders => "loaders",
            CacheValueType::GameVersions => "game_versions",
            CacheValueType::DonationPlatforms => "donation_platforms",
            CacheValueType::FileHash => "file_hash",
            CacheValueType::FileUpdate => "file_update",
            CacheValueType::SearchResults => "search_results",
            CacheValueType::SearchResultsV3 => "search_results_v3",
            CacheValueType::ModpackFiles => "modpack_files",
            CacheValueType::ProjectVersions => "project_versions",
        }
    }

    pub fn from_string(val: &str) -> CacheValueType {
        match val {
            "project" => CacheValueType::Project,
            "project_v3" => CacheValueType::ProjectV3,
            "curseforge_project" => CacheValueType::CurseForgeProject,
            "version" => CacheValueType::Version,
            "version_v3" => CacheValueType::VersionV3,
            "user" => CacheValueType::User,
            "team" => CacheValueType::Team,
            "organization" => CacheValueType::Organization,
            "file" => CacheValueType::File,
            "loader_manifest" => CacheValueType::LoaderManifest,
            "minecraft_manifest" => CacheValueType::MinecraftManifest,
            "categories" => CacheValueType::Categories,
            "report_types" => CacheValueType::ReportTypes,
            "loaders" => CacheValueType::Loaders,
            "game_versions" => CacheValueType::GameVersions,
            "donation_platforms" => CacheValueType::DonationPlatforms,
            "file_hash" => CacheValueType::FileHash,
            "file_update" => CacheValueType::FileUpdate,
            "search_results" => CacheValueType::SearchResults,
            "search_results_v3" => CacheValueType::SearchResultsV3,
            "modpack_files" => CacheValueType::ModpackFiles,
            "project_versions" => CacheValueType::ProjectVersions,
            _ => CacheValueType::Project,
        }
    }

    pub fn from_repairable_str(val: &str) -> Option<CacheValueType> {
        match val {
            "curseforge_project" => Some(CacheValueType::CurseForgeProject),
            _ => None,
        }
    }

    fn is_repairable_install_cache(self) -> bool {
        matches!(self, CacheValueType::CurseForgeProject)
    }

    pub fn expiry(&self) -> i64 {
        match self {
            // Newly published game versions must become selectable without an
            // app restart, so version manifests share the loader refresh window.
            CacheValueType::LoaderManifest
            | CacheValueType::MinecraftManifest
            | CacheValueType::GameVersions
            | CacheValueType::ProjectVersions => BACKGROUND_REFRESH_THRESHOLD,
            _ => PERMANENT_CACHE_SECONDS,
        }
    }

    pub fn get_empty_entry(self, key: String) -> CachedEntry {
        CachedEntry {
            id: key,
            alias: None,
            expires: Utc::now().timestamp() + self.expiry(),
            type_: self,
            data: None,
        }
    }

    pub fn case_sensitive_alias(&self) -> Option<bool> {
        match self {
            CacheValueType::Project
            | CacheValueType::ProjectV3
            | CacheValueType::CurseForgeProject
            | CacheValueType::User
            | CacheValueType::Organization => Some(false),

            CacheValueType::FileHash => Some(true),

            CacheValueType::MinecraftManifest
            | CacheValueType::Categories
            | CacheValueType::ReportTypes
            | CacheValueType::Loaders
            | CacheValueType::GameVersions
            | CacheValueType::DonationPlatforms
            | CacheValueType::Version
            | CacheValueType::VersionV3
            | CacheValueType::Team
            | CacheValueType::File
            | CacheValueType::LoaderManifest
            | CacheValueType::FileUpdate
            | CacheValueType::SearchResults
            | CacheValueType::SearchResultsV3
            | CacheValueType::ModpackFiles
            | CacheValueType::ProjectVersions => None,
        }
    }
}

#[cfg(test)]
mod loader_manifest_expiry_tests {
    use super::{
        BACKGROUND_REFRESH_THRESHOLD, CacheValueType, PERMANENT_CACHE_SECONDS,
    };

    #[test]
    fn version_manifests_expire_at_refresh_threshold() {
        assert_eq!(
            CacheValueType::LoaderManifest.expiry(),
            BACKGROUND_REFRESH_THRESHOLD
        );
        assert_eq!(
            CacheValueType::MinecraftManifest.expiry(),
            BACKGROUND_REFRESH_THRESHOLD
        );
        assert_eq!(
            CacheValueType::GameVersions.expiry(),
            BACKGROUND_REFRESH_THRESHOLD
        );
        assert_eq!(
            CacheValueType::ProjectVersions.expiry(),
            BACKGROUND_REFRESH_THRESHOLD
        );
        assert_eq!(CacheValueType::Project.expiry(), PERMANENT_CACHE_SECONDS);
    }
}

/// Expire rows written under a longer historical TTL when policy shrinks.
fn cache_entry_expired(type_: CacheValueType, expires: i64, now: i64) -> bool {
    expires <= now || expires.saturating_sub(now) > type_.expiry()
}

#[cfg(test)]
mod cache_expiry_tests {
    use super::{
        BACKGROUND_REFRESH_THRESHOLD, CacheValueType, PERMANENT_CACHE_SECONDS,
        cache_entry_expired,
    };

    #[test]
    fn project_version_cache_rejects_legacy_permanent_expiry() {
        let now = 1_000_000;

        assert!(cache_entry_expired(
            CacheValueType::ProjectVersions,
            now + PERMANENT_CACHE_SECONDS,
            now,
        ));
        assert!(!cache_entry_expired(
            CacheValueType::ProjectVersions,
            now + BACKGROUND_REFRESH_THRESHOLD,
            now,
        ));
        assert!(!cache_entry_expired(
            CacheValueType::Project,
            now + PERMANENT_CACHE_SECONDS,
            now,
        ));
        assert!(cache_entry_expired(
            CacheValueType::ProjectVersions,
            now - 1,
            now,
        ));
    }
}

fn cache_read_failure(
    cache_type: CacheValueType,
    cache_error: crate::Error,
    remote_error: Option<crate::Error>,
) -> crate::Error {
    if !cache_type.is_repairable_install_cache() {
        return cache_error;
    }
    let sqlite_code = match cache_error.raw.as_ref() {
        crate::ErrorKind::Sqlx(sqlx::Error::Database(error)) => {
            error.code().map(|code| code.into_owned())
        }
        _ => None,
    };
    let remote_suffix = if remote_error.is_some() {
        "; remote replacement data was unavailable"
    } else {
        Default::default()
    };
    crate::ErrorKind::CacheReadError {
        cache_type: cache_type.as_str().to_string(),
        message: format!("{cache_error}{remote_suffix}"),
        sqlite_code,
    }
    .into()
}

/// Cached modpack file hashes for filtering content
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CachedModpackFiles {
    pub version_id: String,
    pub file_hashes: Vec<String>,
    #[serde(default)]
    pub project_ids: Vec<String>,
}

/// Cached list of versions for a project (without changelogs for fast loading)
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CachedProjectVersions {
    pub project_id: String,
    pub versions: Vec<Version>,
}

// De/serialization strategy:
// - on serialize:
//   - in the `cache` table, save the `data_type` (variant of this value) alongside
//     the data
//   - data column contains the serialized form of the INNER value (i.e. for a
//     `CacheValue::Project`, we serialize it as a `Project,` NOT as a `CacheValue`)
//   - this way, we do not tag the data using serde in any way
// - on deserialize:
//   - use the `data_type` to figure out what type of value to deser as
//   - then wrap that in a `CacheValue`
//
// do NOT use `#[serde(untagged)]` here, since then a value of one variant can be
// deser'd as a value of another variant, if it comes before it in the enum
// definition list.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[allow(clippy::large_enum_variant)]
pub enum CacheValue {
    Project(Project),
    CurseForgeProject(CurseForgeProject),
    Version(Version),
    VersionV3(VersionV3),
    User(User),
    Team(Vec<TeamMember>),
    Organization(Organization),
    File(ModrinthHashMatch),
    LoaderManifest(CachedLoaderManifest),
    MinecraftManifest(daedalus::minecraft::VersionManifest),
    Categories(Vec<Category>),
    ReportTypes(Vec<String>),
    Loaders(Vec<Loader>),
    GameVersions(Vec<GameVersion>),
    DonationPlatforms(Vec<DonationPlatform>),
    FileHash(CachedFileHash),
    FileUpdate(CachedFileUpdate),
    SearchResults(SearchResults),
    SearchResultsV3(SearchResultsV3),
    ModpackFiles(CachedModpackFiles),
    ProjectVersions(CachedProjectVersions),
    ProjectV3(ProjectV3),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SearchResults {
    pub search: String,
    pub result: SearchResult,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SearchResult {
    pub hits: Vec<SearchEntry>,
    pub offset: u32,
    pub limit: u32,
    pub total_hits: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SearchEntry {
    pub project_id: String,
    pub project_type: String,
    pub slug: Option<String>,
    pub author: String,
    pub title: String,
    pub description: String,
    pub categories: Vec<String>,
    pub display_categories: Vec<String>,
    pub versions: Vec<String>,
    pub downloads: i32,
    pub follows: i32,
    pub icon_url: String,
    pub date_created: DateTime<Utc>,
    pub date_modified: DateTime<Utc>,
    pub latest_version: String,
    pub license: String,
    pub client_side: String,
    pub server_side: String,
    pub gallery: Vec<String>,
    pub featured_gallery: Option<String>,
    pub color: Option<u32>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SearchResultsV3 {
    pub search: String,
    pub result: SearchResultV3,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SearchResultV3 {
    pub hits: Vec<serde_json::Value>,
    #[serde(default)]
    pub offset: u32,
    #[serde(default)]
    pub limit: u32,
    #[serde(default)]
    pub total_hits: u32,
}

#[derive(Serialize, Clone, Debug)]
pub struct CachedFileUpdate {
    pub hash: String,
    pub game_version: String,
    pub loaders: Vec<String>,
    pub channel_policy: String,
    pub update_version_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseChannel {
    Release,
    Beta,
    Alpha,
}

impl ReleaseChannel {
    pub fn key(self) -> &'static str {
        match self {
            Self::Release => "release",
            Self::Beta => "beta",
            Self::Alpha => "alpha",
        }
    }

    pub fn from_key(key: &str) -> Self {
        match key {
            "alpha" => Self::Alpha,
            "all" => Self::Alpha,
            "beta" => Self::Beta,
            _ => Self::Release,
        }
    }

    pub fn from_version_type(version_type: &str) -> Self {
        match version_type {
            "alpha" => Self::Alpha,
            "beta" => Self::Beta,
            _ => Self::Release,
        }
    }

    pub fn least_stable(self, other: Self) -> Self {
        if self.instability_rank() >= other.instability_rank() {
            self
        } else {
            other
        }
    }

    fn instability_rank(self) -> u8 {
        match self {
            Self::Release => 0,
            Self::Beta => 1,
            Self::Alpha => 2,
        }
    }

    pub fn version_type_fallbacks(self) -> Vec<Vec<&'static str>> {
        match self {
            Self::Release => {
                vec![vec!["release"], vec!["beta"], vec!["alpha"]]
            }
            Self::Beta => {
                vec![vec!["release", "beta"], vec!["alpha"]]
            }
            Self::Alpha => vec![vec!["release", "beta", "alpha"]],
        }
    }
}

fn default_file_update_channel_policy() -> String {
    ReleaseChannel::Alpha.key().to_string()
}

/// Migrates old cache entries that stored `"loader": "forge"` (singular string)
/// to the current `"loaders": ["forge"]` (array) format.
/// SEE: https://github.com/modrinth/code/issues/5562
impl<'de> serde::Deserialize<'de> for CachedFileUpdate {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Helper {
            hash: String,
            game_version: String,
            #[serde(default)]
            loaders: Option<Vec<String>>,
            #[serde(default)]
            loader: Option<String>,
            #[serde(default = "default_file_update_channel_policy")]
            channel_policy: String,
            update_version_id: String,
        }

        let helper = Helper::deserialize(deserializer)?;
        let loaders = helper.loaders.unwrap_or_else(|| {
            helper.loader.map(|l| vec![l]).unwrap_or_default()
        });

        Ok(CachedFileUpdate {
            hash: helper.hash,
            game_version: helper.game_version,
            loaders,
            channel_policy: helper.channel_policy,
            update_version_id: helper.update_version_id,
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CachedFileHash {
    pub path: String,
    pub size: u64,
    pub hash: String,
    pub project_type: Option<ProjectType>,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub version_id: Option<String>,
}

#[derive(Clone, Copy, Debug)]
pub struct KnownModrinthFile<'a> {
    pub project_id: &'a str,
    pub version_id: &'a str,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CachedLoaderManifest {
    pub loader: String,
    pub manifest: daedalus::modded::Manifest,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ModrinthHashMatch {
    pub hash: String,
    pub project_id: String,
    pub version_id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Project {
    pub id: String,
    pub slug: Option<String>,
    pub project_type: String,
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub team: String,
    pub organization: Option<String>,
    pub title: String,
    pub description: String,
    pub body: String,

    pub published: DateTime<Utc>,
    pub updated: DateTime<Utc>,
    pub approved: Option<DateTime<Utc>>,

    pub status: String,

    pub license: License,

    pub client_side: SideType,
    pub server_side: SideType,

    pub downloads: u32,
    pub followers: u32,

    pub categories: Vec<String>,
    pub additional_categories: Vec<String>,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,

    pub versions: Vec<String>,

    pub icon_url: Option<String>,

    pub issues_url: Option<String>,
    pub source_url: Option<String>,
    pub wiki_url: Option<String>,
    pub discord_url: Option<String>,
    #[serde(default, deserialize_with = "deserialize_donation_links")]
    pub donation_urls: Option<Vec<DonationLink>>,
    pub gallery: Vec<GalleryItem>,
    pub color: Option<u32>,
}

/// Uses serde_json::Value for flexibility since the v3. properly typed in frontend
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProjectV3 {
    pub id: String,
    pub slug: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct License {
    pub id: String,
    pub name: String,
    pub url: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GalleryItem {
    pub url: String,
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub raw_url: String,
    pub featured: bool,
    pub title: Option<String>,
    pub description: Option<String>,
    pub created: DateTime<Utc>,
    pub ordering: i64,
}

#[cfg(test)]
mod gallery_item_tests {
    use super::GalleryItem;

    #[test]
    fn accepts_legacy_response_without_raw_url() {
        let item = serde_json::from_value::<GalleryItem>(serde_json::json!({
            "url": "https://cdn.modrinth.com/data/project/image.png",
            "featured": false,
            "title": null,
            "description": null,
            "created": "2026-07-20T00:00:00Z",
            "ordering": 0
        }))
        .expect("legacy gallery item should deserialize");

        assert!(item.raw_url.is_empty());
    }

    #[test]
    fn accepts_redacted_or_missing_optional_gallery_urls() {
        let item = serde_json::from_value::<GalleryItem>(serde_json::json!({
            "url": "https://cdn.modrinth.com/data/project/image.png",
            "raw_url": null,
            "featured": false,
            "title": null,
            "description": null,
            "created": "2026-07-20T00:00:00Z",
            "ordering": 0
        }))
        .unwrap();
        assert!(item.raw_url.is_empty());
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DonationLink {
    pub id: String,
    pub platform: String,
    pub url: String,
}

fn deserialize_optional_string<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<String, D::Error> {
    Ok(Option::<String>::deserialize(deserializer)?.unwrap_or_default())
}

fn deserialize_donation_links<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Vec<DonationLink>>, D::Error> {
    #[derive(Deserialize)]
    struct Link {
        id: String,
        platform: String,
        url: Option<String>,
    }
    // Resource protocol projections redact credential-bearing URLs to null.
    // Keep the valid project metadata without exposing unusable links.
    Ok(
        Option::<Vec<Link>>::deserialize(deserializer)?.map(|links| {
            links
                .into_iter()
                .filter_map(|link| {
                    link.url.filter(|url| !url.trim().is_empty()).map(|url| {
                        DonationLink {
                            id: link.id,
                            platform: link.platform,
                            url,
                        }
                    })
                })
                .collect()
        }),
    )
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum SideType {
    Required,
    Optional,
    Unsupported,
    Unknown,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Version {
    pub id: String,
    pub project_id: String,
    pub author_id: String,

    pub featured: bool,

    pub name: String,
    pub version_number: String,
    #[serde(default)]
    pub changelog: Option<String>,
    pub changelog_url: Option<String>,

    pub date_published: DateTime<Utc>,
    pub downloads: u32,
    pub version_type: String,

    pub files: Vec<VersionFile>,
    pub dependencies: Vec<Dependency>,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VersionV3 {
    pub id: String,
    pub files: Vec<VersionFile>,
    #[serde(default)]
    pub environment: Option<VersionEnvironment>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
#[serde(rename_all = "snake_case")]
pub enum VersionEnvironment {
    ClientAndServer,
    ClientOnly,
    ClientOnlyServerOptional,
    SingleplayerOnly,
    ServerOnly,
    ServerOnlyClientOptional,
    DedicatedServerOnly,
    ClientOrServer,
    ClientOrServerPrefersBoth,
    #[serde(other)]
    Unknown,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VersionFile {
    pub hashes: HashMap<String, String>,
    pub url: String,
    pub filename: String,
    pub primary: bool,
    pub size: u32,
    pub file_type: Option<FileType>,
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
pub enum FileType {
    RequiredResourcePack,
    OptionalResourcePack,
    SourcesJar,
    DevJar,
    JavadocJar,
    Signature,
    #[serde(other)]
    Unknown,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Dependency {
    pub version_id: Option<String>,
    pub project_id: Option<String>,
    pub file_name: Option<String>,
    pub dependency_type: DependencyType,
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
#[serde(rename_all = "lowercase")]
pub enum DependencyType {
    Required,
    Optional,
    Incompatible,
    Embedded,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TeamMember {
    pub team_id: String,
    pub user: User,
    pub is_owner: bool,
    pub role: String,
    pub ordering: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct User {
    pub id: String,
    pub username: String,
    pub avatar_url: Option<String>,
    pub bio: Option<String>,
    pub created: DateTime<Utc>,
    pub role: String,
    #[serde(default)]
    pub badges: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Organization {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub team_id: String,
    pub description: String,
    pub icon_url: Option<String>,
    pub color: Option<u32>,
    pub members: Vec<TeamMember>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    pub name: String,
    pub project_type: String,
    pub header: String,
    pub icon: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Loader {
    pub name: String,
    pub icon: PathBuf,
    pub supported_project_types: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DonationPlatform {
    pub short: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameVersion {
    pub version: String,
    pub version_type: String,
    pub date: String,
    pub major: bool,
}

impl CacheValue {
    /// Whether this cached value is a collection that should never be empty.
    ///
    /// Mirrors can return an empty array for collection endpoints they have
    /// not synced (e.g. `tag/game_version`); an empty collection is treated as
    /// a poisoned cache entry and refetched instead of being served forever.
    fn is_empty_collection(&self) -> bool {
        match self {
            CacheValue::Categories(values) => values.is_empty(),
            CacheValue::ReportTypes(values) => values.is_empty(),
            CacheValue::Loaders(values) => values.is_empty(),
            CacheValue::GameVersions(values) => values.is_empty(),
            CacheValue::DonationPlatforms(values) => values.is_empty(),
            _ => false,
        }
    }

    pub fn get_entry(self) -> CachedEntry {
        CachedEntry {
            id: self.get_key(),
            alias: self.get_alias(),
            type_: self.get_type(),
            expires: Utc::now().timestamp() + self.get_type().expiry(),
            data: Some(self),
        }
    }

    pub fn get_type(&self) -> CacheValueType {
        match self {
            CacheValue::Project(_) => CacheValueType::Project,
            CacheValue::ProjectV3(_) => CacheValueType::ProjectV3,
            CacheValue::CurseForgeProject(_) => {
                CacheValueType::CurseForgeProject
            }
            CacheValue::Version(_) => CacheValueType::Version,
            CacheValue::VersionV3(_) => CacheValueType::VersionV3,
            CacheValue::User(_) => CacheValueType::User,
            CacheValue::Team { .. } => CacheValueType::Team,
            CacheValue::Organization(_) => CacheValueType::Organization,
            CacheValue::File { .. } => CacheValueType::File,
            CacheValue::LoaderManifest { .. } => CacheValueType::LoaderManifest,
            CacheValue::MinecraftManifest(_) => {
                CacheValueType::MinecraftManifest
            }
            CacheValue::Categories(_) => CacheValueType::Categories,
            CacheValue::ReportTypes(_) => CacheValueType::ReportTypes,
            CacheValue::Loaders(_) => CacheValueType::Loaders,
            CacheValue::GameVersions(_) => CacheValueType::GameVersions,
            CacheValue::DonationPlatforms(_) => {
                CacheValueType::DonationPlatforms
            }
            CacheValue::FileHash(_) => CacheValueType::FileHash,
            CacheValue::FileUpdate(_) => CacheValueType::FileUpdate,
            CacheValue::SearchResults(_) => CacheValueType::SearchResults,
            CacheValue::SearchResultsV3(_) => CacheValueType::SearchResultsV3,
            CacheValue::ModpackFiles(_) => CacheValueType::ModpackFiles,
            CacheValue::ProjectVersions(_) => CacheValueType::ProjectVersions,
        }
    }

    fn get_key(&self) -> String {
        match self {
            CacheValue::Project(project) => project.id.clone(),
            CacheValue::ProjectV3(project) => project.id.clone(),
            CacheValue::CurseForgeProject(project) => project.id.to_string(),
            CacheValue::Version(version) => version.id.clone(),
            CacheValue::VersionV3(version) => version.id.clone(),
            CacheValue::User(user) => user.id.clone(),
            CacheValue::Team(members) => members
                .iter()
                .next()
                .map_or(DEFAULT_ID, |x| x.team_id.as_str())
                .to_string(),
            CacheValue::Organization(org) => org.id.clone(),
            CacheValue::File(file) => file.hash.clone(),
            CacheValue::LoaderManifest(loader) => loader.loader.clone(),
            // These values can only have one key/val pair, so we specify the same key
            CacheValue::MinecraftManifest(_)
            | CacheValue::Categories(_)
            | CacheValue::ReportTypes(_)
            | CacheValue::Loaders(_)
            | CacheValue::GameVersions(_)
            | CacheValue::DonationPlatforms(_) => DEFAULT_ID.to_string(),

            CacheValue::FileHash(hash) => {
                format!(
                    "{}-{}",
                    hash.size,
                    hash.path.trim_end_matches(".disabled")
                )
            }
            CacheValue::FileUpdate(hash) => {
                format!(
                    "{}-{}-{}-{}",
                    hash.hash,
                    hash.loaders.join("+"),
                    hash.channel_policy,
                    hash.game_version
                )
            }
            CacheValue::SearchResults(search) => search.search.clone(),
            CacheValue::SearchResultsV3(search) => search.search.clone(),
            CacheValue::ModpackFiles(files) => files.version_id.clone(),
            CacheValue::ProjectVersions(pv) => pv.project_id.clone(),
        }
    }

    fn get_alias(&self) -> Option<String> {
        match self {
            CacheValue::Project(project) => project.slug.clone(),
            CacheValue::ProjectV3(project) => project.slug.clone(),
            CacheValue::CurseForgeProject(project) => {
                Some(project.slug.clone())
            }
            CacheValue::User(user) => Some(user.username.clone()),
            CacheValue::Organization(org) => Some(org.slug.clone()),

            CacheValue::FileHash(_) => {
                Some(format!("{}.disabled", self.get_key()))
            }

            CacheValue::MinecraftManifest(_)
            | CacheValue::Categories(_)
            | CacheValue::ReportTypes(_)
            | CacheValue::Loaders(_)
            | CacheValue::GameVersions(_)
            | CacheValue::DonationPlatforms(_)
            | CacheValue::Version(_)
            | CacheValue::VersionV3(_)
            | CacheValue::Team { .. }
            | CacheValue::File { .. }
            | CacheValue::LoaderManifest { .. }
            | CacheValue::FileUpdate(_)
            | CacheValue::SearchResults(_)
            | CacheValue::SearchResultsV3(_)
            | CacheValue::ModpackFiles(_)
            | CacheValue::ProjectVersions(_) => None,
        }
    }

    fn to_json_value(&self) -> crate::Result<serde_json::Value> {
        let value = match self {
            CacheValue::Project(project) => serde_json::to_value(project),
            CacheValue::ProjectV3(project) => serde_json::to_value(project),
            CacheValue::CurseForgeProject(project) => {
                serde_json::to_value(project)
            }
            CacheValue::Version(version) => serde_json::to_value(version),
            CacheValue::VersionV3(version) => serde_json::to_value(version),
            CacheValue::User(user) => serde_json::to_value(user),
            CacheValue::Team(members) => serde_json::to_value(members),
            CacheValue::Organization(org) => serde_json::to_value(org),
            CacheValue::File(file) => serde_json::to_value(file),
            CacheValue::LoaderManifest(loader) => serde_json::to_value(loader),
            CacheValue::MinecraftManifest(manifest) => {
                serde_json::to_value(manifest)
            }
            CacheValue::Categories(categories) => {
                serde_json::to_value(categories)
            }
            CacheValue::ReportTypes(report_types) => {
                serde_json::to_value(report_types)
            }
            CacheValue::Loaders(loaders) => serde_json::to_value(loaders),
            CacheValue::GameVersions(versions) => {
                serde_json::to_value(versions)
            }
            CacheValue::DonationPlatforms(platforms) => {
                serde_json::to_value(platforms)
            }
            CacheValue::FileHash(hash) => serde_json::to_value(hash),
            CacheValue::FileUpdate(update) => serde_json::to_value(update),
            CacheValue::SearchResults(search) => serde_json::to_value(search),
            CacheValue::SearchResultsV3(search) => serde_json::to_value(search),
            CacheValue::ModpackFiles(files) => serde_json::to_value(files),
            CacheValue::ProjectVersions(pv) => serde_json::to_value(pv),
        }
        .map_err(|err| {
            crate::ErrorKind::OtherError(format!(
                "Failed to serialize cache value: {err}"
            ))
            .as_error()
        })?;

        Ok(value)
    }
}

#[derive(
    Deserialize, Serialize, PartialEq, Eq, Debug, Copy, Clone, Default,
)]
#[serde(rename_all = "snake_case")]
pub enum CacheBehaviour {
    /// Serve expired data. If fetch fails / launcher is offline, errors are ignored
    /// and expired data is served
    #[default]
    StaleWhileRevalidateSkipOffline,
    /// Only serve locally cached data and never make a network request.
    CacheOnly,
    // Serve expired data, revalidate in background
    StaleWhileRevalidate,
    // Must revalidate if data is expired
    MustRevalidate,
    // Ignore cache- always fetch updated data from origin
    Bypass,
}

#[derive(Copy, Clone)]
enum CacheRefreshSource {
    Foreground,
    Background,
}

impl CacheRefreshSource {
    fn as_str(self) -> &'static str {
        match self {
            Self::Foreground => "foreground",
            Self::Background => "background",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedEntry {
    id: String,
    alias: Option<String>,
    #[serde(rename = "data_type")]
    pub type_: CacheValueType,
    data: Option<CacheValue>,
    pub expires: i64,
}

macro_rules! impl_cache_methods {
    ($(($variant:ident, $type:ty)),*) => {
        impl CachedEntry {
            $(
                paste::paste! {
                    #[tracing::instrument(skip(pool, fetch_semaphore))]
                    pub async fn [<get_ $variant:snake>](
                        id: &str,
                        cache_behaviour: Option<CacheBehaviour>,
                        pool: &SqlitePool,
                        fetch_semaphore: &FetchSemaphore,
                    ) -> crate::Result<Option<$type>>
                    {
                        Ok(Self::[<get_ $variant:snake _many>](&[id], cache_behaviour, pool, fetch_semaphore).await?.into_iter().next())
                    }

                    #[tracing::instrument(skip(pool, fetch_semaphore))]
                    pub async fn [<get_ $variant:snake _many>](
                        ids: &[&str],
                        cache_behaviour: Option<CacheBehaviour>,
                        pool: &SqlitePool,
                        fetch_semaphore: &FetchSemaphore,
                    ) -> crate::Result<Vec<$type>>
                    {
                        let entry =
                            CachedEntry::get_many(CacheValueType::$variant, ids, cache_behaviour, pool, fetch_semaphore).await?;

                        Ok(entry.into_iter().filter_map(|x| if let Some(CacheValue::$variant(value)) = x.data {
                            Some(value)
                        } else {
                            None
                        }).collect())
                    }
                }
            )*
        }
    }
}

macro_rules! impl_cache_method_singular {
    ($(($variant:ident, $type:ty)),*) => {
        impl CachedEntry {
            $(
                paste::paste! {
                    #[tracing::instrument(skip(pool, fetch_semaphore))]
                    pub async fn [<get_ $variant:snake>] (
                        cache_behaviour: Option<CacheBehaviour>,
                        pool: &SqlitePool,
                        fetch_semaphore: &FetchSemaphore,
                    ) -> crate::Result<Option<$type>>
                    {
                        let entry =
                            CachedEntry::get(CacheValueType::$variant, DEFAULT_ID, cache_behaviour, pool, fetch_semaphore).await?;

                        if let Some(CacheValue::$variant(value)) = entry.map(|x| x.data).flatten() {
                            Ok(Some(value))
                        } else {
                            Ok(None)
                        }
                    }
                }
            )*
        }
    }
}

impl_cache_methods!(
    (ProjectV3, ProjectV3),
    (User, User),
    (Team, Vec<TeamMember>),
    (Organization, Organization),
    (File, ModrinthHashMatch),
    (LoaderManifest, CachedLoaderManifest),
    (FileHash, CachedFileHash),
    (FileUpdate, CachedFileUpdate),
    (SearchResults, SearchResults),
    (SearchResultsV3, SearchResultsV3)
);

impl CachedEntry {
    pub async fn get_project(
        id: &ModrinthProjectId,
        cache_behaviour: Option<CacheBehaviour>,
        pool: &SqlitePool,
        fetch_semaphore: &FetchSemaphore,
    ) -> crate::Result<Option<Project>> {
        Ok(Self::get_project_many(
            std::slice::from_ref(id),
            cache_behaviour,
            pool,
            fetch_semaphore,
        )
        .await?
        .into_iter()
        .next())
    }

    pub async fn get_project_many(
        ids: &[ModrinthProjectId],
        cache_behaviour: Option<CacheBehaviour>,
        pool: &SqlitePool,
        fetch_semaphore: &FetchSemaphore,
    ) -> crate::Result<Vec<Project>> {
        let id_refs = ids
            .iter()
            .map(ModrinthProjectId::as_str)
            .collect::<Vec<_>>();
        let entries = Self::get_many(
            CacheValueType::Project,
            &id_refs,
            cache_behaviour,
            pool,
            fetch_semaphore,
        )
        .await?;
        Ok(entries
            .into_iter()
            .filter_map(|entry| match entry.data {
                Some(CacheValue::Project(project)) => Some(project),
                _ => None,
            })
            .collect())
    }

    pub async fn get_curseforge_project(
        id: &CurseForgeProjectId,
        cache_behaviour: Option<CacheBehaviour>,
        pool: &SqlitePool,
        fetch_semaphore: &FetchSemaphore,
    ) -> crate::Result<Option<CurseForgeProject>> {
        Ok(Self::get_curseforge_project_many(
            std::slice::from_ref(id),
            cache_behaviour,
            pool,
            fetch_semaphore,
        )
        .await?
        .into_iter()
        .next())
    }

    pub async fn get_curseforge_project_many(
        ids: &[CurseForgeProjectId],
        cache_behaviour: Option<CacheBehaviour>,
        pool: &SqlitePool,
        fetch_semaphore: &FetchSemaphore,
    ) -> crate::Result<Vec<CurseForgeProject>> {
        let ids = ids
            .iter()
            .map(|id| id.get().to_string())
            .collect::<Vec<_>>();
        let id_refs = ids.iter().map(String::as_str).collect::<Vec<_>>();
        let entries = Self::get_many(
            CacheValueType::CurseForgeProject,
            &id_refs,
            cache_behaviour,
            pool,
            fetch_semaphore,
        )
        .await?;
        Ok(entries
            .into_iter()
            .filter_map(|entry| match entry.data {
                Some(CacheValue::CurseForgeProject(project)) => Some(project),
                _ => None,
            })
            .collect())
    }

    pub async fn get_version(
        id: &ModrinthVersionId,
        cache_behaviour: Option<CacheBehaviour>,
        pool: &SqlitePool,
        fetch_semaphore: &FetchSemaphore,
    ) -> crate::Result<Option<Version>> {
        Ok(Self::get_version_many(
            std::slice::from_ref(id),
            cache_behaviour,
            pool,
            fetch_semaphore,
        )
        .await?
        .into_iter()
        .next())
    }

    pub async fn get_version_many(
        ids: &[ModrinthVersionId],
        cache_behaviour: Option<CacheBehaviour>,
        pool: &SqlitePool,
        fetch_semaphore: &FetchSemaphore,
    ) -> crate::Result<Vec<Version>> {
        let id_refs = ids
            .iter()
            .map(ModrinthVersionId::as_str)
            .collect::<Vec<_>>();
        let entries = Self::get_many(
            CacheValueType::Version,
            &id_refs,
            cache_behaviour,
            pool,
            fetch_semaphore,
        )
        .await?;
        Ok(entries
            .into_iter()
            .filter_map(|entry| match entry.data {
                Some(CacheValue::Version(version)) => Some(version),
                _ => None,
            })
            .collect())
    }

    pub async fn get_version_v3(
        id: &ModrinthVersionId,
        cache_behaviour: Option<CacheBehaviour>,
        pool: &SqlitePool,
        fetch_semaphore: &FetchSemaphore,
    ) -> crate::Result<Option<VersionV3>> {
        Ok(Self::get_version_v3_many(
            std::slice::from_ref(id),
            cache_behaviour,
            pool,
            fetch_semaphore,
        )
        .await?
        .into_iter()
        .next())
    }

    pub async fn get_version_v3_many(
        ids: &[ModrinthVersionId],
        cache_behaviour: Option<CacheBehaviour>,
        pool: &SqlitePool,
        fetch_semaphore: &FetchSemaphore,
    ) -> crate::Result<Vec<VersionV3>> {
        let id_refs = ids
            .iter()
            .map(ModrinthVersionId::as_str)
            .collect::<Vec<_>>();
        let entries = Self::get_many(
            CacheValueType::VersionV3,
            &id_refs,
            cache_behaviour,
            pool,
            fetch_semaphore,
        )
        .await?;
        Ok(entries
            .into_iter()
            .filter_map(|entry| match entry.data {
                Some(CacheValue::VersionV3(version)) => Some(version),
                _ => None,
            })
            .collect())
    }
}

#[cfg(test)]
mod cache_upsert_tests {
    use super::{CacheValue, CacheValueType, CachedEntry};
    use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};

    async fn create_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE cache (
                id TEXT NOT NULL,
                data_type TEXT NOT NULL,
                alias TEXT NULL,
                data JSONB NULL,
                expires INTEGER NOT NULL,
                UNIQUE (data_type, alias),
                PRIMARY KEY (id, data_type)
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        pool
    }

    fn entry(
        id: &str,
        type_: CacheValueType,
        alias: Option<&str>,
        expires: i64,
    ) -> CachedEntry {
        CachedEntry {
            id: id.to_string(),
            alias: alias.map(str::to_string),
            type_,
            data: None,
            expires,
        }
    }

    fn report_types_entry(
        id: &str,
        alias: Option<&str>,
        value: &str,
        expires: i64,
    ) -> CachedEntry {
        CachedEntry {
            id: id.to_string(),
            alias: alias.map(str::to_string),
            type_: CacheValueType::ReportTypes,
            data: Some(CacheValue::ReportTypes(vec![value.to_string()])),
            expires,
        }
    }

    async fn ids(pool: &SqlitePool) -> Vec<String> {
        sqlx::query_scalar("SELECT id FROM cache ORDER BY id")
            .fetch_all(pool)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn cache_upsert_replaces_matching_id() {
        let pool = create_pool().await;
        CachedEntry::upsert_many(
            &[report_types_entry(
                "same-id",
                Some("old-alias"),
                "old-data",
                1,
            )],
            &pool,
        )
        .await
        .unwrap();

        CachedEntry::upsert_many(
            &[report_types_entry(
                "same-id",
                Some("new-alias"),
                "new-data",
                2,
            )],
            &pool,
        )
        .await
        .unwrap();

        let row = sqlx::query_as::<_, (String, String, i64)>(
            "SELECT alias, json_extract(data, '$[0]'), expires
             FROM cache
             WHERE id = 'same-id' AND data_type = 'report_types'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(row, ("new-alias".to_string(), "new-data".to_string(), 2));
    }

    #[tokio::test]
    async fn cache_upsert_moves_alias_to_new_id() {
        let pool = create_pool().await;
        CachedEntry::upsert_many(
            &[entry(
                "old-id",
                CacheValueType::ReportTypes,
                Some("shared-alias"),
                1,
            )],
            &pool,
        )
        .await
        .unwrap();

        CachedEntry::upsert_many(
            &[entry(
                "new-id",
                CacheValueType::ReportTypes,
                Some("shared-alias"),
                2,
            )],
            &pool,
        )
        .await
        .unwrap();

        assert_eq!(ids(&pool).await, vec!["new-id"]);
    }

    #[tokio::test]
    async fn cache_upsert_resolves_id_and_alias_conflicts_with_different_rows()
    {
        let pool = create_pool().await;
        CachedEntry::upsert_many(
            &[
                entry("id-a", CacheValueType::ReportTypes, Some("alias-a"), 1),
                entry("id-b", CacheValueType::ReportTypes, Some("alias-b"), 1),
            ],
            &pool,
        )
        .await
        .unwrap();

        CachedEntry::upsert_many(
            &[entry(
                "id-a",
                CacheValueType::ReportTypes,
                Some("alias-b"),
                2,
            )],
            &pool,
        )
        .await
        .unwrap();

        let rows = sqlx::query_as::<_, (String, String, i64)>(
            "SELECT id, alias, expires FROM cache",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(rows, vec![("id-a".to_string(), "alias-b".to_string(), 2)]);
    }

    #[tokio::test]
    async fn cache_upsert_keeps_same_alias_across_types() {
        let pool = create_pool().await;
        CachedEntry::upsert_many(
            &[
                entry(
                    "report-types",
                    CacheValueType::ReportTypes,
                    Some("shared-alias"),
                    1,
                ),
                entry(
                    "loaders",
                    CacheValueType::Loaders,
                    Some("shared-alias"),
                    1,
                ),
            ],
            &pool,
        )
        .await
        .unwrap();

        assert_eq!(ids(&pool).await, vec!["loaders", "report-types"]);
    }

    #[tokio::test]
    async fn cache_upsert_keeps_multiple_null_aliases() {
        let pool = create_pool().await;
        CachedEntry::upsert_many(
            &[
                entry("id-a", CacheValueType::ReportTypes, None, 1),
                entry("id-b", CacheValueType::ReportTypes, None, 1),
            ],
            &pool,
        )
        .await
        .unwrap();

        assert_eq!(ids(&pool).await, vec!["id-a", "id-b"]);
    }

    #[tokio::test]
    async fn cache_upsert_batch_uses_last_matching_id() {
        let pool = create_pool().await;
        CachedEntry::upsert_many(
            &[
                entry(
                    "same-id",
                    CacheValueType::ReportTypes,
                    Some("first-alias"),
                    1,
                ),
                entry(
                    "same-id",
                    CacheValueType::ReportTypes,
                    Some("last-alias"),
                    2,
                ),
            ],
            &pool,
        )
        .await
        .unwrap();

        let row = sqlx::query_as::<_, (String, i64)>(
            "SELECT alias, expires FROM cache WHERE id = 'same-id'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(row, ("last-alias".to_string(), 2));
    }

    #[tokio::test]
    async fn cache_upsert_batch_uses_last_matching_alias() {
        let pool = create_pool().await;
        CachedEntry::upsert_many(
            &[
                entry(
                    "first-id",
                    CacheValueType::ReportTypes,
                    Some("same-alias"),
                    1,
                ),
                entry(
                    "last-id",
                    CacheValueType::ReportTypes,
                    Some("same-alias"),
                    2,
                ),
            ],
            &pool,
        )
        .await
        .unwrap();

        assert_eq!(ids(&pool).await, vec!["last-id"]);
    }

    #[tokio::test]
    async fn cache_upsert_empty_batch_preserves_existing_rows() {
        let pool = create_pool().await;
        CachedEntry::upsert_many(
            &[entry(
                "existing-id",
                CacheValueType::ReportTypes,
                Some("existing-alias"),
                1,
            )],
            &pool,
        )
        .await
        .unwrap();

        CachedEntry::upsert_many(&[], &pool).await.unwrap();

        assert_eq!(ids(&pool).await, vec!["existing-id"]);
    }

    #[tokio::test]
    async fn cache_upsert_statement_is_atomic_on_non_conflict_error() {
        let pool = create_pool().await;
        sqlx::query(
            "CREATE TRIGGER reject_bad_cache_entry
             BEFORE INSERT ON cache
             WHEN NEW.id = 'bad-id'
             BEGIN
                 SELECT RAISE(ABORT, 'forced failure');
             END",
        )
        .execute(&pool)
        .await
        .unwrap();

        let result = CachedEntry::upsert_many(
            &[
                entry("good-id", CacheValueType::ReportTypes, None, 1),
                entry("bad-id", CacheValueType::ReportTypes, None, 1),
            ],
            &pool,
        )
        .await;

        assert!(result.is_err());
        assert!(ids(&pool).await.is_empty());
    }

    #[tokio::test]
    async fn cache_upsert_reassigns_curseforge_slug_without_2067() {
        let pool = create_pool().await;
        CachedEntry::upsert_many(
            &[entry(
                "42",
                CacheValueType::CurseForgeProject,
                Some("atm10sky"),
                1,
            )],
            &pool,
        )
        .await
        .unwrap();

        CachedEntry::upsert_many(
            &[entry(
                "84",
                CacheValueType::CurseForgeProject,
                Some("atm10sky"),
                2,
            )],
            &pool,
        )
        .await
        .unwrap();

        assert_eq!(ids(&pool).await, vec!["84"]);
    }
}

#[cfg(test)]
mod fetched_cache_persistence_tests {
    use super::{
        CacheBehaviour, CacheRefreshSource, CacheValueType, CachedEntry,
    };
    use crate::util::fetch::FetchSemaphore;
    use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
    use std::io::Write;
    use std::sync::{Arc, Mutex};
    use tokio::sync::Semaphore;

    async fn create_pool(with_cache_table: bool) -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();

        if with_cache_table {
            sqlx::query(
                "CREATE TABLE cache (
                    id TEXT NOT NULL,
                    data_type TEXT NOT NULL,
                    alias TEXT NULL,
                    data JSONB NULL,
                    expires INTEGER NOT NULL,
                    UNIQUE (data_type, alias),
                    PRIMARY KEY (id, data_type)
                )",
            )
            .execute(&pool)
            .await
            .unwrap();
        }

        pool
    }

    fn entry(id: &str) -> CachedEntry {
        CachedEntry {
            id: id.to_string(),
            alias: None,
            type_: CacheValueType::CurseForgeProject,
            data: None,
            expires: 1,
        }
    }

    async fn make_pool_read_only(pool: &SqlitePool) {
        sqlx::query("PRAGMA query_only = TRUE")
            .execute(pool)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn background_cache_write_failure_is_nonfatal() {
        let pool = create_pool(true).await;
        make_pool_read_only(&pool).await;

        CachedEntry::persist_fetched_cache_best_effort(
            CacheValueType::CurseForgeProject,
            &[entry("background-entry")],
            &pool,
            CacheRefreshSource::Background,
        )
        .await;

        let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM cache")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn direct_cache_write_failure_still_propagates() {
        let pool = create_pool(true).await;
        make_pool_read_only(&pool).await;

        let result =
            CachedEntry::upsert_many(&[entry("direct-entry")], &pool).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn cache_read_failure_still_propagates() {
        let pool = create_pool(false).await;

        let result = CachedEntry::get_many(
            CacheValueType::ReportTypes,
            &["missing"],
            Some(CacheBehaviour::CacheOnly),
            &pool,
            &FetchSemaphore(Semaphore::new(1)),
        )
        .await;

        assert!(result.is_err());
    }

    struct TestWriter(Arc<Mutex<Vec<u8>>>);

    impl Write for TestWriter {
        fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buffer);
            Ok(buffer.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn cache_write_warning_contains_diagnostics_without_payload() {
        let pool = create_pool(true).await;
        make_pool_read_only(&pool).await;
        let error = CachedEntry::upsert_many(&[entry("secret-payload")], &pool)
            .await
            .unwrap_err();
        let output = Arc::new(Mutex::new(Vec::new()));
        let writer_output = Arc::clone(&output);
        let subscriber = tracing_subscriber::fmt()
            .without_time()
            .with_ansi(false)
            .with_target(false)
            .with_writer(move || TestWriter(Arc::clone(&writer_output)))
            .finish();

        tracing::subscriber::with_default(subscriber, || {
            CachedEntry::log_cache_write_failure(
                CacheValueType::CurseForgeProject,
                1,
                CacheRefreshSource::Foreground,
                &error,
            );
        });

        let output = String::from_utf8(output.lock().unwrap().clone()).unwrap();
        assert!(output.contains("cache_write_discarded=true"));
        assert!(output.contains("cache_type=\"curseforge_project\""));
        assert!(output.contains("entry_count=1"));
        assert!(output.contains("refresh_source=\"foreground\""));
        assert!(output.contains("error_category=\"sqlx_database\""));
        assert!(output.contains("database_code=\"8\""));
        assert!(!output.contains("secret-payload"));
    }
}

impl_cache_method_singular!(
    (MinecraftManifest, daedalus::minecraft::VersionManifest),
    (Categories, Vec<Category>),
    (ReportTypes, Vec<String>),
    (Loaders, Vec<Loader>),
    (GameVersions, Vec<GameVersion>),
    (DonationPlatforms, Vec<DonationPlatform>)
);

impl CachedEntry {
    async fn persist_fetched_cache_best_effort(
        type_: CacheValueType,
        entries: &[Self],
        pool: &SqlitePool,
        refresh_source: CacheRefreshSource,
    ) {
        if entries.is_empty() {
            return;
        }

        // SQLite only permits one writer. Fetched API cache data is
        // reconstructible, so never let it enter the gaps between install
        // database batches and delay the next content registration. Outside
        // an active install, coordinate refreshes so SQLx does not start
        // dozens of INSERT statements waiting on the same write lock.
        // Standalone cache tests can run without global application state and
        // simply use SQLite's normal locking.
        let app_state = crate::State::get_if_initialized();
        let _write_permit = match app_state.as_ref() {
            Some(state) if !state.install_job_cancellations.is_empty() => {
                tracing::debug!(
                    cache_type = ?type_,
                    entry_count = entries.len(),
                    "Skipping cache persistence while an install is active"
                );
                return;
            }
            Some(state) => match state.install_db_semaphore.try_acquire() {
                Ok(permit) => Some(permit),
                Err(_) => {
                    tracing::debug!(
                        cache_type = ?type_,
                        entry_count = entries.len(),
                        "Skipping cache refresh while database writer is busy"
                    );
                    return;
                }
            },
            None => None,
        };
        if let Err(error) = Self::upsert_many(entries, pool).await {
            Self::log_cache_write_failure(
                type_,
                entries.len(),
                refresh_source,
                &error,
            );
        }
    }

    fn log_cache_write_failure(
        type_: CacheValueType,
        entry_count: usize,
        refresh_source: CacheRefreshSource,
        error: &crate::Error,
    ) {
        let (error_category, database_code) = match error.raw.as_ref() {
            crate::ErrorKind::Sqlx(sqlx::Error::Database(database_error)) => (
                "sqlx_database",
                database_error.code().map(|code| code.into_owned()),
            ),
            crate::ErrorKind::Sqlx(_) => ("sqlx", None),
            _ => ("other", None),
        };

        tracing::warn!(
            cache_write_discarded = true,
            cache_type = type_.as_str(),
            entry_count,
            refresh_source = refresh_source.as_str(),
            error_category,
            database_code = database_code.as_deref().unwrap_or("none"),
            error = %error,
            "Failed to persist fetched cache entries; continuing with fetched values"
        );
    }

    #[tracing::instrument(skip(pool, fetch_semaphore))]
    pub async fn get(
        type_: CacheValueType,
        key: &str,
        cache_behaviour: Option<CacheBehaviour>,
        pool: &SqlitePool,
        fetch_semaphore: &FetchSemaphore,
    ) -> crate::Result<Option<Self>> {
        Ok(Self::get_many(
            type_,
            &[key],
            cache_behaviour,
            pool,
            fetch_semaphore,
        )
        .await?
        .into_iter()
        .next())
    }

    #[tracing::instrument(skip(pool, fetch_semaphore))]
    pub async fn get_many(
        type_: CacheValueType,
        keys: &[&str],
        cache_behaviour: Option<CacheBehaviour>,
        pool: &SqlitePool,
        fetch_semaphore: &FetchSemaphore,
    ) -> crate::Result<Vec<Self>> {
        if keys.is_empty() {
            return Ok(Vec::new());
        }

        if crate::resources::protocol::owns(type_) {
            let state = crate::State::get().await?;
            let service = state
                .resources()
                .await
                .map_err(crate::resources::protocol::error)?;
            return crate::resources::protocol::many(
                service,
                type_,
                keys,
                cache_behaviour,
                &tokio_util::sync::CancellationToken::new(),
            )
            .await;
        }

        let cache_behaviour = cache_behaviour.unwrap_or_default();

        let remaining_keys = DashSet::new();
        for key in keys {
            remaining_keys.insert(*key);
        }

        let mut return_vals = Vec::new();
        let expired_keys = DashSet::new();
        let background_refresh_keys = DashSet::new();
        let mut cache_read_error: Option<crate::Error> = None;

        if cache_behaviour != CacheBehaviour::Bypass {
            let type_str = type_.as_str();
            let serialized_keys = serde_json::to_string(&keys)?;
            let alias_keys = if type_.case_sensitive_alias().unwrap_or(true) {
                serialized_keys.clone()
            } else {
                serde_json::to_string(
                    &keys.iter().map(|x| x.to_lowercase()).collect::<Vec<_>>(),
                )?
            };

            // unsupported type NULL of column #3 ("data"), so cannot be compile time type checked
            // https://github.com/launchbadge/sqlx/issues/1979
            let query = sqlx::query!(
                r#"
                SELECT id, data_type, json(data) as "data?: serde_json::Value", alias, expires
                FROM cache
                WHERE data_type = $1 AND (
                    id IN (SELECT value FROM json_each($2))
                    OR
                    alias IN (SELECT value FROM json_each($3))
                )
                "#,
                type_str,
                serialized_keys,
                alias_keys
            )
            .fetch_all(pool)
            .await;

            let query = match query {
                Ok(query) => query,
                Err(error) => {
                    cache_read_error = Some(error.into());
                    Vec::new()
                }
            };

            let now = Utc::now().timestamp();
            for row in query {
                let parsed_data = if let Some(data) = row.data.clone() {
                    match Self::deserialize_cache_value(type_, data, &row.id) {
                        Ok(value) => Some(value),
                        Err(error) => {
                            cache_read_error = Some(error);
                            break;
                        }
                    }
                } else {
                    None
                };

                if cache_entry_expired(type_, row.expires, now) {
                    if cache_behaviour == CacheBehaviour::MustRevalidate {
                        continue;
                    } else {
                        expired_keys.insert(row.id.clone());
                    }
                } else if parsed_data.is_some()
                    && row.expires - type_.expiry()
                        + BACKGROUND_REFRESH_THRESHOLD
                        <= now
                {
                    background_refresh_keys.insert(row.id.clone());
                }

                let row_id = row.id.clone();
                let row_alias = row.alias.clone();
                let remove_matching_key = |x: &&str| {
                    x != &&*row_id
                        && !row_alias.as_ref().is_some_and(|y| {
                            if type_.case_sensitive_alias().unwrap_or(true) {
                                x == y
                            } else {
                                y.to_lowercase() == x.to_lowercase()
                            }
                        })
                };

                if let Some(data) = parsed_data {
                    if data.get_type() != type_ {
                        cache_read_error = Some(crate::ErrorKind::OtherError(format!(
                            "Cache type mismatch for id {}: expected {:?}, got {:?}",
                            row.id,
                            type_,
                            data.get_type()
                        ))
                        .as_error());
                        break;
                    }

                    if data.is_empty_collection() {
                        // An empty tag collection is not trustworthy: keep the
                        // key in `remaining_keys` so it is refetched (with
                        // mirror fallback) instead of serving an empty result.
                        continue;
                    }

                    remaining_keys.retain(remove_matching_key);

                    return_vals.push(Self {
                        id: row.id,
                        alias: row.alias,
                        type_: CacheValueType::from_string(&row.data_type),
                        data: Some(data),
                        expires: row.expires,
                    });
                } else {
                    remaining_keys.retain(remove_matching_key);
                }
            }

            if cache_read_error.is_some() {
                return_vals.clear();
                expired_keys.clear();
                background_refresh_keys.clear();
                remaining_keys.clear();
                for key in keys {
                    remaining_keys.insert(*key);
                }
            }
        }

        if cache_behaviour == CacheBehaviour::CacheOnly
            && let Some(cache_error) = cache_read_error.take()
        {
            return Err(cache_read_failure(type_, cache_error, None));
        }

        if !remaining_keys.is_empty()
            && cache_behaviour != CacheBehaviour::CacheOnly
        {
            let res = Self::fetch_many(
                type_,
                remaining_keys.clone(),
                fetch_semaphore,
                pool,
                cache_behaviour == CacheBehaviour::Bypass,
            )
            .await;

            match res {
                Err(remote_error) => {
                    if let Some(cache_error) = cache_read_error.take() {
                        return Err(cache_read_failure(
                            type_,
                            cache_error,
                            Some(remote_error),
                        ));
                    }
                    if cache_behaviour
                        == CacheBehaviour::StaleWhileRevalidateSkipOffline
                    {
                        for key in remaining_keys {
                            expired_keys.insert(key.to_string());
                        }
                    } else {
                        return Err(remote_error);
                    }
                }
                Ok(values) => {
                    if values.is_empty()
                        && let Some(cache_error) = cache_read_error.take()
                    {
                        return Err(cache_read_failure(
                            type_,
                            cache_error,
                            None,
                        ));
                    }
                    let entries =
                        values.iter().map(|x| x.0.clone()).collect::<Vec<_>>();

                    Self::persist_fetched_cache_best_effort(
                        type_,
                        &entries,
                        pool,
                        CacheRefreshSource::Foreground,
                    )
                    .await;

                    if !values.is_empty() {
                        return_vals.append(
                            &mut values
                                .into_iter()
                                .filter(|(_, include)| *include)
                                .map(|x| x.0)
                                .collect::<Vec<_>>(),
                        );
                    }
                }
            }
        }

        let should_background_refresh = cache_behaviour
            == CacheBehaviour::StaleWhileRevalidate
            || cache_behaviour
                == CacheBehaviour::StaleWhileRevalidateSkipOffline;

        if should_background_refresh {
            for key in background_refresh_keys {
                expired_keys.insert(key);
            }
        }

        if !expired_keys.is_empty() && should_background_refresh {
            tokio::task::spawn(async move {
                let result = async {
                    // TODO: if possible- find a way to do this without invoking state get
                    let state = crate::state::State::get().await?;

                    let values = Self::fetch_many(
                        type_,
                        expired_keys,
                        &state.api_semaphore,
                        &state.pool,
                        false,
                    )
                    .await?
                    .into_iter()
                    .map(|x| x.0)
                    .collect::<Vec<_>>();

                    Self::persist_fetched_cache_best_effort(
                        type_,
                        &values,
                        &state.pool,
                        CacheRefreshSource::Background,
                    )
                    .await;

                    Ok::<(), crate::Error>(())
                }
                .await;

                if let Err(error) = result {
                    tracing::warn!(
                        cache_type = type_.as_str(),
                        refresh_source = CacheRefreshSource::Background.as_str(),
                        error = %error,
                        "Background cache refresh failed"
                    );
                }
            });
        }

        Ok(return_vals)
    }

    async fn fetch_many(
        type_: CacheValueType,
        keys: DashSet<impl Display + Eq + Hash + Serialize>,
        fetch_semaphore: &FetchSemaphore,
        pool: &SqlitePool,
        force_refresh: bool,
    ) -> crate::Result<Vec<(Self, bool)>> {
        async fn fetch_many_batched<T: DeserializeOwned>(
            method: Method,
            api_url: &str,
            url: &str,
            uri_path: Option<&'static str>,
            keys: &DashSet<impl Display + Eq + Hash + Serialize>,
            fetch_semaphore: &FetchSemaphore,
            pool: &SqlitePool,
        ) -> crate::Result<Vec<T>> {
            const MAX_REQUEST_SIZE: usize = 800;

            let urls = keys
                .iter()
                .collect::<Vec<_>>()
                .chunks(MAX_REQUEST_SIZE)
                .map(|chunk| {
                    encode_json_query_value(&chunk)
                        .map(|keys| format!("{api_url}{url}{keys}"))
                })
                .collect::<Result<Vec<_>, _>>()?;

            let res = futures::future::try_join_all(urls.iter().map(|url| {
                fetch_json::<Vec<_>>(
                    method.clone(),
                    url,
                    None,
                    None,
                    uri_path,
                    fetch_semaphore,
                    pool,
                )
            }))
            .await?;

            Ok(res.into_iter().flatten().collect())
        }

        macro_rules! fetch_original_values {
            ($type:ident, $api_url:expr, $url_suffix:expr, $uri_path:expr, $cache_variant:path) => {{
                let mut results = fetch_many_batched(
                    Method::GET,
                    $api_url,
                    &format!("{}?ids=", $url_suffix),
                    $uri_path,
                    &keys,
                    &fetch_semaphore,
                    &pool,
                )
                .await?
                .into_iter()
                .map($cache_variant)
                .collect::<Vec<_>>();

                let mut values = vec![];
                let visited_keys = DashSet::new();

                for key in keys {
                    let key = key.to_string();
                    let lower_case_key = key.to_lowercase();
                    let case_sensitive = CacheValueType::$type
                        .case_sensitive_alias()
                        .unwrap_or(true);

                    if let Some(position) = results.iter().position(|x| {
                        x.get_key() == key
                            || x.get_alias()
                                .map(|x| {
                                    if case_sensitive {
                                        x == key
                                    } else {
                                        x == lower_case_key
                                    }
                                })
                                .unwrap_or(false)
                    }) {
                        visited_keys.insert(key);
                        if !case_sensitive {
                            visited_keys.insert(lower_case_key);
                        }

                        let result = results.remove(position);

                        values.push((result.get_entry(), true));
                    } else if !visited_keys.contains(&key)
                        && (case_sensitive
                            || !visited_keys.contains(&lower_case_key))
                    {
                        values.push((
                            CacheValueType::$type.get_empty_entry(key),
                            true,
                        ));
                    }
                }

                values
            }};
        }

        macro_rules! fetch_original_value {
            ($type:ident, $api_url:expr, $url_suffix:expr, $uri_path:expr, $cache_variant:path) => {{
                vec![(
                    $cache_variant(
                        fetch_json_nonempty(
                            Method::GET,
                            &*format!("{}{}", $api_url, $url_suffix),
                            None,
                            None,
                            $uri_path,
                            &fetch_semaphore,
                            pool,
                        )
                        .await?,
                    )
                    .get_entry(),
                    true,
                )]
            }};
        }

        Ok(match type_ {
            CacheValueType::Project
            | CacheValueType::ProjectV3
            | CacheValueType::CurseForgeProject
            | CacheValueType::Version
            | CacheValueType::VersionV3
            | CacheValueType::File
            | CacheValueType::ProjectVersions
            | CacheValueType::SearchResults
            | CacheValueType::SearchResultsV3
            | CacheValueType::FileUpdate => {
                return Err(crate::ErrorKind::InputError(
                    "resource queries must use ResourceService".into(),
                )
                .into());
            }

            CacheValueType::User => {
                fetch_original_values!(
                    User,
                    env!("MODRINTH_API_URL"),
                    "users",
                    Some("/v2/users"),
                    CacheValue::User
                )
            }
            CacheValueType::Team => {
                let mut teams = fetch_many_batched::<Vec<TeamMember>>(
                    Method::GET,
                    env!("MODRINTH_API_URL_V3"),
                    "teams?ids=",
                    Some("/v3/teams"),
                    &keys,
                    fetch_semaphore,
                    pool,
                )
                .await?;

                let mut values = vec![];
                for key in keys {
                    let key = key.to_string();

                    if let Some(position) = teams.iter().position(|x| {
                        x.first().is_some_and(|x| x.team_id == key)
                    }) {
                        let team = teams.remove(position);

                        for member in &team {
                            values.push((
                                CacheValue::User(member.user.clone())
                                    .get_entry(),
                                false,
                            ));
                        }

                        values.push((CacheValue::Team(team).get_entry(), true))
                    } else {
                        values.push((
                            CacheValueType::Team.get_empty_entry(key),
                            true,
                        ))
                    }
                }

                values
            }
            CacheValueType::Organization => {
                let mut orgs = fetch_many_batched::<Organization>(
                    Method::GET,
                    env!("MODRINTH_API_URL_V3"),
                    "organizations?ids=",
                    Some("/v3/organizations"),
                    &keys,
                    fetch_semaphore,
                    pool,
                )
                .await?;

                let mut values = vec![];
                let visited_keys = DashSet::new();

                for key in keys {
                    let id = key.to_string();
                    let slug = id.to_lowercase();

                    if let Some(position) = orgs.iter().position(|x| {
                        x.id == id || x.slug.to_lowercase() == slug
                    }) {
                        visited_keys.insert(id);
                        visited_keys.insert(slug);

                        let org = orgs.remove(position);

                        for member in &org.members {
                            values.push((
                                CacheValue::User(member.user.clone())
                                    .get_entry(),
                                false,
                            ));
                        }

                        values.push((
                            CacheValue::Team(org.members.clone()).get_entry(),
                            false,
                        ));

                        values.push((
                            CacheValue::Organization(org).get_entry(),
                            true,
                        ));
                    } else if !visited_keys.contains(&id)
                        && !visited_keys.contains(&slug)
                    {
                        values.push((
                            CacheValueType::Organization.get_empty_entry(id),
                            true,
                        ));
                    }
                }

                values
            }

            CacheValueType::LoaderManifest => {
                let fetch_urls = keys
                    .iter()
                    .map(|x| {
                        let metadata =
                            daedalus::modded::loader_manifest_metadata_from_cache_key(
                                &x.key().to_string(),
                            );

                        (
                            metadata.cache_key,
                            metadata.loader,
                            metadata.game_version,
                            format!(
                                "{}{}",
                                env!("MODRINTH_LAUNCHER_META_URL"),
                                metadata.path,
                            ),
                        )
                    })
                    .collect::<Vec<_>>();

                futures::future::try_join_all(fetch_urls.iter().map(
                    |(_, loader, game_version, url)| {
                        crate::api::loader_metadata::fetch_loader_manifest_official_first(
                            loader,
                            game_version.as_deref(),
                            url,
                            fetch_semaphore,
                            pool,
                            force_refresh,
                        )
                    },
                ))
                .await?
                .into_iter()
                .enumerate()
                .map(|(index, metadata)| {
                    let mut entry = CacheValue::LoaderManifest(metadata)
                        .get_entry();
                    entry.id.clone_from(&fetch_urls[index].0);

                    (entry, true)
                })
                .collect()
            }
            CacheValueType::MinecraftManifest => {
                let launcher_meta_url = format!(
                    "{}minecraft/v{}/manifest.json",
                    env!("MODRINTH_LAUNCHER_META_URL"),
                    daedalus::minecraft::CURRENT_FORMAT_VERSION
                );
                let launcher_meta = tokio::time::timeout(
                    std::time::Duration::from_secs(2),
                    fetch_json(
                        Method::GET,
                        &launcher_meta_url,
                        None,
                        None,
                        None,
                        fetch_semaphore,
                        pool,
                    ),
                )
                .await;
                let manifest = match launcher_meta {
                    Ok(Ok(manifest)) => manifest,
                    Ok(Err(error)) => {
                        tracing::warn!(
                            url = launcher_meta_url,
                            error = %error,
                            "Launcher metadata failed; falling back to Mojang manifest"
                        );
                        fetch_json(
                            Method::GET,
                            daedalus::minecraft::VERSION_MANIFEST_URL,
                            None,
                            None,
                            None,
                            fetch_semaphore,
                            pool,
                        )
                        .await?
                    }
                    Err(_) => {
                        tracing::warn!(
                            url = launcher_meta_url,
                            "Launcher metadata was slow; falling back to Mojang manifest"
                        );
                        fetch_json(
                            Method::GET,
                            daedalus::minecraft::VERSION_MANIFEST_URL,
                            None,
                            None,
                            None,
                            fetch_semaphore,
                            pool,
                        )
                        .await?
                    }
                };
                vec![(
                    CacheValue::MinecraftManifest(manifest).get_entry(),
                    true,
                )]
            }
            CacheValueType::Categories => {
                fetch_original_value!(
                    Categories,
                    env!("MODRINTH_API_URL"),
                    "tag/category",
                    Some("/v2/tag/category"),
                    CacheValue::Categories
                )
            }
            CacheValueType::ReportTypes => {
                fetch_original_value!(
                    ReportTypes,
                    env!("MODRINTH_API_URL"),
                    "tag/report_type",
                    Some("/v2/tag/report_type"),
                    CacheValue::ReportTypes
                )
            }
            CacheValueType::Loaders => {
                fetch_original_value!(
                    Loaders,
                    env!("MODRINTH_API_URL"),
                    "tag/loader",
                    Some("/v2/tag/loader"),
                    CacheValue::Loaders
                )
            }
            CacheValueType::GameVersions => {
                fetch_original_value!(
                    GameVersions,
                    env!("MODRINTH_API_URL"),
                    "tag/game_version",
                    Some("/v2/tag/game_version"),
                    CacheValue::GameVersions
                )
            }
            CacheValueType::DonationPlatforms => {
                fetch_original_value!(
                    DonationPlatforms,
                    env!("MODRINTH_API_URL"),
                    "tag/donation_platform",
                    Some("/v2/tag/donation_platform"),
                    CacheValue::DonationPlatforms
                )
            }
            CacheValueType::FileHash => {
                let state = crate::State::get().await?;

                // The cache key is `{size}-{instance.path}/{relative}`. Resolve
                // each instance's game working directory (which honours a
                // per-instance `game_dir_override`) ONCE per distinct instance
                // path, instead of once per file, so override-instance content
                // is hashed from the correct root without issuing N identical
                // DB lookups per content scan.
                let keys: Vec<String> =
                    keys.into_iter().map(|k| k.to_string()).collect();

                let mut base_dirs: HashMap<String, PathBuf> = HashMap::new();
                for key in &keys {
                    let path =
                        key.split_once('-').map(|x| x.1).unwrap_or_default();
                    // Directly associated instances embed an absolute linked
                    // root in the key; it needs no base resolution.
                    if std::path::Path::new(path).is_absolute() {
                        continue;
                    }
                    if let Some((instance_path, _)) =
                        path.split_once('/').or_else(|| path.split_once('\\'))
                        && !base_dirs.contains_key(instance_path)
                    {
                        let override_dir = crate::state::instances::adapters::sqlite::instance_rows::get_game_dir_override_by_path(
                                    instance_path,
                                    &state.pool,
                                )
                                .await?;
                        base_dirs.insert(
                            instance_path.to_string(),
                            state.directories.resolve_game_dir(
                                instance_path,
                                override_dir.as_deref(),
                            ),
                        );
                    }
                }

                async fn hash_file(
                    state: &crate::State,
                    base_dirs: &HashMap<String, PathBuf>,
                    key: String,
                ) -> crate::Result<(CachedEntry, bool)> {
                    let path =
                        key.split_once('-').map(|x| x.1).unwrap_or_default();

                    // Absolute keys (directly associated instances) point at
                    // the linked installation itself; joining would strip the
                    // base, so use them as-is. `Path::join` would actually
                    // replace the base for absolute paths, but the split below
                    // destroys the leading separator first.
                    let full_path = if std::path::Path::new(path).is_absolute()
                    {
                        PathBuf::from(path)
                    } else {
                        let (base_dir, relative) = match path
                            .split_once('/')
                            .or_else(|| path.split_once('\\'))
                        {
                            Some((instance_path, relative)) => {
                                let base_dir = base_dirs
                                    .get(instance_path)
                                    .cloned()
                                    .unwrap_or_else(|| {
                                        state.directories.instances_dir()
                                    });
                                (base_dir, relative.to_string())
                            }
                            None => (
                                state.directories.instances_dir(),
                                path.to_string(),
                            ),
                        };
                        base_dir.join(relative)
                    };

                    let mut file = tokio::fs::File::open(&full_path).await?;
                    let size = file.metadata().await?.len();

                    let mut hasher = sha1_smol::Sha1::new();

                    let mut buffer = vec![0u8; 262144]; // 256KiB
                    loop {
                        use tokio::io::AsyncReadExt;
                        let bytes_read = file.read(&mut buffer).await?;
                        if bytes_read == 0 {
                            break;
                        }
                        hasher.update(&buffer[..bytes_read]);
                    }

                    let hash = hasher.digest().to_string();

                    Ok((
                        CacheValue::FileHash(CachedFileHash {
                            path: path.to_string(),
                            size,
                            hash,
                            project_type: ProjectType::get_from_parent_folder(
                                &full_path,
                            ),
                            project_id: None,
                            version_id: None,
                        })
                        .get_entry(),
                        true,
                    ))
                }

                use futures::stream::StreamExt;
                let results: Vec<_> = futures::stream::iter(keys)
                    .map(|x| hash_file(&state, &base_dirs, x))
                    .buffer_unordered(64) // hash 64 files at once
                    .collect::<Vec<_>>()
                    .await
                    .into_iter()
                    .filter_map(|x| x.ok())
                    .collect();

                results
            }

            CacheValueType::ModpackFiles => {
                // ModpackFiles are only stored locally during modpack installation,
                // not fetched from an external API
                vec![]
            }
        })
    }

    fn deserialize_cache_value(
        type_: CacheValueType,
        data: serde_json::Value,
        id: &str,
    ) -> crate::Result<CacheValue> {
        fn parse<T: DeserializeOwned>(
            data: serde_json::Value,
            id: &str,
            label: &str,
        ) -> crate::Result<T> {
            serde_json::from_value::<T>(data.clone()).map_err(|err| {
                crate::ErrorKind::OtherError(format!(
                    "Failed to deserialize cache {label} for id {id}: {err}\n\ndata:\n{}",
                    serde_json::to_string_pretty(&data).unwrap(),
                ))
                .as_error()
            })
        }

        let value = match type_ {
            CacheValueType::Project => {
                CacheValue::Project(parse(data, id, "project")?)
            }
            CacheValueType::ProjectV3 => {
                CacheValue::ProjectV3(parse(data, id, "project_v3")?)
            }
            CacheValueType::CurseForgeProject => CacheValue::CurseForgeProject(
                parse(data, id, "curseforge_project")?,
            ),
            CacheValueType::Version => {
                CacheValue::Version(parse(data, id, "version")?)
            }
            CacheValueType::VersionV3 => {
                CacheValue::VersionV3(parse(data, id, "version_v3")?)
            }
            CacheValueType::User => CacheValue::User(parse(data, id, "user")?),
            CacheValueType::Team => CacheValue::Team(parse(data, id, "team")?),
            CacheValueType::Organization => {
                CacheValue::Organization(parse(data, id, "organization")?)
            }
            CacheValueType::File => CacheValue::File(parse(data, id, "file")?),
            CacheValueType::LoaderManifest => {
                CacheValue::LoaderManifest(parse(data, id, "loader_manifest")?)
            }
            CacheValueType::MinecraftManifest => CacheValue::MinecraftManifest(
                parse(data, id, "minecraft_manifest")?,
            ),
            CacheValueType::Categories => {
                CacheValue::Categories(parse(data, id, "categories")?)
            }
            CacheValueType::ReportTypes => {
                CacheValue::ReportTypes(parse(data, id, "report_types")?)
            }
            CacheValueType::Loaders => {
                CacheValue::Loaders(parse(data, id, "loaders")?)
            }
            CacheValueType::GameVersions => {
                CacheValue::GameVersions(parse(data, id, "game_versions")?)
            }
            CacheValueType::DonationPlatforms => CacheValue::DonationPlatforms(
                parse(data, id, "donation_platforms")?,
            ),
            CacheValueType::FileHash => {
                CacheValue::FileHash(parse(data, id, "file_hash")?)
            }
            CacheValueType::FileUpdate => {
                CacheValue::FileUpdate(parse(data, id, "file_update")?)
            }
            CacheValueType::SearchResults => {
                CacheValue::SearchResults(parse(data, id, "search_results")?)
            }
            CacheValueType::SearchResultsV3 => CacheValue::SearchResultsV3(
                parse(data, id, "search_results_v3")?,
            ),
            CacheValueType::ModpackFiles => {
                CacheValue::ModpackFiles(parse(data, id, "modpack_files")?)
            }
            CacheValueType::ProjectVersions => CacheValue::ProjectVersions(
                parse(data, id, "project_versions")?,
            ),
        };

        Ok(value)
    }

    pub(crate) async fn upsert_many(
        items: &[Self],
        exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
    ) -> crate::Result<()> {
        if items.is_empty() {
            return Ok(());
        }

        let mut last_id_positions = HashMap::new();
        let mut last_alias_positions = HashMap::new();

        for (position, item) in items.iter().enumerate() {
            last_id_positions
                .insert((item.id.as_str(), item.type_.as_str()), position);

            if let Some(alias) = item.alias.as_deref() {
                last_alias_positions
                    .insert((item.type_.as_str(), alias), position);
            }
        }

        let items = items
            .iter()
            .enumerate()
            .filter(|(position, item)| {
                let is_last_id = last_id_positions
                    .get(&(item.id.as_str(), item.type_.as_str()))
                    == Some(position);
                let is_last_alias = item.alias.as_deref().is_none_or(|alias| {
                    last_alias_positions.get(&(item.type_.as_str(), alias))
                        == Some(position)
                });

                is_last_id && is_last_alias
            })
            .map(|(_, item)| item)
            .map(|item| {
                let data = item
                    .data
                    .as_ref()
                    .map(|value| value.to_json_value())
                    .transpose()?;

                Ok(serde_json::json!({
                    "id": item.id,
                    "data_type": item.type_.as_str(),
                    "alias": item.alias,
                    "data": data,
                    "expires": item.expires,
                }))
            })
            .collect::<crate::Result<Vec<_>>>()?;
        let items = serde_json::to_string(&items)?;

        sqlx::query(
            "
            INSERT OR REPLACE INTO cache (id, data_type, alias, data, expires)
                SELECT
                    json_extract(value, '$.id') AS id,
                    json_extract(value, '$.data_type') AS data_type,
                    json_extract(value, '$.alias') AS alias,
                    json_extract(value, '$.data') AS data,
                    json_extract(value, '$.expires') AS expires
                FROM
                    json_each($1)
            ",
        )
        .bind(items)
        .execute(exec)
        .await?;

        Ok(())
    }

    pub async fn purge_cache_types(
        cache_types: &[CacheValueType],
        exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
    ) -> crate::Result<()> {
        let cache_types = serde_json::to_string(&cache_types)?;

        sqlx::query!(
            "
            DELETE FROM cache
            WHERE data_type IN (SELECT value FROM json_each($1))
            ",
            cache_types,
        )
        .execute(exec)
        .await?;

        Ok(())
    }

    pub async fn cache_modpack_files(
        version_id: &str,
        file_hashes: Vec<String>,
        project_ids: Vec<String>,
        _pool: &SqlitePool,
    ) -> crate::Result<()> {
        let state = crate::State::get().await?;
        let service = state
            .resources()
            .await
            .map_err(crate::resources::protocol::error)?;
        crate::resources::packs::save_index(
            service.cache(),
            &CachedModpackFiles {
                version_id: version_id.into(),
                file_hashes,
                project_ids,
            },
        )
        .await
        .map_err(crate::resources::protocol::error)
    }

    pub(crate) async fn cache_modpack_files_best_effort(
        version_id: &str,
        file_hashes: Vec<String>,
        project_ids: Vec<String>,
        pool: &SqlitePool,
    ) {
        if let Err(error) = Self::cache_modpack_files(
            version_id,
            file_hashes,
            project_ids,
            pool,
        )
        .await
        {
            tracing::warn!(%error, "Pack index cache write failed");
        }
    }

    pub async fn get_modpack_files(
        version_id: &str,
        pool: &SqlitePool,
        _fetch_semaphore: &FetchSemaphore,
    ) -> crate::Result<Option<CachedModpackFiles>> {
        let state = crate::State::get().await?;
        let service = state
            .resources()
            .await
            .map_err(crate::resources::protocol::error)?;
        if let Some(index) =
            crate::resources::packs::index(service.cache(), version_id).await
        {
            return Ok(Some(index));
        }
        if service.cache().generation().await > 0 {
            return Ok(None);
        }
        let data: Option<String> = sqlx::query_scalar("SELECT data FROM cache WHERE id = ? AND data_type = 'modpack_files' AND length(CAST(data AS BLOB)) <= 1048576").bind(version_id).fetch_optional(pool).await?;
        let index = data
            .and_then(|data| {
                serde_json::from_str::<CachedModpackFiles>(&data).ok()
            })
            .filter(|index| {
                index.version_id == version_id
                    && crate::resources::packs::valid_index(index)
            });
        if let Some(index) = &index {
            let _ = crate::resources::packs::save_index(service.cache(), index)
                .await;
        }
        Ok(index)
    }

    /// Get versions for a project (without changelogs for fast loading)
    #[tracing::instrument(skip(pool, fetch_semaphore))]
    pub async fn get_project_versions(
        project_id: &ModrinthProjectId,
        cache_behaviour: Option<CacheBehaviour>,
        pool: &SqlitePool,
        fetch_semaphore: &FetchSemaphore,
    ) -> crate::Result<Option<Vec<Version>>> {
        let entry = Self::get(
            CacheValueType::ProjectVersions,
            project_id.as_str(),
            cache_behaviour,
            pool,
            fetch_semaphore,
        )
        .await?;

        if let Some(CachedEntry {
            data: Some(CacheValue::ProjectVersions(pv)),
            ..
        }) = entry
        {
            return Ok(Some(pv.versions));
        }

        Ok(None)
    }
}

pub async fn cache_file_hash(
    bytes: bytes::Bytes,
    instance_path: &str,
    path: &str,
    known_hash: Option<&str>,
    project_type: Option<ProjectType>,
    known_modrinth_file: Option<KnownModrinthFile<'_>>,
    exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
) -> crate::Result<()> {
    let size = bytes.len();

    let hash = if let Some(known_hash) = known_hash {
        known_hash.to_string()
    } else {
        sha1_async(bytes).await?
    };

    cache_file_hash_metadata(
        instance_path,
        path,
        size as u64,
        hash,
        project_type,
        known_modrinth_file,
        exec,
    )
    .await
}

pub async fn cache_file_hash_metadata(
    instance_path: &str,
    path: &str,
    size: u64,
    hash: String,
    project_type: Option<ProjectType>,
    known_modrinth_file: Option<KnownModrinthFile<'_>>,
    exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
) -> crate::Result<()> {
    let (project_id, version_id) =
        known_modrinth_file.map_or((None, None), |metadata| {
            (
                Some(metadata.project_id.to_string()),
                Some(metadata.version_id.to_string()),
            )
        });

    // Streamed extraction already computed these values, so avoid buffering the file just to cache them.
    CachedEntry::upsert_many(
        &[CacheValue::FileHash(CachedFileHash {
            path: format!("{instance_path}/{path}"),
            size,
            hash,
            project_type,
            project_id,
            version_id,
        })
        .get_entry()],
        exec,
    )
    .await?;

    Ok(())
}

#[cfg(test)]
mod game_version_cache_tests {
    use super::{
        CacheBehaviour, CacheValue, CachedEntry, CachedProjectVersions,
        GameVersion,
    };
    use crate::util::fetch::FetchSemaphore;
    use sqlx::sqlite::SqlitePoolOptions;
    use tokio::sync::Semaphore;

    fn sample_game_versions() -> Vec<GameVersion> {
        vec![
            GameVersion {
                version: "26.2".to_string(),
                version_type: "release".to_string(),
                date: "2026-06-16T12:03:33Z".to_string(),
                major: false,
            },
            GameVersion {
                version: "26.3-snapshot-6".to_string(),
                version_type: "snapshot".to_string(),
                date: "2026-07-28T12:25:51Z".to_string(),
                major: false,
            },
        ]
    }

    #[test]
    fn empty_collection_values_are_detected() {
        assert!(CacheValue::GameVersions(vec![]).is_empty_collection());
        assert!(CacheValue::Loaders(vec![]).is_empty_collection());
        assert!(CacheValue::Categories(vec![]).is_empty_collection());
        assert!(CacheValue::DonationPlatforms(vec![]).is_empty_collection());
        assert!(
            !CacheValue::GameVersions(sample_game_versions())
                .is_empty_collection()
        );
        assert!(
            !CacheValue::ProjectVersions(CachedProjectVersions {
                project_id: "project".to_string(),
                versions: vec![],
            })
            .is_empty_collection(),
            "project version lists may legitimately be empty"
        );
    }

    async fn create_cache_table(pool: &sqlx::SqlitePool) {
        sqlx::query(
            "CREATE TABLE cache (
                id TEXT NOT NULL,
                data_type TEXT NOT NULL,
                alias TEXT NULL,
                data JSONB NULL,
                expires INTEGER NOT NULL,
                UNIQUE (data_type, alias),
                PRIMARY KEY (id, data_type)
            )",
        )
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn empty_cached_game_versions_are_refetched_instead_of_served() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        create_cache_table(&pool).await;
        let semaphore = FetchSemaphore(Semaphore::new(1));

        CachedEntry::upsert_many(
            &[CacheValue::GameVersions(vec![]).get_entry()],
            &pool,
        )
        .await
        .unwrap();

        let cached = CachedEntry::get_game_versions(
            Some(CacheBehaviour::CacheOnly),
            &pool,
            &semaphore,
        )
        .await
        .unwrap();
        assert!(
            cached.is_none(),
            "an empty cached game version collection must not be served"
        );

        CachedEntry::upsert_many(
            &[CacheValue::GameVersions(sample_game_versions()).get_entry()],
            &pool,
        )
        .await
        .unwrap();

        let cached = CachedEntry::get_game_versions(
            Some(CacheBehaviour::CacheOnly),
            &pool,
            &semaphore,
        )
        .await
        .unwrap();
        let versions =
            cached.expect("populated game versions should be served");
        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0].version, "26.2");
        assert_eq!(versions[1].version_type, "snapshot");
    }
}

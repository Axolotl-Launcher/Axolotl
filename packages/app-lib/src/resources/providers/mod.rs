use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::{ipc::*, *};

mod curseforge;
pub(crate) mod legacy;
mod mcarchive;
mod modrinth;
mod planet_minecraft;
pub(crate) mod search_filters;
mod transport;

pub use curseforge::CurseForgeProvider;
pub use mcarchive::McArchiveProvider;
pub use modrinth::ModrinthProvider;
pub use planet_minecraft::PlanetMinecraftProvider;
pub use transport::{ProtocolRequest, ProtocolTransport};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ProviderQuery {
    ReleaseProtocol {
        release_id: SourceReleaseId,
        version: String,
    },
    ReleasesProtocol {
        project_id: SourceProjectId,
    },
    SearchProtocol {
        query: String,
        version: String,
    },
    ProjectProtocol {
        locator: String,
        version: String,
    },
    Locate {
        locator: String,
    },
    Project {
        id: SourceProjectId,
        slug: Option<String>,
    },
    Description {
        id: SourceProjectId,
        slug: Option<String>,
    },
    Release {
        project_id: Option<SourceProjectId>,
        release_id: SourceReleaseId,
        slug: Option<String>,
    },
    Releases {
        project_id: SourceProjectId,
        slug: Option<String>,
        target: ResourceTarget,
        cursor: Option<String>,
        page_size: u32,
    },
    Changelog {
        project_id: SourceProjectId,
        release_id: SourceReleaseId,
        slug: Option<String>,
    },
    Search {
        query: ResourceSearchQuery,
        cursor: Option<String>,
    },
    Identify {
        integrity: ArtifactIntegrity,
        fingerprint: Option<u32>,
    },
    IdentifyExport {
        integrity: ArtifactIntegrity,
        fingerprint: Option<u32>,
    },
}

impl ProviderQuery {
    pub fn capability(&self) -> SourceCapability {
        match self {
            Self::ReleaseProtocol { .. } | Self::ReleasesProtocol { .. } => {
                SourceCapability::Releases
            }
            Self::SearchProtocol { .. } => SourceCapability::Search,
            Self::ProjectProtocol { .. } => SourceCapability::Project,
            Self::Locate { .. } => SourceCapability::Project,
            Self::Project { .. } => SourceCapability::Project,
            Self::Description { .. } => SourceCapability::Description,
            Self::Release { .. } | Self::Releases { .. } => {
                SourceCapability::Releases
            }
            Self::Changelog { .. } => SourceCapability::Changelog,
            Self::Search { .. } => SourceCapability::Search,
            Self::Identify { .. } | Self::IdentifyExport { .. } => {
                SourceCapability::IdentifyHash
            }
        }
    }

    pub(crate) fn batch_group(&self) -> u8 {
        match self {
            Self::ReleaseProtocol { .. } => 5,
            Self::ProjectProtocol { .. } => 4,
            Self::Project { .. } => 0,
            Self::Release { .. } => 1,
            Self::Identify { .. } | Self::IdentifyExport { .. } => 2,
            _ => 3,
        }
    }

    pub(crate) fn ttl(&self, value: &ProviderValue) -> i64 {
        if matches!(value, ProviderValue::NotFound)
            || matches!(value, ProviderValue::Matches(matches) if matches.is_empty())
            || matches!(value, ProviderValue::Identified(matches) if matches.is_empty())
        {
            return 6 * 60 * 60;
        }
        match self {
            Self::Search { .. } | Self::SearchProtocol { .. } => 60,
            Self::ProjectProtocol { .. }
            | Self::Locate { .. }
            | Self::Project { .. }
            | Self::Description { .. } => 30 * 60,
            Self::Releases { .. } | Self::ReleasesProtocol { .. } => 5 * 60,
            Self::Release { .. }
            | Self::ReleaseProtocol { .. }
            | Self::Changelog { .. }
            | Self::Identify { .. }
            | Self::IdentifyExport { .. } => 24 * 60 * 60,
        }
    }

    pub(crate) fn normalize(&mut self) {
        let target = match self {
            Self::Releases { target, .. } => Some(target),
            Self::Search { query, .. } => {
                query.categories.sort();
                query.categories.dedup();
                query.sources.sort();
                query.sources.dedup();
                Some(&mut query.target)
            }
            _ => None,
        };
        if let Some(target) = target {
            target.game_versions.sort();
            target.game_versions.dedup();
            target.loaders.sort();
            target.loaders.dedup();
        }
        if let Self::Identify { integrity, .. }
        | Self::IdentifyExport { integrity, .. } = self
        {
            for hash in &mut integrity.hashes {
                hash.value.make_ascii_lowercase();
            }
            integrity.hashes.sort_by_key(|hash| {
                format!("{:?}:{}", hash.algorithm, hash.value)
            });
            integrity.hashes.dedup();
        }
    }

    pub(crate) fn cache_identity(&self) -> Self {
        let mut identity = self.clone();
        match &mut identity {
            Self::Project { slug, .. }
            | Self::Description { slug, .. }
            | Self::Release { slug, .. }
            | Self::Releases { slug, .. }
            | Self::Changelog { slug, .. } => *slug = None,
            _ => {}
        }
        identity
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourcePage<T> {
    pub entries: Vec<T>,
    pub cursor: Option<String>,
    pub total: ResourceTotal,
    pub window_exhausted: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDocument {
    pub metadata: SourceProjectMetadata,
    pub description: ResourceText,
    #[serde(default)]
    pub protocol: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseDocument {
    pub release: ResourceRelease,
    pub changelog: Option<ResourceText>,
    #[serde(default)]
    pub protocol: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum ProviderValue {
    Protocol(serde_json::Value),
    Project(ProjectDocument),
    Description(ResourceText),
    Release(ReleaseDocument),
    Releases(SourcePage<ResourceRelease>),
    Changelog(ResourceText),
    Search(SourcePage<SourceProjectMetadata>),
    /// Lookup candidates still require exact size/hash/role verification.
    Matches(Vec<ResourceArtifact>),
    Identified(Vec<ReleaseDocument>),
    NotFound,
}

#[async_trait]
pub trait ResourceProvider: Send + Sync {
    fn local_release_pagination(&self) -> bool {
        false
    }
    fn source(&self) -> ResourceSource;
    fn namespace(&self) -> String;
    fn batch_limit(&self, query: &ProviderQuery) -> usize;
    fn validate_query(&self, _: &ProviderQuery) -> ResourceResult<()> {
        Ok(())
    }
    async fn fetch_many(
        &self,
        queries: &[ProviderQuery],
    ) -> Vec<ResourceResult<ProviderValue>>;
}

pub fn default_providers() -> Vec<Arc<dyn ResourceProvider>> {
    let transport = Arc::new(transport::LauncherTransport);
    vec![
        Arc::new(ModrinthProvider::new(transport.clone())),
        Arc::new(CurseForgeProvider::new(transport.clone())),
        Arc::new(McArchiveProvider::new(transport.clone())),
        Arc::new(PlanetMinecraftProvider::new(transport)),
    ]
}

pub(crate) fn source_project(
    source: SourceId,
    id: impl Into<String>,
) -> ResourceResult<SourceProjectRef> {
    Ok(SourceProjectRef {
        source,
        project_id: SourceProjectId::new(id)?,
    })
}

pub(crate) fn release_ref(
    project: SourceProjectRef,
    id: impl Into<String>,
) -> ResourceResult<SourceReleaseRef> {
    Ok(SourceReleaseRef {
        project,
        release_id: SourceReleaseId::new(id)?,
    })
}

pub(crate) fn positive_id(id: &str) -> ResourceResult<u32> {
    id.parse::<u32>()
        .ok()
        .filter(|value| *value > 0 && value.to_string() == id)
        .ok_or_else(|| {
            ResourceError::InvalidRequest(
                "expected a canonical positive CurseForge ID".into(),
            )
        })
}

pub(crate) fn public_url(value: &str) -> Option<String> {
    let url = url::Url::parse(value).ok()?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query_pairs().any(|(key, _)| {
            matches!(
                key.to_ascii_lowercase().as_str(),
                "token"
                    | "api_key"
                    | "apikey"
                    | "key"
                    | "authorization"
                    | "access_token"
                    | "signature"
                    | "x-amz-signature"
            )
        })
    {
        return None;
    }
    Some(value.to_string())
}

pub(crate) fn protocol_projection(
    value: serde_json::Value,
) -> serde_json::Value {
    match value {
        serde_json::Value::Object(values) => serde_json::Value::Object(
            values
                .into_iter()
                .filter(|(key, _)| {
                    !matches!(
                        key.to_ascii_lowercase()
                            .replace(['_', '-'], "")
                            .as_str(),
                        "apikey"
                            | "authorization"
                            | "password"
                            | "token"
                            | "accesstoken"
                            | "secret"
                            | "headers"
                    )
                })
                .map(|(key, value)| (key, protocol_projection(value)))
                .collect(),
        ),
        serde_json::Value::Array(values) => serde_json::Value::Array(
            values.into_iter().map(protocol_projection).collect(),
        ),
        serde_json::Value::String(ref text)
            if text.starts_with("https://") || text.starts_with("http://") =>
        {
            if public_url(text).is_some() {
                value
            } else {
                serde_json::Value::Null
            }
        }
        _ => value,
    }
}

pub(crate) fn acquisition(
    urls: Vec<String>,
    manual: Option<String>,
) -> ArtifactAcquisition {
    let urls: Vec<_> = urls
        .into_iter()
        .filter_map(|url| public_url(&url))
        .collect();
    if !urls.is_empty() {
        ArtifactAcquisition::PublicDownload { urls }
    } else if let Some(page_url) = manual.and_then(|url| public_url(&url)) {
        ArtifactAcquisition::ManualDownload { page_url }
    } else {
        ArtifactAcquisition::Unavailable
    }
}

pub(crate) fn hashes(
    hashes: impl IntoIterator<Item = (HashAlgorithm, String)>,
) -> ResourceResult<Vec<ContentHash>> {
    hashes
        .into_iter()
        .map(|(algorithm, value)| {
            let expected = match algorithm {
                HashAlgorithm::Md5 => 32,
                HashAlgorithm::Sha1 => 40,
                HashAlgorithm::Sha256 => 64,
                HashAlgorithm::Sha512 => 128,
            };
            if value.len() != expected
                || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
            {
                return Err(ResourceError::InvalidResponse(
                    "malformed artifact hash".into(),
                ));
            }
            Ok(ContentHash {
                algorithm,
                value: value.to_ascii_lowercase(),
            })
        })
        .collect()
}

pub(crate) fn offset(cursor: &Option<String>) -> ResourceResult<usize> {
    cursor
        .as_ref()
        .map(|cursor| {
            cursor.parse().map_err(|_| {
                ResourceError::InvalidRequest("invalid source cursor".into())
            })
        })
        .unwrap_or(Ok(0))
}

pub(crate) fn slice_page<T: Clone>(
    entries: Vec<T>,
    start: usize,
    limit: u32,
) -> ResourceResult<SourcePage<T>> {
    if !(1..=100).contains(&limit) {
        return Err(ResourceError::InvalidRequest(
            "page size must be between 1 and 100".into(),
        ));
    }
    let count = entries.len();
    let end = start.saturating_add(limit as usize).min(count);
    Ok(SourcePage {
        entries: entries.get(start..end).unwrap_or_default().to_vec(),
        cursor: (end < count).then(|| end.to_string()),
        total: ResourceTotal {
            value: Some(count as u64),
            precision: TotalPrecision::Exact,
        },
        window_exhausted: false,
    })
}

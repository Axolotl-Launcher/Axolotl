//! Projections for shared callers that still expose platform protocol types.
//! All online metadata uses the resource service and its independent cache.

use std::sync::Arc;
use tokio_util::sync::CancellationToken;

use super::{providers::*, *};
use crate::state::{CacheBehaviour, CacheValue, CacheValueType, CachedEntry};

pub(crate) fn policy(behaviour: Option<CacheBehaviour>) -> CachePolicy {
    match behaviour.unwrap_or_default() {
        CacheBehaviour::Bypass => CachePolicy::ForceRefresh,
        CacheBehaviour::CacheOnly => CachePolicy::OnlyCached,
        CacheBehaviour::StaleWhileRevalidate
        | CacheBehaviour::StaleWhileRevalidateSkipOffline => {
            CachePolicy::StaleWhileRevalidate
        }
        _ => CachePolicy::UseFresh,
    }
}

pub(crate) fn owns(kind: CacheValueType) -> bool {
    matches!(
        kind,
        CacheValueType::Project
            | CacheValueType::ProjectV3
            | CacheValueType::CurseForgeProject
            | CacheValueType::Version
            | CacheValueType::VersionV3
            | CacheValueType::File
            | CacheValueType::FileUpdate
            | CacheValueType::ProjectVersions
            | CacheValueType::SearchResults
            | CacheValueType::SearchResultsV3
    )
}

fn request(kind: CacheValueType, key: &str) -> ResourceResult<ProviderQuery> {
    Ok(match kind {
        CacheValueType::Project => {
            if key.len() == 8
                && key.bytes().all(|byte| byte.is_ascii_alphanumeric())
            {
                ProviderQuery::Project {
                    id: SourceProjectId::new(key)?,
                    slug: None,
                }
            } else {
                ProviderQuery::Locate {
                    locator: key.into(),
                }
            }
        }
        CacheValueType::CurseForgeProject => ProviderQuery::Project {
            id: SourceProjectId::new(key)?,
            slug: None,
        },
        CacheValueType::ProjectV3 => ProviderQuery::ProjectProtocol {
            locator: key.into(),
            version: "v3".into(),
        },
        CacheValueType::Version => ProviderQuery::Release {
            project_id: None,
            release_id: SourceReleaseId::new(key)?,
            slug: None,
        },
        CacheValueType::VersionV3 => ProviderQuery::ReleaseProtocol {
            release_id: SourceReleaseId::new(key)?,
            version: "v3".into(),
        },
        CacheValueType::File => ProviderQuery::IdentifyExport {
            integrity: ArtifactIntegrity {
                size: None,
                hashes: vec![ContentHash {
                    algorithm: HashAlgorithm::Sha1,
                    value: key.to_ascii_lowercase(),
                }],
            },
            fingerprint: None,
        },
        CacheValueType::ProjectVersions => ProviderQuery::ReleasesProtocol {
            project_id: SourceProjectId::new(key)?,
        },
        CacheValueType::SearchResults | CacheValueType::SearchResultsV3 => {
            ProviderQuery::SearchProtocol {
                query: key.into(),
                version: if kind == CacheValueType::SearchResultsV3 {
                    "v3"
                } else {
                    "v2"
                }
                .into(),
            }
        }
        _ => {
            return Err(ResourceError::Unsupported(
                "non-resource cache projection".into(),
            ));
        }
    })
}

fn project(
    kind: CacheValueType,
    key: &str,
    value: ProviderValue,
) -> crate::Result<Option<CacheValue>> {
    let protocol = match value {
        ProviderValue::NotFound => return Ok(None),
        ProviderValue::Identified(documents)
            if kind == CacheValueType::File =>
        {
            let matches: Vec<_> = documents
                .iter()
                .filter(|document| {
                    document.release.artifacts.iter().any(|artifact| {
                        artifact.integrity.hashes.iter().any(|hash| {
                            hash.algorithm == HashAlgorithm::Sha1
                                && hash.value.eq_ignore_ascii_case(key)
                        })
                    })
                })
                .collect();
            return Ok(if matches.len() == 1 {
                let reference = &matches[0].release.reference;
                Some(CacheValue::File(crate::state::ModrinthHashMatch {
                    hash: key.to_ascii_lowercase(),
                    project_id: reference.project.project_id.as_str().into(),
                    version_id: reference.release_id.as_str().into(),
                }))
            } else {
                None
            });
        }
        ProviderValue::Project(document) => document.protocol,
        ProviderValue::Release(document) => document.protocol,
        ProviderValue::Protocol(value) => Some(value),
        _ => None,
    }
    .ok_or_else(|| {
        crate::ErrorKind::InputError(
            "resource protocol projection missing".into(),
        )
    })?;
    Ok(Some(match kind {
        CacheValueType::Project => {
            CacheValue::Project(serde_json::from_value(protocol)?)
        }
        CacheValueType::ProjectV3 => {
            CacheValue::ProjectV3(serde_json::from_value(protocol)?)
        }
        CacheValueType::CurseForgeProject => {
            CacheValue::CurseForgeProject(serde_json::from_value(protocol)?)
        }
        CacheValueType::Version => {
            CacheValue::Version(serde_json::from_value(protocol)?)
        }
        CacheValueType::VersionV3 => {
            CacheValue::VersionV3(serde_json::from_value(protocol)?)
        }
        CacheValueType::ProjectVersions => {
            CacheValue::ProjectVersions(crate::state::CachedProjectVersions {
                project_id: key.into(),
                versions: serde_json::from_value(protocol)?,
            })
        }
        CacheValueType::SearchResults => {
            CacheValue::SearchResults(crate::state::SearchResults {
                search: key.into(),
                result: serde_json::from_value(protocol)?,
            })
        }
        CacheValueType::SearchResultsV3 => {
            CacheValue::SearchResultsV3(crate::state::SearchResultsV3 {
                search: key.into(),
                result: serde_json::from_value(protocol)?,
            })
        }
        _ => {
            return Err(crate::ErrorKind::InputError(
                "unexpected resource projection".into(),
            )
            .into());
        }
    }))
}

pub(crate) async fn many(
    service: &Arc<ResourceService>,
    kind: CacheValueType,
    keys: &[&str],
    behaviour: Option<CacheBehaviour>,
    cancel: &CancellationToken,
) -> crate::Result<Vec<CachedEntry>> {
    if kind == CacheValueType::FileUpdate {
        return updates(service, keys, behaviour, cancel).await;
    }
    let queries = keys
        .iter()
        .map(|key| request(kind, key))
        .collect::<ResourceResult<Vec<_>>>()
        .map_err(error)?;
    let source = if kind == CacheValueType::CurseForgeProject {
        SourceId::curseforge()
    } else {
        SourceId::modrinth()
    };
    let values = service
        .presentation_many_with_policy(
            &source,
            queries.clone(),
            cancel,
            policy(behaviour),
        )
        .await;
    let mut output = Vec::new();
    for ((key, query), mut value) in keys.iter().zip(queries).zip(values) {
        if value.is_err()
            && matches!(
                behaviour.unwrap_or_default(),
                CacheBehaviour::StaleWhileRevalidate
                    | CacheBehaviour::StaleWhileRevalidateSkipOffline
            )
            && !cancel.is_cancelled()
        {
            value = service
                .presentation_many_with_policy(
                    &source,
                    vec![query],
                    cancel,
                    CachePolicy::OnlyCached,
                )
                .await
                .into_iter()
                .next()
                .unwrap_or(Err(ResourceError::Unavailable));
        }
        if matches!(value, Err(ResourceError::Unavailable))
            && policy(behaviour) == CachePolicy::OnlyCached
        {
            continue;
        }
        let value = value.map_err(error)?.value;
        match project(kind, key, value) {
            Ok(Some(value)) => output.push(value.get_entry()),
            Ok(None) => {}
            Err(error)
                if kind == CacheValueType::Project
                    && matches!(
                        &*error.raw,
                        crate::ErrorKind::JSONError(_)
                    ) =>
            {
                tracing::warn!(project_id = *key, error = %error, "Skipping malformed Modrinth project metadata");
            }
            Err(error) => return Err(error),
        }
    }
    Ok(output)
}

async fn updates(
    service: &Arc<ResourceService>,
    keys: &[&str],
    behaviour: Option<CacheBehaviour>,
    cancel: &CancellationToken,
) -> crate::Result<Vec<CachedEntry>> {
    let policy = policy(behaviour);
    let hashes: Vec<_> = keys
        .iter()
        .filter_map(|key| key.split('-').next())
        .collect();
    let queries = hashes
        .iter()
        .map(|hash| request(CacheValueType::File, hash))
        .collect::<ResourceResult<Vec<_>>>()
        .map_err(error)?;
    let identified = service
        .presentation_many_with_policy(
            &SourceId::modrinth(),
            queries,
            cancel,
            policy,
        )
        .await;
    let mut project_pages = std::collections::HashMap::new();
    let mut entries = Vec::new();
    for (key, identified) in keys.iter().zip(identified) {
        let parts: Vec<_> = key.splitn(4, '-').collect();
        let (hash, loaders, channel, game_version) = if parts.len() == 4
            && matches!(parts[2], "release" | "beta" | "alpha" | "all")
        {
            (parts[0], parts[1], parts[2], parts[3])
        } else {
            let parts: Vec<_> = key.splitn(3, '-').collect();
            if parts.len() != 3 {
                continue;
            }
            (parts[0], parts[1], "alpha", parts[2])
        };
        let identified = match identified {
            Err(ResourceError::Unavailable)
                if policy == CachePolicy::OnlyCached =>
            {
                continue;
            }
            result => result.map_err(error)?.value,
        };
        let ProviderValue::Identified(documents) = identified else {
            continue;
        };
        if documents.len() != 1 {
            continue;
        }
        let current = &documents[0].release;
        let target = ipc::ResourceTarget {
            game_versions: vec![game_version.into()],
            loaders: loaders.split('+').map(str::to_owned).collect(),
            channels: match channel {
                "release" => vec![ReleaseChannel::Release],
                "beta" => vec![ReleaseChannel::Release, ReleaseChannel::Beta],
                _ => vec![
                    ReleaseChannel::Release,
                    ReleaseChannel::Beta,
                    ReleaseChannel::Alpha,
                ],
            },
        };
        let project_id = current.reference.project.project_id.clone();
        if !project_pages.contains_key(&project_id) {
            let page = service
                .presentation_many_with_policy(
                    &SourceId::modrinth(),
                    vec![ProviderQuery::ReleasesProtocol {
                        project_id: current
                            .reference
                            .project
                            .project_id
                            .clone(),
                    }],
                    cancel,
                    policy,
                )
                .await
                .into_iter()
                .next()
                .unwrap_or(Err(ResourceError::Unavailable));
            project_pages.insert(project_id.clone(), page);
        }
        let page = project_pages.get(&project_id).cloned().unwrap();
        let page = match page {
            Err(ResourceError::Unavailable)
                if policy == CachePolicy::OnlyCached =>
            {
                continue;
            }
            result => result.map_err(error)?.value,
        };
        let ProviderValue::Protocol(values) = page else {
            continue;
        };
        let protocols = values.as_array().ok_or_else(|| {
            error(ResourceError::InvalidResponse(
                "release protocols missing".into(),
            ))
        })?;
        let mut releases = Vec::new();
        for value in protocols {
            let release = providers::legacy::release(value.clone())
                .map_err(error)?
                .release;
            let assessment =
                assess_compatibility(&release, &target, ResourceType::Mod);
            if assessment.game_version == CompatibilityState::Compatible
                && assessment.loader == CompatibilityState::Compatible
                && target.channels.contains(&release.channel)
                && !release_is_known_lower(&release, current)
            {
                releases.push(release);
            }
        }
        if let Some((release, _)) =
            select_automatic_release(releases, &SourceId::modrinth())
            && release.reference != current.reference
            && release_is_newer(&release, current)
            && !release.artifacts.iter().any(|artifact| {
                artifact.integrity.hashes.iter().any(|candidate| {
                    candidate.algorithm == HashAlgorithm::Sha1
                        && candidate.value.eq_ignore_ascii_case(hash)
                })
            })
        {
            entries.push(
                CacheValue::FileUpdate(crate::state::CachedFileUpdate {
                    hash: hash.into(),
                    game_version: game_version.into(),
                    loaders: target.loaders,
                    channel_policy: channel.into(),
                    update_version_id: release
                        .reference
                        .release_id
                        .as_str()
                        .into(),
                })
                .get_entry(),
            );
        }
    }
    Ok(entries)
}

pub(crate) fn error(error: impl Into<ResourceError>) -> crate::Error {
    crate::ErrorKind::InputError(error.into().to_string()).into()
}

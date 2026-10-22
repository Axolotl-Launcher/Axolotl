//! Exact pack-member metadata shared by archive importers and repair.

use std::collections::HashMap;

use super::{providers::*, *};
use crate::state::instances::commands::ProjectFileRecord;
use tokio_util::sync::CancellationToken;

pub(crate) async fn artifacts(
    state: &crate::State,
    records: &[ProjectFileRecord],
    cancel: &CancellationToken,
) -> HashMap<String, ResourceArtifact> {
    let Ok(service) = state.resources().await else {
        return HashMap::new();
    };
    let mut output = HashMap::new();
    for source in [
        SourceId::modrinth(),
        SourceId::curseforge(),
        SourceId::mcarchive(),
    ] {
        let applicable: Vec<_> = records
            .iter()
            .filter_map(|record| {
                let reference = record.provider_ref.as_ref()?;
                if reference.provider().as_str() != source.as_str() {
                    return None;
                }
                let project_id =
                    SourceProjectId::new(reference.database_project_id())
                        .ok()?;
                let release_id =
                    SourceReleaseId::new(reference.database_release_id()?)
                        .ok()?;
                Some((
                    record,
                    ProviderQuery::Release {
                        project_id: Some(project_id),
                        release_id,
                        slug: None,
                    },
                ))
            })
            .collect();
        let values = service
            .query_many(
                &source,
                applicable.iter().map(|(_, query)| query.clone()).collect(),
                CachePolicy::OnlyCached,
                cancel,
            )
            .await;
        for ((record, _), value) in applicable.into_iter().zip(values) {
            let Ok(QueryOutcome {
                value: ProviderValue::Release(document),
                ..
            }) = value
            else {
                continue;
            };
            let local = ArtifactIntegrity {
                size: Some(record.size),
                hashes: vec![ContentHash {
                    algorithm: HashAlgorithm::Sha1,
                    value: record.sha1.clone(),
                }],
            };
            let matches: Vec<_> = document
                .release
                .artifacts
                .into_iter()
                .filter(|artifact| {
                    valid_artifact(artifact)
                        && artifact.role == ArtifactRole::Primary
                        && identity::exact_integrity(
                            &local,
                            &artifact.integrity,
                        )
                })
                .collect();
            if matches.len() == 1 {
                output.insert(
                    record.relative_path.clone(),
                    matches.into_iter().next().unwrap(),
                );
            }
        }
    }
    output
}

pub(crate) fn index_key(release: &str) -> ResourceResult<cache::CacheKey> {
    SourceReleaseId::new(release)?;
    cache::CacheKey::new("pack-index".into(), SourceId::modrinth(), &release)
}

pub(crate) fn valid_index(index: &crate::state::CachedModpackFiles) -> bool {
    let reference = SourceProjectRef {
        source: SourceId::modrinth(),
        project_id: match SourceProjectId::new(&index.version_id) {
            Ok(id) => id,
            Err(_) => return false,
        },
    };
    crate::state::instances::adapters::sqlite::resource_rows::validate_project(&reference).is_ok()
        && index.project_ids.iter().all(|id| {
            let reference = SourceProjectRef { source: SourceId::modrinth(), project_id: match SourceProjectId::new(id) { Ok(id) => id, Err(_) => return false } };
            crate::state::instances::adapters::sqlite::resource_rows::validate_project(&reference).is_ok()
        })
        && index.file_hashes.iter().all(|hash| hash.len() == 40 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

pub(crate) async fn save_index(
    cache: &cache::ResourceCache,
    index: &crate::state::CachedModpackFiles,
) -> ResourceResult<()> {
    if !valid_index(index) {
        return Err(ResourceError::InvalidRequest("invalid pack index".into()));
    }
    let key = index_key(&index.version_id)?;
    cache
        .put(
            &key,
            &ProviderValue::Protocol(serde_json::to_value(index)?),
            24 * 60 * 60,
            cache.generation().await,
            chrono::Utc::now().timestamp_micros(),
        )
        .await;
    Ok(())
}

pub(crate) async fn index(
    cache: &cache::ResourceCache,
    release: &str,
) -> Option<crate::state::CachedModpackFiles> {
    let key = index_key(release).ok()?;
    let ProviderValue::Protocol(value) = cache.get_any(&key).await? else {
        return None;
    };
    let index: crate::state::CachedModpackFiles =
        serde_json::from_value(value).ok()?;
    (index.version_id == release && valid_index(&index)).then_some(index)
}

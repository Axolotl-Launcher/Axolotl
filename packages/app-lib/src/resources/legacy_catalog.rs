//! Protocol projections at the boundary of the existing environment solver.

use super::{providers::*, *};
use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio_util::sync::CancellationToken;

fn error(error: ResourceError) -> crate::Error {
    crate::ErrorKind::InputError(error.to_string()).into()
}

pub(crate) async fn exact<T: DeserializeOwned>(
    source: SourceId,
    project_id: Option<String>,
    release_id: String,
    state: &crate::State,
) -> crate::Result<Option<T>> {
    let service = state.resources().await.map_err(error)?;
    let value = service
        .presentation_query(
            &source,
            ProviderQuery::Release {
                project_id: project_id
                    .map(SourceProjectId::new)
                    .transpose()
                    .map_err(|error| {
                        crate::ErrorKind::InputError(error.to_string())
                    })?,
                release_id: SourceReleaseId::new(release_id).map_err(
                    |error| crate::ErrorKind::InputError(error.to_string()),
                )?,
                slug: None,
            },
            &CancellationToken::new(),
        )
        .await
        .map_err(error)?;
    match value.value {
        ProviderValue::Release(document) => document
            .protocol
            .map(serde_json::from_value)
            .transpose()
            .map_err(Into::into),
        ProviderValue::NotFound => Ok(None),
        _ => Err(crate::ErrorKind::InputError(
            "unexpected release catalog response".into(),
        )
        .into()),
    }
}

pub(crate) async fn page<T: DeserializeOwned>(
    source: SourceId,
    project_id: String,
    target: ipc::ResourceTarget,
    state: &crate::State,
) -> crate::Result<(Vec<T>, bool)> {
    let service = state.resources().await.map_err(error)?;
    page_with_service(source, project_id, target, service).await
}

pub(crate) async fn page_with_service<T: DeserializeOwned>(
    source: SourceId,
    project_id: String,
    target: ipc::ResourceTarget,
    service: &std::sync::Arc<ResourceService>,
) -> crate::Result<(Vec<T>, bool)> {
    let project_id = SourceProjectId::new(project_id)
        .map_err(|error| crate::ErrorKind::InputError(error.to_string()))?;
    let cancel = CancellationToken::new();
    let mut cached_catalog = false;
    if source == SourceId::modrinth() {
        let value = service
            .presentation_query(
                &source,
                ProviderQuery::ReleasesProtocol {
                    project_id: project_id.clone(),
                },
                &cancel,
            )
            .await;
        if let Ok(value) = value {
            let ProviderValue::Protocol(Value::Array(values)) = value.value
            else {
                return Err(crate::ErrorKind::InputError(
                    "unexpected release catalog".into(),
                )
                .into());
            };
            let mut documents = Vec::new();
            for value in values {
                let release = providers::legacy::release(value.clone())
                    .map_err(error)?
                    .release;
                if (target.game_versions.is_empty()
                    || release
                        .compatibility
                        .game_versions
                        .iter()
                        .any(|version| target.game_versions.contains(version)))
                    && (target.loaders.is_empty()
                        || release
                            .compatibility
                            .loaders
                            .iter()
                            .any(|loader| target.loaders.contains(loader)))
                    && (target.channels.is_empty()
                        || target.channels.contains(&release.channel))
                {
                    documents.push(serde_json::from_value(value)?);
                }
            }
            return Ok((documents, false));
        } else {
            cached_catalog = true;
        }
    }
    let mut cursor = None;
    let mut all = Vec::new();
    let mut limited = false;
    let mut seen = std::collections::HashSet::new();
    loop {
        let query = ProviderQuery::Releases {
            project_id: project_id.clone(),
            slug: None,
            target: target.clone(),
            cursor: cursor.clone(),
            page_size: 50,
        };
        let value = if cached_catalog {
            service
                .query(&source, query, CachePolicy::OnlyCached, &cancel)
                .await
        } else {
            service.presentation_query(&source, query, &cancel).await
        }
        .map_err(error)?;
        let ProviderValue::Releases(page) = value.value else {
            return Err(crate::ErrorKind::InputError(
                "unexpected release page".into(),
            )
            .into());
        };
        limited |= page.window_exhausted;
        all.extend(page.entries);
        let Some(next) = page.cursor else {
            break;
        };
        if !seen.insert(next.clone()) {
            return Err(crate::ErrorKind::InputError(
                "release cursor did not advance".into(),
            )
            .into());
        }
        cursor = Some(next);
    }
    let values = service
        .presentation_many_with_policy(
            &source,
            all.iter()
                .map(|release| ProviderQuery::Release {
                    project_id: Some(project_id.clone()),
                    release_id: release.reference.release_id.clone(),
                    slug: None,
                })
                .collect(),
            &cancel,
            if cached_catalog {
                CachePolicy::OnlyCached
            } else {
                CachePolicy::UseFresh
            },
        )
        .await;
    let mut documents = Vec::new();
    for value in values {
        match value.map_err(error)?.value {
            ProviderValue::Release(document) => {
                documents.push(serde_json::from_value(
                    document.protocol.ok_or_else(|| {
                        crate::ErrorKind::InputError(
                            "release protocol projection missing".into(),
                        )
                    })?,
                )?)
            }
            _ => {
                return Err(crate::ErrorKind::InputError(
                    "release disappeared from catalog".into(),
                )
                .into());
            }
        }
    }
    Ok((documents, limited))
}

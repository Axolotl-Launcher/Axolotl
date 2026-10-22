use std::sync::Arc;
use tokio_util::sync::CancellationToken;

use super::{ipc::*, providers::*, *};

#[derive(Clone)]
pub(crate) struct CatalogCursor {
    reference: ResourceRef,
    target: ResourceTarget,
    project: SourceProjectRef,
    locator: Option<String>,
    cursor: String,
    expires: i64,
}

impl ResourceService {
    pub async fn presentation_many(
        self: &Arc<Self>,
        source: &SourceId,
        queries: Vec<ProviderQuery>,
        cancel: &CancellationToken,
    ) -> Vec<ResourceResult<QueryOutcome>> {
        self.presentation_many_with_policy(
            source,
            queries,
            cancel,
            CachePolicy::UseFresh,
        )
        .await
    }
    pub async fn presentation_many_with_policy(
        self: &Arc<Self>,
        source: &SourceId,
        queries: Vec<ProviderQuery>,
        cancel: &CancellationToken,
        policy: CachePolicy,
    ) -> Vec<ResourceResult<QueryOutcome>> {
        let mut results = self
            .query_many(source, queries.clone(), policy, cancel)
            .await;
        let mut offline = std::collections::HashSet::new();
        if matches!(
            policy,
            CachePolicy::UseFresh
                | CachePolicy::Revalidate
                | CachePolicy::StaleWhileRevalidate
        ) && !cancel.is_cancelled()
        {
            let failed: Vec<_> = results.iter().enumerate().filter_map(|(index, result)| matches!(result, Err(error) if !matches!(error, ResourceError::Canceled)).then_some(index)).collect();
            let cached = self
                .query_many(
                    source,
                    failed
                        .iter()
                        .map(|index| queries[*index].clone())
                        .collect(),
                    CachePolicy::OnlyCached,
                    cancel,
                )
                .await;
            for (index, cached) in failed.into_iter().zip(cached) {
                if let Ok(cached) = cached
                    && !matches!(cached.value, ProviderValue::NotFound)
                {
                    results[index] = Ok(cached);
                    offline.insert(index);
                }
            }
        }
        if policy == CachePolicy::OnlyCached {
            return results;
        }
        let indices: Vec<_> = results
            .iter()
            .enumerate()
            .filter_map(|(index, result)| match result {
                Ok(QueryOutcome {
                    from_cache: true,
                    value: ProviderValue::Project(document),
                }) if document.protocol.is_none()
                    && !offline.contains(&index) =>
                {
                    Some(index)
                }
                Ok(QueryOutcome {
                    from_cache: true,
                    value: ProviderValue::Release(document),
                }) if document.protocol.is_none()
                    && !offline.contains(&index) =>
                {
                    Some(index)
                }
                _ => None,
            })
            .collect();
        let refresh = indices
            .iter()
            .map(|index| queries[*index].clone())
            .collect();
        for (index, value) in indices.into_iter().zip(
            self.query_many(source, refresh, CachePolicy::ForceRefresh, cancel)
                .await,
        ) {
            match value {
                Err(
                    ResourceError::Network { .. } | ResourceError::Unavailable,
                ) if policy == CachePolicy::UseFresh => {}
                value => results[index] = value,
            }
        }
        results
    }
    pub async fn presentation_query(
        self: &Arc<Self>,
        source: &SourceId,
        query: ProviderQuery,
        cancel: &CancellationToken,
    ) -> ResourceResult<QueryOutcome> {
        self.presentation_many(source, vec![query], cancel)
            .await
            .into_iter()
            .next()
            .ok_or(ResourceError::Unavailable)?
    }
    pub async fn locate(
        self: &Arc<Self>,
        source: &SourceId,
        locator: String,
        cancel: &CancellationToken,
    ) -> ResourceResult<ProjectDocument> {
        self.locate_with_policy(source, locator, cancel, CachePolicy::UseFresh)
            .await
    }
    pub async fn locate_with_policy(
        self: &Arc<Self>,
        source: &SourceId,
        locator: String,
        cancel: &CancellationToken,
        policy: CachePolicy,
    ) -> ResourceResult<ProjectDocument> {
        let query = if *source == SourceId::mcarchive()
            && uuid::Uuid::parse_str(&locator).is_ok()
        {
            let project = SourceProjectRef {
                source: source.clone(),
                project_id: SourceProjectId::new(locator)?,
            };
            let slug = if let Some(store) = self.store() {
                store.project_alias(&project).await?
            } else {
                None
            };
            ProviderQuery::Project {
                id: project.project_id,
                slug,
            }
        } else if matches!(source.as_str(), "modrinth" | "mcarchive") {
            ProviderQuery::Locate { locator }
        } else {
            ProviderQuery::Project {
                id: SourceProjectId::new(locator)?,
                slug: None,
            }
        };
        let result = if matches!(
            policy,
            CachePolicy::UseFresh | CachePolicy::Revalidate
        ) {
            self.presentation_many_with_policy(
                source,
                vec![query],
                cancel,
                policy,
            )
            .await
            .into_iter()
            .next()
            .ok_or(ResourceError::Unavailable)??
        } else {
            self.query(source, query, policy, cancel).await?
        };
        match result.value {
            ProviderValue::Project(document) => Ok(document),
            _ => {
                Err(ResourceError::InvalidResponse("project not found".into()))
            }
        }
    }
    pub async fn preferred_source(&self) -> ResourceResult<SourceId> {
        if let Some(store) = self.store() {
            store.preferred_source().await
        } else {
            Ok(SourceId::modrinth())
        }
    }

    pub async fn source_projects(
        &self,
        reference: &ResourceRef,
    ) -> ResourceResult<(Option<ResourceId>, Vec<SourceProjectRef>)> {
        let id = match reference {
            ResourceRef::Resource { id } => Some(id.clone()),
            ResourceRef::Source { project } => {
                if let Some(store) = self.store() {
                    store.lookup(project).await?
                } else {
                    None
                }
            }
        };
        let mut projects = if let (Some(id), Some(store)) = (&id, self.store())
        {
            store.projects(id).await?
        } else {
            vec![]
        };
        if let ResourceRef::Source { project } = reference
            && !projects.contains(project)
        {
            projects.push(project.clone());
        }
        if projects.is_empty() {
            return Err(ResourceError::InvalidRequest(
                "unknown resource".into(),
            ));
        }
        let preferred = self.preferred_source().await?;
        projects.sort_by_key(|project| {
            (project.source_rank(&preferred), project.project_id.clone())
        });
        Ok((id, projects))
    }

    pub async fn project_document(
        self: &Arc<Self>,
        project: &SourceProjectRef,
        locator: Option<String>,
        cancel: &CancellationToken,
    ) -> ResourceResult<ProjectDocument> {
        let slug = if locator.is_some() {
            locator
        } else if let Some(store) = self.store() {
            store.project_alias(project).await?
        } else {
            None
        };
        let query = ProviderQuery::Project {
            id: project.project_id.clone(),
            slug,
        };
        let result = match self
            .query(
                &project.source,
                query.clone(),
                CachePolicy::UseFresh,
                cancel,
            )
            .await
        {
            Err(ResourceError::Canceled) => {
                return Err(ResourceError::Canceled);
            }
            Err(error) => self
                .query(&project.source, query, CachePolicy::OnlyCached, cancel)
                .await
                .map_err(|_| error)?,
            Ok(result) => result,
        };
        match result.value {
            ProviderValue::Project(document) => {
                if &document.metadata.project != project {
                    return Err(ResourceError::InvalidResponse(
                        "project identity changed".into(),
                    ));
                }
                Ok(document)
            }
            _ => {
                Err(ResourceError::InvalidResponse("project not found".into()))
            }
        }
    }

    pub async fn get(
        self: &Arc<Self>,
        reference: &ResourceRef,
        source: Option<SourceId>,
        locator: Option<String>,
        cancel: &CancellationToken,
    ) -> ResourceResult<Resource> {
        let (id, projects) = self.source_projects(reference).await?;
        let confirmed_projects = projects.clone();
        let mut source_failures = Vec::new();
        let mut metadata = Vec::new();
        let mut last_error = ResourceError::Unavailable;
        let requests = projects.iter().map(|project| {
            let locator = locator.clone().filter(|_| matches!(reference, ResourceRef::Source { project: exact } if exact == project));
            self.project_document(project, locator, cancel)
        });
        for (project, result) in projects
            .iter()
            .zip(futures::future::join_all(requests).await)
        {
            match result {
                Ok(document) => metadata.push(document.metadata),
                Err(ResourceError::Canceled) => {
                    return Err(ResourceError::Canceled);
                }
                Err(error) => {
                    source_failures.push(super::search::source_failure(
                        project.source.clone(),
                        error.clone(),
                    ));
                    last_error = error;
                }
            }
        }
        if metadata.is_empty() {
            return Err(last_error);
        }
        let default = if let Some(source) = source {
            metadata
                .iter()
                .find(|metadata| metadata.project.source == source)
                .ok_or_else(|| {
                    ResourceError::InvalidRequest(
                        "requested source is not confirmed or unavailable"
                            .into(),
                    )
                })?
                .project
                .clone()
        } else {
            metadata[0].project.clone()
        };
        Ok(Resource {
            confirmed_projects,
            source_failures,
            id,
            default_project: default,
            projects: metadata,
        })
    }

    pub async fn description(
        self: &Arc<Self>,
        reference: &ResourceRef,
        source: &SourceId,
        locator: Option<String>,
        cancel: &CancellationToken,
    ) -> ResourceResult<ResourceText> {
        let (_, projects) = self.source_projects(reference).await?;
        let project = projects
            .into_iter()
            .find(|project| &project.source == source)
            .ok_or_else(|| {
                ResourceError::InvalidRequest(
                    "description source is not confirmed".into(),
                )
            })?;
        let slug = if locator.is_some() {
            locator
        } else if let Some(store) = self.store() {
            store.project_alias(&project).await?
        } else {
            None
        };
        match self
            .query(
                source,
                ProviderQuery::Description {
                    id: project.project_id,
                    slug,
                },
                CachePolicy::UseFresh,
                cancel,
            )
            .await?
            .value
        {
            ProviderValue::Description(text) => Ok(text),
            _ => Err(ResourceError::InvalidResponse(
                "description not found".into(),
            )),
        }
    }

    pub async fn release(
        self: &Arc<Self>,
        reference: &SourceReleaseRef,
        locator: Option<String>,
        cancel: &CancellationToken,
    ) -> ResourceResult<ResourceRelease> {
        let slug = if locator.is_some() {
            locator
        } else if let Some(store) = self.store() {
            store.project_alias(&reference.project).await?
        } else {
            None
        };
        match self
            .query(
                &reference.project.source,
                ProviderQuery::Release {
                    project_id: Some(reference.project.project_id.clone()),
                    release_id: reference.release_id.clone(),
                    slug,
                },
                CachePolicy::UseFresh,
                cancel,
            )
            .await?
            .value
        {
            ProviderValue::Release(document)
                if document.release.reference == *reference =>
            {
                Ok(document.release)
            }
            _ => Err(ResourceError::InvalidResponse(
                "release does not belong to selected project".into(),
            )),
        }
    }

    pub async fn releases(
        self: &Arc<Self>,
        reference: &ResourceRef,
        target: ResourceTarget,
        cursor: Option<String>,
        request_id: RequestId,
        locator: Option<String>,
        cancel: &CancellationToken,
    ) -> ResourceResult<ResourcePage<ResourceRelease>> {
        let now = chrono::Utc::now().timestamp();
        self.catalogs.retain(|_, cursor| cursor.expires > now);
        let saved = cursor
            .as_ref()
            .map(|token| {
                self.catalogs
                    .get(token)
                    .map(|value| value.clone())
                    .ok_or(ResourceError::CursorExpired)
            })
            .transpose()?;
        if saved.as_ref().is_some_and(|saved| {
            saved.reference != *reference
                || saved.target != target
                || saved.locator != locator
        }) {
            return Err(ResourceError::CursorExpired);
        }
        let project = if let Some(saved) = &saved {
            saved.project.clone()
        } else {
            match reference {
                ResourceRef::Source { project } => project.clone(),
                ResourceRef::Resource { .. } => {
                    self.get(reference, None, None, cancel)
                        .await?
                        .default_project
                }
            }
        };
        let slug = if locator.is_some() {
            locator.clone()
        } else if let Some(store) = self.store() {
            store.project_alias(&project).await?
        } else {
            None
        };
        match self
            .query(
                &project.source,
                ProviderQuery::Releases {
                    project_id: project.project_id.clone(),
                    slug,
                    target: target.clone(),
                    cursor: saved.map(|saved| saved.cursor),
                    page_size: 50,
                },
                CachePolicy::UseFresh,
                cancel,
            )
            .await?
            .value
        {
            ProviderValue::Releases(page) => {
                if page
                    .entries
                    .iter()
                    .any(|release| release.reference.project != project)
                {
                    return Err(ResourceError::InvalidResponse(
                        "release page contains another project".into(),
                    ));
                }
                let next = if let Some(cursor) = page.cursor {
                    if self.catalogs.len() >= 128
                        && let Some(key) = self
                            .catalogs
                            .iter()
                            .min_by_key(|value| value.expires)
                            .map(|value| value.key().clone())
                    {
                        self.catalogs.remove(&key);
                    }
                    let token = uuid::Uuid::new_v4().to_string();
                    self.catalogs.insert(
                        token.clone(),
                        CatalogCursor {
                            reference: reference.clone(),
                            target,
                            project: project.clone(),
                            locator,
                            cursor,
                            expires: now + 600,
                        },
                    );
                    Some(token)
                } else {
                    None
                };
                Ok(ResourcePage {
                    request_id,
                    has_more: next.is_some(),
                    cursor: next,
                    entries: page.entries,
                    total: page.total,
                    source_failures: vec![],
                    exhausted_sources: if page.window_exhausted {
                        vec![project.source]
                    } else {
                        vec![]
                    },
                })
            }
            _ => Err(ResourceError::InvalidResponse(
                "expected release page".into(),
            )),
        }
    }

    pub async fn changelog(
        self: &Arc<Self>,
        reference: &SourceReleaseRef,
        locator: Option<String>,
        cancel: &CancellationToken,
    ) -> ResourceResult<ResourceText> {
        if let Ok(QueryOutcome {
            value: ProviderValue::Release(document),
            ..
        }) = self
            .query(
                &reference.project.source,
                ProviderQuery::Release {
                    project_id: Some(reference.project.project_id.clone()),
                    release_id: reference.release_id.clone(),
                    slug: locator.clone(),
                },
                CachePolicy::OnlyCached,
                cancel,
            )
            .await
            && let Some(text) = document.changelog
        {
            return Ok(text);
        }
        let slug = if locator.is_some() {
            locator
        } else if let Some(store) = self.store() {
            store.project_alias(&reference.project).await?
        } else {
            None
        };
        match self
            .query(
                &reference.project.source,
                ProviderQuery::Changelog {
                    project_id: reference.project.project_id.clone(),
                    release_id: reference.release_id.clone(),
                    slug,
                },
                CachePolicy::UseFresh,
                cancel,
            )
            .await?
            .value
        {
            ProviderValue::Changelog(text) => Ok(text),
            _ => Err(ResourceError::InvalidResponse(
                "changelog not found".into(),
            )),
        }
    }
}

trait SourceRank {
    fn source_rank(&self, preferred: &SourceId) -> usize;
}
impl SourceRank for SourceProjectRef {
    fn source_rank(&self, preferred: &SourceId) -> usize {
        if &self.source == preferred {
            return 0;
        }
        match self.source.as_str() {
            "modrinth" => 1,
            "curseforge" => 2,
            "mcarchive" => 3,
            "planet_minecraft" => 4,
            _ => 5,
        }
    }
}

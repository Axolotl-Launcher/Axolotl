use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use chrono::Utc;
use futures::future::join_all;
use tokio_util::sync::CancellationToken;
use unicode_normalization::UnicodeNormalization;

use super::{ipc::*, providers::*, *};

#[derive(Clone)]
pub(crate) struct SearchCursor {
    session_id: String,
    created_at: i64,
    query: ResourceSearchQuery,
    sources: Vec<SourceBuffer>,
    seen: HashSet<SourceProjectRef>,
    seen_resources: HashSet<ResourceId>,
    expires_at: i64,
    next_source: usize,
}

#[derive(Clone)]
struct SourceBuffer {
    failure: Option<ResourceSourceFailure>,
    source: SourceId,
    query: ResourceSearchQuery,
    cursor: Option<String>,
    started: bool,
    done: bool,
    exhausted: bool,
    entries: VecDeque<SourceProjectMetadata>,
    total: ResourceTotal,
}

pub fn normalized_title(value: &str) -> String {
    value
        .nfc()
        .collect::<String>()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn wiki_pair(
    left: &SourceProjectMetadata,
    right: &SourceProjectMetadata,
    lookup: &crate::content_search::ContentIdentityLookup,
) -> bool {
    let (mr, cf) = if left.project.source == SourceId::modrinth()
        && right.project.source == SourceId::curseforge()
    {
        (left, right)
    } else if right.project.source == SourceId::modrinth()
        && left.project.source == SourceId::curseforge()
    {
        (right, left)
    } else {
        return false;
    };
    let (Some(mr_slug), Some(cf_slug)) = (&mr.slug, &cf.slug) else {
        return false;
    };
    lookup.modrinth.get(mr_slug).is_some_and(|records| {
        records.len() == 1
            && records[0].counterparts.iter().any(|part| {
                part.provider == "curseforge"
                    && part.slug.eq_ignore_ascii_case(cf_slug)
            })
    }) && lookup.curseforge.get(cf_slug).is_some_and(|records| {
        records.len() == 1
            && records[0].counterparts.iter().any(|part| {
                part.provider == "modrinth"
                    && part.slug.eq_ignore_ascii_case(mr_slug)
            })
    })
}

impl ResourceService {
    pub async fn search(
        self: &Arc<Self>,
        query: ResourceSearchQuery,
        cursor: Option<String>,
        request_id: RequestId,
        cancel: &CancellationToken,
    ) -> ResourceResult<ResourcePage<ResourceSearchResult>> {
        if !(1..=100).contains(&query.page_size) {
            return Err(ResourceError::InvalidRequest(
                "invalid page size".into(),
            ));
        }
        let now = Utc::now().timestamp();
        self.searches.retain(|_, cursor| cursor.expires_at > now);
        let mut state = if let Some(cursor) = cursor {
            let state = self
                .searches
                .get(&cursor)
                .ok_or(ResourceError::CursorExpired)?
                .clone();
            if state.query != query {
                return Err(ResourceError::InvalidRequest(
                    "cursor belongs to another query".into(),
                ));
            }
            state
        } else {
            let resolution =
                crate::content_search::resolve_chinese_content_search(
                    &query.text,
                );
            let split =
                crate::content_search::expand_content_search_query(&query.text)
                    .suggested_split;
            let sources = self
                .sources()
                .into_iter()
                .filter(|source| {
                    (query.sources.is_empty()
                        || query.sources.contains(&source.id))
                        && source.available
                        && (!query.sources.is_empty()
                            || self.search_applicable(&source.id, &query))
                        && source
                            .capabilities
                            .contains(&SourceCapability::Search)
                        && query
                            .types
                            .iter()
                            .all(|kind| source.types.contains(kind))
                })
                .map(|source| {
                    let mut source_query = query.clone();
                    if source.id == SourceId::curseforge() {
                        source_query.page_size = source_query.page_size.min(50);
                    }
                    source_query.categories = query
                        .categories
                        .iter()
                        .filter_map(|category| {
                            if let Some((owner, value)) =
                                category.split_once(':')
                            {
                                (owner == source.id.as_str())
                                    .then(|| value.to_string())
                            } else {
                                Some(category.clone())
                            }
                        })
                        .collect();
                    source_query.text = if resolution.is_chinese {
                        if source.id == SourceId::curseforge() {
                            resolution.curseforge_query.clone()
                        } else if source.id == SourceId::modrinth() {
                            resolution.modrinth_query.clone()
                        } else {
                            None
                        }
                        .unwrap_or_else(|| query.text.clone())
                    } else {
                        split.clone().unwrap_or_else(|| query.text.clone())
                    };
                    let supported_sort =
                        source.supported_search_sorts.contains(&query.sort);
                    SourceBuffer {
                        failure: (!supported_sort).then(|| {
                            source_failure(
                                source.id.clone(),
                                ResourceError::Unsupported(format!(
                                    "source cannot provide {:?} ordering",
                                    query.sort
                                )),
                            )
                        }),
                        source: source.id,
                        query: source_query,
                        cursor: None,
                        started: false,
                        done: !supported_sort,
                        exhausted: false,
                        entries: VecDeque::new(),
                        total: ResourceTotal {
                            value: None,
                            precision: TotalPrecision::Unknown,
                        },
                    }
                })
                .collect();
            SearchCursor {
                session_id: uuid::Uuid::new_v4().to_string(),
                created_at: Utc::now().timestamp_micros(),
                query: query.clone(),
                sources,
                seen: HashSet::new(),
                seen_resources: HashSet::new(),
                expires_at: now + 600,
                next_source: 0,
            }
        };
        let mut failures = Vec::new();
        if query.sources.is_empty() && !query.types.is_empty() {
            for source in self.sources().into_iter().filter(|source| {
                source.available
                    && source.capabilities.contains(&SourceCapability::Search)
                    && source.types.is_empty()
            }) {
                failures.push(source_failure(
                    source.id,
                    ResourceError::Unsupported(
                        "source does not expose content-type filters".into(),
                    ),
                ));
            }
        }
        for requested in &query.sources {
            if !state
                .sources
                .iter()
                .any(|source| &source.source == requested)
            {
                failures.push(source_failure(requested.clone(), ResourceError::Unsupported("source is unavailable or cannot search the requested content type".into())));
            }
        }
        // Each source gets one initial fetch and at most one refill per page.
        let mut failed_this_page = HashSet::new();
        for _ in 0..2 {
            let requests = state
                .sources
                .iter()
                .enumerate()
                .filter(|(index, source)| {
                    !failed_this_page.contains(index)
                        && !source.done
                        && source.entries.len() < query.page_size as usize
                })
                .map(|(index, source)| {
                    let query = ProviderQuery::Search {
                        query: source.query.clone(),
                        cursor: source.cursor.clone(),
                    };
                    let id = source.source.clone();
                    async move {
                        (
                            index,
                            self.query(
                                &id,
                                query,
                                CachePolicy::UseFresh,
                                cancel,
                            )
                            .await,
                        )
                    }
                });
            for (index, result) in join_all(requests).await {
                let source = &mut state.sources[index];
                match result {
                    Ok(QueryOutcome {
                        value: ProviderValue::Search(page),
                        ..
                    }) => {
                        if source.started
                            && page.cursor.is_some()
                            && page.cursor == source.cursor
                        {
                            return Err(ResourceError::InvalidResponse(
                                "source cursor did not advance".into(),
                            ));
                        }
                        source.started = true;
                        source.failure = None;
                        source.cursor = page.cursor;
                        source.done = source.cursor.is_none();
                        source.exhausted = page.window_exhausted;
                        source.total = page.total;
                        for entry in page.entries {
                            if excluded_entry(&source.query.filter, &entry) {
                                continue;
                            }
                            if !state.seen.contains(&entry.project)
                                && !source.entries.iter().any(|current| {
                                    current.project == entry.project
                                })
                            {
                                source.entries.push_back(entry);
                            }
                        }
                    }
                    Err(ResourceError::Canceled) => {
                        return Err(ResourceError::Canceled);
                    }
                    Err(error) => {
                        let failure =
                            source_failure(source.source.clone(), error);
                        source.done = !failure.retryable;
                        source.failure = Some(failure);
                        failed_this_page.insert(index);
                    }
                    _ => {
                        source.done = true;
                        failures.push(source_failure(
                            source.source.clone(),
                            ResourceError::InvalidResponse(
                                "expected search page".into(),
                            ),
                        ));
                    }
                }
            }
        }
        let preferred = self.preferred_source().await?;
        let mut pool = Vec::new();
        // Round robin preserves provider relevance order without inventing scores.
        loop {
            let before = pool.len();
            let count = state.sources.len();
            for offset in 0..count {
                let source =
                    &mut state.sources[(state.next_source + offset) % count];
                if let Some(entry) = source.entries.pop_front() {
                    pool.push(entry);
                }
            }
            if pool.len() == before {
                break;
            }
        }
        sort_metadata(&mut pool, query.sort);
        let mut title_counts = HashMap::new();
        for project in &pool {
            *title_counts
                .entry((
                    project.project.source.clone(),
                    normalized_title(&project.title),
                ))
                .or_insert(0usize) += 1;
        }
        let slugs = |source: SourceId| {
            pool.iter()
                .filter(|project| project.project.source == source)
                .filter_map(|project| project.slug.clone())
                .collect::<Vec<_>>()
        };
        let wiki_lookup = crate::content_search::lookup_content_identities(
            &slugs(SourceId::modrinth()),
            &slugs(SourceId::curseforge()),
        );
        let mut groups: Vec<ResourceSearchResult> = Vec::new();
        let identities = if let Some(store) = self.store() {
            store
                .lookup_many(
                    &pool
                        .iter()
                        .map(|metadata| metadata.project.clone())
                        .collect::<Vec<_>>(),
                )
                .await?
        } else {
            HashMap::new()
        };
        while !pool.is_empty() && groups.len() < query.page_size as usize {
            pool.retain(|metadata| {
                !state.seen.contains(&metadata.project)
                    && !identities
                        .get(&metadata.project)
                        .is_some_and(|id| state.seen_resources.contains(id))
            });
            if pool.is_empty() {
                break;
            }
            if query.sort != ResourceSearchSort::Relevance
                && state.sources.iter().any(|source| {
                    !source.done
                        && source.failure.is_none()
                        && !pool
                            .iter()
                            .any(|entry| entry.project.source == source.source)
                })
            {
                break;
            }
            let first = pool.remove(0);
            if query.sort == ResourceSearchSort::Relevance
                && let Some(index) = state
                    .sources
                    .iter()
                    .position(|source| source.source == first.project.source)
            {
                state.next_source = (index + 1) % state.sources.len();
            }
            let id = identities.get(&first.project).cloned();
            if state.seen.contains(&first.project)
                || id
                    .as_ref()
                    .is_some_and(|id| state.seen_resources.contains(id))
            {
                continue;
            }
            let title = normalized_title(&first.title);
            let unambiguous = !title.is_empty()
                && title_counts
                    .get(&(first.project.source.clone(), title.clone()))
                    == Some(&1);
            let mut projects = vec![first.clone()];
            let mut basis = Vec::new();
            let mut index = 0;
            while index < pool.len() {
                let other = &pool[index];
                let confirmed = id.is_some()
                    && identities.get(&other.project) == id.as_ref();
                let distinct =
                    identities.get(&other.project).is_some_and(|other_id| {
                        projects
                            .iter()
                            .filter_map(|project| {
                                identities.get(&project.project)
                            })
                            .any(|id| id != other_id)
                    });
                let pair = !distinct
                    && !confirmed
                    && wiki_pair(&first, other, &wiki_lookup);
                let exact = !confirmed
                    && !distinct
                    && !pair
                    && unambiguous
                    && other.project.source != first.project.source
                    && normalized_title(&other.title) == title
                    && title_counts
                        .get(&(other.project.source.clone(), title.clone()))
                        == Some(&1);
                if confirmed || pair || exact {
                    basis.push(if confirmed {
                        CandidateBasis::ConfirmedIdentity
                    } else if pair {
                        CandidateBasis::WikiSourcePair
                    } else {
                        CandidateBasis::ExactTitle
                    });
                    projects.push(pool.remove(index));
                } else {
                    index += 1;
                }
            }
            let status = if projects.iter().all(|project| {
                id.is_some() && identities.get(&project.project) == id.as_ref()
            }) {
                IdentityStatus::Confirmed
            } else {
                IdentityStatus::Unconfirmed
            };
            if status == IdentityStatus::Confirmed {
                state.seen_resources.extend(id.clone());
            }
            state
                .seen
                .extend(projects.iter().map(|project| project.project.clone()));
            let default = projects
                .iter()
                .find(|project| project.project.source == preferred)
                .unwrap_or(&projects[0])
                .project
                .clone();
            groups.push(ResourceSearchResult {
                entry: ResourceSearchEntry {
                    query_entry_id: format!(
                        "{}:{}",
                        first.project.source.as_str(),
                        first.project.project_id.as_str()
                    ),
                    resource_id: if status == IdentityStatus::Confirmed {
                        id
                    } else {
                        None
                    },
                    default_project: default,
                    candidates: projects
                        .iter()
                        .map(|project| project.project.clone())
                        .collect(),
                    status,
                    basis,
                },
                projects,
            });
        }
        for entry in pool {
            if let Some(source) = state
                .sources
                .iter_mut()
                .find(|source| source.source == entry.project.source)
            {
                source.entries.push_back(entry);
            }
        }
        if cancel.is_cancelled() {
            return Err(ResourceError::Canceled);
        }
        let has_more = state
            .sources
            .iter()
            .any(|source| !source.done || !source.entries.is_empty());
        let total = if state.sources.len() == 1
            && state.query.filter.is_none()
            && state.seen_resources.is_empty()
        {
            state.sources[0].total.clone()
        } else {
            ResourceTotal {
                value: None,
                precision: TotalPrecision::Unknown,
            }
        };
        let exhausted_sources = state
            .sources
            .iter()
            .filter(|source| source.exhausted)
            .map(|source| source.source.clone())
            .collect();
        failures.extend(
            state
                .sources
                .iter()
                .filter_map(|source| source.failure.clone()),
        );
        let next = if has_more {
            let mut same_session: Vec<_> = self
                .searches
                .iter()
                .filter(|cursor| cursor.session_id == state.session_id)
                .map(|cursor| (cursor.created_at, cursor.key().clone()))
                .collect();
            same_session.sort();
            for (_, key) in same_session
                .iter()
                .take(same_session.len().saturating_sub(1))
            {
                self.searches.remove(key);
            }
            if self.searches.len() >= 128
                && let Some(key) = self
                    .searches
                    .iter()
                    .min_by_key(|cursor| cursor.created_at)
                    .map(|cursor| cursor.key().clone())
            {
                self.searches.remove(&key);
            }
            state.created_at = Utc::now().timestamp_micros();
            state.expires_at = now + 600;
            let token = uuid::Uuid::new_v4().to_string();
            self.searches.insert(token.clone(), state);
            Some(token)
        } else {
            None
        };
        Ok(ResourcePage {
            request_id,
            entries: groups,
            cursor: next,
            has_more,
            total,
            source_failures: failures,
            exhausted_sources,
        })
    }
}

fn excluded_entry(
    filter: &Option<ResourceSearchExpression>,
    entry: &SourceProjectMetadata,
) -> bool {
    fn visit(
        expression: &ResourceSearchExpression,
        entry: &SourceProjectMetadata,
    ) -> bool {
        match expression {
            ResourceSearchExpression::All { terms } => {
                terms.iter().any(|term| visit(term, entry))
            }
            ResourceSearchExpression::Any { terms } => {
                terms.iter().all(|term| visit(term, entry))
            }
            ResourceSearchExpression::Predicate {
                field,
                values,
                excluded,
            } => {
                let project = &entry.project;
                if project.source != SourceId::modrinth()
                    && *excluded
                    && matches!(field.as_str(), "allTypes" | "type")
                {
                    return entry.types.iter().any(|kind| {
                        serde_json::to_value(kind)
                            .ok()
                            .and_then(|value| {
                                value.as_str().map(str::to_lowercase)
                            })
                            .is_some_and(|kind| {
                                values
                                    .iter()
                                    .any(|value| value.to_lowercase() == kind)
                            })
                    });
                }
                *excluded
                    && field == "projectId"
                    && values.iter().any(|value| {
                        value == project.project_id.as_str()
                            || value.split_once(':').is_some_and(
                                |(owner, value)| {
                                    owner == project.source.as_str()
                                        && value == project.project_id.as_str()
                                },
                            )
                    })
            }
        }
    }
    filter
        .as_ref()
        .is_some_and(|expression| visit(expression, entry))
}

pub(crate) fn source_failure(
    source: SourceId,
    error: ResourceError,
) -> ResourceSourceFailure {
    ResourceSourceFailure {
        source,
        code: ResourceIssueCode::SourceUnavailable,
        retryable: matches!(
            error,
            ResourceError::Network { .. } | ResourceError::Unavailable
        ),
        message: error.to_string(),
    }
}

fn sort_metadata(
    entries: &mut [SourceProjectMetadata],
    sort: ResourceSearchSort,
) {
    match sort {
        ResourceSearchSort::Downloads => {
            entries.sort_by_key(|entry| std::cmp::Reverse(entry.downloads))
        }
        ResourceSearchSort::Followers => {
            entries.sort_by_key(|entry| std::cmp::Reverse(entry.followers))
        }
        ResourceSearchSort::Created => {
            entries.sort_by_key(|entry| std::cmp::Reverse(entry.created_at))
        }
        ResourceSearchSort::ProjectPublished => entries
            .sort_by_key(|entry| std::cmp::Reverse(entry.project_published_at)),
        ResourceSearchSort::ProjectUpdated => {
            entries.sort_by_key(|entry| std::cmp::Reverse(entry.updated_at))
        }
        ResourceSearchSort::FirstPublished => entries
            .sort_by_key(|entry| std::cmp::Reverse(entry.first_published_at)),
        ResourceSearchSort::LatestPublished => entries
            .sort_by_key(|entry| std::cmp::Reverse(entry.latest_published_at)),
        ResourceSearchSort::Relevance => {}
    }
}

use chrono::Utc;
use dashmap::{DashMap, mapref::entry::Entry};
use futures::future::join_all;
use std::collections::HashMap;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tokio::sync::{Notify, Semaphore, watch};
use tokio_util::sync::CancellationToken;

use super::{
    cache::{CacheKey, ResourceCache},
    ipc::ResourceSearchQuery,
    providers::*,
    *,
};

#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    PartialEq,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub enum CachePolicy {
    UseFresh,
    /// Fetch current data, allowing presentation callers to retain offline cache.
    Revalidate,
    StaleWhileRevalidate,
    OnlyCached,
    /// Ignore positive, negative, and expired cache entries. A source error
    /// never becomes success by falling back to an old cached response.
    ForceRefresh,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueryOutcome {
    pub value: ProviderValue,
    pub from_cache: bool,
}

struct SourceRuntime {
    provider: Arc<dyn ResourceProvider>,
    semaphore: Arc<Semaphore>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct FlightKey {
    cache: CacheKey,
    policy: CachePolicy,
    generation: i64,
}

struct Flight {
    key: FlightKey,
    query: ProviderQuery,
    subscribers: AtomicUsize,
    result: watch::Sender<Option<ResourceResult<QueryOutcome>>>,
    changed: Arc<Notify>,
    started_at: i64,
    owner: std::sync::Weak<ResourceService>,
}

struct Subscription(Arc<Flight>);

fn try_subscribe(subscribers: &AtomicUsize) -> bool {
    subscribers
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
            if count == 0 {
                None
            } else {
                count.checked_add(1)
            }
        })
        .is_ok()
}

impl Drop for Subscription {
    fn drop(&mut self) {
        if self.0.subscribers.fetch_sub(1, Ordering::AcqRel) == 1
            && let Some(owner) = self.0.owner.upgrade()
        {
            owner.flights.remove_if(&self.0.key, |_, current| {
                Arc::ptr_eq(current, &self.0)
            });
        }
        self.0.changed.notify_waiters();
    }
}

impl Subscription {
    async fn wait(
        self,
        cancel: &CancellationToken,
    ) -> ResourceResult<QueryOutcome> {
        let mut receiver = self.0.result.subscribe();
        loop {
            if cancel.is_cancelled() {
                return Err(ResourceError::Canceled);
            }
            if let Some(value) = receiver.borrow_and_update().clone() {
                return value;
            }
            tokio::select! {
                biased;
                _ = cancel.cancelled() => return Err(ResourceError::Canceled),
                result = receiver.changed() => if result.is_err() { return Err(ResourceError::Canceled); },
            }
        }
    }
}

pub struct ResourceService {
    pub(crate) analysis_sessions:
        DashMap<SessionId, Arc<super::plans::analysis::AnalysisSession>>,
    pub(crate) artifact_metadata_reader:
        Arc<dyn super::plans::artifact_metadata::ArtifactMetadataReader>,
    pub(crate) artifact_metadata: DashMap<
        SourceArtifactRef,
        (ArtifactIntegrity, crate::mod_metadata::LocalModMetadata),
    >,
    pub(crate) export_plans: DashMap<
        PlanId,
        Arc<tokio::sync::Mutex<super::export::plan::PreparedExport>>,
    >,
    pub(crate) export_sessions: DashMap<
        SessionId,
        Arc<tokio::sync::Mutex<super::export::refresh::ExportDraft>>,
    >,
    pub(crate) searches: DashMap<String, super::search::SearchCursor>,
    pub(crate) catalogs: DashMap<String, super::catalog::CatalogCursor>,
    store: Option<crate::state::resources::ResourceStore>,
    cache: Arc<ResourceCache>,
    sources: HashMap<SourceId, SourceRuntime>,
    flights: DashMap<FlightKey, Arc<Flight>>,
    changed: Arc<Notify>,
}

impl ResourceService {
    pub(crate) fn search_applicable(
        &self,
        source: &SourceId,
        query: &ResourceSearchQuery,
    ) -> bool {
        self.sources.get(source).is_some_and(|runtime| {
            runtime
                .provider
                .validate_query(&ProviderQuery::Search {
                    query: query.clone(),
                    cursor: None,
                })
                .is_ok()
        })
    }

    pub fn new(
        cache: Arc<ResourceCache>,
        providers: Vec<Arc<dyn ResourceProvider>>,
    ) -> ResourceResult<Arc<Self>> {
        Self::build(cache, providers, None)
    }

    pub fn with_store(
        cache: Arc<ResourceCache>,
        providers: Vec<Arc<dyn ResourceProvider>>,
        store: crate::state::resources::ResourceStore,
    ) -> ResourceResult<Arc<Self>> {
        Self::build(cache, providers, Some(store))
    }

    fn build(
        cache: Arc<ResourceCache>,
        providers: Vec<Arc<dyn ResourceProvider>>,
        store: Option<crate::state::resources::ResourceStore>,
    ) -> ResourceResult<Arc<Self>> {
        let mut sources = HashMap::new();
        for provider in providers {
            let id = provider.source().id;
            if sources
                .insert(
                    id,
                    SourceRuntime {
                        provider,
                        semaphore: Arc::new(Semaphore::new(3)),
                    },
                )
                .is_some()
            {
                return Err(ResourceError::InvalidRequest(
                    "duplicate source adapter".into(),
                ));
            }
        }
        Ok(Arc::new(Self {
            analysis_sessions: DashMap::new(),
            artifact_metadata_reader: Arc::new(
                super::plans::artifact_metadata::RuntimeArtifactMetadataReader,
            ),
            artifact_metadata: DashMap::new(),
            export_sessions: DashMap::new(),
            export_plans: DashMap::new(),
            searches: DashMap::new(),
            catalogs: DashMap::new(),
            store,
            cache,
            sources,
            flights: DashMap::new(),
            changed: Arc::new(Notify::new()),
        }))
    }

    pub fn sources(&self) -> Vec<ResourceSource> {
        let mut sources: Vec<_> = self
            .sources
            .values()
            .map(|runtime| runtime.provider.source())
            .collect();
        sources.sort_by(|left, right| left.id.cmp(&right.id));
        sources
    }

    pub fn cache(&self) -> &Arc<ResourceCache> {
        &self.cache
    }

    fn refresh_background(
        self: &Arc<Self>,
        source: SourceId,
        queries: Vec<ProviderQuery>,
        generation: i64,
    ) {
        let service = self.clone();
        tokio::spawn(async move {
            let _ = service
                .query_many_at_generation(
                    &source,
                    queries,
                    CachePolicy::UseFresh,
                    &CancellationToken::new(),
                    Some(generation),
                )
                .await;
        });
    }

    pub fn store(&self) -> Option<&crate::state::resources::ResourceStore> {
        self.store.as_ref()
    }

    #[cfg(test)]
    pub(crate) fn subscriber_count(&self) -> usize {
        self.flights
            .iter()
            .map(|flight| flight.subscribers.load(Ordering::Acquire))
            .sum()
    }

    pub async fn query(
        self: &Arc<Self>,
        source: &SourceId,
        query: ProviderQuery,
        policy: CachePolicy,
        cancel: &CancellationToken,
    ) -> ResourceResult<QueryOutcome> {
        self.query_many(source, vec![query], policy, cancel)
            .await
            .into_iter()
            .next()
            .ok_or(ResourceError::Canceled)?
    }

    /// Results retain input order and per-item failures. Repeated keys share
    /// one operation; each caller owns only its own cancellation subscription.
    pub async fn query_many(
        self: &Arc<Self>,
        source: &SourceId,
        queries: Vec<ProviderQuery>,
        policy: CachePolicy,
        cancel: &CancellationToken,
    ) -> Vec<ResourceResult<QueryOutcome>> {
        self.query_many_at_generation(source, queries, policy, cancel, None)
            .await
    }

    async fn query_many_at_generation(
        self: &Arc<Self>,
        source: &SourceId,
        queries: Vec<ProviderQuery>,
        policy: CachePolicy,
        cancel: &CancellationToken,
        expected_generation: Option<i64>,
    ) -> Vec<ResourceResult<QueryOutcome>> {
        if cancel.is_cancelled() {
            return vec![Err(ResourceError::Canceled); queries.len()];
        }
        let Some(runtime) = self.sources.get(source) else {
            return vec![Err(ResourceError::Unavailable); queries.len()];
        };
        let capabilities = runtime.provider.source();
        if !capabilities.available
            && !matches!(
                policy,
                CachePolicy::OnlyCached | CachePolicy::StaleWhileRevalidate
            )
        {
            return vec![Err(ResourceError::Unavailable); queries.len()];
        }
        let namespace = runtime.provider.namespace();
        let original_queries = queries.clone();
        let local_release_pagination =
            runtime.provider.local_release_pagination();
        let generation = self.cache.generation().await;
        if expected_generation.is_some_and(|expected| expected != generation) {
            return vec![Err(ResourceError::Canceled); queries.len()];
        }
        let mut subscriptions = Vec::with_capacity(queries.len());
        let mut background = Vec::new();
        let mut fresh = Vec::new();
        for mut query in queries {
            if cancel.is_cancelled() {
                subscriptions.push(Err(ResourceError::Canceled));
                continue;
            }
            query.normalize();
            if local_release_pagination
                && let ProviderQuery::Releases {
                    cursor, page_size, ..
                } = &mut query
            {
                *cursor = None;
                *page_size = 0;
            }
            if let Err(error) = runtime.provider.validate_query(&query) {
                subscriptions.push(Err(error));
                continue;
            }
            if let ProviderQuery::Search { query, .. } = &query
                && !capabilities.supported_search_sorts.contains(&query.sort)
            {
                subscriptions.push(Err(ResourceError::Unsupported(
                    "source sort semantics unavailable".into(),
                )));
                continue;
            }
            if !capabilities.capabilities.contains(&query.capability())
                && !matches!(query, ProviderQuery::Identify { .. } | ProviderQuery::IdentifyExport { .. } if capabilities.capabilities.contains(&SourceCapability::IdentifyFingerprint))
            {
                subscriptions.push(Err(ResourceError::Unsupported(
                    "source capability unavailable".into(),
                )));
                continue;
            }
            let cache_key = match CacheKey::new(
                namespace.clone(),
                source.clone(),
                &query.cache_identity(),
            ) {
                Ok(key) => key,
                Err(error) => {
                    subscriptions.push(Err(error));
                    continue;
                }
            };
            if !matches!(
                policy,
                CachePolicy::ForceRefresh | CachePolicy::Revalidate
            ) {
                let (value, expired) =
                    if policy == CachePolicy::StaleWhileRevalidate {
                        self.cache
                            .get_with_freshness(&cache_key)
                            .await
                            .map(|(value, fresh)| (Some(value), !fresh))
                            .unwrap_or((None, false))
                    } else if policy == CachePolicy::OnlyCached {
                        (self.cache.get_any(&cache_key).await, false)
                    } else {
                        (self.cache.get(&cache_key).await, false)
                    };
                if let Some(value) = value
                    && validate_value(source, &query, &value).is_ok()
                {
                    if expired {
                        background.push(query.clone());
                    }
                    subscriptions.push(Ok(Ok(QueryOutcome {
                        value,
                        from_cache: true,
                    })));
                    continue;
                }
            }
            if policy == CachePolicy::OnlyCached {
                subscriptions.push(Err(ResourceError::Unavailable));
                continue;
            }
            if !capabilities.available {
                subscriptions.push(Err(ResourceError::Unavailable));
                continue;
            }
            let key = FlightKey {
                cache: cache_key,
                policy,
                generation,
            };
            let flight = match self.flights.entry(key.clone()) {
                Entry::Occupied(entry)
                    if try_subscribe(&entry.get().subscribers) =>
                {
                    entry.get().clone()
                }
                entry => {
                    let flight = Arc::new(Flight {
                        key: key.clone(),
                        query,
                        subscribers: AtomicUsize::new(1),
                        result: watch::channel(None).0,
                        changed: self.changed.clone(),
                        started_at: Utc::now().timestamp_micros(),
                        owner: Arc::downgrade(self),
                    });
                    match entry {
                        Entry::Occupied(mut entry) => {
                            entry.insert(flight.clone());
                        }
                        Entry::Vacant(entry) => {
                            entry.insert(flight.clone());
                        }
                    }
                    fresh.push(flight.clone());
                    flight
                }
            };
            subscriptions.push(Ok(Err(Subscription(flight))));
        }
        if !background.is_empty() {
            self.refresh_background(source.clone(), background, generation);
        }
        let mut groups: HashMap<u8, Vec<Arc<Flight>>> = HashMap::new();
        for flight in fresh {
            groups
                .entry(flight.query.batch_group())
                .or_default()
                .push(flight);
        }
        for group in groups.into_values() {
            let limit = group
                .first()
                .map(|flight| runtime.provider.batch_limit(&flight.query))
                .unwrap_or(1)
                .max(1);
            for batch in group.chunks(limit) {
                let service = self.clone();
                let provider = runtime.provider.clone();
                let semaphore = runtime.semaphore.clone();
                let batch = batch.to_vec();
                tokio::spawn(async move {
                    service.fetch_batch(provider, semaphore, batch).await;
                });
            }
        }
        let results = join_all(subscriptions.into_iter().map(
            |subscription| async move {
                match subscription {
                    Err(error) => Err(error),
                    Ok(Ok(value)) => Ok(value),
                    Ok(Err(subscription)) => subscription.wait(cancel).await,
                }
            },
        ))
        .await;
        if cancel.is_cancelled() {
            vec![Err(ResourceError::Canceled); results.len()]
        } else {
            results
                .into_iter()
                .zip(original_queries)
                .map(|(result, query)| {
                    if local_release_pagination
                        && let ProviderQuery::Releases {
                            cursor, page_size, ..
                        } = query
                    {
                        return result.and_then(|mut outcome| {
                            if let ProviderValue::Releases(page) = outcome.value
                            {
                                outcome.value =
                                    ProviderValue::Releases(slice_page(
                                        page.entries,
                                        offset(&cursor)?,
                                        page_size,
                                    )?);
                            }
                            Ok(outcome)
                        });
                    }
                    result
                })
                .collect()
        }
    }

    async fn fetch_batch(
        self: Arc<Self>,
        provider: Arc<dyn ResourceProvider>,
        semaphore: Arc<Semaphore>,
        batch: Vec<Arc<Flight>>,
    ) {
        let operation = async {
            let _permit = semaphore
                .acquire()
                .await
                .map_err(|_| ResourceError::Canceled)?;
            let active: Vec<_> = batch
                .iter()
                .filter(|flight| flight.subscribers.load(Ordering::Acquire) > 0)
                .cloned()
                .collect();
            if active.is_empty() {
                return Ok(());
            }
            let queries: Vec<_> =
                active.iter().map(|flight| flight.query.clone()).collect();
            let responses = provider.fetch_many(&queries).await;
            if responses.len() != active.len() {
                return Err(ResourceError::InvalidResponse(
                    "batch result count mismatch".into(),
                ));
            }
            for (flight, response) in active.iter().zip(responses) {
                let result = response.and_then(|value| {
                    if provider.namespace() != flight.key.cache.namespace {
                        return Err(ResourceError::Canceled);
                    }
                    validate_value(
                        &flight.key.cache.source,
                        &flight.query,
                        &value,
                    )?;
                    Ok(value)
                });
                if let Ok(value) = &result
                    && flight.subscribers.load(Ordering::Acquire) > 0
                {
                    self.cache
                        .put(
                            &flight.key.cache,
                            value,
                            flight.query.ttl(value),
                            flight.key.generation,
                            flight.started_at,
                        )
                        .await;
                    if let ProviderValue::Project(document) = value
                        && !matches!(
                            flight.query,
                            ProviderQuery::ProjectProtocol { .. }
                        )
                        && matches!(
                            flight.key.cache.source.as_str(),
                            "modrinth" | "mcarchive" | "planet_minecraft"
                        )
                    {
                        let query = ProviderQuery::Description {
                            id: document.metadata.project.project_id.clone(),
                            slug: None,
                        };
                        let description = ProviderValue::Description(
                            document.description.clone(),
                        );
                        if let Ok(key) = CacheKey::new(
                            provider.namespace(),
                            flight.key.cache.source.clone(),
                            &query,
                        ) {
                            self.cache
                                .put(
                                    &key,
                                    &description,
                                    query.ttl(&description),
                                    flight.key.generation,
                                    flight.started_at,
                                )
                                .await;
                        }
                    }
                    if let ProviderValue::Identified(documents) = value {
                        for document in documents {
                            for project_id in [
                                None,
                                Some(
                                    document
                                        .release
                                        .reference
                                        .project
                                        .project_id
                                        .clone(),
                                ),
                            ] {
                                let query = ProviderQuery::Release {
                                    project_id,
                                    release_id: document
                                        .release
                                        .reference
                                        .release_id
                                        .clone(),
                                    slug: None,
                                };
                                let value =
                                    ProviderValue::Release(document.clone());
                                if validate_value(
                                    &flight.key.cache.source,
                                    &query,
                                    &value,
                                )
                                .is_ok()
                                    && let Ok(key) = CacheKey::new(
                                        provider.namespace(),
                                        flight.key.cache.source.clone(),
                                        &query,
                                    )
                                {
                                    self.cache
                                        .put(
                                            &key,
                                            &value,
                                            query.ttl(&value),
                                            flight.key.generation,
                                            flight.started_at,
                                        )
                                        .await;
                                }
                            }
                        }
                    }
                    if let ProviderValue::Release(document) = value {
                        for project_id in [
                            None,
                            Some(
                                document
                                    .release
                                    .reference
                                    .project
                                    .project_id
                                    .clone(),
                            ),
                        ] {
                            let query = ProviderQuery::Release {
                                project_id,
                                release_id: document
                                    .release
                                    .reference
                                    .release_id
                                    .clone(),
                                slug: None,
                            };
                            if let Ok(key) = CacheKey::new(
                                provider.namespace(),
                                flight.key.cache.source.clone(),
                                &query,
                            ) {
                                self.cache
                                    .put(
                                        &key,
                                        value,
                                        query.ttl(value),
                                        flight.key.generation,
                                        flight.started_at,
                                    )
                                    .await;
                            }
                        }
                    }
                }
                flight.result.send_replace(Some(result.map(|value| {
                    QueryOutcome {
                        value,
                        from_cache: false,
                    }
                })));
            }
            Ok(())
        };
        let result = tokio::select! {
            result = operation => result,
            _ = wait_for_abandoned(&batch, &self.changed) => Err(ResourceError::Canceled),
        };
        for flight in batch {
            if let Err(error) = &result {
                flight.result.send_replace(Some(Err(error.clone())));
            }
            self.flights.remove_if(&flight.key, |_, current| {
                Arc::ptr_eq(current, &flight)
            });
        }
    }
}

async fn wait_for_abandoned(batch: &[Arc<Flight>], notify: &Notify) {
    loop {
        let notified = notify.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        if batch
            .iter()
            .all(|flight| flight.subscribers.load(Ordering::Acquire) == 0)
        {
            return;
        }
        notified.await;
    }
}

pub(crate) fn validate_value(
    source: &SourceId,
    query: &ProviderQuery,
    value: &ProviderValue,
) -> ResourceResult<()> {
    let valid = match (query, value) {
        (ProviderQuery::ReleasesProtocol { project_id }, ProviderValue::Protocol(value)) => {
            *source == SourceId::modrinth() && value.as_array().is_some_and(|values| values.iter().all(|value| value["project_id"].as_str() == Some(project_id.as_str()) && serde_json::from_value::<crate::state::Version>(value.clone()).is_ok()))
        }
        (ProviderQuery::ReleaseProtocol { release_id, version }, ProviderValue::Protocol(value)) => {
            *source == SourceId::modrinth() && version == "v3" && value["id"].as_str() == Some(release_id.as_str())
                && serde_json::from_value::<crate::state::VersionV3>(value.clone()).is_ok()
        }
        (ProviderQuery::SearchProtocol { version, .. }, ProviderValue::Protocol(value)) => {
            *source == SourceId::modrinth() && matches!(version.as_str(), "v2" | "v3") && value["hits"].is_array() && value["total_hits"].is_u64()
        }
        (ProviderQuery::ProjectProtocol { .. }, ProviderValue::Project(document)) => document.metadata.project.source == *source && document.description.source == *source && crate::state::instances::adapters::sqlite::resource_rows::validate_project(&document.metadata.project).is_ok(),
        (ProviderQuery::Locate { .. }, ProviderValue::Project(document)) => document.metadata.project.source == *source && document.description.source == *source && crate::state::instances::adapters::sqlite::resource_rows::validate_project(&document.metadata.project).is_ok(),
        (_, ProviderValue::NotFound) => !matches!(
            query,
            ProviderQuery::Search { .. } | ProviderQuery::Releases { .. }
        ),
        (
            ProviderQuery::Project { id, .. },
            ProviderValue::Project(document),
        ) => {
            document.metadata.project.source == *source
                && document.metadata.project.project_id == *id
                && document.description.source == *source
        }
        (
            ProviderQuery::Description { .. },
            ProviderValue::Description(text),
        )
        | (ProviderQuery::Changelog { .. }, ProviderValue::Changelog(text)) => {
            text.source == *source
        }
        (
            ProviderQuery::Release {
                project_id,
                release_id,
                ..
            },
            ProviderValue::Release(document),
        ) => {
            document.release.reference.project.source == *source
                && project_id.as_ref().is_none_or(|id| {
                    document.release.reference.project.project_id == *id
                })
                && document.release.reference.release_id == *release_id
                && document.release.artifacts.iter().all(|artifact| {
                    artifact.reference.release == document.release.reference
                        && valid_artifact(artifact)
                })
        }
        (
            ProviderQuery::Releases { project_id, .. },
            ProviderValue::Releases(page),
        ) => page.entries.iter().all(|release| {
            release.reference.project.source == *source
                && release.reference.project.project_id == *project_id
                && release.artifacts.iter().all(|artifact| {
                    artifact.reference.release == release.reference
                        && valid_artifact(artifact)
                })
        }),
        (ProviderQuery::Search { .. }, ProviderValue::Search(page)) => page
            .entries
            .iter()
            .all(|project| project.project.source == *source),
        (ProviderQuery::Identify { .. }, ProviderValue::Matches(matches)) => {
            matches.iter().all(|artifact| {
                artifact.reference.release.project.source == *source
                    && valid_artifact(artifact)
            })
        }
        (ProviderQuery::IdentifyExport { .. }, ProviderValue::Identified(documents)) => documents.iter().all(|document| {
            document.release.reference.project.source == *source
                && document.release.artifacts.iter().all(|artifact| artifact.reference.release == document.release.reference && valid_artifact(artifact))
        }),
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(ResourceError::InvalidResponse(
            "response identity or query kind mismatch".into(),
        ))
    }
}

pub(crate) fn valid_artifact(artifact: &ResourceArtifact) -> bool {
    if artifact.file_name.as_ref().is_some_and(|name| {
        name.is_empty()
            || matches!(name.as_str(), "." | "..")
            || name
                .chars()
                .any(|character| matches!(character, '/' | '\\'))
    }) {
        return false;
    }
    if hashes(
        artifact
            .integrity
            .hashes
            .iter()
            .map(|hash| (hash.algorithm, hash.value.clone())),
    )
    .is_err()
    {
        return false;
    }
    match &artifact.acquisition {
        ArtifactAcquisition::PublicDownload { urls } => {
            !urls.is_empty() && urls.iter().all(|url| public_url(url).is_some())
        }
        ArtifactAcquisition::ManualDownload { page_url } => {
            public_url(page_url).is_some()
        }
        ArtifactAcquisition::Unavailable => true,
    }
}

#[cfg(test)]
mod subscription_tests {
    use super::*;

    #[test]
    fn joining_never_resurrects_a_flight_abandoned_by_the_last_subscriber() {
        let count = Arc::new(AtomicUsize::new(1));
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let departure = std::thread::spawn({
            let count = count.clone();
            let barrier = barrier.clone();
            move || {
                assert_eq!(count.fetch_sub(1, Ordering::AcqRel), 1);
                barrier.wait();
            }
        });
        barrier.wait();
        assert!(!try_subscribe(&count));
        assert_eq!(count.load(Ordering::Acquire), 0);
        departure.join().unwrap();
        count.store(1, Ordering::Release);
        assert!(try_subscribe(&count));
        assert_eq!(count.fetch_sub(1, Ordering::AcqRel), 2);
        assert_eq!(count.load(Ordering::Acquire), 1);
    }
}

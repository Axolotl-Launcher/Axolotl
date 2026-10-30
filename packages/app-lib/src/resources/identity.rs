//! Byte evidence is separate from project identity and search candidates.

mod counterparts;

use chrono::Utc;
use futures::future::join_all;
use tokio_util::sync::CancellationToken;

use super::{
    providers::{ProviderQuery, ProviderValue},
    *,
};

pub const INITIAL_ARTIFACT_PROBE_LIMIT: usize = 3;
pub const HISTORY_PAGE_LIMIT: usize = 2;
pub const HISTORY_ARTIFACT_PROBE_LIMIT: usize = 10;

pub(crate) fn recognition_source_failure(error: &ResourceError) -> bool {
    matches!(
        error,
        ResourceError::Network { .. }
            | ResourceError::Unavailable
            | ResourceError::InvalidResponse(_)
            | ResourceError::Unsupported(_)
    )
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct LocalArtifactIdentity {
    pub integrity: ArtifactIntegrity,
    pub fingerprint: Option<u32>,
}

fn local_identity_key(integrity: &ArtifactIntegrity) -> Option<String> {
    // The scanner's current SHA-1 and size address the bytes, independently
    // of their path, provider recognition, or a cart's instance revision.
    let sha1 = integrity.hashes.iter().find(|hash| {
        hash.algorithm == HashAlgorithm::Sha1
            && hash.value.len() == 40
            && hash.value.bytes().all(|byte| byte.is_ascii_hexdigit())
    })?;
    Some(format!(
        "local-identity-v1:{}:{}",
        integrity.size?,
        sha1.value.to_ascii_lowercase()
    ))
}

impl super::cache::ResourceCache {
    pub(crate) async fn local_identity(
        &self,
        integrity: &ArtifactIntegrity,
    ) -> Option<LocalArtifactIdentity> {
        let cached: LocalArtifactIdentity =
            self.evidence(&local_identity_key(integrity)?).await?;
        (exact_integrity(integrity, &cached.integrity)
            && cached.fingerprint.is_some()
            && [HashAlgorithm::Sha256, HashAlgorithm::Sha512].iter().all(
                |algorithm| {
                    cached.integrity.hashes.iter().any(|hash| {
                        &hash.algorithm == algorithm
                            && hash.value.len()
                                == if *algorithm == HashAlgorithm::Sha256 {
                                    64
                                } else {
                                    128
                                }
                            && hash
                                .value
                                .bytes()
                                .all(|byte| byte.is_ascii_hexdigit())
                    })
                },
            ))
        .then_some(cached)
    }

    pub(crate) async fn store_local_identity(
        &self,
        identity: &LocalArtifactIdentity,
        generation: i64,
    ) {
        if let Some(key) = local_identity_key(&identity.integrity) {
            self.store_evidence(&key, identity, generation).await;
        }
    }
}

#[derive(Clone, Debug)]
pub struct FileRecognition {
    pub matches: Vec<ResourceArtifact>,
    pub unresolved_sources: Vec<SourceId>,
    pub failures: Vec<(SourceId, ResourceError)>,
    pub ambiguous: bool,
}

#[derive(Clone, Debug)]
pub struct ProjectProbeResult {
    pub status: IdentityStatus,
    /// Byte evidence obtained in this probe. A previously confirmed main-DB
    /// relationship does not imply equivalence of the current release files.
    pub evidence: Vec<VerifiedArtifactMatch>,
    pub failures: Vec<(SourceId, ResourceError)>,
    pub probed_artifacts: usize,
    pub history_pages: usize,
}

/// Shared strong hashes must agree, and both sizes must be known and equal.
/// MD5 alone, fingerprints, attachments and unknown roles are insufficient.
pub fn exact_integrity(
    left: &ArtifactIntegrity,
    right: &ArtifactIntegrity,
) -> bool {
    if left.size.is_none() || left.size != right.size {
        return false;
    }
    let mut strong = false;
    for hash in &left.hashes {
        for other in right
            .hashes
            .iter()
            .filter(|other| other.algorithm == hash.algorithm)
        {
            if !hash.value.eq_ignore_ascii_case(&other.value) {
                return false;
            }
            strong |= matches!(
                hash.algorithm,
                HashAlgorithm::Sha1
                    | HashAlgorithm::Sha256
                    | HashAlgorithm::Sha512
            );
        }
    }
    strong
}

/// A byte match alone does not establish that two projects are the same resource.
pub(crate) fn project_confirmation_rejection(
    left: &SourceProjectMetadata,
    right: &SourceProjectMetadata,
    left_artifact: &ResourceArtifact,
    right_artifact: &ResourceArtifact,
) -> Option<&'static str> {
    if left_artifact.reference.release.project != left.project
        || right_artifact.reference.release.project != right.project
    {
        Some("artifact belongs to a different project")
    } else if !left.types.iter().any(|kind| right.types.contains(kind)) {
        Some("project resource types do not overlap")
    } else if !super::valid_artifact(left_artifact)
        || !super::valid_artifact(right_artifact)
    {
        Some("artifact metadata is incomplete or invalid")
    } else if verify_artifact_pair(left_artifact, right_artifact).is_none() {
        Some("primary artifacts lack matching strong byte evidence")
    } else {
        None
    }
}

/// Optional discovery may retain a valid primary source without linking a
/// counterpart. Persistence failures and cancellation still fail the operation.
pub(crate) async fn try_confirm_projects(
    store: &crate::state::resources::ResourceStore,
    left: &SourceProjectMetadata,
    right: &SourceProjectMetadata,
    left_artifact: &ResourceArtifact,
    right_artifact: &ResourceArtifact,
    cancel: &CancellationToken,
) -> ResourceResult<Option<ResourceId>> {
    if cancel.is_cancelled() {
        return Err(ResourceError::Canceled);
    }
    if let Some(reason) = project_confirmation_rejection(
        left,
        right,
        left_artifact,
        right_artifact,
    ) {
        tracing::info!(
            left = ?left.project,
            right = ?right.project,
            left_types = ?left.types,
            right_types = ?right.types,
            left_role = ?left_artifact.role,
            right_role = ?right_artifact.role,
            left_size = ?left_artifact.integrity.size,
            right_size = ?right_artifact.integrity.size,
            reason,
            "Skipped optional resource project association"
        );
        return Ok(None);
    }
    store
        .confirm_projects(left, right, left_artifact, right_artifact)
        .await
        .map(Some)
}

pub fn verify_artifact_pair(
    left: &ResourceArtifact,
    right: &ResourceArtifact,
) -> Option<VerifiedArtifactMatch> {
    if left.role != ArtifactRole::Primary
        || right.role != ArtifactRole::Primary
        || !exact_integrity(&left.integrity, &right.integrity)
    {
        return None;
    }
    Some(VerifiedArtifactMatch {
        left: left.reference.clone(),
        right: right.reference.clone(),
        integrity: left.integrity.clone(),
        verified_at: Utc::now(),
    })
}

impl ResourceService {
    /// Recognition can find the same bytes on differently classified projects.
    /// Try usable metadata without changing an explicitly recorded identity.
    pub(crate) async fn select_recognized_artifact(
        self: &std::sync::Arc<Self>,
        matches: &[ResourceArtifact],
        existing: Option<&SourceArtifactRef>,
        origin: Option<&SourceId>,
        preferred: &SourceId,
        kind: ResourceType,
        ambiguous: bool,
        policy: CachePolicy,
        cancel: &CancellationToken,
    ) -> ResourceResult<Option<(ResourceArtifact, SourceProjectMetadata)>> {
        let pinned = if let Some(existing) = existing {
            Some(existing.release.project.clone())
        } else if let Some(origin) = origin {
            let projects = matches
                .iter()
                .filter(|artifact| {
                    &artifact.reference.release.project.source == origin
                })
                .map(|artifact| artifact.reference.release.project.clone())
                .collect::<std::collections::HashSet<_>>();
            if projects.len() != 1 {
                return Ok(None);
            }
            projects.into_iter().next()
        } else {
            if ambiguous {
                return Ok(None);
            }
            None
        };
        let pinned_id =
            if let (Some(store), Some(project)) = (self.store(), &pinned) {
                store.lookup(project).await?
            } else {
                None
            };
        let mut candidates = matches.iter().collect::<Vec<_>>();
        candidates.sort_by_key(|artifact| {
            (
                existing != Some(&artifact.reference),
                pinned.as_ref() != Some(&artifact.reference.release.project),
                &artifact.reference.release.project.source != preferred,
            )
        });
        let mut failure = None;
        for artifact in candidates {
            if cancel.is_cancelled() {
                return Err(ResourceError::Canceled);
            }
            let project = &artifact.reference.release.project;
            if let Some(pinned) = &pinned
                && project != pinned
                && !(if let (Some(store), Some(id)) = (self.store(), &pinned_id)
                {
                    store.lookup(project).await?.as_ref() == Some(id)
                } else {
                    false
                })
            {
                continue;
            }
            let metadata = match self
                .project_document_with_policy(project, None, cancel, policy)
                .await
            {
                Ok(document) => document.metadata,
                Err(error) if recognition_source_failure(&error) => {
                    failure = Some(error);
                    continue;
                }
                Err(error) => return Err(error),
            };
            if !metadata.types.contains(&kind) {
                continue;
            }
            return Ok(Some((artifact.clone(), metadata)));
        }
        if let Some(error) = failure {
            Err(error)
        } else {
            Ok(None)
        }
    }

    /// Uses metadata hashes and at most two release pages. Sources requiring
    /// a local fingerprint are probed by the opposing hash-capable source,
    /// without downloading files or enumerating an entire project history.
    pub async fn probe_projects(
        self: &std::sync::Arc<Self>,
        left: &SourceProjectRef,
        left_slug: Option<String>,
        right: &SourceProjectRef,
        right_slug: Option<String>,
        target: ipc::ResourceTarget,
        cancel: &CancellationToken,
    ) -> ProjectProbeResult {
        let mut output = ProjectProbeResult {
            status: IdentityStatus::Unconfirmed,
            evidence: vec![],
            failures: vec![],
            probed_artifacts: 0,
            history_pages: 0,
        };
        if cancel.is_cancelled() {
            return output;
        }
        if let Some(store) = self.store() {
            let identities =
                join_all([store.lookup(left), store.lookup(right)]).await;
            for (source, identity) in
                [&left.source, &right.source].into_iter().zip(&identities)
            {
                if let Err(error) = identity {
                    output.failures.push((source.clone(), error.clone()));
                    return output;
                }
            }
            if matches!((&identities[0], &identities[1]), (Ok(Some(left)), Ok(Some(right))) if left == right)
            {
                output.status = IdentityStatus::Confirmed;
                return output;
            }
        }
        let cache_generation = self.cache().generation().await;
        let projects = join_all([
            self.query(
                &left.source,
                ProviderQuery::Project {
                    id: left.project_id.clone(),
                    slug: left_slug.clone(),
                },
                CachePolicy::UseFresh,
                cancel,
            ),
            self.query(
                &right.source,
                ProviderQuery::Project {
                    id: right.project_id.clone(),
                    slug: right_slug.clone(),
                },
                CachePolicy::UseFresh,
                cancel,
            ),
        ])
        .await;
        let mut metadata = vec![];
        for (source, project) in
            [&left.source, &right.source].into_iter().zip(projects)
        {
            match project {
                Ok(QueryOutcome {
                    value: ProviderValue::Project(document),
                    ..
                }) => metadata.push(document.metadata),
                Err(error) => {
                    output.failures.push((source.clone(), error));
                    return output;
                }
                _ => return output,
            }
        }
        if metadata.iter().any(|project| project.types.is_empty()) {
            return output;
        }
        if !metadata[0]
            .types
            .iter()
            .any(|kind| metadata[1].types.contains(kind))
        {
            output.status = IdentityStatus::Rejected;
            return output;
        }
        let reverse = self
            .sources()
            .iter()
            .find(|source| source.id == right.source)
            .is_some_and(|source| {
                source
                    .capabilities
                    .contains(&SourceCapability::IdentifyFingerprint)
            });
        let (origin, slug, destination) = if reverse {
            (right, right_slug, left)
        } else {
            (left, left_slug, right)
        };
        let mut budget = ProbeBudget::default();
        let mut cursor = None;
        let mut seen = std::collections::HashSet::new();
        loop {
            if !budget.take_history_page() {
                break;
            }
            let response = self
                .query(
                    &origin.source,
                    ProviderQuery::Releases {
                        project_id: origin.project_id.clone(),
                        slug: slug.clone(),
                        target: target.clone(),
                        cursor: cursor.clone(),
                        page_size: 50,
                    },
                    CachePolicy::UseFresh,
                    cancel,
                )
                .await;
            let page = match response {
                Ok(QueryOutcome {
                    value: ProviderValue::Releases(page),
                    ..
                }) => page,
                Ok(_) => {
                    output.failures.push((
                        origin.source.clone(),
                        ResourceError::InvalidResponse(
                            "expected release page".into(),
                        ),
                    ));
                    break;
                }
                Err(error) => {
                    output.failures.push((origin.source.clone(), error));
                    break;
                }
            };
            output.history_pages += 1;
            let candidates: Vec<_> = page
                .entries
                .into_iter()
                .flat_map(|release| release.artifacts)
                .filter(|artifact| {
                    artifact.role == ArtifactRole::Primary
                        && artifact.integrity.size.is_some()
                        && seen.insert(artifact.reference.clone())
                })
                .collect();
            let initial = if output.history_pages == 1 {
                budget.take_initial(candidates.len())
            } else {
                0
            };
            let remaining = budget.take_history_artifacts(
                candidates.len().saturating_sub(initial),
            );
            for batch in [
                candidates.get(..initial).unwrap_or_default(),
                candidates
                    .get(initial..initial + remaining)
                    .unwrap_or_default(),
            ] {
                if batch.is_empty() {
                    continue;
                }
                let files: Vec<_> = batch
                    .iter()
                    .map(|artifact| LocalArtifactIdentity {
                        integrity: artifact.integrity.clone(),
                        fingerprint: None,
                    })
                    .collect();
                let results = self
                    .identify_many(
                        std::slice::from_ref(&destination.source),
                        &files,
                        CachePolicy::UseFresh,
                        cancel,
                    )
                    .await;
                output.probed_artifacts += batch.len();
                for (artifact, recognized) in batch.iter().zip(results) {
                    output.failures.extend(recognized.failures);
                    if recognized.ambiguous {
                        continue;
                    }
                    for matched in recognized.matches.iter().filter(|matched| {
                        matched.reference.release.project == *destination
                    }) {
                        if let Some(evidence) = self
                            .record_artifact_evidence_at_generation(
                                artifact,
                                matched,
                                cache_generation,
                            )
                            .await
                        {
                            if cancel.is_cancelled() {
                                return output;
                            }
                            if let Some(store) = self.store() {
                                let (origin_metadata, destination_metadata) =
                                    if reverse {
                                        (&metadata[1], &metadata[0])
                                    } else {
                                        (&metadata[0], &metadata[1])
                                    };
                                if let Err(error) = store
                                    .confirm_projects(
                                        origin_metadata,
                                        destination_metadata,
                                        artifact,
                                        matched,
                                    )
                                    .await
                                {
                                    output
                                        .failures
                                        .push((origin.source.clone(), error));
                                    continue;
                                }
                            }
                            output.evidence.push(evidence);
                        }
                    }
                }
                if !output.evidence.is_empty() {
                    output.status = IdentityStatus::Confirmed;
                    return output;
                }
                if cancel.is_cancelled() {
                    return output;
                }
            }
            let next = page.cursor;
            if next.is_none() || next == cursor {
                break;
            }
            cursor = next;
        }
        output
    }

    /// Export refresh passes every selected file, without the normal probe cap.
    pub async fn identify_many(
        self: &std::sync::Arc<Self>,
        sources: &[SourceId],
        files: &[LocalArtifactIdentity],
        policy: CachePolicy,
        cancel: &CancellationToken,
    ) -> Vec<FileRecognition> {
        let mut sources = sources.to_vec();
        sources.sort();
        sources.dedup();
        let responses = join_all(sources.iter().map(|source| {
            self.query_many(
                source,
                files
                    .iter()
                    .map(|file| ProviderQuery::Identify {
                        integrity: file.integrity.clone(),
                        fingerprint: file.fingerprint,
                    })
                    .collect(),
                policy,
                cancel,
            )
        }))
        .await;
        let mut output: Vec<_> = files
            .iter()
            .map(|_| FileRecognition {
                matches: vec![],
                unresolved_sources: vec![],
                failures: vec![],
                ambiguous: false,
            })
            .collect();
        for (source, responses) in sources.into_iter().zip(responses) {
            for ((file, output), response) in
                files.iter().zip(output.iter_mut()).zip(responses)
            {
                match response {
                    Ok(outcome) => match outcome.value {
                        ProviderValue::Matches(candidates) => {
                            let candidates: Vec<_> = candidates
                                .into_iter()
                                .filter(|candidate| {
                                    candidate.role == ArtifactRole::Primary
                                        && exact_integrity(
                                            &file.integrity,
                                            &candidate.integrity,
                                        )
                                })
                                .collect();
                            let projects: std::collections::HashSet<_> =
                                candidates
                                    .iter()
                                    .map(|candidate| {
                                        candidate
                                            .reference
                                            .release
                                            .project
                                            .clone()
                                    })
                                    .collect();
                            output.ambiguous |= projects.len() > 1;
                            if candidates.is_empty() {
                                output.unresolved_sources.push(source.clone());
                            }
                            output.matches.extend(candidates);
                        }
                        ProviderValue::NotFound => {
                            output.unresolved_sources.push(source.clone())
                        }
                        _ => output.failures.push((
                            source.clone(),
                            ResourceError::InvalidResponse(
                                "unexpected identification response".into(),
                            ),
                        )),
                    },
                    Err(error) => output.failures.push((source.clone(), error)),
                }
            }
        }
        output
    }

    /// Stable main-database business associations are introduced in phase three.
    pub async fn record_artifact_evidence(
        &self,
        left: &ResourceArtifact,
        right: &ResourceArtifact,
    ) -> Option<VerifiedArtifactMatch> {
        self.record_artifact_evidence_at_generation(
            left,
            right,
            self.cache().generation().await,
        )
        .await
    }

    async fn record_artifact_evidence_at_generation(
        &self,
        left: &ResourceArtifact,
        right: &ResourceArtifact,
        generation: i64,
    ) -> Option<VerifiedArtifactMatch> {
        let key = evidence_key(&left.reference, &right.reference)?;
        let evidence = verify_artifact_pair(left, right);
        if let Some(evidence) = &evidence {
            self.cache()
                .store_evidence(&key, evidence, generation)
                .await;
        } else {
            self.cache().invalidate_evidence(&key).await;
        }
        evidence
    }

    pub async fn artifact_evidence(
        &self,
        left: &SourceArtifactRef,
        right: &SourceArtifactRef,
    ) -> Option<VerifiedArtifactMatch> {
        let key = evidence_key(left, right)?;
        let evidence: VerifiedArtifactMatch =
            self.cache().evidence(&key).await?;
        let references_match = (&evidence.left == left
            && &evidence.right == right)
            || (&evidence.left == right && &evidence.right == left);
        if references_match
            && evidence.integrity.size.is_some()
            && !evidence.integrity.hashes.is_empty()
        {
            Some(evidence)
        } else {
            None
        }
    }
}

fn evidence_key(
    left: &SourceArtifactRef,
    right: &SourceArtifactRef,
) -> Option<String> {
    use sha2::{Digest, Sha256};
    let mut refs = [
        serde_json::to_string(left).ok()?,
        serde_json::to_string(right).ok()?,
    ];
    refs.sort();
    Some(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&refs).ok()?)
    ))
}

pub struct ProbeBudget {
    initial_remaining: usize,
    history_pages_remaining: usize,
    history_artifacts_remaining: usize,
}

impl Default for ProbeBudget {
    fn default() -> Self {
        Self {
            initial_remaining: INITIAL_ARTIFACT_PROBE_LIMIT,
            history_pages_remaining: HISTORY_PAGE_LIMIT,
            history_artifacts_remaining: HISTORY_ARTIFACT_PROBE_LIMIT,
        }
    }
}

impl ProbeBudget {
    pub fn take_initial(&mut self, count: usize) -> usize {
        let count = count.min(self.initial_remaining);
        self.initial_remaining -= count;
        count
    }
    pub fn take_history_page(&mut self) -> bool {
        if self.history_pages_remaining == 0
            || self.history_artifacts_remaining == 0
        {
            return false;
        }
        self.history_pages_remaining -= 1;
        true
    }
    pub fn take_history_artifacts(&mut self, count: usize) -> usize {
        let count = count.min(self.history_artifacts_remaining);
        self.history_artifacts_remaining -= count;
        count
    }
}

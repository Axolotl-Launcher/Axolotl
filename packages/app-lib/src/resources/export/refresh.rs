use super::{categories, scan::linked};
use crate::resources::{
    identity::exact_integrity,
    providers::{
        ProjectDocument, ProviderQuery, ProviderValue, ReleaseDocument,
    },
    *,
};
use sha2::{Digest, Sha256, Sha512};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::Arc,
    time::{Instant, UNIX_EPOCH},
};
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::sync::CancellationToken;

pub struct ExportDraft {
    pub reuse_frozen: bool,
    pub accepted_failures: Vec<ExportEmbeddedFailureChoice>,
    pub queued: Option<super::job::QueuedExport>,
    pub scan: ResourceExportScan,
    pub results: HashMap<String, ExportResolvedFile>,
    pub fingerprints: HashMap<String, u32>,
    pub releases: HashMap<SourceReleaseRef, ReleaseDocument>,
    pub projects: HashMap<SourceProjectRef, ProjectDocument>,
    pub selection_revision: u32,
    pub selected: Vec<String>,
    pub touched: Instant,
}

impl ExportDraft {
    pub fn new(scan: ResourceExportScan) -> Self {
        Self {
            reuse_frozen: false,
            accepted_failures: vec![],
            queued: None,
            scan,
            results: HashMap::new(),
            fingerprints: HashMap::new(),
            releases: HashMap::new(),
            projects: HashMap::new(),
            selection_revision: 0,
            selected: vec![],
            touched: Instant::now(),
        }
    }
    pub fn response(&self, request: RequestId) -> ExportResolutionSession {
        let files = self
            .selected
            .iter()
            .filter_map(|path| self.results.get(path).cloned())
            .collect::<Vec<_>>();
        let pending = files.len() != self.selected.len()
            || files.iter().any(|file| {
                file.local_error.is_none()
                    && file.snapshot.identifiable
                    && [SourceId::modrinth(), SourceId::curseforge()]
                        .iter()
                        .any(|source| {
                            !file.resolutions.iter().any(|resolution| {
                                match resolution {
                                    ExportSourceResolution::Matched {
                                        source: s,
                                        ..
                                    }
                                    | ExportSourceResolution::NoMatch {
                                        source: s,
                                    }
                                    | ExportSourceResolution::Failed {
                                        source: s,
                                        ..
                                    } => s == source,
                                }
                            })
                        })
            });
        let failures = files.iter().any(|file| {
            file.local_error.is_some()
                || file
                    .resolutions
                    .iter()
                    .any(|r| matches!(r, ExportSourceResolution::Failed { .. }))
        });
        ExportResolutionSession {
            id: self.scan.session_id.clone(),
            request_id: request,
            selection_revision: self.selection_revision,
            instance_revision: self.scan.instance_revision,
            state: if pending {
                ExportResolutionState::Refreshing
            } else if failures {
                ExportResolutionState::NeedsDecision
            } else {
                ExportResolutionState::Ready
            },
            files,
        }
    }
}

impl ResourceService {
    pub fn register_export(
        &self,
        scan: ResourceExportScan,
    ) -> ResourceResult<()> {
        self.export_sessions.retain(|_, draft| {
            draft
                .try_lock()
                .map_or(true, |draft| draft.touched.elapsed().as_secs() < 3600)
        });
        self.export_plans.retain(|_, prepared| {
            prepared.try_lock().map_or(true, |p| {
                self.export_sessions.contains_key(&p.plan.session_id)
            })
        });
        if self.export_sessions.len() >= 32 {
            return Err(ResourceError::InvalidRequest(
                "too many active export sessions; close an export window"
                    .into(),
            ));
        }
        self.export_sessions.insert(
            scan.session_id.clone(),
            Arc::new(tokio::sync::Mutex::new(ExportDraft::new(scan))),
        );
        Ok(())
    }
    pub fn close_export(&self, id: &SessionId) {
        self.export_sessions.remove(id);
        self.export_plans.retain(|_, prepared| {
            prepared
                .try_lock()
                .map_or(true, |p| &p.plan.session_id != id)
        });
    }
    pub fn export_draft(
        &self,
        id: &SessionId,
    ) -> ResourceResult<Arc<tokio::sync::Mutex<ExportDraft>>> {
        self.export_sessions
            .get(id)
            .map(|value| value.clone())
            .ok_or_else(|| {
                ResourceError::InvalidRequest(
                    "export session expired; reopen the export window".into(),
                )
            })
    }
}

fn invalid(message: impl Into<String>) -> ResourceError {
    ResourceError::InvalidRequest(message.into())
}
fn canceled(cancel: &CancellationToken) -> ResourceResult<()> {
    if cancel.is_cancelled() {
        Err(ResourceError::Canceled)
    } else {
        Ok(())
    }
}

pub async fn freeze(
    root: &Path,
    original: &ExportFileSnapshot,
    cancel: &CancellationToken,
) -> ResourceResult<(ExportFileSnapshot, u32)> {
    freeze_with_progress(root, original, cancel, &|_, _| {}).await
}

pub async fn freeze_with_progress(
    root: &Path,
    original: &ExportFileSnapshot,
    cancel: &CancellationToken,
    progress: &(dyn Fn(u64, u64) + Send + Sync),
) -> ResourceResult<(ExportFileSnapshot, u32)> {
    canceled(cancel)?;
    let safe = path_util::SafeRelativeUtf8UnixPathBuf::try_from(
        original.relative_path.clone(),
    )
    .map_err(|e| invalid(e.to_string()))?;
    if categories::forbidden(safe.as_str()) {
        return Err(invalid("protected export path"));
    }
    let mut path = root.to_path_buf();
    for segment in safe.as_str().split('/') {
        path.push(segment);
        let metadata = tokio::fs::symlink_metadata(&path)
            .await
            .map_err(|e| invalid(format!("{}: {e}", original.relative_path)))?;
        if linked(&metadata) {
            return Err(invalid(
                "export path contains a symbolic link or junction",
            ));
        }
    }
    let mut file = super::files::open_async(root, safe.as_str())
        .await
        .map_err(|e| invalid(e.to_string()))?;
    let before = file.metadata().await.map_err(|e| invalid(e.to_string()))?;
    if !before.is_file() {
        return Err(invalid("export path is no longer a file"));
    }
    let work_size = before.len().saturating_mul(2).max(1);
    progress(0, work_size);
    let mut buffer = vec![0; 256 * 1024];
    let mut sha1 = sha1_smol::Sha1::new();
    let mut sha256 = Sha256::new();
    let mut sha512 = Sha512::new();
    let mut normalized = 0u64;
    let mut size = 0u64;
    let mut identifiable = original.identifiable;
    loop {
        let read = tokio::select! { biased; _ = cancel.cancelled() => return Err(ResourceError::Canceled), result = file.read(&mut buffer) => result.map_err(|e| invalid(e.to_string()))? };
        if read == 0 {
            break;
        }
        let bytes = &buffer[..read];
        if size == 0
            && (bytes.starts_with(b"PK\x03\x04")
                || bytes.starts_with(b"Rar!\x1a\x07"))
        {
            identifiable = true;
        }
        sha1.update(bytes);
        sha256.update(bytes);
        sha512.update(bytes);
        size += read as u64;
        if size > before.len() {
            return Err(invalid("file grew while calculating export hashes"));
        }
        normalized += bytes
            .iter()
            .filter(|byte| !matches!(byte, 9 | 10 | 13 | 32))
            .count() as u64;
        progress(size, work_size);
    }
    file.rewind().await.map_err(|e| invalid(e.to_string()))?;
    let mut fingerprint =
        crate::util::fingerprint::Murmur2Hasher::new(normalized as u32);
    let mut second_sha1 = sha1_smol::Sha1::new();
    let mut second_size = 0u64;
    if identifiable {
        loop {
            let read = tokio::select! { biased; _ = cancel.cancelled() => return Err(ResourceError::Canceled), result = file.read(&mut buffer) => result.map_err(|e| invalid(e.to_string()))? };
            if read == 0 {
                break;
            }
            fingerprint.update(&buffer[..read]);
            second_sha1.update(&buffer[..read]);
            second_size = second_size.saturating_add(read as u64);
            progress(size.saturating_add(second_size), work_size);
        }
        if second_sha1.digest() != sha1.digest() {
            return Err(invalid(
                "file changed while calculating its fingerprint",
            ));
        }
    }
    let after = tokio::fs::symlink_metadata(&path)
        .await
        .map_err(|e| invalid(e.to_string()))?;
    let canonical = tokio::fs::canonicalize(&path)
        .await
        .map_err(|e| invalid(e.to_string()))?;
    let root_canonical = tokio::fs::canonicalize(root)
        .await
        .map_err(|e| invalid(e.to_string()))?;
    if !canonical.starts_with(root_canonical) {
        return Err(invalid("export file escaped the game directory"));
    }
    if linked(&after)
        || size != before.len()
        || size != after.len()
        || before.modified().ok() != after.modified().ok()
    {
        return Err(invalid("file changed while calculating export hashes"));
    }
    let mut snapshot = original.clone();
    snapshot.identifiable = identifiable;
    snapshot.modified_at = after
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .and_then(|d| d.as_millis().try_into().ok());
    snapshot.integrity = ArtifactIntegrity {
        size: Some(size),
        hashes: vec![
            ContentHash {
                algorithm: HashAlgorithm::Sha1,
                value: sha1.digest().to_string(),
            },
            ContentHash {
                algorithm: HashAlgorithm::Sha256,
                value: format!("{:x}", sha256.finalize()),
            },
            ContentHash {
                algorithm: HashAlgorithm::Sha512,
                value: format!("{:x}", sha512.finalize()),
            },
        ],
    };
    progress(work_size, work_size);
    Ok((snapshot, fingerprint.finish()))
}

fn failure(source: SourceId, error: ResourceError) -> ExportSourceResolution {
    ExportSourceResolution::Failed {
        source,
        code: ResourceIssueCode::SourceUnavailable,
        message: error.to_string(),
    }
}

pub async fn resolve_source(
    service: &Arc<ResourceService>,
    source: SourceId,
    locals: &[(ExportFileSnapshot, u32)],
    known: &HashMap<String, Vec<SourceArtifactRef>>,
    draft: &mut ExportDraft,
    cancel: &CancellationToken,
) -> ResourceResult<Vec<ExportSourceResolution>> {
    let detailed = matches!(source.as_str(), "modrinth" | "curseforge");
    let supports_hash = service
        .sources()
        .iter()
        .find(|s| s.id == source)
        .is_some_and(|s| {
            s.capabilities.contains(&SourceCapability::IdentifyHash)
                || s.capabilities
                    .contains(&SourceCapability::IdentifyFingerprint)
        });
    let responses = if supports_hash {
        service
            .query_many(
                &source,
                locals
                    .iter()
                    .map(|(file, fingerprint)| {
                        if detailed {
                            ProviderQuery::IdentifyExport {
                                integrity: file.integrity.clone(),
                                fingerprint: Some(*fingerprint),
                            }
                        } else {
                            ProviderQuery::Identify {
                                integrity: file.integrity.clone(),
                                fingerprint: Some(*fingerprint),
                            }
                        }
                    })
                    .collect(),
                CachePolicy::ForceRefresh,
                cancel,
            )
            .await
    } else {
        vec![
            Ok(QueryOutcome {
                value: ProviderValue::NotFound,
                from_cache: false
            });
            locals.len()
        ]
    };
    canceled(cancel)?;
    let mut candidates = vec![vec![]; locals.len()];
    let mut errors: Vec<Option<ResourceError>> = vec![None; locals.len()];
    let mut refresh_refs = HashSet::new();
    for (index, response) in responses.into_iter().enumerate() {
        match response {
            Ok(QueryOutcome {
                value: ProviderValue::Identified(documents),
                ..
            }) => {
                for document in documents {
                    candidates[index]
                        .extend(document.release.artifacts.clone());
                    draft
                        .releases
                        .insert(document.release.reference.clone(), document);
                }
            }
            Ok(QueryOutcome {
                value: ProviderValue::Matches(artifacts),
                ..
            }) => {
                refresh_refs.extend(
                    artifacts.iter().map(|a| a.reference.release.clone()),
                );
                candidates[index] = artifacts;
            }
            Ok(QueryOutcome {
                value: ProviderValue::NotFound,
                ..
            }) => {}
            Ok(_) => {
                errors[index] = Some(ResourceError::InvalidResponse(
                    "unexpected export identification response".into(),
                ))
            }
            Err(error) => errors[index] = Some(error),
        }
        for reference in known
            .get(&locals[index].0.relative_path)
            .into_iter()
            .flatten()
            .filter(|reference| reference.release.project.source == source)
        {
            if !candidates[index]
                .iter()
                .any(|artifact| artifact.reference.release == reference.release)
            {
                refresh_refs.insert(reference.release.clone());
            }
        }
    }
    let refs = refresh_refs.into_iter().collect::<Vec<_>>();
    let releases = service
        .query_many(
            &source,
            refs.iter()
                .map(|r| ProviderQuery::Release {
                    project_id: Some(r.project.project_id.clone()),
                    release_id: r.release_id.clone(),
                    slug: None,
                })
                .collect(),
            CachePolicy::ForceRefresh,
            cancel,
        )
        .await;
    let mut release_errors = HashMap::new();
    for (reference, response) in refs.into_iter().zip(releases) {
        match response {
            Ok(QueryOutcome {
                value: ProviderValue::Release(document),
                ..
            }) => {
                draft.releases.insert(reference, document);
            }
            Ok(QueryOutcome {
                value: ProviderValue::NotFound,
                ..
            }) => {
                draft.releases.remove(&reference);
            }
            Ok(_) => {
                release_errors.insert(
                    reference,
                    ResourceError::InvalidResponse(
                        "unexpected export release response".into(),
                    ),
                );
            }
            Err(error) => {
                release_errors.insert(reference, error);
            }
        }
    }
    for (index, (file, _)) in locals.iter().enumerate() {
        for artifact in &candidates[index] {
            if let Some(error) = release_errors.get(&artifact.reference.release)
            {
                errors[index] = Some(error.clone());
            }
        }
        if !detailed {
            candidates[index] = candidates[index]
                .iter()
                .filter_map(|artifact| {
                    draft
                        .releases
                        .get(&artifact.reference.release)
                        .and_then(|document| {
                            document
                                .release
                                .artifacts
                                .iter()
                                .find(|a| a.reference == artifact.reference)
                        })
                        .cloned()
                })
                .collect();
        }
        for reference in known
            .get(&file.relative_path)
            .into_iter()
            .flatten()
            .filter(|r| r.release.project.source == source)
        {
            if let Some(error) = release_errors.get(&reference.release) {
                errors[index] = Some(error.clone());
            }
            if let Some(document) = draft.releases.get(&reference.release) {
                candidates[index].extend(document.release.artifacts.clone());
            }
        }
        for artifact in &candidates[index] {
            if let Some(error) = release_errors.get(&artifact.reference.release)
            {
                errors[index] = Some(error.clone());
            }
        }
        candidates[index].retain(|a| {
            (a.role == ArtifactRole::Primary
                || (!detailed && a.role == ArtifactRole::Unknown))
                && exact_integrity(&file.integrity, &a.integrity)
        });
        candidates[index].sort_by_key(|a| {
            serde_json::to_string(&a.reference).unwrap_or_default()
        });
        candidates[index].dedup_by(|a, b| a.reference == b.reference);
        let projects = candidates[index]
            .iter()
            .map(|a| &a.reference.release.project)
            .collect::<HashSet<_>>();
        if projects.len() > 1 {
            candidates[index].clear();
        }
    }
    let projects = candidates
        .iter()
        .flatten()
        .map(|a| a.reference.release.project.clone())
        .filter(|p| !draft.projects.contains_key(p))
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let responses = service
        .query_many(
            &source,
            projects
                .iter()
                .map(|p| ProviderQuery::Project {
                    id: p.project_id.clone(),
                    slug: None,
                })
                .collect(),
            CachePolicy::ForceRefresh,
            cancel,
        )
        .await;
    let mut project_errors = HashMap::new();
    for (project, response) in projects.into_iter().zip(responses) {
        match response {
            Ok(QueryOutcome {
                value: ProviderValue::Project(document),
                ..
            }) => {
                draft.projects.insert(project, document);
            }
            Ok(_) => {
                project_errors.insert(
                    project,
                    ResourceError::InvalidResponse(
                        "matching export project unavailable".into(),
                    ),
                );
            }
            Err(error) => {
                project_errors.insert(project, error);
            }
        }
    }
    canceled(cancel)?;
    Ok(candidates
        .into_iter()
        .enumerate()
        .map(|(index, artifacts)| {
            let error = errors[index].clone().or_else(|| {
                artifacts.iter().find_map(|a| {
                    project_errors.get(&a.reference.release.project).cloned()
                })
            });
            if let Some(error) = error {
                failure(source.clone(), error)
            } else if artifacts.is_empty() {
                ExportSourceResolution::NoMatch {
                    source: source.clone(),
                }
            } else {
                ExportSourceResolution::Matched {
                    source: source.clone(),
                    artifacts,
                }
            }
        })
        .collect())
}

struct ProgressEmitter<'a> {
    callback: &'a (dyn Fn(ExportRefreshProgress) + Send + Sync),
    last: std::sync::Mutex<Option<(Instant, String)>>,
}

impl ProgressEmitter<'_> {
    fn emit(&self, value: ExportRefreshProgress) {
        let mut last = self.last.lock().unwrap();
        if last.as_ref().is_some_and(|(time, phase)| {
            phase == &value.phase
                && time.elapsed() < std::time::Duration::from_millis(250)
        }) {
            return;
        }
        *last = Some((Instant::now(), value.phase.clone()));
        drop(last);
        (self.callback)(value);
    }

    fn finish(&self, value: ExportRefreshProgress) {
        (self.callback)(value);
    }
}

pub async fn refresh_local(
    service: &Arc<ResourceService>,
    draft: &mut ExportDraft,
    root: &Path,
    selected: Vec<String>,
    revision: u32,
    request: RequestId,
    known: HashMap<String, Vec<SourceArtifactRef>>,
    cancel: &CancellationToken,
    progress: &(dyn Fn(ExportRefreshProgress) + Send + Sync),
) -> ResourceResult<ExportResolutionSession> {
    let progress = ProgressEmitter {
        callback: progress,
        last: std::sync::Mutex::new(None),
    };
    if revision <= draft.selection_revision {
        return Err(invalid("stale export selection revision"));
    }
    let allowed = draft
        .scan
        .files
        .iter()
        .map(|file| (file.relative_path.clone(), file.clone()))
        .collect::<HashMap<_, _>>();
    if selected.iter().any(|path| !allowed.contains_key(path)) {
        return Err(invalid(
            "selected file is not part of the export snapshot",
        ));
    }
    let mut unique = HashSet::new();
    if selected.iter().any(|p| !unique.insert(p)) {
        return Err(invalid("duplicate export file"));
    }
    draft.selected = selected;
    draft.selection_revision = revision;
    draft.touched = Instant::now();
    let mut locals = vec![];
    let source_sets: HashMap<_, _> = draft
        .selected
        .iter()
        .map(|path| {
            let mut sources =
                HashSet::from([SourceId::modrinth(), SourceId::curseforge()]);
            sources.extend(
                known
                    .get(path)
                    .into_iter()
                    .flatten()
                    .map(|reference| reference.release.project.source.clone()),
            );
            (path.clone(), sources)
        })
        .collect();
    let overall_total = draft.selected.len()
        + source_sets.values().map(HashSet::len).sum::<usize>();
    let total_local_weight: f64 = draft
        .selected
        .iter()
        .map(|path| allowed[path].integrity.size.unwrap_or(0).max(1) as f64)
        .sum();
    let mut processed_local_weight = 0.0;
    let mut completed_sources = 0;

    for (index, path) in draft.selected.clone().iter().enumerate() {
        let mut original = allowed[path].clone();
        original.identifiable |=
            known.get(path).is_some_and(|refs| !refs.is_empty());
        let file_weight = original.integrity.size.unwrap_or(0).max(1) as f64;
        let on_file_progress = |current: u64, total: u64| {
            let fraction =
                (current as f64 / total.max(1) as f64).clamp(0.0, 1.0);
            let local_completed = (processed_local_weight
                + fraction * file_weight)
                / total_local_weight.max(1.0)
                * draft.selected.len() as f64;
            let cumulative = completed_sources as f64 + local_completed;
            progress.emit(ExportRefreshProgress {
                session_id: draft.scan.session_id.clone(),
                request_id: request.clone(),
                selection_revision: revision,
                completed: index,
                total: draft.selected.len(),
                phase: "hashing".into(),
                overall_completed: cumulative.floor() as usize,
                overall_total,
                current_file_fraction: cumulative.fract(),
            });
        };
        let previous = draft.results.get(path);
        let unchanged = if let Some(previous) =
            previous.filter(|previous| previous.local_error.is_none())
        {
            let metadata = tokio::fs::metadata(root.join(path)).await.ok();
            metadata.is_some_and(|metadata| {
                metadata.is_file()
                    && Some(metadata.len()) == previous.snapshot.integrity.size
                    && metadata
                        .modified()
                        .ok()
                        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                        .map(|time| time.as_millis() as u64)
                        == previous.snapshot.modified_at
            })
        } else {
            false
        };
        let frozen = if draft.reuse_frozen
            && unchanged
            && (!previous.unwrap().snapshot.identifiable
                || draft.fingerprints.contains_key(path))
        {
            Ok((
                previous.unwrap().snapshot.clone(),
                *draft.fingerprints.get(path).unwrap_or(&0),
            ))
        } else {
            freeze_with_progress(root, &original, cancel, &on_file_progress)
                .await
        };
        match frozen {
            Ok((mut snapshot, fingerprint)) => {
                draft.fingerprints.insert(path.clone(), fingerprint);
                let previous = draft.results.get(path);
                let same = previous.is_some_and(|p| {
                    p.local_error.is_none()
                        && p.snapshot.integrity == snapshot.integrity
                });
                snapshot.revision = if same {
                    previous.unwrap().snapshot.revision
                } else {
                    previous
                        .map_or(1, |p| p.snapshot.revision.saturating_add(1))
                };
                let resolutions = if same {
                    previous.unwrap().resolutions.clone()
                } else {
                    vec![]
                };
                draft.results.insert(
                    path.clone(),
                    ExportResolvedFile {
                        snapshot: snapshot.clone(),
                        resolutions,
                        local_error: None,
                    },
                );
                if snapshot.identifiable {
                    locals.push((snapshot, fingerprint));
                }
            }
            Err(ResourceError::Canceled) => {
                return Err(ResourceError::Canceled);
            }
            Err(error) => {
                draft.results.insert(
                    path.clone(),
                    ExportResolvedFile {
                        snapshot: original,
                        resolutions: vec![],
                        local_error: Some(error.to_string()),
                    },
                );
            }
        }
        let file = &draft.results[path];
        let sources = &source_sets[path];
        completed_sources += if file.local_error.is_some()
            || !file.snapshot.identifiable
        {
            sources.len()
        } else {
            sources.iter().filter(|source| file.resolutions.iter().any(|resolution| match resolution {
                ExportSourceResolution::Matched { source: current, .. } | ExportSourceResolution::NoMatch { source: current } => current == *source,
                ExportSourceResolution::Failed { source: current, .. } => current == *source && draft.accepted_failures.iter().any(|choice| choice.source == *current && choice.snapshot == file.snapshot),
            })).count()
        };
        processed_local_weight += file_weight;
        let cumulative = completed_sources as f64
            + processed_local_weight / total_local_weight.max(1.0)
                * draft.selected.len() as f64;
        progress.emit(ExportRefreshProgress {
            session_id: draft.scan.session_id.clone(),
            request_id: request.clone(),
            selection_revision: revision,
            completed: index + 1,
            total: draft.selected.len(),
            phase: "hashing".into(),
            overall_completed: cumulative.floor() as usize,
            overall_total,
            current_file_fraction: cumulative.fract(),
        });
    }
    let mut sources = vec![SourceId::modrinth(), SourceId::curseforge()];
    for file in &locals {
        for reference in known.get(&file.0.relative_path).into_iter().flatten()
        {
            if !sources.contains(&reference.release.project.source) {
                sources.push(reference.release.project.source.clone());
            }
        }
    }
    for source in sources {
        let pending = locals
            .iter()
            .filter(|(file, _)| {
                let applicable =
                    matches!(source.as_str(), "modrinth" | "curseforge")
                        || known.get(&file.relative_path).is_some_and(|refs| {
                            refs.iter()
                                .any(|r| r.release.project.source == source)
                        });
                applicable
                    && !draft.accepted_failures.iter().any(|choice| {
                        choice.source == source
                            && choice.snapshot
                                == draft.results[&file.relative_path].snapshot
                    })
                    && !draft.results[&file.relative_path]
                        .resolutions
                        .iter()
                        .any(|r| match r {
                            ExportSourceResolution::Matched {
                                source: s,
                                ..
                            }
                            | ExportSourceResolution::NoMatch { source: s } => {
                                s == &source
                            }
                            _ => false,
                        })
            })
            .cloned()
            .collect::<Vec<_>>();
        let mut source_completed = locals.len() - pending.len();
        for batch in pending.chunks(100) {
            progress.emit(ExportRefreshProgress {
                session_id: draft.scan.session_id.clone(),
                request_id: request.clone(),
                selection_revision: revision,
                completed: source_completed,
                total: locals.len(),
                phase: source.as_str().into(),
                overall_completed: draft.selected.len() + completed_sources,
                overall_total,
                current_file_fraction: 0.0,
            });
            let resolutions = resolve_source(
                service,
                source.clone(),
                batch,
                &known,
                draft,
                cancel,
            )
            .await?;
            for ((file, _), resolution) in batch.iter().zip(resolutions) {
                let result =
                    draft.results.get_mut(&file.relative_path).unwrap();
                result.resolutions.retain(|r| match r {
                    ExportSourceResolution::Matched { source: s, .. }
                    | ExportSourceResolution::NoMatch { source: s }
                    | ExportSourceResolution::Failed { source: s, .. } => {
                        s != &source
                    }
                });
                result.resolutions.push(resolution);
            }
            source_completed += batch.len();
            completed_sources += batch.len();
            progress.emit(ExportRefreshProgress {
                session_id: draft.scan.session_id.clone(),
                request_id: request.clone(),
                selection_revision: revision,
                completed: source_completed,
                total: locals.len(),
                phase: source.as_str().into(),
                overall_completed: draft.selected.len() + completed_sources,
                overall_total,
                current_file_fraction: 0.0,
            });
        }
    }
    canceled(cancel)?;
    progress.finish(ExportRefreshProgress {
        session_id: draft.scan.session_id.clone(),
        request_id: request.clone(),
        selection_revision: revision,
        completed: draft.selected.len(),
        total: draft.selected.len(),
        phase: "complete".into(),
        overall_completed: draft.selected.len() + completed_sources,
        overall_total,
        current_file_fraction: 0.0,
    });
    Ok(draft.response(request))
}

pub async fn refresh(
    service: &Arc<ResourceService>,
    instance_id: String,
    session: SessionId,
    selected: Vec<String>,
    revision: u32,
    request: RequestId,
    cancel: &CancellationToken,
    progress: &(dyn Fn(ExportRefreshProgress) + Send + Sync),
) -> ResourceResult<ExportResolutionSession> {
    let state = crate::State::get().await?;
    let draft = service.export_draft(&session)?;
    let mut draft = tokio::select! { biased; _ = cancel.cancelled() => return Err(ResourceError::Canceled), lock = draft.lock() => lock };
    if draft.scan.instance_id != instance_id {
        return Err(invalid("export session instance mismatch"));
    }
    let root = crate::api::instance::get_full_path(&instance_id).await?;
    let known = known_references(&state.pool, &instance_id).await?;
    refresh_local(
        service, &mut draft, &root, selected, revision, request, known, cancel,
        progress,
    )
    .await
}

pub(crate) async fn known_references(
    pool: &sqlx::SqlitePool,
    instance_id: &str,
) -> ResourceResult<HashMap<String, Vec<SourceArtifactRef>>> {
    let mut known: HashMap<String, Vec<SourceArtifactRef>> = HashMap::new();
    use sqlx::Row;
    let rows = sqlx::query("SELECT file.relative_path, link.source, link.project_id, link.release_id, link.artifact_id FROM resource_entry_links link JOIN instance_content_entries entry ON entry.id = link.entry_id JOIN instance_files file ON file.id = entry.file_id JOIN instances instance ON instance.id = entry.instance_id AND instance.applied_content_set_id = entry.content_set_id WHERE entry.instance_id = ? UNION ALL SELECT file.relative_path, ref.provider AS source, ref.provider_project_id AS project_id, COALESCE(ref.provider_release_id, ref.provider_file_id) AS release_id, ref.provider_file_id AS artifact_id FROM instance_content_provider_refs ref JOIN instance_content_entries entry ON entry.id = ref.content_entry_id JOIN instance_files file ON file.id = entry.file_id JOIN instances instance ON instance.id = entry.instance_id AND instance.applied_content_set_id = entry.content_set_id WHERE entry.instance_id = ?").bind(instance_id).bind(instance_id).fetch_all(pool).await.map_err(crate::resources::plans::db)?;
    for row in rows {
        let path: String = row.get("relative_path");
        let reference = (|| {
            let project = SourceProjectRef {
                source: SourceId::new(row.get::<String, _>("source")).ok()?,
                project_id: SourceProjectId::new(
                    row.get::<String, _>("project_id"),
                )
                .ok()?,
            };
            crate::state::instances::adapters::sqlite::resource_rows::validate_project(&project).ok()?;
            Some(SourceArtifactRef {
                release: SourceReleaseRef {
                    project,
                    release_id: SourceReleaseId::new(
                        row.get::<Option<String>, _>("release_id")?,
                    )
                    .ok()?,
                },
                artifact_id: SourceArtifactId::new(
                    row.get::<Option<String>, _>("artifact_id").unwrap_or_else(
                        || {
                            path.rsplit('/')
                                .next()
                                .unwrap_or("file")
                                .to_string()
                        },
                    ),
                )
                .ok()?,
            })
        })();
        if let Some(reference) = reference {
            known.entry(path).or_default().push(reference);
        }
    }
    Ok(known)
}

#[cfg(test)]
mod progress_tests {
    use super::*;

    #[test]
    fn fast_file_updates_are_coalesced_but_phase_changes_and_completion_are_delivered()
     {
        let calls = std::sync::Mutex::new(Vec::new());
        let callback = |value| {
            calls.lock().unwrap().push(value);
        };
        let emitter = ProgressEmitter {
            callback: &callback,
            last: std::sync::Mutex::new(None),
        };
        let mut value = ExportRefreshProgress {
            session_id: SessionId::new("session").unwrap(),
            request_id: RequestId::new("request").unwrap(),
            selection_revision: 1,
            completed: 0,
            total: 10000,
            phase: "hashing".into(),
            overall_completed: 0,
            overall_total: 30000,
            current_file_fraction: 0.0,
        };
        emitter.emit(value.clone());
        for index in 1..10000 {
            value.completed = index;
            value.overall_completed = index;
            emitter.emit(value.clone());
        }
        let hashing_events = calls.lock().unwrap().len();
        assert!(hashing_events < 100, "too many events: {hashing_events}");
        value.phase = "modrinth".into();
        emitter.emit(value.clone());
        assert_eq!(calls.lock().unwrap().len(), hashing_events + 1);
        value.phase = "complete".into();
        value.overall_completed = value.overall_total;
        emitter.finish(value);
        assert_eq!(
            calls.lock().unwrap().last().unwrap().overall_completed,
            30000
        );
    }
}

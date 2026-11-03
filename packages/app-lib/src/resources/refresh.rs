//! Instance-owned refresh work, independent of any window subscription.

mod entry;

const REMOTE_CACHE_SECONDS: i64 = 24 * 60 * 60;

use super::*;
use crate::state::store::{
    content_refresh::RefreshWrite, content_snapshot::SnapshotRecords,
};
use futures::{StreamExt, stream::FuturesUnordered};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::Arc,
};
use tokio::sync::{Mutex, broadcast, mpsc};
use tokio_util::sync::CancellationToken;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefreshMode {
    Normal,
    ForceRemote,
    RetryFailed,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshTask {
    pub id: String,
    pub mode: RefreshMode,
    pub running: bool,
    pub scanned: usize,
    pub checked: usize,
    pub failed: usize,
    pub unresolved: usize,
    pub errors: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshEvent {
    pub instance_id: String,
    pub task_id: String,
    pub sequence: u64,
    pub patch: RefreshPatch,
    pub task: RefreshTask,
}

#[derive(Clone, Debug, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum RefreshPatch {
    Pack {
        pack: Option<crate::state::InstanceContentPack>,
    },
    Content {
        snapshot: crate::state::InstanceContentSnapshot,
    },
    Entry {
        entry_id: String,
        file_id: String,
        sha1: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        artifact_ref: Option<SourceArtifactRef>,
        #[serde(skip_serializing_if = "Option::is_none")]
        presentation: Option<crate::state::ContentItem>,
        #[serde(skip_serializing_if = "Option::is_none")]
        update: Option<RefreshUpdate>,
    },
    Progress,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshUpdate {
    pub candidate: Option<SourceReleaseRef>,
    pub failed: bool,
    pub unresolved: bool,
    pub error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshState {
    pub snapshot: crate::state::InstanceContentSnapshot,
    pub task: Option<RefreshTask>,
    pub sequence: u64,
}

struct Slot {
    unsaved_entries: HashSet<String>,
    pack_failed: bool,
    background_failed: bool,
    outcomes: HashMap<
        String,
        crate::state::store::content_refresh_checks::CheckAttempt,
    >,
    task: RefreshTask,
    sequence: u64,
    pending_mode: Option<RefreshMode>,
    cancel: CancellationToken,
}

pub(crate) struct RefreshCoordinator {
    tasks: parking_lot::Mutex<Vec<tokio::task::JoinHandle<()>>>,
    slots: Mutex<HashMap<String, Slot>>,
    events: broadcast::Sender<RefreshEvent>,
    shutdown: CancellationToken,
}

impl Default for RefreshCoordinator {
    fn default() -> Self {
        Self {
            tasks: Default::default(),
            slots: Mutex::new(HashMap::new()),
            events: broadcast::channel(1024).0,
            shutdown: CancellationToken::new(),
        }
    }
}

impl RefreshCoordinator {
    pub(crate) async fn invalidate(&self, instance_id: &str) {
        if let Some(slot) = self.slots.lock().await.get_mut(instance_id) {
            slot.pending_mode = None;
            slot.cancel.cancel();
        }
    }
    pub(crate) fn subscribe(&self) -> broadcast::Receiver<RefreshEvent> {
        self.events.subscribe()
    }

    pub(crate) async fn shutdown(&self) {
        self.shutdown.cancel();
        let tasks = std::mem::take(&mut *self.tasks.lock());
        for mut task in tasks {
            if tokio::time::timeout(
                std::time::Duration::from_secs(3),
                &mut task,
            )
            .await
            .is_err()
            {
                task.abort();
            }
        }
    }

    pub(crate) async fn start(
        &self,
        state: Arc<crate::State>,
        instance_id: String,
        mode: RefreshMode,
    ) -> ResourceResult<RefreshTask> {
        if self.shutdown.is_cancelled() {
            return Err(ResourceError::Canceled);
        }
        let mut slots = self.slots.lock().await;
        if let Some(slot) = slots.get_mut(&instance_id)
            && slot.task.running
            && !slot.cancel.is_cancelled()
        {
            if ((mode == RefreshMode::ForceRemote
                && slot.task.mode != RefreshMode::ForceRemote)
                || (mode == RefreshMode::RetryFailed
                    && slot.task.mode == RefreshMode::Normal))
                && slot.pending_mode != Some(RefreshMode::ForceRemote)
            {
                slot.pending_mode = Some(mode);
            }
            return Ok(slot.task.clone());
        }
        let sequence = slots.get(&instance_id).map_or(0, |slot| slot.sequence);
        let background_failed = slots
            .get(&instance_id)
            .is_some_and(|slot| slot.background_failed);
        let unsaved_entries = slots
            .get(&instance_id)
            .map(|slot| slot.unsaved_entries.clone())
            .unwrap_or_default();
        let pack_failed =
            slots.get(&instance_id).is_some_and(|slot| slot.pack_failed);
        let task = RefreshTask {
            id: uuid::Uuid::new_v4().to_string(),
            mode,
            running: true,
            scanned: 0,
            checked: 0,
            failed: 0,
            unresolved: 0,
            errors: vec![],
        };
        let cancel = self.shutdown.child_token();
        slots.insert(
            instance_id.clone(),
            Slot {
                unsaved_entries,
                pack_failed,
                background_failed,
                outcomes: HashMap::new(),
                task: task.clone(),
                sequence,
                pending_mode: None,
                cancel: cancel.clone(),
            },
        );
        drop(slots);
        let id = task.id.clone();
        let handle = tokio::spawn(async move {
            let coordinator = &state.content_refresh;
            let mut current_mode = mode;
            loop {
                let result =
                    run_round(&state, &instance_id, &id, current_mode, &cancel)
                        .await;
                let mut slots = coordinator.slots.lock().await;
                let Some(slot) = slots
                    .get_mut(&instance_id)
                    .filter(|slot| slot.task.id == id)
                else {
                    return;
                };
                if let Err(error) = result {
                    slot.task.errors.push(error.to_string());
                    slot.task.failed += 1;
                }
                if let Some(next_mode) =
                    slot.pending_mode.take().filter(|_| !cancel.is_cancelled())
                {
                    slot.task.mode = next_mode;
                    slot.task.scanned = 0;
                    slot.task.checked = 0;
                    slot.task.failed = 0;
                    slot.task.unresolved = 0;
                    slot.task.errors.clear();
                    slot.outcomes.clear();
                    current_mode = next_mode;
                    continue;
                }
                slot.task.running = false;
                coordinator.publish(&instance_id, slot, RefreshPatch::Progress);
                break;
            }
        });
        let mut tasks = self.tasks.lock();
        tasks.retain(|task| !task.is_finished());
        tasks.push(handle);
        Ok(task)
    }

    pub(crate) async fn state(
        &self,
        state: &crate::State,
        instance_id: &str,
    ) -> ResourceResult<RefreshState> {
        let slots = self.slots.lock().await;
        let snapshot =
            crate::state::instances::commands::get_local_content_snapshot(
                instance_id,
                state,
            )
            .await?;
        let slot = slots.get(instance_id);
        Ok(RefreshState {
            snapshot,
            task: slot.map(|slot| slot.task.clone()),
            sequence: slot.map_or(0, |slot| slot.sequence),
        })
    }

    fn publish(&self, instance_id: &str, slot: &mut Slot, patch: RefreshPatch) {
        slot.sequence += 1;
        let _ = self.events.send(RefreshEvent {
            instance_id: instance_id.into(),
            task_id: slot.task.id.clone(),
            sequence: slot.sequence,
            patch,
            task: slot.task.clone(),
        });
    }

    async fn save(
        &self,
        state: &crate::State,
        instance_id: &str,
        task_id: &str,
        writes: Vec<RefreshWrite>,
    ) -> ResourceResult<()> {
        let mut slots = self.slots.lock().await;
        let Some(slot) = slots
            .get_mut(instance_id)
            .filter(|slot| slot.task.id == task_id)
        else {
            return Ok(());
        };
        if slot.cancel.is_cancelled() {
            return Err(ResourceError::Canceled);
        }
        let mut current = Vec::new();
        let _lock = state.lock_instance_content_exclusive(instance_id).await;
        for write in writes {
            slot.unsaved_entries.insert(write.entry_id.clone());
            if entry::bytes_still_current(state, instance_id, &write).await? {
                current.push(write);
            } else {
                slot.unsaved_entries.remove(&write.entry_id);
            }
        }
        let entry_ids = current
            .iter()
            .map(|write| write.entry_id.clone())
            .collect::<HashSet<_>>();
        let applied = crate::state::store::content_refresh::set_results(
            &state.data,
            instance_id,
            current,
        )
        .await;
        let applied = match applied {
            Ok(applied) => {
                slot.unsaved_entries.retain(|id| !entry_ids.contains(id));
                applied
            }
            Err(error) => {
                slot.unsaved_entries.extend(entry_ids);
                return Err(error.into());
            }
        };
        for write in applied {
            tracing::debug!(instance_id, entry_id = %write.entry_id, update = write.check.is_some(), presentation = write.presentation.is_some(), "Saved content refresh result");
            let update = write.check.as_ref().map(|check| {
                use crate::state::store::content_refresh_checks::CheckAttempt;
                if let Some(error) = &check.error {
                    tracing::warn!(instance_id, entry_id = %write.entry_id, error, "Content refresh entry failed");
                }
                if let Some(previous) =
                    slot.outcomes.insert(write.entry_id.clone(), check.attempt)
                {
                    slot.task.failed -=
                        usize::from(previous == CheckAttempt::Failed);
                    slot.task.unresolved -=
                        usize::from(previous == CheckAttempt::Unrecognized);
                } else {
                    slot.task.checked += 1;
                }
                slot.task.failed +=
                    usize::from(check.attempt == CheckAttempt::Failed);
                slot.task.unresolved +=
                    usize::from(check.attempt == CheckAttempt::Unrecognized);
                RefreshUpdate {
                    candidate: check
                        .last_success
                        .as_ref()
                        .and_then(|success| success.candidate.clone()),
                    failed: check.attempt == CheckAttempt::Failed,
                    unresolved: check.attempt == CheckAttempt::Unrecognized,
                    error: check.error.clone(),
                }
            });
            self.publish(
                instance_id,
                slot,
                RefreshPatch::Entry {
                    entry_id: write.entry_id,
                    file_id: write.file_id,
                    sha1: write.context.sha1,
                    artifact_ref: write
                        .artifact
                        .map(|record| record.artifact.reference),
                    presentation: write
                        .presentation
                        .map(|record| record.content),
                    update,
                },
            );
        }
        Ok(())
    }
}

async fn run_round(
    state: &Arc<crate::State>,
    instance_id: &str,
    task_id: &str,
    mode: RefreshMode,
    cancel: &CancellationToken,
) -> ResourceResult<()> {
    use crate::state::store::content_refresh_scan_failures::ContentRefreshScanFailures;
    let retry_scan_paths = if mode == RefreshMode::RetryFailed {
        ContentRefreshScanFailures::failed(&state.derived, instance_id)
            .await?
            .into_iter()
            .map(|(key, _)| key.relative_path)
            .collect::<HashSet<_>>()
    } else {
        HashSet::new()
    };
    let started = std::time::Instant::now();
    let policy = if mode == RefreshMode::Normal {
        CachePolicy::UseFresh
    } else {
        CachePolicy::RefreshSince(chrono::Utc::now().timestamp_micros())
    };
    let unsaved_entries = state
        .content_refresh
        .slots
        .lock()
        .await
        .get(instance_id)
        .map(|slot| slot.unsaved_entries.clone())
        .unwrap_or_default();
    let service = state.resources().await?.clone();
    let initial = crate::state::store::content_snapshot::get_with_saved_pack(
        &state.data,
        &state.derived,
        instance_id,
    )
    .await?;
    let location = crate::state::store::content_refresh_checks::location_key(
        &initial.instance,
    );
    let link = serde_json::to_string(&initial.link)?;
    let pack_due =
        !matches!(initial.link, crate::state::InstanceLink::Unmanaged)
            && pack_needs_refresh(
                initial.pack_presentation.as_ref().filter(|saved| {
                    saved.location_key == location && saved.link_key == link
                }),
                mode,
                chrono::Utc::now().timestamp(),
            );
    let (scan_tx, mut scans) = mpsc::channel(8);
    let scan_state = state.clone();
    let scan_instance = instance_id.to_owned();
    let scan_cancel = cancel.child_token();
    let _cancel_on_exit = scan_cancel.clone().drop_guard();
    let worker_cancel = scan_cancel.clone();
    let mut scan =
        tokio_util::task::AbortOnDropHandle::new(tokio::spawn(async move {
            crate::state::instances::commands::sync_content_files_progressively(
                &scan_instance,
                &scan_state,
                &scan_tx,
                &worker_cancel,
                mode,
            )
            .await
        }));
    let (writes_tx, mut writes_rx) = mpsc::channel::<RefreshWrite>(64);
    let mut workers = FuturesUnordered::new();
    {
        let state = state.clone();
        let instance_id = instance_id.to_owned();
        let task_id = task_id.to_owned();
        let cancel = scan_cancel.clone();
        workers.push(tokio_util::task::AbortOnDropHandle::new(tokio::spawn(
            async move {
                let retry_pack = state
                    .content_refresh
                    .slots
                    .lock()
                    .await
                    .get(&instance_id)
                    .is_some_and(|slot| slot.pack_failed);
                let pack_mode =
                    if mode == RefreshMode::RetryFailed && retry_pack {
                        RefreshMode::ForceRemote
                    } else {
                        mode
                    };
                let result = refresh_pack(
                    &state,
                    &instance_id,
                    &task_id,
                    pack_mode,
                    &cancel,
                )
                .await;
                if let Some(slot) = state
                    .content_refresh
                    .slots
                    .lock()
                    .await
                    .get_mut(&instance_id)
                    .filter(|slot| {
                        slot.task.id == task_id && !slot.cancel.is_cancelled()
                    })
                {
                    slot.pack_failed = result.is_err();
                }
                result
            },
        )));
    }
    let mut pending = VecDeque::new();
    let mut scheduled = HashSet::new();
    let mut buffered = Vec::new();
    let mut interval =
        tokio::time::interval(std::time::Duration::from_millis(50));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut scan_done = false;
    let mut scans_closed = false;
    let mut failed_paths = HashSet::new();
    loop {
        // Keep enough independent rows ready to fill source batches while the
        // provider semaphore still bounds actual network concurrency to three.
        while workers.len() < 32 {
            let Some(item) = pending.pop_front() else {
                break;
            };
            let worker_state = state.clone();
            let worker_service = service.clone();
            let worker_cancel = scan_cancel.clone();
            let worker_tx = writes_tx.clone();
            workers.push(tokio_util::task::AbortOnDropHandle::new(
                tokio::spawn(async move {
                    entry::run(
                        &worker_state,
                        worker_service,
                        item,
                        policy,
                        &worker_cancel,
                        worker_tx,
                    )
                    .await
                }),
            ));
        }
        tokio::select! {
            biased;
            _ = cancel.cancelled() => return Err(ResourceError::Canceled),
            _ = interval.tick() => {
                if !buffered.is_empty() { state.content_refresh.save(state, instance_id, task_id, std::mem::take(&mut buffered)).await?; }
            }
            Some(write) = writes_rx.recv() => {
                buffered.push(write);
                if buffered.len() >= 32 { state.content_refresh.save(state, instance_id, task_id, std::mem::take(&mut buffered)).await?; }
            }
            Some(result) = workers.next(), if !workers.is_empty() => {
                if let Err(error) = result.unwrap_or_else(|error| Err(ResourceError::InvalidRequest(error.to_string()))) {
                    let mut slots = state.content_refresh.slots.lock().await;
                    if let Some(slot) = slots.get_mut(instance_id).filter(|slot| slot.task.id == task_id) {
                        slot.task.failed += 1;
                        slot.task.errors.push(error.to_string());
                        state.content_refresh.publish(instance_id, slot, RefreshPatch::Progress);
                    }
                }
            }
            progress = scans.recv(), if !scans_closed => {
                match progress {
                    Some(crate::state::instances::commands::ScanProgress::Files(files)) => {
                        let mut slots = state.content_refresh.slots.lock().await;
                        let records = crate::state::store::content_snapshot::get_with_saved_pack(&state.data, &state.derived, instance_id).await?;
                        let eligible = files.iter().filter(|file| !failed_paths.contains(&file.relative_path)).collect::<Vec<_>>();
                        let unsaved_file_ids = records.entries.iter().filter(|entry| unsaved_entries.contains(&entry.id))
                            .filter_map(|entry| entry.file_id.as_deref()).collect::<HashSet<_>>();
                        let mut work = entry::work(&records, eligible.iter().filter(|file| !retry_scan_paths.contains(&file.relative_path) && !unsaved_file_ids.contains(file.id.as_str())).map(|file| file.id.as_str()), mode);
                        work.extend(entry::work(&records, eligible.iter().filter(|file| retry_scan_paths.contains(&file.relative_path)).map(|file| file.id.as_str()), RefreshMode::Normal));
                        work.extend(entry::work(&records, eligible.iter().filter(|file| unsaved_file_ids.contains(file.id.as_str())).map(|file| file.id.as_str()), RefreshMode::ForceRemote));
                        for item in work {
                            if scheduled.insert((item.entry.entry_id.clone(), item.context.sha1.clone())) {
                                pending.push_back(item);
                            }
                        }
                        let snapshot = crate::state::instances::commands::content_snapshot::local::project(records);
                        if let Some(slot) = slots.get_mut(instance_id).filter(|slot| slot.task.id == task_id) {
                            slot.task.scanned = snapshot.items.len();
                            state.content_refresh.publish(instance_id, slot, RefreshPatch::Content { snapshot });
                        }
                    }
                    Some(crate::state::instances::commands::ScanProgress::Failed { path, error }) => {
                        if !failed_paths.insert(path.clone()) { continue; }
                        let mut slots = state.content_refresh.slots.lock().await;
                        if let Some(slot) = slots.get_mut(instance_id) { slot.task.failed += 1; slot.task.errors.push(format!("{path}: {error}")); }
                    }
                    None => { scans_closed = true; }
                }
            }
            result = &mut scan, if !scan_done => {
                let outcome = result.map_err(|error| ResourceError::InvalidRequest(error.to_string()))??; scan_done = true;
                let retry_background = state.content_refresh.slots.lock().await
                    .get(instance_id).is_some_and(|slot| slot.background_failed);
                if outcome.stable && (mode != RefreshMode::RetryFailed || retry_background)
                    && (mode != RefreshMode::Normal || !scheduled.is_empty() || pack_due)
                {
                    let state = state.clone(); let instance_id = instance_id.to_owned(); let task_id = task_id.to_owned(); let cancel = scan_cancel.clone();
                    workers.push(tokio_util::task::AbortOnDropHandle::new(tokio::spawn(async move {
                        let result = crate::state::instances::commands::content_snapshot::refresh_background_metadata(&instance_id, &state, &cancel).await;
                        let mut slots = state.content_refresh.slots.lock().await;
                        if let Some(slot) = slots.get_mut(&instance_id).filter(|slot| slot.task.id == task_id && !slot.cancel.is_cancelled()) {
                            slot.background_failed = result.is_err();
                            result?;
                            let snapshot = crate::state::instances::commands::get_local_content_snapshot(&instance_id, &state).await?;
                            state.content_refresh.publish(&instance_id, slot, RefreshPatch::Content { snapshot });
                        }
                        Ok(())
                    })));
                }
            }
        }
        if scan_done
            && scans.is_empty()
            && workers.is_empty()
            && pending.is_empty()
            && writes_rx.is_empty()
        {
            if !buffered.is_empty() {
                state
                    .content_refresh
                    .save(
                        state,
                        instance_id,
                        task_id,
                        std::mem::take(&mut buffered),
                    )
                    .await?;
            }
            break;
        }
    }
    tracing::debug!(
        instance_id,
        elapsed_ms = started.elapsed().as_millis() as u64,
        "Completed content refresh round"
    );
    Ok(())
}

pub async fn start(
    instance_id: String,
    mode: RefreshMode,
) -> ResourceResult<RefreshTask> {
    let state = crate::State::get().await?;
    state
        .content_refresh
        .start(state.clone(), instance_id, mode)
        .await
}

pub async fn get_state(instance_id: String) -> ResourceResult<RefreshState> {
    let state = crate::State::get().await?;
    state.content_refresh.state(&state, &instance_id).await
}

pub async fn subscribe() -> ResourceResult<broadcast::Receiver<RefreshEvent>> {
    Ok(crate::State::get().await?.content_refresh.subscribe())
}

async fn refresh_pack(
    state: &crate::State,
    instance_id: &str,
    task_id: &str,
    mode: RefreshMode,
    cancel: &CancellationToken,
) -> ResourceResult<()> {
    use crate::state::store::content_refresh_pack::*;
    let records = crate::state::store::content_snapshot::get_with_saved_pack(
        &state.data,
        &state.derived,
        instance_id,
    )
    .await?;
    if matches!(records.link, crate::state::InstanceLink::Unmanaged) {
        return Ok(());
    }
    let location_key =
        crate::state::store::content_refresh_checks::location_key(
            &records.instance,
        );
    let link_key = serde_json::to_string(&records.link)?;
    let saved = records.pack_presentation.filter(|saved| {
        saved.location_key == location_key && saved.link_key == link_key
    });
    let now = chrono::Utc::now().timestamp();
    if !pack_needs_refresh(saved.as_ref(), mode, now) {
        return Ok(());
    }
    let metadata = tokio::select! { biased; _ = cancel.cancelled() => return Err(ResourceError::Canceled),
        result = crate::state::instances::commands::get_linked_modpack_info(instance_id, None, Some(if mode == RefreshMode::ForceRemote { crate::state::CacheBehaviour::Bypass } else { crate::state::CacheBehaviour::MustRevalidate }), state) => result
    };
    let result = metadata.map(|metadata| {
        let mut pack =
            crate::state::instances::commands::content_snapshot::pack_snapshot(
                &records.instance.name,
                records.instance.icon_path.clone(),
                &records.link,
                &records.members,
                metadata,
            );
        if let Some(pack) = &mut pack {
            pack.resource_id = records.pack_resource;
        }
        pack
    });
    let mut slots = state.content_refresh.slots.lock().await;
    let Some(slot) = slots
        .get_mut(instance_id)
        .filter(|slot| slot.task.id == task_id && !slot.cancel.is_cancelled())
    else {
        return Ok(());
    };
    let _lock = state.lock_instance_content_exclusive(instance_id).await;
    let current =
        crate::state::store::content_snapshot::get(&state.data, instance_id)
            .await?;
    if location_key
        != crate::state::store::content_refresh_checks::location_key(
            &current.instance,
        )
        || link_key != serde_json::to_string(&current.link)?
    {
        return Ok(());
    }
    let record = pack_attempt(saved, location_key, link_key, now, &result);
    ContentRefreshPacks::upsert(
        &state.derived,
        &PackKey {
            instance_id: instance_id.into(),
            content_set_id: current.content_set.id,
        },
        &record,
    )
    .await?;
    let pack = result?;
    state.content_refresh.publish(
        instance_id,
        slot,
        RefreshPatch::Pack { pack },
    );
    Ok(())
}

fn pack_needs_refresh(
    saved: Option<&crate::state::store::content_refresh_pack::PackRecord>,
    mode: RefreshMode,
    now: i64,
) -> bool {
    match mode {
        RefreshMode::ForceRemote => true,
        RefreshMode::RetryFailed => {
            saved.is_some_and(|saved| saved.last_error.is_some())
        }
        RefreshMode::Normal => saved.is_none_or(|saved| {
            if saved.last_error.is_some() {
                now.saturating_sub(saved.attempted_at) >= REMOTE_CACHE_SECONDS
            } else {
                now.saturating_sub(saved.saved_at) >= REMOTE_CACHE_SECONDS
            }
        }),
    }
}

fn pack_attempt(
    saved: Option<crate::state::store::content_refresh_pack::PackRecord>,
    location_key: String,
    link_key: String,
    now: i64,
    result: &crate::Result<Option<crate::state::InstanceContentPack>>,
) -> crate::state::store::content_refresh_pack::PackRecord {
    use crate::state::store::content_refresh_pack::PackRecord;
    match result {
        Ok(pack) => PackRecord {
            location_key,
            link_key,
            saved_at: now,
            pack: pack.clone(),
            attempted_at: now,
            last_error: None,
        },
        Err(error) => PackRecord {
            location_key,
            link_key,
            saved_at: saved.as_ref().map_or(0, |saved| saved.saved_at),
            pack: saved.and_then(|saved| saved.pack),
            attempted_at: now,
            last_error: Some(error.to_string()),
        },
    }
}

#[cfg(test)]
#[path = "../state/store/tests/content_refresh_runtime.rs"]
mod tests;

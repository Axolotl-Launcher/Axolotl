use super::super::synced_options::{
    nbt_from_bytes, nbt_to_bytes, safe_instance_id, synced_options_path,
};
use super::SERVERS_FILE;
use super::limits::validate_size;
use super::types::{
    CanonicalServer, LocalServer, ProjectionEntry, ProjectionOwner,
    ServerRecord, ServerSource,
};
use crate::state::store::Atom;
use crate::state::store::Collection as _;
use crate::state::store::atom;
use crate::state::store::instance_sync_checkpoints::{
    CheckpointKey, CheckpointRecord as StoredCheckpointRecord,
    CheckpointStatus as StoredStatus, InstanceSyncCheckpoints,
};
use crate::state::store::synced_server_state::{
    Singleton as ServerStateSingleton, SyncedServerState,
};
use crate::state::store::synced_servers::{SyncedServerRecord, SyncedServers};
use crate::state::sync_state::servers::{
    LocalKey, LocalRecord, LocalServers, ProjectionKey, ProjectionRecord,
    Projections,
};
use crate::{ErrorKind, State};
use std::collections::HashSet;
use std::path::PathBuf;

pub(in crate::api::instance) async fn canonical_exists(
    state: &State,
) -> crate::Result<bool> {
    canonical_initialized(state).await
}

async fn canonical_initialized(state: &State) -> crate::Result<bool> {
    Ok(SyncedServerState::get(&state.data, &ServerStateSingleton)
        .await?
        .is_some())
}

pub(super) async fn read_canonical(
    state: &State,
) -> crate::Result<Vec<CanonicalServer>> {
    let mut unit = atom(&state.data).await?;
    let rows = SyncedServers::atom_get_all(&mut unit).await?;
    validate_size(
        rows.len(),
        rows.iter().map(|(_, record)| record.nbt.len()).sum(),
    )?;
    let transaction = unit.sync().await?;
    let Some(native) = transaction.head.native.get("servers") else {
        if rows.is_empty() {
            unit.commit().await?;
            return Ok(Vec::new());
        }
        return Err(ErrorKind::FSError(
            "Shared server native source missing".into(),
        )
        .into());
    };
    let bytes = crate::state::sync_state::manifest::read_native_ref(
        &transaction.root,
        native,
    )
    .await?;
    let bodies = super::codec::servers_from_bytes(bytes)?;
    if bodies.len() != rows.len() {
        return Err(ErrorKind::FSError(
            "Shared server identity/native count mismatch".into(),
        )
        .into());
    }
    let result = rows
        .into_iter()
        .zip(bodies)
        .map(|((id, record), data)| {
            if nbt_from_bytes(record.nbt)? != data {
                return Err(ErrorKind::FSError(
                    "Shared server native identity mismatch".into(),
                )
                .into());
            }
            Ok(CanonicalServer { id, data })
        })
        .collect::<crate::Result<Vec<_>>>()?;
    unit.commit().await?;
    Ok(result)
}

/// Reads canonical servers and instance overrides.
///
/// The two halves of the read reach different tables, and a store collection
/// cannot join the transaction the instance steps run in; the caller's
/// synced-options lock is what keeps them close, not a shared unit.
pub(super) async fn read_server_snapshot(
    instance_id: &str,
    state: &State,
) -> crate::Result<(Vec<CanonicalServer>, Vec<LocalServer>)> {
    let canonical = read_canonical(state).await?;
    let local = load_local(instance_id, state).await?;
    Ok((canonical, local))
}

pub(super) async fn commit_server_state(
    canonical: Option<&[CanonicalServer]>,
    local: Option<(&str, &[LocalServer])>,
    state: &State,
) -> crate::Result<bool> {
    write_server_state(canonical, local, state).await
}

/// A consumed file and its database changes must survive a failed fan-out together.
/// Pending checkpoints retry projection, never import the same change again.
pub(super) async fn commit_captured_servers(
    instance_id: &str,
    canonical: &[CanonicalServer],
    local: &[LocalServer],
    state: &State,
) -> crate::Result<bool> {
    let changed =
        write_canonical_rows(canonical, Some((instance_id, local)), state)
            .await?;
    Ok(changed)
}

/// Writes the canonical list first: the instance rows resolve their excluded
/// server against it, so it has to be there before they are written.
async fn write_server_state(
    canonical: Option<&[CanonicalServer]>,
    local: Option<(&str, &[LocalServer])>,
    state: &State,
) -> crate::Result<bool> {
    let mut canonical_changed = false;
    if let Some(canonical) = canonical {
        canonical_changed =
            write_canonical_rows(canonical, None, state).await?;
    }
    if let Some((instance_id, local)) = local {
        write_local(instance_id, local, state).await?;
    }
    Ok(canonical_changed)
}

/// Writes the canonical list, and — when `checkpoint_for` names an instance —
/// the pending checkpoint its projection waits on, in the same unit.
async fn write_canonical_rows(
    servers: &[CanonicalServer],
    checkpoint_for: Option<(&str, &[LocalServer])>,
    state: &State,
) -> crate::Result<bool> {
    validate_size(servers.len(), 0)?;
    let mut unit = atom(&state.data).await?;
    let stored_size = SyncedServers::stored_size_in(&mut unit).await?;
    validate_size(stored_size.0, stored_size.1)?;
    let current = SyncedServers::atom_get_all(&mut unit).await?;
    let mut desired = Vec::with_capacity(servers.len());
    let mut bytes = 0;
    for (position, server) in servers.iter().enumerate() {
        let nbt = nbt_to_bytes(&server.data)?;
        bytes += nbt.len();
        validate_size(servers.len(), bytes)?;
        desired.push((server.id.clone(), position as i64, nbt));
    }
    let mut unchanged = current.len() == desired.len();
    if unchanged {
        for (((id, record), (desired_id, position, _)), server) in
            current.iter().zip(&desired).zip(servers)
        {
            // Compound field order is not meaningful; quartz_nbt uses HashMap.
            // Comparing the decoded value marks unchanged lists as new revisions.
            if id != desired_id
                || record.position != *position
                || nbt_from_bytes(record.nbt.clone())? != server.data
            {
                unchanged = false;
                break;
            }
        }
    }
    if unchanged {
        SyncedServerState::ensure_in(&mut unit).await?;
        if let Some((instance_id, local)) = checkpoint_for {
            write_local_rows(&mut unit, instance_id, local).await?;
            write_pending_checkpoint(&mut unit, instance_id, "").await?;
        }
        unit.commit().await?;
        return Ok(false);
    }

    let desired_ids = desired
        .iter()
        .map(|(id, _, _)| id.as_str())
        .collect::<HashSet<_>>();
    for (id, _) in &current {
        if !desired_ids.contains(id.as_str()) {
            SyncedServers::atom_set(&mut unit, id, None).await?;
        }
    }

    let position_offset = current
        .iter()
        .map(|(_, record)| record.position)
        .max()
        .unwrap_or(0)
        .max(servers.len() as i64)
        .saturating_add(1);
    SyncedServers::shift_positions_in(&mut unit, position_offset).await?;

    for (id, position, nbt) in desired {
        SyncedServers::atom_set(
            &mut unit,
            &id,
            Some(&SyncedServerRecord { position, nbt }),
        )
        .await?;
    }
    SyncedServerState::bump_in(&mut unit).await?;
    if let Some((instance_id, local)) = checkpoint_for {
        write_local_rows(&mut unit, instance_id, local).await?;
        write_pending_checkpoint(&mut unit, instance_id, "").await?;
    }
    unit.commit().await?;
    Ok(true)
}

pub(super) async fn server_revision(state: &State) -> crate::Result<i64> {
    Ok(SyncedServerState::get(&state.data, &ServerStateSingleton)
        .await?
        .map(|record| record.revision)
        .unwrap_or(0))
}

pub(super) async fn load_local(
    instance_id: &str,
    state: &State,
) -> crate::Result<Vec<LocalServer>> {
    let rows =
        LocalServers::get_scoped(&state.data, &instance_id.to_owned()).await?;
    validate_size(
        rows.len(),
        rows.iter().map(|(_, record)| record.nbt.len()).sum(),
    )?;
    rows.into_iter()
        .map(|(key, record)| {
            Ok(LocalServer {
                id: key.id,
                source: ServerSource::from_str(&record.source).ok_or_else(
                    || ErrorKind::InputError("Unknown server source".into()),
                )?,
                excluded_synced_server_id: record.excluded_synced_server_id,
                data: nbt_from_bytes(record.nbt)?,
                position: record.position,
            })
        })
        .collect()
}

pub(super) async fn write_local(
    instance_id: &str,
    servers: &[LocalServer],
    state: &State,
) -> crate::Result<()> {
    let mut unit = atom(&state.data).await?;
    write_local_rows(&mut unit, instance_id, servers).await?;
    unit.commit().await
}

pub(super) async fn write_local_rows(
    tx: &mut Atom,
    instance_id: &str,
    servers: &[LocalServer],
) -> crate::Result<()> {
    validate_size(servers.len(), 0)?;
    let mut bytes = 0;
    let mut rows = Vec::new();
    for server in servers {
        let nbt = nbt_to_bytes(&server.data)?;
        bytes += nbt.len();
        validate_size(servers.len(), bytes)?;
        rows.push((
            LocalKey {
                instance_id: instance_id.into(),
                id: server.id.clone(),
            },
            LocalRecord {
                source: server.source.as_str().into(),
                excluded_synced_server_id: server
                    .excluded_synced_server_id
                    .clone(),
                nbt,
                position: server.position,
            },
        ));
    }
    LocalServers::atom_set_scoped(tx, &instance_id.to_owned(), &rows).await
}

pub(super) async fn load_projection_entries(
    instance_id: &str,
    state: &State,
) -> crate::Result<Vec<ProjectionEntry>> {
    let rows =
        Projections::get_scoped(&state.data, &instance_id.to_owned()).await?;
    validate_size(
        rows.len(),
        rows.iter().map(|(_, record)| record.nbt.len()).sum(),
    )?;
    rows.into_iter()
        .map(|(key, record)| {
            Ok(ProjectionEntry {
                id: key.server_id,
                owner: ProjectionOwner::from_str(&key.owner).ok_or_else(
                    || ErrorKind::InputError("Unknown projection owner".into()),
                )?,
                data: nbt_from_bytes(record.nbt)?,
                position: record.position,
            })
        })
        .collect()
}

pub(super) async fn begin_server_checkpoint(
    instance_id: &str,
    servers: &[ServerRecord],
    expected_sha1: &str,
    source_revision: i64,
    state: &State,
) -> crate::Result<super::super::synced_options::CheckpointProof> {
    validate_size(servers.len(), 0)?;
    let mut bytes = 0;
    let mut entries = Vec::with_capacity(servers.len());
    for (position, server) in servers.iter().enumerate() {
        let owner = if server.source == ServerSource::UserSynced {
            ProjectionOwner::Synced
        } else {
            ProjectionOwner::Instance
        }
        .as_str();
        let nbt = nbt_to_bytes(&server.data)?;
        bytes += nbt.len();
        validate_size(servers.len(), bytes)?;
        entries.push((
            ProjectionKey {
                instance_id: instance_id.into(),
                owner: owner.into(),
                server_id: server.id.clone(),
            },
            ProjectionRecord {
                nbt,
                position: position as i64,
            },
        ));
    }
    let mut unit = atom(&state.data).await?;
    let revision =
        SyncedServerState::atom_get(&mut unit, &ServerStateSingleton)
            .await?
            .map(|record| record.revision)
            .unwrap_or(source_revision);
    let key = CheckpointKey {
        instance_id: instance_id.to_string(),
        feature: "multiplayer_servers".to_string(),
        variant: "default".to_string(),
    };
    let record = StoredCheckpointRecord {
        expected_sha1: expected_sha1.to_string(),
        merge_base: None,
        source_revision: revision,
        status: StoredStatus::Pending,
        link_mode: None,
    };
    InstanceSyncCheckpoints::atom_set(&mut unit, &key, Some(&record)).await?;
    Projections::atom_set_scoped(&mut unit, &instance_id.to_owned(), &entries)
        .await?;
    unit.commit().await?;
    Ok(super::super::synced_options::CheckpointProof::new(
        expected_sha1,
        revision,
    ))
}

pub(super) fn generated_path(state: &State, instance_id: &str) -> PathBuf {
    synced_options_path(state)
        .join("servers/generated")
        .join(safe_instance_id(instance_id))
        .join(SERVERS_FILE)
}

/// Marks the projection of `instance_id` as waiting, in the unit that also
/// wrote the canonical list.
async fn write_pending_checkpoint(
    unit: &mut crate::state::store::Atom,
    instance_id: &str,
    expected_sha1: &str,
) -> crate::Result<()> {
    let revision = SyncedServerState::atom_get(unit, &ServerStateSingleton)
        .await?
        .map(|record| record.revision)
        .unwrap_or(0);
    let key = CheckpointKey {
        instance_id: instance_id.to_string(),
        feature: "multiplayer_servers".to_string(),
        variant: "default".to_string(),
    };
    let record = StoredCheckpointRecord {
        expected_sha1: expected_sha1.to_string(),
        merge_base: None,
        source_revision: revision,
        status: StoredStatus::Pending,
        link_mode: None,
    };
    InstanceSyncCheckpoints::atom_set(unit, &key, Some(&record)).await?;
    Ok(())
}

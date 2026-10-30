use super::super::synced_options::{
    nbt_from_bytes, nbt_to_bytes, safe_instance_id, synced_options_path,
};
use super::SERVERS_FILE;
use super::limits::validate_size;
use super::types::{
    CanonicalServer, LocalServer, ProjectionEntry, ProjectionOwner,
    ServerRecord, ServerSource,
};
use crate::state::instances::adapters::sqlite::instance_servers::LocalServerRow;
use crate::state::instances::adapters::sqlite::projection_entries::ProjectionEntryRow;
use crate::state::instances::adapters::sqlite::{
    instance_servers, projection_entries,
};
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
use crate::{ErrorKind, State};
use sqlx::{Sqlite, Transaction};
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
    let (count, bytes) = SyncedServers::stored_size(&state.data).await?;
    validate_size(count, bytes)?;
    SyncedServers::get_all(&state.data)
        .await?
        .into_iter()
        .map(|(id, record)| {
            Ok(CanonicalServer {
                id,
                data: nbt_from_bytes(record.nbt)?,
            })
        })
        .collect::<crate::Result<Vec<_>>>()
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
        write_canonical_rows(canonical, Some(instance_id), state).await?;
    write_local(instance_id, local, state).await?;
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
    checkpoint_for: Option<&str>,
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
        if let Some(instance_id) = checkpoint_for {
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
    if let Some(instance_id) = checkpoint_for {
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
    let (count, bytes) =
        instance_servers::get_local_server_size(instance_id, &state.pool)
            .await?;
    validate_size(count, bytes)?;
    instance_servers::get_local_servers(instance_id, &state.pool)
        .await?
        .into_iter()
        .map(|row| {
            Ok(LocalServer {
                id: row.id,
                source: ServerSource::from_str(&row.source).ok_or_else(
                    || {
                        ErrorKind::InputError(format!(
                            "Unknown server source {}",
                            row.source
                        ))
                    },
                )?,
                excluded_synced_server_id: row.excluded_synced_server_id,
                data: nbt_from_bytes(row.nbt)?,
                position: row.position,
            })
        })
        .collect()
}

pub(super) async fn write_local(
    instance_id: &str,
    servers: &[LocalServer],
    state: &State,
) -> crate::Result<()> {
    let mut tx = state.pool.begin().await?;
    write_local_rows(&mut tx, instance_id, servers).await?;
    tx.commit().await?;
    Ok(())
}

pub(super) async fn write_local_rows(
    tx: &mut Transaction<'_, Sqlite>,
    instance_id: &str,
    servers: &[LocalServer],
) -> crate::Result<()> {
    validate_size(servers.len(), 0)?;
    let mut bytes = 0;
    let mut rows = Vec::with_capacity(servers.len());
    for server in servers {
        let nbt = nbt_to_bytes(&server.data)?;
        bytes += nbt.len();
        validate_size(servers.len(), bytes)?;
        rows.push(LocalServerRow {
            id: server.id.clone(),
            source: server.source.as_str().to_string(),
            excluded_synced_server_id: server.excluded_synced_server_id.clone(),
            nbt,
            position: server.position,
        });
    }
    instance_servers::replace_local_servers(tx, instance_id, &rows).await?;
    Ok(())
}

pub(super) async fn load_projection_entries(
    instance_id: &str,
    state: &State,
) -> crate::Result<Vec<ProjectionEntry>> {
    let (count, bytes) =
        projection_entries::get_projection_size(instance_id, &state.pool)
            .await?;
    validate_size(count, bytes)?;
    projection_entries::get_projection_entries(instance_id, &state.pool)
        .await?
        .into_iter()
        .map(|row| {
            Ok(ProjectionEntry {
                id: row.server_id,
                owner: ProjectionOwner::from_str(&row.owner).ok_or_else(
                    || {
                        ErrorKind::InputError(format!(
                            "Unknown server projection owner {}",
                            row.owner
                        ))
                    },
                )?,
                data: nbt_from_bytes(row.nbt)?,
                position: row.position,
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
) -> crate::Result<()> {
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
        entries.push(ProjectionEntryRow {
            owner: owner.to_string(),
            server_id: server.id.clone(),
            nbt,
            position: position as i64,
        });
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
    unit.commit().await?;
    let mut tx = state.pool.begin().await?;
    projection_entries::replace_projection(&mut tx, instance_id, &entries)
        .await?;
    tx.commit().await?;
    Ok(())
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

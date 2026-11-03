use crate::State;
use crate::state::InstanceMetadata;
use crate::state::instances::adapters::sqlite::projection_entries::ProjectionEntryRow;
use crate::state::instances::adapters::sqlite::{
    instance_servers, projection_entries,
};
use crate::state::store::Collection as _;
use crate::state::store::atom;
use crate::state::store::instance_sync_preferences::InstanceSyncPreferences;
use crate::state::store::synced_server_state::SyncedServerState;
use crate::state::store::synced_servers::{SyncedServerRecord, SyncedServers};
use quartz_nbt::{NbtCompound, NbtList, NbtTag};
use std::path::PathBuf;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SyncedServer {
    pub id: String,
    pub name: String,
    pub address: String,
    pub accept_textures: Option<bool>,
}

pub(crate) async fn merge_servers_from_instance(
    metadata: &InstanceMetadata,
    state: &State,
) -> crate::Result<()> {
    if !metadata.synced_options.multiplayer_servers {
        return Ok(());
    }
    reconcile_servers(metadata, state).await
}
pub(crate) async fn reconcile_servers(
    metadata: &InstanceMetadata,
    state: &State,
) -> crate::Result<()> {
    if !metadata.synced_options.multiplayer_servers {
        return Ok(());
    }
    let local =
        crate::api::instance::synced_options::instance_dir(metadata, state)
            .join("servers.dat");
    if !local.exists() {
        return Ok(());
    }
    let bytes = tokio::fs::read(&local).await?;
    let canonical = canonical_path(state);
    if canonical.exists() && tokio::fs::read(&canonical).await? == bytes {
        return Ok(());
    }
    let servers = decode_servers(&bytes)?;
    let mut records = Vec::with_capacity(servers.len());
    for (position, nbt) in servers.into_iter().enumerate() {
        records.push((
            server_id(&nbt),
            SyncedServerRecord {
                position: position as i64,
                nbt: encode_server(&nbt)?,
            },
        ));
    }
    let mut unit = atom(&state.data).await?;
    SyncedServers::atom_set_all(&mut unit, &records).await?;
    SyncedServerState::bump_in(&mut unit).await?;
    unit.commit().await?;
    write_and_project(state).await?;
    Ok(())
}
pub(crate) async fn seed_servers(
    metadata: &InstanceMetadata,
    state: &State,
) -> crate::Result<()> {
    reconcile_servers(metadata, state).await
}
pub(crate) async fn ensure_servers(
    metadata: &InstanceMetadata,
    state: &State,
) -> crate::Result<()> {
    let canonical = canonical_path(state);
    if !canonical.exists() {
        return reconcile_servers(metadata, state).await;
    }
    let target =
        crate::api::instance::synced_options::instance_dir(metadata, state)
            .join("servers.dat");
    if !target.exists() {
        tokio::fs::copy(canonical, target).await?;
    }
    Ok(())
}
pub(crate) async fn detach_servers(
    _: &InstanceMetadata,
    _: &State,
) -> crate::Result<()> {
    Ok(())
}
pub(crate) async fn canonical_exists(state: &State) -> crate::Result<bool> {
    Ok(canonical_path(state).exists())
}
pub async fn list_synced_servers() -> crate::Result<Vec<SyncedServer>> {
    let state = State::get().await?;
    let rows = SyncedServers::get_all(&state.data).await?;
    rows.into_iter()
        .map(|(id, record)| {
            let data = crate::api::instance::synced_options::nbt_from_bytes(
                record.nbt,
            )?;
            Ok(SyncedServer {
                id,
                name: data
                    .get::<_, &str>("name")
                    .unwrap_or_default()
                    .to_string(),
                address: data
                    .get::<_, &str>("ip")
                    .unwrap_or_default()
                    .to_string(),
                accept_textures: data
                    .get::<_, i8>("acceptTextures")
                    .ok()
                    .map(|value| value != 0),
            })
        })
        .collect()
}
pub async fn update_synced_server(server: SyncedServer) -> crate::Result<()> {
    let state = State::get().await?;
    let mut data = NbtCompound::new();
    data.insert("name", server.name);
    data.insert("ip", server.address);
    data.insert("hidden", 0_i8);
    if let Some(value) = server.accept_textures {
        data.insert("acceptTextures", i8::from(value));
    }
    let nbt = encode_server(&data)?;
    let position = SyncedServers::position_for(&state.data, &server.id).await?;
    let mut unit = atom(&state.data).await?;
    SyncedServers::atom_set(
        &mut unit,
        &server.id,
        Some(&SyncedServerRecord { position, nbt }),
    )
    .await?;
    SyncedServerState::bump_in(&mut unit).await?;
    unit.commit().await?;
    write_and_project(&state).await?;
    Ok(())
}
pub async fn remove_synced_server(id: String) -> crate::Result<()> {
    let state = State::get().await?;
    let mut unit = atom(&state.data).await?;
    SyncedServers::atom_set(&mut unit, &id, None).await?;
    SyncedServerState::bump_in(&mut unit).await?;
    unit.commit().await?;
    normalize_positions(&state).await?;
    write_and_project(&state).await?;
    Ok(())
}
fn canonical_path(state: &State) -> PathBuf {
    state.directories.synced_options_dir().join("servers.dat")
}

fn decode_servers(bytes: &[u8]) -> crate::Result<Vec<NbtCompound>> {
    let root =
        crate::api::instance::synced_options::nbt_from_bytes(bytes.to_vec())?;
    let list = root.get::<_, &NbtList>("servers").map_err(|_| {
        crate::ErrorKind::InputError(
            "servers.dat does not contain a valid servers list".to_string(),
        )
    })?;
    list.iter()
        .map(|tag| match tag {
            NbtTag::Compound(value) => Ok(value.clone()),
            _ => Err(crate::ErrorKind::InputError(
                "servers.dat contains an invalid server entry".to_string(),
            )
            .into()),
        })
        .collect()
}

fn encode_server(server: &NbtCompound) -> crate::Result<Vec<u8>> {
    crate::api::instance::synced_options::nbt_to_bytes(server)
}

fn server_id(server: &NbtCompound) -> String {
    let name = server.get::<_, &str>("name").unwrap_or_default();
    let address = server.get::<_, &str>("ip").unwrap_or_default();
    crate::api::instance::synced_options::sha1_bytes(
        format!(
            "{}\0{}",
            name.trim().to_ascii_lowercase(),
            address.trim().to_ascii_lowercase()
        )
        .as_bytes(),
    )
}

async fn write_canonical(state: &State) -> crate::Result<()> {
    let rows = SyncedServers::get_all(&state.data).await?;
    let mut list = NbtList::new();
    for (_, record) in rows {
        list.push(crate::api::instance::synced_options::nbt_from_bytes(
            record.nbt,
        )?);
    }
    let mut root = NbtCompound::new();
    root.insert("servers", list);
    let path = canonical_path(state);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(
        path,
        crate::api::instance::synced_options::nbt_to_bytes(&root)?,
    )
    .await?;
    Ok(())
}

async fn normalize_positions(state: &State) -> crate::Result<()> {
    let rows = SyncedServers::get_all(&state.data).await?;
    let mut unit = atom(&state.data).await?;
    SyncedServers::shift_positions_in(&mut unit, 2_000_000).await?;
    for (position, (id, mut record)) in rows.into_iter().enumerate() {
        record.position = position as i64;
        SyncedServers::atom_set(&mut unit, &id, Some(&record)).await?;
    }
    unit.commit().await?;
    Ok(())
}

async fn write_and_project(state: &State) -> crate::Result<()> {
    write_canonical(state).await?;
    let canonical = canonical_path(state);
    let rows = InstanceSyncPreferences::instances_with_feature_enabled(
        &state.data,
        "multiplayer_servers",
    )
    .await?;
    for id in rows {
        if let Some(metadata) =
            crate::state::get_instance(&id, &state.pool).await?
        {
            let target = crate::api::instance::synced_options::instance_dir(
                &metadata, state,
            )
            .join("servers.dat");
            let excluded =
                instance_servers::has_desynced_synced_server(&id, &state.pool)
                    .await?;
            let canonical_rows = SyncedServers::get_all(&state.data).await?;
            let entries = canonical_rows
                .into_iter()
                .map(|(server_id, record)| ProjectionEntryRow {
                    owner: "synced".to_string(),
                    server_id,
                    nbt: record.nbt,
                    position: record.position,
                })
                .collect::<Vec<_>>();
            let mut tx = state.pool.begin().await?;
            projection_entries::replace_projection(&mut tx, &id, &entries)
                .await?;
            tx.commit().await?;
            if excluded {
                continue;
            }
            if let Some(parent) = target.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            tokio::fs::copy(&canonical, target).await?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_identity_is_case_and_whitespace_insensitive() {
        let mut first = NbtCompound::new();
        first.insert("name", " Example ");
        first.insert("ip", "PLAY.EXAMPLE.COM ");
        let mut second = NbtCompound::new();
        second.insert("name", "example");
        second.insert("ip", "play.example.com");
        assert_eq!(server_id(&first), server_id(&second));
    }

    #[test]
    fn server_compound_round_trips_through_storage() {
        let mut server = NbtCompound::new();
        server.insert("name", "Example");
        server.insert("ip", "play.example.com");
        server.insert("acceptTextures", 1_i8);
        let decoded = crate::api::instance::synced_options::nbt_from_bytes(
            encode_server(&server).unwrap(),
        )
        .unwrap();
        assert_eq!(decoded.get::<_, &str>("name").unwrap(), "Example");
        assert_eq!(decoded.get::<_, &str>("ip").unwrap(), "play.example.com");
    }
}

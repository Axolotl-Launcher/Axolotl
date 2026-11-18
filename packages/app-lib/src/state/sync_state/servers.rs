//! Provenance, exclusions and projection memberships adjacent to native NBT.
use super::collection::Collection;
use crate::state::store::{Atom, Handle};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct LocalKey {
    pub instance_id: String,
    pub id: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct LocalRecord {
    pub source: String,
    pub excluded_synced_server_id: Option<String>,
    pub nbt: Vec<u8>,
    pub position: i64,
}
pub(crate) struct LocalServers;
impl Collection for LocalServers {
    type Key = LocalKey;
    type Scope = String;
    type Record = LocalRecord;
    const NAME: &'static str = "instance_servers";
    fn in_scope(scope: &String, key: &LocalKey) -> bool {
        &key.instance_id == scope
    }
    fn ordered(rows: &mut Vec<(LocalKey, LocalRecord)>) {
        rows.sort_by_key(|(_, record)| record.position);
    }
}
super::collection::verbs!(LocalServers, LocalKey, String, LocalRecord);
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ProjectionKey {
    pub instance_id: String,
    pub owner: String,
    pub server_id: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct ProjectionRecord {
    pub nbt: Vec<u8>,
    pub position: i64,
}
pub(crate) struct Projections;
impl Collection for Projections {
    type Key = ProjectionKey;
    type Scope = String;
    type Record = ProjectionRecord;
    const NAME: &'static str = "instance_server_projection_entries";
    fn in_scope(scope: &String, key: &ProjectionKey) -> bool {
        &key.instance_id == scope
    }
    fn ordered(rows: &mut Vec<(ProjectionKey, ProjectionRecord)>) {
        rows.sort_by_key(|(_, record)| record.position);
    }
}
super::collection::verbs!(Projections, ProjectionKey, String, ProjectionRecord);
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct PackRecord {
    pub version_id: Option<String>,
}
pub(crate) struct PackState;
impl Collection for PackState {
    type Key = String;
    type Scope = String;
    type Record = PackRecord;
    const NAME: &'static str = "instance_server_pack_state";
    fn in_scope(scope: &String, key: &String) -> bool {
        scope == key
    }
}
super::collection::verbs!(PackState, String, String, PackRecord);

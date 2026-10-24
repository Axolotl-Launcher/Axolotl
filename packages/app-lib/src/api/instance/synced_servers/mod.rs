mod codec;
mod limits;
mod modpack;
mod operations;
mod projection;
mod reconciliation;
mod storage;
mod types;

const SERVERS_FILE: &str = "servers.dat";

pub use self::operations::{
    desync_server, list_synced_servers, remove_synced_server,
    update_synced_server,
};
pub(super) use self::operations::{
    detach_servers, ensure_servers, merge_servers_from_instance,
    reconcile_servers, seed_servers,
};
pub(super) use self::storage::canonical_exists;
pub use self::types::{DesyncServerMode, ServerSource, SyncedServer};

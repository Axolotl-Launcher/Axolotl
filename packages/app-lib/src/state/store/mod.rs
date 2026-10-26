//! The records the launcher keeps between runs.
//!
//! Callers name a collection and the fields they look a record up by; which
//! storage answers, and how it is asked, is this package's business alone. A
//! storage is therefore replaceable without touching a single caller, and no
//! caller names one: everything below `Handle` stays in here.
//!
//! Four verbs: `set` and `get` reach one record by its key, `set_all` and
//! `get_all` reach a whole collection. Writing nothing removes the record a key
//! held, so a caller never needs a second verb for that.

pub(crate) mod content_favorites;
pub(crate) mod game_option_pack_bases;
pub(crate) mod game_option_update_state;
pub(crate) mod join_log;
pub(crate) mod official_login_proof;
pub(crate) mod resource_source_projects;

use sqlx::SqlitePool;

/// What a caller passes on instead of a storage: a handle on the launcher's
/// records, created once at startup by whatever provides them today.
#[derive(Clone)]
pub(crate) struct Handle {
    pool: SqlitePool,
}

impl Handle {
    pub(crate) fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub(in crate::state::store) fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

/// A collection of records, and the fields it is looked up by.
pub(crate) trait Collection {
    /// The fields one record is identified by.
    type Key;

    /// What one record carries beyond its key.
    type Record;

    /// Writes `record` under `key`. Nothing to write removes the record the key
    /// held.
    async fn set(
        handle: &Handle,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()>;

    /// The record `key` holds. A key nothing was written under holds none,
    /// which is not an error.
    async fn get(
        handle: &Handle,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>>;

    /// Writes the whole collection as one unit, so it is never left half
    /// updated.
    async fn set_all(
        handle: &Handle,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()>;

    /// Every record the collection holds, with the key each is stored under.
    async fn get_all(
        handle: &Handle,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>>;
}

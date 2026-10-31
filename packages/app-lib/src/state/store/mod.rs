//! The records the launcher keeps between runs.
//!
//! Callers name a collection and the fields they look a record up by; which
//! storage answers, and how it is asked, is this package's business alone. A
//! storage is therefore replaceable without touching a single caller, and no
//! caller names one: everything below `Handle` stays in here.
//!
//! The verbs are centred on `get` and `set`:
//!
//! - `get`/`set` reach one record by its key; writing nothing removes it.
//! - `_all` reaches a whole collection.
//! - `_scoped` reaches the records one scope names.
//! - `atom_` runs the same verb inside an atomic unit, so a group of writes
//!   lands together or not at all.

pub(crate) mod cache;
pub(crate) mod content_dependencies;
pub(crate) mod content_file_fingerprints;
pub(crate) mod content_file_operations;
pub(crate) mod content_refresh;
pub(crate) mod content_refresh_checks;
pub(crate) mod content_refresh_pack;
pub(crate) mod content_refresh_presentations;
pub(crate) mod content_refresh_scan_failures;
pub(crate) mod content_snapshot;
pub(crate) mod content_update_jobs;
pub(crate) mod crash_analysis_mod_snapshots;
pub(crate) mod custom_minecraft_skin_textures;
pub(crate) mod custom_minecraft_skins;
pub(crate) mod derived_db;
pub(crate) mod discovered_javas;
pub(crate) mod game_option_locale_origins;
pub(crate) mod game_option_pack_bases;
pub(crate) mod game_option_update_state;
pub(crate) mod google_translate_ip_cache;
pub(crate) mod install_jobs;
pub(crate) mod instance_content_files;
pub(crate) mod instance_sync_checkpoints;
pub(crate) mod instance_sync_preferences;
pub(crate) mod java_default_versions;
pub(crate) mod java_versions;
pub(crate) mod join_log;
pub(crate) mod maintenance;
pub(crate) mod minecraft_device_tokens;
pub(crate) mod minecraft_users;
pub(crate) mod modrinth_users;
pub(crate) mod official_login_proof;
pub(crate) mod offline_minecraft_skins;
pub(crate) mod pending_backup_repository_cleanups;
pub(crate) mod resource_artifact_evidence;
pub(crate) mod resource_cache_entries;
pub(crate) mod resource_cache_state;
pub(crate) mod resource_entry_graphs;
pub(crate) mod resource_entry_links;
pub(crate) mod resource_execution_journals;
pub(crate) mod resource_favorites;
pub(crate) mod resource_history;
pub(crate) mod resource_history_cleanup_plans;
pub(crate) mod resource_installed_artifacts;
pub(crate) mod resource_legacy_import;
pub(crate) mod resource_operation_plans;
pub(crate) mod resource_pack_unlinks;
pub(crate) mod resource_source_projects;
pub(crate) mod secrets;
pub(crate) mod settings;
pub(crate) mod shared_logs;
pub(crate) mod store_blobs;
pub(crate) mod store_retained_refs;
pub(crate) mod sync_feature_settings;
pub(crate) mod synced_game_option_preferences;
pub(crate) mod synced_game_option_state;
pub(crate) mod synced_game_option_values;
pub(crate) mod synced_hotbar_state;
pub(crate) mod synced_pack_catalog;
pub(crate) mod synced_pack_instances;
pub(crate) mod synced_server_state;
pub(crate) mod synced_servers;
pub(crate) mod telemetry_identity;
pub(crate) mod telemetry_outbox;
pub(crate) mod translation_cache;

#[cfg(test)]
pub(crate) mod tests;

use std::collections::BTreeSet;

use sqlx::{Sqlite, SqliteConnection, SqlitePool};

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

    /// The handle's storage, for the statements a collection writes.
    pub(in crate::state::store) fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// The settings documents of this launch, for a caller composing the
    /// settings from them and the row this handle reaches.
    pub(crate) fn settings_documents(
        &self,
    ) -> Option<crate::state::documents::Documents<'_>> {
        crate::state::documents::Documents::of_launch(&self.pool)
    }

    async fn connection(
        &self,
    ) -> crate::Result<sqlx::pool::PoolConnection<Sqlite>> {
        Ok(self.pool.acquire().await?)
    }
}

/// One atomic unit: everything written through it lands together, or nothing
/// does. Dropping an uncommitted unit rolls it back.
pub(crate) struct Atom {
    transaction: sqlx::Transaction<'static, Sqlite>,
    /// Whether a settings read has to be composed again once this unit lands.
    /// It is registered by a writer that changes the settings row, and acted on
    /// by `commit`, because a unit is not a change until it commits.
    invalidate_settings: bool,
    /// The keys the settings write of this unit names, when it names them: what
    /// `commit` states the domains from. Nothing means every domain, which is
    /// what a writer that cannot say which keys it changed states.
    changed_settings: Option<BTreeSet<String>>,
    /// The records this unit writes, kept for what a commit has to do after it
    /// lands: the settings documents a changed row states are reached through
    /// the handle rather than through the unit's transaction, which is gone by
    /// then.
    handle: Handle,
}

impl Atom {
    fn connection(&mut self) -> &mut SqliteConnection {
        &mut self.transaction
    }

    /// Says that this unit changes the settings row, so a settings read
    /// composed after it commits is the one to keep, and so the domains whose
    /// writes have moved to their documents are stated from the row once it
    /// lands.
    ///
    /// A writer registers it rather than invalidating where it writes: a read
    /// between the write and the commit composes the row this unit has not
    /// replaced yet, and an invalidation at the write would have that answer
    /// cached under the version it moved — with nothing moving it again once
    /// the commit lands, the stale answer would then be the one every later read
    /// is given.
    pub(crate) fn invalidate_settings_after_commit(&mut self) {
        self.invalidate_settings = true;
    }

    /// Names the keys this unit's settings write carries, so the commit states
    /// the documents of the domains holding one of them rather than all of them:
    /// a domain the write did not touch keeps the document it has, which is what
    /// a value edited by hand there survives.
    ///
    /// A unit that names none states every domain — still leaving out the keys a
    /// deployment answers, which no local document may copy.
    pub(crate) fn settings_changed(
        &mut self,
        keys: impl IntoIterator<Item = String>,
    ) {
        self.changed_settings
            .get_or_insert_with(BTreeSet::new)
            .extend(keys);
    }

    pub(crate) async fn commit(self) -> crate::Result<()> {
        let Self {
            transaction,
            invalidate_settings,
            changed_settings,
            handle,
        } = self;
        transaction.commit().await?;
        // After the commit and not before: the row a read composes is the one
        // this commit replaced from here on, and only from here on.
        if invalidate_settings {
            crate::state::documents::settings::invalidate();
            // The documents of the domains whose writes have moved are written
            // after the row and never before, so a start stopped between the two
            // finds a document behind the row rather than ahead of it. A failure
            // here costs the domains and is reported in the log: the row is
            // already committed, so the change has landed and the caller must
            // not be told otherwise.
            crate::state::documents::settings::store_cut_over(
                &handle,
                changed_settings.as_ref(),
            )
            .await;
        }
        Ok(())
    }
}

/// Whether a write was refused because it named a row that is not there.
///
/// SQLite reports a foreign key the statement cannot satisfy as
/// `SQLITE_CONSTRAINT_FOREIGNKEY`, which is the one refusal a caller can answer
/// by dropping the reference itself.
pub(crate) fn names_a_missing_row(error: &crate::Error) -> bool {
    matches!(
        error.raw.as_ref(),
        crate::ErrorKind::Sqlx(sqlx::Error::Database(database))
            if database.code().as_deref() == Some("787")
    )
}

/// Opens an atomic unit on `handle`.
///
/// The unit takes its write reservation as it opens, the way the callers that
/// read a record and write it back need: a unit that read first and only then
/// asked to write could find another writer had committed in between, and be
/// refused instead of applied.
pub(crate) async fn atom(handle: &Handle) -> crate::Result<Atom> {
    Ok(Atom {
        transaction: handle.pool.begin_with("BEGIN IMMEDIATE").await?,
        invalidate_settings: false,
        changed_settings: None,
        handle: handle.clone(),
    })
}

/// A collection of records, and the fields it is looked up by.
///
/// Each collection answers six statements — one record by key, the whole
/// collection, and one scope, each for reading and for writing. The verbs below
/// are written once in terms of them, which is what makes every collection
/// answer the same way.
pub(crate) trait Collection: Sized {
    /// The fields one record is identified by.
    type Key: PartialEq;

    /// The narrower set of fields that names a group of records.
    type Scope;

    /// What one record carries beyond its key.
    type Record;

    /// Whether `key` is one of the records `scope` names.
    fn in_scope(scope: &Self::Scope, key: &Self::Key) -> bool;

    async fn read(
        conn: &mut SqliteConnection,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>>;

    async fn write(
        conn: &mut SqliteConnection,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()>;

    async fn read_all(
        conn: &mut SqliteConnection,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>>;

    async fn replace_all(
        conn: &mut SqliteConnection,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()>;

    async fn read_scoped(
        conn: &mut SqliteConnection,
        scope: &Self::Scope,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>>;

    async fn replace_scoped(
        conn: &mut SqliteConnection,
        scope: &Self::Scope,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()>;

    /// The record `key` holds. A key nothing was written under holds none,
    /// which is not an error.
    async fn get(
        handle: &Handle,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let mut connection = handle.connection().await?;
        Self::read(&mut connection, key).await
    }

    /// Writes `record` under `key`. Nothing to write removes the record the key
    /// held.
    async fn set(
        handle: &Handle,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let mut unit = atom(handle).await?;
        Self::write(unit.connection(), key, record).await?;
        unit.commit().await
    }

    /// Every record the collection holds, with the key each is stored under.
    async fn get_all(
        handle: &Handle,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        let mut connection = handle.connection().await?;
        Self::read_all(&mut connection).await
    }

    /// Writes the whole collection as one unit, so it is never left half
    /// updated.
    async fn set_all(
        handle: &Handle,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        ensure_unique::<Self>(records)?;
        let mut atom = atom(handle).await?;
        Self::replace_all(atom.connection(), records).await?;
        atom.commit().await
    }

    /// The records `scope` names.
    async fn get_scoped(
        handle: &Handle,
        scope: &Self::Scope,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        let mut connection = handle.connection().await?;
        Self::read_scoped(&mut connection, scope).await
    }

    /// Replaces the records `scope` names with `records`, as one unit.
    async fn set_scoped(
        handle: &Handle,
        scope: &Self::Scope,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        ensure_in_scope::<Self>(scope, records)?;
        let mut atom = atom(handle).await?;
        Self::replace_scoped(atom.connection(), scope, records).await?;
        atom.commit().await
    }

    /// The record `key` holds, read inside `atom`.
    async fn atom_get(
        atom: &mut Atom,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        Self::read(atom.connection(), key).await
    }

    /// Writes `record` under `key` inside `atom`.
    async fn atom_set(
        atom: &mut Atom,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        Self::write(atom.connection(), key, record).await
    }

    /// Every record the collection holds, read inside `atom`.
    async fn atom_get_all(
        atom: &mut Atom,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        Self::read_all(atom.connection()).await
    }

    /// Writes the whole collection inside `atom`.
    async fn atom_set_all(
        atom: &mut Atom,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        ensure_unique::<Self>(records)?;
        Self::replace_all(atom.connection(), records).await
    }

    /// The records `scope` names, read inside `atom`.
    async fn atom_get_scoped(
        atom: &mut Atom,
        scope: &Self::Scope,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        Self::read_scoped(atom.connection(), scope).await
    }

    /// Replaces the records `scope` names inside `atom`.
    async fn atom_set_scoped(
        atom: &mut Atom,
        scope: &Self::Scope,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        ensure_in_scope::<Self>(scope, records)?;
        Self::replace_scoped(atom.connection(), scope, records).await
    }
}

/// A replace writes exactly what it was given, so one key twice is a mistake
/// worth reporting rather than a silent last-one-wins.
fn ensure_unique<C: Collection>(
    records: &[(C::Key, C::Record)],
) -> crate::Result<()> {
    for (index, (key, _)) in records.iter().enumerate() {
        if records[..index].iter().any(|(earlier, _)| earlier == key) {
            return Err(crate::ErrorKind::InputError(
                "A replace cannot hold the same key twice".to_string(),
            )
            .into());
        }
    }
    Ok(())
}

/// A scoped replace only touches the records its scope names, so a key outside
/// it would change state the caller did not ask about.
fn ensure_in_scope<C: Collection>(
    scope: &C::Scope,
    records: &[(C::Key, C::Record)],
) -> crate::Result<()> {
    ensure_unique::<C>(records)?;
    if records.iter().all(|(key, _)| C::in_scope(scope, key)) {
        return Ok(());
    }
    Err(crate::ErrorKind::InputError(
        "A scoped replace cannot write a key outside its scope".to_string(),
    )
    .into())
}

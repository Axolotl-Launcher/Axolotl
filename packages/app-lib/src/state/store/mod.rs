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
pub(crate) mod compatibility;
pub(crate) mod configuration;
pub(crate) mod configuration_documents;
pub(crate) mod content_dependencies;
pub(crate) mod content_evidence;
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
#[cfg(test)]
pub(crate) mod custom_minecraft_skin_textures;
#[cfg(test)]
pub(crate) mod custom_minecraft_skins;
pub(crate) mod databases;
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
#[cfg(test)]
pub(crate) mod join_log;
pub(crate) mod legacy_documents;
pub(crate) mod legacy_media;
pub(crate) mod legacy_user_state;
pub(crate) mod lock_diagnostics;
pub(crate) mod maintenance;
pub(crate) mod minecraft_device_tokens;
pub(crate) mod minecraft_users;
#[cfg(test)]
pub(crate) mod modrinth_users;
pub(crate) mod official_login_proof;
#[cfg(test)]
pub(crate) mod offline_minecraft_skins;
pub(crate) mod operations_db;
pub(crate) mod pending_backup_repository_cleanups;
pub(crate) mod records_db;
pub(crate) mod recovery_documents;
pub(crate) mod recovery_mode;
pub(crate) mod resource_artifact_evidence;
pub(crate) mod resource_cache_entries;
pub(crate) mod resource_cache_schema;
pub(crate) mod resource_cache_state;
pub(crate) mod resource_entry_graphs;
pub(crate) mod resource_entry_links;
pub(crate) mod resource_execution_journals;
#[cfg(test)]
pub(crate) mod resource_favorites;
pub(crate) mod resource_history;
pub(crate) mod resource_history_cleanup_plans;
pub(crate) mod resource_identity_db;
pub(crate) mod resource_installed_artifacts;
pub(crate) mod resource_legacy_import;
pub(crate) mod resource_operation_plans;
pub(crate) mod resource_pack_unlinks;
pub(crate) mod resource_source_projects;
pub(crate) mod scheduler_restart_budget;
pub(crate) mod scheduler_tags;
pub(crate) mod secrets;
pub(crate) mod settings;
#[cfg(test)]
pub(crate) mod shared_logs;
pub(crate) mod store_blobs;
pub(crate) mod store_retained_refs;
pub(crate) mod sync_documents;
pub(crate) mod sync_feature_settings;
pub(crate) mod synced_game_option_preferences;
pub(crate) mod synced_game_option_state;
pub(crate) mod synced_game_option_values;
pub(crate) mod synced_hotbar_state;
pub(crate) mod synced_pack_catalog;
pub(crate) mod synced_pack_instances;
pub(crate) mod synced_server_state;
pub(crate) mod synced_servers;
#[cfg(test)]
pub(crate) mod telemetry_identity;
#[cfg(test)]
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
    persistence: Option<crate::state::persistence::Persistence>,
    accounts:
        std::sync::Arc<std::sync::OnceLock<crate::state::accounts::Accounts>>,
}

impl Handle {
    pub(crate) fn persistence(&self) -> crate::state::persistence::Persistence {
        self.persistence
            .clone()
            .expect("Independent state requires an original data root")
    }
    pub(crate) fn new(pool: SqlitePool) -> Self {
        let filename = pool.connect_options().get_filename().to_path_buf();
        let accounts = std::sync::OnceLock::new();
        let recovery_root = recovery_mode::root(&pool);
        if let Some(root) = &recovery_root {
            let _ = accounts.set(crate::state::accounts::Accounts::new(
                &crate::state::persistence::Persistence::new(root),
            ));
        }
        if filename.file_name().is_some_and(|name| name == "system.db") {
            let _ = accounts.set(crate::state::accounts::Accounts::new(
                &crate::state::persistence::Persistence::new(
                    filename
                        .parent()
                        .and_then(std::path::Path::parent)
                        .expect("database data root"),
                ),
            ));
        }
        Self {
            persistence: if recovery_root.is_some() {
                Some(persistence_for_pool(&pool))
            } else if filename
                .file_name()
                .is_some_and(|name| name == "system.db")
            {
                Some(persistence_for_pool(&pool))
            } else {
                #[cfg(test)]
                {
                    Some(test_persistence(&pool))
                }
                #[cfg(not(test))]
                {
                    None
                }
            },
            pool,
            accounts: std::sync::Arc::new(accounts),
        }
    }

    pub(crate) fn accounts(&self) -> &crate::state::accounts::Accounts {
        #[cfg(test)]
        {
            self.accounts
                .get_or_init(crate::state::accounts::Accounts::temporary)
        }
        #[cfg(not(test))]
        {
            self.accounts.get().expect(
                "account service must be rooted in the launcher data directory",
            )
        }
    }

    pub(crate) fn with_persistence(
        pool: SqlitePool,
        persistence: &crate::state::persistence::Persistence,
    ) -> Self {
        #[cfg(test)]
        {
            let options = pool.connect_options();
            let mut roots =
                explicit_test_roots().lock().expect("explicit test roots");
            roots.retain(|_, (options, _)| options.strong_count() > 0);
            roots.insert(
                std::sync::Arc::as_ptr(&options) as usize,
                (std::sync::Arc::downgrade(&options), persistence.clone()),
            );
        }
        Self {
            persistence: Some(persistence.clone()),
            pool,
            accounts: std::sync::Arc::new(std::sync::OnceLock::from(
                crate::state::accounts::Accounts::new(persistence),
            )),
        }
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
        if configuration_documents::runtime(&self.pool) {
            Some(
                crate::state::documents::Documents::at(
                    self.persistence()
                        .root()
                        .join(crate::state::layout::SETTINGS),
                    &self.pool,
                )
                .with_config_root(crate::state::documents::config_root()),
            )
        } else {
            crate::state::documents::Documents::of_launch(&self.pool)
        }
    }

    async fn connection(
        &self,
    ) -> crate::Result<sqlx::pool::PoolConnection<Sqlite>> {
        Ok(self.pool.acquire().await?)
    }
}

/// One SQL atomic unit: writes to its attached databases land together, or
/// nothing does. Independent file services use `state::persistence` and never
/// join this unit. Settings retain their explicit durable publication journal.
/// Dropping an uncommitted unit rolls it back.
pub(crate) struct Atom {
    diagnostics: lock_diagnostics::Scope,
    operation_id: uuid::Uuid,
    configuration: Option<configuration_documents::Pending>,
    sync_transaction: Option<crate::state::sync_state::journal::Transaction>,
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
    #[tracing::instrument(level = "debug", skip_all, fields(operation_id = %self.operation_id))]
    pub(crate) async fn configuration(
        &mut self,
    ) -> crate::Result<&mut configuration_documents::Pending> {
        if self.configuration.is_none() {
            self.diagnostics.phase("configuration_acquire");
            tracing::debug!(
                phase = "configuration_acquire",
                "Core transaction phase"
            );
            let pending =
                configuration_documents::Pending::begin(&self.handle).await?;
            tracing::debug!(
                phase = "configuration_replay",
                "Core transaction phase"
            );
            configuration::publish_committed_in(
                &mut self.transaction,
                self.handle.persistence().root(),
            )
            .await?;
            self.configuration = Some(pending);
            tracing::debug!(
                phase = "configuration_ready",
                "Core transaction phase"
            );
        }
        Ok(self
            .configuration
            .as_mut()
            .expect("initialized document transaction"))
    }
    pub(crate) async fn validate_sync_parents(
        &mut self,
        collection: &str,
        key: &serde_json::Value,
    ) -> crate::Result<()> {
        sync_documents::validate_parents(
            &mut self.transaction,
            collection,
            key,
        )
        .await?;
        if collection == "instance_sync_checkpoints" {
            let instance_id =
                key["instance_id"].as_str().unwrap_or_default().to_owned();
            let feature =
                key["feature"].as_str().unwrap_or_default().to_owned();
            if instance_sync_preferences::InstanceSyncPreferences::atom_get(
                self,
                &instance_sync_preferences::InstancePreferenceKey {
                    instance_id,
                    feature,
                },
            )
            .await?
            .is_none()
            {
                return Err(crate::ErrorKind::InputError(
                    "Sync checkpoint has no matching instance preference"
                        .into(),
                )
                .into());
            }
        }
        Ok(())
    }
    #[tracing::instrument(level = "debug", skip_all, fields(operation_id = %self.operation_id))]
    pub(crate) async fn sync(
        &mut self,
    ) -> crate::Result<&mut crate::state::sync_state::journal::Transaction>
    {
        recovery_mode::ensure_path_writable(self.handle.persistence().root())?;
        if self.sync_transaction.is_none() {
            self.diagnostics.phase("sync_acquire");
            self.configuration().await?;
            tracing::debug!(
                phase = "sync_decision_read",
                "Core transaction phase"
            );
            let decision =
                sync_documents::decision(&mut self.transaction).await?;
            tracing::debug!(
                phase = "sync_authority_read",
                "Core transaction phase"
            );
            let root =
                sync_documents::authority(&self.handle.persistence()).await?;
            tracing::debug!(
                phase = "sync_transaction_acquire",
                "Core transaction phase"
            );
            self.sync_transaction = Some(
                crate::state::sync_state::journal::Transaction::begin(
                    root, decision,
                )
                .await?,
            );
            tracing::debug!(phase = "sync_ready", "Core transaction phase");
        }
        Ok(self
            .sync_transaction
            .as_mut()
            .expect("initialized sync transaction"))
    }
    pub(crate) fn transaction(
        &mut self,
    ) -> &mut sqlx::Transaction<'static, Sqlite> {
        &mut self.transaction
    }
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

    #[tracing::instrument(level = "debug", skip_all, fields(operation_id = %self.operation_id))]
    pub(crate) async fn commit(mut self) -> crate::Result<()> {
        tracing::debug!(
            phase = "commit_prepare_sync",
            "Core transaction phase"
        );
        let persistent = self
            .handle
            .pool
            .connect_options()
            .get_filename()
            .file_name()
            .is_some_and(|name| name == "system.db");
        self.diagnostics.phase("prepare_sync");
        let sync_token = if let Some(sync) = &self.sync_transaction {
            sync.prepare().await?
        } else {
            None
        };
        if let Some(token) = &sync_token {
            sync_documents::decide(&mut self.transaction, token).await?;
        }
        if let Some(pending) = &self.configuration {
            self.diagnostics.phase("prepare_configuration");
            tracing::debug!(
                phase = "commit_prepare_configuration",
                "Core transaction phase"
            );
            let documents = pending
                .snapshot(&self.handle, self.changed_settings.as_ref())
                .await?;
            configuration::prepare_documents(
                &mut self.transaction,
                self.handle.persistence().root(),
                documents,
            )
            .await?;
        }
        if !persistent {
            tracing::debug!(
                phase = "commit_legacy_sql",
                "Core transaction phase"
            );
            self.diagnostics
                .observe(
                    "commit_wait",
                    configuration::commit_settings(
                        self.transaction,
                        &self.handle.pool,
                        self.invalidate_settings,
                        self.changed_settings.as_ref(),
                    ),
                )
                .await?;
            self.diagnostics.complete("committed");
            if self.invalidate_settings {
                crate::state::documents::settings::invalidate();
                crate::state::documents::settings::store_cut_over(
                    &self.handle,
                    self.changed_settings.as_ref(),
                )
                .await;
            }
            tracing::debug!(phase = "complete", "Core transaction phase");
            return Ok(());
        }
        let settings_token = if self.configuration.is_some() {
            configuration::pending_token(&mut self.transaction).await?
        } else {
            None
        };
        tracing::debug!(phase = "commit_sql", "Core transaction phase");
        self.diagnostics
            .observe("commit_wait", self.transaction.commit())
            .await?;
        self.diagnostics.complete("committed");
        tracing::debug!(phase = "core_released", "Core transaction phase");
        if let Some(token) = &settings_token {
            self.diagnostics.phase("file_publish_configuration");
            tracing::debug!(
                phase = "publish_configuration",
                "Core transaction phase"
            );
            let publication = configuration::publish_prepared(
                self.handle.persistence().root(),
                token,
            )
            .await;
            crate::state::documents::settings::invalidate();
            publication?;
        }
        if let (Some(sync), Some(token)) = (&self.sync_transaction, &sync_token)
        {
            tracing::debug!(phase = "publish_sync", "Core transaction phase");
            self.diagnostics
                .observe("file_publish_sync", sync.finish(token))
                .await?;
        }
        // Release the file writer before any pool/SQL writer acquisition. A
        // competing Atom always reserves core first, then the file authority.
        drop(self.sync_transaction);
        drop(self.configuration);
        tracing::debug!(
            phase = "file_writers_released",
            "Core transaction phase"
        );
        if persistent && let Some(token) = settings_token {
            tracing::debug!(
                phase = "clear_configuration_decision",
                "Core transaction phase"
            );
            self.diagnostics
                .observe(
                    "decision_cleanup_wait",
                    configuration::clear_published(&self.handle.pool, &token),
                )
                .await?;
        }
        if self.invalidate_settings {
            crate::state::documents::settings::invalidate();
            if !persistent {
                crate::state::documents::settings::store_cut_over(
                    &self.handle,
                    self.changed_settings.as_ref(),
                )
                .await;
            }
        }
        tracing::debug!(phase = "complete", "Core transaction phase");
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
#[tracing::instrument(level = "debug", skip_all, fields(operation_id = tracing::field::Empty, database = %handle.pool.connect_options().get_filename().display()))]
pub(crate) async fn atom(handle: &Handle) -> crate::Result<Atom> {
    recovery_mode::ensure_writable(&handle.pool)?;

    let (transaction, diagnostics) =
        lock_diagnostics::begin(&handle.pool, "BEGIN IMMEDIATE").await?;
    let operation_id = diagnostics.id();
    tracing::Span::current()
        .record("operation_id", tracing::field::display(operation_id));
    tracing::debug!(phase = "core_acquired", "Core transaction phase");
    Ok(Atom {
        diagnostics,
        operation_id,
        configuration: None,
        sync_transaction: None,
        transaction,
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
        let diagnostics =
            lock_diagnostics::Scope::new(handle.pool(), "autocommit_read");
        diagnostics.label(std::any::type_name::<Self>());
        let mut connection = diagnostics
            .observe("pool_acquire_wait", handle.connection())
            .await?;
        diagnostics
            .observe("read_wait", Self::read(&mut connection, key))
            .await
    }

    /// Writes `record` under `key`. Nothing to write removes the record the key
    /// held.
    async fn set(
        handle: &Handle,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let mut unit = atom(handle).await?;
        unit.diagnostics.label(std::any::type_name::<Self>());
        unit.diagnostics.phase("collection_write");
        unit.diagnostics
            .observe(
                "collection_write_wait",
                Self::write(&mut unit.transaction, key, record),
            )
            .await?;
        unit.commit().await
    }

    /// Every record the collection holds, with the key each is stored under.
    async fn get_all(
        handle: &Handle,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        let diagnostics =
            lock_diagnostics::Scope::new(handle.pool(), "autocommit_read");
        diagnostics.label(std::any::type_name::<Self>());
        let mut connection = diagnostics
            .observe("pool_acquire_wait", handle.connection())
            .await?;
        diagnostics
            .observe("read_wait", Self::read_all(&mut connection))
            .await
    }

    /// Writes the whole collection as one unit, so it is never left half
    /// updated.
    async fn set_all(
        handle: &Handle,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        ensure_unique::<Self>(records)?;
        let mut atom = atom(handle).await?;
        atom.diagnostics.label(std::any::type_name::<Self>());
        atom.diagnostics.phase("collection_replace");
        atom.diagnostics
            .observe(
                "collection_replace_wait",
                Self::replace_all(&mut atom.transaction, records),
            )
            .await?;
        atom.commit().await
    }

    /// The records `scope` names.
    async fn get_scoped(
        handle: &Handle,
        scope: &Self::Scope,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        let diagnostics =
            lock_diagnostics::Scope::new(handle.pool(), "autocommit_read");
        diagnostics.label(std::any::type_name::<Self>());
        let mut connection = diagnostics
            .observe("pool_acquire_wait", handle.connection())
            .await?;
        diagnostics
            .observe("read_wait", Self::read_scoped(&mut connection, scope))
            .await
    }

    /// Replaces the records `scope` names with `records`, as one unit.
    async fn set_scoped(
        handle: &Handle,
        scope: &Self::Scope,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        ensure_in_scope::<Self>(scope, records)?;
        let mut atom = atom(handle).await?;
        atom.diagnostics.label(std::any::type_name::<Self>());
        atom.diagnostics.phase("collection_replace");
        atom.diagnostics
            .observe(
                "collection_replace_wait",
                Self::replace_scoped(&mut atom.transaction, scope, records),
            )
            .await?;
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

/// A core pool has a stable original data root, including before State exists.
pub(crate) fn persistence_for_pool(
    pool: &SqlitePool,
) -> crate::state::persistence::Persistence {
    if let Some(root) = recovery_mode::root(pool) {
        return crate::state::persistence::Persistence::new(root);
    }
    let filename = pool.connect_options().get_filename().to_path_buf();
    if filename.file_name().is_some_and(|name| name == "system.db") {
        return crate::state::persistence::Persistence::new(
            filename
                .parent()
                .and_then(std::path::Path::parent)
                .expect("core data root"),
        );
    }
    #[cfg(test)]
    {
        test_persistence(pool)
    }
    #[cfg(not(test))]
    {
        panic!("Independent state requires the launcher core pool")
    }
}
#[cfg(test)]
fn test_persistence(
    pool: &SqlitePool,
) -> crate::state::persistence::Persistence {
    use std::sync::{Arc, Mutex, OnceLock, Weak};
    type Roots = std::collections::HashMap<
        usize,
        (Weak<sqlx::sqlite::SqliteConnectOptions>, tempfile::TempDir),
    >;
    static ROOTS: OnceLock<Mutex<Roots>> = OnceLock::new();
    let options = pool.connect_options();
    let key = Arc::as_ptr(&options) as usize;
    if let Some((_, persistence)) = explicit_test_roots()
        .lock()
        .expect("explicit test roots")
        .get(&key)
    {
        return persistence.clone();
    }
    let mut roots = ROOTS
        .get_or_init(|| Mutex::new(std::collections::HashMap::new()))
        .lock()
        .expect("test state roots");
    roots.retain(|_, (options, _)| options.strong_count() > 0);
    let (_, directory) = roots.entry(key).or_insert_with(|| {
        (
            Arc::downgrade(&options),
            tempfile::tempdir().expect("test user-state root"),
        )
    });
    crate::state::persistence::Persistence::new(directory.path())
}

#[cfg(test)]
fn explicit_test_roots() -> &'static std::sync::Mutex<
    std::collections::HashMap<
        usize,
        (
            std::sync::Weak<sqlx::sqlite::SqliteConnectOptions>,
            crate::state::persistence::Persistence,
        ),
    >,
> {
    static ROOTS: std::sync::OnceLock<
        std::sync::Mutex<
            std::collections::HashMap<
                usize,
                (
                    std::sync::Weak<sqlx::sqlite::SqliteConnectOptions>,
                    crate::state::persistence::Persistence,
                ),
            >,
        >,
    > = std::sync::OnceLock::new();
    ROOTS.get_or_init(std::sync::Mutex::default)
}

//! The settings documents: one JSON file per domain, under
//! `<settings dir>/settings/`.
//!
//! A document is authoritative for its domain. A key the document leaves out
//! holds the type's default and never the column the domain used to be read
//! from; the row is only read while the domain has no document at all. That
//! keeps a build which predates the documents working, and it is the handover
//! below that ends it.
//!
//! Every domain is handed over on its own: the start that finds no document for
//! it writes one from the rows, and a domain that already has one is left
//! alone. `.handed-over` records that one start saw every domain of that build
//! in place; it does not stand in for a domain, so a build that takes over
//! another domain still reaches an installation that ran an older one.
//!
//! A document carries the shape version it was written in. One a newer build
//! wrote is read as it stands and never written back, so a build that knows
//! less shows the values the user chose without lowering the document to the
//! shape it knows.
//!
//! `AXOLOTL_SETTINGS_CONFIG_DIR` names a directory the deployment provides —
//! a package manager such as Nix' home-manager, for instance — and which the
//! launcher only ever reads. Its documents are read before the local ones and
//! never written: only the local directory is. They sit directly in that
//! directory rather than under a `settings` subdirectory, so the two layers are
//! two depths.
//!
//! The five documents beside the settings — `ai`, `translation`, `log_share`,
//! `java` and `login_proof` — are composed key by key from the layers that
//! exist for them, which `composed` below does. Unlike the settings they are
//! read as a domain at a time rather than as one set of keys, but the order is
//! the same: the row while no local document holds the domain, the
//! deployment's document, the local one.

use std::future::Future;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};
use sqlx::SqlitePool;

use crate::state::State;

pub(crate) mod ai;
pub(crate) mod java;
pub(crate) mod log_share;
pub(crate) mod login_proof;
pub(crate) mod settings;
pub(crate) mod translation;

pub(crate) const DIR_NAME: &str = "settings";
const HANDOVER_MARKER: &str = ".handed-over";

/// The variable the deployment names the directory of documents it provides
/// in.
const CONFIG_DIR_ENV: &str = "AXOLOTL_SETTINGS_CONFIG_DIR";

/// The largest a settings document may be. A document is a few dozen keys, so
/// a megabyte is far past a real one; the cap is here because that directory
/// belongs to the deployment, and a path in it that is not a document — a
/// device, a FIFO, a file with no end — would otherwise be read into memory or
/// never finish reading, at every start.
const MAX_DOCUMENT_BYTES: u64 = 1 << 20;

/// Serializes the writers, which each read a document before writing it back:
/// two settings changing at once would otherwise lose one of them.
static WRITES: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// The directory a deployment ships its own documents in, when the environment
/// names one. An empty value is how a shell clears a variable rather than a
/// directory, so it reads as no deployment at all.
pub(crate) fn config_root() -> Option<PathBuf> {
    #[cfg(test)]
    if let Some(root) = TEST_CONFIG_ROOT.with(|root| root.borrow().clone()) {
        return Some(root);
    }
    config_root_from(std::env::var_os(CONFIG_DIR_ENV))
}

#[cfg(test)]
thread_local! {
    /// The deployment directory one test gives itself. The variable names one
    /// for the whole process, and the tests of a crate share it, so a test that
    /// set it would decide what every test running beside it deploys.
    static TEST_CONFIG_ROOT: std::cell::RefCell<Option<PathBuf>> =
        const { std::cell::RefCell::new(None) };
}

fn config_root_from(value: Option<std::ffi::OsString>) -> Option<PathBuf> {
    value.filter(|dir| !dir.is_empty()).map(PathBuf::from)
}

/// Where the documents of one launch live, and what the fallback reads.
pub(crate) struct Documents<'a> {
    root: PathBuf,
    /// The documents a deployment ships. Only the reads reach it: every write
    /// lands in `root`, so a deployment's document is never touched here.
    config_root: Option<PathBuf>,
    pool: &'a SqlitePool,
}

impl<'a> Documents<'a> {
    pub(crate) fn new(state: &'a State) -> Self {
        Self {
            root: state.directories.settings_dir.join(DIR_NAME),
            config_root: config_root(),
            pool: &state.pool,
        }
    }

    /// A launcher whose state is not built yet: the directory move rewrites
    /// what the documents carry before `State` exists.
    pub(crate) fn at(root: PathBuf, pool: &'a SqlitePool) -> Self {
        Self {
            root,
            config_root: None,
            pool,
        }
    }

    /// The same documents, with a deployment's own directory to read beside
    /// them. Nothing passes it to a write: a deployment owns that directory.
    pub(crate) fn with_config_root(
        mut self,
        config_root: Option<PathBuf>,
    ) -> Self {
        self.config_root = config_root;
        self
    }

    /// The documents of this launch, read with the ones a deployment ships
    /// beside them, and nothing while no start has named their directory.
    ///
    /// The settings are composed from these, and the callers that compose them
    /// hold a handle that knows nothing of documents: this is where a handle
    /// reaches them.
    pub(crate) fn of_launch(pool: &'a SqlitePool) -> Option<Self> {
        let root = settings::root()?;
        Some(Self::at(root, pool).with_config_root(config_root()))
    }

    pub(crate) fn path(&self, name: &str) -> PathBuf {
        self.root.join(format!("{name}.json"))
    }

    fn config_path(&self, name: &str) -> Option<PathBuf> {
        self.config_root
            .as_ref()
            .map(|root| root.join(format!("{name}.json")))
    }

    /// Whether a deployment ships documents, which is the only case a local
    /// copy of a provided value can go on shadowing a later change of it.
    pub(crate) fn ships_documents(&self) -> bool {
        self.config_root.is_some()
    }

    pub(crate) fn pool(&self) -> &'a SqlitePool {
        self.pool
    }

    /// Reads one document, and tells apart the one this build can write from
    /// the one a newer build wrote. The version the caller knows is what the
    /// comparison is made against, so the answer is the domain's own.
    pub(crate) async fn stored<T>(
        &self,
        name: &str,
        schema_version: u32,
    ) -> Stored<T>
    where
        T: DeserializeOwned + Default,
    {
        read_document(&self.path(name), name, schema_version).await
    }

    /// The document a deployment ships for a domain, when it ships one, and
    /// nothing when it has no directory or no file for it. A document of a
    /// shape this build does not know is still read: the values it carries are
    /// the deployment's answer for its keys, and nothing here writes it back.
    pub(crate) async fn shipped<T>(
        &self,
        name: &str,
        schema_version: u32,
    ) -> Option<T>
    where
        T: DeserializeOwned + Default,
    {
        let path = self.config_path(name)?;
        match read_document(&path, name, schema_version).await {
            Stored::Ready { document } | Stored::Newer { document } => {
                Some(document)
            }
            Stored::Missing | Stored::Unreadable => None,
        }
    }

    /// The local document as the keys it names, whichever build wrote it, or
    /// nothing while it holds no readable one.
    ///
    /// A layer contributes its keys to the domain, so it is read as a map
    /// rather than as the domain's type: a value this build cannot read then
    /// costs that key, which `overlay` sorts out, instead of the document. The
    /// version a newer build wrote does not stop the read either — those keys
    /// are still what the domain was last set to, and the write that would lower
    /// them is refused elsewhere.
    pub(crate) async fn stored_keys(
        &self,
        name: &str,
        schema_version: u32,
    ) -> Option<Map<String, Value>> {
        match read_document::<Map<String, Value>>(
            &self.path(name),
            name,
            schema_version,
        )
        .await
        {
            Stored::Ready { document } | Stored::Newer { document } => {
                Some(document)
            }
            Stored::Missing | Stored::Unreadable => None,
        }
    }

    /// The keys the deployment ships for a domain, which is not the same as the
    /// deployment shipping no document for it: one that cannot be read answers
    /// nothing either, but the keys it holds are unknown rather than absent,
    /// and a writer that took the two for one would copy them into the local
    /// document, where they would be the last word for good.
    pub(crate) async fn shipped_keys(
        &self,
        name: &str,
        schema_version: u32,
    ) -> Shipped {
        let Some(path) = self.config_path(name) else {
            return Shipped::Missing;
        };
        match read_document::<Map<String, Value>>(&path, name, schema_version)
            .await
        {
            Stored::Ready { document } | Stored::Newer { document } => {
                Shipped::Keys(document)
            }
            Stored::Missing => Shipped::Missing,
            Stored::Unreadable => Shipped::Unreadable,
        }
    }

    /// Replaces a document with a temporary file and a rename, so a reader
    /// never sees half of one. A write that would change no value, or that
    /// would lower a document a newer build wrote, leaves the file as it is.
    pub(crate) async fn write<T>(
        &self,
        name: &str,
        schema_version: u32,
        document: &T,
    ) -> crate::Result<()>
    where
        T: Serialize,
    {
        let path = self.path(name);
        // `WRITES` only serializes this process, so the file is read again
        // here: a second instance, or a build that took longer than the
        // caller's own read, can have replaced it meanwhile.
        if let Some(existing) = written_document(&path).await {
            // The version records the last write of this document, so a write
            // that leaves the data as it stands must not touch it: raising it
            // for a value the user did not change would lock a build that
            // knows the same shape out of the document from then on.
            if serde_json::to_value(document)? == existing.data {
                return Ok(());
            }
            // The same rule `stored` applies to a read applies to the write
            // that follows it, whatever the caller checked before: writing a
            // document a newer build wrote would lower it to the keys this
            // build knows.
            if existing.schema_version > schema_version {
                tracing::warn!(
                    path = %path.display(),
                    schema_version = existing.schema_version,
                    "Leaving a settings document a newer build wrote alone"
                );
                return Ok(());
            }
        }
        let contents = serde_json::to_vec_pretty(&NewDocument {
            schema_version,
            data: document,
        })?;
        if let Some(parent) = path.parent() {
            crate::util::io::create_dir_all(parent).await?;
        }
        crate::util::io::write(&path, contents).await?;
        Ok(())
    }
}

/// The document one domain reads as, composed of the layers a document set
/// holds for it.
///
/// The base is what `from_rows` recovers while no local document holds the
/// domain, and the domain's own default while one does. A local document is
/// written as what differs from the default — `write` leaves a value that is
/// the default out — so a key it omits holds the default and never the row:
/// reading the row under a local document would bring back a value the user has
/// already moved away from, which no write could ever undo.
///
/// The deployment's document sits above that base and below the local one, so a
/// value it pins is not shadowed by a row frozen before the deployment existed,
/// and the user's own document is the last word.
pub(super) async fn composed<T>(
    documents: &Documents<'_>,
    name: &str,
    schema_version: u32,
    from_rows: impl Future<Output = crate::Result<T>>,
) -> crate::Result<T>
where
    T: Default + Serialize + DeserializeOwned,
{
    let local = documents.stored_keys(name, schema_version).await;
    let base = match local {
        Some(_) => T::default(),
        None => from_rows.await?,
    };
    let mut values = keys_of(&base)?;
    if let Shipped::Keys(shipped) =
        documents.shipped_keys(name, schema_version).await
    {
        overlay::<T>(&mut values, &shipped, name, "deployment");
    }
    if let Some(local) = local {
        overlay::<T>(&mut values, &local, name, "local");
    }
    Ok(serde_json::from_value(Value::Object(values))?)
}

/// The document a local write starts from, with the keys the deployment already
/// answers left out, or nothing when nothing may be written.
///
/// A local document is the last word for every key it carries, so a copy of a
/// value the deployment provides would shadow it: the file would go on answering
/// that key after the deployment changed its own, and the change would never
/// arrive. Leaving those keys out is what keeps the deployment's answer visible
/// until the user changes that key, which the write that changes it states.
///
/// A deployment document this build cannot read says nothing about which keys it
/// answers, so the draft is not writable either: a copy of one of them would be
/// just as final once the file became readable again. The write is left for a
/// later one that can read it, which is what `None` tells the caller.
pub(super) async fn without_shipped<T>(
    documents: &Documents<'_>,
    name: &str,
    schema_version: u32,
    draft: T,
) -> Option<T>
where
    T: Serialize + DeserializeOwned,
{
    let shipped = match documents.shipped_keys(name, schema_version).await {
        Shipped::Keys(shipped) => shipped,
        Shipped::Missing => return Some(draft),
        Shipped::Unreadable => {
            tracing::warn!(
                document = name,
                path = %documents.path(name).display(),
                "Not writing a settings document: the deployment's own cannot be read, so the keys it answers are unknown"
            );
            return None;
        }
    };
    let Ok(Value::Object(values)) = serde_json::to_value(&draft) else {
        return Some(draft);
    };
    let kept: Map<String, Value> = values
        .into_iter()
        .filter(|(key, _)| !shipped.contains_key(key))
        .collect();
    Some(serde_json::from_value(Value::Object(kept)).unwrap_or_else(|error| {
        tracing::warn!(
            document = name,
            %error,
            "Could not read back a settings document written without the deployment's keys"
        );
        draft
    }))
}

/// The keys a domain's type carries, which is what the layers are laid over.
fn keys_of<T: Serialize>(document: &T) -> crate::Result<Map<String, Value>> {
    match serde_json::to_value(document)? {
        Value::Object(keys) => Ok(keys),
        // A domain composed key by key is one whose type is an object; a type
        // of any other shape has no keys for a layer to answer.
        other => Err(crate::ErrorKind::OtherError(format!(
            "a settings document is not a set of keys: {other}"
        ))
        .into()),
    }
}

/// Lays one layer over `values`, one key at a time: the keys the layer names are
/// the whitelist, and each is judged by reading the whole domain back, so a
/// value this build cannot read costs only that key and leaves what the layer
/// below made of it.
///
/// A key the domain's type does not carry is kept as the layer wrote it and
/// ignored by the read back, which is what lets a document from another shape
/// still answer with the keys this build knows.
fn overlay<T>(
    values: &mut Map<String, Value>,
    source: &Map<String, Value>,
    name: &str,
    layer: &str,
) where
    T: DeserializeOwned,
{
    for (key, value) in source {
        let kept = values.insert(key.clone(), value.clone());
        let Err(error) =
            serde_json::from_value::<T>(Value::Object(values.clone()))
        else {
            continue;
        };
        match kept {
            Some(kept) => values.insert(key.clone(), kept),
            None => values.remove(key),
        };
        tracing::warn!(
            document = name,
            layer,
            key,
            %error,
            "Ignoring a settings value a document carries that this build cannot read"
        );
    }
}

/// What the document at `path` holds, whichever layer it belongs to.
async fn read_document<T>(
    path: &Path,
    name: &str,
    schema_version: u32,
) -> Stored<T>
where
    T: DeserializeOwned + Default,
{
    // The shape is checked before anything is read: reading first and judging
    // afterwards would already have paid the cost the check exists to avoid.
    // `std::fs::metadata` follows a symbolic link, so a document a package
    // manager links into a store is judged by the file it points at, which is
    // the only thing the read would reach either.
    match std::fs::metadata(path) {
        Ok(metadata)
            if !metadata.is_file() || metadata.len() > MAX_DOCUMENT_BYTES =>
        {
            tracing::warn!(
                path = %path.display(),
                len = metadata.len(),
                "Ignoring a settings document that is not a regular file of a bounded size"
            );
            return Stored::Unreadable;
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Stored::Missing;
        }
        Err(error) => {
            tracing::warn!(
                path = %path.display(),
                %error,
                "Could not inspect a settings document"
            );
            return Stored::Unreadable;
        }
    }
    let contents = match tokio::fs::read(path).await {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Stored::Missing;
        }
        Err(error) => {
            tracing::warn!(
                path = %path.display(),
                %error,
                "Could not read a settings document"
            );
            return Stored::Unreadable;
        }
    };
    let Ok(stored) = serde_json::from_slice::<StoredDocument<T>>(&contents)
    else {
        tracing::warn!(
            path = %path.display(),
            "Ignoring a settings document that cannot be read"
        );
        return Stored::Unreadable;
    };
    if stored.schema_version > schema_version {
        tracing::debug!(
            name,
            schema_version = stored.schema_version,
            "Found a settings document a newer build wrote"
        );
        return Stored::Newer {
            document: stored.data,
        };
    }
    Stored::Ready {
        document: stored.data,
    }
}

/// What a document on disk already holds, when there is a file there that can
/// be parsed. Nothing means the write has nothing to compare against and goes
/// ahead: a file this build cannot interpret is not one it can lower.
async fn written_document(path: &Path) -> Option<WrittenDocument> {
    let contents = tokio::fs::read(path).await.ok()?;
    serde_json::from_slice(&contents).ok()
}

/// What a deployment's directory holds for one domain.
///
/// `Missing` and `Unreadable` are not the same answer: a document this build
/// cannot read names keys it cannot see, and a writer that took it for no
/// document at all would fill the local one with the values the deployment
/// answers.
pub(crate) enum Shipped {
    /// No document for the domain: the rows and the build's defaults answer it.
    Missing,
    /// A document whose keys are unknown, so nothing may be written for the
    /// domain until it can be read again.
    Unreadable,
    /// The keys the deployment answers, which go over the row and under the
    /// local document.
    Keys(Map<String, Value>),
}

pub(crate) enum Stored<T> {
    /// No document, so the domain is still answered by its rows.
    Missing,
    /// A document that cannot be read: it is left alone and answered by the
    /// rows, so a damaged file does not silently empty the settings.
    Unreadable,
    Ready {
        document: T,
    },
    /// A document a newer build wrote. It is still read, so a build that knows
    /// less keeps showing the values the user chose rather than what the rows
    /// froze; only a write is refused. The comparison that lands here happens in
    /// `stored`, so the variant carries no version of its own.
    Newer {
        document: T,
    },
}

impl<T> Stored<T> {
    /// Whether the domain has a document at all, whichever build wrote it: the
    /// handover only fills in a domain that has none.
    pub(crate) fn is_present(&self) -> bool {
        matches!(self, Self::Ready { .. } | Self::Newer { .. })
    }
}

#[derive(Serialize)]
struct NewDocument<'a, T: Serialize> {
    schema_version: u32,
    data: &'a T,
}

/// The two fields of a stored document a write has to look at before it lands.
/// They are parsed without the domain type, so a document this build cannot
/// read as its own still answers whether the write would change anything.
#[derive(serde::Deserialize)]
struct WrittenDocument {
    schema_version: u32,
    data: serde_json::Value,
}

#[derive(serde::Deserialize)]
struct StoredDocument<T> {
    schema_version: u32,
    /// No default: a document that leaves `data` out is not an empty
    /// authority but a file this build cannot interpret, so it reads as
    /// `Unreadable` and the rows keep answering the domain.
    data: T,
}

/// Turns every domain that has no document yet over to one, and records that
/// the handover is finished once all of them are in place. Reports whether the
/// handover is on record; one that fails part way is attempted again at the
/// next start.
///
/// A domain is asked every start rather than read off the marker, because a
/// build that takes over another domain has to reach the installations that
/// already carry a marker.
pub(crate) async fn hand_over(pool: &SqlitePool, settings_dir: &Path) -> bool {
    let documents = Documents {
        root: settings_dir.join(DIR_NAME),
        // The handover fills in what the rows hold, but not for a key the
        // deployment already answers: a local copy of one would shadow the
        // deployment's answer, so what it ships is read here as well.
        config_root: config_root(),
        pool,
    };
    let marker = documents.root.join(HANDOVER_MARKER);
    let ai_ready = ai::hand_over_if_missing(&documents).await;
    let log_share_ready = log_share::hand_over_if_missing(&documents).await;
    let translation_ready = translation::hand_over_if_missing(&documents).await;
    let login_proof_ready = login_proof::hand_over_if_missing(&documents).await;
    let java_ready = java::hand_over_if_missing(&documents).await;
    // The settings go last: their domains read a document of their own rather
    // than the rows, so the handover that fills them in is what ends the rows'
    // answer for them.
    let settings_ready = settings::hand_over_if_missing(&documents).await;
    if !ai_ready
        || !log_share_ready
        || !translation_ready
        || !login_proof_ready
        || !java_ready
        || !settings_ready
    {
        tracing::warn!(
            "The settings handover is incomplete; the next start retries it"
        );
        return false;
    }
    if tokio::fs::try_exists(&marker).await.unwrap_or(false) {
        return true;
    }
    let written = async {
        if let Some(parent) = marker.parent() {
            crate::util::io::create_dir_all(parent).await?;
        }
        crate::util::io::write(&marker, env!("CARGO_PKG_VERSION")).await
    }
    .await;
    match written {
        Ok(()) => {
            tracing::info!("Handed the settings rows over to documents");
            true
        }
        Err(error) => {
            tracing::warn!(
                path = %marker.display(),
                %error,
                "Failed to record the settings handover"
            );
            false
        }
    }
}

pub(super) fn default_true() -> bool {
    true
}

pub(super) fn is_true(value: &bool) -> bool {
    *value
}

pub(super) fn is_false(value: &bool) -> bool {
    !*value
}

#[cfg(test)]
pub(crate) mod test_support {
    use sqlx::SqlitePool;

    /// An empty in-memory database, opened where `sqlx` may be spelled: this
    /// module only lays the tables its own domains used to have over it.
    pub(crate) use crate::state::store::tests::support::empty_pool as pool;

    /// The pair of directories a launcher reads: the one it writes, and the one
    /// a deployment ships for it.
    pub(crate) fn layered<'a>(
        local: &tempfile::TempDir,
        deployment: &tempfile::TempDir,
        pool: &'a SqlitePool,
    ) -> super::Documents<'a> {
        super::Documents::at(local.path().to_path_buf(), pool)
            .with_config_root(Some(deployment.path().to_path_buf()))
    }

    /// Writes a document the deployment ships, in the shape a document carries.
    pub(crate) fn ship(dir: &tempfile::TempDir, name: &str, data: &str) {
        std::fs::write(
            dir.path().join(format!("{name}.json")),
            format!(r#"{{"schema_version":1,"data":{data}}}"#),
        )
        .expect("a deployment document");
    }

    /// Makes `dir` the directory a deployment ships its documents in, for the
    /// test running on this thread: what the variable does for a whole process.
    pub(crate) fn deploy(dir: &tempfile::TempDir) {
        super::TEST_CONFIG_ROOT
            .with(|root| *root.borrow_mut() = Some(dir.path().to_path_buf()));
    }

    /// The three AI tables, as the migrations leave them.
    pub(crate) async fn create_ai_tables(pool: &SqlitePool) {
        sqlx::query(
            "CREATE TABLE ai_settings (id INTEGER NOT NULL CHECK (id = 0), enabled INTEGER NOT NULL DEFAULT TRUE, legacy_openai_credential_cleanup INTEGER NOT NULL DEFAULT FALSE, PRIMARY KEY (id))",
        )
        .execute(pool)
        .await
        .expect("ai_settings table");
        sqlx::query("INSERT INTO ai_settings (id) VALUES (0)")
            .execute(pool)
            .await
            .expect("ai_settings row");
        sqlx::query(
            "CREATE TABLE ai_provider_configs (provider_id TEXT NOT NULL PRIMARY KEY, custom_name TEXT NULL, protocol TEXT NOT NULL DEFAULT 'openai', enabled INTEGER NOT NULL DEFAULT FALSE, endpoint TEXT NOT NULL DEFAULT '', settings TEXT NOT NULL DEFAULT '{}')",
        )
        .execute(pool)
        .await
        .expect("ai_provider_configs table");
        sqlx::query(
            "CREATE TABLE ai_provider_models (provider_id TEXT NOT NULL, model_id TEXT NOT NULL, display_name TEXT NOT NULL DEFAULT '', enabled INTEGER NOT NULL DEFAULT TRUE, source TEXT NOT NULL DEFAULT 'custom', PRIMARY KEY (provider_id, model_id))",
        )
        .execute(pool)
        .await
        .expect("ai_provider_models table");
    }

    /// The two log sharing tables, as the migrations leave them.
    pub(crate) async fn create_log_share_tables(pool: &SqlitePool) {
        sqlx::query(
            "CREATE TABLE log_share_settings (id INTEGER NOT NULL CHECK (id = 0), share_provider TEXT NOT NULL DEFAULT 'logshare', ai_source TEXT NOT NULL DEFAULT 'logshare', auto_upload INTEGER NOT NULL DEFAULT TRUE, multi_file INTEGER NOT NULL DEFAULT TRUE, metadata_enabled INTEGER NOT NULL DEFAULT TRUE, no_storage INTEGER NOT NULL DEFAULT FALSE, show_progress INTEGER NOT NULL DEFAULT TRUE, PRIMARY KEY (id))",
        )
        .execute(pool)
        .await
        .expect("log_share_settings table");
        sqlx::query("INSERT INTO log_share_settings (id) VALUES (0)")
            .execute(pool)
            .await
            .expect("log_share_settings row");
        sqlx::query(
            "CREATE TABLE crash_analysis_ai_settings (id INTEGER NOT NULL CHECK (id = 0), enabled INTEGER NOT NULL DEFAULT FALSE, provider_id TEXT NOT NULL DEFAULT '', model_id TEXT NOT NULL DEFAULT '', ai_source TEXT NOT NULL DEFAULT 'logshare', PRIMARY KEY (id))",
        )
        .execute(pool)
        .await
        .expect("crash_analysis_ai_settings table");
        sqlx::query("INSERT INTO crash_analysis_ai_settings (id) VALUES (0)")
            .execute(pool)
            .await
            .expect("crash_analysis_ai_settings row");
    }

    /// The translation settings row, as the migrations leave it.
    pub(crate) async fn create_translation_tables(pool: &SqlitePool) {
        sqlx::query(
            "CREATE TABLE translation_settings (id INTEGER NOT NULL CHECK (id = 0), provider TEXT NOT NULL DEFAULT 'google', target_language TEXT NOT NULL DEFAULT '', mode TEXT NOT NULL DEFAULT 'bilingual', auto_translate INTEGER NOT NULL DEFAULT FALSE, style TEXT NOT NULL DEFAULT 'weakened', openai_base_url TEXT NOT NULL DEFAULT 'https://api.openai.com/v1', openai_model TEXT NOT NULL DEFAULT 'gpt-4o-mini', openai_api_key TEXT NULL, deeplx_base_url TEXT NOT NULL DEFAULT 'https://api.deeplx.org/{{apiKey}}/translate', deeplx_api_key TEXT NULL, openai_system_prompt TEXT NOT NULL DEFAULT '', ai_provider_id TEXT NOT NULL DEFAULT '', ai_model_id TEXT NOT NULL DEFAULT '', deepl_api_endpoint TEXT NOT NULL DEFAULT 'https://api-free.deepl.com/v2/translate', deepl_api_key TEXT NULL, PRIMARY KEY (id))",
        )
        .execute(pool)
        .await
        .expect("translation_settings table");
        sqlx::query("INSERT INTO translation_settings (id) VALUES (0)")
            .execute(pool)
            .await
            .expect("translation_settings row");
    }

    /// The official login proof row, as the migrations leave it.
    pub(crate) async fn create_official_login_proof_table(pool: &SqlitePool) {
        sqlx::query(
            "CREATE TABLE official_login_proof (id INTEGER NOT NULL CHECK (id = 0), verified INTEGER NOT NULL DEFAULT FALSE, version INTEGER NOT NULL DEFAULT 1, mac TEXT NOT NULL, PRIMARY KEY (id))",
        )
        .execute(pool)
        .await
        .expect("official_login_proof table");
    }

    /// The two Java tables, as the migrations leave them, the default naming a
    /// runtime through the pair the foreign key covers.
    pub(crate) async fn create_java_tables(pool: &SqlitePool) {
        sqlx::query(
            "CREATE TABLE java_versions (major_version INTEGER NOT NULL, full_version TEXT NOT NULL, architecture TEXT NOT NULL, path TEXT NOT NULL PRIMARY KEY, distribution TEXT, UNIQUE (major_version, path))",
        )
        .execute(pool)
        .await
        .expect("java_versions table");
        sqlx::query(
            "CREATE UNIQUE INDEX java_versions_major_version_path
             ON java_versions(major_version, path)",
        )
        .execute(pool)
        .await
        .expect("java_versions major version path index");
        sqlx::query(
            "CREATE TABLE java_default_versions (major_version INTEGER NOT NULL PRIMARY KEY, path TEXT NOT NULL, FOREIGN KEY (major_version, path) REFERENCES java_versions(major_version, path) ON DELETE CASCADE ON UPDATE CASCADE)",
        )
        .execute(pool)
        .await
        .expect("java_default_versions table");
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::{
        create_ai_tables, create_java_tables, create_log_share_tables,
        create_official_login_proof_table, create_translation_tables, pool,
    };
    use super::{
        Documents, MAX_DOCUMENT_BYTES, ai, config_root_from, hand_over, java,
        log_share, login_proof, translation,
    };
    use crate::state::store::Collection;
    use crate::state::store::Handle;
    use crate::state::store::java_versions::{JavaVersionRecord, JavaVersions};
    use crate::state::store::official_login_proof::{
        Installation, OfficialLoginProof, OfficialLoginProofRecord,
    };

    /// A deployment's directory is its own file tree, so a path in it is not
    /// promised to be a document: a device or a FIFO would block the read
    /// forever or never end it, and a start would never come up. The shape is
    /// judged before the read, and the domain falls back the way it does for
    /// any other document that cannot be read.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_document_that_is_not_a_regular_file_is_unreadable() {
        // `/dev/zero` is the case a read cannot answer: it never ends one.
        if !std::path::Path::new("/dev/zero").exists() {
            eprintln!(
                "skipped: /dev/zero is not available here, so a device cannot stand in for a document"
            );
            return;
        }
        let pool = pool().await;
        let local = tempfile::tempdir().unwrap();
        let deployment = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(
            "/dev/zero",
            deployment.path().join("ai.json"),
        )
        .unwrap();

        let documents = Documents::at(local.path().to_path_buf(), &pool)
            .with_config_root(Some(deployment.path().to_path_buf()));
        let started = std::time::Instant::now();
        let shipped = documents.shipped::<serde_json::Value>("ai", 1).await;
        let elapsed = started.elapsed();

        assert!(
            shipped.is_none(),
            "a device is not a document, so the domain falls back to the local layers"
        );
        assert!(
            elapsed < std::time::Duration::from_secs(5),
            "a read of the device would not have ended, so the check has to come first"
        );
    }

    /// A package manager links a document rather than copying it, so a link is
    /// a document when the file it points at is one: the shape is judged after
    /// following it, which is where the read would go too.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_document_reached_through_a_symbolic_link_is_read() {
        let pool = pool().await;
        let local = tempfile::tempdir().unwrap();
        let deployment = tempfile::tempdir().unwrap();
        let target = deployment.path().join("in-a-store-appearance.json");
        std::fs::write(
            &target,
            r#"{"schema_version":1,"data":{"theme":"oled"}}"#,
        )
        .unwrap();
        std::os::unix::fs::symlink(
            &target,
            deployment.path().join("appearance.json"),
        )
        .unwrap();

        let documents = Documents::at(local.path().to_path_buf(), &pool)
            .with_config_root(Some(deployment.path().to_path_buf()));
        let shipped = documents
            .shipped::<serde_json::Value>("appearance", 1)
            .await
            .expect("a link to a document is the document it points at");

        assert_eq!(
            shipped["theme"],
            serde_json::Value::from("oled"),
            "the document behind the link is read as it stands"
        );
    }

    /// A document past the cap is refused by its size alone: the file here is
    /// valid JSON with trailing space, so the size is the only thing that can
    /// refuse it.
    #[tokio::test]
    async fn a_document_over_the_size_limit_is_unreadable() {
        let pool = pool().await;
        let root = tempfile::tempdir().unwrap();
        let documents = Documents::at(root.path().to_path_buf(), &pool);
        let path = documents.path("ai");
        let mut oversized =
            br#"{"schema_version":1,"data":{"enabled":false}}"#.to_vec();
        oversized.resize(MAX_DOCUMENT_BYTES as usize + 1, b' ');
        std::fs::write(&path, &oversized).unwrap();

        let stored = documents.stored::<serde_json::Value>("ai", 1).await;

        assert!(
            !stored.is_present(),
            "a document past the cap is unreadable rather than parsed"
        );
        assert_eq!(
            std::fs::read(&path).unwrap(),
            oversized,
            "a document that is too large to read is left as it stands"
        );
    }

    /// An empty variable is how a shell clears one, so a deployment that sets
    /// it to nothing must read the same as one that never set it at all: a
    /// directory named by nothing would make every domain look deployed.
    #[test]
    fn an_empty_config_dir_variable_is_the_same_as_unset() {
        let directory = std::path::PathBuf::from("/deploy/settings");

        assert_eq!(config_root_from(None), None);
        assert_eq!(config_root_from(Some(std::ffi::OsString::new())), None);
        assert_eq!(
            config_root_from(Some(directory.clone().into_os_string())),
            Some(directory)
        );
    }

    async fn installed_pool() -> sqlx::SqlitePool {
        let pool = pool().await;
        create_ai_tables(&pool).await;
        create_log_share_tables(&pool).await;
        create_translation_tables(&pool).await;
        create_official_login_proof_table(&pool).await;
        create_java_tables(&pool).await;
        pool
    }

    #[tokio::test]
    async fn a_handover_stops_the_rows_from_being_read() {
        let pool = installed_pool().await;
        let handle = Handle::new(pool.clone());
        // The display of three legacy rows is stated here because the store
        // only reads them; the proof and the runtimes are written through the
        // collections that own those rows.
        sqlx::query("UPDATE ai_settings SET enabled = FALSE")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE log_share_settings SET share_provider = 'mclogs'")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "UPDATE translation_settings SET target_language = 'zh-CN'",
        )
        .execute(&pool)
        .await
        .unwrap();
        OfficialLoginProof::set(
            &handle,
            &Installation,
            Some(&OfficialLoginProofRecord {
                mac: "stored-mac".to_string(),
            }),
        )
        .await
        .unwrap();
        let runtime = "/java/21/bin/java".to_string();
        JavaVersions::set(
            &handle,
            &runtime,
            Some(&JavaVersionRecord {
                major_version: 21,
                full_version: "21.0.1".to_string(),
                architecture: "x86_64".to_string(),
                distribution: None,
            }),
        )
        .await
        .unwrap();

        let settings_dir = tempfile::tempdir().unwrap();
        assert!(hand_over(&pool, settings_dir.path()).await);
        assert!(
            settings_dir
                .path()
                .join("settings")
                .join(".handed-over")
                .is_file()
        );

        let root = settings_dir.path().join("settings");
        let documents = Documents::at(root, &pool);
        assert!(!ai::read(&documents).await.unwrap().enabled);
        assert_eq!(
            log_share::read(&documents).await.unwrap().share_provider,
            "mclogs"
        );
        assert_eq!(
            translation::read(&documents).await.unwrap().target_language,
            "zh-CN"
        );
        assert_eq!(
            login_proof::read(&documents).await.unwrap().as_deref(),
            Some("stored-mac")
        );
        assert_eq!(java::read(&documents).await.unwrap().runtimes.len(), 1);

        sqlx::query("UPDATE ai_settings SET enabled = TRUE")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "UPDATE log_share_settings SET share_provider = 'logshare'",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("UPDATE translation_settings SET target_language = 'en'")
            .execute(&pool)
            .await
            .unwrap();
        OfficialLoginProof::set(&handle, &Installation, None)
            .await
            .unwrap();
        JavaVersions::set(&handle, &runtime, None).await.unwrap();

        assert!(!ai::read(&documents).await.unwrap().enabled);
        assert_eq!(
            log_share::read(&documents).await.unwrap().share_provider,
            "mclogs"
        );
        assert_eq!(
            translation::read(&documents).await.unwrap().target_language,
            "zh-CN"
        );
        assert_eq!(
            login_proof::read(&documents).await.unwrap().as_deref(),
            Some("stored-mac")
        );
        assert_eq!(java::read(&documents).await.unwrap().runtimes.len(), 1);
    }

    #[tokio::test]
    async fn a_marker_does_not_stand_in_for_a_domain_this_build_added() {
        let pool = installed_pool().await;
        sqlx::query(
            "UPDATE translation_settings SET target_language = 'zh-CN'",
        )
        .execute(&pool)
        .await
        .unwrap();

        let settings_dir = tempfile::tempdir().unwrap();
        let root = settings_dir.path().join("settings");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join(".handed-over"), "0.0.0").unwrap();

        assert!(hand_over(&pool, settings_dir.path()).await);

        let documents = Documents::at(root, &pool);
        assert!(
            documents.path(translation::DOCUMENT).is_file(),
            "a marker an older build left does not keep a domain this build took over on its rows"
        );
        assert_eq!(
            translation::read(&documents).await.unwrap().target_language,
            "zh-CN"
        );
    }

    #[tokio::test]
    async fn a_handover_leaves_a_document_that_exists_alone() {
        let pool = installed_pool().await;
        sqlx::query("UPDATE ai_settings SET enabled = FALSE")
            .execute(&pool)
            .await
            .unwrap();

        let settings_dir = tempfile::tempdir().unwrap();
        let root = settings_dir.path().join("settings");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("ai.json"),
            r#"{"schema_version":1,"data":{}}"#,
        )
        .unwrap();

        assert!(hand_over(&pool, settings_dir.path()).await);

        let documents = Documents::at(root, &pool);
        assert!(
            ai::read(&documents).await.unwrap().enabled,
            "an existing document is authoritative, so the handover must not rewrite it"
        );
    }

    #[tokio::test]
    async fn a_write_leaves_a_document_a_newer_build_wrote() {
        let pool = pool().await;
        let settings_dir = tempfile::tempdir().unwrap();
        let root = settings_dir.path().join("settings");
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("java.json");
        let newer = br#"{"schema_version":99,"data":{"runtimes":[{"path":"/old/java/bin/java","major_version":21,"full_version":"21.0.1"}]}}"#;
        std::fs::write(&path, newer).unwrap();

        // The move reads the newer document and writes it back without asking
        // for its version again, which is the race `write` has to answer for.
        let documents = Documents::at(root, &pool);
        java::move_installations(&documents, "/old", "/new", |_| false)
            .await
            .unwrap();

        assert_eq!(
            std::fs::read(&path).unwrap().as_slice(),
            newer.as_slice(),
            "a write must leave a document a newer build wrote byte for byte"
        );
    }

    #[tokio::test]
    async fn a_write_that_changes_nothing_leaves_the_document_alone() {
        let pool = pool().await;
        let settings_dir = tempfile::tempdir().unwrap();
        let root = settings_dir.path().join("settings");
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("ai.json");
        // Written the way another build left it, so a rewrite would show as
        // different bytes although it carried the same values.
        let untouched = br#"{"data":{},"schema_version":1}"#;
        std::fs::write(&path, untouched).unwrap();

        let documents = Documents::at(root, &pool);
        ai::set_enabled(&documents, true).await.unwrap();

        assert_eq!(
            std::fs::read(&path).unwrap().as_slice(),
            untouched.as_slice(),
            "a write that changes no value must leave the file and its version as they stand"
        );
    }
}

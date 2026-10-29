//! The account secrets, kept in the system credential store instead of the rows
//! that name them.
//!
//! A row keeps a mark of which of its secret columns the store holds; the column
//! itself is cleared once the secret is there. Every secret the store will not
//! take — this host has none, or the value is longer than one entry holds — stays
//! in its column with its mark clear, so signing in never depends on the store
//! answering: the row is then read exactly as an older build wrote it.
//!
//! A row whose mark does depend on the store, though, is not read without it: a
//! secret that cannot be read cannot be hidden from an exported report either, so
//! a store that does not answer fails the read instead of emptying the secret.
//!
//! This module is the only way the crate reaches the store.

use std::cell::Cell;
use std::path::Path;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use super::Handle;

/// How much one entry holds, in the unit the platform's cap counts.
///
/// The Windows credential store caps a blob at 2560 bytes and keeps passwords as
/// UTF-16, so an entry holds at most 1280 code units. The Secret Service on Linux
/// answers "password is too long" above 8192 bytes and stores a shortened value
/// instead of refusing, so a secret it is given must stay well under that. macOS
/// has no cap worth naming; it is held to the Linux one, which is measured.
const MOST: usize = if cfg!(windows) { 1280 } else { 8192 };

/// What every key the launcher stores starts with.
const PREFIX: &str = "axolotl-account";

/// The name this run's entries are stored under, once the data directory it
/// belongs to is known.
static NAMESPACE: OnceLock<String> = OnceLock::new();

/// What the store answered about one key.
pub(crate) enum Stored {
    /// The secret the key names.
    Value(String),
    /// The store answered and holds nothing under the key.
    Missing,
    /// No store answered, so nothing was read.
    Unavailable,
}

thread_local! {
    /// Whether this thread already found no credential store to work with, which
    /// is as far as it needs to look: from then on the rows keep their own
    /// secrets.
    static NO_STORE: Cell<bool> = const { Cell::new(false) };
}

/// Whether the run has already said that this host has no credential store.
static REPORTED: AtomicBool = AtomicBool::new(false);

/// Whether this thread has no credential store the launcher can use.
pub(crate) fn unavailable() -> bool {
    NO_STORE.with(Cell::get)
}

/// Names this run's entries after the data directory they belong to.
///
/// A run told to use another data directory — a development build pointed at a
/// directory of its own — shares nothing with the one the identifier alone
/// names, so the two cannot read each other's accounts.
///
/// Called once at startup, before the first secret is read or written. Until it
/// is, and in a test build, the identifier alone answers.
pub(crate) fn identify(directory: &Path) {
    let _ = NAMESPACE.set(namespace_of(directory));
}

/// The store name one data directory's secrets are kept under.
///
/// The digest is what separates the two directories: the path itself is not in
/// the name, which every process on the host can list.
fn namespace_of(directory: &Path) -> String {
    format!("{}-{:016x}", base_identifier(), fingerprint(directory))
}

/// The name a release resolves to.
fn base_identifier() -> String {
    crate::brand::app_data_dir_identifier(crate::brand::BUNDLE_IDENTIFIER)
}

/// A short, stable digest of one path.
///
/// FNV-1a, spelled out: the standard hasher is free to change between Rust
/// releases, and a name that changed with the compiler would strand every secret
/// the previous build stored.
fn fingerprint(directory: &Path) -> u64 {
    let mut digest: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in directory.to_string_lossy().bytes() {
        digest ^= u64::from(byte);
        digest = digest.wrapping_mul(0x0000_0100_0000_01b3);
    }
    digest
}

/// The key one row's secret is stored under.
pub(crate) fn key(parts: &[&str]) -> String {
    let mut key = String::from(PREFIX);
    for part in parts {
        key.push(':');
        key.push_str(part);
    }
    key
}

/// Stores one secret, and says whether the store holds it now.
pub(crate) async fn keep(key: String, value: &str) -> bool {
    if unavailable() {
        return false;
    }

    if size(value) >= MOST {
        tracing::warn!(
            %key,
            bytes = value.len(),
            "A secret is longer than one credential store entry holds, so it stays in the database"
        );
        return false;
    }

    match backend().set(&key, value) {
        Ok(()) => true,
        Err(refusal) => {
            report(&key, refusal);
            false
        }
    }
}

/// How much of one entry `value` spends.
#[cfg(windows)]
fn size(value: &str) -> usize {
    value.encode_utf16().count()
}

#[cfg(not(windows))]
fn size(value: &str) -> usize {
    value.len()
}

/// The secret one key names, when the store still has it.
pub(crate) async fn take(key: String) -> Stored {
    if unavailable() {
        return Stored::Unavailable;
    }

    match backend().get(&key) {
        Ok(Some(value)) => Stored::Value(value),
        Ok(None) => Stored::Missing,
        Err(refusal) => {
            report(&key, refusal);
            Stored::Unavailable
        }
    }
}

/// Drops whatever one key names, which a row that no longer has the secret needs.
///
/// The value the entry held comes back with it, so a caller whose own statement
/// then fails can put it back. `Err` means the entry is still there.
pub(crate) async fn forget(key: String) -> crate::Result<Option<String>> {
    if unavailable() {
        return Ok(None);
    }

    let held = match backend().get(&key) {
        Ok(held) => held,
        Err(refusal) => {
            report(&key, refusal);
            return Err(refused_error(&key));
        }
    };

    match backend().delete(&key) {
        Ok(()) => Ok(held),
        Err(refusal) => {
            report(&key, refusal);
            Err(refused_error(&key))
        }
    }
}

/// The secret one mark says the store holds.
///
/// An empty answer — the store holds nothing under the key — reads as an empty
/// secret, which asks for a new sign-in rather than failing. A store that does
/// not answer at all is an error: an emptied secret reads as a signed-out
/// account, and the export that hides secrets would have nothing left to hide.
pub(crate) async fn resolve(key: String) -> crate::Result<String> {
    match take(key).await {
        Stored::Value(value) => Ok(value),
        Stored::Missing => Ok(String::new()),
        Stored::Unavailable => Err(unreadable()),
    }
}

/// The failure a mark that names the store answers with when the store does not.
pub(crate) fn unreadable() -> crate::Error {
    crate::ErrorKind::OtherError(
        "The credential store is not answering; the account cannot be read right now"
            .to_string(),
    )
    .into()
}

/// The failure a secret the store will not drop answers with.
fn refused_error(key: &str) -> crate::Error {
    crate::ErrorKind::OtherError(format!(
        "The credential store would not drop the secret of {key}; nothing was removed, so it can be removed again"
    ))
    .into()
}

/// The store as one write found it: what each key held, so a write whose own
/// statement then fails leaves the store the way it was.
#[derive(Default)]
pub(crate) struct Undo(Vec<(String, Option<String>)>);

impl Undo {
    /// Reads what one key holds now, before the write that is about to change it.
    pub(crate) async fn watch(&mut self, key: &str) {
        let held = match take(key.to_string()).await {
            Stored::Value(value) => Some(value),
            Stored::Missing | Stored::Unavailable => None,
        };
        self.0.push((key.to_string(), held));
    }

    /// Drops one entry, because nothing will name it afterwards, and keeps what
    /// it held.
    ///
    /// `required` is set for an entry the row's mark says the store holds: a
    /// store that will not drop it then fails the caller's write, so a row is
    /// never deleted out from under a secret that stays in the store.
    pub(crate) async fn drop(
        &mut self,
        key: &str,
        required: bool,
    ) -> crate::Result<()> {
        if required && unavailable() {
            return Err(refused_error(key));
        }

        match forget(key.to_string()).await {
            Ok(held) => {
                self.0.push((key.to_string(), held));
                Ok(())
            }
            Err(error) if required => Err(error),
            Err(error) => {
                tracing::warn!(
                    %key,
                    %error,
                    "The credential store would not drop an entry the row no longer names"
                );
                Ok(())
            }
        }
    }

    /// Puts the store back the way it was, undoing the latest change first.
    pub(crate) async fn restore(self) {
        for (key, held) in self.0.into_iter().rev() {
            let back = match held {
                Some(value) => keep(key.clone(), &value).await,
                None => forget(key.clone()).await.is_ok(),
            };
            if !back {
                tracing::error!(
                    %key,
                    "The credential store would not take a secret back after a write that failed"
                );
            }
        }
    }
}

/// Moves the secrets the account rows still carry into the store, and clears each
/// column once its secret is there.
///
/// Called at startup. It is idempotent — a row that was moved has nothing left in
/// its columns — and every step writes the store before the row, so an
/// interruption leaves a secret in both places rather than in neither.
pub(crate) async fn externalize(handle: &Handle) -> crate::Result<()> {
    let mut connection = handle.connection().await?;

    super::minecraft_users::externalize(&mut connection).await?;
    super::modrinth_users::externalize(&mut connection).await?;
    super::minecraft_device_tokens::externalize(&mut connection).await?;

    Ok(())
}

/// A system credential store.
///
/// The launcher reaches no other one: the backend here is the platform's own, and
/// only a test build substitutes another.
pub(crate) trait Backend: Send + Sync {
    fn set(&self, key: &str, value: &str) -> Result<(), Refused>;

    /// The value under `key`, or nothing when the store holds none.
    fn get(&self, key: &str) -> Result<Option<String>, Refused>;

    fn delete(&self, key: &str) -> Result<(), Refused>;
}

/// Why a store would not take part, in the words the platform gave.
pub(crate) struct Refused(String);

/// The platform's credential store.
#[cfg(not(test))]
struct System;

#[cfg(not(test))]
impl Backend for System {
    fn set(&self, key: &str, value: &str) -> Result<(), Refused> {
        entry(key)?.set_password(value).map_err(refused)
    }

    fn get(&self, key: &str) -> Result<Option<String>, Refused> {
        match entry(key)?.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(refused(error)),
        }
    }

    fn delete(&self, key: &str) -> Result<(), Refused> {
        match entry(key)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(refused(error)),
        }
    }
}

#[cfg(not(test))]
fn entry(key: &str) -> Result<keyring::Entry, Refused> {
    // The data directory the run keeps its rows in is part of the name, so two
    // directories started from one build keep their secrets apart.
    keyring::Entry::new(&namespace(), key).map_err(refused)
}

/// The name this run's entries are stored under.
#[cfg(not(test))]
fn namespace() -> String {
    NAMESPACE.get().cloned().unwrap_or_else(base_identifier)
}

#[cfg(not(test))]
fn refused(error: keyring::Error) -> Refused {
    Refused(error.to_string())
}

/// Reports a refusal once: a host without a credential store refuses every
/// secret, and the run needs to say so once rather than once per token.
fn report(key: &str, Refused(reason): Refused) {
    NO_STORE.with(|flag| flag.set(true));
    if !REPORTED.swap(true, Ordering::Relaxed) {
        tracing::warn!(
            %key,
            %reason,
            "The system credential store is unavailable, so account secrets stay in the database"
        );
    }
}

#[cfg(test)]
fn backend() -> &'static dyn Backend {
    testing::backend()
}

#[cfg(not(test))]
fn backend() -> &'static dyn Backend {
    &System
}

/// The stores a test may put in place of the platform's own.
///
/// A test build reaches no real credential store: without a substitute every call
/// is refused, which is also what a host without one answers. The substitute is
/// per-thread, so a test that puts one in place leaves the tests running beside it
/// on that default.
#[cfg(test)]
pub(crate) mod testing {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::{Backend, NO_STORE, Refused};

    /// A store that holds its secrets in memory.
    #[derive(Default)]
    pub(crate) struct Memory {
        values: Mutex<HashMap<String, String>>,
        refuses_deletes: AtomicBool,
    }

    impl Memory {
        /// The secret under one key, for a test that has to look behind a read.
        pub(crate) fn value(&self, key: &str) -> Option<String> {
            self.lock().get(key).cloned()
        }

        /// Writes a secret the way another build would have left it.
        pub(crate) fn put(&self, key: &str, value: &str) {
            self.lock().insert(key.to_string(), value.to_string());
        }

        /// Every key the store holds, so a test can see what a move left behind.
        pub(crate) fn keys(&self) -> Vec<String> {
            self.lock().keys().cloned().collect()
        }

        /// Refuses to drop an entry, the way a keyring asking for money or for a
        /// screen that is not there does.
        pub(crate) fn refuse_deletes(&self) {
            self.refuses_deletes.store(true, Ordering::Relaxed);
        }

        fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, String>> {
            self.values
                .lock()
                .unwrap_or_else(|error| error.into_inner())
        }
    }

    impl Backend for Memory {
        fn set(&self, key: &str, value: &str) -> Result<(), Refused> {
            self.put(key, value);
            Ok(())
        }

        fn get(&self, key: &str) -> Result<Option<String>, Refused> {
            Ok(self.value(key))
        }

        fn delete(&self, key: &str) -> Result<(), Refused> {
            if self.refuses_deletes.load(Ordering::Relaxed) {
                return Err(Refused("the keyring is locked".to_string()));
            }
            self.lock().remove(key);
            Ok(())
        }
    }

    /// A store that refuses every call, the way a host without one does.
    pub(crate) struct Refusing;

    impl Backend for Refusing {
        fn set(&self, _: &str, _: &str) -> Result<(), Refused> {
            Err(Refused("no credential store".to_string()))
        }

        fn get(&self, _: &str) -> Result<Option<String>, Refused> {
            Err(Refused("no credential store".to_string()))
        }

        fn delete(&self, _: &str) -> Result<(), Refused> {
            Err(Refused("no credential store".to_string()))
        }
    }

    static REFUSING: Refusing = Refusing;

    thread_local! {
        /// The substitute this thread put in place, if any.
        static SUBSTITUTE: RefCell<Option<&'static (dyn Backend)>> =
            const { RefCell::new(None) };
    }

    pub(super) fn backend() -> &'static dyn Backend {
        SUBSTITUTE
            .with_borrow(|substitute| *substitute)
            .unwrap_or(&REFUSING)
    }

    /// A substitute in place on this thread for as long as the guard lives.
    pub(crate) struct Injection {
        memory: Option<&'static Memory>,
    }

    impl Injection {
        /// The store this injection put in place; no store means the host has
        /// none, which is what a test build starts from.
        pub(crate) fn store(&self) -> Option<&'static Memory> {
            self.memory
        }
    }

    /// Leaves this thread without a credential store.
    pub(crate) fn without_store() -> Injection {
        install(None);
        Injection { memory: None }
    }

    /// Puts a store that answers in place, which the test then reads back.
    pub(crate) fn with_store() -> Injection {
        let memory: &'static Memory = Box::leak(Box::new(Memory::default()));
        install(Some(memory));
        Injection {
            memory: Some(memory),
        }
    }

    /// The verdict on the store starts over with every substitute: the refusal a
    /// previous test in this thread saw must not silence this one.
    fn install(memory: Option<&'static Memory>) {
        NO_STORE.with(|flag| flag.set(false));
        let substitute: Option<&'static dyn Backend> = match memory {
            Some(memory) => Some(memory),
            None => None,
        };
        SUBSTITUTE.with_borrow_mut(|slot| *slot = substitute);
    }

    impl Drop for Injection {
        fn drop(&mut self) {
            install(None);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{base_identifier, namespace_of};

    /// Two data directories never share the entries that hold their secrets, and
    /// neither puts its own path into the name.
    #[test]
    fn a_data_directory_names_its_own_entries() {
        let release = Path::new("/home/player/.local/share/axolotl");
        let branch = Path::new("/home/player/.local/share/axolotl-branch");

        assert_eq!(namespace_of(release), namespace_of(release));
        assert_ne!(namespace_of(release), namespace_of(branch));
        assert!(
            namespace_of(release).starts_with(&base_identifier()),
            "the identifier stays in front, so the service is recognisable"
        );
        for directory in [release, branch] {
            let name = namespace_of(directory);
            assert!(
                !name.contains("player") && !name.contains('/'),
                "the name says nothing about where the directory is: {name}"
            );
        }
    }
}

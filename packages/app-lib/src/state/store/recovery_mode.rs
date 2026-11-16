//! Explicit read-only recovery mode. No damaged database is rewritten.
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
fn roots() -> &'static Mutex<
    HashMap<usize, (Arc<sqlx::sqlite::SqliteConnectOptions>, PathBuf)>,
> {
    static ROOTS: OnceLock<
        Mutex<
            HashMap<usize, (Arc<sqlx::sqlite::SqliteConnectOptions>, PathBuf)>,
        >,
    > = OnceLock::new();
    ROOTS.get_or_init(Mutex::default)
}
pub(crate) fn register(pool: &SqlitePool, root: &Path) {
    let options = pool.connect_options();
    roots()
        .lock()
        .expect("recovery roots")
        .insert(Arc::as_ptr(&options) as usize, (options, root.to_owned()));
}
pub(crate) fn root(pool: &SqlitePool) -> Option<PathBuf> {
    let options = pool.connect_options();
    roots()
        .lock()
        .expect("recovery roots")
        .get(&(Arc::as_ptr(&options) as usize))
        .map(|(_, root)| root.clone())
}
pub(crate) fn ensure_writable(pool: &SqlitePool) -> crate::Result<()> {
    if root(pool).is_some() {
        Err(crate::ErrorKind::InputError("Instance database is unavailable; recovery copies allow browsing and launching only".into()).into())
    } else {
        Ok(())
    }
}

/// Unsupported business data is not permission to bypass its guards with a
/// stale copy. Only storage unavailability enters recovery mode.
pub(crate) fn can_recover(error: &crate::Error) -> bool {
    matches!(
        error.raw.as_ref(),
        crate::ErrorKind::Sqlx(_)
            | crate::ErrorKind::SqlxMigrate(_)
            | crate::ErrorKind::StdIOError(_)
            | crate::ErrorKind::IOError(_)
            | crate::ErrorKind::FSError(_)
    )
}

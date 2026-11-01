//! Launcher maintenance entry points; callers do not name a database backend.
pub use crate::state::store::maintenance::IntegrityReport;
pub async fn check_database_integrity() -> crate::Result<IntegrityReport> {
    let state = crate::State::get().await?;
    crate::state::store::maintenance::check(&state.data).await
}

/// Actual opened database, rather than the channel selected for next startup.
pub async fn current_database_path() -> crate::Result<std::path::PathBuf> {
    let state = crate::State::get().await?;
    Ok(crate::state::store::maintenance::database_path(&state.data))
}

pub async fn backup_open_database(
    version: &str,
) -> crate::Result<std::path::PathBuf> {
    let path = current_database_path().await?;
    crate::state::db_backup::backup_app_db_for_update(&path, version).await
}

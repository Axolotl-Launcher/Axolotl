use crate::api::Result;

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("mcarchive")
        .invoke_handler(tauri::generate_handler![
            mcarchive_get_game_versions,
            mcarchive_search_mods,
            mcarchive_get_mod_by_slug,
            mcarchive_get_file_by_filename,
            mcarchive_get_file_by_sha256,
        ])
        .build()
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn mcarchive_get_game_versions()
-> Result<Vec<theseus::mcarchive::McArchiveGameVersion>> {
    Ok(theseus::mcarchive::get_game_versions().await?)
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn mcarchive_search_mods(
    keyword: &str,
    game_version: Option<&str>,
) -> Result<Vec<theseus::mcarchive::McArchiveMod>> {
    Ok(theseus::mcarchive::search_mods(keyword, game_version).await?)
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn mcarchive_get_mod_by_slug(
    slug: &str,
) -> Result<theseus::mcarchive::McArchiveMod> {
    Ok(theseus::mcarchive::get_mod_by_slug(slug).await?)
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn mcarchive_get_file_by_filename(
    filename: &str,
) -> Result<Option<theseus::mcarchive::McArchiveFile>> {
    Ok(theseus::mcarchive::get_file_by_filename(filename).await?)
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn mcarchive_get_file_by_sha256(
    sha256: &str,
) -> Result<Option<theseus::mcarchive::McArchiveFile>> {
    Ok(theseus::mcarchive::get_file_by_sha256(sha256).await?)
}

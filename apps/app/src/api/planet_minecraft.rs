use crate::api::Result;

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("planet-minecraft")
        .invoke_handler(tauri::generate_handler![
            planet_minecraft_connector_available,
            planet_minecraft_search_projects,
            planet_minecraft_get_project,
        ])
        .build()
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub fn planet_minecraft_connector_available() -> bool {
    theseus::planet_minecraft::connector_base_url().is_ok()
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn planet_minecraft_search_projects(
    query: &str,
    game_version: Option<&str>,
) -> Result<Vec<theseus::planet_minecraft::PlanetMinecraftProject>> {
    Ok(theseus::planet_minecraft::search_projects(query, game_version).await?)
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn planet_minecraft_get_project(
    id: &str,
) -> Result<theseus::planet_minecraft::PlanetMinecraftProject> {
    Ok(theseus::planet_minecraft::get_project(id).await?)
}

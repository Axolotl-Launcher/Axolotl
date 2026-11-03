use crate::api::Result;
use theseus::schematics::{self, Detail, Page, Source};

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("schematics")
        .invoke_handler(tauri::generate_handler![
            schematics_list,
            schematics_get,
            schematics_install
        ])
        .build()
}

#[tauri::command]
pub async fn schematics_list(
    sources: Vec<Source>,
    page: u32,
    query: String,
    page_size: u32,
    sort: String,
) -> Result<Page> {
    Ok(schematics::list(sources, page, query, page_size, sort).await?)
}

#[tauri::command]
pub async fn schematics_get(source: Source, id: String) -> Result<Detail> {
    Ok(schematics::detail(source, id).await?)
}

#[tauri::command]
pub async fn schematics_install(
    instance_id: String,
    source: Source,
    id: String,
    file_name: String,
) -> Result<String> {
    Ok(schematics::install(instance_id, source, id, file_name).await?)
}

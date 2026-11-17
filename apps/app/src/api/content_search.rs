use theseus::content_search::{
    ChineseNameLookup, ChineseSearchResolution, ContentIdentityLookup,
    ContentSearchExpansion, WikiIdLookup,
};

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("content-search")
        .invoke_handler(tauri::generate_handler![
            resolve_chinese_content_search,
            expand_content_search_query,
            lookup_chinese_content_names,
            lookup_content_wiki_ids,
            lookup_content_identities,
        ])
        .build()
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub fn resolve_chinese_content_search(
    query: String,
) -> ChineseSearchResolution {
    theseus::content_search::resolve_chinese_content_search(&query)
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub fn expand_content_search_query(query: String) -> ContentSearchExpansion {
    theseus::content_search::expand_content_search_query(&query)
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub fn lookup_chinese_content_names(
    modrinth_slugs: Vec<String>,
    curseforge_slugs: Vec<String>,
) -> ChineseNameLookup {
    theseus::content_search::lookup_chinese_content_names(
        &modrinth_slugs,
        &curseforge_slugs,
    )
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub fn lookup_content_wiki_ids(
    modrinth_slugs: Vec<String>,
    curseforge_slugs: Vec<String>,
) -> WikiIdLookup {
    theseus::content_search::lookup_content_wiki_ids(
        &modrinth_slugs,
        &curseforge_slugs,
    )
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub fn lookup_content_identities(
    modrinth_slugs: Vec<String>,
    curseforge_slugs: Vec<String>,
) -> ContentIdentityLookup {
    theseus::content_search::lookup_content_identities(
        &modrinth_slugs,
        &curseforge_slugs,
    )
}

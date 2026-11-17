use crate::api::search_cancellation::{self, SearchCancellation};
use crate::api::{Result, TheseusSerializableError};
use theseus::prelude::*;

macro_rules! impl_cache_methods {
    ($(($variant:ident, $type:ty)),*) => {
        $(
            paste::paste! {
                #[tracing::instrument(skip_all, level = "info")]
                #[tauri::command]
                pub async fn [<get_ $variant:snake>](id: &str, cache_behaviour: Option<CacheBehaviour>) -> Result<Option<$type>>
                {
                    Ok(theseus::cache::[<get_ $variant:snake>](id, cache_behaviour).await?)
                }

                #[tracing::instrument(skip_all, level = "info")]
                #[tauri::command]
                pub async fn [<get_ $variant:snake _many>](
                    ids: Vec<String>,
                    cache_behaviour: Option<CacheBehaviour>,
                ) -> Result<Vec<$type>>
                {
                    let ids = ids.iter().map(|x| &**x).collect::<Vec<&str>>();
                    let entries =
                        theseus::cache::[<get_ $variant:snake _many>](&*ids, cache_behaviour).await?;

                    Ok(entries)
                }
            }
        )*
    }
}

impl_cache_methods!(
    (Project, Project),
    (ProjectV3, ProjectV3),
    (Version, Version),
    (User, User),
    (Team, Vec<TeamMember>),
    (Organization, Organization),
    (SearchResults, SearchResults)
);

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn get_search_results_v3(
    id: &str,
    cache_behaviour: Option<CacheBehaviour>,
    request_id: Option<String>,
) -> Result<Option<SearchResultsV3>> {
    let Some(request_id) = request_id else {
        return Ok(
            theseus::cache::get_search_results_v3(id, cache_behaviour).await?
        );
    };
    let cancellation = SearchCancellation::register(request_id);

    tokio::select! {
        result = theseus::cache::get_search_results_v3(id, cache_behaviour) => Ok(result?),
        _ = cancellation.cancelled() => Err(TheseusSerializableError::SearchCancelled(
            "Modrinth browse search".to_string(),
        )),
    }
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn get_search_results_v3_many(
    ids: Vec<String>,
    cache_behaviour: Option<CacheBehaviour>,
) -> Result<Vec<SearchResultsV3>> {
    let ids = ids.iter().map(|x| &**x).collect::<Vec<&str>>();
    Ok(
        theseus::cache::get_search_results_v3_many(&ids, cache_behaviour)
            .await?,
    )
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub fn cancel_search_request(request_id: String) {
    if let Some(pending) = search_cancellation::cancel(&request_id) {
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            pending.expire();
        });
    }
}

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("cache")
        .invoke_handler(tauri::generate_handler![
            get_project,
            get_project_many,
            get_project_v3,
            get_project_v3_many,
            get_version,
            get_version_many,
            get_user,
            get_user_many,
            get_team,
            get_team_many,
            get_organization,
            get_organization_many,
            get_search_results,
            get_search_results_many,
            get_search_results_v3,
            get_search_results_v3_many,
            cancel_search_request,
            purge_cache_types,
            get_project_versions,
        ])
        .build()
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn purge_cache_types(cache_types: Vec<String>) -> Result<()> {
    let cache_types = supported_cache_types(&cache_types);
    Ok(theseus::cache::purge_cache_types(&cache_types).await?)
}

fn supported_cache_types(cache_types: &[String]) -> Vec<CacheValueType> {
    cache_types
        .iter()
        .filter_map(|cache_type| {
            let parsed = CacheValueType::from_string(cache_type);
            if parsed.as_str() == cache_type {
                Some(parsed)
            } else {
                tracing::warn!(cache_type, "Ignoring unsupported cache type");
                None
            }
        })
        .collect()
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn get_project_versions(
    project_id: &str,
    cache_behaviour: Option<CacheBehaviour>,
) -> Result<Option<Vec<Version>>> {
    Ok(
        theseus::cache::get_project_versions(project_id, cache_behaviour)
            .await?,
    )
}

#[cfg(test)]
mod tests {
    use super::supported_cache_types;
    use theseus::prelude::CacheValueType;

    #[test]
    fn cache_purge_keeps_supported_types_and_ignores_unknown_types() {
        let cache_types = vec![
            "project".to_string(),
            "curseforge_project".to_string(),
            "future_cache_type".to_string(),
        ];

        assert_eq!(
            supported_cache_types(&cache_types),
            vec![CacheValueType::Project, CacheValueType::CurseForgeProject,]
        );
    }
}

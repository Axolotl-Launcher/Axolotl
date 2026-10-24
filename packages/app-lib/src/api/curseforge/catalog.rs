//! Read-only CurseForge API queries.

use super::download::normalized_download_url;
use super::dto::{
    CurseForgeCategory, CurseForgeFile, CurseForgeFilesRequest,
    CurseForgeFilesResponse, CurseForgeFingerprintResult, CurseForgePagination,
    CurseForgeProject, CurseForgeResponse, CurseForgeSearchRequest,
    UnifiedSearchHit, UnifiedSearchResponse,
};
use super::mapping::{filter_categories, push_query};
use super::transport::{MirrorPolicy, api_key, request_json};
use super::validation::validate_file_name;
use super::{CATEGORY_CACHE, MAX_PAGE_SIZE, MINECRAFT_GAME_ID};
use crate::state::{
    CacheBehaviour, CachedEntry, ContentProvider, CurseForgeProjectId,
};
use crate::{ErrorKind, State};
use reqwest::Method;
use serde_json::{Value, json};

pub async fn search_projects(
    request: CurseForgeSearchRequest,
) -> crate::Result<UnifiedSearchResponse> {
    let page_size = request.page_size.clamp(1, MAX_PAGE_SIZE);
    let mut query = vec![
        ("gameId".to_string(), MINECRAFT_GAME_ID.to_string()),
        ("classId".to_string(), request.class_id.to_string()),
        ("index".to_string(), request.index.to_string()),
        ("pageSize".to_string(), page_size.to_string()),
    ];
    if request.category_ids.is_empty() {
        push_query(&mut query, "categoryId", request.category_id);
    } else if request.category_ids.len() == 1 {
        // Single-category requests are more widely compatible as categoryId.
        push_query(
            &mut query,
            "categoryId",
            request.category_ids.first().copied(),
        );
    } else {
        let category_ids = request
            .category_ids
            .iter()
            .copied()
            .take(10)
            .collect::<Vec<_>>();
        query.push((
            "categoryIds".to_string(),
            serde_json::to_string(&category_ids)?,
        ));
    }
    push_query(&mut query, "searchFilter", request.search_filter);
    push_query(&mut query, "gameVersion", request.game_version);
    push_query(&mut query, "modLoaderType", request.mod_loader_type);
    push_query(&mut query, "sortField", request.sort_field);
    push_query(&mut query, "sortOrder", request.sort_order);

    let response: CurseForgeResponse<Vec<CurseForgeProject>> = request_json(
        Method::GET,
        "/v1/mods/search",
        query,
        None,
        MirrorPolicy::MirrorFirst,
    )
    .await?;
    let pagination = response.pagination.unwrap_or(CurseForgePagination {
        index: request.index,
        page_size,
        result_count: response.data.len() as u32,
        total_count: response.data.len() as u32,
    });

    Ok(UnifiedSearchResponse {
        provider: ContentProvider::CurseForge,
        hits: response
            .data
            .into_iter()
            .map(UnifiedSearchHit::from)
            .collect(),
        offset: pagination.index,
        limit: pagination.page_size,
        total_hits: pagination.total_count,
    })
}

pub async fn get_project(project_id: u32) -> crate::Result<CurseForgeProject> {
    let state = State::get().await?;
    CachedEntry::get_curseforge_project(
        &CurseForgeProjectId::new(project_id)?,
        None,
        &state.pool,
        &state.api_semaphore,
    )
    .await?
    .ok_or_else(|| {
        ErrorKind::OtherError(format!(
            "CurseForge project {project_id} was not found"
        ))
        .as_error()
    })
}

pub async fn get_projects(
    project_ids: Vec<u32>,
) -> crate::Result<Vec<CurseForgeProject>> {
    get_projects_with_cache_behaviour(project_ids, None).await
}

pub async fn get_projects_with_cache_behaviour(
    project_ids: Vec<u32>,
    cache_behaviour: Option<CacheBehaviour>,
) -> crate::Result<Vec<CurseForgeProject>> {
    let project_ids = project_ids
        .into_iter()
        .map(CurseForgeProjectId::new)
        .collect::<crate::Result<Vec<_>>>()?;
    let state = State::get().await?;
    CachedEntry::get_curseforge_project_many(
        &project_ids,
        cache_behaviour,
        &state.pool,
        &state.api_semaphore,
    )
    .await
}

pub async fn get_description(project_id: u32) -> crate::Result<String> {
    let response: CurseForgeResponse<String> = request_json(
        Method::GET,
        &format!("/v1/mods/{project_id}/description"),
        Vec::new(),
        None,
        MirrorPolicy::MirrorFirst,
    )
    .await?;
    Ok(response.data)
}

pub async fn get_files_page(
    project_id: u32,
    request: CurseForgeFilesRequest,
) -> crate::Result<CurseForgeFilesResponse> {
    let page_size = request.page_size.clamp(1, MAX_PAGE_SIZE);
    let mut query = vec![
        ("index".to_string(), request.index.to_string()),
        ("pageSize".to_string(), page_size.to_string()),
    ];
    push_query(&mut query, "gameVersion", request.game_version);
    push_query(&mut query, "modLoaderType", request.mod_loader_type);
    push_query(
        &mut query,
        "gameVersionTypeId",
        request.game_version_type_id,
    );

    let response: CurseForgeResponse<Vec<CurseForgeFile>> = request_json(
        Method::GET,
        &format!("/v1/mods/{project_id}/files"),
        query,
        None,
        MirrorPolicy::MirrorFirst,
    )
    .await?;
    let result_count = response.data.len() as u32;
    let pagination = response.pagination.unwrap_or(CurseForgePagination {
        index: request.index,
        page_size,
        result_count,
        total_count: result_count,
    });

    Ok(CurseForgeFilesResponse {
        files: response.data,
        pagination,
    })
}

/// Fetches every page of a CurseForge project's files and returns the complete
/// file list, since the API caps each page at `MAX_PAGE_SIZE` entries.
pub async fn get_files(
    project_id: u32,
    request: CurseForgeFilesRequest,
) -> crate::Result<CurseForgeFilesResponse> {
    let page_size = request.page_size.clamp(1, MAX_PAGE_SIZE);
    let mut files = Vec::new();
    let mut index = request.index;

    let total_count = loop {
        let response = get_files_page(
            project_id,
            CurseForgeFilesRequest {
                index,
                page_size,
                ..request.clone()
            },
        )
        .await?;
        let pagination = response.pagination;
        files.extend(response.files);

        if files.len() as u32 >= pagination.total_count
            || pagination.result_count == 0
        {
            break pagination.total_count;
        }
        index += pagination.result_count;
    };

    let result_count = files.len() as u32;
    Ok(CurseForgeFilesResponse {
        files,
        pagination: CurseForgePagination {
            index: request.index,
            page_size,
            result_count,
            total_count,
        },
    })
}

pub async fn get_file(
    project_id: u32,
    file_id: u32,
) -> crate::Result<CurseForgeFile> {
    let state = State::get().await?;
    crate::resources::legacy_catalog::exact(
        crate::resources::SourceId::curseforge(),
        Some(project_id.to_string()),
        file_id.to_string(),
        &state,
    )
    .await?
    .ok_or_else(|| {
        ErrorKind::InputError("CurseForge file not found".into()).into()
    })
}

pub async fn get_files_many(
    file_ids: Vec<u32>,
) -> crate::Result<Vec<CurseForgeFile>> {
    use crate::resources::{
        SourceId, SourceReleaseId,
        providers::{ProviderQuery, ProviderValue},
    };
    let state = State::get().await?;
    let service = state
        .resources()
        .await
        .map_err(crate::resources::protocol::error)?;
    let queries = file_ids
        .into_iter()
        .map(|id| {
            Ok(ProviderQuery::Release {
                project_id: None,
                release_id: SourceReleaseId::new(id.to_string())
                    .map_err(crate::resources::protocol::error)?,
                slug: None,
            })
        })
        .collect::<crate::Result<Vec<_>>>()?;
    let values = service
        .presentation_many(
            &SourceId::curseforge(),
            queries,
            &tokio_util::sync::CancellationToken::new(),
        )
        .await;
    let mut files = Vec::new();
    for value in values {
        match value.map_err(crate::resources::protocol::error)?.value {
            ProviderValue::Release(document) => {
                files.push(serde_json::from_value(
                    document.protocol.ok_or_else(|| {
                        ErrorKind::InputError(
                            "CurseForge file projection missing".into(),
                        )
                    })?,
                )?)
            }
            ProviderValue::NotFound => {}
            _ => {
                return Err(ErrorKind::InputError(
                    "unexpected CurseForge file response".into(),
                )
                .into());
            }
        }
    }
    Ok(files)
}

pub async fn get_changelog(
    project_id: u32,
    file_id: u32,
) -> crate::Result<String> {
    let response: CurseForgeResponse<String> = request_json(
        Method::GET,
        &format!("/v1/mods/{project_id}/files/{file_id}/changelog"),
        Vec::new(),
        None,
        MirrorPolicy::MirrorFirst,
    )
    .await?;
    Ok(response.data)
}

pub async fn get_download_url(
    project_id: u32,
    file_id: u32,
) -> crate::Result<Option<String>> {
    let path = format!("/v1/mods/{project_id}/files/{file_id}/download-url");
    let response: CurseForgeResponse<Option<String>> = request_json(
        Method::GET,
        &path,
        Vec::new(),
        None,
        MirrorPolicy::MirrorFirst,
    )
    .await?;
    if let Some(url) = normalized_download_url(response.data) {
        return Ok(Some(url));
    }
    if api_key().is_none() {
        return Ok(None);
    }

    let response: CurseForgeResponse<Option<String>> = request_json(
        Method::GET,
        &path,
        Vec::new(),
        None,
        MirrorPolicy::OfficialOnly,
    )
    .await?;
    Ok(normalized_download_url(response.data))
}

pub(crate) fn derived_curseforge_download_url(
    file_id: u32,
    file_name: &str,
) -> crate::Result<String> {
    validate_file_name(file_name)?;
    Ok(format!(
        "https://edge.forgecdn.net/files/{}/{}/{}",
        file_id / 1000,
        file_id % 1000,
        urlencoding::encode(file_name)
    ))
}

pub async fn get_categories(
    class_id: Option<u32>,
) -> crate::Result<Vec<CurseForgeCategory>> {
    let cached = CATEGORY_CACHE.read().ok().and_then(|cache| cache.clone());
    let categories = if let Some(categories) = cached {
        categories
    } else {
        let response: CurseForgeResponse<Vec<CurseForgeCategory>> =
            request_json(
                Method::GET,
                "/v1/categories",
                vec![("gameId".to_string(), MINECRAFT_GAME_ID.to_string())],
                None,
                MirrorPolicy::MirrorFirst,
            )
            .await?;

        if let Ok(mut cache) = CATEGORY_CACHE.write() {
            *cache = Some(response.data.clone());
        }
        response.data
    };

    Ok(filter_categories(categories, class_id))
}

pub async fn match_fingerprints(
    fingerprints: Vec<u64>,
) -> crate::Result<CurseForgeFingerprintResult> {
    let response: CurseForgeResponse<CurseForgeFingerprintResult> =
        request_json(
            Method::POST,
            &format!("/v1/fingerprints/{MINECRAFT_GAME_ID}"),
            Vec::new(),
            Some(json!({ "fingerprints": fingerprints })),
            MirrorPolicy::MirrorFirst,
        )
        .await?;
    Ok(response.data)
}

/// Resource adapters share routing and credential handling without consulting
/// the legacy main-database resource cache.
pub(crate) async fn resource_request(
    method: Method,
    path: &str,
    query: Vec<(String, String)>,
    body: Option<Value>,
) -> crate::Result<Value> {
    request_json(method, path, query, body, MirrorPolicy::MirrorFirst).await
}

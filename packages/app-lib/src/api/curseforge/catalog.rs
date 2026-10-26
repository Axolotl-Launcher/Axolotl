//! Read-only CurseForge API queries.

use super::constants::{MAX_PAGE_SIZE, MINECRAFT_GAME_ID};
use super::download::normalized_download_url;
use super::dto::{
    CurseForgeCategory, CurseForgeFile, CurseForgeFilesRequest,
    CurseForgeFilesResponse, CurseForgeFingerprintResult, CurseForgePagination,
    CurseForgeProject, CurseForgeResponse, CurseForgeSearchRequest,
    UnifiedSearchHit, UnifiedSearchResponse,
};
use super::mapping::{
    filter_categories, mod_loader_to_slug, project_type_for_class, push_query,
};
use super::transport::{MirrorPolicy, api_key, request_json};
use super::validation::validate_file_name;
use crate::state::{CacheBehaviour, ContentProvider};
use crate::util::download::constants::MODPACK_METADATA_CONCURRENCY;
use crate::{ErrorKind, State};
use futures::{StreamExt, stream};
use reqwest::Method;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::sync::{LazyLock, RwLock};

static CATEGORY_CACHE: LazyLock<RwLock<Option<Vec<CurseForgeCategory>>>> =
    LazyLock::new(|| RwLock::new(None));

pub(super) fn unique_metadata_chunks(mut ids: Vec<u32>) -> Vec<Vec<u32>> {
    ids.sort_unstable();
    ids.dedup();
    ids.chunks(MAX_PAGE_SIZE as usize)
        .map(<[u32]>::to_vec)
        .collect()
}

pub(super) async fn get_modpack_projects(
    project_ids: Vec<u32>,
) -> crate::Result<HashMap<u32, CurseForgeProject>> {
    let chunks = unique_metadata_chunks(project_ids);
    let project_ids = chunks.iter().flatten().copied().collect::<Vec<_>>();
    let batches = stream::iter(chunks)
        .map(|chunk| async move { get_projects(chunk).await })
        .buffer_unordered(MODPACK_METADATA_CONCURRENCY)
        .collect::<Vec<_>>()
        .await;
    let mut projects = HashMap::new();
    for batch in batches {
        for project in batch? {
            projects.insert(project.id, project);
        }
    }
    let missing = project_ids
        .into_iter()
        .filter(|project_id| !projects.contains_key(project_id))
        .collect::<Vec<_>>();
    let fallbacks = stream::iter(missing)
        .map(|project_id| async move { get_project(project_id).await })
        .buffer_unordered(MODPACK_METADATA_CONCURRENCY)
        .collect::<Vec<_>>()
        .await;
    for project in fallbacks {
        let project = project?;
        projects.insert(project.id, project);
    }
    Ok(projects)
}

pub(super) async fn get_modpack_files(
    file_ids: Vec<u32>,
) -> crate::Result<HashMap<u32, CurseForgeFile>> {
    let chunks = unique_metadata_chunks(file_ids);
    let batches = stream::iter(chunks)
        .map(|chunk| async move { get_files_many(chunk).await })
        .buffer_unordered(MODPACK_METADATA_CONCURRENCY)
        .collect::<Vec<_>>()
        .await;
    let mut files = HashMap::new();
    for batch in batches {
        for file in batch? {
            files.insert(file.id, file);
        }
    }
    Ok(files)
}

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
    crate::resources::legacy_catalog::projects_exact(
        crate::resources::SourceId::curseforge(),
        vec![project_id.to_string()],
        &state,
    )
    .await?
    .into_iter()
    .next()
    .ok_or_else(|| {
        ErrorKind::OtherError(format!(
            "CurseForge project {project_id} was not found"
        ))
        .into()
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
    let state = State::get().await?;
    crate::resources::legacy_catalog::projects_exact_with_policy(
        crate::resources::SourceId::curseforge(),
        project_ids.into_iter().map(|id| id.to_string()).collect(),
        &state,
        crate::resources::legacy_catalog::policy(cache_behaviour),
    )
    .await
}

async fn get_files_page(
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
    let state = State::get().await?;
    crate::resources::legacy_catalog::releases_exact(
        crate::resources::SourceId::curseforge(),
        file_ids.into_iter().map(|id| id.to_string()).collect(),
        &state,
    )
    .await
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
    Ok(
        crate::resources::providers::derived_download_url(file_id, file_name)
            .map_err(crate::resources::protocol::error)?,
    )
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

impl From<CurseForgeProject> for UnifiedSearchHit {
    fn from(project: CurseForgeProject) -> Self {
        let mut versions = Vec::new();
        let mut seen_versions = HashSet::new();
        let mut loaders = HashSet::new();
        for index in &project.latest_files_indexes {
            if seen_versions.insert(index.game_version.clone()) {
                versions.push(index.game_version.clone());
            }
            if let Some(mod_loader) = index.mod_loader {
                let loader_slug = mod_loader_to_slug(mod_loader);
                loaders.insert(loader_slug);
            }
        }

        let project_type = project_type_for_class(project.class_id);
        let mut categories: Vec<String> = project
            .categories
            .iter()
            .map(|category| category.slug.clone())
            .collect();
        categories.extend(loaders.iter().map(|s| s.to_string()));

        Self {
            provider: ContentProvider::CurseForge,
            project_id: project.id.to_string(),
            slug: Some(project.slug),
            author: project
                .authors
                .first()
                .map(|author| author.name.clone())
                .unwrap_or_default(),
            author_url: project
                .authors
                .first()
                .map(|author| author.url.clone()),
            title: project.name,
            description: project.summary,
            project_type: project_type.to_string(),
            categories,
            versions,
            downloads: project.download_count,
            icon_url: project.logo.map(|logo| logo.url),
            date_created: project.date_created,
            date_modified: project.date_modified,
            latest_version: project
                .latest_files
                .first()
                .map(|file| file.id.to_string()),
            gallery: project
                .screenshots
                .into_iter()
                .map(|screenshot| screenshot.url)
                .collect(),
            website_url: project.links.website_url,
            source_url: project.links.source_url,
            allow_mod_distribution: project.allow_mod_distribution,
        }
    }
}

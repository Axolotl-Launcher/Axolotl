//! CurseForge routing tests.

use super::fixtures::category;
use crate::api::curseforge::catalog::derived_curseforge_download_url;
use crate::api::curseforge::constants::MINECRAFT_GAME_ID;
use crate::api::curseforge::download::normalized_download_url;
use crate::api::curseforge::dto::{
    CurseForgeApiKeySource, CurseForgeFile, CurseForgeProject, UnifiedSearchHit,
};
use crate::api::curseforge::mapping::filter_categories;
use crate::api::curseforge::selection::validate_cdn_url;
use crate::api::curseforge::transport::{
    MirrorPolicy, RequestRoute, RequestRouteSource, request_routes,
    resolve_api_key, should_try_next_route,
};
use reqwest::StatusCode;
use serde_json::json;

#[test]
fn unified_search_hit_accepts_null_gallery() {
    let hit: UnifiedSearchHit = serde_json::from_value(serde_json::json!({
        "provider": "curseforge",
        "project_id": "250419",
        "slug": null,
        "author": "DarkhaxDev",
        "author_url": null,
        "title": "Enchantment Descriptions",
        "description": "description",
        "project_type": "mod",
        "categories": [],
        "versions": [],
        "downloads": 0,
        "icon_url": null,
        "date_created": "2026-01-01T00:00:00Z",
        "date_modified": "2026-01-01T00:00:00Z",
        "latest_version": null,
        "gallery": null,
        "website_url": null,
        "source_url": null,
        "allow_mod_distribution": null,
    }))
    .unwrap();

    assert!(hit.gallery.is_empty());
}

#[test]
fn curseforge_file_accepts_null_modules() {
    let file: CurseForgeFile = serde_json::from_value(serde_json::json!({
        "id": 4031925,
        "gameId": 432,
        "modId": 250419,
        "isAvailable": true,
        "displayName": "old.jar",
        "fileName": "old.jar",
        "releaseType": 1,
        "fileStatus": 4,
        "fileDate": "2026-01-01T00:00:00Z",
        "fileLength": 1,
        "downloadCount": 0,
        "fileFingerprint": 0,
        "modules": null,
    }))
    .unwrap();

    assert!(file.modules.is_empty());
}

#[test]
fn official_only_requests_exclude_mirror_routes() {
    let routes = request_routes("/v1/mods/search", MirrorPolicy::OfficialOnly);

    assert!(
        routes
            .iter()
            .all(|route| route.source == RequestRouteSource::Official)
    );
}

#[test]
fn mirror_not_found_tries_next_route() {
    let route = RequestRoute {
        url: String::new(),
        use_api_key: false,
        use_system_proxy: false,
        source: RequestRouteSource::Mirror,
    };

    assert!(should_try_next_route(&route, StatusCode::NOT_FOUND, true));
}

#[test]
fn official_forbidden_response_tries_proxy_route() {
    let route = RequestRoute {
        url: String::new(),
        use_api_key: true,
        use_system_proxy: false,
        source: RequestRouteSource::Official,
    };

    assert!(should_try_next_route(&route, StatusCode::FORBIDDEN, true));
}

#[test]
fn projects_accept_negative_popularity_ranks() {
    let project = serde_json::from_value::<CurseForgeProject>(json!({
        "id": 1,
        "gameId": MINECRAFT_GAME_ID,
        "name": "Fixture",
        "slug": "fixture",
        "links": {},
        "summary": "Fixture project",
        "status": 4,
        "downloadCount": 0,
        "isFeatured": false,
        "primaryCategoryId": 6,
        "categories": [],
        "classId": 6,
        "authors": [],
        "logo": null,
        "screenshots": [],
        "mainFileId": 0,
        "latestFiles": [],
        "latestFilesIndexes": [],
        "dateCreated": "2026-01-01T00:00:00Z",
        "dateModified": "2026-01-01T00:00:00Z",
        "dateReleased": "2026-01-01T00:00:00Z",
        "allowModDistribution": true,
        "gamePopularityRank": -10,
        "isAvailable": true
    }))
    .unwrap();

    assert_eq!(project.game_popularity_rank, Some(-10));
}

#[test]
fn category_cache_can_be_filtered_for_each_project_class() {
    let categories = vec![
        category(6, None, true),
        category(406, Some(6), false),
        category(4471, None, true),
        category(4481, Some(4471), false),
    ];

    let mods = filter_categories(categories.clone(), Some(6));
    assert_eq!(
        mods.iter().map(|category| category.id).collect::<Vec<_>>(),
        vec![6, 406]
    );

    let modpacks = filter_categories(categories, Some(4471));
    assert_eq!(
        modpacks
            .iter()
            .map(|category| category.id)
            .collect::<Vec<_>>(),
        vec![4471, 4481]
    );
}

#[test]
fn cdn_urls_are_restricted_to_forgecdn() {
    assert!(
        validate_cdn_url(
            &reqwest::Url::parse("https://edge.forgecdn.net/files/1/2/a.jar")
                .unwrap()
        )
        .is_ok()
    );
    assert!(
        validate_cdn_url(
            &reqwest::Url::parse("https://forgecdn.net.evil.test/a.jar")
                .unwrap()
        )
        .is_err()
    );
}

#[test]
fn blank_download_urls_are_treated_as_unavailable() {
    assert_eq!(normalized_download_url(None), None);
    assert_eq!(normalized_download_url(Some(String::new())), None);
    assert_eq!(normalized_download_url(Some("  \t".to_string())), None);
    assert_eq!(
        normalized_download_url(Some(
            " https://edge.forgecdn.net/files/1/2/a.jar ".to_string(),
        )),
        Some("https://edge.forgecdn.net/files/1/2/a.jar".to_string()),
    );
}

#[test]
fn derived_download_urls_follow_curseforge_file_id_layout() {
    assert_eq!(
        derived_curseforge_download_url(6_767_951, "jei 1.21.5+neoforge.jar")
            .unwrap(),
        "https://edge.forgecdn.net/files/6767/951/jei%201.21.5%2Bneoforge.jar"
    );
    assert_eq!(
        derived_curseforge_download_url(1_234_005, "example.jar").unwrap(),
        "https://edge.forgecdn.net/files/1234/5/example.jar"
    );
    assert!(
        derived_curseforge_download_url(1_234_005, "../example.jar").is_err()
    );
}

#[test]
fn a_key_set_in_the_launcher_wins_over_the_launcher_key() {
    assert_eq!(
        resolve_api_key(Some("override".into()), Some("launcher".into())),
        (
            Some("override".to_string()),
            CurseForgeApiKeySource::Override
        ),
    );
}

#[test]
fn a_blank_override_falls_back_to_the_launcher_key() {
    assert_eq!(
        resolve_api_key(Some("   ".into()), Some("launcher".into())),
        (
            Some("launcher".to_string()),
            CurseForgeApiKeySource::Launcher
        ),
    );
    assert_eq!(
        resolve_api_key(None, Some("launcher".into())),
        (
            Some("launcher".to_string()),
            CurseForgeApiKeySource::Launcher
        ),
    );
}

#[test]
fn keys_are_trimmed_before_they_are_used() {
    assert_eq!(
        resolve_api_key(Some(" override\n".into()), None),
        (
            Some("override".to_string()),
            CurseForgeApiKeySource::Override
        ),
    );
}

#[test]
fn a_build_without_any_key_reports_no_key() {
    assert_eq!(
        resolve_api_key(None, None),
        (None, CurseForgeApiKeySource::Missing),
    );
    assert_eq!(
        resolve_api_key(Some(" ".into()), Some("\t".into())),
        (None, CurseForgeApiKeySource::Missing),
    );
}

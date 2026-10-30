//! Fetch routes tests.

use super::super::*;
use super::fixtures;
use crate::util::download::retired_sources;
use crate::util::download::route_health::{
    ROUTE_HEALTH, ResourceFamily, RouteHealth,
};
use crate::util::fetch::transfer::build_download_routes;
use url::Url;

#[test]
fn modrinth_requests_are_classified_for_logging() {
    assert_eq!(
        modrinth_request_kind("https://api.modrinth.com/v2/project"),
        Some("API")
    );
    assert_eq!(
        modrinth_request_kind(
            "https://cdn.modrinth.com/data/project/version/file.jar"
        ),
        Some("CDN")
    );
    assert_eq!(
        modrinth_request_kind(
            "https://cdn-alt.modrinth.com/data/project/version/file.jar"
        ),
        None
    );
    assert_eq!(modrinth_request_kind("https://example.com/file.jar"), None);
}

#[test]
fn modrinth_cdn_routes_use_the_current_host() {
    for host in ["cdn.modrinth.com", "cdn-alt.modrinth.com"] {
        let url =
            format!("https://{host}/data/project/version/file.jar?download=1");
        let routes = resolve_download_routes_for(
            &url,
            ResourceClass::Modrinth,
            crate::state::DownloadSourceMode::OfficialOnly,
        );
        assert_eq!(routes.len(), 1);
        assert_eq!(
            routes[0].url,
            "https://cdn.modrinth.com/data/project/version/file.jar?download=1"
        );
    }
}

#[test]
fn modrinth_download_url_recognition_excludes_retired_hosts() {
    assert!(!is_official_modrinth_download_url(
        "https://cdn-alt.modrinth.com/data/project/version/file.jar"
    ));
    assert!(is_official_modrinth_download_url(
        "https://cdn.modrinth.com/data/project/version/file.jar"
    ));
    assert!(is_official_modrinth_download_url(
        "https://api.modrinth.com/v2/project/abc"
    ));
    assert!(!is_official_modrinth_download_url(
        "https://example.com/file.jar"
    ));
}

#[test]
fn cache_busted_urls_preserve_existing_query() {
    assert_eq!(
        cache_busted_download_url(
            "https://cdn-alt.modrinth.com/data/project/version/file.jar?download=1",
            2,
        ),
        "https://cdn-alt.modrinth.com/data/project/version/file.jar?download=1&axolotl_retry=2"
    );
    assert_eq!(
        cache_busted_download_url(
            "https://cdn-alt.modrinth.com/data/project/version/file.jar",
            1,
        ),
        "https://cdn-alt.modrinth.com/data/project/version/file.jar?axolotl_retry=1"
    );
    // A previous buster is replaced, not stacked.
    assert_eq!(
        cache_busted_download_url(
            "https://cdn-alt.modrinth.com/data/project/version/file.jar?axolotl_retry=1&download=1",
            3,
        ),
        "https://cdn-alt.modrinth.com/data/project/version/file.jar?download=1&axolotl_retry=3"
    );
}

#[test]
fn retired_modrinth_candidates_are_deduplicated_in_every_source_mode() {
    for mode in [
        crate::state::DownloadSourceMode::Auto,
        crate::state::DownloadSourceMode::OfficialOnly,
        crate::state::DownloadSourceMode::OfficialPreferred,
        crate::state::DownloadSourceMode::MirrorPreferred,
    ] {
        let request = DownloadRequest::new(
            "https://cdn-alt.modrinth.com/data/pack.mrpack",
            ResourceClass::Modpack,
        )
        .with_candidate_urls([
            "https://cdn.modrinth.com/data/pack.mrpack",
            "https://cdn-alt.modrinth.com/data/other.jar?x=%2b",
        ]);
        let routes = build_download_routes(&request, mode);
        assert_eq!(routes.len(), 2);
        assert_eq!(routes[0].url, "https://cdn.modrinth.com/data/pack.mrpack");
        assert_eq!(
            routes[1].url,
            "https://cdn.modrinth.com/data/other.jar?x=%2b"
        );
        assert!(!routes.iter().any(|route| route.url.contains("cdn-alt")));
        assert_eq!(routes[0].source, DownloadRouteSource::Official);
        assert!(is_official_modrinth_download_url(&routes[0].url));
        assert_eq!(modrinth_request_kind(&routes[0].url), Some("CDN"));
        assert_eq!(
            infer_resource_class(&routes[0].url),
            ResourceClass::Modrinth
        );
    }
    assert_eq!(
        modrinth_request_kind("https://cdn.modrinth.com.evil.example/file.jar"),
        None
    );
}

#[test]
fn log_urls_keep_route_but_remove_credentials_and_fragments() {
    assert_eq!(
        sanitize_url_for_log(
            "https://mod.mcimirror.top/data/file.jar?X-Amz-Credential=secret&X-Amz-Signature=signature#fragment"
        ),
        "https://mod.mcimirror.top/data/file.jar"
    );
    assert_eq!(
        sanitize_url_for_log("not-a-url?token=secret#fragment"),
        "not-a-url"
    );
    assert_eq!(
        sanitize_url_for_log(
            "https://username:password@example.com/file.jar?token=secret"
        ),
        "https://example.com/file.jar"
    );
}

#[test]
fn auto_source_health_is_scoped_by_resource_family() {
    let _guard = fixtures::AUTO_SOURCE_TEST_LOCK.lock().unwrap();
    let previous_health = std::mem::take(&mut *ROUTE_HEALTH.lock());
    let manifest_url =
        "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
    let manifest_routes = resolve_download_routes_for(
        manifest_url,
        ResourceClass::Metadata,
        crate::state::DownloadSourceMode::Auto,
    );
    assert_eq!(manifest_routes[0].source, DownloadRouteSource::Official);

    record_route_failure(&manifest_routes[0], ResourceClass::Metadata, None);
    let modrinth_routes = resolve_download_routes_for(
        "https://api.modrinth.com/v2/tag/game_version",
        ResourceClass::Modrinth,
        crate::state::DownloadSourceMode::Auto,
    );
    assert_eq!(modrinth_routes[0].source, DownloadRouteSource::Official);

    *ROUTE_HEALTH.lock() = previous_health;
}

#[test]
fn disabled_tianpao_routes_leave_official_cdn_available() {
    let _guard = fixtures::AUTO_SOURCE_TEST_LOCK.lock().unwrap();
    let previous_health = std::mem::take(&mut *ROUTE_HEALTH.lock());
    let url =
        "https://cdn-alt.modrinth.com/data/project/versions/version/file.jar";
    let mut routes = explicit_mirror_routes(url, ResourceClass::Modrinth);
    routes.push(official_route(url, ResourceClass::Modrinth));
    assert!(routes.iter().all(|route| {
        !Url::parse(&route.url)
            .is_ok_and(|url| retired_sources::is_retired(&url))
    }));
    let official = routes
        .iter()
        .find(|route| is_official_route(route))
        .unwrap();
    let key = route_health_key(official, ResourceClass::Modrinth).unwrap();
    ROUTE_HEALTH.lock().insert(
        key,
        RouteHealth {
            success_samples: 3,
            throughput_bps: Some(100_000_000.0),
            ..RouteHealth::default()
        },
    );

    order_auto_routes(&mut routes, ResourceClass::Modrinth);

    assert_eq!(routes[0].source, DownloadRouteSource::Official);
    *ROUTE_HEALTH.lock() = previous_health;
}

#[test]
fn tianpao_routes_restore_official_sources_in_all_modes_and_skip_unknown_candidates()
 {
    for mode in [
        crate::state::DownloadSourceMode::Auto,
        crate::state::DownloadSourceMode::OfficialOnly,
        crate::state::DownloadSourceMode::OfficialPreferred,
        crate::state::DownloadSourceMode::MirrorPreferred,
    ] {
        for (input, official) in [
            (
                "https://mod.tianpao.top/data/p/file.jar",
                "https://cdn.modrinth.com/data/p/file.jar",
            ),
            (
                "https://mod.tianpao.top/files/1/2/file.jar",
                "https://edge.forgecdn.net/files/1/2/file.jar",
            ),
            (
                "https://mod.tianpao.top/media/attachments/icon.png",
                "https://media.forgecdn.net/attachments/icon.png",
            ),
        ] {
            let request = DownloadRequest::new(input, ResourceClass::Modpack)
                .with_candidate_urls([
                    official,
                    "https://mod.tianpao.top/unknown",
                ]);
            let routes = build_download_routes(&request, mode);
            assert_eq!(routes.len(), 1);
            assert_eq!(routes[0].url, official);
            assert_eq!(routes[0].source, DownloadRouteSource::Official);
        }
        assert!(
            resolve_download_routes_for(
                "https://mod.tianpao.top/unknown",
                ResourceClass::Modpack,
                mode
            )
            .is_empty()
        );
        let request = DownloadRequest::new(
            "https://mod.tianpao.top/unknown",
            ResourceClass::Modpack,
        )
        .with_candidate_urls(["https://cdn.modrinth.com/data/file.jar"]);
        assert_eq!(
            build_download_routes(&request, mode)[0].url,
            "https://cdn.modrinth.com/data/file.jar"
        );
    }
}

#[test]
fn mrpack_urls_are_detected_without_query_string() {
    assert!(is_mrpack_url(
        "https://cdn.modrinth.com/data/project/version/pack.MRPACK?download=1"
    ));
    assert!(!is_mrpack_url(
        "https://cdn.modrinth.com/data/project/version/mod.jar"
    ));
}

#[test]
fn vanilla_libraries_have_both_bmcl_routes() {
    let source =
        "https://libraries.minecraft.net/com/example/library/1/library-1.jar";
    let routes = resolve_download_routes_for(
        source,
        ResourceClass::MinecraftLibrary,
        crate::state::DownloadSourceMode::MirrorPreferred,
    );
    assert_eq!(routes.len(), 3);
    assert_eq!(
        routes[0].url,
        "https://bmclapi2.bangbang93.com/maven/com/example/library/1/library-1.jar"
    );
    assert_eq!(
        routes[1].url,
        "https://bmclapi2.bangbang93.com/libraries/com/example/library/1/library-1.jar"
    );
    assert_eq!(routes[2].url, source);
}

#[test]
fn maven_central_libraries_mirror_to_aliyun_with_official_fallback() {
    let source = "https://repo.maven.apache.org/maven2/org/javassist/javassist/3.30.2-GA/javassist-3.30.2-GA.jar";
    let routes = resolve_download_routes_for(
        source,
        ResourceClass::MinecraftLibrary,
        crate::state::DownloadSourceMode::MirrorPreferred,
    );
    assert_eq!(routes.len(), 2);
    assert_eq!(
        routes[0].url,
        "https://maven.aliyun.com/repository/public/org/javassist/javassist/3.30.2-GA/javassist-3.30.2-GA.jar"
    );
    assert!(routes[0].is_mirror);

    assert_eq!(routes[1].url, source);
    assert!(!routes[1].is_mirror);

    let official_only = resolve_download_routes_for(
        source,
        ResourceClass::MinecraftLibrary,
        crate::state::DownloadSourceMode::OfficialOnly,
    );
    assert_eq!(official_only.len(), 1);
    assert_eq!(official_only[0].url, source);
}

#[test]
fn loader_libraries_prefer_mirrors_with_an_official_fallback() {
    let source = "https://libraries.minecraft.net/net/minecraftforge/forge/1.20.1/forge-1.20.1.jar";
    let routes = resolve_download_routes_for(
        source,
        ResourceClass::MinecraftLibrary,
        crate::state::DownloadSourceMode::MirrorPreferred,
    );
    assert_eq!(routes.len(), 3);
    assert_eq!(
        routes[0].url,
        "https://bmclapi2.bangbang93.com/maven/net/minecraftforge/forge/1.20.1/forge-1.20.1.jar"
    );
    assert_eq!(
        routes[1].url,
        "https://bmclapi2.bangbang93.com/libraries/net/minecraftforge/forge/1.20.1/forge-1.20.1.jar"
    );
    assert_eq!(routes[2].url, source);
    assert!(!routes[2].is_mirror);

    let official_only = resolve_download_routes_for(
        source,
        ResourceClass::MinecraftLibrary,
        crate::state::DownloadSourceMode::OfficialOnly,
    );
    assert_eq!(official_only.len(), 1);
    assert_eq!(official_only[0].url, source);

    let official_preferred = resolve_download_routes_for(
        source,
        ResourceClass::MinecraftLibrary,
        crate::state::DownloadSourceMode::OfficialPreferred,
    );
    assert_eq!(official_preferred.len(), 3);
    assert_eq!(official_preferred[0].url, source);
    assert!(official_preferred[1..].iter().all(|route| route.is_mirror));
    assert!(official_preferred.iter().all(|route| {
        resource_family(route, ResourceClass::MinecraftLibrary)
            == ResourceFamily::Loader
    }));
}

#[test]
fn auto_loader_routes_start_official_and_retain_explicit_mirror_preference() {
    let _guard = fixtures::AUTO_SOURCE_TEST_LOCK.lock().unwrap();
    let previous_health = std::mem::take(&mut *ROUTE_HEALTH.lock());
    for source in [
        "https://maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml",
        "https://maven.minecraftforge.net/net/minecraftforge/forge/maven-metadata.xml",
        "https://maven.fabricmc.net/net/fabricmc/fabric-loader/maven-metadata.xml",
    ] {
        let routes = resolve_download_routes_for(
            source,
            ResourceClass::Loader,
            crate::state::DownloadSourceMode::Auto,
        );
        assert_eq!(routes[0].url, source);
        assert_eq!(routes[0].source, DownloadRouteSource::Official);
        assert!(routes[1..].iter().any(|route| route.is_mirror));
        let mirrors = resolve_download_routes_for(
            source,
            ResourceClass::Loader,
            crate::state::DownloadSourceMode::MirrorPreferred,
        );
        assert!(mirrors[0].is_mirror);
        record_route_failure(&routes[0], ResourceClass::Loader, None);
        let after_failure = resolve_download_routes_for(
            source,
            ResourceClass::Loader,
            crate::state::DownloadSourceMode::Auto,
        );
        assert!(after_failure[0].is_mirror);
    }
    *ROUTE_HEALTH.lock() = previous_health;
}

#[test]
fn maven_central_routes_mirror_to_aliyun_with_official_fallback() {
    for source in [
        "https://repo1.maven.org/maven2/com/example/library/1/library-1.jar?download=1",
        "https://repo.maven.apache.org/maven2/com/example/library/1/library-1.jar?download=1",
    ] {
        let routes = resolve_download_routes_for(
            source,
            ResourceClass::MinecraftLibrary,
            crate::state::DownloadSourceMode::MirrorPreferred,
        );
        assert_eq!(routes.len(), 2);
        assert_eq!(routes[0].source, DownloadRouteSource::Aliyun);
        assert!(routes[0].is_mirror);
        assert!(routes[0]
            .url
            .starts_with("https://maven.aliyun.com/repository/public/com/example/library/1/library-1.jar"));
        assert_eq!(routes[1].url, source);
        assert_eq!(routes[1].source, DownloadRouteSource::Official);
    }

    let unmatched = resolve_download_routes_for(
        "https://repo1.maven.org/repository/com/example/library.jar",
        ResourceClass::MinecraftLibrary,
        crate::state::DownloadSourceMode::MirrorPreferred,
    );
    assert_eq!(unmatched.len(), 1);
    assert_eq!(
        unmatched[0].url,
        "https://repo1.maven.org/repository/com/example/library.jar"
    );

    let prefer_official = resolve_download_routes_for(
        "https://repo1.maven.org/maven2/com/example/library/1/library-1.jar",
        ResourceClass::MinecraftLibrary,
        crate::state::DownloadSourceMode::OfficialOnly,
    );
    assert_eq!(prefer_official.len(), 1);
    assert_eq!(
        prefer_official[0].url,
        "https://repo1.maven.org/maven2/com/example/library/1/library-1.jar"
    );
}

#[test]
fn effective_authority_drives_health_without_removing_fallback_urls() {
    let alias = route(
        "https://effective-authority-alias.invalid/file.jar".to_string(),
        DownloadRouteSource::Mcim,
        true,
        true,
    );
    let direct = route(
        "https://effective-authority-target.invalid/file.jar".to_string(),
        DownloadRouteSource::Official,
        false,
        true,
    );
    remember_effective_route_authority(&alias, &direct.url);

    assert_eq!(
        route_health_key(&alias, ResourceClass::Other),
        route_health_key(&direct, ResourceClass::Other),
    );
    assert!(routes_share_effective_authority(&alias, &direct));

    let mut direct_without_proxy = direct.clone();
    direct_without_proxy.proxy = ProxyPolicy::Direct;
    let mut routes = vec![alias, direct, direct_without_proxy.clone()];
    deduplicate_download_routes(&mut routes);

    assert_eq!(routes.len(), 3);
    assert_eq!(routes[2], direct_without_proxy);
}

#[test]
fn route_deduplication_preserves_distinct_paths_on_one_authority() {
    let first = route(
        "https://mirror.example/maven/library.jar".to_string(),
        DownloadRouteSource::Bmclapi,
        true,
        true,
    );
    let mut second = first.clone();
    second.url = "https://mirror.example/libraries/library.jar".to_string();
    let mut routes = vec![first.clone(), second.clone(), first];

    deduplicate_download_routes(&mut routes);

    assert_eq!(
        routes,
        vec![
            route(
                "https://mirror.example/maven/library.jar".to_string(),
                DownloadRouteSource::Bmclapi,
                true,
                true,
            ),
            second,
        ]
    );
}

#[test]
fn effective_authority_memory_is_scoped_to_the_exact_route_url() {
    let redirected = route(
        "https://route-scope.example/cache-miss.jar".to_string(),
        DownloadRouteSource::Bmclapi,
        true,
        true,
    );
    let direct = route(
        "https://route-scope.example/cache-hit.jar".to_string(),
        DownloadRouteSource::Bmclapi,
        true,
        true,
    );
    remember_effective_route_authority(
        &redirected,
        "https://official.example/cache-miss.jar",
    );

    assert_eq!(
        effective_route_authority(&redirected).as_deref(),
        Some("official.example:443"),
    );
    assert_eq!(
        effective_route_authority(&direct).as_deref(),
        Some("route-scope.example:443"),
    );
}

#[test]
fn h2_route_selection_skips_an_open_breaker_when_possible() {
    let blocked = route(
        "https://blocked-h2.example/file".to_string(),
        DownloadRouteSource::Alternate,
        true,
        true,
    );
    let healthy = route(
        "https://healthy-h2.example/file".to_string(),
        DownloadRouteSource::Official,
        false,
        true,
    );
    for _ in 0..3 {
        crate::util::download::native_breaker::record_failure(&blocked);
    }

    assert_eq!(
        first_h2_route(&[blocked.clone(), healthy.clone()]),
        Some(healthy)
    );
    assert_eq!(
        first_h2_route(std::slice::from_ref(&blocked)),
        Some(blocked.clone())
    );
    crate::util::download::native_breaker::record_success(&blocked);
}

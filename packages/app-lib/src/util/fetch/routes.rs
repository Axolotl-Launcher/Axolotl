use super::model::{
    DownloadRoute, DownloadRouteSource, ProxyPolicy, ResourceClass,
};
use super::persisted_route_health;
use super::route_health_key;
use crate::util::download::constants::{
    ALIYUN_MAVEN_BASE_URL, BMCLAPI_BASE_URL, CURSEFORGE_API_HOST,
    CURSEFORGE_EDGE_HOST, FETCH_API, FETCH_AXOLOTL_RETRY,
    FETCH_BMCLAPI2_BANGBANG93_COM, FETCH_CDN, FETCH_FILES_MINECRAFTFORGE_NET,
    FETCH_JAVA_RUNTIME, FETCH_LAUNCHER_MOJANG_COM,
    FETCH_LIBRARIES_MINECRAFT_NET, FETCH_MAVEN, FETCH_MAVEN_ALIYUN_COM,
    FETCH_MAVEN2, FETCH_MEDIAFILEZ_FORGECDN_NET, FETCH_META_FABRICMC_NET,
    FETCH_MOD_MCIMIRROR_TOP, FETCH_MRPACK, FETCH_PISTON_DATA_MOJANG_COM,
    FETCH_RELEASES, FETCH_REPO_MAVEN_APACHE_ORG, FETCH_REPO1_MAVEN_ORG,
    FETCH_RESOURCES_DOWNLOAD_MINECRAFT_NET, HTTPS_SCHEME, MCIM_BASE_URL,
    MODRINTH_API_HOST, RETIRED_SOURCES_MEDIA_FORGECDN_NET,
    ROUTE_POLICY_LAUNCHERMETA_MOJANG_COM, ROUTE_POLICY_MAVEN_FABRICMC_NET,
    ROUTE_POLICY_MAVEN_MINECRAFTFORGE_NET, ROUTE_POLICY_MAVEN_NEOFORGED_NET,
    ROUTE_POLICY_PISTON_META_MOJANG_COM,
};
use crate::util::download::route_health::{ROUTE_HEALTH, RouteHealth};
use crate::util::download::{modrinth_cdn, retired_sources, route_policy};
use std::collections::HashSet;
#[cfg(test)]
use std::sync::atomic::Ordering;
use std::time::Instant;
use url::Url;

pub(crate) fn url_authority(url: &str) -> Option<String> {
    let url = Url::parse(url).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();
    Some(format!(
        "{host}:{}",
        url.port_or_known_default().unwrap_or(0)
    ))
}

pub(crate) fn original_route_authority(
    route: &DownloadRoute,
) -> Option<String> {
    url_authority(&route.url)
}

pub(crate) fn effective_route_authority(
    route: &DownloadRoute,
) -> Option<String> {
    crate::util::download::route_health::effective_route_authority(route)
}

pub(crate) fn remember_effective_route_authority(
    route: &DownloadRoute,
    final_url: &str,
) {
    crate::util::download::route_health::remember_effective_route_authority(
        route, final_url,
    )
}

pub(crate) fn forget_effective_route_authority(
    route: &DownloadRoute,
    failed_url: &Url,
) {
    crate::util::download::route_health::forget_effective_route_authority(
        route, failed_url,
    )
}

pub(crate) fn deduplicate_download_routes(routes: &mut Vec<DownloadRoute>) {
    let mut seen = HashSet::new();
    routes.retain(|route| seen.insert((route.url.clone(), route.proxy)));
}

pub(crate) fn first_h2_route(
    routes: &[DownloadRoute],
) -> Option<DownloadRoute> {
    routes
        .iter()
        .find(|route| !crate::util::download::native_breaker::is_open(route))
        .or_else(|| routes.first())
        .cloned()
}

pub(crate) fn routes_share_effective_authority(
    left: &DownloadRoute,
    right: &DownloadRoute,
) -> bool {
    left.proxy == right.proxy
        && effective_route_authority(left).is_some_and(|authority| {
            effective_route_authority(right).as_ref() == Some(&authority)
        })
}

pub(crate) fn modrinth_request_kind(url: &str) -> Option<&'static str> {
    if url.starts_with(env!("MODRINTH_API_URL"))
        || url.starts_with(env!("MODRINTH_API_URL_V3"))
    {
        Some(FETCH_API)
    } else if Url::parse(url)
        .is_ok_and(|url| modrinth_cdn::is_current_url(&url))
    {
        Some(FETCH_CDN)
    } else {
        None
    }
}

fn is_modrinth_api_url(url: &str) -> bool {
    Url::parse(url).ok().is_some_and(|parsed| {
        matches!(parsed.host_str(), Some(MODRINTH_API_HOST))
    })
}

/// Appends a cache-busting query parameter so a retry bypasses a corrupt
/// edge-cache object (a truncated or checksum-mismatched body served for an
/// otherwise valid URL). Existing query pairs are preserved.
pub(crate) fn cache_busted_download_url(url: &str, attempt: usize) -> String {
    let Ok(mut parsed) = Url::parse(url) else {
        return url.to_string();
    };
    let original_pairs = parsed
        .query_pairs()
        .filter(|(key, _)| key != FETCH_AXOLOTL_RETRY)
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();
    parsed.set_query(None);
    {
        let mut pairs = parsed.query_pairs_mut();
        for (key, value) in original_pairs {
            pairs.append_pair(&key, &value);
        }
        pairs.append_pair(FETCH_AXOLOTL_RETRY, &attempt.to_string());
    }
    parsed.into()
}

pub(crate) fn sanitize_url_for_log(url: &str) -> String {
    if let Ok(mut url) = Url::parse(url) {
        let _ = url.set_username("");
        let _ = url.set_password(None);
        url.set_query(None);
        url.set_fragment(None);
        return url.into();
    }

    url.split(['?', '#']).next().unwrap_or(url).to_string()
}

pub(crate) fn is_mrpack_url(url: &str) -> bool {
    reqwest::Url::parse(url).ok().is_some_and(|url| {
        url.path().to_ascii_lowercase().ends_with(FETCH_MRPACK)
    })
}

pub(crate) fn route(
    url: String,
    source: DownloadRouteSource,
    is_mirror: bool,
    supports_range: bool,
) -> DownloadRoute {
    DownloadRoute {
        url,
        source,
        is_mirror,
        allow_sensitive_headers: !is_mirror,
        supports_range,
        proxy: ProxyPolicy::System,
    }
}

pub(crate) fn official_route(
    url: &str,
    resource: ResourceClass,
) -> DownloadRoute {
    let url = modrinth_cdn::normalize(url).into_owned();
    let parsed_url = Url::parse(&url).ok();
    let source = parsed_url
        .as_ref()
        .and_then(|url| url.host_str().map(str::to_string))
        .map_or(DownloadRouteSource::Official, |host| match host.as_str() {
            FETCH_BMCLAPI2_BANGBANG93_COM => DownloadRouteSource::Bmclapi,
            FETCH_MOD_MCIMIRROR_TOP => DownloadRouteSource::Mcim,
            FETCH_MAVEN_ALIYUN_COM => DownloadRouteSource::Aliyun,
            _ => DownloadRouteSource::Official,
        });
    let is_mirror = matches!(
        source,
        DownloadRouteSource::Bmclapi
            | DownloadRouteSource::Mcim
            | DownloadRouteSource::Aliyun
    );
    let route = route(
        url,
        source,
        is_mirror,
        !matches!(resource, ResourceClass::Metadata),
    );
    #[cfg(test)]
    let route = {
        let mut route = route;
        if parsed_url
            .as_ref()
            .and_then(|url| url.host_str().and_then(|host| host.parse().ok()))
            .is_some_and(|address: std::net::IpAddr| address.is_loopback())
        {
            route.proxy = ProxyPolicy::Direct;
        }
        route
    };
    route
}

pub(crate) fn is_official_route(route: &DownloadRoute) -> bool {
    !route.is_mirror && route.source == DownloadRouteSource::Official
}

pub(crate) fn official_fallback_route(
    routes: &[DownloadRoute],
) -> Option<DownloadRoute> {
    routes
        .iter()
        .find(|candidate| is_official_route(candidate))
        .cloned()
}

fn url_with_base(original: &Url, base: &str, path: &str) -> Option<String> {
    let mut target = Url::parse(base).ok()?;
    target.set_path(path);
    target.set_query(original.query());
    target.set_fragment(None);
    Some(target.into())
}

pub(crate) fn explicit_mirror_routes(
    url: &str,
    resource: ResourceClass,
) -> Vec<DownloadRoute> {
    let Ok(parsed) = Url::parse(url) else {
        return Vec::new();
    };
    if parsed.scheme() != HTTPS_SCHEME {
        return Vec::new();
    }

    let host = parsed.host_str().unwrap_or_default();
    let path = parsed.path();
    let supports_range = !matches!(resource, ResourceClass::Metadata);
    let mut routes = Vec::new();
    let push_mirror = |routes: &mut Vec<DownloadRoute>,
                       base: &str,
                       path: String,
                       source: DownloadRouteSource| {
        // Disabled MCIM.
        if source == DownloadRouteSource::Mcim {
            return;
        }
        if let Some(url) = url_with_base(&parsed, base, &path) {
            routes.push(route(url, source, true, supports_range));
        }
    };

    match host {
        FETCH_RESOURCES_DOWNLOAD_MINECRAFT_NET => {
            push_mirror(
                &mut routes,
                BMCLAPI_BASE_URL,
                format!("/assets{path}"),
                DownloadRouteSource::Bmclapi,
            );
        }
        FETCH_LIBRARIES_MINECRAFT_NET => {
            push_mirror(
                &mut routes,
                BMCLAPI_BASE_URL,
                format!("/maven{path}"),
                DownloadRouteSource::Bmclapi,
            );
            push_mirror(
                &mut routes,
                BMCLAPI_BASE_URL,
                format!("/libraries{path}"),
                DownloadRouteSource::Bmclapi,
            );
        }
        // Maven Central hosts the bulk of loader-era libraries (Cleanroom,
        // LiteLoader, Babric, legacy Forge processors). Aliyun's public
        // repository mirrors it 1:1 and is reachable without a proxy, which
        // keeps a flaky proxy from stalling a whole install on one authority.
        FETCH_REPO_MAVEN_APACHE_ORG | FETCH_REPO1_MAVEN_ORG
            if path.starts_with(FETCH_MAVEN2) =>
        {
            push_mirror(
                &mut routes,
                ALIYUN_MAVEN_BASE_URL,
                format!(
                    "/repository/public{}",
                    path.trim_start_matches("/maven2")
                ),
                DownloadRouteSource::Aliyun,
            );
        }
        ROUTE_POLICY_MAVEN_MINECRAFTFORGE_NET
        | ROUTE_POLICY_MAVEN_FABRICMC_NET => {
            push_mirror(
                &mut routes,
                BMCLAPI_BASE_URL,
                format!("/maven{path}"),
                DownloadRouteSource::Bmclapi,
            );
        }
        FETCH_FILES_MINECRAFTFORGE_NET if path.starts_with(FETCH_MAVEN) => {
            push_mirror(
                &mut routes,
                BMCLAPI_BASE_URL,
                path.to_string(),
                DownloadRouteSource::Bmclapi,
            );
        }
        ROUTE_POLICY_MAVEN_NEOFORGED_NET
            if path.starts_with(FETCH_RELEASES) =>
        {
            push_mirror(
                &mut routes,
                BMCLAPI_BASE_URL,
                format!("/maven/{}", path.trim_start_matches("/releases/")),
                DownloadRouteSource::Bmclapi,
            );
        }
        FETCH_META_FABRICMC_NET => {
            push_mirror(
                &mut routes,
                BMCLAPI_BASE_URL,
                format!("/fabric-meta{path}"),
                DownloadRouteSource::Bmclapi,
            );
        }
        ROUTE_POLICY_PISTON_META_MOJANG_COM
        | ROUTE_POLICY_LAUNCHERMETA_MOJANG_COM
        | FETCH_LAUNCHER_MOJANG_COM
        | FETCH_PISTON_DATA_MOJANG_COM => push_mirror(
            &mut routes,
            BMCLAPI_BASE_URL,
            path.to_string(),
            DownloadRouteSource::Bmclapi,
        ),
        CURSEFORGE_API_HOST => push_mirror(
            &mut routes,
            MCIM_BASE_URL,
            format!("/curseforge{path}"),
            DownloadRouteSource::Mcim,
        ),
        _ => {}
    }

    routes
}

pub(crate) fn route_host(route: &DownloadRoute) -> Option<String> {
    Url::parse(&route.url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_string))
}

pub(crate) fn is_official_modrinth_download_url(url: &str) -> bool {
    route_policy::is_official_modrinth_download_url(url)
}

fn is_official_version_manifest_url(url: &str) -> bool {
    route_policy::is_official_version_manifest_url(url)
}

pub(crate) fn order_auto_routes(
    routes: &mut [DownloadRoute],
    resource: ResourceClass,
) {
    let health = ROUTE_HEALTH.lock().clone();
    routes.sort_by(|left, right| {
        let route_health = |route: &DownloadRoute| {
            let Some(key) = route_health_key(route, resource) else {
                return RouteHealth::default();
            };
            health
                .get(&key)
                .cloned()
                .unwrap_or_else(|| persisted_route_health(&key, route.proxy))
        };
        let left_health = route_health(left);
        let right_health = route_health(right);
        let now = Instant::now();
        let left_cooling =
            left_health.cooldown_until.is_some_and(|until| until > now);
        let right_cooling =
            right_health.cooldown_until.is_some_and(|until| until > now);
        left_cooling
            .cmp(&right_cooling)
            .then_with(|| {
                left_health
                    .consecutive_failures
                    .cmp(&right_health.consecutive_failures)
            })
            .then_with(|| {
                // Automatic content downloads start from the supplied
                // official URL. Health can still move a failing official
                // route behind a mirror, but connectivity alone is not a
                // reason to call a mirror faster.
                (!is_official_route(left)).cmp(&(!is_official_route(right)))
            })
            .then_with(|| {
                (right_health.success_samples > 0)
                    .cmp(&(left_health.success_samples > 0))
            })
            .then_with(|| {
                let ordering = if matches!(resource, ResourceClass::Metadata) {
                    left_health.ttfb_ms.partial_cmp(&right_health.ttfb_ms)
                } else {
                    right_health
                        .throughput_bps
                        .partial_cmp(&left_health.throughput_bps)
                };
                ordering.unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| left.is_mirror.cmp(&right.is_mirror))
    });
}

pub fn resolve_download_routes_for(
    url: &str,
    resource: ResourceClass,
    mode: crate::state::DownloadSourceMode,
) -> Vec<DownloadRoute> {
    let Ok(url) = retired_sources::normalize(url) else {
        return Vec::new();
    };
    let url = url.into_owned();
    let official = official_route(&url, resource);
    let mut routes = explicit_mirror_routes(&url, resource);
    routes.push(official);
    // Modrinth API calls are authenticated and remain official-only.
    let mode = if is_modrinth_api_url(&url) {
        crate::state::DownloadSourceMode::OfficialOnly
    } else {
        mode
    };
    match mode {
        crate::state::DownloadSourceMode::Auto
            if is_official_version_manifest_url(&url) =>
        {
            routes.sort_by_key(|route| route.is_mirror)
        }
        crate::state::DownloadSourceMode::Auto => {
            order_auto_routes(&mut routes, resource)
        }
        crate::state::DownloadSourceMode::OfficialOnly => {
            routes.retain(|route| !route.is_mirror);
        }
        crate::state::DownloadSourceMode::OfficialPreferred => {
            routes.sort_by_key(|route| route.is_mirror);
        }
        crate::state::DownloadSourceMode::MirrorPreferred => {
            routes.sort_by_key(|route| !route.is_mirror);
        }
    }
    routes
}

pub(crate) fn source_mode_for_resource(
    resource: ResourceClass,
) -> crate::state::DownloadSourceMode {
    let Some(state) = crate::State::get_if_initialized() else {
        return crate::state::DownloadSourceMode::OfficialOnly;
    };

    match resource {
        ResourceClass::Metadata => state.minecraft_metadata_source(),
        ResourceClass::MinecraftAsset
        | ResourceClass::MinecraftLibrary
        | ResourceClass::Loader
        | ResourceClass::Java => state.minecraft_file_source(),
        ResourceClass::Modrinth | ResourceClass::Modpack => {
            state.modrinth_source()
        }
        ResourceClass::CurseForge => state.curseforge_source(),
        ResourceClass::Other => crate::state::DownloadSourceMode::OfficialOnly,
    }
}

pub(crate) fn infer_resource_class(url: &str) -> ResourceClass {
    let Ok(parsed) = Url::parse(url) else {
        return ResourceClass::Other;
    };
    let host = parsed.host_str().unwrap_or_default();
    match host {
        FETCH_RESOURCES_DOWNLOAD_MINECRAFT_NET => ResourceClass::MinecraftAsset,
        FETCH_LIBRARIES_MINECRAFT_NET => ResourceClass::MinecraftLibrary,
        ROUTE_POLICY_MAVEN_MINECRAFTFORGE_NET
        | FETCH_FILES_MINECRAFTFORGE_NET
        | ROUTE_POLICY_MAVEN_FABRICMC_NET
        | ROUTE_POLICY_MAVEN_NEOFORGED_NET
        | FETCH_META_FABRICMC_NET => ResourceClass::Loader,
        FETCH_REPO1_MAVEN_ORG | FETCH_REPO_MAVEN_APACHE_ORG => {
            ResourceClass::MinecraftLibrary
        }
        ROUTE_POLICY_PISTON_META_MOJANG_COM
            if parsed.path().contains(FETCH_JAVA_RUNTIME) =>
        {
            ResourceClass::Java
        }
        FETCH_PISTON_DATA_MOJANG_COM
            if parsed.path().contains(FETCH_JAVA_RUNTIME) =>
        {
            ResourceClass::Java
        }
        ROUTE_POLICY_PISTON_META_MOJANG_COM
        | ROUTE_POLICY_LAUNCHERMETA_MOJANG_COM => ResourceClass::Metadata,
        FETCH_LAUNCHER_MOJANG_COM | FETCH_PISTON_DATA_MOJANG_COM => {
            ResourceClass::MinecraftLibrary
        }
        MODRINTH_API_HOST | modrinth_cdn::HOST => ResourceClass::Modrinth,
        CURSEFORGE_API_HOST
        | CURSEFORGE_EDGE_HOST
        | RETIRED_SOURCES_MEDIA_FORGECDN_NET
        | FETCH_MEDIAFILEZ_FORGECDN_NET => ResourceClass::CurseForge,
        _ => ResourceClass::Other,
    }
}

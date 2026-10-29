//! Pure route classification helpers used by the download coordinator.

use crate::util::download::constants::{
    HTTPS_SCHEME, MODRINTH_API_HOST, ROUTE_POLICY_FABRICMC,
    ROUTE_POLICY_LAUNCHERMETA_MOJANG_COM, ROUTE_POLICY_MAVEN_FABRICMC_NET,
    ROUTE_POLICY_MAVEN_MINECRAFTFORGE_NET, ROUTE_POLICY_MAVEN_NEOFORGED_NET,
    ROUTE_POLICY_MINECRAFTFORGE, ROUTE_POLICY_NEOFORGED,
    ROUTE_POLICY_PISTON_META_MOJANG_COM, ROUTE_POLICY_VERSION_MANIFEST,
};
use crate::util::fetch::ResourceClass;
use url::Url;

pub(crate) fn is_official_modrinth_download_url(url: &str) -> bool {
    Url::parse(url).is_ok_and(|url| {
        super::modrinth_cdn::is_current_url(&url)
            || (url.scheme() == HTTPS_SCHEME
                && url.host_str() == Some(MODRINTH_API_HOST))
    })
}

pub(crate) fn is_official_version_manifest_url(url: &str) -> bool {
    Url::parse(url).is_ok_and(|url| {
        matches!(
            url.host_str(),
            Some(
                ROUTE_POLICY_PISTON_META_MOJANG_COM
                    | ROUTE_POLICY_LAUNCHERMETA_MOJANG_COM
            )
        ) && url.path().contains(ROUTE_POLICY_VERSION_MANIFEST)
    })
}

/// Identifies loader artifacts for route health and download diagnostics.
pub(crate) fn is_loader_repository(url: &str, resource: ResourceClass) -> bool {
    if !matches!(
        resource,
        ResourceClass::MinecraftLibrary | ResourceClass::Loader
    ) {
        return false;
    }
    let Ok(url) = Url::parse(url) else {
        return false;
    };
    if matches!(
        url.host_str(),
        Some(
            ROUTE_POLICY_MAVEN_MINECRAFTFORGE_NET
                | ROUTE_POLICY_MAVEN_FABRICMC_NET
                | ROUTE_POLICY_MAVEN_NEOFORGED_NET
        )
    ) {
        return true;
    }
    let path = url.path().to_ascii_lowercase();
    [
        ROUTE_POLICY_MINECRAFTFORGE,
        ROUTE_POLICY_FABRICMC,
        ROUTE_POLICY_NEOFORGED,
    ]
    .iter()
    .any(|loader| path.contains(loader))
}

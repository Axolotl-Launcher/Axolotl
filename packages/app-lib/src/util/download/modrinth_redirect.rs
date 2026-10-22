//! Modrinth CDN redirect normalization for native download requests.

use crate::util::download::constants::{
    MAX_REDIRECT_LOCATION_BYTES, MODRINTH_REDIRECT_HTTPS,
};
use url::Url;

use super::modrinth_cdn;

pub(crate) fn repair_official_redirect(
    original: &Url,
    redirect: &Url,
    location: &str,
) -> Option<Url> {
    let mut repaired = modrinth_cdn::normalize_parsed(redirect.clone());
    if !location.is_ascii()
        && modrinth_cdn::is_current_url(&repaired)
        && !original.path().is_empty()
    {
        repaired.set_path(original.path());
        repaired.set_query(original.query());
        repaired.set_fragment(original.fragment());
        return Some(repaired);
    }
    (repaired != *redirect).then_some(repaired)
}

pub(crate) fn is_official_redirect(location: Option<&str>) -> bool {
    let Some(location) = location.filter(|location| {
        location.len() <= MAX_REDIRECT_LOCATION_BYTES
            && location.is_ascii()
            && location.get(..8).is_some_and(|scheme| {
                scheme.eq_ignore_ascii_case(MODRINTH_REDIRECT_HTTPS)
            })
    }) else {
        return false;
    };
    Url::parse(location).is_ok_and(|url| {
        modrinth_cdn::is_current_url(&modrinth_cdn::normalize_parsed(url))
    })
}

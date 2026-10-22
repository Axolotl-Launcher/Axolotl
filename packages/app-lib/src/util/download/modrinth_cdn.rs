//! Compatibility for URLs saved before Modrinth retired its alternate CDN.

pub(crate) use crate::util::download::constants::MODRINTH_CDN_HOST as HOST;
use crate::util::download::constants::MODRINTH_RETIRED_CDN_HOST as RETIRED_HOST;
use crate::util::download::constants::{
    HTTPS_SCHEME,
    MODRINTH_CDN_REPLACING_A_VALIDATED_HOSTNAME_PRESERVES_URL_VALIDITY,
};
use std::borrow::Cow;
use url::Url;

pub(crate) fn is_current_url(url: &Url) -> bool {
    url.scheme() == HTTPS_SCHEME
        && url.host_str() == Some(HOST)
        && url.port_or_known_default() == Some(443)
        && url.username().is_empty()
        && url.password().is_none()
}

/// Replaces only the retired HTTPS authority, preserving the original suffix
/// byte-for-byte, including percent encoding, query parameters and fragments.
pub(crate) fn normalize(input: &str) -> Cow<'_, str> {
    let Ok(url) = Url::parse(input) else {
        return Cow::Borrowed(input);
    };
    if url.scheme() != HTTPS_SCHEME
        || url.host_str() != Some(RETIRED_HOST)
        || url.port_or_known_default() != Some(443)
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Cow::Borrowed(input);
    }
    let Some((scheme, rest)) = input.split_once("://") else {
        return Cow::Borrowed(input);
    };
    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..authority_end];
    if !authority.eq_ignore_ascii_case(RETIRED_HOST)
        && !authority.eq_ignore_ascii_case(&format!("{RETIRED_HOST}:443"))
    {
        return Cow::Borrowed(input);
    }
    Cow::Owned(format!("{scheme}://{HOST}{}", &rest[authority_end..]))
}

pub(crate) fn normalize_parsed(url: Url) -> Url {
    let normalized = normalize(url.as_str());
    if matches!(normalized, Cow::Borrowed(_)) {
        return url;
    }
    Url::parse(normalized.as_ref()).expect(
        MODRINTH_CDN_REPLACING_A_VALIDATED_HOSTNAME_PRESERVES_URL_VALIDITY,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retired_urls_preserve_encoded_paths_queries_and_fragments() {
        for prefix in [
            "https://cdn-alt.modrinth.com",
            "HTTPS://CDN-ALT.MODRINTH.COM:443",
        ] {
            let suffix =
                "/data/%e9%87%91/file+name%2B.jar?token=a%2bb&x=1&x=2#fragment";
            let url = format!("{prefix}{suffix}");
            let normalized = normalize(&url);
            assert_eq!(
                normalized,
                format!(
                    "{}://{HOST}{suffix}",
                    prefix.split_once("://").unwrap().0
                )
            );
            assert!(is_current_url(&Url::parse(&normalized).unwrap()));
        }
    }

    #[test]
    fn only_the_retired_public_https_authority_is_rewritten() {
        for url in [
            "https://cdn.modrinth.com/file.jar?x=%2b",
            "https://cdn-raw.modrinth.com/file.png",
            "http://cdn-alt.modrinth.com/file.jar",
            "https://cdn-alt.modrinth.com:8443/file.jar",
            "https://user:secret@cdn-alt.modrinth.com/file.jar",
            "https://cdn-alt.modrinth.com@evil.example/file.jar",
            "https://cdn-alt.modrinth.com.evil.example/file.jar",
            "https://evil.example/cdn-alt.modrinth.com",
            "invalid URL",
        ] {
            assert!(matches!(normalize(url), Cow::Borrowed(_)), "{url}");
        }
        assert_eq!(
            normalize("https://cdn-alt.modrinth.com"),
            "https://cdn.modrinth.com"
        );
        assert!(!is_current_url(
            &Url::parse("https://cdn-alt.modrinth.com/file.jar").unwrap()
        ));
    }
}

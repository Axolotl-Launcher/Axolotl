//! Restore official origins for saved URLs from retired download services.

use crate::util::download::constants::TIANPAO_RETIRED_HOST as TIANPAO_HOST;
use crate::util::download::constants::TIANPAO_UNRECOVERABLE_MESSAGE as UNRECOVERABLE_MESSAGE;
use crate::util::download::constants::{
    CURSEFORGE_EDGE_HOST, HTTPS_SCHEME, MODRINTH_DATA_PREFIX,
    RETIRED_SOURCES_FILES, RETIRED_SOURCES_MEDIA, RETIRED_SOURCES_MEDIA_2,
    RETIRED_SOURCES_MEDIA_FORGECDN_NET,
};
use std::borrow::Cow;
use url::Url;

pub(crate) fn is_retired(url: &Url) -> bool {
    url.host_str().is_some_and(|host| {
        host.trim_end_matches('.')
            .eq_ignore_ascii_case(TIANPAO_HOST)
    })
}

pub(crate) fn normalize(input: &str) -> crate::Result<Cow<'_, str>> {
    let Ok(url) = Url::parse(input) else {
        return Ok(Cow::Borrowed(input));
    };
    if !is_retired(&url) {
        return Ok(super::modrinth_cdn::normalize(input));
    }
    let error = || {
        crate::Error::from(crate::ErrorKind::InputError(
            UNRECOVERABLE_MESSAGE.to_string(),
        ))
    };
    if url.scheme() != HTTPS_SCHEME
        || url.port_or_known_default() != Some(443)
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(error());
    }
    let Some((scheme, rest)) = input.split_once("://") else {
        return Err(error());
    };
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..end];
    if !authority.eq_ignore_ascii_case(TIANPAO_HOST)
        && !authority.eq_ignore_ascii_case(&format!("{TIANPAO_HOST}:443"))
    {
        return Err(error());
    }
    let suffix = &rest[end..];
    let path = suffix.split(['?', '#']).next().unwrap_or_default();
    let (host, suffix) = if path.starts_with(MODRINTH_DATA_PREFIX)
        && url.path().starts_with(MODRINTH_DATA_PREFIX)
    {
        (super::modrinth_cdn::HOST, suffix)
    } else if path.starts_with(RETIRED_SOURCES_FILES)
        && url.path().starts_with(RETIRED_SOURCES_FILES)
    {
        (CURSEFORGE_EDGE_HOST, suffix)
    } else if path.starts_with(RETIRED_SOURCES_MEDIA)
        && url.path().starts_with(RETIRED_SOURCES_MEDIA)
    {
        (
            RETIRED_SOURCES_MEDIA_FORGECDN_NET,
            &suffix[RETIRED_SOURCES_MEDIA_2.len()..],
        )
    } else {
        return Err(error());
    };
    Ok(Cow::Owned(format!("{scheme}://{host}{suffix}")))
}

pub(crate) fn normalize_parsed(url: Url) -> crate::Result<Url> {
    let normalized = normalize(url.as_str())?;
    if matches!(normalized, Cow::Borrowed(_)) {
        return Ok(url);
    }
    Ok(Url::parse(normalized.as_ref())?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tianpao_saved_paths_restore_official_origins_without_reencoding() {
        for (path, expected) in [
            (
                "/data/p/%e9%87%91+%2B.jar?x=%2b&x=2#file",
                "cdn.modrinth.com/data/p/%e9%87%91+%2B.jar?x=%2b&x=2#file",
            ),
            (
                "/files/123/456/a%20b.jar",
                "edge.forgecdn.net/files/123/456/a%20b.jar",
            ),
            (
                "/media/attachments/123/icon.png?x=1",
                "media.forgecdn.net/attachments/123/icon.png?x=1",
            ),
        ] {
            assert_eq!(
                normalize(&format!("https://mod.tianpao.top{path}")).unwrap(),
                format!("https://{expected}")
            );
            assert_eq!(
                normalize(&format!("HTTPS://MOD.TIANPAO.TOP:443{path}"))
                    .unwrap(),
                format!("HTTPS://{expected}")
            );
        }
    }
    #[test]
    fn tianpao_unknown_or_unsafe_addresses_are_rejected() {
        for input in [
            "https://mod.tianpao.top/unknown",
            "https://mod.tianpao.top/data",
            "https://mod.tianpao.top/media",
            "http://mod.tianpao.top/data/file",
            "https://mod.tianpao.top:8443/files/file",
            "https://user@mod.tianpao.top/data/file",
            "https://mod.tianpao.top./data/file",
            "https://mod.tianpao.top/data/../unknown",
        ] {
            assert!(normalize(input).is_err(), "{input}");
        }
        for input in [
            "https://mod.tianpao.top.evil.example/data/file",
            "https://mod.tianpao.top@evil.example/file",
            "https://cdn.modrinth.com/data/file",
        ] {
            assert_eq!(normalize(input).unwrap(), input);
        }
    }
}

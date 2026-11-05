//! MCIM, the Modrinth and CurseForge mirror at <https://www.mcimirror.top>.
//!
//! The integration follows <https://www.mcimirror.top/guide/>: only the
//! domains in its replacement table change, file downloads try the official
//! CDN first, and a cached API response is trusted only while its cache dates
//! show that the mirror is still refreshing.

use crate::util::download::constants::{
    CURSEFORGE_EDGE_HOST, FETCH_MOD_MCIMIRROR_TOP, MCIM_CURSEFORGE_PREFIX,
    MCIM_MODRINTH_PREFIX, MCIM_STALE_AFTER, MODRINTH_CDN_HOST,
};
use chrono::{DateTime, Utc};
use serde_json::Value;
use url::Url;

/// When MCIM last fetched an entry from upstream.
const SYNC_AT: &str = "sync_at";
/// When an MCIM refresh last confirmed an entry unchanged.
const CHECKED_AT: &str = "checked_at";
/// CurseForge wraps entries in a `data` envelope.
const CURSEFORGE_DATA: &str = "data";

/// Whether `url` is an official file CDN URL that MCIM mirrors.
///
/// MCIM answers those paths with a redirect to a third-party file mirror and
/// does not guarantee that downloads through it are stable, so it asks
/// launchers to download from the official CDN first and use the mirror only
/// when that fails.
pub(crate) fn mirrors_file_url(url: &str) -> bool {
    Url::parse(url).is_ok_and(|url| {
        matches!(
            url.host_str(),
            Some(MODRINTH_CDN_HOST | CURSEFORGE_EDGE_HOST)
        )
    })
}

/// Whether `url` addresses MCIM's mirrored APIs, whose JSON entries carry
/// cache dates.
pub(crate) fn is_api_url(url: &str) -> bool {
    Url::parse(url).is_ok_and(|url| {
        url.host_str() == Some(FETCH_MOD_MCIMIRROR_TOP)
            && [MCIM_MODRINTH_PREFIX, MCIM_CURSEFORGE_PREFIX]
                .into_iter()
                .any(|prefix| {
                    url.path()
                        .strip_prefix(prefix)
                        .is_some_and(|rest| rest.starts_with('/'))
                })
    })
}

/// Refuses an MCIM API response whose refresh has stalled.
///
/// MCIM serves a cache and leaves it to the launcher to decide whether a
/// response is current enough to trust. An entry is as current as the later
/// of its `sync_at` and `checked_at`, and a response as current as its oldest
/// entry. A response without cache dates (search, which passes through to
/// upstream) is accepted.
pub(crate) fn check_cache_dates(body: &[u8]) -> crate::Result<()> {
    check_cache_dates_at(body, Utc::now())
}

fn check_cache_dates_at(body: &[u8], now: DateTime<Utc>) -> crate::Result<()> {
    let Some(oldest) = oldest_cache_date(body) else {
        return Ok(());
    };
    if now
        .signed_duration_since(oldest)
        .to_std()
        .is_ok_and(|age| age > MCIM_STALE_AFTER)
    {
        return Err(crate::ErrorKind::OtherError(format!(
            "MCIM last refreshed this response at {}, too long ago to trust",
            oldest.to_rfc3339()
        ))
        .into());
    }
    Ok(())
}

fn oldest_cache_date(body: &[u8]) -> Option<DateTime<Utc>> {
    let value = serde_json::from_slice::<Value>(body).ok()?;
    let entries = match value.get(CURSEFORGE_DATA) {
        Some(data @ (Value::Array(_) | Value::Object(_))) => data,
        _ => &value,
    };
    match entries {
        Value::Array(entries) => {
            entries.iter().filter_map(entry_cache_date).min()
        }
        entry => entry_cache_date(entry),
    }
}

fn entry_cache_date(entry: &Value) -> Option<DateTime<Utc>> {
    [SYNC_AT, CHECKED_AT]
        .into_iter()
        .filter_map(|field| {
            DateTime::parse_from_rfc3339(entry.get(field)?.as_str()?).ok()
        })
        .map(|date| date.with_timezone(&Utc))
        .max()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-11-05T08:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn check(body: Value) -> crate::Result<()> {
        check_cache_dates_at(&serde_json::to_vec(&body).unwrap(), now())
    }

    #[test]
    fn recent_entries_are_trusted() {
        check(json!({"id": "AANobbMI", "sync_at": "2026-11-05T07:53:14.646Z"}))
            .unwrap();
        check(json!([
            {"id": "a", "sync_at": "2026-11-04T09:00:00Z"},
            {"id": "b", "sync_at": "2026-11-05T07:54:52.901Z"},
        ]))
        .unwrap();
    }

    #[test]
    fn stalled_entries_are_refused() {
        assert!(
            check(json!({"id": "a", "sync_at": "2026-11-01T00:00:00Z"}))
                .is_err()
        );
        // One stalled part makes the whole response untrustworthy.
        assert!(
            check(json!([
                {"id": "a", "sync_at": "2026-11-05T07:00:00Z"},
                {"id": "b", "sync_at": "2026-11-02T00:00:00Z"},
            ]))
            .is_err()
        );
    }

    #[test]
    fn a_recent_check_keeps_an_unchanged_entry_current() {
        check(json!({
            "id": "a",
            "sync_at": "2026-09-01T00:00:00Z",
            "checked_at": "2026-11-05T06:00:00Z",
        }))
        .unwrap();
        assert!(
            check(json!({
                "id": "a",
                "sync_at": "2026-09-01T00:00:00Z",
                "checked_at": null,
            }))
            .is_err()
        );
    }

    #[test]
    fn curseforge_envelopes_are_unwrapped() {
        check(
            json!({"data": {"id": 238222, "sync_at": "2026-11-05T07:00:00Z"}}),
        )
        .unwrap();
        assert!(
            check(json!({
                "data": [{"id": 1, "sync_at": "2026-10-01T00:00:00Z"}],
                "pagination": {"index": 0},
            }))
            .is_err()
        );
    }

    #[test]
    fn responses_without_cache_dates_are_accepted() {
        check(json!({"hits": [], "total_hits": 0})).unwrap();
        check(json!([])).unwrap();
        check_cache_dates_at(b"not json", now()).unwrap();
    }

    #[test]
    fn only_mirrored_hosts_and_apis_are_recognised() {
        assert!(mirrors_file_url(
            "https://cdn.modrinth.com/data/AANobbMI/versions/x/sodium.jar"
        ));
        assert!(mirrors_file_url(
            "https://edge.forgecdn.net/files/1/2/a.jar"
        ));
        assert!(!mirrors_file_url(
            "https://mediafilez.forgecdn.net/files/1/2/a.jar"
        ));
        assert!(!mirrors_file_url("https://api.modrinth.com/v2/project/a"));

        assert!(is_api_url(
            "https://mod.mcimirror.top/modrinth/v2/project/a"
        ));
        assert!(is_api_url("https://mod.mcimirror.top/curseforge/v1/mods/1"));
        assert!(!is_api_url("https://mod.mcimirror.top/data/a/versions/b/c"));
        assert!(!is_api_url("https://mod.mcimirror.top/modrinthx/v2/a"));
        assert!(!is_api_url("https://example.com/modrinth/v2/project/a"));
    }
}

//! Fetch policy tests.

use crate::util::download::constants::RETRY_AFTER_CAP;
use std::time::Duration;
use url::Url;

use super::super::*;

#[test]
fn fetch_retries_use_short_jittered_backoff() {
    let cases = [
        (1, Duration::from_millis(85), Duration::from_millis(115)),
        (2, Duration::from_millis(255), Duration::from_millis(345)),
        (3, Duration::from_millis(637), Duration::from_millis(863)),
        (4, Duration::from_millis(637), Duration::from_millis(863)),
    ];
    for (attempt, minimum, maximum) in cases {
        let delay = fetch_retry_delay(attempt);
        assert!(delay >= minimum, "attempt {attempt}: {delay:?}");
        assert!(delay <= maximum, "attempt {attempt}: {delay:?}");
    }
}

#[test]
fn retry_after_is_allowed_up_to_its_own_cap() {
    assert_eq!(
        clamp_failure_cooldown(Duration::from_secs(30)),
        Duration::from_secs(30),
    );
    assert_eq!(
        clamp_failure_cooldown(Duration::from_secs(3600)),
        RETRY_AFTER_CAP
    );
}

#[test]
fn source_matching_is_origin_safe() {
    assert!(same_origin(
        &Url::parse("https://api.curseforge.com/v1/mods").unwrap(),
        &Url::parse("https://api.curseforge.com/v1/files").unwrap(),
    ));
    assert!(!same_origin(
        &Url::parse("https://api.curseforge.com/v1/mods").unwrap(),
        &Url::parse("https://edge.forgecdn.net/files/1/2/a.jar").unwrap(),
    ));
    assert!(is_sensitive_header("x-api-key"));
    assert!(!header_requires_official_only("x-api-key"));
    assert!(is_sensitive_header("Authorization"));
    assert!(header_requires_official_only("Authorization"));
    assert!(!is_sensitive_header("accept"));
}

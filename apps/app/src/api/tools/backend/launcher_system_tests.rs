use super::*;
#[test]
fn browser_urls_do_not_accept_credentials_local_targets_or_executable_schemes()
{
    assert!(safe_url("https://www.minecraft.net/en-us/article/test").is_ok());
    for url in [
        "file:///C:/Windows/a.exe",
        "javascript:alert(1)",
        "http://example.com",
        "https://user:pass@example.com",
        "https://localhost",
        "https://localhost:123",
        "https://127.0.0.1",
        "https://[::1]",
        "https://a.localhost",
    ] {
        assert!(safe_url(url).is_err(), "{url}");
    }
}
#[test]
fn checked_update_handles_cannot_cross_control_sessions() {
    let registry = launcher_tools::ScopedRegistry::default();
    let id = registry
        .insert(
            "owner",
            1,
            UpdateHandle {
                rid: 123,
                version: "2.0.0".into(),
                _lease: None,
            },
        )
        .unwrap();
    assert!(registry.get("other", id, 1).is_err());
    assert!(registry.get("owner", id, 2).is_err());
    assert_eq!(registry.take("owner", id, 1).unwrap().version, "2.0.0");
    assert!(registry.take("owner", id, 1).is_err());
}

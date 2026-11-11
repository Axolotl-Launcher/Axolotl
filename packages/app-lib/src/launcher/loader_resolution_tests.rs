use super::*;
use daedalus::minecraft::RuleAction;

#[tokio::test]
async fn installed_launch_profile_never_calls_online_resolution() {
    let profile = prefer_local_launch_profile(Some("1.20.1-47.4.28"), || async {
            panic!("installed profile must launch even if the catalogs are unavailable")
        }).await.unwrap();
    assert_eq!(profile, "1.20.1-47.4.28");
    let mut calls = 0;
    let restored = prefer_local_launch_profile(None, || async {
        calls += 1;
        Ok("restored")
    })
    .await
    .unwrap();
    assert_eq!(restored, "restored");
    assert_eq!(calls, 1);
    let error = prefer_local_launch_profile::<(), _, _>(None, || async {
        Err(crate::ErrorKind::OtherError("recovery failed".into()).as_error())
    })
    .await
    .unwrap_err();
    assert!(error.to_string().contains("recovery failed"));
}

#[tokio::test]
async fn exact_loader_fallback_handles_successful_incomplete_catalogs_and_errors()
 {
    for primary in [
        Ok(None),
        Err(crate::ErrorKind::OtherError("official unavailable".into())
            .as_error()),
    ] {
        let result = resolve_exact_loader_fallback(primary, || async {
            Ok(Some(loader_version("47.4.28", false)))
        })
        .await
        .unwrap()
        .unwrap();
        assert_eq!(result.id, "47.4.28");
    }
    let result = resolve_exact_loader_fallback(
        Ok(Some(loader_version("47.4.28", false))),
        || async { panic!("found version must not consult another source") },
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(result.id, "47.4.28");
    assert!(
        resolve_exact_loader_fallback(Ok(None), || async { Ok(None) })
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        resolve_exact_loader_fallback(Ok(None), || async {
            Err(crate::ErrorKind::OtherError("fallback unavailable".into())
                .as_error())
        })
        .await
        .unwrap_err()
        .to_string()
        .contains("fallback unavailable")
    );
}

#[test]
fn disallow_only_library_rules_are_not_downloaded() {
    let rules = [d::minecraft::Rule {
        action: RuleAction::Disallow,
        os: None,
        features: None,
    }];

    assert!(!parse_rules(&rules, "8", &QuickPlayType::None, false,));
    assert!(parse_rules(&[], "8", &QuickPlayType::None, false,));
}

fn loader_version(id: &str, stable: bool) -> LoaderVersion {
    LoaderVersion {
        id: id.to_string(),
        url: format!("https://example.invalid/{id}"),
        stable,
        profile_source: Default::default(),
        fallback_url: None,
    }
}

fn forge_manifest(ids: &[&str]) -> Manifest {
    Manifest {
        game_versions: vec![d::modded::Version {
            id: "1.20.1".into(),
            stable: true,
            version_group: None,
            loaders: ids.iter().map(|id| loader_version(id, false)).collect(),
        }],
        version_groups: vec![],
    }
}

#[tokio::test]
async fn missing_exact_loader_refreshes_once_and_keeps_requested_version() {
    let mut calls = vec![];
    let selected = resolve_loader_version_with_refresh(
        "1.20.1",
        Some("47.4.28"),
        None,
        |behaviour| {
            calls.push(behaviour);
            let manifest = if behaviour == Some(CacheBehaviour::Bypass) {
                forge_manifest(&["47.4.29", "47.4.28", "47.4.22"])
            } else {
                forge_manifest(&["47.4.22"])
            };
            std::future::ready(Ok(manifest))
        },
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(selected.id, "47.4.28");
    assert_eq!(calls, [None, Some(CacheBehaviour::Bypass)]);
}

#[tokio::test]
async fn loader_refresh_respects_offline_mode_and_stops_on_missing_version() {
    for (behaviour, count) in [
        (None, 2),
        (Some(CacheBehaviour::CacheOnly), 1),
        (Some(CacheBehaviour::Bypass), 1),
    ] {
        let mut calls = 0;
        let result = resolve_loader_version_with_refresh(
            "1.20.1",
            Some("47.4.28"),
            behaviour,
            |_| {
                calls += 1;
                std::future::ready(Ok(forge_manifest(&["47.4.22"])))
            },
        )
        .await
        .unwrap();
        assert!(result.is_none());
        assert_eq!(calls, count);
    }
}

#[tokio::test]
async fn matching_loader_does_not_refresh_and_refresh_errors_are_preserved() {
    let mut calls = 0;
    let result = resolve_loader_version_with_refresh(
        "1.20.1",
        Some("47.4.28"),
        None,
        |_| {
            calls += 1;
            std::future::ready(Ok(forge_manifest(&["47.4.28"])))
        },
    )
    .await
    .unwrap();
    assert!(result.is_some());
    assert_eq!(calls, 1);

    let result = resolve_loader_version_with_refresh(
        "1.20.1",
        Some("47.4.28"),
        None,
        |behaviour| {
            std::future::ready(if behaviour == Some(CacheBehaviour::Bypass) {
                Err(crate::ErrorKind::OtherError("metadata unavailable".into())
                    .as_error())
            } else {
                Ok(forge_manifest(&[]))
            })
        },
    )
    .await;
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("metadata unavailable")
    );
}

#[test]
fn exact_game_version_precedes_generic_manifest_entry() {
    let mut manifest = forge_manifest(&["47.4.28"]);
    let mut generic = manifest.game_versions[0].clone();
    generic.id = d::modded::DUMMY_REPLACE_STRING.into();
    generic.loaders = vec![loader_version("47.4.22", false)];
    manifest.game_versions.insert(0, generic);
    let loaders =
        loader_versions_for_game_version(&manifest, "1.20.1").unwrap();
    assert_eq!(
        select_loader_version(loaders, Some("47.4.28")).unwrap().id,
        "47.4.28"
    );
}

#[test]
fn liteloader_runtime_detection_uses_the_maven_coordinate() {
    assert!(is_liteloader_library(
        "com.mumfrey:liteloader:1.12.2-SNAPSHOT"
    ));
    assert!(!is_liteloader_library("example:liteloader:1.12.2-SNAPSHOT"));
    assert!(!is_liteloader_library("com.mumfrey:liteloader"));
}

#[test]
fn repair_exact_loader_selection_never_substitutes_or_persists_fallback() {
    let versions = vec![
        loader_version("47.4.22", true),
        loader_version("47.2.0", false),
    ];

    assert_eq!(
        select_loader_version(&versions, Some("47.2.0"))
            .map(|version| version.id.as_str()),
        Some("47.2.0")
    );
    assert!(select_loader_version(&versions, Some("47.1.0")).is_none());
    assert_eq!(
        select_loader_version(&versions, None)
            .map(|version| version.id.as_str()),
        Some("47.4.22")
    );
    assert_eq!(explicit_loader_version(Some("47.1.0")), Some("47.1.0"));
    assert_eq!(explicit_loader_version(None), None);
    assert_eq!(explicit_loader_version(Some("latest")), None);
    assert_eq!(explicit_loader_version(Some("stable")), None);
    assert_eq!(
        missing_loader_version_fallback(
            ModLoader::Forge,
            "1.20.1",
            Some("47.1.0")
        ),
        Err("Loader version 47.1.0 is not available for forge 1.20.1"
            .to_string())
    );
    assert_eq!(
        missing_loader_version_fallback(ModLoader::Forge, "1.20.1", None),
        Ok("stable")
    );
}

#[test]
fn installed_instances_use_persisted_exact_loader_offline() {
    for loader in [
        ModLoader::Fabric,
        ModLoader::Forge,
        ModLoader::NeoForge,
        ModLoader::Quilt,
    ] {
        let version = installed_offline_loader_version(
            loader,
            Some("persisted-exact-version"),
        )
        .expect("installed exact loader should not require metadata");
        assert_eq!(version.id, "persisted-exact-version", "{loader:?}");
        assert!(version.url.is_empty(), "{loader:?}");
    }

    assert!(
        installed_offline_loader_version(ModLoader::Forge, Some("latest"))
            .is_none()
    );
    assert!(installed_offline_loader_version(ModLoader::Forge, None).is_none());
}

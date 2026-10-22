mod fixtures;

use content_management::Version;
use serde_json::{Value, from_str};

fn mr_primary(value: &Value) -> &Value {
    value["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|file| file["primary"] == true)
        .unwrap()
}

fn cf_sha1(value: &Value) -> &str {
    value["hashes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|hash| hash["algo"] == 1)
        .unwrap()["value"]
        .as_str()
        .unwrap()
}

#[test]
fn iris_common_bytes_have_different_provider_dependency_declarations() {
    let mr: Value = from_str(fixtures::MR_IRIS).unwrap();
    let cf: Value = from_str(fixtures::CF_IRIS).unwrap();
    assert_eq!(mr_primary(&mr)["hashes"]["sha1"], cf_sha1(&cf));
    assert_eq!(mr_primary(&mr)["size"], cf["fileLength"]);
    let version: Version = from_str(fixtures::MR_IRIS).unwrap();
    assert_eq!(version.id, "G5dd9TM4");
    assert!(version.dependencies.is_empty());
    assert!(
        cf["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|edge| edge["modId"] == 394468 && edge["relationType"] == 3)
    );
}

#[test]
fn balm_common_project_history_does_not_make_current_bytes_equivalent() {
    let mr43: Value = from_str(fixtures::MR_BALM_43).unwrap();
    let cf43: Value = from_str(fixtures::CF_BALM_43).unwrap();
    let mr44: Value = from_str(fixtures::MR_BALM_44).unwrap();
    let cf44: Value = from_str(fixtures::CF_BALM_44).unwrap();
    assert_eq!(mr43["project_id"], mr44["project_id"]);
    assert_eq!(cf43["modId"], cf44["modId"]);
    assert_eq!(mr_primary(&mr43)["hashes"]["sha1"], cf_sha1(&cf43));
    assert_eq!(mr_primary(&mr43)["size"], cf43["fileLength"]);
    assert_eq!(mr_primary(&mr44)["filename"], cf44["fileName"]);
    assert_eq!(mr_primary(&mr44)["size"], cf44["fileLength"]);
    assert_ne!(mr_primary(&mr44)["hashes"]["sha1"], cf_sha1(&cf44));
    assert_eq!(cf_sha1(&cf44), "b3f8a6a5823d3b4f2dce19c8aea9373e24bcb443");
    for sample in [fixtures::MR_BALM_43, fixtures::MR_BALM_44] {
        let release: Version = from_str(sample).unwrap();
        assert!(
            release
                .game_versions
                .iter()
                .any(|version| version == "1.20.1")
        );
        assert_eq!(release.loaders, ["forge"]);
    }
}

#[test]
fn itemphysic_search_baseline_retains_full_lite_and_unofficial_projects() {
    let mr: Value = from_str(fixtures::MR_SEARCH).unwrap();
    let cf: Value = from_str(fixtures::CF_SEARCH).unwrap();
    let mr_hits = mr["hits"].as_array().unwrap();
    let cf_hits = cf["data"].as_array().unwrap();
    assert!(mr_hits.iter().any(|hit| hit["title"] == "ItemPhysic"));
    assert!(mr_hits.iter().any(|hit| hit["title"] == "ItemPhysic Lite"));
    for slug in [
        "itemphysic",
        "itemphysic-lite",
        "itemphysic-lite-fabric",
        "itemphysic-1-7-10-unofficial",
    ] {
        assert!(cf_hits.iter().any(|hit| hit["slug"] == slug));
    }
    let ids: std::collections::HashSet<_> = cf_hits
        .iter()
        .map(|hit| hit["id"].as_u64().unwrap())
        .collect();
    assert_eq!(ids.len(), cf_hits.len());
}

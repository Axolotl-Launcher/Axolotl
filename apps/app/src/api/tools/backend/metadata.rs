use super::{business_error, page};
use launcher_tools::{GameReleaseType, MetadataLoader, ToolError};
use serde_json::{Value, json};

fn loader_key(loader: MetadataLoader) -> &'static str {
    match loader {
        MetadataLoader::Fabric => "fabric",
        MetadataLoader::Forge => "forge",
        MetadataLoader::Quilt => "quilt",
        MetadataLoader::NeoForge => "neo",
        MetadataLoader::Optifine => "optifine",
        MetadataLoader::Cleanroom => "cleanroom",
        MetadataLoader::LiteLoader => "lite_loader",
        MetadataLoader::LegacyFabric => "legacy_fabric",
        MetadataLoader::Babric => "babric",
    }
}

pub(super) async fn games(
    release_type: Option<GameReleaseType>,
    search: Option<String>,
    offset: usize,
    limit: usize,
) -> Result<Value, ToolError> {
    let manifest = theseus::metadata::get_minecraft_versions()
        .await
        .map_err(business_error)?;
    let filter = release_type.map(|kind| match kind {
        GameReleaseType::Release => "release",
        GameReleaseType::Snapshot => "snapshot",
        GameReleaseType::OldAlpha => "old_alpha",
        GameReleaseType::OldBeta => "old_beta",
    });
    let search = search.map(|s| s.to_lowercase());
    let mut versions: Vec<_> = manifest
        .versions
        .into_iter()
        .filter(|version| {
            filter.is_none_or(|kind| version.type_.as_str() == kind)
                && search.as_ref().is_none_or(|query| {
                    version.id.to_lowercase().contains(query)
                })
        })
        .collect();
    versions.sort_by(|a, b| {
        b.release_time.cmp(&a.release_time).then(a.id.cmp(&b.id))
    });
    let mut result = page(versions.into_iter().map(|version|json!({"game_version":version.id,"release_type":version.type_,"released_at":version.release_time})).collect(),offset,limit)?;
    result["latest_release"] = json!(manifest.latest.release);
    result["latest_snapshot"] = json!(manifest.latest.snapshot);
    Ok(result)
}

pub(super) fn selected_loaders<'a>(
    manifest: &'a daedalus::modded::Manifest,
    game_version: &str,
) -> &'a [daedalus::modded::LoaderVersion] {
    // Match runtime resolution, including shared loader-profile groups and
    // manifests where one placeholder profile applies to many game versions.
    let version = manifest
        .game_versions
        .iter()
        .find(|version| version.id == game_version)
        .or_else(|| {
            manifest.game_versions.iter().find(|version| {
                version.id.replace(
                    daedalus::modded::DUMMY_REPLACE_STRING,
                    game_version,
                ) == game_version
            })
        });
    let Some(version) = version else {
        return &[];
    };
    if let Some(group) = &version.version_group {
        manifest
            .version_groups
            .iter()
            .find(|candidate| &candidate.id == group)
            .map_or(&[], |group| group.loaders.as_slice())
    } else {
        &version.loaders
    }
}

pub(super) async fn loaders(
    loader: MetadataLoader,
    game_version: String,
    stable_only: bool,
    offset: usize,
    limit: usize,
) -> Result<Value, ToolError> {
    let manifest = theseus::metadata::get_loader_versions_for_game(
        loader_key(loader),
        &game_version,
    )
    .await
    .map_err(business_error)?;
    page(selected_loaders(&manifest, &game_version).iter().filter(|version|!stable_only || version.stable)
        .map(|version|json!({"loader_version":version.id,"stable":version.stable})).collect(), offset,limit)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn manifest(value: Value) -> daedalus::modded::Manifest {
        serde_json::from_value(value).unwrap()
    }
    #[test]
    fn group_resolution_and_placeholder_match_runtime() {
        let manifest = manifest(
            json!({"gameVersions":[{"id":"${modrinth.gameVersion}","stable":true,"versionGroup":"shared","loaders":[]}],
            "versionGroups":[{"id":"shared","loaders":[{"id":"new","url":"https://example.com/profile","stable":false},{"id":"old","url":"https://example.com/profile","stable":true}]}]}),
        );
        let selected = selected_loaders(&manifest, "1.21.1");
        assert_eq!(
            selected.iter().map(|v| v.id.as_str()).collect::<Vec<_>>(),
            ["new", "old"]
        );
        assert_eq!(selected.iter().filter(|v| v.stable).count(), 1);
        assert_eq!(loader_key(MetadataLoader::NeoForge), "neo");
    }
    #[test]
    fn exact_version_wins_and_unknown_versions_are_empty() {
        let manifest = manifest(
            json!({"gameVersions":[{"id":"1.21","stable":true,"loaders":[{"id":"specific","url":"https://example.com/profile","stable":true}]}]}),
        );
        assert_eq!(selected_loaders(&manifest, "1.21")[0].id, "specific");
        assert!(selected_loaders(&manifest, "1.20").is_empty());
    }
}

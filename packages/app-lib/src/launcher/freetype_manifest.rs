use std::collections::HashSet;
use std::path::{Path, PathBuf};

use daedalus::minecraft::{LibraryDownload, OsRule};
use serde_json::{Map, Value, json};

use crate::util::{fetch, io};

use super::direct_link::LinkedLauncherDialect;
use super::{freetype, pcl_ce_json, version_json};

#[derive(Default)]
pub(super) struct PreparedManifestRepair {
    write: Option<ManifestWrite>,
    dialect: Option<LinkedLauncherDialect>,
}

struct ManifestWrite {
    path: PathBuf,
    original: Vec<u8>,
    contents: Vec<u8>,
    downloads: Vec<LibraryDownload>,
}

impl PreparedManifestRepair {
    pub(super) fn downloads(&self) -> &[LibraryDownload] {
        self.write
            .as_ref()
            .map_or(&[], |write| write.downloads.as_slice())
    }

    /// Publish atomically after verifying every JAR in the updated manifest.
    pub(super) async fn persist(self) -> crate::Result<()> {
        let Some(write) = self.write else {
            return Ok(());
        };
        let lock = fetch::destination_download_lock(&write.path);
        let _guard = lock.lock().await;
        let latest = io::read(&write.path).await?;
        if latest == write.original {
            io::write(write.path, write.contents).await?;
        } else if !matches!(
            patched_manifest(&write.path, &latest, self.dialect)?,
            ManifestPatch::Unchanged
        ) {
            return Err(crate::ErrorKind::FSError(format!(
                "Version manifest {} changed while updating FreeType",
                write.path.display()
            ))
            .into());
        }
        Ok(())
    }
}

/// Inspect the entire inheritance chain and prepare a single atomic write
/// when all changes belong to the same manifest.
pub(super) async fn prepare(
    paths: &[PathBuf],
    versions_dir: &Path,
    dialect: Option<LinkedLauncherDialect>,
) -> crate::Result<PreparedManifestRepair> {
    if paths.is_empty() {
        return Ok(PreparedManifestRepair::default());
    }
    let versions_dir = io::canonicalize(versions_dir)?;
    let mut seen = HashSet::new();
    let mut repair = PreparedManifestRepair {
        dialect,
        ..Default::default()
    };
    for path in paths {
        let path = io::canonicalize(path)?;
        if !path.starts_with(&versions_dir) || path == versions_dir {
            return Err(crate::ErrorKind::InputError(format!(
                "FreeType version manifest {} is outside {}",
                path.display(),
                versions_dir.display()
            ))
            .into());
        }
        if !seen.insert(path.clone()) {
            continue;
        }
        let bytes = io::read(&path).await?;
        match patched_manifest(&path, &bytes, dialect)? {
            ManifestPatch::Unchanged => {}
            // An untouched custom native declaration could still require the old
            // Java binding. Keep all manifests in the chain as they were.
            ManifestPatch::Unsupported => {
                return Ok(PreparedManifestRepair::default());
            }
            ManifestPatch::Changed {
                contents,
                mut downloads,
            } => {
                if repair.write.is_some() {
                    tracing::debug!(
                        "Keeping the FreeType repair in memory because it spans multiple version manifests"
                    );
                    return Ok(PreparedManifestRepair::default());
                }
                let mut artifact_paths = HashSet::new();
                downloads.retain(|download| {
                    artifact_paths.insert(download.path.clone())
                });
                repair.write = Some(ManifestWrite {
                    path,
                    original: bytes,
                    contents,
                    downloads,
                });
            }
        }
    }
    Ok(repair)
}

enum ManifestPatch {
    Unchanged,
    Unsupported,
    Changed {
        contents: Vec<u8>,
        downloads: Vec<LibraryDownload>,
    },
}

fn patched_manifest(
    path: &Path,
    bytes: &[u8],
    dialect: Option<LinkedLauncherDialect>,
) -> crate::Result<ManifestPatch> {
    let mut manifest = if dialect == Some(LinkedLauncherDialect::PclCe) {
        pcl_ce_json::parse_raw(path, bytes)?
    } else {
        version_json::parse_version_json(path, bytes, true)?
    };
    let mut downloads = Vec::new();
    if patch_manifest(&mut manifest, dialect, &mut downloads).is_err() {
        return Ok(ManifestPatch::Unsupported);
    }
    if downloads.is_empty() {
        return Ok(ManifestPatch::Unchanged);
    }
    let mut contents = serde_json::to_vec_pretty(&manifest)?;
    contents.push(b'\n');
    Ok(ManifestPatch::Changed {
        contents,
        downloads,
    })
}

fn patch_manifest(
    manifest: &mut Value,
    dialect: Option<LinkedLauncherDialect>,
    downloads: &mut Vec<LibraryDownload>,
) -> Result<(), ()> {
    let Some(manifest) = manifest.as_object_mut() else {
        return Ok(());
    };
    let case_insensitive = dialect == Some(LinkedLauncherDialect::PclCe);
    let pcl_patches = matches!(
        dialect,
        Some(LinkedLauncherDialect::Pcl | LinkedLauncherDialect::PclCe)
    ) && get_field(manifest, "time", case_insensitive)
        .is_none()
        && get_field(manifest, "patches", case_insensitive).is_some();
    let hmcl_patches = dialect == Some(LinkedLauncherDialect::Hmcl);
    let hmcl_empty_root = hmcl_patches
        && manifest.get("root").and_then(Value::as_bool) == Some(true)
        && manifest.get("patches").is_some_and(Value::is_array)
        && manifest.get("inheritsFrom").is_none_or(Value::is_null);

    if !pcl_patches && !hmcl_empty_root {
        patch_libraries(manifest, case_insensitive, downloads)?;
    }
    if (pcl_patches || hmcl_patches)
        && let Some(patches) =
            get_field_mut(manifest, "patches", case_insensitive)
                .and_then(Value::as_array_mut)
    {
        for patch in patches.iter_mut().filter_map(Value::as_object_mut) {
            if hmcl_patches
                || get_field(patch, "id", case_insensitive)
                    .is_some_and(|id| !id.is_null())
            {
                patch_libraries(patch, case_insensitive, downloads)?;
            }
        }
    }
    Ok(())
}

fn patch_libraries(
    manifest: &mut Map<String, Value>,
    case_insensitive: bool,
    artifacts: &mut Vec<LibraryDownload>,
) -> Result<(), ()> {
    let Some(libraries) =
        get_field_mut(manifest, "libraries", case_insensitive)
            .and_then(Value::as_array_mut)
    else {
        return Ok(());
    };
    for library in libraries.iter_mut().filter_map(Value::as_object_mut) {
        let Some(name) = get_field(library, "name", case_insensitive)
            .and_then(Value::as_str)
            .filter(|name| freetype::is_freetype_coordinate(name))
        else {
            continue;
        };
        let artifact = freetype::replacement(name).ok_or(())?;
        if !standard_library(library, artifact, case_insensitive) {
            return Err(());
        }
        let mut patched = library.clone();
        let download = artifact.download();
        *field(&mut patched, "name", case_insensitive) =
            Value::String(artifact.coordinate());
        if let Some(downloadable) =
            get_field_mut(&mut patched, "downloadable", case_insensitive)
        {
            *downloadable = Value::Bool(true);
        }
        if let Some(checksums) =
            get_field_mut(&mut patched, "checksums", case_insensitive)
        {
            *checksums = json!([download.sha1]);
        }
        let downloads =
            object(field(&mut patched, "downloads", case_insensitive));
        patch_download(
            field(downloads, "artifact", case_insensitive),
            &download,
            case_insensitive,
        );
        if let Some(target) = artifact.target() {
            restrict_native_rules(&mut patched, target, case_insensitive);
        }
        if patched != *library {
            *library = patched;
            artifacts.push(download);
        }
    }
    Ok(())
}

fn standard_library(
    library: &Map<String, Value>,
    artifact: &freetype::Artifact,
    case_insensitive: bool,
) -> bool {
    let read = |name| {
        get_field(library, name, case_insensitive)
            .filter(|value| !value.is_null())
    };
    [
        "hint",
        "filename",
        "MMC-hint",
        "MMC-filename",
        "natives",
        "extract",
    ]
    .iter()
    .all(|name| read(name).is_none())
        && ["downloadable", "include_in_classpath"].iter().all(|name| {
            read(name).is_none_or(|value| value.as_bool() == Some(true))
        })
        && read("downloads")
            .and_then(Value::as_object)
            .and_then(|downloads| {
                get_field(downloads, "classifiers", case_insensitive)
            })
            .is_none_or(|classifiers| {
                classifiers.is_null()
                    || classifiers.as_object().is_some_and(Map::is_empty)
            })
        && read("rules").is_none_or(|rules| {
            rules.as_array().is_some_and(|rules| {
                rules
                    .iter()
                    .all(|rule| standard_rule(rule, artifact, case_insensitive))
            })
        })
}

fn standard_rule(
    rule: &Value,
    artifact: &freetype::Artifact,
    case_insensitive: bool,
) -> bool {
    let Some(rule) = rule.as_object() else {
        return false;
    };
    if get_field(rule, "action", case_insensitive).and_then(Value::as_str)
        != Some("allow")
        || get_field(rule, "features", case_insensitive)
            .is_some_and(|features| !features.is_null())
    {
        return false;
    }
    let Some(os) =
        get_field(rule, "os", case_insensitive).filter(|os| !os.is_null())
    else {
        return true;
    };
    let Some(os) = os.as_object() else {
        return false;
    };
    let fields: Map<String, Value> = ["name", "arch", "version"]
        .into_iter()
        .filter_map(|key| {
            get_field(os, key, case_insensitive)
                .map(|value| (key.to_owned(), value.clone()))
        })
        .collect();
    serde_json::from_value::<OsRule>(Value::Object(fields))
        .is_ok_and(|os| artifact.supports_os_rule(&os))
}

fn patch_download(
    data: &mut Value,
    download: &LibraryDownload,
    case_insensitive: bool,
) {
    let data = object(data);
    *field(data, "path", case_insensitive) = json!(download.path);
    *field(data, "sha1", case_insensitive) = json!(download.sha1);
    *field(data, "size", case_insensitive) = json!(download.size);
    *field(data, "url", case_insensitive) = json!(download.url);
}

fn restrict_native_rules(
    library: &mut Map<String, Value>,
    target: OsRule,
    case_insensitive: bool,
) {
    let rules = field(library, "rules", case_insensitive);
    let target = json!(target);
    let Some(rules) = rules.as_array_mut() else {
        if rules.is_null() {
            *rules = json!([{ "action": "allow", "os": target }]);
        }
        return;
    };
    if rules.is_empty() {
        rules.push(json!({ "action": "allow", "os": target }));
        return;
    }
    for rule in rules.iter_mut().filter_map(Value::as_object_mut) {
        if get_field(rule, "action", case_insensitive).and_then(Value::as_str)
            != Some("allow")
        {
            continue;
        }
        let os = object(field(rule, "os", case_insensitive));
        let name = field(os, "name", case_insensitive);
        if name.is_null() || name.as_str() == Some("universal") {
            *name = target["name"].clone();
        }
        let arch = field(os, "arch", case_insensitive);
        if arch.is_null() {
            *arch = target["arch"].clone();
        }
    }
}

fn object(value: &mut Value) -> &mut Map<String, Value> {
    if !value.is_object() {
        *value = Value::Object(Map::new());
    }
    value.as_object_mut().expect("JSON object")
}

fn field_matches(key: &str, name: &str, case_insensitive: bool) -> bool {
    key == name || (case_insensitive && key.eq_ignore_ascii_case(name))
}

fn get_field<'a>(
    data: &'a Map<String, Value>,
    name: &str,
    case_insensitive: bool,
) -> Option<&'a Value> {
    data.iter()
        .find(|(key, _)| field_matches(key, name, case_insensitive))
        .map(|(_, value)| value)
}

fn get_field_mut<'a>(
    data: &'a mut Map<String, Value>,
    name: &str,
    case_insensitive: bool,
) -> Option<&'a mut Value> {
    data.iter_mut()
        .find(|(key, _)| field_matches(key, name, case_insensitive))
        .map(|(_, value)| value)
}

fn field<'a>(
    data: &'a mut Map<String, Value>,
    name: &str,
    case_insensitive: bool,
) -> &'a mut Value {
    let key = data
        .keys()
        .find(|key| field_matches(key, name, case_insensitive))
        .cloned()
        .unwrap_or_else(|| name.to_owned());
    data.entry(key).or_insert(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launcher::local_version::load_version_for_dialect;

    const OLD: &str = "org.lwjgl:lwjgl-freetype:3.4.3";
    const FIXED: &str = "org.lwjgl:lwjgl-freetype:3.4.3-mojang-2.15.0";

    fn write_manifest(dir: &Path, version: &str, manifest: Value) -> PathBuf {
        let dir = dir.join(version);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{version}.json"));
        std::fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        path
    }

    async fn persist(
        paths: &[PathBuf],
        versions_dir: &Path,
        dialect: Option<LinkedLauncherDialect>,
    ) -> crate::Result<()> {
        prepare(paths, versions_dir, dialect).await?.persist().await
    }

    #[tokio::test]
    async fn freetype_persist_updates_managed_version_json_idempotently() {
        let dir = tempfile::tempdir().unwrap();
        let versions = dir.path().join("versions");
        let path = write_manifest(
            &versions,
            "26.3",
            json!({
                "id": "26.3",
                "libraries": [
                    {"name": OLD},
                    {"name": format!("{OLD}:natives-linux"), "rules":[{"action":"allow", "os":{"name":"linux"}}]},
                ],
            }),
        );
        let paths = std::slice::from_ref(&path);
        let repair = prepare(paths, &versions, None).await.unwrap();
        assert_eq!(repair.downloads().len(), 2);
        assert!(repair.downloads().iter().any(|download| download.sha1
            == "134bc57b83e78a7f954a0f078093e319739d42ea"));
        assert!(repair.downloads().iter().any(|download| download.sha1
            == "f20ae220999240d484896d839694b02c2a655608"));
        repair.persist().await.unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let manifest: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(manifest["id"], "26.3");
        assert_eq!(manifest["libraries"][0]["name"], FIXED);
        assert_eq!(
            manifest["libraries"][1]["name"],
            format!("{FIXED}:natives-linux")
        );
        assert_eq!(
            manifest["libraries"][1]["rules"][0]["os"]["arch"],
            "x86_64"
        );
        assert_eq!(
            manifest["libraries"][1]["downloads"]["artifact"]["size"],
            1423204
        );
        persist(paths, &versions, None).await.unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }

    #[tokio::test]
    async fn freetype_persist_preserves_and_reloads_hmcl_inheritance() {
        let dir = tempfile::tempdir().unwrap();
        let versions = dir.path().join("versions");
        let parent = write_manifest(
            &versions,
            "26.3",
            json!({
                "id":"26.3",
                "launcherExtension": {"custom": 1},
                "libraries": [{
                    "name": OLD,
                    "downloads": {"artifact": {"extension": "keep"}},
                }, {"name": "org.example:unrelated:1.0.0", "custom": true}],
            }),
        );
        let child = write_manifest(
            &versions,
            "modded",
            json!({
                "id":"modded", "inheritsFrom":"26.3", "customChild": true,
            }),
        );
        let child_bytes = std::fs::read(&child).unwrap();
        let paths = [parent.clone(), child.clone(), parent.clone()];
        persist(&paths, &versions, Some(LinkedLauncherDialect::Hmcl))
            .await
            .unwrap();

        let new_parent_bytes = std::fs::read(&parent).unwrap();
        let parent: Value = serde_json::from_slice(&new_parent_bytes).unwrap();
        assert_eq!(parent["launcherExtension"]["custom"], 1);
        let libraries = parent["libraries"].as_array().unwrap();
        assert_eq!(libraries[0]["name"], FIXED);
        assert_eq!(libraries[0]["downloads"]["artifact"]["extension"], "keep");
        assert_eq!(
            libraries[0]["downloads"]["artifact"]["sha1"],
            "134bc57b83e78a7f954a0f078093e319739d42ea"
        );
        assert_eq!(
            libraries[1],
            json!({"name": "org.example:unrelated:1.0.0", "custom": true})
        );

        assert_eq!(std::fs::read(&child).unwrap(), child_bytes);
        let merged = load_version_for_dialect(
            dir.path(),
            "modded",
            Some(&child),
            LinkedLauncherDialect::Hmcl,
        )
        .unwrap();
        assert!(
            merged
                .libraries
                .iter()
                .any(|library| library.library.name == FIXED)
        );
        assert!(
            !merged
                .libraries
                .iter()
                .any(|library| library.library.name == OLD)
        );

        persist(&paths, &versions, Some(LinkedLauncherDialect::Hmcl))
            .await
            .unwrap();
        assert_eq!(std::fs::read(&paths[0]).unwrap(), new_parent_bytes);
    }

    #[tokio::test]
    async fn freetype_persist_patches_pcl_ce_source_fields_without_renaming() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("profile.json");
        std::fs::write(&path, format!(r#"{}{{
            // PCL-CE accepts comments, trailing commas and case-insensitive selectors.
            "ID": "profile", "MainClass": "keep.Main",
            "PATCHES": [
                {{"ID": null, "LIBRARIES": [{{"NAME": "{OLD}"}}]}},
                {{"ID": "game", "LIBRARIES": [{{
                    "NAME": "{OLD}:natives-windows-arm64",
                    "RULES": [{{"ACTION": "allow", "OS": {{"NAME": "windows", "custom": 1}}}}],
                    "DOWNLOADS": {{"ARTIFACT": {{"URL": "old", "x-owner": "keep"}}}},
                }}]}},
            ],
        }}"#, '\u{feff}')).unwrap();

        persist(
            std::slice::from_ref(&path),
            dir.path(),
            Some(LinkedLauncherDialect::PclCe),
        )
        .await
        .unwrap();
        let stored: Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(stored["MainClass"], "keep.Main");
        assert_eq!(stored["PATCHES"][0]["LIBRARIES"][0]["NAME"], OLD);
        let library = &stored["PATCHES"][1]["LIBRARIES"][0];
        assert_eq!(library["NAME"], format!("{FIXED}:natives-windows-arm64"));
        assert_eq!(
            library["RULES"][0]["OS"],
            json!({"NAME":"windows", "arch":"aarch64", "custom":1})
        );
        assert_eq!(library["DOWNLOADS"]["ARTIFACT"]["x-owner"], "keep");
        assert!(
            library["DOWNLOADS"]["ARTIFACT"]["URL"]
                .as_str()
                .unwrap()
                .starts_with("https://libraries.minecraft.net/")
        );
        assert!(pcl_ce_json::read(&path).is_ok());
    }

    #[tokio::test]
    async fn freetype_persist_rejects_a_manifest_outside_versions_before_writing()
     {
        let dir = tempfile::tempdir().unwrap();
        let versions = dir.path().join("versions");
        let parent = write_manifest(
            &versions,
            "26.3",
            json!({"libraries":[{"name":OLD}]}),
        );
        let outside = write_manifest(
            dir.path(),
            "elsewhere",
            json!({"libraries":[{"name":OLD}]}),
        );
        let before = std::fs::read(&parent).unwrap();
        let error = persist(&[parent.clone(), outside], &versions, None)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("outside"));
        assert_eq!(std::fs::read(&parent).unwrap(), before);
    }

    #[tokio::test]
    async fn freetype_persist_concurrent_launches_can_upgrade_shared_parent() {
        let dir = tempfile::tempdir().unwrap();
        let versions = dir.path().join("versions");
        let parent = write_manifest(
            &versions,
            "26.3",
            json!({"id":"26.3", "libraries":[{"name":OLD}]}),
        );
        let child_a = write_manifest(
            &versions,
            "first",
            json!({"id":"first", "inheritsFrom":"26.3"}),
        );
        let child_b = write_manifest(
            &versions,
            "second",
            json!({"id":"second", "inheritsFrom":"26.3"}),
        );
        let first = [parent.clone(), child_a];
        let second = [parent.clone(), child_b];
        let dialect = Some(LinkedLauncherDialect::Hmcl);
        let (first, second) = tokio::join!(
            persist(&first, &versions, dialect),
            persist(&second, &versions, dialect)
        );
        first.unwrap();
        second.unwrap();
        let manifest: Value =
            serde_json::from_slice(&std::fs::read(&parent).unwrap()).unwrap();
        assert_eq!(manifest["libraries"][0]["name"], FIXED);
    }

    #[tokio::test]
    async fn freetype_persist_skips_repairs_split_across_inheritance_chain() {
        let dir = tempfile::tempdir().unwrap();
        let versions = dir.path().join("versions");
        let parent_manifest = json!({
            "id": "26.3", "libraries": [{"name": OLD}],
        });
        let child_manifest = json!({
            "id": "modded", "inheritsFrom": "26.3",
            "libraries": [{"name": format!("{OLD}:natives-windows")}],
        });
        let parent = write_manifest(&versions, "26.3", parent_manifest);
        let child = write_manifest(&versions, "modded", child_manifest);
        let paths = [parent, child];
        let original =
            paths.each_ref().map(|path| std::fs::read(path).unwrap());

        let repair =
            prepare(&paths, &versions, Some(LinkedLauncherDialect::Hmcl))
                .await
                .unwrap();
        assert!(repair.downloads().is_empty());
        repair.persist().await.unwrap();
        assert_eq!(
            paths.each_ref().map(|path| std::fs::read(path).unwrap()),
            original
        );
    }

    #[tokio::test]
    async fn freetype_persist_keeps_shared_chain_with_custom_native_entries() {
        let dir = tempfile::tempdir().unwrap();
        let versions = dir.path().join("versions");
        let parent = write_manifest(
            &versions,
            "26.3",
            json!({"id":"26.3", "libraries":[{"name":OLD}]}),
        );
        let parent_before = std::fs::read(&parent).unwrap();
        for library in [
            json!({"name":format!("{OLD}:natives-linux-arm64")}),
            json!({"name":OLD, "natives":{"linux":"natives-linux"}}),
            json!({"name":OLD, "MMC-hint":"local"}),
            json!({"name":format!("{OLD}:natives-windows"), "rules":[{"action":"allow", "os":{"name":"windows", "arch":"aarch64"}}]}),
        ] {
            let child = write_manifest(
                &versions,
                "custom",
                json!({"id":"custom", "inheritsFrom":"26.3", "libraries":[library]}),
            );
            let child_before = std::fs::read(&child).unwrap();
            let repair = prepare(
                &[parent.clone(), child.clone()],
                &versions,
                Some(LinkedLauncherDialect::Generic),
            )
            .await
            .unwrap();
            assert!(repair.downloads().is_empty());
            repair.persist().await.unwrap();
            assert_eq!(std::fs::read(&parent).unwrap(), parent_before);
            assert_eq!(std::fs::read(&child).unwrap(), child_before);
        }
    }
}

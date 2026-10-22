//! Parse mod metadata files from inside JAR archives to extract mod identity
//! information (mod ID, name, version, authors, etc.) when Modrinth API
//! lookups fail or have no match.
//!
//! Supported formats:
//! - Fabric: `fabric.mod.json` (JSON)
//! - Quilt: `quilt.mod.json` (JSON, same shape wrapped under `quilt_loader`)
//! - Forge: `META-INF/mods.toml` (TOML)
//! - NeoForge: `META-INF/neoforge.mods.toml` (TOML)
//! - Legacy Forge: `mcmod.info` (JSON array)

mod fabric;
pub mod icon;
pub mod manifest;
mod mcmod_info;
mod toml_mod;

use bytes::Bytes;
use serde::{Deserialize, Serialize};

const LOCAL_METADATA_VERSION: u32 = 2;

/// Unified local mod metadata extracted from inside a JAR.
///
/// Only `mod_id` is required; all other fields are best-effort.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalModMetadata {
    #[serde(default)]
    pub metadata_version: u32,
    /// Unique mod identifier (e.g. "sodium", "minecraft")
    pub mod_id: String,
    /// Human-readable display name
    pub name: Option<String>,
    /// Mod version string
    pub version: Option<String>,
    /// Author list
    #[serde(default)]
    pub authors: Vec<String>,
    /// Short description
    pub description: Option<String>,
    /// Website or project URL
    pub url: Option<String>,
    /// Path to icon inside the JAR (e.g. "icon.png" or "assets/.../icon.png")
    pub icon_path: Option<String>,
    /// Supported Minecraft version range (e.g. ">=1.20", "[1.20,1.21)", "1.12.2")
    pub minecraft_version: Option<String>,
    /// Required loader version (e.g. ">=0.15.0", "[52,)")
    pub loader_version: Option<String>,
    /// Loader type (e.g. "fabric", "forge", "neoforge", "quilt")
    pub loader: Option<String>,
    /// Required dependencies declared in the embedded metadata (Fabric
    /// `depends`, Quilt `depends`, Forge mandatory dependencies).
    ///
    /// `None` marks JSON written before dependency extraction existed and
    /// triggers a one-time re-extraction of the file.
    #[serde(default)]
    pub dependencies: Option<Vec<LocalModDependency>>,
}

impl LocalModMetadata {
    pub(crate) fn needs_refresh(&self) -> bool {
        self.metadata_version < LOCAL_METADATA_VERSION
            || self.dependencies.is_none()
    }
}

/// One required dependency declared in embedded mod metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalModDependency {
    pub mod_id: String,
    #[serde(default)]
    pub version_range: Option<String>,
}

/// Dependency ids that refer to the runtime environment instead of content
/// that can be linked against installed files.
pub(crate) fn is_env_dependency_id(id: &str) -> bool {
    matches!(
        id,
        "minecraft"
            | "java"
            | "fabricloader"
            | "quilt_loader"
            | "forge"
            | "neoforge"
            | "fml"
    )
}

/// Try to extract `LocalModMetadata` from raw JAR bytes.
///
/// Returns `None` when the JAR does not contain any known mod metadata file
/// or when none of the supported formats can be successfully parsed.
pub fn extract_mod_metadata(bytes: &Bytes) -> Option<LocalModMetadata> {
    let cursor = std::io::Cursor::new(&**bytes);
    let mut archive = zip::ZipArchive::new(cursor).ok()?;

    // Try each known metadata path in priority order.
    if let Some(meta) = try_fabric(&mut archive) {
        return Some(meta);
    }
    if let Some(meta) = try_quilt(&mut archive) {
        return Some(meta);
    }
    if let Some(meta) =
        try_toml_path(&mut archive, "META-INF/neoforge.mods.toml")
    {
        return Some(meta);
    }
    if let Some(meta) = try_toml_path(&mut archive, "META-INF/mods.toml") {
        return Some(meta);
    }
    if let Some(meta) = try_mcmod_info(&mut archive) {
        return Some(meta);
    }

    None
}

// ── format-specific parsers ────────────────────────────────────────────────

fn try_fabric(
    archive: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>,
) -> Option<LocalModMetadata> {
    let mut file = archive.by_name("fabric.mod.json").ok()?;
    let parsed: fabric::FabricModJson =
        serde_json::from_reader(&mut file).ok()?;

    let authors = merge_authors(&parsed.authors, &parsed.contributors);
    // A mod without an id cannot be identified; skip it rather than
    // fabricating a shared placeholder that would make mods collide.
    let mod_id = parsed.id.clone()?;

    Some(LocalModMetadata {
        metadata_version: LOCAL_METADATA_VERSION,
        mod_id,
        name: parsed.name,
        version: parsed.version,
        authors,
        description: parsed.description,
        url: extract_contact_url(&parsed._contact),
        icon_path: parsed.icon.as_ref().and_then(|icon| icon.resolve()),
        minecraft_version: fabric::fabric_dep_value(
            &parsed.depends,
            "minecraft",
        ),
        loader_version: fabric::fabric_dep_value(
            &parsed.depends,
            "fabricloader",
        ),
        loader: Some("fabric".into()),
        dependencies: Some(fabric::fabric_dependencies(&parsed.depends)),
    })
}

fn try_quilt(
    archive: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>,
) -> Option<LocalModMetadata> {
    let mut file = archive.by_name("quilt.mod.json").ok()?;
    let parsed: fabric::QuiltModJson =
        serde_json::from_reader(&mut file).ok()?;

    let inner = parsed.quilt_loader;
    let authors = merge_authors(&inner.authors, &inner.contributors);
    let mod_id = inner.id.clone()?;

    Some(LocalModMetadata {
        metadata_version: LOCAL_METADATA_VERSION,
        mod_id,
        name: inner.name,
        version: inner.version,
        authors,
        description: inner.description,
        url: extract_contact_url(&inner._contact),
        icon_path: inner.icon.as_ref().and_then(|icon| icon.resolve()),
        minecraft_version: fabric::quilt_dep_value(&inner.depends, "minecraft"),
        loader_version: fabric::quilt_dep_value(&inner.depends, "quilt_loader"),
        loader: Some("quilt".into()),
        dependencies: Some(fabric::quilt_dependencies(&inner.depends)),
    })
}

fn try_toml_path(
    archive: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>,
    path: &str,
) -> Option<LocalModMetadata> {
    let mut content = String::new();
    {
        let mut file = archive.by_name(path).ok()?;
        std::io::Read::read_to_string(&mut file, &mut content).ok()?;
    }
    let parsed: toml_mod::ModsToml = toml::from_str(&content).ok()?;

    // A mod jar may declare several [[mods]] entries (bundled mods); report
    // the first entry that carries an ID. An id-less entry (e.g. the
    // "minecraft" marker used by some packs) must not discard the metadata
    // of the real mod that follows.
    let entry = parsed
        .mods?
        .into_iter()
        .find(|entry| entry.mod_id.is_some())?;
    let mod_id = entry.mod_id.clone()?;

    let authors: Vec<String> = entry
        .authors
        .as_deref()
        .map(|s| {
            s.split(',')
                .map(|a| a.trim().to_string())
                .filter(|a| !a.is_empty())
                .collect()
        })
        .unwrap_or_default();

    // Determine loader type from the file path.
    let is_neoforge = path.contains("neoforge");
    let loader = if is_neoforge {
        Some("neoforge".into())
    } else {
        Some("forge".into())
    };
    // The root `loaderVersion` IS the Forge/NeoForge loader version.
    let loader_version = parsed.loader_version.clone();

    // Look up dependencies for this mod's modId.
    let minecraft_version = parsed
        .dependencies
        .as_ref()
        .and_then(|deps| deps.get(&mod_id))
        .and_then(|entries| {
            entries
                .iter()
                .find(|dep| {
                    dep.mod_id.as_deref() == Some("minecraft") && dep.required()
                })
                .map(|dep| {
                    dep.version_range
                        .clone()
                        .filter(|range| !range.trim().is_empty())
                        .unwrap_or_else(|| "*".into())
                })
        });
    let dependencies = parsed
        .dependencies
        .as_ref()
        .and_then(|deps| deps.get(&mod_id))
        .map(|entries| {
            entries
                .iter()
                .filter(|dep| dep.required())
                .filter_map(|dep| {
                    let dep_id = dep.mod_id.clone()?;
                    if is_env_dependency_id(&dep_id) {
                        return None;
                    }
                    Some(LocalModDependency {
                        mod_id: dep_id,
                        version_range: dep.version_range.clone(),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    // Forge `mods.toml` commonly stores the version as a Gradle placeholder
    // (e.g. `${file.jarVersion}`) that the loader resolves at runtime from the
    // JAR manifest's `Implementation-Version`. Resolve it here so the
    // placeholder never surfaces as a version in the UI.
    let version = resolve_toml_version(entry.version.clone(), archive);

    Some(LocalModMetadata {
        metadata_version: LOCAL_METADATA_VERSION,
        mod_id,
        name: entry.display_name,
        version,
        authors,
        description: entry.description,
        url: entry.display_url,
        icon_path: entry.logo_file,
        minecraft_version,
        loader_version,
        loader,
        dependencies: Some(dependencies),
    })
}

fn try_mcmod_info(
    archive: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>,
) -> Option<LocalModMetadata> {
    let mut file = archive.by_name("mcmod.info").ok()?;
    let entries: Vec<mcmod_info::McmodInfoEntry> =
        serde_json::from_reader(&mut file).ok()?;

    let entry = entries.into_iter().next()?;
    let mod_id = entry.modid.clone()?;

    Some(LocalModMetadata {
        metadata_version: LOCAL_METADATA_VERSION,
        mod_id,
        name: entry.name,
        version: entry.version,
        authors: entry.authors.unwrap_or_default(),
        description: entry.description,
        url: entry.url,
        icon_path: entry.logo_file,
        minecraft_version: entry.mcversion,
        loader_version: None,
        loader: Some("forge".into()),
        dependencies: Some(Vec::new()),
    })
}

// ── helpers ────────────────────────────────────────────────────────────────

fn merge_authors(
    primary: &[fabric::FabricAuthorOrArray],
    contributors: &[fabric::FabricAuthorOrArray],
) -> Vec<String> {
    primary
        .iter()
        .chain(contributors.iter())
        .filter_map(|author| match author {
            fabric::FabricAuthorOrArray::Plain(s) => Some(s.clone()),
            fabric::FabricAuthorOrArray::Object { name } => name.clone(),
        })
        .collect()
}

/// Extract a URL from Fabric's `contact` object (often has `"homepage"`, `"sources"`, etc.).
fn extract_contact_url(contact: &Option<serde_json::Value>) -> Option<String> {
    let obj = contact.as_ref()?.as_object()?;
    // Prefer homepage, then sources, then any string value.
    if let Some(homepage) = obj.get("homepage").and_then(|v| v.as_str()) {
        return Some(homepage.to_string());
    }
    if let Some(sources) = obj.get("sources").and_then(|v| v.as_str()) {
        return Some(sources.to_string());
    }
    // Fallback: return the first string field found.
    obj.values().find_map(|v| v.as_str().map(String::from))
}

/// Resolve a Forge `mods.toml` version string.
///
/// Gradle builds often write an unresolved placeholder (e.g.
/// `${file.jarVersion}`) which the loader substitutes from the JAR manifest at
/// runtime; surface the real `Implementation-Version` from the manifest when
/// present, falling back to the original value otherwise.
fn resolve_toml_version(
    version: Option<String>,
    archive: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>,
) -> Option<String> {
    let placeholder = version?;
    if !placeholder.starts_with("${") {
        return Some(placeholder);
    }

    Some(
        manifest::archive_manifest(archive)
            .and_then(|manifest| manifest.implementation_version)
            .filter(|resolved| !resolved.trim().is_empty())
            .unwrap_or(placeholder),
    )
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    fn build_jar(entries: &[(&str, &str)]) -> bytes::Bytes {
        let mut buffer = std::io::Cursor::new(Vec::new());
        {
            let mut archive = zip::ZipWriter::new(&mut buffer);
            let options = zip::write::FileOptions::<()>::default();
            for (name, content) in entries {
                archive.start_file(*name, options).expect("start zip entry");
                archive
                    .write_all(content.as_bytes())
                    .expect("write zip entry");
            }
            archive.finish().expect("finish zip");
        }
        bytes::Bytes::from(buffer.into_inner())
    }

    #[test]
    fn required_minecraft_without_a_range_is_unrestricted_but_missing_dependency_is_unknown()
     {
        for (dependency, expected) in [
            ("", None),
            (
                "[[dependencies.example]]\nmodId = 'minecraft'\ntype = 'required'",
                Some("*"),
            ),
            (
                "[[dependencies.example]]\nmodId = 'minecraft'\ntype = 'optional'",
                None,
            ),
            (
                "[[dependencies.example]]\nmodId = 'minecraft'\nmandatory = true\nversionRange = '[1,)'",
                Some("[1,)"),
            ),
        ] {
            let content = format!(
                "modLoader = 'javafml'\n[[mods]]\nmodId = 'example'\nversion = '1'\n{dependency}"
            );
            let jar = build_jar(&[("META-INF/neoforge.mods.toml", &content)]);
            let metadata = super::extract_mod_metadata(&jar).unwrap();
            assert_eq!(metadata.minecraft_version.as_deref(), expected);
            let mut old = metadata.clone();
            old.metadata_version = 1;
            assert!(old.needs_refresh());
            assert!(!metadata.needs_refresh());
        }
    }

    #[test]
    fn neoforge_exact_minecraft_dependency_is_an_explicit_compatibility_declaration()
     {
        let jar = build_jar(&[(
            "META-INF/neoforge.mods.toml",
            r#"
modLoader = "javafml"
loaderVersion = "[0,)"
[[mods]]
modId = "create"
version = "6.0.10"
[[dependencies."create"]]
modId = "minecraft"
type = "required"
versionRange = "[1.21.1]"
[[dependencies."create"]]
modId = "flywheel"
type = "required"
versionRange = "[1.0.0,2.0)"
[[dependencies."create"]]
modId = "sodium"
type = "optional"
[[dependencies."create"]]
modId = "radium"
type = "incompatible"
"#,
        )]);
        let metadata = super::extract_mod_metadata(&jar).unwrap();
        assert!(!metadata.needs_refresh());
        assert_eq!(
            metadata
                .dependencies
                .as_ref()
                .unwrap()
                .iter()
                .map(|dependency| dependency.mod_id.as_str())
                .collect::<Vec<_>>(),
            ["flywheel"]
        );
        let mut legacy = serde_json::to_value(&metadata).unwrap();
        legacy.as_object_mut().unwrap().remove("metadata_version");
        assert!(
            serde_json::from_value::<super::LocalModMetadata>(legacy)
                .unwrap()
                .needs_refresh()
        );
        assert_eq!(metadata.minecraft_version.as_deref(), Some("[1.21.1]"));
        let declared = crate::resources::ReleaseCompatibility {
            game_versions: metadata.minecraft_version.into_iter().collect(),
            loaders: metadata.loader.into_iter().collect(),
        };
        let assessment = crate::resources::assess_declared_compatibility(
            &declared,
            &crate::resources::ipc::ResourceTarget {
                game_versions: vec!["1.21.1".into()],
                loaders: vec!["neoforge".into()],
                channels: vec![],
            },
            crate::resources::ResourceType::Mod,
        );
        assert_eq!(
            assessment.game_version,
            crate::resources::CompatibilityState::Compatible
        );
        assert_eq!(
            assessment.loader,
            crate::resources::CompatibilityState::Compatible
        );
    }

    #[test]
    fn forge_placeholder_version_resolves_from_manifest() {
        let jar = build_jar(&[
            (
                "META-INF/MANIFEST.MF",
                "Manifest-Version: 1.0\nImplementation-Title: Example\nImplementation-Version: 1.2.3\n",
            ),
            (
                "META-INF/mods.toml",
                "modLoader = \"javafml\"\nloaderVersion = \"[4,)\"\n\n[[mods]]\nmodId = \"example\"\ndisplayName = \"Example\"\nversion = \"${file.jarVersion}\"\n",
            ),
        ]);

        let meta = super::extract_mod_metadata(&jar).expect("mod metadata");
        assert_eq!(meta.mod_id, "example");
        assert_eq!(meta.version.as_deref(), Some("1.2.3"));
    }

    #[test]
    fn resolved_mods_toml_version_is_kept_as_is() {
        let jar = build_jar(&[
            (
                "META-INF/MANIFEST.MF",
                "Manifest-Version: 1.0\nImplementation-Version: 9.9.9\n",
            ),
            (
                "META-INF/mods.toml",
                "[[mods]]\nmodId = \"example\"\nversion = \"1.0.0\"\n",
            ),
        ]);

        let meta = super::extract_mod_metadata(&jar).expect("mod metadata");
        assert_eq!(meta.version.as_deref(), Some("1.0.0"));
    }

    #[test]
    fn placeholder_version_without_manifest_attribute_is_kept() {
        let jar = build_jar(&[(
            "META-INF/mods.toml",
            "[[mods]]\nmodId = \"example\"\nversion = \"${file.jarVersion}\"\n",
        )]);

        let meta = super::extract_mod_metadata(&jar).expect("mod metadata");
        assert_eq!(meta.version.as_deref(), Some("${file.jarVersion}"));
    }
}

use std::io::{Cursor, Read};

use cafebabe::attributes::{Annotation, AnnotationElementValue, AttributeData};

use super::mcmod_info::{McmodInfo, McmodInfoEntry};
use super::{
    LOCAL_METADATA_VERSION, LocalModDependency, LocalModMetadata,
    LocalProvidedMod, is_env_dependency_id,
};

const FORGE_MOD: &str = "Lnet/minecraftforge/fml/common/Mod;";
const FML_MOD: &str = "Lcpw/mods/fml/common/Mod;";
const MAX_CLASS_SIZE: u64 = 16 * 1024 * 1024;

#[derive(Default)]
struct ModAnnotation {
    mod_id: String,
    name: Option<String>,
    version: Option<String>,
    minecraft_version: Option<String>,
    dependencies: String,
    use_metadata: Option<bool>,
}

/// Extract legacy Forge and Cleanroom metadata without loading Java classes.
pub(super) fn extract(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
) -> Option<LocalModMetadata> {
    let mut entries = archive
        .by_name("mcmod.info")
        .ok()
        .and_then(|file| serde_json::from_reader::<_, McmodInfo>(file).ok())
        .map(McmodInfo::entries)
        .unwrap_or_default();
    entries.retain(|entry| {
        entry
            .modid
            .as_deref()
            .is_some_and(|id| !id.trim().is_empty())
    });
    let annotations = read_annotations(archive);
    if !annotations.is_empty() {
        // Forge discovers mods through @Mod, then looks up their metadata by
        // ID. Old entries in mcmod.info do not provide additional loaded mods.
        entries.retain(|entry| {
            annotations.iter().any(|annotation| {
                entry.modid.as_deref() == Some(annotation.mod_id.as_str())
            })
        });
    }
    for annotation in &annotations {
        if !entries.iter().any(|entry| {
            entry.modid.as_deref() == Some(annotation.mod_id.as_str())
        }) {
            entries.push(McmodInfoEntry {
                modid: Some(annotation.mod_id.clone()),
                ..Default::default()
            });
        }
    }
    let primary = entries.first()?;
    let mod_id = primary.modid.clone()?;
    let primary_annotation =
        annotations.iter().find(|item| item.mod_id == mod_id);
    let mut additional_mods = Vec::new();
    let mut dependencies = Vec::new();
    for entry in &entries {
        let Some(id) = entry.modid.as_deref() else {
            continue;
        };
        let annotation = annotations.iter().find(|item| item.mod_id == id);
        if id != mod_id
            && !additional_mods
                .iter()
                .any(|other: &LocalProvidedMod| other.mod_id == id)
        {
            additional_mods.push(LocalProvidedMod {
                mod_id: id.to_string(),
                version: mod_version(entry, annotation),
            });
        }
        if entry.use_dependency_information
            && annotation.and_then(|item| item.use_metadata) != Some(false)
        {
            for required in &entry.required_mods {
                push_dependency(required, &mut dependencies);
            }
        } else if let Some(annotation) = annotation {
            annotation_dependencies(
                &annotation.dependencies,
                &mut dependencies,
            );
        }
    }
    dependencies.retain(|dependency| {
        dependency.mod_id != mod_id
            && !additional_mods
                .iter()
                .any(|item| item.mod_id == dependency.mod_id)
    });

    Some(LocalModMetadata {
        metadata_version: LOCAL_METADATA_VERSION,
        mod_id,
        additional_mods,
        name: primary
            .name
            .clone()
            .or_else(|| primary_annotation.and_then(|item| item.name.clone())),
        version: mod_version(primary, primary_annotation),
        authors: primary.authors.clone().unwrap_or_default(),
        description: primary.description.clone(),
        url: primary.url.clone(),
        icon_path: primary.logo_file.clone(),
        minecraft_version: primary_annotation
            .and_then(|item| item.minecraft_version.clone())
            .or_else(|| primary.mcversion.clone()),
        loader_version: None,
        loader: Some("forge".into()),
        dependencies: Some(dependencies),
    })
}

fn mod_version(
    entry: &McmodInfoEntry,
    annotation: Option<&ModAnnotation>,
) -> Option<String> {
    annotation
        .and_then(|item| item.version.clone())
        .or_else(|| entry.version.clone())
}

fn read_annotations(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
) -> Vec<ModAnnotation> {
    let mut annotations = Vec::new();
    let mut options = cafebabe::ParseOptions::default();
    options.parse_bytecode(false);
    for index in 0..archive.len() {
        let Ok(file) = archive.by_index(index) else {
            continue;
        };
        if !file.name().is_ok_and(|name| name.ends_with(".class"))
            || file.size() > MAX_CLASS_SIZE
        {
            continue;
        }
        let mut data = Vec::new();
        if file
            .take(MAX_CLASS_SIZE + 1)
            .read_to_end(&mut data)
            .is_err()
            || data.len() as u64 > MAX_CLASS_SIZE
        {
            continue;
        }
        if ![FORGE_MOD, FML_MOD].iter().any(|descriptor| {
            data.windows(descriptor.len())
                .any(|part| part == descriptor.as_bytes())
        }) {
            continue;
        }
        let Ok(class) = cafebabe::parse_class_with_options(&data, &options)
        else {
            continue;
        };
        for attribute in class.attributes {
            let AttributeData::RuntimeVisibleAnnotations(items) =
                attribute.data
            else {
                continue;
            };
            for item in items {
                if let Some(annotation) = read_annotation(item) {
                    annotations.push(annotation);
                }
            }
        }
    }
    annotations
}

fn read_annotation(annotation: Annotation<'_>) -> Option<ModAnnotation> {
    if ![FORGE_MOD, FML_MOD]
        .contains(&annotation.type_descriptor.to_string().as_str())
    {
        return None;
    }
    let mut result = ModAnnotation::default();
    for element in annotation.elements {
        match (element.name.as_ref(), element.value) {
            ("modid", AnnotationElementValue::StringConstant(value)) => {
                result.mod_id = value.into_owned();
            }
            ("name", AnnotationElementValue::StringConstant(value))
                if !value.is_empty() =>
            {
                result.name = Some(value.into_owned());
            }
            ("version", AnnotationElementValue::StringConstant(value))
                if !value.is_empty() =>
            {
                result.version = Some(value.into_owned());
            }
            (
                "acceptedMinecraftVersions",
                AnnotationElementValue::StringConstant(value),
            ) if !value.is_empty() => {
                result.minecraft_version = Some(value.into_owned());
            }
            ("dependencies", AnnotationElementValue::StringConstant(value)) => {
                result.dependencies = value.into_owned();
            }
            ("useMetadata", AnnotationElementValue::BooleanConstant(value)) => {
                result.use_metadata = Some(value != 0);
            }
            _ => {}
        }
    }
    (!result.mod_id.trim().is_empty()).then_some(result)
}

/// Only required client dependencies affect the launcher's installed mod list.
fn annotation_dependencies(
    value: &str,
    dependencies: &mut Vec<LocalModDependency>,
) {
    for clause in value.split(';') {
        let Some((prefix, reference)) = clause.trim().split_once(':') else {
            continue;
        };
        let parts = prefix
            .split('-')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>();
        if reference.contains(':')
            || !parts.contains(&"required")
            || parts.contains(&"server")
            || parts.iter().filter(|part| **part == "required").count() != 1
            || parts
                .iter()
                .filter(|part| matches!(**part, "before" | "after"))
                .count()
                > 1
            || parts
                .iter()
                .filter(|part| matches!(**part, "client" | "server"))
                .count()
                > 1
            || parts.iter().any(|part| {
                !matches!(
                    *part,
                    "required" | "before" | "after" | "client" | "server"
                )
            })
        {
            continue;
        }
        push_dependency(reference, dependencies);
    }
}

fn push_dependency(
    reference: &str,
    dependencies: &mut Vec<LocalModDependency>,
) {
    let (id, range) = reference
        .trim()
        .split_once('@')
        .map_or((reference.trim(), None), |(id, range)| {
            (id.trim(), Some(range.trim()))
        });
    if id.is_empty() || id == "*" || is_env_dependency_id(id) {
        return;
    }
    let version_range =
        range.filter(|range| !range.is_empty()).map(str::to_string);
    if dependencies.iter().any(|dependency| {
        dependency.mod_id == id && dependency.version_range == version_range
    }) {
        return;
    }
    dependencies.push(LocalModDependency {
        mod_id: id.to_string(),
        version_range,
    });
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    fn jar(entries: &[(&str, &[u8])]) -> bytes::Bytes {
        let mut buffer = Cursor::new(Vec::new());
        {
            let mut archive = zip::ZipWriter::new(&mut buffer);
            for (name, content) in entries {
                archive
                    .start_file(*name, zip::write::FileOptions::<()>::default())
                    .unwrap();
                archive.write_all(content).unwrap();
            }
            archive.finish().unwrap();
        }
        bytes::Bytes::from(buffer.into_inner())
    }

    fn metadata(entries: &[(&str, &[u8])]) -> LocalModMetadata {
        super::super::extract_mod_metadata(&jar(entries)).expect("mod metadata")
    }

    /// A Java 8 class with a real RuntimeVisibleAnnotations attribute.
    fn mod_class(
        id: &str,
        dependencies: &str,
        use_metadata: Option<bool>,
    ) -> Vec<u8> {
        mod_class_with_descriptor(id, dependencies, use_metadata, FORGE_MOD)
    }

    fn mod_class_with_descriptor(
        id: &str,
        dependencies: &str,
        use_metadata: Option<bool>,
        descriptor: &str,
    ) -> Vec<u8> {
        fn utf8(pool: &mut Vec<Vec<u8>>, value: &str) -> u16 {
            let mut entry = vec![1];
            entry.extend_from_slice(&(value.len() as u16).to_be_bytes());
            entry.extend_from_slice(value.as_bytes());
            pool.push(entry);
            pool.len() as u16
        }
        let mut pool = Vec::new();
        utf8(&mut pool, "example/Example");
        pool.push(vec![7, 0, 1]);
        utf8(&mut pool, "java/lang/Object");
        pool.push(vec![7, 0, 3]);
        let attribute_name = utf8(&mut pool, "RuntimeVisibleAnnotations");
        let annotation_type = utf8(&mut pool, descriptor);
        let mut elements = Vec::new();
        for (name, value) in [
            ("modid", id),
            ("name", "Annotated Example"),
            ("version", "2.0"),
            ("acceptedMinecraftVersions", "[1.12.2]"),
            ("dependencies", dependencies),
        ] {
            let name = utf8(&mut pool, name);
            let value = utf8(&mut pool, value);
            elements.push((name, b's', value));
        }
        if let Some(value) = use_metadata {
            let name = utf8(&mut pool, "useMetadata");
            let mut entry = vec![3];
            entry.extend_from_slice(&i32::from(value).to_be_bytes());
            pool.push(entry);
            elements.push((name, b'Z', pool.len() as u16));
        }
        let mut attribute = Vec::new();
        for value in [1, annotation_type, elements.len() as u16] {
            attribute.extend_from_slice(&value.to_be_bytes());
        }
        for (name, tag, value) in elements {
            attribute.extend_from_slice(&name.to_be_bytes());
            attribute.push(tag);
            attribute.extend_from_slice(&value.to_be_bytes());
        }
        let mut class = vec![0xca, 0xfe, 0xba, 0xbe, 0, 0, 0, 52];
        class.extend_from_slice(&((pool.len() + 1) as u16).to_be_bytes());
        for entry in pool {
            class.extend(entry);
        }
        for value in [0x21u16, 2, 4, 0, 0, 0, 1, attribute_name] {
            class.extend_from_slice(&value.to_be_bytes());
        }
        class.extend_from_slice(&(attribute.len() as u32).to_be_bytes());
        class.extend(attribute);
        class
    }

    fn dependency_ids(metadata: &LocalModMetadata) -> Vec<&str> {
        metadata
            .dependencies
            .as_ref()
            .unwrap()
            .iter()
            .map(|dependency| dependency.mod_id.as_str())
            .collect()
    }

    #[test]
    fn mcmod_array_extracts_required_mods_but_not_optional_ordering() {
        let info = br#"[{"modid":"example","authorList":["Author"],"useDependencyInformation":true,"requiredMods":["library@[1.0,)","forge@[14,)","cleanroom","FML"],"dependencies":["optional"]}]"#;
        let result = metadata(&[("mcmod.info", info)]);
        assert_eq!(dependency_ids(&result), ["library"]);
        assert_eq!(
            result.dependencies.unwrap()[0].version_range.as_deref(),
            Some("[1.0,)")
        );
        assert_eq!(result.authors, ["Author"]);
    }

    #[test]
    fn mcmod_object_skips_entries_without_an_id() {
        let info = br#"{"modListVersion":"2","modList":[{}, {"modid":"example","useDependencyInformation":true,"requiredMods":["library"]}]}"#;
        let result = metadata(&[("mcmod.info", info)]);
        assert_eq!(result.mod_id, "example");
        assert_eq!(dependency_ids(&result), ["library"]);
    }

    #[test]
    fn annotation_only_mods_extract_identity_and_required_client_dependencies()
    {
        let class = mod_class(
            "example",
            "required-after:library@[1.0,);after:optional;required-before:before;required-client:clientlib;required-server:serverlib;required:other;required-after:FML;required-after:forge;required-after:cleanroom;required-before:*;required-after:library",
            None,
        );
        let result = metadata(&[("example/Example.class", &class)]);
        assert_eq!(result.mod_id, "example");
        assert_eq!(result.name.as_deref(), Some("Annotated Example"));
        assert_eq!(result.version.as_deref(), Some("2.0"));
        assert_eq!(result.minecraft_version.as_deref(), Some("[1.12.2]"));
        assert_eq!(
            dependency_ids(&result),
            ["library", "before", "clientlib", "other", "library"]
        );
        assert_eq!(
            result.dependencies.unwrap()[0].version_range.as_deref(),
            Some("[1.0,)")
        );
    }

    #[test]
    fn metadata_dependency_flag_controls_annotation_precedence() {
        let info = br#"[{"modid":"example","useDependencyInformation":true,"requiredMods":["metadata_library"]}]"#;
        for use_metadata in [None, Some(true)] {
            let class = mod_class(
                "example",
                "required-after:annotation_library",
                use_metadata,
            );
            let result =
                metadata(&[("mcmod.info", info), ("Example.class", &class)]);
            assert_eq!(dependency_ids(&result), ["metadata_library"]);
        }
        let class = mod_class(
            "example",
            "required-after:annotation_library",
            Some(false),
        );
        let result =
            metadata(&[("mcmod.info", info), ("Example.class", &class)]);
        assert_eq!(dependency_ids(&result), ["annotation_library"]);
    }

    #[test]
    fn earlier_fml_annotations_use_the_same_dependency_format() {
        let class = mod_class_with_descriptor(
            "example",
            "required-after:library",
            None,
            FML_MOD,
        );
        let result = metadata(&[("Example.class", &class)]);
        assert_eq!(result.mod_id, "example");
        assert_eq!(dependency_ids(&result), ["library"]);
    }

    #[test]
    fn required_clauses_keep_distinct_ranges_and_ignore_invalid_instructions() {
        let class = mod_class(
            "example",
            "required - after - client:library@[1,);required-after:library@[1,);required:library@[2,);required-required:invalid_required;required-before-after:invalid_order;required-client-client:invalid_side;required-unknown:invalid_instruction;required:invalid:reference",
            None,
        );
        let result = metadata(&[("Example.class", &class)]);
        let dependencies = result.dependencies.unwrap();
        assert_eq!(dependencies.len(), 2);
        assert_eq!(dependencies[0].mod_id, "library");
        assert_eq!(dependencies[0].version_range.as_deref(), Some("[1,)"));
        assert_eq!(dependencies[1].mod_id, "library");
        assert_eq!(dependencies[1].version_range.as_deref(), Some("[2,)"));
    }

    #[test]
    fn metadata_without_dependency_flag_uses_annotations() {
        let info = br#"[{"modid":"example","requiredMods":["ignored"]}]"#;
        let class = mod_class("example", "required-after:library", Some(true));
        let result =
            metadata(&[("mcmod.info", info), ("Example.class", &class)]);
        assert_eq!(dependency_ids(&result), ["library"]);
    }

    #[test]
    fn bundled_mods_provide_all_ids_and_merge_external_dependencies() {
        let info = br#"[{"modid":"example"},{"modid":"bundled"}]"#;
        let main = mod_class(
            "example",
            "required-after:bundled;required-after:library",
            None,
        );
        let bundled =
            mod_class("bundled", "required-after:second_library", None);
        let result = metadata(&[
            ("mcmod.info", info),
            ("Main.class", &main),
            ("Bundled.class", &bundled),
        ]);
        assert_eq!(
            result.mod_ids().collect::<Vec<_>>(),
            ["example", "bundled"]
        );
        assert_eq!(result.version_for_mod("bundled"), Some("2.0"));
        assert_eq!(dependency_ids(&result), ["library", "second_library"]);
    }

    #[test]
    fn mcmod_only_bundled_ids_keep_their_metadata_versions() {
        let info = br#"[{"modid":"example","version":"1.0"},{"modid":"bundled","version":"3.1"}]"#;
        let result = metadata(&[("mcmod.info", info)]);
        assert_eq!(
            result.mod_ids().collect::<Vec<_>>(),
            ["example", "bundled"]
        );
        assert_eq!(result.version_for_mod("example"), Some("1.0"));
        assert_eq!(result.version_for_mod("bundled"), Some("3.1"));
    }

    #[test]
    fn stale_metadata_entries_do_not_provide_mods_or_hide_external_dependencies()
     {
        let info = br#"[{"modid":"stale"},{"modid":"example"}]"#;
        let class = mod_class("example", "required-after:stale", None);
        let result =
            metadata(&[("mcmod.info", info), ("Example.class", &class)]);
        assert_eq!(result.mod_id, "example");
        assert!(result.additional_mods.is_empty());
        assert_eq!(dependency_ids(&result), ["stale"]);
    }

    #[test]
    fn malformed_classes_do_not_discard_valid_mcmod_info() {
        let result = metadata(&[
            ("mcmod.info", br#"[{"modid":"example"}]"#),
            ("Bad.class", FORGE_MOD.as_bytes()),
        ]);
        assert_eq!(result.mod_id, "example");
        assert!(dependency_ids(&result).is_empty());
    }

    #[test]
    fn malformed_mcmod_info_falls_back_to_annotations() {
        let class = mod_class("example", "required-after:library", None);
        let result =
            metadata(&[("mcmod.info", b"invalid"), ("Example.class", &class)]);
        assert_eq!(dependency_ids(&result), ["library"]);
    }

    #[test]
    fn empty_dependency_caches_are_refreshed_once() {
        let mut result =
            metadata(&[("mcmod.info", br#"[{"modid":"example"}]"#)]);
        assert!(!result.needs_refresh());
        let mut version_two = result.clone();
        version_two.metadata_version = 2;
        assert!(version_two.needs_refresh());
        let mut old_json = serde_json::to_value(&result).unwrap();
        old_json.as_object_mut().unwrap().remove("metadata_version");
        old_json.as_object_mut().unwrap().remove("additional_mods");
        let old: LocalModMetadata = serde_json::from_value(old_json).unwrap();
        assert!(old.needs_refresh());
        assert!(old.additional_mods.is_empty());
        result.dependencies = None;
        assert!(result.needs_refresh());
    }
}

use daedalus::minecraft::{
    Library, LibraryDownload, LibraryDownloads, Os, OsRule, Rule, RuleAction,
};

use super::local_version::LinkedLibrary;

const LEGACY_COORDINATE: &str = "org.lwjgl:lwjgl-freetype:3.4.3";
const FIXED_COORDINATE: &str = "org.lwjgl:lwjgl-freetype:3.4.3-mojang-2.15.0";

pub(super) struct Artifact {
    classifier: &'static str,
    sha1: &'static str,
    size: u32,
    target: Option<(Os, &'static str)>,
}

// Mojang's replacements, as used in HMCL commit 29a05601b33b71ad8bb017484ae5bcab76c4ee83.
const ARTIFACTS: &[Artifact] = &[
    Artifact {
        classifier: "",
        sha1: "134bc57b83e78a7f954a0f078093e319739d42ea",
        size: 469281,
        target: None,
    },
    Artifact {
        classifier: "natives-linux",
        sha1: "f20ae220999240d484896d839694b02c2a655608",
        size: 1423204,
        target: Some((Os::Linux, "x86_64")),
    },
    Artifact {
        classifier: "natives-windows",
        sha1: "f85d0f5acaa594f9e93d657f8feff85d6a427b6a",
        size: 1190826,
        target: Some((Os::Windows, "x86_64")),
    },
    Artifact {
        classifier: "natives-windows-arm64",
        sha1: "5f475c76490bb368f235017985d8d9dc6b5b5121",
        size: 1043884,
        target: Some((Os::Windows, "aarch64")),
    },
    Artifact {
        classifier: "natives-macos",
        sha1: "6a46815640d9ce43533200986a330642a96e9590",
        size: 1225020,
        target: Some((Os::Osx, "x86_64")),
    },
    Artifact {
        classifier: "natives-macos-arm64",
        sha1: "95ef1d3420179187f0b5e1ba947329b539e73412",
        size: 1125234,
        target: Some((Os::Osx, "aarch64")),
    },
];

impl Artifact {
    pub(super) fn coordinate(&self) -> String {
        if self.classifier.is_empty() {
            FIXED_COORDINATE.to_owned()
        } else {
            format!("{FIXED_COORDINATE}:{}", self.classifier)
        }
    }

    pub(super) fn download(&self) -> LibraryDownload {
        let path = daedalus::get_path_from_artifact(&self.coordinate())
            .expect("built-in FreeType artifact has a valid Maven coordinate");
        LibraryDownload {
            url: format!("https://libraries.minecraft.net/{path}"),
            path: Some(path),
            sha1: self.sha1.to_owned(),
            size: self.size,
        }
    }

    pub(super) fn target(&self) -> Option<OsRule> {
        self.target.as_ref().map(|(os, arch)| OsRule {
            name: Some(os.clone()),
            arch: Some((*arch).to_owned()),
            version: None,
        })
    }
}

pub(super) fn patch_libraries(
    game_version: &str,
    libraries: &mut [Library],
    java_arch: &str,
) -> bool {
    if !should_patch(game_version, java_arch, libraries.iter()) {
        return false;
    }
    patch_entries(libraries.iter_mut(), |library| library, |_| {});
    true
}

pub(super) fn patch_linked_libraries(
    game_version: &str,
    libraries: &mut [LinkedLibrary],
    java_arch: &str,
) -> bool {
    if !should_patch(
        game_version,
        java_arch,
        libraries.iter().map(|library| &library.library),
    ) {
        return false;
    }
    patch_entries(
        libraries.iter_mut(),
        |library| &mut library.library,
        |library| {
            library.hint = None;
            library.filename = None;
        },
    );
    true
}

pub(super) fn is_patched_library(library: &Library) -> bool {
    artifact_for_coordinate(&library.name, FIXED_COORDINATE).is_some()
}

pub(super) fn matches_fixed_native_target(
    library: &Library,
    java_arch: &str,
) -> bool {
    if !supported_java_target(std::env::consts::OS, java_arch) {
        return true;
    }
    artifact_for_coordinate(&library.name, FIXED_COORDINATE)
        .and_then(|artifact| artifact.target.as_ref())
        .is_none_or(|(os, arch)| {
            os == &Os::native()
                && match java_arch.to_ascii_lowercase().as_str() {
                    "amd64" | "x86_64" => *arch == "x86_64",
                    "arm64" | "aarch64" => *arch == "aarch64",
                    _ => false,
                }
        })
}

fn should_patch<'a>(
    game_version: &str,
    java_arch: &str,
    mut libraries: impl Iterator<Item = &'a Library>,
) -> bool {
    affected_game_version(game_version)
        && supported_java_target(std::env::consts::OS, java_arch)
        && libraries.any(|library| {
            artifact_for_coordinate(&library.name, LEGACY_COORDINATE).is_some()
        })
}

pub(super) fn affected_game_version(game_version: &str) -> bool {
    game_version == "26.3"
        || game_version
            .strip_prefix("26.4-snapshot-")
            .is_some_and(|snapshot| {
                matches!(
                    snapshot,
                    "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "10"
                )
            })
}

fn supported_java_target(os: &str, java_arch: &str) -> bool {
    matches!(
        (os, java_arch.to_ascii_lowercase().as_str()),
        ("linux" | "windows" | "macos", "amd64" | "x86_64")
            | ("windows" | "macos", "aarch64" | "arm64")
    )
}

fn artifact_for_coordinate(
    name: &str,
    unclassified: &str,
) -> Option<&'static Artifact> {
    let suffix = name.strip_prefix(unclassified)?;
    let classifier = if suffix.is_empty() {
        ""
    } else {
        suffix
            .strip_prefix(':')
            .filter(|suffix| !suffix.is_empty())?
    };
    ARTIFACTS
        .iter()
        .find(|artifact| artifact.classifier == classifier)
}

pub(super) fn replacement(name: &str) -> Option<&'static Artifact> {
    artifact_for_coordinate(name, LEGACY_COORDINATE)
        .or_else(|| artifact_for_coordinate(name, FIXED_COORDINATE))
}

pub(super) fn replacement_for_classifier(
    classifier: &str,
) -> Option<&'static Artifact> {
    ARTIFACTS.iter().find(|artifact| {
        artifact.target.is_some() && artifact.classifier == classifier
    })
}

fn patch_entries<'a, T: 'a>(
    entries: impl Iterator<Item = &'a mut T>,
    library_of: impl Fn(&mut T) -> &mut Library,
    reset_local_path: impl Fn(&mut T),
) {
    for entry in entries {
        let library = library_of(entry);
        let Some(artifact) = replacement(&library.name) else {
            continue;
        };

        library.name = artifact.coordinate();
        library.downloadable = true;
        if let Some(checksums) = &mut library.checksums {
            *checksums = vec![artifact.sha1.to_owned()];
        }
        let downloads = library.downloads.get_or_insert(LibraryDownloads {
            artifact: None,
            classifiers: None,
        });
        downloads.artifact = Some(artifact.download());
        if let Some(classifiers) = &mut downloads.classifiers {
            for (classifier, download) in classifiers {
                if let Some(native) = replacement_for_classifier(classifier) {
                    *download = native.download();
                }
            }
        }
        if let Some(target) = artifact.target() {
            restrict_native_rules(library, target);
        }
        reset_local_path(entry);
    }
}

fn restrict_native_rules(library: &mut Library, target: OsRule) {
    let rules = library.rules.get_or_insert_with(Vec::new);
    if rules.is_empty() {
        rules.push(Rule {
            action: RuleAction::Allow,
            os: Some(target),
            features: None,
        });
        return;
    }

    // Mojang's native entries can allow the entire OS. Limit those allows
    // so an ARM JVM cannot pick the x86 archive (or the other way around).
    for rule in rules
        .iter_mut()
        .filter(|rule| matches!(rule.action, RuleAction::Allow))
    {
        let Some(os) = &mut rule.os else {
            rule.os = Some(target.clone());
            continue;
        };
        if matches!(os.name, None | Some(Os::Universal)) {
            os.name.clone_from(&target.name);
        }
        if os.arch.is_none() {
            os.arch.clone_from(&target.arch);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn library(name: &str) -> Library {
        serde_json::from_value(json!({"name": name})).unwrap()
    }

    fn download(library: &Library) -> &LibraryDownload {
        library
            .downloads
            .as_ref()
            .unwrap()
            .artifact
            .as_ref()
            .unwrap()
    }

    #[test]
    fn freetype_patch_matches_official_version_ids_and_jvm_platforms() {
        assert!(affected_game_version("26.3"));
        for snapshot in 1..=10 {
            assert!(affected_game_version(&format!(
                "26.4-snapshot-{snapshot}"
            )));
        }
        for unaffected in [
            "26.3-rc-3",
            "26.3.1",
            "26.4-snapshot-0",
            "26.4-snapshot-01",
            "26.4-snapshot-11",
            "26.4-snapshot-10-custom",
            "26.4-pre-1",
            "26.4",
        ] {
            assert!(!affected_game_version(unaffected), "{unaffected}");
        }

        for (os, arch) in [
            ("linux", "amd64"),
            ("windows", "x86_64"),
            ("windows", "aarch64"),
            ("macos", "x86_64"),
            ("macos", "arm64"),
        ] {
            assert!(supported_java_target(os, arch));
        }
        for (os, arch) in [
            ("windows", "x86"),
            ("linux", "aarch64"),
            ("linux", "riscv64"),
            ("freebsd", "x86_64"),
        ] {
            assert!(!supported_java_target(os, arch));
        }
    }

    #[test]
    fn freetype_patch_replaces_legacy_and_normalizes_fixed_duplicates() {
        let mut libraries =
            vec![
            library(LEGACY_COORDINATE),
            serde_json::from_value(json!({
                "name": FIXED_COORDINATE,
                "downloads": {"artifact": {
                    "path": "stale.jar", "url": "https://example.com/stale.jar",
                    "sha1": "stale", "size": 1
                }}
            })).unwrap(),
            serde_json::from_value(json!({
                "name": format!("{LEGACY_COORDINATE}:natives-windows-arm64"),
                "rules": [{"action":"allow", "os":{"name":"windows"}}],
                "extract": {"exclude": ["META-INF/"]},
                "include_in_classpath": false
            })).unwrap(),
            library("org.lwjgl:lwjgl:3.4.3"),
            library("org.lwjgl:lwjgl-freetype:3.4.3:natives-linux-arm64"),
        ];

        patch_entries(libraries.iter_mut(), |library| library, |_| {});

        let fixed_path = "org/lwjgl/lwjgl-freetype/3.4.3-mojang-2.15.0/lwjgl-freetype-3.4.3-mojang-2.15.0.jar";
        for fixed in &libraries[..2] {
            assert_eq!(fixed.name, FIXED_COORDINATE);
            assert!(is_patched_library(fixed));
            let artifact = download(fixed);
            assert_eq!(artifact.path.as_deref(), Some(fixed_path));
            assert_eq!(
                artifact.url,
                format!("https://libraries.minecraft.net/{fixed_path}")
            );
            assert_eq!(
                artifact.sha1,
                "134bc57b83e78a7f954a0f078093e319739d42ea"
            );
            assert_eq!(artifact.size, 469281);
        }

        let native = &libraries[2];
        assert_eq!(
            native.name,
            format!("{FIXED_COORDINATE}:natives-windows-arm64")
        );
        assert_eq!(
            download(native).sha1,
            "5f475c76490bb368f235017985d8d9dc6b5b5121"
        );
        assert_eq!(download(native).size, 1043884);
        let os = native.rules.as_ref().unwrap()[0].os.as_ref().unwrap();
        assert_eq!(os.name, Some(Os::Windows));
        assert_eq!(os.arch.as_deref(), Some("aarch64"));
        assert_eq!(
            native.extract.as_ref().unwrap().exclude.as_ref().unwrap(),
            &["META-INF/"]
        );
        assert!(!native.include_in_classpath);
        assert_eq!(libraries[3].name, "org.lwjgl:lwjgl:3.4.3");
        assert_eq!(
            libraries[4].name,
            "org.lwjgl:lwjgl-freetype:3.4.3:natives-linux-arm64"
        );
    }

    #[test]
    fn freetype_patch_entry_points_skip_unaffected_launches_and_are_idempotent()
    {
        let mut managed = [library(LEGACY_COORDINATE)];
        assert!(!patch_libraries("26.4-pre-1", &mut managed, "x86_64"));
        assert_eq!(managed[0].name, LEGACY_COORDINATE);
        if !supported_java_target(std::env::consts::OS, "x86_64") {
            return;
        }

        assert!(patch_libraries("26.3", &mut managed, "x86_64"));
        assert!(!patch_libraries("26.3", &mut managed, "x86_64"));

        let mut linked = [LinkedLibrary {
            library: library(LEGACY_COORDINATE),
            hint: Some("local".to_owned()),
            filename: Some("freetype.jar".to_owned()),
        }];
        assert!(patch_linked_libraries(
            "26.4-snapshot-10",
            &mut linked,
            "x86_64"
        ));
        assert!(!patch_linked_libraries(
            "26.4-snapshot-10",
            &mut linked,
            "x86_64"
        ));
        assert_eq!(linked[0].library.name, FIXED_COORDINATE);
        assert!(linked[0].hint.is_none());
        assert!(linked[0].filename.is_none());
    }
}

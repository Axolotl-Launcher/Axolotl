use crate::resources::ExportCategory;
use regex::Regex;
use std::sync::LazyLock;

struct Rule {
    id: &'static str,
    selected: bool,
    paths: &'static [&'static str],
    excluded: &'static [&'static str],
    included: &'static [&'static str],
}

const RULES: &[Rule] = &[
    Rule {
        id: "gameSettings",
        selected: true,
        paths: &["options.txt", "configureddefaults/"],
        excluded: &[],
        included: &[],
    },
    Rule {
        id: "personalRecords",
        selected: false,
        paths: &["hotbar.nbt", "command_history.txt"],
        excluded: &[],
        included: &[],
    },
    Rule {
        id: "optifine",
        selected: true,
        paths: &["optionsof.txt", "optionsshaders.txt"],
        excluded: &[],
        included: &[],
    },
    Rule {
        id: "oldResources",
        selected: false,
        paths: &[
            "mods/**/*.disabled",
            "mods/**/*.old",
            "coremods/**/*.disabled",
            "coremods/**/*.old",
            "lib/**/*.disabled",
            "lib/**/*.old",
            "plugins/**/*.disabled",
            "plugins/**/*.old",
        ],
        excluded: &[],
        included: &[],
    },
    Rule {
        id: "mapRecords",
        selected: false,
        paths: &[
            "journeymap/data/",
            "xaero/",
            "XaeroWaypoints/",
            "XaeroWorldMap/",
        ],
        excluded: &[],
        included: &[],
    },
    Rule {
        id: "jeiRecords",
        selected: false,
        paths: &["config/jei/world/"],
        excluded: &[],
        included: &[],
    },
    Rule {
        id: "emiRecords",
        selected: false,
        paths: &["emi.json"],
        excluded: &[],
        included: &[],
    },
    Rule {
        id: "patchouliRecords",
        selected: false,
        paths: &["patchouli_data.json"],
        excluded: &[],
        included: &[],
    },
    Rule {
        id: "mods",
        selected: true,
        paths: &["mods/", "coremods/", "lib/", "plugins/"],
        excluded: &[
            "mods/.connector/",
            "mods/mcef-libraries/",
            "mods/mcef-cache/",
        ],
        included: &[],
    },
    Rule {
        id: "modConfig",
        selected: true,
        paths: &[
            "config/",
            "journeymap/config/",
            "journeymap/server/",
            "TrashSlotSaveState.json",
            "customfov.txt",
            "gg.essential.mod/",
            "essential/",
            "paragliderSettings.nbt",
            "local/client_config.json",
            "local/ftbl.json",
            "local/client/sidebar_buttons.json",
            "local/client/ftbutilities.cfg",
            "local/client/ftblib.cfg",
            "local/client/xencraft.cfg",
            "liteloader.properties",
            "default_reference.xml",
            "CustomSkinLoader/CustomSkinLoader.json",
        ],
        excluded: &[
            "config/accountsx/",
            "config/jei/world/",
            "config/worldedit/",
            "config/spark/",
            "essential/*/",
            "essential/*.jar*",
            "essential/screenshot-checksum-caches.json",
        ],
        included: &[
            "config/worldedit/worldedit.properties",
            "config/spark/config.json",
        ],
    },
    Rule {
        id: "defaultConfig",
        selected: true,
        paths: &["defaultconfigs/"],
        excluded: &[],
        included: &[],
    },
    Rule {
        id: "importantData",
        selected: true,
        paths: &[
            "addons/",
            "multiblocked/",
            "modpack-update-checker/",
            "global_packs/",
            "global_resource_packs/",
            "global_data_packs/",
            "optional_data_packs/",
            "maps/",
            "icon.png",
            "mods-resourcepacks/",
            "matmos/",
            "resource_assorts/",
            "resource_assorts.json",
            "patchouli_books/",
            "datapacks/",
            "kubejs*/",
            "openloader/",
            "worldshape/",
            "resources/",
            "scripts/",
            "structures/",
            "fontfiles/",
            "oresources/",
            "packmenu/",
            "craftpresence/",
            "pointblanks/",
            "template*/",
        ],
        excluded: &[
            "kubejs*/probe/",
            "kubejs*/exported/",
            "kubejs*/jsconfig.json",
            "kubejs*/README.txt",
            "template*/playerdata/",
            "template*/stats/",
        ],
        included: &[],
    },
    Rule {
        id: "resourcePacks",
        selected: true,
        paths: &["resourcepacks/", "texturepacks/"],
        excluded: &[],
        included: &[],
    },
    Rule {
        id: "shaders",
        selected: true,
        paths: &["shaderpacks/"],
        excluded: &[],
        included: &[],
    },
    Rule {
        id: "worlds",
        selected: false,
        paths: &["saves/"],
        excluded: &[],
        included: &[],
    },
    Rule {
        id: "screenshots",
        selected: false,
        paths: &["screenshots/"],
        excluded: &[],
        included: &[],
    },
    Rule {
        id: "schematics",
        selected: false,
        paths: &["schematics/"],
        excluded: &[],
        included: &[],
    },
    Rule {
        id: "recordings",
        selected: false,
        paths: &["replay_recordings/", "replay_videos/"],
        excluded: &[],
        included: &[],
    },
    Rule {
        id: "servers",
        selected: false,
        paths: &["servers.dat"],
        excluded: &[],
        included: &[],
    },
];

const FORBIDDEN: &[&str] = &[
    "**/.axolotl-export-*.tmp",
    "assets/",
    "versions/",
    "libraries/",
    "*-natives/",
    "structureCacheV1/",
    ".fabric/",
    ".git/",
    "avatar-cache/",
    "cosmetic-cache/",
    "PCL/",
    "__MACOSX/",
    "modrinth_logs/",
    "launcher_logs/",
    "**/*.log",
    "**/*.dat_old",
    "**/*.BakaCoreInfo",
    "**/.DS_Store*",
    "hmclversion.cfg",
    "log4j2.xml",
    "profile.json",
    "essential/microsoft_accounts.json",
    "essential/microsoft_accounts.json*",
    "config/accountsx/",
    "accounts.json",
    "launcher_accounts.json",
    "launcher_profiles.json",
    ".axolotl/",
    ".theseus/",
];

fn compile(pattern: &str) -> Regex {
    let directory = pattern.ends_with('/');
    let segments = pattern.trim_end_matches('/').split('/').collect::<Vec<_>>();
    let mut expression = String::from("^");
    for (index, segment) in segments.iter().enumerate() {
        if *segment == "**" {
            expression.push_str("(?:[^/]+/)*");
            continue;
        }
        expression.push_str(
            &segment
                .split('*')
                .map(regex::escape)
                .collect::<Vec<_>>()
                .join("[^/]*"),
        );
        if index + 1 < segments.len() {
            expression.push('/');
        }
    }
    if directory {
        expression.push_str("(?:/.*)?");
    }
    expression.push('$');
    Regex::new(&expression).expect("static export path pattern")
}

fn file_pattern(pattern: &str) -> Regex {
    let expression = compile(pattern);
    if pattern.ends_with('/') {
        Regex::new(&expression.as_str().replace("(?:/.*)?$", "/.+$"))
            .expect("static export file pattern")
    } else {
        expression
    }
}

struct CompiledRule {
    rule: &'static Rule,
    paths: Vec<Regex>,
    excluded: Vec<Regex>,
    included: Vec<Regex>,
}
static COMPILED: LazyLock<Vec<CompiledRule>> = LazyLock::new(|| {
    RULES
        .iter()
        .map(|rule| CompiledRule {
            rule,
            paths: rule.paths.iter().map(|p| file_pattern(p)).collect(),
            excluded: rule.excluded.iter().map(|p| file_pattern(p)).collect(),
            included: rule.included.iter().map(|p| file_pattern(p)).collect(),
        })
        .collect()
});
static BLOCKED: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    FORBIDDEN
        .iter()
        .map(|p| {
            regex::RegexBuilder::new(compile(p).as_str())
                .case_insensitive(true)
                .build()
                .expect("static protected export pattern")
        })
        .collect()
});

pub fn forbidden(path: &str) -> bool {
    BLOCKED.iter().any(|pattern| pattern.is_match(path))
}

pub fn classify(path: &str) -> (&'static str, bool) {
    COMPILED
        .iter()
        .find(|rule| rule.paths.iter().any(|p| p.is_match(path)))
        .map(|rule| {
            (
                rule.rule.id,
                !rule.excluded.iter().any(|p| p.is_match(path))
                    || rule.included.iter().any(|p| p.is_match(path)),
            )
        })
        .unwrap_or(("other", true))
}

pub fn categories() -> Vec<ExportCategory> {
    let mut categories = RULES
        .iter()
        .map(|rule| ExportCategory {
            id: rule.id.into(),
            default_selected: rule.selected,
            paths: rule.paths.iter().map(|p| (*p).into()).collect(),
            children: vec![],
            title: None,
            modified_at: None,
        })
        .collect::<Vec<_>>();
    categories.push(ExportCategory {
        id: "other".into(),
        default_selected: false,
        paths: vec![],
        children: vec![],
        title: None,
        modified_at: None,
    });
    categories
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_rules_keep_categories_and_default_exclusions_distinct() {
        for (path, category, selected) in [
            ("essential/config.json", "modConfig", true),
            ("essential/sub/config.json", "modConfig", false),
            ("config/a.toml", "modConfig", true),
            ("defaultconfigs/a.toml", "defaultConfig", true),
            ("configureddefaults/options.txt", "gameSettings", true),
            ("config/jei/world/a.json", "jeiRecords", true),
            ("config/worldedit/worldedit.properties", "modConfig", true),
            ("config/worldedit/sessions/a", "modConfig", false),
            ("config/spark/config.json", "modConfig", true),
            ("config/spark/tmp", "modConfig", false),
            ("kubejs/probe/a", "importantData", false),
            ("kubejs-extra/exported/a", "importantData", false),
            ("template-x/playerdata/a", "importantData", false),
            ("kubejs/server_scripts/a.js", "importantData", true),
            ("mods/a.jar.disabled", "oldResources", true),
            ("mods/sub/a.jar.old", "oldResources", true),
            ("mods/.connector/a.jar", "mods", false),
            ("mods/mcef-cache/a", "mods", false),
            ("datapacks/a.zip", "importantData", true),
            ("saves/world/datapacks/a.zip", "worlds", true),
            ("maps/a", "importantData", true),
            ("journeymap/data/a", "mapRecords", true),
            ("shaderpacks/a.zip.txt", "shaders", true),
            ("config-extra/a", "other", true),
            ("unknown.txt", "other", true),
        ] {
            assert_eq!(classify(path), (category, selected), "{path}");
        }
    }

    #[test]
    fn global_exclusions_cannot_be_selected() {
        for path in [
            "Essential/Microsoft_Accounts.json",
            "assets/a",
            "versions/v/a",
            "linux-natives/a",
            ".fabric/a",
            "PCL/a",
            "a/b.log",
            "x.dat_old",
            "a/.DS_Store-x",
            "profile.json",
            "essential/microsoft_accounts.json",
            "config/accountsx/a.json",
        ] {
            assert!(forbidden(path), "{path}");
        }
        for path in [
            "config/profile.json",
            "mods/a.jar.old",
            "defaultconfigs/a",
            "essential/config.json",
            "natives-data/a",
        ] {
            assert!(!forbidden(path), "{path}");
        }
    }

    #[test]
    fn glob_stars_obey_segment_boundaries() {
        assert!(compile("mods/**/*.old").is_match("mods/a.old"));
        assert!(compile("mods/**/*.old").is_match("mods/a/b.old"));
        assert!(!compile("essential/*.jar*").is_match("essential/a/b.jar"));
        assert!(compile("essential/*/").is_match("essential/a/b/c"));
        assert!(!compile("kubejs*/probe/").is_match("kubejs/a/probe/b"));
    }
}

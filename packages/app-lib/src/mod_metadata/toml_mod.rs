use serde::Deserialize;
use std::collections::HashMap;

/// Forge / NeoForge mods.toml — `[[mods]]` array of tables plus root metadata.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModsToml {
    /// Name of the mod loader (e.g. "javafml").
    #[allow(dead_code)]
    pub mod_loader: Option<String>,
    /// Required loader version range (e.g. "[52,)"). For Forge this IS the Forge version.
    pub loader_version: Option<String>,
    #[serde(rename = "mods")]
    pub mods: Option<Vec<ModsTomlEntry>>,
    /// Dependencies keyed by modId: `[[dependencies.<modId>]]`.
    pub dependencies: Option<HashMap<String, Vec<ForgeDependencyEntry>>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModsTomlEntry {
    pub mod_id: Option<String>,
    pub display_name: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub authors: Option<String>,
    pub logo_file: Option<String>,
    #[serde(alias = "displayURL")]
    pub display_url: Option<String>,
    #[allow(dead_code)]
    pub credits: Option<String>,
}

/// An entry in a Forge/NeoForge `[[dependencies.<modId>]]` array.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ForgeDependencyEntry {
    pub mod_id: Option<String>,
    pub mandatory: Option<bool>,
    #[serde(rename = "type")]
    pub dependency_type: Option<String>,
    pub version_range: Option<String>,
    #[allow(dead_code)]
    pub ordering: Option<String>,
    #[allow(dead_code)]
    pub side: Option<String>,
}

impl ForgeDependencyEntry {
    pub fn required(&self) -> bool {
        match self.dependency_type.as_deref() {
            Some("required") => true,
            Some(_) => false,
            None => self.mandatory.unwrap_or(true),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neoforge_dependency_types_and_forge_mandatory_are_distinct() {
        for (declaration, required) in [
            ("type = 'required'", true),
            ("type = 'optional'", false),
            ("type = 'incompatible'", false),
            ("type = 'discouraged'", false),
            ("type = 'unknown'", false),
            ("mandatory = false", false),
            ("mandatory = true", true),
            ("", true),
        ] {
            let entry: ForgeDependencyEntry =
                toml::from_str(&format!("modId = 'example'\n{declaration}"))
                    .unwrap();
            assert_eq!(entry.required(), required, "{declaration}");
        }
    }

    #[test]
    fn parses_camel_case_mods_toml_fields() {
        let toml = r#"
modLoader = "javafml"
loaderVersion = "[4,)"

[[mods]]
modId = "sodium"
displayName = "Sodium"
logoFile = "sodium-icon.png"
displayURL = "https://example.com"

[[dependencies.sodium]]
modId = "minecraft"
type = "required"
versionRange = "1.21.1"
"#;
        let parsed: ModsToml = toml::from_str(toml).unwrap();
        let entry = parsed.mods.unwrap().into_iter().next().unwrap();
        assert_eq!(entry.mod_id.as_deref(), Some("sodium"));
        assert_eq!(entry.display_name.as_deref(), Some("Sodium"));
        assert_eq!(entry.logo_file.as_deref(), Some("sodium-icon.png"));
        assert_eq!(entry.display_url.as_deref(), Some("https://example.com"));

        let dependencies = parsed.dependencies.unwrap();
        let minecraft = dependencies["sodium"].first().unwrap();
        assert_eq!(minecraft.mod_id.as_deref(), Some("minecraft"));
        assert_eq!(minecraft.version_range.as_deref(), Some("1.21.1"));
    }
}

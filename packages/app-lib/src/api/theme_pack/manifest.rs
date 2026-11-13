//! The `manifest.json` contract every theme pack has to satisfy.
//!
//! The manifest is the only thing the launcher trusts before it reads any other
//! file in the pack. Everything a pack asks for, the colors it replaces, the
//! styles it injects, the pages it adds and the hosts it may call, is declared
//! here and checked again whenever the pack is used.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

use crate::ErrorKind;

pub const MANIFEST_FILE: &str = "manifest.json";
pub const SCHEMA_VERSION: u32 = 1;

/// A pack may not take the launcher's own namespace or impersonate a built-in
/// source, so ids with this prefix are rejected.
const RESERVED_ID_PREFIX: &str = "axolotl-";
const MAX_ID_LENGTH: usize = 64;
const MAX_TEXT_LENGTH: usize = 256;

/// A theme pack, as written by its author.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ThemePackManifest {
    pub schema_version: u32,
    pub id: String,
    /// A locale map, for example `{ "en-US": "Nord", "zh-CN": "极夜" }`.
    pub name: BTreeMap<String, String>,
    pub version: String,
    #[serde(default)]
    pub description: BTreeMap<String, String>,
    pub engines: ThemePackEngines,
    #[serde(default)]
    pub capabilities: Vec<ThemePackCapability>,
    #[serde(default)]
    pub tokens: ThemePackTokens,
    #[serde(default)]
    pub styles: Vec<String>,
    #[serde(default)]
    pub background: Option<String>,
    #[serde(default)]
    pub fonts: ThemePackFonts,
    #[serde(default)]
    pub permissions: Vec<ThemePackPermission>,
    #[serde(default)]
    pub network: ThemePackNetwork,
    #[serde(default)]
    pub pages: Vec<ThemePackPage>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ThemePackEngines {
    /// A semver requirement against the launcher version, for example
    /// `">=1.11.0"`.
    pub axolotl: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ThemePackCapability {
    Tokens,
    Styles,
    Pages,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct ThemePackTokens {
    /// CSS custom properties for the light palette.
    #[serde(default)]
    pub light: BTreeMap<String, String>,
    /// CSS custom properties for the dark palette.
    #[serde(default)]
    pub dark: BTreeMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct ThemePackFonts {
    /// A font file under `assets/`, used as the interface font.
    #[serde(default)]
    pub ui: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemePackPermission {
    #[serde(rename = "storage")]
    Storage,
    #[serde(rename = "instances.read")]
    InstancesRead,
    #[serde(rename = "instances.logs")]
    InstancesLogs,
    #[serde(rename = "network")]
    Network,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct ThemePackNetwork {
    /// Exact hosts (`api.example.com`) or subdomain wildcards
    /// (`*.example.com`). A bare `*` is rejected.
    #[serde(default)]
    pub allow: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ThemePackPage {
    pub id: String,
    pub title: BTreeMap<String, String>,
    #[serde(default)]
    pub icon: Option<String>,
    /// An HTML entry under `pages/`.
    pub entry: String,
    #[serde(default)]
    pub order: i32,
    /// Must be a subset of the pack's permissions.
    #[serde(default)]
    pub requires: Vec<ThemePackPermission>,
}

impl ThemePackManifest {
    pub fn parse(bytes: &[u8]) -> crate::Result<Self> {
        serde_json::from_slice(bytes).map_err(|error| {
            ErrorKind::InputError(format!(
                "The theme pack manifest is not valid JSON: {error}"
            ))
            .into()
        })
    }

    pub fn load(pack_dir: &Path) -> crate::Result<Self> {
        let bytes =
            std::fs::read(pack_dir.join(MANIFEST_FILE)).map_err(|error| {
                ErrorKind::InputError(format!(
                    "The theme pack manifest could not be read: {error}"
                ))
            })?;
        Self::parse(&bytes)
    }

    /// Rejects a manifest the launcher cannot honor, and every file it names
    /// that is missing or escapes the pack directory.
    pub fn validate(&self, pack_dir: &Path) -> crate::Result<()> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(invalid(format!(
                "Theme pack schema version {} is not supported",
                self.schema_version
            )));
        }
        validate_id(&self.id)?;
        semver::Version::parse(&self.version).map_err(|error| {
            invalid(format!("The theme pack version is not semver: {error}"))
        })?;
        validate_locale_map("name", &self.name)?;
        if !self.description.is_empty() {
            validate_locale_map("description", &self.description)?;
        }
        semver::VersionReq::parse(&self.engines.axolotl).map_err(|error| {
            invalid(format!(
                "The theme pack's launcher requirement is not a version range: {error}"
            ))
        })?;
        if self.capabilities.is_empty() {
            return Err(invalid("A theme pack must declare a capability"));
        }
        let mut capabilities = BTreeSet::new();
        for capability in &self.capabilities {
            capabilities.insert(format!("{capability:?}"));
        }
        if capabilities.len() != self.capabilities.len() {
            return Err(invalid("A theme pack declares a capability twice"));
        }

        if capabilities.contains("Tokens") {
            validate_tokens("light", &self.tokens.light)?;
            validate_tokens("dark", &self.tokens.dark)?;
        } else if !self.tokens.light.is_empty() || !self.tokens.dark.is_empty()
        {
            return Err(invalid(
                "A theme pack that declares tokens must list the tokens capability",
            ));
        }
        if capabilities.contains("Styles") && self.styles.is_empty() {
            return Err(invalid(
                "A theme pack with the styles capability must list a stylesheet",
            ));
        }
        if !capabilities.contains("Styles") && !self.styles.is_empty() {
            return Err(invalid(
                "A theme pack that lists stylesheets must declare the styles capability",
            ));
        }
        if !capabilities.contains("Tokens")
            && !capabilities.contains("Styles")
            && (self.background.is_some() || self.fonts.ui.is_some())
        {
            return Err(invalid(
                "A background or interface font requires the tokens or styles capability",
            ));
        }
        if self.styles.len() > 32 {
            return Err(invalid(
                "A theme pack may list at most 32 stylesheets",
            ));
        }
        for style in &self.styles {
            let relative = validate_relative(style)?;
            if relative.first().map(String::as_str) != Some("styles") {
                return Err(invalid(format!(
                    "Stylesheet {style} must live under styles/"
                )));
            }
            if relative.last().is_none_or(|name| !name.ends_with(".css")) {
                return Err(invalid(format!(
                    "Stylesheet {style} must have the .css extension"
                )));
            }
            let path = checked_file(pack_dir, style)?;
            let css = std::fs::read_to_string(&path).map_err(|error| {
                invalid(format!(
                    "Stylesheet {style} could not be read: {error}"
                ))
            })?;
            validate_css(style, &css)?;
        }

        if let Some(background) = &self.background {
            let relative = validate_relative(background)?;
            if relative.first().map(String::as_str) != Some("assets") {
                return Err(invalid(format!(
                    "Background {background} must live under assets/"
                )));
            }
            checked_file(pack_dir, background)?;
        }
        if let Some(font) = &self.fonts.ui {
            let relative = validate_relative(font)?;
            if relative.first().map(String::as_str) != Some("assets") {
                return Err(invalid(format!(
                    "Font {font} must live under assets/"
                )));
            }
            checked_file(pack_dir, font)?;
        }

        let mut permissions = BTreeSet::new();
        for permission in &self.permissions {
            permissions.insert(format!("{permission:?}"));
        }
        if permissions.len() != self.permissions.len() {
            return Err(invalid("A theme pack declares a permission twice"));
        }
        if !self.network.allow.is_empty() {
            if !permissions.contains("Network") {
                return Err(invalid(
                    "A theme pack with network hosts must request the network permission",
                ));
            }
            if self.network.allow.len() > 64 {
                return Err(invalid(
                    "A theme pack may allow at most 64 network hosts",
                ));
            }
            for host in &self.network.allow {
                validate_host(host)?;
            }
        }

        if !self.pages.is_empty() {
            if !capabilities.contains("Pages") {
                return Err(invalid(
                    "A theme pack with pages must declare the pages capability",
                ));
            }
            if self.pages.len() > 16 {
                return Err(invalid("A theme pack may add at most 16 pages"));
            }
        }
        let mut page_ids = BTreeSet::new();
        for page in &self.pages {
            validate_page_id(&page.id)?;
            if !page_ids.insert(page.id.clone()) {
                return Err(invalid(format!(
                    "The theme pack declares the page {} twice",
                    page.id
                )));
            }
            validate_locale_map("page title", &page.title)?;
            let entry = validate_relative(&page.entry)?;
            if entry.first().map(String::as_str) != Some("pages") {
                return Err(invalid(format!(
                    "Page entry {} must live under pages/",
                    page.entry
                )));
            }
            if entry.last().is_none_or(|name| !name.ends_with(".html")) {
                return Err(invalid(format!(
                    "Page entry {} must have the .html extension",
                    page.entry
                )));
            }
            checked_file(pack_dir, &page.entry)?;
            if let Some(icon) = &page.icon {
                let icon_relative = validate_relative(icon)?;
                if icon_relative.first().map(String::as_str) != Some("assets") {
                    return Err(invalid(format!(
                        "Page icon {icon} must live under assets/"
                    )));
                }
                checked_file(pack_dir, icon)?;
            }
            if !(-1_000..=1_000).contains(&page.order) {
                return Err(invalid(
                    "A page order must be between -1000 and 1000",
                ));
            }
            for required in &page.requires {
                if !permissions.contains(&format!("{required:?}")) {
                    return Err(invalid(format!(
                        "Page {} requires a permission the pack does not request",
                        page.id
                    )));
                }
            }
        }

        Ok(())
    }

    /// Whether the launcher version satisfies `engines.axolotl`. Pre-release
    /// launcher builds compare on the release triple so the packs written for
    /// the upcoming stable are not rejected while testing it.
    pub fn supports_launcher(&self, version: &semver::Version) -> bool {
        let Ok(requirement) = semver::VersionReq::parse(&self.engines.axolotl)
        else {
            return false;
        };
        if requirement.matches(version) {
            return true;
        }
        if version.pre.is_empty() {
            return false;
        }
        let stable =
            semver::Version::new(version.major, version.minor, version.patch);
        requirement.matches(&stable)
    }

    pub fn has_capability(&self, capability: ThemePackCapability) -> bool {
        self.capabilities.contains(&capability)
    }

    pub fn has_permission(&self, permission: ThemePackPermission) -> bool {
        self.permissions.contains(&permission)
    }

    /// Picks the best locale match, falling back to English and then to any
    /// translated value, so a pack without an English name still renders.
    pub fn localized_name<'a>(&'a self, locale: Option<&str>) -> &'a str {
        localized(&self.name, locale).unwrap_or(&self.id)
    }

    pub fn localized_description<'a>(
        &'a self,
        locale: Option<&str>,
    ) -> Option<&'a str> {
        localized(&self.description, locale)
    }

    /// Returns the page entry path without checking the filesystem; callers
    /// resolving a page should go through [`crate::api::theme_pack::asset_path`].
    pub fn page(&self, page_id: &str) -> Option<&ThemePackPage> {
        self.pages.iter().find(|page| page.id == page_id)
    }
}

impl ThemePackPage {
    pub fn localized_title<'a>(&'a self, locale: Option<&str>) -> &'a str {
        localized(&self.title, locale).unwrap_or(&self.id)
    }
}

fn localized<'a>(
    map: &'a BTreeMap<String, String>,
    locale: Option<&str>,
) -> Option<&'a str> {
    if let Some(locale) = locale {
        let normalized = locale.replace('_', "-");
        if let Some(value) = map.get(&normalized) {
            return Some(value);
        }
        let language = normalized
            .split('-')
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        if let Some((_, value)) = map.iter().find(|(key, _)| {
            key.split(['-', '_'])
                .next()
                .is_some_and(|key| key.eq_ignore_ascii_case(&language))
        }) {
            return Some(value);
        }
    }
    if let Some(value) = map.get("en-US").or_else(|| map.get("en")) {
        return Some(value);
    }
    map.values().next().map(String::as_str)
}

fn validate_locale_map(
    field: &str,
    map: &BTreeMap<String, String>,
) -> crate::Result<()> {
    if map.is_empty() {
        return Err(invalid(format!(
            "The theme pack {field} must have at least one localized value"
        )));
    }
    if map.len() > 64 {
        return Err(invalid(format!(
            "The theme pack {field} has too many locales"
        )));
    }
    for (locale, value) in map {
        if locale.trim().is_empty() || locale.len() > 35 {
            return Err(invalid(format!(
                "The theme pack {field} has an invalid locale key"
            )));
        }
        if value.trim().is_empty() {
            return Err(invalid(format!(
                "The theme pack {field} has an empty value"
            )));
        }
        if value.chars().count() > 200 {
            return Err(invalid(format!(
                "The theme pack {field} value is too long"
            )));
        }
    }
    Ok(())
}

fn validate_tokens(
    palette: &str,
    tokens: &BTreeMap<String, String>,
) -> crate::Result<()> {
    const MAX_TOKEN_COUNT: usize = 512;
    const MAX_TOKEN_LENGTH: usize = 256;
    if tokens.len() > MAX_TOKEN_COUNT {
        return Err(invalid(format!(
            "The {palette} token set declares too many tokens"
        )));
    }
    for (key, value) in tokens {
        let valid_name = key.starts_with("--")
            && key.len() <= 64
            && !key[2..].is_empty()
            && key[2..].chars().all(|character| {
                character.is_ascii_lowercase()
                    || character.is_ascii_digit()
                    || character == '-'
            });
        if !valid_name {
            return Err(invalid(format!(
                "The {palette} token {key} has an invalid name"
            )));
        }
        if value.trim().is_empty()
            || value.chars().count() > MAX_TOKEN_LENGTH
            || value.chars().any(char::is_control)
        {
            return Err(invalid(format!(
                "The {palette} token {key} is empty or too long"
            )));
        }
        let lowered = value.to_ascii_lowercase();
        if lowered.contains('\\')
            || lowered.contains("url(")
            || lowered.contains("image-set(")
            || lowered.contains("src(")
            || lowered.contains("var(")
            || lowered.contains("expression(")
            || lowered.contains("@import")
            || lowered.contains(';')
            || lowered.contains('{')
            || lowered.contains('}')
        {
            return Err(invalid(format!(
                "The {palette} token {key} contains CSS the launcher does not allow"
            )));
        }
    }
    for key in ["--surface-1", "--surface-3", "--color-text-primary"] {
        if !tokens.contains_key(key) {
            return Err(invalid(format!(
                "The {palette} token set must define {key}"
            )));
        }
    }
    Ok(())
}

/// A pack id: lowercase ASCII, digits, `.`, `_` and `-`, starting with a
/// letter or digit, at most 64 characters.
pub fn validate_id(id: &str) -> crate::Result<()> {
    if id.len() > MAX_ID_LENGTH || id.len() < 2 {
        return Err(invalid("A theme pack id must be 2 to 64 characters"));
    }
    let mut chars = id.chars();
    let Some(first) = chars.next() else {
        return Err(invalid("A theme pack id cannot be empty"));
    };
    if !first.is_ascii_alphanumeric() {
        return Err(invalid(
            "A theme pack id must start with a letter or digit",
        ));
    }
    if !chars.all(|character| {
        character.is_ascii_lowercase()
            || character.is_ascii_digit()
            || matches!(character, '.' | '_' | '-')
    }) {
        return Err(invalid(
            "A theme pack id may only contain lowercase letters, digits, '.', '_' and '-'",
        ));
    }
    if id.starts_with(RESERVED_ID_PREFIX) {
        return Err(invalid(
            "Theme pack ids may not use the reserved axolotl- prefix",
        ));
    }
    if id.ends_with('.') || is_windows_device_name(id) {
        return Err(invalid(
            "Theme pack ids may not end with '.' or use a Windows device name",
        ));
    }
    Ok(())
}

fn validate_page_id(id: &str) -> crate::Result<()> {
    if id.is_empty()
        || id.len() > MAX_ID_LENGTH
        || !id.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '_' | '-')
        })
    {
        return Err(invalid(format!(
            "Page id {id} may only contain lowercase letters, digits, '_' and '-'"
        )));
    }
    Ok(())
}

/// Splits a pack-relative path so every caller applies the same rules: no
/// absolute paths, drive letters, backslashes, `.`/`..` or empty components.
pub fn validate_relative(path: &str) -> crate::Result<Vec<String>> {
    if path.is_empty() || path.len() > 512 {
        return Err(invalid("A theme pack path is empty or too long"));
    }
    if path.starts_with('/') || path.contains('\\') {
        return Err(invalid(format!(
            "Theme pack path {path} must be relative and use forward slashes"
        )));
    }
    if path.contains('\0') || path.chars().any(char::is_control) {
        return Err(invalid(format!(
            "Theme pack path {path} contains a control character"
        )));
    }
    let mut components = Vec::new();
    for component in path.split('/') {
        if component.is_empty()
            || component == "."
            || component == ".."
            || component.trim_end() != component
            || component.trim_end_matches([' ', '.']) == ".."
            || component.contains(':')
            || is_windows_device_name(component)
        {
            return Err(invalid(format!(
                "Theme pack path {path} contains an unsafe component"
            )));
        }
        components.push(component.to_string());
    }
    Ok(components)
}

/// Rejects the reserved DOS device names, which Windows resolves in every
/// directory even without an extension (`con`, `nul.txt`, `COM1`, …).
pub fn is_windows_device_name(component: &str) -> bool {
    let stem = component
        .split('.')
        .next()
        .unwrap_or(component)
        .trim_end_matches(' ')
        .to_ascii_lowercase();
    matches!(stem.as_str(), "con" | "prn" | "aux" | "nul")
        || ((stem.starts_with("com") || stem.starts_with("lpt"))
            && stem.len() == 4
            && matches!(stem.as_bytes()[3], b'1'..=b'9'))
}

/// Resolves a pack-relative path and proves the result is a regular file
/// inside the pack directory. `canonicalize` also defeats a symlink that
/// points outside the pack.
pub fn checked_file(pack_dir: &Path, relative: &str) -> crate::Result<PathBuf> {
    let components = validate_relative(relative)?;
    let mut path = pack_dir.to_path_buf();
    for component in components {
        path.push(component);
    }
    let canonical_pack = std::fs::canonicalize(pack_dir).map_err(|error| {
        invalid(format!(
            "The theme pack directory is not accessible: {error}"
        ))
    })?;
    let canonical = std::fs::canonicalize(&path).map_err(|_| {
        invalid(format!("Theme pack file {relative} does not exist"))
    })?;
    if !canonical.starts_with(&canonical_pack) {
        return Err(invalid(format!(
            "Theme pack file {relative} resolves outside the pack"
        )));
    }
    if !canonical.is_file() {
        return Err(invalid(format!(
            "Theme pack path {relative} is not a regular file"
        )));
    }
    Ok(canonical)
}

/// Rejects remote references in pack CSS so a stylesheet cannot phone home for
/// a background image and leak the user's IP or a locally injected identifier.
pub fn validate_css(name: &str, css: &str) -> crate::Result<()> {
    // Decode CSS escapes and drop comments before looking for references.
    // Without this, `u\72l(https://…)` would slip past a literal `url(` scan.
    let normalized = normalize_css(css);
    // A backslash left after decoding means the source used a second escape
    // layer (`\5c 75 rl`, `\\`). The browser would decode that layer later and
    // could reveal a `url(` or `@import`; reject instead of rescanning forever.
    if normalized.contains('\\') {
        return Err(invalid(format!(
            "Stylesheet {name} contains an escaped sequence the launcher does not allow"
        )));
    }
    let lowered = normalized.to_ascii_lowercase();
    if lowered.contains("@import") {
        return Err(invalid(format!("Stylesheet {name} may not use @import")));
    }
    // `image-set()` and `src()` accept bare strings, so they never reach the
    // `url()` scan below. Reject them outright: pack stylesheets are applied to
    // the main window and must not fetch remote images.
    if lowered.contains("image-set(")
        || lowered.contains("-webkit-image-set(")
        || lowered.contains("src(")
    {
        return Err(invalid(format!(
            "Stylesheet {name} references a resource the launcher does not allow"
        )));
    }
    let mut rest = lowered.as_str();
    while let Some(index) = rest.find("url(") {
        rest = &rest[index + 4..];
        let Some(end) = rest.find(')') else {
            return Err(invalid(format!(
                "Stylesheet {name} has an unterminated url() reference"
            )));
        };
        let argument = rest[..end].trim().trim_matches(['"', '\'']).trim();
        let scheme = argument.split_once(':').map(|(scheme, _)| scheme).filter(
            |scheme| {
                !scheme.is_empty()
                    && scheme.chars().all(|character| {
                        character.is_ascii_alphanumeric()
                            || matches!(character, '+' | '-' | '.')
                    })
            },
        );
        if argument.starts_with("//")
            || matches!(scheme, Some(scheme) if scheme != "data" && scheme != "axolotl-theme")
            || argument.starts_with("data:text/html")
        {
            return Err(invalid(format!(
                "Stylesheet {name} references a resource the launcher does not allow"
            )));
        }
        rest = &rest[end..];
    }
    if lowered.contains("expression(") || lowered.contains("-moz-binding") {
        return Err(invalid(format!(
            "Stylesheet {name} uses CSS the launcher does not allow"
        )));
    }
    Ok(())
}

/// Expands CSS backslash escapes and removes comments so validation sees the
/// same text the renderer will.
fn normalize_css(css: &str) -> String {
    let characters: Vec<char> = css.chars().collect();
    let mut normalized = String::with_capacity(css.len());
    let mut index = 0;
    while index < characters.len() {
        if characters[index] == '/' && characters.get(index + 1) == Some(&'*') {
            index += 2;
            while index + 1 < characters.len()
                && !(characters[index] == '*' && characters[index + 1] == '/')
            {
                index += 1;
            }
            index = (index + 2).min(characters.len());
            continue;
        }
        if characters[index] == '\\' && index + 1 < characters.len() {
            let mut end = index + 1;
            let mut value = 0u32;
            let mut digits = 0;
            while end < characters.len()
                && digits < 6
                && characters[end].is_ascii_hexdigit()
            {
                value = value * 16
                    + characters[end].to_digit(16).unwrap_or_default();
                end += 1;
                digits += 1;
            }
            if digits > 0 {
                if let Some(character) = char::from_u32(value) {
                    normalized.push(character);
                }
                if end < characters.len() && characters[end].is_whitespace() {
                    end += 1;
                }
                index = end;
                continue;
            }
            normalized.push(characters[index + 1]);
            index += 2;
            continue;
        }
        normalized.push(characters[index]);
        index += 1;
    }
    normalized
}

fn validate_host(host: &str) -> crate::Result<()> {
    if host.is_empty() || host.len() > 253 {
        return Err(invalid("A theme pack network host is empty or too long"));
    }
    let name = host.strip_prefix("*.").unwrap_or(host);
    if name.is_empty() || name == "*" || name.contains('*') {
        return Err(invalid(format!(
            "Theme pack network host {host} is not an exact host or *.subdomain"
        )));
    }
    if name.contains('/') || name.contains(':') || name.contains('@') {
        return Err(invalid(format!(
            "Theme pack network host {host} must be a host name"
        )));
    }
    if name.split('.').any(|label| {
        label.is_empty()
            || label.len() > 63
            || !label.chars().all(|character| {
                character.is_ascii_alphanumeric() || character == '-'
            })
            || label.starts_with('-')
            || label.ends_with('-')
    }) {
        return Err(invalid(format!(
            "Theme pack network host {host} is not a valid domain name"
        )));
    }
    Ok(())
}

fn invalid(message: impl Into<String>) -> crate::Error {
    ErrorKind::InputError(message.into()).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> ThemePackManifest {
        serde_json::from_str(
            r##"{
                "schemaVersion": 1,
                "id": "nord-night",
                "name": {"en-US": "Nord Night", "zh-CN": "北境之夜"},
                "version": "1.0.0",
                "engines": {"axolotl": ">=1.11.0"},
                "capabilities": ["tokens", "styles", "pages"],
                "tokens": {
                    "light": {
                        "--surface-1": "#f2f4f8",
                        "--surface-3": "#ffffff",
                        "--color-text-primary": "#101215"
                    },
                    "dark": {
                        "--surface-1": "#101215",
                        "--surface-3": "#262b34",
                        "--color-text-primary": "#eceff4"
                    }
                },
                "styles": ["styles/theme.css"],
                "background": "assets/background.png",
                "fonts": {"ui": "assets/Inter.woff2"},
                "permissions": ["instances.read", "network"],
                "network": {"allow": ["api.example.com", "*.example.org"]},
                "pages": [{
                    "id": "nord-home",
                    "title": {"en-US": "Nord"},
                    "icon": "assets/icon.png",
                    "entry": "pages/home.html",
                    "order": 20,
                    "requires": ["instances.read"]
                }]
            }"##,
        )
        .expect("the fixture manifest is valid")
    }

    fn pack_dir(files: &[&str]) -> tempfile::TempDir {
        let directory = tempfile::tempdir().expect("temp dir");
        for file in files {
            let path = directory.path().join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(
                &path,
                if file.ends_with(".css") {
                    ".surface { color: var(--color-text-primary); }"
                } else {
                    "fixture"
                },
            )
            .unwrap();
        }
        directory
    }

    #[test]
    fn accepts_a_well_formed_manifest() {
        let directory = pack_dir(&[
            "styles/theme.css",
            "assets/background.png",
            "assets/Inter.woff2",
            "assets/icon.png",
            "pages/home.html",
        ]);
        manifest()
            .validate(directory.path())
            .expect("the fixture should validate");
    }

    #[test]
    fn rejects_a_manifest_without_the_required_tokens() {
        let directory = pack_dir(&[
            "styles/theme.css",
            "assets/background.png",
            "assets/Inter.woff2",
            "assets/icon.png",
            "pages/home.html",
        ]);
        let mut manifest = manifest();
        manifest.tokens.dark.remove("--surface-3");
        let error = manifest.validate(directory.path()).unwrap_err();
        assert!(error.to_string().contains("--surface-3"));
    }

    #[test]
    fn rejects_tokens_without_the_tokens_capability() {
        let directory = pack_dir(&["styles/theme.css"]);
        let mut manifest = manifest();
        manifest
            .capabilities
            .retain(|capability| *capability != ThemePackCapability::Tokens);
        let error = manifest.validate(directory.path()).unwrap_err();
        assert!(error.to_string().contains("tokens capability"));
    }

    #[test]
    fn rejects_styles_without_the_styles_capability() {
        let directory = pack_dir(&["styles/theme.css"]);
        let mut manifest = manifest();
        manifest
            .capabilities
            .retain(|capability| *capability != ThemePackCapability::Styles);
        let error = manifest.validate(directory.path()).unwrap_err();
        assert!(error.to_string().contains("styles capability"));
    }

    #[test]
    fn rejects_page_only_packs_that_restyle_the_launcher() {
        let directory = pack_dir(&["assets/background.png"]);
        let mut manifest = manifest();
        manifest.capabilities = vec![ThemePackCapability::Pages];
        manifest.tokens.light.clear();
        manifest.tokens.dark.clear();
        manifest.styles.clear();
        manifest.fonts.ui = None;
        manifest.pages = Vec::new();
        let error = manifest.validate(directory.path()).unwrap_err();
        assert!(error.to_string().contains("background or interface font"));
    }

    #[test]
    fn rejects_paths_that_escape_the_pack() {
        let directory = pack_dir(&["styles/theme.css"]);
        let mut manifest = manifest();
        manifest.styles = vec!["../outside.css".to_string()];
        assert!(manifest.validate(directory.path()).is_err());
    }

    #[test]
    fn rejects_a_bare_wildcard_host() {
        let directory = pack_dir(&[
            "styles/theme.css",
            "assets/background.png",
            "assets/Inter.woff2",
            "assets/icon.png",
            "pages/home.html",
        ]);
        let mut manifest = manifest();
        manifest.network.allow = vec!["*".to_string()];
        assert!(manifest.validate(directory.path()).is_err());
    }

    #[test]
    fn rejects_a_page_that_requires_a_missing_permission() {
        let directory = pack_dir(&[
            "styles/theme.css",
            "assets/background.png",
            "assets/Inter.woff2",
            "assets/icon.png",
            "pages/home.html",
        ]);
        let mut manifest = manifest();
        manifest.pages[0]
            .requires
            .push(ThemePackPermission::InstancesLogs);
        assert!(manifest.validate(directory.path()).is_err());
    }

    #[test]
    fn rejects_css_that_references_a_remote_resource() {
        let directory = pack_dir(&[
            "styles/theme.css",
            "assets/background.png",
            "assets/Inter.woff2",
            "assets/icon.png",
            "pages/home.html",
        ]);
        std::fs::write(
            directory.path().join("styles/theme.css"),
            ".x { background: url(https://tracker.example/pixel.png); }",
        )
        .unwrap();
        assert!(manifest().validate(directory.path()).is_err());
    }

    #[test]
    fn rejects_escaped_css_references() {
        let directory = pack_dir(&[
            "styles/theme.css",
            "assets/background.png",
            "assets/Inter.woff2",
            "assets/icon.png",
            "pages/home.html",
        ]);
        for css in [
            r#".x { background: u\72l(https://tracker.example/pixel.png); }"#,
            r#".x { background: url(\68 ttps://tracker.example/pixel.png); }"#,
            r#".x { background: url(/* gap */ https://tracker.example/a.png); }"#,
            r#".x { background: url(f\69 le:///etc/passwd); }"#,
        ] {
            std::fs::write(directory.path().join("styles/theme.css"), css)
                .unwrap();
            assert!(
                manifest().validate(directory.path()).is_err(),
                "expected {css} to be rejected"
            );
        }
    }

    #[test]
    fn rejects_image_set_css_references() {
        let directory = pack_dir(&[
            "styles/theme.css",
            "assets/background.png",
            "assets/Inter.woff2",
            "assets/icon.png",
            "pages/home.html",
        ]);
        for css in [
            r#".x { background: image-set("https://tracker.example/x.png" 1x); }"#,
            r#".x { background: -webkit-image-set("https://tracker.example/x.png" 1x); }"#,
            r#".x { src("https://tracker.example/x.woff2"); }"#,
        ] {
            std::fs::write(directory.path().join("styles/theme.css"), css)
                .unwrap();
            assert!(
                manifest().validate(directory.path()).is_err(),
                "expected {css} to be rejected"
            );
        }
    }

    #[test]
    fn rejects_unterminated_css_references() {
        let directory = pack_dir(&[
            "styles/theme.css",
            "assets/background.png",
            "assets/Inter.woff2",
            "assets/icon.png",
            "pages/home.html",
        ]);
        std::fs::write(
            directory.path().join("styles/theme.css"),
            ".x { background: url(https://tracker.example/pixel.png",
        )
        .unwrap();
        assert!(manifest().validate(directory.path()).is_err());
    }

    #[test]
    fn accepts_relative_css_references() {
        let directory = pack_dir(&[
            "styles/theme.css",
            "assets/background.png",
            "assets/Inter.woff2",
            "assets/icon.png",
            "pages/home.html",
        ]);
        std::fs::write(
            directory.path().join("styles/theme.css"),
            ".x { background: url(../assets/background.png); }",
        )
        .unwrap();
        assert!(manifest().validate(directory.path()).is_ok());
    }

    #[test]
    fn accepts_theme_origin_css_references() {
        let directory = pack_dir(&[
            "styles/theme.css",
            "assets/background.png",
            "assets/Inter.woff2",
            "assets/icon.png",
            "pages/home.html",
        ]);
        std::fs::write(
            directory.path().join("styles/theme.css"),
            ".x { background: url(axolotl-theme://localhost/assets/background.png); }",
        )
        .unwrap();
        assert!(manifest().validate(directory.path()).is_ok());
    }

    #[test]
    fn rejects_double_escaped_css_references() {
        for css in [
            r#".x { background: \5c 75 rl(https://tracker.example/a.png); }"#,
            r#".x { \5c 40 import "https://tracker.example/a.css"; }"#,
            r#".x { background: url(\\tracker.example/a.png); }"#,
        ] {
            assert!(
                validate_css("theme.css", css).is_err(),
                "expected {css} to be rejected"
            );
        }
    }

    #[test]
    fn validates_every_token_not_only_the_required_ones() {
        let directory = pack_dir(&[
            "styles/theme.css",
            "assets/background.png",
            "assets/Inter.woff2",
            "assets/icon.png",
            "pages/home.html",
        ]);
        let mut manifest = manifest();
        manifest.tokens.light.insert(
            "--custom-beacon".to_string(),
            "url(https://tracker.example/pixel)".to_string(),
        );
        assert!(manifest.validate(directory.path()).is_err());

        manifest.tokens.light.insert(
            "--custom-beacon".to_string(),
            "var(--custom-beacon)".to_string(),
        );
        assert!(manifest.validate(directory.path()).is_err());

        manifest
            .tokens
            .light
            .insert("--custom-beacon".to_string(), "#123456".to_string());
        assert!(manifest.validate(directory.path()).is_ok());
    }

    #[test]
    fn rejects_windows_special_path_components() {
        for path in [
            "pages/con.html",
            "assets/nul.txt",
            "assets/.. ",
            "assets/COM1",
        ] {
            assert!(
                validate_relative(path).is_err(),
                "expected {path} to be rejected"
            );
        }
        assert!(validate_relative("pages/home.html").is_ok());
    }

    #[test]
    fn rejects_windows_device_names_as_ids() {
        for id in [
            "con", "con.txt", "prn", "aux", "nul", "com1", "lpt9", "nord.",
        ] {
            assert!(validate_id(id).is_err(), "expected {id} to be rejected");
        }
        for id in ["console", "com10", "lpt0", "theme-pack"] {
            assert!(validate_id(id).is_ok(), "expected {id} to be accepted");
        }
    }

    #[test]
    fn picks_the_best_locale_name() {
        let manifest = manifest();
        assert_eq!(manifest.localized_name(Some("zh-CN")), "北境之夜");
        assert_eq!(manifest.localized_name(Some("zh-TW")), "北境之夜");
        assert_eq!(manifest.localized_name(Some("fr-FR")), "Nord Night");
        assert_eq!(manifest.localized_name(None), "Nord Night");
    }

    #[test]
    fn accepts_packs_against_a_prerelease_launcher() {
        let manifest = manifest();
        let version = semver::Version::parse("1.11.0-beta.4").unwrap();
        assert!(manifest.supports_launcher(&version));
        let old = semver::Version::parse("1.10.0").unwrap();
        assert!(!manifest.supports_launcher(&old));
    }
}

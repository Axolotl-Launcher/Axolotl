//! Wire types shared by every schematic source.

use serde::{Deserialize, Serialize};

/// Which upstream site an item came from.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceId {
    Reden,
    BuildSchematics,
    McSchematics,
}

impl SourceId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Reden => "reden",
            Self::BuildSchematics => "buildschematics",
            Self::McSchematics => "mc_schematics",
        }
    }

    /// Parse a source id from its wire form.
    ///
    /// Named `parse` rather than `from_str` so it does not shadow the
    /// [`std::str::FromStr`] convention, which clippy flags as confusing.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "reden" => Some(Self::Reden),
            "buildschematics" => Some(Self::BuildSchematics),
            "mc_schematics" | "mcschematics" => Some(Self::McSchematics),
            _ => None,
        }
    }

    pub fn all() -> &'static [Self] {
        &[Self::Reden, Self::BuildSchematics, Self::McSchematics]
    }

    /// Human-facing label, kept untranslated: the frontend maps it through i18n.
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Reden => "Reden",
            Self::BuildSchematics => "BuildSchematics",
            Self::McSchematics => "mc-schematics",
        }
    }
}

/// The on-disk format of the delivered file.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SchematicFormat {
    Litematic,
    Schem,
    /// Legacy MCEdit `.schematic`.
    Schematic,
    /// Vanilla structure file. Not tracked by the content scanner.
    Nbt,
    Unknown,
}

impl SchematicFormat {
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_ascii_lowercase().as_str() {
            "litematic" => Self::Litematic,
            "schem" => Self::Schem,
            "schematic" => Self::Schematic,
            "nbt" => Self::Nbt,
            _ => Self::Unknown,
        }
    }

    pub fn from_url(url: &str) -> Self {
        let path = url.split(['?', '#']).next().unwrap_or(url);
        match path.rsplit('.').next() {
            Some(ext) if !ext.contains('/') => Self::from_extension(ext),
            _ => Self::Unknown,
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Litematic => "litematic",
            Self::Schem => "schem",
            Self::Schematic => "schematic",
            Self::Nbt => "nbt",
            Self::Unknown => "",
        }
    }
}

/// What the delivered file actually is.
///
/// Both are local-only project types upstream; the distinction only drives the
/// destination folder and the wording shown to the user.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentKind {
    Schematic,
    World,
}

/// How a download has to be carried out.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Delivery {
    /// Fetch `url` directly.
    Direct,
    /// The item lives behind a netdisk share; open it externally.
    Quark,
    /// Nothing downloadable; send the user to the source page.
    External,
}

/// Everything the downloader needs to fetch one item.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadPlan {
    pub delivery: Delivery,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub referer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_name_hint: Option<String>,
    #[serde(default)]
    pub requires_login: bool,
}

impl DownloadPlan {
    pub fn direct(url: impl Into<String>) -> Self {
        Self {
            delivery: Delivery::Direct,
            url: url.into(),
            referer: None,
            file_name_hint: None,
            requires_login: false,
        }
    }

    pub fn with_referer(mut self, referer: impl Into<String>) -> Self {
        self.referer = Some(referer.into());
        self
    }

    pub fn with_file_name(mut self, name: impl Into<String>) -> Self {
        self.file_name_hint = Some(name.into());
        self
    }

    pub fn external(url: impl Into<String>) -> Self {
        Self {
            delivery: Delivery::External,
            url: url.into(),
            referer: None,
            file_name_hint: None,
            requires_login: false,
        }
    }
}

/// One browsable item.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchematicItem {
    /// Provider-qualified, e.g. `reden:abc`. Unique across sources.
    pub id: String,
    pub source_id: SourceId,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub format: SchematicFormat,
    pub content_kind: ContentKind,
    #[serde(default)]
    pub versions: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub downloads: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<i64>,
    #[serde(default)]
    pub images: Vec<String>,
    pub detail_url: String,
    pub download: DownloadPlan,
    /// True when only part of the metadata could be resolved (for example a
    /// source-specific title that has not been backfilled yet).
    #[serde(default)]
    pub degraded: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub degraded_reason: Option<String>,
}

/// How precise the reported total is.
///
/// Reden has been observed reporting `total: 100` for a query that returns far
/// more results, so a plain `bool` would lose information the UI needs.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TotalPrecision {
    /// The source reported an exact total.
    Exact,
    /// The source reported a total we do not trust.
    Estimated,
    /// The source did not report a total at all.
    Unknown,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceTotal {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<u64>,
    pub precision: TotalPrecision,
}

impl ResourceTotal {
    pub fn unknown() -> Self {
        Self {
            value: None,
            precision: TotalPrecision::Unknown,
        }
    }

    pub fn exact(value: u64) -> Self {
        Self {
            value: Some(value),
            precision: TotalPrecision::Exact,
        }
    }

    pub fn estimated(value: u64) -> Self {
        Self {
            value: Some(value),
            precision: TotalPrecision::Estimated,
        }
    }
}

/// Health of one source at the time of the call.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceStatus {
    pub source_id: SourceId,
    pub state: SourceState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// True when the payload came from the on-disk index rather than upstream.
    #[serde(default)]
    pub cached: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fetched_at: Option<i64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceState {
    Ok,
    /// Serving from cache or with partial metadata.
    Degraded,
    Offline,
    /// Not queried in this call.
    Unknown,
}

/// One source's failure inside an otherwise successful batch.
///
/// Mirrors the shape upstream uses for provider failures so the frontend can
/// reuse its rendering.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceFailure {
    pub source_id: SourceId,
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

/// What a source can do. The frontend renders from this rather than matching on
/// the source id, so adding a source does not require frontend changes.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceCapabilities {
    pub supports_search: bool,
    pub supports_version_filter: bool,
    pub supports_server_side_paging: bool,
    pub supports_thumbnail: bool,
    pub supports_quark: bool,
    /// True when the source has a generator that builds a file on demand.
    pub supports_generator: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListResult {
    pub items: Vec<SchematicItem>,
    pub page: u32,
    /// False when the source has no further pages.
    pub has_more: bool,
    pub total: ResourceTotal,
    pub source_status: SourceStatus,
    #[serde(default)]
    pub source_failures: Vec<SourceFailure>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchematicDetail {
    pub item: SchematicItem,
    #[serde(default)]
    pub images: Vec<String>,
    #[serde(default)]
    pub materials: Vec<MaterialEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub source_status: Vec<SourceStatus>,
}

/// One material requirement reported by a source.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialEntry {
    pub name: String,
    pub count: u64,
}

/// Which page of a source to read.
#[derive(Clone, Debug, Default)]
pub struct ListQuery {
    pub page: u32,
    pub page_size: u32,
    pub query: Option<String>,
    pub versions: Vec<String>,
}

/// Size arguments for a generator item.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateSize {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub z: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_from_url_handles_query_and_missing_extension() {
        assert_eq!(
            SchematicFormat::from_url("https://x/b/abc/house.schem"),
            SchematicFormat::Schem
        );
        assert_eq!(
            SchematicFormat::from_url("https://x/a/house.litematic?token=1"),
            SchematicFormat::Litematic
        );
        assert_eq!(
            SchematicFormat::from_url("https://x/a/page"),
            SchematicFormat::Unknown
        );
    }

    #[test]
    fn source_ids_round_trip() {
        for id in SourceId::all() {
            assert_eq!(SourceId::parse(id.as_str()), Some(*id));
        }
    }

    #[test]
    fn item_serialises_camel_case() {
        let item = SchematicItem {
            id: "reden:abc".into(),
            source_id: SourceId::Reden,
            title: "House".into(),
            description: None,
            format: SchematicFormat::Schem,
            content_kind: ContentKind::Schematic,
            versions: vec!["1.21".into()],
            downloads: Some(3),
            updated_at: None,
            images: vec![],
            detail_url: "https://redenmc.com/share/abc".into(),
            download: DownloadPlan::direct("https://x/a.schem"),
            degraded: false,
            degraded_reason: None,
        };
        let value = serde_json::to_value(&item).unwrap();
        assert!(value.get("sourceId").is_some(), "expected camelCase keys");
        assert!(value.get("contentKind").is_some());
        assert!(value.get("source_id").is_none());
    }
}

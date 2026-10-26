//! Reden source adapter.
//!
//! Upstream quirks that this adapter encodes (all verified against the live
//! API, not inferred from docs):
//!
//! - Paging uses `page`, never `offset`. Passing `offset` returns page 1 again.
//! - The browse endpoint **ignores `pageSize`** and returns 24 items per page.
//! - Responses are wrapped as `{"d": [...], "count": N}`, not `{"items": ...}`.
//! - `Origin` is an allowlist: sending any origin other than
//!   `https://redenmc.com` (including the API's own host) yields 403. This
//!   adapter therefore sends no `Origin` header at all.
//! - Generator items (`type == "LitematicaGen"`) build the file on demand and
//!   return it directly as an attachment; there is no netdisk hop.
//! - `ItemDto.link` is a related video link, not a fallback download.

use async_trait::async_trait;
use serde::Deserialize;
use tokio_util::sync::CancellationToken;

use super::super::error::{NetworkKind, SchematicSourceError, SourceResult};
use super::super::http::{self, RequestHeaders};
use super::super::source::SchematicBrowseSource;
use super::super::types::{
    ContentKind, DownloadPlan, GenerateSize, ListQuery, ListResult,
    ResourceTotal, SchematicDetail, SchematicFormat, SchematicItem,
    SourceCapabilities, SourceId, SourceState, SourceStatus,
};

const BASE: &str = "https://api.redenmc.com";
const BROWSE_URL: &str = "https://api.redenmc.com/api/mc-services/yisibite";
const SEARCH_URL: &str =
    "https://api.redenmc.com/api/mc-services/litematica/search";
/// The browse endpoint returns a fixed page size regardless of what we ask for.
const BROWSE_PAGE_SIZE: u32 = 24;
const SITE: &str = "https://redenmc.com/";

/// Reden wraps list payloads in `d`.
#[derive(Deserialize)]
struct Envelope<T> {
    d: Vec<T>,
    #[serde(default)]
    count: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ItemDto {
    #[serde(default)]
    key: Option<String>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default, rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    downloads: Option<u64>,
    #[serde(default)]
    versions: Option<Vec<String>>,
    #[serde(default)]
    image_url: Option<String>,
    #[serde(default)]
    images: Option<Vec<String>>,
    #[serde(default)]
    author: Option<AuthorDto>,
    #[serde(default)]
    attachments: Option<Vec<AttachmentDto>>,
    #[serde(default)]
    link: Option<String>,
    #[serde(default)]
    status: Option<String>,
}

#[derive(Deserialize)]
struct AuthorDto {
    #[serde(default)]
    username: Option<String>,
}

#[derive(Deserialize)]
struct AttachmentDto {
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    name: Option<String>,
}

impl ItemDto {
    fn key(&self) -> Option<&str> {
        self.key.as_deref().or(self.id.as_deref())
    }

    fn display_title(&self) -> String {
        self.name
            .clone()
            .or_else(|| self.title.clone())
            .or_else(|| self.key().map(str::to_string))
            .unwrap_or_else(|| "Untitled".to_string())
    }

    fn is_generator(&self) -> bool {
        matches!(self.kind.as_deref(), Some("LitematicaGen"))
    }

    fn is_approved(&self) -> bool {
        // Pending items exist in the listing and have no usable file yet.
        !matches!(self.status.as_deref(), Some("Pending"))
    }

    fn detail_url(&self) -> String {
        match self.key() {
            Some(key) => format!("{BASE}/share/{key}"),
            None => SITE.to_string(),
        }
    }

    fn to_item(&self) -> Option<SchematicItem> {
        let key = self.key()?;
        if !self.is_approved() {
            return None;
        }

        let direct = self
            .attachments
            .as_ref()
            .and_then(|list| list.iter().find_map(|a| a.url.clone()));

        let (format, download) = if self.is_generator() {
            // The generator needs size arguments; the plan is filled in by
            // `download_plan`, and until then the item is marked as needing a
            // generation call rather than a plain fetch.
            (
                SchematicFormat::Litematic,
                DownloadPlan {
                    delivery: super::super::types::Delivery::Direct,
                    url: format!("{BASE}/api/mc-services/yisibite/{key}"),
                    referer: Some(SITE.to_string()),
                    file_name_hint: None,
                    requires_login: false,
                },
            )
        } else if let Some(url) = direct {
            (
                SchematicFormat::from_url(&url),
                DownloadPlan::direct(url).with_referer(SITE),
            )
        } else {
            (
                SchematicFormat::Unknown,
                DownloadPlan::external(self.detail_url()),
            )
        };

        let mut images: Vec<String> = self.images.clone().unwrap_or_default();
        if images.is_empty()
            && let Some(image) = self.image_url.clone()
        {
            images.push(image);
        }
        images.retain(|url| !url.is_empty());

        Some(SchematicItem {
            id: format!("reden:{key}"),
            source_id: SourceId::Reden,
            title: self.display_title(),
            description: self
                .description
                .clone()
                .or_else(|| self.summary.clone()),
            format,
            content_kind: ContentKind::Schematic,
            versions: self.versions.clone().unwrap_or_default(),
            downloads: self.downloads,
            updated_at: None,
            images,
            detail_url: self.detail_url(),
            download,
            degraded: false,
            degraded_reason: None,
        })
    }
}

#[derive(Deserialize)]
struct DetailDto {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    author: Option<AuthorDto>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    images: Option<Vec<String>>,
    #[serde(default)]
    image_url: Option<String>,
    #[serde(default)]
    versions: Option<Vec<String>>,
    #[serde(default)]
    downloads: Option<u64>,
}

pub struct RedenSource;

impl RedenSource {
    fn item_from_detail(
        key: &str,
        detail: &DetailDto,
        base: &SchematicItem,
    ) -> SchematicItem {
        let mut item = base.clone();
        if let Some(name) = detail.name.clone() {
            item.title = name;
        }
        if let Some(description) = detail
            .description
            .clone()
            .or_else(|| detail.summary.clone())
        {
            item.description = Some(description);
        }
        if let Some(versions) = detail.versions.clone() {
            item.versions = versions;
        }
        if let Some(downloads) = detail.downloads {
            item.downloads = Some(downloads);
        }

        let mut images: Vec<String> = detail.images.clone().unwrap_or_default();
        if images.is_empty()
            && let Some(image) = detail.image_url.clone()
        {
            images.push(image);
        }
        images.retain(|url| !url.is_empty());
        if !images.is_empty() {
            item.images = images;
        }
        let _ = key;
        item
    }
}

#[async_trait]
impl SchematicBrowseSource for RedenSource {
    fn id(&self) -> SourceId {
        SourceId::Reden
    }

    fn capabilities(&self) -> SourceCapabilities {
        SourceCapabilities {
            supports_search: true,
            supports_version_filter: true,
            supports_server_side_paging: true,
            supports_thumbnail: true,
            // Generator items return the file directly; no netdisk hop.
            supports_quark: false,
            supports_generator: true,
        }
    }

    async fn list(
        &self,
        query: &ListQuery,
        cancel: &CancellationToken,
    ) -> SourceResult<ListResult> {
        let page = query.page.max(1);
        let url = match query.query.as_deref().map(str::trim) {
            Some(text) if !text.is_empty() => format!(
                "{SEARCH_URL}?q={}&page={page}",
                urlencoding::encode(text)
            ),
            _ => format!("{BROWSE_URL}?lang=zh_cn&page={page}"),
        };

        let response = http::get_text(&url, &RequestHeaders::default(), cancel)
            .await
            .map_err(|error| match error {
                SchematicSourceError::Network { status: Some(403), .. } => {
                    SchematicSourceError::upstream_changed(
                        "Reden rejected the request (403); the Origin allowlist may have changed",
                    )
                }
                other => other,
            })?;

        let envelope: Envelope<ItemDto> = serde_json::from_str(&response.body)
            .map_err(|error| {
                SchematicSourceError::parse("reden list", error.to_string())
            })?;

        let mut items: Vec<SchematicItem> =
            envelope.d.iter().filter_map(ItemDto::to_item).collect();

        if !query.versions.is_empty() {
            items.retain(|item| {
                matches_versions(&item.versions, &query.versions)
            });
        }

        let total = match envelope.count {
            // Reden has been observed reporting a suspiciously round total, so
            // it is reported as an estimate rather than trusted as exact.
            Some(count) => ResourceTotal::estimated(count),
            None => ResourceTotal::unknown(),
        };

        let effective_page_size = query.page_size.max(1).min(BROWSE_PAGE_SIZE);
        Ok(ListResult {
            has_more: envelope.d.len() >= effective_page_size as usize,
            items,
            page,
            total,
            source_status: SourceStatus {
                source_id: SourceId::Reden,
                state: SourceState::Ok,
                message: None,
                cached: false,
                fetched_at: None,
            },
            source_failures: Vec::new(),
        })
    }

    async fn detail(
        &self,
        id: &str,
        cancel: &CancellationToken,
    ) -> SourceResult<SchematicDetail> {
        let key = strip_prefix(id);
        let info_url =
            format!("{BASE}/api/mc-services/yisibite/{key}/info/zh_cn");
        let response = http::get_text(
            &info_url,
            &RequestHeaders::with_referer(SITE),
            cancel,
        )
        .await?;

        let detail: DetailDto =
            serde_json::from_str(&response.body).map_err(|error| {
                SchematicSourceError::parse("reden detail", error.to_string())
            })?;

        let listing_url = format!("{BROWSE_URL}?lang=zh_cn&page=1");
        let base =
            http::get_text(&listing_url, &RequestHeaders::default(), cancel)
                .await
                .ok()
                .and_then(|response| {
                    serde_json::from_str::<Envelope<ItemDto>>(&response.body)
                        .ok()
                })
                .and_then(|envelope| {
                    envelope
                        .d
                        .iter()
                        .find(|dto| dto.key() == Some(key))
                        .and_then(ItemDto::to_item)
                })
                .unwrap_or_else(|| SchematicItem {
                    id: format!("reden:{key}"),
                    source_id: SourceId::Reden,
                    title: key.to_string(),
                    description: None,
                    format: SchematicFormat::Unknown,
                    content_kind: ContentKind::Schematic,
                    versions: Vec::new(),
                    downloads: None,
                    updated_at: None,
                    images: Vec::new(),
                    detail_url: format!("{BASE}/share/{key}"),
                    download: DownloadPlan::external(format!(
                        "{BASE}/share/{key}"
                    )),
                    degraded: true,
                    degraded_reason: Some(
                        "item no longer present in the Reden listing"
                            .to_string(),
                    ),
                });

        let item = Self::item_from_detail(key, &detail, &base);
        Ok(SchematicDetail {
            images: item.images.clone(),
            description: item.description.clone(),
            materials: Vec::new(),
            source_status: vec![SourceStatus {
                source_id: SourceId::Reden,
                state: if item.degraded {
                    SourceState::Degraded
                } else {
                    SourceState::Ok
                },
                message: item.degraded_reason.clone(),
                cached: false,
                fetched_at: None,
            }],
            item,
        })
    }

    async fn download_plan(
        &self,
        id: &str,
        size: Option<GenerateSize>,
        cancel: &CancellationToken,
    ) -> SourceResult<DownloadPlan> {
        let key = strip_prefix(id);
        let detail = self.detail(id, cancel).await?;

        if detail.item.format != SchematicFormat::Unknown
            && detail.item.download.delivery
                == super::super::types::Delivery::Direct
            && detail.item.download.url.contains("/yisibite/")
            && !detail.item.download.url.contains("xSize=")
        {
            // Generator item: the size arguments decide the output.
            let size = size.unwrap_or(GenerateSize {
                x: Some(128),
                y: None,
                z: Some(128),
            });
            let mut url = format!("{BASE}/api/mc-services/yisibite/{key}");
            let mut parts: Vec<String> = Vec::new();
            if let Some(x) = size.x {
                parts.push(format!("xSize={x}"));
            }
            if let Some(y) = size.y {
                parts.push(format!("ySize={y}"));
            }
            if let Some(z) = size.z {
                parts.push(format!("zSize={z}"));
            }
            if !parts.is_empty() {
                url.push('?');
                url.push_str(&parts.join("&"));
            }
            return Ok(DownloadPlan {
                delivery: super::super::types::Delivery::Direct,
                url,
                referer: Some(SITE.to_string()),
                file_name_hint: None,
                requires_login: false,
            });
        }

        Ok(detail.item.download)
    }

    async fn health_check(&self) -> SourceStatus {
        let cancel = CancellationToken::new();
        let url = format!("{BROWSE_URL}?lang=zh_cn&page=1");
        match http::get_text(&url, &RequestHeaders::default(), &cancel).await {
            Ok(_) => SourceStatus {
                source_id: SourceId::Reden,
                state: SourceState::Ok,
                message: None,
                cached: false,
                fetched_at: None,
            },
            Err(error) => SourceStatus {
                source_id: SourceId::Reden,
                state: match error {
                    SchematicSourceError::Network {
                        kind: NetworkKind::Timeout,
                        ..
                    } => SourceState::Offline,
                    _ => SourceState::Degraded,
                },
                message: Some(error.to_string()),
                cached: false,
                fetched_at: None,
            },
        }
    }
}

fn strip_prefix(id: &str) -> &str {
    id.strip_prefix("reden:").unwrap_or(id)
}

/// Match a requested version against an item's declared versions.
///
/// Reden publishes wildcard versions such as `1.21.x`, so a request for
/// `1.21.4` has to match it. Comparison is therefore done on the leading
/// components, with `x` treated as a wildcard.
fn matches_versions(item_versions: &[String], requested: &[String]) -> bool {
    if item_versions.is_empty() {
        // No declared versions: do not hide the item.
        return true;
    }
    requested.iter().any(|wanted| {
        item_versions.iter().any(|declared| {
            version_matches(wanted, declared)
                || version_matches(declared, wanted)
        })
    })
}

/// Compare two version strings where `x` (or `X`) is a wildcard on **either**
/// side.
///
/// Reden publishes entries tagged `1.21.x` while the user filters by a concrete
/// `1.21.4`, so a one-sided wildcard is not enough: the filter has to match
/// whichever side carries the placeholder.
fn version_matches(pattern: &str, candidate: &str) -> bool {
    let pattern = pattern.trim();
    let candidate = candidate.trim();
    if pattern == candidate {
        return true;
    }
    let pattern_parts: Vec<&str> = pattern.split('.').collect();
    let candidate_parts: Vec<&str> = candidate.split('.').collect();

    let is_wildcard = |part: &str| part == "x" || part == "X";

    // A shorter concrete version is a prefix match ("1.21" matches "1.21.4"),
    // but a shorter wildcard version stops at its own length.
    let compared = pattern_parts.len().min(candidate_parts.len());
    if compared == 0 {
        return false;
    }

    for index in 0..compared {
        let left = pattern_parts[index];
        let right = candidate_parts[index];
        if is_wildcard(left) || is_wildcard(right) {
            continue;
        }
        if left != right {
            return false;
        }
    }

    // A shorter version is a prefix match: "1.21" matches "1.21.4" because the
    // user expressed no opinion about the patch level.
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_uses_the_d_field() {
        let body = r#"{"d":[{"key":"abc","name":"House","type":"LitematicaShare"}],"count":1461}"#;
        let envelope: Envelope<ItemDto> = serde_json::from_str(body).unwrap();
        assert_eq!(envelope.d.len(), 1);
        assert_eq!(envelope.count, Some(1461));
    }

    #[test]
    fn generator_items_build_the_file_on_demand() {
        let body = r#"{"d":[{"key":"gen1","name":"World Eater","type":"LitematicaGen"}]}"#;
        let envelope: Envelope<ItemDto> = serde_json::from_str(body).unwrap();
        let item = envelope.d[0].to_item().unwrap();
        assert!(item.download.url.contains("/yisibite/gen1"));
        assert!(!item.download.url.contains("xSize="), "size is added later");
    }

    #[test]
    fn pending_items_are_hidden() {
        let body =
            r#"{"d":[{"key":"p1","name":"Pending","status":"Pending"}]}"#;
        let envelope: Envelope<ItemDto> = serde_json::from_str(body).unwrap();
        assert!(envelope.d[0].to_item().is_none());
    }

    #[test]
    fn link_is_not_used_as_a_download() {
        // `link` holds a related video, not a file.
        let body = r#"{"d":[{"key":"v1","name":"With video","type":"LitematicaGen","link":"https://www.bilibili.com/video/BV1za4y1P7cw"}]}"#;
        let envelope: Envelope<ItemDto> = serde_json::from_str(body).unwrap();
        let item = envelope.d[0].to_item().unwrap();
        assert!(!item.download.url.contains("bilibili"));
    }

    #[test]
    fn wildcard_versions_match_in_both_directions() {
        assert!(version_matches("1.21.x", "1.21.4"));
        assert!(version_matches("1.21.4", "1.21.x"));
        assert!(version_matches("1.21", "1.21.4"));
        assert!(!version_matches("1.20", "1.21.4"));
    }

    #[test]
    fn version_filter_keeps_items_without_declared_versions() {
        assert!(matches_versions(&[], &["1.21".to_string()]));
        assert!(matches_versions(
            &["1.21.x".to_string()],
            &["1.21.4".to_string()]
        ));
        assert!(!matches_versions(
            &["1.19".to_string()],
            &["1.21.4".to_string()]
        ));
    }
}

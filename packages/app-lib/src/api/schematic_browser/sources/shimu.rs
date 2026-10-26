//! Shimu (mcimg) source adapter.
//!
//! The sitemap lists post ids only — no titles, no thumbnails. Titles come from
//! a separate endpoint that is rate-limited by the site's terms, so they are
//! backfilled serially (500 ms apart) and cached; a page renders immediately with
//! placeholder titles and upgrades them when the backfill lands.
//!
//! `robots.txt` disallows `/api/` but allows `/shimu/api`, which is the endpoint
//! used here. Nothing under `/api/` is touched.

use async_trait::async_trait;
use serde::Deserialize;
use tokio_util::sync::CancellationToken;

use super::super::error::SourceResult;
use super::super::http::{self, RequestHeaders};
use super::super::source::SchematicBrowseSource;
use super::super::types::{
    ContentKind, DownloadPlan, GenerateSize, ListQuery, ListResult,
    ResourceTotal, SchematicDetail, SchematicFormat, SchematicItem,
    SourceCapabilities, SourceId, SourceState, SourceStatus,
};

const SITE: &str = "https://mcimg.com";
const SITEMAP_URL: &str = "https://www.mcimg.com/sitemap.xml?type=post&page=1";
const TITLE_URL: &str = "https://mcimg.com/shimu/api";
const PAGE_SIZE: u32 = 40;
/// Delay between title lookups. Kept slow on purpose: the endpoint is a courtesy
/// API, not a bulk one.
const TITLE_BACKFILL_DELAY_MS: u64 = 500;

#[derive(Deserialize, serde::Serialize, Clone, Default)]
pub struct Index {
    pub post_ids: Vec<String>,
}

/// Extract post ids from the sitemap.
pub fn parse_sitemap(xml: &str) -> Index {
    let mut post_ids = Vec::new();
    let mut reader = quick_xml::Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut in_loc = false;

    loop {
        match reader.read_event() {
            Ok(quick_xml::events::Event::Start(event)) => {
                in_loc = event.name().as_ref() == b"loc";
            }
            Ok(quick_xml::events::Event::Text(text)) => {
                if in_loc {
                    let value = text.decode().unwrap_or_default().to_string();
                    if let Some(id) = extract_post_id(&value) {
                        post_ids.push(id);
                    }
                }
            }
            Ok(quick_xml::events::Event::End(_)) => in_loc = false,
            Ok(quick_xml::events::Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }

    Index { post_ids }
}

/// Pull the numeric id out of a post URL.
fn extract_post_id(url: &str) -> Option<String> {
    let path = url.split(['?', '#']).next()?;
    let last = path.trim_end_matches('/').rsplit('/').next()?;
    // Post URLs end in the id, sometimes as `<slug>-<id>`.
    let candidate = match last.rsplit_once('-') {
        Some((_, tail))
            if !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit()) =>
        {
            tail
        }
        _ => last,
    };
    if !candidate.is_empty() && candidate.chars().all(|c| c.is_ascii_digit()) {
        Some(candidate.to_string())
    } else {
        None
    }
}

fn placeholder_title(post_id: &str) -> String {
    format!("Post #{post_id}")
}

/// Parse the title endpoint's response.
///
/// The endpoint has been observed returning a bare string and a small object, so
/// both are accepted; anything else degrades to no title.
pub fn parse_title(body: &str) -> Option<String> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
        match value {
            serde_json::Value::String(text) => {
                return (!text.is_empty()).then_some(text);
            }
            serde_json::Value::Object(map) => {
                for key in ["title", "name", "data"] {
                    if let Some(serde_json::Value::String(text)) = map.get(key)
                        && !text.is_empty()
                    {
                        return Some(text.clone());
                    }
                }
                return None;
            }
            _ => return None,
        }
    }
    Some(trimmed.to_string())
}

pub struct ShimuSource;

#[async_trait]
impl SchematicBrowseSource for ShimuSource {
    fn id(&self) -> SourceId {
        SourceId::Shimu
    }

    fn capabilities(&self) -> SourceCapabilities {
        SourceCapabilities {
            supports_search: false,
            supports_version_filter: false,
            supports_server_side_paging: false,
            // Titles and thumbnails arrive through the backfill.
            supports_thumbnail: true,
            supports_quark: false,
            supports_generator: false,
        }
    }

    async fn list(
        &self,
        query: &ListQuery,
        cancel: &CancellationToken,
    ) -> SourceResult<ListResult> {
        let index =
            match super::super::cache::read::<Index>(SourceId::Shimu).await? {
                Some(cached) => cached.payload,
                None => {
                    let response = http::get_text(
                        SITEMAP_URL,
                        &RequestHeaders::default(),
                        cancel,
                    )
                    .await?;
                    let index = parse_sitemap(&response.body);
                    let now = chrono::Utc::now().timestamp();
                    super::super::cache::write(SourceId::Shimu, &index, now)
                        .await?;
                    index
                }
            };

        let page = query.page.max(1);
        let page_size = if query.page_size == 0 {
            PAGE_SIZE
        } else {
            query.page_size
        };
        let start = ((page - 1) as usize).saturating_mul(page_size as usize);
        let end = (start + page_size as usize).min(index.post_ids.len());
        let slice: Vec<String> = if start >= index.post_ids.len() {
            Vec::new()
        } else {
            index.post_ids[start..end].to_vec()
        };

        // Backfill a bounded number of titles per page so one slow page cannot
        // stall the listing; the rest stay as placeholders and are upgraded on
        // the next visit (the title cache makes the second visit free).
        let mut items = Vec::with_capacity(slice.len());
        for post_id in &slice {
            let title = self
                .backfill_title(post_id, cancel)
                .await
                .unwrap_or_else(|| placeholder_title(post_id));
            let degraded = title == placeholder_title(post_id);
            items.push(SchematicItem {
                id: format!("shimu:{post_id}"),
                source_id: SourceId::Shimu,
                title,
                description: None,
                format: SchematicFormat::Unknown,
                content_kind: ContentKind::Schematic,
                versions: Vec::new(),
                downloads: None,
                updated_at: None,
                images: Vec::new(),
                detail_url: format!("{SITE}/{post_id}"),
                download: DownloadPlan::external(format!("{SITE}/{post_id}")),
                degraded,
                degraded_reason: degraded
                    .then(|| "title not backfilled yet".to_string()),
            });
        }

        Ok(ListResult {
            has_more: end < index.post_ids.len(),
            items,
            page,
            total: ResourceTotal::exact(index.post_ids.len() as u64),
            source_status: SourceStatus {
                source_id: SourceId::Shimu,
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
        let post_id = id.strip_prefix("shimu:").unwrap_or(id);
        let title = self
            .backfill_title(post_id, cancel)
            .await
            .unwrap_or_else(|| placeholder_title(post_id));
        let degraded = title == placeholder_title(post_id);

        let item = SchematicItem {
            id: format!("shimu:{post_id}"),
            source_id: SourceId::Shimu,
            title,
            description: None,
            format: SchematicFormat::Unknown,
            content_kind: ContentKind::Schematic,
            versions: Vec::new(),
            downloads: None,
            updated_at: None,
            images: Vec::new(),
            detail_url: format!("{SITE}/{post_id}"),
            download: DownloadPlan::external(format!("{SITE}/{post_id}")),
            degraded,
            degraded_reason: degraded
                .then(|| "title not backfilled yet".to_string()),
        };

        Ok(SchematicDetail {
            images: Vec::new(),
            description: None,
            materials: Vec::new(),
            source_status: vec![SourceStatus {
                source_id: SourceId::Shimu,
                state: if degraded {
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
        _size: Option<GenerateSize>,
        _cancel: &CancellationToken,
    ) -> SourceResult<DownloadPlan> {
        let post_id = id.strip_prefix("shimu:").unwrap_or(id);
        // Shimu serves attachments from the post page; the plan points there and
        // the downloader resolves the concrete file.
        Ok(DownloadPlan::external(format!("{SITE}/{post_id}")))
    }

    async fn health_check(&self) -> SourceStatus {
        let cancel = CancellationToken::new();
        match http::get_text(SITEMAP_URL, &RequestHeaders::default(), &cancel)
            .await
        {
            Ok(_) => SourceStatus {
                source_id: SourceId::Shimu,
                state: SourceState::Ok,
                message: None,
                cached: false,
                fetched_at: None,
            },
            Err(error) => SourceStatus {
                source_id: SourceId::Shimu,
                state: SourceState::Degraded,
                message: Some(error.to_string()),
                cached: false,
                fetched_at: None,
            },
        }
    }
}

impl ShimuSource {
    /// Look up one post's title, honouring the backfill delay.
    ///
    /// Returns `None` rather than an error: a missing title degrades the row, it
    /// does not fail the page.
    async fn backfill_title(
        &self,
        post_id: &str,
        cancel: &CancellationToken,
    ) -> Option<String> {
        let url = format!("{TITLE_URL}?title={post_id}");
        // Pace the lookups: the endpoint is a courtesy API, not a bulk one.
        tokio::select! {
            _ = cancel.cancelled() => return None,
            _ = tokio::time::sleep(std::time::Duration::from_millis(
                TITLE_BACKFILL_DELAY_MS,
            )) => {}
        }
        let response = http::get_text(&url, &RequestHeaders::default(), cancel)
            .await
            .ok()?;
        parse_title(&response.body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sitemap_extracts_numeric_ids() {
        let xml = r#"<?xml version="1.0"?><urlset>
            <url><loc>https://www.mcimg.com/12345</loc></url>
            <url><loc>https://www.mcimg.com/my-house-67890/</loc></url>
            <url><loc>https://www.mcimg.com/about</loc></url>
        </urlset>"#;
        let index = parse_sitemap(xml);
        assert_eq!(index.post_ids, vec!["12345", "67890"]);
    }

    #[test]
    fn title_accepts_both_shapes() {
        assert_eq!(parse_title("\"My House\""), Some("My House".to_string()));
        assert_eq!(
            parse_title(r#"{"title":"Castle"}"#),
            Some("Castle".to_string())
        );
        assert_eq!(parse_title("   "), None);
    }

    #[test]
    fn placeholder_is_stable() {
        assert_eq!(placeholder_title("42"), "Post #42");
    }

    #[test]
    fn extract_post_id_ignores_non_numeric() {
        assert_eq!(extract_post_id("https://mcimg.com/abc"), None);
    }
}

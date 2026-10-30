//! BuildSchematics source adapter.
//!
//! The site publishes a single-line sitemap of about 1.3 MB listing every preset
//! page. The sitemap is parsed here, in Rust, and only the requested page slice
//! crosses the IPC boundary — shipping the raw document to the frontend would
//! cost more than the rest of the browse call combined.
//!
//! The site's terms forbid repeatedly pulling resources, so the index is cached
//! for at least 24 hours and never fetched in a loop.

use async_trait::async_trait;
use serde::Deserialize;
use tokio_util::sync::CancellationToken;

use super::super::error::{SchematicSourceError, SourceResult};
use super::super::http::{self, RequestHeaders};
use super::super::source::SchematicBrowseSource;
use super::super::types::{
    ContentKind, DownloadPlan, GenerateSize, ListQuery, ListResult,
    ResourceTotal, SchematicDetail, SchematicFormat, SchematicItem,
    SourceCapabilities, SourceId, SourceState, SourceStatus,
};

const SITE: &str = "https://buildschematics.com";
const SITEMAP_URL: &str = "https://buildschematics.com/sitemap.xml";
const PAGE_SIZE: u32 = 60;
/// How long a parsed index may be reused.
///
/// The site's terms forbid repeatedly pulling its resources, so this is a floor
/// as much as a cache lifetime.
const CACHE_TTL_SECONDS: i64 = super::super::cache::MIN_TTL_SECONDS;

#[derive(Deserialize, serde::Serialize, Clone, Default)]
pub struct Index {
    /// Slugs from `/presets/<slug>` URLs.
    pub slugs: Vec<String>,
}

/// Extract preset slugs from the sitemap.
///
/// Returns an error for malformed XML rather than the entries collected so far:
/// a truncated document would otherwise be cached as a valid, silently short
/// index.
pub fn parse_sitemap(xml: &str) -> SourceResult<Index> {
    let mut slugs = Vec::new();
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
                    if let Some(rest) = value.split("/presets/").nth(1) {
                        let slug = rest.trim_end_matches('/');
                        if !slug.is_empty() {
                            slugs.push(slug.to_string());
                        }
                    }
                }
            }
            Ok(quick_xml::events::Event::End(_)) => in_loc = false,
            Ok(quick_xml::events::Event::Eof) => break,
            Err(error) => {
                return Err(SchematicSourceError::parse(
                    "buildschematics sitemap",
                    error.to_string(),
                ));
            }
            _ => {}
        }
    }

    if slugs.is_empty() {
        // A sitemap with no preset links is not a valid index for this site; it
        // means the document shape changed or an error page came back.
        return Err(SchematicSourceError::upstream_changed(
            "the sitemap contained no /presets/ links",
        ));
    }

    Ok(Index { slugs })
}

fn title_from_slug(slug: &str) -> String {
    slug.split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => {
                    first.to_uppercase().collect::<String>() + chars.as_str()
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Build the download URL for a preset slug.
///
/// The CDN path is `/dl2storage800/<Name>.<ext>`; the extension varies between
/// `.litematic` and `.nbt`, so it is probed at download time rather than guessed
/// here.
fn cdn_url_for(slug: &str, extension: &str) -> String {
    format!("https://dl2.9mcbox.com/dl2storage800/{slug}.{extension}")
}

pub struct BuildSchematicsSource;

#[async_trait]
impl SchematicBrowseSource for BuildSchematicsSource {
    fn id(&self) -> SourceId {
        SourceId::BuildSchematics
    }

    fn capabilities(&self) -> SourceCapabilities {
        SourceCapabilities {
            // The site has no search endpoint, but this adapter filters the
            // cached index locally, so search works from the user's point of
            // view. Declaring it false would hide the control.
            supports_search: true,
            supports_version_filter: false,
            supports_server_side_paging: false,
            supports_thumbnail: false,
            supports_quark: false,
            supports_generator: false,
        }
    }

    async fn list(
        &self,
        query: &ListQuery,
        cancel: &CancellationToken,
    ) -> SourceResult<ListResult> {
        let now = chrono::Utc::now().timestamp();
        let (index, cached, fetched_at) =
            match super::super::cache::read::<Index>(SourceId::BuildSchematics)
                .await?
            {
                // A cached index still expires. Without this check new presets
                // would never appear and removed ones would never leave, because
                // the file would be reused forever.
                Some(hit) if hit.is_fresh(now, CACHE_TTL_SECONDS) => {
                    (hit.payload, true, Some(hit.fetched_at))
                }
                _ => {
                    let response = http::get_text(
                        SITEMAP_URL,
                        &RequestHeaders::default(),
                        cancel,
                    )
                    .await?;
                    let index = parse_sitemap(&response.body)?;
                    if let Err(error) = super::super::cache::write(
                        SourceId::BuildSchematics,
                        &index,
                        now,
                    )
                    .await
                    {
                        tracing::warn!(
                            "Failed to cache BuildSchematics index: {}",
                            error
                        );
                    }
                    (index, false, Some(now))
                }
            };

        let mut slugs = index.slugs;
        if let Some(text) = query.query.as_deref().map(str::trim)
            && !text.is_empty()
        {
            let needle = text.to_ascii_lowercase();
            slugs.retain(|slug| slug.to_ascii_lowercase().contains(&needle));
        }

        let total_count = slugs.len() as u64;
        let page = query.page.max(1);
        let page_size = if query.page_size == 0 {
            PAGE_SIZE
        } else {
            query.page_size
        };
        let start = ((page - 1) as usize).saturating_mul(page_size as usize);
        let end = (start + page_size as usize).min(slugs.len());
        let slice: Vec<String> = if start >= slugs.len() {
            Vec::new()
        } else {
            slugs[start..end].to_vec()
        };

        let items = slice
            .iter()
            .map(|slug| SchematicItem {
                id: format!("buildschematics:{slug}"),
                source_id: SourceId::BuildSchematics,
                title: title_from_slug(slug),
                description: None,
                // The sitemap carries no extension; resolved at download time.
                format: SchematicFormat::Unknown,
                content_kind: ContentKind::Schematic,
                versions: Vec::new(),
                downloads: None,
                updated_at: None,
                images: Vec::new(),
                detail_url: format!("{SITE}/presets/{slug}"),
                download: DownloadPlan::direct(cdn_url_for(slug, "litematic")),
                degraded: false,
                degraded_reason: None,
            })
            .collect();

        Ok(ListResult {
            has_more: end < slugs.len(),
            items,
            page,
            total: ResourceTotal::exact(total_count),
            source_status: SourceStatus {
                source_id: SourceId::BuildSchematics,
                state: SourceState::Ok,
                message: None,
                // Report where the index actually came from, so the UI can show
                // "cached" and the age of what the user is looking at.
                cached,
                fetched_at,
            },
            source_failures: Vec::new(),
        })
    }

    async fn detail(
        &self,
        id: &str,
        cancel: &CancellationToken,
    ) -> SourceResult<SchematicDetail> {
        let slug = id.strip_prefix("buildschematics:").unwrap_or(id);
        let detail_url = format!("{SITE}/presets/{slug}");

        let item = SchematicItem {
            id: format!("buildschematics:{slug}"),
            source_id: SourceId::BuildSchematics,
            title: title_from_slug(slug),
            description: None,
            format: SchematicFormat::Unknown,
            content_kind: ContentKind::Schematic,
            versions: Vec::new(),
            downloads: None,
            updated_at: None,
            images: Vec::new(),
            detail_url: detail_url.clone(),
            download: DownloadPlan::direct(cdn_url_for(slug, "litematic")),
            degraded: false,
            degraded_reason: None,
        };
        let _ = cancel;

        Ok(SchematicDetail {
            images: Vec::new(),
            description: None,
            materials: Vec::new(),
            source_status: vec![SourceStatus {
                source_id: SourceId::BuildSchematics,
                state: SourceState::Ok,
                message: None,
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
        let slug = id.strip_prefix("buildschematics:").unwrap_or(id);
        // Most presets are `.litematic`; a few are `.nbt`. The downloader falls
        // back to the other extension when the first probe 404s.
        Ok(DownloadPlan::direct(cdn_url_for(slug, "litematic"))
            .with_file_name(format!("{slug}.litematic")))
    }

    async fn health_check(&self) -> SourceStatus {
        let cancel = CancellationToken::new();
        match http::get_text(SITEMAP_URL, &RequestHeaders::default(), &cancel)
            .await
        {
            Ok(_) => SourceStatus {
                source_id: SourceId::BuildSchematics,
                state: SourceState::Ok,
                message: None,
                cached: false,
                fetched_at: None,
            },
            Err(error) => SourceStatus {
                source_id: SourceId::BuildSchematics,
                state: SourceState::Degraded,
                message: Some(error.to_string()),
                cached: false,
                fetched_at: None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sitemap_extracts_preset_slugs() {
        let xml = r#"<?xml version="1.0"?><urlset>
            <url><loc>https://buildschematics.com/</loc></url>
            <url><loc>https://buildschematics.com/presets/medieval-house</loc></url>
            <url><loc>https://buildschematics.com/presets/modern-villa/</loc></url>
        </urlset>"#;
        let index = parse_sitemap(xml).expect("valid sitemap");
        assert_eq!(index.slugs, vec!["medieval-house", "modern-villa"]);
    }

    #[test]
    fn cdn_url_uses_the_storage_bucket() {
        assert_eq!(
            cdn_url_for("castle", "litematic"),
            "https://dl2.9mcbox.com/dl2storage800/castle.litematic"
        );
    }

    #[test]
    fn titles_are_humanised() {
        assert_eq!(title_from_slug("medieval_house_2"), "Medieval House 2");
    }
}

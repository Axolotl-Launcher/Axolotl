//! mc-schematics.com source adapter.
//!
//! The friendliest of the four: `robots.txt` allows crawling, the sitemap is a
//! standard `<urlset>`, item pages embed JSON-LD `CreativeWork` metadata, and
//! files are served from a public R2 bucket with no authentication at all.
//!
//! Files come in two formats (sampled 40 items: 31 `.schem`, 8 `.litematic`), so
//! the download link is resolved from the page rather than assumed.

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

const SITE: &str = "https://mc-schematics.com";
const SITEMAP_URL: &str = "https://mc-schematics.com/sitemap.xml";
const BROWSE_PAGE_SIZE: u32 = 21;
/// How long a parsed sitemap index may be reused before it is refetched.
const CACHE_TTL_SECONDS: i64 = super::super::cache::MIN_TTL_SECONDS;

/// One `<loc>` entry from the sitemap.
#[derive(Deserialize, Clone)]
pub struct SitemapEntry {
    pub loc: String,
}

/// Index built from the sitemap: item URLs only.
#[derive(Deserialize, serde::Serialize, Clone, Default)]
pub struct Index {
    pub items: Vec<String>,
}

/// Extract item URLs (`/s/<slug>-<hash>`) from a sitemap document.
///
/// A malformed document is an error rather than a partial index: caching a
/// truncated result would silently hide most of the catalogue.
pub fn parse_sitemap(xml: &str) -> SourceResult<Index> {
    let mut items = Vec::new();
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
                    if let Some(path) = value.split('/').nth(3)
                        && path == "s"
                    {
                        items.push(value);
                    }
                }
            }
            Ok(quick_xml::events::Event::End(_)) => in_loc = false,
            Ok(quick_xml::events::Event::Eof) => break,
            Err(error) => {
                return Err(SchematicSourceError::parse(
                    "mc-schematics sitemap",
                    error.to_string(),
                ));
            }
            _ => {}
        }
    }

    if items.is_empty() {
        return Err(SchematicSourceError::upstream_changed(
            "the sitemap contained no /s/ links",
        ));
    }

    Ok(Index { items })
}

/// JSON-LD block embedded in an item page.
#[derive(Deserialize, Default)]
struct JsonLd {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    image: Option<ImageField>,
    #[serde(default)]
    author: Option<AuthorField>,
}

#[derive(Deserialize, Clone)]
#[serde(untagged)]
enum ImageField {
    One(String),
    Many(Vec<String>),
}

impl ImageField {
    fn into_vec(self) -> Vec<String> {
        match self {
            Self::One(value) => vec![value],
            Self::Many(values) => values,
        }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum AuthorField {
    One(Person),
    Many(Vec<Person>),
}

#[derive(Deserialize)]
struct Person {
    #[serde(default)]
    name: Option<String>,
}

impl AuthorField {
    fn first_name(self) -> Option<String> {
        match self {
            Self::One(person) => person.name,
            Self::Many(people) => people.into_iter().find_map(|p| p.name),
        }
    }
}

/// Pull the JSON-LD object out of an item page.
pub fn parse_json_ld(html: &str) -> Option<JsonLd> {
    let marker = "application/ld+json";
    let start = html.find(marker)?;
    let after = &html[start + marker.len()..];
    let open = after.find('>')? + 1;
    let body = &after[open..];
    let close = body.find("</script>")?;
    let json = body[..close].trim();
    serde_json::from_str(json).ok()
}

/// Find the direct file link on an item page.
///
/// The link sits on `files.mc-schematics.com` and carries the real extension, so
/// both `.schem` and `.litematic` items resolve through the same path.
pub fn parse_file_link(html: &str) -> Option<String> {
    let needle = "https://files.mc-schematics.com/";
    let start = html.find(needle)?;
    let rest = &html[start..];
    let end = rest
        .find(|c: char| c == '"' || c == '\'' || c == '<' || c == ' ')
        .unwrap_or(rest.len());
    let url = &rest[..end];
    let lower = url.to_ascii_lowercase();
    if lower.ends_with(".schem")
        || lower.ends_with(".litematic")
        || lower.ends_with(".schematic")
    {
        Some(url.to_string())
    } else {
        None
    }
}

pub struct McSchematicsSource;

fn item_url(slug_path: &str) -> String {
    if slug_path.starts_with("http") {
        slug_path.to_string()
    } else {
        format!("{SITE}{slug_path}")
    }
}

fn slug_of(url: &str) -> String {
    url.trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(url)
        .to_string()
}

fn title_from_slug(slug: &str) -> String {
    // Slugs look like `modern-apartment-...-97b216b528`; drop the trailing hash
    // and turn dashes into spaces.
    let without_hash = match slug.rsplit_once('-') {
        Some((head, tail))
            if tail.len() == 10
                && tail.chars().all(|c| c.is_ascii_hexdigit()) =>
        {
            head
        }
        _ => slug,
    };
    without_hash
        .split('-')
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

#[async_trait]
impl SchematicBrowseSource for McSchematicsSource {
    fn id(&self) -> SourceId {
        SourceId::McSchematics
    }

    fn capabilities(&self) -> SourceCapabilities {
        SourceCapabilities {
            // Filtering happens on the cached sitemap index, so search works even
            // though the site exposes no search endpoint.
            supports_search: true,
            supports_version_filter: false,
            supports_server_side_paging: false,
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
        let now = chrono::Utc::now().timestamp();
        let (index, cached, fetched_at) =
            match super::super::cache::read::<Index>(SourceId::McSchematics)
                .await?
            {
                // The index still expires: without this the sitemap would be
                // reused forever and new items would never show up.
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
                    super::super::cache::write(
                        SourceId::McSchematics,
                        &index,
                        now,
                    )
                    .await?;
                    (index, false, Some(now))
                }
            };

        let mut urls = index.items;
        if let Some(text) = query.query.as_deref().map(str::trim)
            && !text.is_empty()
        {
            let needle = text.to_ascii_lowercase();
            urls.retain(|url| url.to_ascii_lowercase().contains(&needle));
        }

        let total_count = urls.len() as u64;
        let page = query.page.max(1);
        let page_size = if query.page_size == 0 {
            BROWSE_PAGE_SIZE
        } else {
            query.page_size
        };
        let start = ((page - 1) as usize).saturating_mul(page_size as usize);
        let end = (start + page_size as usize).min(urls.len());
        let slice: Vec<String> = if start >= urls.len() {
            Vec::new()
        } else {
            urls[start..end].to_vec()
        };

        let mut items = Vec::with_capacity(slice.len());
        for url in &slice {
            let slug = slug_of(url);
            items.push(SchematicItem {
                id: format!("mc_schematics:{slug}"),
                source_id: SourceId::McSchematics,
                title: title_from_slug(&slug),
                description: None,
                // Resolved when the detail page is read; the slug carries no
                // extension.
                format: SchematicFormat::Unknown,
                content_kind: ContentKind::Schematic,
                versions: Vec::new(),
                downloads: None,
                updated_at: None,
                images: Vec::new(),
                detail_url: item_url(url),
                download: DownloadPlan::external(item_url(url)),
                degraded: true,
                degraded_reason: Some(
                    "metadata loads with the detail page".to_string(),
                ),
            });
        }

        Ok(ListResult {
            has_more: end < urls.len(),
            items,
            page,
            total: ResourceTotal::exact(total_count),
            source_status: SourceStatus {
                source_id: SourceId::McSchematics,
                state: SourceState::Ok,
                message: None,
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
        let slug = id.strip_prefix("mc_schematics:").unwrap_or(id);
        let url = format!("{SITE}/s/{slug}");
        let response =
            http::get_text(&url, &RequestHeaders::default(), cancel).await?;

        let json_ld = parse_json_ld(&response.body);
        let file_link = parse_file_link(&response.body);

        let title = json_ld
            .as_ref()
            .and_then(|ld| ld.name.clone())
            .unwrap_or_else(|| title_from_slug(slug));
        let description =
            json_ld.as_ref().and_then(|ld| ld.description.clone());
        let images = json_ld
            .as_ref()
            .and_then(|ld| ld.image.as_ref())
            .map(|image| image.clone().into_vec())
            .unwrap_or_default();

        let (format, download, degraded, degraded_reason) = match file_link {
            Some(link) => (
                SchematicFormat::from_url(&link),
                DownloadPlan::direct(link),
                false,
                None,
            ),
            None => (
                SchematicFormat::Unknown,
                DownloadPlan::external(url.clone()),
                true,
                Some("no direct file link on the item page".to_string()),
            ),
        };

        let item = SchematicItem {
            id: format!("mc_schematics:{slug}"),
            source_id: SourceId::McSchematics,
            title,
            description: description.clone(),
            format,
            content_kind: ContentKind::Schematic,
            versions: Vec::new(),
            downloads: None,
            updated_at: None,
            images: images.clone(),
            detail_url: url,
            download,
            degraded,
            degraded_reason: degraded_reason.clone(),
        };

        Ok(SchematicDetail {
            images,
            description,
            materials: Vec::new(),
            source_status: vec![SourceStatus {
                source_id: SourceId::McSchematics,
                state: if degraded {
                    SourceState::Degraded
                } else {
                    SourceState::Ok
                },
                message: degraded_reason,
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
        cancel: &CancellationToken,
    ) -> SourceResult<DownloadPlan> {
        let detail = self.detail(id, cancel).await?;
        Ok(detail.item.download)
    }

    async fn health_check(&self) -> SourceStatus {
        let cancel = CancellationToken::new();
        match http::get_text(SITEMAP_URL, &RequestHeaders::default(), &cancel)
            .await
        {
            Ok(_) => SourceStatus {
                source_id: SourceId::McSchematics,
                state: SourceState::Ok,
                message: None,
                cached: false,
                fetched_at: None,
            },
            Err(error) => SourceStatus {
                source_id: SourceId::McSchematics,
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
    fn sitemap_keeps_only_item_pages() {
        let xml = r#"<?xml version="1.0"?><urlset>
            <url><loc>https://mc-schematics.com/</loc></url>
            <url><loc>https://mc-schematics.com/browse</loc></url>
            <url><loc>https://mc-schematics.com/s/house-97b216b528</loc></url>
            <url><loc>https://mc-schematics.com/c/houses</loc></url>
        </urlset>"#;
        let index = parse_sitemap(xml).expect("valid sitemap");
        assert_eq!(index.items.len(), 1);
        assert!(index.items[0].ends_with("/s/house-97b216b528"));
    }

    #[test]
    fn json_ld_supplies_metadata() {
        let html = r#"<html><script type="application/ld+json">{"@type":"CreativeWork","name":"Modern Apartment","description":"A clean build","image":"https://files.mc-schematics.com/x.jpg"}</script></html>"#;
        let ld = parse_json_ld(html).unwrap();
        assert_eq!(ld.name.as_deref(), Some("Modern Apartment"));
        assert_eq!(ld.image.map(ImageField::into_vec).unwrap().len(), 1);
    }

    #[test]
    fn file_link_accepts_both_formats() {
        let schem = r#"<a href="https://files.mc-schematics.com/b/97b216b528/house.schem">dl</a>"#;
        assert!(parse_file_link(schem).unwrap().ends_with(".schem"));

        let litematic = r#"<a href="https://files.mc-schematics.com/b/abc/house.litematic">dl</a>"#;
        assert!(parse_file_link(litematic).unwrap().ends_with(".litematic"));
    }

    #[test]
    fn file_link_ignores_non_archive_urls() {
        let html = r#"<img src="https://files.mc-schematics.com/x.jpg">"#;
        assert!(parse_file_link(html).is_none());
    }

    #[test]
    fn slug_titles_drop_the_hash() {
        assert_eq!(
            title_from_slug("modern-apartment-clean-design-97b216b528"),
            "Modern Apartment Clean Design"
        );
    }
}

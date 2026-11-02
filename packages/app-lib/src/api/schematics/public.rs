//! Public source adapters. Reference: Axolotl PR #1016's acquisition protocols.

mod covers;

use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use dom_query::Document;
use futures::{StreamExt, stream};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;

use super::{Detail, File, Page, Project, Source, failure};

const MAX_METADATA: usize = 16 * 1024 * 1024;
const INDEX_TTL: u64 = 24 * 60 * 60;
static INDEXES: LazyLock<tokio::sync::Mutex<HashMap<Source, Index>>> =
    LazyLock::new(Default::default);

#[derive(Clone, Deserialize, Serialize)]
struct Index {
    fetched_at: u64,
    ids: Vec<String>,
}

pub(super) struct Resolved {
    pub detail: Detail,
    downloads: Vec<(String, Url)>,
}

fn site(source: Source) -> &'static str {
    match source {
        Source::Reden => "https://redenmc.com",
        Source::BuildSchematics => "https://buildschematics.com",
        Source::Shimu => "https://www.mcimg.com",
    }
}

fn allowed(source: Source, url: &Url) -> bool {
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port_or_known_default() == Some(443)
        && match source {
            Source::Reden => matches!(
                url.host_str(),
                Some("api.redenmc.com" | "redenmc.com" | "static.redenmc.com")
            ),
            Source::BuildSchematics => matches!(
                url.host_str(),
                Some(
                    "buildschematics.com"
                        | "dl2.buildschematics.com"
                        | "dl2.9mcbox.com"
                )
            ),
            Source::Shimu => false,
        }
}

fn safe_url(source: Source, raw: &str) -> Option<Url> {
    let url = Url::parse(site(source)).ok()?.join(raw).ok()?;
    allowed(source, &url).then_some(url)
}

async fn fetch(
    source: Source,
    url: Url,
    limit: usize,
) -> crate::Result<Vec<u8>> {
    if !allowed(source, &url) {
        return Err(failure("Unsupported schematic resource host"));
    }
    let client = crate::util::fetch::reqwest_client_builder()
        .redirect(reqwest::redirect::Policy::custom(move |attempt| {
            if attempt.previous().len() < 5 && allowed(source, attempt.url()) {
                attempt.follow()
            } else {
                attempt.error("Unsupported schematic redirect")
            }
        }))
        .build()?;
    let mut response = client
        .get(url)
        .header(reqwest::header::REFERER, site(source))
        .timeout(Duration::from_secs(if limit > MAX_METADATA {
            300
        } else {
            30
        }))
        .send()
        .await?
        .error_for_status()?;
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        return Err(failure("The schematic resource is too large"));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if bytes.len() + chunk.len() > limit {
            return Err(failure("The schematic resource is too large"));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

async fn text(source: Source, url: Url) -> crate::Result<String> {
    String::from_utf8(fetch(source, url, MAX_METADATA).await?)
        .map_err(|_| failure("Invalid schematic source response"))
}

fn endpoint(
    source: Source,
    prefix: &str,
    id: &str,
    suffix: &str,
) -> crate::Result<Url> {
    if id.is_empty()
        || id.len() > 300
        || matches!(id, "." | "..")
        || id.chars().any(|c| c.is_control() || "/\\".contains(c))
    {
        return Err(failure("Invalid schematic identity"));
    }
    let base = if source == Source::Reden && prefix.starts_with("/api/") {
        "https://api.redenmc.com"
    } else {
        site(source)
    };
    let mut url = Url::parse(base)?.join(prefix)?;
    url.path_segments_mut()
        .map_err(|_| failure("Invalid schematic path"))?
        .pop_if_empty()
        .push(id);
    if !suffix.is_empty() {
        for segment in suffix.split('/') {
            url.path_segments_mut()
                .map_err(|_| failure("Invalid schematic path"))?
                .push(segment);
        }
    }
    Ok(url)
}

fn file_name(name: &str) -> Option<String> {
    let name = crate::util::io::sanitize_download_file_name(name).ok()?;
    [".litematic", ".schem", ".schematic", ".nbt"]
        .iter()
        .any(|extension| name.to_ascii_lowercase().ends_with(extension))
        .then_some(name)
}

fn title(id: &str) -> String {
    id.replace(['-', '_'], " ")
}

fn project_prefix(source: Source) -> &'static str {
    match source {
        Source::Reden => "/zh_cn/litematica/",
        Source::BuildSchematics => "/presets/",
        Source::Shimu => "/post/",
    }
}

fn base_project(source: Source, id: &str) -> crate::Result<Project> {
    let source_url =
        endpoint(source, project_prefix(source), id, "")?.to_string();
    Ok(Project {
        source,
        id: id.to_string(),
        title: title(id),
        summary: String::new(),
        author: String::new(),
        cover: None,
        game_versions: vec![],
        files: vec![],
        files_resolved: false,
        source_url,
    })
}

fn string(value: &Value, field: &str) -> String {
    value[field].as_str().unwrap_or_default().to_string()
}

fn reden_project(value: &Value) -> crate::Result<Project> {
    let id = value["key"]
        .as_str()
        .or_else(|| value["id"].as_str())
        .ok_or_else(|| failure("Reden returned an invalid schematic"))?;
    let mut project = base_project(Source::Reden, id)?;
    project.title = value["name"]
        .as_str()
        .or_else(|| value["title"].as_str())
        .unwrap_or(id)
        .to_string();
    project.summary = string(value, "summary");
    project.author = string(&value["author"], "username");
    project.cover = value["imageUrl"]
        .as_str()
        .and_then(|raw| safe_url(Source::Reden, raw))
        .map(Into::into);
    project.game_versions = value["versions"]
        .as_array()
        .map(|versions| {
            versions
                .iter()
                .filter_map(|version| version.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    Ok(project)
}

#[derive(Deserialize)]
struct RedenPage {
    d: Vec<Value>,
    count: Option<u64>,
}

async fn reden_page(page: u32, query: &str) -> crate::Result<RedenPage> {
    let mut url = Url::parse(if query.trim().is_empty() {
        "https://api.redenmc.com/api/mc-services/yisibite"
    } else {
        "https://api.redenmc.com/api/mc-services/litematica/search"
    })?;
    url.query_pairs_mut()
        .append_pair("page", &page.to_string())
        .append_pair("lang", "zh_cn");
    if !query.is_empty() {
        url.query_pairs_mut().append_pair("q", query);
    }
    serde_json::from_str(&text(Source::Reden, url).await?)
        .map_err(|_| failure("Invalid Reden list"))
}

fn xml_urls(xml: &str) -> crate::Result<Vec<Url>> {
    let mut reader = quick_xml::Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut in_loc = false;
    let mut urls = Vec::new();
    loop {
        match reader.read_event() {
            Ok(quick_xml::events::Event::Start(event)) => {
                in_loc = event.name().as_ref() == b"loc"
            }
            Ok(quick_xml::events::Event::Text(value)) if in_loc => {
                let value = value
                    .decode()
                    .map_err(|_| failure("Invalid schematic sitemap"))?;
                if let Ok(url) = Url::parse(&value) {
                    urls.push(url);
                }
            }
            Ok(quick_xml::events::Event::End(_)) => in_loc = false,
            Ok(quick_xml::events::Event::Eof) => break,
            Err(_) => return Err(failure("Invalid schematic sitemap")),
            _ => {}
        }
    }
    Ok(urls)
}

async fn index(source: Source) -> crate::Result<Index> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let mut indexes = INDEXES.lock().await;
    if let Some(index) = indexes
        .get(&source)
        .filter(|index| now.saturating_sub(index.fetched_at) < INDEX_TTL)
    {
        return Ok(index.clone());
    }
    let directory = crate::State::get()
        .await?
        .directories
        .caches_dir()
        .join("schematic-indexes");
    let path = directory.join(format!("{}.json", source.id()));
    if tokio::fs::metadata(&path)
        .await
        .is_ok_and(|metadata| metadata.len() <= MAX_METADATA as u64)
        && let Ok(bytes) = tokio::fs::read(&path).await
        && let Ok(index) = serde_json::from_slice::<Index>(&bytes)
        && index.fetched_at <= now
        && now - index.fetched_at < INDEX_TTL
    {
        indexes.insert(source, index.clone());
        return Ok(index);
    }
    let mut urls = xml_urls(
        &text(source, Url::parse(site(source))?.join("/sitemap.xml")?).await?,
    )?;
    if source == Source::BuildSchematics
        && let Some(child) = urls.iter().find(|url| {
            url.path() == "/sitemaps/schematic-builds" && allowed(source, url)
        })
    {
        urls = xml_urls(&text(source, child.clone()).await?)?;
    }
    let prefix = project_prefix(source);
    let mut ids = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for url in urls {
        if allowed(source, &url)
            && let Some(id) = url
                .path()
                .strip_prefix(prefix)
                .map(|id| id.trim_end_matches('/'))
            && !id.is_empty()
            && !id.contains('/')
            && seen.insert(id.to_owned())
        {
            ids.push(id.to_owned());
        }
    }
    if ids.is_empty() {
        return Err(failure("The source sitemap contains no schematics"));
    }
    let index = Index {
        fetched_at: now,
        ids,
    };
    if let Ok(bytes) = serde_json::to_vec(&index) {
        let save: crate::Result<()> = async {
            tokio::fs::create_dir_all(&directory).await?;
            let temporary = tempfile::NamedTempFile::new_in(&directory)?;
            tokio::fs::write(temporary.path(), bytes).await?;
            temporary
                .persist(&path)
                .map_err(|error| crate::Error::from(error.error))?;
            Ok(())
        }
        .await;
        if let Err(error) = save {
            tracing::warn!("Could not cache schematic sitemap: {error}");
        }
    }
    indexes.insert(source, index.clone());
    Ok(index)
}

pub(super) async fn list(
    source: Source,
    page: u32,
    query: &str,
    size: u32,
    sort: &str,
) -> crate::Result<Page> {
    let start = (page as usize - 1) * size as usize;
    if source == Source::Reden {
        if sort != "default" {
            return Err(failure("Unsupported Reden sort"));
        }
        // Reden's API always returns 24 entries; its pageSize parameter is ignored.
        let first = start / 24 + 1;
        let reply = reden_page(first as u32, query).await?;
        let total = reply
            .count
            .ok_or_else(|| failure("Reden omitted its resource count"))?
            as usize;
        let end = (start + size as usize).min(total);
        let mut rows = reply.d;
        let mut remaining = stream::iter(first + 1..=end.div_ceil(24))
            .map(|page| reden_page(page as u32, query))
            .buffered(3);
        while let Some(reply) = remaining.next().await {
            rows.extend(reply?.d);
        }
        let items = rows
            .iter()
            .skip(start % 24)
            .take(size as usize)
            .map(reden_project)
            .collect::<crate::Result<Vec<_>>>()?;
        let items = stream::iter(items)
            .map(|mut project| async move {
                if let Some(url) = project.cover.take() {
                    project.cover = covers::load(&url).await;
                }
                project
            })
            .buffered(6)
            .collect()
            .await;
        return Ok(Page {
            page_count: total.div_ceil(size as usize).max(1) as u32,
            total: total as u64,
            failed_sources: vec![],
            items,
        });
    }
    if !matches!(sort, "default" | "name") {
        return Err(failure("Unsupported schematic source sort"));
    }
    let mut ids = index(source).await?.ids;
    let needle = query.trim().to_lowercase();
    if !needle.is_empty() {
        ids.retain(|id| title(id).to_lowercase().contains(&needle));
    }
    if sort == "name" {
        ids.sort_by_cached_key(|id| title(id).to_lowercase());
    }
    let page_count = ids.len().div_ceil(size as usize).max(1) as u32;
    let items = ids
        .iter()
        .skip(start)
        .take(size as usize)
        .map(|id| base_project(source, id))
        .collect::<crate::Result<Vec<_>>>()?;
    Ok(Page {
        total: ids.len() as u64,
        failed_sources: vec![],
        items,
        page_count,
    })
}

fn meta(document: &Document, selector: &str) -> Option<String> {
    document
        .select(selector)
        .iter()
        .find_map(|node| node.attr("content").map(|value| value.to_string()))
}

fn creative_work(value: &Value) -> Option<&Value> {
    if value["@type"] == "CreativeWork" {
        return Some(value);
    }
    value
        .as_array()
        .or_else(|| value["@graph"].as_array())
        .and_then(|values| values.iter().find_map(creative_work))
}

fn generator_dimensions(
    value: &Value,
) -> crate::Result<Vec<(&'static str, u32)>> {
    let mut dimensions = Vec::new();
    for (axis, enabled) in [("x", "hasX"), ("y", "hasY"), ("z", "hasZ")] {
        if value[enabled].as_bool() != Some(true) {
            continue;
        }
        let mut minimum = 1;
        let mut maximum = i32::MAX as u32;
        let mut moduli = Vec::new();
        for condition in
            value["conditions"][axis].as_array().into_iter().flatten()
        {
            let condition = condition
                .as_str()
                .ok_or_else(|| failure("Invalid Reden generator condition"))?;
            let (kind, arguments) = condition
                .trim()
                .split_once('(')
                .ok_or_else(|| failure("Invalid Reden generator condition"))?;
            let arguments = arguments
                .strip_suffix(')')
                .ok_or_else(|| failure("Invalid Reden generator condition"))?;
            let arguments = arguments
                .split(',')
                .map(|argument| argument.trim().parse::<u32>())
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| failure("Invalid Reden generator condition"))?;
            match (kind.trim(), arguments.as_slice()) {
                ("min", [bound]) => minimum = minimum.max(*bound),
                ("max", [bound]) => maximum = maximum.min(*bound),
                ("mod", [modulus, remainder])
                    if *modulus > 0 && remainder < modulus =>
                {
                    moduli.push((*modulus, *remainder))
                }
                _ => {
                    return Err(failure(
                        "Unsupported Reden generator condition",
                    ));
                }
            }
        }
        if minimum > maximum {
            return Err(failure(
                "Reden generator dimensions have no valid range",
            ));
        }
        let preferred = 128u32.clamp(minimum, maximum);
        // Bound the search rather than trusting a provider-supplied range to
        // keep native work finite. The returned value satisfies every rule.
        let start = preferred.saturating_sub(65_536).max(minimum);
        let end = preferred.saturating_add(65_536).min(maximum);
        let size = (start..=end)
            .filter(|size| moduli.iter().all(|(modulus, remainder)| size % modulus == *remainder))
            .min_by_key(|size| (size.abs_diff(preferred), *size))
            .ok_or_else(|| failure("Cannot select valid Reden generator dimensions. Open the source website"))?;
        dimensions.push((axis, size));
    }
    Ok(dimensions)
}

pub(super) async fn detail(
    source: Source,
    id: &str,
) -> crate::Result<Resolved> {
    let mut project = base_project(source, id)?;
    let mut downloads = Vec::new();
    let description;
    let description_format;
    if source == Source::Reden {
        let body = text(
            source,
            endpoint(source, "/api/mc-services/yisibite/", id, "info/zh_cn")?,
        )
        .await?;
        let value: Value = serde_json::from_str(&body)
            .map_err(|_| failure("Invalid Reden details"))?;
        let value = value["d"]
            .as_array()
            .and_then(|items| items.first())
            .unwrap_or(&value);
        project = reden_project(value)?;
        description = value["description"]
            .as_str()
            .or_else(|| value["summary"].as_str())
            .unwrap_or_default()
            .to_string();
        description_format = "markdown";
        // List payloads currently report Pending even for approved files. Only
        // the authoritative detail response decides whether acquisition is ready.
        if value["status"].as_str() == Some("Approved") {
            if value["type"] == "LitematicaGen" {
                let dimensions = generator_dimensions(value)?;
                let suffix = dimensions
                    .iter()
                    .map(|(axis, size)| format!("{axis}{size}"))
                    .collect::<Vec<_>>()
                    .join("-");
                let name = if suffix.is_empty() {
                    format!("{id}.litematic")
                } else {
                    format!("{id}-{suffix}.litematic")
                };
                let mut url =
                    endpoint(source, "/api/mc-services/yisibite/", id, "")?;
                for (axis, size) in dimensions {
                    url.query_pairs_mut()
                        .append_pair(&format!("{axis}Size"), &size.to_string());
                }
                downloads.push((name, url));
            } else if let Some(attachments) = value["attachments"].as_array() {
                for attachment in attachments {
                    if let Some(name) =
                        attachment["name"].as_str().and_then(file_name)
                        && let Some(url) = attachment["url"]
                            .as_str()
                            .and_then(|url| safe_url(source, url))
                    {
                        downloads.push((name, url));
                    }
                }
            }
        }
    } else {
        let html = text(source, Url::parse(&project.source_url)?).await?;
        let document = Document::from(html.as_str());
        let ld = document
            .select("script[type='application/ld+json']")
            .iter()
            .find_map(|node| {
                let value: Value = serde_json::from_str(&node.text()).ok()?;
                creative_work(&value).cloned()
            });
        project.title = ld
            .as_ref()
            .and_then(|value| value["name"].as_str().map(str::to_string))
            .or_else(|| meta(&document, "meta[property='og:title']"))
            .unwrap_or(project.title);
        description = ld
            .as_ref()
            .and_then(|value| value["description"].as_str().map(str::to_string))
            .or_else(|| meta(&document, "meta[name='description']"))
            .unwrap_or_default();
        project.summary = description.clone();
        description_format = "plain";
        project.author = ld
            .as_ref()
            .map(|value| {
                value["author"]
                    .as_str()
                    .or_else(|| value["author"]["name"].as_str())
                    .unwrap_or_default()
                    .to_string()
            })
            .unwrap_or_default();
        let image = ld
            .as_ref()
            .and_then(|value| value["image"].as_str().map(str::to_string))
            .or_else(|| meta(&document, "meta[property='og:image']"));
        project.cover = image
            .and_then(|image| safe_url(source, &image))
            .map(Into::into);
        let free = ld
            .as_ref()
            .is_none_or(|value| value["isAccessibleForFree"] != false);
        if free {
            for node in document.select("a[href]").iter() {
                if let Some(url) =
                    node.attr("href").and_then(|href| safe_url(source, &href))
                {
                    let candidate = url
                        .query_pairs()
                        .find(|(key, _)| key == "file")
                        .map(|(_, value)| value.into_owned())
                        .or_else(|| {
                            url.path_segments()
                                .and_then(|mut path| path.next_back())
                                .map(str::to_string)
                        });
                    if let Some(name) =
                        candidate.and_then(|name| file_name(&name))
                        && !downloads
                            .iter()
                            .any(|(existing, _)| existing == &name)
                    {
                        downloads.push((name, url));
                    }
                }
            }
        }
    }
    if source == Source::Reden
        && let Some(url) = project.cover.take()
    {
        project.cover = covers::load(&url).await;
    }
    project.files = downloads
        .iter()
        .map(|(name, _)| File {
            name: name.clone(),
            size: None,
        })
        .collect();
    project.files_resolved = true;
    Ok(Resolved {
        detail: Detail {
            project,
            description,
            description_format,
        },
        downloads,
    })
}

pub(super) async fn download(
    source: Source,
    id: &str,
    name: &str,
) -> crate::Result<(String, Vec<u8>)> {
    let resolved = detail(source, id).await?;
    let (name, url) = resolved
        .downloads
        .into_iter()
        .find(|(candidate, _)| name.is_empty() || candidate == name)
        .ok_or_else(|| {
            failure(
                "This schematic has no direct download. Open its source page",
            )
        })?;
    let bytes = fetch(source, url, super::MAX_FILE_SIZE).await?;
    Ok((name, bytes))
}

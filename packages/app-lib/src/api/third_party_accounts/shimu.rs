//! Authenticated, free schematic discovery. Resource URLs supplied by posts are
//! never followed: downloads always use Shimu's resource endpoint.

use std::collections::{HashMap, HashSet, VecDeque};

use std::sync::Arc;
use std::time::Instant;

use dom_query::Document;
use futures::{StreamExt, stream};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::*;

use crate::api::schematics::MAX_FILE_SIZE;
const CATALOG_PAGE_SIZE: u32 = 100;
const CATALOG_TTL: Duration = Duration::from_secs(120);
static CATALOGS: LazyLock<tokio::sync::Mutex<VecDeque<CachedCatalog>>> =
    LazyLock::new(|| tokio::sync::Mutex::new(VecDeque::new()));

struct CachedCatalog {
    user_id: u64,
    query: String,
    sort: String,
    loaded_at: Instant,
    items: Arc<Vec<SchematicPost>>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SchematicFile {
    pub name: String,
    pub size: Option<u64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchematicPost {
    pub source: &'static str,
    pub post_id: u64,
    pub title: String,
    pub summary: String,
    pub author: String,
    pub cover: Option<String>,
    pub game_versions: Vec<String>,
    pub files: Vec<SchematicFile>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchematicDetail {
    #[serde(flatten)]
    pub project: SchematicPost,
    pub description: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchematicPage {
    pub items: Vec<SchematicPost>,
    pub page_count: u32,
    pub total: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Post {
    post_id: u64,
    title: String,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    author_name: Option<String>,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    cover: Option<String>,
    #[serde(default)]
    is_paid: Value,
    #[serde(default)]
    pay_amount: Option<f64>,
    #[serde(default)]
    version_id: Value,
}

fn free(post: &Post) -> bool {
    (post.is_paid == "0" || post.is_paid == 0 || post.is_paid == false)
        && post.pay_amount.is_none_or(|amount| amount == 0.0)
}

fn valid_filename(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 240
        && name.trim() == name
        && !name
            .chars()
            .any(|c| c.is_control() || "/\\:*?\"<>|".contains(c))
        && name.rsplit('.').next().is_some_and(|extension| {
            ["litematic", "schematic", "schem"]
                .contains(&extension.to_ascii_lowercase().as_str())
        })
}

fn files(content: &str) -> Vec<SchematicFile> {
    let document = Document::from(content);
    let mut seen = HashSet::new();
    document
        .select(".re-file[data-name]")
        .iter()
        .filter_map(|node| {
            let name = node.attr("data-name")?.to_string();
            if !valid_filename(&name) || !seen.insert(name.clone()) {
                return None;
            }
            let size =
                node.attr("data-size").and_then(|value| value.parse().ok());
            if size.is_some_and(|size| size > MAX_FILE_SIZE as u64) {
                return None;
            }
            Some(SchematicFile { name, size })
        })
        .collect()
}

fn project(
    post: Post,
    versions: &HashMap<String, String>,
) -> Option<SchematicPost> {
    if !free(&post) {
        return None;
    }
    let files = files(post.content.as_deref().unwrap_or_default());
    if files.is_empty() {
        return None;
    }
    let cover = post.cover.and_then(|cover| trusted_cover(&cover));
    Some(SchematicPost {
        source: "shimu",
        post_id: post.post_id,
        title: post.title,
        summary: post.summary.unwrap_or_default(),
        author: post.author_name.unwrap_or_default(),
        cover,
        game_versions: match &post.version_id {
            Value::Number(id) => {
                versions.get(&id.to_string()).cloned().into_iter().collect()
            }
            Value::String(ids) => ids
                .split(',')
                .filter_map(|id| versions.get(id.trim()).cloned())
                .collect(),
            _ => vec![],
        },
        files,
    })
}

fn trusted_cover(cover: &str) -> Option<String> {
    if cover.trim().is_empty() {
        return None;
    }
    let host = Url::parse(HOST).ok()?;
    let url = host.join(cover).ok()?;
    (url.origin() == host.origin()
        && url.username().is_empty()
        && url.password().is_none())
    .then(|| url.to_string())
}

fn installable(post: &Post, post_id: u64, file_name: &str) -> bool {
    post.post_id == post_id
        && free(post)
        && files(post.content.as_deref().unwrap_or_default())
            .iter()
            .any(|file| file.name == file_name)
}

fn connection() -> crate::Result<Connection> {
    let _guard = session()?;
    read_connection(&CredentialStore)?
        .filter(|connection| connection.expires_at > now())
        .ok_or_else(|| failure("Sign in to Shimu Space to continue"))
}

async fn get(
    path: &str,
    query: &[(&str, String)],
) -> crate::Result<reqwest::Response> {
    let token = connection()?.access_token;
    let response = http_client()
        .await?
        .get(format!("{HOST}/shimu{path}"))
        .query(query)
        .timeout(std::time::Duration::from_secs(
            if path.ends_with("/resource") { 300 } else { 30 },
        ))
        .bearer_auth(&token)
        .send()
        .await
        .map_err(|_| failure("Could not connect to Shimu Space"))?;
    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        let _guard = session()?;
        if let Some(mut connection) = read_connection(&CredentialStore)?
            .filter(|connection| connection.access_token == token)
        {
            connection.expires_at = 0;
            let value = serde_json::to_string(&connection)
                .map_err(|_| storage_error())?;
            oauth_credentials::write(&CredentialStore, Some(&value))?;
        }
        return Err(failure("Sign in to Shimu Space again to continue"));
    }
    if !response.status().is_success() {
        return Err(failure("Shimu Space rejected the request"));
    }
    Ok(response)
}

pub async fn list(
    page: u32,
    query: String,
    page_size: u32,
    sort: String,
) -> crate::Result<SchematicPage> {
    if page == 0
        || page > 10000
        || query.chars().count() > 200
        || ![5, 10, 15, 20, 50, 100].contains(&page_size)
        || !["hot", "new", "featured"].contains(&sort.as_str())
    {
        return Err(failure("Invalid schematic search"));
    }
    let user_id = connection()?.user.user_id;
    let cached = {
        let mut catalogs = CATALOGS.lock().await;
        catalogs.retain(|catalog| catalog.loaded_at.elapsed() < CATALOG_TTL);
        catalogs
            .iter()
            .find(|catalog| {
                catalog.user_id == user_id
                    && catalog.query == query
                    && catalog.sort == sort
            })
            .map(|catalog| Arc::clone(&catalog.items))
    };
    let items = if let Some(items) = cached {
        items
    } else {
        // Cache misses may fetch many pages. Other queries must be able to
        // inspect and update the cache while these requests are in flight.
        let items = Arc::new(load_catalog(&query, &sort).await?);
        let mut catalogs = CATALOGS.lock().await;
        catalogs.retain(|catalog| {
            catalog.loaded_at.elapsed() < CATALOG_TTL
                && !(catalog.user_id == user_id
                    && catalog.query == query
                    && catalog.sort == sort)
        });
        catalogs.push_front(CachedCatalog {
            user_id,
            query,
            sort,
            loaded_at: Instant::now(),
            items: Arc::clone(&items),
        });
        catalogs.truncate(4);
        items
    };
    // Page only the eligible resources. The upstream total also includes paid
    // posts and posts without schematic attachments, so it cannot size our pages.
    let offset = (page as usize - 1) * page_size as usize;
    Ok(SchematicPage {
        total: items.len() as u64,
        page_count: items.len().div_ceil(page_size as usize).max(1) as u32,
        items: items
            .iter()
            .skip(offset)
            .take(page_size as usize)
            .cloned()
            .collect(),
    })
}

#[derive(Deserialize)]
struct PostPage {
    code: u16,
    total: u64,
    rows: Vec<Post>,
}

async fn post_page(
    page: u32,
    query: &str,
    sort: &str,
) -> crate::Result<PostPage> {
    let reply = get(
        "/api/forum/post/list",
        &[
            ("pageNum", page.to_string()),
            ("pageSize", CATALOG_PAGE_SIZE.to_string()),
            ("keyword", query.to_string()),
            ("sort", sort.to_string()),
            ("isPaid", "0".into()),
        ],
    )
    .await?
    .json::<PostPage>()
    .await
    .map_err(|_| failure("Invalid Shimu Space resource list"))?;
    if reply.code != 200 {
        return Err(failure("Could not load Shimu Space resources"));
    }
    Ok(reply)
}

async fn load_catalog(
    query: &str,
    sort: &str,
) -> crate::Result<Vec<SchematicPost>> {
    let (first, versions) =
        tokio::try_join!(post_page(1, query, sort), versions())?;
    let last_page = first.total.div_ceil(CATALOG_PAGE_SIZE as u64).max(1);
    if last_page > 10000 {
        return Err(failure(
            "Too many resources. Narrow your schematic search",
        ));
    }
    let mut items = Vec::new();
    let mut seen = HashSet::new();
    let mut append = |posts: Vec<Post>| {
        for post in posts {
            if seen.insert(post.post_id)
                && let Some(project) = project(post, &versions)
            {
                items.push(project);
            }
        }
    };
    append(first.rows);
    // Buffered requests preserve source ordering while bounding concurrency.
    let mut remaining = stream::iter(2..=last_page as u32)
        .map(|page| post_page(page, query, sort))
        .buffered(4);
    while let Some(reply) = remaining.next().await {
        append(reply?.rows);
    }
    Ok(items)
}

async fn versions() -> crate::Result<HashMap<String, String>> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Version {
        version_id: u64,
        name: String,
    }
    #[derive(Deserialize)]
    struct Reply {
        code: u16,
        data: Vec<Version>,
    }
    let reply = get("/api/forum/version/list", &[])
        .await?
        .json::<Reply>()
        .await
        .map_err(|_| failure("Invalid Shimu Space versions"))?;
    if reply.code != 200 {
        return Err(failure("Could not load Shimu Space versions"));
    }
    Ok(reply
        .data
        .into_iter()
        .map(|version| (version.version_id.to_string(), version.name))
        .collect())
}

async fn post(post_id: u64) -> crate::Result<Post> {
    if post_id == 0 {
        return Err(failure("Invalid schematic identity"));
    }
    #[derive(Deserialize)]
    struct Reply {
        code: u16,
        data: Post,
    }
    let reply = get(&format!("/api/forum/post/{post_id}"), &[])
        .await?
        .json::<Reply>()
        .await
        .map_err(|_| failure("Invalid Shimu Space resource details"))?;
    if reply.code != 200 || reply.data.post_id != post_id {
        return Err(failure("Could not load Shimu Space resource details"));
    }
    Ok(reply.data)
}

pub async fn detail(post_id: u64) -> crate::Result<SchematicDetail> {
    let post = post(post_id).await?;
    let description = post.content.clone().unwrap_or_default();
    let versions = versions().await?;
    let project = project(post, &versions).ok_or_else(|| {
        failure("Only free schematic attachments can be opened")
    })?;
    Ok(SchematicDetail {
        project,
        description,
    })
}

pub(crate) async fn download(
    post_id: u64,
    file_name: &str,
) -> crate::Result<Vec<u8>> {
    if !valid_filename(file_name) {
        return Err(failure("Invalid schematic file"));
    }
    // Re-read the authoritative post before any file request. Neither a cached
    // list nor a preview authorizes a download or installation.
    let post = post(post_id).await?;
    if !installable(&post, post_id, file_name) {
        return Err(failure(
            "Only free schematic attachments can be installed",
        ));
    }
    let mut response = get(
        &format!("/api/forum/post/{post_id}/resource"),
        &[("name", file_name.to_string())],
    )
    .await?;
    if response
        .content_length()
        .is_some_and(|size| size > MAX_FILE_SIZE as u64)
    {
        return Err(failure("The schematic file is too large"));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| failure("Schematic download failed"))?
    {
        if bytes.len() + chunk.len() > MAX_FILE_SIZE {
            return Err(failure("The schematic file is too large"));
        }
        bytes.extend_from_slice(&chunk);
    }

    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn covers_only_use_the_documented_shimu_origin() {
        assert_eq!(
            trusted_cover("/uploads/cover.png").as_deref(),
            Some("https://www.mcimg.com/uploads/cover.png")
        );
        assert!(
            trusted_cover("https://www.mcimg.com/uploads/cover.png").is_some()
        );
        for cover in [
            "https://attacker.invalid/cover.png",
            "//attacker.invalid/cover.png",
            "https://www.mcimg.com.attacker.invalid/cover.png",
            "https://user@www.mcimg.com/cover.png",
            "https://www.mcimg.com:444/cover.png",
            "http://www.mcimg.com/cover.png",
            "data:image/png;base64,AAAA",
            "",
        ] {
            assert!(trusted_cover(cover).is_none(), "{cover}");
        }
    }
}

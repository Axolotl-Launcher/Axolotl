//! Minecraft official news from the Minecraft website search API.
use reqwest::Method;
use serde::{Deserialize, Serialize};

use crate::State;
use crate::util::fetch::fetch_json;

const MINECRAFT_NEWS_SEARCH_URL: &str = "https://net-secondary.web.minecraft-services.net/api/v1.0/en-us/search?pageSize=24&sortType=Recent&category=News&newsOnly=true&geography=USA";
const MINECRAFT_ARTICLE_BASE: &str = "https://www.minecraft.net/en-us/article/";

#[derive(Deserialize)]
struct NewsSearchResponse {
    result: NewsSearchResult,
}

#[derive(Deserialize)]
struct NewsSearchResult {
    results: Vec<NewsEntry>,
}

#[derive(Deserialize)]
struct NewsEntry {
    title: String,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    image: Option<String>,
    #[serde(default)]
    time: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MinecraftNewsItem {
    pub title: String,
    pub category: Option<String>,
    pub tag: Option<String>,
    pub date: Option<String>,
    pub image_url: Option<String>,
    pub read_more_url: String,
}

pub async fn get_minecraft_news(
    limit: usize,
) -> crate::Result<Vec<MinecraftNewsItem>> {
    let state = State::get().await?;
    let news = fetch_json::<NewsSearchResponse>(
        Method::GET,
        MINECRAFT_NEWS_SEARCH_URL,
        None,
        None,
        None,
        &state.api_semaphore,
    )
    .await?;

    let mut items: Vec<MinecraftNewsItem> = news
        .result
        .results
        .into_iter()
        .filter_map(|entry| {
            let read_more_url = article_url(entry.url.as_deref()?)?;
            Some(MinecraftNewsItem {
                title: entry.title,
                category: None,
                tag: None,
                date: entry
                    .time
                    .and_then(|timestamp| {
                        chrono::DateTime::from_timestamp(timestamp, 0)
                    })
                    .map(|timestamp| timestamp.to_rfc3339()),
                image_url: entry.image,
                read_more_url,
            })
        })
        .collect();

    items.sort_by(|a, b| b.date.cmp(&a.date));
    items.truncate(limit);
    Ok(items)
}

fn article_url(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url.trim()).ok()?;
    if parsed.scheme() != "https"
        || parsed.host_str() != Some("www.minecraft.net")
    {
        return None;
    }
    let article_id = parsed
        .path_segments()?
        .rfind(|segment| !segment.is_empty())?;
    url::Url::parse(MINECRAFT_ARTICLE_BASE)
        .ok()?
        .join(article_id)
        .ok()
        .map(|url| url.to_string())
}

#[cfg(test)]
mod tests {
    use super::article_url;

    #[test]
    fn derives_article_urls_from_search_results() {
        assert_eq!(
            article_url("https://www.minecraft.net/en-us/article/example"),
            Some("https://www.minecraft.net/en-us/article/example".to_string())
        );
        assert_eq!(
            article_url("https://www.minecraft.net/fr-fr/article/example"),
            Some("https://www.minecraft.net/en-us/article/example".to_string())
        );
        assert_eq!(article_url("https://example.com/article/example"), None);
        assert_eq!(article_url("javascript:alert(1)"), None);
        assert_eq!(article_url("https://"), None);
        assert_eq!(article_url("  "), None);
    }
}

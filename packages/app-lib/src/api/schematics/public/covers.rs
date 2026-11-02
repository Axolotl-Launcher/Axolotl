//! Reden's image CDN requires a Referer unavailable to webview image requests.

use std::collections::VecDeque;
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};

use base64::{Engine, engine::general_purpose::STANDARD};

use super::{Source, fetch, safe_url};

const MAX_IMAGE: usize = 4 * 1024 * 1024;
const MAX_ENTRIES: usize = 64;
const MAX_CACHE_BYTES: usize = 16 * 1024 * 1024;
const SUCCESS_TTL: Duration = Duration::from_secs(60 * 60);
const FAILURE_TTL: Duration = Duration::from_secs(60);
static CACHE: LazyLock<tokio::sync::Mutex<CoverCache>> =
    LazyLock::new(Default::default);

struct Entry {
    url: String,
    data: Option<Arc<str>>,
    expires: Instant,
}

impl Entry {
    fn size(&self) -> usize {
        self.url.len() + self.data.as_ref().map_or(0, |data| data.len())
    }
}

#[derive(Default)]
struct CoverCache {
    entries: VecDeque<Entry>,
    bytes: usize,
}

impl CoverCache {
    fn prune(&mut self, now: Instant) {
        self.entries.retain(|entry| entry.expires > now);
        self.bytes = self.entries.iter().map(Entry::size).sum();
    }

    // The outer option distinguishes a miss from a cached failed request.
    fn get(&mut self, url: &str, now: Instant) -> Option<Option<Arc<str>>> {
        self.prune(now);
        let position =
            self.entries.iter().position(|entry| entry.url == url)?;
        let entry = self.entries.remove(position)?;
        let data = entry.data.clone();
        self.entries.push_back(entry);
        Some(data)
    }

    fn insert(&mut self, url: String, data: Option<Arc<str>>, now: Instant) {
        self.prune(now);
        if let Some(position) =
            self.entries.iter().position(|entry| entry.url == url)
        {
            self.bytes -= self.entries.remove(position).unwrap().size();
        }
        let expires = now
            + if data.is_some() {
                SUCCESS_TTL
            } else {
                FAILURE_TTL
            };
        let entry = Entry { url, data, expires };
        let size = entry.size();
        if size > MAX_CACHE_BYTES {
            return;
        }
        while self.entries.len() >= MAX_ENTRIES
            || self.bytes + size > MAX_CACHE_BYTES
        {
            if let Some(previous) = self.entries.pop_front() {
                self.bytes -= previous.size();
            }
        }
        self.bytes += size;
        self.entries.push_back(entry);
    }
}

/// Determine the actual raster format: Reden serves WebP at some .jpg URLs.
fn data_url(bytes: &[u8]) -> Option<Arc<str>> {
    if bytes.len() > MAX_IMAGE {
        return None;
    }
    let mime = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png"
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        "image/jpeg"
    } else if bytes.len() >= 12
        && &bytes[..4] == b"RIFF"
        && &bytes[8..12] == b"WEBP"
    {
        "image/webp"
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        "image/gif"
    } else {
        return None;
    };
    Some(format!("data:{mime};base64,{}", STANDARD.encode(bytes)).into())
}

pub(super) async fn load(raw: &str) -> Option<String> {
    let url = safe_url(Source::Reden, raw)?;
    let key = url.as_str();
    let cached = CACHE.lock().await.get(key, Instant::now());
    if let Some(data) = cached {
        return data.map(|data| data.to_string());
    }
    // Reuse native proxy, Referer, HTTPS host/redirect checks and bounded reads.
    // No cache mutex is held while waiting on the CDN.
    let data = match fetch(Source::Reden, url.clone(), MAX_IMAGE).await {
        Ok(bytes) => data_url(&bytes),
        Err(error) => {
            tracing::debug!(%error, "Could not load Reden schematic cover");
            None
        }
    };
    CACHE
        .lock()
        .await
        .insert(key.to_string(), data.clone(), Instant::now());
    data.map(|data| data.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cover_mime_follows_raster_signature_and_rejects_error_documents() {
        for (bytes, mime) in [
            (b"RIFF\x00\x00\x00\x00WEBP".as_slice(), "image/webp"),
            (b"\x89PNG\r\n\x1a\n".as_slice(), "image/png"),
            (b"\xff\xd8\xff".as_slice(), "image/jpeg"),
            (b"GIF87a".as_slice(), "image/gif"),
            (b"GIF89a".as_slice(), "image/gif"),
        ] {
            assert!(
                data_url(bytes)
                    .unwrap()
                    .starts_with(&format!("data:{mime};base64,"))
            );
        }
        for bytes in [
            b"<html>denied by Referer ACL</html>".as_slice(),
            b"<svg></svg>",
            b"RIFF",
            b"",
        ] {
            assert!(data_url(bytes).is_none());
        }
        assert!(data_url(&vec![0xff; MAX_IMAGE + 1]).is_none());
    }

    #[test]
    fn cover_cache_bounds_encoded_bytes_and_entry_count() {
        let mut cache = CoverCache::default();
        let now = Instant::now();
        for index in 0..=MAX_ENTRIES {
            cache.insert(index.to_string(), None, now);
        }
        assert_eq!(cache.entries.len(), MAX_ENTRIES);
        assert!(cache.get("0", now).is_none());
        assert_eq!(cache.get("1", now), Some(None));
        let data: Arc<str> = "a".repeat(MAX_IMAGE).into();
        for index in 0..5 {
            cache.insert(format!("image-{index}"), Some(data.clone()), now);
        }
        assert!(cache.bytes <= MAX_CACHE_BYTES);
        assert!(cache.get("image-0", now).is_none());
        assert!(cache.get("image-4", now).unwrap().is_some());
    }

    #[test]
    fn cover_cache_retries_failures_earlier_than_successes() {
        let mut cache = CoverCache::default();
        let now = Instant::now();
        cache.insert("failure".into(), None, now);
        cache.insert(
            "success".into(),
            Some("data:image/webp;base64,AAAA".into()),
            now,
        );
        assert_eq!(cache.get("failure", now), Some(None));
        assert!(cache.get("failure", now + FAILURE_TTL).is_none());
        assert!(cache.get("success", now + FAILURE_TTL).unwrap().is_some());
        assert!(cache.get("success", now + SUCCESS_TTL).is_none());
        assert_eq!(cache.bytes, 0);
    }
}

//! Browsing for schematic (Litematica) sharing sites.
//!
//! Four upstream sources are aggregated behind one trait. The command layer only
//! talks to [`source::SchematicBrowseSource`], so adding a source never touches a
//! command.
//!
//! ## Why the network lives here
//!
//! None of these sites can be read from the webview:
//!
//! - Reden treats `Origin` as an allowlist and rejects anything that is not its
//!   own web origin — including its own API host.
//! - BuildSchematics sends no CORS headers at all, so a browser `fetch` cannot
//!   read the response body.
//!
//! ## Why this does not use the provider install pipeline
//!
//! Upstream marks `ProjectType::Schematic` and `WorldSave` as local-only types
//! that never go through Modrinth API resolution. Files written into an
//! instance's `schematics/` directory are picked up by the existing content
//! scanner, so no new `ContentType` and no provider registration is needed.

pub mod cache;
pub mod download;
pub mod error;
pub mod http;
pub mod net;
pub mod source;
pub mod sources;
pub mod types;

use tokio_util::sync::CancellationToken;

use crate::api::schematic_browser::error::{
    SchematicSourceError, SourceResult,
};
use crate::api::schematic_browser::types::{
    GenerateSize, ListQuery, ListResult, SchematicDetail, SchematicItem,
    SourceCapabilities, SourceFailure, SourceId, SourceStatus,
};

/// Resolve the source for an id, or report it as unsupported.
fn require(
    source_id: SourceId,
) -> SourceResult<std::sync::Arc<dyn source::SchematicBrowseSource>> {
    source::get(source_id).ok_or_else(|| {
        SchematicSourceError::unsupported(format!(
            "source {} is not available",
            source_id.as_str()
        ))
    })
}

fn failure_of(
    source_id: SourceId,
    error: &SchematicSourceError,
) -> SourceFailure {
    SourceFailure {
        source_id,
        code: error.code().to_string(),
        message: error.to_string(),
        retryable: error.is_retryable(),
    }
}

/// List one source's items.
pub async fn list(
    source_id: SourceId,
    query: &ListQuery,
    cancel: &CancellationToken,
) -> SourceResult<ListResult> {
    require(source_id)?.list(query, cancel).await
}

/// List several sources, collecting per-source failures instead of failing the
/// whole call.
///
/// One unreachable site must not hide the others, which is why the result keeps
/// a failure list alongside the items.
pub async fn list_many(
    source_ids: &[SourceId],
    query: &ListQuery,
    cancel: &CancellationToken,
) -> (Vec<ListResult>, Vec<SourceFailure>) {
    let mut results = Vec::new();
    let mut failures = Vec::new();

    for source_id in source_ids {
        match list(*source_id, query, cancel).await {
            Ok(result) => results.push(result),
            Err(SchematicSourceError::Canceled) => break,
            Err(error) => failures.push(failure_of(*source_id, &error)),
        }
    }

    (results, failures)
}

/// List every source and present the merged result as one page.
///
/// The tab's "all sources" option needs a single ordered list. Each source is
/// asked for the same page number and the pages are merged, then ordered by
/// download count with the title as the tie-break; that is what the caller
/// shows, so paging stays predictable without a second round trip per source.
pub async fn list_all(
    query: &ListQuery,
    cancel: &CancellationToken,
) -> SourceResult<ListResult> {
    let ids = SourceId::all();
    let (results, failures) = list_many(&ids, query, cancel).await;

    if results.is_empty() {
        // Every source failed; report the first failure rather than an empty
        // list, so the UI can offer a retry against a real reason.
        let first = failures.first().ok_or_else(|| {
            SchematicSourceError::unsupported("no schematic sources are available")
        })?;
        return Err(SchematicSourceError::unsupported(format!(
            "every source failed ({}: {})",
            first.code, first.message
        )));
    }

    let mut items: Vec<SchematicItem> = results
        .iter()
        .flat_map(|result| result.items.iter().cloned())
        .collect();
    items.sort_by(|a, b| {
        b.downloads
            .unwrap_or(0)
            .cmp(&a.downloads.unwrap_or(0))
            .then_with(|| a.title.cmp(&b.title))
    });

    let total_value: u64 = results
        .iter()
        .filter_map(|result| result.total.value)
        .sum();
    let any_estimated = results
        .iter()
        .any(|result| result.total.precision != types::TotalPrecision::Exact);

    let status = results[0].source_status.clone();
    Ok(ListResult {
        page: query.page,
        has_more: results.iter().any(|result| result.has_more),
        total: if any_estimated {
            types::ResourceTotal::estimated(total_value)
        } else if total_value > 0 {
            types::ResourceTotal::exact(total_value)
        } else {
            types::ResourceTotal::unknown()
        },
        items,
        source_status: status,
        source_failures: failures,
    })
}

/// Fetch a thumbnail and return it as a `data:` URI.
///
/// See [`http::get_bytes`] for why this has to happen in Rust rather than in the
/// webview's `<img>`; the base64 hop keeps the component markup unchanged.
pub async fn fetch_image(
    url: &str,
    referer: Option<&str>,
    cancel: &CancellationToken,
) -> SourceResult<String> {
    let headers = match referer {
        Some(value) => http::RequestHeaders::with_referer(value),
        None => http::RequestHeaders::default(),
    };
    let bytes = http::get_bytes(url, &headers, cancel).await?;
    let mime = sniff_image_mime(&bytes);
    Ok(format!(
        "data:{mime};base64,{}",
        base64_encode(&bytes)
    ))
}

/// Guess the MIME type from the magic bytes, defaulting to png.
fn sniff_image_mime(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        "image/png"
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "image/jpeg"
    } else if bytes.starts_with(b"GIF8") {
        "image/gif"
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        "image/webp"
    } else {
        "image/png"
    }
}

/// Minimal base64 encoder, so the crate does not grow a dependency for one use.
fn base64_encode(input: &[u8]) -> String {
    const ALPHABET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[((triple >> 18) & 0x3F) as usize] as char);
        out.push(ALPHABET[((triple >> 12) & 0x3F) as usize] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[((triple >> 6) & 0x3F) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[(triple & 0x3F) as usize] as char
        } else {
            '='
        });
    }
    out
}

/// Read one item's detail.
pub async fn detail(
    source_id: SourceId,
    id: &str,
    cancel: &CancellationToken,
) -> SourceResult<SchematicDetail> {
    require(source_id)?.detail(id, cancel).await
}

/// Resolve how to download one item.
pub async fn download_plan(
    source_id: SourceId,
    id: &str,
    size: Option<GenerateSize>,
    cancel: &CancellationToken,
) -> SourceResult<types::DownloadPlan> {
    require(source_id)?.download_plan(id, size, cancel).await
}

/// Capability matrix for every source, used by the frontend to decide which
/// controls to render.
pub fn capabilities() -> Vec<(SourceId, SourceCapabilities)> {
    source::all()
        .iter()
        .map(|source| (source.id(), source.capabilities()))
        .collect()
}

/// Probe every source's health.
pub async fn health() -> Vec<SourceStatus> {
    // Run the probes concurrently: a source whose host is slow or unreachable
    // would otherwise hold up every source behind it, and each probe carries its
    // own retries and timeout.
    let probes = source::all()
        .into_iter()
        .map(|source| async move { source.health_check().await });
    futures::future::join_all(probes).await
}

/// Items across several sources, flattened and de-duplicated by id.
pub fn merge_items(results: &[ListResult]) -> Vec<SchematicItem> {
    let mut seen = std::collections::HashSet::new();
    let mut merged = Vec::new();
    for result in results {
        for item in &result.items {
            if seen.insert(item.id.clone()) {
                merged.push(item.clone());
            }
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capabilities_cover_every_source() {
        let matrix = capabilities();
        assert_eq!(matrix.len(), SourceId::all().len());
        for (id, caps) in matrix {
            assert_eq!(id.as_str().is_empty(), false);
            // Every source must be able to serve a listing.
            let _ = caps.supports_server_side_paging;
        }
    }

    #[test]
    fn merge_deduplicates_by_id() {
        let make = |id: &str, source: SourceId| SchematicItem {
            id: id.to_string(),
            source_id: source,
            title: id.to_string(),
            description: None,
            format: types::SchematicFormat::Schem,
            content_kind: types::ContentKind::Schematic,
            versions: Vec::new(),
            downloads: None,
            updated_at: None,
            images: Vec::new(),
            detail_url: String::new(),
            download: types::DownloadPlan::direct("https://x/a.schem"),
            degraded: false,
            degraded_reason: None,
        };
        let results = vec![
            ListResult {
                items: vec![make("reden:1", SourceId::Reden)],
                page: 1,
                has_more: false,
                total: types::ResourceTotal::unknown(),
                source_status: SourceStatus {
                    source_id: SourceId::Reden,
                    state: types::SourceState::Ok,
                    message: None,
                    cached: false,
                    fetched_at: None,
                },
                source_failures: Vec::new(),
            },
            ListResult {
                items: vec![
                    make("reden:1", SourceId::Reden),
                    make("mc:2", SourceId::McSchematics),
                ],
                page: 1,
                has_more: false,
                total: types::ResourceTotal::unknown(),
                source_status: SourceStatus {
                    source_id: SourceId::McSchematics,
                    state: types::SourceState::Ok,
                    message: None,
                    cached: false,
                    fetched_at: None,
                },
                source_failures: Vec::new(),
            },
        ];
        let merged = merge_items(&results);
        assert_eq!(merged.len(), 2, "duplicate ids must collapse");
    }

    #[test]
    fn failures_carry_a_stable_code() {
        let error = SchematicSourceError::http(403, "origin rejected");
        let failure = failure_of(SourceId::Reden, &error);
        assert_eq!(failure.code, "http_403");
        assert!(!failure.retryable);
    }
}

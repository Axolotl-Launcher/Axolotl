//! Thin HTTP helpers shared by the source adapters.
//!
//! Every request goes through the launcher's configured client, so the user's
//! proxy settings and the launcher user agent apply automatically. Requests are
//! wrapped in the per-host limiter and retry loop from [`super::net`].

use std::time::Duration;

use crate::util::fetch::reqwest_client_builder;
use crate::util::download::retired_sources;

use tokio_util::sync::CancellationToken;

use super::error::{NetworkKind, SchematicSourceError, SourceResult};
use super::net::{self, HostKey};

/// How long one request may take before it counts as a timeout.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
/// Largest response body we will buffer.
const MAX_BODY_BYTES: usize = 16 * 1024 * 1024;
/// Redirects to follow before giving up.
const MAX_REDIRECTS: usize = 5;

/// A response with its body already read.
pub struct HttpResponse {
    pub status: u16,
    pub body: String,
    pub final_url: String,
}

impl HttpResponse {
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

fn client() -> SourceResult<reqwest::Client> {
    // Build on the launcher's standard client builder so proxy, DNS, TLS and
    // user-agent settings apply, then replace the redirect policy: the default
    // follows redirects anywhere, and a redirected request can land on a
    // service on the user's own machine. Only http(s) to a public host is
    // allowed, and retired download hosts are rejected outright.
    reqwest_client_builder()
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if retired_sources::is_retired(attempt.url()) {
                attempt.error("retired download source")
            } else if attempt.previous().len() >= MAX_REDIRECTS {
                attempt.error("too many redirects")
            } else if !is_public_http_url(attempt.url()) {
                attempt.error("redirect target is not a public http(s) host")
            } else {
                attempt.follow()
            }
        }))
        .build()
        .map_err(|error| {
            SchematicSourceError::network(
                NetworkKind::Unknown,
                error.to_string(),
            )
        })
}

/// Whether a URL points at a routable public host.
///
/// Blocks loopback, link-local, private ranges and non-http schemes so a
/// compromised or malicious source cannot use a redirect to reach local services.
fn is_public_http_url(url: &reqwest::Url) -> bool {
    if !matches!(url.scheme(), "http" | "https") {
        return false;
    }
    match url.host() {
        Some(url::Host::Ipv4(address)) => {
            let octets = address.octets();
            !(address.is_loopback()
                || address.is_private()
                || address.is_link_local()
                || address.is_broadcast()
                || address.is_unspecified()
                || octets[0] == 0
                // 100.64.0.0/10 carrier-grade NAT
                || (octets[0] == 100 && (64..128).contains(&octets[1])))
        }
        Some(url::Host::Ipv6(address)) => {
            !(address.is_loopback()
                || address.is_unspecified()
                // Unique local + link local
                || (address.segments()[0] & 0xfe00) == 0xfc00
                || (address.segments()[0] & 0xffc0) == 0xfe80)
        }
        // A hostname is resolved by the HTTP client; blocking by name is not
        // reliable, so names are allowed and only literal addresses are checked.
        Some(url::Host::Domain(_)) => true,
        None => false,
    }
}

/// Extra headers a source needs.
#[derive(Default, Clone)]
pub struct RequestHeaders {
    pub referer: Option<String>,
}

impl RequestHeaders {
    pub fn with_referer(referer: impl Into<String>) -> Self {
        Self {
            referer: Some(referer.into()),
        }
    }
}

/// Whether requests currently leave through a proxy.
///
/// A direct connection and a proxied one leave the machine by different paths,
/// so they get separate admission and breaker state. Reading the live proxy mode
/// is async and hits SQLite, and this is consulted per request, so the answer is
/// cached and refreshed once per batch by [`refresh_proxy_state`].
static PROXIED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(true);

fn proxied() -> bool {
    PROXIED.load(std::sync::atomic::Ordering::Relaxed)
}

/// Refresh the cached proxy flag. The command layer calls this once per batch
/// after reading the settings, instead of per request.
pub fn refresh_proxy_state(mode: crate::util::proxy::ProxyMode) {
    PROXIED.store(
        mode != crate::util::proxy::ProxyMode::None,
        std::sync::atomic::Ordering::Relaxed,
    );
}

fn classify_transport(error: &reqwest::Error) -> NetworkKind {
    if error.is_timeout() {
        NetworkKind::Timeout
    } else if error.is_connect() {
        NetworkKind::Connect
    } else if error.is_body() || error.is_decode() {
        NetworkKind::Body
    } else {
        let text = error.to_string().to_ascii_lowercase();
        if text.contains("dns") || text.contains("resolve") {
            NetworkKind::Dns
        } else if text.contains("tls") || text.contains("certificate") {
            NetworkKind::Tls
        } else {
            NetworkKind::Unknown
        }
    }
}

/// Read a response body while enforcing the size cap.
///
/// Reads incrementally instead of buffering the whole body, so a server that
/// omits `Content-Length` cannot make us allocate unbounded memory.
async fn read_limited(
    response: reqwest::Response,
    cancel: &CancellationToken,
) -> SourceResult<Vec<u8>> {
    use futures::StreamExt;

    let mut stream = response.bytes_stream();
    let mut buffer: Vec<u8> = Vec::new();

    loop {
        let chunk = tokio::select! {
            _ = cancel.cancelled() => {
                return Err(SchematicSourceError::Canceled)
            }
            next = stream.next() => next,
        };
        match chunk {
            Some(Ok(bytes)) => {
                if buffer.len() + bytes.len() > MAX_BODY_BYTES {
                    return Err(SchematicSourceError::parse(
                        "response",
                        format!(
                            "body exceeded the {MAX_BODY_BYTES} byte limit"
                        ),
                    ));
                }
                buffer.extend_from_slice(&bytes);
            }
            Some(Err(error)) => {
                return Err(SchematicSourceError::network(
                    classify_transport(&error),
                    error.to_string(),
                ));
            }
            None => break,
        }
    }

    Ok(buffer)
}

/// Parse `Retry-After` in either of the two forms the spec allows.
///
/// It may be a delay in seconds or an HTTP date. Only handling the numeric form
/// would ignore a perfectly valid header and retry too early.
fn retry_after_ms(headers: &reqwest::header::HeaderMap) -> u64 {
    let Some(raw) = headers
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
    else {
        return 1_000;
    };

    if let Ok(seconds) = raw.parse::<u64>() {
        return seconds.saturating_mul(1_000).max(1_000);
    }

    if let Ok(when) = chrono::DateTime::parse_from_rfc2822(raw) {
        let now = chrono::Utc::now();
        let millis = when
            .with_timezone(&chrono::Utc)
            .signed_duration_since(now)
            .num_milliseconds();
        return millis.max(1_000) as u64;
    }

    1_000
}

/// Read a host's rate-limit headers.
///
/// Reden sends `x-ratelimit-limit`, `x-ratelimit-remaining` and
/// `x-ratelimit-reset`; the reset value is an absolute Unix timestamp rather than
/// a duration, so it is compared against the clock instead of being added to it.
fn parse_quota_headers(headers: &reqwest::header::HeaderMap) -> net::QuotaHint {
    fn number(headers: &reqwest::header::HeaderMap, name: &str) -> Option<u32> {
        headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.trim().parse::<u32>().ok())
    }

    net::QuotaHint {
        limit: number(headers, "x-ratelimit-limit"),
        remaining: number(headers, "x-ratelimit-remaining"),
        reset_at: headers
            .get("x-ratelimit-reset")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.trim().parse::<i64>().ok()),
    }
}

fn status_error(status: u16, url: &str) -> SchematicSourceError {
    match status {
        403 => SchematicSourceError::http(
            403,
            format!(
                "403 from {} — the site rejected this request",
                net::redact_url(url)
            ),
        ),
        404 => SchematicSourceError::not_found(net::redact_url(url)),
        429 => SchematicSourceError::RateLimited {
            retry_after_ms: 1_000,
        },
        _ => SchematicSourceError::http(
            status,
            format!("HTTP {status} from {}", net::redact_url(url)),
        ),
    }
}

/// GET a URL as text, with per-host admission, retries and cancellation.
///
/// Deliberately sends **no `Origin` header**: Reden treats Origin as an
/// allowlist and rejects anything that is not its own web origin, including its
/// own API host.
pub async fn get_text(
    url: &str,
    headers: &RequestHeaders,
    cancel: &CancellationToken,
) -> SourceResult<HttpResponse> {
    let key = HostKey::from_url(url, proxied());

    net::with_retries(&key, cancel, || async {
        let mut request = client()?
            .get(url)
            .timeout(REQUEST_TIMEOUT);
        if let Some(referer) = headers.referer.as_deref() {
            request = request.header(reqwest::header::REFERER, referer);
        }

        let response = tokio::select! {
            _ = cancel.cancelled() => {
                return Err(SchematicSourceError::Canceled)
            }
            result = request.send() => result.map_err(|error| {
                SchematicSourceError::network(classify_transport(&error), error.to_string())
            })?,
        };

        let status = response.status().as_u16();
        let final_url = response.url().to_string();

        // Feed the host's own quota report back into the limiter. Reden publishes
        // `x-ratelimit-limit` / `remaining` / `reset`, where `reset` is an
        // absolute Unix timestamp.
        net::record_quota_hint(&key, parse_quota_headers(response.headers()));

        if status == 429 {
            return Err(SchematicSourceError::RateLimited {
                retry_after_ms: retry_after_ms(response.headers()),
            });
        }

        if !(200..300).contains(&status) {
            return Err(status_error(status, url));
        }

        // Reject an oversized body before reading it, when the server reports a
        // length, and stream the rest so a lying or absent length cannot force a
        // multi-gigabyte allocation.
        if let Some(length) = response.content_length()
            && length > MAX_BODY_BYTES as u64
        {
            return Err(SchematicSourceError::parse(
                "response",
                format!("body of {length} bytes exceeds the {MAX_BODY_BYTES} byte limit"),
            ));
        }

        let bytes = tokio::select! {
            _ = cancel.cancelled() => {
                return Err(SchematicSourceError::Canceled)
            }
            result = read_limited(response, cancel) => result?,
        };

        if bytes.len() > MAX_BODY_BYTES {
            return Err(SchematicSourceError::parse(
                "response",
                format!("body exceeded {MAX_BODY_BYTES} bytes"),
            ));
        }

        let body = String::from_utf8_lossy(&bytes).into_owned();
        Ok(HttpResponse {
            status,
            body,
            final_url,
        })
    })
    .await
}

/// GET a URL as bytes. Used for thumbnails and file downloads.
pub async fn get_bytes(
    url: &str,
    headers: &RequestHeaders,
    cancel: &CancellationToken,
) -> SourceResult<Vec<u8>> {
    // The caller supplies this URL (the thumbnail command forwards it verbatim),
    // so the initial request needs the same public-host check a redirect gets:
    // otherwise a crafted source URL could reach loopback or a private service
    // and hand its bytes back to the renderer.
    let parsed = reqwest::Url::parse(url).map_err(|error| {
        SchematicSourceError::parse("url", error.to_string())
    })?;
    if !is_public_http_url(&parsed) {
        return Err(SchematicSourceError::unsupported(
            "only public http(s) urls can be fetched",
        ));
    }

    let key = HostKey::from_url(url, proxied());

    net::with_retries(&key, cancel, || async {
        let mut request = client()?.get(url).timeout(REQUEST_TIMEOUT);
        if let Some(referer) = headers.referer.as_deref() {
            request = request.header(reqwest::header::REFERER, referer);
        }

        let response = tokio::select! {
            _ = cancel.cancelled() => {
                return Err(SchematicSourceError::Canceled)
            }
            result = request.send() => result.map_err(|error| {
                SchematicSourceError::network(classify_transport(&error), error.to_string())
            })?,
        };

        let status = response.status().as_u16();
        if !(200..300).contains(&status) {
            return Err(status_error(status, url));
        }

        // Reject an oversized body before reading it, when the server reports a
        // length, and stream the rest so a lying or absent length cannot force a
        // multi-gigabyte allocation.
        if let Some(length) = response.content_length()
            && length > MAX_BODY_BYTES as u64
        {
            return Err(SchematicSourceError::parse(
                "response",
                format!("body of {length} bytes exceeds the {MAX_BODY_BYTES} byte limit"),
            ));
        }

        let bytes = tokio::select! {
            _ = cancel.cancelled() => {
                return Err(SchematicSourceError::Canceled)
            }
            result = read_limited(response, cancel) => result?,
        };
        Ok(bytes.to_vec())
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_errors_are_classified() {
        // Build a timeout error through a real request to a black hole address.
        // Classification is a pure function of the reqwest error, so an
        // unreachable host with a tiny timeout is enough.
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let error = runtime.block_on(async {
            reqwest::Client::builder()
                .timeout(Duration::from_millis(1))
                .build()
                .unwrap()
                .get("http://192.0.2.1:9/never")
                .send()
                .await
                .unwrap_err()
        });
        assert!(matches!(
            classify_transport(&error),
            NetworkKind::Timeout | NetworkKind::Connect
        ));
    }

    #[test]
    fn status_errors_keep_retry_semantics() {
        assert!(!status_error(403, "https://x/y").is_retryable());
        assert!(status_error(429, "https://x/y").is_retryable());
        assert!(status_error(503, "https://x/y").is_retryable());
        assert!(matches!(
            status_error(404, "https://x/y"),
            SchematicSourceError::NotFound { .. }
        ));
    }
}

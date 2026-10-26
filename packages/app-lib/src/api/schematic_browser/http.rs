//! Thin HTTP helpers shared by the source adapters.
//!
//! Every request goes through the launcher's configured client, so the user's
//! proxy settings and the launcher user agent apply automatically. Requests are
//! wrapped in the per-host limiter and retry loop from [`super::net`].

use std::time::Duration;

use tokio_util::sync::CancellationToken;

use super::error::{NetworkKind, SchematicSourceError, SourceResult};
use super::net::{self, HostKey};

/// How long one request may take before it counts as a timeout.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
/// Largest response body we will buffer.
const MAX_BODY_BYTES: usize = 16 * 1024 * 1024;

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
    // `State` may not be initialized in unit tests; fall back to a plain client
    // so parsing tests never depend on launcher startup.
    match crate::State::get_if_initialized() {
        Some(state) => Ok(state.configured_http_client()),
        None => reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .user_agent(crate::launcher_user_agent())
            .build()
            .map_err(|error| {
                SchematicSourceError::network(
                    NetworkKind::Unknown,
                    error.to_string(),
                )
            }),
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

        if status == 429 {
            let retry_after_ms = response
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.trim().parse::<u64>().ok())
                .map(|seconds| seconds.saturating_mul(1_000))
                .unwrap_or(1_000);
            return Err(SchematicSourceError::RateLimited { retry_after_ms });
        }

        if !(200..300).contains(&status) {
            return Err(status_error(status, url));
        }

        let bytes = tokio::select! {
            _ = cancel.cancelled() => {
                return Err(SchematicSourceError::Canceled)
            }
            result = response.bytes() => result.map_err(|error| {
                SchematicSourceError::network(classify_transport(&error), error.to_string())
            })?,
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

        let bytes = tokio::select! {
            _ = cancel.cancelled() => {
                return Err(SchematicSourceError::Canceled)
            }
            result = response.bytes() => result.map_err(|error| {
                SchematicSourceError::network(classify_transport(&error), error.to_string())
            })?,
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

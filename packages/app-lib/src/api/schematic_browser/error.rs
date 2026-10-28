//! Errors raised by the schematic browsing sources.
//!
//! These stay separate from [`crate::ErrorKind`]: the IPC layer maps them into
//! the command envelope, and each variant carries the detail the UI needs to
//! decide between "retry" and "open the source page".

use serde::{Deserialize, Serialize};

/// Why a network-level call failed.
///
/// Kept apart from the HTTP status so the retry decision does not have to infer
/// intent from a status code alone.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkKind {
    Timeout,
    Dns,
    Tls,
    Connect,
    Body,
    Unknown,
}

impl NetworkKind {
    /// Whether repeating the same request can plausibly succeed.
    pub fn is_retryable(self) -> bool {
        matches!(self, Self::Timeout | Self::Connect | Self::Body)
    }
}

/// Which local write failed.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageCode {
    DiskFull,
    PathInvalid,
    PermissionDenied,
    Io,
}

/// A failure reported by one schematic source.
///
/// Tagged by `error_type` rather than `kind` because [`Self::Network`] already
/// carries a `kind` field describing the transport failure.
#[derive(Clone, Debug, thiserror::Error, Serialize, Deserialize)]
#[serde(tag = "error_type", rename_all = "snake_case")]
pub enum SchematicSourceError {
    /// The caller cancelled the request.
    #[error("request canceled")]
    Canceled,

    /// The transport failed. `status` is present when a response did arrive.
    #[error("network error ({kind:?}): {message}")]
    Network {
        kind: NetworkKind,
        status: Option<u16>,
        message: String,
    },

    /// The source asked us to slow down.
    #[error("rate limited, retry after {retry_after_ms}ms")]
    RateLimited { retry_after_ms: u64 },

    /// The breaker for this host is open.
    #[error("circuit open for {host}")]
    CircuitOpen { host: String },

    /// A response arrived but did not match the shape we expect.
    #[error("could not parse {what}: {detail}")]
    Parse { what: String, detail: String },

    /// The item or page does not exist.
    #[error("not found: {what}")]
    NotFound { what: String },

    /// The source does not support this operation.
    #[error("unsupported by this source: {what}")]
    Unsupported { what: String },

    /// The upstream site changed in a way we cannot read.
    ///
    /// Distinct from [`Self::Network`] on purpose: the UI offers "open the
    /// source page" instead of a retry that cannot help.
    #[error("upstream layout changed: {detail}")]
    UpstreamChanged { detail: String },

    /// A local write failed.
    #[error("storage error ({code:?}): {message}")]
    Storage { code: StorageCode, message: String },
}

impl SchematicSourceError {
    pub fn network(kind: NetworkKind, message: impl Into<String>) -> Self {
        Self::Network {
            kind,
            status: None,
            message: message.into(),
        }
    }

    pub fn http(status: u16, message: impl Into<String>) -> Self {
        Self::Network {
            kind: NetworkKind::Unknown,
            status: Some(status),
            message: message.into(),
        }
    }

    pub fn parse(what: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::Parse {
            what: what.into(),
            detail: detail.into(),
        }
    }

    pub fn not_found(what: impl Into<String>) -> Self {
        Self::NotFound { what: what.into() }
    }

    pub fn unsupported(what: impl Into<String>) -> Self {
        Self::Unsupported { what: what.into() }
    }

    pub fn upstream_changed(detail: impl Into<String>) -> Self {
        Self::UpstreamChanged {
            detail: detail.into(),
        }
    }

    pub fn storage(code: StorageCode, message: impl Into<String>) -> Self {
        Self::Storage {
            code,
            message: message.into(),
        }
    }

    /// Whether repeating the request may succeed.
    ///
    /// A 403 from Reden is not retryable: it means we sent a rejected `Origin`.
    /// A 429 is retryable once the retry-after delay elapses.
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Network { kind, status, .. } => match status {
                Some(429) => true,
                Some(status) if (500..600).contains(status) => true,
                Some(_) => false,
                None => kind.is_retryable(),
            },
            Self::RateLimited { .. } => true,
            _ => false,
        }
    }

    /// A stable code for the IPC envelope.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Canceled => "canceled",
            Self::Network {
                status: Some(403), ..
            } => "http_403",
            Self::Network {
                status: Some(404), ..
            } => "http_404",
            Self::Network {
                status: Some(429), ..
            } => "http_429",
            Self::Network {
                status: Some(500..=599),
                ..
            } => "http_5xx",
            Self::Network {
                kind: NetworkKind::Timeout,
                ..
            } => "network_timeout",
            Self::Network {
                kind: NetworkKind::Dns,
                ..
            } => "network_dns",
            Self::Network {
                kind: NetworkKind::Tls,
                ..
            } => "network_tls",
            Self::Network { .. } => "network",
            Self::RateLimited { .. } => "rate_limited",
            Self::CircuitOpen { .. } => "circuit_open",
            Self::Parse { .. } => "parse_failed",
            Self::NotFound { .. } => "not_found",
            Self::Unsupported { .. } => "unsupported",
            Self::UpstreamChanged { .. } => "upstream_changed",
            Self::Storage {
                code: StorageCode::DiskFull,
                ..
            } => "disk_full",
            Self::Storage {
                code: StorageCode::PathInvalid,
                ..
            } => "path_invalid",
            Self::Storage {
                code: StorageCode::PermissionDenied,
                ..
            } => "permission_denied",
            Self::Storage { .. } => "storage",
        }
    }
}

pub type SourceResult<T> = Result<T, SchematicSourceError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forbidden_is_not_retryable_but_rate_limited_is() {
        assert!(
            !SchematicSourceError::http(403, "origin rejected").is_retryable()
        );
        assert!(SchematicSourceError::http(429, "slow down").is_retryable());
        assert!(SchematicSourceError::http(503, "unavailable").is_retryable());
        assert!(!SchematicSourceError::not_found("item").is_retryable());
    }

    #[test]
    fn upstream_change_is_distinct_from_network() {
        let changed = SchematicSourceError::upstream_changed("missing #items");
        assert_eq!(changed.code(), "upstream_changed");
        assert!(!changed.is_retryable());

        let network =
            SchematicSourceError::network(NetworkKind::Timeout, "timed out");
        assert_eq!(network.code(), "network_timeout");
        assert!(network.is_retryable());
    }

    #[test]
    fn status_codes_map_to_stable_codes() {
        assert_eq!(SchematicSourceError::http(403, "x").code(), "http_403");
        assert_eq!(SchematicSourceError::http(404, "x").code(), "http_404");
        assert_eq!(SchematicSourceError::http(500, "x").code(), "http_5xx");
    }
}

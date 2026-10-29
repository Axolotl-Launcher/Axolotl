//! Download attempt diagnostics and bounded retry history.

use crate::ErrorKind;
use crate::util::download::constants::{
    DIAGNOSTICS_API_RESPONSE_FAILURE, DIAGNOSTICS_BUDGET_EXHAUSTED,
    DIAGNOSTICS_CANCELLED, DIAGNOSTICS_CERTIFICATE, DIAGNOSTICS_CHECKSUM,
    DIAGNOSTICS_CONNECT, DIAGNOSTICS_CONTENT_RANGE, DIAGNOSTICS_DNS,
    DIAGNOSTICS_DNS_ONLY, DIAGNOSTICS_ENDPOINT_CHANGED,
    DIAGNOSTICS_HASH_MISMATCH, DIAGNOSTICS_I_O_FAILURE,
    DIAGNOSTICS_INSUFFICIENT_SAMPLE, DIAGNOSTICS_INTEGRITY,
    DIAGNOSTICS_INVALID_CERTIFICATE, DIAGNOSTICS_INVALID_ENCODING,
    DIAGNOSTICS_INVALID_RANGE, DIAGNOSTICS_IO,
    DIAGNOSTICS_JSON_VALIDATION_FAILED, DIAGNOSTICS_LOCAL_PRESSURE,
    DIAGNOSTICS_LOOKUP, DIAGNOSTICS_MEASURING,
    DIAGNOSTICS_N_OLDER_DIAGNOSTIC_DETAILS_OMITTED, DIAGNOSTICS_NETWORK,
    DIAGNOSTICS_NO_GAIN, DIAGNOSTICS_NO_RESPONSE_RECEIVED, DIAGNOSTICS_OTHER,
    DIAGNOSTICS_PROTOCOL, DIAGNOSTICS_PROXY_INELIGIBLE, DIAGNOSTICS_RANGE,
    DIAGNOSTICS_RANGE_UNSUPPORTED, DIAGNOSTICS_RESOLVE,
    DIAGNOSTICS_RESOURCE_WAIT, DIAGNOSTICS_ROLLBACK,
    DIAGNOSTICS_RULE_INELIGIBLE, DIAGNOSTICS_SELECTED,
    DIAGNOSTICS_SERVER_COOLDOWN, DIAGNOSTICS_SLOW, DIAGNOSTICS_SSL,
    DIAGNOSTICS_STALE_EPOCH, DIAGNOSTICS_STALL, DIAGNOSTICS_STREAM_RESET,
    DIAGNOSTICS_TIMEOUT, DIAGNOSTICS_TLS, DIAGNOSTICS_TRUNCATED,
    DIAGNOSTICS_VALIDATION, HTTP_SCHEME, MAX_DOWNLOAD_ATTEMPT_HISTORY,
    MAX_DOWNLOAD_DIAGNOSTIC_BYTES,
};
use crate::util::fetch::{
    DownloadRoute, DownloadRouteSource, ProxyPolicy, route_host,
    sanitize_url_for_log,
};
use reqwest::StatusCode;
use std::collections::VecDeque;

/// Stable local diagnostic codes; presentation text must not be used to
/// classify endpoint failures or decide protocol health.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EndpointReason {
    DnsOnly,
    RuleIneligible,
    InvalidCertificate,
    RangeUnsupported,
    InvalidRange,
    InvalidEncoding,
    Truncated,
    InsufficientSample,
    NoGain,
    Selected,
    Rollback,
    ProxyIneligible,
    BudgetExhausted,
    StaleEpoch,
    ServerCooldown,
    LocalPressure,
    Cancelled,
    Measuring,
}

impl EndpointReason {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::DnsOnly => DIAGNOSTICS_DNS_ONLY,
            Self::RuleIneligible => DIAGNOSTICS_RULE_INELIGIBLE,
            Self::InvalidCertificate => DIAGNOSTICS_INVALID_CERTIFICATE,
            Self::RangeUnsupported => DIAGNOSTICS_RANGE_UNSUPPORTED,
            Self::InvalidRange => DIAGNOSTICS_INVALID_RANGE,
            Self::InvalidEncoding => DIAGNOSTICS_INVALID_ENCODING,
            Self::Truncated => DIAGNOSTICS_TRUNCATED,
            Self::InsufficientSample => DIAGNOSTICS_INSUFFICIENT_SAMPLE,
            Self::NoGain => DIAGNOSTICS_NO_GAIN,
            Self::Selected => DIAGNOSTICS_SELECTED,
            Self::Rollback => DIAGNOSTICS_ROLLBACK,
            Self::ProxyIneligible => DIAGNOSTICS_PROXY_INELIGIBLE,
            Self::BudgetExhausted => DIAGNOSTICS_BUDGET_EXHAUSTED,
            Self::StaleEpoch => DIAGNOSTICS_STALE_EPOCH,
            Self::ServerCooldown => DIAGNOSTICS_SERVER_COOLDOWN,
            Self::LocalPressure => DIAGNOSTICS_LOCAL_PRESSURE,
            Self::Cancelled => DIAGNOSTICS_CANCELLED,
            Self::Measuring => DIAGNOSTICS_MEASURING,
        }
    }
}

#[derive(Debug)]
pub(crate) struct DownloadAttemptDiagnostic {
    pub(crate) attempt: usize,
    pub(crate) source: DownloadRouteSource,
    pub(crate) url: String,
    pub(crate) proxy: ProxyPolicy,
    pub(crate) dns_candidates: Vec<std::net::IpAddr>,
    pub(crate) remote_addr: Option<std::net::SocketAddr>,
    pub(crate) http_version: Option<reqwest::Version>,
    pub(crate) status: Option<u16>,
    pub(crate) category: &'static str,
    pub(crate) decision: &'static str,
    pub(crate) detail: String,
}

pub(crate) fn bounded_diagnostic_text(
    value: impl AsRef<str>,
    max_chars: usize,
) -> String {
    value.as_ref().chars().take(max_chars).collect()
}

pub(crate) fn download_error_category(error: &crate::Error) -> &'static str {
    match error.raw.as_ref() {
        ErrorKind::DownloadEndpointChanged => DIAGNOSTICS_ENDPOINT_CHANGED,
        ErrorKind::DownloadTransferSlow => DIAGNOSTICS_SLOW,
        ErrorKind::DownloadStreamStalled { .. } => DIAGNOSTICS_STALL,
        ErrorKind::DownloadH2Error(source) => {
            if source.is_reset() {
                DIAGNOSTICS_STREAM_RESET
            } else {
                DIAGNOSTICS_PROTOCOL
            }
        }
        ErrorKind::FetchError(source) => {
            let detail = format!("{source:?}").to_ascii_lowercase();
            if source.status().is_some() {
                HTTP_SCHEME
            } else if source.is_timeout() && source.is_body() {
                DIAGNOSTICS_STALL
            } else if source.is_timeout() {
                DIAGNOSTICS_TIMEOUT
            } else if source.is_connect()
                && [DIAGNOSTICS_CERTIFICATE, DIAGNOSTICS_TLS, DIAGNOSTICS_SSL]
                    .iter()
                    .any(|needle| detail.contains(needle))
            {
                DIAGNOSTICS_TLS
            } else if source.is_connect()
                && [DIAGNOSTICS_DNS, DIAGNOSTICS_LOOKUP, DIAGNOSTICS_RESOLVE]
                    .iter()
                    .any(|needle| detail.contains(needle))
            {
                DIAGNOSTICS_DNS
            } else if source.is_connect() {
                DIAGNOSTICS_CONNECT
            } else {
                DIAGNOSTICS_NETWORK
            }
        }
        ErrorKind::NetworkError(message) => {
            if message.contains(DIAGNOSTICS_NO_RESPONSE_RECEIVED) {
                DIAGNOSTICS_TIMEOUT
            } else {
                DIAGNOSTICS_NETWORK
            }
        }
        ErrorKind::DownloadResourceWaitTimeout(_) => DIAGNOSTICS_RESOURCE_WAIT,
        ErrorKind::LabrinthError(_) | ErrorKind::HttpError { .. } => {
            HTTP_SCHEME
        }
        ErrorKind::HashError(_, _) => DIAGNOSTICS_INTEGRITY,
        ErrorKind::JSONError(_) => DIAGNOSTICS_INTEGRITY,
        ErrorKind::IOError(_) | ErrorKind::StdIOError(_) => DIAGNOSTICS_IO,
        ErrorKind::OtherError(message) => {
            let message = message.to_ascii_lowercase();
            if message.contains(DIAGNOSTICS_CONTENT_RANGE)
                || message.contains(DIAGNOSTICS_RANGE)
            {
                DIAGNOSTICS_RANGE
            } else if message.contains(DIAGNOSTICS_INTEGRITY)
                || message.contains(DIAGNOSTICS_CHECKSUM)
                || message.contains(DIAGNOSTICS_VALIDATION)
            {
                DIAGNOSTICS_INTEGRITY
            } else if message.contains(DIAGNOSTICS_TRUNCATED) {
                DIAGNOSTICS_STALL
            } else {
                DIAGNOSTICS_OTHER
            }
        }
        _ => DIAGNOSTICS_OTHER,
    }
}

pub(crate) fn download_error_detail(error: &crate::Error) -> String {
    match error.raw.as_ref() {
        ErrorKind::FetchError(source) => source.status().map_or_else(
            || {
                let mut detail =
                    format!("{} failure", download_error_category(error));
                let mut cause = std::error::Error::source(source);
                while let Some(error) = cause {
                    if let Some(io) = error.downcast_ref::<std::io::Error>() {
                        // Error messages can contain proxy credentials or URLs.
                        // Keep only structured, non-sensitive OS diagnostics.
                        detail.push_str(&format!("; io={:?}", io.kind()));
                        if let Some(code) = io.raw_os_error() {
                            detail.push_str(&format!("; os={code}"));
                        }
                        break;
                    }
                    cause = error.source();
                }
                detail
            },
            |status| format!("HTTP {}", status.as_u16()),
        ),
        ErrorKind::LabrinthError(error) => error.status.map_or_else(
            || DIAGNOSTICS_API_RESPONSE_FAILURE.to_string(),
            |status| format!("HTTP {status}"),
        ),
        ErrorKind::HttpError { status, .. } => format!("HTTP {status}"),
        ErrorKind::HashError(_, _) => DIAGNOSTICS_HASH_MISMATCH.to_string(),
        ErrorKind::JSONError(_) => {
            DIAGNOSTICS_JSON_VALIDATION_FAILED.to_string()
        }
        ErrorKind::IOError(_) | ErrorKind::StdIOError(_) => {
            DIAGNOSTICS_I_O_FAILURE.to_string()
        }
        ErrorKind::OtherError(_) | ErrorKind::NetworkError(_) => {
            format!("{} failure", download_error_category(error))
        }
        _ => bounded_diagnostic_text(error.to_string(), 256),
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn push_download_attempt_diagnostic(
    history: &mut VecDeque<DownloadAttemptDiagnostic>,
    route: &DownloadRoute,
    attempt: usize,
    category: &'static str,
    decision: &'static str,
    detail: impl AsRef<str>,
    status: Option<StatusCode>,
    remote_addr: Option<std::net::SocketAddr>,
    http_version: Option<reqwest::Version>,
) {
    if history.len() == MAX_DOWNLOAD_ATTEMPT_HISTORY {
        history.pop_front();
    }
    let dns_candidates = route_host(route)
        .map(|host| {
            super::proxy_context::resolver(route.proxy)
                .resolved_addresses(&host)
        })
        .unwrap_or_default()
        .into_iter()
        .take(8)
        .collect();
    history.push_back(DownloadAttemptDiagnostic {
        attempt,
        source: route.source,
        url: bounded_diagnostic_text(sanitize_url_for_log(&route.url), 512),
        proxy: route.proxy,
        dns_candidates,
        remote_addr,
        http_version,
        status: status.map(|status| status.as_u16()),
        category,
        decision,
        detail: bounded_diagnostic_text(detail, 256),
    });
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn record_download_attempt_failure(
    history: &mut VecDeque<DownloadAttemptDiagnostic>,
    route: &DownloadRoute,
    attempt: usize,
    error: &crate::Error,
    decision: &'static str,
    status: Option<StatusCode>,
    remote_addr: Option<std::net::SocketAddr>,
    http_version: Option<reqwest::Version>,
) {
    push_download_attempt_diagnostic(
        history,
        route,
        attempt,
        download_error_category(error),
        decision,
        download_error_detail(error),
        status,
        remote_addr,
        http_version,
    );
}

pub(crate) fn attach_download_attempt_history(
    error: crate::Error,
    history: &VecDeque<DownloadAttemptDiagnostic>,
    attempts: usize,
    attempt_budget: usize,
) -> crate::Error {
    let mut context = format!(
        "Download failed after {attempts}/{attempt_budget} attempts. Recent attempt history:"
    );
    for item in history {
        let line = format!(
            "\n- attempt={}; source={}; url={}; proxy={:?}; dns={:?}; remote={:?}; http={:?}; status={:?}; category={}; decision={}; detail={}",
            item.attempt,
            item.source.as_str(),
            item.url,
            item.proxy,
            item.dns_candidates,
            item.remote_addr,
            item.http_version,
            item.status,
            item.category,
            item.decision,
            item.detail,
        );
        if context.len() + line.len() > MAX_DOWNLOAD_DIAGNOSTIC_BYTES {
            context.push_str(DIAGNOSTICS_N_OLDER_DIAGNOSTIC_DETAILS_OMITTED);
            break;
        }
        context.push_str(&line);
    }
    error.with_context(context)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn connection_details_keep_os_errors_without_request_secrets() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        let error = reqwest::Client::builder()
            .no_proxy()
            .connect_timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap()
            .get(format!("http://{address}/?token=private-test-token"))
            .send()
            .await
            .unwrap_err();
        let detail = download_error_detail(&error.into());
        assert!(detail.contains("io=ConnectionRefused"), "{detail}");
        assert!(detail.contains("os="), "{detail}");
        assert!(!detail.contains("private-test-token"));
        assert!(!detail.contains("127.0.0.1"));
    }
}

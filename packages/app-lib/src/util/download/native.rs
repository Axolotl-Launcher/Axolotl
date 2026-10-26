//! Policy boundary for the native download engine.

use crate::util::download::constants::{
    ALL_PROXY_ENV_LOWER, ALL_PROXY_ENV_UPPER, H2_LARGE_MIN_SUCCESS_SAMPLES,
    H2_LARGE_PREFERRED_SPEED_RATIO, H2_MIN_SUCCESS_SAMPLES,
    H2_PREFERRED_SPEED_RATIO, HTTP_PROXY_ENV_LOWER, HTTP_PROXY_ENV_UPPER,
    HTTPS_PROXY_ENV_LOWER, HTTPS_PROXY_ENV_UPPER,
    NATIVE_AUTHORITY_IS_TEMPORARILY_USING_HTTP_1_1, NATIVE_AUTOCONFIGURL,
    NATIVE_CERTIFICATE_POLICY_REQUIRES_THE_REQWEST_TRANSPORT,
    NATIVE_CONFIGURED_PROXY_REQUIRES_THE_REQWEST_TRANSPORT,
    NATIVE_DOWNLOAD_SOURCE_HAS_BEEN_RETIRED, NATIVE_PROXYENABLE,
    NATIVE_SOFTWARE_MICROSOFT_WINDOWS_CURRENTVERSION_INTERNET_SETTINGS,
    NATIVE_SYSTEM_PROXY_REQUIRES_THE_REQWEST_TRANSPORT,
};

use crate::util::fetch::{DownloadRoute, ProxyPolicy};

use super::constants::{
    H2_LARGE_FILE_LIMIT as LARGE_FILE_LIMIT,
    H2_MEDIUM_FILE_LIMIT as MEDIUM_FILE_LIMIT,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NativeH2Policy {
    /// Whether this selection may establish the authority's first H2
    /// connection instead of requiring an already-warm multiplexed session.
    pub(crate) allow_cold_connection: bool,
    pub(crate) abort_if_slow: bool,
    pub(crate) expected_speed: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NativeH2IneligibleReason {
    RetiredSource,
    Http1Fallback,
    SystemProxy,
    ConfiguredProxy,
    CertificatePolicy,
}

impl NativeH2IneligibleReason {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::RetiredSource => NATIVE_DOWNLOAD_SOURCE_HAS_BEEN_RETIRED,
            Self::Http1Fallback => {
                NATIVE_AUTHORITY_IS_TEMPORARILY_USING_HTTP_1_1
            }
            Self::SystemProxy => {
                NATIVE_SYSTEM_PROXY_REQUIRES_THE_REQWEST_TRANSPORT
            }
            Self::ConfiguredProxy => {
                NATIVE_CONFIGURED_PROXY_REQUIRES_THE_REQWEST_TRANSPORT
            }
            Self::CertificatePolicy => {
                NATIVE_CERTIFICATE_POLICY_REQUIRES_THE_REQWEST_TRANSPORT
            }
        }
    }
}

pub(crate) fn h2_ineligible_reason(
    route: &DownloadRoute,
) -> Option<NativeH2IneligibleReason> {
    if url::Url::parse(&route.url)
        .is_ok_and(|url| super::retired_sources::is_retired(&url))
    {
        return Some(NativeH2IneligibleReason::RetiredSource);
    }
    let authority = crate::util::fetch::url_authority(&route.url)?;
    if crate::util::fetch::authority_uses_http1_fallback_for(
        &authority,
        route.proxy,
    ) {
        return Some(NativeH2IneligibleReason::Http1Fallback);
    }
    if let Some(clients) = super::proxy_context::clients() {
        if clients.force_http1 {
            return Some(NativeH2IneligibleReason::Http1Fallback);
        }
        if clients.ignore_ssl_errors {
            return Some(NativeH2IneligibleReason::CertificatePolicy);
        }
        if route.proxy == ProxyPolicy::System {
            match clients.proxy.mode {
                crate::util::proxy::ProxyMode::None => return None,
                crate::util::proxy::ProxyMode::Custom => {
                    return Some(NativeH2IneligibleReason::ConfiguredProxy);
                }
                crate::util::proxy::ProxyMode::System => {}
            }
        }
    }
    if route.proxy == ProxyPolicy::System && system_proxy_configured() {
        return Some(NativeH2IneligibleReason::SystemProxy);
    }
    None
}

pub(crate) fn explicit_h2_policy(
    route: &DownloadRoute,
) -> Option<NativeH2Policy> {
    let authority = crate::util::fetch::url_authority(&route.url)?;
    if !super::native_reputation::transport_enabled(
        &authority,
        route.proxy,
        super::native_reputation::NativeTransport::H2MultiRange,
    ) {
        return None;
    }
    h2_ineligible_reason(route)
        .is_none()
        .then_some(NativeH2Policy {
            allow_cold_connection: false,
            abort_if_slow: true,
            expected_speed: None,
        })
}

pub(crate) async fn h2_policy(
    route: &DownloadRoute,
    size: Option<u64>,
) -> Option<NativeH2Policy> {
    if h2_ineligible_reason(route).is_some() {
        return None;
    }
    let Some(size) = size else {
        return Some(NativeH2Policy {
            allow_cold_connection: true,
            abort_if_slow: true,
            expected_speed: None,
        });
    };
    let authority = crate::util::fetch::url_authority(&route.url)?;
    let h2 = super::native_reputation::get_transport(
        &authority,
        route.proxy,
        super::native_reputation::NativeTransport::H2Single,
    );
    let multi_range = super::native_reputation::get_transport(
        &authority,
        route.proxy,
        super::native_reputation::NativeTransport::Http1MultiRange,
    );
    let expected_speed = h2
        .and_then(|health| health.throughput_bps)
        .map(|speed| speed.min(u64::MAX as f64) as u64);
    if size < MEDIUM_FILE_LIMIT {
        // The connection slot serializes the first handshake, so a batch of
        // medium files shares one cold-start attempt instead of stampeding the
        // CDN with independent TCP/TLS connections.
        return Some(NativeH2Policy {
            allow_cold_connection: true,
            abort_if_slow: true,
            expected_speed,
        });
    }
    if !super::h2_pool::has_live_connection(route).await {
        return None;
    }
    let h2 =
        h2.filter(|health| health.success_samples >= H2_MIN_SUCCESS_SAMPLES)?;
    let h2_speed = h2.throughput_bps?;
    let range_speed = multi_range.and_then(|health| health.throughput_bps);
    if size < LARGE_FILE_LIMIT {
        let h2_preferred = range_speed
            .map(|range_speed| {
                h2_speed >= range_speed * H2_PREFERRED_SPEED_RATIO
            })
            .unwrap_or(true);
        return h2_preferred.then_some(NativeH2Policy {
            allow_cold_connection: false,
            abort_if_slow: true,
            expected_speed: Some(h2_speed.min(u64::MAX as f64) as u64),
        });
    }
    let range = multi_range
        .filter(|health| health.success_samples >= H2_MIN_SUCCESS_SAMPLES)?;
    let range_speed = range.throughput_bps?;
    (h2.success_samples >= H2_LARGE_MIN_SUCCESS_SAMPLES
        && h2_speed >= range_speed * H2_LARGE_PREFERRED_SPEED_RATIO)
        .then_some(NativeH2Policy {
            allow_cold_connection: false,
            abort_if_slow: true,
            expected_speed: Some(h2_speed.min(u64::MAX as f64) as u64),
        })
}

pub(crate) fn system_proxy_configured() -> bool {
    let environment_proxy = [
        HTTPS_PROXY_ENV_UPPER,
        HTTPS_PROXY_ENV_LOWER,
        HTTP_PROXY_ENV_UPPER,
        HTTP_PROXY_ENV_LOWER,
        ALL_PROXY_ENV_UPPER,
        ALL_PROXY_ENV_LOWER,
    ]
    .into_iter()
    .any(|name| std::env::var_os(name).is_some_and(|value| !value.is_empty()));
    environment_proxy || platform_proxy_configured()
}

#[cfg(windows)]
fn platform_proxy_configured() -> bool {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;

    let Ok(settings) = RegKey::predef(HKEY_CURRENT_USER).open_subkey(
        NATIVE_SOFTWARE_MICROSOFT_WINDOWS_CURRENTVERSION_INTERNET_SETTINGS,
    ) else {
        return false;
    };
    let enabled = settings
        .get_value::<u32, _>(NATIVE_PROXYENABLE)
        .is_ok_and(|value| value != 0);
    let auto_configured = settings
        .get_value::<String, _>(NATIVE_AUTOCONFIGURL)
        .is_ok_and(|value| !value.trim().is_empty());
    enabled || auto_configured
}

#[cfg(not(windows))]
fn platform_proxy_configured() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::fetch::{DownloadRouteSource, ProxyPolicy};

    fn route(proxy: ProxyPolicy) -> DownloadRoute {
        DownloadRoute {
            url: "https://native-policy.example/file".to_string(),
            source: DownloadRouteSource::Official,
            is_mirror: false,
            allow_sensitive_headers: true,
            supports_range: true,
            proxy,
        }
    }

    #[test]
    fn direct_routes_ignore_system_proxy_environment() {
        assert_ne!(
            h2_ineligible_reason(&route(ProxyPolicy::Direct)),
            Some(NativeH2IneligibleReason::SystemProxy)
        );
    }

    #[test]
    fn explicit_range_policy_requires_warm_connection() {
        let policy = explicit_h2_policy(&route(ProxyPolicy::Direct)).unwrap();
        assert!(!policy.allow_cold_connection);
    }

    #[tokio::test]
    async fn medium_file_can_establish_a_cold_h2_connection() {
        assert_eq!(
            h2_policy(&route(ProxyPolicy::Direct), Some(2 * 1024 * 1024),)
                .await,
            Some(NativeH2Policy {
                allow_cold_connection: true,
                abort_if_slow: true,
                expected_speed: None,
            })
        );
    }

    #[tokio::test]
    async fn small_file_can_establish_h2_without_history() {
        assert_eq!(
            h2_policy(&route(ProxyPolicy::Direct), Some(1024 * 1024),).await,
            Some(NativeH2Policy {
                allow_cold_connection: true,
                abort_if_slow: true,
                expected_speed: None,
            })
        );
    }

    #[tokio::test]
    async fn large_file_still_requires_a_warm_h2_connection_and_history() {
        assert!(
            h2_policy(&route(ProxyPolicy::Direct), Some(32 * 1024 * 1024),)
                .await
                .is_none()
        );
    }
}

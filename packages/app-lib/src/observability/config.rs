//! Environment-driven configuration for OTLP export.
//!
//! The launcher reads the standard OpenTelemetry exporter variables, but is
//! deliberately stricter than the SDK defaults: export only starts when
//! `AXOLOTL_OBSERVABILITY` opts in, only `http/protobuf` is accepted, and plain
//! HTTP endpoints are limited to loopback hosts so credentials and telemetry
//! never travel unencrypted to a remote collector.

use std::fmt;
use std::time::Duration;

const ENABLE_VAR: &str = "AXOLOTL_OBSERVABILITY";
const LOGS_VAR: &str = "AXOLOTL_OBSERVABILITY_LOGS";
const RAW_LOGS_VAR: &str = "AXOLOTL_OBSERVABILITY_RAW_LOGS";

const GENERAL_ENDPOINT_VAR: &str = "OTEL_EXPORTER_OTLP_ENDPOINT";
const GENERAL_HEADERS_VAR: &str = "OTEL_EXPORTER_OTLP_HEADERS";
const GENERAL_PROTOCOL_VAR: &str = "OTEL_EXPORTER_OTLP_PROTOCOL";
const GENERAL_TIMEOUT_VAR: &str = "OTEL_EXPORTER_OTLP_TIMEOUT";

const SERVICE_NAME_VAR: &str = "OTEL_SERVICE_NAME";
const RESOURCE_ATTRIBUTES_VAR: &str = "OTEL_RESOURCE_ATTRIBUTES";
const SAMPLER_VAR: &str = "OTEL_TRACES_SAMPLER";
const SAMPLER_ARG_VAR: &str = "OTEL_TRACES_SAMPLER_ARG";
const METRIC_INTERVAL_VAR: &str = "OTEL_METRIC_EXPORT_INTERVAL";

const BSP_MAX_QUEUE_SIZE_VAR: &str = "OTEL_BSP_MAX_QUEUE_SIZE";
const BSP_MAX_EXPORT_BATCH_SIZE_VAR: &str = "OTEL_BSP_MAX_EXPORT_BATCH_SIZE";
const BSP_SCHEDULE_DELAY_VAR: &str = "OTEL_BSP_SCHEDULE_DELAY";

const DEFAULT_SERVICE_NAME: &str = "axolotl-launcher";
const DEFAULT_SAMPLE_RATIO: f64 = 0.1;
const DEFAULT_TIMEOUT_MS: u64 = 10_000;
const DEFAULT_BSP_MAX_QUEUE_SIZE: usize = 2_048;
const DEFAULT_BSP_MAX_EXPORT_BATCH_SIZE: usize = 512;
const DEFAULT_BSP_SCHEDULE_DELAY_MS: u64 = 5_000;
const DEFAULT_METRIC_INTERVAL_MS: u64 = 60_000;

const MIN_QUEUE_SIZE: usize = 1;
const MAX_QUEUE_SIZE: usize = 65_536;
const MIN_SCHEDULE_DELAY_MS: u64 = 100;
const MAX_SCHEDULE_DELAY_MS: u64 = 60_000;
const MIN_TIMEOUT_MS: u64 = 100;
const MAX_TIMEOUT_MS: u64 = 120_000;
const MIN_METRIC_INTERVAL_MS: u64 = 1_000;
const MAX_METRIC_INTERVAL_MS: u64 = 600_000;

/// Signals the launcher can export. Each has its own standard OTLP variables
/// that override the shared ones.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Signal {
    Traces,
    Metrics,
    Logs,
}

impl Signal {
    const fn endpoint_var(self) -> &'static str {
        match self {
            Self::Traces => "OTEL_EXPORTER_OTLP_TRACES_ENDPOINT",
            Self::Metrics => "OTEL_EXPORTER_OTLP_METRICS_ENDPOINT",
            Self::Logs => "OTEL_EXPORTER_OTLP_LOGS_ENDPOINT",
        }
    }

    const fn headers_var(self) -> &'static str {
        match self {
            Self::Traces => "OTEL_EXPORTER_OTLP_TRACES_HEADERS",
            Self::Metrics => "OTEL_EXPORTER_OTLP_METRICS_HEADERS",
            Self::Logs => "OTEL_EXPORTER_OTLP_LOGS_HEADERS",
        }
    }

    const fn protocol_var(self) -> &'static str {
        match self {
            Self::Traces => "OTEL_EXPORTER_OTLP_TRACES_PROTOCOL",
            Self::Metrics => "OTEL_EXPORTER_OTLP_METRICS_PROTOCOL",
            Self::Logs => "OTEL_EXPORTER_OTLP_LOGS_PROTOCOL",
        }
    }

    const fn timeout_var(self) -> &'static str {
        match self {
            Self::Traces => "OTEL_EXPORTER_OTLP_TRACES_TIMEOUT",
            Self::Metrics => "OTEL_EXPORTER_OTLP_METRICS_TIMEOUT",
            Self::Logs => "OTEL_EXPORTER_OTLP_LOGS_TIMEOUT",
        }
    }

    const fn signal_path(self) -> &'static str {
        match self {
            Self::Traces => "/v1/traces",
            Self::Metrics => "/v1/metrics",
            Self::Logs => "/v1/logs",
        }
    }
}

/// Resolved sampler selection from the standard OTEL variables.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SamplerMode {
    AlwaysOn,
    AlwaysOff,
    TraceIdRatio,
    ParentBasedAlwaysOn,
    ParentBasedAlwaysOff,
    ParentBasedTraceIdRatio,
}

impl SamplerMode {
    const fn uses_ratio(self) -> bool {
        matches!(self, Self::TraceIdRatio | Self::ParentBasedTraceIdRatio)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SamplerConfig {
    pub(crate) mode: SamplerMode,
    pub(crate) ratio: f64,
}

/// Resolved exporter settings shared by one signal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SignalConfig {
    /// Fully resolved endpoint, including the `/v1/<signal>` path when the
    /// generic `OTEL_EXPORTER_OTLP_ENDPOINT` variable was used.
    pub(crate) endpoint: String,
    pub(crate) headers: Vec<(String, String)>,
    pub(crate) timeout: Duration,
}

/// Span batch processor limits, clamped to values that keep a crash from
/// losing unbounded data or stalling the exporter thread for minutes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BatchSettings {
    pub(crate) max_queue_size: usize,
    pub(crate) max_export_batch_size: usize,
    pub(crate) scheduled_delay: Duration,
}

/// Validated observability configuration.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ObservabilityConfig {
    pub(crate) service_name: String,
    pub(crate) resource_attributes: Vec<(String, String)>,
    pub(crate) traces: SignalConfig,
    pub(crate) metrics: SignalConfig,
    pub(crate) logs: Option<SignalConfig>,
    pub(crate) sampler: SamplerConfig,
    pub(crate) batch: BatchSettings,
    pub(crate) metric_interval: Duration,
    pub(crate) raw_logs: bool,
}

/// A configuration problem. Messages name the offending variable but never
/// echo its value, which may contain credentials.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigError {
    message: String,
}

impl ConfigError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ConfigError {}

impl ObservabilityConfig {
    /// Read configuration from the process environment.
    pub(crate) fn from_env() -> Result<Option<Self>, ConfigError> {
        Self::from_env_with(|key| std::env::var(key).ok())
    }

    /// Read configuration from an injectable environment, for tests.
    pub(crate) fn from_env_with(
        read: impl Fn(&str) -> Option<String>,
    ) -> Result<Option<Self>, ConfigError> {
        if !flag_enabled(env(&read, ENABLE_VAR).as_deref()) {
            return Ok(None);
        }

        let logs_enabled = flag_enabled(env(&read, LOGS_VAR).as_deref());
        let raw_logs =
            logs_enabled && flag_enabled(env(&read, RAW_LOGS_VAR).as_deref());

        for signal in [Signal::Traces, Signal::Metrics] {
            validate_protocol(&read, signal)?;
        }
        if logs_enabled {
            validate_protocol(&read, Signal::Logs)?;
        }

        let traces = resolve_signal(&read, Signal::Traces)?
            .ok_or_else(|| missing_endpoint_error(Signal::Traces))?;
        let metrics = resolve_signal(&read, Signal::Metrics)?
            .ok_or_else(|| missing_endpoint_error(Signal::Metrics))?;
        let logs = if logs_enabled {
            Some(
                resolve_signal(&read, Signal::Logs)?
                    .ok_or_else(|| missing_endpoint_error(Signal::Logs))?,
            )
        } else {
            None
        };

        let max_queue_size = parse_usize(
            &read,
            BSP_MAX_QUEUE_SIZE_VAR,
            DEFAULT_BSP_MAX_QUEUE_SIZE,
        )?
        .clamp(MIN_QUEUE_SIZE, MAX_QUEUE_SIZE);
        let max_export_batch_size = parse_usize(
            &read,
            BSP_MAX_EXPORT_BATCH_SIZE_VAR,
            DEFAULT_BSP_MAX_EXPORT_BATCH_SIZE,
        )?
        .clamp(MIN_QUEUE_SIZE, max_queue_size);
        let scheduled_delay = Duration::from_millis(
            parse_u64(
                &read,
                BSP_SCHEDULE_DELAY_VAR,
                DEFAULT_BSP_SCHEDULE_DELAY_MS,
            )?
            .clamp(MIN_SCHEDULE_DELAY_MS, MAX_SCHEDULE_DELAY_MS),
        );

        let metric_interval = Duration::from_millis(
            parse_u64(&read, METRIC_INTERVAL_VAR, DEFAULT_METRIC_INTERVAL_MS)?
                .clamp(MIN_METRIC_INTERVAL_MS, MAX_METRIC_INTERVAL_MS),
        );

        let service_name = env(&read, SERVICE_NAME_VAR)
            .unwrap_or_else(|| DEFAULT_SERVICE_NAME.to_owned());
        let mut resource_attributes =
            parse_key_values(&read, RESOURCE_ATTRIBUTES_VAR)?;
        resource_attributes.retain(|(key, _)| key != "service.name");

        Ok(Some(Self {
            service_name,
            resource_attributes,
            traces,
            metrics,
            logs,
            sampler: parse_sampler(&read)?,
            batch: BatchSettings {
                max_queue_size,
                max_export_batch_size,
                scheduled_delay,
            },
            metric_interval,
            raw_logs,
        }))
    }
}

fn resolve_signal(
    read: &impl Fn(&str) -> Option<String>,
    signal: Signal,
) -> Result<Option<SignalConfig>, ConfigError> {
    let Some(endpoint) = resolve_endpoint(read, signal)? else {
        return Ok(None);
    };
    validate_endpoint(&endpoint, signal.endpoint_var())?;

    let headers = match env(read, signal.headers_var()) {
        Some(headers) => parse_headers(&headers, signal.headers_var())?,
        None => parse_key_values(read, GENERAL_HEADERS_VAR)?,
    };

    let timeout = Duration::from_millis(
        parse_u64(
            read,
            signal.timeout_var(),
            parse_u64(read, GENERAL_TIMEOUT_VAR, DEFAULT_TIMEOUT_MS)?,
        )?
        .clamp(MIN_TIMEOUT_MS, MAX_TIMEOUT_MS),
    );

    Ok(Some(SignalConfig {
        endpoint,
        headers,
        timeout,
    }))
}

fn resolve_endpoint(
    read: &impl Fn(&str) -> Option<String>,
    signal: Signal,
) -> Result<Option<String>, ConfigError> {
    if let Some(endpoint) = env(read, signal.endpoint_var()) {
        return Ok(Some(endpoint));
    }
    let Some(base) = env(read, GENERAL_ENDPOINT_VAR) else {
        return Ok(None);
    };
    Ok(Some(append_signal_path(&base, signal.signal_path())?))
}

fn append_signal_path(
    base: &str,
    signal_path: &str,
) -> Result<String, ConfigError> {
    let mut url = url::Url::parse(base).map_err(|_| {
        ConfigError::new("OTEL_EXPORTER_OTLP_ENDPOINT must be an absolute URL")
    })?;
    let path = url.path().trim_end_matches('/').to_owned();
    if !path.ends_with(signal_path) {
        url.set_path(&format!("{path}{signal_path}"));
    }
    Ok(url.to_string())
}

fn validate_protocol(
    read: &impl Fn(&str) -> Option<String>,
    signal: Signal,
) -> Result<(), ConfigError> {
    let (var, value) = match env(read, signal.protocol_var()) {
        Some(value) => (signal.protocol_var(), value),
        None => match env(read, GENERAL_PROTOCOL_VAR) {
            Some(value) => (GENERAL_PROTOCOL_VAR, value),
            None => return Ok(()),
        },
    };
    if value.eq_ignore_ascii_case("http/protobuf") {
        Ok(())
    } else {
        Err(ConfigError::new(format!(
            "{var} must be http/protobuf; the launcher does not support {value:?}"
        )))
    }
}

fn missing_endpoint_error(signal: Signal) -> ConfigError {
    ConfigError::new(format!(
        "AXOLOTL_OBSERVABILITY is enabled but neither {GENERAL_ENDPOINT_VAR} nor {} is set",
        signal.endpoint_var()
    ))
}

fn validate_endpoint(endpoint: &str, var: &str) -> Result<(), ConfigError> {
    let Some((scheme, rest)) = endpoint.split_once("://") else {
        return Err(ConfigError::new(format!(
            "{var} must be an absolute http(s) URL"
        )));
    };
    let scheme = scheme.to_ascii_lowercase();
    if scheme != "https" && scheme != "http" {
        return Err(ConfigError::new(format!(
            "{var} must use the http or https scheme"
        )));
    }

    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    if authority.is_empty() {
        return Err(ConfigError::new(format!("{var} is missing a host")));
    }
    if authority.contains('@') {
        return Err(ConfigError::new(format!(
            "{var} must not contain userinfo"
        )));
    }

    let host = if let Some(rest) = authority.strip_prefix('[') {
        rest.split(']').next().unwrap_or_default()
    } else {
        authority
            .rsplit_once(':')
            .map_or(authority, |(host, _)| host)
    };
    if host.is_empty() {
        return Err(ConfigError::new(format!("{var} is missing a host")));
    }
    if scheme == "http" && !is_loopback_host(host) {
        return Err(ConfigError::new(format!(
            "{var} must use https unless the host is localhost"
        )));
    }
    Ok(())
}

fn is_loopback_host(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host == "127.0.0.1"
        || host == "::1"
}

fn parse_sample_ratio(
    read: &impl Fn(&str) -> Option<String>,
) -> Result<f64, ConfigError> {
    let Some(value) = env(read, SAMPLER_ARG_VAR) else {
        return Ok(DEFAULT_SAMPLE_RATIO);
    };
    let ratio = value.parse::<f64>().map_err(|_| {
        ConfigError::new(format!("{SAMPLER_ARG_VAR} must be a number"))
    })?;
    if !ratio.is_finite() {
        return Err(ConfigError::new(format!(
            "{SAMPLER_ARG_VAR} must be finite"
        )));
    }
    Ok(ratio.clamp(0.0, 1.0))
}

fn parse_sampler(
    read: &impl Fn(&str) -> Option<String>,
) -> Result<SamplerConfig, ConfigError> {
    let raw = env(read, SAMPLER_VAR);
    let mode = match raw
        .as_deref()
        .map(str::trim)
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        None | Some("parentbased_traceidratio") => {
            SamplerMode::ParentBasedTraceIdRatio
        }
        Some("always_on") => SamplerMode::AlwaysOn,
        Some("always_off") => SamplerMode::AlwaysOff,
        Some("traceidratio") => SamplerMode::TraceIdRatio,
        Some("parentbased_always_on") => SamplerMode::ParentBasedAlwaysOn,
        Some("parentbased_always_off") => SamplerMode::ParentBasedAlwaysOff,
        Some(value) => {
            return Err(ConfigError::new(format!(
                "{SAMPLER_VAR} has an unsupported sampler {value:?}"
            )));
        }
    };
    let ratio = if mode.uses_ratio() {
        parse_sample_ratio(read)?
    } else {
        DEFAULT_SAMPLE_RATIO
    };
    Ok(SamplerConfig { mode, ratio })
}

fn parse_headers(
    raw: &str,
    var: &str,
) -> Result<Vec<(String, String)>, ConfigError> {
    let mut headers = Vec::new();
    for pair in raw.split(',') {
        let pair = pair.trim();
        if pair.is_empty() {
            continue;
        }
        let Some((key, value)) = pair.split_once('=') else {
            return Err(ConfigError::new(format!(
                "{var} must contain comma separated key=value pairs"
            )));
        };
        let key = key.trim();
        if key.is_empty() {
            return Err(ConfigError::new(format!(
                "{var} contains an empty header name"
            )));
        }
        let key = urlencoding::decode(key).map_err(|_| {
            ConfigError::new(format!("{var} contains an invalid header name"))
        })?;
        let value = urlencoding::decode(value.trim()).map_err(|_| {
            ConfigError::new(format!("{var} contains an invalid header value"))
        })?;
        headers.push((key.into_owned(), value.into_owned()));
    }
    Ok(headers)
}

fn parse_key_values(
    read: &impl Fn(&str) -> Option<String>,
    var: &str,
) -> Result<Vec<(String, String)>, ConfigError> {
    match env(read, var) {
        Some(raw) => parse_headers(&raw, var),
        None => Ok(Vec::new()),
    }
}

fn parse_u64(
    read: &impl Fn(&str) -> Option<String>,
    var: &str,
    default: u64,
) -> Result<u64, ConfigError> {
    match env(read, var) {
        Some(value) => value.parse::<u64>().map_err(|_| {
            ConfigError::new(format!("{var} must be a whole number"))
        }),
        None => Ok(default),
    }
}

fn parse_usize(
    read: &impl Fn(&str) -> Option<String>,
    var: &str,
    default: usize,
) -> Result<usize, ConfigError> {
    match env(read, var) {
        Some(value) => value.parse::<usize>().map_err(|_| {
            ConfigError::new(format!("{var} must be a whole number"))
        }),
        None => Ok(default),
    }
}

fn env(read: &impl Fn(&str) -> Option<String>, var: &str) -> Option<String> {
    read(var)
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn flag_enabled(value: Option<&str>) -> bool {
    value.is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn config_from(
        pairs: &[(&str, &str)],
    ) -> Result<Option<ObservabilityConfig>, ConfigError> {
        let env: HashMap<String, String> = pairs
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect();
        ObservabilityConfig::from_env_with(|key| env.get(key).cloned())
    }

    fn enabled_pairs() -> Vec<(&'static str, &'static str)> {
        vec![
            (ENABLE_VAR, "1"),
            (GENERAL_ENDPOINT_VAR, "https://collector.example.com"),
        ]
    }

    #[test]
    fn disabled_by_default() {
        assert_eq!(config_from(&[]), Ok(None));
        assert_eq!(config_from(&[(ENABLE_VAR, "0")]), Ok(None));
        assert_eq!(config_from(&[(ENABLE_VAR, "off")]), Ok(None));
    }

    #[test]
    fn enabled_flags_are_case_insensitive() {
        for flag in ["1", "true", "TRUE", "yes", "on"] {
            let config = config_from(&[
                (ENABLE_VAR, flag),
                (GENERAL_ENDPOINT_VAR, "https://collector.example.com"),
            ])
            .unwrap()
            .expect("configuration should be enabled");
            assert_eq!(config.service_name, DEFAULT_SERVICE_NAME);
        }
    }

    #[test]
    fn requires_an_endpoint() {
        let error = config_from(&[(ENABLE_VAR, "1")]).unwrap_err();
        assert!(error.to_string().contains(GENERAL_ENDPOINT_VAR));
    }

    #[test]
    fn appends_signal_paths_to_the_generic_endpoint() {
        let config = config_from(&enabled_pairs()).unwrap().unwrap();
        assert_eq!(
            config.traces.endpoint,
            "https://collector.example.com/v1/traces"
        );
        assert_eq!(
            config.metrics.endpoint,
            "https://collector.example.com/v1/metrics"
        );
    }

    #[test]
    fn signal_specific_endpoints_override_the_generic_one() {
        let config = config_from(&[
            (ENABLE_VAR, "on"),
            (GENERAL_ENDPOINT_VAR, "https://collector.example.com"),
            (
                "OTEL_EXPORTER_OTLP_TRACES_ENDPOINT",
                "https://traces.example.com/custom",
            ),
        ])
        .unwrap()
        .unwrap();
        assert_eq!(config.traces.endpoint, "https://traces.example.com/custom");
        assert_eq!(
            config.metrics.endpoint,
            "https://collector.example.com/v1/metrics"
        );
    }

    #[test]
    fn rejects_insecure_remote_endpoints() {
        let error = config_from(&[
            (ENABLE_VAR, "1"),
            (GENERAL_ENDPOINT_VAR, "http://collector.example.com"),
        ])
        .unwrap_err();
        assert!(error.to_string().contains("https"));
    }

    #[test]
    fn accepts_loopback_plain_http_endpoints() {
        for endpoint in [
            "http://localhost:4318",
            "http://127.0.0.1:4318",
            "http://[::1]:4318",
        ] {
            let config = config_from(&[
                (ENABLE_VAR, "1"),
                (GENERAL_ENDPOINT_VAR, endpoint),
            ])
            .unwrap()
            .unwrap_or_else(|| panic!("{endpoint} should be accepted"));
            assert!(config.traces.endpoint.starts_with(endpoint));
        }
    }

    #[test]
    fn rejects_userinfo_endpoints() {
        let error = config_from(&[
            (ENABLE_VAR, "1"),
            (
                GENERAL_ENDPOINT_VAR,
                "https://user:secret@collector.example.com",
            ),
        ])
        .unwrap_err();
        assert!(error.to_string().contains("userinfo"));
        assert!(!error.to_string().contains("secret"));
    }

    #[test]
    fn rejects_grpc_protocol() {
        let mut pairs = enabled_pairs();
        pairs.push((GENERAL_PROTOCOL_VAR, "grpc"));
        let error = config_from(&pairs).unwrap_err();
        assert!(error.to_string().contains("http/protobuf"));
    }

    #[test]
    fn accepts_explicit_http_protobuf() {
        let mut pairs = enabled_pairs();
        pairs.push((GENERAL_PROTOCOL_VAR, "http/protobuf"));
        assert!(config_from(&pairs).unwrap().is_some());
    }

    #[test]
    fn signal_specific_protocol_overrides_the_generic_one() {
        let mut pairs = enabled_pairs();
        pairs.push((GENERAL_PROTOCOL_VAR, "grpc"));
        pairs.push(("OTEL_EXPORTER_OTLP_TRACES_PROTOCOL", "http/protobuf"));
        pairs.push(("OTEL_EXPORTER_OTLP_METRICS_PROTOCOL", "http/protobuf"));
        // The generic grpc value still applies to logs, but logs are off.
        assert!(config_from(&pairs).unwrap().is_some());
    }

    #[test]
    fn parses_headers() {
        let mut pairs = enabled_pairs();
        pairs.push((
            GENERAL_HEADERS_VAR,
            "authorization=Bearer token,x-scope = launcher ,x-encoded=Bearer%20abc%2Ctenant",
        ));
        let config = config_from(&pairs).unwrap().unwrap();
        assert_eq!(
            config.traces.headers,
            vec![
                ("authorization".to_owned(), "Bearer token".to_owned()),
                ("x-scope".to_owned(), "launcher".to_owned()),
                ("x-encoded".to_owned(), "Bearer abc,tenant".to_owned()),
            ]
        );
        assert_eq!(config.traces.headers, config.metrics.headers);
    }

    #[test]
    fn rejects_headers_without_a_value() {
        let mut pairs = enabled_pairs();
        pairs.push((GENERAL_HEADERS_VAR, "authorization"));
        assert!(config_from(&pairs).is_err());
    }

    #[test]
    fn parses_resource_attributes_and_service_name() {
        let mut pairs = enabled_pairs();
        pairs.push((SERVICE_NAME_VAR, "custom-launcher"));
        pairs.push((
            RESOURCE_ATTRIBUTES_VAR,
            "deployment.environment=dev,service.name=ignored",
        ));
        let config = config_from(&pairs).unwrap().unwrap();
        assert_eq!(config.service_name, "custom-launcher");
        assert_eq!(
            config.resource_attributes,
            vec![("deployment.environment".to_owned(), "dev".to_owned())]
        );
    }

    #[test]
    fn clamps_the_sample_ratio() {
        let mut pairs = enabled_pairs();
        pairs.push((SAMPLER_ARG_VAR, "4.5"));
        assert_eq!(config_from(&pairs).unwrap().unwrap().sampler.ratio, 1.0);

        let mut pairs = enabled_pairs();
        pairs.push((SAMPLER_ARG_VAR, "-2"));
        assert_eq!(config_from(&pairs).unwrap().unwrap().sampler.ratio, 0.0);

        let mut pairs = enabled_pairs();
        pairs.push((SAMPLER_ARG_VAR, "nan"));
        assert!(config_from(&pairs).is_err());
    }

    #[test]
    fn defaults_the_sample_ratio_to_ten_percent() {
        let config = config_from(&enabled_pairs()).unwrap().unwrap();
        assert_eq!(config.sampler.ratio, DEFAULT_SAMPLE_RATIO);
        assert_eq!(config.sampler.mode, SamplerMode::ParentBasedTraceIdRatio);
    }

    #[test]
    fn honors_the_standard_trace_sampler() {
        let cases = [
            ("always_on", SamplerMode::AlwaysOn),
            ("always_off", SamplerMode::AlwaysOff),
            ("traceidratio", SamplerMode::TraceIdRatio),
            ("parentbased_always_on", SamplerMode::ParentBasedAlwaysOn),
            ("parentbased_always_off", SamplerMode::ParentBasedAlwaysOff),
        ];
        for (value, expected) in cases {
            let mut pairs = enabled_pairs();
            pairs.push((SAMPLER_VAR, value));
            let config = config_from(&pairs).unwrap().unwrap();
            assert_eq!(config.sampler.mode, expected);
        }

        let mut pairs = enabled_pairs();
        pairs.push((SAMPLER_VAR, "not-a-sampler"));
        assert!(config_from(&pairs).is_err());
    }

    #[test]
    fn clamps_batch_settings() {
        let mut pairs = enabled_pairs();
        pairs.push((BSP_MAX_QUEUE_SIZE_VAR, "0"));
        pairs.push((BSP_MAX_EXPORT_BATCH_SIZE_VAR, "999999"));
        pairs.push((BSP_SCHEDULE_DELAY_VAR, "1"));
        let config = config_from(&pairs).unwrap().unwrap();
        assert_eq!(config.batch.max_queue_size, MIN_QUEUE_SIZE);
        assert_eq!(config.batch.max_export_batch_size, MIN_QUEUE_SIZE);
        assert_eq!(
            config.batch.scheduled_delay,
            Duration::from_millis(MIN_SCHEDULE_DELAY_MS)
        );
    }

    #[test]
    fn limits_the_export_batch_to_the_queue_size() {
        let mut pairs = enabled_pairs();
        pairs.push((BSP_MAX_QUEUE_SIZE_VAR, "100"));
        pairs.push((BSP_MAX_EXPORT_BATCH_SIZE_VAR, "500"));
        let config = config_from(&pairs).unwrap().unwrap();
        assert_eq!(config.batch.max_export_batch_size, 100);
    }

    #[test]
    fn rejects_non_numeric_settings() {
        let mut pairs = enabled_pairs();
        pairs.push((BSP_MAX_QUEUE_SIZE_VAR, "lots"));
        assert!(config_from(&pairs).is_err());
    }

    #[test]
    fn logs_require_an_explicit_opt_in() {
        let config = config_from(&enabled_pairs()).unwrap().unwrap();
        assert!(config.logs.is_none());
        assert!(!config.raw_logs);

        let mut pairs = enabled_pairs();
        pairs.push((LOGS_VAR, "1"));
        let config = config_from(&pairs).unwrap().unwrap();
        assert!(config.logs.is_some());
        assert!(!config.raw_logs);
    }

    #[test]
    fn raw_logs_require_logs() {
        let mut pairs = enabled_pairs();
        pairs.push((RAW_LOGS_VAR, "1"));
        let config = config_from(&pairs).unwrap().unwrap();
        assert!(config.logs.is_none());
        assert!(!config.raw_logs);

        let mut pairs = enabled_pairs();
        pairs.push((LOGS_VAR, "1"));
        pairs.push((RAW_LOGS_VAR, "yes"));
        let config = config_from(&pairs).unwrap().unwrap();
        assert!(config.logs.is_some());
        assert!(config.raw_logs);
    }

    #[test]
    fn logs_need_their_own_endpoint_when_the_generic_one_is_absent() {
        let pairs = vec![
            (ENABLE_VAR, "1"),
            (
                "OTEL_EXPORTER_OTLP_TRACES_ENDPOINT",
                "https://collector.example.com/v1/traces",
            ),
            (
                "OTEL_EXPORTER_OTLP_METRICS_ENDPOINT",
                "https://collector.example.com/v1/metrics",
            ),
            (LOGS_VAR, "1"),
        ];
        let error = config_from(&pairs).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT")
        );
    }

    #[test]
    fn clamps_the_metric_interval() {
        let mut pairs = enabled_pairs();
        pairs.push((METRIC_INTERVAL_VAR, "5"));
        let config = config_from(&pairs).unwrap().unwrap();
        assert_eq!(
            config.metric_interval,
            Duration::from_millis(MIN_METRIC_INTERVAL_MS)
        );
    }

    #[test]
    fn appending_a_signal_path_is_idempotent() {
        assert_eq!(
            append_signal_path(
                "https://collector.example.com/v1/traces",
                "/v1/traces"
            )
            .unwrap(),
            "https://collector.example.com/v1/traces"
        );
        assert_eq!(
            append_signal_path("https://collector.example.com/", "/v1/logs")
                .unwrap(),
            "https://collector.example.com/v1/logs"
        );
        assert_eq!(
            append_signal_path(
                "https://collector.example.com/otlp?tenant=a",
                "/v1/traces"
            )
            .unwrap(),
            "https://collector.example.com/otlp/v1/traces?tenant=a"
        );
    }

    #[test]
    fn endpoint_validation_matrix() {
        for endpoint in [
            "https://collector.example.com",
            "https://collector.example.com:4318/v1/traces",
            "http://localhost:4318",
            "http://127.0.0.1",
            "http://[::1]:4318",
        ] {
            assert!(
                validate_endpoint(endpoint, "TEST").is_ok(),
                "{endpoint} should be accepted"
            );
        }
        for endpoint in [
            "collector.example.com",
            "ftp://collector.example.com",
            "http://collector.example.com",
            "http://10.0.0.1:4318",
            "https://user@collector.example.com",
            "https://",
        ] {
            assert!(
                validate_endpoint(endpoint, "TEST").is_err(),
                "{endpoint} should be rejected"
            );
        }
    }
}

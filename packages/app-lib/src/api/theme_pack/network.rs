//! The only network path a theme pack gets.
//!
//! A pack page runs in a sandboxed frame whose CSP forbids `connect-src`, so it
//! cannot reach the network directly. Instead the page asks the launcher for a
//! request; the launcher checks the pack's declared host allowlist, resolves
//! the name itself and refuses private addresses, then performs the request
//! with no cookies, no forwarded `Authorization` and a fixed user agent.

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::net::Ipv6Addr;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::time::Duration;
use std::time::Instant;

use base64::Engine as _;
use futures::StreamExt as _;
use serde::Deserialize;
use serde::Serialize;
use tokio::sync::OwnedSemaphorePermit;
use tokio::sync::Semaphore;
use url::Url;

use super::ThemePackStorage;
use super::manifest::ThemePackManifest;
use super::manifest::ThemePackPermission;
use crate::ErrorKind;

const MAX_REQUEST_BODY: usize = 1024 * 1024;
const MAX_RESPONSE_BODY: usize = 8 * 1024 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_REDIRECTS: usize = 5;
const MAX_CONCURRENT_PER_PACK: usize = 4;
const MAX_REQUESTS_PER_MINUTE: usize = 60;

/// Headers a pack page may set. Everything that could smuggle a credential or
/// rewrite the authority of the request is refused.
const FORBIDDEN_HEADERS: &[&str] = &[
    "host",
    "cookie",
    "origin",
    "referer",
    "connection",
    "content-length",
    "transfer-encoding",
    "upgrade",
    "proxy-authorization",
    "proxy-connection",
    "te",
    "trailer",
];

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ThemeNetworkRequest {
    pub method: String,
    pub url: String,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    /// A base64 body, used only with POST.
    #[serde(default)]
    pub body: Option<String>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ThemeNetworkResponse {
    pub status: u16,
    pub url: String,
    /// Only the content type; cookies and authorization headers never come
    /// back to the page.
    pub content_type: Option<String>,
    pub body: String,
}

struct PackBudget {
    permits: Arc<Semaphore>,
    requests: Mutex<VecDeque<Instant>>,
}

fn budgets() -> &'static Mutex<HashMap<String, Arc<PackBudget>>> {
    static BUDGETS: OnceLock<Mutex<HashMap<String, Arc<PackBudget>>>> =
        OnceLock::new();
    BUDGETS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn budget_for(pack_id: &str) -> Arc<PackBudget> {
    let mut budgets = budgets().lock().expect("theme pack budget lock");
    budgets
        .entry(pack_id.to_string())
        .or_insert_with(|| {
            Arc::new(PackBudget {
                permits: Arc::new(Semaphore::new(MAX_CONCURRENT_PER_PACK)),
                requests: Mutex::new(VecDeque::new()),
            })
        })
        .clone()
}

fn take_rate_limit_slot(budget: &PackBudget) -> crate::Result<()> {
    let mut requests = budget.requests.lock().expect("theme pack rate lock");
    let now = Instant::now();
    while let Some(front) = requests.front() {
        if now.duration_since(*front) > Duration::from_secs(60) {
            requests.pop_front();
        } else {
            break;
        }
    }
    if requests.len() >= MAX_REQUESTS_PER_MINUTE {
        return Err(invalid(
            "This theme pack has made too many network requests; try again in a minute",
        ));
    }
    requests.push_back(now);
    Ok(())
}

pub async fn fetch(
    storage: &ThemePackStorage,
    pack_id: &str,
    request: ThemeNetworkRequest,
) -> crate::Result<ThemeNetworkResponse> {
    let pack_dir = storage.pack_dir(pack_id)?;
    let manifest = ThemePackManifest::load(&pack_dir)?;
    if !manifest.has_permission(ThemePackPermission::Network) {
        return Err(invalid(
            "This theme pack did not request the network permission",
        ));
    }
    let allow = manifest.network.allow.clone();
    let budget = budget_for(pack_id);
    let _permit: OwnedSemaphorePermit = budget
        .permits
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| invalid("The theme pack network queue is closed"))?;
    take_rate_limit_slot(&budget)?;

    tokio::time::timeout(REQUEST_TIMEOUT, execute(request, &allow))
        .await
        .map_err(|_| invalid("The theme pack network request timed out"))?
}

async fn execute(
    request: ThemeNetworkRequest,
    allow: &[String],
) -> crate::Result<ThemeNetworkResponse> {
    let method = request.method.to_ascii_uppercase();
    if method != "GET" && method != "POST" {
        return Err(invalid("Theme pack requests may only use GET and POST"));
    }
    let body = match request.body.as_deref() {
        Some(body) => {
            let decoded = base64::engine::general_purpose::STANDARD
                .decode(body)
                .map_err(|_| {
                    invalid("The theme pack request body is not base64")
                })?;
            if decoded.len() > MAX_REQUEST_BODY {
                return Err(invalid(
                    "The theme pack request body is larger than 1 MB",
                ));
            }
            Some(decoded)
        }
        None => None,
    };
    if method == "POST" && body.is_none() {
        return Err(invalid("A theme pack POST needs a body"));
    }
    let headers = sanitize_headers(&request.headers)?;

    let mut current = parse_endpoint(&request.url, allow)?;
    let mut method = method;
    let mut body = body;
    let mut headers = headers;
    for _ in 0..=MAX_REDIRECTS {
        let (url, host) = current;
        let addresses = resolve_public(&host).await?;
        let mut builder = crate::util::http::client_builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(REQUEST_TIMEOUT)
            .user_agent(crate::launcher_user_agent());
        for address in &addresses {
            builder = builder.resolve(&host, *address);
        }
        let client = builder.build()?;
        let request_method = if method == "GET" {
            reqwest::Method::GET
        } else {
            reqwest::Method::POST
        };
        let mut builder = client.request(request_method, url.clone());
        for (name, value) in &headers {
            builder = builder.header(name, value);
        }
        if let Some(body) = &body {
            builder = builder.body(body.clone());
        }
        let response = builder.send().await?;
        let status = response.status();
        if status.is_redirection() {
            let Some(location) = response
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|value| value.to_str().ok())
            else {
                return Err(invalid(
                    "The theme pack response redirected without a location",
                ));
            };
            let next = url.join(location).map_err(|_| {
                invalid(
                    "The theme pack response redirected to an invalid address",
                )
            })?;
            current = parse_endpoint(next.as_str(), allow)?;
            if !current.1.eq_ignore_ascii_case(&host) {
                // A credential meant for one host must not follow a redirect
                // to another allowed host.
                headers.remove("authorization");
            }
            if status.as_u16() == 303
                || ((status.as_u16() == 301 || status.as_u16() == 302)
                    && method == "POST")
            {
                method = "GET".to_string();
                body = None;
            }
            continue;
        }
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let final_url = response.url().to_string();
        let mut stream = response.bytes_stream();
        let mut collected = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            if collected.len() + chunk.len() > MAX_RESPONSE_BODY {
                return Err(invalid(
                    "The theme pack network response is larger than 8 MB",
                ));
            }
            collected.extend_from_slice(&chunk);
        }
        return Ok(ThemeNetworkResponse {
            status: status.as_u16(),
            url: final_url,
            content_type,
            body: base64::engine::general_purpose::STANDARD.encode(collected),
        });
    }
    Err(invalid("The theme pack response redirected too many times"))
}

fn sanitize_headers(
    headers: &BTreeMap<String, String>,
) -> crate::Result<BTreeMap<String, String>> {
    if headers.len() > 32 {
        return Err(invalid("The theme pack request has too many headers"));
    }
    let mut sanitized = BTreeMap::new();
    for (name, value) in headers {
        let lower = name.to_ascii_lowercase();
        if lower.is_empty()
            || lower.len() > 64
            || !lower
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        {
            return Err(invalid("A theme pack request header name is invalid"));
        }
        if FORBIDDEN_HEADERS.contains(&lower.as_str())
            || lower.starts_with("proxy-")
            || lower.starts_with("sec-")
        {
            return Err(invalid(format!(
                "The theme pack may not set the {lower} header"
            )));
        }
        if value.len() > 4096
            || value.chars().any(|c| c == '\r' || c == '\n' || c == '\0')
        {
            return Err(invalid(
                "A theme pack request header value is invalid",
            ));
        }
        sanitized.insert(lower, value.clone());
    }
    Ok(sanitized)
}

/// Checks the scheme, the allowlist and the shape of the address. Name
/// resolution happens per hop in [`resolve_public`], so a host that changes its
/// answer between the check and the connection is resolved again every time.
pub fn parse_endpoint(
    url: &str,
    allow: &[String],
) -> crate::Result<(Url, String)> {
    let parsed = Url::parse(url)
        .map_err(|_| invalid("The theme pack network address is not a URL"))?;
    if parsed.scheme() != "https" {
        return Err(invalid("Theme pack network requests must use https"));
    }
    if parsed.port_or_known_default() != Some(443) {
        return Err(invalid("Theme pack network requests must use port 443"));
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(invalid(
            "Theme pack network requests may not embed credentials",
        ));
    }
    let Some(host) = parsed.host_str().map(str::to_ascii_lowercase) else {
        return Err(invalid("The theme pack network address has no host"));
    };
    if !allow.iter().any(|pattern| host_allowed(&host, pattern)) {
        return Err(invalid(format!(
            "The theme pack is not allowed to contact {host}"
        )));
    }
    Ok((parsed, host))
}

/// Exact hosts match themselves; `*.example.com` matches subdomains only, so a
/// wildcard can never be used to reach the bare apex.
pub fn host_allowed(host: &str, pattern: &str) -> bool {
    let pattern = pattern.trim().trim_end_matches('.');
    let host = host.trim_end_matches('.');
    if let Some(suffix) = pattern.strip_prefix("*.") {
        let suffix = format!(".{}", suffix.to_ascii_lowercase());
        return host.len() > suffix.len()
            && host.to_ascii_lowercase().ends_with(&suffix);
    }
    host.eq_ignore_ascii_case(pattern)
}

pub fn is_public_ip(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => is_public_v4(address),
        IpAddr::V6(address) => is_public_v6(address),
    }
}

fn is_public_v4(address: Ipv4Addr) -> bool {
    let octets = address.octets();
    !(address.is_loopback()
        || address.is_private()
        || address.is_link_local()
        || address.is_multicast()
        || address.is_broadcast()
        || address.is_unspecified()
        || address.is_documentation()
        || octets[0] == 0
        // 100.64.0.0/10 carrier-grade NAT.
        || (octets[0] == 100 && (64..=127).contains(&octets[1]))
        // 198.18.0.0/15 benchmarking.
        || (octets[0] == 198 && (18..=19).contains(&octets[1]))
        // 240.0.0.0/4 reserved.
        || octets[0] >= 240)
}

fn is_public_v6(address: Ipv6Addr) -> bool {
    if let Some(mapped) = address.to_ipv4_mapped() {
        return is_public_v4(mapped);
    }
    let segments = address.segments();
    !(address.is_loopback()
        || address.is_unspecified()
        || address.is_multicast()
        || (segments[0] & 0xfe00) == 0xfc00 // unique local fc00::/7
        || (segments[0] & 0xffc0) == 0xfe80 // link local fe80::/10
        || (segments[0] & 0xffc0) == 0xfec0 // site local fec0::/10
        || (segments[0] & 0xff00) == 0xff00) // multicast, already covered
}

async fn resolve_public(host: &str) -> crate::Result<Vec<SocketAddr>> {
    let addresses: Vec<SocketAddr> = tokio::net::lookup_host((host, 443u16))
        .await
        .map_err(|error| {
            invalid(format!(
                "The theme pack host {host} could not be resolved: {error}"
            ))
        })?
        .collect();
    if addresses.is_empty() {
        return Err(invalid(format!(
            "The theme pack host {host} resolved to no address"
        )));
    }
    for address in &addresses {
        if !is_public_ip(address.ip()) {
            return Err(invalid(format!(
                "The theme pack host {host} resolves to a private address"
            )));
        }
    }
    Ok(addresses)
}

fn invalid(message: impl Into<String>) -> crate::Error {
    ErrorKind::InputError(message.into()).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_exact_hosts_and_subdomains_only() {
        assert!(host_allowed("api.example.com", "api.example.com"));
        assert!(host_allowed("API.EXAMPLE.COM", "api.example.com"));
        assert!(host_allowed("a.example.org", "*.example.org"));
        assert!(host_allowed("deep.a.example.org", "*.example.org"));
        assert!(!host_allowed("example.org", "*.example.org"));
        assert!(!host_allowed("evil-example.org", "*.example.org"));
        assert!(!host_allowed(
            "api.example.com.evil.test",
            "api.example.com"
        ));
    }

    #[test]
    fn rejects_private_addresses() {
        assert!(!is_public_ip("127.0.0.1".parse().unwrap()));
        assert!(!is_public_ip("10.0.0.1".parse().unwrap()));
        assert!(!is_public_ip("192.168.1.1".parse().unwrap()));
        assert!(!is_public_ip("169.254.169.254".parse().unwrap()));
        assert!(!is_public_ip("100.64.0.1".parse().unwrap()));
        assert!(!is_public_ip("::1".parse().unwrap()));
        assert!(!is_public_ip("fc00::1".parse().unwrap()));
        assert!(!is_public_ip("fe80::1".parse().unwrap()));
        assert!(!is_public_ip("::ffff:10.0.0.1".parse().unwrap()));
        assert!(is_public_ip("1.1.1.1".parse().unwrap()));
        assert!(is_public_ip("2606:4700:4700::1111".parse().unwrap()));
    }

    #[test]
    fn rejects_plain_http_and_non_default_ports() {
        let allow = vec!["example.com".to_string()];
        assert!(parse_endpoint("http://example.com/", &allow).is_err());
        assert!(parse_endpoint("https://example.com:8443/", &allow).is_err());
        assert!(
            parse_endpoint("https://user:secret@example.com/", &allow).is_err()
        );
        assert!(parse_endpoint("https://other.com/", &allow).is_err());
        assert!(parse_endpoint("https://example.com/path", &allow).is_ok());
    }
}

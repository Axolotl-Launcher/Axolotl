//! Credential resolution, mirror routing, and request plumbing.

use super::auth::{api_key_generation, record_key_response};
use super::constants::API_BASE_URL;
use super::dto::CurseForgeApiKeySource;
use crate::state::{
    DownloadSourceMode, curseforge_api_key_override_if_ready,
    curseforge_settings,
};
use crate::util::fetch::{
    DownloadRouteSource, ProxyPolicy, ResourceClass,
    resolve_download_routes_for,
};
use crate::{ErrorKind, State};
use reqwest::{Method, StatusCode};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

// Metadata allows a bounded route fallback beyond the connect/read limits.
// This budget also covers admission, response bodies and Retry-After waits.
const METADATA_REQUEST_TIMEOUT: Duration = Duration::from_secs(90);
const API_PERMIT_TIMEOUT: Duration = Duration::from_secs(20);
static NEXT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);

pub(crate) fn metadata_cache_scope() -> String {
    use sha2::{Digest, Sha256};
    let state = State::get_if_initialized();
    let settings = state
        .as_ref()
        .map(|state| (state.curseforge_source(), state.network_scope()));
    // Only a digest leaves this function: credentials and proxy settings must
    // never become cache keys or diagnostic strings in plain text.
    let identity = serde_json::to_vec(&(
        api_base_url(),
        api_key_generation(),
        api_key(),
        settings,
    ))
    .expect("metadata cache identity is serializable");
    format!("{:x}", Sha256::digest(identity))
}

struct RequestTrace {
    id: u64,
    started: Instant,
    route: usize,
    stage: &'static str,
    outcome: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum RequestOrigin {
    Legacy,
    Resource,
}

fn request_identity(
    method: &Method,
    path: &str,
    query: &[(String, String)],
    body: &Option<Value>,
) -> String {
    use sha2::{Digest, Sha256};
    fn canonical(value: &Value) -> Value {
        match value {
            Value::Object(object) => {
                let mut keys = object.keys().collect::<Vec<_>>();
                keys.sort();
                Value::Object(
                    keys.into_iter()
                        .map(|key| (key.clone(), canonical(&object[key])))
                        .collect(),
                )
            }
            Value::Array(values) => {
                Value::Array(values.iter().map(canonical).collect())
            }
            value => value.clone(),
        }
    }
    let mut sorted_query = query.to_vec();
    sorted_query.sort();
    let identity = serde_json::to_vec(&(
        method.as_str(),
        path,
        sorted_query,
        body.as_ref().map(canonical),
    ))
    .expect("JSON request identity is serializable");
    format!("{:x}", Sha256::digest(identity))
}

impl RequestTrace {
    fn new(
        method: &Method,
        path: &str,
        query: &[(String, String)],
        body: &Option<Value>,
        origin: RequestOrigin,
    ) -> Self {
        let query_key = request_identity(method, path, query, body);
        let fields = query
            .iter()
            .filter(|(key, _)| {
                matches!(
                    key.as_str(),
                    "index" | "pageSize" | "gameVersion" | "modLoaderType"
                )
            })
            .collect::<Vec<_>>();
        let item_count = body
            .as_ref()
            .and_then(|body| {
                ["fingerprints", "modIds", "fileIds", "hashes"]
                    .iter()
                    .find_map(|key| {
                        body.get(key).and_then(Value::as_array).map(Vec::len)
                    })
            })
            .unwrap_or(1);
        let trace = Self {
            id: NEXT_REQUEST_ID.fetch_add(1, Ordering::Relaxed),
            started: Instant::now(),
            route: 0,
            stage: "state",
            outcome: "canceled_or_dropped",
        };
        tracing::info!(request_id = trace.id, method = %method, path, %query_key, ?fields, item_count, ?origin, "Started CurseForge metadata request");
        trace
    }

    fn stage(&mut self, stage: &'static str) {
        self.stage = stage;
        tracing::debug!(
            request_id = self.id,
            route = self.route,
            stage,
            elapsed_ms = self.started.elapsed().as_millis() as u64,
            "CurseForge metadata request stage"
        );
    }
}

impl Drop for RequestTrace {
    fn drop(&mut self) {
        if matches!(self.outcome, "success" | "canceled_or_dropped") {
            tracing::info!(
                request_id = self.id,
                route = self.route,
                stage = self.stage,
                outcome = self.outcome,
                elapsed_ms = self.started.elapsed().as_millis() as u64,
                "Finished CurseForge metadata request"
            );
        } else {
            tracing::warn!(
                request_id = self.id,
                route = self.route,
                stage = self.stage,
                outcome = self.outcome,
                elapsed_ms = self.started.elapsed().as_millis() as u64,
                "Finished CurseForge metadata request"
            );
        }
    }
}

pub(crate) async fn metadata_deadline<T>(
    deadline: tokio::time::Instant,
    work: impl std::future::Future<Output = crate::Result<T>>,
) -> crate::Result<T> {
    let result =
        tokio::time::timeout_at(deadline, work).await.map_err(|_| {
            ErrorKind::DownloadResourceWaitTimeout(
                "CurseForge metadata request timed out before completion"
                    .into(),
            )
        })?;
    // Synchronous decoding cannot be preempted; reject a result that finished
    // after the deadline rather than reporting it as an in-budget success.
    if tokio::time::Instant::now() >= deadline {
        return Err(ErrorKind::DownloadResourceWaitTimeout(
            "CurseForge metadata request timed out before completion".into(),
        )
        .into());
    }
    result
}

async fn api_permit(
    semaphore: &tokio::sync::Semaphore,
    budget: Duration,
) -> crate::Result<tokio::sync::SemaphorePermit<'_>> {
    tokio::time::timeout(budget, semaphore.acquire()).await.map_err(|_| ErrorKind::DownloadResourceWaitTimeout("CurseForge API request timed out waiting for an available request slot".into()))?.map_err(|_| ErrorKind::OtherError("CurseForge API request slots are closed".into()).into())
}

static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .read_timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(crate::launcher_user_agent())
        .no_proxy()
        .build()
        .expect("CurseForge client configuration should be valid")
});

async fn response_body(
    response: reqwest::Response,
    deadline: tokio::time::Instant,
) -> crate::Result<bytes::Bytes> {
    metadata_deadline(deadline, async { Ok(response.bytes().await?) }).await
}

#[cfg(debug_assertions)]
static LOCAL_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .read_timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(crate::launcher_user_agent())
        .no_proxy()
        .build()
        .expect("Local CurseForge client configuration should be valid")
});

pub(crate) fn api_key() -> Option<String> {
    api_key_with_source().0
}

/// A key set in the launcher wins over `AXOLOTL_CURSEFORGE_API_KEY`, which in
/// turn wins over the one this build was given. Either of the launcher sources
/// can be absent: builds without `CURSEFORGE_API_KEY` stay valid, they just
/// cannot reach the official API until a key is provided.
pub(crate) fn api_key_with_source() -> (Option<String>, CurseForgeApiKeySource)
{
    let launcher_key = std::env::var("AXOLOTL_CURSEFORGE_API_KEY")
        .ok()
        .or_else(|| option_env!("CURSEFORGE_API_KEY").map(str::to_string));

    resolve_api_key(curseforge_api_key_override_if_ready(), launcher_key)
}

pub(super) fn resolve_api_key(
    override_key: Option<String>,
    launcher_key: Option<String>,
) -> (Option<String>, CurseForgeApiKeySource) {
    if let Some(key) = curseforge_settings::normalize(override_key) {
        return (Some(key), CurseForgeApiKeySource::Override);
    }

    match curseforge_settings::normalize(launcher_key) {
        Some(key) => (Some(key), CurseForgeApiKeySource::Launcher),
        None => (None, CurseForgeApiKeySource::Missing),
    }
}

fn api_base_url() -> String {
    #[cfg(debug_assertions)]
    if let Ok(value) = std::env::var("AXOLOTL_CURSEFORGE_API_BASE_URL")
        && value.starts_with("http://127.0.0.1:")
    {
        return value.trim_end_matches('/').to_string();
    }

    API_BASE_URL.to_string()
}

fn request_client(
    state: &State,
    _url: &str,
    use_system_proxy: bool,
) -> reqwest::Client {
    #[cfg(debug_assertions)]
    if _url.starts_with("http://127.0.0.1:") {
        return LOCAL_CLIENT.clone();
    }

    if use_system_proxy {
        state.download_clients().system
    } else {
        CLIENT.clone()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MirrorPolicy {
    MirrorFirst,
    OfficialOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RequestRouteSource {
    Official,
    Mirror,
}

pub(super) struct RequestRoute {
    pub(super) url: String,
    pub(super) use_api_key: bool,
    pub(super) use_system_proxy: bool,
    pub(super) source: RequestRouteSource,
}

#[cfg(test)]
pub(super) fn request_routes(
    path: &str,
    mirror_policy: MirrorPolicy,
) -> Vec<RequestRoute> {
    let mode = match mirror_policy {
        MirrorPolicy::MirrorFirst => DownloadSourceMode::MirrorPreferred,
        MirrorPolicy::OfficialOnly => DownloadSourceMode::OfficialOnly,
    };
    request_routes_with_mode(path, mode)
}

fn request_routes_with_mode(
    path: &str,
    mode: DownloadSourceMode,
) -> Vec<RequestRoute> {
    let base_url = api_base_url();
    if base_url != API_BASE_URL {
        return vec![RequestRoute {
            url: format!("{base_url}{path}"),
            use_api_key: true,
            use_system_proxy: false,
            source: RequestRouteSource::Official,
        }];
    }

    resolve_download_routes_for(
        &format!("{API_BASE_URL}{path}"),
        ResourceClass::CurseForge,
        mode,
    )
    .into_iter()
    .map(|route| RequestRoute {
        use_api_key: route.allow_sensitive_headers,
        use_system_proxy: route.proxy == ProxyPolicy::System,
        source: match route.source {
            DownloadRouteSource::Bmclapi
            | DownloadRouteSource::Mcim
            | DownloadRouteSource::Aliyun => RequestRouteSource::Mirror,
            DownloadRouteSource::Official | DownloadRouteSource::Alternate => {
                RequestRouteSource::Official
            }
        },
        url: route.url,
    })
    .collect()
}

pub(crate) async fn request_json<T: DeserializeOwned>(
    method: Method,
    path: &str,
    query: Vec<(String, String)>,
    body: Option<Value>,
    mirror_policy: MirrorPolicy,
) -> crate::Result<T> {
    request_json_with_origin(
        method,
        path,
        query,
        body,
        mirror_policy,
        RequestOrigin::Legacy,
    )
    .await
}

pub(super) async fn request_json_with_origin<T: DeserializeOwned>(
    method: Method,
    path: &str,
    query: Vec<(String, String)>,
    body: Option<Value>,
    mirror_policy: MirrorPolicy,
    origin: RequestOrigin,
) -> crate::Result<T> {
    let mut trace = RequestTrace::new(&method, path, &query, &body, origin);
    let deadline = tokio::time::Instant::now() + METADATA_REQUEST_TIMEOUT;
    let result = metadata_deadline(
        deadline,
        request_json_inner(
            method,
            path,
            query,
            body,
            mirror_policy,
            deadline,
            &mut trace,
        ),
    )
    .await;
    trace.outcome = match &result {
        Ok(_) => "success",
        Err(error)
            if matches!(error.raw.as_ref(), ErrorKind::FetchError(error) if error.is_timeout())
                || matches!(
                    error.raw.as_ref(),
                    ErrorKind::DownloadResourceWaitTimeout(_)
                ) =>
        {
            "timeout"
        }
        Err(_) => "error",
    };
    result
}

async fn request_json_inner<T: DeserializeOwned>(
    method: Method,
    path: &str,
    query: Vec<(String, String)>,
    body: Option<Value>,
    mirror_policy: MirrorPolicy,
    deadline: tokio::time::Instant,
    trace: &mut RequestTrace,
) -> crate::Result<T> {
    let key = api_key();
    let key_generation = api_key_generation();
    let state = State::get().await?;
    let source_mode = if method != Method::GET
        || mirror_policy == MirrorPolicy::OfficialOnly
    {
        DownloadSourceMode::OfficialOnly
    } else {
        state.curseforge_source()
    };
    let routes = request_routes_with_mode(path, source_mode);
    let mut last_error = None;

    for (route_index, route) in routes.iter().enumerate() {
        trace.route = route_index + 1;
        trace.stage("permit");
        let started = Instant::now();
        tracing::info!(
            source = ?route.source,
            request_id = trace.id,
            method = %method,
            url = %crate::util::fetch::sanitize_url_for_log(&route.url),
            route = route_index + 1,
            use_system_proxy = route.use_system_proxy,
            "Attempting CurseForge API request"
        );
        let permit =
            api_permit(&state.api_semaphore.0, API_PERMIT_TIMEOUT).await?;
        let mut request =
            request_client(&state, &route.url, route.use_system_proxy)
                .request(method.clone(), &route.url)
                .header("accept", "application/json")
                .query(&query);
        if route.use_api_key {
            let Some(key) = key.as_ref() else {
                drop(permit);
                let error: crate::Error = ErrorKind::InputError(
                    "CurseForge integration is waiting for an API key"
                        .to_string(),
                )
                .into();
                if route_index + 1 < routes.len() {
                    last_error = Some(error);
                    trace.stage("next_route");
                    continue;
                }
                return Err(error);
            };
            request = request.header("x-api-key", key);
        }
        if let Some(body) = &body {
            request = request.json(body);
        }
        trace.stage("headers");
        let response = match request.send().await {
            Ok(response) => response,
            Err(error) if route_index + 1 < routes.len() => {
                drop(permit);
                tracing::warn!(
                    request_id = trace.id,
                    url = %crate::util::fetch::sanitize_url_for_log(&route.url),
                    route = route_index + 1,
                    timeout = error.is_timeout(),
                    "CurseForge request failed, retrying with another route"
                );
                last_error = Some(error.into());
                trace.stage("next_route");
                continue;
            }
            Err(error) => return Err(error.into()),
        };
        drop(permit);

        let status = response.status();
        let retry_after = response
            .headers()
            .get("retry-after")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok())
            .map(|seconds| Duration::from_secs(seconds.min(30)));
        trace.stage("body");
        let bytes = response_body(response, deadline).await?;

        tracing::info!(
            source = ?route.source,
            request_id = trace.id,
            method = %method,
            url = %crate::util::fetch::sanitize_url_for_log(&route.url),
            route = route_index + 1,
            status = status.as_u16(),
            response_bytes = bytes.len(),
            elapsed_ms = started.elapsed().as_millis(),
            "Received CurseForge API response body"
        );

        if status.is_success() {
            trace.stage("decode");
            match serde_json::from_slice(&bytes) {
                Ok(value) => {
                    record_key_response(key_generation, false);
                    return Ok(value);
                }
                Err(error)
                    if route.source == RequestRouteSource::Mirror
                        && route_index + 1 < routes.len() =>
                {
                    tracing::warn!(
                        request_id = trace.id,
                        url = %crate::util::fetch::sanitize_url_for_log(&route.url),
                        route = route_index + 1,
                        "CurseForge mirror returned incompatible response data; falling back to official source"
                    );
                    last_error = Some(error.into());
                    trace.stage("next_route");
                    continue;
                }
                Err(error) => return Err(error.into()),
            }
        }

        if status == StatusCode::UNAUTHORIZED {
            record_key_response(key_generation, true);
        }

        let message = response_error_message(status, &bytes);
        let route_error = ErrorKind::OtherError(format!(
            "CurseForge request to {} failed with HTTP {}: {message}",
            route.url,
            status.as_u16()
        ));

        if should_try_next_route(route, status, route_index + 1 < routes.len())
        {
            trace.stage("retry_after");
            if let Some(delay) = retry_after {
                tokio::time::sleep(delay).await;
            }
            tracing::warn!(
                request_id = trace.id,
                url = %crate::util::fetch::sanitize_url_for_log(&route.url),
                route = route_index + 1,
                status = status.as_u16(),
                "CurseForge route rejected the request, trying another route"
            );
            last_error = Some(route_error.into());
            trace.stage("next_route");
            continue;
        }

        return Err(route_error.into());
    }

    Err(last_error.unwrap_or_else(|| {
        ErrorKind::OtherError("CurseForge request exhausted routes".to_string())
            .into()
    }))
}

pub(super) fn should_try_next_route(
    route: &RequestRoute,
    status: StatusCode,
    has_next_route: bool,
) -> bool {
    has_next_route
        && (route.source == RequestRouteSource::Mirror
            || status == StatusCode::TOO_MANY_REQUESTS
            || status == StatusCode::FORBIDDEN
            || status.is_server_error())
}

fn response_error_message(status: StatusCode, bytes: &[u8]) -> String {
    serde_json::from_slice::<Value>(bytes)
        .ok()
        .and_then(|value| {
            value
                .get("description")
                .or_else(|| value.get("message"))
                .or_else(|| value.get("error"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| {
            status
                .canonical_reason()
                .unwrap_or("request failed")
                .to_string()
        })
}

#[cfg(test)]
#[path = "tests/transport_budget.rs"]
mod budget_tests;

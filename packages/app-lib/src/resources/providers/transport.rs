use async_trait::async_trait;
use reqwest::Method;
use serde_json::Value;

use super::*;

#[derive(Clone, Debug)]
pub struct ProtocolRequest {
    pub method: Method,
    pub path: String,
    pub query: Vec<(String, String)>,
    pub body: Option<Value>,
    pub fields: Option<String>,
}

impl ProtocolRequest {
    pub(crate) fn item_count(&self) -> usize {
        if let Some(count) = self.body.as_ref().and_then(|body| {
            ["hashes", "modIds", "fileIds", "fingerprints"]
                .iter()
                .find_map(|key| {
                    body.get(key).and_then(Value::as_array).map(Vec::len)
                })
        }) {
            return count;
        }
        self.query
            .iter()
            .find_map(|(key, value)| {
                (key == "ids")
                    .then(|| {
                        serde_json::from_str::<Vec<Value>>(value)
                            .ok()
                            .map(|ids| ids.len())
                    })
                    .flatten()
            })
            .unwrap_or(1)
    }

    pub fn get(path: impl Into<String>) -> Self {
        Self {
            method: Method::GET,
            path: path.into(),
            query: vec![],
            body: None,
            fields: None,
        }
    }

    pub fn post(path: &str, body: Value) -> Self {
        Self {
            method: Method::POST,
            body: Some(body),
            ..Self::get(path)
        }
    }
}

#[async_trait]
pub trait ProtocolTransport: Send + Sync {
    fn cache_namespace(&self, _source: &SourceId) -> String {
        String::new()
    }
    async fn request(
        &self,
        source: &SourceId,
        request: ProtocolRequest,
    ) -> ResourceResult<Value>;
}

pub(crate) struct LauncherTransport;

struct RequestPacer {
    next: tokio::sync::Mutex<tokio::time::Instant>,
    interval: std::time::Duration,
}

impl RequestPacer {
    fn new(interval: std::time::Duration) -> Self {
        Self {
            next: tokio::sync::Mutex::new(tokio::time::Instant::now()),
            interval,
        }
    }

    async fn wait(&self) {
        let mut next = self.next.lock().await;
        tokio::time::sleep_until(*next).await;
        *next = tokio::time::Instant::now() + self.interval;
    }
}

// Keep background discovery below Modrinth's 300 requests/minute allowance,
// leaving capacity for foreground launcher requests outside this service.
static MODRINTH_REQUESTS: std::sync::LazyLock<RequestPacer> =
    std::sync::LazyLock::new(|| {
        RequestPacer::new(std::time::Duration::from_millis(250))
    });

#[async_trait]
impl ProtocolTransport for LauncherTransport {
    fn cache_namespace(&self, source: &SourceId) -> String {
        if source == &SourceId::curseforge() {
            crate::api::curseforge::metadata_cache_scope()
        } else {
            String::new()
        }
    }
    async fn request(
        &self,
        source: &SourceId,
        request: ProtocolRequest,
    ) -> ResourceResult<Value> {
        if source == &SourceId::curseforge() {
            return crate::api::curseforge::resource_request(
                request.method,
                &request.path,
                request.query,
                request.body,
            )
            .await
            .map_err(Into::into);
        }
        let base = if source == &SourceId::modrinth() {
            let base = env!("MODRINTH_API_URL").trim_end_matches('/');
            if request.path.starts_with("/v3/") {
                base.trim_end_matches("/v2").to_string()
            } else {
                base.to_string()
            }
        } else if source == &SourceId::mcarchive() {
            "https://mcarchive.net/api/v1".into()
        } else if source == &SourceId::planet_minecraft() {
            crate::api::planet_minecraft::connector_base_url()
                .map_err(|_| ResourceError::Unavailable)?
        } else {
            return Err(ResourceError::Unsupported(
                "unknown source transport".into(),
            ));
        };
        let mut url = url::Url::parse(&format!("{base}{}", request.path))
            .map_err(|_| {
                ResourceError::InvalidRequest("invalid source URL".into())
            })?;
        url.query_pairs_mut().extend_pairs(request.query);
        if source == &SourceId::modrinth() {
            MODRINTH_REQUESTS.wait().await;
            let state = crate::State::get().await?;
            return crate::util::fetch::fetch_json(
                request.method,
                url.as_str(),
                None,
                request.body,
                None,
                &state.api_semaphore,
            )
            .await
            .map_err(Into::into);
        }
        if source == &SourceId::mcarchive() {
            crate::api::mcarchive::get_json(
                url.as_str(),
                request.fields.as_deref().unwrap_or("{}"),
            )
            .await
            .map_err(Into::into)
        } else {
            crate::api::planet_minecraft::connector_get(url.as_str())
                .await
                .map_err(Into::into)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_counts_describe_actual_batch_payloads() {
        let mut teams = ProtocolRequest::get("/teams");
        teams
            .query
            .push(("ids".into(), "[\"team1\",\"team2\"]".into()));
        assert_eq!(teams.item_count(), 2);
        assert_eq!(
            ProtocolRequest::post(
                "/version_files/update",
                serde_json::json!({"hashes":["a","b","c"]})
            )
            .item_count(),
            3
        );
        assert_eq!(
            ProtocolRequest::post(
                "/v1/mods/files",
                serde_json::json!({"fileIds":[1,2]})
            )
            .item_count(),
            2
        );
        assert_eq!(
            ProtocolRequest::get("/project/example/version").item_count(),
            1
        );
    }

    #[tokio::test]
    async fn concurrent_requests_share_a_paced_start_budget() {
        let pacer = RequestPacer::new(std::time::Duration::from_millis(30));
        let start = tokio::time::Instant::now();
        let mut arrivals = futures::future::join_all((0..4).map(|_| async {
            pacer.wait().await;
            start.elapsed()
        }))
        .await;
        arrivals.sort();
        for pair in arrivals.windows(2) {
            assert!(pair[1] - pair[0] >= std::time::Duration::from_millis(25));
        }
    }
}

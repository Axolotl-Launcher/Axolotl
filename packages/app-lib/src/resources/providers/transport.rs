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
    async fn request(
        &self,
        source: &SourceId,
        request: ProtocolRequest,
    ) -> ResourceResult<Value>;
}

pub(crate) struct LauncherTransport;

#[async_trait]
impl ProtocolTransport for LauncherTransport {
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
            let state = crate::State::get().await?;
            return crate::util::fetch::fetch_json(
                request.method,
                url.as_str(),
                None,
                request.body,
                None,
                &state.api_semaphore,
                &state.pool,
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

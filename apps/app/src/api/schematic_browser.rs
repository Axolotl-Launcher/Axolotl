//! IPC surface for the schematic browser.
//!
//! The commands are thin: they resolve the caller's arguments, refresh the
//! cached proxy flag once per batch, and delegate to
//! [`crate::api::schematic_browser`]. All policy lives in the library so the
//! commands stay testable without a Tauri runtime.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use serde::Serialize;
use tauri::Runtime;
use tokio_util::sync::CancellationToken;

use crate::api::schematic_browser::{self, error::SchematicSourceError, types};

/// In-flight requests, keyed by the caller-supplied `requestId`.
static CANCELLATIONS: LazyLock<Mutex<HashMap<String, CancellationToken>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn begin_request(request_id: &str) -> CancellationToken {
    let token = CancellationToken::new();
    let mut map = CANCELLATIONS.lock().unwrap();
    // A repeated id replaces the previous token; the older request keeps its own
    // clone and finishes on its own terms.
    map.insert(request_id.to_string(), token.clone());
    token
}

/// Remove a request's cancellation record, but only if it still belongs to this
/// call.
///
/// Ids can be reused. Removing by id alone would let an older request that
/// happens to finish late delete the record of a newer request with the same id,
/// which then could no longer be cancelled. Comparing the token identity makes
/// the removal safe.
fn end_request(request_id: &str, token: &CancellationToken) {
    let mut map = CANCELLATIONS.lock().unwrap();
    if let Some(existing) = map.get(request_id)
        && existing == token
    {
        map.remove(request_id);
    }
}

fn parse_source(source_id: &str) -> Result<types::SourceId, String> {
    types::SourceId::parse(source_id)
        .ok_or_else(|| format!("unknown source: {source_id}"))
}

fn to_message(error: SchematicSourceError) -> String {
    format!("{}: {}", error.code(), error)
}

/// Keep the cached proxy flag in step with the settings before a batch of
/// requests, so per-request admission does not have to read SQLite.
async fn refresh_proxy() {
    if let Ok(state) = crate::State::get().await
        && let Ok(config) = state.proxy_config().await
    {
        schematic_browser::http::refresh_proxy_state(config.mode);
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListPayload {
    pub items: Vec<types::SchematicItem>,
    pub page: u32,
    pub has_more: bool,
    pub total: types::ResourceTotal,
    pub source_status: types::SourceStatus,
    pub source_failures: Vec<types::SourceFailure>,
}

impl From<types::ListResult> for ListPayload {
    fn from(result: types::ListResult) -> Self {
        Self {
            items: result.items,
            page: result.page,
            has_more: result.has_more,
            total: result.total,
            source_status: result.source_status,
            source_failures: result.source_failures,
        }
    }
}

#[tauri::command]
pub async fn schematic_browser_list(
    source_id: String,
    page: Option<u32>,
    page_size: Option<u32>,
    query: Option<String>,
    versions: Option<Vec<String>>,
    request_id: String,
) -> Result<ListPayload, String> {
    // Register the cancellation token before anything that awaits, so a cancel
    // that arrives while the proxy settings are being refreshed still lands.
    let cancel = begin_request(&request_id);
    refresh_proxy().await;
    let source = parse_source(&source_id)?;
    let list_query = types::ListQuery {
        page: page.unwrap_or(1),
        page_size: page_size.unwrap_or(0),
        query,
        versions: versions.unwrap_or_default(),
    };
    let result = schematic_browser::list(source, &list_query, &cancel).await;
    end_request(&request_id, &cancel);
    result.map(ListPayload::from).map_err(to_message)
}

#[tauri::command]
pub async fn schematic_browser_detail(
    source_id: String,
    id: String,
    request_id: String,
) -> Result<types::SchematicDetail, String> {
    let cancel = begin_request(&request_id);
    refresh_proxy().await;
    let source = parse_source(&source_id)?;
    let result = schematic_browser::detail(source, &id, &cancel).await;
    end_request(&request_id, &cancel);
    result.map_err(to_message)
}

#[tauri::command]
pub async fn schematic_browser_download_plan(
    source_id: String,
    id: String,
    size: Option<types::GenerateSize>,
    request_id: String,
) -> Result<types::DownloadPlan, String> {
    let cancel = begin_request(&request_id);
    refresh_proxy().await;
    let source = parse_source(&source_id)?;
    let result =
        schematic_browser::download_plan(source, &id, size, &cancel).await;
    end_request(&request_id, &cancel);
    result.map_err(to_message)
}

#[tauri::command]
pub fn schematic_browser_cancel(request_id: String) -> bool {
    let token = CANCELLATIONS.lock().unwrap().remove(&request_id);
    match token {
        Some(token) => {
            token.cancel();
            true
        }
        None => false,
    }
}

#[tauri::command]
pub fn schematic_browser_capabilities() -> Vec<SourceCapabilityPayload> {
    schematic_browser::capabilities()
        .into_iter()
        .map(|(source_id, capabilities)| SourceCapabilityPayload {
            source_id: source_id.as_str().to_string(),
            display_name: source_id.display_name().to_string(),
            capabilities,
        })
        .collect()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceCapabilityPayload {
    pub source_id: String,
    pub display_name: String,
    pub capabilities: types::SourceCapabilities,
}

#[tauri::command]
pub async fn schematic_browser_health() -> Vec<types::SourceStatus> {
    refresh_proxy().await;
    schematic_browser::health().await
}

pub fn init<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("schematic-browser")
        .invoke_handler(tauri::generate_handler![
            schematic_browser_list,
            schematic_browser_detail,
            schematic_browser_download_plan,
            schematic_browser_cancel,
            schematic_browser_capabilities,
            schematic_browser_health,
        ])
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_sources_are_rejected() {
        assert!(parse_source("nope").is_err());
        assert!(parse_source("reden").is_ok());
    }

    #[test]
    fn cancelling_an_unknown_request_reports_false() {
        assert!(!schematic_browser_cancel("never-registered".into()));
    }

    #[test]
    fn a_registered_request_can_be_cancelled() {
        let token = begin_request("req-1");
        assert!(!token.is_cancelled());
        assert!(schematic_browser_cancel("req-1".into()));
        assert!(token.is_cancelled());
        // The entry is gone, so a second cancel is a no-op.
        assert!(!schematic_browser_cancel("req-1".into()));
    }

    #[test]
    fn capabilities_are_exposed_for_every_source() {
        let payloads = schematic_browser_capabilities();
        assert_eq!(payloads.len(), types::SourceId::all().len());
        assert!(payloads.iter().all(|p| !p.display_name.is_empty()));
    }
}

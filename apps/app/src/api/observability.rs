//! Frontend telemetry bridge.
//!
//! The launcher webview sends a small, bounded vocabulary of events through
//! this plugin. Every field is validated against an allowlist before it
//! reaches the telemetry registry: event names map to static identifiers,
//! attribute keys are fixed per event, and free-form string values are
//! reduced to identifier-shaped tokens. Payloads are never logged or echoed
//! back to the webview.

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Mutex, OnceLock, PoisonError};
use std::time::{Duration, Instant};

use launcher_observability::{AttributeValue, attrs, events};
use serde::{Deserialize, Serialize};

/// Largest number of events accepted in one call.
const MAX_EVENTS_PER_CALL: usize = 32;
/// Largest serialized payload accepted in one call.
const MAX_PAYLOAD_BYTES: usize = 16 * 1024;
/// Calls accepted per rate-limit window.
const MAX_CALLS_PER_WINDOW: usize = 100;
/// Rate-limit window.
const RATE_LIMIT_WINDOW: Duration = Duration::from_secs(60);
/// Longest accepted string attribute value.
const MAX_VALUE_LEN: usize = 64;
/// Durations beyond this are treated as malformed rather than recorded.
const MAX_DURATION_MS: u64 = 24 * 60 * 60 * 1000;

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("observability")
        .invoke_handler(tauri::generate_handler![
            observability_status,
            observability_record,
            observability_flush,
        ])
        .build()
}

/// Signals exposed by the running telemetry pipeline.
#[derive(Debug, Clone, Serialize)]
pub struct ObservabilityStatus {
    enabled: bool,
    signals: Vec<String>,
    session_id: Option<String>,
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub fn observability_status() -> ObservabilityStatus {
    let enabled = theseus::observability::is_enabled();
    ObservabilityStatus {
        enabled,
        signals: if enabled {
            theseus::observability::signals()
                .iter()
                .map(|signal| (*signal).to_owned())
                .collect()
        } else {
            Vec::new()
        },
        session_id: theseus::observability::session_id().map(str::to_owned),
    }
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub fn observability_record(events: Vec<FrontendEvent>) -> Result<(), String> {
    if !theseus::observability::is_enabled() {
        return Ok(());
    }
    if events.len() > MAX_EVENTS_PER_CALL {
        return Err("too many observability events".to_owned());
    }
    if !payload_fits(&events) {
        return Err("observability payload too large".to_owned());
    }
    if !allow_call(Instant::now()) {
        tracing::debug!(
            "observability rate limit reached; dropping frontend events"
        );
        return Ok(());
    }
    for event in &events {
        if emit_event(event).is_err() {
            return Err("observability event rejected".to_owned());
        }
    }
    Ok(())
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn observability_flush() -> bool {
    tauri::async_runtime::spawn_blocking(|| {
        theseus::observability::flush(Duration::from_secs(2))
    })
    .await
    .unwrap_or(false)
}

/// One event as sent by the launcher webview.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FrontendEvent {
    name: FrontendEventName,
    #[serde(default)]
    attributes: BTreeMap<String, FrontendValue>,
    #[serde(default)]
    duration_ms: Option<u64>,
}

/// Attribute values accepted from the webview. Floating point values are
/// accepted by the schema but no allowlisted attribute currently uses them.
#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
enum FrontendValue {
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(String),
}

/// The complete frontend event vocabulary. Unknown names fail to
/// deserialize, which keeps the wire format closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum FrontendEventName {
    PageView,
    Command,
    UiError,
    InstanceCreate,
    InstanceStart,
    InstanceStop,
    ProjectInstall,
    PackInstall,
    AccountLogin,
    AccountLogout,
    ThemeApply,
    AppUpdate,
}

impl FrontendEventName {
    /// Registry event name and the static action label carried in the
    /// `name` attribute, when the event identifies an action.
    fn definition(self) -> (&'static str, Option<&'static str>) {
        match self {
            Self::PageView => (events::UI_NAVIGATION, None),
            Self::Command => (events::UI_ACTION, None),
            Self::UiError => (events::UI_ERROR, None),
            Self::InstanceCreate => {
                (events::UI_ACTION, Some("instance_create"))
            }
            Self::InstanceStart => (events::UI_ACTION, Some("instance_start")),
            Self::InstanceStop => (events::UI_ACTION, Some("instance_stop")),
            Self::ProjectInstall => {
                (events::UI_ACTION, Some("project_install"))
            }
            Self::PackInstall => (events::UI_ACTION, Some("pack_install")),
            Self::AccountLogin => (events::UI_ACTION, Some("account_login")),
            Self::AccountLogout => (events::UI_ACTION, Some("account_logout")),
            Self::ThemeApply => (events::UI_ACTION, Some("theme_apply")),
            Self::AppUpdate => (events::UI_ACTION, Some("app_update")),
        }
    }

    /// Attributes the webview may attach to this event.
    fn allowed_attributes(self) -> &'static [AllowedAttribute] {
        match self {
            Self::PageView => &[AllowedAttribute::RouteName],
            Self::Command => &[
                AllowedAttribute::Name,
                AllowedAttribute::Outcome,
                AllowedAttribute::ErrorKind,
            ],
            Self::UiError => {
                &[AllowedAttribute::ErrorKind, AllowedAttribute::UiArea]
            }
            Self::InstanceCreate => &[
                AllowedAttribute::Source,
                AllowedAttribute::InstanceLoader,
                AllowedAttribute::InstanceGameVersion,
            ],
            Self::InstanceStart => &[
                AllowedAttribute::Source,
                AllowedAttribute::InstanceLoader,
                AllowedAttribute::InstanceGameVersion,
            ],
            Self::InstanceStop => &[
                AllowedAttribute::Source,
                AllowedAttribute::InstanceLoader,
                AllowedAttribute::InstanceGameVersion,
                AllowedAttribute::ExitCode,
            ],
            Self::ProjectInstall => &[
                AllowedAttribute::Source,
                AllowedAttribute::InstanceLoader,
                AllowedAttribute::InstanceGameVersion,
            ],
            Self::PackInstall => &[AllowedAttribute::Source],
            Self::AccountLogin => &[AllowedAttribute::Source],
            Self::AccountLogout => &[],
            Self::ThemeApply => &[
                AllowedAttribute::PackId,
                AllowedAttribute::UiArea,
                AllowedAttribute::Source,
            ],
            Self::AppUpdate => {
                &[AllowedAttribute::Source, AllowedAttribute::UpdateChannel]
            }
        }
    }
}

/// Attribute keys the webview may send, with their expected value shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AllowedAttribute {
    RouteName,
    ErrorKind,
    UiArea,
    Name,
    Outcome,
    Source,
    InstanceLoader,
    InstanceGameVersion,
    ExitCode,
    PackId,
    UpdateChannel,
}

impl AllowedAttribute {
    fn parse(key: &str) -> Option<Self> {
        match key {
            attrs::ROUTE_NAME => Some(Self::RouteName),
            attrs::ERROR_KIND => Some(Self::ErrorKind),
            attrs::UI_AREA => Some(Self::UiArea),
            attrs::NAME => Some(Self::Name),
            attrs::OUTCOME => Some(Self::Outcome),
            attrs::SOURCE => Some(Self::Source),
            attrs::INSTANCE_LOADER => Some(Self::InstanceLoader),
            attrs::INSTANCE_GAME_VERSION => Some(Self::InstanceGameVersion),
            attrs::EXIT_CODE => Some(Self::ExitCode),
            attrs::PACK_ID => Some(Self::PackId),
            attrs::UPDATE_CHANNEL => Some(Self::UpdateChannel),
            _ => None,
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::RouteName => attrs::ROUTE_NAME,
            Self::ErrorKind => attrs::ERROR_KIND,
            Self::UiArea => attrs::UI_AREA,
            Self::Name => attrs::NAME,
            Self::Outcome => attrs::OUTCOME,
            Self::Source => attrs::SOURCE,
            Self::InstanceLoader => attrs::INSTANCE_LOADER,
            Self::InstanceGameVersion => attrs::INSTANCE_GAME_VERSION,
            Self::ExitCode => attrs::EXIT_CODE,
            Self::PackId => attrs::PACK_ID,
            Self::UpdateChannel => attrs::UPDATE_CHANNEL,
        }
    }

    fn kind(self) -> ValueKind {
        match self {
            Self::RouteName => ValueKind::RouteName,
            Self::Name => ValueKind::CommandName,
            Self::ExitCode => ValueKind::Integer,
            _ => ValueKind::Identifier,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValueKind {
    /// Route names never contain separators, so a path cannot be smuggled in.
    RouteName,
    /// Identifier-shaped values; `|` and `/` are rejected.
    Identifier,
    /// Tauri command names, which may contain the plugin separator (`|`).
    CommandName,
    Integer,
}

fn emit_event(event: &FrontendEvent) -> Result<(), ()> {
    let (event_name, action) = event.name.definition();
    let allowed = event.name.allowed_attributes();
    let mut builder = theseus::observability::telemetry().event(event_name);
    if let Some(action) = action {
        builder = builder.attr(attrs::NAME, action);
    }
    for (key, value) in &event.attributes {
        let attribute = AllowedAttribute::parse(key).ok_or(())?;
        if !allowed.contains(&attribute) {
            return Err(());
        }
        if let Some(value) = convert_value(attribute, value) {
            builder = builder.attr(attribute.key(), value);
        }
    }
    if let Some(duration_ms) =
        event.duration_ms.filter(|value| *value <= MAX_DURATION_MS)
    {
        builder = builder.duration_ms(duration_ms);
    }
    builder.emit().map_err(|_| ())
}

fn convert_value(
    attribute: AllowedAttribute,
    value: &FrontendValue,
) -> Option<AttributeValue> {
    match (attribute.kind(), value) {
        (ValueKind::RouteName, FrontendValue::String(text)) => {
            sanitize_identifier(text, false).map(AttributeValue::String)
        }
        (ValueKind::Identifier, FrontendValue::String(text)) => {
            sanitize_identifier(text, false).map(AttributeValue::String)
        }
        (ValueKind::CommandName, FrontendValue::String(text)) => {
            sanitize_identifier(text, true).map(AttributeValue::String)
        }
        (ValueKind::Integer, FrontendValue::Integer(number)) => {
            Some(AttributeValue::I64(*number))
        }
        _ => None,
    }
}

/// Reduces a webview string to a short identifier-shaped token. Anything
/// else is dropped instead of being forwarded to the telemetry pipeline.
/// Path separators are always rejected; only command names may contain the
/// plugin separator (`plugin:updater|check`).
fn sanitize_identifier(value: &str, allow_pipe: bool) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || value.len() > MAX_VALUE_LEN {
        return None;
    }
    value
        .chars()
        .all(|character| {
            character.is_ascii_alphanumeric()
                || matches!(character, '.' | '-' | '_' | ':')
                || (allow_pipe && character == '|')
        })
        .then(|| value.to_owned())
}

fn payload_fits(events: &[FrontendEvent]) -> bool {
    serde_json::to_vec(events)
        .is_ok_and(|payload| payload.len() <= MAX_PAYLOAD_BYTES)
}

#[derive(Default)]
struct CallWindow {
    calls: VecDeque<Instant>,
}

fn allow_call(now: Instant) -> bool {
    static WINDOW: OnceLock<Mutex<CallWindow>> = OnceLock::new();
    let mut window = WINDOW
        .get_or_init(|| Mutex::new(CallWindow::default()))
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    while window
        .calls
        .front()
        .is_some_and(|start| now.duration_since(*start) >= RATE_LIMIT_WINDOW)
    {
        window.calls.pop_front();
    }
    if window.calls.len() >= MAX_CALLS_PER_WINDOW {
        return false;
    }
    window.calls.push_back(now);
    true
}

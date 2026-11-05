use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Deserialize, Serialize)]
pub struct SubmitRequest {
    pub attempt_id: String,
    pub thread_id: String,
    pub turn_id: String,
    pub provider_id: String,
    pub model_id: String,
    pub text: String,
    #[serde(default)]
    pub attachments: Vec<Value>,
    #[serde(default)]
    pub instance_id: Option<String>,
    #[serde(default)]
    pub instructions: String,
    #[serde(default = "locale")]
    pub locale: String,
    #[serde(default)]
    pub check_only: bool,
    #[serde(default)]
    pub computer_use: bool,
    #[serde(default)]
    pub verify_attempt: Option<String>,
    #[serde(default)]
    pub history_attempts: Option<Vec<String>>,
}

fn locale() -> String {
    "en-US".into()
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Job {
    pub lease_id: String,
    pub attempt_id: String,
    pub thread_id: String,
    pub provider_id: String,
    pub model_id: String,
    pub locale: String,
    pub context: String,
    pub instructions: String,
    pub messages: Vec<Value>,
    pub resume: Option<Value>,
    pub model_turns: usize,
    pub tool_calls: usize,
    pub check_only: bool,
}

#[derive(Deserialize)]
pub struct ToolRequest {
    pub call_id: String,
    pub name: String,
    pub arguments: serde_json::Map<String, Value>,
}

#[derive(Deserialize)]
pub struct ReviewAction {
    pub call_id: String,
    pub name: String,
    pub arguments: Value,
}

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum CheckpointRequest {
    Get {
        namespace: String,
        id: Option<String>,
    },
    List {
        namespace: String,
        id: Option<String>,
        before: Option<String>,
        limit: Option<u32>,
    },
    Put {
        namespace: String,
        record: agent_window::persistence::GraphCheckpoint,
    },
    PutWrites {
        namespace: String,
        id: String,
        writes: Vec<agent_window::persistence::GraphWrite>,
    },
}

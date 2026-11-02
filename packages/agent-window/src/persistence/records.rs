use super::{StoreError, Value};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttemptState {
    Queued,
    Running,
    Awaiting,
    Completed,
    Failed,
    Cancelled,
    Interrupted,
}

impl AttemptState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Awaiting => "awaiting",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Interrupted => "interrupted",
        }
    }

    pub(super) fn parse(value: &str) -> Result<Self, StoreError> {
        match value {
            "queued" => Ok(Self::Queued),
            "running" => Ok(Self::Running),
            "awaiting" => Ok(Self::Awaiting),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            "interrupted" => Ok(Self::Interrupted),
            _ => Err(StoreError::Conflict("Invalid stored attempt state")),
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed
                | Self::Failed
                | Self::Cancelled
                | Self::Interrupted
        )
    }

    pub(super) fn can_transition(self, to: Self) -> bool {
        !self.is_terminal()
            && self != to
            && match self {
                Self::Queued => matches!(
                    to,
                    Self::Running | Self::Cancelled | Self::Interrupted
                ),
                _ => to != Self::Queued,
            }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attempt {
    pub id: String,
    pub thread_id: String,
    pub turn_id: String,
    pub state: AttemptState,
    pub computer_use: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    pub sequence: i64,
    pub attempt_id: String,
    pub event_id: String,
    pub payload: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Approval {
    pub id: String,
    pub attempt_id: String,
    pub call_id: String,
    pub tool: String,
    pub arguments: Value,
    pub revision: i64,
    pub expires_at: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApprovalPermit {
    pub id: String,
    pub revision: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApprovalRecord {
    #[serde(flatten)]
    pub approval: Approval,
    pub state: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecutionRequest {
    pub attempt_id: String,
    pub call_id: String,
    pub tool: String,
    pub arguments: Value,
    pub is_write: bool,
    /// Trusted native-host classification; never deserialized from worker input.
    #[serde(skip)]
    pub recovery_control: bool,
    pub approval: Option<ApprovalPermit>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Execution {
    pub tool: String,
    pub arguments: Value,
    pub is_write: bool,
    pub state: String,
    pub result: Option<Value>,
}

#[derive(Debug)]
pub enum Admission {
    Execute,
    Existing(Execution),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Checkpoint {
    pub id: String,
    pub payload: Value,
}

//! Provider-neutral resource queries with independent, rebuildable caching.

pub use content_management::resources::*;

pub mod cache;
pub mod catalog;
pub(crate) mod execution;
pub mod export;
pub mod favorites;
pub mod history;
pub mod history_cleanup;
pub mod identity;
pub mod installed;
pub(crate) mod legacy_catalog;
mod naming;
pub(crate) mod packs;
pub mod plans;
pub(crate) mod protocol;
pub mod providers;
pub mod search;
mod service;
pub mod updates;
mod write_access;

pub(crate) use service::valid_artifact;
pub use service::{CachePolicy, QueryOutcome, ResourceService};

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ResourceError {
    #[error("resource cursor expired; restart the query")]
    CursorExpired,
    #[error("unsupported resource operation: {0}")]
    Unsupported(String),
    #[error("invalid resource request: {0}")]
    InvalidRequest(String),
    #[error("invalid source response: {0}")]
    InvalidResponse(String),
    #[error("resource source is unavailable")]
    Unavailable,
    #[error("no compatible release found")]
    NoCompatibleRelease,
    #[error("resource request failed (HTTP status {status:?})")]
    Network { status: Option<u16> },
    #[error("resource request canceled")]
    Canceled,
    #[error("resource cache unavailable: {0}")]
    Cache(String),
    #[error("resource database unavailable: {0}")]
    Database(String),
}

pub type ResourceResult<T> = Result<T, ResourceError>;

#[cfg(test)]
mod tests;

impl From<InvalidResourceIdentifier> for ResourceError {
    fn from(error: InvalidResourceIdentifier) -> Self {
        Self::InvalidResponse(error.to_string())
    }
}

impl From<serde_json::Error> for ResourceError {
    fn from(_: serde_json::Error) -> Self {
        Self::InvalidResponse("unexpected JSON schema".into())
    }
}

impl From<crate::Error> for ResourceError {
    fn from(error: crate::Error) -> Self {
        match &*error.raw {
            crate::ErrorKind::ResourceError(error) => error.clone(),
            crate::ErrorKind::FetchError(error) if error.is_decode() => {
                Self::InvalidResponse(error.to_string())
            }
            crate::ErrorKind::FetchError(error) => Self::Network {
                status: error.status().map(|status| status.as_u16()),
            },
            crate::ErrorKind::HttpError { status, .. } => Self::Network {
                status: Some(*status),
            },
            crate::ErrorKind::JSONError(_) => {
                Self::InvalidResponse(error.to_string())
            }
            crate::ErrorKind::Sqlx(_) | crate::ErrorKind::SqlxMigrate(_) => {
                Self::Database(error.to_string())
            }
            crate::ErrorKind::NetworkError(_)
            | crate::ErrorKind::ApiIsDownError(_) => {
                Self::Network { status: None }
            }
            crate::ErrorKind::DownloadResourceWaitTimeout(_) => {
                Self::Unavailable
            }
            crate::ErrorKind::InputError(_) => {
                Self::InvalidRequest(error.to_string())
            }
            _ => Self::InvalidResponse(error.to_string()),
        }
    }
}

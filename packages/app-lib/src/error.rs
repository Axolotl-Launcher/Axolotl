//! Theseus error type
use std::sync::Arc;

use crate::util;
use data_url::DataUrlError;
use derive_more::Display;
use serde::{Deserialize, Serialize};
use tracing_error::InstrumentError;

#[derive(Serialize, Deserialize, Debug, Display)]
#[display("{description}")]
pub struct LabrinthError {
    pub error: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,
}

#[derive(thiserror::Error, Debug)]
pub enum ErrorKind {
    #[error("{0:?}")]
    Any(eyre::Report),

    #[error("Filesystem error: {0}")]
    FSError(String),

    #[error("Serialization error (INI): {0}")]
    INIError(#[from] serde_ini::de::Error),

    #[error("Serialization error (JSON): {0}")]
    JSONError(#[from] serde_json::Error),

    #[error("Serialization error (NBT): {0}")]
    NBTError(#[from] quartz_nbt::io::NbtIoError),

    #[error("NBT data structure error: {0}")]
    NBTReprError(#[from] quartz_nbt::NbtReprError),

    #[error("Serialization error (websocket): {0}")]
    WebsocketSerializationError(
        #[from] ariadne::networking::serialization::SerializationError,
    ),

    #[error("Error parsing UUID: {0}")]
    UUIDError(#[from] uuid::Error),

    #[error("Error parsing URL: {0}")]
    URLError(#[from] url::ParseError),

    #[error("Unable to read {0} from any source")]
    NoValueFor(String),

    #[error("Metadata error: {0}")]
    MetadataError(#[from] daedalus::Error),

    #[error("Minecraft authentication error: {0}")]
    MinecraftAuthenticationError(
        #[from] crate::state::MinecraftAuthenticationError,
    ),

    #[error("I/O error: {0}")]
    IOError(#[from] util::io::IOError),

    #[error("I/O (std) error: {0}")]
    StdIOError(#[from] std::io::Error),

    #[error("Error launching Minecraft: {0}")]
    LauncherError(String),

    #[error("Error fetching URL: {0}")]
    FetchError(#[from] reqwest::Error),

    #[error("Network download error: {0}")]
    NetworkError(String),

    #[error("download endpoint changed; resume the validated partial file")]
    DownloadEndpointChanged,

    #[error("download transfer remained below its expected throughput")]
    DownloadTransferSlow,

    #[error("download {context} stream receive timed out")]
    DownloadStreamStalled { context: String },

    #[error("{0}")]
    DownloadResourceWaitTimeout(String),

    #[error("HTTP/2 transport error: {0}")]
    DownloadH2Error(#[from] h2::Error),

    #[error("HTTP {status} for {method} {url}")]
    HttpError {
        status: u16,
        method: String,
        url: String,
    },

    #[error("Too many API errors, try again in {0} minutes")]
    ApiIsDownError(u32),

    #[error("{0}")]
    LabrinthError(LabrinthError),

    #[error("Websocket error: {0}")]
    WSError(#[from] async_tungstenite::tungstenite::Error),

    #[error("Websocket closed before {0} could be received!")]
    WSClosedError(String),

    #[error("Incorrect Sha1 hash for download: {0} != {1}")]
    HashError(String, String),

    #[error("Regex error: {0}")]
    RegexError(#[from] regex::Error),

    #[error("Paths stored in the database need to be valid UTF-8: {0}")]
    UTFError(std::path::PathBuf),

    #[error("Invalid input: {0}")]
    InputError(String),

    #[error("{0}")]
    ResourceError(#[from] crate::resources::ResourceError),

    #[error(
        "The instance upgrade plan is stale (planned revision {planned_revision}, current revision {current_revision})"
    )]
    StaleInstanceUpgradePlan {
        planned_revision: u64,
        current_revision: u64,
    },

    #[error(
        "The instance upgrade plan filesystem source is stale for instance {instance_id}"
    )]
    StaleInstanceUpgradePlanSource { instance_id: String },

    #[error("Join handle error: {0}")]
    JoinError(#[from] tokio::task::JoinError),

    #[error("Recv error: {0}")]
    RecvError(#[from] tokio::sync::oneshot::error::RecvError),

    #[error("Error acquiring semaphore: {0}")]
    AcquireError(#[from] tokio::sync::AcquireError),

    #[error("Instance {0} is not managed by the app!")]
    UnmanagedInstanceError(String),

    #[error(
        "Instance {instance_id} is not ready while install stage is {stage}"
    )]
    InstanceNotReady { instance_id: String, stage: String },

    #[error("User is not logged in, no credentials available!")]
    NoCredentialsError,

    #[error("JRE error: {0}")]
    JREError(#[from] crate::util::jre::JREError),

    #[error("Error parsing date: {0}")]
    ChronoParseError(#[from] chrono::ParseError),

    #[error("Event error: {0}")]
    EventError(#[from] crate::event::EventError),

    #[error("Zip error: {0}")]
    ZipError(#[from] async_zip::error::ZipError),

    #[error("File watching error: {0}")]
    NotifyError(#[from] notify::Error),

    #[error("Error stripping prefix: {0}")]
    StripPrefixError(#[from] std::path::StripPrefixError),

    #[error("Error: {0}")]
    OtherError(String),

    #[cfg(feature = "tauri")]
    #[error("Tauri error: {0}")]
    TauriError(#[from] tauri::Error),

    #[error("Error interacting with database: {0}")]
    Sqlx(#[from] sqlx::Error),

    #[error("Unable to read {cache_type} cache: {message}")]
    CacheReadError {
        cache_type: String,
        message: String,
        sqlite_code: Option<String>,
    },

    #[error("Error while applying migrations: {0}")]
    SqlxMigrate(#[from] sqlx::migrate::MigrateError),

    #[error("Move directory error: {0}")]
    DirectoryMoveError(String),

    /// Carries the resolver's message rather than the upstream error type so a
    /// hickory-resolver minor bump cannot change this crate's public API.
    #[error("Error resolving DNS: {0}")]
    DNSError(String),

    #[error("An online profile for {user_name} is not available")]
    OnlineMinecraftProfileUnavailable { user_name: String },

    #[error("Invalid data URL: {0}")]
    InvalidDataUrl(#[from] DataUrlError),

    #[error("Invalid data URL: {0}")]
    InvalidDataUrlBase64(#[from] data_url::forgiving_base64::InvalidBase64),

    #[error("Invalid PNG")]
    InvalidPng,

    #[error("Invalid PNG: {0}")]
    PngDecodingError(#[from] png::DecodingError),

    #[error("PNG encoding error: {0}")]
    PngEncodingError(#[from] png::EncodingError),

    #[error(
        "A skin texture must have a dimension of either 64x64 or 64x32 pixels"
    )]
    InvalidSkinTexture,

    #[error("RPC error: {0}")]
    RpcError(String),

    #[cfg(windows)]
    #[error("Windows error: {0}")]
    WindowsError(#[from] windows_core::Error),

    #[error("zbus error: {0}")]
    ZbusError(#[from] zbus::Error),

    #[error("Deserialization error: {0}")]
    DeserializationError(#[from] serde::de::value::Error),

    #[error("Discord IPC error: {0}")]
    DiscordRichPresenceError(#[from] discord_rich_presence::error::Error),
}

#[derive(Debug)]
pub struct Error {
    pub raw: Arc<ErrorKind>,
    pub source: tracing_error::TracedError<Arc<ErrorKind>>,
    context: Option<String>,
}

impl Error {
    pub(crate) fn from_shared_kind(raw: Arc<ErrorKind>) -> Self {
        Self {
            source: Arc::clone(&raw).in_current_span(),
            raw,
            context: None,
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.source()
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(fmt, "{}", self.source)?;
        if let Some(context) = &self.context {
            write!(fmt, "\n{context}")?;
        }
        Ok(())
    }
}

impl Error {
    pub(crate) fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }

    /// Returns a user-facing message for this error, replacing raw database
    /// errors that are meaningless to end users with an actionable hint.
    ///
    /// SQLite foreign-key violation (code 787) surfaces when an instance is
    /// deleted while a content write is still in flight; the write-path
    /// validation already reports most of these cleanly, this is the final
    /// fallback at the Tauri boundary.
    ///
    /// A missing migration means the database was last opened by a newer build
    /// that recorded a schema this one does not know. sqlx refuses to continue,
    /// and its own wording ("previously applied but is missing in the resolved
    /// migrations") reads like internal corruption rather than the downgrade it
    /// actually is, so it is replaced with the situation and the two ways out.
    pub fn user_facing_message(&self) -> String {
        if let ErrorKind::Sqlx(sqlx::Error::Database(db_error)) =
            self.raw.as_ref()
            && db_error.code().as_deref() == Some("787")
        {
            return "This instance was deleted or is being modified; refresh the instance list and try again."
                .to_string();
        }
        if let ErrorKind::SqlxMigrate(
            sqlx::migrate::MigrateError::VersionMissing(version),
        ) = self.raw.as_ref()
        {
            return format!(
                "The app database was created by a newer Axolotl build (migration {version} is missing from this build), so this build cannot open it. Install the newer build again, or downgrade the database before starting this build."
            );
        }
        self.to_string()
    }
}

impl From<hickory_resolver::net::NetError> for ErrorKind {
    fn from(source: hickory_resolver::net::NetError) -> Self {
        Self::DNSError(source.to_string())
    }
}

impl<E: Into<ErrorKind>> From<E> for Error {
    fn from(source: E) -> Self {
        let error = Into::<ErrorKind>::into(source);
        let boxed_error = Arc::new(error);

        Self {
            raw: boxed_error.clone(),
            source: boxed_error.in_current_span(),
            context: None,
        }
    }
}

impl From<eyre::Report> for Error {
    fn from(value: eyre::Report) -> Self {
        let error = Arc::new(ErrorKind::Any(value));

        Self {
            raw: error.clone(),
            source: error.in_current_span(),
            context: None,
        }
    }
}

impl ErrorKind {
    pub fn as_error(self) -> Error {
        self.into()
    }
}

pub type Result<T> = core::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_migration_is_explained_as_a_downgrade() {
        let error: Error =
            sqlx::migrate::MigrateError::VersionMissing(20260909010000).into();
        let message = error.user_facing_message();

        assert!(
            message.contains("was created by a newer Axolotl build"),
            "the frontend matches on this wording, got: {message}"
        );
        assert!(
            message.contains("20260909010000"),
            "the message should name the migration that is missing, got: {message}"
        );
        assert!(
            !message.contains("previously applied but is missing"),
            "the raw sqlx wording should have been replaced, got: {message}"
        );
    }

    #[test]
    fn other_migration_failures_keep_their_own_message() {
        let error: Error =
            sqlx::migrate::MigrateError::VersionMismatch(20260909010000).into();

        assert_eq!(error.user_facing_message(), error.to_string());
    }
}

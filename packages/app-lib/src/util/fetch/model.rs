use crate::install::InstallProgressReporter;
use crate::util::download::constants::{
    BULK_SEGMENTED_DOWNLOAD_THRESHOLD, FETCH_ALIYUN, FETCH_ALTERNATE,
    FETCH_BMCLAPI, FETCH_MCIM, FETCH_OFFICIAL,
    LARGE_FILE_RANGE_CONNECTION_FLOOR, MAX_SEGMENT_CONCURRENCY,
    MIN_PARALLEL_RANGES, MIN_SEGMENT_SIZE,
};
use parking_lot::Mutex;
#[cfg(test)]
use reqwest::header;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, LazyLock, Weak};
use tokio::sync::Mutex as AsyncMutex;

#[derive(
    Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum ResourceClass {
    Metadata,
    MinecraftAsset,
    MinecraftLibrary,
    Loader,
    Java,
    Modrinth,
    CurseForge,
    Modpack,
    #[default]
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadRouteSource {
    Official,
    Bmclapi,
    Mcim,
    Aliyun,
    Alternate,
}

impl DownloadRouteSource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Official => FETCH_OFFICIAL,
            Self::Bmclapi => FETCH_BMCLAPI,
            Self::Mcim => FETCH_MCIM,
            Self::Aliyun => FETCH_ALIYUN,
            Self::Alternate => FETCH_ALTERNATE,
        }
    }
}

#[derive(
    Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum ProxyPolicy {
    #[default]
    System,
    Direct,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DownloadRoute {
    pub url: String,
    pub source: DownloadRouteSource,
    pub is_mirror: bool,
    pub allow_sensitive_headers: bool,
    pub supports_range: bool,
    pub proxy: ProxyPolicy,
}

#[derive(
    Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum ContentValidation {
    #[default]
    None,
    Json,
    Jar,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Integrity {
    pub size: Option<u64>,
    pub sha1: Option<String>,
    pub sha512: Option<String>,
    pub sha256: Option<String>,
    pub md5: Option<String>,
    pub content: ContentValidation,
}

impl Integrity {
    pub fn sha1(hash: impl Into<String>) -> Self {
        Self {
            sha1: Some(hash.into()),
            ..Self::default()
        }
    }

    pub fn with_size(mut self, size: u64) -> Self {
        self.size = Some(size);
        self
    }

    pub fn with_content_validation(
        mut self,
        content: ContentValidation,
    ) -> Self {
        self.content = content;
        self
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.size.is_none()
            && self.sha1.is_none()
            && self.sha512.is_none()
            && self.sha256.is_none()
            && self.md5.is_none()
            && self.content == ContentValidation::None
    }

    /// Resuming a partial download is only safe when a content hash can
    /// prove the stitched-together file is what the server intended.
    pub(crate) fn supports_resume(&self) -> bool {
        self.size.is_some() && self.has_hash()
    }

    pub(crate) fn has_hash(&self) -> bool {
        self.sha1.is_some()
            || self.sha512.is_some()
            || self.sha256.is_some()
            || self.md5.is_some()
    }
}

#[derive(Clone, Debug)]
pub struct DownloadRequest {
    pub url: String,
    pub resource: ResourceClass,
    pub integrity: Integrity,
    pub download_meta: Option<DownloadMeta>,
    pub header: Option<(String, String)>,
    pub candidate_urls: Vec<String>,
    /// Whether range-segmented (multi-connection) downloading is allowed.
    /// Batch schedulers restrict small files so they share a connection budget.
    pub allow_segmented_download: bool,
    /// Whether HTTP/1.1 may multiply one file into concurrent Range requests.
    /// Batch policy enables this for large items with a known integrity hash.
    pub(crate) allow_http1_segmented_download: bool,
    pub(crate) batch_download: bool,
    pub(crate) prefer_http1: bool,
    /// Minimum independent Range connections, bounded by data and resource limits.
    pub(crate) range_connection_floor: Option<usize>,
    /// Requested H2 range streams when the file does not qualify for
    /// independent connections. Streams on one socket share its bandwidth.
    pub(crate) h2_range_concurrency: Option<usize>,
    pub(crate) cancellation: Option<tokio_util::sync::CancellationToken>,
    pub(crate) install_tracking: Option<DownloadInstallTracking>,
    pub(crate) endpoint_migrations: Arc<std::sync::atomic::AtomicU8>,
    pub(crate) endpoint_migration_generations: Arc<Mutex<HashSet<(u64, u64)>>>,
    pub(crate) transfer_counters: Arc<
        crate::util::download::endpoint_selection::metrics::TransferCounters,
    >,
}

#[derive(Clone, Debug)]
pub(crate) struct DownloadInstallTracking {
    pub(crate) reporter: InstallProgressReporter,
    pub(crate) item_id: String,
    pub(crate) item_name: String,
}

impl DownloadRequest {
    pub fn new(url: impl Into<String>, resource: ResourceClass) -> Self {
        Self {
            url: url.into(),
            resource,
            integrity: Integrity::default(),
            download_meta: None,
            header: None,
            candidate_urls: Vec::new(),
            allow_segmented_download: true,
            allow_http1_segmented_download: true,
            batch_download: false,
            prefer_http1: false,
            range_connection_floor: None,
            h2_range_concurrency: None,
            cancellation: None,
            install_tracking: None,
            endpoint_migrations: Arc::new(std::sync::atomic::AtomicU8::new(0)),
            endpoint_migration_generations: Arc::default(),
            transfer_counters: Arc::default(),
        }
    }

    pub fn with_segmented_download(mut self, allow: bool) -> Self {
        self.allow_segmented_download = allow;
        self.allow_http1_segmented_download = allow;
        self
    }

    pub(crate) fn with_http1_segmented_download(mut self, allow: bool) -> Self {
        self.allow_http1_segmented_download = allow;
        self
    }

    /// Small batch items share connections. Large, hash-verified items may
    /// use spare connections for ranges, including after the batch thins out.
    pub(crate) fn with_batch_download_policy(mut self) -> Self {
        self.batch_download = true;
        self.allow_segmented_download = true;
        self.allow_http1_segmented_download = self.integrity.supports_resume()
            && self
                .integrity
                .size
                .is_some_and(|size| size >= BULK_SEGMENTED_DOWNLOAD_THRESHOLD);
        self
    }

    pub(crate) fn with_range_connection_floor(mut self, floor: usize) -> Self {
        self.range_connection_floor =
            Some(floor.clamp(1, MAX_SEGMENT_CONCURRENCY));
        self
    }

    pub(crate) fn supports_range_recovery(&self) -> bool {
        self.allow_segmented_download
            && (self.allow_http1_segmented_download || self.batch_download)
            && self.resource != ResourceClass::Metadata
            && self.integrity.has_hash()
    }

    pub(crate) fn prefers_independent_ranges(&self, size: u64) -> bool {
        self.supports_range_recovery()
            && self.allow_http1_segmented_download
            && (size >= BULK_SEGMENTED_DOWNLOAD_THRESHOLD
                || (self.range_connection_floor.is_some()
                    && size >= MIN_SEGMENT_SIZE * MIN_PARALLEL_RANGES as u64))
    }

    pub(crate) fn minimum_range_connections(&self, size: u64) -> usize {
        self.range_connection_floor.unwrap_or_else(|| {
            if self.prefers_independent_ranges(size) {
                LARGE_FILE_RANGE_CONNECTION_FLOOR
            } else {
                1
            }
        })
    }

    pub(crate) fn with_h2_range_concurrency(
        mut self,
        concurrency: usize,
    ) -> Self {
        self.h2_range_concurrency = Some(concurrency.max(1));
        self
    }

    pub fn with_integrity(mut self, integrity: Integrity) -> Self {
        self.integrity = integrity;
        self
    }

    pub fn with_download_meta(mut self, download_meta: DownloadMeta) -> Self {
        self.download_meta = Some(download_meta);
        self
    }

    pub fn with_header(
        mut self,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        self.header = Some((name.into(), value.into()));
        self
    }

    pub fn with_candidate_urls<I, S>(mut self, urls: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.candidate_urls.extend(urls.into_iter().map(Into::into));
        self
    }

    pub fn with_install_tracking(
        mut self,
        reporter: InstallProgressReporter,
        item_id: impl Into<String>,
        item_name: impl Into<String>,
    ) -> Self {
        self.cancellation = Some(reporter.cancellation_token());
        self.install_tracking = Some(DownloadInstallTracking {
            reporter,
            item_id: item_id.into(),
            item_name: item_name.into(),
        });
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DownloadResult {
    pub path: PathBuf,
    pub url: String,
    pub source: DownloadRouteSource,
    pub size: u64,
    pub attempts: usize,
    pub fallback_count: usize,
    #[serde(default)]
    pub verified_sha1: Option<String>,
    #[serde(default)]
    pub verified_sha512: Option<String>,
    #[serde(skip)]
    pub(crate) verified_file:
        Option<crate::util::download::verified_file::VerifiedFile>,
}

pub(crate) async fn attach_verified_integrity(
    mut result: DownloadResult,
    integrity: &Integrity,
) -> DownloadResult {
    result.verified_sha1 = integrity.sha1.clone();
    result.verified_sha512 = integrity.sha512.clone();
    result.verified_file = None;
    result
}

pub(crate) static IN_FLIGHT_DOWNLOADS: LazyLock<
    dashmap::DashMap<String, Weak<AsyncMutex<()>>>,
> = LazyLock::new(dashmap::DashMap::new);

#[derive(Debug, derive_more::Display, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[display(rename_all = "snake_case")]
pub enum DownloadReason {
    Standalone,
    Dependency,
    Modpack,
    Update,
}

#[derive(Debug, Clone, Serialize)]
pub struct DownloadMeta {
    pub reason: DownloadReason,
    pub game_version: String,
    pub loader: String,
    pub dependent_on: Option<String>,
}

impl DownloadMeta {
    pub fn to_header_value(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

pub type FetchProgressFn<'a> = dyn FnMut(
        u64,
        u64,
    ) -> Pin<Box<dyn Future<Output = crate::Result<()>> + Send + 'a>>
    + Send
    + 'a;

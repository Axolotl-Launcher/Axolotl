//! Central download configuration and protocol data.
//!
//! Durations, byte counts, budgets and stable strings belong here. Structural
//! zero/one values and literals required by formatting/derive macros stay at use sites.

use std::time::Duration;

/// Automatic file concurrency; the user-adjusted limits are retained.
pub(crate) const AUTO_DOWNLOAD_CONCURRENCY_INITIAL: usize = 96;
pub(crate) const AUTO_DOWNLOAD_CONCURRENCY_MAX: usize = 256;
pub(crate) const AUTO_DOWNLOAD_CONCURRENCY_MIN: usize = 32;
pub(crate) const AUTO_DOWNLOAD_CONCURRENCY_STEP: usize = 8;
pub(crate) const AUTO_DOWNLOAD_EXPANSION_ERROR_RATE: f64 = 0.1;
pub(crate) const AUTO_DOWNLOAD_MIN_GAIN_PERCENT: u64 = 101;
pub(crate) const AUTO_DOWNLOAD_PERCENT_BASE: u64 = 100;
pub(crate) const AUTO_DOWNLOAD_PRESSURE_ERROR_RATE: f64 = 0.05;
pub(crate) const AUTO_DOWNLOAD_PRESSURE_WINDOWS: usize = 8;
pub(crate) const AUTO_DOWNLOAD_PROBE_COOLDOWN: Duration =
    Duration::from_secs(30);
pub(crate) const AUTO_DOWNLOAD_SAMPLE_INTERVAL: Duration =
    Duration::from_secs(3);
pub(crate) const AUTO_DOWNLOAD_TRIAL_WINDOWS: usize = 2;
pub(crate) const AUTO_DOWNLOAD_UTILIZATION_WINDOWS: usize = 2;
pub(crate) const DOWNLOAD_CONCURRENCY_HARD_MAX: usize = 256;

/// Per-file Range splitting, retry and tail recovery.
pub(crate) const ADAPTIVE_RANGE_BYTES: u64 = 4 * 1024 * 1024;
pub(crate) const DOWNLOAD_FAIR_SHARE_DIVISOR: usize = 2;
pub(crate) const INITIAL_SEGMENT_CONCURRENCY: usize = 4;
pub(crate) const MAX_GLOBAL_TAIL_HEDGES: usize = 8;
pub(crate) const MAX_SEGMENT_CONCURRENCY: usize = 16;
pub(crate) const MAX_TAIL_HEDGES_PER_FILE: usize = 2;
pub(crate) const MIN_PARALLEL_RANGES: usize = 2;
pub(crate) const MIN_SEGMENT_SIZE: u64 = 256 * 1024;
pub(crate) const RANGE_CHECKPOINT_BYTES: u64 = 4 * 1024 * 1024;
pub(crate) const RANGE_FULL_FAILURE_LIMIT: u32 = 1;
#[cfg(not(test))]
pub(crate) const RANGE_IDLE_RECONNECT_TIMEOUT: Duration =
    Duration::from_secs(8);
pub(crate) const RANGE_JOURNAL_MAX_BYTES: u64 = 16 * 1024;
pub(crate) const RANGE_REDUCED_CONCURRENCY: usize = 2;
pub(crate) const RANGE_REDUCED_FAILURE_LIMIT: u32 = 3;
pub(crate) const RANGE_SPLITTING_DISABLE_THRESHOLD: u32 = 2;
pub(crate) const SEGMENTED_DOWNLOAD_THRESHOLD: u64 = 4 * 1024 * 1024;
pub(crate) const SEGMENT_EXPANSION_INTERVAL: Duration =
    Duration::from_millis(1500);
pub(crate) const SEGMENT_EXPANSION_SAMPLE_COUNT: usize = 3;
pub(crate) const SEGMENT_RETRY_ATTEMPTS: usize = 3;
#[cfg(not(test))]
pub(crate) const TAIL_HEDGE_IDLE_TIMEOUT: Duration = Duration::from_secs(5);
pub(crate) const TAIL_HEDGE_MIN_REMAINING: u64 = 16 * 1024 * 1024;
pub(crate) const TAIL_SPLIT_PERCENT: u64 = 40;

/// Shared authority and physical connection admission.
pub(crate) const ADAPTIVE_CONNECTION_COOLDOWN: Duration =
    Duration::from_secs(5);
pub(crate) const ADAPTIVE_CONNECTION_GAIN_PERCENT: u128 = 101;
pub(crate) const ADAPTIVE_CONNECTION_INITIAL_TARGET: usize = 2;
pub(crate) const ADAPTIVE_CONNECTION_MAX_TARGET: usize = 31;
pub(crate) const ADAPTIVE_CONNECTION_PERCENT_BASE: u128 = 100;
pub(crate) const ADAPTIVE_CONNECTION_RATE_SPREAD: u64 = 2;
pub(crate) const ADAPTIVE_CONNECTION_TRIAL_TIMEOUT: Duration =
    Duration::from_secs(25);
pub(crate) const ADAPTIVE_CONNECTION_TRIAL_WINDOWS: u8 = 2;
pub(crate) const ADAPTIVE_CONNECTION_WINDOW: Duration = Duration::from_secs(3);
pub(crate) const CONNECTION_FAILURE_COOLDOWN_WINDOWS: u32 = 4;
pub(crate) const CONNECTION_GROWTH_FACTOR: usize = 2;
pub(crate) const CONNECTION_STABILITY_WINDOW: Duration =
    Duration::from_millis(500);
pub(crate) const INITIAL_CONNECTION_TARGET: usize = 4;
pub(crate) const MAX_ASSET_H2_STREAMS_PER_AUTHORITY: usize = 32;
pub(crate) const MAX_CONNECTIONS_PER_AUTHORITY: usize = 32;
pub(crate) const MAX_CONNECTION_TARGET: usize = 64;
pub(crate) const MAX_H2_STREAMS_PER_AUTHORITY: usize = 256;
pub(crate) const MAX_PHYSICAL_DOWNLOAD_CONNECTIONS: usize = 256;

/// HTTP/2 transport, receive flow control and asset batches.
pub(crate) const ASSET_BATCH_CONCURRENCY: usize = 512;
pub(crate) const ASSET_BATCH_RETRY_PASSES: usize = 3;
pub(crate) const ASSET_RESOURCE_WAIT_TIMEOUT: Duration =
    Duration::from_secs(30);
pub(crate) const H2_ADDITIONAL_CONNECTIONS_ENABLED: bool = true;
pub(crate) const H2_ADMISSION_LOG_THRESHOLD: Duration =
    Duration::from_millis(25);
pub(crate) const H2_CONNECTION_ATTEMPTS: usize = 2;
pub(crate) const H2_CONNECTION_WAIT_TIMEOUT: Duration = Duration::from_secs(15);
pub(crate) const H2_CONNECTION_WINDOW: u32 = 64 * 1024 * 1024;
pub(crate) const H2_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
pub(crate) const H2_FALLBACK_TTL: Duration = Duration::from_secs(5 * 60);
pub(crate) const H2_HAPPY_EYEBALLS_IPV4_DELAY: Duration =
    Duration::from_millis(150);
pub(crate) const H2_HAPPY_EYEBALLS_IPV6_DELAY: Duration =
    Duration::from_millis(80);
pub(crate) const H2_IDLE_EVICTION_TIMEOUT: Duration =
    Duration::from_secs(5 * 60);
pub(crate) const H2_INITIAL_STREAM_WINDOW: u32 = 1024 * 1024;
pub(crate) const H2_LARGE_FILE_LIMIT: u64 = 64 * 1024 * 1024;
pub(crate) const H2_LARGE_MIN_SUCCESS_SAMPLES: u32 = 3;
pub(crate) const H2_LARGE_PREFERRED_SPEED_RATIO: f64 = 1.25;
pub(crate) const H2_MEDIUM_FILE_LIMIT: u64 = 4 * 1024 * 1024;
pub(crate) const H2_MIN_SUCCESS_SAMPLES: u32 = 2;
pub(crate) const H2_PREFERRED_SPEED_RATIO: f64 = 1.1;
pub(crate) const H2_RANGE_FILE_THRESHOLD: u64 = 2 * 1024 * 1024;
pub(crate) const H2_RANGE_MAX_WORKERS: usize = 8;
pub(crate) const H2_RANGE_RETRY_ATTEMPTS: usize = 4;
pub(crate) const H2_RANGE_RETRY_DELAY_MS: u64 = 100;
pub(crate) const H2_SLOW_FALLBACK_TTL: Duration = Duration::from_secs(30);
pub(crate) const H2_STREAM_READY_TIMEOUT: Duration = Duration::from_secs(30);
pub(crate) const H2_TLS_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
pub(crate) const MAX_PROGRESS_WAIT: Duration = Duration::from_secs(1);
pub(crate) const MIN_PROGRESS_BYTES: u64 = 256 * 1024;
pub(crate) const STREAM_RECV_TIMEOUT: Duration = Duration::from_secs(30);

/// Disk admission, pressure sampling and output buffering.
pub(crate) const HDD_RANGE_CONCURRENCY: usize = 1;
pub(crate) const LOCAL_CPU_PRESSURE_PERCENT: u32 = 95;
pub(crate) const LOCAL_FALLBACK_WRITE_CONCURRENCY: usize = 4;
pub(crate) const LOCAL_PRESSURE_SAMPLE_TTL: Duration = Duration::from_secs(10);
pub(crate) const LOCAL_WRITE_EWMA_ALPHA: f64 = 0.25;
pub(crate) const LOCAL_WRITE_PRESSURE_MS: f64 = 50.0;
pub(crate) const PRESSURE_RANGE_CONCURRENCY: usize = 2;
pub(crate) const RANGE_WRITER_BUFFER_CAPACITY: usize = 256 * 1024;
pub(crate) const SSD_RANGE_CONCURRENCY: usize = 8;
pub(crate) const STREAM_OUTPUT_CAPACITY: usize = 256 * 1024;
pub(crate) const STREAM_OUTPUT_FLUSH_INTERVAL: Duration =
    Duration::from_secs(1);
pub(crate) const UNKNOWN_DISK_RANGE_CONCURRENCY: usize = 4;

/// General HTTP fetch timing, progress, integrity and partial files.
pub(crate) const COLD_START_ROUTE_HEALTH_SAMPLE_THRESHOLD: u32 = 2;
pub(crate) const DOWNLOAD_BUFFER_BYTES: usize = 256 * 1024;
pub(crate) const DOWNLOAD_FAILURE_DIAGNOSTIC_ATTEMPTS: usize = 2;
pub(crate) const DOWNLOAD_INITIAL_SPEED_FLOOR: u64 = 256 * 1024;
pub(crate) const DOWNLOAD_PERCENT_BASE: u64 = 100;
pub(crate) const DOWNLOAD_PROGRESS_INTERVAL: Duration =
    Duration::from_millis(500);
pub(crate) const DOWNLOAD_PROGRESS_LOG_INTERVAL: u64 = 16 * 1024 * 1024;
pub(crate) const DOWNLOAD_PROGRESS_MIN_BYTES: u64 = 256 * 1024;
pub(crate) const DOWNLOAD_SCAN_CACHE_TTL: Duration =
    Duration::from_secs(30 * 60);
pub(crate) const DOWNLOAD_SCHEDULER_INTERVAL: Duration =
    Duration::from_millis(250);
pub(crate) const DOWNLOAD_SCHEDULING_GRACE: Duration = Duration::from_secs(1);
pub(crate) const DOWNLOAD_SPEED_FLOOR_PERCENT: u64 = 85;
pub(crate) const DOWNLOAD_SPEED_MIN_SAMPLES: usize = 5;
pub(crate) const DOWNLOAD_SPEED_RECENT_SAMPLES: usize = 10;
pub(crate) const DOWNLOAD_SPEED_SAMPLE_HISTORY: usize = 30;
pub(crate) const DOWNLOAD_SPEED_SAMPLE_INTERVAL_MS: u128 = 100;
pub(crate) const DOWNLOAD_STABILITY_WINDOW: Duration = Duration::from_secs(2);
pub(crate) const FETCH_RETRY_DELAYS: [Duration; 3] = [
    Duration::from_millis(100),
    Duration::from_millis(300),
    Duration::from_millis(750),
];
pub(crate) const FETCH_RETRY_JITTER_MAX: f64 = 1.15;
pub(crate) const FETCH_RETRY_JITTER_MIN: f64 = 0.85;
pub(crate) const FILE_H2_KEEPALIVE: Duration = Duration::from_secs(15);
pub(crate) const FILE_POOL_MAX_IDLE_PER_HOST: usize = 64;
pub(crate) const FILE_RETRY_SINGLE_STREAM_ROUNDS: [bool; 3] =
    [false, true, true];
pub(crate) const FILE_TCP_KEEPALIVE: Duration = Duration::from_secs(10);
pub(crate) const FILE_TRANSFER_CONNECT_TIMEOUT: Duration =
    Duration::from_secs(20);
#[cfg(not(test))]
pub(crate) const FILE_TRANSFER_FIRST_BYTE_TIMEOUT: Duration =
    Duration::from_secs(20);
#[cfg(not(test))]
pub(crate) const FILE_TRANSFER_READ_TIMEOUT: Duration = Duration::from_secs(60);
pub(crate) const FILE_VALIDATION_CONCURRENCY: usize = 8;
pub(crate) const HTTPS_PORT: u16 = 443;
#[cfg(not(test))]
pub(crate) const JOB_PROBE_WINDOW: Duration = Duration::from_secs(5 * 60);
pub(crate) const MAX_CERTIFICATE_DIAGNOSTIC_CHARS: usize = 512;
pub(crate) const MAX_DOWNLOAD_ATTEMPT_HISTORY: usize = 12;
pub(crate) const MAX_DOWNLOAD_DIAGNOSTIC_BYTES: usize = 8 * 1024;
pub(crate) const MAX_DOWNLOAD_REDIRECTS: usize = 5;
pub(crate) const MAX_FAILURE_COOLDOWN: Duration = Duration::from_secs(1);
pub(crate) const MAX_H2_RANGES_PER_FILE: usize = 16;
pub(crate) const MAX_REDIRECT_LOCATION_BYTES: usize = 8 * 1024;
pub(crate) const MAX_TASK_PROBE_STATES: usize = 64;
pub(crate) const METADATA_ATTEMPT_BUDGET: usize = 4;
pub(crate) const METADATA_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
#[cfg(not(test))]
pub(crate) const METADATA_HEDGE_DELAY: Duration = Duration::from_secs(2);
pub(crate) const MINECRAFT_DOWNLOAD_PROGRESS_MIN_BYTES: u64 = 256 * 1024;
pub(crate) const MODRINTH_CDN_ATTEMPTS: usize = 3;
pub(crate) const MODRINTH_CDN_ATTEMPT_TIMEOUT: Duration =
    Duration::from_secs(120);
#[cfg(not(test))]
pub(crate) const REASSIGNABLE_FIRST_BYTE_TIMEOUT: Duration =
    Duration::from_secs(5);
pub(crate) const RESOURCE_WAIT_TIMEOUT: Duration = Duration::from_secs(45);
pub(crate) const RETRY_AFTER_CAP: Duration = Duration::from_secs(60);
pub(crate) const STALE_PARTIAL_DOWNLOAD_MAX_AGE: Duration =
    Duration::from_secs(7 * 24 * 60 * 60);
pub(crate) const TASK_PROBE_MAX_ROUTES: usize = 3;
#[cfg(not(test))]
pub(crate) const TASK_PROBE_MAX_WAIT: Duration = Duration::from_secs(10);
#[cfg(not(test))]
pub(crate) const TASK_PROBE_WINDOW: Duration = Duration::from_secs(60);

/// Slow-route decisions, cooldowns and persisted transport reputation.
pub(crate) const NATIVE_BREAKER_DEFAULT_COOLDOWN: Duration =
    Duration::from_secs(1);
pub(crate) const NATIVE_BREAKER_FAILURE_THRESHOLD: u32 = 3;
pub(crate) const NATIVE_BREAKER_JITTER_MAX: f64 = 1.2;
pub(crate) const NATIVE_BREAKER_JITTER_MIN: f64 = 0.8;
pub(crate) const NATIVE_BREAKER_MAX_COOLDOWN: Duration =
    Duration::from_secs(60);
pub(crate) const NATIVE_BREAKER_MAX_EXPONENT: u32 = 6;
pub(crate) const NATIVE_REPUTATION_DISABLE_FAILURES: u32 = 3;
pub(crate) const NATIVE_REPUTATION_DISABLE_SECS: u64 = 120;
pub(crate) const NATIVE_REPUTATION_EWMA_ALPHA: f64 = 0.25;
pub(crate) const NATIVE_REPUTATION_MAX_AGE_SECS: u64 = 7 * 24 * 60 * 60;
pub(crate) const NATIVE_REPUTATION_MAX_ENTRIES: usize = 256;
pub(crate) const NATIVE_REPUTATION_SCHEMA_VERSION: u32 = 3;
pub(crate) const NATIVE_REPUTATION_WRITE_DELAY: Duration =
    Duration::from_secs(5);
pub(crate) const NATIVE_SLOW_ABSOLUTE_SPEED_FLOOR: u64 = 16 * 1024;
pub(crate) const NATIVE_SLOW_COLD_SPEED_FLOOR: u64 = 256 * 1024;
pub(crate) const NATIVE_SLOW_EXPECTED_PERCENT: u64 = 40;
#[cfg(not(test))]
pub(crate) const NATIVE_SLOW_IDLE_TIMEOUT: Duration = Duration::from_secs(20);
pub(crate) const NATIVE_SLOW_MIN_FLOW: Duration = Duration::from_secs(10);
pub(crate) const NATIVE_SLOW_MIN_SAVINGS: Duration = Duration::from_secs(2);
pub(crate) const NATIVE_SLOW_RECONNECT_OVERHEAD: Duration =
    Duration::from_millis(600);
pub(crate) const NATIVE_SLOW_REQUIRED_WINDOWS: u8 = 2;
pub(crate) const NATIVE_SLOW_WINDOW: Duration = Duration::from_secs(3);
pub(crate) const NATIVE_SWITCH_MIN_SAVINGS_RATIO: f64 = 0.15;
pub(crate) const NATIVE_SWITCH_PERCENT_BASE: u64 = 100;
pub(crate) const NATIVE_SWITCH_SPEED_PERCENT: u64 = 125;
pub(crate) const RECONNECT_OVERHEAD_MAX: Duration = Duration::from_secs(5);
pub(crate) const RECONNECT_OVERHEAD_MIN: Duration = Duration::from_millis(100);
pub(crate) const ROUTE_HEALTH_EWMA_ALPHA: f64 = 0.25;
pub(crate) const ROUTE_PROBE_BYTES: u64 = 256 * 1024;
pub(crate) const ROUTE_PROBE_CONCURRENCY: usize = 2;
pub(crate) const ROUTE_PROBE_MIN_IMPROVEMENT_PERCENT: u64 = 25;
pub(crate) const ROUTE_PROBE_TIMEOUT: Duration = Duration::from_secs(5);
pub(crate) const ROUTE_PROBE_TIMEOUT_FACTOR: u32 = 2;

/// Download DNS providers, bounded caches and lookup timing.
pub const BUILTIN_DOH_SERVERS: [&str; 3] = [
    "https://doh.pub/dns-query",
    "https://doh.pysio.online/dns-query",
    "https://cloudflare-dns.com/dns-query",
];
pub(crate) const DNS_CACHE_TTL: Duration = Duration::from_secs(5 * 60);
pub(crate) const DNS_CANDIDATE_CACHE_TTL: Duration = Duration::from_secs(30);
pub(crate) const DNS_CONNECTION_FAILURES_BEFORE_REFRESH: u8 = 2;
pub(crate) const DNS_FAILURE_CACHE_TTL: Duration = Duration::from_secs(2);
pub(crate) const DNS_FAMILY_COLLECTION_GRACE: Duration =
    Duration::from_millis(500);
pub(crate) const DNS_LOOKUP_TIMEOUT: Duration = Duration::from_secs(6);
pub(crate) const DNS_SUCCESSFUL_ADDRESS_CACHE_TTL: Duration =
    Duration::from_secs(10 * 60);
pub(crate) const DNS_TIMEOUT_GRACE: Duration = Duration::from_millis(250);
pub(crate) const DOH_CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
pub(crate) const DOH_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
// Reserve part of the DNS lookup deadline for system DNS when DoH is unavailable.
pub(crate) const DOH_FALLBACK_TIMEOUT: Duration = Duration::from_secs(3);
pub(crate) const DOWNLOAD_DNS_PREWARM_CONCURRENCY: usize = 8;
pub(crate) const MAX_DNS_MESSAGE_BYTES: usize = 65535;
pub(crate) const MAX_METADATA_HOSTS: usize = 128;

/// Endpoint selection, probe budgets, hint storage and Fastly catalog data.
pub(crate) const ENDPOINT_BUDGET_WINDOW: Duration = Duration::from_secs(3600);
pub(crate) const ENDPOINT_COOLDOWN: Duration = Duration::from_secs(60);
pub(crate) const ENDPOINT_DNS_TIMEOUT: Duration = Duration::from_secs(5);
pub(crate) const ENDPOINT_FAILURES_BEFORE_REFRESH: u8 = 2;
pub(crate) const ENDPOINT_GENERATION_HISTORY: usize = 2;
pub(crate) const ENDPOINT_HINT_LIFETIME: Duration = Duration::from_secs(5 * 60);
pub(crate) const ENDPOINT_HOURLY_BYTES: u64 = 64 * 1024 * 1024;
pub(crate) const ENDPOINT_MAX_CANDIDATES: usize = 16;
pub(crate) const ENDPOINT_MAX_MEASURED_CANDIDATES: usize = 4;
pub(crate) const ENDPOINT_MAX_RATE_SPREAD: u64 = 2;
pub(crate) const ENDPOINT_MIN_SAMPLE_TIME: Duration =
    Duration::from_millis(250);
pub(crate) const ENDPOINT_MIN_SUCCESS_SAMPLES: usize = 2;
pub(crate) const ENDPOINT_PROBE_ROUND_CONCURRENCY: usize = 1;
pub(crate) const ENDPOINT_ROLLBACK_PERCENT_BASE: u64 = 100;
pub(crate) const ENDPOINT_ROLLBACK_SPEED_PERCENT: u64 = 80;
pub(crate) const ENDPOINT_ROLLBACK_WINDOWS: u8 = 2;
pub(crate) const ENDPOINT_ROUND_BYTES: u64 = 32 * 1024 * 1024;
pub(crate) const ENDPOINT_ROUND_TIMEOUT: Duration = Duration::from_secs(20);
pub(crate) const ENDPOINT_SAMPLE_BYTES: u64 = 4 * 1024 * 1024;
pub(crate) const ENDPOINT_SAMPLE_TIMEOUT: Duration = Duration::from_secs(3);
pub(crate) const ENDPOINT_SELECTION_LIFETIME: Duration =
    Duration::from_secs(5 * 60);
pub(crate) const ENDPOINT_STORE_MAX_AGE: u64 = 7 * 24 * 60 * 60;
pub(crate) const ENDPOINT_STORE_MAX_BYTES: u64 = 64 * 1024;
pub(crate) const ENDPOINT_STORE_MAX_ENTRIES: usize = 32;
pub(crate) const ENDPOINT_STORE_VERSION: u8 = 1;
pub(crate) const ENDPOINT_VALIDATION_BYTES: u64 = 64 * 1024;
pub(crate) const ENDPOINT_VALIDATION_CONCURRENCY: usize = 3;
pub(crate) const ENDPOINT_VALIDATION_TIMEOUT: Duration = Duration::from_secs(3);
pub(crate) const FASTLY_CATALOG_LIFETIME: Duration =
    Duration::from_secs(90 * 24 * 60 * 60);
pub(crate) const FASTLY_CATALOG_VERSION: u32 = 1;
pub(crate) const FASTLY_POOL_THIRD_OCTETS: [u8; 4] = [1, 65, 129, 193];
pub(crate) const FASTLY_REVIEWED_UNIX_SECONDS: u64 = 1_792_540_800;
pub(crate) const FASTLY_SEED_PREFIXES: [[u8; 3]; 3] =
    [[146, 75, 113], [151, 101, 41], [146, 75, 93]];

/// Installation download pipelines and content finalization.
pub(crate) const AGGREGATE_FAILURE_PATH_CHAR_LIMIT: usize = 160;
pub(crate) const AGGREGATE_FAILURE_PATH_LIMIT: usize = 3;
pub(crate) const AUTO_RETRY_PASSES: usize = 2;
pub(crate) const CONTENT_CHECKPOINT_FILE_COUNT: usize = 25;
pub(crate) const CONTENT_CHECKPOINT_INTERVAL: Duration = Duration::from_secs(2);
pub(crate) const CONTENT_DATABASE_BATCH_SIZE: usize = 25;
pub(crate) const CONTENT_DATABASE_FLUSH_INTERVAL: Duration =
    Duration::from_millis(500);
pub(crate) const CURSEFORGE_H2_RANGE_THRESHOLD: u64 = 64 * 1024 * 1024;
pub(crate) const CURSEFORGE_OVERRIDE_EXTRACTION_CONCURRENCY: usize = 4;
pub(crate) const CURSEFORGE_REQUESTED_H2_RANGES: usize = 4;
pub(crate) const CURSEFORGE_VERIFY_BUFFER_SIZE: usize = 256 * 1024;
pub(crate) const FINALIZE_WAIT_TIMEOUT: Duration = Duration::from_secs(45);
pub(crate) const INSTALL_FILE_REPORTED_ATTEMPTS: u32 = 5;
pub(crate) const ITEM_FAILURE_REASON_CHAR_LIMIT: usize = 1_024;
pub(crate) const LIVE_PROGRESS_EMIT_INTERVAL: Duration =
    Duration::from_millis(100);
pub(crate) const LIVE_PROGRESS_MAX_WAIT: Duration = Duration::from_secs(1);
pub(crate) const LIVE_PROGRESS_MIN_BYTES: u64 = 64 * 1024;
pub(crate) const MAX_CONCURRENT_INSTALL_JOBS: usize = 1;
pub(crate) const MAX_PACK_FILE_DOWNLOAD_CONCURRENCY: usize = 4;
pub(crate) const MODPACK_DATABASE_BATCH_SIZE: usize = 25;
pub(crate) const MODPACK_DATABASE_FLUSH_INTERVAL: Duration =
    Duration::from_millis(500);
pub(crate) const MODPACK_DATABASE_QUEUE_CAPACITY: usize = 128;
pub(crate) const MODPACK_FILE_INSTALL_ATTEMPTS: usize = 3;
pub(crate) const MODPACK_METADATA_CONCURRENCY: usize = 4;
pub(crate) const MODPACK_REQUESTED_H2_RANGES: usize = 16;
pub(crate) const MODPACK_VERIFICATION_CONCURRENCY: usize = 4;
pub(crate) const MODPACK_VERIFICATION_QUEUE_CAPACITY: usize = 128;
pub(crate) const NATIVE_CONTENT_FINALIZE_CONCURRENCY: usize = 4;
pub(crate) const OVERRIDE_EXTRACTION_CONCURRENCY: usize = 4;
pub(crate) const PROGRESS_GRANULARITY: u64 = 1024 * 1024;

/// Origin hosts, repositories and download URL components.
pub(crate) const ALIYUN_MAVEN_BASE_URL: &str =
    "https://maven.aliyun.com/repository/public";
pub(crate) const BMCLAPI_BASE_URL: &str = "https://bmclapi2.bangbang93.com";
pub(crate) const CURSEFORGE_API_HOST: &str = "api.curseforge.com";
pub(crate) const CURSEFORGE_EDGE_HOST: &str = "edge.forgecdn.net";
pub(crate) const FABRIC_MAVEN: &str = "https://maven.fabricmc.net";
pub(crate) const FETCH_MAVEN: &str = "/maven/";
pub(crate) const FORGE_MAVEN: &str = "https://maven.minecraftforge.net";
pub(crate) const H2_POOL_HTTP_2_ROUTE_HAS_NO_AUTHORITY: &str =
    "HTTP/2 route has no authority";
pub(crate) const H2_RANGE_INVALID_RANGE_RESUME_URL: &str =
    "invalid range resume URL";
pub(crate) const LAUNCHER_META_MAVEN: &str =
    "https://launcher-meta.modrinth.com/maven";
pub(crate) const LIBRARIES_MAVEN: &str = "https://libraries.minecraft.net";
pub(crate) const MACHINA_LWJGL_RELEASE: &str = "https://github.com/MinecraftMachina/lwjgl/releases/download/2.9.4-20150209-mmachina.2/";
pub(crate) const MAVEN_CENTRAL: &str = "https://repo.maven.apache.org/maven2";
pub(crate) const MCIM_BASE_URL: &str = "https://mod.mcimirror.top";
pub(crate) const MODRINTH_API_HOST: &str = "api.modrinth.com";
pub(crate) const MODRINTH_CDN_AUTHORITY: &str = "cdn.modrinth.com:443";
pub(crate) const MODRINTH_CDN_HOST: &str = "cdn.modrinth.com";
pub(crate) const MODRINTH_CDN_ORIGIN: &str = "https://cdn.modrinth.com/";
pub(crate) const MODRINTH_DATA_PREFIX: &str = "/data/";
pub(crate) const MODRINTH_RETIRED_CDN_HOST: &str = "cdn-alt.modrinth.com";
pub(crate) const MOJANG_LWJGL_PATH: &str = "https://libraries.minecraft.net/org/lwjgl/lwjgl/lwjgl-platform/2.9.4-nightly-20150209/";
pub(crate) const NEOFORGE_MAVEN: &str = "https://maven.neoforged.net/releases";
pub(crate) const QUILT_MAVEN: &str =
    "https://maven.quiltmc.org/repository/release";
pub(crate) const SPONGE_MAVEN: &str = "https://repo.spongepowered.org/maven";
pub(crate) const TIANPAO_RETIRED_HOST: &str = "mod.tianpao.top";

/// Protocol identifiers, diagnostic reasons and error text.
pub(crate) const ALL_PROXY_ENV_LOWER: &str = "all_proxy";
pub(crate) const ALL_PROXY_ENV_UPPER: &str = "ALL_PROXY";
pub(crate) const API_KEY_HEADER: &str = "x-api-key";
pub(crate) const AUTHORIZATION_HEADER: &str = "Authorization";
pub(crate) const CATALOG_MODE_ON: &str = "on";
pub(crate) const DERIVED_VERSION_CACHE_FORMAT: &str = "5";
pub(crate) const DIAGNOSTICS_API_RESPONSE_FAILURE: &str =
    "API response failure";
pub(crate) const DIAGNOSTICS_BUDGET_EXHAUSTED: &str = "budget_exhausted";
pub(crate) const DIAGNOSTICS_CANCELLED: &str = "cancelled";
pub(crate) const DIAGNOSTICS_CERTIFICATE: &str = "certificate";
pub(crate) const DIAGNOSTICS_CHECKSUM: &str = "checksum";
pub(crate) const DIAGNOSTICS_CONNECT: &str = "connect";
pub(crate) const DIAGNOSTICS_CONTENT_RANGE: &str = "content-range";
pub(crate) const DIAGNOSTICS_DNS: &str = "dns";
pub(crate) const DIAGNOSTICS_DNS_ONLY: &str = "dns_only";
pub(crate) const DIAGNOSTICS_ENDPOINT_CHANGED: &str = "endpoint_changed";
pub(crate) const DIAGNOSTICS_HASH_MISMATCH: &str = "hash mismatch";
pub(crate) const DIAGNOSTICS_INSUFFICIENT_SAMPLE: &str = "insufficient_sample";
pub(crate) const DIAGNOSTICS_INTEGRITY: &str = "integrity";
pub(crate) const DIAGNOSTICS_INVALID_CERTIFICATE: &str = "invalid_certificate";
pub(crate) const DIAGNOSTICS_INVALID_ENCODING: &str = "invalid_encoding";
pub(crate) const DIAGNOSTICS_INVALID_RANGE: &str = "invalid_range";
pub(crate) const DIAGNOSTICS_IO: &str = "io";
pub(crate) const DIAGNOSTICS_I_O_FAILURE: &str = "I/O failure";
pub(crate) const DIAGNOSTICS_JSON_VALIDATION_FAILED: &str =
    "JSON validation failed";
pub(crate) const DIAGNOSTICS_LOCAL_PRESSURE: &str = "local_pressure";
pub(crate) const DIAGNOSTICS_LOOKUP: &str = "lookup";
pub(crate) const DIAGNOSTICS_MEASURING: &str = "measuring";
pub(crate) const DIAGNOSTICS_NETWORK: &str = "network";
pub(crate) const DIAGNOSTICS_NO_GAIN: &str = "no_gain";
pub(crate) const DIAGNOSTICS_NO_RESPONSE_RECEIVED: &str =
    "no response received";
pub(crate) const DIAGNOSTICS_N_OLDER_DIAGNOSTIC_DETAILS_OMITTED: &str =
    "\n- older diagnostic details omitted";
pub(crate) const DIAGNOSTICS_OTHER: &str = "other";
pub(crate) const DIAGNOSTICS_PROTOCOL: &str = "protocol";
pub(crate) const DIAGNOSTICS_PROXY_INELIGIBLE: &str = "proxy_ineligible";
pub(crate) const DIAGNOSTICS_RANGE: &str = "range";
pub(crate) const DIAGNOSTICS_RANGE_UNSUPPORTED: &str = "range_unsupported";
pub(crate) const DIAGNOSTICS_RESOLVE: &str = "resolve";
pub(crate) const DIAGNOSTICS_ROLLBACK: &str = "rollback";
pub(crate) const DIAGNOSTICS_RULE_INELIGIBLE: &str = "rule_ineligible";
pub(crate) const DIAGNOSTICS_SELECTED: &str = "selected";
pub(crate) const DIAGNOSTICS_SERVER_COOLDOWN: &str = "server_cooldown";
pub(crate) const DIAGNOSTICS_SLOW: &str = "slow";
pub(crate) const DIAGNOSTICS_SSL: &str = "ssl";
pub(crate) const DIAGNOSTICS_STALE_EPOCH: &str = "stale_epoch";
pub(crate) const DIAGNOSTICS_STALL: &str = "stall";
pub(crate) const DIAGNOSTICS_STREAM_RESET: &str = "stream_reset";
pub(crate) const DIAGNOSTICS_TIMEOUT: &str = "timeout";
pub(crate) const DIAGNOSTICS_TLS: &str = "tls";
pub(crate) const DIAGNOSTICS_TRUNCATED: &str = "truncated";
pub(crate) const DIAGNOSTICS_VALIDATION: &str = "validation";
pub(crate) const DOWNLOAD_DNS_APPLICATION_DNS_MESSAGE: &str =
    "application/dns-message";
pub(crate) const DOWNLOAD_DNS_CANDIDATE_DNS_HOST_QUERY_CAPACITY_EXHAUSTED:
    &str = "Candidate DNS host query capacity exhausted";
pub(crate) const DOWNLOAD_DNS_CANDIDATE_DNS_LOCK_WAIT_TIMED_OUT: &str =
    "Candidate DNS lock wait timed out";
pub(crate) const DOWNLOAD_DNS_CONFLICTING_DOH_CNAME_ANSWERS: &str =
    "Conflicting DoH CNAME answers";
pub(crate) const DOWNLOAD_DNS_CYCLIC_DOH_CNAME_CHAIN: &str =
    "Cyclic DoH CNAME chain";
pub(crate) const DOWNLOAD_DNS_DNS_HOST_OVERRIDE_MUST_BE_A_HOSTNAME_WITHOUT_A:
    &str = "DNS host override must be a hostname without a port";
pub(crate) const DOWNLOAD_DNS_DNS_RESPONSE_CONTAINED_NO_ADDRESSES: &str =
    "DNS response contained no addresses";
pub(crate) const DOWNLOAD_DNS_DOH_CNAME_OWNER_ALSO_CONTAINS_ADDRESS_RECORDS:
    &str = "DoH CNAME owner also contains address records";
pub(crate) const DOWNLOAD_DNS_DOH_RESPONSE_CONTAINED_NO_ADDRESSES: &str =
    "DoH response contained no addresses";
pub(crate) const DOWNLOAD_DNS_DOH_RESPONSE_IS_TOO_LARGE: &str =
    "DoH response is too large";
pub(crate) const DOWNLOAD_DNS_INVALID_OR_UNSUCCESSFUL_DOH_DNS_RESPONSE: &str =
    "Invalid or unsuccessful DoH DNS response";
pub(crate) const DOWNLOAD_DNS_MISSING_DNS_QUESTION: &str =
    "Missing DNS question";
pub(crate) const DOWNLOAD_DNS_NO_CANDIDATE_DNS_ANSWERS_BEFORE_DEADLINE: &str =
    "No candidate DNS answers before deadline";
pub(crate) const DOWNLOAD_DNS_NO_DOH_PROVIDERS_AVAILABLE: &str =
    "No DoH providers available";
pub(crate) const DOWNLOAD_DNS_NO_USABLE_DOH_ADDRESS_FAMILY_BEFORE_DEADLINE:
    &str = "No usable DoH address family before deadline";
pub(crate) const DOWNLOAD_DNS_SYSTEM_CANDIDATE_DNS_LOOKUP_TIMED_OUT: &str =
    "System candidate DNS lookup timed out";
pub const DOWNLOAD_META_HEADER: &str = "modrinth-download-meta";
pub(crate) const ENDPOINT_CATALOG_ENV: &str = "AXOLOTL_MODRINTH_CATALOG";
pub(crate) const ENDPOINT_SELECTION_ENV: &str = "AXOLOTL_MODRINTH_IP_SELECTION";
pub(crate) const ENDPOINT_STORE_FILE_NAME: &str =
    "modrinth-endpoint-hints.json";
pub(crate) const FASTLY_KNOWN_CNAME: &str = "72b69f.atlas.fastly.net";
pub(crate) const FETCH_ABORT: &str = "abort";
pub(crate) const FETCH_ACTIVE_SEGMENT_BODY: &str = "active segment body";
pub(crate) const FETCH_ALIYUN: &str = "aliyun";
pub(crate) const FETCH_ALTERNATE: &str = "alternate";
pub(crate) const FETCH_API: &str = "API";
pub(crate) const FETCH_AUTHORIZATION: &str = "authorization";
pub(crate) const FETCH_AXOLOTL_RETRY: &str = "axolotl_retry";
pub(crate) const FETCH_BMCLAPI: &str = "bmclapi";
pub(crate) const FETCH_BMCLAPI2_BANGBANG93_COM: &str =
    "bmclapi2.bangbang93.com";
pub(crate) const FETCH_BYTES: &str = "bytes ";
pub(crate) const FETCH_CDN: &str = "CDN";
pub(crate) const FETCH_CHECKPOINT_RESUME_FAILED_DUE_TO_LOCAL_I_O: &str =
    "Checkpoint resume failed due to local I/O";
pub(crate) const FETCH_CLEAN_RETRY: &str = "clean_retry";
pub(crate) const FETCH_CLEAR_PARTIAL_AND_RETRY: &str =
    "clear_partial_and_retry";
pub(crate) const FETCH_CLEAR_PARTIAL_AND_SWITCH: &str =
    "clear_partial_and_switch";
pub(crate) const FETCH_CLIENT_CONFIGURATION_SHOULD_BE_VALID: &str =
    "client configuration should be valid";
pub(crate) const FETCH_CONFIGURED_CONNECTION_LIMIT_IS_ONE: &str =
    "configured connection limit is one";
pub(crate) const FETCH_COOKIE: &str = "cookie";
pub(crate) const FETCH_COOLDOWN_AND_SWITCH: &str = "cooldown_and_switch";
pub(crate) const FETCH_COOLDOWN_THEN_RETRY: &str = "cooldown_then_retry";
pub(crate) const FETCH_CREATING_DOWNLOAD_FILE: &str = "creating download file";
pub(crate) const FETCH_DISABLE_RANGE_AND_RETRY_SINGLE: &str =
    "disable_range_and_retry_single";
pub(crate) const FETCH_DISABLE_RANGE_AND_SWITCH: &str =
    "disable_range_and_switch";
pub(crate) const FETCH_DOWNLOAD_CANCELED: &str = "download canceled";
pub(crate) const FETCH_DROP_ROUTE: &str = "drop_route";
pub(crate) const FETCH_DROP_ROUTE_AFTER_CLEAN_RETRY: &str =
    "drop_route_after_clean_retry";
pub(crate) const FETCH_EFFECTIVE_CONCURRENCY_CAP_REACHED: &str =
    "effective concurrency cap reached";
pub(crate) const FETCH_EO_CACHE_STATUS: &str = "eo-cache-status";
pub(crate) const FETCH_EXPANSION_COOLDOWN_ACTIVE: &str =
    "expansion cooldown active";
pub(crate) const FETCH_FALLBACK_OFFICIAL: &str = "fallback_official";
pub(crate) const FETCH_FALLBACK_SINGLE: &str = "fallback_single";
pub(crate) const FETCH_FETCH_PERMIT_BATCH_HAS_ENOUGH_PERMITS: &str =
    "fetch permit batch has enough permits";
pub(crate) const FETCH_FILES_MINECRAFTFORGE_NET: &str =
    "files.minecraftforge.net";
pub(crate) const FETCH_FINALIZING_DOWNLOAD: &str = "finalizing download";
pub(crate) const FETCH_GITHUB_COM: &str = "github.com";
pub(crate) const FETCH_GOAWAY: &str = "goaway";
pub(crate) const FETCH_HEDGE_FAILED: &str = "hedge_failed";
pub(crate) const FETCH_HTTP: &str = "http://";
pub(crate) const FETCH_HTTP2: &str = "http2";
pub(crate) const FETCH_HTTP_2: &str = "http/2";
pub(crate) const FETCH_ICONS: &str = "icons";
pub(crate) const FETCH_INSUFFICIENT_AGGREGATE_SPEED_SAMPLES: &str =
    "insufficient aggregate speed samples";
pub(crate) const FETCH_INVALID_CONTENT_RANGE: &str = "invalid Content-Range";
pub(crate) const FETCH_INVALID_HEDGED_CONTENT_RANGE: &str =
    "invalid hedged Content-Range";
pub(crate) const FETCH_JAVA_RUNTIME: &str = "java-runtime";
pub(crate) const FETCH_LAUNCHER_MOJANG_COM: &str = "launcher.mojang.com";
pub(crate) const FETCH_LIBRARIES_MINECRAFT_NET: &str =
    "libraries.minecraft.net";
pub(crate) const FETCH_MAVEN2: &str = "/maven2/";
pub(crate) const FETCH_MAVEN_ALIYUN_COM: &str = "maven.aliyun.com";
pub(crate) const FETCH_MCIM: &str = "mcim";
pub(crate) const FETCH_MEDIAFILEZ_FORGECDN_NET: &str =
    "mediafilez.forgecdn.net";
pub(crate) const FETCH_MELOONG_COM: &str = "meloong.com";
pub(crate) const FETCH_META_FABRICMC_NET: &str = "meta.fabricmc.net";
pub(crate) const FETCH_MIRROR: &str = "mirror";
pub(crate) const FETCH_MISSING: &str = "<missing>";
pub(crate) const FETCH_MOD_MCIMIRROR_TOP: &str = "mod.mcimirror.top";
pub(crate) const FETCH_MOMOT_RS: &str = "momot.rs";
pub(crate) const FETCH_MRPACK: &str = ".mrpack";
pub(crate) const FETCH_NCAUSED_BY: &str = "\nCaused by: ";
pub(crate) const FETCH_NO_GLOBAL_PERMIT_AVAILABLE: &str =
    "no global permit available";
pub(crate) const FETCH_OFFICIAL: &str = "official";
pub(crate) const FETCH_OPENING_DOWNLOAD_FILE: &str = "opening download file";
pub(crate) const FETCH_OPTIFINE_NET: &str = "optifine.net";
pub(crate) const FETCH_PART_RANGES_JSON: &str = ".part.ranges.json";
pub(crate) const FETCH_PART_RANGES_JSON_2: &str = ".part.ranges.json.";
pub(crate) const FETCH_PISTON_DATA_MOJANG_COM: &str = "piston-data.mojang.com";
pub(crate) const FETCH_PROTOCOL_ERROR: &str = "protocol error";
pub(crate) const FETCH_PROXIED_CLIENT_CONFIGURATION_SHOULD_BE_VALID: &str =
    "proxied client configuration should be valid";
pub(crate) const FETCH_PROXY_AUTHORIZATION: &str = "proxy-authorization";
pub(crate) const FETCH_PROXY_CONFIGURATION_SHOULD_BE_VALID: &str =
    "proxy configuration should be valid";
pub(crate) const FETCH_RANGE_BYTE_COUNT_MISMATCH: &str =
    "range byte count mismatch";
pub(crate) const FETCH_RANGE_END_OVERFLOW: &str = "range end overflow";
pub(crate) const FETCH_RANGE_RESPONSE_ENDED_BEFORE_EXPECTED_BOUNDARY: &str =
    "range response ended before expected boundary";
pub(crate) const FETCH_REFUSED_STREAM: &str = "refused stream";
pub(crate) const FETCH_RELEASES: &str = "/releases/";
pub(crate) const FETCH_REMOVING: &str = "removing";
pub(crate) const FETCH_REPO1_MAVEN_ORG: &str = "repo1.maven.org";
pub(crate) const FETCH_REPO_MAVEN_APACHE_ORG: &str = "repo.maven.apache.org";
pub(crate) const FETCH_RESOURCES_DOWNLOAD_MINECRAFT_NET: &str =
    "resources.download.minecraft.net";
pub(crate) const FETCH_RESOURCE_VALIDATOR_CHANGED_BETWEEN_RANGES: &str =
    "resource validator changed between ranges";
pub(crate) const FETCH_RESOURCE_VALIDATOR_CHANGED_DURING_TAIL_HEDGE: &str =
    "resource validator changed during tail hedge";
pub(crate) const FETCH_RESUMABLE_PARTIAL_USES_A_SINGLE_STREAM: &str =
    "resumable partial uses a single stream";
pub(crate) const FETCH_RESUME_OR_SWITCH: &str = "resume_or_switch";
pub(crate) const FETCH_RETRY_NEXT_ROUND: &str = "retry_next_round";
pub(crate) const FETCH_RETRY_SAME_ROUTE: &str = "retry_same_route";
pub(crate) const FETCH_SEGMENT: &str = ".segment-";
pub(crate) const FETCH_SEGMENTED_CONTENT_VALIDATION_FAILED: &str =
    "segmented content validation failed";
pub(crate) const FETCH_SELECTED_ENDPOINT_CHANGED: &str =
    "selected endpoint changed";
pub(crate) const FETCH_SERVER_IGNORED_RANGE_AND_RETURNED_200: &str =
    "server ignored Range and returned 200";
pub(crate) const FETCH_SERVER_RETURNED_HTTP_416_FOR_RANGE_REQUEST: &str =
    "server returned HTTP 416 for range request";
pub(crate) const FETCH_STOP: &str = "stop";
pub(crate) const FETCH_STREAM_ERROR: &str = "stream error";
pub(crate) const FETCH_SWITCH_ROUTE: &str = "switch_route";
pub(crate) const FETCH_SWITCH_ROUTE_OR_RETRY_ROUND: &str =
    "switch_route_or_retry_round";
pub(crate) const FETCH_TIMED_OUT_WAITING_FOR_CDN_CONNECTION_RESOURCES: &str =
    "timed out waiting for CDN connection resources";
pub(crate) const FETCH_TIMED_OUT_WAITING_FOR_DESTINATION_DOWNLOAD_LOCK: &str =
    "timed out waiting for destination download lock";
pub(crate) const FETCH_TIMED_OUT_WAITING_FOR_NATIVE_DOWNLOAD_RESOURCES: &str =
    "timed out waiting for native download resources";
pub(crate) const FETCH_TOO_LITTLE_DATA_REMAINS: &str =
    "too little data remains";
pub(crate) const FETCH_TOO_MANY_REDIRECTS: &str = "too many redirects";
pub(crate) const FETCH_UNCONFIGURED: &str = "unconfigured";
pub(crate) const FETCH_UNKNOWN: &str = "unknown";
pub(crate) const FETCH_V2_SESSION: &str = "/v2/session";
pub(crate) const FETCH_V2_USER: &str = "/v2/user";
pub(crate) const FETCH_V3_FRIEND: &str = "/v3/friend/";
pub(crate) const FETCH_V3_FRIENDS: &str = "/v3/friends";
pub(crate) const FETCH_V3_NOTIFICATION: &str = "/v3/notification";
pub(crate) const H2_DOWNLOAD_ASSET: &str = "asset";
pub(crate) const H2_DOWNLOAD_AXOLOTL_LAUNCHER: &str = "Axolotl Launcher";
pub(crate) const H2_DOWNLOAD_BELOW_EXPECTATION: &str = "below expectation";
pub(crate) const H2_DOWNLOAD_BYTES_0_0: &str = "bytes=0-0";
pub(crate) const H2_DOWNLOAD_CONTENT_RANGE: &str = "Content-Range";
pub(crate) const H2_DOWNLOAD_DOWNLOADED_ASSET_IS_EMPTY: &str =
    "downloaded asset is empty";
pub(crate) const H2_DOWNLOAD_DOWNLOADED_FILE_IS_EMPTY: &str =
    "downloaded file is empty";
pub(crate) const H2_DOWNLOAD_EMPTY: &str = "empty";
pub(crate) const H2_DOWNLOAD_GET: &str = "GET";
pub(crate) const H2_DOWNLOAD_HTTP_2: &str = "HTTP/2";
pub(crate) const H2_DOWNLOAD_INVALID_JAR: &str = "Invalid JAR";
pub(crate) const H2_DOWNLOAD_PROBE: &str = "probe";
pub(crate) const H2_DOWNLOAD_RANGE_STREAM: &str = "range stream";
pub(crate) const H2_DOWNLOAD_SEGMENT: &str = "segment";
pub(crate) const H2_DOWNLOAD_TIMED_OUT_WAITING_FOR_ASSET_DESTINATION_LOCK:
    &str = "timed out waiting for asset destination lock";
pub(crate) const H2_DOWNLOAD_TIMED_OUT_WAITING_FOR_ASSET_HTTP_2_STREAM_PERMIT: &str = "timed out waiting for asset HTTP/2 stream permit";
pub(crate) const H2_POOL_ADMISSION: &str = "admission";
pub(crate) const H2_POOL_ENDPOINT_CHANGED_BEFORE_H2_STREAM_ASSIGNMENT: &str =
    "Endpoint changed before H2 stream assignment";
pub(crate) const H2_POOL_ENDPOINT_CHANGED_WHILE_WAITING_FOR_THE_H2_CONNECTION_SLOT: &str = "Endpoint changed while waiting for the H2 connection slot";
pub(crate) const H2_POOL_ENDPOINT_DID_NOT_NEGOTIATE_H2: &str =
    "Endpoint did not negotiate h2";
pub(crate) const H2_POOL_ENDPOINT_SELECTION_CHANGED_DURING_H2_EXPANSION: &str =
    "Endpoint selection changed during H2 expansion";
pub(crate) const H2_POOL_ENDPOINT_SELECTION_CHANGED_DURING_THE_H2_HANDSHAKE:
    &str = "Endpoint selection changed during the H2 handshake";
pub(crate) const H2_POOL_H2_CONNECTION_DEADLINE_EXHAUSTED: &str =
    "H2 connection deadline exhausted";
pub(crate) const H2_POOL_HEADERS: &str = "headers";
pub(crate) const H2_POOL_HTTP_2_POLICY_REQUIRES_AN_EXISTING_SHARED_CONNECTION: &str = "HTTP/2 policy requires an existing shared connection";
pub(crate) const H2_POOL_HTTP_2_POOL_HAS_NO_LIVE_CONNECTION: &str =
    "HTTP/2 pool has no live connection";
pub(crate) const H2_POOL_PINNED_TCP_CONNECTION_TIMED_OUT: &str =
    "Pinned TCP connection timed out";
pub(crate) const H2_POOL_SHARED_HTTP_2_CONNECTION_IS_NOT_COVERED_BY_THE: &str =
    "shared HTTP/2 connection is not covered by the native connection budget";
pub(crate) const H2_RANGE_INCORRECT_SIZE: &str = "Incorrect size";
pub(crate) const HTTPS_PROXY_ENV_LOWER: &str = "https_proxy";
pub(crate) const HTTPS_PROXY_ENV_UPPER: &str = "HTTPS_PROXY";
pub(crate) const HTTPS_SCHEME: &str = "https";
pub(crate) const HTTP_PROXY_ENV_LOWER: &str = "http_proxy";
pub(crate) const HTTP_PROXY_ENV_UPPER: &str = "HTTP_PROXY";
pub(crate) const HTTP_SCHEME: &str = "http";
pub(crate) const IDENTITY_ENCODING: &str = "identity";
pub(crate) const INTEGRITY_HASH_FOR_DOWNLOAD: &str = " hash for download";
pub(crate) const INTEGRITY_INCORRECT: &str = "Incorrect ";
pub(crate) const INTEGRITY_MD5: &str = "md5";
pub(crate) const INTEGRITY_SHA1: &str = "sha1";
pub(crate) const INTEGRITY_SHA256: &str = "sha256";
pub(crate) const INTEGRITY_SHA512: &str = "sha512";
pub(crate) const MODEL_FASTLY_RULE: &str = "fastly_rule";
pub(crate) const MODEL_HISTORY: &str = "history";
pub(crate) const MODEL_VERIFIED: &str = "verified";
pub(crate) const MODRINTH_CDN_REPLACING_A_VALIDATED_HOSTNAME_PRESERVES_URL_VALIDITY: &str = "replacing a validated hostname preserves URL validity";
pub(crate) const MODRINTH_REDIRECT_HTTPS: &str = "https://";
pub(crate) const NATIVE_AUTHORITY_IS_TEMPORARILY_USING_HTTP_1_1: &str =
    "authority is temporarily using HTTP/1.1";
pub(crate) const NATIVE_AUTOCONFIGURL: &str = "AutoConfigURL";
pub(crate) const NATIVE_BUDGET_AUTHORITY_BATCH_CONTAINS_ENOUGH_PERMITS: &str =
    "authority batch contains enough permits";
pub(crate) const NATIVE_BUDGET_GLOBAL_BATCH_CONTAINS_ENOUGH_PERMITS: &str =
    "global batch contains enough permits";
pub(crate) const NATIVE_BUDGET_REGULAR_BATCH_CONTAINS_ENOUGH_PERMITS: &str =
    "regular batch contains enough permits";
pub(crate) const NATIVE_CERTIFICATE_POLICY_REQUIRES_THE_REQWEST_TRANSPORT:
    &str = "certificate policy requires the reqwest transport";
pub(crate) const NATIVE_CONFIGURED_PROXY_REQUIRES_THE_REQWEST_TRANSPORT: &str =
    "configured proxy requires the reqwest transport";
pub(crate) const NATIVE_DOWNLOAD_SOURCE_HAS_BEEN_RETIRED: &str =
    "download source has been retired";
pub(crate) const NATIVE_PROXYENABLE: &str = "ProxyEnable";
pub(crate) const NATIVE_REPUTATION_FILE_NAME: &str =
    "native-download-reputation.json";
pub(crate) const NATIVE_REPUTATION_TMP: &str = ".tmp";
pub(crate) const NATIVE_REQUEST_TIMED_OUT_WAITING_FOR_CDN_RESPONSE_CONNECTION_BUDGET: &str = "timed out waiting for CDN response connection budget";
pub(crate) const NATIVE_SOFTWARE_MICROSOFT_WINDOWS_CURRENTVERSION_INTERNET_SETTINGS: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings";
pub(crate) const NATIVE_SYSTEM_PROXY_REQUIRES_THE_REQWEST_TRANSPORT: &str =
    "system proxy requires the reqwest transport";
pub(crate) const PARTIAL_FILE_SUFFIX: &str = ".part";
pub(crate) const PROBE_CERTNOTVALIDFORNAME: &str = "certnotvalidforname";
pub(crate) const PROBE_X_CACHE: &str = "x-cache";
pub(crate) const PROBE_X_SERVED_BY: &str = "x-served-by";
pub(crate) const RANGE_JOURNAL_RANGE_CHECKPOINT_EXCEEDS_ITS_BOUNDS: &str =
    "range checkpoint exceeds its bounds";
pub(crate) const RANGE_JOURNAL_SUFFIX: &str = ".ranges.json";
pub(crate) const RANGE_OUTPUT_CREATING_RANGED_DOWNLOAD_OUTPUT: &str =
    "creating ranged download output";
pub(crate) const RANGE_OUTPUT_FLUSHING_RANGED_DOWNLOAD_OUTPUT: &str =
    "flushing ranged download output";
pub(crate) const RANGE_OUTPUT_INVALID_RANGE_BOUNDS: &str =
    "invalid range bounds";
pub(crate) const RANGE_OUTPUT_OFFSET_OVERFLOW: &str = "offset overflow";
pub(crate) const RANGE_OUTPUT_OPENING_RANGED_DOWNLOAD_OUTPUT: &str =
    "opening ranged download output";
pub(crate) const RANGE_OUTPUT_RANGE_OUTPUT_LENGTH_CHANGED: &str =
    "range output length changed";
pub(crate) const RANGE_OUTPUT_WRITE_EXCEEDS_RANGE: &str = "write exceeds range";
pub(crate) const RETIRED_SOURCES_FILES: &str = "/files/";
pub(crate) const RETIRED_SOURCES_MEDIA: &str = "/media/";
pub(crate) const RETIRED_SOURCES_MEDIA_2: &str = "/media";
pub(crate) const RETIRED_SOURCES_MEDIA_FORGECDN_NET: &str =
    "media.forgecdn.net";
pub(crate) const ROUTE_HEALTH_CURSEFORGE: &str = "curseforge";
pub(crate) const ROUTE_HEALTH_LOADER: &str = "loader";
pub(crate) const ROUTE_HEALTH_MINECRAFT: &str = "minecraft";
pub(crate) const ROUTE_HEALTH_MODRINTH: &str = "modrinth";
pub(crate) const ROUTE_POLICY_FABRICMC: &str = "fabricmc";
pub(crate) const ROUTE_POLICY_LAUNCHERMETA_MOJANG_COM: &str =
    "launchermeta.mojang.com";
pub(crate) const ROUTE_POLICY_MAVEN_FABRICMC_NET: &str = "maven.fabricmc.net";
pub(crate) const ROUTE_POLICY_MAVEN_MINECRAFTFORGE_NET: &str =
    "maven.minecraftforge.net";
pub(crate) const ROUTE_POLICY_MAVEN_NEOFORGED_NET: &str = "maven.neoforged.net";
pub(crate) const ROUTE_POLICY_MINECRAFTFORGE: &str = "minecraftforge";
pub(crate) const ROUTE_POLICY_NEOFORGED: &str = "neoforged";
pub(crate) const ROUTE_POLICY_PISTON_META_MOJANG_COM: &str =
    "piston-meta.mojang.com";
pub(crate) const ROUTE_POLICY_VERSION_MANIFEST: &str = "version_manifest";
pub(crate) const SELECTION_MODE_AUTO: &str = "auto";
pub(crate) const SELECTION_MODE_OBSERVE: &str = "observe";
pub(crate) const SELECTION_MODE_OFF: &str = "off";
pub(crate) const STORE_MISSING_HINTS_DIRECTORY: &str =
    "Missing hints directory";
pub(crate) const TIANPAO_UNRECOVERABLE_MESSAGE: &str = "This Tianpao download source has been retired and its official URL cannot be recovered. Re-resolve the file or choose a verified local file.";

/// Test-only transport fixtures and shortened timeouts.
#[cfg(test)]
pub(crate) const DOWNLOAD_DNS_TEST_DNS_LOOKUP_FAILED: &str =
    "test DNS lookup failed";
#[cfg(test)]
pub(crate) const FETCH_127_0_0_1: &str = "127.0.0.1";
#[cfg(test)]
pub(crate) const FETCH_LOCALHOST: &str = "localhost";
#[cfg(test)]
pub(crate) const FILE_TRANSFER_FIRST_BYTE_TIMEOUT: Duration =
    Duration::from_secs(2);
#[cfg(test)]
pub(crate) const FILE_TRANSFER_READ_TIMEOUT: Duration = Duration::from_secs(2);
#[cfg(test)]
pub(crate) const H2_CONNECTION_PROBE_TIMEOUT: Duration = Duration::from_secs(5);
#[cfg(test)]
pub(crate) const H2_DNS_REFRESH_TIMEOUT: Duration = Duration::from_secs(6);
#[cfg(test)]
pub(crate) const H2_POOL_TEST_INVALID_443: &str = "test.invalid:443";
#[cfg(test)]
pub(crate) const JOB_PROBE_WINDOW: Duration = Duration::from_secs(10);
#[cfg(test)]
pub(crate) const METADATA_HEDGE_DELAY: Duration = Duration::from_millis(100);
#[cfg(test)]
pub(crate) const NATIVE_SLOW_IDLE_TIMEOUT: Duration =
    Duration::from_millis(500);
#[cfg(test)]
pub(crate) const RANGE_IDLE_RECONNECT_TIMEOUT: Duration =
    Duration::from_millis(250);
#[cfg(test)]
pub(crate) const REASSIGNABLE_FIRST_BYTE_TIMEOUT: Duration =
    Duration::from_millis(500);
#[cfg(test)]
pub(crate) const TAIL_HEDGE_IDLE_TIMEOUT: Duration = Duration::from_millis(150);
#[cfg(test)]
pub(crate) const TASK_PROBE_MAX_WAIT: Duration = Duration::from_secs(2);
#[cfg(test)]
pub(crate) const TASK_PROBE_WINDOW: Duration = Duration::from_secs(5);
#[cfg(test)]
pub(crate) const TEST_BODY_SIZE: usize = 2 * 1024 * 1024;
#[cfg(test)]
pub(crate) const TEST_FRAME_SIZE: usize = 16 * 1024;

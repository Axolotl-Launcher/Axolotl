//! Attribute names and values used by first-party instrumentation.

/// Result of an operation: `ok`, `error`, `cancelled`, `timeout`, `skipped`.
pub const OUTCOME: &str = "outcome";
/// Stable error category, never an error message.
pub const ERROR_KIND: &str = "error.kind";
/// Instance UUID. Safe as a trace attribute; do not use as a metric label.
pub const INSTANCE_ID: &str = "instance.id";
/// Mod loader family, such as `fabric`, `forge`, `neoforge`, `quilt`.
pub const INSTANCE_LOADER: &str = "instance.loader";
/// Minecraft version string.
pub const INSTANCE_GAME_VERSION: &str = "instance.game_version";
/// What created an instance, such as `vanilla`, `modpack`, `import`.
pub const INSTANCE_SOURCE: &str = "instance.source";
/// Content provider, such as `modrinth`, `curseforge`, `mojang`, `maven`.
pub const PROVIDER: &str = "provider";
/// Network route: `official`, `cdn`, `mirror`.
pub const ROUTE: &str = "route";
/// Sanitized network host. Never include a full URL or path.
pub const NET_HOST: &str = "net.host";
/// HTTP status code.
pub const HTTP_STATUS: &str = "http.status";
/// Transfer size in bytes.
pub const BYTES: &str = "bytes";
/// Duration in milliseconds.
pub const DURATION_MS: &str = "duration_ms";
/// Persisted install job identifier.
pub const JOB_ID: &str = "job.id";
/// Install/launch phase name.
pub const PHASE: &str = "phase";
/// Theme pack identifier.
pub const PACK_ID: &str = "pack.id";
/// Theme pack page identifier.
pub const PAGE_ID: &str = "page.id";
/// MCP tool name.
pub const MCP_TOOL: &str = "mcp.tool";
/// MCP access level: `public`, `controlled`, `full`.
pub const MCP_ACCESS: &str = "mcp.access";
/// Update channel.
pub const UPDATE_CHANNEL: &str = "update.channel";
/// Frontend route name, never a path containing identifiers.
pub const ROUTE_NAME: &str = "route.name";
/// Frontend area or page name.
pub const UI_AREA: &str = "ui.area";
/// Numeric exit code from a game process.
pub const EXIT_CODE: &str = "exit.code";
/// Whether an operation ran in offline mode.
pub const OFFLINE: &str = "offline";
/// Major Java version.
pub const JAVA_MAJOR: &str = "java.major";
/// Java vendor.
pub const JAVA_VENDOR: &str = "java.vendor";
/// Database migration version.
pub const DB_MIGRATION_VERSION: &str = "db.migration.version";
/// Number of items found or processed.
pub const ITEM_COUNT: &str = "item.count";
/// Whether a resource came from cache.
pub const CACHE: &str = "cache";
/// Number of retries performed.
pub const RETRY_COUNT: &str = "retry.count";
/// Source of an application event, such as `ready` or `fallback`.
pub const SOURCE: &str = "source";
/// Generic step name.
pub const STEP: &str = "step";
/// Generic phase name.
pub const NAME: &str = "name";

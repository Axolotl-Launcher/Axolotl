//! The launcher's own settings, kept as one row.
//!
//! A few columns of the row belong to other readers: the proxy, the DNS server,
//! the download engine and the CurseForge key are written by the modules that
//! own them, so they are not part of this record and a write from here leaves
//! them as they were. `OwnedColumns` is how those modules reach them.
//!
//! The columns holding JSON are carried as the text they are stored as: what
//! the text means is the caller's business.

use sqlx::Row;
use sqlx::SqliteConnection;
use sqlx::SqlitePool;
use sqlx::sqlite::SqliteRow;

use super::Collection;

/// The settings, kept once for the whole launcher.
pub(crate) struct Settings;

/// There is one settings row per launcher.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Singleton;

/// Where a file may be downloaded from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DownloadSource {
    Auto,
    OfficialOnly,
    MirrorPreferred,
    OfficialPreferred,
}

impl DownloadSource {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::OfficialOnly => "official_only",
            Self::MirrorPreferred => "mirror_preferred",
            Self::OfficialPreferred => "official_preferred",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "auto" => Some(Self::Auto),
            "official_only" => Some(Self::OfficialOnly),
            "mirror_preferred" => Some(Self::MirrorPreferred),
            "official_preferred" => Some(Self::OfficialPreferred),
            _ => None,
        }
    }
}

/// How the home page is laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HomeLayout {
    Standard,
    Minimal,
}

impl HomeLayout {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Minimal => "minimal",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "standard" => Some(Self::Standard),
            "minimal" => Some(Self::Minimal),
            _ => None,
        }
    }
}

/// What the launcher's settings carry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SettingsRecord {
    pub max_concurrent_downloads: i64,
    pub max_concurrent_writes: i64,
    pub theme: String,
    pub default_page: String,
    pub collapsed_navigation: bool,
    pub advanced_rendering: bool,
    pub native_decorations: bool,
    pub telemetry: bool,
    pub discord_rpc: bool,
    pub developer_mode: bool,
    pub onboarded: bool,
    pub extra_launch_args: String,
    pub custom_env_vars: String,
    pub mc_memory_max: i64,
    pub mc_force_fullscreen: bool,
    pub mc_game_resolution_x: i64,
    pub mc_game_resolution_y: i64,
    pub hide_on_process_start: bool,
    pub hook_pre_launch: Option<String>,
    pub hook_wrapper: Option<String>,
    pub hook_post_exit: Option<String>,
    pub custom_dir: Option<String>,
    pub prev_custom_dir: Option<String>,
    pub migrated: bool,
    pub personalized_ads: bool,
    pub toggle_sidebar: bool,
    pub feature_flags: String,
    pub hide_nametag_skins_page: bool,
    pub nametag_uses_app_font: bool,
    pub skipped_update: Option<String>,
    pub pending_update_toast_for_version: Option<String>,
    pub auto_download_updates: Option<bool>,
    pub version: i64,
    pub locale: String,
    pub accent_color: String,
    pub custom_background_path: Option<String>,
    pub custom_background_blur: i64,
    pub custom_background_opacity: i64,
    pub use_minecraft_mirror: bool,
    pub use_modrinth_mirror: bool,
    pub use_curseforge_mirror: bool,
    pub auto_concurrent_downloads: bool,
    pub minecraft_metadata_source: DownloadSource,
    pub minecraft_file_source: DownloadSource,
    pub modrinth_source: DownloadSource,
    pub curseforge_source: DownloadSource,
    pub onboarding_version: i64,
    pub onboarding_instance_tour_completed: bool,
    pub sidebar_instance_count: i64,
    pub mc_memory_auto: bool,
    pub mc_memory_optimize: bool,
    pub auto_set_java_high_performance_mode: bool,
    pub transparent_background: bool,
    pub transparent_background_opacity: i64,
    pub transparent_background_blur: bool,
    pub home_layout: HomeLayout,
    pub minimal_home_instance_id: Option<String>,
    pub auto_hide_downloads_button: bool,
    pub home_widgets: Option<String>,
    pub mojang_auth_source: DownloadSource,
    pub terracotta_public_nodes: String,
    pub telemetry_consent_version: i64,
    pub bypass_curseforge_download_restrictions: bool,
    pub enter_lightweight_mode_on_game_launch: bool,
    pub close_behavior: String,
    pub mc_maximize_window: bool,
    pub log_level: String,
    pub sync_features_across_devices: bool,
    pub show_files_tab_in_instances: bool,
    pub show_worlds_tab_in_instances: bool,
    pub show_screenshots_tab_in_instances: bool,
    pub show_skin_selector_in_sidebar: bool,
    pub ignore_ssl_errors: bool,
    pub home_widget_background_opacity: i64,
    pub hidden_nav_items: String,
    pub custom_window_title_enabled: bool,
    pub default_window_title: String,
    pub custom_background_component_opacity: i64,
    pub ui_font: Option<String>,
    pub mono_font: Option<String>,
    pub backup_repository_path: Option<String>,
    pub allow_external_scheme: bool,
    pub allow_privileged_scheme: bool,
    pub doh_enabled: bool,
    pub resource_chinese_filenames: bool,
    pub resource_preserve_filenames: bool,
    pub default_game_language: String,
}

/// The settings columns their own modules write.
///
/// These are not part of `SettingsRecord`: the settings page writes back the
/// record it read, and a record carrying these would put the proxy and the
/// CurseForge key back to the values that read saw. The modules that own them
/// change them instead, so a write from either side leaves the other's columns
/// as they were.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OwnedColumns {
    pub proxy_mode: String,
    pub proxy_url: String,
    pub proxy_username: String,
    pub proxy_password: String,
    pub curseforge_api_key: String,
}

impl Settings {
    /// Changes the settings row inside the caller's unit, so the row the
    /// change is applied to cannot be replaced between the read and the write.
    pub(crate) async fn update_in(
        unit: &mut super::Atom,
        change: impl FnOnce(&mut SettingsRecord),
    ) -> crate::Result<()> {
        let mut record = <Self as Collection>::atom_get(unit, &Singleton)
            .await?
            .ok_or_else(missing_row)?;
        change(&mut record);
        <Self as Collection>::atom_set(unit, &Singleton, Some(&record)).await
    }

    /// Reads the columns the proxy and the CurseForge key live in.
    pub(crate) async fn owned_columns(
        handle: &super::Handle,
    ) -> crate::Result<OwnedColumns> {
        let mut connection = handle.connection().await?;
        read_owned_columns(&mut connection)
            .await?
            .ok_or_else(missing_row)
    }

    /// Changes the columns their own modules write inside the caller's unit,
    /// so the values read back are the ones the change is applied to.
    pub(crate) async fn update_owned_in(
        unit: &mut super::Atom,
        change: impl FnOnce(&mut OwnedColumns),
    ) -> crate::Result<()> {
        let mut columns = read_owned_columns(unit.connection())
            .await?
            .ok_or_else(missing_row)?;
        change(&mut columns);
        write_owned_columns(unit.connection(), &columns).await
    }
}

fn missing_row() -> crate::Error {
    crate::ErrorKind::OtherError(
        "The launcher settings row is missing".to_string(),
    )
    .into()
}

async fn read_owned_columns(
    conn: &mut SqliteConnection,
) -> crate::Result<Option<OwnedColumns>> {
    let row = sqlx::query(
        "
        SELECT
            proxy_mode, proxy_url, proxy_username, proxy_password,
            curseforge_api_key
        FROM settings
        WHERE id = 0
        ",
    )
    .fetch_optional(&mut *conn)
    .await?;

    let Some(row) = row else {
        return Ok(None);
    };

    Ok(Some(OwnedColumns {
        proxy_mode: row.try_get("proxy_mode")?,
        proxy_url: row.try_get("proxy_url")?,
        proxy_username: row.try_get("proxy_username")?,
        proxy_password: row.try_get("proxy_password")?,
        curseforge_api_key: row.try_get("curseforge_api_key")?,
    }))
}

async fn write_owned_columns(
    conn: &mut SqliteConnection,
    columns: &OwnedColumns,
) -> crate::Result<()> {
    sqlx::query(
        "
        UPDATE settings
        SET
            proxy_mode = ?,
            proxy_url = ?,
            proxy_username = ?,
            proxy_password = ?,
            curseforge_api_key = ?
        WHERE id = 0
        ",
    )
    .bind(&columns.proxy_mode)
    .bind(&columns.proxy_url)
    .bind(&columns.proxy_username)
    .bind(&columns.proxy_password)
    .bind(&columns.curseforge_api_key)
    .execute(&mut *conn)
    .await?;

    Ok(())
}

impl Collection for Settings {
    type Key = Singleton;
    type Scope = Singleton;
    type Record = SettingsRecord;

    fn in_scope(_scope: &Self::Scope, _key: &Self::Key) -> bool {
        true
    }

    async fn read(
        conn: &mut SqliteConnection,
        _key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let row = sqlx::query(
            "
            SELECT
                max_concurrent_downloads, max_concurrent_writes, theme,
                default_page, collapsed_navigation, advanced_rendering,
                native_decorations, telemetry, discord_rpc, developer_mode,
                onboarded, json(extra_launch_args) AS extra_launch_args,
                json(custom_env_vars) AS custom_env_vars, mc_memory_max,
                mc_force_fullscreen, mc_game_resolution_x, mc_game_resolution_y,
                hide_on_process_start, hook_pre_launch, hook_wrapper,
                hook_post_exit, custom_dir, prev_custom_dir, migrated,
                personalized_ads, toggle_sidebar,
                json(feature_flags) AS feature_flags,
                hide_nametag_skins_page, nametag_uses_app_font,
                skipped_update,
                pending_update_toast_for_version, auto_download_updates,
                version, locale, accent_color, custom_background_path,
                custom_background_blur, custom_background_opacity,
                use_minecraft_mirror, use_modrinth_mirror,
                use_curseforge_mirror, auto_concurrent_downloads,
                minecraft_metadata_source, minecraft_file_source,
                modrinth_source, curseforge_source, onboarding_version,
                onboarding_instance_tour_completed, sidebar_instance_count,
                mc_memory_auto, mc_memory_optimize,
                auto_set_java_high_performance_mode, transparent_background,
                transparent_background_opacity, transparent_background_blur,
                home_layout, minimal_home_instance_id, auto_hide_downloads_button,
                json(home_widgets) AS home_widgets, mojang_auth_source,
                json(terracotta_public_nodes) AS terracotta_public_nodes,
                telemetry_consent_version,
                bypass_curseforge_download_restrictions,
                enter_lightweight_mode_on_game_launch, close_behavior,
                mc_maximize_window, log_level, sync_features_across_devices,
                show_files_tab_in_instances, show_worlds_tab_in_instances,
                show_screenshots_tab_in_instances,
                show_skin_selector_in_sidebar, ignore_ssl_errors,
                home_widget_background_opacity, hidden_nav_items,
                custom_window_title_enabled, default_window_title,
                custom_background_component_opacity, ui_font, mono_font,
                backup_repository_path, allow_external_scheme,
                allow_privileged_scheme, doh_enabled,
                resource_chinese_filenames, resource_preserve_filenames,
                default_game_language
            FROM settings
            WHERE id = 0
            ",
        )
        .fetch_optional(&mut *conn)
        .await?;

        row.map(to_record).transpose()
    }

    async fn write(
        conn: &mut SqliteConnection,
        _key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let Some(record) = record else {
            sqlx::query("DELETE FROM settings WHERE id = 0")
                .execute(&mut *conn)
                .await?;

            return Ok(());
        };

        write_row(conn, record).await
    }

    async fn read_all(
        conn: &mut SqliteConnection,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        Ok(Self::read(conn, &Singleton)
            .await?
            .map(|record| vec![(Singleton, record)])
            .unwrap_or_default())
    }

    async fn replace_all(
        conn: &mut SqliteConnection,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        if records.is_empty() {
            sqlx::query("DELETE FROM settings")
                .execute(&mut *conn)
                .await?;
            return Ok(());
        }
        for (_, record) in records {
            write_row(conn, record).await?;
        }
        Ok(())
    }

    async fn read_scoped(
        conn: &mut SqliteConnection,
        scope: &Self::Scope,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        Ok(Self::read(conn, scope)
            .await?
            .map(|record| vec![(*scope, record)])
            .unwrap_or_default())
    }

    async fn replace_scoped(
        conn: &mut SqliteConnection,
        scope: &Self::Scope,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        Self::write(conn, scope, records.first().map(|(_, record)| record))
            .await
    }
}
fn to_record(row: SqliteRow) -> crate::Result<SettingsRecord> {
    let source = |name: &str| -> crate::Result<DownloadSource> {
        Ok(to_source(&row.try_get::<String, _>(name)?))
    };
    Ok(SettingsRecord {
        max_concurrent_downloads: row.try_get("max_concurrent_downloads")?,
        max_concurrent_writes: row.try_get("max_concurrent_writes")?,
        theme: row.try_get("theme")?,
        default_page: row.try_get("default_page")?,
        collapsed_navigation: row.try_get("collapsed_navigation")?,
        advanced_rendering: row.try_get("advanced_rendering")?,
        native_decorations: row.try_get("native_decorations")?,
        telemetry: row.try_get("telemetry")?,
        discord_rpc: row.try_get("discord_rpc")?,
        developer_mode: row.try_get("developer_mode")?,
        onboarded: row.try_get("onboarded")?,
        extra_launch_args: row.try_get("extra_launch_args")?,
        custom_env_vars: row.try_get("custom_env_vars")?,
        mc_memory_max: row.try_get("mc_memory_max")?,
        mc_force_fullscreen: row.try_get("mc_force_fullscreen")?,
        mc_game_resolution_x: row.try_get("mc_game_resolution_x")?,
        mc_game_resolution_y: row.try_get("mc_game_resolution_y")?,
        hide_on_process_start: row.try_get("hide_on_process_start")?,
        hook_pre_launch: row.try_get("hook_pre_launch")?,
        hook_wrapper: row.try_get("hook_wrapper")?,
        hook_post_exit: row.try_get("hook_post_exit")?,
        custom_dir: row.try_get("custom_dir")?,
        prev_custom_dir: row.try_get("prev_custom_dir")?,
        migrated: row.try_get("migrated")?,
        personalized_ads: row.try_get("personalized_ads")?,
        toggle_sidebar: row.try_get("toggle_sidebar")?,
        feature_flags: row.try_get("feature_flags")?,
        hide_nametag_skins_page: row.try_get("hide_nametag_skins_page")?,
        nametag_uses_app_font: row.try_get("nametag_uses_app_font")?,
        skipped_update: row.try_get("skipped_update")?,
        pending_update_toast_for_version: row
            .try_get("pending_update_toast_for_version")?,
        auto_download_updates: row.try_get("auto_download_updates")?,
        version: row.try_get("version")?,
        locale: row.try_get("locale")?,
        accent_color: row.try_get("accent_color")?,
        custom_background_path: row.try_get("custom_background_path")?,
        custom_background_blur: row.try_get("custom_background_blur")?,
        custom_background_opacity: row.try_get("custom_background_opacity")?,
        use_minecraft_mirror: row.try_get("use_minecraft_mirror")?,
        use_modrinth_mirror: row.try_get("use_modrinth_mirror")?,
        use_curseforge_mirror: row.try_get("use_curseforge_mirror")?,
        auto_concurrent_downloads: row.try_get("auto_concurrent_downloads")?,
        minecraft_metadata_source: source("minecraft_metadata_source")?,
        minecraft_file_source: source("minecraft_file_source")?,
        modrinth_source: source("modrinth_source")?,
        curseforge_source: source("curseforge_source")?,
        onboarding_version: row.try_get("onboarding_version")?,
        onboarding_instance_tour_completed: row
            .try_get("onboarding_instance_tour_completed")?,
        sidebar_instance_count: row.try_get("sidebar_instance_count")?,
        mc_memory_auto: row.try_get("mc_memory_auto")?,
        mc_memory_optimize: row.try_get("mc_memory_optimize")?,
        auto_set_java_high_performance_mode: row
            .try_get("auto_set_java_high_performance_mode")?,
        transparent_background: row.try_get("transparent_background")?,
        transparent_background_opacity: row
            .try_get("transparent_background_opacity")?,
        transparent_background_blur: row
            .try_get("transparent_background_blur")?,
        home_layout: to_layout(&row.try_get::<String, _>("home_layout")?),
        minimal_home_instance_id: row.try_get("minimal_home_instance_id")?,
        auto_hide_downloads_button: row
            .try_get("auto_hide_downloads_button")?,
        home_widgets: row.try_get("home_widgets")?,
        mojang_auth_source: source("mojang_auth_source")?,
        terracotta_public_nodes: row.try_get("terracotta_public_nodes")?,
        telemetry_consent_version: row.try_get("telemetry_consent_version")?,
        bypass_curseforge_download_restrictions: row
            .try_get("bypass_curseforge_download_restrictions")?,
        enter_lightweight_mode_on_game_launch: row
            .try_get("enter_lightweight_mode_on_game_launch")?,
        close_behavior: row.try_get("close_behavior")?,
        mc_maximize_window: row.try_get("mc_maximize_window")?,
        log_level: row.try_get("log_level")?,
        sync_features_across_devices: row
            .try_get("sync_features_across_devices")?,
        show_files_tab_in_instances: row
            .try_get("show_files_tab_in_instances")?,
        show_worlds_tab_in_instances: row
            .try_get("show_worlds_tab_in_instances")?,
        show_screenshots_tab_in_instances: row
            .try_get("show_screenshots_tab_in_instances")?,
        show_skin_selector_in_sidebar: row
            .try_get("show_skin_selector_in_sidebar")?,
        ignore_ssl_errors: row.try_get("ignore_ssl_errors")?,
        home_widget_background_opacity: row
            .try_get("home_widget_background_opacity")?,
        hidden_nav_items: row.try_get("hidden_nav_items")?,
        custom_window_title_enabled: row
            .try_get("custom_window_title_enabled")?,
        default_window_title: row.try_get("default_window_title")?,
        custom_background_component_opacity: row
            .try_get("custom_background_component_opacity")?,
        ui_font: row.try_get("ui_font")?,
        mono_font: row.try_get("mono_font")?,
        backup_repository_path: row.try_get("backup_repository_path")?,
        allow_external_scheme: row.try_get("allow_external_scheme")?,
        allow_privileged_scheme: row.try_get("allow_privileged_scheme")?,
        doh_enabled: row.try_get("doh_enabled")?,
        resource_chinese_filenames: row
            .try_get("resource_chinese_filenames")?,
        resource_preserve_filenames: row
            .try_get("resource_preserve_filenames")?,
        default_game_language: row.try_get("default_game_language")?,
    })
}
async fn write_row(
    conn: &mut SqliteConnection,
    record: &SettingsRecord,
) -> crate::Result<()> {
    sqlx::query(
        "
        INSERT INTO settings (
            id, max_concurrent_downloads, max_concurrent_writes, theme,
            default_page, collapsed_navigation, advanced_rendering,
            native_decorations, telemetry, discord_rpc, developer_mode,
            onboarded, extra_launch_args, custom_env_vars, mc_memory_max,
            mc_force_fullscreen, mc_game_resolution_x, mc_game_resolution_y,
            hide_on_process_start, hook_pre_launch, hook_wrapper,
            hook_post_exit, custom_dir, prev_custom_dir, migrated,
            personalized_ads, toggle_sidebar, feature_flags,
            hide_nametag_skins_page, nametag_uses_app_font,
            skipped_update,
            pending_update_toast_for_version, auto_download_updates, version,
            locale, accent_color, custom_background_path,
            custom_background_blur, custom_background_opacity,
            use_minecraft_mirror, use_modrinth_mirror, use_curseforge_mirror,
            auto_concurrent_downloads, minecraft_metadata_source,
            minecraft_file_source, modrinth_source, curseforge_source,
            onboarding_version, onboarding_instance_tour_completed,
            sidebar_instance_count, mc_memory_auto, mc_memory_optimize,
            auto_set_java_high_performance_mode, transparent_background,
            transparent_background_opacity, transparent_background_blur,
            home_layout, minimal_home_instance_id, auto_hide_downloads_button,
            home_widgets, mojang_auth_source, terracotta_public_nodes,
            telemetry_consent_version, bypass_curseforge_download_restrictions,
            enter_lightweight_mode_on_game_launch, close_behavior,
            mc_maximize_window, log_level, sync_features_across_devices,
            show_files_tab_in_instances, show_worlds_tab_in_instances,
            show_screenshots_tab_in_instances, show_skin_selector_in_sidebar,
            ignore_ssl_errors, home_widget_background_opacity,
            hidden_nav_items, custom_window_title_enabled,
            default_window_title, custom_background_component_opacity,
            ui_font, mono_font, backup_repository_path, allow_external_scheme,
            allow_privileged_scheme, doh_enabled, resource_chinese_filenames,
            resource_preserve_filenames, default_game_language
        )
        VALUES (
            0, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, jsonb(?),
            jsonb(?), ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?,
            ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?,
            ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?,
            ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, jsonb(?), ?,
            jsonb(?), ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?,
            ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?,
            ?, ?, ?
        )
        ON CONFLICT (id) DO UPDATE SET
            max_concurrent_downloads = excluded.max_concurrent_downloads,
            max_concurrent_writes = excluded.max_concurrent_writes,
            theme = excluded.theme,
            default_page = excluded.default_page,
            collapsed_navigation = excluded.collapsed_navigation,
            advanced_rendering = excluded.advanced_rendering,
            native_decorations = excluded.native_decorations,
            telemetry = excluded.telemetry,
            discord_rpc = excluded.discord_rpc,
            developer_mode = excluded.developer_mode,
            onboarded = excluded.onboarded,
            extra_launch_args = excluded.extra_launch_args,
            custom_env_vars = excluded.custom_env_vars,
            mc_memory_max = excluded.mc_memory_max,
            mc_force_fullscreen = excluded.mc_force_fullscreen,
            mc_game_resolution_x = excluded.mc_game_resolution_x,
            mc_game_resolution_y = excluded.mc_game_resolution_y,
            hide_on_process_start = excluded.hide_on_process_start,
            hook_pre_launch = excluded.hook_pre_launch,
            hook_wrapper = excluded.hook_wrapper,
            hook_post_exit = excluded.hook_post_exit,
            custom_dir = excluded.custom_dir,
            prev_custom_dir = excluded.prev_custom_dir,
            migrated = excluded.migrated,
            personalized_ads = excluded.personalized_ads,
            toggle_sidebar = excluded.toggle_sidebar,
            feature_flags = excluded.feature_flags,
            hide_nametag_skins_page = excluded.hide_nametag_skins_page,
            nametag_uses_app_font = excluded.nametag_uses_app_font,
            skipped_update = excluded.skipped_update,
            pending_update_toast_for_version =
                excluded.pending_update_toast_for_version,
            auto_download_updates = excluded.auto_download_updates,
            version = excluded.version,
            locale = excluded.locale,
            accent_color = excluded.accent_color,
            custom_background_path = excluded.custom_background_path,
            custom_background_blur = excluded.custom_background_blur,
            custom_background_opacity = excluded.custom_background_opacity,
            use_minecraft_mirror = excluded.use_minecraft_mirror,
            use_modrinth_mirror = excluded.use_modrinth_mirror,
            use_curseforge_mirror = excluded.use_curseforge_mirror,
            auto_concurrent_downloads = excluded.auto_concurrent_downloads,
            minecraft_metadata_source = excluded.minecraft_metadata_source,
            minecraft_file_source = excluded.minecraft_file_source,
            modrinth_source = excluded.modrinth_source,
            curseforge_source = excluded.curseforge_source,
            onboarding_version = excluded.onboarding_version,
            onboarding_instance_tour_completed =
                excluded.onboarding_instance_tour_completed,
            sidebar_instance_count = excluded.sidebar_instance_count,
            mc_memory_auto = excluded.mc_memory_auto,
            mc_memory_optimize = excluded.mc_memory_optimize,
            auto_set_java_high_performance_mode =
                excluded.auto_set_java_high_performance_mode,
            transparent_background = excluded.transparent_background,
            transparent_background_opacity =
                excluded.transparent_background_opacity,
            transparent_background_blur =
                excluded.transparent_background_blur,
            home_layout = excluded.home_layout,
            minimal_home_instance_id = excluded.minimal_home_instance_id,
            auto_hide_downloads_button = excluded.auto_hide_downloads_button,
            home_widgets = excluded.home_widgets,
            mojang_auth_source = excluded.mojang_auth_source,
            terracotta_public_nodes = excluded.terracotta_public_nodes,
            telemetry_consent_version = excluded.telemetry_consent_version,
            bypass_curseforge_download_restrictions =
                excluded.bypass_curseforge_download_restrictions,
            enter_lightweight_mode_on_game_launch =
                excluded.enter_lightweight_mode_on_game_launch,
            close_behavior = excluded.close_behavior,
            mc_maximize_window = excluded.mc_maximize_window,
            log_level = excluded.log_level,
            sync_features_across_devices =
                excluded.sync_features_across_devices,
            show_files_tab_in_instances =
                excluded.show_files_tab_in_instances,
            show_worlds_tab_in_instances =
                excluded.show_worlds_tab_in_instances,
            show_screenshots_tab_in_instances =
                excluded.show_screenshots_tab_in_instances,
            show_skin_selector_in_sidebar =
                excluded.show_skin_selector_in_sidebar,
            ignore_ssl_errors = excluded.ignore_ssl_errors,
            home_widget_background_opacity =
                excluded.home_widget_background_opacity,
            hidden_nav_items = excluded.hidden_nav_items,
            custom_window_title_enabled =
                excluded.custom_window_title_enabled,
            default_window_title = excluded.default_window_title,
            custom_background_component_opacity =
                excluded.custom_background_component_opacity,
            ui_font = excluded.ui_font,
            mono_font = excluded.mono_font,
            backup_repository_path = excluded.backup_repository_path,
            allow_external_scheme = excluded.allow_external_scheme,
            allow_privileged_scheme = excluded.allow_privileged_scheme,
            doh_enabled = excluded.doh_enabled,
            resource_chinese_filenames = excluded.resource_chinese_filenames,
            resource_preserve_filenames = excluded.resource_preserve_filenames,
            default_game_language = excluded.default_game_language
        ",
    )
    .bind(record.max_concurrent_downloads)
    .bind(record.max_concurrent_writes)
    .bind(&record.theme)
    .bind(&record.default_page)
    .bind(record.collapsed_navigation)
    .bind(record.advanced_rendering)
    .bind(record.native_decorations)
    .bind(record.telemetry)
    .bind(record.discord_rpc)
    .bind(record.developer_mode)
    .bind(record.onboarded)
    .bind(&record.extra_launch_args)
    .bind(&record.custom_env_vars)
    .bind(record.mc_memory_max)
    .bind(record.mc_force_fullscreen)
    .bind(record.mc_game_resolution_x)
    .bind(record.mc_game_resolution_y)
    .bind(record.hide_on_process_start)
    .bind(&record.hook_pre_launch)
    .bind(&record.hook_wrapper)
    .bind(&record.hook_post_exit)
    .bind(&record.custom_dir)
    .bind(&record.prev_custom_dir)
    .bind(record.migrated)
    .bind(record.personalized_ads)
    .bind(record.toggle_sidebar)
    .bind(&record.feature_flags)
    .bind(record.hide_nametag_skins_page)
    .bind(record.nametag_uses_app_font)
    .bind(&record.skipped_update)
    .bind(&record.pending_update_toast_for_version)
    .bind(record.auto_download_updates)
    .bind(record.version)
    .bind(&record.locale)
    .bind(&record.accent_color)
    .bind(&record.custom_background_path)
    .bind(record.custom_background_blur)
    .bind(record.custom_background_opacity)
    .bind(record.use_minecraft_mirror)
    .bind(record.use_modrinth_mirror)
    .bind(record.use_curseforge_mirror)
    .bind(record.auto_concurrent_downloads)
    .bind(record.minecraft_metadata_source.as_str())
    .bind(record.minecraft_file_source.as_str())
    .bind(record.modrinth_source.as_str())
    .bind(record.curseforge_source.as_str())
    .bind(record.onboarding_version)
    .bind(record.onboarding_instance_tour_completed)
    .bind(record.sidebar_instance_count)
    .bind(record.mc_memory_auto)
    .bind(record.mc_memory_optimize)
    .bind(record.auto_set_java_high_performance_mode)
    .bind(record.transparent_background)
    .bind(record.transparent_background_opacity)
    .bind(record.transparent_background_blur)
    .bind(record.home_layout.as_str())
    .bind(&record.minimal_home_instance_id)
    .bind(record.auto_hide_downloads_button)
    .bind(&record.home_widgets)
    .bind(record.mojang_auth_source.as_str())
    .bind(&record.terracotta_public_nodes)
    .bind(record.telemetry_consent_version)
    .bind(record.bypass_curseforge_download_restrictions)
    .bind(record.enter_lightweight_mode_on_game_launch)
    .bind(&record.close_behavior)
    .bind(record.mc_maximize_window)
    .bind(&record.log_level)
    .bind(record.sync_features_across_devices)
    .bind(record.show_files_tab_in_instances)
    .bind(record.show_worlds_tab_in_instances)
    .bind(record.show_screenshots_tab_in_instances)
    .bind(record.show_skin_selector_in_sidebar)
    .bind(record.ignore_ssl_errors)
    .bind(record.home_widget_background_opacity)
    .bind(&record.hidden_nav_items)
    .bind(record.custom_window_title_enabled)
    .bind(&record.default_window_title)
    .bind(record.custom_background_component_opacity)
    .bind(&record.ui_font)
    .bind(&record.mono_font)
    .bind(&record.backup_repository_path)
    .bind(record.allow_external_scheme)
    .bind(record.allow_privileged_scheme)
    .bind(record.doh_enabled)
    .bind(record.resource_chinese_filenames)
    .bind(record.resource_preserve_filenames)
    .bind(&record.default_game_language)
    .execute(&mut *conn)
    .await?;

    Ok(())
}

/// A source this build does not know reads the way the launcher has always read
/// it: as choosing for itself.
fn to_source(source: &str) -> DownloadSource {
    DownloadSource::parse(source).unwrap_or_else(|| {
        tracing::warn!(
            %source,
            "Reading a settings download source this build does not know as auto"
        );
        DownloadSource::Auto
    })
}

/// The same holds for a home layout this build does not know.
fn to_layout(layout: &str) -> HomeLayout {
    HomeLayout::parse(layout).unwrap_or_else(|| {
        tracing::warn!(
            %layout,
            "Reading a settings home layout this build does not know as standard"
        );
        HomeLayout::Standard
    })
}

/// The rows the settings documents hand over from.
///
/// These are read-only on purpose: a domain that has a document never writes
/// its column again, and the statements below only feed the handover and the
/// fallback a domain answers with while it has no document.
pub(crate) struct LegacyAiSettings {
    pub(crate) enabled: bool,
    pub(crate) legacy_openai_credential_cleanup: bool,
}

pub(crate) async fn read_legacy_ai_settings(
    pool: &SqlitePool,
) -> crate::Result<Option<LegacyAiSettings>> {
    let row = sqlx::query(
        "SELECT enabled, legacy_openai_credential_cleanup FROM ai_settings WHERE id = 0",
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|row| LegacyAiSettings {
        enabled: row.get::<i64, _>("enabled") != 0,
        legacy_openai_credential_cleanup: row
            .get::<i64, _>("legacy_openai_credential_cleanup")
            != 0,
    }))
}

pub(crate) struct LegacyAiProviderConfig {
    pub(crate) provider_id: String,
    pub(crate) custom_name: Option<String>,
    pub(crate) protocol: String,
    pub(crate) enabled: bool,
    pub(crate) endpoint: String,
    pub(crate) settings: String,
}

pub(crate) async fn read_legacy_ai_provider_configs(
    pool: &SqlitePool,
) -> crate::Result<Vec<LegacyAiProviderConfig>> {
    let rows = sqlx::query(
        "SELECT provider_id, custom_name, protocol, enabled, endpoint, settings FROM ai_provider_configs ORDER BY provider_id",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| LegacyAiProviderConfig {
            provider_id: row.get("provider_id"),
            custom_name: row.get("custom_name"),
            protocol: row.get("protocol"),
            enabled: row.get::<i64, _>("enabled") != 0,
            endpoint: row.get("endpoint"),
            settings: row.get("settings"),
        })
        .collect())
}

pub(crate) struct LegacyAiProviderModel {
    pub(crate) provider_id: String,
    pub(crate) model_id: String,
    pub(crate) display_name: String,
    pub(crate) enabled: bool,
    pub(crate) source: String,
}

pub(crate) async fn read_legacy_ai_provider_models(
    pool: &SqlitePool,
) -> crate::Result<Vec<LegacyAiProviderModel>> {
    let rows = sqlx::query(
        "SELECT provider_id, model_id, display_name, enabled, source FROM ai_provider_models ORDER BY provider_id, model_id",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| LegacyAiProviderModel {
            provider_id: row.get("provider_id"),
            model_id: row.get("model_id"),
            display_name: row.get("display_name"),
            enabled: row.get::<i64, _>("enabled") != 0,
            source: row.get("source"),
        })
        .collect())
}

pub(crate) struct LegacyTranslationSettings {
    pub(crate) provider: String,
    pub(crate) target_language: String,
    pub(crate) mode: String,
    pub(crate) auto_translate: bool,
    pub(crate) style: String,
    pub(crate) ai_provider_id: String,
    pub(crate) ai_model_id: String,
    pub(crate) ai_system_prompt: String,
    pub(crate) deepl_api_endpoint: String,
    pub(crate) deepl_api_key: Option<String>,
}

pub(crate) async fn read_legacy_translation_settings(
    pool: &SqlitePool,
) -> crate::Result<Option<LegacyTranslationSettings>> {
    let row = sqlx::query(
        "SELECT provider, target_language, mode, auto_translate, style, ai_provider_id, ai_model_id, openai_system_prompt, deepl_api_endpoint, deepl_api_key FROM translation_settings WHERE id = 0",
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|row| LegacyTranslationSettings {
        provider: row.get("provider"),
        target_language: row.get("target_language"),
        mode: row.get("mode"),
        auto_translate: row.get::<i64, _>("auto_translate") != 0,
        style: row.get("style"),
        ai_provider_id: row.get("ai_provider_id"),
        ai_model_id: row.get("ai_model_id"),
        ai_system_prompt: row.get("openai_system_prompt"),
        deepl_api_endpoint: row.get("deepl_api_endpoint"),
        deepl_api_key: row.get("deepl_api_key"),
    }))
}

pub(crate) struct LegacyLogShareSettings {
    pub(crate) share_provider: String,
    pub(crate) ai_source: String,
    pub(crate) auto_upload: bool,
    pub(crate) multi_file: bool,
    pub(crate) metadata_enabled: bool,
    pub(crate) no_storage: bool,
    pub(crate) show_progress: bool,
    pub(crate) crash_enabled: bool,
    pub(crate) crash_provider_id: String,
    pub(crate) crash_model_id: String,
}

pub(crate) async fn read_legacy_log_share_settings(
    pool: &SqlitePool,
) -> crate::Result<Option<LegacyLogShareSettings>> {
    let row = sqlx::query(
        "SELECT s.share_provider, a.ai_source, s.auto_upload, s.multi_file, s.metadata_enabled, s.no_storage, s.show_progress, a.enabled, a.provider_id, a.model_id FROM log_share_settings s JOIN crash_analysis_ai_settings a ON a.id = s.id WHERE s.id = 0",
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|row| LegacyLogShareSettings {
        share_provider: row.get("share_provider"),
        ai_source: row.get("ai_source"),
        auto_upload: row.get::<i64, _>("auto_upload") != 0,
        multi_file: row.get::<i64, _>("multi_file") != 0,
        metadata_enabled: row.get::<i64, _>("metadata_enabled") != 0,
        no_storage: row.get::<i64, _>("no_storage") != 0,
        show_progress: row.get::<i64, _>("show_progress") != 0,
        crash_enabled: row.get::<i64, _>("enabled") != 0,
        crash_provider_id: row.get("provider_id"),
        crash_model_id: row.get("model_id"),
    }))
}

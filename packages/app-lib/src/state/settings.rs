//! Theseus settings file

use crate::util::download::constants::{
    AUTO_DOWNLOAD_CONCURRENCY_INITIAL, DOWNLOAD_CONCURRENCY_HARD_MAX,
};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Sqlite};
use std::collections::HashMap;

// Types
#[derive(
    Serialize, Deserialize, Debug, Clone, Copy, Default, Eq, PartialEq,
)]
#[repr(u8)]
#[serde(rename_all = "snake_case")]
pub enum DownloadSourceMode {
    #[default]
    Auto = 0,
    OfficialOnly = 1,
    MirrorPreferred = 2,
    OfficialPreferred = 3,
}

impl DownloadSourceMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::OfficialOnly => "official_only",
            Self::MirrorPreferred => "mirror_preferred",
            Self::OfficialPreferred => "official_preferred",
        }
    }

    pub fn from_string(value: &str) -> Self {
        match value {
            "official_only" => Self::OfficialOnly,
            "mirror_preferred" => Self::MirrorPreferred,
            "official_preferred" => Self::OfficialPreferred,
            _ => Self::Auto,
        }
    }

    pub(crate) fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::OfficialOnly,
            2 => Self::MirrorPreferred,
            3 => Self::OfficialPreferred,
            _ => Self::Auto,
        }
    }

    pub(crate) fn prefers_mirror(self, auto_prefers_mirror: bool) -> bool {
        match self {
            Self::Auto => auto_prefers_mirror,
            Self::OfficialOnly | Self::OfficialPreferred => false,
            Self::MirrorPreferred => true,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub enum HomeLayout {
    #[default]
    Standard,
    Minimal,
}

impl HomeLayout {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Minimal => "minimal",
        }
    }

    pub fn from_string(value: &str) -> Self {
        match value {
            "minimal" => Self::Minimal,
            _ => Self::Standard,
        }
    }
}

impl Serialize for HomeLayout {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for HomeLayout {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Ok(Self::from_string(&value))
    }
}

/// Global Theseus settings
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Settings {
    #[serde(default = "default_true")]
    pub resource_chinese_filenames: bool,
    #[serde(default = "default_true")]
    pub resource_preserve_filenames: bool,
    pub max_concurrent_downloads: usize,
    pub max_concurrent_writes: usize,
    #[serde(default)]
    pub auto_concurrent_downloads: bool,
    #[serde(default)]
    pub minecraft_metadata_source: DownloadSourceMode,
    #[serde(default)]
    pub minecraft_file_source: DownloadSourceMode,
    #[serde(default)]
    pub modrinth_source: DownloadSourceMode,
    #[serde(default)]
    pub curseforge_source: DownloadSourceMode,
    #[serde(default = "default_true")]
    pub bypass_curseforge_download_restrictions: bool,
    #[serde(default)]
    pub ignore_ssl_errors: bool,
    #[serde(default = "default_doh_enabled")]
    pub doh_enabled: bool,
    #[serde(default)]
    pub mojang_auth_source: DownloadSourceMode,
    #[serde(default, rename = "use_minecraft_mirror", skip_serializing)]
    legacy_use_minecraft_mirror: Option<bool>,
    #[serde(default, rename = "use_modrinth_mirror", skip_serializing)]
    legacy_use_modrinth_mirror: Option<bool>,
    #[serde(default, rename = "use_curseforge_mirror", skip_serializing)]
    legacy_use_curseforge_mirror: Option<bool>,

    pub theme: Theme,
    pub accent_color: AccentColor,
    pub locale: String,
    pub default_page: DefaultPage,
    pub collapsed_navigation: bool,
    pub hide_nametag_skins_page: bool,
    pub advanced_rendering: bool,
    pub native_decorations: bool,
    pub toggle_sidebar: bool,
    pub custom_background_path: Option<String>,
    pub custom_background_blur: u32,
    pub custom_background_opacity: u32,
    #[serde(default = "default_custom_background_component_opacity")]
    pub custom_background_component_opacity: u32,
    #[serde(default)]
    pub ui_font: Option<String>,
    #[serde(default)]
    pub mono_font: Option<String>,
    pub transparent_background: bool,
    pub transparent_background_opacity: u32,
    pub transparent_background_blur: bool,
    pub sidebar_instance_count: u32,
    #[serde(default)]
    pub close_behavior: String,
    #[serde(default = "default_log_level")]
    pub log_level: String,
    #[serde(default)]
    pub auto_hide_downloads_button: bool,
    #[serde(default)]
    pub home_layout: HomeLayout,
    #[serde(default)]
    pub minimal_home_instance_id: Option<String>,
    #[serde(default)]
    pub home_widgets: Option<serde_json::Value>,
    #[serde(default = "default_home_widget_background_opacity")]
    pub home_widget_background_opacity: u32,
    #[serde(default)]
    pub hidden_nav_items: Vec<String>,
    #[serde(default)]
    pub custom_window_title_enabled: bool,
    #[serde(default = "default_window_title")]
    pub default_window_title: String,
    #[serde(default = "default_terracotta_public_nodes")]
    pub terracotta_public_nodes: Vec<String>,

    pub telemetry: bool,
    #[serde(default)]
    pub telemetry_consent_version: u32,
    pub discord_rpc: bool,
    #[serde(skip, default)]
    pub personalized_ads: bool,

    pub onboarded: bool,
    pub onboarding_version: usize,
    pub onboarding_instance_tour_completed: bool,

    pub extra_launch_args: Vec<String>,
    pub custom_env_vars: Vec<(String, String)>,
    pub memory: MemorySettings,
    pub force_fullscreen: bool,
    pub maximize_window: bool,
    pub game_resolution: WindowSize,
    pub hide_on_process_start: bool,
    pub enter_lightweight_mode_on_game_launch: bool,
    pub auto_set_java_high_performance_mode: bool,
    pub hooks: Hooks,

    pub custom_dir: Option<String>,
    pub prev_custom_dir: Option<String>,
    #[serde(default)]
    pub backup_repository_path: Option<String>,
    pub migrated: bool,

    pub developer_mode: bool,
    pub feature_flags: HashMap<FeatureFlag, bool>,
    #[serde(default)]
    pub sync_features_across_devices: bool,
    #[serde(default = "default_true")]
    pub show_files_tab_in_instances: bool,
    #[serde(default = "default_true")]
    pub show_worlds_tab_in_instances: bool,
    #[serde(default)]
    pub show_screenshots_tab_in_instances: bool,
    #[serde(default = "default_true")]
    pub show_skin_selector_in_sidebar: bool,

    pub skipped_update: Option<String>,
    pub pending_update_toast_for_version: Option<String>,
    pub auto_download_updates: Option<bool>,
    #[serde(default = "default_true")]
    pub allow_external_scheme: bool,
    #[serde(default)]
    pub allow_privileged_scheme: bool,

    pub version: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct PrivacySettings {
    pub telemetry: bool,
    pub discord_rpc: bool,
    pub consent_version: u32,
}

fn default_true() -> bool {
    true
}

fn default_doh_enabled() -> bool {
    true
}

/// Fully opaque home widget cards; users can dial this down to reveal the
/// custom/transparent window background behind them.
fn default_home_widget_background_opacity() -> u32 {
    100
}

/// Fully opaque launcher components over a custom background image. Lowering
/// this lets the image show through chrome and the content surface (#335).
fn default_custom_background_component_opacity() -> u32 {
    100
}

fn default_window_title() -> String {
    "Minecraft".to_string()
}

const MAX_FONT_FAMILY_LENGTH: usize = 128;

/// Font family names are picked from the host's installed fonts, but the
/// settings table is plain user data: store them trimmed and bounded, with an
/// empty selection normalised to NULL (follow the launcher default).
fn sanitize_font_family(family: Option<String>) -> Option<String> {
    family
        .map(|value| {
            value
                .trim()
                .chars()
                .take(MAX_FONT_FAMILY_LENGTH)
                .collect::<String>()
        })
        .filter(|value| !value.is_empty())
}

/// Default log level, kept in sync with the `log_level` column default and
/// the logger's own fallback.
fn default_log_level() -> String {
    crate::logger::DEFAULT_LOG_LEVEL.to_string()
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Eq, Hash, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FeatureFlag {
    PagePath,
    ProjectBackground,
    WorldsTab,
    WorldsInHome,
    ServerRamAsBytesAlwaysOn,
    AlwaysShowAppControls,
    SkipUnknownPackWarning,
    PrideFundraiser,
    ServersInApp,
    ServerProjectQa,
    I18nDebug,
    ShowInstancePlayTime,
    SkipNonEssentialWarnings,
    AdvancedFiltersCollapsed,
    PageTransitions,
    ShowVersionEnvironmentColumn,
    AutoInstallDependencies,
}

impl Settings {
    const CURRENT_VERSION: usize = 3;

    pub async fn get<'a, E>(exec: E) -> crate::Result<Self>
    where
        E: sqlx::Acquire<'a, Database = sqlx::Sqlite>,
    {
        let mut connection = exec.acquire().await?;
        Self::get_with_connection(&mut connection).await
    }

    pub(crate) async fn get_with_connection(
        connection: &mut sqlx::SqliteConnection,
    ) -> crate::Result<Self> {
        let res = sqlx::query!(
            "
            SELECT
                max_concurrent_writes, max_concurrent_downloads,
                auto_concurrent_downloads, minecraft_metadata_source,
                minecraft_file_source, modrinth_source, curseforge_source, mojang_auth_source,
                theme, locale, default_page, collapsed_navigation, hide_nametag_skins_page, advanced_rendering, native_decorations,
				discord_rpc, developer_mode, telemetry, telemetry_consent_version, personalized_ads,
                onboarded, onboarding_version, onboarding_instance_tour_completed,
                json(extra_launch_args) extra_launch_args, json(custom_env_vars) custom_env_vars,
                mc_memory_max, mc_memory_auto, mc_force_fullscreen, mc_maximize_window, mc_game_resolution_x, mc_game_resolution_y, hide_on_process_start, enter_lightweight_mode_on_game_launch,
                auto_set_java_high_performance_mode,
                hook_pre_launch, hook_wrapper, hook_post_exit,
                custom_dir, prev_custom_dir, migrated, json(feature_flags) feature_flags, toggle_sidebar,
                skipped_update, pending_update_toast_for_version, auto_download_updates, accent_color,
                custom_background_path, custom_background_blur, custom_background_opacity,
                transparent_background, transparent_background_opacity, transparent_background_blur,
                sidebar_instance_count, home_layout, minimal_home_instance_id,
                json(home_widgets) as \"home_widgets?: String\", auto_hide_downloads_button,
                json(terracotta_public_nodes) terracotta_public_nodes,
                version
            FROM settings
            "
        )
            .fetch_one(&mut *connection)
            .await?;

        let close_behavior: String = sqlx::query_scalar(
            "SELECT close_behavior FROM settings WHERE id = 0",
        )
        .fetch_one(&mut *connection)
        .await?;

        let home_widget_background_opacity: i64 = sqlx::query_scalar(
            "SELECT home_widget_background_opacity FROM settings WHERE id = 0",
        )
        .fetch_one(&mut *connection)
        .await?;

        let custom_background_component_opacity: i64 = sqlx::query_scalar(
            "SELECT custom_background_component_opacity FROM settings WHERE id = 0",
        )
        .fetch_one(&mut *connection)
        .await?;

        let ui_font: Option<String> =
            sqlx::query_scalar("SELECT ui_font FROM settings WHERE id = 0")
                .fetch_one(&mut *connection)
                .await?;

        let mono_font: Option<String> =
            sqlx::query_scalar("SELECT mono_font FROM settings WHERE id = 0")
                .fetch_one(&mut *connection)
                .await?;

        let hidden_nav_items_json: String = sqlx::query_scalar(
            "SELECT hidden_nav_items FROM settings WHERE id = 0",
        )
        .fetch_one(&mut *connection)
        .await?;
        let mut hidden_nav_items: Vec<String> =
            serde_json::from_str(&hidden_nav_items_json).unwrap_or_default();
        // Home and Library must stay reachable from the nav rail.
        hidden_nav_items.retain(|id| id != "home" && id != "library");

        let custom_window_title_enabled: bool = sqlx::query_scalar(
            "SELECT custom_window_title_enabled FROM settings WHERE id = 0",
        )
        .fetch_one(&mut *connection)
        .await?;
        let default_window_title: String = sqlx::query_scalar(
            "SELECT default_window_title FROM settings WHERE id = 0",
        )
        .fetch_one(&mut *connection)
        .await?;
        let backup_repository_path: Option<String> = sqlx::query_scalar(
            "SELECT backup_repository_path FROM settings WHERE id = 0",
        )
        .fetch_one(&mut *connection)
        .await?;

        let log_level: String =
            sqlx::query_scalar("SELECT log_level FROM settings WHERE id = 0")
                .fetch_one(&mut *connection)
                .await?;

        let (
            sync_features_across_devices,
            show_files_tab_in_instances,
            show_worlds_tab_in_instances,
            show_screenshots_tab_in_instances,
            show_skin_selector_in_sidebar,
        ): (bool, bool, bool, bool, bool) = sqlx::query_as(
            "SELECT sync_features_across_devices, show_files_tab_in_instances, show_worlds_tab_in_instances, show_screenshots_tab_in_instances, show_skin_selector_in_sidebar FROM settings WHERE id = 0",
        )
        .fetch_one(&mut *connection)
        .await?;

        let bypass_curseforge_download_restrictions: bool = sqlx::query_scalar(
            "SELECT bypass_curseforge_download_restrictions FROM settings WHERE id = 0",
        )
        .fetch_one(&mut *connection)
        .await?;
        let ignore_ssl_errors: bool = sqlx::query_scalar(
            "SELECT ignore_ssl_errors FROM settings WHERE id = 0",
        )
        .fetch_one(&mut *connection)
        .await?;
        let doh_enabled: bool =
            sqlx::query_scalar("SELECT doh_enabled FROM settings WHERE id = 0")
                .fetch_one(&mut *connection)
                .await?;
        let settings = Self {
            resource_chinese_filenames: sqlx::query_scalar(
                "SELECT resource_chinese_filenames FROM settings WHERE id = 0",
            )
            .fetch_one(&mut *connection)
            .await?,
            resource_preserve_filenames: sqlx::query_scalar(
                "SELECT resource_preserve_filenames FROM settings WHERE id = 0",
            )
            .fetch_one(&mut *connection)
            .await?,
            max_concurrent_downloads: res.max_concurrent_downloads as usize,
            max_concurrent_writes: res.max_concurrent_writes as usize,
            auto_concurrent_downloads: res.auto_concurrent_downloads == 1,
            minecraft_metadata_source: DownloadSourceMode::from_string(
                &res.minecraft_metadata_source,
            ),
            minecraft_file_source: DownloadSourceMode::from_string(
                &res.minecraft_file_source,
            ),
            modrinth_source: DownloadSourceMode::from_string(
                &res.modrinth_source,
            ),
            curseforge_source: DownloadSourceMode::from_string(
                &res.curseforge_source,
            ),
            bypass_curseforge_download_restrictions,
            ignore_ssl_errors,
            doh_enabled,
            mojang_auth_source: DownloadSourceMode::from_string(
                &res.mojang_auth_source,
            ),
            legacy_use_minecraft_mirror: None,
            legacy_use_modrinth_mirror: None,
            legacy_use_curseforge_mirror: None,
            theme: Theme::from_string(&res.theme),
            accent_color: AccentColor::from_string(&res.accent_color),
            locale: res.locale,
            default_page: DefaultPage::from_string(&res.default_page),
            collapsed_navigation: res.collapsed_navigation == 1,
            hide_nametag_skins_page: res.hide_nametag_skins_page == 1,
            advanced_rendering: res.advanced_rendering == 1,
            native_decorations: res.native_decorations == 1,
            toggle_sidebar: res.toggle_sidebar == 1,
            custom_background_path: res.custom_background_path,
            custom_background_blur: res.custom_background_blur as u32,
            custom_background_opacity: res.custom_background_opacity as u32,
            custom_background_component_opacity:
                custom_background_component_opacity.clamp(0, 100) as u32,
            ui_font: sanitize_font_family(ui_font),
            mono_font: sanitize_font_family(mono_font),
            transparent_background: res.transparent_background == 1,
            transparent_background_opacity: res.transparent_background_opacity
                as u32,
            transparent_background_blur: res.transparent_background_blur == 1,
            sidebar_instance_count: res.sidebar_instance_count as u32,
            close_behavior,
            log_level,
            auto_hide_downloads_button: res.auto_hide_downloads_button == 1,
            home_layout: HomeLayout::from_string(&res.home_layout),
            minimal_home_instance_id: res.minimal_home_instance_id,
            home_widgets: res
                .home_widgets
                .as_ref()
                .and_then(|value| serde_json::from_str(value).ok()),
            home_widget_background_opacity: home_widget_background_opacity
                .clamp(0, 100)
                as u32,
            hidden_nav_items,
            custom_window_title_enabled,
            default_window_title,
            terracotta_public_nodes: res
                .terracotta_public_nodes
                .as_ref()
                .and_then(|value| serde_json::from_str(value).ok())
                .unwrap_or_else(default_terracotta_public_nodes),
            telemetry: res.telemetry == 1,
            telemetry_consent_version: res.telemetry_consent_version as u32,
            discord_rpc: res.discord_rpc == 1,
            developer_mode: res.developer_mode == 1,
            personalized_ads: res.personalized_ads == 1,
            onboarded: res.onboarded == 1,
            onboarding_version: res.onboarding_version as usize,
            onboarding_instance_tour_completed: res
                .onboarding_instance_tour_completed
                == 1,
            extra_launch_args: res
                .extra_launch_args
                .as_ref()
                .and_then(|x| serde_json::from_str(x).ok())
                .unwrap_or_default(),
            custom_env_vars: res
                .custom_env_vars
                .as_ref()
                .and_then(|x| serde_json::from_str(x).ok())
                .unwrap_or_default(),
            memory: MemorySettings {
                maximum: res.mc_memory_max as u32,
                automatic: res.mc_memory_auto == 1,
                optimize_before_launch: sqlx::query_scalar(
                    "SELECT mc_memory_optimize FROM settings WHERE id = 0",
                )
                .fetch_one(&mut *connection)
                .await?,
            },
            force_fullscreen: res.mc_force_fullscreen == 1,
            maximize_window: res.mc_maximize_window == 1,
            game_resolution: WindowSize(
                res.mc_game_resolution_x as u16,
                res.mc_game_resolution_y as u16,
            ),
            hide_on_process_start: res.hide_on_process_start == 1,
            enter_lightweight_mode_on_game_launch: res
                .enter_lightweight_mode_on_game_launch
                == 1,
            auto_set_java_high_performance_mode: res
                .auto_set_java_high_performance_mode
                == 1,
            hooks: Hooks {
                pre_launch: res.hook_pre_launch,
                wrapper: res.hook_wrapper,
                post_exit: res.hook_post_exit,
            },
            custom_dir: res.custom_dir,
            prev_custom_dir: res.prev_custom_dir,
            backup_repository_path,
            migrated: res.migrated == 1,
            feature_flags: res
                .feature_flags
                .as_ref()
                .and_then(|x| serde_json::from_str(x).ok())
                .unwrap_or_default(),
            sync_features_across_devices,
            show_files_tab_in_instances,
            show_worlds_tab_in_instances,
            show_screenshots_tab_in_instances,
            show_skin_selector_in_sidebar,
            skipped_update: res.skipped_update,
            pending_update_toast_for_version: res
                .pending_update_toast_for_version,
            auto_download_updates: res.auto_download_updates.map(|x| x == 1),
            allow_external_scheme: sqlx::query_scalar(
                "SELECT allow_external_scheme FROM settings WHERE id = 0",
            )
            .fetch_one(&mut *connection)
            .await
            .unwrap_or(true),
            allow_privileged_scheme: sqlx::query_scalar(
                "SELECT allow_privileged_scheme FROM settings WHERE id = 0",
            )
            .fetch_one(&mut *connection)
            .await
            .unwrap_or(false),
            version: res.version as usize,
        };
        Ok(settings)
    }

    pub async fn update<'a, E>(&self, exec: E) -> crate::Result<()>
    where
        E: sqlx::Acquire<'a, Database = sqlx::Sqlite>,
    {
        let mut connection = exec.acquire().await?;
        self.update_with_connection(&mut connection).await
    }

    pub(crate) async fn update_with_connection(
        &self,
        connection: &mut sqlx::SqliteConnection,
    ) -> crate::Result<()> {
        let max_concurrent_writes = self.max_concurrent_writes as i32;
        let max_concurrent_downloads =
            self.max_concurrent_downloads
                .clamp(1, DOWNLOAD_CONCURRENCY_HARD_MAX) as i32;
        let theme = self.theme.as_str();
        let accent_color = self.accent_color.as_str();
        let default_page = self.default_page.as_str();
        let extra_launch_args = serde_json::to_string(&self.extra_launch_args)?;
        let custom_env_vars = serde_json::to_string(&self.custom_env_vars)?;
        let feature_flags = serde_json::to_string(&self.feature_flags)?;
        let custom_background_blur = self.custom_background_blur.min(40) as i32;
        let custom_background_opacity =
            self.custom_background_opacity.clamp(10, 100) as i32;
        let transparent_background_opacity =
            self.transparent_background_opacity.min(100) as i32;
        let sidebar_instance_count = self.sidebar_instance_count.min(50) as i32;
        let home_layout = self.home_layout.as_str();
        let home_widgets = self
            .home_widgets
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;
        let terracotta_public_nodes =
            serde_json::to_string(&self.terracotta_public_nodes)?;
        let version = self.version as i64;
        let onboarding_version = self.onboarding_version as i64;
        let minecraft_metadata_source = self.minecraft_metadata_source.as_str();
        let minecraft_file_source = self.minecraft_file_source.as_str();
        let modrinth_source = self.modrinth_source.as_str();
        let curseforge_source = self.curseforge_source.as_str();
        let mojang_auth_source = self.mojang_auth_source.as_str();
        let auto_prefers_mirror = self.auto_prefers_mirror();
        let use_minecraft_mirror = self
            .minecraft_file_source
            .prefers_mirror(auto_prefers_mirror);
        let use_modrinth_mirror =
            self.modrinth_source.prefers_mirror(auto_prefers_mirror);
        let use_curseforge_mirror =
            self.curseforge_source.prefers_mirror(auto_prefers_mirror);

        sqlx::query!(
            "
            UPDATE settings
            SET
                max_concurrent_writes = $1,
                max_concurrent_downloads = $2,

                theme = $3,
                locale = $4,
                default_page = $5,
                collapsed_navigation = $6,
                advanced_rendering = $7,
                native_decorations = $8,

                discord_rpc = $9,
                developer_mode = $10,
                telemetry = $11,
                personalized_ads = $12,

                onboarded = $13,

                extra_launch_args = jsonb($14),
                custom_env_vars = jsonb($15),
                mc_memory_max = $16,
                mc_memory_auto = $17,
                 mc_force_fullscreen = $18,
                 mc_maximize_window = $19,
                 mc_game_resolution_x = $20,
                 mc_game_resolution_y = $21,
                 hide_on_process_start = $22,
                 auto_set_java_high_performance_mode = $23,

                 hook_pre_launch = $24,
                 hook_wrapper = $25,
                 hook_post_exit = $26,

                 custom_dir = $27,
                 prev_custom_dir = $28,
                 migrated = $29,

                 toggle_sidebar = $30,
                 feature_flags = $31,
                 hide_nametag_skins_page = $32,

                 skipped_update = $33,
                 pending_update_toast_for_version = $34,
                 auto_download_updates = $35,
                 accent_color = $36,
                 custom_background_path = $37,
                 custom_background_blur = $38,
                 custom_background_opacity = $39,

                 version = $40,
                 auto_concurrent_downloads = $41,
                 minecraft_metadata_source = $42,
                 minecraft_file_source = $43,
                 modrinth_source = $44,
                 curseforge_source = $45,
                 use_minecraft_mirror = $46,
                 use_modrinth_mirror = $47,
                 use_curseforge_mirror = $48,
                 onboarding_version = $49,
                 onboarding_instance_tour_completed = $50,
                 sidebar_instance_count = $51,
                 transparent_background = $52,
                 transparent_background_opacity = $53,
                 transparent_background_blur = $54,
                 home_layout = $55,
                 minimal_home_instance_id = $56,
                 auto_hide_downloads_button = $57,
                 home_widgets = jsonb($58),
                 mojang_auth_source = $59,
				terracotta_public_nodes = jsonb($60),
				telemetry_consent_version = $61
            ",
            max_concurrent_writes,
            max_concurrent_downloads,
            theme,
            self.locale,
            default_page,
            self.collapsed_navigation,
            self.advanced_rendering,
            self.native_decorations,
            self.discord_rpc,
            self.developer_mode,
            self.telemetry,
            self.personalized_ads,
            self.onboarded,
            extra_launch_args,
            custom_env_vars,
            self.memory.maximum,
            self.memory.automatic,
            self.force_fullscreen,
            self.maximize_window,
            self.game_resolution.0,
            self.game_resolution.1,
            self.hide_on_process_start,
            self.auto_set_java_high_performance_mode,
            self.hooks.pre_launch,
            self.hooks.wrapper,
            self.hooks.post_exit,
            self.custom_dir,
            self.prev_custom_dir,
            self.migrated,
            self.toggle_sidebar,
            feature_flags,
            self.hide_nametag_skins_page,
            self.skipped_update,
            self.pending_update_toast_for_version,
            self.auto_download_updates,
            accent_color,
            self.custom_background_path,
            custom_background_blur,
            custom_background_opacity,
            version,
            self.auto_concurrent_downloads,
            minecraft_metadata_source,
            minecraft_file_source,
            modrinth_source,
            curseforge_source,
            use_minecraft_mirror,
            use_modrinth_mirror,
            use_curseforge_mirror,
            onboarding_version,
            self.onboarding_instance_tour_completed,
            sidebar_instance_count,
            self.transparent_background,
            transparent_background_opacity,
            self.transparent_background_blur,
            home_layout,
            self.minimal_home_instance_id,
            self.auto_hide_downloads_button,
            home_widgets,
            mojang_auth_source,
            terracotta_public_nodes,
            self.telemetry_consent_version,
        )
        .execute(&mut *connection)
        .await?;

        sqlx::query(
			"UPDATE settings SET enter_lightweight_mode_on_game_launch = ? WHERE id = 0",
		)
		.bind(self.enter_lightweight_mode_on_game_launch)
		.execute(&mut *connection)
		.await?;

        sqlx::query("UPDATE settings SET close_behavior = ? WHERE id = 0")
            .bind(&self.close_behavior)
            .execute(&mut *connection)
            .await?;

        sqlx::query("UPDATE settings SET log_level = ? WHERE id = 0")
            .bind(&self.log_level)
            .execute(&mut *connection)
            .await?;

        sqlx::query(
            "UPDATE settings SET home_widget_background_opacity = ? WHERE id = 0",
        )
        .bind(self.home_widget_background_opacity.clamp(0, 100) as i64)
        .execute(&mut *connection)
        .await?;

        sqlx::query(
            "UPDATE settings SET custom_background_component_opacity = ? WHERE id = 0",
        )
        .bind(self.custom_background_component_opacity.clamp(0, 100) as i64)
        .execute(&mut *connection)
        .await?;

        sqlx::query(
            "UPDATE settings SET ui_font = ?, mono_font = ? WHERE id = 0",
        )
        .bind(sanitize_font_family(self.ui_font.clone()))
        .bind(sanitize_font_family(self.mono_font.clone()))
        .execute(&mut *connection)
        .await?;

        let mut hidden_nav_items = self.hidden_nav_items.clone();
        // Boundary guard: never persist core nav items as hidden.
        hidden_nav_items.retain(|id| id != "home" && id != "library");

        sqlx::query("UPDATE settings SET hidden_nav_items = ? WHERE id = 0")
            .bind(serde_json::to_string(&hidden_nav_items)?)
            .execute(&mut *connection)
            .await?;

        sqlx::query(
            "UPDATE settings SET custom_window_title_enabled = ? WHERE id = 0",
        )
        .bind(self.custom_window_title_enabled)
        .execute(&mut *connection)
        .await?;

        sqlx::query(
            "UPDATE settings SET default_window_title = ? WHERE id = 0",
        )
        .bind(self.default_window_title.trim())
        .execute(&mut *connection)
        .await?;

        sqlx::query(
            "UPDATE settings SET backup_repository_path = ? WHERE id = 0",
        )
        .bind(self.backup_repository_path.as_deref())
        .execute(&mut *connection)
        .await?;

        sqlx::query(
            "UPDATE settings SET bypass_curseforge_download_restrictions = ? WHERE id = 0",
        )
        .bind(self.bypass_curseforge_download_restrictions)
        .execute(&mut *connection)
        .await?;
        sqlx::query("UPDATE settings SET resource_chinese_filenames = ?, resource_preserve_filenames = ? WHERE id = 0")
            .bind(self.resource_chinese_filenames)
            .bind(self.resource_preserve_filenames)
            .execute(&mut *connection).await?;
        sqlx::query("UPDATE settings SET ignore_ssl_errors = ? WHERE id = 0")
            .bind(self.ignore_ssl_errors)
            .execute(&mut *connection)
            .await?;
        sqlx::query("UPDATE settings SET mc_memory_optimize = ? WHERE id = 0")
            .bind(self.memory.optimize_before_launch)
            .execute(&mut *connection)
            .await?;
        sqlx::query(
            "UPDATE settings SET sync_features_across_devices = ?, show_files_tab_in_instances = ?, show_worlds_tab_in_instances = ?, show_screenshots_tab_in_instances = ?, show_skin_selector_in_sidebar = ? WHERE id = 0",
        )
        .bind(self.sync_features_across_devices)
        .bind(self.show_files_tab_in_instances)
        .bind(self.show_worlds_tab_in_instances)
        .bind(self.show_screenshots_tab_in_instances)
        .bind(self.show_skin_selector_in_sidebar)
        .execute(&mut *connection)
        .await?;
        sqlx::query(
            "UPDATE settings SET allow_external_scheme = ? WHERE id = 0",
        )
        .bind(self.allow_external_scheme)
        .execute(&mut *connection)
        .await?;
        sqlx::query(
            "UPDATE settings SET allow_privileged_scheme = ? WHERE id = 0",
        )
        .bind(self.allow_privileged_scheme)
        .execute(&mut *connection)
        .await?;

        sqlx::query("UPDATE settings SET doh_enabled = ? WHERE id = 0")
            .bind(self.doh_enabled)
            .execute(&mut *connection)
            .await?;

        Ok(())
    }

    pub fn effective_max_concurrent_downloads(&self) -> usize {
        if self.auto_concurrent_downloads {
            AUTO_DOWNLOAD_CONCURRENCY_INITIAL
        } else {
            self.max_concurrent_downloads
                .clamp(1, DOWNLOAD_CONCURRENCY_HARD_MAX)
        }
    }

    pub(crate) fn apply_legacy_download_source_settings(&mut self) {
        let has_legacy_settings = self.legacy_use_minecraft_mirror.is_some()
            || self.legacy_use_modrinth_mirror.is_some()
            || self.legacy_use_curseforge_mirror.is_some();
        let has_explicit_source_settings = self.minecraft_metadata_source
            != DownloadSourceMode::Auto
            || self.minecraft_file_source != DownloadSourceMode::Auto
            || self.modrinth_source != DownloadSourceMode::Auto
            || self.curseforge_source != DownloadSourceMode::Auto;
        if !has_legacy_settings || has_explicit_source_settings {
            return;
        }

        if self.legacy_use_minecraft_mirror == Some(false)
            && self.legacy_use_modrinth_mirror == Some(false)
            && self.legacy_use_curseforge_mirror == Some(true)
        {
            self.minecraft_metadata_source = DownloadSourceMode::Auto;
            self.minecraft_file_source = DownloadSourceMode::Auto;
            self.modrinth_source = DownloadSourceMode::Auto;
            self.curseforge_source = DownloadSourceMode::Auto;
            return;
        }

        if let Some(enabled) = self.legacy_use_minecraft_mirror {
            let source = legacy_download_source(enabled);
            self.minecraft_metadata_source = source;
            self.minecraft_file_source = source;
        }
        if let Some(enabled) = self.legacy_use_modrinth_mirror {
            self.modrinth_source = legacy_download_source(enabled);
        }
        if let Some(enabled) = self.legacy_use_curseforge_mirror {
            self.curseforge_source = legacy_download_source(enabled);
        }
    }

    pub(crate) fn auto_prefers_mirror(&self) -> bool {
        let timezone = std::env::var("TZ").ok().or_else(|| {
            std::fs::read_link("/etc/localtime")
                .ok()
                .map(|path| path.to_string_lossy().into_owned())
        });

        if let Some(timezone) = timezone {
            return locale_prefers_mirror(&timezone);
        }

        locale_prefers_mirror(&self.locale)
            || ["LC_ALL", "LC_MESSAGES", "LANG"]
                .into_iter()
                .filter_map(|key| std::env::var(key).ok())
                .any(|value| locale_prefers_mirror(&value))
    }

    pub async fn migrate(exec: &Pool<Sqlite>) -> crate::Result<()> {
        let mut settings = Self::get(exec).await?;

        if settings.version < Settings::CURRENT_VERSION {
            tracing::info!(
                "Migrating settings version {} to {:?}",
                settings.version,
                Settings::CURRENT_VERSION
            );
        }
        while settings.version < Settings::CURRENT_VERSION {
            if let Err(err) = settings.perform_migration() {
                tracing::error!(
                    "Failed to migrate settings from version {}: {}",
                    settings.version,
                    err
                );
                return Err(err);
            }
        }

        settings.update(exec).await?;

        Ok(())
    }

    pub fn perform_migration(&mut self) -> crate::Result<()> {
        match self.version {
            1 => {
                let quoter = shlex::Quoter::new().allow_nul(true);

                // Previously split by spaces
                if let Some(pre_launch) = self.hooks.pre_launch.as_ref() {
                    self.hooks.pre_launch =
                        Some(quoter.join(pre_launch.split(' ')).unwrap())
                }

                // Previously treated as complete path to command
                if let Some(wrapper) = self.hooks.wrapper.as_ref() {
                    self.hooks.wrapper =
                        Some(quoter.quote(wrapper).unwrap().to_string())
                }

                // Previously split by spaces
                if let Some(post_exit) = self.hooks.post_exit.as_ref() {
                    self.hooks.post_exit =
                        Some(quoter.join(post_exit.split(' ')).unwrap())
                }

                self.version = 2;
            }
            2 => {
                // Update old default memory setting from 2GB to 4GB (depending on system memory)
                const LEGACY_DEFAULT_MEMORY_MB: u32 = 2048;
                if self.memory.maximum == LEGACY_DEFAULT_MEMORY_MB {
                    self.memory.maximum =
                        crate::api::jre::default_memory_max_mb();
                }

                self.version = 3;
            }
            version => {
                return Err(crate::ErrorKind::OtherError(format!(
                    "Invalid settings version: {version}"
                ))
                .into());
            }
        }

        Ok(())
    }
}

pub fn default_terracotta_public_nodes() -> Vec<String> {
    vec!["wss://center.node.1tmc.top".to_string()]
}

fn locale_prefers_mirror(value: &str) -> bool {
    let normalized = value.to_ascii_lowercase().replace('_', "-");

    normalized.starts_with("zh-cn")
        || normalized.starts_with("zh-hans")
        || normalized.contains("asia/shanghai")
        || normalized.contains("asia/chongqing")
        || normalized.contains("asia/harbin")
        || normalized.contains("asia/urumqi")
}

fn legacy_download_source(enabled: bool) -> DownloadSourceMode {
    if enabled {
        DownloadSourceMode::MirrorPreferred
    } else {
        DownloadSourceMode::OfficialOnly
    }
}

/// Accent color used for interactive controls and highlights.
///
/// Serialized as a plain string: either a preset name (`pink`, `orange`, ...)
/// or `custom:#rrggbb` for a user-defined color. Unknown values fall back to
/// [`AccentColor::Pink`], keeping older builds forward-compatible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccentColor {
    Pink,
    Orange,
    Green,
    Blue,
    Purple,
    System,
    Custom(String),
}

impl AccentColor {
    pub fn as_str(&self) -> &str {
        match self {
            AccentColor::Pink => "pink",
            AccentColor::Orange => "orange",
            AccentColor::Green => "green",
            AccentColor::Blue => "blue",
            AccentColor::Purple => "purple",
            AccentColor::System => "system",
            AccentColor::Custom(value) => {
                if Self::is_valid_custom(value) {
                    value
                } else {
                    "pink"
                }
            }
        }
    }

    pub fn from_string(string: &str) -> AccentColor {
        match string {
            "orange" => AccentColor::Orange,
            "green" => AccentColor::Green,
            "blue" => AccentColor::Blue,
            "purple" => AccentColor::Purple,
            "system" => AccentColor::System,
            other => match Self::parse_custom(other) {
                Some(custom) => custom,
                None => AccentColor::Pink,
            },
        }
    }

    fn is_valid_custom(string: &str) -> bool {
        string.strip_prefix("custom:#").is_some_and(|hex| {
            hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit())
        })
    }

    fn parse_custom(string: &str) -> Option<AccentColor> {
        if Self::is_valid_custom(string) {
            Some(AccentColor::Custom(string.to_ascii_lowercase()))
        } else {
            None
        }
    }
}

impl Serialize for AccentColor {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for AccentColor {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Ok(AccentColor::from_string(&value))
    }
}

/// Theseus theme
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    Dark,
    Light,
    Oled,
    System,
}

impl Theme {
    pub fn as_str(&self) -> &'static str {
        match self {
            Theme::Dark => "dark",
            Theme::Light => "light",
            Theme::Oled => "oled",
            Theme::System => "system",
        }
    }

    pub fn from_string(string: &str) -> Theme {
        match string {
            "dark" => Theme::Dark,
            "light" => Theme::Light,
            "oled" => Theme::Oled,
            "system" => Theme::System,
            _ => Theme::Dark,
        }
    }
}

/// Minecraft memory settings
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct MemorySettings {
    pub maximum: u32,
    #[serde(default = "default_true")]
    pub automatic: bool,
    #[serde(default)]
    pub optimize_before_launch: bool,
}

/// Game window size
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct WindowSize(pub u16, pub u16);

/// Game initialization hooks
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde_with::serde_as]
pub struct Hooks {
    #[serde_as(as = "serde_with::NoneAsEmptyString")]
    pub pre_launch: Option<String>,
    #[serde_as(as = "serde_with::NoneAsEmptyString")]
    pub wrapper: Option<String>,
    #[serde_as(as = "serde_with::NoneAsEmptyString")]
    pub post_exit: Option<String>,
}

/// Opening window to start with
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum DefaultPage {
    Home,
    DiscoverContent,
    Library,
}

impl DefaultPage {
    pub fn as_str(&self) -> &'static str {
        match self {
            DefaultPage::Home => "home",
            DefaultPage::DiscoverContent => "discover_content",
            DefaultPage::Library => "library",
        }
    }

    pub fn from_string(string: &str) -> Self {
        match string {
            "home" => Self::Home,
            "discover_content" => Self::DiscoverContent,
            "library" => Self::Library,
            _ => Self::Home,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_layout_uses_stable_wire_values() {
        assert_eq!(HomeLayout::from_string("standard"), HomeLayout::Standard);
        assert_eq!(HomeLayout::from_string("minimal"), HomeLayout::Minimal);
        assert_eq!(HomeLayout::from_string("unknown"), HomeLayout::Standard);
        assert_eq!(
            serde_json::to_string(&HomeLayout::Standard).unwrap(),
            "\"standard\""
        );
        assert_eq!(
            serde_json::to_string(&HomeLayout::Minimal).unwrap(),
            "\"minimal\""
        );
        assert_eq!(
            serde_json::from_str::<HomeLayout>("\"future-layout\"").unwrap(),
            HomeLayout::Standard
        );
    }

    #[test]
    fn accent_color_parses_preset_values() {
        assert_eq!(AccentColor::from_string("pink"), AccentColor::Pink);
        assert_eq!(AccentColor::from_string("orange"), AccentColor::Orange);
        assert_eq!(AccentColor::from_string("green"), AccentColor::Green);
        assert_eq!(AccentColor::from_string("blue"), AccentColor::Blue);
        assert_eq!(AccentColor::from_string("purple"), AccentColor::Purple);
        assert_eq!(AccentColor::from_string("system"), AccentColor::System);
    }

    #[test]
    fn accent_color_normalizes_custom_hex_to_lowercase() {
        assert_eq!(
            AccentColor::from_string("custom:#DB2777"),
            AccentColor::Custom("custom:#db2777".to_owned())
        );
    }

    #[test]
    fn accent_color_falls_back_to_pink_on_invalid_values() {
        for value in [
            "",
            "magenta",
            "custom:",
            "custom:#db27",
            "custom:#db2777aa",
            "custom:#db277g",
        ] {
            assert_eq!(AccentColor::from_string(value), AccentColor::Pink);
        }
    }

    #[test]
    fn accent_color_serializes_as_plain_strings() {
        let custom = AccentColor::Custom("custom:#db2777".to_owned());
        assert_eq!(
            serde_json::to_string(&AccentColor::Blue).unwrap(),
            "\"blue\""
        );
        assert_eq!(
            serde_json::to_string(&AccentColor::System).unwrap(),
            "\"system\""
        );
        assert_eq!(
            serde_json::to_string(&custom).unwrap(),
            "\"custom:#db2777\""
        );
    }

    #[test]
    fn accent_color_serializes_invalid_custom_as_pink() {
        let invalid = AccentColor::Custom("not-a-color".to_owned());
        assert_eq!(invalid.as_str(), "pink");
        assert_eq!(serde_json::to_string(&invalid).unwrap(), "\"pink\"");
    }

    #[test]
    fn accent_color_deserializes_from_plain_strings() {
        let color: AccentColor =
            serde_json::from_str("\"custom:#1bd96a\"").unwrap();
        assert_eq!(color, AccentColor::Custom("custom:#1bd96a".to_owned()));
        let preset: AccentColor = serde_json::from_str("\"purple\"").unwrap();
        assert_eq!(preset, AccentColor::Purple);
        let system: AccentColor = serde_json::from_str("\"system\"").unwrap();
        assert_eq!(system, AccentColor::System);
    }

    #[test]
    fn download_source_mode_uses_stable_wire_values() {
        assert_eq!(
            serde_json::to_string(&DownloadSourceMode::Auto).unwrap(),
            "\"auto\""
        );
        assert_eq!(
            serde_json::to_string(&DownloadSourceMode::OfficialOnly).unwrap(),
            "\"official_only\""
        );
        assert_eq!(
            serde_json::to_string(&DownloadSourceMode::MirrorPreferred)
                .unwrap(),
            "\"mirror_preferred\""
        );
        assert_eq!(
            serde_json::to_string(&DownloadSourceMode::OfficialPreferred)
                .unwrap(),
            "\"official_preferred\""
        );
    }

    #[tokio::test]
    async fn official_preferred_sources_round_trip_in_a_fresh_database() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();
        sqlx::query(
            "
            UPDATE settings
            SET
                minecraft_metadata_source = 'official_preferred',
                minecraft_file_source = 'official_preferred',
                modrinth_source = 'official_preferred',
                curseforge_source = 'official_preferred'
            WHERE id = 0
            ",
        )
        .execute(&pool)
        .await
        .unwrap();

        let settings = Settings::get(&pool).await.unwrap();
        assert_eq!(
            settings.minecraft_metadata_source,
            DownloadSourceMode::OfficialPreferred
        );
        assert_eq!(
            settings.minecraft_file_source,
            DownloadSourceMode::OfficialPreferred
        );
        assert_eq!(
            settings.modrinth_source,
            DownloadSourceMode::OfficialPreferred
        );
        assert_eq!(
            settings.curseforge_source,
            DownloadSourceMode::OfficialPreferred
        );
    }

    #[tokio::test]
    async fn curseforge_bypass_defaults_on_and_round_trips() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();

        let mut settings = Settings::get(&pool).await.unwrap();
        assert!(settings.resource_chinese_filenames);
        assert!(settings.resource_preserve_filenames);
        settings.resource_chinese_filenames = false;
        settings.resource_preserve_filenames = false;
        assert!(settings.bypass_curseforge_download_restrictions);

        settings.bypass_curseforge_download_restrictions = false;
        settings.update(&pool).await.unwrap();

        let reloaded = Settings::get(&pool).await.unwrap();
        assert!(!reloaded.resource_chinese_filenames);
        assert!(!reloaded.resource_preserve_filenames);
        assert!(!reloaded.bypass_curseforge_download_restrictions);
    }

    #[tokio::test]
    async fn ignore_ssl_errors_defaults_off_and_round_trips() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();

        let mut settings = Settings::get(&pool).await.unwrap();
        assert!(!settings.ignore_ssl_errors);

        settings.ignore_ssl_errors = true;
        settings.update(&pool).await.unwrap();

        let reloaded = Settings::get(&pool).await.unwrap();
        assert!(reloaded.ignore_ssl_errors);
    }

    #[tokio::test]
    async fn memory_optimization_round_trips_in_a_fresh_database() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();

        let mut settings = Settings::get(&pool).await.unwrap();
        assert!(!settings.memory.optimize_before_launch);

        settings.memory.optimize_before_launch = true;
        settings.update(&pool).await.unwrap();

        let reloaded = Settings::get(&pool).await.unwrap();
        assert!(reloaded.memory.optimize_before_launch);
    }

    #[tokio::test]
    async fn lightweight_mode_setting_defaults_off_and_round_trips() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();

        let mut settings = Settings::get(&pool).await.unwrap();
        assert!(!settings.enter_lightweight_mode_on_game_launch);

        settings.enter_lightweight_mode_on_game_launch = true;
        settings.update(&pool).await.unwrap();

        let reloaded = Settings::get(&pool).await.unwrap();
        assert!(reloaded.enter_lightweight_mode_on_game_launch);
    }

    #[tokio::test]
    async fn lightweight_mode_migration_upgrades_existing_settings_database() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE settings (id INTEGER PRIMARY KEY CHECK (id = 0))",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO settings (id) VALUES (0)")
            .execute(&pool)
            .await
            .unwrap();

        sqlx::raw_sql(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/migrations/20260821120001_lightweight-mode.sql"
        )))
        .execute(&pool)
        .await
        .unwrap();

        let enabled: i64 = sqlx::query_scalar(
            "SELECT enter_lightweight_mode_on_game_launch FROM settings WHERE id = 0",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(enabled, 0);
        assert!(
            sqlx::query("PRAGMA foreign_key_check")
                .fetch_all(&pool)
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn home_widgets_round_trip_in_a_fresh_database() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();

        let expected = serde_json::json!({
            "version": 1,
            "widgets": [
                {
                    "id": "calendar-one",
                    "kind": "calendar",
                    "size": "2x2"
                }
            ]
        });
        let mut settings = Settings::get(&pool).await.unwrap();
        settings.home_widgets = Some(expected.clone());
        settings.update(&pool).await.unwrap();

        let reloaded = Settings::get(&pool).await.unwrap();
        assert_eq!(reloaded.home_widgets, Some(expected));
    }

    #[tokio::test]
    async fn font_columns_upgrade_an_existing_settings_database() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE settings (id INTEGER PRIMARY KEY CHECK (id = 0))",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO settings (id) VALUES (0)")
            .execute(&pool)
            .await
            .unwrap();

        sqlx::raw_sql(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/migrations/20260919120000_custom-fonts.sql"
        )))
        .execute(&pool)
        .await
        .unwrap();

        let fonts: (Option<String>, Option<String>) = sqlx::query_as(
            "SELECT ui_font, mono_font FROM settings WHERE id = 0",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(fonts, (None, None));
        assert!(
            sqlx::query("PRAGMA foreign_key_check")
                .fetch_all(&pool)
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn fonts_default_to_none_and_round_trip_in_a_fresh_database() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();

        let mut settings = Settings::get(&pool).await.unwrap();
        assert_eq!(settings.ui_font, None);
        assert_eq!(settings.mono_font, None);

        settings.ui_font = Some("  Microsoft YaHei  ".to_string());
        settings.mono_font = Some("JetBrains Mono".to_string());
        settings.update(&pool).await.unwrap();

        let reloaded = Settings::get(&pool).await.unwrap();
        assert_eq!(reloaded.ui_font.as_deref(), Some("Microsoft YaHei"));
        assert_eq!(reloaded.mono_font.as_deref(), Some("JetBrains Mono"));

        settings.mono_font = Some("   ".to_string());
        settings.update(&pool).await.unwrap();

        let cleared = Settings::get(&pool).await.unwrap();
        assert_eq!(cleared.mono_font, None);
    }

    #[tokio::test]
    async fn terracotta_public_nodes_default_and_empty_list_round_trip() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();

        let mut settings = Settings::get(&pool).await.unwrap();
        assert_eq!(
            settings.terracotta_public_nodes,
            default_terracotta_public_nodes()
        );

        settings.terracotta_public_nodes.clear();
        settings.update(&pool).await.unwrap();

        let reloaded = Settings::get(&pool).await.unwrap();
        assert!(reloaded.terracotta_public_nodes.is_empty());
    }

    #[tokio::test]
    async fn mojang_auth_source_round_trip_in_a_fresh_database() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();

        let mut settings = Settings::get(&pool).await.unwrap();
        assert_eq!(settings.mojang_auth_source, DownloadSourceMode::Auto);
        settings.mojang_auth_source = DownloadSourceMode::MirrorPreferred;
        settings.update(&pool).await.unwrap();

        let reloaded = Settings::get(&pool).await.unwrap();
        assert_eq!(
            reloaded.mojang_auth_source,
            DownloadSourceMode::MirrorPreferred
        );
    }

    #[test]
    fn auto_source_detection_distinguishes_mainland_locales() {
        assert!(locale_prefers_mirror("zh-CN"));
        assert!(locale_prefers_mirror("zh_Hans"));
        assert!(locale_prefers_mirror("Asia/Shanghai"));
        assert!(!locale_prefers_mirror("zh-TW"));
        assert!(!locale_prefers_mirror("en-US"));
    }

    #[tokio::test]
    async fn legacy_mirror_settings_keep_their_previous_intent() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();
        let settings = Settings::get(&pool).await.unwrap();
        assert!(settings.auto_concurrent_downloads);
        assert!(settings.auto_set_java_high_performance_mode);
        assert!(!settings.auto_hide_downloads_button);
        assert_eq!(settings.home_layout, HomeLayout::Standard);
        assert_eq!(settings.minimal_home_instance_id, None);
        assert_eq!(settings.home_widgets, None);
        assert_eq!(
            settings.minecraft_metadata_source,
            DownloadSourceMode::Auto
        );
        assert_eq!(settings.minecraft_file_source, DownloadSourceMode::Auto);
        assert_eq!(settings.modrinth_source, DownloadSourceMode::Auto);
        assert_eq!(settings.curseforge_source, DownloadSourceMode::Auto);
        let mut legacy = serde_json::to_value(settings).unwrap();
        let object = legacy.as_object_mut().unwrap();
        object.remove("minecraft_metadata_source");
        object.remove("minecraft_file_source");
        object.remove("modrinth_source");
        object.remove("curseforge_source");
        object.insert("use_minecraft_mirror".to_string(), true.into());
        object.insert("use_modrinth_mirror".to_string(), false.into());
        object.insert("use_curseforge_mirror".to_string(), false.into());

        let mut migrated: Settings = serde_json::from_value(legacy).unwrap();
        migrated.apply_legacy_download_source_settings();

        assert_eq!(
            migrated.minecraft_metadata_source,
            DownloadSourceMode::MirrorPreferred
        );
        assert_eq!(
            migrated.minecraft_file_source,
            DownloadSourceMode::MirrorPreferred
        );
        assert_eq!(migrated.modrinth_source, DownloadSourceMode::OfficialOnly);
        assert_eq!(
            migrated.curseforge_source,
            DownloadSourceMode::OfficialOnly
        );
    }

    #[tokio::test]
    async fn download_source_reset_migration_sets_all_sources_to_auto() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();
        sqlx::query(
            "
            UPDATE settings
            SET
                minecraft_metadata_source = 'official_only',
                minecraft_file_source = 'mirror_preferred',
                modrinth_source = 'official_only',
                curseforge_source = 'mirror_preferred'
            ",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/migrations/20260721120000_reset-download-sources-to-auto.sql"
        )))
        .execute(&pool)
        .await
        .unwrap();

        let settings = Settings::get(&pool).await.unwrap();
        assert_eq!(
            settings.minecraft_metadata_source,
            DownloadSourceMode::Auto
        );
        assert_eq!(settings.minecraft_file_source, DownloadSourceMode::Auto);
        assert_eq!(settings.modrinth_source, DownloadSourceMode::Auto);
        assert_eq!(settings.curseforge_source, DownloadSourceMode::Auto);
    }

    #[tokio::test]
    async fn telemetry_schema_migrates_fresh_and_existing_settings_databases() {
        let fresh = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!().run(&fresh).await.unwrap();
        let settings = Settings::get(&fresh).await.unwrap();
        assert!(!settings.telemetry);
        assert_eq!(settings.telemetry_consent_version, 0);
        assert!(
            sqlx::query("PRAGMA foreign_key_check")
                .fetch_all(&fresh)
                .await
                .unwrap()
                .is_empty()
        );

        let upgrade = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE settings (id INTEGER PRIMARY KEY CHECK (id = 0), telemetry INTEGER NOT NULL DEFAULT 0, discord_rpc INTEGER NOT NULL DEFAULT 1)",
        )
        .execute(&upgrade)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO settings (id, telemetry, discord_rpc) VALUES (0, 0, 1)",
        )
        .execute(&upgrade)
        .await
        .unwrap();
        sqlx::raw_sql(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/migrations/20260814120000_telemetry.sql"
        )))
        .execute(&upgrade)
        .await
        .unwrap();
        let consent_version = sqlx::query_scalar::<_, i64>(
            "SELECT telemetry_consent_version FROM settings WHERE id = 0",
        )
        .fetch_one(&upgrade)
        .await
        .unwrap();
        assert_eq!(consent_version, 0);
        assert!(
            sqlx::query("PRAGMA foreign_key_check")
                .fetch_all(&upgrade)
                .await
                .unwrap()
                .is_empty()
        );
    }
}

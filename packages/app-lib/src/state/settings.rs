//! Theseus settings file

use crate::state::store::Collection as _;
use crate::state::store::settings::{
    DownloadSource as StoredDownloadSource, HomeLayout as StoredHomeLayout,
    Settings as StoredSettings, SettingsRecord, Singleton,
};
use crate::state::store::{Atom, Handle};
use crate::util::download::constants::{
    AUTO_DOWNLOAD_CONCURRENCY_INITIAL, DOWNLOAD_CONCURRENCY_HARD_MAX,
};
use serde::{Deserialize, Serialize};
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
    #[serde(default)]
    pub default_game_language: String,
    pub default_page: DefaultPage,
    pub collapsed_navigation: bool,
    pub hide_nametag_skins_page: bool,
    pub nametag_uses_app_font: bool,
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

/// Minecraft language codes use lower-case names with underscores. An empty
/// value keeps following the launcher locale.
fn normalize_game_language(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace('-', "_")
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

    pub async fn get(handle: &Handle) -> crate::Result<Self> {
        let Some(record) = StoredSettings::get(handle, &Singleton).await?
        else {
            return Err(crate::ErrorKind::OtherError(
                "The launcher settings are missing from the database"
                    .to_string(),
            )
            .into());
        };
        Ok(Self::from_record(record))
    }

    fn from_record(record: SettingsRecord) -> Self {
        let mut hidden_nav_items: Vec<String> =
            serde_json::from_str(&record.hidden_nav_items).unwrap_or_default();
        // Home and Library must stay reachable from the nav rail.
        hidden_nav_items.retain(|id| id != "home" && id != "library");

        Self {
            resource_chinese_filenames: record.resource_chinese_filenames,
            resource_preserve_filenames: record.resource_preserve_filenames,
            max_concurrent_downloads: record.max_concurrent_downloads as usize,
            max_concurrent_writes: record.max_concurrent_writes as usize,
            auto_concurrent_downloads: record.auto_concurrent_downloads,
            minecraft_metadata_source: to_source_mode(
                record.minecraft_metadata_source,
            ),
            minecraft_file_source: to_source_mode(record.minecraft_file_source),
            modrinth_source: to_source_mode(record.modrinth_source),
            curseforge_source: to_source_mode(record.curseforge_source),
            bypass_curseforge_download_restrictions: record
                .bypass_curseforge_download_restrictions,
            ignore_ssl_errors: record.ignore_ssl_errors,
            doh_enabled: record.doh_enabled,
            mojang_auth_source: to_source_mode(record.mojang_auth_source),
            legacy_use_minecraft_mirror: None,
            legacy_use_modrinth_mirror: None,
            legacy_use_curseforge_mirror: None,
            theme: Theme::from_string(&record.theme),
            accent_color: AccentColor::from_string(&record.accent_color),
            locale: record.locale,
            default_game_language: record.default_game_language,
            default_page: DefaultPage::from_string(&record.default_page),
            collapsed_navigation: record.collapsed_navigation,
            hide_nametag_skins_page: record.hide_nametag_skins_page,
            nametag_uses_app_font: record.nametag_uses_app_font,
            advanced_rendering: record.advanced_rendering,
            native_decorations: record.native_decorations,
            toggle_sidebar: record.toggle_sidebar,
            custom_background_path: record.custom_background_path,
            custom_background_blur: record.custom_background_blur as u32,
            custom_background_opacity: record.custom_background_opacity as u32,
            custom_background_component_opacity: record
                .custom_background_component_opacity
                .clamp(0, 100)
                as u32,
            ui_font: sanitize_font_family(record.ui_font),
            mono_font: sanitize_font_family(record.mono_font),
            transparent_background: record.transparent_background,
            transparent_background_opacity: record
                .transparent_background_opacity
                as u32,
            transparent_background_blur: record.transparent_background_blur,
            sidebar_instance_count: record.sidebar_instance_count as u32,
            close_behavior: record.close_behavior,
            log_level: record.log_level,
            auto_hide_downloads_button: record.auto_hide_downloads_button,
            home_layout: to_home_layout(record.home_layout),
            minimal_home_instance_id: record.minimal_home_instance_id,
            home_widgets: record
                .home_widgets
                .as_deref()
                .and_then(|value| serde_json::from_str(value).ok()),
            home_widget_background_opacity: record
                .home_widget_background_opacity
                .clamp(0, 100)
                as u32,
            hidden_nav_items,
            custom_window_title_enabled: record.custom_window_title_enabled,
            default_window_title: record.default_window_title,
            terracotta_public_nodes: serde_json::from_str(
                &record.terracotta_public_nodes,
            )
            .ok()
            .unwrap_or_else(default_terracotta_public_nodes),
            telemetry: record.telemetry,
            telemetry_consent_version: record.telemetry_consent_version as u32,
            discord_rpc: record.discord_rpc,
            developer_mode: record.developer_mode,
            personalized_ads: record.personalized_ads,
            onboarded: record.onboarded,
            onboarding_version: record.onboarding_version as usize,
            onboarding_instance_tour_completed: record
                .onboarding_instance_tour_completed,
            extra_launch_args: serde_json::from_str(&record.extra_launch_args)
                .ok()
                .unwrap_or_default(),
            custom_env_vars: serde_json::from_str(&record.custom_env_vars)
                .ok()
                .unwrap_or_default(),
            memory: MemorySettings {
                maximum: record.mc_memory_max as u32,
                automatic: record.mc_memory_auto,
                optimize_before_launch: record.mc_memory_optimize,
            },
            force_fullscreen: record.mc_force_fullscreen,
            maximize_window: record.mc_maximize_window,
            game_resolution: WindowSize(
                record.mc_game_resolution_x as u16,
                record.mc_game_resolution_y as u16,
            ),
            hide_on_process_start: record.hide_on_process_start,
            enter_lightweight_mode_on_game_launch: record
                .enter_lightweight_mode_on_game_launch,
            auto_set_java_high_performance_mode: record
                .auto_set_java_high_performance_mode,
            hooks: Hooks {
                pre_launch: record.hook_pre_launch,
                wrapper: record.hook_wrapper,
                post_exit: record.hook_post_exit,
            },
            custom_dir: record.custom_dir,
            prev_custom_dir: record.prev_custom_dir,
            backup_repository_path: record.backup_repository_path,
            migrated: record.migrated,
            feature_flags: serde_json::from_str(&record.feature_flags)
                .ok()
                .unwrap_or_default(),
            sync_features_across_devices: record.sync_features_across_devices,
            show_files_tab_in_instances: record.show_files_tab_in_instances,
            show_worlds_tab_in_instances: record.show_worlds_tab_in_instances,
            show_screenshots_tab_in_instances: record
                .show_screenshots_tab_in_instances,
            show_skin_selector_in_sidebar: record.show_skin_selector_in_sidebar,
            skipped_update: record.skipped_update,
            pending_update_toast_for_version: record
                .pending_update_toast_for_version,
            auto_download_updates: record.auto_download_updates,
            allow_external_scheme: record.allow_external_scheme,
            allow_privileged_scheme: record.allow_privileged_scheme,
            version: record.version as usize,
        }
    }

    pub async fn update(&self, handle: &Handle) -> crate::Result<()> {
        StoredSettings::set(handle, &Singleton, Some(&self.to_record()?)).await
    }

    fn to_record(&self) -> crate::Result<SettingsRecord> {
        let auto_prefers_mirror = self.auto_prefers_mirror();
        Ok(SettingsRecord {
            max_concurrent_downloads: self
                .max_concurrent_downloads
                .clamp(1, DOWNLOAD_CONCURRENCY_HARD_MAX)
                as i64,
            max_concurrent_writes: self.max_concurrent_writes as i64,
            theme: self.theme.as_str().to_string(),
            default_page: self.default_page.as_str().to_string(),
            collapsed_navigation: self.collapsed_navigation,
            advanced_rendering: self.advanced_rendering,
            native_decorations: self.native_decorations,
            telemetry: self.telemetry,
            discord_rpc: self.discord_rpc,
            developer_mode: self.developer_mode,
            onboarded: self.onboarded,
            extra_launch_args: serde_json::to_string(&self.extra_launch_args)?,
            custom_env_vars: serde_json::to_string(&self.custom_env_vars)?,
            mc_memory_max: self.memory.maximum as i64,
            mc_force_fullscreen: self.force_fullscreen,
            mc_game_resolution_x: self.game_resolution.0 as i64,
            mc_game_resolution_y: self.game_resolution.1 as i64,
            hide_on_process_start: self.hide_on_process_start,
            hook_pre_launch: self.hooks.pre_launch.clone(),
            hook_wrapper: self.hooks.wrapper.clone(),
            hook_post_exit: self.hooks.post_exit.clone(),
            custom_dir: self.custom_dir.clone(),
            prev_custom_dir: self.prev_custom_dir.clone(),
            migrated: self.migrated,
            personalized_ads: self.personalized_ads,
            toggle_sidebar: self.toggle_sidebar,
            feature_flags: serde_json::to_string(&self.feature_flags)?,
            hide_nametag_skins_page: self.hide_nametag_skins_page,
            nametag_uses_app_font: self.nametag_uses_app_font,
            skipped_update: self.skipped_update.clone(),
            pending_update_toast_for_version: self
                .pending_update_toast_for_version
                .clone(),
            auto_download_updates: self.auto_download_updates,
            version: self.version as i64,
            locale: self.locale.clone(),
            default_game_language: normalize_game_language(
                &self.default_game_language,
            ),
            accent_color: self.accent_color.as_str().to_string(),
            custom_background_path: self.custom_background_path.clone(),
            custom_background_blur: self.custom_background_blur.min(40) as i64,
            custom_background_opacity: self
                .custom_background_opacity
                .clamp(10, 100) as i64,
            use_minecraft_mirror: self
                .minecraft_file_source
                .prefers_mirror(auto_prefers_mirror),
            use_modrinth_mirror: self
                .modrinth_source
                .prefers_mirror(auto_prefers_mirror),
            use_curseforge_mirror: self
                .curseforge_source
                .prefers_mirror(auto_prefers_mirror),
            auto_concurrent_downloads: self.auto_concurrent_downloads,
            minecraft_metadata_source: from_source_mode(
                self.minecraft_metadata_source,
            ),
            minecraft_file_source: from_source_mode(self.minecraft_file_source),
            modrinth_source: from_source_mode(self.modrinth_source),
            curseforge_source: from_source_mode(self.curseforge_source),
            onboarding_version: self.onboarding_version as i64,
            onboarding_instance_tour_completed: self
                .onboarding_instance_tour_completed,
            sidebar_instance_count: self.sidebar_instance_count.min(50) as i64,
            mc_memory_auto: self.memory.automatic,
            mc_memory_optimize: self.memory.optimize_before_launch,
            auto_set_java_high_performance_mode: self
                .auto_set_java_high_performance_mode,
            transparent_background: self.transparent_background,
            transparent_background_opacity: self
                .transparent_background_opacity
                .min(100) as i64,
            transparent_background_blur: self.transparent_background_blur,
            home_layout: from_home_layout(self.home_layout),
            minimal_home_instance_id: self.minimal_home_instance_id.clone(),
            auto_hide_downloads_button: self.auto_hide_downloads_button,
            home_widgets: self
                .home_widgets
                .as_ref()
                .map(serde_json::to_string)
                .transpose()?,
            mojang_auth_source: from_source_mode(self.mojang_auth_source),
            terracotta_public_nodes: serde_json::to_string(
                &self.terracotta_public_nodes,
            )?,
            telemetry_consent_version: self.telemetry_consent_version as i64,
            bypass_curseforge_download_restrictions: self
                .bypass_curseforge_download_restrictions,
            enter_lightweight_mode_on_game_launch: self
                .enter_lightweight_mode_on_game_launch,
            close_behavior: self.close_behavior.clone(),
            mc_maximize_window: self.maximize_window,
            log_level: self.log_level.clone(),
            sync_features_across_devices: self.sync_features_across_devices,
            show_files_tab_in_instances: self.show_files_tab_in_instances,
            show_worlds_tab_in_instances: self.show_worlds_tab_in_instances,
            show_screenshots_tab_in_instances: self
                .show_screenshots_tab_in_instances,
            show_skin_selector_in_sidebar: self.show_skin_selector_in_sidebar,
            ignore_ssl_errors: self.ignore_ssl_errors,
            home_widget_background_opacity: self
                .home_widget_background_opacity
                .clamp(0, 100)
                as i64,
            hidden_nav_items: {
                let mut hidden_nav_items = self.hidden_nav_items.clone();
                // Boundary guard: never persist core nav items as hidden.
                hidden_nav_items.retain(|id| id != "home" && id != "library");
                serde_json::to_string(&hidden_nav_items)?
            },
            custom_window_title_enabled: self.custom_window_title_enabled,
            default_window_title: self.default_window_title.trim().to_string(),
            custom_background_component_opacity: self
                .custom_background_component_opacity
                .clamp(0, 100)
                as i64,
            ui_font: sanitize_font_family(self.ui_font.clone()),
            mono_font: sanitize_font_family(self.mono_font.clone()),
            backup_repository_path: self.backup_repository_path.clone(),
            allow_external_scheme: self.allow_external_scheme,
            allow_privileged_scheme: self.allow_privileged_scheme,
            doh_enabled: self.doh_enabled,
            resource_chinese_filenames: self.resource_chinese_filenames,
            resource_preserve_filenames: self.resource_preserve_filenames,
        })
    }
    /// The settings as they stand inside `unit`, for a caller that means to
    /// change some of them and write them back.
    pub(crate) async fn load_in(unit: &mut Atom) -> crate::Result<Self> {
        match StoredSettings::atom_get(unit, &Singleton).await? {
            Some(record) => Ok(Self::from_record(record)),
            None => Err(crate::ErrorKind::OtherError(
                "The launcher settings are missing from the database"
                    .to_string(),
            )
            .into()),
        }
    }

    /// Writes these settings inside `unit`.
    pub(crate) async fn save_in(&self, unit: &mut Atom) -> crate::Result<()> {
        StoredSettings::atom_set(unit, &Singleton, Some(&self.to_record()?))
            .await
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

    pub async fn migrate(handle: &Handle) -> crate::Result<()> {
        let mut settings = Self::get(handle).await?;

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

        settings.update(handle).await?;

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

fn to_source_mode(source: StoredDownloadSource) -> DownloadSourceMode {
    match source {
        StoredDownloadSource::Auto => DownloadSourceMode::Auto,
        StoredDownloadSource::OfficialOnly => DownloadSourceMode::OfficialOnly,
        StoredDownloadSource::MirrorPreferred => {
            DownloadSourceMode::MirrorPreferred
        }
        StoredDownloadSource::OfficialPreferred => {
            DownloadSourceMode::OfficialPreferred
        }
    }
}

fn from_source_mode(mode: DownloadSourceMode) -> StoredDownloadSource {
    match mode {
        DownloadSourceMode::Auto => StoredDownloadSource::Auto,
        DownloadSourceMode::OfficialOnly => StoredDownloadSource::OfficialOnly,
        DownloadSourceMode::MirrorPreferred => {
            StoredDownloadSource::MirrorPreferred
        }
        DownloadSourceMode::OfficialPreferred => {
            StoredDownloadSource::OfficialPreferred
        }
    }
}

fn to_home_layout(layout: StoredHomeLayout) -> HomeLayout {
    match layout {
        StoredHomeLayout::Standard => HomeLayout::Standard,
        StoredHomeLayout::Minimal => HomeLayout::Minimal,
    }
}

fn from_home_layout(layout: HomeLayout) -> StoredHomeLayout {
    match layout {
        HomeLayout::Standard => StoredHomeLayout::Standard,
        HomeLayout::Minimal => StoredHomeLayout::Minimal,
    }
}
#[cfg(test)]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_game_language_normalizes_to_minecraft_code() {
        assert_eq!(normalize_game_language("  EN-US  "), "en_us");
        assert_eq!(normalize_game_language("zh_CN"), "zh_cn");
        assert_eq!(normalize_game_language("   "), "");
    }

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

    #[test]
    fn auto_source_detection_distinguishes_mainland_locales() {
        assert!(locale_prefers_mirror("zh-CN"));
        assert!(locale_prefers_mirror("zh_Hans"));
        assert!(locale_prefers_mirror("Asia/Shanghai"));
        assert!(!locale_prefers_mirror("zh-TW"));
        assert!(!locale_prefers_mirror("en-US"));
    }
}

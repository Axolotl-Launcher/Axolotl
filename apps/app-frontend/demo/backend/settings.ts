import type { AppSettings } from '@/helpers/settings'
import { DEFAULT_FEATURE_FLAGS } from '@/store/theme'

import type { Handlers } from './types'

/**
 * A fresh install's settings (the defaults of app-lib's settings table), with
 * onboarding marked done so visitors land on the launcher itself.
 */
const settings: AppSettings = {
    doh_enabled: false,
    max_concurrent_downloads: 10,
    max_concurrent_writes: 10,
    auto_concurrent_downloads: true,
    minecraft_metadata_source: 'auto',
    minecraft_file_source: 'auto',
    modrinth_source: 'auto',
    curseforge_source: 'auto',
    bypass_curseforge_download_restrictions: true,
    resource_chinese_filenames: true,
    resource_preserve_filenames: true,
    ignore_ssl_errors: false,
    mojang_auth_source: 'auto',

    theme: 'dark',
    accent_color: 'pink',
    locale: 'zh-CN',
    default_page: 'Home',
    collapsed_navigation: false,
    hide_nametag_skins_page: false,
    advanced_rendering: true,
    native_decorations: false,
    toggle_sidebar: false,
    custom_background_path: null,
    custom_background_blur: 0,
    custom_background_opacity: 100,
    custom_background_component_opacity: 100,
    ui_font: null,
    mono_font: null,
    transparent_background: false,
    transparent_background_opacity: 100,
    transparent_background_blur: false,
    sidebar_instance_count: 0,
    auto_hide_downloads_button: false,
    home_layout: 'standard',
    minimal_home_instance_id: null,
    close_behavior: 'ask',
    log_level: 'info',
    home_widgets: null,
    home_widget_background_opacity: 100,
    hidden_nav_items: [],
    custom_window_title_enabled: false,
    default_window_title: 'Minecraft',
    terracotta_public_nodes: ['wss://center.node.1tmc.top'],

    telemetry: false,
    telemetry_consent_version: 1,
    discord_rpc: false,
    allow_external_scheme: true,
    allow_privileged_scheme: false,
    onboarded: true,
    onboarding_version: 99,
    onboarding_instance_tour_completed: true,

    extra_launch_args: [],
    custom_env_vars: [],
    memory: { maximum: 2048 } as AppSettings['memory'],
    force_fullscreen: false,
    maximize_window: false,
    game_resolution: [854, 480],
    hide_on_process_start: false,
    enter_lightweight_mode_on_game_launch: false,
    auto_set_java_high_performance_mode: true,
    hooks: {} as AppSettings['hooks'],

    custom_dir: null,
    prev_custom_dir: null,
    migrated: true,

    developer_mode: false,
    feature_flags: { ...DEFAULT_FEATURE_FLAGS },
    sync_features_across_devices: false,
    show_files_tab_in_instances: true,
    show_worlds_tab_in_instances: true,
    show_screenshots_tab_in_instances: false,
    show_skin_selector_in_sidebar: true,

    skipped_update: null,
    pending_update_toast_for_version: null,
    auto_download_updates: null,

    version: 1,
}

export const settingsHandlers: Handlers = {
    'plugin:settings|settings_get': () => structuredClone(settings),
    'plugin:settings|settings_set': ({ settings: next }) => {
        Object.assign(settings, next)
    },
    'plugin:settings|settings_patch': ({ patch }) => {
        Object.assign(settings, patch)
        return structuredClone(settings)
    },
    'plugin:auth|set_mojang_auth_use_mirror': () => true,
}

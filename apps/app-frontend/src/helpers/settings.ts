/**
 * All theseus API calls return serialized values (both return values and errors);
 * So, for example, addDefaultInstance creates a blank instance object, where the Rust struct is serialized,
 *  and deserialized into a usable JS object.
 */
import { queryOptions } from '@tanstack/vue-query'
import { invoke } from '@tauri-apps/api/core'

import type { HomeDashboardConfig } from '@/components/home/home-dashboard'
import type { Hooks, MemorySettings } from '@/helpers/types'
import type { AccentColorSetting, ColorTheme, FeatureFlag, HomeLayout } from '@/store/theme.ts'
import { DEFAULT_FEATURE_FLAGS } from '@/store/theme.ts'

export type { BrowseContentDisplayMode, BrowseContentProjectType } from './browse-display-mode.ts'
export {
    getLastBrowseContentDisplayMode,
    getLastBrowseContentProjectType,
    isBrowseContentProjectType,
    setLastBrowseContentDisplayMode,
    setLastBrowseContentProjectType,
} from './browse-display-mode.ts'

// Settings object
/*

Settings {
    "memory": MemorySettings,
    "game_resolution": [int int],
    "custom_java_args": [String ...],
    "custom_env_args" : [(string, string) ... ]>,
    "java_globals": Hash of (string, Path),
    "default_user": Uuid string (can be null),
    "hooks": Hooks,
    "max_concurrent_downloads": uint,
    "version": u32,
    "collapsed_navigation": bool,
}

Memorysettings {
    "min": u32, can be null,
    "max": u32,
}

*/

export type UpdateChannel = 'release' | 'beta'
export type UpdatePreferences = {
    immediateUpdateFetch: boolean
    updatesPaused: boolean
}
export type DownloadSourceMode =
    'auto' | 'official_only' | 'mirror_preferred' | 'official_preferred'
export type ProxyMode = 'none' | 'system' | 'custom'
export type ProxyConfig = {
    mode: ProxyMode
    url: string
    username: string
    password: string
}
export type ProxyTestResult = {
    success: boolean
    latency_ms: number | null
    message: string
}
export type DohSettings = {
    doh_enabled: boolean
}

export async function getUpdateChannel(): Promise<UpdateChannel> {
    const channel = await invoke<string>('get_update_channel')
    return channel === 'beta' ? 'beta' : 'release'
}

export async function setUpdateChannel(channel: UpdateChannel): Promise<void> {
    await invoke('set_update_channel', { channel })
}

export async function copyReleaseDatabaseToBeta(): Promise<void> {
    await invoke('copy_release_database_to_beta')
}

export async function betaDatabaseExists(): Promise<boolean> {
    return await invoke('beta_database_exists')
}

export async function getCurrentAppDatabasePath(): Promise<string> {
    return await invoke('get_current_app_database_path')
}

export async function copyDatabaseBetweenChannels(
    sourceChannel: UpdateChannel,
    targetChannel: UpdateChannel,
): Promise<void> {
    await invoke('copy_database_between_channels', { sourceChannel, targetChannel })
}

export async function getUpdatePreferences(): Promise<UpdatePreferences> {
    return await invoke('get_update_preferences')
}

export async function setUpdatePreferences(preferences: UpdatePreferences): Promise<void> {
    await invoke('set_update_preferences', preferences)
}

export type BrowseContentSource =
    'all' | 'modrinth' | 'curseforge' | 'mcarchive' | 'planet_minecraft'

const BROWSE_CONTENT_SOURCE_STORAGE_KEY = 'axolotl-browse-content-source'
const BROWSE_DEFAULT_INSTANCE_STORAGE_KEY = 'axolotl-browse-default-instance'

export function getLastBrowseContentSource(): BrowseContentSource | null {
    const value = localStorage.getItem(BROWSE_CONTENT_SOURCE_STORAGE_KEY)
    return value === 'all' ||
        value === 'modrinth' ||
        value === 'curseforge' ||
        value === 'mcarchive' ||
        value === 'planet_minecraft'
        ? value
        : null
}

export function setLastBrowseContentSource(source: BrowseContentSource) {
    localStorage.setItem(BROWSE_CONTENT_SOURCE_STORAGE_KEY, source)
}

export function getBrowseDefaultInstanceId(): string | null {
    return localStorage.getItem(BROWSE_DEFAULT_INSTANCE_STORAGE_KEY)
}

export function setBrowseDefaultInstanceId(instanceId: string | null) {
    if (instanceId) {
        localStorage.setItem(BROWSE_DEFAULT_INSTANCE_STORAGE_KEY, instanceId)
    } else {
        localStorage.removeItem(BROWSE_DEFAULT_INSTANCE_STORAGE_KEY)
    }
}

export type AppSettings = DohSettings & {
    max_concurrent_downloads: number
    max_concurrent_writes: number
    auto_concurrent_downloads: boolean
    minecraft_metadata_source: DownloadSourceMode
    minecraft_file_source: DownloadSourceMode
    modrinth_source: DownloadSourceMode
    curseforge_source: DownloadSourceMode
    bypass_curseforge_download_restrictions: boolean
    resource_chinese_filenames: boolean
    resource_preserve_filenames: boolean
    ignore_ssl_errors: boolean
    mojang_auth_source: DownloadSourceMode

    theme: ColorTheme
    accent_color: AccentColorSetting
    locale: string
    default_page: 'Home' | 'DiscoverContent' | 'Library'
    collapsed_navigation: boolean
    hide_nametag_skins_page: boolean
    advanced_rendering: boolean
    native_decorations: boolean
    toggle_sidebar: boolean
    custom_background_path: string | null
    custom_background_blur: number
    custom_background_opacity: number
    custom_background_component_opacity: number
    ui_font: string | null
    mono_font: string | null
    transparent_background: boolean
    transparent_background_opacity: number
    transparent_background_blur: boolean
    sidebar_instance_count: number
    auto_hide_downloads_button: boolean
    home_layout: HomeLayout
    minimal_home_instance_id: string | null
    close_behavior: 'ask' | 'close' | 'lightweight'
    log_level: 'error' | 'warn' | 'info' | 'debug' | 'trace'
    home_widgets: HomeDashboardConfig | null
    home_widget_background_opacity: number
    hidden_nav_items: string[]
    custom_window_title_enabled: boolean
    default_window_title: string
    terracotta_public_nodes: string[]

    telemetry: boolean
    telemetry_consent_version: number
    discord_rpc: boolean
    allow_external_scheme: boolean
    allow_privileged_scheme: boolean
    onboarded: boolean
    onboarding_version: number
    onboarding_instance_tour_completed: boolean

    extra_launch_args: string[]
    custom_env_vars: [string, string][]
    memory: MemorySettings
    force_fullscreen: boolean
    maximize_window: boolean
    game_resolution: [number, number]
    hide_on_process_start: boolean
    enter_lightweight_mode_on_game_launch: boolean
    auto_set_java_high_performance_mode: boolean
    hooks: Hooks

    custom_dir?: string | null
    prev_custom_dir?: string | null
    migrated: boolean

    developer_mode: boolean
    feature_flags: Record<FeatureFlag, boolean>
    sync_features_across_devices: boolean
    show_files_tab_in_instances: boolean
    show_worlds_tab_in_instances: boolean
    show_screenshots_tab_in_instances: boolean
    show_skin_selector_in_sidebar: boolean

    skipped_update: string | null
    pending_update_toast_for_version: string | null
    auto_download_updates: boolean | null

    version: number
}

export type PrivacySettings = {
    telemetry: boolean
    discord_rpc: boolean
    consent_version: number
}

type LegacyMirrorSettings = {
    use_minecraft_mirror?: boolean
    use_modrinth_mirror?: boolean
    use_curseforge_mirror?: boolean
}

function normalizeDownloadSettings(settings: AppSettings & LegacyMirrorSettings): AppSettings {
    settings.close_behavior ??= 'ask'
    settings.log_level ??= 'info'
    const hasLegacySettings =
        typeof settings.use_minecraft_mirror === 'boolean' &&
        typeof settings.use_modrinth_mirror === 'boolean' &&
        typeof settings.use_curseforge_mirror === 'boolean'
    const usesLegacyDefaults =
        hasLegacySettings &&
        !settings.use_minecraft_mirror &&
        !settings.use_modrinth_mirror &&
        settings.use_curseforge_mirror
    const legacySource = (enabled: boolean | undefined): DownloadSourceMode =>
        enabled ? 'mirror_preferred' : 'official_only'

    settings.auto_concurrent_downloads ??= true
    settings.auto_set_java_high_performance_mode ??= true
    settings.minecraft_metadata_source ??=
        usesLegacyDefaults || !hasLegacySettings
            ? 'auto'
            : legacySource(settings.use_minecraft_mirror)
    settings.minecraft_file_source ??=
        usesLegacyDefaults || !hasLegacySettings
            ? 'auto'
            : legacySource(settings.use_minecraft_mirror)
    settings.modrinth_source ??=
        usesLegacyDefaults || !hasLegacySettings
            ? 'auto'
            : legacySource(settings.use_modrinth_mirror)
    settings.curseforge_source ??=
        usesLegacyDefaults || !hasLegacySettings
            ? 'auto'
            : legacySource(settings.use_curseforge_mirror)
    settings.bypass_curseforge_download_restrictions ??= true
    settings.resource_chinese_filenames ??= true
    settings.resource_preserve_filenames ??= true
    settings.ignore_ssl_errors ??= false
    settings.mojang_auth_source ??= 'auto'
    settings.terracotta_public_nodes ??= ['wss://center.node.1tmc.top']
    settings.hidden_nav_items ??= []
    settings.allow_external_scheme ??= true
    settings.allow_privileged_scheme ??= false
    settings.custom_window_title_enabled ??= false
    settings.default_window_title ??= 'Minecraft'
    settings.custom_background_component_opacity ??= 100
    settings.ui_font ??= null
    settings.mono_font ??= null
    settings.feature_flags ??= { ...DEFAULT_FEATURE_FLAGS }
    settings.sync_features_across_devices ??= false
    settings.show_files_tab_in_instances ??= true
    settings.show_worlds_tab_in_instances ??= true
    settings.show_screenshots_tab_in_instances ??= false
    settings.show_skin_selector_in_sidebar ??= true
    for (const [key, value] of Object.entries(DEFAULT_FEATURE_FLAGS)) {
        settings.feature_flags[key as FeatureFlag] ??= value
    }

    return settings
}

export const appSettingsKeys = {
    all: ['app-settings'] as const,
    update: ['app-settings', 'update'] as const,
}

export function appSettingsQueryOptions() {
    return queryOptions({
        queryKey: appSettingsKeys.all,
        queryFn: get,
        staleTime: 0,
    })
}

export function serializeEnvVars(vars: [string, string][] | undefined | null): string {
    return (vars ?? []).map(([key, value]) => `${key}=${value}`).join(' ')
}

export function parseEnvVars(input: string | undefined | null): [string, string][] {
    if (!input?.trim()) return []

    const vars: [string, string][] = []
    for (const entry of input.trim().split(/\s+/)) {
        const separator = entry.indexOf('=')
        if (separator <= 0 || entry.includes('\0')) {
            throw new Error(
                'Environment variables must use NAME=value and cannot contain null characters.',
            )
        }
        vars.push([entry.slice(0, separator), entry.slice(separator + 1)])
    }
    return vars
}

// Get full settings object
export async function get() {
    const settings = normalizeDownloadSettings(
        (await invoke('plugin:settings|settings_get')) as AppSettings & LegacyMirrorSettings,
    )
    return settings
}

export type AppSettingsPatch = {
    [K in keyof AppSettings]?: AppSettings[K] extends Record<string, unknown>
        ? Partial<AppSettings[K]>
        : AppSettings[K]
}

/** Merges edited fields into the authoritative backend settings. */
export async function update(patch: AppSettingsPatch): Promise<AppSettings> {
    return normalizeDownloadSettings(await invoke('plugin:settings|settings_patch', { patch }))
}

export async function cancel_directory_change(): Promise<void> {
    return await invoke('plugin:settings|cancel_directory_change')
}

export async function getPrivacySettings(): Promise<PrivacySettings> {
    return await invoke('plugin:settings|privacy_get')
}

export async function savePrivacySettings(privacy: PrivacySettings): Promise<PrivacySettings> {
    return await invoke('plugin:settings|privacy_set', { privacy })
}

export async function setTelemetryEnabled(enabled: boolean): Promise<PrivacySettings> {
    return await invoke('plugin:settings|telemetry_set', { enabled })
}

export async function setDiscordRpcEnabled(enabled: boolean): Promise<PrivacySettings> {
    return await invoke('plugin:settings|discord_rpc_set', { enabled })
}

export async function getProxyConfig(): Promise<ProxyConfig> {
    return await invoke('plugin:settings|proxy_get')
}

export async function setProxyConfig(config: ProxyConfig): Promise<void> {
    await invoke('plugin:settings|proxy_set', { config })
}

export async function testProxyConfig(config: ProxyConfig): Promise<ProxyTestResult> {
    return await invoke('plugin:settings|proxy_test', { config })
}

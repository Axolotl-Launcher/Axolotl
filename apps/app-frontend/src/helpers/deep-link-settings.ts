import type { AppSettings } from './settings'

export type PrivilegedSettingChange = { key: string; value: string }

type AbilityKind = 'bool' | 'int' | 'text'

type SettingAbility = {
    kind: AbilityKind
    read: (settings: AppSettings) => string
    apply: (settings: AppSettings, value: string) => void
}

const show = (value: unknown): string =>
    value === null || value === undefined ? '' : String(value)

function coerce(kind: AbilityKind, value: string, clearable: boolean) {
    if (kind === 'bool') return value === 'true'
    if (kind === 'int') return Number(value)
    if (clearable && value === '') return null
    return value
}

function direct(key: string, kind: AbilityKind, clearable = false): SettingAbility {
    return {
        kind,
        read: (settings) => show((settings as Record<string, unknown>)[key]),
        apply: (settings, value) => {
            ;(settings as Record<string, unknown>)[key] = coerce(kind, value, clearable)
        },
    }
}

const bool = (key: string) => direct(key, 'bool')
const int = (key: string) => direct(key, 'int')
const text = (key: string, clearable = false) => direct(key, 'text', clearable)

const memoryMax: SettingAbility = {
    kind: 'int',
    read: (settings) => show(settings.memory?.maximum),
    apply: (settings, value) => {
        settings.memory.maximum = Number(value)
    },
}

const memoryAuto: SettingAbility = {
    kind: 'bool',
    read: (settings) => show(settings.memory?.automatic),
    apply: (settings, value) => {
        settings.memory.automatic = value === 'true'
    },
}

const memoryOptimize: SettingAbility = {
    kind: 'bool',
    read: (settings) => show(settings.memory?.optimize_before_launch),
    apply: (settings, value) => {
        settings.memory.optimize_before_launch = value === 'true'
    },
}

const gameResolutionX: SettingAbility = {
    kind: 'int',
    read: (settings) => show(settings.game_resolution?.[0]),
    apply: (settings, value) => {
        settings.game_resolution = [Number(value), settings.game_resolution[1]]
    },
}

const gameResolutionY: SettingAbility = {
    kind: 'int',
    read: (settings) => show(settings.game_resolution?.[1]),
    apply: (settings, value) => {
        settings.game_resolution = [settings.game_resolution[0], Number(value)]
    },
}

const hook = (field: 'pre_launch' | 'wrapper' | 'post_exit'): SettingAbility => ({
    kind: 'text',
    read: (settings) => show(settings.hooks?.[field]),
    apply: (settings, value) => {
        settings.hooks[field] = value === '' ? null : value
    },
})

// 键表：与后端 handler.rs 白名单一一对应
const abilities: Record<string, SettingAbility> = {
    theme: text('theme'),
    accent_color: text('accent_color'),
    locale: text('locale'),
    default_page: text('default_page'),
    collapsed_navigation: bool('collapsed_navigation'),
    toggle_sidebar: bool('toggle_sidebar'),
    hide_nametag_skins_page: bool('hide_nametag_skins_page'),
    nametag_uses_app_font: bool('nametag_uses_app_font'),
    advanced_rendering: bool('advanced_rendering'),
    native_decorations: bool('native_decorations'),
    auto_hide_downloads_button: bool('auto_hide_downloads_button'),
    close_behavior: text('close_behavior'),
    log_level: text('log_level'),
    home_layout: text('home_layout'),
    sidebar_instance_count: int('sidebar_instance_count'),
    show_files_tab_in_instances: bool('show_files_tab_in_instances'),
    show_worlds_tab_in_instances: bool('show_worlds_tab_in_instances'),
    show_screenshots_tab_in_instances: bool('show_screenshots_tab_in_instances'),
    show_skin_selector_in_sidebar: bool('show_skin_selector_in_sidebar'),
    custom_window_title_enabled: bool('custom_window_title_enabled'),
    default_window_title: text('default_window_title'),
    default_game_language: text('default_game_language'),
    transparent_background: bool('transparent_background'),
    transparent_background_blur: bool('transparent_background_blur'),
    transparent_background_opacity: int('transparent_background_opacity'),
    custom_background_blur: int('custom_background_blur'),
    custom_background_opacity: int('custom_background_opacity'),
    custom_background_component_opacity: int('custom_background_component_opacity'),
    home_widget_background_opacity: int('home_widget_background_opacity'),
    hide_on_process_start: bool('hide_on_process_start'),
    enter_lightweight_mode_on_game_launch: bool('enter_lightweight_mode_on_game_launch'),
    auto_set_java_high_performance_mode: bool('auto_set_java_high_performance_mode'),
    force_fullscreen: bool('force_fullscreen'),
    maximize_window: bool('maximize_window'),
    download_engine: text('download_engine'),
    minecraft_metadata_source: text('minecraft_metadata_source'),
    minecraft_file_source: text('minecraft_file_source'),
    modrinth_source: text('modrinth_source'),
    curseforge_source: text('curseforge_source'),
    mojang_auth_source: text('mojang_auth_source'),
    max_concurrent_downloads: int('max_concurrent_downloads'),
    max_concurrent_writes: int('max_concurrent_writes'),
    auto_concurrent_downloads: bool('auto_concurrent_downloads'),
    bypass_curseforge_download_restrictions: bool('bypass_curseforge_download_restrictions'),
    ignore_ssl_errors: bool('ignore_ssl_errors'),
    memory_max: memoryMax,
    memory_auto: memoryAuto,
    memory_optimize: memoryOptimize,
    game_resolution_x: gameResolutionX,
    game_resolution_y: gameResolutionY,
    hooks_pre_launch: hook('pre_launch'),
    hooks_wrapper: hook('wrapper'),
    hooks_post_exit: hook('post_exit'),
    custom_dir: text('custom_dir', true),
    backup_repository_path: text('backup_repository_path', true),
    custom_background_path: text('custom_background_path', true),
}

export function hasSettingAbility(key: string): boolean {
    return Object.prototype.hasOwnProperty.call(abilities, key)
}

// 全部键合法才应用
export function applySettingChanges(
    settings: AppSettings,
    changes: PrivilegedSettingChange[],
): boolean {
    if (!changes.every((change) => hasSettingAbility(change.key))) return false
    for (const change of changes) abilities[change.key].apply(settings, change.value)
    return true
}

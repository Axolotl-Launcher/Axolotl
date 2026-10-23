import { effectScope, reactive, type Ref, watch } from 'vue'

import { type PreviewState, usePreview, usePreviewDomain } from '../../lib/state/index'
import type { PreviewSettings } from '../../lib/state/settings'
import { injectPreviewWindow } from '../../lib/window'
import { applySystemAccentHue, deriveAccentVariants, parseCustomAccentColor } from './accent'
import { DEFAULT_MONO_FONT_STACK, DEFAULT_UI_FONT_STACK, resolveFontFamily } from './fonts'

/* ---- The machine the preview window stands for ------------------------------
   The window is drawn as the macOS build (traffic lights, no native frame), so
   every platform check of the settings pages resolves the way it does there. */

/** `getOS()` (plugin:utils|get_os). */
export const PREVIEW_OS = 'MacOS' as const
/** `platform()` from @tauri-apps/plugin-os. */
export const PREVIEW_PLATFORM = 'macos' as const
/** `version()` from @tauri-apps/plugin-os. */
export const PREVIEW_OS_VERSION = '15.6.1'
/** Footer name: `platformName()` in Settings.vue. */
export const PREVIEW_OS_NAME = 'macOS'
/** `getVersion()`: the app's package.json version. */
export const PREVIEW_APP_VERSION = '1.10.0'
/** `AxolotlBrandConfig.productName`. */
export const PREVIEW_PRODUCT_NAME = 'Axolotl Launcher'
/** `~/Library/Application Support/<identifier>` on macOS (tauri identifier red.ghs.axolotl). */
export const PREVIEW_APP_DIRECTORY = '/Users/steve/Library/Application Support/red.ghs.axolotl'
/** macOS' default (blue) accent, as NSColor.controlAccentColor reports it in sRGB. */
export const PREVIEW_SYSTEM_ACCENT_SOURCE = '#007aff'

/** The app's DEFAULT_FEATURE_FLAGS (store/theme.ts). */
export const DEFAULT_FEATURE_FLAGS = {
    project_background: false,
    page_path: false,
    worlds_tab: false,
    worlds_in_home: true,
    server_project_qa: false,
    show_version_environment_column: false,
    server_ram_as_bytes_always_on: false,
    always_show_app_controls: false,
    skip_non_essential_warnings: false,
    skip_unknown_pack_warning: false,
    i18n_debug: false,
    show_instance_play_time: true,
    page_transitions: true,
    advanced_filters_collapsed: true,
    auto_install_dependencies: true,
}

export type FeatureFlag = keyof typeof DEFAULT_FEATURE_FLAGS

export type DownloadSourceMode =
    'auto' | 'official_only' | 'mirror_preferred' | 'official_preferred'
export type ProxyMode = 'none' | 'system' | 'custom'
export type UpdateChannel = 'release' | 'beta'
export type LogLevel = 'error' | 'warn' | 'info' | 'debug' | 'trace'

/** Mirrors `ShortcutBinding` in the app's theme store. */
export interface ShortcutBinding {
    device: 'keyboard' | 'mouse'
    code: string
    mod: boolean
    shift: boolean
    alt: boolean
}

/**
 * The fields of the app's `AppSettings` (helpers/settings.ts) that the shared
 * preview settings do not carry, plus the theme-store preferences the app keeps
 * in local storage. Defaults are the settings table's column defaults and
 * `normalizeDownloadSettings()`, for a Release build (whose log level starts at
 * info).
 */
export interface SettingsExtras {
    // Downloads & resources
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
    doh_enabled: boolean
    // Interface
    locale: string
    hide_nametag_skins_page: boolean
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
    minimal_home_instance_id: string | null
    log_level: LogLevel
    home_widget_background_opacity: number
    custom_window_title_enabled: boolean
    default_window_title: string
    terracotta_public_nodes: string[]
    // Privacy
    telemetry_consent_version: number
    allow_external_scheme: boolean
    allow_privileged_scheme: boolean
    // Launch
    auto_set_java_high_performance_mode: boolean
    custom_dir: string | null
    prev_custom_dir: string | null
    // Developer & sync
    sync_features_across_devices: boolean
    show_files_tab_in_instances: boolean
    show_worlds_tab_in_instances: boolean
    show_screenshots_tab_in_instances: boolean
    show_skin_selector_in_sidebar: boolean
    // Updates
    skipped_update: string | null
    pending_update_toast_for_version: string | null
    auto_download_updates: boolean | null
    // Theme-store preferences kept in local storage by the app
    show_scroll_top: boolean
    quick_scroll_enabled: boolean
    nav_shortcuts: Record<string, boolean>
    shortcut_bindings: Record<string, ShortcutBinding>
}

export function createDefaultExtras(): SettingsExtras {
    return {
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
        doh_enabled: true,
        locale: '',
        hide_nametag_skins_page: false,
        toggle_sidebar: false,
        custom_background_path: null,
        custom_background_blur: 12,
        custom_background_opacity: 65,
        custom_background_component_opacity: 100,
        ui_font: null,
        mono_font: null,
        transparent_background: false,
        transparent_background_opacity: 55,
        transparent_background_blur: false,
        sidebar_instance_count: 0,
        minimal_home_instance_id: null,
        log_level: 'info',
        home_widget_background_opacity: 100,
        custom_window_title_enabled: false,
        default_window_title: 'Minecraft',
        terracotta_public_nodes: ['wss://center.node.1tmc.top'],
        telemetry_consent_version: 0,
        allow_external_scheme: true,
        allow_privileged_scheme: false,
        auto_set_java_high_performance_mode: true,
        custom_dir: null,
        prev_custom_dir: null,
        sync_features_across_devices: false,
        show_files_tab_in_instances: true,
        show_worlds_tab_in_instances: true,
        show_screenshots_tab_in_instances: false,
        show_skin_selector_in_sidebar: true,
        skipped_update: null,
        pending_update_toast_for_version: null,
        auto_download_updates: null,
        show_scroll_top: true,
        quick_scroll_enabled: false,
        nav_shortcuts: {},
        shortcut_bindings: {},
    }
}

export type AllSettings = PreviewSettings & SettingsExtras

/** Save indicator states, as `SettingsSaveStatus` takes them. */
export type SaveStatus = 'idle' | 'saving' | 'saved' | 'error'

/** Page-level UI state that survives leaving the page, like the app's route hash. */
export interface SettingsScreenState {
    selectedCategoryId: string
    expandedGroups: Record<string, boolean>
    developerModeCounter: number
    /** The app-update download the footer reports (`injectAppUpdateDownloadProgress`). */
    updateDownload: { progress: number; version: string }
}

/**
 * One object over both stores: keys the shared preview settings own read and
 * write there (so the shell reacts immediately), every other key lives in the
 * area's extras. Both targets are reactive, so reads through it are tracked.
 */
function createMergedSettings(shared: PreviewSettings, extras: SettingsExtras): AllSettings {
    const owner = (key: PropertyKey): Record<PropertyKey, unknown> =>
        (key in shared ? shared : extras) as unknown as Record<PropertyKey, unknown>
    return new Proxy({} as AllSettings, {
        get: (_, key) => {
            // A plain view, not a reactive object of its own: Vue must neither wrap
            // it nor mistake it for one of the stores it forwards to.
            if (key === '__v_skip') return true
            if (typeof key === 'string' && key.startsWith('__v_')) return undefined
            return owner(key)[key]
        },
        set: (_, key, value) => {
            owner(key)[key] = value
            return true
        },
        has: (_, key) => key in shared || key in extras,
        ownKeys: () => [...new Set([...Reflect.ownKeys(shared), ...Reflect.ownKeys(extras)])],
        getOwnPropertyDescriptor: (_, key) =>
            key in shared || key in extras
                ? { enumerable: true, configurable: true, writable: true, value: owner(key)[key] }
                : undefined,
    })
}

/**
 * Window-wide effects the app's theme store applies to the document, applied
 * to the preview window instead: the custom and "follow system" accents
 * (`accent-custom` with the two derived variants) and the interface and
 * monospace fonts. They live as long as the window, not the settings page.
 */
function startWindowEffects(
    preview: PreviewState,
    extras: SettingsExtras,
    root: Ref<HTMLElement | null>,
) {
    const scope = effectScope(true)
    const systemAccent = applySystemAccentHue(PREVIEW_SYSTEM_ACCENT_SOURCE)
    scope.run(() => {
        watch(
            () => [root.value, String(preview.settings.accent_color), preview.theme.resolved.value],
            () => {
                const el = root.value
                if (!el) return
                const accent = String(preview.settings.accent_color)
                const customHex =
                    parseCustomAccentColor(accent) ?? (accent === 'system' ? systemAccent : null)
                if (customHex) {
                    const variants = deriveAccentVariants(customHex)
                    el.style.setProperty('--custom-accent-light', variants.light)
                    el.style.setProperty('--custom-accent-dark', variants.dark)
                    el.classList.add('accent-custom')
                } else {
                    el.style.removeProperty('--custom-accent-light')
                    el.style.removeProperty('--custom-accent-dark')
                    el.classList.remove('accent-custom')
                }
            },
            // After the window's own class binding has been patched.
            { immediate: true, flush: 'post' },
        )
        watch(
            () => [root.value, extras.ui_font, extras.mono_font],
            () => {
                const el = root.value
                if (!el) return
                if (extras.ui_font) {
                    el.style.setProperty(
                        '--font-standard',
                        resolveFontFamily(extras.ui_font, DEFAULT_UI_FONT_STACK),
                    )
                } else {
                    el.style.removeProperty('--font-standard')
                }
                if (extras.mono_font) {
                    el.style.setProperty(
                        '--mono-font',
                        resolveFontFamily(extras.mono_font, DEFAULT_MONO_FONT_STACK),
                    )
                } else {
                    el.style.removeProperty('--mono-font')
                }
            },
            { immediate: true, flush: 'post' },
        )
    })
    return () => scope.stop()
}

interface SettingsDomain {
    extras: SettingsExtras
    settings: AllSettings
    screen: SettingsScreenState
}

/**
 * The settings page's state: created with the first category that opens and
 * kept for the life of the window, so leaving the page and coming back shows
 * what the visitor changed, as the app's stores and database do.
 */
export function useSettingsPage() {
    const preview = usePreview()
    const { root } = injectPreviewWindow()
    const domain = usePreviewDomain<SettingsDomain>('settings', (state) => {
        const extras = reactive(createDefaultExtras()) as SettingsExtras
        const screen = reactive<SettingsScreenState>({
            selectedCategoryId: 'interface',
            expandedGroups: {
                launcher: true,
                game: true,
                'data-privacy': true,
                support: true,
                developer: false,
            },
            developerModeCounter: 0,
            updateDownload: { progress: 0, version: PREVIEW_APP_VERSION },
        })
        const stopEffects = startWindowEffects(state, extras, root)
        return {
            value: { extras, screen, settings: createMergedSettings(state.settings, extras) },
            dispose: stopEffects,
        }
    })

    /** Resolves after `ms`, on the window's clock, standing in for an `invoke` round-trip. */
    function simulate(ms = 220): Promise<void> {
        return new Promise((resolve) => {
            preview.clock.after(ms, resolve)
        })
    }

    /** `themeStore.getFeatureFlag()`: the stored value, or the app default. */
    function getFeatureFlag(flag: FeatureFlag): boolean {
        return preview.settings.feature_flags[flag] ?? DEFAULT_FEATURE_FLAGS[flag]
    }

    function setFeatureFlag(flag: FeatureFlag, value: boolean) {
        preview.settings.feature_flags[flag] = value
    }

    return {
        preview,
        settings: domain.settings,
        shared: preview.settings,
        extras: domain.extras,
        screen: domain.screen,
        simulate,
        getFeatureFlag,
        setFeatureFlag,
    }
}

export const THEME_OPTIONS = ['dark', 'light', 'oled', 'system'] as const

/** The system accent the app derives on this machine (`setSystemAccentColor`). */
export const PREVIEW_SYSTEM_ACCENT = applySystemAccentHue(PREVIEW_SYSTEM_ACCENT_SOURCE)

/**
 * The app's theme store (store/theme.ts) over the preview's state, with the
 * same field and action names, so ported templates read unchanged. Fields
 * read and write the settings they mirror; the actions that paint the document
 * are no-ops because the window effects above and the shell apply the values.
 */
export function useThemeStore() {
    const { settings, preview, getFeatureFlag } = useSettingsPage()
    const navShortcut = (id: string) => ({
        get: () => !!settings.nav_shortcuts[id],
        set: (value: boolean) => {
            settings.nav_shortcuts[id] = value
        },
    })
    const fields: Record<string, { get: () => unknown; set?: (value: never) => void }> = {
        selectedTheme: {
            get: () => settings.theme,
            set: (value: AllSettings['theme']) => (settings.theme = value),
        },
        selectedAccentColor: {
            get: () => settings.accent_color,
            set: (value: AllSettings['accent_color']) => (settings.accent_color = value),
        },
        systemAccentColor: { get: () => PREVIEW_SYSTEM_ACCENT },
        systemAccentSupported: { get: () => true },
        advancedRendering: {
            get: () => settings.advanced_rendering,
            set: (value: boolean) => (settings.advanced_rendering = value),
        },
        hideNametagSkinsPage: {
            get: () => settings.hide_nametag_skins_page,
            set: (value: boolean) => (settings.hide_nametag_skins_page = value),
        },
        toggleSidebar: {
            get: () => settings.toggle_sidebar,
            set: (value: boolean) => (settings.toggle_sidebar = value),
        },
        customBackgroundPath: {
            get: () => settings.custom_background_path,
            set: (value: string | null) => (settings.custom_background_path = value),
        },
        customBackgroundBlur: {
            get: () => settings.custom_background_blur,
            set: (value: number) => (settings.custom_background_blur = value),
        },
        customBackgroundOpacity: {
            get: () => settings.custom_background_opacity,
            set: (value: number) => (settings.custom_background_opacity = value),
        },
        customBackgroundComponentOpacity: {
            get: () => settings.custom_background_component_opacity,
            set: (value: number) => (settings.custom_background_component_opacity = value),
        },
        uiFont: {
            get: () => settings.ui_font,
            set: (value: string | null) => (settings.ui_font = value),
        },
        monoFont: {
            get: () => settings.mono_font,
            set: (value: string | null) => (settings.mono_font = value),
        },
        transparentBackground: {
            get: () => settings.transparent_background,
            set: (value: boolean) => (settings.transparent_background = value),
        },
        transparentBackgroundOpacity: {
            get: () => settings.transparent_background_opacity,
            set: (value: number) => (settings.transparent_background_opacity = value),
        },
        transparentBackgroundBlur: {
            get: () => settings.transparent_background_blur,
            set: (value: boolean) => (settings.transparent_background_blur = value),
        },
        homeWidgetBackgroundOpacity: {
            get: () => settings.home_widget_background_opacity,
            set: (value: number) => (settings.home_widget_background_opacity = value),
        },
        hiddenNavItems: {
            get: () => settings.hidden_nav_items,
            set: (value: string[]) => (settings.hidden_nav_items = value),
        },
        sidebarInstanceCount: {
            get: () => settings.sidebar_instance_count,
            set: (value: number) => (settings.sidebar_instance_count = value),
        },
        autoHideDownloadsButton: {
            get: () => settings.auto_hide_downloads_button,
            set: (value: boolean) => (settings.auto_hide_downloads_button = value),
        },
        homeLayout: {
            get: () => settings.home_layout,
            set: (value: AllSettings['home_layout']) => (settings.home_layout = value),
        },
        minimalHomeInstanceId: {
            get: () => settings.minimal_home_instance_id,
            set: (value: string | null) => (settings.minimal_home_instance_id = value),
        },
        closeBehavior: {
            get: () => settings.close_behavior,
            set: (value: AllSettings['close_behavior']) => (settings.close_behavior = value),
        },
        showScrollTop: {
            get: () => settings.show_scroll_top,
            set: (value: boolean) => (settings.show_scroll_top = value),
        },
        quickScrollEnabled: {
            get: () => settings.quick_scroll_enabled,
            set: (value: boolean) => (settings.quick_scroll_enabled = value),
        },
        shortcutNavHome: navShortcut('home'),
        shortcutNavWorlds: navShortcut('worlds'),
        shortcutNavDiscover: navShortcut('discover'),
        shortcutNavSkins: navShortcut('skins'),
        shortcutNavMultiplayer: navShortcut('multiplayer'),
        shortcutNavLibrary: navShortcut('library'),
        shortcutNavLab: navShortcut('lab'),
        shortcutNavDownloads: navShortcut('downloads'),
        shortcutNavCreate: navShortcut('create'),
        shortcutNavSettings: navShortcut('settings'),
        shortcutBindings: {
            get: () => settings.shortcut_bindings,
            set: (value: Record<string, ShortcutBinding>) => (settings.shortcut_bindings = value),
        },
        devMode: {
            get: () => settings.developer_mode,
            set: (value: boolean) => (settings.developer_mode = value),
        },
        featureFlags: { get: () => preview.settings.feature_flags },
    }

    const store = {
        setThemeState(theme: (typeof THEME_OPTIONS)[number]) {
            if (THEME_OPTIONS.includes(theme)) settings.theme = theme
        },
        setAccentColor(value: string) {
            ;(settings as { accent_color: string }).accent_color = value
        },
        isNavItemHidden(id: string) {
            return settings.hidden_nav_items.includes(id)
        },
        getFeatureFlag,
        getThemeOptions() {
            return THEME_OPTIONS
        },
        // Document painting is done by the window effects and the shell.
        setThemeClass() {},
        setTransparentBackgroundClass() {},
        setHomeWidgetBackgroundOpacity() {},
        setCustomBackgroundClass() {},
        setCustomBackgroundOpacity() {},
        setUiFont() {},
        setMonoFont() {},
    } as Record<string, unknown>
    for (const [name, field] of Object.entries(fields)) {
        Object.defineProperty(store, name, {
            enumerable: true,
            get: field.get,
            set: field.set ?? (() => {}),
        })
    }
    return store as ThemeStoreFacade
}

/** The fields and actions of `useTheming()` the settings pages use. */
export interface ThemeStoreFacade {
    selectedTheme: AllSettings['theme']
    selectedAccentColor: string
    readonly systemAccentColor: string
    readonly systemAccentSupported: boolean
    advancedRendering: boolean
    hideNametagSkinsPage: boolean
    toggleSidebar: boolean
    customBackgroundPath: string | null
    customBackgroundBlur: number
    customBackgroundOpacity: number
    customBackgroundComponentOpacity: number
    uiFont: string | null
    monoFont: string | null
    transparentBackground: boolean
    transparentBackgroundOpacity: number
    transparentBackgroundBlur: boolean
    homeWidgetBackgroundOpacity: number
    hiddenNavItems: string[]
    sidebarInstanceCount: number
    autoHideDownloadsButton: boolean
    homeLayout: AllSettings['home_layout']
    minimalHomeInstanceId: string | null
    closeBehavior: AllSettings['close_behavior']
    showScrollTop: boolean
    quickScrollEnabled: boolean
    shortcutNavHome: boolean
    shortcutNavWorlds: boolean
    shortcutNavDiscover: boolean
    shortcutNavSkins: boolean
    shortcutNavMultiplayer: boolean
    shortcutNavLibrary: boolean
    shortcutNavLab: boolean
    shortcutNavDownloads: boolean
    shortcutNavCreate: boolean
    shortcutNavSettings: boolean
    shortcutBindings: Record<string, ShortcutBinding>
    devMode: boolean
    readonly featureFlags: Record<string, boolean>
    setThemeState: (theme: (typeof THEME_OPTIONS)[number]) => void
    setAccentColor: (value: string) => void
    isNavItemHidden: (id: string) => boolean
    getFeatureFlag: (flag: FeatureFlag) => boolean
    getThemeOptions: () => typeof THEME_OPTIONS
    setThemeClass: () => void
    setTransparentBackgroundClass: () => void
    setHomeWidgetBackgroundOpacity: () => void
    setCustomBackgroundClass: () => void
    setCustomBackgroundOpacity: () => void
    setUiFont: () => void
    setMonoFont: () => void
}

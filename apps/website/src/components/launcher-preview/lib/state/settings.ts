import { computed, reactive, ref } from 'vue'

export type ThemeSetting = 'dark' | 'light' | 'oled' | 'system'
export type ResolvedTheme = 'dark' | 'light' | 'oled'
export type AccentColor = 'pink' | 'orange' | 'green' | 'blue' | 'purple' | 'system'

/**
 * The launcher's app settings that the preview reproduces, named after the
 * desktop app's `AppSettings` (apps/app-frontend/src/helpers/types.d.ts) with
 * the defaults of its settings table (packages/app-lib/migrations).
 */
export interface PreviewSettings {
    theme: ThemeSetting
    accent_color: AccentColor
    default_page: 'Home' | 'DiscoverContent' | 'Library'
    collapsed_navigation: boolean
    advanced_rendering: boolean
    native_decorations: boolean
    auto_hide_downloads_button: boolean
    worlds_in_home: boolean
    home_layout: 'standard' | 'minimal'
    close_behavior: 'ask' | 'close' | 'lightweight'
    hidden_nav_items: string[]
    /** The theme store's `sidebarInstanceCount`: recent instances under the nav rail, 0 = all. */
    sidebar_instance_count: number
    telemetry: boolean
    discord_rpc: boolean
    developer_mode: boolean
    max_concurrent_downloads: number
    max_concurrent_writes: number
    hide_on_process_start: boolean
    enter_lightweight_mode_on_game_launch: boolean
    force_fullscreen: boolean
    maximize_window: boolean
    game_resolution: [number, number]
    memory: { maximum: number; automatic: boolean; optimize_before_launch: boolean }
    extra_launch_args: string[]
    custom_env_vars: [string, string][]
    hooks: { pre_launch: string | null; wrapper: string | null; post_exit: string | null }
    feature_flags: Record<string, boolean>
    sidebar_expanded: boolean
}

export function createDefaultSettings(theme: ThemeSetting): PreviewSettings {
    return {
        theme,
        accent_color: 'pink',
        default_page: 'Home',
        collapsed_navigation: false,
        advanced_rendering: true,
        native_decorations: false,
        auto_hide_downloads_button: false,
        worlds_in_home: true,
        home_layout: 'standard',
        close_behavior: 'ask',
        hidden_nav_items: [],
        sidebar_instance_count: 0,
        telemetry: true,
        discord_rpc: true,
        developer_mode: false,
        max_concurrent_downloads: 10,
        max_concurrent_writes: 10,
        hide_on_process_start: false,
        enter_lightweight_mode_on_game_launch: false,
        force_fullscreen: false,
        maximize_window: false,
        game_resolution: [854, 480],
        memory: { maximum: 2048, automatic: true, optimize_before_launch: false },
        extra_launch_args: [],
        custom_env_vars: [],
        hooks: { pre_launch: null, wrapper: null, post_exit: null },
        feature_flags: { page_transitions: true, worlds_tab: false, page_path: false },
        sidebar_expanded: true,
    }
}

/** Reads the theme the website is currently painted in. */
export function readSiteTheme(): ResolvedTheme {
    if (typeof document === 'undefined') return 'dark'
    const classes = document.documentElement.classList
    if (classes.contains('light-mode')) return 'light'
    if (classes.contains('oled-mode')) return 'oled'
    return 'dark'
}

export function createSettingsState() {
    const settings = reactive(createDefaultSettings(readSiteTheme()))
    /** The page's own theme stands in for the operating system's appearance. */
    const systemTheme = ref<ResolvedTheme>(readSiteTheme())

    let observer: MutationObserver | null = null
    if (typeof MutationObserver !== 'undefined') {
        let previousSiteTheme = systemTheme.value
        observer = new MutationObserver(() => {
            const siteTheme = readSiteTheme()
            if (siteTheme === previousSiteTheme) return
            // Until the visitor picks a theme inside the window, the window keeps
            // following the website when the website's theme is switched.
            if (settings.theme === previousSiteTheme) settings.theme = siteTheme
            previousSiteTheme = siteTheme
            systemTheme.value = siteTheme
        })
        observer.observe(document.documentElement, {
            attributes: true,
            attributeFilter: ['class'],
        })
    }

    const resolvedTheme = computed<ResolvedTheme>(() =>
        settings.theme === 'system' ? systemTheme.value : settings.theme,
    )

    const theme = {
        resolved: resolvedTheme,
        className: computed(() => `${resolvedTheme.value}-mode`),
        accentClassName: computed(
            () => `accent-${settings.accent_color === 'system' ? 'pink' : settings.accent_color}`,
        ),
    }

    function dispose() {
        observer?.disconnect()
        observer = null
    }

    return { settings, theme, dispose }
}

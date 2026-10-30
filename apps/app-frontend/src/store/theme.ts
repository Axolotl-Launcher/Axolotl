import { defineStore } from 'pinia'

import {
    DEFAULT_MONO_FONT_STACK,
    DEFAULT_UI_FONT_STACK,
    resolveFontFamily,
} from '../helpers/font-family.ts'

/**
 * Mirrors `KeyBinding` from `@modrinth/ui`. Declared here so this module stays
 * loadable without the UI package, which the settings tests rely on.
 */
export interface ShortcutBinding {
    device: 'keyboard' | 'mouse'
    code: string
    mod: boolean
    shift: boolean
    alt: boolean
}

let systemThemeMq: MediaQueryList | null = null

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

export const THEME_OPTIONS = ['dark', 'light', 'oled', 'system'] as const
export const ACCENT_COLOR_OPTIONS = ['pink', 'orange', 'green', 'blue', 'purple'] as const
export const DEFAULT_CUSTOM_ACCENT_COLOR = '#db2777'

export type FeatureFlag = keyof typeof DEFAULT_FEATURE_FLAGS
export type FeatureFlags = Record<FeatureFlag, boolean>
export type ColorTheme = (typeof THEME_OPTIONS)[number]
export type AccentColor = (typeof ACCENT_COLOR_OPTIONS)[number]
export type CustomAccentColor = `custom:#${string}`
export type AccentColorSetting = AccentColor | 'system' | CustomAccentColor
export type HomeLayout = 'standard' | 'minimal'
export type CloseBehavior = 'ask' | 'close' | 'lightweight'

/**
 * Extracts the `#rrggbb` hex from a `custom:#rrggbb` accent setting,
 * or returns null if the value is not a valid custom accent.
 */
export function parseCustomAccentColor(value: string): string | null {
    const match = /^custom:(#[0-9a-fA-F]{6})$/.exec(value)
    return match ? match[1].toLowerCase() : null
}

export function hexToHsl(hex: string): { h: number; s: number; l: number } {
    const n = parseInt(hex.slice(1), 16)
    const r = ((n >> 16) & 255) / 255
    const g = ((n >> 8) & 255) / 255
    const b = (n & 255) / 255
    const max = Math.max(r, g, b)
    const min = Math.min(r, g, b)
    const l = (max + min) / 2
    const d = max - min
    const s = d === 0 ? 0 : d / (1 - Math.abs(2 * l - 1))
    let h = 0
    if (d !== 0) {
        if (max === r) h = ((g - b) / d) % 6
        else if (max === g) h = (b - r) / d + 2
        else h = (r - g) / d + 4
        h *= 60
        if (h < 0) h += 360
    }
    return { h, s: s * 100, l: l * 100 }
}

export function hslToHex(h: number, s: number, l: number): string {
    const sn = s / 100
    const ln = l / 100
    const c = (1 - Math.abs(2 * ln - 1)) * sn
    const hp = (((h % 360) + 360) % 360) / 60
    const x = c * (1 - Math.abs((hp % 2) - 1))
    let [r, g, b] = [0, 0, 0]
    if (hp < 1) [r, g, b] = [c, x, 0]
    else if (hp < 2) [r, g, b] = [x, c, 0]
    else if (hp < 3) [r, g, b] = [0, c, x]
    else if (hp < 4) [r, g, b] = [0, x, c]
    else if (hp < 5) [r, g, b] = [x, 0, c]
    else [r, g, b] = [c, 0, x]
    const m = ln - c / 2
    const toHex = (channel: number) =>
        Math.round((channel + m) * 255)
            .toString(16)
            .padStart(2, '0')
    return `#${toHex(r)}${toHex(g)}${toHex(b)}`
}

export function applySystemAccentHue(systemHex: string): string {
    const { h } = hexToHsl(systemHex)
    const { s, l } = hexToHsl(DEFAULT_CUSTOM_ACCENT_COLOR)
    return hslToHex(h, s, l)
}

const clamp = (value: number, min: number, max: number) => Math.min(Math.max(value, min), max)

/**
 * Derives per-theme accent variants from a single base color, mirroring the
 * relationship between the preset accents' light and dark values: hue is kept,
 * the light variant is clamped dark enough for light surfaces, and the dark
 * variant is lifted ~18 lightness points with a saturation boost.
 */
export function deriveAccentVariants(hex: string): { light: string; dark: string } {
    const { h, s, l } = hexToHsl(hex)
    const lightL = clamp(l, 34, 57)
    return {
        light: hslToHex(h, s, lightL),
        dark: hslToHex(h, Math.min(s * 1.15, 100), Math.min(lightL + 18, 80)),
    }
}

export type ThemeStore = {
    selectedTheme: ColorTheme
    selectedAccentColor: AccentColorSetting
    systemAccentColor: string | null
    systemAccentSupported: boolean | null
    advancedRendering: boolean
    hideNametagSkinsPage: boolean
    /** Whether the skins-page nametag uses the launcher's UI font instead of the pixel font. */
    nametagUsesAppFont: boolean
    toggleSidebar: boolean
    customBackgroundPath: string | null
    customBackgroundBlur: number
    customBackgroundOpacity: number
    customBackgroundComponentOpacity: number
    /** Installed family replacing the launcher's UI font; null follows the default. */
    uiFont: string | null
    /** Installed family replacing the monospace stack; null follows the default. */
    monoFont: string | null
    transparentBackground: boolean
    transparentBackgroundOpacity: number
    transparentBackgroundBlur: boolean
    homeWidgetBackgroundOpacity: number
    hiddenNavItems: string[]
    sidebarInstanceCount: number
    autoHideDownloadsButton: boolean
    homeLayout: HomeLayout
    minimalHomeInstanceId: string | null
    closeBehavior: CloseBehavior

    /** Whether the floating "back to top" button is shown on scrollable pages. */
    showScrollTop: boolean
    /** Whether quick scrolling shortcuts (Home/End/PageUp/PageDown) are active. */
    quickScrollEnabled: boolean
    /** Per-nav-item quick jump shortcuts (Ctrl/Cmd + number), off by default. */
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

    /**
     * The combination every shortcut answers to, keyed by action id. Kept here so
     * pages re-render when one is recorded; the value itself lives in local
     * storage, next to the enable flags.
     */
    shortcutBindings: Record<string, ShortcutBinding>

    devMode: boolean
    featureFlags: FeatureFlags
}

export const DEFAULT_THEME_STORE: ThemeStore = {
    selectedTheme: 'dark',
    selectedAccentColor: 'pink',
    systemAccentColor: null,
    systemAccentSupported: null,
    advancedRendering: true,
    hideNametagSkinsPage: false,
    nametagUsesAppFont: false,
    toggleSidebar: false,
    customBackgroundPath: null,
    customBackgroundBlur: 12,
    customBackgroundOpacity: 65,
    customBackgroundComponentOpacity: 100,
    uiFont: null,
    monoFont: null,
    transparentBackground: false,
    transparentBackgroundOpacity: 55,
    transparentBackgroundBlur: false,
    homeWidgetBackgroundOpacity: 100,
    hiddenNavItems: [],
    sidebarInstanceCount: 0,
    autoHideDownloadsButton: false,
    homeLayout: 'standard',
    minimalHomeInstanceId: null,
    closeBehavior: 'ask',

    showScrollTop: true,
    quickScrollEnabled: false,

    shortcutNavHome: false,
    shortcutNavWorlds: false,
    shortcutNavDiscover: false,
    shortcutNavSkins: false,
    shortcutNavMultiplayer: false,
    shortcutNavLibrary: false,
    shortcutNavLab: false,
    shortcutNavDownloads: false,
    shortcutNavCreate: false,
    shortcutNavSettings: false,

    shortcutBindings: {},

    devMode: false,
    featureFlags: DEFAULT_FEATURE_FLAGS,
}

export const useTheming = defineStore('themeStore', {
    state: () => DEFAULT_THEME_STORE,
    actions: {
        setThemeState(newTheme: ColorTheme) {
            if (THEME_OPTIONS.includes(newTheme)) {
                this.selectedTheme = newTheme
            } else {
                console.warn('Selected theme is not present. Check themeOptions.')
            }

            this.setThemeClass()
        },
        setAccentColor(newAccentColor: AccentColorSetting) {
            if (
                newAccentColor === 'system' ||
                parseCustomAccentColor(newAccentColor) !== null ||
                ACCENT_COLOR_OPTIONS.includes(newAccentColor as AccentColor)
            ) {
                this.selectedAccentColor = newAccentColor
            } else {
                console.warn('Selected accent color is not available.')
            }

            const html = document.documentElement
            for (const accentColor of ACCENT_COLOR_OPTIONS) {
                html.classList.remove(`accent-${accentColor}`)
            }
            html.classList.remove('accent-custom')

            const customHex =
                parseCustomAccentColor(this.selectedAccentColor) ??
                (this.selectedAccentColor === 'system' ? this.systemAccentColor : null)
            if (customHex) {
                const variants = deriveAccentVariants(customHex)
                html.style.setProperty('--custom-accent-light', variants.light)
                html.style.setProperty('--custom-accent-dark', variants.dark)
                html.classList.add('accent-custom')
            } else if (this.selectedAccentColor === 'system') {
                html.style.removeProperty('--custom-accent-light')
                html.style.removeProperty('--custom-accent-dark')
                html.classList.add('accent-pink')
            } else {
                html.style.removeProperty('--custom-accent-light')
                html.style.removeProperty('--custom-accent-dark')
                html.classList.add(`accent-${this.selectedAccentColor}`)
            }
        },
        setSystemAccentColor(systemHex: string) {
            if (!/^#[0-9a-fA-F]{6}$/.test(systemHex)) {
                this.setSystemAccentUnavailable()
                return
            }

            this.systemAccentColor = applySystemAccentHue(systemHex)
            this.systemAccentSupported = true
            if (this.selectedAccentColor === 'system') this.setAccentColor('system')
        },
        setSystemAccentUnavailable() {
            this.systemAccentColor = null
            this.systemAccentSupported = false
            if (this.selectedAccentColor === 'system') this.setAccentColor('system')
        },
        setThemeClass() {
            const html = document.getElementsByTagName('html')[0]
            for (const theme of THEME_OPTIONS) {
                html.classList.remove(`${theme}-mode`)
            }

            systemThemeMq?.removeEventListener('change', this.setThemeClass)
            systemThemeMq = null

            let theme = this.selectedTheme
            if (this.selectedTheme === 'system') {
                systemThemeMq = window.matchMedia('(prefers-color-scheme: dark)')
                systemThemeMq.addEventListener('change', this.setThemeClass)
                theme = systemThemeMq.matches ? 'dark' : 'light'
            }

            html.classList.add(`${theme}-mode`)
        },
        /**
         * Applies the transparent window mode to the root element. The palette
         * lives on `body` in `global.scss`; this only supplies the toggle and the
         * alpha every translucent surface is derived from.
         */
        setTransparentBackgroundClass() {
            const html = document.documentElement
            html.classList.toggle('transparent-window', this.transparentBackground)
            html.style.setProperty(
                '--transparent-window-alpha',
                `${Math.min(Math.max(this.transparentBackgroundOpacity, 0), 100)}%`,
            )
        },
        setHomeWidgetBackgroundOpacity() {
            const html = document.documentElement
            html.style.setProperty(
                '--home-widget-bg-opacity',
                `${Math.min(Math.max(this.homeWidgetBackgroundOpacity, 0), 100)}%`,
            )
        },
        /**
         * The translucent palette for the custom-background mode lives on `body`
         * in `global.scss`; this supplies the toggle and the alpha it derives
         * every surface from. The image and the transparent window are mutually
         * exclusive, so the class follows the same condition the shell uses.
         */
        setCustomBackgroundClass() {
            const html = document.documentElement
            html.classList.toggle(
                'has-custom-background',
                !!this.customBackgroundPath && !this.transparentBackground,
            )
        },
        /**
         * Publishes the two opacity settings for the custom-background palette in
         * `global.scss`, as the alpha each layer should paint with.
         *
         * "Background visibility" is how much of the image comes through the page
         * layer. "Component opacity" is how much comes through the components
         * stacked on top of it, and it *compounds* with the page rather than
         * replacing it: the share of the image that reaches the eye through a
         * component is `visibility x component`. At 100% a component is fully
         * opaque; at 0% it is exactly as transparent as the page beneath it.
         */
        setCustomBackgroundOpacity() {
            const html = document.documentElement
            const visibility = Math.min(Math.max(this.customBackgroundOpacity, 0), 100) / 100
            const component =
                Math.min(Math.max(this.customBackgroundComponentOpacity, 0), 100) / 100

            // The `-setting` suffix distinguishes the raw value from the
            // `--custom-bg-*-alpha` the palette derives, which declares the
            // fallback that keeps the palette resolvable before this runs.
            html.style.setProperty('--custom-bg-page-alpha-setting', `${(1 - visibility) * 100}%`)
            html.style.setProperty(
                '--custom-bg-component-alpha-setting',
                `${(1 - visibility * (1 - component)) * 100}%`,
            )
        },
        /**
         * The font tokens are declared on `body` in the shared defaults, so the
         * overrides live on the body element; an `<html>` level value would be
         * shadowed by body's own declaration for the whole app.
         */
        setUiFont() {
            const body = document.body
            if (this.uiFont) {
                body.style.setProperty(
                    '--font-standard',
                    resolveFontFamily(this.uiFont, DEFAULT_UI_FONT_STACK),
                )
            } else {
                body.style.removeProperty('--font-standard')
            }
        },
        setMonoFont() {
            const body = document.body
            if (this.monoFont) {
                body.style.setProperty(
                    '--mono-font',
                    resolveFontFamily(this.monoFont, DEFAULT_MONO_FONT_STACK),
                )
            } else {
                body.style.removeProperty('--mono-font')
            }
        },
        isNavItemHidden(id: string) {
            return this.hiddenNavItems.includes(id)
        },
        getFeatureFlag(key: FeatureFlag) {
            return this.featureFlags[key] ?? DEFAULT_FEATURE_FLAGS[key]
        },
        getThemeOptions() {
            return THEME_OPTIONS
        },
    },
})

import { DEFAULT_UI_FONT_STACK } from '@/helpers/font-family'
import { getThemePack, themePackAssetUrl, type ThemePackManifest } from '@/helpers/theme-packs'
import { applyCustomThemeColors, type ColorTheme, type CustomThemeColors } from '@/store/theme'

export type ThemeRuntimeSource = 'builtin' | 'custom' | `pack:${string}` | null | undefined

export type ThemeColorBucket = 'light' | 'dark'

/** The persisted appearance settings the runtime needs to paint the interface. */
export interface ThemeRuntimeSelection {
    themeSource: ThemeRuntimeSource
    customThemeColors?: CustomThemeColors | null
    /** The user's interface font. A pack font is only used when this is empty. */
    uiFont?: string | null
}

export interface ThemeRuntimeResult {
    /** Set when the selected pack is not installed; the built-in theme was used instead. */
    missingPackId: string | null
}

const PACK_SOURCE_PREFIX = 'pack:'
const PACK_FONT_FAMILY = 'Theme Pack UI'
const PACK_BACKGROUND_PROPERTIES = [
    'background-image',
    'background-size',
    'background-position',
    'background-repeat',
] as const

let cachedSelection: ThemeRuntimeSelection | null = null
let cachedTheme: ColorTheme = 'dark'
let cachedManifest: ThemePackManifest | null = null
let requestToken = 0

const appliedTokens = new Set<string>()
const appliedLinks: HTMLLinkElement[] = []
let appliedFontStyle: HTMLStyleElement | null = null
let backgroundApplied = false
let packFontApplied = false
let systemThemeQuery: MediaQueryList | null = null

function canUseDom(): boolean {
    return typeof document !== 'undefined'
}

export function parseThemeSource(source: ThemeRuntimeSource): {
    kind: 'builtin' | 'custom' | 'pack'
    packId: string | null
} {
    if (typeof source === 'string' && source.startsWith(PACK_SOURCE_PREFIX)) {
        const packId = source.slice(PACK_SOURCE_PREFIX.length)
        if (packId) return { kind: 'pack', packId }
    }
    if (source === 'custom') return { kind: 'custom', packId: null }
    return { kind: 'builtin', packId: null }
}

/**
 * Picks the manifest token bucket for a color theme. `system` follows the same
 * media query the theme store uses; dark and OLED share the dark bucket.
 */
export function resolveColorBucket(theme: ColorTheme, prefersDark: boolean): ThemeColorBucket {
    if (theme === 'light') return 'light'
    if (theme === 'system') return prefersDark ? 'dark' : 'light'
    return 'dark'
}

/** Returns the manifest tokens that are safe to publish as CSS custom properties. */
export function selectPackTokens(
    manifest: Pick<ThemePackManifest, 'tokens'>,
    bucket: ThemeColorBucket,
): [string, string][] {
    const tokens = manifest.tokens?.[bucket] ?? {}
    const forbidden = [
        '\\',
        'url(',
        'image-set(',
        'src(',
        'var(',
        'expression(',
        '@import',
        ';',
        '{',
        '}',
    ]
    return Object.entries(tokens).filter(
        ([name, value]) =>
            /^--[a-z0-9-]+$/.test(name) &&
            value.trim().length > 0 &&
            value.length <= 256 &&
            !forbidden.some((token) => value.toLowerCase().includes(token)),
    )
}

function clearAppliedTokens() {
    if (canUseDom() && appliedTokens.size > 0) {
        const style = document.documentElement.style
        for (const name of appliedTokens) style.removeProperty(name)
    }
    appliedTokens.clear()
}

function clearAppliedStyles() {
    for (const link of appliedLinks) link.remove()
    appliedLinks.length = 0
    appliedFontStyle?.remove()
    appliedFontStyle = null
    // Only drop the font variable when it still holds our pack stack; the user's
    // own interface font (set by the theme store) must survive.
    if (packFontApplied && canUseDom()) {
        const style = document.body.style
        if (style.getPropertyValue('--font-standard').includes(PACK_FONT_FAMILY)) {
            style.removeProperty('--font-standard')
        }
    }
    packFontApplied = false
}

function clearAppliedBackground() {
    if (canUseDom() && backgroundApplied) {
        const style = document.body.style
        for (const property of PACK_BACKGROUND_PROPERTIES) style.removeProperty(property)
    }
    backgroundApplied = false
}

function clearPackApplication() {
    clearAppliedTokens()
    clearAppliedStyles()
    clearAppliedBackground()
}

function applyPackBackground(manifest: ThemePackManifest) {
    if (!manifest.background) return
    const style = document.body.style
    style.setProperty(
        'background-image',
        `url('${themePackAssetUrl(manifest.id, manifest.background)}')`,
    )
    style.setProperty('background-size', 'cover')
    style.setProperty('background-position', 'center')
    style.setProperty('background-repeat', 'no-repeat')
    backgroundApplied = true
}

function applyPackFont(manifest: ThemePackManifest, uiFont: string | null | undefined) {
    // An explicit interface font always wins; the pack font is the fallback.
    if (uiFont || !manifest.fonts?.ui) return
    const style = document.createElement('style')
    style.dataset.themePackFont = manifest.id
    style.textContent = `@font-face { font-family: '${PACK_FONT_FAMILY}'; src: url('${themePackAssetUrl(
        manifest.id,
        manifest.fonts.ui,
    )}'); font-display: swap; }`
    document.head.appendChild(style)
    appliedFontStyle = style
    document.body.style.setProperty(
        '--font-standard',
        `'${PACK_FONT_FAMILY}', ${DEFAULT_UI_FONT_STACK}`,
    )
    packFontApplied = true
}

function applyPackManifest(
    theme: ColorTheme,
    manifest: ThemePackManifest,
    selection: ThemeRuntimeSelection,
) {
    clearPackApplication()
    if (!canUseDom()) return

    // Pack tokens replace any custom palette for this run.
    applyCustomThemeColors(theme, null)

    const bucket = resolveColorBucket(
        theme,
        document.documentElement.classList.contains('dark-mode'),
    )
    const style = document.documentElement.style
    for (const [name, value] of selectPackTokens(manifest, bucket)) {
        style.setProperty(name, value)
        appliedTokens.add(name)
    }

    for (const stylesheet of manifest.styles ?? []) {
        if (!stylesheet) continue
        const link = document.createElement('link')
        link.rel = 'stylesheet'
        link.href = themePackAssetUrl(manifest.id, stylesheet)
        link.dataset.themePack = manifest.id
        document.head.appendChild(link)
        appliedLinks.push(link)
    }

    applyPackBackground(manifest)
    applyPackFont(manifest, selection.uiFont)
}

function applySourceTheme(
    theme: ColorTheme,
    selection: ThemeRuntimeSelection,
    kind: 'builtin' | 'custom',
) {
    clearPackApplication()
    if (!canUseDom()) return
    applyCustomThemeColors(theme, kind === 'custom' ? selection.customThemeColors : null)
}

function handleSystemThemeChange() {
    if (cachedTheme !== 'system' || !cachedSelection) return
    void reapplyThemeRuntime('system')
}

function ensureSystemThemeListener() {
    if (systemThemeQuery) return
    if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return
    systemThemeQuery = window.matchMedia('(prefers-color-scheme: dark)')
    systemThemeQuery.addEventListener('change', handleSystemThemeChange)
}

/**
 * Applies the persisted theme source to the document. Pack styles and tokens
 * are tracked so switching away removes every injected element and variable.
 */
export async function applyThemeRuntime(
    theme: ColorTheme,
    selection: ThemeRuntimeSelection,
): Promise<ThemeRuntimeResult> {
    cachedSelection = {
        themeSource: selection.themeSource,
        customThemeColors: selection.customThemeColors ?? null,
        uiFont: selection.uiFont ?? null,
    }
    cachedTheme = theme

    const token = ++requestToken
    const { kind, packId } = parseThemeSource(selection.themeSource)

    if (kind !== 'pack' || !packId) {
        cachedManifest = null
        applySourceTheme(theme, cachedSelection, kind === 'custom' ? 'custom' : 'builtin')
        ensureSystemThemeListener()
        return { missingPackId: null }
    }

    // A backend read failure is treated like a missing pack so a stale selection
    // cannot break startup; the caller only reports the pack id.
    const installed = await getThemePack(packId).catch(() => null)
    // A newer selection started while this pack was loading; drop this result.
    if (token !== requestToken) return { missingPackId: null }

    if (!installed) {
        cachedManifest = null
        applySourceTheme(theme, cachedSelection, 'builtin')
        ensureSystemThemeListener()
        return { missingPackId: packId }
    }

    cachedManifest = installed.manifest
    applyPackManifest(theme, installed.manifest, cachedSelection)
    ensureSystemThemeListener()
    return { missingPackId: null }
}

/**
 * Re-applies the last selection after the resolved light/dark mode changed.
 * The pack manifest is reused so no settings or pack read is needed.
 */
export async function reapplyThemeRuntime(theme: ColorTheme): Promise<ThemeRuntimeResult> {
    const selection = cachedSelection
    if (!selection) return { missingPackId: null }

    cachedTheme = theme
    const { kind, packId } = parseThemeSource(selection.themeSource)

    if (kind === 'pack' && packId && cachedManifest?.id === packId) {
        applyPackManifest(theme, cachedManifest, selection)
        return { missingPackId: null }
    }

    return applyThemeRuntime(theme, selection)
}

/** Removes every runtime effect and drops cached state. */
export function resetThemeRuntime() {
    requestToken += 1
    cachedSelection = null
    cachedManifest = null
    clearPackApplication()
    systemThemeQuery?.removeEventListener('change', handleSystemThemeChange)
    systemThemeQuery = null
}

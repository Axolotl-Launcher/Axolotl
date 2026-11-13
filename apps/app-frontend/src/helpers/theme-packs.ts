import { invoke } from '@tauri-apps/api/core'

export type ThemePackCapability = 'tokens' | 'styles' | 'pages'
export type ThemePackPermission = 'storage' | 'instances.read' | 'instances.logs' | 'network'

export type ThemePackManifest = {
    schemaVersion: number
    id: string
    name: Record<string, string>
    version: string
    description: Record<string, string>
    engines: { axolotl: string }
    capabilities: ThemePackCapability[]
    tokens: { light: Record<string, string>; dark: Record<string, string> }
    styles: string[]
    background: string | null
    fonts: { ui: string | null }
    permissions: ThemePackPermission[]
    network: { allow: string[] }
    pages: {
        id: string
        title: Record<string, string>
        icon: string | null
        entry: string
        order: number
        requires: ThemePackPermission[]
    }[]
}

export type InstalledThemePack = {
    manifest: ThemePackManifest
    path: string
    installedAt: number
    sizeBytes: number
}

export type ThemePackLimits = {
    maxArchiveBytes: number
    maxUncompressedBytes: number
    maxEntries: number
}

export type ResolvedThemePackPage = {
    packId: string
    pageId: string
    title: Record<string, string>
    path: string
    url: string
}

/**
 * The appearance state sent to the backend when exporting a snapshot pack.
 * `customThemeColors` stays opaque: the backend only copies it into `origin.json`.
 */
export type ThemePackSnapshot = {
    schemaVersion: number
    source: string
    theme: string
    accentColor: string | null
    customThemeColors: Record<string, unknown> | null
    tokensLight: Record<string, string>
    tokensDark: Record<string, string>
    packId: string | null
}

function themePackCommand(command: string): string {
    return `plugin:theme-pack|${command}`
}

export function listThemePacks(): Promise<InstalledThemePack[]> {
    return invoke<InstalledThemePack[]>(themePackCommand('theme_pack_list'))
}

export function getThemePack(packId: string): Promise<InstalledThemePack | null> {
    return invoke<InstalledThemePack | null>(themePackCommand('theme_pack_get'), { packId })
}

export function installThemePack(
    archivePath: string,
    replace = false,
): Promise<InstalledThemePack> {
    return invoke<InstalledThemePack>(themePackCommand('theme_pack_install'), {
        archivePath,
        replace,
    })
}

export function removeThemePack(packId: string, removeData = false): Promise<boolean> {
    return invoke<boolean>(themePackCommand('theme_pack_remove'), { packId, removeData })
}

export function getThemePackLimits(): Promise<ThemePackLimits> {
    return invoke<ThemePackLimits>(themePackCommand('theme_pack_limits'))
}

export function exportThemePackSnapshot(
    destination: string,
    snapshot: ThemePackSnapshot,
): Promise<string> {
    return invoke<string>(themePackCommand('theme_pack_export_snapshot'), {
        destination,
        snapshot,
    })
}

export type ThemePackPageBounds = {
    x: number
    y: number
    width: number
    height: number
}

/** Opens a pack page in a dedicated child webview and returns its label. */
export function openThemePackPage(
    packId: string,
    pageId: string,
    bounds: ThemePackPageBounds,
): Promise<string> {
    return invoke<string>(themePackCommand('theme_pack_open_page'), {
        packId,
        pageId,
        ...bounds,
    })
}

export function setThemePackPageBounds(label: string, bounds: ThemePackPageBounds): Promise<void> {
    return invoke<void>(themePackCommand('theme_pack_page_bounds'), { label, ...bounds })
}

export function closeThemePackPage(label: string): Promise<void> {
    return invoke<void>(themePackCommand('theme_pack_close_page'), { label })
}

export function resolveThemePackPage(
    packId: string,
    pageId: string,
): Promise<ResolvedThemePackPage> {
    return invoke<ResolvedThemePackPage>(themePackCommand('theme_pack_resolve_page'), {
        packId,
        pageId,
    })
}

/**
 * Picks a locale entry from a manifest map using the same fallback order as the
 * backend: exact tag, language prefix, English, then any declared value.
 */
export function pickManifestText(
    values: Record<string, string> | undefined,
    locale: string,
): string {
    if (!values) return ''
    const normalized = locale.replace('_', '-')
    const exact = values[normalized] ?? values[locale]
    if (exact) return exact
    const language = normalized.split('-')[0].toLowerCase()
    for (const [key, value] of Object.entries(values)) {
        if (key.toLowerCase().split('-')[0] === language && value) return value
    }
    return values['en-US'] ?? values['en'] ?? Object.values(values).find((value) => !!value) ?? ''
}

export function themePackDisplayName(manifest: ThemePackManifest, locale: string): string {
    return pickManifestText(manifest.name, locale) || manifest.id
}

export function themePackDescription(manifest: ThemePackManifest, locale: string): string {
    return pickManifestText(manifest.description, locale)
}

export type ThemePackNavPage = {
    packId: string
    pageId: string
    title: string
    iconUrl: string | null
}

/** Fired whenever the active theme source changes so the shell can re-read nav pages. */
export const THEME_SOURCE_CHANGED_EVENT = 'axolotl-theme-source-changed'

export function notifyThemeSourceChanged(source: string): void {
    window.dispatchEvent(new CustomEvent(THEME_SOURCE_CHANGED_EVENT, { detail: source }))
}

/**
 * Pages the active pack contributes to the launcher navigation. A page is only
 * offered when the pack declares the `pages` capability and the permissions
 * the page itself requires.
 */
export async function listActiveThemePackPages(
    source: string | null | undefined,
    locale: string,
): Promise<ThemePackNavPage[]> {
    if (!source?.startsWith('pack:')) return []
    const pack = await getThemePack(source.slice('pack:'.length))
    if (!pack) return []
    const { manifest } = pack
    if (!manifest.capabilities.includes('pages')) return []
    return manifest.pages
        .filter((page) =>
            page.requires.every((permission) => manifest.permissions.includes(permission)),
        )
        .sort((left, right) => left.order - right.order || left.id.localeCompare(right.id))
        .map((page) => ({
            packId: manifest.id,
            pageId: page.id,
            title: pickManifestText(page.title, locale) || page.id,
            iconUrl: page.icon ? themePackAssetUrl(manifest.id, page.icon) : null,
        }))
}

export type ThemePackPagesRefresher = {
    refresh: (apply: (pages: ThemePackNavPage[]) => void) => Promise<void>
    invalidate: () => void
}

/**
 * Loads the active pack's navigation pages with latest-result-wins semantics.
 * `invalidate` must run when the caller is disposed so a pending read cannot
 * publish into a dead component.
 */
export function createThemePackPagesRefresher(
    readSource: () => Promise<string | null | undefined>,
    readLocale: () => string,
    listPages: (
        source: string | null | undefined,
        locale: string,
    ) => Promise<ThemePackNavPage[]> = listActiveThemePackPages,
): ThemePackPagesRefresher {
    let request = 0
    return {
        async refresh(apply) {
            const current = ++request
            const source = await readSource().catch(() => null)
            if (!source || current !== request) return
            const pages = await listPages(source, readLocale()).catch(() => [])
            if (current !== request) return
            apply(pages)
        },
        invalidate() {
            request += 1
        },
    }
}

/** The origin the Rust `axolotl-theme` protocol serves pack assets from. */
export const THEME_PACK_ORIGIN = navigator.userAgent.includes('Windows')
    ? 'http://axolotl-theme.localhost'
    : 'axolotl-theme://localhost'

/**
 * `encodeURIComponent` leaves characters such as `'` and `)` untouched, but the
 * runtime inserts asset URLs into CSS strings (`url('…')`). Escape the RFC 3986
 * sub-delims as well so a pack path can never terminate the CSS declaration.
 */
function encodeAssetSegment(value: string): string {
    return encodeURIComponent(value).replace(
        /[!'()*]/g,
        (character) => `%${character.charCodeAt(0).toString(16).toUpperCase()}`,
    )
}

export function themePackAssetUrl(packId: string, relativePath: string): string {
    return `${THEME_PACK_ORIGIN}/${encodeAssetSegment(packId)}/${relativePath
        .split('/')
        .map(encodeAssetSegment)
        .join('/')}`
}

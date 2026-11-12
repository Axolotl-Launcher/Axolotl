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

export function listThemePacks(): Promise<InstalledThemePack[]> {
    return invoke<InstalledThemePack[]>('theme_pack_list')
}

export function getThemePack(packId: string): Promise<InstalledThemePack | null> {
    return invoke<InstalledThemePack | null>('theme_pack_get', { packId })
}

export function installThemePack(
    archivePath: string,
    replace = false,
): Promise<InstalledThemePack> {
    return invoke<InstalledThemePack>('theme_pack_install', { archivePath, replace })
}

export function removeThemePack(packId: string, removeData = false): Promise<boolean> {
    return invoke<boolean>('theme_pack_remove', { packId, removeData })
}

export function getThemePackLimits(): Promise<ThemePackLimits> {
    return invoke<ThemePackLimits>('theme_pack_limits')
}

export function resolveThemePackPage(
    packId: string,
    pageId: string,
): Promise<ResolvedThemePackPage> {
    return invoke<ResolvedThemePackPage>('theme_pack_resolve_page', { packId, pageId })
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

/** The origin the Rust `axolotl-theme` protocol serves pack assets from. */
export const THEME_PACK_ORIGIN = navigator.userAgent.includes('Windows')
    ? 'http://axolotl-theme.localhost'
    : 'axolotl-theme://localhost'

export function themePackAssetUrl(packId: string, relativePath: string): string {
    return `${THEME_PACK_ORIGIN}/${encodeURIComponent(packId)}/${relativePath
        .split('/')
        .map((part) => encodeURIComponent(part))
        .join('/')}`
}

import { isDyeColor } from './colors.ts'
import { isBannerPatternId, MAX_BANNER_LAYERS } from './patterns.ts'
import type { BannerGameVersion } from './serialization.ts'
import { type BannerDesign, type BannerLayer, cloneBannerDesign, designsMatch } from './types.ts'

const RECENT_KEY = 'axolotl:lab:banner-designer:recent:v1'
const MAX_RECENT = 6
const EXPORT_KEY = 'axolotl:lab:banner-designer:export:v1'

/** Shapes the export modal can copy, mirroring its format chips. */
export type BannerExportFormat = 'give' | 'data' | 'json'

export interface BannerExportPreference {
    format: BannerExportFormat
    version: BannerGameVersion
}

const EXPORT_FORMATS: readonly BannerExportFormat[] = ['give', 'data', 'json']
const EXPORT_VERSIONS: readonly BannerGameVersion[] = ['modern', 'legacy', 'classic']

export interface RecentBanner {
    id: string
    design: BannerDesign
    savedAt: number
}

function safeParse<T>(key: string, fallback: T): T {
    try {
        return JSON.parse(localStorage.getItem(key) ?? '') as T
    } catch {
        return fallback
    }
}

function persist(records: RecentBanner[]) {
    try {
        localStorage.setItem(RECENT_KEY, JSON.stringify(records))
    } catch {
        // A full or unavailable store must never break closing the editor.
    }
}

function isLayer(value: unknown): value is BannerLayer {
    if (!value || typeof value !== 'object') return false
    const layer = value as Partial<BannerLayer>
    return (
        typeof layer.id === 'string' &&
        typeof layer.pattern === 'string' &&
        typeof layer.color === 'string' &&
        isBannerPatternId(layer.pattern) &&
        isDyeColor(layer.color)
    )
}

function isDesign(value: unknown): value is BannerDesign {
    if (!value || typeof value !== 'object') return false
    const design = value as Partial<BannerDesign>
    return (
        typeof design.baseColor === 'string' &&
        isDyeColor(design.baseColor) &&
        Array.isArray(design.layers) &&
        design.layers.length <= MAX_BANNER_LAYERS &&
        design.layers.every(isLayer)
    )
}

export function loadRecentBanners(): RecentBanner[] {
    const records = safeParse<RecentBanner[]>(RECENT_KEY, [])
    if (!Array.isArray(records)) return []
    return records
        .filter(
            (record) =>
                record &&
                typeof record.id === 'string' &&
                typeof record.savedAt === 'number' &&
                isDesign(record.design),
        )
        .slice(0, MAX_RECENT)
}

let sequence = 0

function nextId() {
    sequence += 1
    return `banner-${Date.now().toString(36)}-${sequence}`
}

/** Puts `design` at the top of the list, replacing an identical banner if there is one. */
export function recordRecentBanner(design: BannerDesign): RecentBanner[] {
    const record: RecentBanner = {
        id: nextId(),
        design: cloneBannerDesign(design),
        savedAt: Date.now(),
    }
    const records = loadRecentBanners().filter((entry) => !designsMatch(entry.design, design))
    const next = [record, ...records].slice(0, MAX_RECENT)
    persist(next)
    return next
}

export function removeRecentBanner(id: string): RecentBanner[] {
    const next = loadRecentBanners().filter((record) => record.id !== id)
    persist(next)
    return next
}

export function clearRecentBanners(): RecentBanner[] {
    try {
        localStorage.removeItem(RECENT_KEY)
    } catch {
        // Nothing to do: the list is cleared in memory either way.
    }
    return []
}

/**
 * Remembers the last export dialect so a player who is on an older game version
 * does not have to switch back from the default on every export.
 */
export function loadExportPreference(): BannerExportPreference | null {
    const value = safeParse<Partial<BannerExportPreference> | null>(EXPORT_KEY, null)
    if (!value || typeof value !== 'object') return null
    const format = value.format as BannerExportFormat
    const version = value.version as BannerGameVersion
    if (!EXPORT_FORMATS.includes(format) || !EXPORT_VERSIONS.includes(version)) return null
    return { format, version }
}

export function saveExportPreference(preference: BannerExportPreference): void {
    try {
        localStorage.setItem(EXPORT_KEY, JSON.stringify(preference))
    } catch {
        // Preferences are a convenience; a full store must not break exporting.
    }
}

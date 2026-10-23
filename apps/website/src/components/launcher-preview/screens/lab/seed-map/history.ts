import type { SeedMapEdition } from './backend.ts'

export type SeedMapHistorySource = 'manual' | 'random' | 'instance' | 'share'

export type SeedMapHistoryEntry = {
    id: string
    seed: string
    edition: SeedMapEdition
    gameVersion: string
    source: SeedMapHistorySource
    instanceName?: string
    worldName?: string
    firstViewedAt: number
    lastViewedAt: number
    completedFeatures: string[]
    completedOres: string[]
}

export type SeedMapHistoryDraft = {
    seed: string
    edition: SeedMapEdition
    gameVersion: string
    source: SeedMapHistorySource
    instanceName?: string
    worldName?: string
    completedFeatures?: string[]
    completedOres?: string[]
}

type StorageLike = Pick<Storage, 'getItem' | 'setItem'>

export const SEED_MAP_HISTORY_KEY = 'axolotl.lab.seed-map.history.v1'
export const SEED_MAP_HISTORY_LIMIT = 60

const SOURCES: readonly SeedMapHistorySource[] = ['manual', 'random', 'instance', 'share']

export function seedMapHistoryId(seed: string, edition: SeedMapEdition): string {
    return `${edition}:${seed}`
}

export function loadSeedMapHistory(
    storage: StorageLike | null = getBrowserStorage(),
): SeedMapHistoryEntry[] {
    if (!storage) return []
    try {
        const raw = storage.getItem(SEED_MAP_HISTORY_KEY)
        if (!raw) return []
        return sanitizeSeedMapHistory(JSON.parse(raw))
    } catch {
        return []
    }
}

/**
 * Records a viewed seed. Revisiting a known seed refreshes its timestamp and
 * game version in place; richer attribution (an instance/world label) always
 * wins over a plain manual revisit so the list keeps its most useful context.
 */
export function recordSeedMapHistory(
    draft: SeedMapHistoryDraft,
    now: number = Date.now(),
    storage: StorageLike | null = getBrowserStorage(),
): SeedMapHistoryEntry[] {
    const seed = draft.seed.trim().slice(0, 256)
    if (!seed) return loadSeedMapHistory(storage)
    const entries = loadSeedMapHistory(storage)
    const id = seedMapHistoryId(seed, draft.edition)
    const existing = entries.find((entry) => entry.id === id)
    const keepExistingAttribution =
        existing !== undefined && existing.instanceName !== undefined && draft.source !== 'instance'
    const next: SeedMapHistoryEntry = {
        id,
        seed,
        edition: draft.edition,
        gameVersion: draft.gameVersion,
        source: keepExistingAttribution ? existing.source : draft.source,
        instanceName: keepExistingAttribution ? existing.instanceName : draft.instanceName,
        worldName: keepExistingAttribution ? existing.worldName : draft.worldName,
        firstViewedAt: existing?.firstViewedAt ?? now,
        lastViewedAt: now,
        completedFeatures: draft.completedFeatures ?? existing?.completedFeatures ?? [],
        completedOres: draft.completedOres ?? existing?.completedOres ?? [],
    }
    const merged = [next, ...entries.filter((entry) => entry.id !== id)].slice(
        0,
        SEED_MAP_HISTORY_LIMIT,
    )
    persist(merged, storage)
    return merged
}

/**
 * Persists exploration progress for a known seed without touching its
 * position in the list or its timestamps. Unknown ids are ignored.
 */
export function updateSeedMapHistoryProgress(
    id: string,
    completedFeatures: string[],
    completedOres: string[],
    storage: StorageLike | null = getBrowserStorage(),
): SeedMapHistoryEntry[] {
    const entries = loadSeedMapHistory(storage)
    const entry = entries.find((candidate) => candidate.id === id)
    if (!entry) return entries
    entry.completedFeatures = completedFeatures.slice(0, 2_000)
    entry.completedOres = completedOres.slice(0, 10_000)
    persist(entries, storage)
    return entries
}

export function removeSeedMapHistoryEntry(
    id: string,
    storage: StorageLike | null = getBrowserStorage(),
): SeedMapHistoryEntry[] {
    const entries = loadSeedMapHistory(storage).filter((entry) => entry.id !== id)
    persist(entries, storage)
    return entries
}

export function clearSeedMapHistory(
    storage: StorageLike | null = getBrowserStorage(),
): SeedMapHistoryEntry[] {
    persist([], storage)
    return []
}

export function sanitizeSeedMapHistory(value: unknown): SeedMapHistoryEntry[] {
    if (!Array.isArray(value)) return []
    const seen = new Set<string>()
    const entries: SeedMapHistoryEntry[] = []
    for (const item of value) {
        if (!item || typeof item !== 'object') continue
        const source = item as Partial<SeedMapHistoryEntry>
        if (typeof source.seed !== 'string' || !source.seed.trim()) continue
        const edition: SeedMapEdition =
            source.edition === 'java-large-biomes' ? 'java-large-biomes' : 'java'
        const seed = source.seed.trim().slice(0, 256)
        const id = seedMapHistoryId(seed, edition)
        if (seen.has(id)) continue
        seen.add(id)
        const lastViewedAt = finiteTime(source.lastViewedAt)
        entries.push({
            id,
            seed,
            edition,
            gameVersion:
                typeof source.gameVersion === 'string' ? source.gameVersion.slice(0, 32) : '',
            source: SOURCES.includes(source.source as SeedMapHistorySource)
                ? (source.source as SeedMapHistorySource)
                : 'manual',
            instanceName:
                typeof source.instanceName === 'string'
                    ? source.instanceName.slice(0, 128)
                    : undefined,
            worldName:
                typeof source.worldName === 'string' ? source.worldName.slice(0, 128) : undefined,
            firstViewedAt: finiteTime(source.firstViewedAt) || lastViewedAt,
            lastViewedAt,
            completedFeatures: sanitizeKeyList(source.completedFeatures, 2_000),
            completedOres: sanitizeKeyList(source.completedOres, 10_000),
        })
    }
    return entries.sort((a, b) => b.lastViewedAt - a.lastViewedAt).slice(0, SEED_MAP_HISTORY_LIMIT)
}

function persist(entries: SeedMapHistoryEntry[], storage: StorageLike | null): void {
    try {
        storage?.setItem(SEED_MAP_HISTORY_KEY, JSON.stringify(entries))
    } catch {
        // Ignore quota errors: history is a convenience and must never break the map.
    }
}

function sanitizeKeyList(value: unknown, limit: number): string[] {
    if (!Array.isArray(value)) return []
    return value.filter((key): key is string => typeof key === 'string').slice(0, limit)
}

function finiteTime(value: unknown): number {
    return typeof value === 'number' && Number.isFinite(value) && value > 0 ? value : 0
}

function getBrowserStorage(): Storage | null {
    return typeof window === 'undefined' ? null : window.localStorage
}

// `lab/seed-map/area-analysis.ts`; named apart from SeedMapAreaAnalysis.vue for Nuxt's component scan.
import type {
    SeedMapContainer,
    SeedMapLoot,
    SeedMapStructureDetails,
    SeedMapStructureQuery,
} from './advanced.ts'
import { getSeedMapContainerLoot, getSeedMapStructureDetails } from './advanced.ts'
import { findSeedMapFeatures, scanSeedMapOres, type SeedMapFeatureQuery } from './backend.ts'
import { featureMask, SEED_MAP_FEATURES, type SeedMapFeature } from './features.ts'
import { mergeSeedMapLootItems } from './loot-items.ts'
import type { SeedMapOreKind } from './ores.ts'
import { SeedMapPreviewLimit, type SeedMapPreviewLimitReason } from './preview-data.ts'

export type SeedMapArea = { minX: number; minZ: number; maxX: number; maxZ: number }
export type SeedMapAreaContext = Pick<
    SeedMapStructureQuery,
    'seed' | 'edition' | 'version' | 'dimension'
>
export type AreaProgress = { done: number; total: number; phase?: 'structures' | 'analysis' }
export const SEED_MAP_AREA_ORE_MAX_CHUNKS = 16536
export const SEED_MAP_AREA_LOOT_MAX_CHUNKS = 262144
const MAX_CANDIDATES = 1_000_000
const MAX_STRUCTURES = 50_000
const MAX_CONTAINERS = 100_000
const MAX_MATCHES = 2000
const MINERALS: readonly SeedMapOreKind[] = [
    'diamond',
    'iron',
    'iron_vein',
    'copper',
    'copper_vein',
    'gold',
    'redstone',
    'lapis',
    'coal',
    'netherite',
    'emerald',
    'quartz',
    'nether_gold',
]
const LOOT_STRUCTURES = new Set([
    'outpost',
    'shipwreck',
    'ancient-city',
    'ruined-portal',
    'stronghold',
    'desert-pyramid',
    'jungle-temple',
    'igloo',
    'buried-treasure',
    'fortress',
    'bastion',
    'end-city',
    'abandoned-camp',
])

export function seedMapArea(
    first: { x: number; z: number },
    second: { x: number; z: number },
): SeedMapArea {
    return {
        minX: Math.min(first.x, second.x),
        minZ: Math.min(first.z, second.z),
        maxX: Math.max(first.x, second.x),
        maxZ: Math.max(first.z, second.z),
    }
}

export function areaChunkCount(area: SeedMapArea) {
    return (
        (Math.floor(area.maxX / 16) - Math.floor(area.minX / 16) + 1) *
        (Math.floor(area.maxZ / 16) - Math.floor(area.minZ / 16) + 1)
    )
}

export function areaContains(area: SeedMapArea, x: number, z: number) {
    return x >= area.minX && x <= area.maxX && z >= area.minZ && z <= area.maxZ
}

function checkArea(area: SeedMapArea, maximum: number) {
    if (
        !Object.values(area).every(
            (value) => Number.isSafeInteger(value) && Math.abs(value) <= 30_000_000,
        ) ||
        area.minX > area.maxX ||
        area.minZ > area.maxZ ||
        areaChunkCount(area) > maximum
    )
        throw new Error('Invalid analysis area')
}

function checkCanceled(signal: AbortSignal) {
    signal.throwIfAborted()
}

export type AreaOreResult = { counts: { ore: SeedMapOreKind; count: number }[]; truncated: boolean }
export async function analyzeAreaOres(
    context: SeedMapAreaContext,
    area: SeedMapArea,
    kinds: SeedMapOreKind[],
    yMin: number,
    yMax: number,
    signal: AbortSignal,
    onProgress: (progress: AreaProgress) => void,
    scan = scanSeedMapOres,
): Promise<AreaOreResult> {
    checkArea(area, SEED_MAP_AREA_ORE_MAX_CHUNKS)
    if (!Number.isInteger(yMin) || !Number.isInteger(yMax) || yMin > yMax)
        throw new Error('Invalid height range')
    const ores = [...new Set(kinds.filter((kind) => MINERALS.includes(kind)))]
    const tiles: {
        area: SeedMapArea
        minCX: number
        maxCX: number
        minCZ: number
        maxCZ: number
    }[] = []
    const maxCX = Math.floor(area.maxX / 16)
    const maxCZ = Math.floor(area.maxZ / 16)
    for (let cx = Math.floor(area.minX / 16); cx <= maxCX; cx += 8) {
        for (let cz = Math.floor(area.minZ / 16); cz <= maxCZ; cz += 8) {
            const endX = Math.min(cx + 7, maxCX)
            const endZ = Math.min(cz + 7, maxCZ)
            tiles.push({
                area: {
                    minX: Math.max(area.minX, cx * 16),
                    minZ: Math.max(area.minZ, cz * 16),
                    maxX: Math.min(area.maxX, endX * 16 + 15),
                    maxZ: Math.min(area.maxZ, endZ * 16 + 15),
                },
                minCX: Math.max(-1_875_000, cx - 1),
                maxCX: Math.min(1_875_000, endX + 1),
                minCZ: Math.max(-1_875_000, cz - 1),
                maxCZ: Math.min(1_875_000, endZ + 1),
            })
        }
    }
    const counts = new Map<SeedMapOreKind, number>()
    let done = 0
    let truncated = false
    const total = tiles.reduce(
        (sum, tile) =>
            sum + (tile.maxCX - tile.minCX + 1) * (tile.maxCZ - tile.minCZ + 1) * ores.length,
        0,
    )
    onProgress({ done, total })
    for (const tile of tiles) {
        checkCanceled(signal)
        const chunks: number[][] = []
        for (let cx = tile.minCX; cx <= tile.maxCX; cx++)
            for (let cz = tile.minCZ; cz <= tile.maxCZ; cz++) chunks.push([cx, cz])
        const found = new Map<SeedMapOreKind, Set<string>>()
        let candidates = 0
        outer: for (const ore of ores) {
            const group = ore === 'iron_vein' ? 'iron' : ore === 'copper_vein' ? 'copper' : ore
            const positions = found.get(group) ?? new Set<string>()
            found.set(group, positions)
            for (let offset = 0; offset < chunks.length; offset += 4) {
                checkCanceled(signal)
                const batch = chunks.slice(offset, offset + 4)
                const results = await scan({ ...context, ores: [ore], chunks: batch.flat() })
                checkCanceled(signal)
                for (const result of results) {
                    truncated ||= result.truncated
                    for (const hit of result.hits) {
                        if (
                            hit.ore !== ore ||
                            hit.y < yMin ||
                            hit.y > yMax ||
                            !areaContains(tile.area, hit.x, hit.z)
                        )
                            continue
                        const key = `${hit.x},${hit.y},${hit.z}`
                        if (!positions.has(key)) {
                            positions.add(key)
                            candidates++
                        }
                        if (candidates >= MAX_CANDIDATES) {
                            truncated = true
                            break outer
                        }
                    }
                }
                done += batch.length
                onProgress({ done, total })
            }
        }
        for (const [ore, positions] of found)
            counts.set(ore, (counts.get(ore) ?? 0) + positions.size)
    }
    return {
        counts: [...counts]
            .map(([ore, count]) => ({ ore, count }))
            .filter((row) => row.count > 0)
            .sort((a, b) => b.count - a.count),
        truncated,
    }
}

export type AreaLootMatch = {
    feature: SeedMapFeature
    container: SeedMapContainer
    items: NonNullable<SeedMapLoot['items']>
    estimated: boolean
    positionEstimated: boolean
}
export type AreaLootResult = {
    matches: AreaLootMatch[]
    containers: number
    unsupported: number
    failed: number
    truncated: boolean
    unresolved: boolean
    /** Set when a structure lies outside the preview's data. */
    previewLimit?: SeedMapPreviewLimitReason
}
export async function searchAreaLoot(
    context: SeedMapAreaContext,
    area: SeedMapArea,
    supportedMask: number,
    matchesItem: (id: string) => boolean,
    signal: AbortSignal,
    onProgress: (progress: AreaProgress) => void,
    services: {
        features: (query: SeedMapFeatureQuery) => Promise<SeedMapFeature[]>
        details: (query: SeedMapStructureQuery) => Promise<SeedMapStructureDetails>
        loot: (query: SeedMapStructureQuery, container: number) => Promise<SeedMapLoot>
    } = {
        features: findSeedMapFeatures,
        details: getSeedMapStructureDetails,
        loot: getSeedMapContainerLoot,
    },
): Promise<AreaLootResult> {
    checkArea(area, SEED_MAP_AREA_LOOT_MAX_CHUNKS)
    const result: AreaLootResult = {
        matches: [],
        containers: 0,
        unsupported: 0,
        failed: 0,
        truncated: false,
        unresolved: false,
    }
    const mask =
        featureMask(
            SEED_MAP_FEATURES.filter(
                (item) =>
                    item.dimensions.includes(context.dimension) && LOOT_STRUCTURES.has(item.kind),
            ).map((item) => item.kind),
        ) & supportedMask
    if (!mask) return result
    const structures = new Map<string, SeedMapFeature>()
    const bounds = {
        minX: Math.max(-30_000_000, area.minX - 512),
        minZ: Math.max(-30_000_000, area.minZ - 512),
        maxX: Math.min(30_000_000, area.maxX + 512),
        maxZ: Math.min(30_000_000, area.maxZ + 512),
    }
    const cells =
        Math.ceil((bounds.maxX - bounds.minX + 1) / 512) *
        Math.ceil((bounds.maxZ - bounds.minZ + 1) / 512)
    let discovered = 0
    onProgress({ done: 0, total: cells, phase: 'structures' })
    discover: for (let x = bounds.minX; x <= bounds.maxX; x += 512) {
        for (let z = bounds.minZ; z <= bounds.maxZ; z += 512) {
            checkCanceled(signal)
            const features = await services.features({
                ...context,
                minX: x,
                minZ: z,
                maxX: Math.min(x + 511, bounds.maxX),
                maxZ: Math.min(z + 511, bounds.maxZ),
                featureMask: mask,
            })
            checkCanceled(signal)
            result.truncated ||= features.length >= 4096
            for (const feature of features) {
                if (!LOOT_STRUCTURES.has(feature.kind)) continue
                structures.set(`${feature.kind}:${feature.x}:${feature.z}`, feature)
                if (structures.size >= MAX_STRUCTURES) {
                    result.truncated = true
                    break discover
                }
            }
            onProgress({ done: ++discovered, total: cells, phase: 'structures' })
        }
    }
    let done = 0
    let attempted = 0
    onProgress({ done: 0, total: structures.size, phase: 'analysis' })
    for (const feature of structures.values()) {
        checkCanceled(signal)
        const query = {
            ...context,
            x: feature.x,
            z: feature.z,
            featureMask: featureMask([feature.kind]),
            includeBlocks: false,
        }
        try {
            const details = await services.details(query)
            checkCanceled(signal)
            result.truncated ||= details.truncated || details.limits.includes('partial_layout')
            if (!details.supported) result.unsupported++
            const containers = details.containers.filter((container) =>
                areaContains(area, container.x, container.z),
            )
            for (const [index, container] of containers.entries()) {
                if (attempted >= MAX_CONTAINERS) {
                    result.truncated = true
                    break
                }
                checkCanceled(signal)
                attempted++
                try {
                    const loot = await services.loot(query, container.id)
                    checkCanceled(signal)
                    result.containers++
                    result.truncated ||= loot.truncated
                    result.unresolved ||= !!loot.unresolvedTables?.length
                    if (!loot.supported) {
                        result.unsupported++
                        continue
                    }
                    const items = mergeSeedMapLootItems(
                        (loot.items ?? []).filter((item) => matchesItem(item.id)),
                    )
                    if (items.length)
                        result.matches.push({
                            feature,
                            container,
                            items,
                            estimated: loot.estimated === true || container.estimated === true,
                            positionEstimated: details.limits.includes(
                                'container_positions_approximate',
                            ),
                        })
                    if (result.matches.length >= MAX_MATCHES) {
                        result.truncated = true
                        break
                    }
                } catch (error) {
                    checkCanceled(signal)
                    result.failed++
                    if (error instanceof SeedMapPreviewLimit) result.previewLimit ??= error.reason
                }
                onProgress({
                    done: done + (index + 1) / containers.length,
                    total: structures.size,
                    phase: 'analysis',
                })
            }
        } catch (error) {
            checkCanceled(signal)
            result.failed++
            if (error instanceof SeedMapPreviewLimit) result.previewLimit ??= error.reason
        }
        onProgress({ done: ++done, total: structures.size })
        if (attempted >= MAX_CONTAINERS || result.matches.length >= MAX_MATCHES) {
            result.truncated = true
            break
        }
    }
    return result
}

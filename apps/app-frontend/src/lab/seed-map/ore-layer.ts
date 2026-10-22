import { ref } from 'vue'

import { scanSeedMapOres, SEED_MAP_ENGINE_REVISION, type SeedMapEdition } from './backend.ts'
import type { SeedMapDimension } from './biomes.ts'
import {
    preferSeedMapOreHit,
    SEED_MAP_ORE_CACHE_LIMIT,
    SEED_MAP_ORE_CACHE_TARGET,
    SEED_MAP_ORE_MAX_SCALE,
    type SeedMapOreChunk,
    seedMapOreColumnKey,
    type SeedMapOreHit,
    type SeedMapOreKind,
    seedMapOreScanBudget,
} from './ores.ts'

export type SeedMapOreViewport = {
    minX: number
    minZ: number
    maxX: number
    maxZ: number
}

export type SeedMapOreLayerContext = {
    enabled: boolean
    seed: string
    edition: SeedMapEdition
    version: string
    dimension: SeedMapDimension
    scale: number
    selectedOres: SeedMapOreKind[]
    yMin: number | null
    yMax: number | null
    bounds: SeedMapOreViewport
    center: { x: number; z: number }
}

type OreChunkCache = Map<SeedMapOreKind, SeedMapOreHit[]>

const SCAN_BATCH_CHUNKS = 8

export function useSeedMapOreLayer(options: {
    onUpdate: () => void
    onError: (message: string) => void
}) {
    const hits = ref<SeedMapOreHit[]>([])
    const scanning = ref(false)
    const truncated = ref(false)
    const progress = ref(0)
    const scannedChunks = ref(0)
    const totalChunks = ref(0)
    const chunkCache = new Map<string, OreChunkCache>()
    let context: SeedMapOreLayerContext | null = null
    let contextKey = ''
    let refreshTimer: ReturnType<typeof setTimeout> | undefined
    let rebuildTimer: ReturnType<typeof setTimeout> | undefined
    let refreshGeneration = 0
    let lastRebuild = 0

    function refresh(nextContext: SeedMapOreLayerContext, delay: number | null = 180): void {
        context = cloneContext(nextContext)
        refreshGeneration++
        if (refreshTimer) clearTimeout(refreshTimer)
        refreshTimer = undefined
        scanning.value = false
        const nextKey = `${SEED_MAP_ENGINE_REVISION}|${nextContext.seed}|${nextContext.edition}|${nextContext.version}|${nextContext.dimension}|${[...nextContext.selectedOres].sort().join(',')}`
        if (nextKey !== contextKey) {
            contextKey = nextKey
            truncated.value = false
            chunkCache.clear()
        }
        rebuildVisibleHits()
        if (!canScan(nextContext)) {
            scanning.value = false
            progress.value = 0
            scannedChunks.value = 0
            totalChunks.value = 0
            return
        }
        const generation = refreshGeneration
        if (delay !== null)
            refreshTimer = setTimeout(() => void scanVisibleChunks(generation), delay)
    }

    function refreshFilter(yMin: number | null, yMax: number | null): void {
        if (!context) return
        context.yMin = yMin
        context.yMax = yMax
        rebuildVisibleHits()
    }

    function clear(): void {
        refreshGeneration++
        if (refreshTimer) clearTimeout(refreshTimer)
        if (rebuildTimer) clearTimeout(rebuildTimer)
        chunkCache.clear()
        truncated.value = false
        hits.value = []
        scanning.value = false
        progress.value = 0
        scannedChunks.value = 0
        totalChunks.value = 0
        options.onUpdate()
    }

    function dispose(): void {
        clear()
    }

    async function scanVisibleChunks(generation: number): Promise<void> {
        refreshTimer = undefined
        if (generation !== refreshGeneration || !context || !canScan(context)) return
        scanning.value = true
        progress.value = 0
        try {
            const missing = missingChunks(context)
            totalChunks.value = missing.length
            scannedChunks.value = 0
            for (
                let offset = 0;
                offset < missing.length && generation === refreshGeneration;
                offset += SCAN_BATCH_CHUNKS
            ) {
                const batch = missing.slice(offset, offset + SCAN_BATCH_CHUNKS)
                const results = await scanSeedMapOres({
                    seed: context.seed,
                    edition: context.edition,
                    version: context.version,
                    dimension: context.dimension,
                    ores: [...context.selectedOres],
                    chunks: batch.flatMap(({ cx, cz }) => [cx, cz]),
                })
                if (generation !== refreshGeneration) return
                for (const cell of results) {
                    truncated.value ||= cell.truncated
                    const cache = chunkCache.get(chunkKey(cell.cx, cell.cz)) ?? new Map()
                    const grouped = new Map<SeedMapOreKind, SeedMapOreHit[]>()
                    for (const hit of cell.hits) {
                        const list = grouped.get(hit.ore) ?? []
                        list.push(hit)
                        grouped.set(hit.ore, list)
                    }
                    for (const ore of context.selectedOres) {
                        cache.set(ore, grouped.get(ore) ?? [])
                    }
                    chunkCache.set(chunkKey(cell.cx, cell.cz), cache)
                }
                scannedChunks.value = Math.min(missing.length, offset + batch.length)
                progress.value = Math.min(1, scannedChunks.value / Math.max(1, missing.length))
                pruneCache(context)
                scheduleRebuild()
            }
        } catch (error) {
            if (generation === refreshGeneration) {
                options.onError(error instanceof Error ? error.message : String(error))
            }
        } finally {
            if (generation === refreshGeneration) {
                scanning.value = false
                progress.value = 1
                rebuildVisibleHits()
            }
        }
    }

    function missingChunks(activeContext: SeedMapOreLayerContext): SeedMapOreChunk[] {
        const chunks = chunksInViewport(activeContext.bounds, activeContext.center)
        const budget = seedMapOreScanBudget(activeContext.selectedOres)
        const missing: SeedMapOreChunk[] = []
        for (const chunk of chunks) {
            const cache = chunkCache.get(chunkKey(chunk.cx, chunk.cz))
            if (activeContext.selectedOres.every((ore) => cache?.has(ore))) continue
            missing.push(chunk)
            if (missing.length >= budget) break
        }
        return missing
    }

    function rebuildVisibleHits(): void {
        if (!context || !canDisplay(context)) {
            hits.value = []
            options.onUpdate()
            return
        }
        const columns = new Map<string, SeedMapOreHit>()
        const selected = new Set(context.selectedOres)
        for (const chunk of chunksInViewport(context.bounds, context.center, false)) {
            const cache = chunkCache.get(chunkKey(chunk.cx, chunk.cz))
            if (!cache) continue
            for (const [ore, oreHits] of cache) {
                if (!selected.has(ore)) continue
                for (const hit of oreHits) {
                    if (context.yMin !== null && hit.y < context.yMin) continue
                    if (context.yMax !== null && hit.y > context.yMax) continue
                    const key = seedMapOreColumnKey(hit)
                    columns.set(key, preferSeedMapOreHit(columns.get(key), hit))
                }
            }
        }
        hits.value = [...columns.values()].sort(
            (a, b) =>
                (a.x - context!.center.x) ** 2 +
                (a.z - context!.center.z) ** 2 -
                ((b.x - context!.center.x) ** 2 + (b.z - context!.center.z) ** 2),
        )
        lastRebuild = performance.now()
        options.onUpdate()
    }

    function scheduleRebuild(): void {
        if (rebuildTimer) return
        const delay = Math.max(0, 140 - (performance.now() - lastRebuild))
        rebuildTimer = setTimeout(() => {
            rebuildTimer = undefined
            rebuildVisibleHits()
        }, delay)
    }

    function pruneCache(activeContext: SeedMapOreLayerContext): void {
        if (chunkCache.size <= SEED_MAP_ORE_CACHE_LIMIT) return
        const centerX = Math.floor(activeContext.center.x / 16)
        const centerZ = Math.floor(activeContext.center.z / 16)
        const entries = [...chunkCache.keys()].map((key) => {
            const [cx, cz] = key.split(',').map(Number)
            return { key, distance: (cx - centerX) ** 2 + (cz - centerZ) ** 2 }
        })
        entries.sort((a, b) => b.distance - a.distance)
        const removeCount = chunkCache.size - SEED_MAP_ORE_CACHE_TARGET
        for (let index = 0; index < removeCount; index++) {
            chunkCache.delete(entries[index].key)
        }
    }

    return {
        hits,
        scanning,
        truncated,
        progress,
        scannedChunks,
        totalChunks,
        refresh,
        refreshFilter,
        clear,
        dispose,
    }
}

function canDisplay(context: SeedMapOreLayerContext): boolean {
    return (
        context.enabled &&
        context.dimension !== 'end' &&
        context.scale <= SEED_MAP_ORE_MAX_SCALE &&
        context.selectedOres.length > 0
    )
}

function canScan(context: SeedMapOreLayerContext): boolean {
    return canDisplay(context) && context.seed.trim().length > 0
}

function chunksInViewport(
    bounds: SeedMapOreViewport,
    center: { x: number; z: number },
    sort = true,
): SeedMapOreChunk[] {
    const chunks: SeedMapOreChunk[] = []
    for (let cx = Math.floor(bounds.minX / 16); cx <= Math.floor(bounds.maxX / 16); cx++) {
        for (let cz = Math.floor(bounds.minZ / 16); cz <= Math.floor(bounds.maxZ / 16); cz++) {
            chunks.push({ cx, cz })
        }
    }
    if (sort) {
        const centerX = center.x / 16
        const centerZ = center.z / 16
        chunks.sort(
            (a, b) =>
                (a.cx - centerX) ** 2 +
                (a.cz - centerZ) ** 2 -
                ((b.cx - centerX) ** 2 + (b.cz - centerZ) ** 2),
        )
    }
    return chunks
}

function chunkKey(cx: number, cz: number): string {
    return `${cx},${cz}`
}

function cloneContext(context: SeedMapOreLayerContext): SeedMapOreLayerContext {
    return {
        ...context,
        selectedOres: [...context.selectedOres],
        bounds: { ...context.bounds },
        center: { ...context.center },
    }
}

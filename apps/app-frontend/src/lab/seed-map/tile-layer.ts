import type { SeedMapTile, SeedMapTileRequest } from './backend.ts'

export type SeedMapTileBounds = { minX: number; minZ: number; maxX: number; maxZ: number }
export type SeedMapTileLevel = { scale: number; pixels: number }
export type SeedMapTileView = {
    epoch: number
    context: Omit<SeedMapTileRequest, 'epoch' | 'x' | 'z' | 'scale' | 'width' | 'height'>
    bounds: SeedMapTileBounds
    center: { x: number; z: number }
    level: SeedMapTileLevel
    concurrency: number
}

type TileCell = { x: number; z: number; span: number }
export type SeedMapDrawableTile = TileCell & SeedMapTileLevel & { bitmap: ImageBitmap }
type CachedTile = SeedMapDrawableTile & { context: string }
type TileJob = TileCell & { key: string; context: string; request: SeedMapTileRequest }

const TILE_SCALES = [1, 4, 16, 64, 256] as const
const CACHE_BYTES = 32 * 1024 * 1024

/** Choose the closest native sampling interval; at most two samples per CSS pixel. */
export function seedMapTileLevel(blocksPerPixel: number): SeedMapTileLevel {
    const scale =
        TILE_SCALES.find((scale) => scale >= blocksPerPixel / 2) ??
        TILE_SCALES[TILE_SCALES.length - 1]
    return { scale, pixels: 256 }
}

export function seedMapPreviewLevel(level: SeedMapTileLevel): SeedMapTileLevel | undefined {
    return level.pixels === 256 && level.scale < 256
        ? { scale: level.scale * 4, pixels: 64 }
        : undefined
}

export function seedMapTileContextKey(context: SeedMapTileView['context']): string {
    return JSON.stringify([
        context.seed,
        context.edition,
        context.version,
        context.dimension,
        context.elevation ?? 128,
        context.terrain,
        context.contours,
        [...(context.highlightBiomes ?? [])].sort((a, b) => a - b),
    ])
}

function tileKey(context: string, level: SeedMapTileLevel, x: number, z: number): string {
    return `${context}|${level.scale}|${level.pixels}|${x}|${z}`
}

function cells(bounds: SeedMapTileBounds, level: SeedMapTileLevel, padding = 0): TileCell[] {
    const span = level.scale * level.pixels
    const result: TileCell[] = []
    for (
        let x = Math.floor(bounds.minX / span) - padding;
        x < Math.ceil(bounds.maxX / span) + padding;
        x++
    ) {
        for (
            let z = Math.floor(bounds.minZ / span) - padding;
            z < Math.ceil(bounds.maxZ / span) + padding;
            z++
        ) {
            result.push({ x: x * span, z: z * span, span })
        }
    }
    return result
}

function intersects(tile: TileCell, bounds: SeedMapTileBounds): boolean {
    return (
        tile.x < bounds.maxX &&
        tile.z < bounds.maxZ &&
        tile.x + tile.span > bounds.minX &&
        tile.z + tile.span > bounds.minZ
    )
}

/** Owns native tile jobs and bitmaps; changing LoD only changes drawing order and missing jobs. */
export function createSeedMapTileLayer(options: {
    render: (request: SeedMapTileRequest) => Promise<SeedMapTile>
    onUpdate: () => void
    onLoading: (loading: boolean) => void
    onCoverage: (epoch: number) => void
    onError: (message: string) => void
}) {
    const cache = new Map<string, CachedTile>()
    const inFlight = new Map<string, TileJob>()
    let queue: TileJob[] = []
    let view: SeedMapTileView | undefined
    let context = ''
    let coveredEpoch = -1
    let bytes = 0
    let disposed = false
    let active = false

    function updateState() {
        if (!view || disposed || !active) return
        const preview = seedMapPreviewLevel(view.level) ?? view.level
        const level = view.level
        const missing = cells(view.bounds, level).filter(
            (cell) => !cache.has(tileKey(context, level, cell.x, cell.z)),
        )
        if (
            coveredEpoch !== view.epoch &&
            missing.every((cell) => cache.has(tileKey(context, preview, cell.x, cell.z)))
        ) {
            coveredEpoch = view.epoch
            options.onCoverage(view.epoch)
        }
        options.onLoading(
            missing.length > 0 &&
                (queue.length > 0 || [...inFlight.values()].some((job) => job.context === context)),
        )
    }

    function invalidate(next?: SeedMapTileView) {
        active = false
        view = next
        context = next ? seedMapTileContextKey(next.context) : ''
        queue = []
        options.onLoading(false)
    }

    function refresh(next: SeedMapTileView) {
        if (disposed) return
        invalidate(next)
        active = true
        const distance = (cell: TileCell) =>
            (cell.x + cell.span / 2 - next.center.x) ** 2 +
            (cell.z + cell.span / 2 - next.center.z) ** 2
        const preview = seedMapPreviewLevel(next.level)
        const plans = [
            ...(preview ? [{ level: preview, padding: 0 }] : []),
            { level: next.level, padding: 0 },
            { level: next.level, padding: next.level.pixels === 256 ? 1 : 0 },
        ]
        const queued = new Set<string>()
        for (const { level, padding } of plans) {
            for (const cell of cells(next.bounds, level, padding).sort(
                (a, b) => distance(a) - distance(b),
            )) {
                const key = tileKey(context, level, cell.x, cell.z)
                if (cache.has(key) || inFlight.has(key) || queued.has(key)) continue
                if (level === preview && cache.has(tileKey(context, next.level, cell.x, cell.z)))
                    continue
                queued.add(key)
                queue.push({
                    ...cell,
                    key,
                    context,
                    request: {
                        ...next.context,
                        epoch: next.epoch,
                        x: cell.x,
                        z: cell.z,
                        scale: level.scale,
                        width: level.pixels,
                        height: level.pixels,
                    },
                })
            }
        }
        updateState()
        pump()
    }

    function prune() {
        if (!view || bytes <= CACHE_BYTES) return
        const level = view.level
        const preview = seedMapPreviewLevel(level)
        const visibleCells = cells(view.bounds, level).length
        const budget = Math.max(
            CACHE_BYTES,
            visibleCells * (level.pixels ** 2 * 2 + (preview?.pixels ?? 0) ** 2) * 4,
        )
        if (bytes <= budget) return
        const priority = (tile: CachedTile) => {
            if (tile.context !== context || !intersects(tile, view!.bounds)) return 0
            if (
                (tile.scale === level.scale && tile.pixels === level.pixels) ||
                (preview && tile.scale === preview.scale && tile.pixels === preview.pixels)
            )
                return 2
            return 1
        }
        const entries = [...cache.entries()].sort(([, a], [, b]) => priority(a) - priority(b))
        for (const [key, tile] of entries) {
            if (bytes <= budget) break
            cache.delete(key)
            bytes -= tile.pixels ** 2 * 4
            tile.bitmap.close()
        }
    }

    function pump() {
        if (!view || disposed || !active) return
        const limit = Math.max(1, Math.min(4, view.concurrency))
        while (queue.length > 0 && inFlight.size < limit) {
            const job = queue.shift()!
            inFlight.set(job.key, job)
            void run(job)
        }
        updateState()
    }

    async function run(job: TileJob) {
        try {
            const tile = await options.render(job.request)
            if (disposed || job.context !== context) {
                tile.bitmap.close()
                return
            }
            const previous = cache.get(job.key)
            if (previous) {
                previous.bitmap.close()
                bytes -= previous.pixels ** 2 * 4
            }
            cache.delete(job.key)
            cache.set(job.key, {
                x: job.x,
                z: job.z,
                span: job.span,
                scale: job.request.scale,
                pixels: job.request.width,
                bitmap: tile.bitmap,
                context: job.context,
            })
            bytes += job.request.width ** 2 * 4
            prune()
            options.onUpdate()
        } catch (error) {
            if (
                !disposed &&
                job.context === context &&
                view &&
                job.request.scale === view.level.scale &&
                job.request.width === view.level.pixels &&
                intersects(job, view.bounds)
            ) {
                options.onError(error instanceof Error ? error.message : String(error))
            }
        } finally {
            inFlight.delete(job.key)
            pump()
        }
    }

    function drawable(next: SeedMapTileView): SeedMapDrawableTile[] {
        const key = seedMapTileContextKey(next.context)
        const target = (tile: CachedTile) =>
            tile.scale === next.level.scale && tile.pixels === next.level.pixels
        const result = [...cache.values()].filter(
            (tile) => tile.context === key && intersects(tile, next.bounds),
        )
        result.sort((a, b) => Number(target(a)) - Number(target(b)) || b.scale - a.scale)
        return result
    }

    function dispose() {
        disposed = true
        queue = []
        view = undefined
        for (const tile of cache.values()) tile.bitmap.close()
        cache.clear()
        bytes = 0
    }

    function clear() {
        invalidate()
        for (const tile of cache.values()) tile.bitmap.close()
        cache.clear()
        bytes = 0
    }

    return { invalidate, refresh, drawable, dispose, clear }
}

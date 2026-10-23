import type { SeedMapDimension } from './biomes.ts'

/**
 * The seed map's engine output for one world, generated ahead of time with the
 * launcher's own engine (`axolotl-cubiomes`, see `meta.json`) and served as
 * static files: tile mosaics, biome rasters, structures, structure details and
 * loot, and ore scans for a window of chunks. Requests the files cannot answer
 * fail with a {@link SeedMapPreviewLimit}.
 */
const ROOT = '/launcher-preview/lab/seed-map/'

export type SeedMapPreviewLimitReason =
    'world' | 'area' | 'option' | 'ore' | 'import' | 'underground' | 'blocks'

const LIMIT_PREFIX = 'seed-map-preview-limit:'

/** A request outside the generated data; the page explains it and links the download. */
export class SeedMapPreviewLimit extends Error {
    constructor(readonly reason: SeedMapPreviewLimitReason) {
        super(`${LIMIT_PREFIX}${reason}`)
        this.name = 'SeedMapPreviewLimit'
    }
}

/** The limit a failure message stands for, as the page only keeps error messages. */
export function seedMapPreviewLimitReason(message: string): SeedMapPreviewLimitReason | null {
    return message.startsWith(LIMIT_PREFIX)
        ? (message.slice(LIMIT_PREFIX.length) as SeedMapPreviewLimitReason)
        : null
}

export type SeedMapPreviewMeta = {
    seed: string
    edition: string
    version: string
    spawn: { x: number; z: number; approximate: boolean }
}

export type SeedMapMosaic = {
    file: string
    context: SeedMapDimension
    scale: number
    pixels: number
    x: number
    z: number
    columns: number
    rows: number
    biomes?: string
    dimmed?: string
    ids?: string
}

type FeatureList = { extent: [number, number, number, number]; xz: number[]; endShip?: number[] }
export type SeedMapPreviewFeatures = {
    order: string[]
    lists: Record<string, FeatureList>
    strongholds?: number[]
    slime?: {
        extent: [number, number, number, number]
        chunkX: number
        chunkZ: number
        width: number
        height: number
        bits: string
    }
}

export type SeedMapPreviewDetails = {
    details: Record<string, unknown>
    loot: Record<string, ({ items: unknown[] } | { full: unknown })[]>
    unsupported: Record<string, unknown>
    extent: [number, number, number, number]
    denseExtent: [number, number, number, number]
}

export type SeedMapPreviewOres = {
    capacity: number
    dimensions: Record<string, { windows: [number, number, number, number][]; kinds: string[] }>
}

export type SeedMapPixels = { width: number; height: number; data: Uint8ClampedArray }

const cache = new Map<string, Promise<unknown>>()

function cached<T>(key: string, load: () => Promise<T>): Promise<T> {
    let entry = cache.get(key) as Promise<T> | undefined
    if (!entry) {
        entry = load()
        cache.set(key, entry)
        entry.catch(() => cache.delete(key))
    }
    return entry
}

async function fetchOk(path: string): Promise<Response> {
    const response = await fetch(ROOT + path)
    if (!response.ok) throw new Error(`Could not load the seed map data (${response.status}).`)
    return response
}

export function seedMapPreviewJson<T>(path: string): Promise<T> {
    return cached(path, async () => (await (await fetchOk(path)).json()) as T)
}

export function seedMapPreviewBytes(path: string): Promise<Uint8Array> {
    return cached(path, async () => new Uint8Array(await (await fetchOk(path)).arrayBuffer()))
}

const decodeOptions: ImageBitmapOptions = {
    colorSpaceConversion: 'none',
    premultiplyAlpha: 'none',
}

/** A mosaic image as decoded, without colour management. */
export function seedMapPreviewBitmap(path: string): Promise<ImageBitmap> {
    return cached(`bitmap:${path}`, async () =>
        createImageBitmap(await (await fetchOk(path)).blob(), decodeOptions),
    )
}

/** A mosaic's RGBA pixels, for tiles composed per pixel. */
export function seedMapPreviewPixels(path: string): Promise<SeedMapPixels> {
    return cached(`pixels:${path}`, async () => {
        const bitmap = await seedMapPreviewBitmap(path)
        const canvas = document.createElement('canvas')
        canvas.width = bitmap.width
        canvas.height = bitmap.height
        const context = canvas.getContext('2d', { willReadFrequently: true })
        if (!context) throw new Error('Could not read the seed map data.')
        context.drawImage(bitmap, 0, 0)
        const { data } = context.getImageData(0, 0, bitmap.width, bitmap.height)
        return { width: bitmap.width, height: bitmap.height, data }
    })
}

export const seedMapPreviewMeta = () => seedMapPreviewJson<SeedMapPreviewMeta>('meta.json')
export const seedMapPreviewMosaics = () => seedMapPreviewJson<SeedMapMosaic[]>('tiles.json')
export const seedMapPreviewPalette = () =>
    seedMapPreviewJson<Record<string, [number, number, number]>>('palette.json')
export const seedMapPreviewFeatures = (dimension: SeedMapDimension) =>
    seedMapPreviewJson<SeedMapPreviewFeatures>(`features-${dimension}.json`)
export const seedMapPreviewDetails = (dimension: SeedMapDimension) =>
    seedMapPreviewJson<SeedMapPreviewDetails>(`details-${dimension}.json`)
export const seedMapPreviewOres = () => seedMapPreviewJson<SeedMapPreviewOres>('ores.json')

/** `mod.rs` `parse_seed`: a 64-bit integer literal, otherwise Java's `String.hashCode`. */
export function parseSeedMapSeed(seed: string): bigint {
    const trimmed = seed.trim()
    if (/^[+-]?\d+$/.test(trimmed)) {
        const value = BigInt(trimmed)
        if (value >= -(2n ** 63n) && value < 2n ** 63n) return BigInt.asUintN(64, value)
    }
    let hash = 0
    for (let index = 0; index < trimmed.length; index++) {
        hash = (Math.imul(hash, 31) + trimmed.charCodeAt(index)) | 0
    }
    return BigInt.asUintN(64, BigInt(hash))
}

/** Requests for any other seed, edition or game version have no data here. */
export async function assertSeedMapPreviewWorld(seed: string, edition: string, version: string) {
    const meta = await seedMapPreviewMeta()
    if (
        edition !== meta.edition ||
        version !== meta.version ||
        parseSeedMapSeed(seed) !== parseSeedMapSeed(meta.seed)
    ) {
        throw new SeedMapPreviewLimit('world')
    }
    return meta
}

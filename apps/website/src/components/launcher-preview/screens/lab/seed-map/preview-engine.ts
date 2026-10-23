import type { SeedMapLoot, SeedMapStructureDetails, SeedMapStructureQuery } from './advanced.ts'
import type {
    SeedMapFeatureQuery,
    SeedMapOreScanChunk,
    SeedMapSpawnPoint,
    SeedMapTileRequest,
} from './backend.ts'
import type { SeedMapDimension } from './biomes.ts'
import { SEED_MAP_FEATURES, type SeedMapFeature, type SeedMapFeatureKind } from './features.ts'
import type { SeedMapOreHit, SeedMapOreKind } from './ores.ts'
import {
    assertSeedMapPreviewWorld,
    type SeedMapMosaic,
    seedMapPreviewBitmap,
    seedMapPreviewBytes,
    seedMapPreviewDetails,
    seedMapPreviewFeatures,
    SeedMapPreviewLimit,
    seedMapPreviewMosaics,
    seedMapPreviewOres,
    seedMapPreviewPalette,
    seedMapPreviewPixels,
} from './preview-data.ts'

/**
 * The seed map plugin's commands (`apps/app/src/seed_map`) answered from the
 * generated data, with the native results' order, caps and truncation.
 */

const ALL_FEATURES = SEED_MAP_FEATURES.reduce((mask, feature) => mask | feature.mask, 0)
const STRONGHOLD = SEED_MAP_FEATURES.find((feature) => feature.kind === 'stronghold')!.mask
const SLIME_CHUNK = SEED_MAP_FEATURES.find((feature) => feature.kind === 'slime-chunk')!.mask
const FEATURE_CAPACITY = 4096
const ORE_CAPACITY = 4096

const bitmapOptions: ImageBitmapOptions = {
    colorSpaceConversion: 'none',
    premultiplyAlpha: 'none',
}

/** `cubiomes_bridge.c`: unhighlighted pixels, before terrain shading. */
function dimChannel(dimension: SeedMapDimension, value: number) {
    return dimension === 'overworld' ? (77 * value) >> 8 : 255 - (((255 - value) * 51) >> 8)
}

async function biomeIds(mosaic: SeedMapMosaic, mosaics: SeedMapMosaic[]) {
    if (mosaic.ids) return { pixels: await seedMapPreviewPixels(mosaic.ids), x: 0, z: 0 }
    // The Overworld renderer samples the same points as the biome raster of its scale.
    const source = mosaics.find(
        (entry) => entry.context === mosaic.context && entry.scale === mosaic.scale && entry.biomes,
    )
    if (mosaic.context !== 'overworld' || !source?.biomes) throw new SeedMapPreviewLimit('area')
    return {
        pixels: await seedMapPreviewPixels(source.biomes),
        x: (mosaic.x - source.x) / mosaic.scale,
        z: (mosaic.z - source.z) / mosaic.scale,
    }
}

/** `render_tile`: one cell of a mosaic, composed per pixel for highlights and flat colours. */
export async function previewTileBitmap(request: SeedMapTileRequest): Promise<ImageBitmap> {
    await assertSeedMapPreviewWorld(request.seed, request.edition, request.version)
    if (Math.min(Math.max(request.elevation ?? 128, -64), 320) !== 128 || request.contours) {
        throw new SeedMapPreviewLimit('option')
    }
    const mosaics = await seedMapPreviewMosaics()
    const mosaic = mosaics.find(
        (entry) =>
            entry.context === request.dimension &&
            entry.scale === request.scale &&
            entry.pixels === request.width &&
            request.width === request.height,
    )
    if (!mosaic) throw new SeedMapPreviewLimit('area')
    const span = mosaic.scale * mosaic.pixels
    const column = (request.x - mosaic.x) / span
    const row = (request.z - mosaic.z) / span
    if (
        !Number.isInteger(column) ||
        !Number.isInteger(row) ||
        column < 0 ||
        row < 0 ||
        column >= mosaic.columns ||
        row >= mosaic.rows
    ) {
        throw new SeedMapPreviewLimit('area')
    }
    const size = mosaic.pixels
    const left = column * size
    const top = row * size
    // The Nether renderer has no terrain pass, so its mosaics are flat colours.
    const shaded = request.terrain && request.dimension !== 'nether'
    const highlight = request.highlightBiomes?.length ? new Set(request.highlightBiomes) : null
    if (!highlight && (shaded || request.dimension === 'nether')) {
        const bitmap = await seedMapPreviewBitmap(mosaic.file)
        return createImageBitmap(bitmap, left, top, size, size, bitmapOptions)
    }
    const ids = await biomeIds(mosaic, mosaics)
    const palette = await seedMapPreviewPalette()
    const normal = shaded ? await seedMapPreviewPixels(mosaic.file) : null
    const dimmed =
        shaded && highlight && mosaic.dimmed ? await seedMapPreviewPixels(mosaic.dimmed) : null
    if (shaded && highlight && !dimmed) throw new SeedMapPreviewLimit('option')
    const image = new ImageData(size, size)
    for (let y = 0; y < size; y++) {
        for (let x = 0; x < size; x++) {
            const id =
                ids.pixels.data[((ids.z + top + y) * ids.pixels.width + ids.x + left + x) * 4]
            const lit = !highlight || highlight.has(id)
            const target = (y * size + x) * 4
            if (normal) {
                const source = lit ? normal : dimmed!
                const index = ((top + y) * source.width + left + x) * 4
                image.data[target] = source.data[index]
                image.data[target + 1] = source.data[index + 1]
                image.data[target + 2] = source.data[index + 2]
            } else {
                const color = palette[id]
                if (!color) throw new SeedMapPreviewLimit('area')
                for (let channel = 0; channel < 3; channel++) {
                    image.data[target + channel] = lit
                        ? color[channel]
                        : dimChannel(request.dimension, color[channel])
                }
            }
            image.data[target + 3] = 255
        }
    }
    return createImageBitmap(image, bitmapOptions)
}

function covers(
    extent: [number, number, number, number],
    query: Pick<SeedMapFeatureQuery, 'minX' | 'minZ' | 'maxX' | 'maxZ'>,
) {
    return (
        query.minX >= extent[0] &&
        query.minZ >= extent[1] &&
        query.maxX <= extent[2] &&
        query.maxZ <= extent[3]
    )
}

/** `find_features`: structures in engine order, then strongholds, then slime chunks. */
export async function previewFeatures(query: SeedMapFeatureQuery): Promise<SeedMapFeature[]> {
    await assertSeedMapPreviewWorld(query.seed, query.edition, query.version)
    if (query.minX > query.maxX || query.minZ > query.maxZ) {
        throw new Error('The map bounds are invalid.')
    }
    const data = await seedMapPreviewFeatures(query.dimension)
    const mask = (query.featureMask ?? ALL_FEATURES) & ALL_FEATURES
    const inside = (x: number, z: number) =>
        x >= query.minX && x <= query.maxX && z >= query.minZ && z <= query.maxZ
    const result: SeedMapFeature[] = []
    for (const kind of data.order) {
        const definition = SEED_MAP_FEATURES.find((feature) => feature.kind === kind)
        const list = data.lists[kind]
        if (!definition || !(mask & definition.mask) || !list) continue
        if (!covers(list.extent, query)) throw new SeedMapPreviewLimit('area')
        for (let index = 0; index < list.xz.length; index += 2) {
            const x = list.xz[index]
            const z = list.xz[index + 1]
            if (!inside(x, z)) continue
            const feature: SeedMapFeature = {
                kind: kind as SeedMapFeatureKind,
                x,
                z,
                approximate: false,
            }
            if (list.endShip) feature.endShip = list.endShip[index / 2] === 1
            result.push(feature)
            if (result.length === FEATURE_CAPACITY) return result
        }
    }
    if (query.dimension === 'overworld' && mask & STRONGHOLD && data.strongholds) {
        for (let index = 0; index < data.strongholds.length; index += 2) {
            const x = data.strongholds[index]
            const z = data.strongholds[index + 1]
            if (!inside(x, z)) continue
            result.push({ kind: 'stronghold', x, z, approximate: false })
            if (result.length === FEATURE_CAPACITY) return result
        }
    }
    if (query.dimension === 'overworld' && mask & SLIME_CHUNK && data.slime) {
        const slime = data.slime
        if (!covers(slime.extent, query)) throw new SeedMapPreviewLimit('area')
        const bits = Uint8Array.from(atob(slime.bits), (char) => char.charCodeAt(0))
        for (let cx = Math.floor(query.minX / 16); cx <= Math.floor(query.maxX / 16); cx++) {
            for (let cz = Math.floor(query.minZ / 16); cz <= Math.floor(query.maxZ / 16); cz++) {
                const index = (cx - slime.chunkX) * slime.height + (cz - slime.chunkZ)
                if (!(bits[index >> 3] & (1 << (index & 7)))) continue
                result.push({
                    kind: 'slime-chunk',
                    x: cx * 16 + 8,
                    z: cz * 16 + 8,
                    approximate: false,
                })
                if (result.length === FEATURE_CAPACITY) return result
            }
        }
    }
    return result
}

export async function previewSpawn(
    seed: string,
    edition: string,
    version: string,
): Promise<SeedMapSpawnPoint> {
    const meta = await assertSeedMapPreviewWorld(seed, edition, version)
    return { ...meta.spawn }
}

/** `biome_at`, from the rasters sampled at each tile pixel's lookup point. */
export async function previewBiomeAt(query: {
    seed: string
    edition: string
    version: string
    dimension: SeedMapDimension
    x: number
    y: number
    z: number
}): Promise<number> {
    await assertSeedMapPreviewWorld(query.seed, query.edition, query.version)
    if (Math.min(Math.max(query.y, -64), 319) !== 128) throw new SeedMapPreviewLimit('option')
    const rasters = (await seedMapPreviewMosaics())
        .filter((entry) => entry.context === query.dimension && entry.biomes)
        .sort((a, b) => a.scale - b.scale)
    for (const raster of rasters) {
        const half = Math.floor(raster.scale / 2)
        const column = (query.x - half - raster.x) / raster.scale
        const row = (query.z - half - raster.z) / raster.scale
        const width = raster.columns * raster.pixels
        if (
            Number.isInteger(column) &&
            Number.isInteger(row) &&
            column >= 0 &&
            row >= 0 &&
            column < width &&
            row < raster.rows * raster.pixels
        ) {
            const pixels = await seedMapPreviewPixels(raster.biomes!)
            return pixels.data[(row * width + column) * 4]
        }
    }
    throw new SeedMapPreviewLimit('area')
}

type OreIndex = { bytes: DataView; offsets: Map<string, number> }
const oreIndexes = new Map<string, Promise<OreIndex>>()

function oreIndex(dimension: string, kind: string, windows: [number, number, number, number][]) {
    const key = `${dimension}-${kind}`
    let index = oreIndexes.get(key)
    if (!index) {
        index = seedMapPreviewBytes(`ores/${key}.bin`).then((data) => {
            const bytes = new DataView(data.buffer, data.byteOffset, data.byteLength)
            const offsets = new Map<string, number>()
            let offset = 0
            for (const [cx0, cz0, cx1, cz1] of windows) {
                for (let cx = cx0; cx <= cx1; cx++) {
                    for (let cz = cz0; cz <= cz1; cz++) {
                        offsets.set(`${cx}:${cz}`, offset)
                        offset += 2 + bytes.getUint16(offset, true) * 4
                    }
                }
            }
            return { bytes, offsets }
        })
        oreIndexes.set(key, index)
        index.catch(() => oreIndexes.delete(key))
    }
    return index
}

/** `scan_ores`: each kind's hits up to the shared 4096 budget, deduplicated per chunk. */
export async function previewScanOres(request: {
    seed: string
    edition: string
    version: string
    dimension: SeedMapDimension
    ores: SeedMapOreKind[]
    chunks: number[]
}): Promise<SeedMapOreScanChunk[]> {
    await assertSeedMapPreviewWorld(request.seed, request.edition, request.version)
    if (request.dimension === 'end') {
        throw new Error('Ore prediction supports Java 1.13+ Overworld and Nether maps.')
    }
    if (
        request.chunks.length % 2 !== 0 ||
        request.chunks.length > 192 ||
        request.ores.length > 25 ||
        request.chunks.some((coordinate) => Math.abs(coordinate) > 1_875_000)
    ) {
        throw new Error('The ore scan chunk list is invalid.')
    }
    const data = (await seedMapPreviewOres()).dimensions[request.dimension]
    const kinds = [...new Set(request.ores)]
    if (!data || kinds.some((kind) => !data.kinds.includes(kind))) {
        throw new SeedMapPreviewLimit('ore')
    }
    const indexes = await Promise.all(
        kinds.map((kind) => oreIndex(request.dimension, kind, data.windows)),
    )
    const capacity = Math.min(
        Math.max(Math.floor(ORE_CAPACITY / Math.max(kinds.length, 1)), 1),
        16_384,
    )
    const results: SeedMapOreScanChunk[] = []
    for (let index = 0; index < request.chunks.length; index += 2) {
        const cx = request.chunks[index]
        const cz = request.chunks[index + 1]
        const hits: SeedMapOreHit[] = []
        const seen = new Set<string>()
        let truncated = false
        kinds.forEach((kind, kindIndex) => {
            const { bytes, offsets } = indexes[kindIndex]
            const offset = offsets.get(`${cx}:${cz}`)
            if (offset === undefined) throw new SeedMapPreviewLimit('area')
            const count = bytes.getUint16(offset, true)
            truncated ||= count >= capacity
            for (let hit = 0; hit < Math.min(count, capacity); hit++) {
                const at = offset + 2 + hit * 4
                const x = cx * 16 + bytes.getInt8(at)
                const z = cz * 16 + bytes.getInt8(at + 1)
                const y = bytes.getInt16(at + 2, true)
                const key = `${kind}:${x}:${y}:${z}`
                if (seen.has(key)) continue
                seen.add(key)
                hits.push({ ore: kind, x, y, z, yMin: y, yMax: y, status: 'terrain_unverified' })
            }
        })
        results.push({ cx, cz, hits, truncated })
    }
    return results
}

function structureKind(featureMask: number) {
    return SEED_MAP_FEATURES.find((feature) => feature.mask === featureMask)?.kind
}

/** `structure_details`, for the structures inside the generated detail extents. */
export async function previewStructureDetails(
    query: SeedMapStructureQuery,
): Promise<SeedMapStructureDetails> {
    await assertSeedMapPreviewWorld(query.seed, query.edition, query.version)
    // Template blocks are read from the app's structure resources.
    if (query.includeBlocks) throw new SeedMapPreviewLimit('blocks')
    const kind = structureKind(query.featureMask)
    const data = await seedMapPreviewDetails(query.dimension)
    const value = kind ? data.details[`${kind}:${query.x}:${query.z}`] : undefined
    if (value === 0 && kind)
        return structuredClone(data.unsupported[kind]) as SeedMapStructureDetails
    if (!value) throw new SeedMapPreviewLimit('area')
    return structuredClone(value) as SeedMapStructureDetails
}

/** `container_loot`: the details without pieces or containers, plus the container's items. */
export async function previewContainerLoot(
    query: SeedMapStructureQuery,
    container: number,
): Promise<SeedMapLoot> {
    const details = await previewStructureDetails({ ...query, includeBlocks: false })
    const kind = structureKind(query.featureMask)
    const data = await seedMapPreviewDetails(query.dimension)
    const entry = data.loot[`${kind}:${query.x}:${query.z}`]?.[container]
    if (!entry) throw new SeedMapPreviewLimit('area')
    if ('full' in entry) return structuredClone(entry.full) as SeedMapLoot
    return {
        ...(details as unknown as SeedMapLoot),
        pieces: [],
        containers: [],
        items: structuredClone(entry.items),
    } as SeedMapLoot
}

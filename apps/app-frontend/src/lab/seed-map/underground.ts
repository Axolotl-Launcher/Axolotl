import { invoke } from '@tauri-apps/api/core'

import type { SchematicMeshData } from '../schematic-preview/mesh-worker'
import type { SeedMapEdition } from './backend'
import type { SeedMapDimension } from './biomes'

export type UndergroundQuery = {
    seed: string
    edition: SeedMapEdition
    version: string
    dimension: SeedMapDimension
    x: number
    y: number
    z: number
}
export type UndergroundLayer = 'terrain' | 'surface' | 'bedrock' | 'water' | 'lava' | 'ores'
export type UndergroundVolume = {
    blocks: Uint8Array
    width: number
    height: number
    truncated: boolean
    surface?: Int16Array
}
export type UndergroundMesh = {
    layers: Record<UndergroundLayer, SchematicMeshData>
    spawn: [number, number, number]
    faces: number
    noAir: boolean
}
export type UndergroundMeshRequest = UndergroundVolume & {
    textureUvs: Record<string, [number, number, number, number]>
    surfaceOffset?: number
    nether?: boolean
    focusY?: number
}

export const UNDERGROUND_RADIUS = 12
export const UNDERGROUND_MEMORY_BUDGET = 512 * 1024 * 1024
export function undergroundChunkKey(position: [number, number, number]) {
    return position.join(':')
}
export function undergroundChunkQueue(query: UndergroundQuery, position: [number, number, number]) {
    const cx = Math.max(-1_875_000, Math.min(1_874_999, Math.floor(position[0] / 16))),
        cz = Math.max(-1_875_000, Math.min(1_874_999, Math.floor(position[2] / 16)))
    const chunks: [number, number, number][] = []
    const modern =
        Number(query.version.split('.')[0]) >= 26 || /^1\.(?:1[89]|2\d)(?:\.|$)/.test(query.version)
    const chunkY = query.dimension === 'overworld' && modern ? -64 : 0
    for (let dx = -UNDERGROUND_RADIUS; dx <= UNDERGROUND_RADIUS; dx++)
        for (let dz = -UNDERGROUND_RADIUS; dz <= UNDERGROUND_RADIUS; dz++) {
            if (dx * dx + dz * dz > UNDERGROUND_RADIUS ** 2) continue
            const x = (cx + dx) * 16,
                z = (cz + dz) * 16
            if (x < -30_000_000 || x > 29_999_984 || z < -30_000_000 || z > 29_999_984) continue
            chunks.push([x, chunkY, z])
        }
    return chunks.sort(
        (a, b) =>
            (a[0] + 8 - position[0]) ** 2 +
                (a[2] + 8 - position[2]) ** 2 -
                (b[0] + 8 - position[0]) ** 2 -
                (b[2] + 8 - position[2]) ** 2 ||
            a[0] - b[0] ||
            a[2] - b[2],
    )
}

export function undergroundEviction(
    queue: [number, number, number][],
    loaded: { position: [number, number, number] }[],
    next: [number, number, number],
): [number, number, number] | undefined {
    const ranks = new Map(queue.map((position, index) => [undergroundChunkKey(position), index]))
    const rank = (position: [number, number, number]) =>
        ranks.get(undergroundChunkKey(position)) ?? Infinity
    const farthest = loaded.reduce<{ position: [number, number, number] } | undefined>(
        (result, value) =>
            !result || rank(value.position) > rank(result.position) ? value : result,
        undefined,
    )
    return farthest && rank(next) < rank(farthest.position) ? farthest.position : undefined
}

export function undergroundOrigin(query: UndergroundQuery): [number, number, number] {
    const modern =
        Number(query.version.split('.')[0]) >= 26 || /^1\.(?:1[89]|2\d)(?:\.|$)/.test(query.version)
    const min = query.dimension === 'overworld' && modern ? -64 : 0
    const max = query.dimension === 'nether' ? 128 : modern ? 320 : 256
    const horizontal = (value: number) =>
        Math.max(-30_000_000, Math.min(29_999_936, Math.floor((value - 32) / 16) * 16))
    return [
        horizontal(query.x),
        Math.max(min, Math.min(max - 64, Math.floor((query.y - 32) / 8) * 8)),
        horizontal(query.z),
    ]
}

export function decodeUndergroundVolume(buffer: ArrayBuffer): UndergroundVolume {
    if (buffer.byteLength < 16) throw new Error('Invalid underground volume header')
    const header = new DataView(buffer)
    const width = header.getUint32(8, true)
    const height = header.getUint32(12, true)
    const schema = header.getUint32(0, true)
    const length = width * width * height
    if (
        height < 64 ||
        height > 384 ||
        height % 8 !== 0 ||
        !(
            (schema === 1 && width === 64 && buffer.byteLength === 16 + length) ||
            (schema === 2 &&
                width === 18 &&
                height === 64 &&
                buffer.byteLength === 16 + length + width * width * 2) ||
            (schema === 3 && width === 18 && buffer.byteLength === 16 + length + width * width * 2)
        )
    ) {
        throw new Error('Unsupported underground volume format')
    }
    return {
        blocks: new Uint8Array(buffer, 16, length),
        width,
        height,
        truncated: Boolean(header.getUint32(4, true) & 1),
        surface:
            schema === 2 || schema === 3
                ? Int16Array.from({ length: width * width }, (_, index) =>
                      header.getInt16(16 + length + index * 2, true),
                  )
                : undefined,
    }
}

export async function loadUndergroundVolume(query: UndergroundQuery, id: number) {
    return decodeUndergroundVolume(
        await invoke<ArrayBuffer>('plugin:seed-map|seed_map_underground_volume', {
            request: { ...query, id, chunk: true },
        }),
    )
}

export function cancelUndergroundVolume(id: number) {
    return invoke('plugin:seed-map|seed_map_cancel_underground', { id })
}

import { expect, it } from 'vitest'

import {
    decodeUndergroundVolume,
    undergroundChunkQueue,
    undergroundEviction,
    undergroundOrigin,
} from './underground.ts'
import { buildUndergroundMesh, undergroundTexture } from './underground-mesh.ts'

const textureUvs: Record<string, [number, number, number, number]> = Object.fromEntries(
    ['stone', 'deepslate', 'netherrack', 'water_still', 'lava_still', 'bedrock', 'diamond_ore'].map(
        (id) => [`minecraft:block/${id}`, [0, 0, 1, 1]],
    ),
)

it('X-Ray hides all solid rock except faces bounding cavities and omits artificial region cut walls', () => {
    const blocks = new Uint8Array(5 * 5 * 5).fill(1)
    const request = { blocks, width: 5, height: 5, truncated: false, textureUvs }
    expect(buildUndergroundMesh(request).faces).toBe(0)
    blocks[(2 * 5 + 2) * 5 + 2] = 0
    const mesh = buildUndergroundMesh(request)
    expect(mesh.faces).toBe(6)
    expect(mesh.layers.terrain.positions.length).toBe(6 * 6 * 3)
    expect(mesh.layers.ores.positions.length).toBe(0)
    for (let index = 0; index < mesh.layers.terrain.positions.length; index += 9) {
        const p = mesh.layers.terrain.positions.slice(index, index + 9)
        const a = [p[3] - p[0], p[4] - p[1], p[5] - p[2]]
        const b = [p[6] - p[0], p[7] - p[1], p[8] - p[2]]
        const normal = [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
        expect(normal.map((value) => value || 0)).toEqual(
            Array.from(mesh.layers.terrain.normals.slice(index, index + 3)),
        )
    }
})

it('minerals remain independent of invisible host rock and fluids have separate textured surfaces', () => {
    const blocks = new Uint8Array(5 * 5 * 5).fill(1)
    blocks[(2 * 5 + 2) * 5 + 2] = 16
    const mesh = buildUndergroundMesh({ blocks, width: 5, height: 5, truncated: false, textureUvs })
    expect(mesh.layers.ores.positions.length).toBe(6 * 6 * 3)
    expect(mesh.layers.terrain.positions.length).toBe(0)
    blocks.fill(0)
    blocks[(2 * 5 + 2) * 5 + 2] = 4
    blocks[(3 * 5 + 2) * 5 + 2] = 4
    const fluids = buildUndergroundMesh({
        blocks,
        width: 5,
        height: 5,
        truncated: false,
        textureUvs,
    })
    expect(fluids.faces).toBe(10)
    expect(fluids.layers.water.positions.length).toBe(10 * 6 * 3)
    expect(fluids.layers.lava.positions.length).toBe(0)
    expect(undergroundTexture(80, 0)).toBe('minecraft:block/deepslate_diamond_ore')
    expect(undergroundTexture(25, 1)).toBe('minecraft:block/ancient_debris_top')
    expect(undergroundTexture(2, 1)).toBe('minecraft:block/deepslate_top')
    expect(undergroundTexture(6, 0)).toBe('minecraft:block/bedrock')
    const bedrockBlocks = new Uint8Array(5 * 5 * 5)
    bedrockBlocks[(2 * 5 + 2) * 5 + 2] = 6
    const bedrock = buildUndergroundMesh({
        blocks: bedrockBlocks,
        width: 5,
        height: 5,
        truncated: false,
        textureUvs,
    })
    expect(bedrock.layers.bedrock.positions.length).toBe(6 * 6 * 3)
    expect(bedrock.layers.terrain.positions.length).toBe(0)
})

it('underground origins honor negative coordinates, legacy heights and world boundaries', () => {
    const query = {
        seed: '1',
        edition: 'java' as const,
        version: '26.3',
        dimension: 'overworld' as const,
        x: -1,
        y: -63,
        z: -17,
    }
    expect(undergroundOrigin(query)).toEqual([-48, -64, -64])
    expect(undergroundOrigin({ ...query, x: 30_000_000, y: 319, z: -30_000_000 })).toEqual([
        29_999_936, 256, -30_000_000,
    ])
    expect(undergroundOrigin({ ...query, version: '1.17.1' })[1]).toBe(0)
    expect(undergroundOrigin({ ...query, dimension: 'nether', y: 127 })[1]).toBe(64)
})

it('volume decoder rejects incompatible data and retains truncation status', () => {
    expect(() => decodeUndergroundVolume(new ArrayBuffer(16))).toThrow()
    const buffer = new ArrayBuffer(16 + 64 ** 3)
    const header = new DataView(buffer)
    ;[1, 1, 64, 64].forEach((value, index) => header.setUint32(index * 4, value, true))
    expect(decodeUndergroundVolume(buffer).truncated).toBe(true)
    expect(decodeUndergroundVolume(buffer).blocks.length).toBe(64 ** 3)
})

it('idle prefetch is nearest-first, radius bounded and handles negative coordinates and height limits', () => {
    const query = {
        seed: '1',
        edition: 'java' as const,
        version: '26.3',
        dimension: 'overworld' as const,
        x: 0,
        y: 16,
        z: 0,
    }
    const queue = undergroundChunkQueue(query, [-1, 16, -17])
    expect(queue[0]).toEqual([-16, -64, -32])
    expect(queue.length).toBe(441)
    for (const [x, y, z] of queue) {
        expect((x / 16 + 1) ** 2 + (z / 16 + 2) ** 2 <= 144).toBeTruthy()
        expect(y).toBe(-64)
    }
    const boundary = undergroundChunkQueue(query, [-30_000_000, -64, -30_000_000])
    expect(
        boundary.every(([x, y, z]) => x >= -30_000_000 && z >= -30_000_000 && y === -64),
    ).toBeTruthy()
    expect(undergroundChunkQueue({ ...query, dimension: 'nether' }, [0, 127, 0])[0][1]).toBe(0)
    expect(undergroundChunkQueue(query, [0, -64, 0])[0][1]).toBe(-64)
    expect(undergroundChunkQueue(query, [0, 319, 0])[0][1]).toBe(-64)
    expect(undergroundChunkQueue(query, [0, -32, 0])).toEqual(
        undergroundChunkQueue(query, [0, 96, 0]),
    )
})

it('a padded chunk draws one surface block per column above the underground range and culls adjoining cave walls', () => {
    const width = 18,
        height = 384
    const blocks = new Uint8Array(width * width * height).fill(1)
    const surface = new Int16Array(width * width).fill(100)
    const mesh = buildUndergroundMesh({
        blocks,
        width,
        height,
        surface,
        truncated: false,
        textureUvs,
        surfaceOffset: -64,
    })
    expect(mesh.layers.terrain.positions.length).toBe(0)
    expect(mesh.layers.surface.positions.length).toBe(16 * 16 * 2 * 6 * 3)
    expect(
        mesh.layers.surface.positions
            .filter((_, index) => index % 3 === 1)
            .every((y) => y === 164 || y === 165),
    ).toBeTruthy()
    blocks[(0 * width + 8) * height + 32] = 0
    const cave = buildUndergroundMesh({
        blocks,
        width,
        height,
        surface,
        truncated: false,
        textureUvs,
    })
    expect(cave.layers.terrain.positions.length).toBe(6 * 3)
})

it('full-height chunks decode padded columns and signed surface heights', () => {
    const buffer = new ArrayBuffer(16 + 18 * 18 * 384 + 18 * 18 * 2)
    const view = new DataView(buffer)
    ;[3, 0, 18, 384].forEach((value, index) => view.setUint32(index * 4, value, true))
    view.setInt16(16 + 18 * 18 * 384, -32, true)
    const result = decodeUndergroundVolume(buffer)
    expect(result.surface?.[0]).toBe(-32)
    expect(result.blocks.length).toBe(18 * 18 * 384)
})

it('one full-height chunk contains cave geometry below and above world Y=0 at the same time', () => {
    const width = 18,
        height = 384
    const blocks = new Uint8Array(width * width * height).fill(1)
    const surface = new Int16Array(width * width).fill(-32768)
    blocks[(8 * width + 8) * height + 20] = 0
    blocks[(8 * width + 8) * height + 160] = 0
    const mesh = buildUndergroundMesh({
        blocks,
        width,
        height,
        surface,
        surfaceOffset: -64,
        truncated: false,
        textureUvs,
    })
    const worldYs = mesh.layers.terrain.positions
        .filter((_, index) => index % 3 === 1)
        .map((y) => y - 64)
    expect(worldYs.some((y) => y < 0)).toBeTruthy()
    expect(worldYs.some((y) => y > 0)).toBeTruthy()
})

it('memory eviction uses stable queue priority and does not churn equally distant chunks', () => {
    const query = {
        seed: '1',
        edition: 'java' as const,
        version: '26.3',
        dimension: 'overworld' as const,
        x: 8,
        y: 32,
        z: 8,
    }
    const queue = undergroundChunkQueue(query, [8, 32, 8])
    const loaded = queue.slice(0, 3).map((position) => ({ position }))
    expect(undergroundEviction(queue, loaded, queue[3])).toBe(undefined)
    expect(
        undergroundEviction(
            queue,
            queue.slice(1, 4).map((position) => ({ position })),
            queue[0],
        ),
    ).toEqual(queue[3])
    const vertical: [number, number, number] = [queue[0][0], queue[0][1] + 32, queue[0][2]]
    const adjacentLayer: [number, number, number] = [queue[0][0], queue[0][1] + 64, queue[0][2]]
    expect(undergroundEviction([adjacentLayer], [{ position: queue[0] }], adjacentLayer)).toEqual(
        queue[0],
    )
    expect(vertical).not.toEqual(adjacentLayer)
})

import assert from 'node:assert/strict'
import test from 'node:test'

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

test('X-Ray hides all solid rock except faces bounding cavities and omits artificial region cut walls', () => {
    const blocks = new Uint8Array(5 * 5 * 5).fill(1)
    const request = { blocks, width: 5, height: 5, truncated: false, textureUvs }
    assert.equal(buildUndergroundMesh(request).faces, 0)
    blocks[(2 * 5 + 2) * 5 + 2] = 0
    const mesh = buildUndergroundMesh(request)
    assert.equal(mesh.faces, 6)
    assert.equal(mesh.layers.terrain.positions.length, 6 * 6 * 3)
    assert.equal(mesh.layers.ores.positions.length, 0)
    for (let index = 0; index < mesh.layers.terrain.positions.length; index += 9) {
        const p = mesh.layers.terrain.positions.slice(index, index + 9)
        const a = [p[3] - p[0], p[4] - p[1], p[5] - p[2]]
        const b = [p[6] - p[0], p[7] - p[1], p[8] - p[2]]
        const normal = [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
        assert.deepEqual(
            normal.map((value) => value || 0),
            Array.from(mesh.layers.terrain.normals.slice(index, index + 3)),
        )
    }
})

test('minerals remain independent of invisible host rock and fluids have separate textured surfaces', () => {
    const blocks = new Uint8Array(5 * 5 * 5).fill(1)
    blocks[(2 * 5 + 2) * 5 + 2] = 16
    const mesh = buildUndergroundMesh({ blocks, width: 5, height: 5, truncated: false, textureUvs })
    assert.equal(mesh.layers.ores.positions.length, 6 * 6 * 3)
    assert.equal(mesh.layers.terrain.positions.length, 0)
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
    assert.equal(fluids.faces, 10)
    assert.equal(fluids.layers.water.positions.length, 10 * 6 * 3)
    assert.equal(fluids.layers.lava.positions.length, 0)
    assert.equal(undergroundTexture(80, 0), 'minecraft:block/deepslate_diamond_ore')
    assert.equal(undergroundTexture(25, 1), 'minecraft:block/ancient_debris_top')
    assert.equal(undergroundTexture(2, 1), 'minecraft:block/deepslate_top')
    assert.equal(undergroundTexture(6, 0), 'minecraft:block/bedrock')
    const bedrockBlocks = new Uint8Array(5 * 5 * 5)
    bedrockBlocks[(2 * 5 + 2) * 5 + 2] = 6
    const bedrock = buildUndergroundMesh({
        blocks: bedrockBlocks,
        width: 5,
        height: 5,
        truncated: false,
        textureUvs,
    })
    assert.equal(bedrock.layers.bedrock.positions.length, 6 * 6 * 3)
    assert.equal(bedrock.layers.terrain.positions.length, 0)
})

test('underground origins honor negative coordinates, legacy heights and world boundaries', () => {
    const query = {
        seed: '1',
        edition: 'java' as const,
        version: '26.3',
        dimension: 'overworld' as const,
        x: -1,
        y: -63,
        z: -17,
    }
    assert.deepEqual(undergroundOrigin(query), [-48, -64, -64])
    assert.deepEqual(
        undergroundOrigin({ ...query, x: 30_000_000, y: 319, z: -30_000_000 }),
        [29_999_936, 256, -30_000_000],
    )
    assert.equal(undergroundOrigin({ ...query, version: '1.17.1' })[1], 0)
    assert.equal(undergroundOrigin({ ...query, dimension: 'nether', y: 127 })[1], 64)
})

test('volume decoder rejects incompatible data and retains truncation status', () => {
    assert.throws(() => decodeUndergroundVolume(new ArrayBuffer(16)))
    const buffer = new ArrayBuffer(16 + 64 ** 3)
    const header = new DataView(buffer)
    ;[1, 1, 64, 64].forEach((value, index) => header.setUint32(index * 4, value, true))
    assert.equal(decodeUndergroundVolume(buffer).truncated, true)
    assert.equal(decodeUndergroundVolume(buffer).blocks.length, 64 ** 3)
})

test('idle prefetch is nearest-first, radius bounded and handles negative coordinates and height limits', () => {
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
    assert.deepEqual(queue[0], [-16, -64, -32])
    assert.equal(queue.length, 441)
    for (const [x, y, z] of queue) {
        assert.ok((x / 16 + 1) ** 2 + (z / 16 + 2) ** 2 <= 144)
        assert.equal(y, -64)
    }
    const boundary = undergroundChunkQueue(query, [-30_000_000, -64, -30_000_000])
    assert.ok(boundary.every(([x, y, z]) => x >= -30_000_000 && z >= -30_000_000 && y === -64))
    assert.equal(undergroundChunkQueue({ ...query, dimension: 'nether' }, [0, 127, 0])[0][1], 0)
    assert.equal(undergroundChunkQueue(query, [0, -64, 0])[0][1], -64)
    assert.equal(undergroundChunkQueue(query, [0, 319, 0])[0][1], -64)
    assert.deepEqual(
        undergroundChunkQueue(query, [0, -32, 0]),
        undergroundChunkQueue(query, [0, 96, 0]),
    )
})

test('a padded chunk draws one surface block per column above the underground range and culls adjoining cave walls', () => {
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
    assert.equal(mesh.layers.terrain.positions.length, 0)
    assert.equal(mesh.layers.surface.positions.length, 16 * 16 * 2 * 6 * 3)
    assert.ok(
        mesh.layers.surface.positions
            .filter((_, index) => index % 3 === 1)
            .every((y) => y === 164 || y === 165),
    )
    blocks[(0 * width + 8) * height + 32] = 0
    const cave = buildUndergroundMesh({
        blocks,
        width,
        height,
        surface,
        truncated: false,
        textureUvs,
    })
    assert.equal(cave.layers.terrain.positions.length, 6 * 3)
})

test('full-height chunks decode padded columns and signed surface heights', () => {
    const buffer = new ArrayBuffer(16 + 18 * 18 * 384 + 18 * 18 * 2)
    const view = new DataView(buffer)
    ;[3, 0, 18, 384].forEach((value, index) => view.setUint32(index * 4, value, true))
    view.setInt16(16 + 18 * 18 * 384, -32, true)
    const result = decodeUndergroundVolume(buffer)
    assert.equal(result.surface?.[0], -32)
    assert.equal(result.blocks.length, 18 * 18 * 384)
})

test('one full-height chunk contains cave geometry below and above world Y=0 at the same time', () => {
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
    assert.ok(worldYs.some((y) => y < 0))
    assert.ok(worldYs.some((y) => y > 0))
})

test('memory eviction uses stable queue priority and does not churn equally distant chunks', () => {
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
    assert.equal(undergroundEviction(queue, loaded, queue[3]), undefined)
    assert.deepEqual(
        undergroundEviction(
            queue,
            queue.slice(1, 4).map((position) => ({ position })),
            queue[0],
        ),
        queue[3],
    )
    const vertical: [number, number, number] = [queue[0][0], queue[0][1] + 32, queue[0][2]]
    const adjacentLayer: [number, number, number] = [queue[0][0], queue[0][1] + 64, queue[0][2]]
    assert.deepEqual(
        undergroundEviction([adjacentLayer], [{ position: queue[0] }], adjacentLayer),
        queue[0],
    )
    assert.notDeepEqual(vertical, adjacentLayer)
})

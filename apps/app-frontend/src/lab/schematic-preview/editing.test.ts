import { expect, it } from 'vitest'

import {
    filterSchematicAirGeometry,
    isSchematicAir,
    measureSchematicPoints,
    normalizeSchematicAirBlocks,
    schematicBlockPaletteIndex,
    schematicBlockStateKey,
    type SchematicCachedChunk,
    schematicChunkKey,
    selectConnectedSchematicBlocks,
    selectSchematicBlockState,
    selectSchematicCuboid,
    selectSchematicLayer,
    selectSchematicMaterial,
} from './editing.ts'

it('air block detection tolerates noncanonical casing and whitespace', () => {
    expect(isSchematicAir(' Minecraft:Air ')).toBe(true)
    expect(isSchematicAir('minecraft:CAVE_AIR')).toBe(true)
    expect(isSchematicAir('minecraft:void_air')).toBe(true)
    expect(isSchematicAir('minecraft:light')).toBe(true)
    expect(isSchematicAir('minecraft:barrier')).toBe(true)
    expect(isSchematicAir('minecraft:structure_void')).toBe(true)
    expect(isSchematicAir('minecraft:stone')).toBe(false)
})

it('chunk normalization collapses every air palette entry to index zero', () => {
    const blocks = new Uint32Array([1, 2, 3, 2])
    const normalized = normalizeSchematicAirBlocks(blocks, [
        { name: 'minecraft:air', properties: {} },
        { name: 'minecraft:stone', properties: {} },
        { name: ' Minecraft:CAVE_AIR ', properties: {} },
        { name: 'minecraft:void_air', properties: {} },
        { name: 'minecraft:barrier', properties: {} },
    ])

    expect([...normalized]).toEqual([1, 0, 0, 0])
    expect([...blocks]).toEqual([1, 2, 3, 2])
})

it('mesh validation removes stale geometry whose cached block is air', () => {
    const chunk: SchematicCachedChunk = {
        regionId: 'region-0',
        position: [0, 0, 0],
        blocks: new Uint32Array(4096),
    }
    chunk.blocks[1] = 1
    const mesh = {
        positions: new Float32Array([0, 0, 0, 1, 0, 0]),
        normals: new Float32Array([0, 1, 0, 0, 1, 0]),
        uvs: new Float32Array([0, 0, 1, 0]),
        colors: new Float32Array([1, 1, 1, 1, 1, 1]),
        blockPositions: new Float32Array([0, 0, 0, 1, 0, 0]),
    }
    const filtered = filterSchematicAirGeometry(mesh, chunk, [
        { name: 'minecraft:air', properties: {} },
        { name: 'minecraft:stone', properties: {} },
    ])

    expect([...filtered.blockPositions]).toEqual([1, 0, 0])
    expect([...filtered.positions]).toEqual([1, 0, 0])
})

it('point measurement reports signed offsets, inclusive size, and distance', () => {
    const measurement = measureSchematicPoints([4, -2, 10], [-2, 1, 2])
    expect(measurement.delta).toEqual([-6, 3, -8])
    expect(measurement.size).toEqual([7, 4, 9])
    expect(measurement.distance).toBe(Math.sqrt(109))
})

function fixture() {
    const blocks = new Uint32Array(4096)
    blocks[0] = 1
    blocks[1] = 1
    blocks[16] = 2
    blocks[256] = 2
    const chunk: SchematicCachedChunk = { regionId: 'region-0', position: [-1, 0, 0], blocks }
    return new Map([[schematicChunkKey(chunk.regionId, chunk.position), chunk]])
}

it('palette lookup handles negative chunk coordinates', () => {
    expect(
        schematicBlockPaletteIndex(fixture(), { regionId: 'region-0', position: [-16, 0, 0] }),
    ).toBe(1)
})

it('block state keys preserve properties for states sharing a block name', () => {
    expect(
        schematicBlockStateKey({ name: 'minecraft:oak_slab', properties: { type: 'top' } }),
    ).not.toBe(
        schematicBlockStateKey({ name: 'minecraft:oak_slab', properties: { type: 'bottom' } }),
    )
    expect(
        schematicBlockStateKey({
            name: 'minecraft:oak_slab',
            properties: { waterlogged: 'false', type: 'top' },
        }),
    ).toBe('minecraft:oak_slab[type=top,waterlogged=false]')
})

it('cuboid selection skips air and keeps the selected region', () => {
    const selected = selectSchematicCuboid(
        fixture(),
        { regionId: 'region-0', position: [-16, 0, 0] },
        { regionId: 'region-0', position: [-15, 1, 1] },
    )
    expect(selected.length).toBe(4)
    expect(selected.every((location) => location.regionId === 'region-0')).toBeTruthy()
})

it('material, layer, and connected expansion use cached block data', () => {
    const chunks = fixture()
    const palette = [
        { name: 'minecraft:air', properties: {} },
        { name: 'minecraft:stone', properties: {} },
        { name: 'minecraft:dirt', properties: {} },
    ]
    expect(selectSchematicMaterial(chunks, palette, 'minecraft:dirt').length).toBe(2)
    const stateChunks = fixture()
    stateChunks.get(schematicChunkKey('region-0', [-1, 0, 0]))!.blocks[2] = 3
    expect(
        selectSchematicBlockState(
            stateChunks,
            [...palette, { name: 'minecraft:dirt', properties: { type: 'top' } }],
            { name: 'minecraft:dirt', properties: {} },
        ).length,
    ).toBe(2)
    expect(selectSchematicLayer(chunks, 0).length).toBe(3)
    expect(
        selectConnectedSchematicBlocks(chunks, {
            regionId: 'region-0',
            position: [-16, 0, 0],
        }).length,
    ).toBe(4)
})

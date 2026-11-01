import { Cull, type Identifier } from 'deepslate'
import { expect, it } from 'vitest'

import { createBlockGeometry } from './block-geometry.ts'

const cubeFaces = {
    down: { texture: '#all', cullface: 'down' },
    up: { texture: '#all', cullface: 'up' },
    north: { texture: '#all', cullface: 'north' },
    south: { texture: '#all', cullface: 'south' },
    west: { texture: '#all', cullface: 'west' },
    east: { texture: '#all', cullface: 'east' },
}

const resources = {
    blockDefinitions: {
        'minecraft:oak_leaves': { variants: { '': { model: 'minecraft:block/leaves' } } },
    },
    blockModels: {
        'minecraft:block/leaves': {
            textures: { all: 'minecraft:block/oak_leaves' },
            elements: [{ from: [0, 0, 0], to: [16, 16, 16], faces: cubeFaces }],
        },
    },
    defaultBlockProperties: {
        'minecraft:oak_leaves': { distance: '7', persistent: 'false', waterlogged: 'false' },
    },
}

function trackingAtlas(requested: Set<string>) {
    return {
        getTextureAtlas: () => ({}) as ImageData,
        getTextureUV: (id: Identifier) => {
            requested.add(id.toString())
            return [0, 0, 1, 1] as [number, number, number, number]
        },
    }
}

function leaves(waterlogged: string) {
    return { name: 'minecraft:oak_leaves', properties: { waterlogged } }
}

it('waterlogged fluid is meshed apart from the host block', () => {
    const requested = new Set<string>()
    const geometry = createBlockGeometry(resources, trackingAtlas(requested))

    const block = geometry.mesh(leaves('true'), Cull.none())
    expect(block.quads).toHaveLength(6)
    expect([...requested]).toEqual(['minecraft:block/oak_leaves'])

    const fluid = geometry.water(leaves('true'), Cull.none())
    expect(fluid?.quads).toHaveLength(6)
    expect(requested.has('minecraft:block/water_still')).toBe(true)
    expect(requested.has('minecraft:block/water_flow')).toBe(true)
})

it('dry blocks carry no fluid', () => {
    const geometry = createBlockGeometry(resources, trackingAtlas(new Set()))
    expect(geometry.water(leaves('false'), Cull.none())).toBeUndefined()
})

it('unresolved waterlogged blocks keep the fluid out of their fallback mesh', () => {
    const requested = new Set<string>()
    const geometry = createBlockGeometry(resources, trackingAtlas(requested))
    geometry.mesh(
        { name: 'minecraft:unknown_block', properties: { waterlogged: 'true' } },
        Cull.none(),
    )
    expect(requested.size).toBe(0)
})

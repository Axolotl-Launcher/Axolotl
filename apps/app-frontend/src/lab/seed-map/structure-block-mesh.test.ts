import { expect, it } from 'vitest'

import { buildStructureBlockMeshes } from './structure-block-mesh.ts'
import type { StructureTemplatePack } from './structure-blocks.ts'

function pack(waterlogged: boolean): StructureTemplatePack {
    return {
        schemaVersion: 2,
        version: '26.3',
        dataVersion: 5023,
        templates: {
            'end_city/base_floor': {
                size: [1, 1, 1],
                overwriteAir: true,
                palette: [
                    { name: 'minecraft:oak_leaves', properties: { waterlogged: `${waterlogged}` } },
                ],
                blocks: [[0, 0, 0, 0]],
            },
        },
        textureColors: {
            'minecraft:block/leaves': [0.2, 0.6, 0.2],
            'minecraft:block/water_still': [0.247, 0.463, 0.894],
            'minecraft:block/water_flow': [0.247, 0.463, 0.894],
        },
        resources: {
            defaultBlockProperties: {},
            blockDefinitions: {
                'minecraft:oak_leaves': {
                    variants: {
                        'waterlogged=false': { model: 'minecraft:block/leaves' },
                        'waterlogged=true': { model: 'minecraft:block/leaves' },
                    },
                },
            },
            blockModels: {
                'minecraft:block/leaves': {
                    textures: { all: 'minecraft:block/leaves' },
                    elements: [
                        {
                            from: [0, 0, 0],
                            to: [16, 16, 16],
                            faces: {
                                down: { texture: '#all' },
                                up: { texture: '#all' },
                                north: { texture: '#all' },
                                south: { texture: '#all' },
                                west: { texture: '#all' },
                                east: { texture: '#all' },
                            },
                        },
                    ],
                },
            },
        },
    }
}

function mesh(waterlogged: boolean) {
    return buildStructureBlockMeshes(
        [
            {
                id: 0,
                name: 'base_floor',
                rotation: 0,
                positionKnown: true,
                position: { x: 0, y: 0, z: 0 },
            },
        ],
        pack(waterlogged),
    ).meshes[0]
}

it('structure previews append waterlogged fluid beside the host quads', () => {
    const dry = mesh(false)
    const wet = mesh(true)

    // Six host quads and, when waterlogged, six fluid quads; each quad expands to six vertices.
    expect(dry.positions).toHaveLength(6 * 6 * 3)
    expect(wet.positions).toHaveLength(2 * 6 * 6 * 3)
    expect(dry.colors).toHaveLength(dry.positions.length)
    expect(wet.colors).toHaveLength(wet.positions.length)
    const blue = (values: Float32Array) =>
        Array.from(values).filter((value, index) => index % 3 === 2 && value > 0.5).length
    expect(blue(dry.colors), 'a dry block has no fluid color').toBe(0)
    expect(blue(wet.colors), 'the fluid quads carry the water color').toBe(6 * 6)
})

it('structure previews keep dry blocks unchanged when no fluid texture is available', () => {
    const withoutFluidTextures = pack(true)
    withoutFluidTextures.textureColors = { 'minecraft:block/leaves': [0.2, 0.6, 0.2] }
    const result = buildStructureBlockMeshes(
        [
            {
                id: 0,
                name: 'base_floor',
                rotation: 0,
                positionKnown: true,
                position: { x: 0, y: 0, z: 0 },
            },
        ],
        withoutFluidTextures,
    )
    expect(result.meshes[0].positions).toHaveLength(6 * 6 * 3)
})

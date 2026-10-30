import { expect, it } from 'vitest'

import { previewTriangles, projectSchematicBlockPreviewPosition } from './block-preview.ts'
import type { SchematicWorkerResources } from './resources.ts'

const leavesResources: SchematicWorkerResources = {
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
            textures: { all: 'minecraft:block/oak_leaves' },
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
    defaultBlockProperties: { 'minecraft:oak_leaves': { waterlogged: 'false' } },
    textureUvs: {
        'minecraft:block/oak_leaves': [0, 0, 0.1, 0.1],
        'minecraft:block/water_still': [0.1, 0, 0.2, 0.1],
        'minecraft:block/water_flow': [0.2, 0, 0.3, 0.1],
    },
    missingTextureUv: [0.9, 0.9, 1, 1],
}

it('full block previews are not vertically compressed', () => {
    const corners = Array.from({ length: 8 }, (_, index) =>
        projectSchematicBlockPreviewPosition([index & 1, (index >> 1) & 1, (index >> 2) & 1]),
    )
    const width = Math.max(...corners.map(({ x }) => x)) - Math.min(...corners.map(({ x }) => x))
    const height = Math.max(...corners.map(({ y }) => y)) - Math.min(...corners.map(({ y }) => y))

    expect(height > width).toBeTruthy()
})

it('block previews draw waterlogged fluid as translucent triangles', () => {
    const dry = previewTriangles(
        { name: 'minecraft:oak_leaves', properties: { waterlogged: 'false' } },
        leavesResources,
    )
    const wet = previewTriangles(
        { name: 'minecraft:oak_leaves', properties: { waterlogged: 'true' } },
        leavesResources,
    )

    expect(dry.length).toBeGreaterThan(0)
    expect(dry.every((triangle) => !triangle.translucent)).toBeTruthy()
    const fluid = wet.filter((triangle) => triangle.translucent)
    expect(fluid).toHaveLength(12)
    expect(wet.length - fluid.length).toBe(dry.length)
    for (const triangle of fluid) {
        for (const vertex of triangle.vertices) {
            expect(vertex.color[2], 'fluid is blue').toBeGreaterThan(vertex.color[0])
        }
    }
})

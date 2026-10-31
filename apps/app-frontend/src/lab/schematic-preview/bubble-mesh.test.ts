import { expect, it } from 'vitest'

import { BUBBLES_PER_CELL, getSchematicBubbleQuads, isSchematicBubbleColumn } from './bubble-mesh'

const CELL: [number, number, number] = [3, 64, -5]
const WATER_SURFACE = 8 / 9

it('recognises the bubble column block', () => {
    expect(isSchematicBubbleColumn({ name: 'minecraft:bubble_column', properties: {} })).toBe(true)
    expect(isSchematicBubbleColumn({ name: ' minecraft:Water', properties: {} })).toBe(false)
})

it('bakes two crossed quads per bubble', () => {
    // One quad would vanish edge-on as the camera turns, so every bubble is a crossed pair.
    expect(BUBBLES_PER_CELL).toBeGreaterThan(0)
    expect(getSchematicBubbleQuads(CELL)).toHaveLength(BUBBLES_PER_CELL * 2)
})

it('keeps every bubble inside its own cell and under the water surface', () => {
    for (const quad of getSchematicBubbleQuads(CELL)) {
        for (const vertex of quad) {
            const [x, y, z] = vertex.position
            expect(x).toBeGreaterThan(CELL[0])
            expect(x).toBeLessThan(CELL[0] + 1)
            expect(z).toBeGreaterThan(CELL[2])
            expect(z).toBeLessThan(CELL[2] + 1)
            expect(y).toBeGreaterThan(CELL[1])
            expect(y).toBeLessThan(CELL[1] + WATER_SURFACE)
        }
    }
})

it('maps the whole sprite, not a corner of it', () => {
    const uvs = new Set(
        getSchematicBubbleQuads(CELL).flatMap((quad) => quad.map((vertex) => vertex.uv.join(','))),
    )
    expect([...uvs].sort()).toEqual(['0,0', '0,1', '1,0', '1,1'])
})

it('crosses the two quads of a bubble at a right angle', () => {
    const quads = getSchematicBubbleQuads(CELL)
    for (let index = 0; index < quads.length; index += 2) {
        const [first, second] = [quads[index], quads[index + 1]]
        const horizontal = (quad: (typeof quads)[number]) => [
            quad[1].position[0] - quad[0].position[0],
            quad[1].position[2] - quad[0].position[2],
        ]
        const [left, right] = [horizontal(first), horizontal(second)]
        expect(left[0] * right[0] + left[1] * right[1]).toBeCloseTo(0, 10)
    }
})

it('meshes the same cell the same way every time', () => {
    expect(getSchematicBubbleQuads(CELL)).toEqual(getSchematicBubbleQuads(CELL))
})

it('scatters bubbles differently in the next cell', () => {
    expect(getSchematicBubbleQuads([CELL[0] + 1, CELL[1], CELL[2]])).not.toEqual(
        getSchematicBubbleQuads(CELL),
    )
})

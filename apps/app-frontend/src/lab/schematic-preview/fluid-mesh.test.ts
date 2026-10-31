import { Cull, type Mesh, type Quad, type TextureAtlasProvider } from 'deepslate'
import { expect, it } from 'vitest'

import type { SchematicBlockState } from './backend'
import {
    getSchematicFluidMesh,
    schematicFluidHeight,
    type SchematicFluidNeighborhood,
} from './fluid-mesh'

const ATLAS_SIZE = 2048
const SPRITE = 16 / ATLAS_SIZE
const STILL: [number, number, number, number] = [0, 0, SPRITE, SPRITE]
const FLOW: [number, number, number, number] = [32 / ATLAS_SIZE, 0, 48 / ATLAS_SIZE, SPRITE]

const AIR: SchematicBlockState = { name: 'minecraft:air', properties: {} }
const STONE: SchematicBlockState = { name: 'minecraft:stone', properties: {} }

function water(level = '0'): SchematicBlockState {
    return { name: 'minecraft:water', properties: { level } }
}

function atlas(): TextureAtlasProvider {
    return {
        getTextureAtlas: () => ({}) as ImageData,
        getTextureUV: (id) => (id.toString().endsWith('water_still') ? STILL : FLOW),
    }
}

/** Cells default to air; `null` marks a cell the mesh job carries no data for. */
function cells(entries: Record<string, SchematicBlockState | null>): SchematicFluidNeighborhood {
    return {
        blockAt: (dx, dy, dz) => {
            const key = `${dx},${dy},${dz}`
            return key in entries ? (entries[key] ?? undefined) : AIR
        },
    }
}

/** Water on its own, with every side and the floor culled so only the surface is left. */
function surfaceOnly(neighborhood?: SchematicFluidNeighborhood) {
    const cull: Cull = { north: true, south: true, east: true, west: true, down: true }
    return getSchematicFluidMesh({
        state: water(),
        atlas: atlas(),
        cull,
        ...(neighborhood ? { neighborhood } : {}),
    })
}

/** The surface is the quad whose lowest vertex sits above every other quad's lowest vertex. */
function topFace(mesh: Mesh | undefined) {
    const lowest = (quad: Quad) => Math.min(...quad.vertices().map((vertex) => vertex.pos.y))
    return (mesh?.quads ?? []).reduce<Quad | undefined>(
        (best, quad) => (!best || lowest(quad) > lowest(best) ? quad : best),
        undefined,
    )
}

function heights(mesh: Mesh | undefined) {
    return (mesh?.quads ?? []).map((quad) => quad.vertices().map((vertex) => vertex.pos.y))
}

/** Atlas texture coordinates of a quad, expressed in sprite pixels. */
function spritePixels(quad: Quad | undefined, sprite: readonly number[]) {
    return (quad?.vertices() ?? []).map((vertex) => {
        const [u, v] = vertex.texture ?? [Number.NaN, Number.NaN]
        return [
            ((u - sprite[0]) / (sprite[2] - sprite[0])) * 16,
            ((v - sprite[1]) / (sprite[3] - sprite[1])) * 16,
        ]
    })
}

function spriteOf(quad: Quad | undefined) {
    const u = Math.min(...(quad?.vertices() ?? []).map((vertex) => vertex.texture?.[0] ?? 0))
    return u < SPRITE ? 'still' : 'flow'
}

it('reports the vanilla surface height of a fluid level', () => {
    expect(schematicFluidHeight(water())).toBeCloseTo(8 / 9, 6)
    expect(schematicFluidHeight(water('3'))).toBeCloseTo(5 / 9, 6)
    expect(schematicFluidHeight(water('7'))).toBeCloseTo(1 / 9, 6)
    expect(schematicFluidHeight(water('8')), 'a falling column fills the cell').toBe(1)
    expect(schematicFluidHeight(STONE)).toBeUndefined()
    expect(
        schematicFluidHeight({ name: 'minecraft:oak_leaves', properties: { waterlogged: 'true' } }),
    ).toBeCloseTo(8 / 9, 6)
})

it('a level pool renders one flat surface without side faces', () => {
    const pool: Record<string, SchematicBlockState> = {}
    for (const x of [-1, 0, 1]) {
        for (const z of [-1, 0, 1]) pool[`${x},0,${z}`] = water()
    }
    const mesh = surfaceOnly(cells(pool))
    expect(heights(mesh)).toHaveLength(1)
    const quad = topFace(mesh)
    for (const vertex of quad?.vertices() ?? []) {
        expect(vertex.pos.y).toBeCloseTo(8 / 9 - 0.001, 6)
    }
    expect(spriteOf(quad), 'still water keeps the still sprite').toBe('still')
})

/** A source block that only drains east, so the surface slopes towards positive x. */
function slopeEast(): Record<string, SchematicBlockState> {
    return {
        '0,0,0': water(),
        '1,0,0': water('3'),
        '1,0,1': STONE,
        '0,0,1': STONE,
        '1,0,-1': STONE,
        '0,0,-1': STONE,
        '-1,0,0': STONE,
        '-1,0,1': STONE,
        '-1,0,-1': STONE,
    }
}

it('water flowing next to a lower level slopes towards it', () => {
    // Solid blocks keep the west corners at full height; only the flowing east neighbour lowers the
    // east corners, which is the slope vanilla draws between two levels.
    const mesh = surfaceOnly(cells(slopeEast()))
    const vertices = topFace(mesh)?.vertices() ?? []
    // Vanilla weights the full source ten times over and the shallow neighbour once: 85/99.
    expect(vertices[0].pos.y).toBeCloseTo(8 / 9 - 0.001, 6)
    expect(vertices[1].pos.y).toBeCloseTo(8 / 9 - 0.001, 6)
    expect(vertices[2].pos.y).toBeCloseTo(85 / 99 - 0.001, 6)
    expect(vertices[3].pos.y).toBeCloseTo(85 / 99 - 0.001, 6)
    expect(spriteOf(topFace(mesh)), 'a flowing surface uses the flowing sprite').toBe('flow')
})

it('rotates the flowing surface so its streaks follow the flow', () => {
    // Flowing east rotates the sprite a quarter turn: the sprite's vertical axis runs along x, and
    // the vanilla shrink keeps the rotated corners a quarter of the sprite inside its centre.
    const pixels = spritePixels(topFace(surfaceOnly(cells(slopeEast()))), FLOW)
    const expected = [
        [11, 5],
        [5, 5],
        [5, 11],
        [11, 11],
    ]
    expected.forEach(([u, v], index) => {
        expect(pixels[index][0]).toBeCloseTo(u, 6)
        expect(pixels[index][1]).toBeCloseTo(v, 6)
    })
})

it('air beside the water lowers the corner, a wall does not', () => {
    // Vanilla counts air as an empty cell but skips solid cells, so a lone source keeps a flat
    // surface that sits lower than the same block against a wall.
    const isolated = topFace(surfaceOnly(cells({ '0,0,0': water() })))
    for (const vertex of isolated?.vertices() ?? []) {
        expect(vertex.pos.y).toBeCloseTo(80 / 117 - 0.001, 6)
    }
    const walled: Record<string, SchematicBlockState> = { '0,0,0': water() }
    for (const x of [-1, 0, 1]) {
        for (const z of [-1, 0, 1]) {
            if (x !== 0 || z !== 0) walled[`${x},0,${z}`] = STONE
        }
    }
    for (const vertex of topFace(surfaceOnly(cells(walled)))?.vertices() ?? []) {
        expect(vertex.pos.y).toBeCloseTo(8 / 9 - 0.001, 6)
    }
})

it('a cell with fluid above fills the block', () => {
    const falling = surfaceOnly(cells({ '0,0,0': water(), '1,1,0': water() }))
    expect(
        topFace(falling)?.vertices()[3].pos.y,
        'the north-east corner is a waterfall',
    ).toBeCloseTo(1 - 0.001, 6)
})

it('a neighbour without data keeps the surface where a solid neighbour keeps it', () => {
    const base = { '0,0,0': water(), '0,0,-1': water(), '-1,0,0': water(), '-1,0,-1': water() }
    const corner = (east: SchematicBlockState | null) =>
        topFace(surfaceOnly(cells({ ...base, '1,0,0': east })))?.vertices()[3].pos.y ?? Number.NaN
    expect(corner(null)).toBeCloseTo(corner(STONE), 6)
    expect(
        corner(null),
        'a chunk corner must not drop towards a cell that is merely unknown',
    ).toBeGreaterThan(corner(AIR))
})

it('the surface and the side faces share one rim', () => {
    // Vanilla lowers the four corners while it draws the top face and raises the sides to the same
    // edge. A rim at two different heights leaves a notch that sees into the fluid volume.
    const mesh = getSchematicFluidMesh({
        state: water(),
        atlas: atlas(),
        cull: { down: true },
    })
    const [surface, ...sides] = mesh?.quads ?? []
    const rim = 8 / 9 - 0.001
    expect(surface.vertices()).toHaveLength(4)
    for (const vertex of surface.vertices()) expect(vertex.pos.y).toBeCloseTo(rim, 9)
    expect(sides).toHaveLength(4)
    for (const side of sides) {
        expect(Math.max(...side.vertices().map((vertex) => vertex.pos.y))).toBeCloseTo(rim, 9)
    }
})

it('only the side faces are pulled inside the block bounds', () => {
    // The surface keeps the whole block so two cells of the same fluid meet edge to edge even
    // though the face between them is culled.
    const mesh = getSchematicFluidMesh({
        state: water(),
        atlas: atlas(),
        cull: { down: true, east: true, south: true },
    })
    const quads = mesh?.quads ?? []
    const surface = quads[0]
    for (const axis of ['x', 'z'] as const) {
        const values = surface.vertices().map((vertex) => vertex.pos[axis])
        expect(Math.min(...values)).toBeCloseTo(0, 9)
        expect(Math.max(...values)).toBeCloseTo(1, 9)
    }
    const onSide = (value: number) =>
        Math.abs(value - 0.001) < 1e-9 || Math.abs(value - 0.999) < 1e-9
    for (const side of quads.slice(1)) {
        for (const vertex of side.vertices()) {
            expect(onSide(vertex.pos.x) || onSide(vertex.pos.z)).toBe(true)
        }
    }
})

it('side faces rise to the corner heights and crop the flowing sprite to them', () => {
    const mesh = getSchematicFluidMesh({
        state: water(),
        atlas: atlas(),
        cull: { up: true, down: true, south: true, east: true, west: true },
        neighborhood: cells({ '0,0,0': water() }),
    })
    const [side] = mesh?.quads ?? []
    const vertices = side.vertices()
    const surface = 80 / 117
    expect(vertices[0].pos.y).toBeCloseTo(surface, 6)
    expect(vertices[1].pos.y).toBeCloseTo(0, 6)
    expect(vertices[2].pos.y).toBeCloseTo(0, 6)
    expect(vertices[3].pos.y).toBeCloseTo(surface, 6)
    expect(vertices[0].pos.z).toBeCloseTo(0.001, 6)
    // The sprite is cropped to the fluid height: the upper edge samples the lower half only.
    const cropped = (1 - surface) * 8
    const pixels = spritePixels(side, FLOW)
    expect(pixels[0][0]).toBeCloseTo(8, 6)
    expect(pixels[0][1]).toBeCloseTo(cropped, 6)
    expect(pixels[1][0]).toBeCloseTo(8, 6)
    expect(pixels[1][1]).toBeCloseTo(8, 6)
    expect(pixels[2][0]).toBeCloseTo(0, 6)
    expect(pixels[2][1]).toBeCloseTo(8, 6)
    expect(pixels[3][0]).toBeCloseTo(0, 6)
    expect(pixels[3][1]).toBeCloseTo(cropped, 6)

    // An exposed floor is lifted a thousandth so it never shares a plane with the block below.
    const exposed = getSchematicFluidMesh({
        state: water(),
        atlas: atlas(),
        cull: { up: true, north: true, south: true, east: true, west: true },
    })
    const floor = exposed?.quads[0].vertices() ?? []
    expect(floor).toHaveLength(4)
    for (const vertex of floor) expect(vertex.pos.y).toBeCloseTo(0.001, 6)
})

it('a fluid without visible faces produces no mesh at all', () => {
    const cull: Cull = { north: true, south: true, east: true, west: true, down: true, up: true }
    expect(
        getSchematicFluidMesh({ state: water(), atlas: atlas(), cull }),
        'water buried in stone has nothing to draw',
    ).toBeUndefined()
    expect(
        getSchematicFluidMesh({ state: STONE, atlas: atlas(), cull: Cull.none() }),
        'a block that holds no water meshes no fluid',
    ).toBeUndefined()
})

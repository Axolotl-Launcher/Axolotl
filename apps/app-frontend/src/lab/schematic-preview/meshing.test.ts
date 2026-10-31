import { Cull, type Identifier, Mesh, Quad, Vector } from 'deepslate'
import { expect, it } from 'vitest'

import type { SchematicBlockState } from './backend.ts'
import type { SchematicFluidNeighborhood } from './fluid-mesh.ts'
import {
    applySeamlessSchematicGlassUvs,
    extractSchematicNeighborFace,
    getSchematicFluidCull,
    getSchematicMeshOcclusionFaces,
    getSchematicSpecialBlockMesh,
    getSchematicWaterloggedFluidMesh,
    isSchematicOccluding,
    isSchematicWaterlogged,
    isSeamlessSchematicGlassPair,
    schematicBlockAt,
    shouldCullSchematicFace,
} from './meshing.ts'

const palette = [
    { name: 'minecraft:air', properties: {} },
    { name: 'minecraft:white_stained_glass', properties: {} },
    { name: 'minecraft:black_stained_glass', properties: {} },
    { name: 'minecraft:stone', properties: {} },
    { name: 'minecraft:sugar_cane', properties: {} },
    { name: 'minecraft:chest', properties: { facing: 'north', type: 'single' } },
    { name: 'minecraft:observer', properties: { facing: 'north', powered: 'false' } },
]

it('seamless glass only removes faces shared by the same full glass block', () => {
    expect(isSeamlessSchematicGlassPair(palette[1].name, palette[1].name)).toBe(true)
    expect(isSeamlessSchematicGlassPair(palette[1].name, palette[2].name)).toBe(false)
    expect(isSeamlessSchematicGlassPair('minecraft:glass', 'minecraft:glass_pane')).toBe(false)
    expect(shouldCullSchematicFace(1, 1, palette)).toBe(true)
    expect(shouldCullSchematicFace(1, 1, palette, false)).toBe(false)
    expect(shouldCullSchematicFace(1, 2, palette)).toBe(false)
    expect(shouldCullSchematicFace(1, 3, palette)).toBe(true)
    expect(shouldCullSchematicFace(1, 3, palette, false)).toBe(true)
    expect(isSchematicOccluding(4, palette)).toBe(false)
    expect(isSchematicOccluding(5, palette)).toBe(false)
    expect(isSchematicOccluding(6, palette)).toBe(false)
})

function addNorthFace(mesh: Mesh, x0: number, y0: number, x1: number, y1: number) {
    mesh.quads.push(
        Quad.fromPoints(
            new Vector(x1, y0, 0),
            new Vector(x0, y0, 0),
            new Vector(x0, y1, 0),
            new Vector(x1, y1, 0),
        ),
    )
}

it('mesh occlusion requires complete coverage of each individual boundary face', () => {
    const partial = new Mesh()
    addNorthFace(partial, 0, 0, 1, 0.5)
    expect(getSchematicMeshOcclusionFaces(partial)).toEqual({})

    const tiled = new Mesh()
    addNorthFace(tiled, 0, 0, 1, 0.5)
    addNorthFace(tiled, 0, 0.5, 1, 1)
    expect(getSchematicMeshOcclusionFaces(tiled)).toEqual({ north: true })
})

it('neighbor faces preserve seamless glass culling across chunk boundaries', () => {
    const current = new Uint32Array(4096)
    const eastNeighbor = new Uint32Array(4096)
    current[15] = 1
    eastNeighbor[0] = 1
    const east = extractSchematicNeighborFace(eastNeighbor, 'east')
    const neighbor = schematicBlockAt(current, { east }, 16, 0, 0)
    expect(neighbor).toBe(1)
    expect(shouldCullSchematicFace(current[15], neighbor, palette)).toBe(true)

    eastNeighbor[0] = 2
    const differentGlass = extractSchematicNeighborFace(eastNeighbor, 'east')
    expect(schematicBlockAt(current, { east: differentGlass }, 16, 0, 0)).toBe(2)
    expect(shouldCullSchematicFace(1, 2, palette)).toBe(false)
})

it('connected glass crops only the joined texture borders', () => {
    const blocks = new Uint32Array(4096)
    blocks[0] = 1
    blocks[1] = 1
    const vertices = [
        { pos: { x: 0, y: 0, z: 0 }, texture: [0, 1] as [number, number] },
        { pos: { x: 1, y: 0, z: 0 }, texture: [1, 1] as [number, number] },
        { pos: { x: 1, y: 1, z: 0 }, texture: [1, 0] as [number, number] },
        { pos: { x: 0, y: 1, z: 0 }, texture: [0, 0] as [number, number] },
    ]
    applySeamlessSchematicGlassUvs(vertices, [0, 0, 0], 1, blocks, {}, palette)
    expect(vertices.map((vertex) => vertex.texture)).toEqual([
        [0, 1],
        [15 / 16, 1],
        [15 / 16, 0],
        [0, 0],
    ])
})

it('disabled seamless glass preserves the original texture coordinates', () => {
    const blocks = new Uint32Array(4096)
    blocks[0] = 1
    blocks[1] = 1
    const vertices = [
        { pos: { x: 0, y: 0, z: 0 }, texture: [0, 1] as [number, number] },
        { pos: { x: 1, y: 0, z: 0 }, texture: [1, 1] as [number, number] },
        { pos: { x: 1, y: 1, z: 0 }, texture: [1, 0] as [number, number] },
        { pos: { x: 0, y: 1, z: 0 }, texture: [0, 0] as [number, number] },
    ]
    applySeamlessSchematicGlassUvs(vertices, [0, 0, 0], 1, blocks, {}, palette, false)
    expect(vertices.map((vertex) => vertex.texture)).toEqual([
        [0, 1],
        [1, 1],
        [1, 0],
        [0, 0],
    ])
})

it('chests use Minecraft special geometry and entity textures', () => {
    for (const [name, texture] of [
        ['minecraft:chest', 'minecraft:entity/chest/normal'],
        ['minecraft:trapped_chest', 'minecraft:entity/chest/trapped'],
        ['minecraft:ender_chest', 'minecraft:entity/chest/ender'],
    ]) {
        const requestedTextures = new Set<string>()
        const mesh = getSchematicSpecialBlockMesh(
            { name, properties: { facing: 'east', type: 'single', waterlogged: 'false' } },
            {
                getTextureAtlas: () => ({}) as ImageData,
                getTextureUV: (id: Identifier) => {
                    requestedTextures.add(id.toString())
                    return [0, 0, 1, 1]
                },
            },
            Cull.none(),
        )

        expect(mesh.quads.length > 0, `${name} generated no special mesh`).toBeTruthy()
        expect(requestedTextures.has(texture), `${name} did not request ${texture}`).toBeTruthy()
    }
})

it('double chests use the matching left and right entity textures', () => {
    for (const [name, texture] of [
        ['minecraft:chest', 'normal'],
        ['minecraft:trapped_chest', 'trapped'],
        ['minecraft:copper_chest', 'copper'],
        ['minecraft:exposed_copper_chest', 'copper_exposed'],
        ['minecraft:weathered_copper_chest', 'copper_weathered'],
        ['minecraft:oxidized_copper_chest', 'copper_oxidized'],
        ['minecraft:waxed_copper_chest', 'copper'],
        ['minecraft:waxed_exposed_copper_chest', 'copper_exposed'],
        ['minecraft:waxed_weathered_copper_chest', 'copper_weathered'],
        ['minecraft:waxed_oxidized_copper_chest', 'copper_oxidized'],
    ]) {
        for (const type of ['left', 'right']) {
            const requestedTextures = new Set<string>()
            const mesh = getSchematicSpecialBlockMesh(
                { name, properties: { facing: 'north', type, waterlogged: 'false' } },
                {
                    getTextureAtlas: () => ({}) as ImageData,
                    getTextureUV: (id: Identifier) => {
                        requestedTextures.add(id.toString())
                        return [0, 0, 1, 1]
                    },
                },
                Cull.none(),
            )

            expect(mesh.quads.length > 0, `${name}[type=${type}] generated no mesh`).toBeTruthy()
            expect(requestedTextures).toEqual(
                new Set([`minecraft:entity/chest/${texture}_${type}`]),
            )
        }
    }
})

it('double chest halves meet across every facing direction', () => {
    const atlas = {
        getTextureAtlas: () => ({}) as ImageData,
        getTextureUV: () => [0, 0, 1, 1] as [number, number, number, number],
    }
    const facings = [
        { facing: 'north', axis: 'x', offset: [1, 0, 0] as const },
        { facing: 'south', axis: 'x', offset: [-1, 0, 0] as const },
        { facing: 'west', axis: 'z', offset: [0, 0, -1] as const },
        { facing: 'east', axis: 'z', offset: [0, 0, 1] as const },
    ]
    const coordinate = (mesh: ReturnType<typeof getSchematicSpecialBlockMesh>, axis: 'x' | 'z') =>
        mesh.quads.flatMap((quad) => quad.vertices().map((vertex) => vertex.pos[axis]))

    for (const { facing, axis, offset } of facings) {
        const left = getSchematicSpecialBlockMesh(
            { name: 'minecraft:chest', properties: { facing, type: 'left' } },
            atlas,
            Cull.none(),
        )
        const right = getSchematicSpecialBlockMesh(
            { name: 'minecraft:chest', properties: { facing, type: 'right' } },
            atlas,
            Cull.none(),
        )
        const leftCoordinates = coordinate(left, axis)
        const rightCoordinates = coordinate(right, axis).map(
            (value) => value + (axis === 'x' ? offset[0] : offset[2]),
        )
        const positive = (axis === 'x' ? offset[0] : offset[2]) > 0
        const leftEdge = positive ? Math.max(...leftCoordinates) : Math.min(...leftCoordinates)
        const rightEdge = positive ? Math.min(...rightCoordinates) : Math.max(...rightCoordinates)
        const combined = [...leftCoordinates, ...rightCoordinates]

        expect(
            Math.abs(leftEdge - rightEdge) < 1e-6,
            `${facing} chest halves have a gap`,
        ).toBeTruthy()
        expect(
            Math.abs(Math.max(...combined) - Math.min(...combined) - 30 / 16) < 1e-6,
            `${facing} double chest is not 30 pixels wide`,
        ).toBeTruthy()
    }
})

it('ender chests remain single even with an invalid double-chest type', () => {
    const requestedTextures = new Set<string>()
    getSchematicSpecialBlockMesh(
        { name: 'minecraft:ender_chest', properties: { facing: 'north', type: 'left' } },
        {
            getTextureAtlas: () => ({}) as ImageData,
            getTextureUV: (id: Identifier) => {
                requestedTextures.add(id.toString())
                return [0, 0, 1, 1]
            },
        },
        Cull.none(),
    )

    expect(requestedTextures.has('minecraft:entity/chest/ender')).toBeTruthy()
    expect(!requestedTextures.has('minecraft:entity/chest/ender_left')).toBeTruthy()
})

function trackingAtlas(requested: Set<string>) {
    return {
        getTextureAtlas: () => ({}) as ImageData,
        getTextureUV: (id: Identifier) => {
            requested.add(id.toString())
            return [0, 0, 1, 1] as [number, number, number, number]
        },
    }
}

it('waterlogged blocks mesh their fluid inside the block bounds', () => {
    const requested = new Set<string>()
    const fluid = getSchematicWaterloggedFluidMesh(
        { name: 'minecraft:oak_leaves', properties: { waterlogged: 'true' } },
        trackingAtlas(requested),
        Cull.none(),
    )
    expect(requested).toEqual(
        new Set(['minecraft:block/water_still', 'minecraft:block/water_flow']),
    )
    const quads = fluid?.quads ?? []
    const vertices = quads.flatMap((quad) => quad.vertices())
    expect(vertices).toHaveLength(24)
    const rim = 8 / 9 - 0.001
    expect(Math.max(...vertices.map((vertex) => vertex.pos.y))).toBeCloseTo(rim, 6)
    expect(
        vertices.filter((vertex) => Math.abs(vertex.pos.y - rim) < 1e-6),
        'the surface and the four side faces share one rim, so the edge stays closed',
    ).toHaveLength(12)
    // The surface and the floor keep the whole block, so two neighbouring fluids meet instead of
    // leaving a gap; only the side faces are pulled inside the host block.
    const axis = (name: 'x' | 'z') => vertices.map((vertex) => vertex.pos[name])
    expect(Math.min(...axis('x'))).toBeCloseTo(0, 6)
    expect(Math.max(...axis('x'))).toBeCloseTo(1, 6)
    expect(Math.min(...axis('z'))).toBeCloseTo(0, 6)
    expect(Math.max(...axis('z'))).toBeCloseTo(1, 6)
    expect(
        vertices.filter(
            (vertex) =>
                Math.abs(vertex.pos.x - 1 / 64) < 1e-6 ||
                Math.abs(vertex.pos.x - 63 / 64) < 1e-6 ||
                Math.abs(vertex.pos.z - 1 / 64) < 1e-6 ||
                Math.abs(vertex.pos.z - 63 / 64) < 1e-6,
        ),
        'only the four side faces sit inside the block bounds',
    ).toHaveLength(16)
    for (const vertex of vertices) {
        for (const coordinate of [vertex.pos.x, vertex.pos.y, vertex.pos.z]) {
            expect(coordinate).toBeGreaterThanOrEqual(0)
            expect(coordinate).toBeLessThanOrEqual(1)
        }
    }
    expect(vertices[0].color[2]).toBeGreaterThan(0.8)
    expect(vertices[0].color[0]).toBeLessThan(0.3)
})

it('adjacent waterlogged cells meet at the face they share', () => {
    // The fluid face between two water-holding cells is culled, so nothing covers a gap if the two
    // volumes pull apart. The surface has to reach the block bounds for the join to close.
    const meshFor = (cull: Cull) =>
        getSchematicWaterloggedFluidMesh(
            { name: 'minecraft:oak_leaves', properties: { waterlogged: 'true' } },
            trackingAtlas(new Set()),
            cull,
        )
    const east = Cull.none()
    east.east = true
    const west = Cull.none()
    west.west = true
    // Only the surface matters: the side faces span the whole block along the axis they do not face,
    // so a surface that stops short would still show the block bounds through them.
    const span = (mesh: Mesh | undefined) =>
        (mesh?.quads[0].vertices() ?? []).map((vertex) => vertex.pos.x)
    expect(
        Math.max(...span(meshFor(east))),
        'the eastern cell reaches the face it shares with its neighbour',
    ).toBeCloseTo(1, 9)
    expect(
        Math.min(...span(meshFor(west))),
        'the western cell reaches the face it shares with its neighbour',
    ).toBeCloseTo(0, 9)
})

it('waterlogged fluid takes the surface height from its neighbours', () => {
    const state = { name: 'minecraft:oak_leaves', properties: { waterlogged: 'true' } }
    const surface = (neighborhood?: SchematicFluidNeighborhood) =>
        (
            getSchematicWaterloggedFluidMesh(
                state,
                trackingAtlas(new Set()),
                Cull.none(),
                neighborhood,
            )?.quads[0].vertices() ?? []
        ).map((vertex) => vertex.pos.y)
    const flat = surface()
    expect(flat).toHaveLength(4)
    for (const height of flat) expect(height).toBeCloseTo(8 / 9 - 0.001, 9)

    // Vanilla has no waterlogged special case: the surface samples the same 2x2 corners as any other
    // water, so a shallow neighbour lowers the corners that face it.
    const sloping = surface({
        blockAt: (dx, dy, dz): SchematicBlockState => {
            if (dx === 0 && dy === 0 && dz === 0) return { name: 'minecraft:water', properties: {} }
            if (dx === 1 && dy === 0 && dz === 0) {
                return { name: 'minecraft:water', properties: { level: '3' } }
            }
            return { name: 'minecraft:stone', properties: {} }
        },
    })
    expect(sloping[0], 'the walled corners keep the full height').toBeCloseTo(8 / 9 - 0.001, 9)
    expect(sloping[1]).toBeCloseTo(8 / 9 - 0.001, 9)
    expect(sloping[2], 'the eastern corners drop towards the shallow neighbour').toBeCloseTo(
        85 / 99 - 0.001,
        9,
    )
    expect(sloping[3]).toBeCloseTo(85 / 99 - 0.001, 9)
})

it('waterlogged fluid follows the neighbor cull', () => {
    const heights = (cull: Cull) =>
        (
            getSchematicWaterloggedFluidMesh(
                { name: 'minecraft:oak_leaves', properties: { waterlogged: 'true' } },
                trackingAtlas(new Set()),
                cull,
            )?.quads ?? []
        ).map((quad) => quad.vertices().map((vertex) => vertex.pos.y))
    const surface = 8 / 9
    const bottom = 1 / 64
    const culled = Cull.none()
    culled.up = true
    const remaining = heights(culled)
    const flat = (value: number, quad: number[]) =>
        quad.every((height) => Math.abs(height - value) < 1e-6)

    // The fluid keeps its surface below the block top, so the cull only removes the top face: the
    // floor and the four sides, whose upper edge still reaches the surface, stay.
    expect(heights(Cull.none())).toHaveLength(6)
    expect(heights(Cull.none()).filter((quad) => flat(bottom, quad))).toHaveLength(1)
    expect(
        heights(Cull.none()).filter((quad) => quad.every((height) => height > bottom)),
        'exactly one face is the whole surface',
    ).toHaveLength(1)
    expect(remaining).toHaveLength(5)
    expect(
        remaining.filter((quad) => quad.every((height) => height > bottom)),
        'the up face must be culled',
    ).toHaveLength(0)
    expect(
        remaining.filter((quad) => flat(bottom, quad)),
        'the down face must remain',
    ).toHaveLength(1)
    expect(
        remaining.filter((quad) => quad.some((height) => Math.abs(height - surface) < 1e-6)),
        'the four side faces must remain',
    ).toHaveLength(4)
})

it('waterlogged fluid is culled against any neighbor that holds water', () => {
    const blocks = new Uint32Array(16 * 16 * 16)
    const fluidPalette: SchematicBlockState[] = [
        { name: 'minecraft:air', properties: {} },
        { name: 'minecraft:oak_leaves', properties: { waterlogged: 'true' } },
        { name: 'minecraft:water', properties: { level: '0' } },
        { name: 'minecraft:stone', properties: {} },
        { name: 'minecraft:oak_leaves', properties: { waterlogged: 'false' } },
    ]
    const index = (x: number, y: number, z: number) => y * 256 + z * 16 + x
    blocks[index(1, 1, 1)] = 1
    blocks[index(2, 1, 1)] = 1
    blocks[index(1, 2, 1)] = 2
    blocks[index(1, 0, 1)] = 3
    blocks[index(1, 1, 2)] = 4

    const fluidCull = getSchematicFluidCull([1, 1, 1], blocks, {}, fluidPalette, Cull.none())
    expect(fluidCull.east, 'a waterlogged neighbor holds water').toBe(true)
    expect(fluidCull.up, 'a water neighbor holds water').toBe(true)
    expect(fluidCull.down, 'stone is not water').toBeFalsy()
    expect(fluidCull.south, 'dry leaves are not water').toBeFalsy()
    expect(fluidCull.west, 'air is not water').toBeFalsy()
    expect(fluidCull.north, 'air is not water').toBeFalsy()
    blocks[index(15, 1, 1)] = 1
    expect(
        getSchematicFluidCull(
            [15, 1, 1],
            blocks,
            { east: new Uint32Array(256).fill(2) },
            fluidPalette,
            Cull.none(),
        ).east,
        'fluid is also culled across a chunk boundary',
    ).toBe(true)
})

it('a block above never hides the waterlogged surface', () => {
    // A fluid surface sits at the fluid height, not at the top of the block, so a block above it
    // never reaches it and only another cell of fluid can hide it. Vanilla's renderer draws the
    // surface for the same reason and skips it only for a cell of the same fluid.
    const fluidPalette: SchematicBlockState[] = [
        { name: 'minecraft:air', properties: {} },
        {
            name: 'minecraft:oak_trapdoor',
            properties: { facing: 'north', half: 'bottom', open: 'false', waterlogged: 'true' },
        },
        { name: 'minecraft:stone', properties: {} },
        { name: 'minecraft:water', properties: { level: '0' } },
    ]
    const index = (x: number, y: number, z: number) => y * 256 + z * 16 + x
    const blocks = new Uint32Array(16 * 16 * 16)
    blocks[index(1, 1, 1)] = 1
    // The host's own top face is hidden by whatever sits above it, so the host cull says `up`.
    const hostCull = Cull.none()
    hostCull.up = true
    const surface = (above: number) => {
        blocks[index(1, 2, 1)] = above
        const fluidCull = getSchematicFluidCull([1, 1, 1], blocks, {}, fluidPalette, hostCull)
        return {
            up: fluidCull.up,
            quads: getSchematicWaterloggedFluidMesh(
                fluidPalette[1],
                trackingAtlas(new Set()),
                fluidCull,
            )?.quads.length,
        }
    }

    expect(surface(2), 'a block above leaves the surface visible below it').toEqual({
        up: false,
        quads: 6,
    })
    expect(surface(0), 'air above keeps the surface').toEqual({ up: false, quads: 6 })
    expect(surface(3), 'a cell of water above still joins the two volumes').toEqual({
        up: true,
        quads: 5,
    })
})

it('water and lava do not cull each other', () => {
    // Vanilla compares the two fluid states, so only a cell of the same fluid joins up. Water beside
    // lava and the water inside waterlogged blocks both leave the lava faces alone.
    const fluidPalette: SchematicBlockState[] = [
        { name: 'minecraft:air', properties: {} },
        { name: 'minecraft:lava', properties: { level: '0' } },
        { name: 'minecraft:water', properties: { level: '0' } },
        { name: 'minecraft:oak_leaves', properties: { waterlogged: 'true' } },
    ]
    const index = (x: number, y: number, z: number) => y * 256 + z * 16 + x
    const blocks = new Uint32Array(16 * 16 * 16)
    blocks[index(1, 1, 1)] = 1
    blocks[index(2, 1, 1)] = 2
    blocks[index(1, 2, 1)] = 1
    blocks[index(1, 1, 2)] = 1
    blocks[index(1, 0, 1)] = 3

    const fluidCull = getSchematicFluidCull([1, 1, 1], blocks, {}, fluidPalette, Cull.none())
    expect(fluidCull.up, 'lava above merges').toBe(true)
    expect(fluidCull.south, 'lava next door merges').toBe(true)
    expect(fluidCull.east, 'water does not hide the face of lava').toBeFalsy()
    expect(fluidCull.down, 'the water in waterlogged leaves is not lava').toBeFalsy()
})

it('blocks without waterlogged fluid mesh no fluid at all', () => {
    const requested = new Set<string>()
    const atlas = trackingAtlas(requested)
    expect(
        getSchematicWaterloggedFluidMesh(
            { name: 'minecraft:oak_leaves', properties: { waterlogged: 'false' } },
            atlas,
            Cull.none(),
        ),
    ).toBeUndefined()
    expect(
        getSchematicWaterloggedFluidMesh(
            { name: 'minecraft:stone', properties: {} },
            atlas,
            Cull.none(),
        ),
    ).toBeUndefined()
    expect(requested.size).toBe(0)
    expect(
        isSchematicWaterlogged({
            name: 'minecraft:oak_leaves',
            properties: { waterlogged: 'true' },
        }),
    ).toBe(true)
})

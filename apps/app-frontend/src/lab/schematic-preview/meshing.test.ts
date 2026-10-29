import { Cull, type Identifier, Mesh, Quad, Vector } from 'deepslate'
import { expect, it } from 'vitest'

import {
    applySeamlessSchematicGlassUvs,
    extractSchematicNeighborFace,
    getSchematicMeshOcclusionFaces,
    getSchematicSpecialBlockMesh,
    isSchematicOccluding,
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

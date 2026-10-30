import {
    BlockState,
    type Cull,
    Identifier,
    Mesh,
    Quad,
    SpecialRenderers,
    type TextureAtlasProvider,
    Vector,
} from 'deepslate'

import type { SchematicBlockState } from './backend.ts'
import { isSchematicAir } from './editing.ts'

export const SCHEMATIC_DIRECTIONS = ['west', 'east', 'down', 'up', 'north', 'south'] as const
export type SchematicDirection = (typeof SCHEMATIC_DIRECTIONS)[number]
export type SchematicNeighborFaces = Partial<Record<SchematicDirection, Uint32Array>>
export type SchematicOcclusionFaces = Partial<Record<SchematicDirection, true>>
export type SchematicTexturedVertex = {
    pos: { x: number; y: number; z: number }
    texture?: [number, number]
}

export const SCHEMATIC_OPPOSITE_DIRECTIONS: Record<SchematicDirection, SchematicDirection> = {
    west: 'east',
    east: 'west',
    down: 'up',
    up: 'down',
    north: 'south',
    south: 'north',
}

export const SCHEMATIC_DIRECTION_OFFSETS: Record<
    SchematicDirection,
    readonly [number, number, number]
> = {
    west: [-1, 0, 0],
    east: [1, 0, 0],
    down: [0, -1, 0],
    up: [0, 1, 0],
    north: [0, 0, -1],
    south: [0, 0, 1],
}

const TRANSLUCENT_BLOCK_PARTS = [
    'glass',
    'ice',
    'water',
    'lava',
    'slime_block',
    'honey_block',
    'nether_portal',
    'end_gateway',
]

const NON_OCCLUDING_BLOCK_PARTS = [
    'observer',
    'leaves',
    'pane',
    'fence',
    'wall',
    'door',
    'trapdoor',
    'slab',
    'stairs',
    'plant',
    'flower',
    'sapling',
    'grass',
    'fern',
    'bamboo',
    'sugar_cane',
    'cactus',
    'kelp',
    'seagrass',
    'vine',
    'torch',
    'rail',
    'button',
    'pressure_plate',
    'carpet',
    'candle',
    'chain',
    'rod',
    'mushroom',
    'fungus',
    'roots',
    'crops',
    'wheat',
    'carrots',
    'potatoes',
    'beetroots',
    'stem',
    'berry',
    'lichen',
    'ladder',
    'scaffolding',
    'iron_bars',
    'coral',
    'sea_pickle',
    'frogspawn',
    'campfire',
    'brewing_stand',
    'grindstone',
    'chest',
]

const SCHEMATIC_OCCLUSION_PLANES: Record<
    SchematicDirection,
    {
        axis: 'x' | 'y' | 'z'
        boundary: number
        projection: readonly ['x' | 'y' | 'z', 'x' | 'y' | 'z']
    }
> = {
    west: { axis: 'x', boundary: 0, projection: ['y', 'z'] },
    east: { axis: 'x', boundary: 1, projection: ['y', 'z'] },
    down: { axis: 'y', boundary: 0, projection: ['x', 'z'] },
    up: { axis: 'y', boundary: 1, projection: ['x', 'z'] },
    north: { axis: 'z', boundary: 0, projection: ['x', 'y'] },
    south: { axis: 'z', boundary: 1, projection: ['x', 'y'] },
}

const OCCLUSION_GRID_SIZE = 16
const OCCLUSION_EPSILON = 1e-5

type SchematicPoint2d = readonly [number, number]

function pointInTriangle(
    point: SchematicPoint2d,
    first: SchematicPoint2d,
    second: SchematicPoint2d,
    third: SchematicPoint2d,
) {
    const area =
        (second[0] - first[0]) * (third[1] - first[1]) -
        (second[1] - first[1]) * (third[0] - first[0])
    if (Math.abs(area) <= OCCLUSION_EPSILON) return false
    const edge = (start: SchematicPoint2d, end: SchematicPoint2d) =>
        (point[0] - end[0]) * (start[1] - end[1]) - (start[0] - end[0]) * (point[1] - end[1])
    const firstEdge = edge(first, second)
    const secondEdge = edge(second, third)
    const thirdEdge = edge(third, first)
    const hasNegative =
        firstEdge < -OCCLUSION_EPSILON ||
        secondEdge < -OCCLUSION_EPSILON ||
        thirdEdge < -OCCLUSION_EPSILON
    const hasPositive =
        firstEdge > OCCLUSION_EPSILON ||
        secondEdge > OCCLUSION_EPSILON ||
        thirdEdge > OCCLUSION_EPSILON
    return !(hasNegative && hasPositive)
}

export function getSchematicMeshOcclusionFaces(mesh: Mesh): SchematicOcclusionFaces {
    const coverage = Object.fromEntries(
        SCHEMATIC_DIRECTIONS.map((direction) => [direction, new Uint8Array(256)]),
    ) as Record<SchematicDirection, Uint8Array>

    for (const quad of mesh.quads) {
        const vertices = quad.vertices()
        for (const direction of SCHEMATIC_DIRECTIONS) {
            const plane = SCHEMATIC_OCCLUSION_PLANES[direction]
            if (
                !vertices.every(
                    (vertex) =>
                        Math.abs(vertex.pos[plane.axis] - plane.boundary) <= OCCLUSION_EPSILON,
                )
            ) {
                continue
            }
            const points = vertices.map(
                (vertex) =>
                    [
                        vertex.pos[plane.projection[0]],
                        vertex.pos[plane.projection[1]],
                    ] as SchematicPoint2d,
            )
            for (let row = 0; row < OCCLUSION_GRID_SIZE; row += 1) {
                for (let column = 0; column < OCCLUSION_GRID_SIZE; column += 1) {
                    const point: SchematicPoint2d = [
                        (column + 0.5) / OCCLUSION_GRID_SIZE,
                        (row + 0.5) / OCCLUSION_GRID_SIZE,
                    ]
                    if (
                        pointInTriangle(point, points[0], points[1], points[2]) ||
                        pointInTriangle(point, points[0], points[2], points[3])
                    ) {
                        coverage[direction][row * OCCLUSION_GRID_SIZE + column] = 1
                    }
                }
            }
        }
    }

    return Object.fromEntries(
        SCHEMATIC_DIRECTIONS.filter((direction) => coverage[direction].every(Boolean)).map(
            (direction) => [direction, true],
        ),
    ) as SchematicOcclusionFaces
}

const CHEST_TEXTURES: Record<string, string> = {
    'minecraft:chest': 'normal',
    'minecraft:trapped_chest': 'trapped',
    'minecraft:ender_chest': 'ender',
    'minecraft:copper_chest': 'copper',
    'minecraft:exposed_copper_chest': 'copper_exposed',
    'minecraft:weathered_copper_chest': 'copper_weathered',
    'minecraft:oxidized_copper_chest': 'copper_oxidized',
    'minecraft:waxed_copper_chest': 'copper',
    'minecraft:waxed_exposed_copper_chest': 'copper_exposed',
    'minecraft:waxed_weathered_copper_chest': 'copper_weathered',
    'minecraft:waxed_oxidized_copper_chest': 'copper_oxidized',
}

type ChestHalf = 'left' | 'right'
type ChestPosition = readonly [number, number, number]
type ChestTextureUv = readonly [number, number, number, number]

function addChestFace(
    mesh: Mesh,
    positions: readonly [ChestPosition, ChestPosition, ChestPosition, ChestPosition],
    atlasUv: ChestTextureUv,
    textureUv: ChestTextureUv,
) {
    const [atlasU0, atlasV0, atlasU1, atlasV1] = atlasUv
    const atlasU = (pixel: number) => atlasU0 + (pixel / 64) * (atlasU1 - atlasU0)
    const atlasV = (pixel: number) => atlasV0 + (pixel / 64) * (atlasV1 - atlasV0)
    const [textureU0, textureV0, textureU1, textureV1] = textureUv
    const mappedUv = [
        atlasU(textureU1),
        atlasV(textureV0),
        atlasU(textureU0),
        atlasV(textureV0),
        atlasU(textureU0),
        atlasV(textureV1),
        atlasU(textureU1),
        atlasV(textureV1),
    ]
    const textureLimit: [number, number, number, number] = [
        Math.min(mappedUv[0], mappedUv[2]),
        Math.min(mappedUv[1], mappedUv[5]),
        Math.max(mappedUv[0], mappedUv[2]),
        Math.max(mappedUv[1], mappedUv[5]),
    ]
    const quad = Quad.fromPoints(
        new Vector(...positions[0]),
        new Vector(...positions[1]),
        new Vector(...positions[2]),
        new Vector(...positions[3]),
    )
    quad.setColor([1, 1, 1]).setTexture(mappedUv, textureLimit)
    mesh.quads.push(quad)
}

function addChestCube(
    mesh: Mesh,
    atlasUv: ChestTextureUv,
    from: ChestPosition,
    geometrySize: ChestPosition,
    textureSize: ChestPosition,
    textureOffset: readonly [number, number],
    sideCrop = 0,
) {
    const [x0, y0, z0] = from
    const [geometryWidth, geometryHeight, geometryDepth] = geometrySize
    const [textureWidth, textureHeight, textureDepth] = textureSize
    const x1 = x0 + geometryWidth
    const y1 = y0 + geometryHeight
    const z1 = z0 + geometryDepth
    const p000 = [x0, y0, z0] as const
    const p100 = [x1, y0, z0] as const
    const p110 = [x1, y1, z0] as const
    const p010 = [x0, y1, z0] as const
    const p001 = [x0, y0, z1] as const
    const p101 = [x1, y0, z1] as const
    const p111 = [x1, y1, z1] as const
    const p011 = [x0, y1, z1] as const
    const [textureU, textureV] = textureOffset
    const u0 = textureU
    const u1 = u0 + textureDepth
    const u2 = u1 + textureWidth
    const u3 = u2 + textureWidth
    const u4 = u2 + textureDepth
    const u5 = u4 + textureWidth
    const v0 = textureV
    const v1 = v0 + textureDepth
    const v2 = v1 + textureHeight

    addChestFace(mesh, [p101, p001, p000, p100], atlasUv, [u1, v0, u2, v1])
    addChestFace(mesh, [p110, p010, p011, p111], atlasUv, [u2, v1, u3, v0])
    addChestFace(mesh, [p000, p001, p011, p010], atlasUv, [u0, v1 + sideCrop, u1, v2])
    addChestFace(mesh, [p100, p000, p010, p110], atlasUv, [u1, v1 + sideCrop, u2, v2])
    addChestFace(mesh, [p101, p100, p110, p111], atlasUv, [u2, v1 + sideCrop, u4, v2])
    addChestFace(mesh, [p001, p101, p111, p011], atlasUv, [u4, v1 + sideCrop, u5, v2])
}

function chestTransform(facing: string) {
    const [cosine, sine] =
        facing === 'east'
            ? [0, 1]
            : facing === 'north'
              ? [-1, 0]
              : facing === 'west'
                ? [0, -1]
                : [1, 0]
    const scale = 1 / 16
    const translateX = 8 - 8 * cosine - 8 * sine
    const translateZ = 8 + 8 * sine - 8 * cosine
    return new Float32Array([
        cosine * scale,
        0,
        -sine * scale,
        0,
        0,
        scale,
        0,
        0,
        sine * scale,
        0,
        cosine * scale,
        0,
        translateX * scale,
        0,
        translateZ * scale,
        1,
    ])
}

function getDoubleChestMesh(
    state: SchematicBlockState,
    texture: string,
    half: ChestHalf,
    atlas: TextureAtlasProvider,
) {
    const atlasUv = atlas.getTextureUV(
        Identifier.parse(`minecraft:entity/chest/${texture}_${half}`),
    )
    const bodyFromX = half === 'left' ? 0 : 1
    const lockFromX = half === 'left' ? 0 : 15
    const mesh = new Mesh()
    addChestCube(mesh, atlasUv, [bodyFromX, 0, 1], [15, 10, 14], [15, 10, 14], [0, 19])
    addChestCube(mesh, atlasUv, [bodyFromX, 10, 1], [15, 4, 14], [15, 5, 14], [0, 0], 1)
    addChestCube(mesh, atlasUv, [lockFromX, 7, 15], [1, 4, 1], [1, 4, 1], [0, 0])
    return mesh.transform(chestTransform(state.properties.facing ?? 'south'))
}

function normalizedBlockName(name: string) {
    return name.trim().toLowerCase()
}

export function getSchematicSpecialBlockMesh(
    state: SchematicBlockState,
    atlas: TextureAtlasProvider,
    cull: Cull,
) {
    const normalizedName = normalizedBlockName(state.name)
    const chestTexture = CHEST_TEXTURES[normalizedName]
    const chestType = state.properties.type
    if (
        chestTexture !== undefined &&
        normalizedName !== 'minecraft:ender_chest' &&
        (chestType === 'left' || chestType === 'right')
    ) {
        return getDoubleChestMesh(state, chestTexture, chestType, atlas)
    }
    return SpecialRenderers.getBlockMesh(
        new BlockState(state.name, state.properties),
        undefined,
        atlas,
        cull,
    )
}

const WATERLOGGED_FLUID_INSET = 1 / 64

export function isSchematicWaterlogged(state: SchematicBlockState) {
    return state.properties.waterlogged === 'true'
}

/**
 * Vanilla keeps the fluid of a waterlogged block flush with the block bounds, where it shares
 * planes with the host block's own faces. Translucent geometry is drawn after opaque geometry with
 * `depthFunc: LessEqual`, so a flush fluid would paint over cutout blocks such as leaves. Pulling
 * the fluid a fraction of a pixel inside keeps it visible only through the host block's gaps.
 */
function insetWaterloggedFluid(mesh: Mesh) {
    const inset = (value: number) =>
        value <= 0 ? WATERLOGGED_FLUID_INSET : value >= 1 ? 1 - WATERLOGGED_FLUID_INSET : value
    for (const quad of mesh.quads) {
        for (const vertex of quad.vertices()) {
            vertex.pos = new Vector(inset(vertex.pos.x), inset(vertex.pos.y), inset(vertex.pos.z))
        }
    }
    return mesh
}

/** Fluid held by a waterlogged block, meshed apart from the block so it can stay translucent. */
export function getSchematicWaterloggedFluidMesh(
    state: SchematicBlockState,
    atlas: TextureAtlasProvider,
    cull: Cull,
) {
    if (!isSchematicWaterlogged(state)) return undefined
    const fluid = getSchematicSpecialBlockMesh(
        { name: 'minecraft:water', properties: { level: '0' } },
        atlas,
        cull,
    )
    return fluid.quads.length > 0 ? insetWaterloggedFluid(fluid) : undefined
}

function seamlessGlassId(name: string) {
    const normalized = normalizedBlockName(name)
    const path = normalized.split(':').at(-1) ?? normalized
    return path === 'glass' || path === 'tinted_glass' || path.endsWith('_stained_glass')
        ? normalized
        : undefined
}

export function isSchematicTranslucent(name: string) {
    const normalized = normalizedBlockName(name)
    return TRANSLUCENT_BLOCK_PARTS.some((part) => normalized.includes(part))
}

export function isSeamlessSchematicGlassPair(currentName: string, neighborName: string) {
    const current = seamlessGlassId(currentName)
    return current !== undefined && current === seamlessGlassId(neighborName)
}

export function isSchematicOccluding(
    paletteIndex: number,
    palette: readonly SchematicBlockState[],
) {
    const state = palette[paletteIndex]
    return (
        state !== undefined &&
        !isSchematicAir(state.name) &&
        !isSchematicTranslucent(state.name) &&
        !NON_OCCLUDING_BLOCK_PARTS.some((part) => state.name.includes(part))
    )
}

export function shouldCullSchematicFace(
    currentPaletteIndex: number,
    neighborPaletteIndex: number,
    palette: readonly SchematicBlockState[],
    seamlessGlass = true,
    neighborOccludes = isSchematicOccluding(neighborPaletteIndex, palette),
) {
    if (neighborOccludes) return true
    if (!seamlessGlass) return false
    const current = palette[currentPaletteIndex]
    const neighbor = palette[neighborPaletteIndex]
    return Boolean(current && neighbor && isSeamlessSchematicGlassPair(current.name, neighbor.name))
}

export function schematicNeighborChunkPosition(
    position: readonly [number, number, number],
    direction: SchematicDirection,
): [number, number, number] {
    const offset = SCHEMATIC_DIRECTION_OFFSETS[direction]
    return [position[0] + offset[0], position[1] + offset[1], position[2] + offset[2]]
}

export function extractSchematicNeighborFace(blocks: Uint32Array, direction: SchematicDirection) {
    const face = new Uint32Array(256)
    for (let first = 0; first < 16; first += 1) {
        for (let second = 0; second < 16; second += 1) {
            if (direction === 'west' || direction === 'east') {
                const x = direction === 'west' ? 15 : 0
                face[first * 16 + second] = blocks[first * 256 + second * 16 + x] ?? 0
            } else if (direction === 'down' || direction === 'up') {
                const y = direction === 'down' ? 15 : 0
                face[first * 16 + second] = blocks[y * 256 + first * 16 + second] ?? 0
            } else {
                const z = direction === 'north' ? 15 : 0
                face[first * 16 + second] = blocks[first * 256 + z * 16 + second] ?? 0
            }
        }
    }
    return face
}

export function schematicBlockAt(
    blocks: Uint32Array,
    neighborFaces: SchematicNeighborFaces,
    x: number,
    y: number,
    z: number,
) {
    if (x === -1 && y >= 0 && y < 16 && z >= 0 && z < 16) {
        return neighborFaces.west?.[y * 16 + z] ?? 0
    }
    if (x === 16 && y >= 0 && y < 16 && z >= 0 && z < 16) {
        return neighborFaces.east?.[y * 16 + z] ?? 0
    }
    if (y === -1 && x >= 0 && x < 16 && z >= 0 && z < 16) {
        return neighborFaces.down?.[z * 16 + x] ?? 0
    }
    if (y === 16 && x >= 0 && x < 16 && z >= 0 && z < 16) {
        return neighborFaces.up?.[z * 16 + x] ?? 0
    }
    if (z === -1 && x >= 0 && x < 16 && y >= 0 && y < 16) {
        return neighborFaces.north?.[y * 16 + x] ?? 0
    }
    if (z === 16 && x >= 0 && x < 16 && y >= 0 && y < 16) {
        return neighborFaces.south?.[y * 16 + x] ?? 0
    }
    if (x < 0 || x >= 16 || y < 0 || y >= 16 || z < 0 || z >= 16) return 0
    return blocks[y * 256 + z * 16 + x] ?? 0
}

function holdsSchematicWater(state: SchematicBlockState) {
    return isSchematicWaterlogged(state) || normalizedBlockName(state.name) === 'minecraft:water'
}

/**
 * Waterlogged fluid is inset away from every block boundary so the host block's own faces stay in
 * front of it. Two adjacent waterlogged blocks would then show a seam where their fluid volumes
 * pull apart, so fluid faces shared with another water-holding block are culled the way vanilla
 * culls faces between two cells of the same fluid.
 */
export function getSchematicFluidCull(
    position: readonly [number, number, number],
    blocks: Uint32Array,
    neighborFaces: SchematicNeighborFaces,
    palette: readonly SchematicBlockState[],
    cull: Cull,
) {
    const fluidCull: Cull = { ...cull }
    for (const direction of SCHEMATIC_DIRECTIONS) {
        if (fluidCull[direction]) continue
        const offset = SCHEMATIC_DIRECTION_OFFSETS[direction]
        const neighbor =
            palette[
                schematicBlockAt(
                    blocks,
                    neighborFaces,
                    position[0] + offset[0],
                    position[1] + offset[1],
                    position[2] + offset[2],
                )
            ]
        if (neighbor && holdsSchematicWater(neighbor)) fluidCull[direction] = true
    }
    return fluidCull
}

export function applySeamlessSchematicGlassUvs(
    vertices: SchematicTexturedVertex[],
    position: readonly [number, number, number],
    currentPaletteIndex: number,
    blocks: Uint32Array,
    neighborFaces: SchematicNeighborFaces,
    palette: readonly SchematicBlockState[],
    seamlessGlass = true,
) {
    if (!seamlessGlass) return
    const current = palette[currentPaletteIndex]
    if (!current || !seamlessGlassId(current.name) || vertices.length < 3) return
    const textured = vertices.every((vertex) => vertex.texture !== undefined)
    if (!textured) return

    const edge1 = [
        vertices[1].pos.x - vertices[0].pos.x,
        vertices[1].pos.y - vertices[0].pos.y,
        vertices[1].pos.z - vertices[0].pos.z,
    ]
    const edge2 = [
        vertices[2].pos.x - vertices[0].pos.x,
        vertices[2].pos.y - vertices[0].pos.y,
        vertices[2].pos.z - vertices[0].pos.z,
    ]
    const normal = [
        edge1[1] * edge2[2] - edge1[2] * edge2[1],
        edge1[2] * edge2[0] - edge1[0] * edge2[2],
        edge1[0] * edge2[1] - edge1[1] * edge2[0],
    ]
    const normalAxis = normal.reduce(
        (best, value, axis) => (Math.abs(value) > Math.abs(normal[best]) ? axis : best),
        0,
    )
    const originalTextures = vertices.map((vertex) => [...vertex.texture!] as [number, number])
    const coordinates = vertices.map((vertex) => [vertex.pos.x, vertex.pos.y, vertex.pos.z])

    for (const direction of SCHEMATIC_DIRECTIONS) {
        const offset = SCHEMATIC_DIRECTION_OFFSETS[direction]
        const axis = offset.findIndex((value) => value !== 0)
        if (axis === normalAxis) continue
        const neighborIndex = schematicBlockAt(
            blocks,
            neighborFaces,
            position[0] + offset[0],
            position[1] + offset[1],
            position[2] + offset[2],
        )
        const neighbor = palette[neighborIndex]
        if (!neighbor || !isSeamlessSchematicGlassPair(current.name, neighbor.name)) continue

        const axisCoordinates = coordinates.map((coordinate) => coordinate[axis])
        const edgeCoordinate =
            offset[axis] < 0 ? Math.min(...axisCoordinates) : Math.max(...axisCoordinates)
        const edgeVertices = axisCoordinates
            .map((coordinate, index) => ({ coordinate, index }))
            .filter(({ coordinate }) => Math.abs(coordinate - edgeCoordinate) < 1e-5)
            .map(({ index }) => index)
        if (edgeVertices.length !== 2) continue
        const innerVertices = vertices
            .map((_, index) => index)
            .filter((index) => !edgeVertices.includes(index))
        for (const textureAxis of [0, 1] as const) {
            const edgeValues = edgeVertices.map((index) => originalTextures[index][textureAxis])
            if (Math.abs(edgeValues[0] - edgeValues[1]) >= 1e-7) continue
            const innerValue =
                innerVertices.reduce(
                    (sum, index) => sum + originalTextures[index][textureAxis],
                    0,
                ) / innerVertices.length
            const edgeValue = edgeValues[0]
            const shift = (innerValue - edgeValue) / 16
            if (Math.abs(shift) < 1e-7) continue
            for (const index of edgeVertices) {
                vertices[index].texture![textureAxis] = originalTextures[index][textureAxis] + shift
            }
            break
        }
    }
}

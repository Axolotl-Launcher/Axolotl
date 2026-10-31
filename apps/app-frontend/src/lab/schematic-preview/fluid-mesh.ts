import {
    BlockColors,
    type Color,
    type Cull,
    Identifier,
    Mesh,
    Quad,
    type TextureAtlasProvider,
    type UV,
    Vector,
} from 'deepslate'

import type { SchematicBlockState } from './backend'
import { isSchematicAir } from './editing'

const WATER_BLOCK = 'minecraft:water'
/** Vanilla fills a source cell to 8/9 of the block and lowers each flowing level by a ninth. */
const FLUID_SOURCE_HEIGHT = 8 / 9
const FLUID_LEVEL_COUNT = 9
/** A `level` of 8 or more marks a falling column, which fills the cell instead of sloping. */
const FLUID_FALLING_LEVEL = 8
/** Vanilla counts near-full fluid ten times as much as shallow fluid when averaging corners. */
const FLUID_FULL_HEIGHT = 0.8
const FLUID_FULL_HEIGHT_WEIGHT = 10
/** Fluid faces sit a thousandth of a cell inside the block bounds so they never z-fight. */
const FLUID_FACE_INSET = 0.001
const FLUID_STILL_TEXTURE = 'minecraft:block/water_still'
const FLUID_FLOW_TEXTURE = 'minecraft:block/water_flow'
/** Fluid sprites are 16x16, so the flow texture is mapped in sprite pixels before the atlas lookup. */
const FLUID_SPRITE_SIZE = 16
/** Vanilla pulls rotated flow UVs a quarter of the sprite towards their average to stay in bounds. */
const FLUID_FLOW_UV_SHRINK = 4 / FLUID_SPRITE_SIZE

type SpritePoint = readonly [number, number]
type Position = readonly [number, number, number]

export type SchematicFluidNeighborhood = {
    /** State at an offset from the fluid cell, or `undefined` where the volume carries no data. */
    blockAt: (dx: number, dy: number, dz: number) => SchematicBlockState | undefined
}

export type SchematicFluidMeshOptions = {
    state: SchematicBlockState
    atlas: TextureAtlasProvider
    cull: Cull
    /** Decides the corner heights; a fluid without neighbours stays flat, like water in a block. */
    neighborhood?: SchematicFluidNeighborhood
    /** Distance the fluid faces sit inside the block bounds. */
    inset?: number
}

export function isSchematicWaterlogged(state: SchematicBlockState) {
    return state.properties.waterlogged === 'true'
}

export function isSchematicWaterBlock(state: SchematicBlockState) {
    return state.name.trim().toLowerCase() === WATER_BLOCK
}

export function holdsSchematicWater(state: SchematicBlockState) {
    return isSchematicWaterlogged(state) || isSchematicWaterBlock(state)
}

/** Surface height of the water a cell holds, or `undefined` when the cell holds no water. */
export function schematicFluidHeight(state: SchematicBlockState) {
    if (!holdsSchematicWater(state)) return undefined
    const level = Number.parseInt(state.properties.level ?? '0', 10)
    if (!Number.isFinite(level) || level <= 0) return FLUID_SOURCE_HEIGHT
    if (level >= FLUID_FALLING_LEVEL) return 1
    return (FLUID_FALLING_LEVEL - level) / FLUID_LEVEL_COUNT
}

/**
 * Vanilla averages the 2x2 cells around a corner: fluid counts with its height and near-full fluid
 * counts ten times over, air counts as an empty cell, and solid or unknown cells are skipped, which
 * keeps the surface at full height against a wall. A cell whose neighbour above holds the same
 * fluid is a falling column and fills the block.
 */
function cornerHeight(
    neighborhood: SchematicFluidNeighborhood,
    cornerX: number,
    cornerZ: number,
    fallback: number,
) {
    let samples = 0
    let total = 0
    for (let index = 0; index < 4; index += 1) {
        const dx = cornerX - (index & 1)
        const dz = cornerZ - ((index >> 1) & 1)
        const above = neighborhood.blockAt(dx, 1, dz)
        if (above && schematicFluidHeight(above) !== undefined) return 1
        const state = neighborhood.blockAt(dx, 0, dz)
        if (!state) continue
        const height = schematicFluidHeight(state)
        if (height === undefined) {
            if (isSchematicAir(state.name)) samples += 1
            continue
        }
        if (height >= FLUID_FULL_HEIGHT) {
            total += height * FLUID_FULL_HEIGHT_WEIGHT
            samples += FLUID_FULL_HEIGHT_WEIGHT
        } else {
            total += height
            samples += 1
        }
    }
    return samples > 0 ? total / samples : fallback
}

/**
 * Vanilla rotates the flowing surface along the fluid's velocity, which it derives from the heights
 * of the neighbouring cells. The same direction falls out of the surface gradient, and it stays
 * defined at a waterfall lip where no neighbour holds the fluid: water runs towards the low corners.
 */
function surfaceFlow(northWest: number, southWest: number, southEast: number, northEast: number) {
    const flowX = (northWest + southWest) / 2 - (southEast + northEast) / 2
    const flowZ = (northWest + northEast) / 2 - (southWest + southEast) / 2
    const length = Math.hypot(flowX, flowZ)
    return length > 1e-6 ? ([flowX / length, flowZ / length] as const) : undefined
}

/** The vanilla rotation of the flowing sprite around its centre, in sprite pixels. */
function flowSurfacePoints(flowX: number, flowZ: number): SpritePoint[] {
    const angle = Math.atan2(flowZ, flowX) - Math.PI / 2
    const center = FLUID_SPRITE_SIZE / 2
    const horizontal = Math.cos(angle) * 0.25 * FLUID_SPRITE_SIZE
    const vertical = Math.sin(angle) * 0.25 * FLUID_SPRITE_SIZE
    const points: SpritePoint[] = [
        [center - (horizontal + vertical), center - (horizontal - vertical)],
        [center - (horizontal - vertical), center + (horizontal + vertical)],
        [center + (horizontal + vertical), center + (horizontal - vertical)],
        [center + (horizontal - vertical), center - (horizontal + vertical)],
    ]
    return points.map(
        ([u, v]) =>
            [
                u + (center - u) * FLUID_FLOW_UV_SHRINK,
                v + (center - v) * FLUID_FLOW_UV_SHRINK,
            ] as SpritePoint,
    )
}

function spriteUv(uv: UV, [u, v]: SpritePoint): [number, number] {
    const [u0, v0, u1, v1] = uv
    return [u0 + (u / FLUID_SPRITE_SIZE) * (u1 - u0), v0 + (v / FLUID_SPRITE_SIZE) * (v1 - v0)]
}

function addQuad(
    mesh: Mesh,
    positions: readonly [Position, Position, Position, Position],
    sprite: UV,
    points: readonly SpritePoint[],
    tint: Color,
) {
    const quad = Quad.fromPoints(
        new Vector(...positions[0]),
        new Vector(...positions[1]),
        new Vector(...positions[2]),
        new Vector(...positions[3]),
    )
    quad.setColor(tint)
    quad.setTexture(points.flatMap((point) => spriteUv(sprite, point)))
    mesh.quads.push(quad)
}

/**
 * Vanilla fluid geometry: the top surface is a quad whose four corners come from the neighbouring
 * cells, and the side faces rise to those corners, so water flowing between two levels connects with
 * a slope instead of one flat box per cell.
 *
 * Only the side faces are pulled inside the block bounds. The surface and the floor keep the full
 * block extent, so two cells of the same fluid meet edge to edge even though the face between them
 * is culled.
 */
export function getSchematicFluidMesh(options: SchematicFluidMeshOptions): Mesh | undefined {
    const { state, atlas, cull, neighborhood } = options
    const height = schematicFluidHeight(state)
    if (height === undefined) return undefined
    const inset = options.inset ?? FLUID_FACE_INSET
    const corner = (x: number, z: number) =>
        neighborhood ? cornerHeight(neighborhood, x, z, height) : height
    const northWest = corner(0, 0)
    const southWest = corner(0, 1)
    const southEast = corner(1, 1)
    const northEast = corner(1, 0)
    // Vanilla lowers the four corners by the face inset while it draws the top face, and the side
    // faces rise to the same lowered edge. Lifting the sides to the raw corners instead leaves a
    // notch along the whole rim that sees straight into the fluid volume.
    const drop = cull.up ? 0 : FLUID_FACE_INSET
    const rimNorthWest = northWest - drop
    const rimSouthWest = southWest - drop
    const rimSouthEast = southEast - drop
    const rimNorthEast = northEast - drop
    const floor = cull.down ? 0 : inset

    const still = atlas.getTextureUV(Identifier.parse(FLUID_STILL_TEXTURE))
    const flow = atlas.getTextureUV(Identifier.parse(FLUID_FLOW_TEXTURE))
    const tint = BlockColors.water?.({}) ?? [1, 1, 1]
    const mesh = new Mesh()

    if (!cull.up) {
        const direction = surfaceFlow(northWest, southWest, southEast, northEast)
        addQuad(
            mesh,
            [
                [0, rimNorthWest, 0],
                [0, rimSouthWest, 1],
                [1, rimSouthEast, 1],
                [1, rimNorthEast, 0],
            ],
            direction ? flow : still,
            direction
                ? flowSurfacePoints(...direction)
                : [
                      [0, 0],
                      [0, FLUID_SPRITE_SIZE],
                      [FLUID_SPRITE_SIZE, FLUID_SPRITE_SIZE],
                      [FLUID_SPRITE_SIZE, 0],
                  ],
            tint,
        )
    }

    if (!cull.down) {
        addQuad(
            mesh,
            [
                [0, floor, 1],
                [0, floor, 0],
                [1, floor, 0],
                [1, floor, 1],
            ],
            still,
            [
                [0, FLUID_SPRITE_SIZE],
                [0, 0],
                [FLUID_SPRITE_SIZE, 0],
                [FLUID_SPRITE_SIZE, FLUID_SPRITE_SIZE],
            ],
            tint,
        )
    }

    // Vanilla crops the flowing sprite's lower half to the fluid height, so a shallow cell shows a
    // shorter stretch of the same texture instead of a squashed one.
    const sideHeight = (value: number) => (1 - value) * FLUID_SPRITE_SIZE * 0.5
    const sides = [
        {
            face: 'north',
            first: [0, inset],
            second: [1, inset],
            firstHeight: rimNorthWest,
            secondHeight: rimNorthEast,
        },
        {
            face: 'south',
            first: [1, 1 - inset],
            second: [0, 1 - inset],
            firstHeight: rimSouthEast,
            secondHeight: rimSouthWest,
        },
        {
            face: 'west',
            first: [inset, 1],
            second: [inset, 0],
            firstHeight: rimSouthWest,
            secondHeight: rimNorthWest,
        },
        {
            face: 'east',
            first: [1 - inset, 0],
            second: [1 - inset, 1],
            firstHeight: rimNorthEast,
            secondHeight: rimSouthEast,
        },
    ] as const

    for (const side of sides) {
        if (cull[side.face]) continue
        const [firstX, firstZ] = side.first
        const [secondX, secondZ] = side.second
        addQuad(
            mesh,
            [
                [secondX, side.secondHeight, secondZ],
                [secondX, floor, secondZ],
                [firstX, floor, firstZ],
                [firstX, side.firstHeight, firstZ],
            ],
            flow,
            [
                [FLUID_SPRITE_SIZE / 2, sideHeight(side.secondHeight)],
                [FLUID_SPRITE_SIZE / 2, FLUID_SPRITE_SIZE / 2],
                [0, FLUID_SPRITE_SIZE / 2],
                [0, sideHeight(side.firstHeight)],
            ],
            tint,
        )
    }

    return mesh.quads.length > 0 ? mesh : undefined
}

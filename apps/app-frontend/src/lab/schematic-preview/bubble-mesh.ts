import type { SchematicBlockState } from './backend'

/**
 * A bubble column draws no block model in the game: its water comes from the fluid state, and the
 * bubbles are the `bubble_column_up` particle, whose sprite is `minecraft:bubble`. The preview has
 * no particle engine, so the same sprite is baked into two crossed quads per bubble, placed by a
 * hash of the block position so a chunk always meshes the same way.
 */
export const BUBBLES_PER_CELL = 5

/** Centre range inside the cell, so no bubble pokes through a wall. */
const BUBBLE_MARGIN = 0.25
/** The water surface sits at 8/9, so the bubbles stay well below it. */
const BUBBLE_CEILING = 0.7
const BUBBLE_RADIUS_MINIMUM = 0.075
const BUBBLE_RADIUS_MAXIMUM = 0.125

export type SchematicBubbleVertex = {
    position: [number, number, number]
    uv: [number, number]
}

export type SchematicBubbleQuad = [
    SchematicBubbleVertex,
    SchematicBubbleVertex,
    SchematicBubbleVertex,
    SchematicBubbleVertex,
]

export function isSchematicBubbleColumn(state: SchematicBlockState) {
    return state.name.trim().toLowerCase() === 'minecraft:bubble_column'
}

function cellSeed(x: number, y: number, z: number) {
    let hash = 0x811c9dc5
    for (const value of [x, y, z]) {
        for (let shift = 0; shift < 32; shift += 8) {
            hash = Math.imul(hash ^ ((value >> shift) & 0xff), 0x01000193)
        }
    }
    return hash >>> 0
}

/** xorshift32: small, deterministic, and enough for scattering bubbles. */
function createRandom(seed: number) {
    let state = seed || 0x9e3779b9
    return () => {
        state ^= state << 13
        state >>>= 0
        state ^= state >>> 17
        state ^= state << 5
        state >>>= 0
        return state / 0x100000000
    }
}

export function getSchematicBubbleQuads(position: [number, number, number]): SchematicBubbleQuad[] {
    const [cellX, cellY, cellZ] = position
    const random = createRandom(cellSeed(cellX, cellY, cellZ))
    const quads: SchematicBubbleQuad[] = []
    for (let bubble = 0; bubble < BUBBLES_PER_CELL; bubble += 1) {
        const centerX = cellX + BUBBLE_MARGIN + random() * (1 - BUBBLE_MARGIN * 2)
        const centerZ = cellZ + BUBBLE_MARGIN + random() * (1 - BUBBLE_MARGIN * 2)
        const centerY = cellY + BUBBLE_MARGIN + random() * (BUBBLE_CEILING - BUBBLE_MARGIN)
        const radius =
            BUBBLE_RADIUS_MINIMUM + random() * (BUBBLE_RADIUS_MAXIMUM - BUBBLE_RADIUS_MINIMUM)
        const yaw = random() * Math.PI
        // Two quads at right angles, so the sprite reads as a bubble from any direction.
        for (const angle of [yaw, yaw + Math.PI / 2]) {
            const offsetX = Math.cos(angle) * radius
            const offsetZ = Math.sin(angle) * radius
            quads.push([
                { position: [centerX - offsetX, centerY + radius, centerZ - offsetZ], uv: [0, 0] },
                { position: [centerX + offsetX, centerY + radius, centerZ + offsetZ], uv: [1, 0] },
                { position: [centerX + offsetX, centerY - radius, centerZ + offsetZ], uv: [1, 1] },
                { position: [centerX - offsetX, centerY - radius, centerZ - offsetZ], uv: [0, 1] },
            ])
        }
    }
    return quads
}

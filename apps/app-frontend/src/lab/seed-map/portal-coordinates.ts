import type { SeedMapDimension } from './biomes.ts'

/** Horizontal scale between the Overworld and the Nether. */
export const PORTAL_COORDINATE_SCALE = 8

export type PortalCounterpart = {
    dimension: SeedMapDimension
    x: number
    z: number
}

/**
 * Overworld to Nether. Minecraft converts portal positions with `Mth.floor`, so negative
 * coordinates round down as well: overworld X=-1 maps to nether X=-1, not X=0.
 */
export function netherFromOverworld(value: number): number {
    return Math.floor(value / PORTAL_COORDINATE_SCALE)
}

/** Nether to Overworld. Scaling up is exact, so no rounding is involved. */
export function overworldFromNether(value: number): number {
    return value * PORTAL_COORDINATE_SCALE
}

/**
 * The position the same portal would link to in the paired dimension. The End has no
 * horizontal counterpart, so it returns null.
 */
export function portalCounterpart(
    dimension: SeedMapDimension,
    x: number,
    z: number,
): PortalCounterpart | null {
    if (dimension === 'overworld') {
        return {
            dimension: 'nether',
            x: netherFromOverworld(x),
            z: netherFromOverworld(z),
        }
    }
    if (dimension === 'nether') {
        return {
            dimension: 'overworld',
            x: overworldFromNether(x),
            z: overworldFromNether(z),
        }
    }
    return null
}

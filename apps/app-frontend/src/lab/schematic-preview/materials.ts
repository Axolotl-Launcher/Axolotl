import type { SchematicFluidKind } from './fluid-mesh'

/**
 * The passes a chunk is drawn in. Every fluid gets one of its own, so its opacity never has to be
 * shared, and the list doubles as the order the scene builds its materials and walks its meshes.
 */
export type SchematicMaterialKind = 'opaque' | 'translucent' | SchematicFluidKind

export const SCHEMATIC_MATERIAL_KINDS: readonly SchematicMaterialKind[] = [
    'opaque',
    'translucent',
    'water',
    'lava',
]

/** Glass and ice: solid enough to read the shape, sheer enough to see what is behind it. */
export const SCHEMATIC_TRANSLUCENT_OPACITY = 0.68

/** Water: it hides most of what it covers without turning into a wall. */
export const SCHEMATIC_WATER_OPACITY = 0.82

/**
 * Lava: all but opaque. It still belongs to the translucent blocks, so the eye toggle hides it with
 * the glass, and it keeps writing depth so the water behind it stays hidden.
 */
export const SCHEMATIC_LAVA_OPACITY = 0.95

export type SchematicMaterialSettings = {
    transparent: boolean
    alphaTest: number
    opacity: number
    depthWrite: boolean
}

export function schematicMaterialSettings(kind: SchematicMaterialKind): SchematicMaterialSettings {
    if (kind === 'opaque') {
        return { transparent: false, alphaTest: 0.08, opacity: 1, depthWrite: true }
    }
    if (kind === 'lava') {
        return {
            transparent: true,
            alphaTest: 0,
            opacity: SCHEMATIC_LAVA_OPACITY,
            depthWrite: true,
        }
    }
    return {
        transparent: true,
        alphaTest: 0,
        opacity: kind === 'water' ? SCHEMATIC_WATER_OPACITY : SCHEMATIC_TRANSLUCENT_OPACITY,
        depthWrite: false,
    }
}

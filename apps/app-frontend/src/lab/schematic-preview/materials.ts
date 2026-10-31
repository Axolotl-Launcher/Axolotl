import type { SchematicFluidKind } from './fluid-mesh'

/**
 * The passes a chunk is drawn in. Geometry of one block can only land in one of them, so the list
 * doubles as the order the scene builds its materials and walks its meshes.
 */
export type SchematicMaterialKind = 'opaque' | 'translucent' | 'fluid'

export const SCHEMATIC_MATERIAL_KINDS: readonly SchematicMaterialKind[] = [
    'opaque',
    'translucent',
    'fluid',
]

/** Glass and ice: solid enough to read the shape, sheer enough to see what is behind it. */
export const SCHEMATIC_TRANSLUCENT_OPACITY = 0.68

/** Water: it hides most of what it covers without turning into a wall. */
export const SCHEMATIC_FLUID_OPACITY = 0.82

export type SchematicMaterialSettings = {
    transparent: boolean
    alphaTest: number
    opacity: number
    depthWrite: boolean
}

/**
 * The pass a fluid's geometry belongs to. Lava is opaque in the game, so it draws with the solid
 * blocks and occludes what is behind it; water blends and needs its own pass, because sharing one
 * with the glass would tie its opacity to theirs.
 */
export function schematicFluidMeshLayer(kind: SchematicFluidKind): SchematicMaterialKind {
    return kind === 'lava' ? 'opaque' : 'fluid'
}

export function schematicMaterialSettings(kind: SchematicMaterialKind): SchematicMaterialSettings {
    if (kind === 'opaque') {
        return { transparent: false, alphaTest: 0.08, opacity: 1, depthWrite: true }
    }
    return {
        transparent: true,
        alphaTest: 0,
        opacity: kind === 'fluid' ? SCHEMATIC_FLUID_OPACITY : SCHEMATIC_TRANSLUCENT_OPACITY,
        depthWrite: false,
    }
}

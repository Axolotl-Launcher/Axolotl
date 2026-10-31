import { describe, expect, it } from 'vitest'

import {
    SCHEMATIC_FLUID_OPACITY,
    SCHEMATIC_MATERIAL_KINDS,
    SCHEMATIC_TRANSLUCENT_OPACITY,
    schematicFluidMeshLayer,
    schematicMaterialSettings,
} from './materials'

describe('schematic materials', () => {
    it('draws water more opaque than glass, but still see-through', () => {
        const fluid = schematicMaterialSettings('fluid')
        const translucent = schematicMaterialSettings('translucent')

        expect(fluid.transparent).toBe(true)
        expect(fluid.opacity).toBe(SCHEMATIC_FLUID_OPACITY)
        expect(translucent.opacity).toBe(SCHEMATIC_TRANSLUCENT_OPACITY)
        expect(fluid.opacity).toBeGreaterThan(translucent.opacity)
        expect(fluid.opacity).toBeLessThan(1)
    })

    it('only the solid pass is opaque and writes depth', () => {
        const opaque = schematicMaterialSettings('opaque')
        expect(opaque).toEqual({
            transparent: false,
            alphaTest: 0.08,
            opacity: 1,
            depthWrite: true,
        })
        for (const kind of ['translucent', 'fluid'] as const) {
            expect(schematicMaterialSettings(kind)).toMatchObject({
                transparent: true,
                alphaTest: 0,
                depthWrite: false,
            })
        }
    })

    it('draws lava with the solid blocks, since it is opaque in the game', () => {
        expect(schematicFluidMeshLayer('lava')).toBe('opaque')
        expect(schematicFluidMeshLayer('water')).toBe('fluid')
    })

    it('builds every pass, so no geometry lands in a pass nothing renders', () => {
        expect(new Set(SCHEMATIC_MATERIAL_KINDS).size).toBe(SCHEMATIC_MATERIAL_KINDS.length)
        expect([...SCHEMATIC_MATERIAL_KINDS]).toEqual(
            expect.arrayContaining(['opaque', 'translucent', 'fluid']),
        )
    })
})

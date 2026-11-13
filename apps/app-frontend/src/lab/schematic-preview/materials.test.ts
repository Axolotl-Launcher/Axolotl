import { describe, expect, it } from 'vitest'

import {
    SCHEMATIC_LAVA_OPACITY,
    SCHEMATIC_MATERIAL_KINDS,
    SCHEMATIC_TRANSLUCENT_OPACITY,
    SCHEMATIC_WATER_OPACITY,
    schematicMaterialSettings,
} from './materials'

describe('schematic materials', () => {
    it('draws water more opaque than glass, but still see-through', () => {
        const water = schematicMaterialSettings('water')
        const translucent = schematicMaterialSettings('translucent')

        expect(water.transparent).toBe(true)
        expect(water.opacity).toBe(SCHEMATIC_WATER_OPACITY)
        expect(translucent.opacity).toBe(SCHEMATIC_TRANSLUCENT_OPACITY)
        expect(water.opacity).toBeGreaterThan(translucent.opacity)
        expect(water.opacity).toBeLessThan(1)
    })

    it('keeps lava a translucent block, nearly solid and still hiding what it covers', () => {
        const lava = schematicMaterialSettings('lava')

        expect(lava.transparent).toBe(true)
        expect(lava.opacity).toBe(SCHEMATIC_LAVA_OPACITY)
        expect(lava.opacity).toBeGreaterThan(SCHEMATIC_WATER_OPACITY)
        expect(lava.opacity).toBeLessThan(1)
        expect(lava.alphaTest).toBe(0)
        expect(lava.depthWrite).toBe(true)
    })

    it('draws the bubbles blended, full colour and without writing depth', () => {
        // They sit inside the water, so they must not occlude it; the sprite carries its own alpha.
        expect(schematicMaterialSettings('bubble')).toEqual({
            transparent: true,
            alphaTest: 0,
            opacity: 1,
            depthWrite: false,
        })
    })

    it('only the solid pass is opaque and clips on alpha', () => {
        expect(schematicMaterialSettings('opaque')).toEqual({
            transparent: false,
            alphaTest: 0.08,
            opacity: 1,
            depthWrite: true,
        })
        for (const kind of ['translucent', 'bubble', 'water', 'lava'] as const) {
            expect(schematicMaterialSettings(kind).transparent).toBe(true)
        }
    })

    it('builds every pass, so no geometry lands in a pass nothing renders', () => {
        expect(new Set(SCHEMATIC_MATERIAL_KINDS).size).toBe(SCHEMATIC_MATERIAL_KINDS.length)
        expect([...SCHEMATIC_MATERIAL_KINDS]).toEqual(
            expect.arrayContaining(['opaque', 'translucent', 'bubble', 'water', 'lava']),
        )
    })
})

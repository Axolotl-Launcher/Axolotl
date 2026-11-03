import { expect, it } from 'vitest'

import { escapeSchematicCsvCell, normalizeSchematicLayerRange } from './utils.ts'

it('layer ranges remain ordered and inside structure bounds', () => {
    expect(normalizeSchematicLayerRange(20, 5, -4, 16)).toEqual([5, 16])
    expect(normalizeSchematicLayerRange(-20, -10, -4, 16)).toEqual([-4, -4])
})

it('material CSV cells escape quotes, commas, and line breaks', () => {
    expect(escapeSchematicCsvCell('minecraft:stone')).toBe('minecraft:stone')
    expect(escapeSchematicCsvCell('name,"quoted"')).toBe('"name,""quoted"""')
    expect(escapeSchematicCsvCell('two\nlines')).toBe('"two\nlines"')
})

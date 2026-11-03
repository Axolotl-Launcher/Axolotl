import { expect, it } from 'vitest'

import { createJavaFormatter } from './formatter.ts'
import { parseIdentifier } from './identifier.ts'

it('formats Java 1.12 ingredients and results with legacy data values', () => {
    const formatter = createJavaFormatter('1.12')
    const stone = parseIdentifier('minecraft:stone:1')
    expect(formatter.ingredient(stone, false)).toEqual({ item: 'minecraft:stone', data: 1 })
    expect(formatter.result(stone, 2)).toEqual({
        item: 'minecraft:stone',
        data: 1,
        count: 2,
    })
    expect(formatter.typeName('crafting_shaped')).toBe('crafting_shaped')
})

it('formats Java 1.13 ingredients, tags, and string cooking results', () => {
    const formatter = createJavaFormatter('1.13')
    const iron = parseIdentifier('minecraft:iron_ingot')
    expect(formatter.ingredient(iron, false)).toEqual({ item: 'minecraft:iron_ingot' })
    expect(formatter.ingredient(iron, true)).toEqual({ tag: 'minecraft:iron_ingot' })
    expect(formatter.result(iron)).toEqual({ item: 'minecraft:iron_ingot' })
    expect(formatter.cookingResult(iron)).toBe('minecraft:iron_ingot')
    expect(formatter.typeName('smelting')).toBe('smelting')
})

it('namespaces recipe types from Java 1.14 onwards', () => {
    const formatter = createJavaFormatter('1.14')
    expect(formatter.typeName('smelting')).toBe('minecraft:smelting')
})

it('switches results to id objects from Java 1.20', () => {
    const formatter = createJavaFormatter('1.20')
    const iron = parseIdentifier('minecraft:iron_ingot')
    expect(formatter.result(iron, 3)).toEqual({ id: 'minecraft:iron_ingot', count: 3 })
    expect(formatter.cookingResult(iron)).toEqual({ id: 'minecraft:iron_ingot' })
    expect(formatter.stonecutterResult(iron, 4)).toEqual({
        result: { id: 'minecraft:iron_ingot', count: 4 },
    })
})

it('uses string ingredients and tag references from Java 1.21.2', () => {
    const formatter = createJavaFormatter('1.21.2')
    const iron = parseIdentifier('minecraft:iron_ingot')
    expect(formatter.ingredient(iron, false)).toBe('minecraft:iron_ingot')
    expect(formatter.ingredient(iron, true)).toBe('#minecraft:iron_ingot')
})

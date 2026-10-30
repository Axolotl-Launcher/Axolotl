import { expect, it } from 'vitest'

import { generateJavaRecipe } from './recipe-engine.ts'
import type { RecipeSlotContext, RecipeState } from './types.ts'

function context(): RecipeSlotContext {
    return {
        itemsById: {
            'minecraft:iron_ingot': {
                id: 'minecraft:iron_ingot',
                name: 'Iron Ingot',
                texture: 'iron.png',
            },
            'minecraft:stick': { id: 'minecraft:stick', name: 'Stick', texture: 'stick.png' },
            'minecraft:stone': { id: 'minecraft:stone', name: 'Stone', texture: 'stone.png' },
            'minecraft:stone_bricks': {
                id: 'minecraft:stone_bricks',
                name: 'Stone Bricks',
                texture: 'bricks.png',
            },
            'minecraft:diamond': {
                id: 'minecraft:diamond',
                name: 'Diamond',
                texture: 'diamond.png',
            },
            'minecraft:diamond_sword': {
                id: 'minecraft:diamond_sword',
                name: 'Diamond Sword',
                texture: 'sword.png',
            },
            'minecraft:iron_bars': {
                id: 'minecraft:iron_bars',
                name: 'Iron Bars',
                texture: 'bars.png',
            },
            'minecraft:oak_planks': {
                id: 'minecraft:oak_planks',
                name: 'Oak Planks',
                texture: 'planks.png',
            },
            'minecraft:beef': { id: 'minecraft:beef', name: 'Raw Beef', texture: 'beef.png' },
            'minecraft:cooked_beef': {
                id: 'minecraft:cooked_beef',
                name: 'Steak',
                texture: 'steak.png',
            },
        },
        customItemsByUid: {},
        customTagsByUid: {},
        vanillaTags: {
            'minecraft:planks': ['minecraft:oak_planks'],
        },
    }
}

function recipe(overrides: Partial<RecipeState> = {}): RecipeState {
    return {
        id: 'recipe-1',
        recipeType: 'crafting',
        group: '',
        category: '',
        showNotification: true,
        nameMode: 'auto',
        name: '',
        slots: {},
        crafting: { shapeless: false, keepWhitespace: false, twoByTwo: false },
        cooking: { time: null, experience: 0 },
        smithing: { trimPattern: '' },
        ...overrides,
    }
}

const ctx = context()

it('builds shaped crafting patterns and keys', () => {
    const state = recipe({
        slots: {
            'crafting.1': { kind: 'item', id: 'minecraft:iron_ingot' },
            'crafting.2': { kind: 'item', id: 'minecraft:iron_ingot' },
            'crafting.3': { kind: 'item', id: 'minecraft:iron_ingot' },
            'crafting.4': { kind: 'item', id: 'minecraft:stick' },
            'crafting.result': { kind: 'item', id: 'minecraft:iron_bars', count: 16 },
        },
    })
    const output = generateJavaRecipe(state, '1.21.2', ctx)
    expect(output.pattern).toEqual(['###', '/  '])
    expect(output.key).toEqual({
        '#': 'minecraft:iron_ingot',
        '/': 'minecraft:stick',
    })
    expect(output.result).toEqual({ id: 'minecraft:iron_bars', count: 16 })
    expect(output.type).toBe('minecraft:crafting_shaped')
})

it('builds shapeless crafting with tag ingredients', () => {
    const state = recipe({
        crafting: { shapeless: true, keepWhitespace: false, twoByTwo: false },
        slots: {
            'crafting.1': { kind: 'vanilla_tag', id: 'minecraft:planks' },
            'crafting.2': { kind: 'vanilla_tag', id: 'minecraft:planks' },
            'crafting.result': { kind: 'item', id: 'minecraft:stick' },
        },
    })
    const output = generateJavaRecipe(state, '1.21.2', ctx)
    expect(output.ingredients).toEqual(['#minecraft:planks', '#minecraft:planks'])
    expect(output.type).toBe('minecraft:crafting_shapeless')
})

it('two by two crafting only keeps the 2x2 grid', () => {
    const state = recipe({
        crafting: { shapeless: false, keepWhitespace: false, twoByTwo: true },
        slots: {
            'crafting.1': { kind: 'item', id: 'minecraft:oak_planks' },
            'crafting.2': { kind: 'item', id: 'minecraft:oak_planks' },
            'crafting.4': { kind: 'item', id: 'minecraft:oak_planks' },
            'crafting.5': { kind: 'item', id: 'minecraft:oak_planks' },
            'crafting.result': { kind: 'item', id: 'minecraft:oak_planks', count: 4 },
        },
    })
    const output = generateJavaRecipe(state, '1.21', ctx)
    expect(output.pattern).toEqual(['##', '##'])
})

it('formats smelting across version boundaries', () => {
    const state = recipe({
        recipeType: 'smelting',
        slots: {
            'cooking.ingredient': { kind: 'item', id: 'minecraft:beef' },
            'cooking.result': { kind: 'item', id: 'minecraft:cooked_beef' },
        },
        cooking: { time: null, experience: 0.35 },
    })
    const legacy = generateJavaRecipe(state, '1.14', ctx)
    expect(legacy.type).toBe('minecraft:smelting')
    expect(legacy.cookingtime).toBe(200)
    expect(legacy.result).toBe('minecraft:cooked_beef')

    const modern = generateJavaRecipe(state, '1.20', ctx)
    expect(modern.result).toEqual({ id: 'minecraft:cooked_beef' })
})

it('formats stonecutting result and count', () => {
    const state = recipe({
        recipeType: 'stonecutter',
        slots: {
            'stonecutter.ingredient': { kind: 'item', id: 'minecraft:stone' },
            'stonecutter.result': { kind: 'item', id: 'minecraft:stone_bricks', count: 2 },
        },
    })
    const legacy = generateJavaRecipe(state, '1.14', ctx)
    expect(legacy.result).toBe('minecraft:stone_bricks')
    expect(legacy.count).toBe(2)

    const modern = generateJavaRecipe(state, '1.20', ctx)
    expect(modern.result).toEqual({ id: 'minecraft:stone_bricks', count: 2 })
})

it('formats legacy smithing, trim, and transform recipes', () => {
    const smithing = recipe({
        recipeType: 'smithing',
        slots: {
            'smithing.base': { kind: 'item', id: 'minecraft:iron_bars' },
            'smithing.addition': { kind: 'item', id: 'minecraft:diamond' },
            'smithing.result': { kind: 'item', id: 'minecraft:diamond_sword' },
        },
    })
    const legacy = generateJavaRecipe(smithing, '1.16', ctx)
    expect(legacy.type).toBe('minecraft:smithing')
    expect(legacy.base).toEqual({ item: 'minecraft:iron_bars' })
    expect(legacy.result).toEqual({ item: 'minecraft:diamond_sword' })

    const trim = recipe({
        recipeType: 'smithing_trim',
        smithing: { trimPattern: 'minecraft:coast' },
        slots: {
            'smithing.template': { kind: 'item', id: 'minecraft:iron_bars' },
            'smithing.base': { kind: 'item', id: 'minecraft:diamond_sword' },
            'smithing.addition': { kind: 'item', id: 'minecraft:diamond' },
        },
    })
    const trimOutput = generateJavaRecipe(trim, '1.21.5', ctx)
    expect(trimOutput.type).toBe('minecraft:smithing_trim')
    expect(trimOutput.pattern).toBe('minecraft:coast')

    const transform = recipe({
        recipeType: 'smithing_transform',
        slots: {
            'smithing.template': { kind: 'item', id: 'minecraft:iron_bars' },
            'smithing.base': { kind: 'item', id: 'minecraft:diamond_sword' },
            'smithing.addition': { kind: 'item', id: 'minecraft:diamond' },
            'smithing.result': { kind: 'item', id: 'minecraft:diamond_sword' },
        },
    })
    const transformOutput = generateJavaRecipe(transform, '1.19', ctx)
    expect(transformOutput.type).toBe('minecraft:smithing_transform')
    expect(transformOutput.template).toEqual({ item: 'minecraft:iron_bars' })
})

it('gates category, show_notification, and stonecutter group by version', () => {
    const shaped = recipe({
        category: 'building',
        showNotification: false,
        slots: {
            'crafting.1': { kind: 'item', id: 'minecraft:oak_planks' },
            'crafting.result': { kind: 'item', id: 'minecraft:stick' },
        },
    })
    expect(generateJavaRecipe(shaped, '1.18', ctx).category).toBe(undefined)
    expect(generateJavaRecipe(shaped, '1.18', ctx).show_notification).toBe(undefined)
    expect(generateJavaRecipe(shaped, '1.19', ctx).category).toBe('building')
    expect(generateJavaRecipe(shaped, '1.19', ctx).show_notification).toBe(false)

    const stonecutter = recipe({
        recipeType: 'stonecutter',
        group: 'stone',
        slots: {
            'stonecutter.ingredient': { kind: 'item', id: 'minecraft:stone' },
            'stonecutter.result': { kind: 'item', id: 'minecraft:stone_bricks' },
        },
    })
    expect(generateJavaRecipe(stonecutter, '1.20', ctx).group).toBe('stone')
    expect(generateJavaRecipe(stonecutter, '26.1', ctx).group).toBe(undefined)
})

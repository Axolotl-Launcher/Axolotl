import { expect, it } from 'vitest'

import {
    getAutoRecipeName,
    getCurrentRecipeName,
    resolveRecipeNames,
    sanitizeRecipeName,
} from './naming.ts'
import type { RecipeSlotContext, RecipeState } from './types.ts'

function context(): RecipeSlotContext {
    return {
        itemsById: {
            'minecraft:iron_ingot': {
                id: 'minecraft:iron_ingot',
                name: 'Iron Ingot',
                texture: 'a',
            },
            'minecraft:beef': { id: 'minecraft:beef', name: 'Raw Beef', texture: 'b' },
            'minecraft:cooked_beef': { id: 'minecraft:cooked_beef', name: 'Steak', texture: 'c' },
            'minecraft:stone': { id: 'minecraft:stone', name: 'Stone', texture: 'd' },
            'minecraft:stone_bricks': {
                id: 'minecraft:stone_bricks',
                name: 'Stone Bricks',
                texture: 'e',
            },
            'minecraft:stick': { id: 'minecraft:stick', name: 'Stick', texture: 'f' },
        },
        customItemsByUid: {},
        customTagsByUid: {},
        vanillaTags: {},
    }
}

const ctx = context()

function recipe(overrides: Partial<RecipeState> = {}): RecipeState {
    return {
        id: crypto.randomUUID(),
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

it('sanitizes recipe file names', () => {
    expect(sanitizeRecipeName('  Iron Bars!  ')).toBe('iron_bars')
    expect(sanitizeRecipeName('a__b---c')).toBe('a_b_c')
})

it('generates automatic names for every recipe type', () => {
    const crafting = recipe({
        slots: { 'crafting.result': { kind: 'item', id: 'minecraft:iron_ingot' } },
    })
    expect(getAutoRecipeName(crafting, ctx)).toBe('iron_ingot')

    const smelting = recipe({
        recipeType: 'smelting',
        slots: {
            'cooking.ingredient': { kind: 'item', id: 'minecraft:beef' },
            'cooking.result': { kind: 'item', id: 'minecraft:cooked_beef' },
        },
    })
    expect(getAutoRecipeName(smelting, ctx)).toBe('cooked_beef_from_smelting')

    const stonecutter = recipe({
        recipeType: 'stonecutter',
        slots: {
            'stonecutter.ingredient': { kind: 'item', id: 'minecraft:stone' },
            'stonecutter.result': { kind: 'item', id: 'minecraft:stone_bricks' },
        },
    })
    expect(getAutoRecipeName(stonecutter, ctx)).toBe('stone_bricks_from_stone_stonecutting')
})

it('assigns unique names across duplicate auto names', () => {
    const first = recipe({
        slots: { 'crafting.result': { kind: 'item', id: 'minecraft:iron_ingot' } },
    })
    const second = recipe({
        slots: { 'crafting.result': { kind: 'item', id: 'minecraft:iron_ingot' } },
    })
    const names = resolveRecipeNames([first, second], ctx)
    expect(names.get(first.id)?.resolvedName).toBe('iron_ingot')
    expect(names.get(second.id)?.resolvedName).toBe('iron_ingot_2')
})

it('manual names are preserved and empty manual names fall back to auto', () => {
    const first = recipe({
        nameMode: 'manual',
        name: 'my_recipe',
        slots: { 'crafting.result': { kind: 'item', id: 'minecraft:iron_ingot' } },
    })
    const second = recipe({
        nameMode: 'manual',
        name: '',
        slots: { 'crafting.result': { kind: 'item', id: 'minecraft:iron_ingot' } },
    })
    const names = resolveRecipeNames([first, second], ctx)
    expect(names.get(first.id)?.resolvedName).toBe('my_recipe')
    expect(names.get(second.id)?.resolvedName).toBe('iron_ingot')
    expect(getCurrentRecipeName(second, [first, second], ctx).resolvedName).toBe('iron_ingot')
})

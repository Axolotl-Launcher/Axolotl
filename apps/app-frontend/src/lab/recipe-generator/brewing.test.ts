import { expect, it } from 'vitest'

import { createDatapackFiles } from './datapack.ts'
import {
    createBrewingState,
    createDefaultRecipeGeneratorStore,
    createDefaultRecipeState,
    migrateRecipeProject,
    switchRecipeProjectVersion,
} from './project.ts'
import { generateJavaRecipe } from './recipe-engine.ts'
import type { RecipeSlotContext } from './types.ts'
import { validateRecipe } from './validation.ts'
import { defaultCookingTime } from './versions.ts'

const ctx: RecipeSlotContext = {
    itemsById: {},
    vanillaTags: {},
    customItemsByUid: {},
    customTagsByUid: {},
}

it('brewing matches vanilla 26.3 input predicate and output component shapes', () => {
    const state = createDefaultRecipeState('brewing')
    state.slots = {
        'brewing.input': { kind: 'item', id: 'minecraft:potion' },
        'brewing.reagent': { kind: 'item', id: 'minecraft:nether_wart' },
        'brewing.result': { kind: 'item', id: 'minecraft:potion' },
    }
    state.brewing = {
        ...createBrewingState(),
        inputPotion: 'minecraft:water',
        outputPotion: 'minecraft:awkward',
    }
    expect(generateJavaRecipe(state, '26.3', ctx)).toEqual({
        type: 'minecraft:brewing',
        input: { item: 'minecraft:potion', potion_contents: { potions: 'minecraft:water' } },
        reagent: { item: 'minecraft:nether_wart' },
        output: {
            id: 'minecraft:potion',
            components: { 'minecraft:potion_contents': { potion: 'minecraft:awkward' } },
        },
    })
    expect(validateRecipe(state, '26.3', ctx)).toEqual([])
    expect(() => generateJavaRecipe(state, '26.2', ctx)).toThrow()
    expect(
        validateRecipe(state, '26.2', ctx).some((issue) => issue.code === 'unsupported-type'),
    ).toBeTruthy()
})

it('brewing supports arbitrary items, counts, predicates and components', () => {
    const state = createDefaultRecipeState('brewing')
    state.slots = {
        'brewing.input': { kind: 'item', id: 'minecraft:bucket' },
        'brewing.reagent': { kind: 'item', id: 'minecraft:potent_sulfur' },
        'brewing.result': { kind: 'item', id: 'minecraft:sulfur_cube_bucket', count: 2 },
    }
    state.brewing!.reagentPredicate = '{"components":{"minecraft:custom_data":{"quality":1}}}'
    state.brewing!.outputComponents = '{"minecraft:custom_name":"Custom bucket"}'
    const output = generateJavaRecipe(state, '26.3', ctx)
    expect(output.output).toEqual({
        id: 'minecraft:sulfur_cube_bucket',
        count: 2,
        components: { 'minecraft:custom_name': 'Custom bucket' },
    })
    expect(output.reagent).toEqual({
        item: 'minecraft:potent_sulfur',
        components: { 'minecraft:custom_data': { quality: 1 } },
    })
    for (const invalid of ['{', '[]', 'null', '"value"']) {
        state.brewing!.outputComponents = invalid
        expect(
            validateRecipe(state, '26.3', ctx).some(
                (issue) => issue.code === 'invalid-brewing-json',
            ),
        ).toBeTruthy()
        expect(() => generateJavaRecipe(state, '26.3', ctx)).toThrow()
    }
})

it('v1 projects migrate without changing game version, recipe data or custom resources', () => {
    const old = createDefaultRecipeGeneratorStore()
    old.selectedVersion = '26.2'
    old.recipes[0].cooking.time = 73
    old.customItems = [
        {
            uid: 'custom',
            id: 'custom:item',
            name: 'My item',
            texture: 'data:image/png;base64,AA',
            createdAt: '2026-10-17',
        },
    ]
    const migrated = migrateRecipeProject({ ...old, version: 1 })
    expect(migrated.version).toBe(2)
    expect(migrated.selectedVersion).toBe('26.2')
    expect(migrated.recipes[0].cooking.time).toBe(73)
    expect(migrated.customItems).toEqual(old.customItems)
})

it('version switching preserves brewing fields and actual blast/smoker processing ticks', () => {
    const project = createDefaultRecipeGeneratorStore()
    project.selectedVersion = '26.2'
    const blast = createDefaultRecipeState('blasting')
    blast.cooking.time = 101
    const brew = createDefaultRecipeState('brewing')
    brew.brewing!.outputPotion = 'minecraft:strength'
    project.recipes = [blast, brew]
    switchRecipeProjectVersion(project, '26.3')
    expect(blast.cooking.time).toBe(202)
    expect(defaultCookingTime('blasting', '26.3')).toBe(200)
    expect(defaultCookingTime('smoking', '26.2')).toBe(100)
    blast.cooking.time = 201
    switchRecipeProjectVersion(project, '26.2')
    expect(blast.cooking.time).toBe(101)
    expect(brew.recipeType).toBe('brewing')
    expect(brew.brewing!.outputPotion).toBe('minecraft:strength')
    expect(defaultCookingTime('smelting', '26.3')).toBe(200)
    expect(defaultCookingTime('campfire_cooking', '26.3')).toBe(100)
})

it('26.3 datapack exports brewing recipes with the exact format range', () => {
    const files = createDatapackFiles(
        '26.3',
        [{ name: 'test_brew', json: { type: 'minecraft:brewing' } }],
        [],
    )
    const metadata = files.find((file) => file.path === 'pack.mcmeta')!
    expect(JSON.parse(metadata.content as string).pack.min_format).toEqual([121, 0])
    expect(files.some((file) => file.path.endsWith('/recipe/test_brew.json'))).toBeTruthy()
})

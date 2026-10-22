import assert from 'node:assert/strict'
import test from 'node:test'

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

test('brewing matches vanilla 26.3 input predicate and output component shapes', () => {
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
    assert.deepEqual(generateJavaRecipe(state, '26.3', ctx), {
        type: 'minecraft:brewing',
        input: { item: 'minecraft:potion', potion_contents: { potions: 'minecraft:water' } },
        reagent: { item: 'minecraft:nether_wart' },
        output: {
            id: 'minecraft:potion',
            components: { 'minecraft:potion_contents': { potion: 'minecraft:awkward' } },
        },
    })
    assert.deepEqual(validateRecipe(state, '26.3', ctx), [])
    assert.throws(() => generateJavaRecipe(state, '26.2', ctx))
    assert.ok(validateRecipe(state, '26.2', ctx).some((issue) => issue.code === 'unsupported-type'))
})

test('brewing supports arbitrary items, counts, predicates and components', () => {
    const state = createDefaultRecipeState('brewing')
    state.slots = {
        'brewing.input': { kind: 'item', id: 'minecraft:bucket' },
        'brewing.reagent': { kind: 'item', id: 'minecraft:potent_sulfur' },
        'brewing.result': { kind: 'item', id: 'minecraft:sulfur_cube_bucket', count: 2 },
    }
    state.brewing!.reagentPredicate = '{"components":{"minecraft:custom_data":{"quality":1}}}'
    state.brewing!.outputComponents = '{"minecraft:custom_name":"Custom bucket"}'
    const output = generateJavaRecipe(state, '26.3', ctx)
    assert.deepEqual(output.output, {
        id: 'minecraft:sulfur_cube_bucket',
        count: 2,
        components: { 'minecraft:custom_name': 'Custom bucket' },
    })
    assert.deepEqual(output.reagent, {
        item: 'minecraft:potent_sulfur',
        components: { 'minecraft:custom_data': { quality: 1 } },
    })
    for (const invalid of ['{', '[]', 'null', '"value"']) {
        state.brewing!.outputComponents = invalid
        assert.ok(
            validateRecipe(state, '26.3', ctx).some(
                (issue) => issue.code === 'invalid-brewing-json',
            ),
        )
        assert.throws(() => generateJavaRecipe(state, '26.3', ctx))
    }
})

test('v1 projects migrate without changing game version, recipe data or custom resources', () => {
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
    assert.equal(migrated.version, 2)
    assert.equal(migrated.selectedVersion, '26.2')
    assert.equal(migrated.recipes[0].cooking.time, 73)
    assert.deepEqual(migrated.customItems, old.customItems)
})

test('version switching preserves brewing fields and actual blast/smoker processing ticks', () => {
    const project = createDefaultRecipeGeneratorStore()
    project.selectedVersion = '26.2'
    const blast = createDefaultRecipeState('blasting')
    blast.cooking.time = 101
    const brew = createDefaultRecipeState('brewing')
    brew.brewing!.outputPotion = 'minecraft:strength'
    project.recipes = [blast, brew]
    switchRecipeProjectVersion(project, '26.3')
    assert.equal(blast.cooking.time, 202)
    assert.equal(defaultCookingTime('blasting', '26.3'), 200)
    assert.equal(defaultCookingTime('smoking', '26.2'), 100)
    blast.cooking.time = 201
    switchRecipeProjectVersion(project, '26.2')
    assert.equal(blast.cooking.time, 101)
    assert.equal(brew.recipeType, 'brewing')
    assert.equal(brew.brewing!.outputPotion, 'minecraft:strength')
    assert.equal(defaultCookingTime('smelting', '26.3'), 200)
    assert.equal(defaultCookingTime('campfire_cooking', '26.3'), 100)
})

test('26.3 datapack exports brewing recipes with the exact format range', () => {
    const files = createDatapackFiles(
        '26.3',
        [{ name: 'test_brew', json: { type: 'minecraft:brewing' } }],
        [],
    )
    const metadata = files.find((file) => file.path === 'pack.mcmeta')!
    assert.deepEqual(JSON.parse(metadata.content as string).pack.min_format, [121, 0])
    assert.ok(files.some((file) => file.path.endsWith('/recipe/test_brew.json')))
})

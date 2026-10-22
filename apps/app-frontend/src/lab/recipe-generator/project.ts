import type { BrewingState, RecipeGeneratorStore, RecipeState } from './types.ts'
import {
    ALL_RECIPE_TYPES,
    convertCookingTime,
    isJavaVersionId,
    LATEST_JAVA_VERSION,
} from './versions.ts'

export const RECIPE_PROJECT_STORAGE_KEY = 'axolotl.lab.recipe-generator.v1'

export function createBrewingState(): BrewingState {
    return {
        inputPotion: '',
        reagentPotion: '',
        outputPotion: '',
        inputPredicate: '{}',
        reagentPredicate: '{}',
        outputComponents: '{}',
    }
}

export function createDefaultRecipeState(
    type: RecipeState['recipeType'] = 'crafting',
): RecipeState {
    return {
        id: crypto.randomUUID(),
        recipeType: type,
        group: '',
        category: '',
        showNotification: true,
        nameMode: 'auto',
        name: '',
        slots: {},
        crafting: { shapeless: false, keepWhitespace: false, twoByTwo: false },
        cooking: { time: null, experience: 0 },
        smithing: { trimPattern: '' },
        brewing: createBrewingState(),
    }
}

export function createDefaultRecipeGeneratorStore(): RecipeGeneratorStore {
    const recipe = createDefaultRecipeState()
    return {
        version: 2,
        selectedVersion: LATEST_JAVA_VERSION,
        recipes: [recipe],
        selectedRecipeId: recipe.id,
        customItems: [],
        customTags: [],
    }
}

/** Reads both project schemas without changing the selected game version or recipe type. */
export function migrateRecipeProject(value: unknown): RecipeGeneratorStore {
    const fallback = createDefaultRecipeGeneratorStore()
    if (!value || typeof value !== 'object') return fallback
    const source = value as Partial<RecipeGeneratorStore>
    if (!Array.isArray(source.recipes)) return fallback
    const recipes = source.recipes
        .filter(
            (recipe) =>
                recipe &&
                typeof recipe === 'object' &&
                ALL_RECIPE_TYPES.includes(recipe.recipeType),
        )
        .map((recipe) => {
            const defaults = createDefaultRecipeState(recipe.recipeType)
            return {
                ...defaults,
                ...recipe,
                id: typeof recipe.id === 'string' && recipe.id ? recipe.id : defaults.id,
                slots: recipe.slots && typeof recipe.slots === 'object' ? recipe.slots : {},
                crafting: { ...defaults.crafting, ...recipe.crafting },
                cooking: { ...defaults.cooking, ...recipe.cooking },
                smithing: { ...defaults.smithing, ...recipe.smithing },
                brewing: { ...createBrewingState(), ...recipe.brewing },
            }
        })
    if (!recipes.length) recipes.push(fallback.recipes[0])
    return {
        version: 2,
        selectedVersion: isJavaVersionId(source.selectedVersion)
            ? source.selectedVersion
            : fallback.selectedVersion,
        recipes,
        selectedRecipeId: recipes.some((recipe) => recipe.id === source.selectedRecipeId)
            ? source.selectedRecipeId!
            : recipes[0].id,
        customItems: Array.isArray(source.customItems) ? source.customItems : [],
        customTags: Array.isArray(source.customTags) ? source.customTags : [],
    }
}

export function loadRecipeProject(
    storage: Pick<Storage, 'getItem'> | undefined = globalThis.localStorage,
): RecipeGeneratorStore {
    try {
        const raw = storage?.getItem(RECIPE_PROJECT_STORAGE_KEY)
        return raw ? migrateRecipeProject(JSON.parse(raw)) : createDefaultRecipeGeneratorStore()
    } catch {
        return createDefaultRecipeGeneratorStore()
    }
}

export function switchRecipeProjectVersion(
    project: RecipeGeneratorStore,
    version: RecipeGeneratorStore['selectedVersion'],
    previous = project.selectedVersion,
) {
    for (const recipe of project.recipes)
        recipe.cooking.time = convertCookingTime(
            recipe.cooking.time,
            recipe.recipeType,
            previous,
            version,
        )
    project.selectedVersion = version
}

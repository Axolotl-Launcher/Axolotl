import { isVersionAtLeast } from '../shared/minecraft/versions.ts'
import type { JavaVersionId, RecipeSlot, RecipeType } from './types.ts'

export {
    compareMinecraftVersions,
    getJavaVersionMeta,
    isJavaVersionId,
    isVersionAtLeast,
    JAVA_VERSIONS,
    type JavaVersionMeta,
    LATEST_JAVA_VERSION,
} from '../shared/minecraft/versions.ts'

const recipeTypeAvailability: Record<RecipeType, { minVersion: string; maxVersion?: string }> = {
    crafting: { minVersion: '1.12' },
    brewing: { minVersion: '26.3' },
    smelting: { minVersion: '1.13' },
    blasting: { minVersion: '1.14' },
    smoking: { minVersion: '1.14' },
    campfire_cooking: { minVersion: '1.14' },
    stonecutter: { minVersion: '1.14' },
    smithing: { minVersion: '1.16', maxVersion: '1.18' },
    smithing_trim: { minVersion: '1.19' },
    smithing_transform: { minVersion: '1.19' },
}

export const ALL_RECIPE_TYPES: readonly RecipeType[] = [
    'crafting',
    'brewing',
    'smelting',
    'blasting',
    'smoking',
    'campfire_cooking',
    'stonecutter',
    'smithing',
    'smithing_trim',
    'smithing_transform',
]

export function isRecipeTypeAvailable(version: JavaVersionId, type: RecipeType): boolean {
    const availability = recipeTypeAvailability[type]
    if (!isVersionAtLeast(version, availability.minVersion)) return false
    if (
        availability.maxVersion &&
        isVersionAtLeast(version, availability.maxVersion) &&
        version !== availability.maxVersion
    ) {
        return false
    }
    return true
}

export function getSupportedRecipeTypes(version: JavaVersionId): RecipeType[] {
    return ALL_RECIPE_TYPES.filter((type) => isRecipeTypeAvailable(version, type))
}

export function coerceRecipeTypeForVersion(
    type: RecipeType | undefined,
    version: JavaVersionId,
): RecipeType {
    const supported = getSupportedRecipeTypes(version)
    if (type && supported.includes(type)) return type
    return supported[0] ?? 'crafting'
}

export function getRecipeCategoryOptions(type: RecipeType): string[] | undefined {
    switch (type) {
        case 'crafting':
            return ['equipment', 'building', 'misc', 'redstone']
        case 'smelting':
            return ['food', 'blocks', 'misc']
        case 'blasting':
            return ['blocks', 'misc']
        case 'smoking':
        case 'campfire_cooking':
            return ['food']
        default:
            return undefined
    }
}

export function supportsRecipeCategory(version: JavaVersionId, type: RecipeType): boolean {
    return isVersionAtLeast(version, '1.19') && getRecipeCategoryOptions(type) !== undefined
}

export function supportsShowNotification(
    version: JavaVersionId,
    type: RecipeType,
    shapeless: boolean,
): boolean {
    if (type === 'crafting') {
        return isVersionAtLeast(version, shapeless ? '26.1' : '1.19')
    }
    return (
        isVersionAtLeast(version, '26.1') &&
        [
            'smelting',
            'blasting',
            'smoking',
            'campfire_cooking',
            'stonecutter',
            'smithing_trim',
            'smithing_transform',
        ].includes(type)
    )
}

export function supportsSmithingTrimPattern(version: JavaVersionId): boolean {
    return isVersionAtLeast(version, '1.21.5')
}

export function supportsItemTags(version: JavaVersionId): boolean {
    return isVersionAtLeast(version, '1.13')
}

export function supportsCustomTags(version: JavaVersionId): boolean {
    return isVersionAtLeast(version, '1.13')
}

export function supportsVanillaTagList(version: JavaVersionId): boolean {
    return isVersionAtLeast(version, '1.14')
}

export const DEFAULT_COOKING_TIME: Record<
    'smelting' | 'blasting' | 'smoking' | 'campfire_cooking',
    number
> = {
    smelting: 200,
    blasting: 100,
    smoking: 100,
    campfire_cooking: 100,
}

export const RESULT_SLOTS_BY_TYPE: Record<RecipeType, readonly RecipeSlot[] | undefined> = {
    crafting: ['crafting.result'],
    brewing: ['brewing.result'],
    smelting: ['cooking.result'],
    blasting: ['cooking.result'],
    smoking: ['cooking.result'],
    campfire_cooking: ['cooking.result'],
    stonecutter: ['stonecutter.result'],
    smithing: ['smithing.result'],
    smithing_trim: undefined,
    smithing_transform: ['smithing.result'],
}

export const SLOT_KEYS_BY_TYPE: Record<RecipeType, readonly RecipeSlot[]> = {
    brewing: ['brewing.input', 'brewing.reagent'],
    crafting: [
        'crafting.1',
        'crafting.2',
        'crafting.3',
        'crafting.4',
        'crafting.5',
        'crafting.6',
        'crafting.7',
        'crafting.8',
        'crafting.9',
    ],
    smelting: ['cooking.ingredient'],
    blasting: ['cooking.ingredient'],
    smoking: ['cooking.ingredient'],
    campfire_cooking: ['cooking.ingredient'],
    stonecutter: ['stonecutter.ingredient'],
    smithing: ['smithing.template', 'smithing.base', 'smithing.addition'],
    smithing_trim: ['smithing.template', 'smithing.base', 'smithing.addition'],
    smithing_transform: ['smithing.template', 'smithing.base', 'smithing.addition'],
}

export function defaultCookingTime(
    type: keyof typeof DEFAULT_COOKING_TIME,
    version: JavaVersionId,
): number {
    return (type === 'blasting' || type === 'smoking') && isVersionAtLeast(version, '26.3')
        ? 200
        : DEFAULT_COOKING_TIME[type]
}

/** Recipe time is divided by two for these appliances starting in 26.3. */
export function convertCookingTime(
    time: number | null,
    type: RecipeType,
    from: JavaVersionId,
    to: JavaVersionId,
): number | null {
    if (time === null || (type !== 'blasting' && type !== 'smoking')) return time
    const fromMultiplier = isVersionAtLeast(from, '26.3') ? 2 : 1
    const toMultiplier = isVersionAtLeast(to, '26.3') ? 2 : 1
    return fromMultiplier === toMultiplier
        ? time
        : Math.max(1, Math.ceil(time / fromMultiplier) * toMultiplier)
}

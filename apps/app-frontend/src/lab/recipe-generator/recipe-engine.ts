import { createJavaFormatter, type JavaRecipeFormatter } from './formatter.ts'
import { fullId, parseIdentifier, rawId } from './identifier.ts'
import type {
    JavaVersionId,
    RecipeSlot,
    RecipeSlotContext,
    RecipeState,
    SlotValue,
} from './types.ts'
import {
    defaultCookingTime,
    isRecipeTypeAvailable,
    isVersionAtLeast,
    supportsRecipeCategory,
    supportsShowNotification,
    supportsSmithingTrimPattern,
} from './versions.ts'

export const CRAFTING_GRID_SLOTS: readonly RecipeSlot[] = [
    'crafting.1',
    'crafting.2',
    'crafting.3',
    'crafting.4',
    'crafting.5',
    'crafting.6',
    'crafting.7',
    'crafting.8',
    'crafting.9',
]

const TWO_BY_TWO_DISABLED_INDICES = new Set([2, 5, 6, 7, 8])

const PATTERN_CHARACTERS = ['#', ...'ABCDEFGHIJKLMNOPQRSTUVWXYZ', ...'abcdefghijklmnopqrstuvwxyz']

const DINNERBONE_RULES: { char: string; keywords: string[] }[] = [
    { char: '/', keywords: ['stick', 'rod', 'torch', 'arrow', 'bone'] },
    { char: '_', keywords: ['slab', 'carpet', 'paper', 'map'] },
    { char: '=', keywords: ['ingot', 'brick'] },
    { char: '.', keywords: ['nugget', 'dust', 'powder', 'seed', 'redstone'] },
    { char: 'o', keywords: ['diamond', 'emerald', 'quartz', 'shard', 'pearl', 'ball', 'egg'] },
    { char: '~', keywords: ['string', 'vine'] },
    { char: ')', keywords: ['bow'] },
    { char: 'u', keywords: ['bucket', 'bottle'] },
]

export type ResolvedSlotValue = {
    ref: ReturnType<typeof parseIdentifier>
    isTag: boolean
}

export function resolveSlotValue(
    value: SlotValue | undefined,
    ctx: RecipeSlotContext,
): ResolvedSlotValue | undefined {
    if (!value) return undefined
    switch (value.kind) {
        case 'item':
            return { ref: parseIdentifier(value.id), isTag: false }
        case 'vanilla_tag':
            return { ref: parseIdentifier(value.id), isTag: true }
        case 'custom_item': {
            const custom = ctx.customItemsByUid[value.uid]
            return custom ? { ref: parseIdentifier(custom.id), isTag: false } : undefined
        }
        case 'custom_tag': {
            const custom = ctx.customTagsByUid[value.uid]
            return custom ? { ref: parseIdentifier(custom.id), isTag: true } : undefined
        }
    }
}

export function slotCount(value: SlotValue | undefined): number | undefined {
    return value && (value.kind === 'item' || value.kind === 'custom_item')
        ? value.count
        : undefined
}

function dinnerboneChallenge(path: string, isTag: boolean): string | null {
    if (isTag) return null
    const normalized = path.toLowerCase()
    for (const rule of DINNERBONE_RULES) {
        if (rule.keywords.some((keyword) => normalized.includes(keyword))) return rule.char
    }
    return null
}

function pickKeyName(
    path: string,
    value: SlotValue,
    ctx: RecipeSlotContext,
    usedKeys: Set<string>,
): string {
    const resolved = resolveSlotValue(value, ctx)
    const candidates = [dinnerboneChallenge(path, resolved?.isTag === true)]
    for (const word of path.match(/[a-zA-Z]+/g) ?? []) {
        candidates.push(word[0].toUpperCase(), word[0].toLowerCase())
    }
    for (const letter of path.match(/[a-zA-Z]/g) ?? []) {
        candidates.push(letter.toUpperCase(), letter.toLowerCase())
    }
    for (const candidate of candidates) {
        if (candidate && !usedKeys.has(candidate)) return candidate
    }
    const next = PATTERN_CHARACTERS.find((candidate) => !usedKeys.has(candidate))
    if (!next) throw new Error('Ran out of pattern characters')
    return next
}

function resolveKeyInfo(
    value: SlotValue,
    ctx: RecipeSlotContext,
): { reverseKey: string; path: string } {
    const resolved = resolveSlotValue(value, ctx)
    if (resolved) return { reverseKey: fullId(resolved.ref), path: resolved.ref.id }
    const uid = value.kind === 'custom_item' || value.kind === 'custom_tag' ? value.uid : ''
    return { reverseKey: `${value.kind}:${uid}`, path: uid }
}

export function assignCraftingKeys(
    grid: (SlotValue | undefined)[],
    ctx: RecipeSlotContext,
): { key: Record<string, SlotValue>; reverse: Record<string, string> } {
    const cells: { reverseKey: string; path: string; value: SlotValue }[] = []
    const counts = new Map<string, number>()
    let primary: string | undefined
    let primaryCount = 0

    for (const value of grid) {
        if (!value) continue
        const info = resolveKeyInfo(value, ctx)
        cells.push({ reverseKey: info.reverseKey, path: info.path, value })
        const count = (counts.get(info.reverseKey) ?? 0) + 1
        counts.set(info.reverseKey, count)
        if (count > primaryCount) {
            primary = info.reverseKey
            primaryCount = count
        }
    }

    const key: Record<string, SlotValue> = {}
    const reverse: Record<string, string> = {}
    const usedKeys = new Set<string>(['#'])

    for (const cell of cells) {
        if (reverse[cell.reverseKey]) continue
        const keyName =
            cell.reverseKey === primary ? '#' : pickKeyName(cell.path, cell.value, ctx, usedKeys)
        key[keyName] = cell.value
        reverse[cell.reverseKey] = keyName
        usedKeys.add(keyName)
    }

    return { key, reverse }
}

export function buildPattern(
    grid: (SlotValue | undefined)[],
    reverse: Record<string, string>,
    ctx: RecipeSlotContext,
    keepWhitespace: boolean,
): string[] {
    const pattern: string[] = []
    for (const [index, value] of grid.entries()) {
        const rowIndex = Math.floor(index / 3)
        pattern[rowIndex] = pattern[rowIndex] ?? ''
        if (!value) {
            pattern[rowIndex] += ' '
            continue
        }
        const { reverseKey } = resolveKeyInfo(value, ctx)
        pattern[rowIndex] += reverse[reverseKey] ?? '#'
    }

    if (keepWhitespace) return pattern

    while (pattern.length > 0 && pattern[0].trim() === '') pattern.shift()
    while (pattern.length > 0 && pattern[pattern.length - 1].trim() === '') pattern.pop()
    if (pattern.length === 0) return pattern

    let minColumn = Number.POSITIVE_INFINITY
    let maxColumn = 0
    for (const row of pattern) {
        let firstNonWhitespace = -1
        let lastNonWhitespace = -1
        for (let index = 0; index < row.length; index += 1) {
            if (row[index] === ' ') continue
            if (firstNonWhitespace === -1) firstNonWhitespace = index
            lastNonWhitespace = index
        }
        if (firstNonWhitespace === -1) continue
        minColumn = Math.min(minColumn, firstNonWhitespace)
        maxColumn = Math.max(maxColumn, lastNonWhitespace + 1)
    }
    return pattern.map((row) => row.slice(minColumn, maxColumn))
}

function craftingGrid(state: RecipeState): (SlotValue | undefined)[] {
    return CRAFTING_GRID_SLOTS.map((slot, index) => {
        const value = state.slots[slot]
        if (state.crafting.twoByTwo && TWO_BY_TWO_DISABLED_INDICES.has(index)) return undefined
        return value
    })
}

function buildCrafting(
    state: RecipeState,
    version: JavaVersionId,
    ctx: RecipeSlotContext,
    fmt: JavaRecipeFormatter,
): Record<string, unknown> {
    const grid = craftingGrid(state)
    const populated = grid.filter((value): value is SlotValue => Boolean(value))
    const result = state.slots['crafting.result']
    const resolvedResult = result ? resolveSlotValue(result, ctx) : undefined
    const resultCount = slotCount(result)
    const group = state.group.length > 0 ? state.group : undefined
    const category =
        supportsRecipeCategory(version, 'crafting') && state.category.length > 0
            ? state.category
            : undefined
    const showNotification =
        supportsShowNotification(version, 'crafting', state.crafting.shapeless) &&
        state.showNotification === false
            ? { show_notification: false }
            : {}
    const output: Record<string, unknown> = {
        type: fmt.typeName(state.crafting.shapeless ? 'crafting_shapeless' : 'crafting_shaped'),
        ...(category ? { category } : {}),
        ...showNotification,
    }

    if (state.crafting.shapeless) {
        return {
            ...output,
            ingredients: populated.map((value) => {
                const resolved = resolveSlotValue(value, ctx)
                return fmt.ingredient(resolved!.ref, resolved!.isTag)
            }),
            ...(group ? { group } : {}),
            result:
                resolvedResult && !resolvedResult.isTag
                    ? fmt.result(resolvedResult.ref, resultCount)
                    : {},
        }
    }

    const { key, reverse } = assignCraftingKeys(grid, ctx)
    return {
        ...output,
        pattern: buildPattern(grid, reverse, ctx, state.crafting.keepWhitespace),
        key: Object.fromEntries(
            Object.entries(key).map(([keyName, value]) => {
                const resolved = resolveSlotValue(value, ctx)
                return [keyName, fmt.ingredient(resolved!.ref, resolved!.isTag)]
            }),
        ),
        ...(group ? { group } : {}),
        result:
            resolvedResult && !resolvedResult.isTag
                ? fmt.result(resolvedResult.ref, resultCount)
                : {},
    }
}

function buildCooking(
    state: RecipeState,
    version: JavaVersionId,
    ctx: RecipeSlotContext,
    fmt: JavaRecipeFormatter,
    type: 'smelting' | 'blasting' | 'smoking' | 'campfire_cooking',
): Record<string, unknown> {
    const ingredient = resolveSlotValue(state.slots['cooking.ingredient'], ctx)
    const result = resolveSlotValue(state.slots['cooking.result'], ctx)
    const group = state.group.length > 0 ? state.group : undefined
    const category =
        supportsRecipeCategory(version, state.recipeType) && state.category.length > 0
            ? state.category
            : undefined
    const showNotification =
        supportsShowNotification(version, state.recipeType, false) &&
        state.showNotification === false
            ? { show_notification: false }
            : {}
    return {
        type: fmt.typeName(type),
        ...(category ? { category } : {}),
        ...(group ? { group } : {}),
        ...showNotification,
        experience: state.cooking.experience,
        cookingtime: state.cooking.time ?? defaultCookingTime(type, version),
        ingredient: ingredient ? fmt.ingredient(ingredient.ref, ingredient.isTag) : {},
        result:
            result && !result.isTag
                ? fmt.cookingResult(result.ref, slotCount(state.slots['cooking.result']))
                : {},
    }
}

function buildStonecutter(
    state: RecipeState,
    version: JavaVersionId,
    ctx: RecipeSlotContext,
    fmt: JavaRecipeFormatter,
): Record<string, unknown> {
    const ingredient = resolveSlotValue(state.slots['stonecutter.ingredient'], ctx)
    const result = resolveSlotValue(state.slots['stonecutter.result'], ctx)
    const group =
        !isVersionAtLeast(version, '26.1') && state.group.length > 0 ? state.group : undefined
    const showNotification =
        supportsShowNotification(version, 'stonecutter', false) && state.showNotification === false
            ? { show_notification: false }
            : {}
    return {
        type: fmt.typeName('stonecutting'),
        ...(group ? { group } : {}),
        ...showNotification,
        ingredient: ingredient ? fmt.ingredient(ingredient.ref, ingredient.isTag) : {},
        ...fmt.stonecutterResult(
            result?.ref ?? { namespace: 'minecraft', id: 'air' },
            slotCount(state.slots['stonecutter.result']),
        ),
    }
}

function buildSmithing(
    state: RecipeState,
    version: JavaVersionId,
    ctx: RecipeSlotContext,
    fmt: JavaRecipeFormatter,
): Record<string, unknown> {
    const template = resolveSlotValue(state.slots['smithing.template'], ctx)
    const base = resolveSlotValue(state.slots['smithing.base'], ctx)
    const addition = resolveSlotValue(state.slots['smithing.addition'], ctx)
    const result = resolveSlotValue(state.slots['smithing.result'], ctx)

    if (state.recipeType === 'smithing') {
        return {
            type: fmt.typeName('smithing'),
            result: result && !result.isTag ? fmt.result(result.ref) : {},
            base: base ? fmt.ingredient(base.ref, base.isTag) : {},
            addition: addition ? fmt.ingredient(addition.ref, addition.isTag) : {},
        }
    }

    const showNotification =
        supportsShowNotification(version, state.recipeType, false) &&
        state.showNotification === false
            ? { show_notification: false }
            : {}

    if (state.recipeType === 'smithing_trim') {
        return {
            type: fmt.typeName('smithing_trim'),
            ...showNotification,
            template: template ? fmt.ingredient(template.ref, template.isTag) : {},
            base: base ? fmt.ingredient(base.ref, base.isTag) : {},
            addition: addition ? fmt.ingredient(addition.ref, addition.isTag) : {},
            ...(supportsSmithingTrimPattern(version) && state.smithing.trimPattern
                ? { pattern: state.smithing.trimPattern }
                : {}),
        }
    }

    return {
        type: fmt.typeName('smithing_transform'),
        ...showNotification,
        template: template ? fmt.ingredient(template.ref, template.isTag) : {},
        base: base ? fmt.ingredient(base.ref, base.isTag) : {},
        addition: addition ? fmt.ingredient(addition.ref, addition.isTag) : {},
        result: result && !result.isTag ? fmt.result(result.ref) : {},
    }
}

export function generateJavaRecipe(
    state: RecipeState,
    version: JavaVersionId,
    ctx: RecipeSlotContext,
): Record<string, unknown> {
    if (!isRecipeTypeAvailable(version, state.recipeType))
        throw new Error('This recipe type is not supported by the selected version.')
    const fmt = createJavaFormatter(version)
    switch (state.recipeType) {
        case 'brewing':
            return buildBrewing(state, ctx)
        case 'crafting':
            return buildCrafting(state, version, ctx, fmt)
        case 'smelting':
        case 'blasting':
        case 'smoking':
        case 'campfire_cooking':
            return buildCooking(state, version, ctx, fmt, state.recipeType)
        case 'stonecutter':
            return buildStonecutter(state, version, ctx, fmt)
        case 'smithing':
        case 'smithing_trim':
        case 'smithing_transform':
            return buildSmithing(state, version, ctx, fmt)
        default:
            throw new Error(`Unsupported recipe type: ${state.recipeType satisfies never}`)
    }
}

export function getCraftingGridValues(state: RecipeState): (SlotValue | undefined)[] {
    return craftingGrid(state)
}

export function parseRecipeJsonObject(source: string): Record<string, unknown> {
    const value: unknown = JSON.parse(source.trim() || '{}')
    if (!value || typeof value !== 'object' || Array.isArray(value))
        throw new Error('Expected a JSON object')
    return value as Record<string, unknown>
}

function buildBrewing(state: RecipeState, ctx: RecipeSlotContext): Record<string, unknown> {
    const brewing = state.brewing
    const ingredient = (slot: 'brewing.input' | 'brewing.reagent', source = '', potion = '') => {
        const resolved = resolveSlotValue(state.slots[slot], ctx)
        const predicate = parseRecipeJsonObject(source)
        return {
            ...predicate,
            ...(resolved ? { item: `${resolved.isTag ? '#' : ''}${rawId(resolved.ref)}` } : {}),
            ...(potion
                ? {
                      potion_contents: {
                          ...(typeof predicate.potion_contents === 'object' &&
                          predicate.potion_contents
                              ? predicate.potion_contents
                              : {}),
                          potions: potion,
                      },
                  }
                : {}),
        }
    }
    const result = resolveSlotValue(state.slots['brewing.result'], ctx)
    if (result?.isTag) throw new Error('A brewing result must be an item')
    const components = parseRecipeJsonObject(brewing?.outputComponents ?? '')
    if (brewing?.outputPotion)
        components['minecraft:potion_contents'] = {
            ...(typeof components['minecraft:potion_contents'] === 'object' &&
            components['minecraft:potion_contents']
                ? components['minecraft:potion_contents']
                : {}),
            potion: brewing.outputPotion,
        }
    return {
        type: 'minecraft:brewing',
        input: ingredient('brewing.input', brewing?.inputPredicate, brewing?.inputPotion),
        reagent: ingredient('brewing.reagent', brewing?.reagentPredicate, brewing?.reagentPotion),
        output: {
            ...(result ? { id: rawId(result.ref) } : {}),
            ...(slotCount(state.slots['brewing.result']) !== undefined
                ? { count: slotCount(state.slots['brewing.result']) }
                : {}),
            ...(Object.keys(components).length ? { components } : {}),
        },
    }
}

import { parseIdentifier } from './identifier.ts'
import { parseRecipeJsonObject } from './recipe-engine.ts'
import type {
    JavaVersionId,
    RecipeSlot,
    RecipeSlotContext,
    RecipeState,
    SlotValue,
} from './types.ts'
import {
    isRecipeTypeAvailable,
    RESULT_SLOTS_BY_TYPE,
    supportsItemTags,
    supportsSmithingTrimPattern,
} from './versions.ts'

export type RecipeIssueCode =
    | 'unsupported-type'
    | 'invalid-brewing-json'
    | 'invalid-result-count'
    | 'missing-ingredient'
    | 'missing-result'
    | 'missing-template'
    | 'missing-base'
    | 'missing-addition'
    | 'missing-trim-pattern'
    | 'tag-in-result'
    | 'missing-custom-item'
    | 'missing-custom-tag'
    | 'invalid-identifier'
    | 'tags-not-supported'

export type RecipeIssue = {
    code: RecipeIssueCode
    slot?: RecipeSlot
}

const RESULT_SLOTS = new Set<RecipeSlot>([
    'crafting.result',
    'cooking.result',
    'stonecutter.result',
    'smithing.result',
    'brewing.result',
])

function isTagValue(value: SlotValue): boolean {
    return value.kind === 'vanilla_tag' || value.kind === 'custom_tag'
}

function isValidIdentifierPart(value: string, allowSlash: boolean): boolean {
    const trimmed = value.trim()
    if (!trimmed || /\s/.test(trimmed)) return false
    return new RegExp(`^[a-z0-9_.\\-${allowSlash ? '/' : ''}]+$`).test(trimmed)
}

function hasInvalidIdentifier(value: SlotValue | undefined, ctx: RecipeSlotContext): boolean {
    if (!value) return false
    const resolved =
        value.kind === 'custom_item'
            ? ctx.customItemsByUid[value.uid]
            : value.kind === 'custom_tag'
              ? ctx.customTagsByUid[value.uid]
              : value.kind === 'item' || value.kind === 'vanilla_tag'
                ? { id: value.id }
                : undefined
    if (!resolved) return false
    const ref = parseIdentifier(resolved.id)
    return !isValidIdentifierPart(ref.namespace, false) || !isValidIdentifierPart(ref.id, true)
}

export function validateRecipe(
    state: RecipeState,
    version: JavaVersionId,
    ctx: RecipeSlotContext,
): RecipeIssue[] {
    const issues: RecipeIssue[] = []

    if (!isRecipeTypeAvailable(version, state.recipeType)) {
        issues.push({ code: 'unsupported-type' })
    }

    const hasTag = Object.values(state.slots).some((value) => value && isTagValue(value))
    if (hasTag && !supportsItemTags(version)) {
        issues.push({ code: 'tags-not-supported' })
    }

    for (const [slot, value] of Object.entries(state.slots) as [RecipeSlot, SlotValue][]) {
        if (!value) continue
        if (value.kind === 'custom_item' && !ctx.customItemsByUid[value.uid]) {
            issues.push({ code: 'missing-custom-item', slot })
        }
        if (value.kind === 'custom_tag' && !ctx.customTagsByUid[value.uid]) {
            issues.push({ code: 'missing-custom-tag', slot })
        }
        if (isTagValue(value) && RESULT_SLOTS.has(slot)) {
            issues.push({ code: 'tag-in-result', slot })
        }
        if (hasInvalidIdentifier(value, ctx)) {
            issues.push({ code: 'invalid-identifier', slot })
        }
    }

    if (state.recipeType === 'brewing') {
        const result = state.slots['brewing.result']
        if (
            result &&
            'count' in result &&
            result.count !== undefined &&
            (!Number.isInteger(result.count) || result.count < 1 || result.count > 99)
        )
            issues.push({ code: 'invalid-result-count' })
        for (const field of ['inputPredicate', 'reagentPredicate', 'outputComponents'] as const) {
            try {
                parseRecipeJsonObject(state.brewing?.[field] ?? '')
            } catch {
                issues.push({ code: 'invalid-brewing-json' })
                break
            }
        }
    }
    const typeIssues = validateTypeRules(state, version)
    return [...issues, ...typeIssues]
}

function validateTypeRules(state: RecipeState, version: JavaVersionId): RecipeIssue[] {
    const issues: RecipeIssue[] = []
    switch (state.recipeType) {
        case 'brewing':
            if (!state.slots['brewing.input'] || !state.slots['brewing.reagent'])
                issues.push({ code: 'missing-ingredient' })
            if (!state.slots['brewing.result']) issues.push({ code: 'missing-result' })
            break
        case 'crafting': {
            const hasIngredient = Object.keys(state.slots).some(
                (slot) =>
                    slot.startsWith('crafting.') &&
                    slot !== 'crafting.result' &&
                    state.slots[slot as RecipeSlot],
            )
            if (!hasIngredient) issues.push({ code: 'missing-ingredient' })
            if (!state.slots['crafting.result']) issues.push({ code: 'missing-result' })
            break
        }
        case 'smelting':
        case 'blasting':
        case 'smoking':
        case 'campfire_cooking':
            if (!state.slots['cooking.ingredient']) issues.push({ code: 'missing-ingredient' })
            if (!state.slots['cooking.result']) issues.push({ code: 'missing-result' })
            break
        case 'stonecutter':
            if (!state.slots['stonecutter.ingredient']) issues.push({ code: 'missing-ingredient' })
            if (!state.slots['stonecutter.result']) issues.push({ code: 'missing-result' })
            break
        case 'smithing':
            if (!state.slots['smithing.base']) issues.push({ code: 'missing-base' })
            if (!state.slots['smithing.addition']) issues.push({ code: 'missing-addition' })
            if (!state.slots['smithing.result']) issues.push({ code: 'missing-result' })
            break
        case 'smithing_trim':
            if (!state.slots['smithing.template']) issues.push({ code: 'missing-template' })
            if (!state.slots['smithing.base']) issues.push({ code: 'missing-base' })
            if (!state.slots['smithing.addition']) issues.push({ code: 'missing-addition' })
            if (supportsSmithingTrimPattern(version) && !state.smithing.trimPattern.trim()) {
                issues.push({ code: 'missing-trim-pattern' })
            }
            break
        case 'smithing_transform':
            if (!state.slots['smithing.template']) issues.push({ code: 'missing-template' })
            if (!state.slots['smithing.base']) issues.push({ code: 'missing-base' })
            if (!state.slots['smithing.addition']) issues.push({ code: 'missing-addition' })
            if (!state.slots['smithing.result']) issues.push({ code: 'missing-result' })
            break
    }
    return issues
}

export function resultSlotsForType(type: RecipeState['recipeType']): readonly RecipeSlot[] {
    return RESULT_SLOTS_BY_TYPE[type] ?? []
}

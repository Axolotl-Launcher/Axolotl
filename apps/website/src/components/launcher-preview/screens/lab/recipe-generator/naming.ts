import { parseIdentifier, rawId } from './identifier.ts'
import type { RecipeSlot, RecipeSlotContext, RecipeState, SlotValue } from './types.ts'

const FALLBACK_NAME = 'recipe'

function uniqueNonEmpty(values: Array<string | undefined>): string[] {
    return [...new Set(values.filter((value): value is string => Boolean(value)))]
}

export function sanitizeRecipeName(value: string): string {
    return value
        .trim()
        .toLowerCase()
        .replace(/[^a-z0-9_]+/g, '_')
        .replace(/_+/g, '_')
        .replace(/^_+|_+$/g, '')
}

function itemSlug(value: SlotValue | undefined, ctx: RecipeSlotContext): string | undefined {
    const resolved =
        value?.kind === 'custom_item'
            ? ctx.customItemsByUid[value.uid]
            : value?.kind === 'custom_tag'
              ? ctx.customTagsByUid[value.uid]
              : value?.kind === 'item' || value?.kind === 'vanilla_tag'
                ? { id: value.id }
                : undefined
    if (!resolved) return undefined
    const ref = parseIdentifier(resolved.id)
    const base = rawId(ref).startsWith('minecraft:')
        ? rawId(ref).slice('minecraft:'.length)
        : rawId(ref).replace(':', '_')
    const slug = sanitizeRecipeName(base.replace(/[:/.-]+/g, '_'))
    if (!slug) return undefined
    return ref.data === undefined || ref.data === 0 ? slug : `${slug}_data_${ref.data}`
}

export function getAutoNameCandidates(recipe: RecipeState, ctx: RecipeSlotContext): string[] {
    switch (recipe.recipeType) {
        case 'brewing':
            return [
                ensureName(
                    `${itemSlug(recipe.slots['brewing.result'], ctx) ?? 'potion'}_${recipe.brewing?.outputPotion?.replace('minecraft:', '') ?? ''}_brewing`,
                ),
            ]
        case 'crafting':
            return uniqueNonEmpty([
                itemSlug(recipe.slots['crafting.result'], ctx) ?? 'crafting_recipe',
            ])
        case 'smelting':
        case 'blasting':
        case 'smoking':
        case 'campfire_cooking': {
            const result = itemSlug(recipe.slots['cooking.result'], ctx)
            const ingredient = itemSlug(recipe.slots['cooking.ingredient'], ctx)
            const suffix = {
                smelting: 'smelting',
                blasting: 'blasting',
                smoking: 'smoking',
                campfire_cooking: 'campfire_cooking',
            }[recipe.recipeType]
            const names =
                recipe.recipeType === 'smelting'
                    ? [
                          result ? `${result}_from_${suffix}` : undefined,
                          ingredient ? `${ingredient}_${suffix}` : undefined,
                          result && ingredient
                              ? `${result}_from_${suffix}_${ingredient}`
                              : undefined,
                          result,
                          ensureName(suffix),
                      ]
                    : [
                          result ? `${result}_from_${suffix}` : undefined,
                          ingredient ? `${ingredient}_${suffix}` : undefined,
                          ensureName(suffix),
                      ]
            return uniqueNonEmpty(names)
        }
        case 'stonecutter': {
            const result = itemSlug(recipe.slots['stonecutter.result'], ctx)
            const ingredient = itemSlug(recipe.slots['stonecutter.ingredient'], ctx)
            let base = 'stonecutting_recipe'
            if (result && ingredient) base = `${result}_from_${ingredient}_stonecutting`
            else if (result) base = `${result}_stonecutting`
            else if (ingredient) base = `${ingredient}_stonecutting`
            return uniqueNonEmpty([ensureName(base)])
        }
        case 'smithing_trim': {
            const template = itemSlug(recipe.slots['smithing.template'], ctx)
            return uniqueNonEmpty([template ? `${template}_smithing_trim` : 'smithing_trim'])
        }
        case 'smithing':
        case 'smithing_transform': {
            const result = itemSlug(recipe.slots['smithing.result'], ctx)
            const baseItem = itemSlug(recipe.slots['smithing.base'], ctx)
            const base = result
                ? `${result}_smithing`
                : baseItem
                  ? `${baseItem}_smithing`
                  : 'smithing_recipe'
            return uniqueNonEmpty([ensureName(base)])
        }
    }
}

function ensureName(value: string): string {
    return sanitizeRecipeName(value) || FALLBACK_NAME
}

export function getAutoRecipeName(recipe: RecipeState, ctx: RecipeSlotContext): string {
    return getAutoNameCandidates(recipe, ctx)[0] ?? FALLBACK_NAME
}

type NameEntry = {
    recipe: RecipeState
    fixedName?: string
    possibleNames: string[]
}

function assignUniqueNames(entries: NameEntry[]): Map<string, string> {
    const usedNames = new Set<string>()
    const namesById = new Map<string, string>()

    for (const entry of entries) {
        if (!entry.fixedName) continue
        namesById.set(entry.recipe.id, entry.fixedName)
        usedNames.add(entry.fixedName)
    }

    for (const entry of entries) {
        if (entry.fixedName) continue
        const possibleNames = uniqueNonEmpty(
            entry.possibleNames.length ? entry.possibleNames : [FALLBACK_NAME],
        )
        const existingName = possibleNames.find((name) => !usedNames.has(name))
        if (existingName) {
            namesById.set(entry.recipe.id, existingName)
            usedNames.add(existingName)
            continue
        }
        const base = possibleNames[possibleNames.length - 1] ?? FALLBACK_NAME
        let index = 2
        let selected = `${base}_${index}`
        while (usedNames.has(selected)) {
            index += 1
            selected = `${base}_${index}`
        }
        namesById.set(entry.recipe.id, selected)
        usedNames.add(selected)
    }

    return namesById
}

function getSidebarTitle(recipe: RecipeState, ctx: RecipeSlotContext): string | undefined {
    const result = recipe.slots[resultSlotForType(recipe.recipeType)]
    const resolved =
        result?.kind === 'custom_item'
            ? ctx.customItemsByUid[result.uid]
            : result?.kind === 'item'
              ? { name: ctx.itemsById[result.id]?.name }
              : undefined
    if (resolved?.name) return resolved.name
    return undefined
}

function resultSlotForType(type: RecipeState['recipeType']): RecipeSlot | undefined {
    switch (type) {
        case 'brewing':
            return 'brewing.result'
        case 'crafting':
            return 'crafting.result'
        case 'smelting':
        case 'blasting':
        case 'smoking':
        case 'campfire_cooking':
            return 'cooking.result'
        case 'stonecutter':
            return 'stonecutter.result'
        case 'smithing':
        case 'smithing_transform':
            return 'smithing.result'
        case 'smithing_trim':
            return undefined
    }
}

export type ResolvedRecipeNaming = {
    autoName: string
    resolvedName: string
    sidebarTitle: string
}

export function resolveRecipeNames(
    recipes: RecipeState[],
    ctx: RecipeSlotContext,
): Map<string, ResolvedRecipeNaming> {
    const entries: NameEntry[] = recipes.map((recipe) => {
        const manualName =
            recipe.nameMode === 'manual' ? sanitizeRecipeName(recipe.name) : undefined
        return {
            recipe,
            fixedName: manualName || undefined,
            possibleNames: getAutoNameCandidates(recipe, ctx),
        }
    })
    const assigned = assignUniqueNames(entries)
    const result = new Map<string, ResolvedRecipeNaming>()
    for (const entry of entries) {
        result.set(entry.recipe.id, {
            autoName: getAutoRecipeName(entry.recipe, ctx),
            resolvedName: assigned.get(entry.recipe.id) ?? getAutoRecipeName(entry.recipe, ctx),
            sidebarTitle: getSidebarTitle(entry.recipe, ctx) ?? '',
        })
    }
    return result
}

export function getCurrentRecipeName(
    recipe: RecipeState,
    recipes: RecipeState[],
    ctx: RecipeSlotContext,
): ResolvedRecipeNaming {
    const resolved = resolveRecipeNames(recipes, ctx).get(recipe.id)
    return (
        resolved ?? {
            autoName: getAutoRecipeName(recipe, ctx),
            resolvedName: getAutoRecipeName(recipe, ctx),
            sidebarTitle: getSidebarTitle(recipe, ctx) ?? '',
        }
    )
}

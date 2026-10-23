import { parseIdentifier, rawId } from './identifier.ts'
import type { RecipeSlotContext, SlotValue } from './types.ts'

export type SlotDisplay = {
    label: string
    texture: string | null
    isTag: boolean
    count?: number
}

export function getSlotDisplay(
    value: SlotValue | undefined,
    ctx: RecipeSlotContext,
): SlotDisplay | null {
    if (!value) return null
    if (value.kind === 'item') {
        const item = ctx.itemsById[value.id]
        return {
            label: item?.name ?? rawId(parseIdentifier(value.id)),
            texture: item?.texture ?? null,
            isTag: false,
            count: value.count,
        }
    }
    if (value.kind === 'custom_item') {
        const item = ctx.customItemsByUid[value.uid]
        return item
            ? { label: item.name, texture: item.texture || null, isTag: false, count: value.count }
            : { label: value.uid, texture: null, isTag: false, count: value.count }
    }
    if (value.kind === 'vanilla_tag') {
        const members = ctx.vanillaTags[value.id] ?? []
        const firstTexture = members
            .map((id) => ctx.itemsById[id]?.texture)
            .find((texture): texture is string => Boolean(texture))
        return {
            label: `#${value.id}`,
            texture: firstTexture ?? null,
            isTag: true,
        }
    }
    const tag = ctx.customTagsByUid[value.uid]
    return tag
        ? { label: `#${tag.id}`, texture: null, isTag: true }
        : { label: value.uid, texture: null, isTag: true }
}

export function getSlotTextureHash(
    value: SlotValue | undefined,
    ctx: RecipeSlotContext,
): string | null {
    const display = getSlotDisplay(value, ctx)
    if (display?.texture) return display.texture
    return null
}

import type { LoadedVersionResources } from '../shared/minecraft/resources.ts'
import type { CustomItem, CustomTag, RecipeSlotContext } from './types.ts'

export function buildSlotContext(
    customItems: CustomItem[],
    customTags: CustomTag[],
    resources: LoadedVersionResources,
): RecipeSlotContext {
    const customItemsByUid: Record<string, CustomItem> = {}
    for (const item of customItems) customItemsByUid[item.uid] = item
    const customTagsByUid: Record<string, CustomTag> = {}
    for (const tag of customTags) customTagsByUid[tag.uid] = tag
    return {
        itemsById: resources.itemsById,
        customItemsByUid,
        customTagsByUid,
        vanillaTags: resources.vanillaTags,
    }
}

import itemNameIndexData from './assets/vanilla/item-name-index.json' with { type: 'json' }

type RecipeItemNameIndex = {
    en_us: Record<string, string>
    zh_cn: Record<string, string>
}

const itemNameIndex = itemNameIndexData as RecipeItemNameIndex

function translationKey(id: string): string | null {
    const normalized = id.trim().toLowerCase()
    const namespaced = normalized.includes(':') ? normalized : `minecraft:${normalized}`
    if (!namespaced.startsWith('minecraft:') || /:\d+$/.test(namespaced)) return null
    const path = namespaced.slice('minecraft:'.length)
    if (!path) return null
    const blockKey = `block.minecraft.${path}`
    if (blockKey in itemNameIndex.en_us || blockKey in itemNameIndex.zh_cn) return blockKey
    const itemKey = `item.minecraft.${path}`
    if (itemKey in itemNameIndex.en_us || itemKey in itemNameIndex.zh_cn) return itemKey
    return null
}

export function resolveRecipeItemName(id: string, locale: string, readable: string): string {
    const preferred = locale.toLowerCase().startsWith('zh') ? 'zh_cn' : 'en_us'
    const key = translationKey(id)
    if (!key) return readable
    return itemNameIndex[preferred][key] ?? itemNameIndex.en_us[key] ?? readable
}

export function resolveRecipePotionName(id: string, locale: string): string {
    const key = `item.minecraft.potion.effect.${id.replace('minecraft:', '')}`
    const preferred = locale.toLowerCase().startsWith('zh') ? 'zh_cn' : 'en_us'
    return itemNameIndex[preferred][key] ?? itemNameIndex.en_us[key] ?? id
}

/** Names from the same pinned vanilla language files as the shared item atlas. */
export function resolveMinecraftAttributeName(
    category: 'enchantment' | 'effect',
    id: string,
    locale: string,
): string | undefined {
    const normalized = id.includes(':') ? id : `minecraft:${id}`
    if (!/^minecraft:[a-z0-9_]+$/.test(normalized)) return undefined
    const path = normalized.slice('minecraft:'.length)
    const enchantmentAliases: Record<string, string> = {
        curse_of_binding: 'binding_curse',
        curse_of_vanishing: 'vanishing_curse',
    }
    const key = `${category}.minecraft.${category === 'enchantment' ? (enchantmentAliases[path] ?? path) : path}`
    const preferred = locale.toLowerCase().startsWith('zh') ? 'zh_cn' : 'en_us'
    return itemNameIndex[preferred][key] ?? itemNameIndex.en_us[key]
}

export function minecraftEnchantmentLevel(level: number): string {
    return itemNameIndex.en_us[`enchantment.level.${level}`] ?? String(level)
}

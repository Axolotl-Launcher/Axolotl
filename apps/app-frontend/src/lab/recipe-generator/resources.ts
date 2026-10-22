import atlasLayoutData from './assets/texture-atlas.json'
import atlasUrl from './assets/texture-atlas.png?url'
import type { CustomItem, CustomTag, JavaVersionId, RecipeSlotContext } from './types.ts'

export type TextureAtlas = {
    url: string
    layout: Record<string, [number, number, number, number]>
}

export const TEXTURE_ATLAS: TextureAtlas = {
    url: atlasUrl,
    layout: atlasLayoutData as Record<string, [number, number, number, number]>,
}

export const TEXTURE_ATLAS_SIZE = {
    width: 2_048,
    height: 1440,
} as const

export type TextureManifestItem = {
    id: string
    readable: string
    texture: string
}

export type TextureManifest = {
    version: string
    items: TextureManifestItem[]
}

export type ItemInfo = {
    id: string
    name: string
    texture: string | null
}

export type LoadedVersionResources = {
    version: JavaVersionId
    items: ItemInfo[]
    itemsById: Record<string, ItemInfo>
    vanillaTags: Record<string, string[]>
}

const itemLoaders = import.meta.glob<{ default: TextureManifest }>('./assets/items/*.json')
const tagLoaders = import.meta.glob<{ default: Record<string, string[]> }>('./assets/tags/*.json')

const versionResourcesCache = new Map<JavaVersionId, Promise<LoadedVersionResources>>()
const itemCache = new Map<string, Promise<ItemInfo[]>>()
const tagCache = new Map<string, Promise<Record<string, string[]>>>()

export async function loadVersionResources(
    version: JavaVersionId,
): Promise<LoadedVersionResources> {
    const cached = versionResourcesCache.get(version)
    if (cached) return cached
    const promise = (async () => {
        const [items, vanillaTags] = await Promise.all([
            loadItems(version),
            loadVanillaTags(version),
        ])
        const itemsById: Record<string, ItemInfo> = {}
        for (const item of items) itemsById[item.id] = item
        return { version, items, itemsById, vanillaTags }
    })()
    versionResourcesCache.set(version, promise)
    return promise
}

export async function loadItems(version: JavaVersionId): Promise<ItemInfo[]> {
    const cached = itemCache.get(version)
    if (cached) return cached
    const loader = itemLoaders[`./assets/items/${version}.json`]
    if (!loader) throw new Error(`No item manifest for ${version}`)
    const promise = loader().then(({ default: manifest }) =>
        (manifest.items ?? []).map((item) => ({
            id: item.id,
            name: item.readable,
            texture: item.texture || null,
        })),
    )
    itemCache.set(version, promise)
    return promise
}

export async function loadVanillaTags(version: JavaVersionId): Promise<Record<string, string[]>> {
    const cached = tagCache.get(version)
    if (cached) return cached
    const loader = tagLoaders[`./assets/tags/${version}.json`]
    const promise = loader ? loader().then(({ default: tags }) => tags) : Promise.resolve({})
    tagCache.set(version, promise)
    return promise
}

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

export function clearResourceCaches(): void {
    versionResourcesCache.clear()
    itemCache.clear()
    tagCache.clear()
}

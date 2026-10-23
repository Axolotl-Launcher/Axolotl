import { loadItems, TEXTURE_ATLAS } from './resources.ts'
import { LATEST_JAVA_VERSION } from './versions.ts'

const cache = new Map<string, Promise<HTMLImageElement>>()
let atlas: Promise<HTMLImageElement> | undefined

function loadImage(source: string): Promise<HTMLImageElement> {
    return new Promise((resolve, reject) => {
        const image = new Image()
        image.onload = () => resolve(image)
        image.onerror = () => reject(new Error(`Cannot load Minecraft item image: ${source}`))
        image.src = source
    })
}

/** Reuses the recipe atlas for map details and loot without packaging extra images. */
export function loadMinecraftItemImage(id: string): Promise<HTMLImageElement> {
    const cached = cache.get(id)
    if (cached) return cached
    const promise = (async () => {
        const item = (await loadItems(LATEST_JAVA_VERSION)).find((item) => item.id === id)
        const region = item?.texture ? TEXTURE_ATLAS.layout[item.texture] : undefined
        if (!region) throw new Error(`No Minecraft item texture: ${id}`)
        atlas ??= loadImage(TEXTURE_ATLAS.url)
        const image = await atlas
        const canvas = document.createElement('canvas')
        canvas.width = region[2]
        canvas.height = region[3]
        canvas.getContext('2d')?.drawImage(image, ...region, 0, 0, region[2], region[3])
        return await loadImage(canvas.toDataURL('image/png'))
    })()
    cache.set(id, promise)
    void promise.catch(() => cache.delete(id))
    return promise
}

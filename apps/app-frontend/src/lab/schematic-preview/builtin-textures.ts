import blocksAtlasUrl from './assets/vanilla/texture-atlas.png?url'
import atlasUvData from './assets/vanilla/texture-layout.json'

type RawTextureRegion = [number, number, number, number]
export type BuiltinTextureRegion = { x: number; y: number; width: number; height: number }
const atlasUvs = atlasUvData as Record<string, RawTextureRegion>

function normalizeTextureId(value: string) {
    const normalized = value.trim().toLowerCase()
    return normalized.includes(':') ? normalized : `minecraft:${normalized}`
}

export function builtinTextureRegion(textureId: string): BuiltinTextureRegion | undefined {
    const normalized = normalizeTextureId(textureId)
    const [namespace, path] = normalized.split(':', 2)
    if (namespace !== 'minecraft' || !path) return undefined
    const region = atlasUvs[path]
    if (!region) return undefined
    return { x: region[0], y: region[1], width: region[2], height: region[3] }
}

export function builtinTextureUvs(
    canvasWidth = BUILTIN_ATLAS_WIDTH,
    canvasHeight = BUILTIN_ATLAS_HEIGHT,
) {
    const textureUvs: Record<string, [number, number, number, number]> = {}
    for (const [name, region] of Object.entries(atlasUvs)) {
        const visibleHeight = Math.min(region[2], region[3])
        textureUvs[`minecraft:${name}`] = [
            region[0] / canvasWidth,
            region[1] / canvasHeight,
            (region[0] + region[2]) / canvasWidth,
            (region[1] + visibleHeight) / canvasHeight,
        ]
    }
    return textureUvs
}

export async function loadBuiltinAtlas(signal?: AbortSignal) {
    const response = await fetch(blocksAtlasUrl, { signal })
    if (!response.ok) throw new Error('Unable to load the built-in Minecraft texture atlas.')
    const bitmap = await createImageBitmap(await response.blob())
    if (signal?.aborted) {
        bitmap.close()
        signal.throwIfAborted()
    }
    return bitmap
}

export const BUILTIN_ATLAS_WIDTH = 2048
export const BUILTIN_ATLAS_IMAGE_HEIGHT = 2128
export const BUILTIN_ATLAS_HEIGHT = 4096

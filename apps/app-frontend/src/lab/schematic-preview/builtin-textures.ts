import bubbleParticleUrl from './assets/vanilla/particle-bubble.png?url'
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

/**
 * The bubble column sprite. It is a particle in the game and never lands in the atlas, so it ships
 * on its own and is pasted into the atlas at `BUILTIN_BUBBLE_ORIGIN`.
 */
export async function loadBuiltinBubbleParticle(signal?: AbortSignal) {
    const response = await fetch(bubbleParticleUrl, { signal })
    if (!response.ok) throw new Error('Unable to load the built-in bubble particle sprite.')
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

/**
 * The atlas is taller than the block textures it holds, so the bubble sprite is pasted into the
 * empty rows below them. That keeps every pass on the one atlas texture: a pass that sampled a
 * missing image would draw plain white quads instead of the sprite.
 */
export const BUILTIN_BUBBLE_ORIGIN = { x: 0, y: BUILTIN_ATLAS_IMAGE_HEIGHT } as const

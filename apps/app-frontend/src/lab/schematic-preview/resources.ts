import { CanvasTexture, NearestFilter, SRGBColorSpace } from 'three'

import type { SchematicBlockState } from './backend.ts'

export type SchematicWorkerResources = {
    blockDefinitions: Record<string, unknown>
    blockModels: Record<string, unknown>
    defaultBlockProperties: Record<string, Record<string, string>>
    textureUvs: Record<string, [number, number, number, number]>
    missingTextureUv: [number, number, number, number]
}

export type SchematicBlockNames = {
    en_us: Record<string, string>
    zh_cn: Record<string, string>
}

export type LoadedSchematicResources = {
    workerResources: SchematicWorkerResources
    previewResources: SchematicWorkerResources
    blockNames: SchematicBlockNames
    availableBlockStates: SchematicBlockState[]
    texture: CanvasTexture
    atlas: HTMLCanvasElement
}

type JsonObject = Record<string, unknown>

function asObject(value: unknown): JsonObject | undefined {
    return value !== null && typeof value === 'object' && !Array.isArray(value)
        ? (value as JsonObject)
        : undefined
}

function identifier(value: string) {
    const normalized = value.trim().toLowerCase()
    return normalized.includes(':') ? normalized : `minecraft:${normalized}`
}

function conventionalBlockTextureIds(blockId: string) {
    const normalized = identifier(blockId)
    const [namespace, path] = normalized.split(':', 2) as [string, string]
    return [
        `${namespace}:block/${path}`,
        `${namespace}:block/${path}_side`,
        `${namespace}:block/${path}_top`,
        `${namespace}:block/${path}_front`,
        `${namespace}:block/${path}_still`,
    ]
}

function blockTranslationKey(blockName: string) {
    const normalizedName = blockName.trim().toLowerCase()
    const [namespace, path] = normalizedName.includes(':')
        ? (normalizedName.split(':', 2) as [string, string])
        : ['minecraft', normalizedName]
    return `block.${namespace}.${path.replaceAll('/', '.')}`
}

function humanizeBlockName(blockName: string) {
    const path = blockName.trim().split(':').at(-1) ?? blockName
    return path
        .split(/[_./-]+/)
        .filter(Boolean)
        .map((word) => `${word.charAt(0).toUpperCase()}${word.slice(1)}`)
        .join(' ')
}

export function resolveSchematicBlockName(
    blockName: string,
    blockNames: SchematicBlockNames,
    locale: string,
) {
    const translationKey = blockTranslationKey(blockName)
    const preferred = locale.toLowerCase().startsWith('zh') ? 'zh_cn' : 'en_us'
    const fallback = preferred === 'zh_cn' ? 'en_us' : 'zh_cn'
    return (
        blockNames[preferred][translationKey] ??
        blockNames[fallback][translationKey] ??
        humanizeBlockName(blockName)
    )
}

function firstModelReference(value: unknown): string | undefined {
    if (Array.isArray(value)) {
        for (const entry of value) {
            const model = firstModelReference(entry)
            if (model) return model
        }
        return undefined
    }
    const object = asObject(value)
    return typeof object?.model === 'string' ? object.model : undefined
}

function blockModelReference(definition: unknown) {
    const object = asObject(definition)
    const variants = asObject(object?.variants)
    if (variants) {
        for (const variant of Object.values(variants)) {
            const model = firstModelReference(variant)
            if (model) return model
        }
    }
    if (Array.isArray(object?.multipart)) {
        for (const part of object.multipart) {
            const model = firstModelReference(asObject(part)?.apply)
            if (model) return model
        }
    }
    return undefined
}

function modelTextures(
    modelId: string,
    resources: SchematicWorkerResources,
    visited = new Set<string>(),
): Record<string, string> {
    if (visited.has(modelId)) return {}
    visited.add(modelId)
    const model = asObject(resources.blockModels[modelId])
    if (!model) return {}
    const parent = typeof model.parent === 'string' ? identifier(model.parent) : undefined
    const textures = parent ? modelTextures(parent, resources, visited) : {}
    for (const [key, value] of Object.entries(asObject(model.textures) ?? {})) {
        if (typeof value === 'string') textures[key] = value
    }
    return textures
}

function resolveTextureReference(value: string, textures: Record<string, string>) {
    let current = value
    const visited = new Set<string>()
    while (current.startsWith('#')) {
        const key = current.slice(1)
        if (visited.has(key)) return undefined
        visited.add(key)
        const next = textures[key]
        if (!next) return undefined
        current = next
    }
    return identifier(current)
}

export function resolveSchematicMaterialTexture(
    blockName: string,
    resources: SchematicWorkerResources,
) {
    const normalizedName = blockName.trim().toLowerCase()
    const [namespace, path] = normalizedName.includes(':')
        ? (normalizedName.split(':', 2) as [string, string])
        : ['minecraft', normalizedName]
    const modelReference = blockModelReference(resources.blockDefinitions[normalizedName])
    if (modelReference) {
        const modelId = identifier(modelReference)
        const textures = modelTextures(modelId, resources)
        const preferredKeys = ['all', 'side', 'top', 'end', 'front', 'texture', 'particle']
        for (const key of [...preferredKeys, ...Object.keys(textures)]) {
            const value = textures[key]
            if (!value) continue
            const textureId = resolveTextureReference(value, textures)
            if (textureId && resources.textureUvs[textureId]) return resources.textureUvs[textureId]
        }
    }

    const candidates = conventionalBlockTextureIds(`${namespace}:${path}`)
    for (const candidate of candidates) {
        if (resources.textureUvs[candidate]) return resources.textureUvs[candidate]
    }
    return undefined
}

export function configureSchematicTexture(texture: CanvasTexture) {
    // Deepslate UVs address atlas rows in the canvas's original top-to-bottom order.
    texture.flipY = false
    texture.magFilter = NearestFilter
    texture.minFilter = NearestFilter
    texture.generateMipmaps = false
    texture.needsUpdate = true
    return texture
}

export function minecraftVersionFromDataVersion(dataVersion?: number) {
    if (dataVersion === undefined) return undefined
    if (dataVersion === 5023) return '26.3'
    if (dataVersion === 4903) return '26.2'
    if (dataVersion === 4786) return '26.1'
    if (dataVersion > 4786) return undefined
    if (dataVersion >= 4671) return '1.21.11'
    if (dataVersion >= 4554) return '1.21.9'
    if (dataVersion >= 4437) return '1.21.6'
    if (dataVersion >= 4325) return '1.21.5'
    if (dataVersion >= 4189) return '1.21.4'
    if (dataVersion >= 4082) return '1.21.3'
    if (dataVersion >= 3953) return '1.21'
    if (dataVersion >= 3463) return '1.20'
    if (dataVersion >= 3105) return '1.19'
    if (dataVersion >= 2860) return '1.18'
    if (dataVersion >= 2724) return '1.17'
    if (dataVersion >= 2566) return '1.16'
    if (dataVersion >= 2200) return '1.15'
    if (dataVersion >= 1901) return '1.14'
    if (dataVersion >= 1451) return '1.13'
    return undefined
}

export async function createSchematicResources(
    version: string,
    palette: readonly SchematicBlockState[],
    signal?: AbortSignal,
): Promise<LoadedSchematicResources> {
    const builtin = await import('./builtin-resources.ts')
    signal?.throwIfAborted()
    const canvas = document.createElement('canvas')
    canvas.width = builtin.BUILTIN_ATLAS_WIDTH
    canvas.height = builtin.BUILTIN_ATLAS_HEIGHT
    const context = canvas.getContext('2d')
    if (!context) throw new Error('Unable to create the built-in texture atlas.')
    const builtInAtlas = await builtin.loadBuiltinAtlas(signal)
    context.drawImage(builtInAtlas, 0, 0)
    builtInAtlas.close()
    const availableBuiltinStates = builtin.listBuiltinBlockStates()
    const builtInBlocks = builtin.loadBuiltinBlockResources(version, palette)
    const previewBlocks = builtin.loadBuiltinBlockResources(version, [
        ...availableBuiltinStates,
        ...palette,
    ])
    const textureUvs = builtin.builtinTextureUvs()
    const missingTextureUv: [number, number, number, number] = textureUvs[
        'minecraft:block/gray_concrete'
    ] ?? [0, 0, 1 / 64, 1 / 64]
    const texture = configureSchematicTexture(new CanvasTexture(canvas))
    texture.colorSpace = SRGBColorSpace
    return {
        atlas: canvas,
        blockNames: builtin.loadBuiltinBlockNames(),
        availableBlockStates: availableBuiltinStates,
        workerResources: {
            ...builtInBlocks,
            textureUvs,
            missingTextureUv,
        },
        previewResources: {
            ...previewBlocks,
            textureUvs,
            missingTextureUv,
        },
        texture,
    }
}

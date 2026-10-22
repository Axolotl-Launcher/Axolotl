import blockModelsData from './assets/vanilla/block-model-index.json'
import blockNameData from './assets/vanilla/block-name-index.json'
import blockDefaultPropertiesData from './assets/vanilla/block-property-defaults.json'
import blockDefinitionsData from './assets/vanilla/block-state-index.json'
import type { SchematicBlockState } from './backend.ts'

type JsonObject = Record<string, unknown>
const blockDefinitions = blockDefinitionsData as Record<string, unknown>
const blockModels = blockModelsData as Record<string, unknown>
const blockDefaultProperties = blockDefaultPropertiesData as Record<string, Record<string, string>>
const blockNames = blockNameData as {
    en_us: Record<string, string>
    zh_cn: Record<string, string>
}

const BLOCK_ALIASES: Record<string, string> = {
    trapdoor: 'oak_trapdoor',
    chain: 'iron_chain',
    grass_path: 'dirt_path',
    grass: 'short_grass',
    sign: 'oak_sign',
    wall_sign: 'oak_wall_sign',
    banner: 'white_banner',
    wall_banner: 'white_wall_banner',
    bed: 'red_bed',
    skull: 'skeleton_skull',
    wall_skull: 'skeleton_wall_skull',
}

function identifier(value: string) {
    const normalized = value.trim().toLowerCase()
    return normalized.includes(':') ? normalized : `minecraft:${normalized}`
}

function minecraftPath(value: string) {
    const normalized = value.trim().toLowerCase()
    const [namespace, path] = normalized.includes(':')
        ? (normalized.split(':', 2) as [string, string])
        : ['minecraft', normalized]
    if (namespace !== 'minecraft') return undefined
    return BLOCK_ALIASES[path] ?? path
}

function collectModelReferences(value: unknown, references: Set<string>) {
    if (Array.isArray(value)) {
        for (const item of value) collectModelReferences(item, references)
        return
    }
    if (!value || typeof value !== 'object') return
    for (const [key, item] of Object.entries(value as JsonObject)) {
        if (key === 'model' && typeof item === 'string') {
            references.add(identifier(item))
        } else {
            collectModelReferences(item, references)
        }
    }
}

function builtinModel(modelId: string) {
    const [namespace, path] = modelId.split(':', 2)
    const model = namespace === 'minecraft' && path ? blockModels[path] : undefined
    if (!model || typeof model !== 'object' || Array.isArray(model)) return model
    const textures = (model as JsonObject).textures
    if (!textures || typeof textures !== 'object' || Array.isArray(textures)) return model

    return {
        ...(model as JsonObject),
        textures: Object.fromEntries(
            Object.entries(textures).map(([key, value]) => [
                key,
                value && typeof value === 'object' && !Array.isArray(value)
                    ? ((value as JsonObject).sprite ?? value)
                    : value,
            ]),
        ),
    }
}

export function completeBuiltinModelDependencies(
    _version: string,
    definitions: Record<string, unknown>,
    models: Record<string, unknown>,
) {
    const pendingModels = new Set<string>()
    for (const definition of Object.values(definitions)) {
        collectModelReferences(definition, pendingModels)
    }

    const visitedModels = new Set<string>()
    while (pendingModels.size > 0) {
        const modelId = pendingModels.values().next().value as string | undefined
        if (!modelId) break
        pendingModels.delete(modelId)
        if (visitedModels.has(modelId)) continue
        visitedModels.add(modelId)

        let model = models[modelId]
        if (!model) {
            model = builtinModel(modelId)
            if (model) models[modelId] = model
        }
        if (!model || typeof model !== 'object' || Array.isArray(model)) continue
        const parent = (model as JsonObject).parent
        if (typeof parent === 'string') pendingModels.add(identifier(parent))
    }
}

export function loadBuiltinBlockResources(
    version: string,
    palette: readonly SchematicBlockState[],
) {
    const definitions: Record<string, unknown> = {}
    const models: Record<string, unknown> = {}
    const defaultProperties: Record<string, Record<string, string>> = {}
    for (const state of palette) {
        const path = minecraftPath(state.name)
        if (!path) continue
        const definition = blockDefinitions[path]
        if (definition) definitions[state.name] = definition
        const defaults = blockDefaultProperties[path]
        if (defaults) defaultProperties[state.name] = defaults
    }
    completeBuiltinModelDependencies(version, definitions, models)

    return {
        blockDefinitions: definitions,
        blockModels: models,
        defaultBlockProperties: defaultProperties,
    }
}

export function listBuiltinBlockStates(): SchematicBlockState[] {
    return Object.keys(blockDefinitions).map((path) => ({
        name: `minecraft:${path}`,
        properties: { ...(blockDefaultProperties[path] ?? {}) },
    }))
}

export function loadBuiltinBlockNames() {
    return {
        en_us: { ...blockNames.en_us },
        zh_cn: { ...blockNames.zh_cn },
    }
}

export {
    BUILTIN_ATLAS_HEIGHT,
    BUILTIN_ATLAS_IMAGE_HEIGHT,
    BUILTIN_ATLAS_WIDTH,
    type BuiltinTextureRegion,
    builtinTextureRegion,
    builtinTextureUvs,
    loadBuiltinAtlas,
} from './builtin-textures'

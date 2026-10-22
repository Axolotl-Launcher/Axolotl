import {
    BlockDefinition,
    BlockModel,
    type Cull,
    Identifier,
    type TextureAtlasProvider,
} from 'deepslate'

import type { SchematicBlockState } from './backend'
import { getSchematicSpecialBlockMesh } from './meshing'

export type BlockGeometryResources = {
    blockDefinitions: Record<string, unknown>
    blockModels: Record<string, unknown>
    defaultBlockProperties: Record<string, Record<string, string>>
}

/** Shared model resolution for schematic and structure template mesh workers. */
export function createBlockGeometry(
    resources: BlockGeometryResources,
    atlas: TextureAtlasProvider,
) {
    const definitions: Record<string, BlockDefinition> = {}
    const models: Record<string, BlockModel> = {}
    const invalidModels = new Set<string>()
    const warnings: string[] = []
    const provider = {
        getBlockModel: (id: Identifier) =>
            invalidModels.has(id.toString()) ? null : (models[id.toString()] ?? null),
    }
    for (const [id, value] of Object.entries(resources.blockDefinitions)) {
        try {
            definitions[id] = BlockDefinition.fromJson(value)
        } catch {
            warnings.push(`Skipped invalid blockstate ${id}`)
        }
    }
    for (const [id, value] of Object.entries(resources.blockModels)) {
        try {
            models[id] = BlockModel.fromJson(value)
        } catch {
            warnings.push(`Skipped invalid block model ${id}`)
        }
    }
    for (const [id, model] of Object.entries(models)) {
        try {
            model.flatten(provider)
        } catch {
            invalidModels.add(id)
            warnings.push(`Skipped unresolved block model ${id}`)
        }
    }
    return {
        warnings,
        mesh(state: SchematicBlockState, cull: Cull) {
            const properties = {
                ...(resources.defaultBlockProperties[state.name] ?? {}),
                ...state.properties,
            }
            const definition = definitions[state.name]
            if (definition) {
                const mesh = definition.getMesh(
                    Identifier.parse(state.name),
                    properties,
                    atlas,
                    provider,
                    cull,
                )
                if (mesh.quads.length) return mesh
            }
            return getSchematicSpecialBlockMesh({ ...state, properties }, atlas, cull)
        },
    }
}

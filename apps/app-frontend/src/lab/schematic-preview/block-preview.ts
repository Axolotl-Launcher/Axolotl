import {
    BlockDefinition,
    BlockModel,
    Cull,
    Identifier,
    type Mesh,
    type TextureAtlasProvider,
} from 'deepslate'

import type { SchematicBlockState } from './backend'
import { SCHEMATIC_FLUID_OPACITY } from './materials'
import { getSchematicWaterloggedFluidMesh } from './meshing'
import type { LoadedSchematicResources, SchematicWorkerResources } from './resources'

type PreviewVertex = {
    position: [number, number, number]
    texture?: [number, number]
    color: [number, number, number]
}

type PreviewTriangle = {
    vertices: [PreviewVertex, PreviewVertex, PreviewVertex]
    depth: number
    shade: number
    translucent: boolean
}

type PreparedResources = {
    definitions: Map<string, BlockDefinition | null>
    models: Map<string, BlockModel | null>
    modelProvider: {
        getBlockModel: (id: Identifier) => BlockModel | null
    }
}

const preparedResourceCache = new WeakMap<SchematicWorkerResources, PreparedResources>()
const previewCameraYaw = Math.PI / 4
const previewCameraPitch = Math.PI / 6

function prepareResources(resources: SchematicWorkerResources) {
    const cached = preparedResourceCache.get(resources)
    if (cached) return cached
    const prepared: PreparedResources = {
        definitions: new Map(),
        models: new Map(),
        modelProvider: {
            getBlockModel(id) {
                const key = id.toString()
                if (prepared.models.has(key)) return prepared.models.get(key) ?? null
                const source = resources.blockModels[key]
                if (!source) {
                    prepared.models.set(key, null)
                    return null
                }
                try {
                    const model = BlockModel.fromJson(source)
                    prepared.models.set(key, model)
                    model.flatten(prepared.modelProvider)
                    return model
                } catch {
                    prepared.models.set(key, null)
                    return null
                }
            },
        },
    }
    preparedResourceCache.set(resources, prepared)
    return prepared
}

function blockDefinition(blockName: string, resources: SchematicWorkerResources) {
    const prepared = prepareResources(resources)
    if (prepared.definitions.has(blockName)) return prepared.definitions.get(blockName) ?? null
    const source = resources.blockDefinitions[blockName]
    if (!source) {
        prepared.definitions.set(blockName, null)
        return null
    }
    try {
        const definition = BlockDefinition.fromJson(source)
        prepared.definitions.set(blockName, definition)
        return definition
    } catch {
        prepared.definitions.set(blockName, null)
        return null
    }
}

export function projectSchematicBlockPreviewPosition(position: [number, number, number]) {
    const horizontal = Math.cos(previewCameraYaw)
    const depth = Math.sin(previewCameraYaw)
    return {
        x: position[0] * horizontal - position[2] * depth,
        y:
            (position[0] * depth + position[2] * horizontal) * Math.sin(previewCameraPitch) -
            position[1] * Math.cos(previewCameraPitch),
    }
}

function triangleShade(vertices: [PreviewVertex, PreviewVertex, PreviewVertex]) {
    const [first, second, third] = vertices.map((vertex) => vertex.position)
    const edgeA = first.map((value, index) => second[index] - value)
    const edgeB = first.map((value, index) => third[index] - value)
    const normal = [
        edgeA[1] * edgeB[2] - edgeA[2] * edgeB[1],
        edgeA[2] * edgeB[0] - edgeA[0] * edgeB[2],
        edgeA[0] * edgeB[1] - edgeA[1] * edgeB[0],
    ]
    const length = Math.hypot(...normal) || 1
    const light = [0.42, 0.82, 0.39]
    return (
        0.66 +
        0.34 *
            Math.abs(normal.reduce((sum, value, index) => sum + value * light[index], 0) / length)
    )
}

function meshTriangles(mesh: Mesh, translucent: boolean): PreviewTriangle[] {
    return mesh.quads.flatMap((quad) => {
        const vertices = quad.vertices().map((vertex): PreviewVertex => ({
            position: [vertex.pos.x, vertex.pos.y, vertex.pos.z],
            texture: vertex.texture ? [...vertex.texture] : undefined,
            color: [vertex.color[0], vertex.color[1], vertex.color[2]],
        }))
        return [
            [vertices[0], vertices[1], vertices[2]],
            [vertices[0], vertices[2], vertices[3]],
        ].map((triangle) => {
            const corners = triangle as [PreviewVertex, PreviewVertex, PreviewVertex]
            return {
                vertices: corners,
                depth:
                    corners.reduce(
                        (sum, vertex) =>
                            sum +
                            vertex.position[0] +
                            vertex.position[1] * 0.7 +
                            vertex.position[2],
                        0,
                    ) / 3,
                shade: triangleShade(corners),
                translucent,
            }
        })
    })
}

/** Exported for tests: the triangle list a swatch draws, host geometry first. */
export function previewTriangles(state: SchematicBlockState, resources: SchematicWorkerResources) {
    const definition = blockDefinition(state.name, resources)
    if (!definition) return []
    const prepared = prepareResources(resources)
    const atlas: TextureAtlasProvider = {
        getTextureAtlas: () => ({}) as ImageData,
        getTextureUV: (id) => resources.textureUvs[id.toString()] ?? resources.missingTextureUv,
    }
    try {
        const properties = {
            ...(resources.defaultBlockProperties[state.name] ?? {}),
            ...state.properties,
        }
        const mesh = definition.getMesh(
            Identifier.parse(state.name),
            properties,
            atlas,
            prepared.modelProvider,
            Cull.none(),
        )
        // Waterlogged fluid is meshed apart from the host, so it joins the same depth-sorted list
        // and is blended instead of being baked into the block's own faces.
        const fluid = getSchematicWaterloggedFluidMesh({ ...state, properties }, atlas, Cull.none())
        return [...meshTriangles(mesh, false), ...(fluid ? meshTriangles(fluid, true) : [])]
    } catch {
        return []
    }
}

function affineTransform(
    source: [[number, number], [number, number], [number, number]],
    target: [[number, number], [number, number], [number, number]],
) {
    const [[x0, y0], [x1, y1], [x2, y2]] = source
    const determinant = x0 * (y1 - y2) + x1 * (y2 - y0) + x2 * (y0 - y1)
    if (Math.abs(determinant) < 0.0001) return undefined
    const coefficient = (values: [number, number, number]) => ({
        first:
            (values[0] * (y1 - y2) + values[1] * (y2 - y0) + values[2] * (y0 - y1)) / determinant,
        second:
            (values[0] * (x2 - x1) + values[1] * (x0 - x2) + values[2] * (x1 - x0)) / determinant,
        offset:
            (values[0] * (x1 * y2 - x2 * y1) +
                values[1] * (x2 * y0 - x0 * y2) +
                values[2] * (x0 * y1 - x1 * y0)) /
            determinant,
    })
    const horizontal = coefficient([target[0][0], target[1][0], target[2][0]])
    const vertical = coefficient([target[0][1], target[1][1], target[2][1]])
    return [
        horizontal.first,
        vertical.first,
        horizontal.second,
        vertical.second,
        horizontal.offset,
        vertical.offset,
    ] as const
}

function traceTriangle(context: CanvasRenderingContext2D, points: [number, number][]) {
    context.beginPath()
    context.moveTo(...points[0])
    context.lineTo(...points[1])
    context.lineTo(...points[2])
    context.closePath()
}

export function renderSchematicBlockPreview(
    target: HTMLCanvasElement,
    state: SchematicBlockState,
    resources: LoadedSchematicResources,
) {
    const triangles = previewTriangles(state, resources.previewResources)
    if (triangles.length === 0) return false
    const projected = triangles.flatMap((triangle) =>
        triangle.vertices.map((vertex) => projectSchematicBlockPreviewPosition(vertex.position)),
    )
    const minX = Math.min(...projected.map((point) => point.x))
    const maxX = Math.max(...projected.map((point) => point.x))
    const minY = Math.min(...projected.map((point) => point.y))
    const maxY = Math.max(...projected.map((point) => point.y))
    const padding = target.width * 0.1
    const scale = Math.min(
        (target.width - padding * 2) / Math.max(0.1, maxX - minX),
        (target.height - padding * 2) / Math.max(0.1, maxY - minY),
    )
    const offsetX = (target.width - (minX + maxX) * scale) / 2
    const offsetY = (target.height - (minY + maxY) * scale) / 2
    const context = target.getContext('2d')
    if (!context) return false
    context.clearRect(0, 0, target.width, target.height)
    context.imageSmoothingEnabled = false

    for (const triangle of triangles.sort((left, right) => left.depth - right.depth)) {
        const points = triangle.vertices.map((vertex) => {
            const point = projectSchematicBlockPreviewPosition(vertex.position)
            return [point.x * scale + offsetX, point.y * scale + offsetY] as [number, number]
        })
        const texture = triangle.vertices.map((vertex) => vertex.texture)
        let drewTexture = false
        if (texture.every((value): value is [number, number] => value !== undefined)) {
            const transform = affineTransform(
                texture.map(([u, v]) => [
                    u * resources.atlas.width,
                    v * resources.atlas.height,
                ]) as [[number, number], [number, number], [number, number]],
                points as [[number, number], [number, number], [number, number]],
            )
            if (transform) {
                context.save()
                traceTriangle(context, points)
                context.clip()
                context.setTransform(...transform)
                context.drawImage(resources.atlas, 0, 0)
                context.restore()
                drewTexture = true
            }
        }
        const color = triangle.vertices
            .reduce(
                (result, vertex) => result.map((value, index) => value + vertex.color[index]),
                [0, 0, 0],
            )
            .map((value) => Math.round((value / 3) * 255))
        context.save()
        traceTriangle(context, points)
        context.clip()
        // The swatch blends its fluid exactly like the scene does, so the two never disagree.
        if (triangle.translucent) context.globalAlpha = SCHEMATIC_FLUID_OPACITY
        if (!drewTexture) {
            context.fillStyle = '#8c9692'
            context.fillRect(0, 0, target.width, target.height)
        }
        context.globalCompositeOperation = 'multiply'
        context.fillStyle = `rgb(${color.join(' ')})`
        context.fillRect(0, 0, target.width, target.height)
        context.globalCompositeOperation = 'source-atop'
        context.fillStyle = `rgb(0 0 0 / ${1 - triangle.shade})`
        context.fillRect(0, 0, target.width, target.height)
        context.restore()
    }
    return true
}

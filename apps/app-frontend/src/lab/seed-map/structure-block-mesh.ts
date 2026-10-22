import { Cull } from 'deepslate'

import { createBlockGeometry } from '../schematic-preview/block-geometry'
import {
    getSchematicMeshOcclusionFaces,
    SCHEMATIC_DIRECTION_OFFSETS,
    SCHEMATIC_DIRECTIONS,
    SCHEMATIC_OPPOSITE_DIRECTIONS,
    type SchematicDirection,
} from '../schematic-preview/meshing'
import type { SeedMapStructurePiece } from './advanced'
import {
    assembleStructureBlocks,
    type BlockPosition,
    rotateStructurePosition,
    type StructureBlock,
    structureBlockKey,
    type StructureBlocksResult,
    structureChestBlocks,
    type StructureTemplatePack,
    transformStructurePosition,
} from './structure-blocks'

export function buildStructureBlockMeshes(
    pieces: SeedMapStructurePiece[],
    pack: StructureTemplatePack,
    sparseBlocks?: StructureBlock[],
): StructureBlocksResult {
    const blocks = sparseBlocks
        ? new Map(sparseBlocks.map((block) => [structureBlockKey(block.position), block]))
        : assembleStructureBlocks(pieces, pack)
    const textureIds = Object.keys(pack.textureColors)
    const textureIndices = new Map(textureIds.map((id, index) => [id, index]))
    const textureSlotWidth = 2
    const geometry = createBlockGeometry(pack.resources, {
        getTextureAtlas: () => ({}) as ImageData,
        getTextureUV: (id) => {
            const index = textureIndices.get(id.toString())
            if (index === undefined) throw new Error(`Missing texture color: ${id}`)
            return [index * textureSlotWidth, 0, index * textureSlotWidth + 1, 1]
        },
    })
    const origin: BlockPosition = [pieces[0].position.x, pieces[0].position.y, pieces[0].position.z]
    const models = new Map<string, ReturnType<typeof geometry.mesh>>()
    const occlusion = new Map<string, ReturnType<typeof getSchematicMeshOcclusionFaces>>()
    const stateKey = (block: StructureBlock) => JSON.stringify(block.state)
    for (const block of blocks.values()) {
        const key = stateKey(block)
        if (occlusion.has(key)) continue
        const model = geometry.mesh(block.state, Cull.none())
        occlusion.set(key, getSchematicMeshOcclusionFaces(model))
    }
    const positions = new Map<number, number[]>()
    const colors = new Map<number, number[]>()
    const missing = new Set<string>()
    let coordinateCount = 0
    const directionAfterRotation = (direction: SchematicDirection, rotation: number) => {
        const offset = rotateStructurePosition(
            [...SCHEMATIC_DIRECTION_OFFSETS[direction]],
            rotation,
        )
        return SCHEMATIC_DIRECTIONS.find((candidate) =>
            SCHEMATIC_DIRECTION_OFFSETS[candidate].every((value, index) => value === offset[index]),
        )!
    }
    const mirrorDirection = (direction: SchematicDirection, mirror: boolean) =>
        !mirror
            ? direction
            : direction === 'west'
              ? 'east'
              : direction === 'east'
                ? 'west'
                : direction
    for (const block of blocks.values()) {
        const cull = Cull.none()
        for (const direction of SCHEMATIC_DIRECTIONS) {
            const worldDirection = directionAfterRotation(
                mirrorDirection(direction, block.mirror),
                block.rotation,
            )
            const delta = SCHEMATIC_DIRECTION_OFFSETS[worldDirection]
            const neighbor = blocks.get(
                structureBlockKey(
                    block.position.map((value, index) => value + delta[index]) as BlockPosition,
                ),
            )
            if (!neighbor) continue
            const facing = mirrorDirection(
                directionAfterRotation(
                    SCHEMATIC_OPPOSITE_DIRECTIONS[worldDirection],
                    (360 - neighbor.rotation) % 360,
                ),
                neighbor.mirror,
            )
            cull[direction] = occlusion.get(stateKey(neighbor))?.[facing] === true
        }
        const key = `${stateKey(block)}:${SCHEMATIC_DIRECTIONS.map((direction) => +!!cull[direction]).join('')}`
        let model = models.get(key)
        if (!model) {
            model = geometry.mesh(block.state, cull)
            models.set(key, model)
        }
        if (!model.quads.length) {
            if (!SCHEMATIC_DIRECTIONS.every((direction) => cull[direction]))
                missing.add(block.state.name)
            continue
        }
        let target = positions.get(block.pieceId)
        if (!target) {
            target = []
            positions.set(block.pieceId, target)
            colors.set(block.pieceId, [])
        }
        for (const quad of model.quads) {
            coordinateCount += 18
            if (coordinateCount > 6_000_000) throw new Error('Structure mesh limit exceeded')
            const sourceVertices = quad.vertices()
            // Virtual UV slots carry the texture identity through Deepslate's model resolver.
            const averageU =
                sourceVertices.reduce((sum, vertex) => {
                    if (!vertex.texture) throw new Error('Untextured structure face')
                    return sum + vertex.texture[0]
                }, 0) / sourceVertices.length
            const textureIndex = Math.floor(averageU / textureSlotWidth)
            const color = pack.textureColors[textureIds[textureIndex]]
            if (!color) throw new Error('Unresolved structure face color')
            const vertices = sourceVertices.map((vertex) => {
                const local = transformStructurePosition(
                    [vertex.pos.x - 0.5, vertex.pos.y, vertex.pos.z - 0.5],
                    block.rotation,
                    block.mirror,
                )
                return [
                    block.position[0] - origin[0] + local[0] + 0.5,
                    block.position[1] - origin[1] + local[1],
                    block.position[2] - origin[2] + local[2] + 0.5,
                ]
            })
            for (const index of block.mirror ? [0, 2, 1, 0, 3, 2] : [0, 1, 2, 0, 2, 3]) {
                target.push(...vertices[index])
                colors
                    .get(block.pieceId)!
                    .push(
                        ...color.map(
                            (channel, axis) => channel * sourceVertices[index].color[axis],
                        ),
                    )
            }
        }
    }
    return {
        meshes: [...positions].map(([pieceId, vertices]) => ({
            pieceId,
            positions: new Float32Array(vertices),
            colors: new Float32Array(colors.get(pieceId)!),
        })),
        origin,
        blockCount: blocks.size,
        missingModels: [...missing],
        chests: structureChestBlocks(blocks),
    }
}

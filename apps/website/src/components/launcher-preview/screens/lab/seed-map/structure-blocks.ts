import type {
    SeedMapStructureDetails,
    SeedMapStructurePiece,
    SeedMapStructureQuery,
} from './advanced.ts'
import catalog from './assets/catalog.json' with { type: 'json' }

/** `schematic-preview/backend.ts` `SchematicBlockState`. */
type SchematicBlockState = {
    name: string
    properties: Record<string, string>
}

/** `schematic-preview/block-geometry.ts` `BlockGeometryResources`. */
type BlockGeometryResources = {
    blockDefinitions: Record<string, unknown>
    blockModels: Record<string, unknown>
    defaultBlockProperties: Record<string, Record<string, string>>
}

export type BlockPosition = [number, number, number]
export type StructureTemplate = {
    size: BlockPosition
    palette: SchematicBlockState[]
    alternativePalettes?: SchematicBlockState[][]
    blocks: [number, number, number, number][]
    overwriteAir: boolean
}
export type StructureTemplatePack = {
    schemaVersion: 2
    version: string
    dataVersion: number
    templates: Record<string, StructureTemplate>
    resources: BlockGeometryResources
    textureColors: Record<string, [number, number, number]>
}
export type StructureBlock = {
    position: BlockPosition
    state: SchematicBlockState
    rotation: number
    mirror: boolean
    pieceId: number
}
export type StructureBlockMesh = { pieceId: number; positions: Float32Array; colors: Float32Array }
export type StructureChest = {
    pieceId: number
    position: BlockPosition
    name: string
    containerId?: number
}
export type StructureBlocksResult = {
    meshes: StructureBlockMesh[]
    origin: BlockPosition
    blockCount: number
    missingModels: string[]
    chests: StructureChest[]
}
export type StructureBlocksRequest = {
    version: string
    pieces: SeedMapStructurePiece[]
    mode?: 'chests' | 'blocks'
    sparseBlocks?: StructureBlock[]
}
export type StructureBlocksResponse =
    | { type: 'ready'; result: StructureBlocksResult }
    | { type: 'chests'; chests: StructureChest[] }
    | { type: 'error'; message: string }

export const END_CITY_TEMPLATE_NAMES = [
    'base_floor',
    'base_roof',
    'bridge_end',
    'bridge_gentle_stairs',
    'bridge_piece',
    'bridge_steep_stairs',
    'fat_tower_base',
    'fat_tower_middle',
    'fat_tower_top',
    'second_floor_1',
    'second_floor_2',
    'second_roof',
    'ship',
    'third_floor_1',
    'third_floor_2',
    'third_roof',
    'tower_base',
    'tower_floor',
    'tower_piece',
    'tower_top',
] as const

export function seedMapStructureResourceVersion(version: string): string {
    return (
        (catalog.versions as Record<string, { resourceVersion?: string }>)[version]
            ?.resourceVersion ?? version
    )
}

export function supportsStructureBlocks(version: string, kind: string) {
    return (
        (catalog.versions as Record<string, { structures: string[] }>)[
            seedMapStructureResourceVersion(version)
        ]?.structures.includes(kind) ?? false
    )
}

type TemplateCatalog = Record<
    string,
    {
        dataVersion: number
        structures: string[]
        templates: Record<string, { size: BlockPosition; paletteCount: number }>
    }
>

export function seedMapTemplatePieces(
    details: SeedMapStructureDetails | null,
    query: Pick<SeedMapStructureQuery, 'version' | 'x' | 'z'>,
    kind: string,
): SeedMapStructurePiece[] {
    if (!details?.supported || !supportsStructureBlocks(query.version, kind)) return []
    const templates = (catalog.versions as TemplateCatalog)[
        seedMapStructureResourceVersion(query.version)
    ].templates
    const variant = details.variant
    const pieces: SeedMapStructurePiece[] = []
    const append = (
        id: number,
        templateId: string,
        position: BlockPosition,
        rotation: number,
        mirror = false,
    ) => {
        const size = templates[templateId]?.size
        if (!size || !position.every(Number.isSafeInteger) || ![0, 90, 180, 270].includes(rotation))
            return
        const corners = [0, size[0] - 1].flatMap((x) =>
            [0, size[2] - 1].map((z) => transformStructurePosition([x, 0, z], rotation, mirror)),
        )
        pieces.push({
            id,
            name: templateId.replace(/^end_city\//, ''),
            templateId,
            rotation,
            mirror,
            positionKnown: true,
            position: { x: position[0], y: position[1], z: position[2] },
            min: {
                x: position[0] + Math.min(...corners.map((p) => p[0])),
                y: position[1],
                z: position[2] + Math.min(...corners.map((p) => p[2])),
            },
            max: {
                x: position[0] + Math.max(...corners.map((p) => p[0])),
                y: position[1] + size[1] - 1,
                z: position[2] + Math.max(...corners.map((p) => p[2])),
            },
        })
    }
    if (
        kind === 'end-city' ||
        kind === 'shipwreck' ||
        kind === 'outpost' ||
        kind === 'ancient-city'
    ) {
        for (const piece of details.pieces) {
            if (!piece.positionKnown) continue
            const name = piece.name.replace(/^minecraft:/, '')
            const templateId =
                kind === 'end-city' ? `end_city/${name.replace(/^end_city\//, '')}` : name
            if (kind === 'shipwreck' && !templateId.startsWith('shipwreck/')) continue
            if (kind === 'outpost' && templateId !== 'pillager_outpost/watchtower') continue
            if (kind === 'ancient-city' && !templateId.startsWith('ancient_city/')) continue
            append(
                piece.id,
                templateId,
                [piece.position.x, piece.position.y, piece.position.z],
                kind === 'outpost' ? (variant?.rotation ?? piece.rotation) : piece.rotation,
            )
        }
    } else if (kind === 'igloo' && variant && details.pieces.length) {
        const base = details.pieces.find((piece) => piece.name === 'igloo/top')?.position
        if (!base) return []
        const rotation = variant.rotation + (variant.mirror ? 180 : 0)
        const place = (id: number, name: string, offset: BlockPosition, pivot: BlockPosition) => {
            const shifted = rotateStructurePosition(pivot, rotation)
            append(
                id,
                name,
                [
                    base.x + offset[0] + pivot[0] - shifted[0],
                    base.y + offset[1],
                    base.z + offset[2] + pivot[2] - shifted[2],
                ],
                rotation,
            )
        }
        if (
            variant.basement &&
            Number.isInteger(variant.size) &&
            variant.size >= 4 &&
            variant.size <= 11
        ) {
            place(
                details.pieces.find((piece) => piece.name === 'igloo/bottom')?.id ??
                    variant.size + 1,
                'igloo/bottom',
                [0, -3 - variant.size * 3, -2],
                [3, 0, 7],
            )
            for (let i = 0; i < variant.size - 1; i++)
                place(i + 1, 'igloo/middle', [2, -3 - i * 3, 4], [1, 0, 1])
        }
        place(
            details.pieces.find((piece) => piece.name === 'igloo/top')!.id,
            'igloo/top',
            [0, 0, 0],
            [3, 0, 5],
        )
    } else if (kind === 'ruined-portal' && variant) {
        const id = `ruined_portal/${variant.giant ? 'giant_' : ''}portal_${variant.start}`
        const size = templates[id]?.size
        if (!size) return []
        const pivot: BlockPosition = [Math.floor(size[0] / 2), 0, Math.floor(size[2] / 2)]
        const rotated = rotateStructurePosition(pivot, variant.rotation)
        append(
            details.pieces[0]?.id ?? 0,
            id,
            [
                Math.floor(query.x / 16) * 16 + pivot[0] - rotated[0],
                0,
                Math.floor(query.z / 16) * 16 + pivot[2] - rotated[2],
            ],
            variant.rotation,
            !!variant.mirror,
        )
    } else if (kind === 'nether-fossil' && variant?.offset) {
        append(
            0,
            `nether_fossils/fossil_${variant.start + 1}`,
            [
                Math.floor(query.x / 16) * 16 + variant.offset.x,
                variant.offset.y,
                Math.floor(query.z / 16) * 16 + variant.offset.z,
            ],
            variant.rotation,
        )
    }
    return pieces
}

/** Minecraft clockwise rotation about the template origin, in block coordinates. */
export function rotateStructurePosition([x, y, z]: BlockPosition, rotation: number): BlockPosition {
    switch (rotation) {
        case 0:
            return [x, y, z]
        case 90:
            return [-z, y, x]
        case 180:
            return [-x, y, -z]
        case 270:
            return [z, y, -x]
        default:
            throw new Error('Unsupported structure rotation')
    }
}

export function transformStructurePosition(
    [x, y, z]: BlockPosition,
    rotation: number,
    mirror = false,
): BlockPosition {
    return rotateStructurePosition([mirror ? -x : x, y, z], rotation)
}

export function structureBlockKey(position: BlockPosition) {
    return position.join(',')
}

export function structureChestBlocks(blocks: Map<string, StructureBlock>): StructureChest[] {
    return [...blocks.values()]
        .filter((block) =>
            ['minecraft:chest', 'minecraft:trapped_chest'].includes(block.state.name),
        )
        .map((block) => ({
            pieceId: block.pieceId,
            position: block.position,
            name: block.state.name,
        }))
}

/** Piece order and air processing match EndCityPiece's two BlockIgnoreProcessor modes. */
export function assembleStructureBlocks(
    pieces: SeedMapStructurePiece[],
    pack: StructureTemplatePack,
) {
    if (
        pack.schemaVersion !== 2 ||
        !supportsStructureBlocks(pack.version, 'end-city') ||
        (catalog.versions as Record<string, { dataVersion: number }>)[pack.version].dataVersion !==
            pack.dataVersion
    ) {
        throw new Error('Incompatible structure templates')
    }
    if (!pieces.length || pieces.length > 2048) throw new Error('Invalid structure piece count')
    const blocks = new Map<string, StructureBlock>()
    let processed = 0
    for (const piece of pieces) {
        const name = piece.templateId ?? piece.name.replace(/^minecraft:/, '')
        if (!piece.positionKnown) {
            throw new Error('Structure template or position is unavailable')
        }
        const template = pack.templates[name] ?? pack.templates[`end_city/${name}`]
        if (!template) throw new Error('Missing structure template')
        for (const [x, y, z, index] of template.blocks) {
            if (++processed > 2_000_000) throw new Error('Structure block limit exceeded')
            const state = template.palette[index]
            if (!state) throw new Error('Invalid template palette index')
            if (['minecraft:structure_block', 'minecraft:structure_void'].includes(state.name))
                continue
            const offset = transformStructurePosition([x, y, z], piece.rotation, piece.mirror)
            const position: BlockPosition = [
                piece.position.x + offset[0],
                piece.position.y + y,
                piece.position.z + offset[2],
            ]
            if (!position.every(Number.isSafeInteger)) throw new Error('Invalid block position')
            const key = structureBlockKey(position)
            if (
                ['minecraft:air', 'minecraft:cave_air', 'minecraft:void_air'].includes(state.name)
            ) {
                if (template.overwriteAir) blocks.delete(key)
            } else
                blocks.set(key, {
                    position,
                    state,
                    rotation: piece.rotation,
                    mirror: !!piece.mirror,
                    pieceId: piece.id,
                })
        }
    }
    return blocks
}

import type {
    SeedMapContainer,
    SeedMapStructureDetails,
    SeedMapStructurePiece,
} from './advanced.ts'
import type { StructureChest } from './structure-blocks.ts'
import { seedMapTemplatePieces } from './structure-blocks.ts'

export const SEED_MAP_LAYOUT_MAX_SCALE = 2

export function seedMapLootContainerForChest(
    chest: StructureChest,
    containers: SeedMapContainer[],
    kind: string,
) {
    const originOffset = kind === 'end-city' ? -1 : 0
    return containers.find(
        (container) =>
            container.piece === chest.pieceId &&
            container.x === chest.position[0] + originOffset &&
            container.z === chest.position[2] + originOffset,
    )
}

/** Vanilla Nether fortress turn rooms place chests at local Y=2 above the piece minimum. */
export function seedMapKnownLayoutChests(
    details: SeedMapStructureDetails | null,
    kind: string,
): StructureChest[] {
    if (kind !== 'fortress' || !details?.supported) return []
    const pieces = new Map(details.pieces.map((piece) => [piece.id, piece]))
    return details.containers.flatMap((container) => {
        const piece = pieces.get(container.piece)
        if (
            !piece?.positionKnown ||
            !piece.min ||
            !['NeSCLT', 'NeSCRT'].includes(piece.name) ||
            container.table.replace(/^minecraft:/, '') !== 'chests/nether_bridge'
        )
            return []
        const position: [number, number, number] = [container.x, piece.min.y + 2, container.z]
        return position.every(Number.isSafeInteger)
            ? [{ pieceId: piece.id, position, name: 'minecraft:chest', containerId: container.id }]
            : []
    })
}

export function seedMapStructureBadgePositions(
    structures: { id: string; minX: number; minY: number; maxX: number; maxY: number }[],
    viewport: { width: number; height: number },
) {
    const badges: { id: string; x: number; y: number; width: number }[] = []
    const width = Math.min(176, Math.max(80, viewport.width - 112))
    const minY = 32
    const maxY = Math.max(minY, viewport.height - 76)
    for (const structure of structures) {
        if (
            structure.maxX < 0 ||
            structure.minX > viewport.width ||
            structure.maxY < 0 ||
            structure.minY > viewport.height
        )
            continue
        const x = Math.max(56, Math.min(viewport.width - width - 56, structure.minX))
        let y = Math.max(minY, Math.min(maxY, structure.minY - 40))
        const overlaps = (top: number) =>
            badges.some(
                (badge) =>
                    x < badge.x + badge.width + 4 &&
                    x + width + 4 > badge.x &&
                    top < badge.y + 36 &&
                    top + 36 > badge.y,
            )
        if (overlaps(y)) {
            const candidates = badges
                .flatMap((badge) => [badge.y + 36, badge.y - 36])
                .filter((top) => top >= minY && top <= maxY && !overlaps(top))
                .sort((a, b) => Math.abs(a - y) - Math.abs(b - y))
            y = candidates[0] ?? y
        }
        badges.push({ id: structure.id, x, y, width })
    }
    return badges
}

export function seedMapPieceBounds(piece: SeedMapStructurePiece) {
    return { min: piece.min ?? piece.position, max: piece.max ?? piece.position }
}

export function seedMapLayoutPieces(details: SeedMapStructureDetails | null) {
    return (
        details?.pieces.filter(
            (piece) =>
                piece.positionKnown &&
                piece.min &&
                piece.max &&
                ['x', 'z'].every((axis) => {
                    const key = axis as 'x' | 'z'
                    return (
                        Number.isFinite(piece.min![key]) &&
                        Number.isFinite(piece.max![key]) &&
                        piece.max![key] >= piece.min![key]
                    )
                }),
        ) ?? []
    )
}

export function seedMapVolumePieces(details: SeedMapStructureDetails | null) {
    return seedMapLayoutPieces(details).filter(
        (piece) =>
            piece.min &&
            piece.max &&
            ['x', 'y', 'z'].every((axis) => {
                const key = axis as 'x' | 'y' | 'z'
                return (
                    Number.isFinite(piece.min![key]) &&
                    Number.isFinite(piece.max![key]) &&
                    piece.max![key] >= piece.min![key]
                )
            }),
    )
}

export function seedMapStructureCapabilities(
    details: SeedMapStructureDetails,
    version = '',
    kind = '',
    position = { x: 0, z: 0 },
) {
    const templates = seedMapTemplatePieces(details, { version, ...position }, kind)
    return {
        variant: details.capabilities?.variant ?? !!details.variant,
        projection: seedMapLayoutPieces(details).length > 0,
        volume: seedMapVolumePieces(details).length > 0 || templates.length > 0,
        containers: details.capabilities?.containers ?? details.containers.length > 0,
        complete:
            seedMapLayoutPieces(details).length > 0 &&
            seedMapLayoutPieces(details).length === details.pieces.length &&
            !details.truncated &&
            !details.limits.some((limit) =>
                ['partial_layout', 'layout_unavailable', 'piece_positions_unavailable'].includes(
                    limit,
                ),
            ),
        absoluteHeight: false,
        blocks: templates.length > 0,
    }
}

export function drawSeedMapLayout(
    context: CanvasRenderingContext2D,
    pieces: SeedMapStructurePiece[],
    scale: number,
    toScreen: (x: number, z: number) => { x: number; y: number },
    selected?: number,
) {
    if (scale > SEED_MAP_LAYOUT_MAX_SCALE) return
    context.save()
    const palette = seedMapLayoutPalette(context.canvas)
    for (const piece of pieces) {
        const { min, max } = seedMapPieceBounds(piece)
        const point = toScreen(min.x, min.z)
        const width = (max.x - min.x + 1) / scale
        const height = (max.z - min.z + 1) / scale
        if (
            point.x + width < 0 ||
            point.y + height < 0 ||
            point.x > context.canvas.width ||
            point.y > context.canvas.height
        )
            continue
        const style = seedMapLayoutPieceStyle(piece, selected, palette)
        context.fillStyle = style.color
        context.globalAlpha = style.fillAlpha
        context.fillRect(point.x, point.y, width, height)
        context.globalAlpha = 1
        context.strokeStyle = style.color
        context.lineWidth = style.lineWidth
        context.strokeRect(point.x, point.y, width, height)
    }
    context.restore()
}

export function seedMapLayoutPalette(element: Element) {
    const style = getComputedStyle(element)
    return {
        normal: style.getPropertyValue('--color-blue').trim(),
        selected: style.getPropertyValue('--color-orange').trim(),
        portal: style.getPropertyValue('--color-green').trim(),
    }
}

export function seedMapLayoutPieceStyle(
    piece: SeedMapStructurePiece,
    selected: number | undefined,
    palette: ReturnType<typeof seedMapLayoutPalette>,
) {
    const isSelected = piece.id === selected
    const isPortalRoom = piece.endPortalRoom === true
    return {
        color: isSelected ? palette.selected : isPortalRoom ? palette.portal : palette.normal,
        fillAlpha: isSelected ? 0.35 : isPortalRoom ? 0.3 : 0.2,
        lineWidth: isSelected || isPortalRoom ? 2 : 1,
    }
}

export function hitSeedMapLayout(
    pieces: SeedMapStructurePiece[],
    x: number,
    z: number,
    selected?: number,
) {
    const hits = pieces.filter((piece) => {
        const { min, max } = seedMapPieceBounds(piece)
        return x >= min.x && x < max.x + 1 && z >= min.z && z < max.z + 1
    })
    hits.sort((a, b) => {
        const size = (piece: SeedMapStructurePiece) => {
            const { min, max } = seedMapPieceBounds(piece)
            return (max.x - min.x + 1) * (max.z - min.z + 1)
        }
        return size(a) - size(b) || a.id - b.id
    })
    const index = hits.findIndex((piece) => piece.id === selected)
    return hits[(index + 1) % hits.length]
}

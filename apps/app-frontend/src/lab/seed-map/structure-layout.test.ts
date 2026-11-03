import { expect, it } from 'vitest'

import type {
    SeedMapStructureDetails,
    SeedMapStructurePiece,
    SeedMapStructureQuery,
} from './advanced.ts'
import { createSeedMapLayoutLayer } from './layout-layer.ts'
import {
    hitSeedMapLayout,
    seedMapLayoutPieces,
    seedMapLayoutPieceStyle,
    seedMapLootContainerForChest,
    seedMapStructureBadgePositions,
    seedMapStructureCapabilities,
} from './structure-layout.ts'

it('structure labels stay visible when an origin is offscreen and avoid each other', () => {
    const badges = seedMapStructureBadgePositions(
        [
            { id: 'end-city', minX: -300, minY: -200, maxX: 250, maxY: 180 },
            { id: 'fortress', minX: -120, minY: -100, maxX: 150, maxY: 200 },
            { id: 'outside', minX: 900, minY: 10, maxX: 950, maxY: 50 },
        ],
        { width: 640, height: 480 },
    )
    expect(badges.length).toBe(2)
    expect(badges[0].x).toBe(56)
    expect(badges[0].y).toBe(32)
    expect(Math.abs(badges[0].y - badges[1].y) >= 36).toBeTruthy()
    expect(
        badges.every((badge) => badge.x + badge.width <= 640 && badge.y + 32 <= 480),
    ).toBeTruthy()
})

const piece: SeedMapStructurePiece = {
    id: 1,
    name: 'SHPR',
    rotation: 0,
    positionKnown: true,
    position: { x: -20, y: 64, z: 8 },
    min: { x: -20, y: 64, z: 8 },
    max: { x: -10, y: 72, z: 23 },
}
const details: SeedMapStructureDetails = {
    supported: true,
    truncated: false,
    pieces: [piece],
    containers: [],
    limits: ['relative_height'],
}
const query: SeedMapStructureQuery = {
    seed: '1',
    edition: 'java',
    version: '26.3',
    dimension: 'overworld',
    x: -20,
    z: 8,
    featureMask: 512,
}

it('End City template chests match Cubiomes loot coordinates across the shifted origin', () => {
    const chest = {
        pieceId: 4,
        position: [19, 51, -29] as [number, number, number],
        name: 'minecraft:chest',
    }
    const containers = [{ id: 12, piece: 4, x: 18, z: -30, table: 'chests/end_city_treasure' }]

    expect(seedMapLootContainerForChest(chest, containers, 'end-city')?.id).toBe(12)
    expect(seedMapLootContainerForChest(chest, containers, 'stronghold')).toBe(undefined)
})

it('End portal room projections use a distinct color while selection keeps priority', () => {
    const palette = { normal: '#4b83c3', selected: '#e28b38', portal: '#18a765' }
    const portalRoom = { ...piece, endPortalRoom: true }

    expect(seedMapLayoutPieceStyle(portalRoom, undefined, palette)).toEqual({
        color: palette.portal,
        fillAlpha: 0.3,
        lineWidth: 2,
    })
    expect(seedMapLayoutPieceStyle(piece, undefined, palette)).toEqual({
        color: palette.normal,
        fillAlpha: 0.2,
        lineWidth: 1,
    })
    expect(seedMapLayoutPieceStyle(portalRoom, portalRoom.id, palette).color).toBe(palette.selected)
})

it('only real horizontal bounds become projections, including negative coordinates', () => {
    const result = {
        ...details,
        pieces: [
            piece,
            { ...piece, id: 2, positionKnown: false },
            { ...piece, id: 3, min: undefined },
            { ...piece, id: 4, min: { x: NaN, y: 0, z: 0 } },
        ],
    }
    expect(seedMapLayoutPieces(result)).toEqual([piece])
    expect(seedMapStructureCapabilities(result).complete).toBe(false)
    expect(hitSeedMapLayout([piece], -10.1, 23.9)).toBe(piece)
    expect(hitSeedMapLayout([piece], -9, 24)).toBe(undefined)
})

it('relative and partial layouts never advertise absolute world heights or blocks', () => {
    const result = seedMapStructureCapabilities({
        ...details,
        limits: ['partial_layout', 'height_placeholder'],
    })
    expect(result.projection).toBe(true)
    expect(result.volume).toBe(true)
    expect(result.complete).toBe(false)
    expect(result.absoluteHeight).toBe(false)
    expect(result.blocks).toBe(false)
    expect(seedMapStructureCapabilities({ ...details, pieces: [] }).volume).toBe(false)
    expect(
        seedMapStructureCapabilities({
            ...details,
            containers: [],
            capabilities: { variant: true, containers: true },
        }).containers,
    ).toBe(true)
})

it('overlapping projected pieces cycle without losing the containing structure', () => {
    const small = { ...piece, id: 2, max: { x: -16, y: 72, z: 12 } }
    expect(hitSeedMapLayout([piece, small], -18, 10)?.id).toBe(2)
    expect(hitSeedMapLayout([piece, small], -18, 10, 2)?.id).toBe(1)
})

function harness() {
    const pending: {
        query: SeedMapStructureQuery
        resolve: (result: SeedMapStructureDetails) => void
        reject: (error: Error) => void
    }[] = []
    let updates = 0
    const layer = createSeedMapLayoutLayer(
        (query) => new Promise((resolve, reject) => pending.push({ query, resolve, reject })),
        () => updates++,
    )
    return { layer, pending, updates: () => updates }
}
const settle = () => new Promise((resolve) => setTimeout(resolve, 0))

it('visible and selected queries share native work and reuse completed layouts', async () => {
    const { layer, pending } = harness()
    layer.update([query])
    const selected = layer.load(query)
    expect(pending.length).toBe(1)
    pending[0].resolve(details)
    expect(await selected).toBe(details)
    await settle()
    expect(await layer.load(query)).toBe(details)
    expect(pending.length).toBe(1)
    layer.dispose()
})

it('native queue is bounded and cancels obsolete queued work after seed changes', async () => {
    const { layer, pending, updates } = harness()
    layer.update([query, { ...query, x: 10 }, { ...query, x: 20 }, { ...query, x: 30 }])
    expect(pending.length).toBe(2)
    const next = { ...query, seed: '2' }
    layer.update([next])
    pending[0].resolve(details)
    pending[1].resolve(details)
    await settle()
    expect(pending.length).toBe(3)
    expect(pending[2].query.seed).toBe('2')
    expect(updates()).toBe(0)
    expect(layer.peek(query)).toBe(undefined)
    pending[2].resolve(details)
    await settle()
    expect(updates()).toBe(1)
    layer.dispose()
})

it('failed native queries release their queue slot and remain retryable', async () => {
    const { layer, pending } = harness()
    const first = layer.load(query)
    pending[0].reject(new Error('retry'))
    await expect(first).rejects.toThrow(/retry/)
    await settle()
    const second = layer.load(query)
    pending[1].resolve(details)
    expect(await second).toBe(details)
    layer.dispose()
})

it('rapid selection changes remove obsolete selected queries from the queue', async () => {
    const { layer, pending } = harness()
    layer.update([query, { ...query, x: 10 }])
    const old = layer.load({ ...query, x: 20 })
    const rejected = expect(old).rejects.toThrow(/no longer visible/)
    const latest = layer.load({ ...query, x: 30 })
    await rejected
    pending[0].resolve(details)
    await settle()
    expect(pending[2].query.x).toBe(30)
    pending[1].resolve(details)
    pending[2].resolve(details)
    expect(await latest).toBe(details)
    layer.dispose()
})

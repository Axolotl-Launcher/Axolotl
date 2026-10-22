import assert from 'node:assert/strict'
import test from 'node:test'

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

test('structure labels stay visible when an origin is offscreen and avoid each other', () => {
    const badges = seedMapStructureBadgePositions(
        [
            { id: 'end-city', minX: -300, minY: -200, maxX: 250, maxY: 180 },
            { id: 'fortress', minX: -120, minY: -100, maxX: 150, maxY: 200 },
            { id: 'outside', minX: 900, minY: 10, maxX: 950, maxY: 50 },
        ],
        { width: 640, height: 480 },
    )
    assert.equal(badges.length, 2)
    assert.equal(badges[0].x, 56)
    assert.equal(badges[0].y, 32)
    assert.ok(Math.abs(badges[0].y - badges[1].y) >= 36)
    assert.ok(badges.every((badge) => badge.x + badge.width <= 640 && badge.y + 32 <= 480))
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

test('End City template chests match Cubiomes loot coordinates across the shifted origin', () => {
    const chest = {
        pieceId: 4,
        position: [19, 51, -29] as [number, number, number],
        name: 'minecraft:chest',
    }
    const containers = [{ id: 12, piece: 4, x: 18, z: -30, table: 'chests/end_city_treasure' }]

    assert.equal(seedMapLootContainerForChest(chest, containers, 'end-city')?.id, 12)
    assert.equal(seedMapLootContainerForChest(chest, containers, 'stronghold'), undefined)
})

test('End portal room projections use a distinct color while selection keeps priority', () => {
    const palette = { normal: '#4b83c3', selected: '#e28b38', portal: '#18a765' }
    const portalRoom = { ...piece, endPortalRoom: true }

    assert.deepEqual(seedMapLayoutPieceStyle(portalRoom, undefined, palette), {
        color: palette.portal,
        fillAlpha: 0.3,
        lineWidth: 2,
    })
    assert.deepEqual(seedMapLayoutPieceStyle(piece, undefined, palette), {
        color: palette.normal,
        fillAlpha: 0.2,
        lineWidth: 1,
    })
    assert.equal(
        seedMapLayoutPieceStyle(portalRoom, portalRoom.id, palette).color,
        palette.selected,
    )
})

test('only real horizontal bounds become projections, including negative coordinates', () => {
    const result = {
        ...details,
        pieces: [
            piece,
            { ...piece, id: 2, positionKnown: false },
            { ...piece, id: 3, min: undefined },
            { ...piece, id: 4, min: { x: NaN, y: 0, z: 0 } },
        ],
    }
    assert.deepEqual(seedMapLayoutPieces(result), [piece])
    assert.equal(seedMapStructureCapabilities(result).complete, false)
    assert.equal(hitSeedMapLayout([piece], -10.1, 23.9), piece)
    assert.equal(hitSeedMapLayout([piece], -9, 24), undefined)
})

test('relative and partial layouts never advertise absolute world heights or blocks', () => {
    const result = seedMapStructureCapabilities({
        ...details,
        limits: ['partial_layout', 'height_placeholder'],
    })
    assert.equal(result.projection, true)
    assert.equal(result.volume, true)
    assert.equal(result.complete, false)
    assert.equal(result.absoluteHeight, false)
    assert.equal(result.blocks, false)
    assert.equal(seedMapStructureCapabilities({ ...details, pieces: [] }).volume, false)
    assert.equal(
        seedMapStructureCapabilities({
            ...details,
            containers: [],
            capabilities: { variant: true, containers: true },
        }).containers,
        true,
    )
})

test('overlapping projected pieces cycle without losing the containing structure', () => {
    const small = { ...piece, id: 2, max: { x: -16, y: 72, z: 12 } }
    assert.equal(hitSeedMapLayout([piece, small], -18, 10)?.id, 2)
    assert.equal(hitSeedMapLayout([piece, small], -18, 10, 2)?.id, 1)
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

test('visible and selected queries share native work and reuse completed layouts', async () => {
    const { layer, pending } = harness()
    layer.update([query])
    const selected = layer.load(query)
    assert.equal(pending.length, 1)
    pending[0].resolve(details)
    assert.equal(await selected, details)
    await settle()
    assert.equal(await layer.load(query), details)
    assert.equal(pending.length, 1)
    layer.dispose()
})

test('native queue is bounded and cancels obsolete queued work after seed changes', async () => {
    const { layer, pending, updates } = harness()
    layer.update([query, { ...query, x: 10 }, { ...query, x: 20 }, { ...query, x: 30 }])
    assert.equal(pending.length, 2)
    const next = { ...query, seed: '2' }
    layer.update([next])
    pending[0].resolve(details)
    pending[1].resolve(details)
    await settle()
    assert.equal(pending.length, 3)
    assert.equal(pending[2].query.seed, '2')
    assert.equal(updates(), 0)
    assert.equal(layer.peek(query), undefined)
    pending[2].resolve(details)
    await settle()
    assert.equal(updates(), 1)
    layer.dispose()
})

test('failed native queries release their queue slot and remain retryable', async () => {
    const { layer, pending } = harness()
    const first = layer.load(query)
    pending[0].reject(new Error('retry'))
    await assert.rejects(first, /retry/)
    await settle()
    const second = layer.load(query)
    pending[1].resolve(details)
    assert.equal(await second, details)
    layer.dispose()
})

test('rapid selection changes remove obsolete selected queries from the queue', async () => {
    const { layer, pending } = harness()
    layer.update([query, { ...query, x: 10 }])
    const old = layer.load({ ...query, x: 20 })
    const rejected = assert.rejects(old, /no longer visible/)
    const latest = layer.load({ ...query, x: 30 })
    await rejected
    pending[0].resolve(details)
    await settle()
    assert.equal(pending[2].query.x, 30)
    pending[1].resolve(details)
    pending[2].resolve(details)
    assert.equal(await latest, details)
    layer.dispose()
})

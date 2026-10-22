import assert from 'node:assert/strict'
import test from 'node:test'

import type { SeedMapLoot, SeedMapStructureDetails } from './advanced.ts'
import {
    analyzeAreaOres,
    areaChunkCount,
    searchAreaLoot,
    SEED_MAP_AREA_LOOT_MAX_CHUNKS,
    SEED_MAP_AREA_ORE_MAX_CHUNKS,
    seedMapArea,
} from './area-analysis.ts'
import type { SeedMapFeatureQuery, SeedMapOreScanChunk } from './backend.ts'
import type { SeedMapOreKind } from './ores.ts'

const context = {
    seed: '10292992',
    edition: 'java' as const,
    version: '26.3',
    dimension: 'overworld' as const,
}
const area = seedMapArea({ x: 0, z: 0 }, { x: -1, z: -1 })
const progress = () => {}
const item = (count: number) => ({ id: 'minecraft:diamond', count, enchantments: [] })
const details: SeedMapStructureDetails = {
    supported: true,
    truncated: false,
    containers: [
        { id: 0, piece: 0, x: 0, z: 0 },
        { id: 1, piece: 0, x: 2, z: 0 },
    ],
    pieces: [],
    limits: [],
}

test('area normalization includes negative chunk boundaries and rejects excessive regions before scanning', async () => {
    assert.deepEqual(area, { minX: -1, minZ: -1, maxX: 0, maxZ: 0 })
    assert.equal(areaChunkCount(area), 4)
    let calls = 0
    await assert.rejects(
        analyzeAreaOres(
            context,
            { minX: 0, minZ: 0, maxX: 10000, maxZ: 10000 },
            ['iron'],
            0,
            255,
            new AbortController().signal,
            progress,
            async () => {
                calls++
                return []
            },
        ),
    )
    assert.equal(calls, 0)
})

test('counts block positions once across ordinary, deep and large vein candidates, clips X/Z and Y, and omits zero rows', async () => {
    const requests: { ores: SeedMapOreKind[]; chunks: number[] }[] = []
    const result = await analyzeAreaOres(
        context,
        area,
        ['iron', 'iron_vein', 'diamond', 'dirt'],
        -64,
        10,
        new AbortController().signal,
        progress,
        async (request) => {
            requests.push(request)
            const hit = (x: number, y: number, z: number) => ({
                ore: request.ores[0],
                x,
                y,
                z,
                yMin: y,
                yMax: y,
                status: 'terrain_unverified' as const,
            })
            return [
                {
                    cx: request.chunks[0],
                    cz: request.chunks[1],
                    truncated: false,
                    hits:
                        request.ores[0] === 'diamond'
                            ? []
                            : [hit(0, 1, 0), hit(-1, -40, -1), hit(4, 1, 0), hit(0, 11, 0)],
                },
            ]
        },
    )
    assert.deepEqual(result.counts, [{ ore: 'iron', count: 2 }])
    assert.equal(result.truncated, false)
    assert.ok(requests.every((request) => request.ores.length === 1 && request.ores[0] !== 'dirt'))
    assert.ok(requests.some((request) => request.chunks.includes(-2)))
})

test('ore cancellation prevents subsequent requests and truncated scans remain explicitly partial', async () => {
    const controller = new AbortController()
    let calls = 0
    await assert.rejects(
        analyzeAreaOres(
            context,
            area,
            ['iron', 'diamond'],
            0,
            255,
            controller.signal,
            progress,
            async () => {
                calls++
                controller.abort()
                return []
            },
        ),
        { name: 'AbortError' },
    )
    assert.equal(calls, 1)
    const scan = async (): Promise<SeedMapOreScanChunk[]> => [
        { cx: 0, cz: 0, hits: [], truncated: true },
    ]
    assert.equal(
        (
            await analyzeAreaOres(
                context,
                area,
                ['diamond'],
                0,
                255,
                new AbortController().signal,
                progress,
                scan,
            )
        ).truncated,
        true,
    )
})

test('loot searches padded structure origins, filters container coordinates, merges identical items, and retains estimates', async () => {
    const queries: SeedMapFeatureQuery[] = []
    const containers: number[] = []
    const result = await searchAreaLoot(
        context,
        area,
        0x3ffffff,
        (id) => id === 'minecraft:diamond',
        new AbortController().signal,
        progress,
        {
            features: async (query) => {
                queries.push(query)
                return [{ kind: 'ancient-city', x: 300, z: 0, approximate: true }]
            },
            details: async (query) => {
                assert.equal(query.includeBlocks, false)
                return details
            },
            loot: async (_, id): Promise<SeedMapLoot> => {
                containers.push(id)
                return {
                    supported: true,
                    truncated: false,
                    estimated: true,
                    items: [item(2), item(3)],
                    limits: [],
                }
            },
        },
    )
    assert.ok(queries.some((query) => query.minX === -513))
    assert.ok(queries.every((query) => ((query.featureMask ?? 0) & (1 << 10)) === 0))
    assert.deepEqual(containers, [0])
    assert.equal(result.containers, 1)
    assert.equal(result.matches[0].items[0].count, 5)
    assert.equal(result.matches[0].estimated, true)
    assert.equal(result.matches[0].container.x, 0)
    assert.equal(result.matches[0].feature.x, 300)
})

test('loot failures, unsupported predictions and missing subtables cannot masquerade as complete results', async () => {
    const result = await searchAreaLoot(
        context,
        area,
        0x3ffffff,
        () => true,
        new AbortController().signal,
        progress,
        {
            features: async () => [{ kind: 'outpost', x: 0, z: 0, approximate: false }],
            details: async () => ({
                ...details,
                containers: [
                    { id: 0, piece: 0, x: 0, z: 0 },
                    { id: 1, piece: 0, x: 0, z: 0 },
                    { id: 2, piece: 0, x: 0, z: 0 },
                ],
            }),
            loot: async (_, id) => {
                if (id === 0) throw new Error('Unavailable')
                return {
                    supported: id !== 1,
                    truncated: true,
                    unresolvedTables: ['missing'],
                    items: [],
                    limits: [],
                }
            },
        },
    )
    assert.equal(result.failed, 1)
    assert.equal(result.unsupported, 1)
    assert.equal(result.truncated, true)
    assert.equal(result.unresolved, true)
})

test('canceling a loot search between detail and loot prevents chest work', async () => {
    const controller = new AbortController()
    let calls = 0
    await assert.rejects(
        searchAreaLoot(context, area, 0x3ffffff, () => true, controller.signal, progress, {
            features: async () => [{ kind: 'outpost', x: 0, z: 0, approximate: false }],
            details: async () => {
                controller.abort()
                return details
            },
            loot: async () => {
                calls++
                return { supported: true, truncated: false, limits: [] }
            },
        }),
        { name: 'AbortError' },
    )
    assert.equal(calls, 0)
})

test('mineral and loot limits independently allow exactly 16536 and 262144 selected chunks', async () => {
    assert.equal(SEED_MAP_AREA_ORE_MAX_CHUNKS, 16536)
    assert.equal(SEED_MAP_AREA_LOOT_MAX_CHUNKS, 262144)
    const oreArea = { minX: 0, minZ: 0, maxX: 16536 * 16 - 1, maxZ: 15 }
    const lootArea = { minX: 0, minZ: 0, maxX: 262144 * 16 - 1, maxZ: 15 }
    const signal = new AbortController().signal
    const ore = await analyzeAreaOres(context, oreArea, [], 0, 255, signal, progress)
    assert.deepEqual(ore.counts, [])
    await assert.rejects(
        analyzeAreaOres(
            context,
            { ...oreArea, maxX: oreArea.maxX + 1 },
            [],
            0,
            255,
            signal,
            progress,
        ),
    )
    assert.equal(
        (await searchAreaLoot(context, lootArea, 0, () => true, signal, progress)).containers,
        0,
    )
    await assert.rejects(
        searchAreaLoot(
            context,
            { ...lootArea, maxX: lootArea.maxX + 1 },
            0,
            () => true,
            signal,
            progress,
        ),
    )
})

test('streamed mineral tiles never double count candidates crossing a tile seam', async () => {
    const result = await analyzeAreaOres(
        context,
        { minX: 0, minZ: 0, maxX: 143, maxZ: 15 },
        ['iron', 'iron_vein'],
        0,
        20,
        new AbortController().signal,
        progress,
        async (request) => [
            {
                cx: 0,
                cz: 0,
                truncated: false,
                hits: [127, 128].map((x) => ({
                    ore: request.ores[0],
                    x,
                    y: 5,
                    z: 0,
                    yMin: 5,
                    yMax: 5,
                    status: 'terrain_unverified',
                })),
            },
        ],
    )
    assert.deepEqual(result.counts, [{ ore: 'iron', count: 2 }])
})

test('loot result capacity reached exactly at a structure boundary is marked incomplete', async () => {
    const result = await searchAreaLoot(
        context,
        area,
        0x3ffffff,
        () => true,
        new AbortController().signal,
        progress,
        {
            features: async () => [
                { kind: 'outpost', x: 0, z: 0, approximate: false },
                { kind: 'outpost', x: 1, z: 0, approximate: false },
            ],
            details: async () => ({
                ...details,
                containers: Array.from({ length: 2000 }, (_, id) => ({ id, piece: 0, x: 0, z: 0 })),
            }),
            loot: async () => ({ supported: true, truncated: false, items: [item(1)], limits: [] }),
        },
    )
    assert.equal(result.matches.length, 2000)
    assert.equal(result.truncated, true)
})

test('approximate container positions retain their limitation in region search matches', async () => {
    const result = await searchAreaLoot(
        context,
        area,
        0x3ffffff,
        () => true,
        new AbortController().signal,
        progress,
        {
            features: async () => [{ kind: 'ruined-portal', x: 0, z: 0, approximate: false }],
            details: async () => ({ ...details, limits: ['container_positions_approximate'] }),
            loot: async () => ({ supported: true, truncated: false, items: [item(1)], limits: [] }),
        },
    )
    assert.equal(result.matches[0].positionEstimated, true)
})

test('loot discovery resets progress before details and advances while reading chests', async () => {
    let latest = { done: 0, total: 0, phase: '' as string | undefined }
    const signal = new AbortController().signal
    await searchAreaLoot(
        context,
        area,
        0x3ffffff,
        () => true,
        signal,
        (value) => {
            latest = { ...value, phase: value.phase }
        },
        {
            features: async () => [{ kind: 'outpost', x: 0, z: 0, approximate: false }],
            details: async () => {
                assert.deepEqual(latest, { done: 0, total: 1, phase: 'analysis' })
                return {
                    ...details,
                    containers: [
                        { id: 0, piece: 0, x: 0, z: 0 },
                        { id: 1, piece: 0, x: 0, z: 0 },
                    ],
                }
            },
            loot: async (_, id) => {
                if (id === 1) assert.equal(latest.done, 0.5)
                return { supported: true, truncated: false, items: [], limits: [] }
            },
        },
    )
    assert.equal(latest.done, 1)
})

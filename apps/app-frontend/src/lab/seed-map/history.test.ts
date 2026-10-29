import { expect, it } from 'vitest'

import {
    clearSeedMapHistory,
    loadSeedMapHistory,
    recordSeedMapHistory,
    removeSeedMapHistoryEntry,
    sanitizeSeedMapHistory,
    SEED_MAP_HISTORY_LIMIT,
    seedMapHistoryId,
    updateSeedMapHistoryProgress,
} from './history.ts'

function memoryStorage(): Pick<Storage, 'getItem' | 'setItem'> {
    const values = new Map<string, string>()
    return {
        getItem: (key) => values.get(key) ?? null,
        setItem: (key, value) => void values.set(key, value),
    }
}

it('recording keeps newest entries first and dedupes revisits', () => {
    const storage = memoryStorage()
    recordSeedMapHistory(
        { seed: '111', edition: 'java', gameVersion: '26.2', source: 'manual' },
        1_000,
        storage,
    )
    recordSeedMapHistory(
        { seed: '222', edition: 'java', gameVersion: '26.2', source: 'random' },
        2_000,
        storage,
    )
    const entries = recordSeedMapHistory(
        { seed: '111', edition: 'java', gameVersion: '1.21.3', source: 'manual' },
        3_000,
        storage,
    )
    expect(entries.length).toBe(2)
    expect(entries[0].seed).toBe('111')
    expect(entries[0].gameVersion).toBe('1.21.3')
    expect(entries[0].firstViewedAt).toBe(1_000)
    expect(entries[0].lastViewedAt).toBe(3_000)
    expect(entries[1].seed).toBe('222')
})

it('instance attribution survives a later manual revisit', () => {
    const storage = memoryStorage()
    recordSeedMapHistory(
        {
            seed: '10292992',
            edition: 'java',
            gameVersion: '26.2',
            source: 'instance',
            instanceName: 'Fabric 26.2',
            worldName: '生存世界',
        },
        1_000,
        storage,
    )
    const entries = recordSeedMapHistory(
        { seed: '10292992', edition: 'java', gameVersion: '26.2', source: 'manual' },
        2_000,
        storage,
    )
    expect(entries[0].source).toBe('instance')
    expect(entries[0].instanceName).toBe('Fabric 26.2')
    expect(entries[0].worldName).toBe('生存世界')
    expect(entries[0].lastViewedAt).toBe(2_000)
})

it('the same seed in different editions stays as separate entries', () => {
    const storage = memoryStorage()
    recordSeedMapHistory(
        { seed: '7', edition: 'java', gameVersion: '26.2', source: 'manual' },
        1_000,
        storage,
    )
    const entries = recordSeedMapHistory(
        { seed: '7', edition: 'java-large-biomes', gameVersion: '26.2', source: 'manual' },
        2_000,
        storage,
    )
    expect(entries.length).toBe(2)
    expect(entries[0].id).not.toBe(entries[1].id)
    expect(seedMapHistoryId('7', 'java')).toBe('java:7')
})

it('history is capped and blank seeds are ignored', () => {
    const storage = memoryStorage()
    for (let index = 0; index < SEED_MAP_HISTORY_LIMIT + 10; index++) {
        recordSeedMapHistory(
            { seed: `seed-${index}`, edition: 'java', gameVersion: '26.2', source: 'manual' },
            index,
            storage,
        )
    }
    recordSeedMapHistory(
        { seed: '   ', edition: 'java', gameVersion: '26.2', source: 'manual' },
        99_999,
        storage,
    )
    const entries = loadSeedMapHistory(storage)
    expect(entries.length).toBe(SEED_MAP_HISTORY_LIMIT)
    expect(entries[0].seed).toBe(`seed-${SEED_MAP_HISTORY_LIMIT + 9}`)
})

it('remove and clear update the persisted list', () => {
    const storage = memoryStorage()
    recordSeedMapHistory(
        { seed: 'a', edition: 'java', gameVersion: '26.2', source: 'manual' },
        1_000,
        storage,
    )
    recordSeedMapHistory(
        { seed: 'b', edition: 'java', gameVersion: '26.2', source: 'manual' },
        2_000,
        storage,
    )
    const afterRemove = removeSeedMapHistoryEntry(seedMapHistoryId('a', 'java'), storage)
    expect(afterRemove.map((entry) => entry.seed)).toEqual(['b'])
    expect(clearSeedMapHistory(storage)).toEqual([])
    expect(loadSeedMapHistory(storage)).toEqual([])
})

it('exploration progress is stored per seed and survives revisits', () => {
    const storage = memoryStorage()
    recordSeedMapHistory(
        { seed: 'base', edition: 'java', gameVersion: '26.2', source: 'manual' },
        1_000,
        storage,
    )
    const id = seedMapHistoryId('base', 'java')
    updateSeedMapHistoryProgress(id, ['village:0:0'], ['diamond:1:2:3'], storage)
    let entries = loadSeedMapHistory(storage)
    expect(entries[0].completedFeatures).toEqual(['village:0:0'])
    expect(entries[0].completedOres).toEqual(['diamond:1:2:3'])

    entries = recordSeedMapHistory(
        { seed: 'base', edition: 'java', gameVersion: '26.2', source: 'manual' },
        2_000,
        storage,
    )
    expect(entries[0].completedFeatures).toEqual(['village:0:0'])

    entries = recordSeedMapHistory(
        {
            seed: 'base',
            edition: 'java',
            gameVersion: '26.2',
            source: 'manual',
            completedFeatures: ['village:0:0', 'monument:5:5'],
            completedOres: [],
        },
        3_000,
        storage,
    )
    expect(entries[0].completedFeatures).toEqual(['village:0:0', 'monument:5:5'])
    expect(entries[0].completedOres).toEqual([])
    expect(
        updateSeedMapHistoryProgress(seedMapHistoryId('missing', 'java'), ['x'], [], storage).map(
            (entry) => entry.seed,
        ),
    ).toEqual(['base'])
})

it('sanitizing drops malformed entries and duplicate ids', () => {
    const entries = sanitizeSeedMapHistory([
        { seed: 'ok', edition: 'java', gameVersion: '26.2', source: 'share', lastViewedAt: 10 },
        { seed: 'ok', edition: 'java', gameVersion: '26.2', source: 'manual', lastViewedAt: 20 },
        { seed: '', edition: 'java' },
        { seed: 42 },
        null,
        'nonsense',
        { seed: 'weird-source', edition: 'java', gameVersion: '26.2', source: 'nope' },
    ])
    expect(entries.map((entry) => entry.seed)).toEqual(['ok', 'weird-source'])
    expect(entries[0].source).toBe('share')
    expect(entries[1].source).toBe('manual')
})

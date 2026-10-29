import { expect, it } from 'vitest'

import {
    clearRecentSchematics,
    loadRecentSchematics,
    recordRecentSchematic,
    removeRecentSchematic,
} from './storage.ts'

function createStorage() {
    const data = new Map<string, string>()
    return {
        getItem(key: string) {
            return data.get(key) ?? null
        },
        setItem(key: string, value: string) {
            data.set(key, value)
        },
        removeItem(key: string) {
            data.delete(key)
        },
    }
}

it('recent schematics deduplicate sources and retain at most five entries', () => {
    Object.defineProperty(globalThis, 'localStorage', {
        value: createStorage(),
        configurable: true,
    })
    for (let index = 0; index < 12; index += 1) {
        recordRecentSchematic(
            { kind: 'external', path: `/tmp/${index}.litematic` },
            `${index}.litematic`,
        )
    }
    recordRecentSchematic({ kind: 'external', path: '/tmp/5.litematic' }, 'renamed.litematic')
    const records = loadRecentSchematics()
    expect(records.length).toBe(5)
    expect(records[0].fileName).toBe('renamed.litematic')
    expect(
        records.filter(
            (record) =>
                record.source.kind === 'external' && record.source.path === '/tmp/5.litematic',
        ).length,
    ).toBe(1)
})

it('recent schematics can be removed individually or cleared', () => {
    Object.defineProperty(globalThis, 'localStorage', {
        value: createStorage(),
        configurable: true,
    })
    const records = recordRecentSchematic(
        { kind: 'instance', instanceId: 'demo', relativePath: 'house.schem' },
        'house.schem',
    )
    expect(removeRecentSchematic(records[0].id).length).toBe(0)
    recordRecentSchematic({ kind: 'external', path: '/tmp/house.schem' }, 'house.schem')
    expect(clearRecentSchematics()).toEqual([])
    expect(loadRecentSchematics()).toEqual([])
})

it('recent schematics retain instance-root file sources', () => {
    Object.defineProperty(globalThis, 'localStorage', {
        value: createStorage(),
        configurable: true,
    })
    const source = {
        kind: 'instance_file' as const,
        instanceId: 'demo',
        relativePath: 'config/worldedit/schematics/house.schem',
    }
    recordRecentSchematic(source, 'house.schem')
    expect(loadRecentSchematics()[0].source).toEqual(source)
    recordRecentSchematic(
        { kind: 'instance', instanceId: 'demo', relativePath: source.relativePath },
        'house.schem',
    )
    expect(loadRecentSchematics().length).toBe(2)
})

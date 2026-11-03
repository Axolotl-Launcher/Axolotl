import { expect, it } from 'vitest'

import { getBrowseFilterMemory, setBrowseFilterMemory } from './browse-filter-memory.ts'

const storageKey = 'axolotl-browse-filter-memory-v1'
const originalStorageDescriptor = Object.getOwnPropertyDescriptor(globalThis, 'localStorage')

function installMemoryStorage() {
    const values = new Map<string, string>()
    Object.defineProperty(globalThis, 'localStorage', {
        configurable: true,
        value: {
            getItem: (key: string) => values.get(key) ?? null,
            setItem: (key: string, value: string) => values.set(key, value),
        },
    })
    return values
}

function restoreStorage() {
    if (originalStorageDescriptor) {
        Object.defineProperty(globalThis, 'localStorage', originalStorageDescriptor)
    } else {
        delete (globalThis as { localStorage?: Storage }).localStorage
    }
}

it('browse filters are remembered independently for each project type', () => {
    installMemoryStorage()

    try {
        setBrowseFilterMemory('mod', {
            filters: [{ type: 'game_version', option: '1.21.1' }],
            toggledGroups: ['all_versions'],
            overriddenProvidedFilterTypes: ['game_version'],
        })
        setBrowseFilterMemory('modpack', {
            filters: [{ type: 'modpack_loader', option: 'neoforge' }],
            toggledGroups: [],
            overriddenProvidedFilterTypes: [],
        })

        expect(getBrowseFilterMemory('mod')).toEqual({
            filters: [{ type: 'game_version', option: '1.21.1' }],
            toggledGroups: ['all_versions'],
            overriddenProvidedFilterTypes: ['game_version'],
        })
        expect(getBrowseFilterMemory('modpack')).toEqual({
            filters: [{ type: 'modpack_loader', option: 'neoforge' }],
            toggledGroups: [],
            overriddenProvidedFilterTypes: [],
        })
        expect(getBrowseFilterMemory('server')).toBe(null)
    } finally {
        restoreStorage()
    }
})

it('invalid browse filter memory is ignored', () => {
    const values = installMemoryStorage()

    try {
        values.set(storageKey, '{invalid json')
        expect(getBrowseFilterMemory('mod')).toBe(null)

        values.set(
            storageKey,
            JSON.stringify({
                mod: {
                    filters: [{ type: 'game_version', option: 121 }],
                    toggledGroups: [],
                    overriddenProvidedFilterTypes: [],
                },
            }),
        )
        expect(getBrowseFilterMemory('mod')).toBe(null)
    } finally {
        restoreStorage()
    }
})

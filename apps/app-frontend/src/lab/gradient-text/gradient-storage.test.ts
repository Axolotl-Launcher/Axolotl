import { expect, it } from 'vitest'

import {
    createDefaultGradientTextState,
    GRADIENT_TEXT_STORAGE_KEY,
    loadGradientTextState,
    parseGradientPresets,
    saveGradientTextState,
    serializeGradientPresets,
} from './gradient-storage.ts'

function createStorage(initial: Record<string, string> = {}) {
    const data = new Map(Object.entries(initial))
    return {
        getItem(key: string) {
            return data.get(key) ?? null
        },
        setItem(key: string, value: string) {
            data.set(key, value)
        },
    }
}

it('recovers from corrupt local storage', () => {
    const state = loadGradientTextState(createStorage({ [GRADIENT_TEXT_STORAGE_KEY]: '{bad json' }))
    expect(state.colors).toEqual(['#A855F7', '#22C55E'])
    expect(state.adapterId).toBe('vanilla')
    expect(state.vanillaCharacter).toBe('§')
})

it('defaults vanilla output to the section sign without overwriting a saved ampersand', () => {
    expect(createDefaultGradientTextState().vanillaCharacter).toBe('§')

    const storage = createStorage({
        [GRADIENT_TEXT_STORAGE_KEY]: JSON.stringify({
            ...createDefaultGradientTextState(),
            vanillaCharacter: '&',
        }),
    })
    expect(loadGradientTextState(storage).vanillaCharacter).toBe('&')
})

it('persists only valid normalized local state', () => {
    const storage = createStorage()
    const state = createDefaultGradientTextState()
    state.colors = ['#abc']
    state.adapterId = 'minimessage'
    saveGradientTextState(state, storage)
    expect(loadGradientTextState(storage).colors).toEqual(['#AABBCC'])
    expect(loadGradientTextState(storage).adapterId).toBe('minimessage')
})

it('rejects invalid imported presets and round-trips valid presets', () => {
    const presets = parseGradientPresets([
        { name: 'Sunset', colors: ['#FF0000', '#00FF00'] },
        { name: 'Solid', colors: ['#112233'] },
        { name: 'Broken', colors: ['not-a-color'] },
    ])
    expect(presets.length).toBe(2)
    expect(JSON.parse(serializeGradientPresets(presets))).toEqual([
        { name: 'Sunset', colors: ['#FF0000', '#00FF00'] },
        { name: 'Solid', colors: ['#112233'] },
    ])
})

import { expect, it } from 'vitest'

import { loadSkinArmorPreview, saveSkinArmorPreview } from './skin-armor-preview.ts'

const originalStorage = Object.getOwnPropertyDescriptor(globalThis, 'localStorage')

function withStorage(run: (values: Map<string, string>) => void): void {
    const values = new Map<string, string>()
    Object.defineProperty(globalThis, 'localStorage', {
        configurable: true,
        value: {
            getItem: (key: string) => values.get(key) ?? null,
            setItem: (key: string, value: string) => values.set(key, value),
        },
    })
    try {
        run(values)
    } finally {
        if (originalStorage) Object.defineProperty(globalThis, 'localStorage', originalStorage)
        else delete (globalThis as { localStorage?: Storage }).localStorage
    }
}

it('armor preview selections survive storage round trips', () => {
    withStorage(() => {
        const config = loadSkinArmorPreview()
        config.helmet.material = 'diamond'
        config.helmet.trimPattern = 'sentry'
        config.helmet.trimMaterial = 'gold'
        saveSkinArmorPreview(config)

        expect(loadSkinArmorPreview()).toEqual(config)
    })
})

it('invalid stored armor values fall back per slot and field', () => {
    withStorage((values) => {
        values.set(
            'axolotl:skin-armor-preview',
            JSON.stringify({
                helmet: { material: 'diamond', trimPattern: 'invalid', trimMaterial: 'gold' },
                boots: { material: 'turtle', trimPattern: 'sentry', trimMaterial: 'invalid' },
            }),
        )

        const config = loadSkinArmorPreview()
        expect(config.helmet).toEqual({
            material: 'diamond',
            trimPattern: null,
            trimMaterial: 'gold',
        })
        expect(config.boots).toEqual({
            material: null,
            trimPattern: 'sentry',
            trimMaterial: 'iron',
        })
        expect(config.chestplate.material).toBe(null)
    })
})

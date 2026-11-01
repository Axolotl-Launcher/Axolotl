import { describe, expect, it } from 'vitest'
import type { Component } from 'vue'

import { collectSlotContributions } from './slots'
import type { LabPluginRegistration } from './validation'

const component = { name: 'Fake' } as unknown as Component

function registration(
    id: string,
    contributions: Array<{ slot: 'lab.hub.footer' | 'instance.actions'; order?: number }>,
): LabPluginRegistration {
    return {
        folder: id,
        plugin: {
            manifest: {
                id,
                name: id,
                description: id,
                version: '1.0.0',
            },
            slots: contributions.map((entry) => ({ ...entry, component })),
        },
    }
}

describe('collectSlotContributions', () => {
    it('only returns contributions registered for the requested slot', () => {
        const contributions = collectSlotContributions(
            [
                registration('a', [{ slot: 'instance.actions' }]),
                registration('b', [{ slot: 'lab.hub.footer' }]),
            ],
            'instance.actions',
            () => true,
        )
        expect(contributions.map((entry) => entry.plugin.manifest.id)).toEqual(['a'])
    })

    it('skips plugins that are turned off', () => {
        const contributions = collectSlotContributions(
            [
                registration('a', [{ slot: 'instance.actions' }]),
                registration('b', [{ slot: 'instance.actions' }]),
            ],
            'instance.actions',
            (id) => id === 'a',
        )
        expect(contributions.map((entry) => entry.plugin.manifest.id)).toEqual(['a'])
    })

    it('sorts by order and then by plugin id', () => {
        const contributions = collectSlotContributions(
            [
                registration('zeta', [{ slot: 'lab.hub.footer' }]),
                registration('alpha', [{ slot: 'lab.hub.footer' }]),
                registration('first', [{ slot: 'lab.hub.footer', order: -10 }]),
            ],
            'lab.hub.footer',
            () => true,
        )
        expect(contributions.map((entry) => entry.plugin.manifest.id)).toEqual([
            'first',
            'alpha',
            'zeta',
        ])
    })

    it('keeps every contribution when one plugin fills a slot twice', () => {
        const contributions = collectSlotContributions(
            [
                registration('a', [
                    { slot: 'lab.hub.footer', order: 2 },
                    { slot: 'lab.hub.footer', order: 1 },
                ]),
            ],
            'lab.hub.footer',
            () => true,
        )
        expect(contributions).toHaveLength(2)
    })
})

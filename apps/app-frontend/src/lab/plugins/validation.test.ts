import { describe, expect, it } from 'vitest'
import type { Component } from 'vue'

import type { LabPlugin } from './types'
import type { LabPluginEntry } from './validation'
import { collectLabPlugins, validateLabPlugin } from './validation'

const component = { name: 'FakePluginComponent' } as unknown as Component

function makePlugin(overrides: Partial<LabPlugin['manifest']> = {}): LabPlugin {
    return {
        manifest: {
            id: 'demo-plugin',
            name: 'Demo plugin',
            description: 'Does demo things',
            version: '1.0.0',
            ...overrides,
        },
        tool: {
            title: 'Demo',
            description: 'Demo tool',
            component,
        },
    }
}

function makeEntry(folder: string, plugin: unknown): LabPluginEntry {
    return { folder, module: { default: plugin } }
}

describe('validateLabPlugin', () => {
    it('accepts a plugin whose folder matches its manifest id', () => {
        const result = validateLabPlugin(makeEntry('demo-plugin', makePlugin()))
        expect('plugin' in result).toBe(true)
    })

    it('rejects a module without a default export', () => {
        const result = validateLabPlugin({ folder: 'demo-plugin', module: {} })
        expect(result).toEqual({ reason: 'missing default export' })
    })

    it('rejects a manifest id that differs from the folder', () => {
        const result = validateLabPlugin(makeEntry('other-folder', makePlugin()))
        expect('reason' in result).toBe(true)
    })

    it('rejects a manifest id that is not kebab-case', () => {
        const result = validateLabPlugin(makeEntry('DemoPlugin', makePlugin({ id: 'DemoPlugin' })))
        expect(result).toEqual({ reason: 'manifest.id "DemoPlugin" is not kebab-case' })
    })

    it('rejects a version that is not semantic', () => {
        const result = validateLabPlugin(makeEntry('demo-plugin', makePlugin({ version: 'v1' })))
        expect('reason' in result).toBe(true)
    })

    it('rejects a plugin that renders nothing', () => {
        const plugin = makePlugin()
        delete plugin.tool
        const result = validateLabPlugin(makeEntry('demo-plugin', plugin))
        expect(result).toEqual({
            reason: 'renders nothing: provide a tool page or a slot contribution',
        })
    })

    it('accepts a slot-only plugin', () => {
        const plugin: LabPlugin = {
            manifest: {
                id: 'slot-only',
                name: 'Slot only',
                description: 'Adds a button',
                version: '0.1.0',
            },
            slots: [{ slot: 'instance.actions', component }],
        }
        const result = validateLabPlugin(makeEntry('slot-only', plugin))
        expect('plugin' in result).toBe(true)
    })

    it('rejects an unknown slot name', () => {
        const plugin = {
            manifest: {
                id: 'bad-slot',
                name: 'Bad slot',
                description: 'Points nowhere',
                version: '1.0.0',
            },
            slots: [{ slot: 'somewhere.else', component }],
        }
        const result = validateLabPlugin(makeEntry('bad-slot', plugin))
        expect(result).toEqual({ reason: 'invalid slot contribution' })
    })
})

describe('collectLabPlugins', () => {
    it('keeps the first plugin when two entries claim one id', () => {
        const { plugins, rejections } = collectLabPlugins([
            makeEntry('demo-plugin', makePlugin()),
            makeEntry('demo-plugin', makePlugin()),
        ])
        expect(plugins).toHaveLength(1)
        expect(rejections).toEqual([
            { folder: 'demo-plugin', reason: 'duplicate plugin id "demo-plugin"' },
        ])
    })

    it('sorts plugins by folder so the order is stable', () => {
        const { plugins } = collectLabPlugins([
            makeEntry('zeta', makePlugin({ id: 'zeta' })),
            makeEntry('alpha', makePlugin({ id: 'alpha' })),
        ])
        expect(plugins.map((entry) => entry.plugin.manifest.id)).toEqual(['alpha', 'zeta'])
    })
})

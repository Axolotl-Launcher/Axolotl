import { describe, expect, it } from 'vitest'
import type { Component } from 'vue'

import { collectLabPluginToolEntries, labPluginToolRoute } from './plugin-tools'
import type { LabPluginRegistration } from './validation'

const component = { name: 'Fake' } as unknown as Component

function registration(id: string, withTool: boolean): LabPluginRegistration {
    return {
        folder: id,
        plugin: {
            manifest: {
                id,
                name: id,
                description: `${id} description`,
                version: '1.0.0',
            },
            ...(withTool
                ? {
                      tool: {
                          title: `${id} title`,
                          description: `${id} tool`,
                          component,
                      },
                  }
                : {}),
        },
    }
}

describe('labPluginToolRoute', () => {
    it('builds the tool URL from the plugin id', () => {
        expect(labPluginToolRoute('motd-preview')).toBe('/lab/p/motd-preview')
    })
})

describe('collectLabPluginToolEntries', () => {
    it('skips plugins that are turned off', () => {
        const entries = collectLabPluginToolEntries(
            [registration('a', true), registration('b', true)],
            (id) => id === 'b',
        )
        expect(entries.map((entry) => entry.id)).toEqual(['b'])
    })

    it('skips plugins that only fill slots', () => {
        const entries = collectLabPluginToolEntries([registration('slot-only', false)], () => true)
        expect(entries).toEqual([])
    })

    it('defaults the category to creation', () => {
        const entries = collectLabPluginToolEntries([registration('a', true)], () => true)
        expect(entries[0].category).toBe('creation')
    })

    it('marks the entry as a plugin and keeps the route', () => {
        const entries = collectLabPluginToolEntries([registration('a', true)], () => true)
        expect(entries[0]).toMatchObject({ plugin: true, route: '/lab/p/a' })
    })
})

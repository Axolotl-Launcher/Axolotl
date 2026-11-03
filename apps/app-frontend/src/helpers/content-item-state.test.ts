import type { ContentItem } from '@modrinth/ui'
import { expect, it } from 'vitest'

import { applyContentItemUpdates } from './content-item-state.ts'

function contentItem(path: string): ContentItem {
    const fileName = path.split('/').pop() ?? path
    return {
        id: 'content',
        file_name: fileName,
        file_path: path,
        size: 1,
        enabled: true,
        project_type: 'mod',
        project: {
            id: 'local:content',
            slug: 'content',
            title: 'Content',
            icon_url: 'C:/icons/content.png',
        },
        version: {
            id: 'local:content',
            version_number: '1.0.0',
            file_name: fileName,
        },
        update: null,
        provider_refs: [],
    } as ContentItem
}

it('toggle updates survive recomputing an icon-bearing display clone', () => {
    const source = contentItem('mods/content.jar')
    const rendered = {
        ...source,
        project: { ...source.project!, icon_url: 'asset://localhost/icons/content.png' },
    }

    applyContentItemUpdates([source], rendered, source.file_name, source.file_path, {
        file_name: 'content.jar.disabled',
        file_path: 'mods/content.jar.disabled',
        enabled: false,
    })

    expect(source.enabled).toBe(false)
    expect(source.file_name).toBe('content.jar.disabled')
    expect(source.file_path).toBe('mods/content.jar.disabled')
    expect(rendered.enabled).toBe(false)
    expect(rendered.file_path).toBe('mods/content.jar.disabled')
})

it('operation completion clears busy state on backing rows and a replacement row with the same entry', () => {
    const source = contentItem('mods/content.jar')
    source.instanceEntryId = 'entry'
    const rendered = { ...source }
    applyContentItemUpdates([source], rendered, source.file_name, source.file_path, {
        installing: true,
    })
    expect(source.installing).toBe(true)
    const refreshed = { ...source, file_path: 'mods/restored.jar' }
    const other = { ...source, instanceEntryId: 'other' }
    applyContentItemUpdates([refreshed, other], rendered, source.file_name, source.file_path, {
        installing: false,
    })
    expect(refreshed.installing).toBe(false)
    expect(rendered.installing).toBe(false)
    expect(other.installing).toBe(true)
})

it('single-entry toggles never mutate another copy with the same project, version, hash or filename', () => {
    for (const stableIds of [false, true]) {
        const source = contentItem('mods/one/content.jar')
        const other = contentItem('mods/two/content.jar')
        if (stableIds) {
            source.instanceEntryId = 'first'
            other.instanceEntryId = 'second'
        }
        const selected = { ...source }
        applyContentItemUpdates([source, other], selected, source.file_name, source.file_path, {
            enabled: false,
            file_name: 'content.jar.disabled',
            file_path: 'mods/one/content.jar.disabled',
        })
        expect(source.enabled).toBe(false)
        expect(other.enabled).toBe(true)
        expect(other.file_path).toBe('mods/two/content.jar')
        expect(other.file_name).toBe('content.jar')
    }
})

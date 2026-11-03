import type { ContentItem } from '@modrinth/ui'
import { expect, it, vi } from 'vitest'

import { isUnresolvedContentProject } from '@/helpers/content-identity'
import { drainContentMetadata } from '@/helpers/content-metadata-refresh'

import modsSource from './Mods.vue?raw'

const mkUnresolved = (path: string): ContentItem =>
    ({
        id: path,
        file_name: path.split('/').pop() ?? path,
        file_path: path,
        project_type: 'mod',
        origin_provider: null,
        provider_refs: [],
        project: { id: `local:${path}`, slug: path, title: path, icon_url: undefined },
    }) as ContentItem

const mkResolved = (path: string): ContentItem => ({
    ...mkUnresolved(path),
    project: { id: 'modrinth', slug: `slug-${path}`, title: `Title ${path}`, icon_url: undefined },
})

/**
 * Mounting `Mods.vue` in this suite is not affordable: the page reaches for
 * three UI providers, the app-backup and content-manager contexts, a query
 * client, a router, the theme store and roughly a dozen child components, so a
 * mount would assert on the mock rig rather than on the page. These cases pin
 * the page's refresh boundary instead: metadata arrives through the backend
 * subscription, without a second frontend drain or viewport-driven requests.
 * The standalone drain helper retains its own batching coverage below.
 */
it('does not subscribe the content layout to visible items any more', () => {
    expect(modsSource).not.toContain('@visible-items')
    expect(modsSource).not.toContain('handleVisibleItems')
    expect(modsSource).not.toContain('visibleMetadataPendingPaths')
    expect(modsSource).not.toContain('refreshAllContentMetadata')
    expect(modsSource).not.toContain('get_content_items_by_paths')
    expect(modsSource).toContain('createContentRefreshSubscription(')
})

it('requests every unresolved path once a snapshot lands, in list order', async () => {
    const paths = Array.from({ length: 150 }, (_, index) => `mods/${index}.jar`)
    const items = [
        ...paths.map(mkUnresolved),
        ...Array.from({ length: 20 }, (_, i) => mkResolved(`mods/r${i}.jar`)),
    ]
    const requested: string[][] = []

    const fetchChunk = vi.fn(async (chunk: string[]) => {
        requested.push(chunk)
        return chunk.map((path) => mkResolved(path))
    })
    const translate = vi.fn(async (value: ContentItem[]) => value)
    const merge = vi.fn()

    const targets = items.filter(isUnresolvedContentProject).map((item) => item.file_path!)
    await drainContentMetadata({
        paths: targets,
        fetchChunk,
        translate,
        originalsForChunk: () => [],
        merge,
        isCurrent: () => true,
    })

    expect(targets).toEqual(paths)
    expect(fetchChunk).toHaveBeenCalledTimes(Math.ceil(paths.length / 64))
    expect(requested.flat()).toEqual(paths)
    expect(requested[0]).toHaveLength(64)
    expect(requested.at(-1)).toHaveLength(150 - 128)
})

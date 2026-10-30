import type { ContentItem } from '@modrinth/ui'
import { expect, it, vi } from 'vitest'

import { isUnresolvedContentProject } from './content-identity'
import { drainContentMetadata } from './content-metadata-refresh'

const mkItem = (overrides: Partial<ContentItem> & { file_path: string }): ContentItem =>
    ({
        id: overrides.file_path,
        file_name: overrides.file_path.split('/').pop() ?? overrides.file_path,
        project_type: 'mod',
        origin_provider: 'modrinth',
        provider_refs: [],
        ...overrides,
    }) as ContentItem

const unresolved = (index: number) =>
    mkItem({
        file_path: `mods/${index}.jar`,
        project: {
            id: `local:mods/${index}.jar`,
            slug: `mods/${index}.jar`,
            title: `${index}.jar`,
            icon_url: undefined,
        },
    })

const resolved = (index: number) =>
    mkItem({
        file_path: `mods/${index}.jar`,
        project: {
            id: 'modrinth',
            slug: `project-${index}`,
            title: `Project ${index}`,
            icon_url: undefined,
        },
    })

it('treats the local placeholder project id as unresolved and a provider id as resolved', () => {
    expect(isUnresolvedContentProject(unresolved(1))).toBe(true)
    expect(isUnresolvedContentProject(resolved(1))).toBe(false)
    expect(
        isUnresolvedContentProject(
            mkItem({
                file_path: 'mods/curseforge.jar',
                project: {
                    id: 'curseforge:12345',
                    slug: 'curseforge-project',
                    title: 'CurseForge Project',
                    icon_url: undefined,
                },
            }),
        ),
    ).toBe(false)
})

it('drains 100 paths in two chunks of 64 and 36, translating and merging each one', async () => {
    const paths = Array.from({ length: 100 }, (_, index) => `mods/${index}.jar`)
    const fetchChunk = vi.fn(async (chunk: string[]) => chunk.map(() => unresolved(0)))
    const translate = vi.fn(async (items: ContentItem[]) => items)
    const merge = vi.fn()

    // No `chunkSize`: the default has to be what bounds the request.
    await drainContentMetadata({
        paths,
        fetchChunk,
        translate,
        originalsForChunk: () => [],
        merge,
        isCurrent: () => true,
    })

    expect(fetchChunk).toHaveBeenCalledTimes(2)
    expect(fetchChunk.mock.calls[0][0]).toHaveLength(64)
    expect(fetchChunk.mock.calls[1][0]).toHaveLength(36)
    expect(fetchChunk.mock.calls[0][0][0]).toBe('mods/0.jar')
    expect(fetchChunk.mock.calls[1][0][0]).toBe('mods/64.jar')
    expect(translate).toHaveBeenCalledTimes(2)
    expect(merge).toHaveBeenCalledTimes(2)
})

it('stops after the chunk in flight when the request is superseded', async () => {
    const paths = Array.from({ length: 100 }, (_, index) => `mods/${index}.jar`)
    let current = true
    const fetchChunk = vi.fn(async (chunk: string[]) => {
        // The instance request is superseded while chunk one is in flight, so
        // the drain must stop rather than open chunk two.
        current = false
        return chunk.map(() => unresolved(0))
    })
    const translate = vi.fn(async (items: ContentItem[]) => items)
    const merge = vi.fn()

    await drainContentMetadata({
        paths,
        chunkSize: 64,
        fetchChunk,
        translate,
        originalsForChunk: () => [],
        merge,
        isCurrent: () => current,
    })

    expect(fetchChunk).toHaveBeenCalledOnce()
    expect(translate).not.toHaveBeenCalled()
    expect(merge).not.toHaveBeenCalled()
})

it('finishes the chunk it already started before the request is superseded', async () => {
    const paths = Array.from({ length: 100 }, (_, index) => `mods/${index}.jar`)
    let current = true
    const fetchChunk = vi.fn(async (chunk: string[]) => chunk.map(() => unresolved(0)))
    const translate = vi.fn(async (items: ContentItem[]) => items)
    const merge = vi.fn()

    await drainContentMetadata({
        paths,
        chunkSize: 64,
        fetchChunk,
        translate,
        originalsForChunk: () => [],
        merge,
        isCurrent: () => {
            // Superseded after chunk one was merged: chunk two must not be
            // requested even though the previous chunk completed cleanly.
            if (translate.mock.calls.length === 1 && merge.mock.calls.length === 1) current = false
            return current
        },
    })

    expect(fetchChunk).toHaveBeenCalledOnce()
    expect(translate).toHaveBeenCalledOnce()
    expect(merge).toHaveBeenCalledOnce()
})

it('requests every path of the full list in ceil(N / chunkSize) chunks', async () => {
    const paths = Array.from({ length: 100 }, (_, index) => `mods/${index}.jar`)

    const run = async (onScreen: string[]) => {
        // The drain is handed `paths` and `chunkSize` and nothing else: the
        // caller's on-screen subset never reaches it, so the two runs below ask
        // the same question and this case can only pin the request count --
        // ceil(100 / 64) == 2 requests covering all 100 paths.
        expect(onScreen.length).toBeGreaterThan(0)
        const fetchChunk = vi.fn(async (chunk: string[]) => chunk.map(() => unresolved(0)))
        const translate = vi.fn(async (items: ContentItem[]) => items)
        const merge = vi.fn()
        await drainContentMetadata({
            paths,
            chunkSize: 64,
            fetchChunk,
            translate,
            originalsForChunk: () => [],
            merge,
            isCurrent: () => true,
        })
        return fetchChunk.mock.calls.length
    }

    const scrolledToTop = paths.slice(0, 8)
    const scrolledToMiddle = paths.slice(40, 48)
    const topCalls = await run(scrolledToTop)
    const middleCalls = await run(scrolledToMiddle)

    expect(scrolledToTop).not.toEqual(scrolledToMiddle)
    expect(topCalls).toBe(Math.ceil(paths.length / 64))
    expect(middleCalls).toBe(topCalls)
    expect(topCalls).toBe(2)
})

it('hands merge the list items for the chunk, not the fetched ones', async () => {
    const paths = ['mods/a.jar', 'mods/b.jar']
    const listItems = paths.map(() => unresolved(0))
    listItems[0] = { ...listItems[0], file_path: paths[0], id: 'list-a' }
    listItems[1] = { ...listItems[1], file_path: paths[1], id: 'list-b' }
    const fetched = paths.map((path) => ({
        ...resolved(0),
        file_path: path,
        id: `fetched-${path}`,
    }))
    const seen: Array<Map<string, ContentItem>> = []

    await drainContentMetadata({
        paths,
        chunkSize: 64,
        fetchChunk: async () => fetched,
        translate: async (items) => items,
        originalsForChunk: (chunk) => listItems.filter((item) => chunk.includes(item.file_path!)),
        merge: (_items, originals) => seen.push(originals),
        isCurrent: () => true,
    })

    expect(seen).toHaveLength(1)
    expect(seen[0].get('mods/a.jar')?.id).toBe('list-a')
    expect(seen[0].get('mods/b.jar')?.id).toBe('list-b')
})

it('leaves the list untouched when there is nothing to drain', async () => {
    const fetchChunk = vi.fn(async (chunk: string[]) => chunk.map(() => unresolved(0)))
    const translate = vi.fn(async (items: ContentItem[]) => items)
    const merge = vi.fn()

    await drainContentMetadata({
        paths: [],
        fetchChunk,
        translate,
        originalsForChunk: () => [],
        merge,
        isCurrent: () => true,
    })

    expect(fetchChunk).not.toHaveBeenCalled()
    expect(translate).not.toHaveBeenCalled()
    expect(merge).not.toHaveBeenCalled()
})

import { expect, it } from 'vitest'

import type { ContentFavorite, ContentFavoriteInput } from '../helpers/content-favorites.ts'
import type { SourceProjectMetadata } from '../helpers/resources.ts'
import { createContentFavoritesStore } from './useContentFavorites.ts'

function favorite(
    provider: ContentFavorite['provider'],
    projectId: string,
    contentType: ContentFavorite['content_type'],
    savedAt: number,
): ContentFavorite {
    return {
        provider,
        project_id: projectId,
        content_type: contentType,
        saved_at: savedAt,
    }
}

it('content favorites load, add, remove, and keep provider-qualified identities', async () => {
    const records = [
        favorite('modrinth', 'same-id', 'mod', 1),
        favorite('curseforge', 'same-id', 'resourcepack', 2),
    ]
    const store = createContentFavoritesStore({
        async list() {
            return records
        },
        async add(input: ContentFavoriteInput) {
            const saved = favorite(input.provider, input.project_id, input.content_type, 3)
            records.push(saved)
            return saved
        },
        async remove(provider, projectId) {
            const index = records.findIndex(
                (record) => record.provider === provider && record.project_id === projectId,
            )
            if (index >= 0) records.splice(index, 1)
        },
    })

    await store.load()
    expect(
        store.favorites.value.map((record) => `${record.provider}:${record.project_id}`),
    ).toEqual(['curseforge:same-id', 'modrinth:same-id'])
    expect(store.isFavorite('modrinth', 'same-id')).toBe(true)
    expect(store.isFavorite('curseforge', 'same-id')).toBe(true)

    await store.add({ provider: 'modrinth', project_id: 'iris', content_type: 'shader' })
    expect(store.isFavorite('modrinth', 'iris')).toBe(true)
    await store.remove('modrinth', 'same-id')
    expect(store.isFavorite('modrinth', 'same-id')).toBe(false)
    expect(store.isFavorite('curseforge', 'same-id')).toBe(true)
})

it('refreshing a favorite replaces its updated locator even when its identity is unchanged', async () => {
    let saved = { ...favorite('modrinth', 'same-id', 'mod', 1), locator: 'old-slug' }
    const store = createContentFavoritesStore({
        list: async () => [saved],
        add: async () => saved,
        remove: async () => {},
    })
    await store.load()
    saved = { ...saved, locator: 'new-slug' }
    await store.load(true)
    expect(store.favorites.value[0].locator).toBe('new-slug')
})

it('content favorite writes roll back busy state when persistence fails', async () => {
    const store = createContentFavoritesStore({
        async list() {
            return []
        },
        async add() {
            throw new Error('database unavailable')
        },
        async remove() {
            throw new Error('database unavailable')
        },
    })

    await expect(
        store.add({ provider: 'modrinth', project_id: 'sodium', content_type: 'mod' }),
    ).rejects.toThrow()
    expect(store.isFavorite('modrinth', 'sodium')).toBe(false)
    expect(store.isPending('modrinth', 'sodium')).toBe(false)

    await store.load()
    store.favorites.value = [favorite('curseforge', 'embeddium', 'mod', 4)]
    await expect(store.remove('curseforge', 'embeddium')).rejects.toThrow()
    expect(store.isFavorite('curseforge', 'embeddium')).toBe(true)
    expect(store.isPending('curseforge', 'embeddium')).toBe(false)
})

it('a confirmed resource is bookmarked through all known sources and removed as one resource', async () => {
    const saved = {
        ...favorite('modrinth', 'abcdefgh', 'mod', 10),
        resource_id: 'resource',
        projects: [
            { source: 'modrinth' as const, projectId: 'abcdefgh' },
            { source: 'curseforge' as const, projectId: '123' },
        ],
    }
    const store = createContentFavoritesStore({
        list: async () => [saved],
        add: async () => saved,
        remove: async () => {},
    })
    await store.load()
    expect(store.isFavorite('curseforge', '123')).toBe(true)
    await store.remove('curseforge', '123')
    expect(store.isFavorite('modrinth', 'abcdefgh')).toBe(false)
    expect(store.favorites.value.length).toBe(0)
})

it('favorite metadata survives reads, isolates providers, and expires when locators change or favorites are removed', async () => {
    const original = { ...favorite('modrinth', 'same-id', 'mod', 1), locator: 'old' }
    const other = favorite('curseforge', 'same-id', 'mod', 2)
    let records = [original, other]
    const store = createContentFavoritesStore({
        list: async () => records,
        add: async () => original,
        remove: async () => {},
    })
    const metadata = { title: 'Original' } as SourceProjectMetadata
    await store.load()
    store.setMetadata(original, metadata)
    await store.load(true)
    expect(store.getMetadata(original)).toBe(metadata)
    expect(store.getMetadata(other)).toBe(undefined)
    records = [{ ...original, locator: 'new' }, other]
    await store.load(true)
    expect(store.getMetadata(records[0])).toBe(undefined)
    store.setMetadata(original, { title: 'Stale request' } as SourceProjectMetadata)
    expect(store.getMetadata(records[0])).toBe(undefined)
    store.setMetadata(records[0], metadata)
    await store.remove('modrinth', 'same-id')
    expect(store.getMetadata(records[0])).toBe(undefined)
    store.setMetadata(records[0], metadata)
    expect(store.getMetadata(records[0])).toBe(undefined)
})

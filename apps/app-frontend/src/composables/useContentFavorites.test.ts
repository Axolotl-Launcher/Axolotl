import assert from 'node:assert/strict'
import test from 'node:test'

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

test('content favorites load, add, remove, and keep provider-qualified identities', async () => {
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
    assert.deepEqual(
        store.favorites.value.map((record) => `${record.provider}:${record.project_id}`),
        ['curseforge:same-id', 'modrinth:same-id'],
    )
    assert.equal(store.isFavorite('modrinth', 'same-id'), true)
    assert.equal(store.isFavorite('curseforge', 'same-id'), true)

    await store.add({ provider: 'modrinth', project_id: 'iris', content_type: 'shader' })
    assert.equal(store.isFavorite('modrinth', 'iris'), true)
    await store.remove('modrinth', 'same-id')
    assert.equal(store.isFavorite('modrinth', 'same-id'), false)
    assert.equal(store.isFavorite('curseforge', 'same-id'), true)
})

test('refreshing a favorite replaces its updated locator even when its identity is unchanged', async () => {
    let saved = { ...favorite('modrinth', 'same-id', 'mod', 1), locator: 'old-slug' }
    const store = createContentFavoritesStore({
        list: async () => [saved],
        add: async () => saved,
        remove: async () => {},
    })
    await store.load()
    saved = { ...saved, locator: 'new-slug' }
    await store.load(true)
    assert.equal(store.favorites.value[0].locator, 'new-slug')
})

test('content favorite writes roll back busy state when persistence fails', async () => {
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

    await assert.rejects(
        store.add({ provider: 'modrinth', project_id: 'sodium', content_type: 'mod' }),
    )
    assert.equal(store.isFavorite('modrinth', 'sodium'), false)
    assert.equal(store.isPending('modrinth', 'sodium'), false)

    await store.load()
    store.favorites.value = [favorite('curseforge', 'embeddium', 'mod', 4)]
    await assert.rejects(store.remove('curseforge', 'embeddium'))
    assert.equal(store.isFavorite('curseforge', 'embeddium'), true)
    assert.equal(store.isPending('curseforge', 'embeddium'), false)
})

test('a confirmed resource is bookmarked through all known sources and removed as one resource', async () => {
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
    assert.equal(store.isFavorite('curseforge', '123'), true)
    await store.remove('curseforge', '123')
    assert.equal(store.isFavorite('modrinth', 'abcdefgh'), false)
    assert.equal(store.favorites.value.length, 0)
})

test('favorite metadata survives reads, isolates providers, and expires when locators change or favorites are removed', async () => {
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
    assert.equal(store.getMetadata(original), metadata)
    assert.equal(store.getMetadata(other), undefined)
    records = [{ ...original, locator: 'new' }, other]
    await store.load(true)
    assert.equal(store.getMetadata(records[0]), undefined)
    store.setMetadata(original, { title: 'Stale request' } as SourceProjectMetadata)
    assert.equal(store.getMetadata(records[0]), undefined)
    store.setMetadata(records[0], metadata)
    await store.remove('modrinth', 'same-id')
    assert.equal(store.getMetadata(records[0]), undefined)
    store.setMetadata(records[0], metadata)
    assert.equal(store.getMetadata(records[0]), undefined)
})

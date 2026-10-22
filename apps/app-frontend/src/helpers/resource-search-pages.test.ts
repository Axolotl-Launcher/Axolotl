import assert from 'node:assert/strict'
import test from 'node:test'

import { ResourceSearchPages } from './resource-search-pages.ts'
import type { ResourcePage, ResourceSearchQuery, ResourceSearchResult } from './resources.ts'

const query = (text = ''): ResourceSearchQuery => ({
    text,
    sources: [],
    types: ['mod'],
    target: { gameVersions: [], loaders: [], channels: [] },
    categories: [],
    sort: 'relevance',
    pageSize: 10,
})
const page = (index: number, hasMore = true): ResourcePage<ResourceSearchResult> => ({
    requestId: String(index),
    entries: [],
    cursor: hasMore ? `cursor-${index}` : null,
    hasMore,
    total: { value: null, precision: 'unknown' },
    sourceFailures: [],
    exhaustedSources: [],
})

test('deep pages use successive opaque cursors and navigating back retains the same page', async () => {
    const cursors: (string | null)[] = []
    const pager = new ResourceSearchPages(async (_query, cursor) => {
        cursors.push(cursor)
        return page(cursors.length - 1)
    })
    const signal = new AbortController().signal
    const third = await pager.get(query(), 2, signal)
    assert.deepEqual(cursors, [null, 'cursor-0', 'cursor-1'])
    await pager.get(query(), 0, signal)
    assert.equal(await pager.get(query(), 2, signal), third)
    assert.equal(cursors.length, 3)
})

test('a new query starts a new cursor chain and stale responses cannot replace it', async () => {
    let resolveOld!: (value: ResourcePage<ResourceSearchResult>) => void
    const oldResponse = new Promise<ResourcePage<ResourceSearchResult>>((resolve) => {
        resolveOld = resolve
    })
    const pager = new ResourceSearchPages(async (request) =>
        request.text === 'old' ? oldResponse : page(20, false),
    )
    const signal = new AbortController().signal
    const old = pager.get(query('old'), 0, signal)
    const latest = await pager.get(query('new'), 0, signal)
    resolveOld(page(1))
    await assert.rejects(old, { name: 'AbortError' })
    assert.equal(await pager.get(query('new'), 0, signal), latest)
})

test('failure and cancellation retain the last complete page and retry its cursor', async () => {
    let fail = true
    const cursors: (string | null)[] = []
    const pager = new ResourceSearchPages(async (_request, cursor) => {
        cursors.push(cursor)
        if (cursor && fail) throw new Error('source unavailable')
        return page(cursor ? 1 : 0)
    })
    const controller = new AbortController()
    const first = await pager.get(query(), 0, controller.signal)
    await assert.rejects(pager.get(query(), 1, controller.signal))
    assert.equal(await pager.get(query(), 0, controller.signal), first)
    fail = false
    await pager.get(query(), 1, controller.signal)
    assert.deepEqual(cursors, [null, 'cursor-0', 'cursor-0'])
    controller.abort()
    await assert.rejects(pager.get(query(), 2, controller.signal), { name: 'AbortError' })
    assert.equal(cursors.length, 3)
})

test('a failed page is requested again and an expired cursor rebuilds the query once', async () => {
    let calls = 0
    let expired = true
    const pager = new ResourceSearchPages(async (_request, cursor) => {
        calls++
        if (calls === 1)
            return {
                ...page(0),
                sourceFailures: [
                    {
                        source: 'modrinth',
                        code: 'sourceUnavailable',
                        message: 'unavailable',
                        retryable: true,
                    },
                ],
            }
        if (cursor && expired) {
            expired = false
            throw { code: 'cursorExpired' }
        }
        return page(cursor ? 1 : 0)
    })
    const signal = new AbortController().signal
    await pager.get(query(), 0, signal)
    await pager.get(query(), 0, signal)
    assert.equal(calls, 2)
    const second = await pager.get(query(), 1, signal)
    assert.equal(second.requestId, '1')
    assert.equal(calls, 5)
})

test('permanent source capability notices do not restart successful cursor pages', async () => {
    let calls = 0
    const pager = new ResourceSearchPages(async () => ({
        ...page(calls++),
        sourceFailures: [
            {
                source: 'modrinth',
                code: 'sourceUnavailable',
                message: 'release ordering unsupported',
                retryable: false,
            },
        ],
    }))
    const signal = new AbortController().signal
    const first = await pager.get(query(), 0, signal)
    assert.equal(await pager.get(query(), 0, signal), first)
    await pager.get(query(), 1, signal)
    assert.equal(calls, 2)
})

test('explicit retry invalidates permanent failures and refreshing a deep page retains earlier cursors', async () => {
    const cursors: (string | null)[] = []
    const pager = new ResourceSearchPages(async (_query, cursor) => {
        cursors.push(cursor)
        return {
            ...page(cursors.length),
            sourceFailures: [
                {
                    source: 'modrinth',
                    code: 'sourceUnavailable',
                    message: 'failed',
                    retryable: false,
                },
            ],
        }
    })
    const signal = new AbortController().signal
    const first = await pager.get(query(), 0, signal)
    pager.invalidate()
    assert.notEqual(await pager.get(query(), 0, signal), first)
    await pager.get(query(), 2, signal)
    pager.invalidate(2)
    await pager.get(query(), 2, signal)
    assert.deepEqual(cursors, [null, null, 'cursor-2', 'cursor-3', 'cursor-3'])
})

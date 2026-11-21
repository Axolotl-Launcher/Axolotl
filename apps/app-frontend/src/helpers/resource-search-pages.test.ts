import { expect, it } from 'vitest'

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
    cursor: null,
    hasMore,
    total: { value: null, precision: 'unknown' },
    sourceFailures: [],
    exhaustedSources: [],
})

it('reads a distant page directly in one request and revisits it from cache', async () => {
    const pages: number[] = []
    const pager = new ResourceSearchPages(async (_query, index) => {
        pages.push(index)
        return page(index)
    })
    const signal = new AbortController().signal
    const third = await pager.get(query(), 3, signal)
    expect(pages).toEqual([3])
    expect(pager.indexOf(third)).toBe(3)
    await pager.get(query(), 1, signal)
    expect(await pager.get(query(), 3, signal)).toBe(third)
    expect(pages).toEqual([3, 1])
})

it('a new query clears the cache and stale responses cannot replace it', async () => {
    let resolveOld!: (value: ResourcePage<ResourceSearchResult>) => void
    const oldResponse = new Promise<ResourcePage<ResourceSearchResult>>((resolve) => {
        resolveOld = resolve
    })
    const pager = new ResourceSearchPages(async (request) =>
        request.text === 'old' ? oldResponse : page(20, false),
    )
    const signal = new AbortController().signal
    const old = pager.get(query('old'), 1, signal)
    const latest = await pager.get(query('new'), 1, signal)
    resolveOld(page(1))
    await expect(old).rejects.toMatchObject({ name: 'AbortError' })
    expect(await pager.get(query('new'), 1, signal)).toBe(latest)
})

it('a failed page is retried while earlier pages stay cached', async () => {
    let fail = true
    const pages: number[] = []
    const pager = new ResourceSearchPages(async (_request, index) => {
        pages.push(index)
        if (index === 2 && fail) throw new Error('source unavailable')
        return page(index)
    })
    const signal = new AbortController().signal
    const first = await pager.get(query(), 1, signal)
    await expect(pager.get(query(), 2, signal)).rejects.toThrow()
    expect(await pager.get(query(), 1, signal)).toBe(first)
    fail = false
    await pager.get(query(), 2, signal)
    expect(pages).toEqual([1, 2, 2])
})

it('a retryable source failure is requested again on the next read', async () => {
    let calls = 0
    const pager = new ResourceSearchPages(async () => {
        calls++
        if (calls === 1)
            return {
                ...page(1),
                sourceFailures: [
                    {
                        source: 'modrinth',
                        code: 'sourceUnavailable',
                        message: 'unavailable',
                        retryable: true,
                    },
                ],
            }
        return page(1)
    })
    const signal = new AbortController().signal
    await pager.get(query(), 1, signal)
    await pager.get(query(), 1, signal)
    expect(calls).toBe(2)
})

it('permanent source capability notices do not restart a successful page', async () => {
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
    const first = await pager.get(query(), 1, signal)
    expect(await pager.get(query(), 1, signal)).toBe(first)
    expect(calls).toBe(1)
})

it('explicit retry invalidates a cached page and cancellation aborts a request', async () => {
    let calls = 0
    const pager = new ResourceSearchPages(async () => page(calls++))
    const controller = new AbortController()
    const first = await pager.get(query(), 1, controller.signal)
    pager.invalidate(1)
    expect(await pager.get(query(), 1, controller.signal)).not.toBe(first)
    controller.abort()
    await expect(pager.get(query(), 2, controller.signal)).rejects.toMatchObject({
        name: 'AbortError',
    })
})

it('invalidate keeps pages before its starting index', async () => {
    const pager = new ResourceSearchPages(async (_request, index) => page(index))
    const signal = new AbortController().signal
    const first = await pager.get(query(), 1, signal)
    await pager.get(query(), 2, signal)
    pager.invalidate(2)
    expect(await pager.get(query(), 1, signal)).toBe(first)
    expect(pager.indexOf(await pager.get(query(), 2, signal))).toBe(2)
})

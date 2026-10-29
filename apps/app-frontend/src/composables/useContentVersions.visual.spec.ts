import { afterEach, beforeEach, expect, it, vi } from 'vitest'

import type { ResourceRelease } from '@/helpers/resources'

import {
    contentVersionContext,
    type ContentVersionSource,
    useContentVersions,
} from './useContentVersions'

// The composable takes its resource helpers as an argument, so this spec stubs them
// directly instead of mocking `@/helpers/resources`. A module mock that silently fails
// to apply used to turn the `vi.mocked(...)` calls below into confusing
// `TypeError: ... is not a function` failures; injected stubs cannot be affected by
// whatever the module graph happened to contain.
const resourceReleases = vi.fn<ContentVersionSource['resourceReleases']>()
const resourceReleaseDocuments = vi.fn<ContentVersionSource['resourceReleaseDocuments']>()
const source: ContentVersionSource = { resourceReleases, resourceReleaseDocuments }

beforeEach(() => {
    resourceReleases.mockReset()
    resourceReleaseDocuments.mockReset()
})
afterEach(() => vi.useRealTimers())

const project = { source: 'modrinth' as const, projectId: 'project' }
const release = { reference: { project, releaseId: 'version' } } as ResourceRelease
const page = (entries: ResourceRelease[], cursor: string | null = null) => ({
    requestId: 'test',
    hasMore: cursor !== null,
    entries,
    cursor,
    total: { precision: 'exact' as const, value: entries.length },
    sourceFailures: [],
    exhaustedSources: [],
})

it('loads all pages and pinned version, then retains context for retry after a source error', async () => {
    const state = useContentVersions(source)
    resourceReleases
        .mockRejectedValueOnce(new Error('offline'))
        .mockResolvedValueOnce(page([], 'next'))
        .mockResolvedValueOnce(page([]))
    resourceReleaseDocuments.mockResolvedValue([{ release, changelog: null, protocol: null }])
    await state.open(project, 'version')
    expect(state.loading.value).toBe(false)
    expect(state.error.value).toBeInstanceOf(Error)
    await state.retry()
    expect(state.error.value).toBeNull()
    expect(state.loading.value).toBe(false)
    expect(state.versions.value).toEqual([release])
    expect(resourceReleases).toHaveBeenLastCalledWith(
        { kind: 'source', project },
        undefined,
        'next',
        undefined,
        expect.any(AbortSignal),
    )
    state.reset()
})

it('cancels old requests and rejects late responses after switching projects or closing', async () => {
    const state = useContentVersions(source)
    let complete!: (value: ReturnType<typeof page>) => void
    resourceReleases
        .mockImplementationOnce(
            () =>
                new Promise((resolve) => {
                    complete = resolve
                }),
        )
        .mockResolvedValue(page([]))
    const old = state.open(project)
    const signal = resourceReleases.mock.calls[0][4]!
    await state.open({ ...project, projectId: 'second' })
    expect(signal.aborted).toBe(true)
    complete(page([release]))
    await old
    expect(state.versions.value).toEqual([])
    expect(state.error.value).toBeNull()
    state.reset()
    expect(state.loading.value).toBe(false)
})

it('aborts a timed-out native request and leaves a retryable error without a spinner', async () => {
    vi.useFakeTimers()
    const state = useContentVersions(source)
    resourceReleases.mockImplementation(
        (_project, _target, _cursor, _locator, signal) =>
            new Promise((_resolve, reject) => {
                signal!.addEventListener('abort', () => reject(signal!.reason), { once: true })
            }),
    )
    const pending = state.open(project)
    await vi.advanceTimersByTimeAsync(30_000)
    await pending
    expect(state.loading.value).toBe(false)
    expect(state.error.value).toBeInstanceOf(Error)
    expect(resourceReleases.mock.calls[0][4]!.aborted).toBe(true)
    state.reset()
})

it('shows each page as it arrives and keeps them when a later page fails, then resumes there', async () => {
    const state = useContentVersions(source)
    const second = { reference: { project, releaseId: 'second' } } as ResourceRelease
    let failSecondPage!: (error: Error) => void
    resourceReleases
        .mockResolvedValueOnce(page([release], 'page-2'))
        .mockImplementationOnce(
            () =>
                new Promise((_resolve, reject) => {
                    failSecondPage = reject
                }),
        )
        .mockResolvedValueOnce(page([release, second]))
    const pending = state.open(project)
    await vi.waitFor(() => expect(state.versions.value).toEqual([release]))
    expect(state.loading.value).toBe(true)
    failSecondPage(new Error('offline'))
    await pending
    expect(state.versions.value).toEqual([release])
    expect(state.error.value).toBeInstanceOf(Error)
    expect(state.loading.value).toBe(false)

    await state.retry()
    expect(resourceReleases).toHaveBeenLastCalledWith(
        { kind: 'source', project },
        undefined,
        'page-2',
        undefined,
        expect.any(AbortSignal),
    )
    expect(state.versions.value).toEqual([release, second])
    expect(state.error.value).toBeNull()
    state.reset()
})

it('uses persisted provider identity while local metadata is still displayed', () => {
    const item = {
        project: { id: 'local:fabric-api' },
        version: { id: 'display-version' },
        origin_provider: 'modrinth',
    } as Parameters<typeof contentVersionContext>[0]
    const entry = {
        provider: 'curseforge',
        providerProjectId: '1234',
        providerReleaseId: '5678',
        artifactRef: null,
    } as Parameters<typeof contentVersionContext>[1]
    expect(contentVersionContext(item, entry)).toEqual({
        project: { source: 'curseforge', projectId: '1234' },
        pinned: '5678',
    })
    expect(contentVersionContext(item)).toBeNull()
})

import { afterEach, beforeEach, expect, it, vi } from 'vitest'

import {
    type ResourceRelease,
    resourceReleaseDocuments,
    resourceReleases,
} from '@/helpers/resources'

import { contentVersionContext, useContentVersions } from './useContentVersions'

vi.mock('@/helpers/resources', () => ({
    resourceReleases: vi.fn(),
    resourceReleaseDocuments: vi.fn(),
}))
beforeEach(() => vi.resetAllMocks())
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
    const state = useContentVersions()
    vi.mocked(resourceReleases)
        .mockRejectedValueOnce(new Error('offline'))
        .mockResolvedValueOnce(page([], 'next'))
        .mockResolvedValueOnce(page([]))
    vi.mocked(resourceReleaseDocuments).mockResolvedValue([
        { release, changelog: null, protocol: null },
    ])
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
    const state = useContentVersions()
    let complete!: (value: ReturnType<typeof page>) => void
    vi.mocked(resourceReleases)
        .mockImplementationOnce(
            () =>
                new Promise((resolve) => {
                    complete = resolve
                }),
        )
        .mockResolvedValue(page([]))
    const old = state.open(project)
    const signal = vi.mocked(resourceReleases).mock.calls[0][4]!
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
    const state = useContentVersions()
    vi.mocked(resourceReleases).mockImplementation(
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
    expect(vi.mocked(resourceReleases).mock.calls[0][4]!.aborted).toBe(true)
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

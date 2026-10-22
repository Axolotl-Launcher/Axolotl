import { beforeEach, expect, it, vi } from 'vitest'

import { getCurseForgeFiles, getCurseForgeFilesPage } from './curseforge'
import { resourceReleaseDocuments, resourceReleases } from './resources'

vi.mock('./resources.ts', async (importOriginal) => ({
    ...(await importOriginal<typeof import('./resources')>()),
    resourceReleases: vi.fn(),
    resourceReleaseDocuments: vi.fn(),
}))

function page(start: number, count: number, cursor: string | null) {
    return {
        entries: Array.from({ length: count }, (_, offset) => ({
            reference: {
                project: { source: 'curseforge', projectId: '715572' },
                releaseId: String(start + offset),
            },
        })),
        cursor,
        hasMore: cursor !== null,
        total: { value: 51, precision: 'exact' },
        exhaustedSources: [],
        sourceFailures: [],
        requestId: 'test',
    } as Awaited<ReturnType<typeof resourceReleases>>
}

beforeEach(() => {
    vi.resetAllMocks()
    vi.mocked(resourceReleaseDocuments).mockImplementation(
        async (_source, ids) =>
            ids.map((id) => ({ protocol: { id: Number(id) } })) as Awaited<
                ReturnType<typeof resourceReleaseDocuments>
            >,
    )
})

it('loads more than fifty CurseForge files using opaque cursors and preserves source identity', async () => {
    vi.mocked(resourceReleases)
        .mockResolvedValueOnce(page(1, 50, 'opaque-session-token'))
        .mockResolvedValueOnce(page(51, 1, null))
    const result = await getCurseForgeFiles(715572, {})
    expect(result.files.map((file) => file.id)).toEqual(Array.from({ length: 51 }, (_, i) => i + 1))
    expect(resourceReleases).toHaveBeenNthCalledWith(
        2,
        { kind: 'source', project: { source: 'curseforge', projectId: '715572' } },
        { gameVersions: [], loaders: [], channels: [] },
        'opaque-session-token',
    )
    expect(result.pagination).toMatchObject({ index: 0, resultCount: 51, totalCount: 51 })
})

it('resolves a numeric page offset by walking source cursors', async () => {
    vi.mocked(resourceReleases)
        .mockResolvedValueOnce(page(1, 50, 'next-token'))
        .mockResolvedValueOnce(page(51, 1, null))
    const result = await getCurseForgeFilesPage(715572, { index: 50 })
    expect(result.files.map((file) => file.id)).toEqual([51])
    expect(resourceReleaseDocuments).toHaveBeenCalledTimes(1)
    expect(resourceReleases).toHaveBeenLastCalledWith(
        expect.anything(),
        expect.anything(),
        'next-token',
    )
})

it('starts with a fresh cursor when retrying a failed install lookup', async () => {
    vi.mocked(resourceReleases)
        .mockRejectedValueOnce(new Error('expired session'))
        .mockResolvedValueOnce(page(1, 1, null))
    await expect(getCurseForgeFiles(715572, {})).rejects.toThrow('expired session')
    expect((await getCurseForgeFiles(715572, {})).files).toHaveLength(1)
    expect(resourceReleases).toHaveBeenLastCalledWith(expect.anything(), expect.anything(), null)
})

it('rejects an incomplete install catalog when the provider window is exhausted', async () => {
    const exhausted = page(1, 1, 'unusable-token')
    exhausted.exhaustedSources = ['curseforge']
    vi.mocked(resourceReleases).mockResolvedValueOnce(exhausted)
    await expect(getCurseForgeFiles(715572, {})).rejects.toThrow('release window is incomplete')
    expect(resourceReleaseDocuments).not.toHaveBeenCalled()
    expect(resourceReleases).toHaveBeenCalledTimes(1)
})

it('continues through an empty page with a valid continuation token', async () => {
    vi.mocked(resourceReleases)
        .mockResolvedValueOnce(page(1, 0, 'next-token'))
        .mockResolvedValueOnce(page(1, 1, null))
    expect((await getCurseForgeFiles(715572, {})).files).toHaveLength(1)
    expect(resourceReleases).toHaveBeenCalledTimes(2)
})

it('rejects a missing continuation token instead of silently dropping remaining versions', async () => {
    const invalid = page(1, 1, null)
    invalid.hasMore = true
    vi.mocked(resourceReleases).mockResolvedValueOnce(invalid)
    await expect(getCurseForgeFiles(715572, {})).rejects.toThrow('invalid release cursor')
})

it('rejects a cursor cycle across multiple pages without returning a partial catalog', async () => {
    vi.mocked(resourceReleases)
        .mockResolvedValueOnce(page(1, 1, 'a'))
        .mockResolvedValueOnce(page(2, 1, 'b'))
        .mockResolvedValueOnce(page(3, 1, 'a'))
    await expect(getCurseForgeFiles(715572, {})).rejects.toThrow('invalid release cursor')
    expect(resourceReleases).toHaveBeenCalledTimes(3)
})

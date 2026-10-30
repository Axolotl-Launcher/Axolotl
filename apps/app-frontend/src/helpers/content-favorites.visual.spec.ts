import { expect, it, vi } from 'vitest'

import { prefetchContentFavorites, resolveContentFavorite } from './content-favorites'
import { resourceGet, resourceLocate, resourceProjectDocuments } from './resources'

vi.mock('./resources', () => ({
    resourceGet: vi.fn(),
    resourceLocate: vi.fn(),
    resourceProjectDocuments: vi.fn(),
}))

it('prefetches each source as one batch and keeps expired metadata when the network fails', async () => {
    vi.clearAllMocks()
    const metadata = {
        project: { source: 'modrinth', projectId: 'abcdefgh' },
        title: 'Saved project',
    }
    vi.mocked(resourceProjectDocuments)
        .mockRejectedValueOnce(new Error('offline'))
        .mockResolvedValueOnce([{ metadata }] as Awaited<
            ReturnType<typeof resourceProjectDocuments>
        >)
    const favorites = [
        {
            provider: 'modrinth' as const,
            project_id: 'abcdefgh',
            content_type: 'mod' as const,
            saved_at: 1,
        },
        {
            provider: 'modrinth' as const,
            project_id: 'ijklmnop',
            content_type: 'mod' as const,
            saved_at: 2,
        },
    ]
    const cached = await prefetchContentFavorites(favorites)
    expect(resourceProjectDocuments).toHaveBeenCalledTimes(2)
    expect(resourceProjectDocuments).toHaveBeenLastCalledWith(
        'modrinth',
        ['abcdefgh', 'ijklmnop'],
        undefined,
        'onlyCached',
    )
    expect(cached.get('modrinth:abcdefgh')).toEqual(metadata)
    expect(resourceGet).not.toHaveBeenCalled()
})

it('uses the available confirmed source while preserving the saved favorite identity', async () => {
    vi.clearAllMocks()
    const metadata = {
        project: { source: 'curseforge', projectId: '123' },
        title: 'Available project',
    }
    vi.mocked(resourceGet).mockResolvedValue({
        defaultProject: metadata.project,
        projects: [metadata],
    } as Awaited<ReturnType<typeof resourceGet>>)
    const favorite = {
        provider: 'modrinth' as const,
        project_id: 'abcdefgh',
        resource_id: 'resource',
        content_type: 'mod' as const,
        saved_at: 10,
    }
    expect(await resolveContentFavorite(favorite)).toEqual(metadata)
    expect(resourceGet).toHaveBeenCalledExactlyOnceWith(
        { kind: 'resource', id: 'resource' },
        undefined,
        undefined,
        undefined,
    )
    expect(resourceLocate).not.toHaveBeenCalled()
    expect(favorite.provider).toBe('modrinth')
})

it('reads local metadata for every favorite source without falling back to a network request', async () => {
    vi.clearAllMocks()
    vi.mocked(resourceProjectDocuments).mockRejectedValue(new Error('not cached'))
    const sources = ['modrinth', 'curseforge', 'mcarchive', 'planet_minecraft'] as const
    await expect(
        prefetchContentFavorites(
            sources.map((provider) => ({
                provider,
                project_id: 'one',
                content_type: 'mod',
                saved_at: 1,
            })),
            undefined,
            'onlyCached',
        ),
    ).resolves.toEqual(new Map())
    expect(resourceProjectDocuments).toHaveBeenCalledTimes(4)
    for (const source of sources) {
        expect(resourceProjectDocuments).toHaveBeenCalledWith(
            source,
            ['one'],
            undefined,
            'onlyCached',
        )
    }
    expect(resourceLocate).not.toHaveBeenCalled()
    expect(resourceGet).not.toHaveBeenCalled()
})

it('keeps a cached project when another item in the same batch has no local metadata', async () => {
    vi.clearAllMocks()
    const metadata = { project: { source: 'modrinth', projectId: 'one' }, title: 'Saved' }
    vi.mocked(resourceProjectDocuments).mockImplementation(async (_, ids) => {
        if (ids.includes('two')) throw new Error('cache miss')
        return [{ metadata }] as Awaited<ReturnType<typeof resourceProjectDocuments>>
    })
    const cached = await prefetchContentFavorites(
        ['one', 'two'].map((project_id) => ({
            provider: 'modrinth',
            project_id,
            content_type: 'mod',
            saved_at: 1,
        })),
        undefined,
        'onlyCached',
    )
    expect(cached.get('modrinth:one')).toEqual(metadata)
    expect(cached.has('modrinth:two')).toBe(false)
    expect(resourceProjectDocuments).toHaveBeenCalledTimes(3)
    expect(
        vi.mocked(resourceProjectDocuments).mock.calls.every((call) => call[3] === 'onlyCached'),
    ).toBe(true)
})

it('retains a source-only historical locator and reports failure when the resource has no presentation', async () => {
    vi.clearAllMocks()
    const metadata = {
        project: { source: 'mcarchive', projectId: 'uuid' },
        title: 'Archive project',
    }
    vi.mocked(resourceLocate).mockResolvedValue({ metadata } as Awaited<
        ReturnType<typeof resourceLocate>
    >)
    const favorite = {
        provider: 'mcarchive' as const,
        project_id: 'uuid',
        locator: 'original-slug',
        content_type: 'mod' as const,
        saved_at: 10,
    }
    expect(await resolveContentFavorite(favorite)).toEqual(metadata)
    expect(resourceLocate).toHaveBeenCalledExactlyOnceWith('mcarchive', 'original-slug', undefined)
    vi.mocked(resourceGet).mockRejectedValue(new Error('all sources unavailable'))
    await expect(resolveContentFavorite({ ...favorite, resource_id: 'resource' })).rejects.toThrow(
        'all sources unavailable',
    )
})

it('reports each settled source before unrelated batches complete', async () => {
    vi.clearAllMocks()
    let finish!: (value: Awaited<ReturnType<typeof resourceProjectDocuments>>) => void
    const metadata = { project: { source: 'curseforge', projectId: 'two' }, title: 'Fast project' }
    vi.mocked(resourceProjectDocuments).mockImplementation(async (source) =>
        source === 'modrinth'
            ? new Promise((resolve) => {
                  finish = resolve
              })
            : ([{ metadata }] as Awaited<ReturnType<typeof resourceProjectDocuments>>),
    )
    const progress = vi.fn()
    const pending = prefetchContentFavorites(
        [
            { provider: 'modrinth', project_id: 'one', content_type: 'mod', saved_at: 1 },
            { provider: 'curseforge', project_id: 'two', content_type: 'mod', saved_at: 2 },
        ],
        undefined,
        'useFresh',
        progress,
    )
    await vi.waitFor(() => expect(progress).toHaveBeenCalledTimes(1))
    expect(progress).toHaveBeenCalledWith('curseforge', expect.any(Map))
    expect(progress.mock.calls[0][1].get('curseforge:two')).toEqual(metadata)
    finish([])
    const complete = await pending
    expect(progress).toHaveBeenCalledTimes(2)
    expect(complete.get('curseforge:two')).toEqual(metadata)
})

import { invoke } from '@tauri-apps/api/core'
import { expect, it, vi } from 'vitest'

import { get_project, get_project_versions, purgeLauncherCache } from './cache.js'
import { resourceLocate } from './resources'

vi.mock('./resources', () => ({
    resourceCachePolicy: () => 'useFresh',
    resourceLocate: vi.fn(),
    resourceProjectDocuments: vi.fn(),
    resourceReleaseDocuments: vi.fn(),
    resourceRequest: vi.fn(),
}))
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))

it('clears disk and metadata caches through one storage operation', async () => {
    const result = { removed_bytes: 100, remaining_bytes: 20, failures: [] }
    vi.mocked(invoke).mockClear().mockResolvedValueOnce(result)
    expect(await purgeLauncherCache()).toEqual(result)
    expect(invoke).toHaveBeenCalledTimes(1)
    expect(invoke).toHaveBeenCalledWith('plugin:storage|storage_cache_clear')
})

it('requests a lightweight project version list without fetching each changelog', async () => {
    vi.mocked(invoke).mockResolvedValue([{ id: 'release1' }])
    expect(await get_project_versions('abcdefgh', 'must_revalidate')).toEqual([{ id: 'release1' }])
    expect(invoke).toHaveBeenCalledWith('plugin:cache|get_project_versions', {
        projectId: 'abcdefgh',
        cacheBehaviour: 'must_revalidate',
    })
})

it('project links resolve slugs while retaining all source date semantics', async () => {
    vi.mocked(resourceLocate).mockResolvedValue({
        metadata: {
            createdAt: '2020-01-01',
            projectPublishedAt: '2021-01-01',
            updatedAt: '2025-01-01',
            firstPublishedAt: null,
            latestPublishedAt: null,
        },
        protocol: {
            id: 'abcdefgh',
            slug: 'sample-project',
            published: '2020-01-01',
            approved: '2021-01-01',
        },
    } as Awaited<ReturnType<typeof resourceLocate>>)
    const project = await get_project('sample-project')
    expect(resourceLocate).toHaveBeenCalledWith('modrinth', 'sample-project', undefined, 'useFresh')
    expect(project.id).toBe('abcdefgh')
    expect(project.resourceDates).toEqual({
        createdAt: '2020-01-01',
        projectPublishedAt: '2021-01-01',
        updatedAt: '2025-01-01',
        firstPublishedAt: null,
        latestPublishedAt: null,
    })
})

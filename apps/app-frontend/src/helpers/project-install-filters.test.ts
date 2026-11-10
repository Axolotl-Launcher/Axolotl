import { beforeEach, expect, it, vi } from 'vitest'

import { getCurseForgeFiles } from './curseforge'
import {
    filteredProjectRelease,
    installPreferencesEqual,
    projectInstallPreferences,
} from './project-install-filters'
import { resourceReleases } from './resources'

vi.mock('./curseforge', () => ({ getCurseForgeFiles: vi.fn() }))
vi.mock('./resources', () => ({
    resourceReleases: vi.fn(),
    sourceResource: (source: string, projectId: string) => ({
        kind: 'source',
        project: { source, projectId },
    }),
}))
beforeEach(() => vi.clearAllMocks())
const target = { gameVersions: ['26.3'], loaders: ['fabric'] }
it('keeps unlocked empty axes distinct from the target and preserves custom version filters', () => {
    expect(projectInstallPreferences({ g: '', l: '' }, target)).toEqual({
        gameVersions: [],
        loaders: [],
    })
    expect(projectInstallPreferences({ g: '1.21.1', l: 'neoforge' }, target)).toEqual({
        gameVersions: ['1.21.1'],
        loaders: ['neoforge'],
    })
    expect(projectInstallPreferences({}, target)).toEqual(target)
    expect(installPreferencesEqual({}, target)).toBe(false)
    expect(installPreferencesEqual({ gameVersions: [] }, { gameVersions: ['26.3'] })).toBe(false)
    expect(
        installPreferencesEqual({ loaders: ['forge', 'fabric'] }, { loaders: ['fabric', 'forge'] }),
    ).toBe(true)
})
it.each(['modrinth', 'curseforge'] as const)(
    'freezes the filtered %s release without a target-version fallback',
    async (source) => {
        const preferences = projectInstallPreferences({ g: '1.21.1', l: 'neoforge' }, target)
        vi.mocked(resourceReleases).mockResolvedValueOnce({
            entries: [
                {
                    reference: { releaseId: 'chosen' },
                    compatibility: { gameVersions: ['1.21.1'], loaders: ['neoforge'] },
                    channel: 'beta',
                    artifacts: [{ role: 'primary', available: true }],
                },
            ],
            hasMore: false,
        } as never)
        expect(await filteredProjectRelease(source, 'project', preferences, ['beta'])).toBe(
            'chosen',
        )
        expect(resourceReleases).toHaveBeenCalledWith(
            { kind: 'source', project: { source, projectId: 'project' } },
            {
                gameVersions: ['1.21.1'],
                loaders: ['neoforge'],
                channels: source === 'curseforge' ? [] : ['beta'],
            },
            null,
        )
        vi.mocked(resourceReleases).mockResolvedValueOnce({ entries: [], hasMore: false } as never)
        expect(await filteredProjectRelease(source, 'project', preferences)).toBeNull()
    },
)
it('continues pages with unavailable files without broadening the filters', async () => {
    vi.mocked(resourceReleases)
        .mockResolvedValueOnce({ entries: [], hasMore: true, cursor: 'next' } as never)
        .mockResolvedValueOnce({
            entries: [
                {
                    reference: { releaseId: 'later' },
                    compatibility: { gameVersions: ['26.3'], loaders: ['fabric'] },
                    channel: 'release',
                    artifacts: [{ role: 'primary', available: true }],
                },
            ],
            hasMore: false,
        } as never)
    expect(await filteredProjectRelease('curseforge', 'project', target)).toBe('later')
    expect(resourceReleases).toHaveBeenLastCalledWith(
        { kind: 'source', project: { source: 'curseforge', projectId: 'project' } },
        { ...target, channels: [] },
        'next',
    )
})

it('applies multi-value CurseForge filters across pages without sending unsupported axes', async () => {
    vi.mocked(resourceReleases)
        .mockResolvedValueOnce({
            entries: [
                {
                    reference: { releaseId: 'wrong-channel' },
                    compatibility: { gameVersions: ['1.21.1'], loaders: ['forge'] },
                    channel: 'release',
                    artifacts: [{ role: 'primary', available: true }],
                },
            ],
            hasMore: true,
            cursor: 'next',
        } as never)
        .mockResolvedValueOnce({
            entries: [
                {
                    reference: { releaseId: 'wanted-beta' },
                    compatibility: { gameVersions: ['1.21.2'], loaders: ['neoforge'] },
                    channel: 'beta',
                    artifacts: [{ role: 'primary', available: true }],
                },
            ],
            hasMore: false,
        } as never)
    const selected = { gameVersions: ['1.21.1', '1.21.2'], loaders: ['forge', 'neoforge'] }
    expect(await filteredProjectRelease('curseforge', 'project', selected, ['beta'])).toBe(
        'wanted-beta',
    )
    expect(resourceReleases).toHaveBeenLastCalledWith(
        { kind: 'source', project: { source: 'curseforge', projectId: 'project' } },
        { gameVersions: [], loaders: [], channels: [] },
        'next',
    )
})

it('matches CurseForge shader runtime tags without silently ignoring them', async () => {
    vi.mocked(getCurseForgeFiles).mockResolvedValueOnce({
        files: [
            { id: 1, isAvailable: true, gameVersions: ['1.21.1', 'OptiFine'], releaseType: 1 },
            { id: 2, isAvailable: true, gameVersions: ['1.21.1', 'Iris'], releaseType: 1 },
        ],
    } as never)
    expect(
        await filteredProjectRelease(
            'curseforge',
            '123',
            { gameVersions: ['1.21.1'], loaders: ['iris'] },
            [],
            'shader',
        ),
    ).toBe('2')
    expect(resourceReleases).not.toHaveBeenCalled()
})

it('prefers a stable Modrinth release on a later page over the newest beta', async () => {
    const release = (id: string, channel: string, publishedAt: string) => ({
        reference: { releaseId: id },
        channel,
        publishedAt,
        compatibility: { gameVersions: ['1.21.1'], loaders: ['neoforge'] },
        artifacts: [{ role: 'primary', available: true }],
    })
    vi.mocked(resourceReleases)
        .mockResolvedValueOnce({
            entries: [release('beta', 'beta', '2026-10-01')],
            hasMore: true,
            cursor: 'stable-page',
        } as never)
        .mockResolvedValueOnce({
            entries: [release('stable', 'release', '2026-09-01')],
            hasMore: false,
        } as never)
    expect(
        await filteredProjectRelease('modrinth', 'project', {
            gameVersions: ['1.21.1'],
            loaders: ['neoforge'],
        }),
    ).toBe('stable')
})

it('distinguishes an exhausted release window from a completed search with no matches', async () => {
    vi.mocked(resourceReleases).mockResolvedValueOnce({
        entries: [],
        hasMore: false,
        cursor: null,
        exhaustedSources: ['curseforge'],
    } as never)
    await expect(filteredProjectRelease('curseforge', '123', target)).rejects.toThrow(
        'Source release window is incomplete',
    )
    vi.mocked(resourceReleases).mockResolvedValueOnce({
        entries: [],
        hasMore: false,
        cursor: null,
        exhaustedSources: [],
    } as never)
    expect(await filteredProjectRelease('curseforge', '123', target)).toBeNull()
})
it('retains a valid file already found at the provider window boundary', async () => {
    vi.mocked(resourceReleases).mockResolvedValueOnce({
        entries: [
            {
                reference: { releaseId: 'valid' },
                compatibility: target,
                channel: 'release',
                artifacts: [{ role: 'primary', available: true }],
            },
        ],
        hasMore: false,
        exhaustedSources: ['curseforge'],
    } as never)
    expect(await filteredProjectRelease('curseforge', '123', target)).toBe('valid')
})

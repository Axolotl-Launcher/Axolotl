import { expect, it, vi } from 'vitest'

import {
    loadUpgradeProjectDisplayMetadata,
    loadUpgradeVersionDisplayMetadata,
} from './upgrade-version-metadata'

vi.mock('./cache.js', () => ({
    get_project_many: async (ids: string[]) => ids.map((id) => ({ id, title: id })),
    get_version_many: async (ids: string[]) => ids.map((id) => ({ id, version_number: `v-${id}` })),
}))
vi.mock('./curseforge', () => ({
    getCurseForgeProjects: async () => [],
    getCurseForgeFile: vi.fn(),
    getCurseForgeChangelog: vi.fn(),
    getCurseForgeImageUrl: (value: string) => value,
}))

it('returns the entire requested batch even when it exceeds the retained cache capacity', async () => {
    const identities = Array.from({ length: 2100 }, (_, index) => ({
        provider: 'modrinth' as const,
        projectId: `project-${index}`,
        releaseId: `release-${index}`,
    }))
    const projects = await loadUpgradeProjectDisplayMetadata(identities)
    const versions = await loadUpgradeVersionDisplayMetadata(identities)
    expect(projects.size).toBe(2100)
    expect(versions.size).toBe(2100)
    expect(versions.get('modrinth:project-0:release-0')?.version).toBe('v-release-0')
})

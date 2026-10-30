import { beforeEach, expect, it, vi } from 'vitest'

import { contentProjectUrl } from './content-project-links'
import { get_content_snapshot, type InstanceContentSnapshot } from './instance'
import { loadInstanceContentData, normalizeInstanceContentSnapshot } from './instance-content'
vi.mock('./instance', () => ({ get_content_snapshot: vi.fn() }))
// content-project-links reaches content-identity, which imports invoke.
vi.mock('@tauri-apps/api/core', () => ({ convertFileSrc: (path: string) => path, invoke: vi.fn() }))
beforeEach(() => vi.resetAllMocks())
const snapshot = (): InstanceContentSnapshot => ({
    instanceId: 'instance',
    revision: 1,
    pack: null,
    pendingManualDownloads: [],
    warnings: [],
    items: [
        {
            fileId: 'file',
            entryId: 'entry',
            memberId: null,
            ownershipKind: 'user_added',
            materializationState: 'present',
            overrideKind: 'none',
            expectedRelativePath: 'mods/example.jar',
            required: true,
            projectType: 'mod',
            provider: 'modrinth',
            providerProjectId: 'project',
            providerReleaseId: 'release',
            content: null,
            dependency: null,
            capabilities: {
                canToggle: true,
                canDelete: true,
                canUpdate: true,
                canChangeVersion: true,
                canRestorePackDefault: false,
            },
        },
    ],
})

it('loads the local snapshot without a second remote refresh', async () => {
    vi.mocked(get_content_snapshot).mockResolvedValue(snapshot())
    const data = await loadInstanceContentData('instance', 'bypass')
    expect(data?.contentItems[0].file_name).toBe('example.jar')
    expect(get_content_snapshot).toHaveBeenCalledOnce()
})

it('normalizes only changed entries during incremental refresh', () => {
    const initial = snapshot()
    initial.items.push({
        ...initial.items[0],
        entryId: 'other',
        fileId: 'other-file',
        expectedRelativePath: 'mods/other.jar',
    })
    const cache = new WeakMap()
    const first = normalizeInstanceContentSnapshot('instance', initial, cache)
    const update = {
        ...initial,
        items: [
            {
                ...initial.items[0],
                resourceUpdate: {
                    project: { source: 'modrinth', projectId: 'project' },
                    releaseId: 'next',
                },
            },
            initial.items[1],
        ],
    }
    const next = normalizeInstanceContentSnapshot('instance', update, cache)
    expect(next.contentItems[0]).not.toBe(first.contentItems[0])
    expect(next.contentItems[0].resourceUpdate?.releaseId).toBe('next')
    expect(next.contentItems[1]).toBe(first.contentItems[1])
})

it('restores a source qualified saved candidate and honors explicit no update', () => {
    const value = snapshot()
    value.items[0].resourceUpdate = {
        project: { source: 'curseforge', projectId: '123' },
        releaseId: '456',
    }
    const saved = normalizeInstanceContentSnapshot('instance', value)
    expect(saved.contentItems[0].resourceUpdate?.releaseId).toBe('456')
    expect(saved.contentItems[0].instanceCapabilities?.canUpdate).toBe(true)
    value.items[0].resourceUpdate = null
    const cleared = normalizeInstanceContentSnapshot('instance', value)
    expect(cleared.contentItems[0].resourceUpdate).toBeNull()
    expect(cleared.contentItems[0].instanceCapabilities?.canUpdate).toBe(false)
})

it('keeps local pack membership and accepts an empty authoritative snapshot', () => {
    const value = snapshot()
    value.pack = {
        name: 'Local pack',
        iconPath: null,
        provider: null,
        projectId: null,
        versionId: null,
        reconciled: true,
        canUpdate: false,
        metadata: null,
    }
    value.items[0].ownershipKind = 'pack_managed'
    const grouped = normalizeInstanceContentSnapshot('instance', value)
    expect(grouped.linkedContentItems).toHaveLength(1)
    expect(grouped.modpack?.project.title).toBe('Local pack')
    value.items = []
    const empty = normalizeInstanceContentSnapshot('instance', value)
    expect(empty.contentItems).toEqual([])
    expect(empty.linkedContentItems).toEqual([])
})

it('uses exact artifact source for connector content and keeps loader independent', () => {
    const value = snapshot()
    value.items[0].artifactRef = {
        release: {
            project: { source: 'planet_minecraft', projectId: 'opaque' },
            releaseId: 'release',
        },
        artifactId: 'download',
    }
    const data = normalizeInstanceContentSnapshot('instance', value)
    expect(data.contentItems[0].installedSource).toBe('planet_minecraft')
    expect(data.contentItems[0].origin_provider).toBe('planet_minecraft')
})

it.each([
    ['modrinth', 'project', 'release', 'https://modrinth.com/project/project'],
    ['curseforge', '123', '456', 'https://www.curseforge.com/projects/123'],
])(
    'restores project actions for newly recognized %s files',
    (source, projectId, releaseId, url) => {
        const value = snapshot()
        value.items[0].content = {
            id: 'entry',
            file_name: 'example.jar',
            size: 3,
            enabled: true,
            project_type: 'mod',
            provider_refs: [],
            project: { id: projectId, slug: projectId, title: 'Recognized' },
        }
        value.items[0].artifactRef = {
            release: { project: { source, projectId }, releaseId },
            artifactId: 'download',
        }
        const item = normalizeInstanceContentSnapshot('instance', value).contentItems[0]
        expect(item.provider_refs).toHaveLength(1)
        expect(contentProjectUrl(item)).toBe(url)
    },
)

import { beforeEach, expect, it, vi } from 'vitest'

import { get_content_snapshot, type InstanceContentSnapshot } from './instance'
import {
    type InstanceContentData,
    loadInstanceContentData,
    loadInstanceContentInStages,
    refreshInstanceContentUpdates,
} from './instance-content'
import { resourceCheckUpdates } from './resources'

vi.mock('./instance', () => ({ get_content_snapshot: vi.fn(), refresh_content: vi.fn() }))
vi.mock('./resources', () => ({ resourceCheckUpdates: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ convertFileSrc: (path: string) => path }))

beforeEach(() => vi.resetAllMocks())

it('publishes the local list while the enriched snapshot is still pending', async () => {
    let complete!: (value: InstanceContentSnapshot) => void
    vi.mocked(get_content_snapshot)
        .mockResolvedValueOnce(snapshot())
        .mockImplementationOnce(
            () =>
                new Promise((resolve) => {
                    complete = resolve
                }),
        )
    const showLocal = vi.fn((_data: InstanceContentData) => true)
    const pending = loadInstanceContentInStages('instance', undefined, showLocal)
    await vi.waitFor(() => expect(showLocal).toHaveBeenCalledOnce())
    expect(get_content_snapshot).toHaveBeenNthCalledWith(1, 'instance', true)
    expect(showLocal.mock.calls[0][0].contentItems[0].file_name).toBe('example.jar')
    const enriched = snapshot()
    enriched.revision = 2
    complete(enriched)
    expect((await pending)?.snapshot.revision).toBe(2)
})

it('does not start enrichment when the first local response belongs to an obsolete request', async () => {
    vi.mocked(get_content_snapshot).mockResolvedValueOnce(snapshot())
    await loadInstanceContentInStages('instance', undefined, () => false)
    expect(get_content_snapshot).toHaveBeenCalledOnce()
})

it('connector installation source comes from exact durable artifact evidence instead of legacy local ownership', async () => {
    const value = snapshot()
    value.items[0].provider = null
    value.items[0].artifactRef = {
        release: {
            project: { source: 'planet_minecraft', projectId: 'opaque' },
            releaseId: 'version',
        },
        artifactId: 'download',
    }
    vi.mocked(get_content_snapshot).mockResolvedValue(value)
    const data = await loadInstanceContentData('instance')
    expect(data?.contentItems[0].installedSource).toBe('planet_minecraft')
    expect(data?.contentItems[0].origin_provider).toBe('planet_minecraft')
    expect(data?.contentItems[0].instanceOwnershipKind).toBe('user_added')
})

it('keeps a local pack grouped without an online project and ungroups it only after explicit unlink', async () => {
    const local = snapshot()
    local.pack = {
        name: 'Local pack',
        iconPath: null,
        provider: null,
        projectId: null,
        versionId: null,
        reconciled: true,
        canUpdate: false,
        metadata: null,
    }
    local.items[0].ownershipKind = 'pack_managed'
    local.items[0].memberId = 'member'
    vi.mocked(get_content_snapshot).mockResolvedValue(local)
    const grouped = (await loadInstanceContentData('instance'))!
    expect(grouped.modpack?.project.title).toBe('Local pack')
    expect(grouped.linkedContentItems).toHaveLength(1)
    expect(grouped.contentItems).toHaveLength(0)
    expect(grouped.snapshot.warnings).toHaveLength(0)
    expect(local.items[0].ownershipKind).toBe('pack_managed')

    const unlinked = snapshot()
    vi.mocked(get_content_snapshot).mockResolvedValue(unlinked)
    const ungrouped = (await loadInstanceContentData('instance'))!
    expect(ungrouped.modpack).toBeNull()
    expect(ungrouped.linkedContentItems).toHaveLength(0)
    expect(ungrouped.contentItems).toHaveLength(1)
    expect(ungrouped.contentItems[0].file_path).toBe(grouped.linkedContentItems[0].file_path)
})

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

it('returns local content without waiting for remote update discovery', async () => {
    vi.mocked(get_content_snapshot).mockResolvedValue(snapshot())
    vi.mocked(resourceCheckUpdates).mockImplementation(() => new Promise(() => {}))
    const data = await loadInstanceContentData('instance')
    expect(data?.contentItems[0].file_name).toBe('example.jar')
    expect(data?.contentItems[0].instanceCapabilities?.canUpdate).toBe(false)
    expect(resourceCheckUpdates).not.toHaveBeenCalled()
})

it('enriches updates separately and preserves the displayed snapshot', async () => {
    const original = snapshot()
    vi.mocked(get_content_snapshot).mockResolvedValue(original)
    vi.mocked(resourceCheckUpdates).mockResolvedValue({
        entries: ['entry'],
        releases: {
            entry: {
                project: { source: 'modrinth', projectId: 'project' },
                releaseId: 'new-release',
            },
        },
        unresolved: [],
        failures: [],
        failedEntries: [],
    })
    const data = (await loadInstanceContentData('instance'))!
    const enriched = await refreshInstanceContentUpdates(data)
    expect(enriched.contentItems[0].resourceUpdate?.releaseId).toBe('new-release')
    expect(enriched.contentItems[0].instanceCapabilities?.canUpdate).toBe(true)
    expect(data.contentItems[0].resourceUpdate).toBeNull()
    expect(data.contentItems[0].instanceCapabilities?.canUpdate).toBe(false)
    expect(original.items[0].capabilities.canUpdate).toBe(true)
})

it('keeps local content available when update discovery fails', async () => {
    vi.mocked(get_content_snapshot).mockResolvedValue(snapshot())
    vi.mocked(resourceCheckUpdates).mockRejectedValue(new Error('offline'))
    const data = (await loadInstanceContentData('instance'))!
    const enriched = await refreshInstanceContentUpdates(data)
    expect(enriched.contentItems).toHaveLength(1)
    expect(enriched.snapshot.warnings[0].code).toBe('resource_update_unavailable')
    expect(data.snapshot.warnings).toHaveLength(0)
})

it('discards update responses after the instance request is canceled', async () => {
    vi.mocked(get_content_snapshot).mockResolvedValue(snapshot())
    let resolve!: (result: Awaited<ReturnType<typeof resourceCheckUpdates>>) => void
    vi.mocked(resourceCheckUpdates).mockImplementation(
        () =>
            new Promise((done) => {
                resolve = done
            }),
    )
    const controller = new AbortController()
    const data = (await loadInstanceContentData('instance'))!
    const pending = refreshInstanceContentUpdates(data, controller.signal)
    controller.abort()
    resolve({
        entries: [],
        releases: {},
        unresolved: [],
        failures: ['late failure'],
        failedEntries: [],
    })
    expect(await pending).toBe(data)
    expect(resourceCheckUpdates).toHaveBeenCalledWith('instance', controller.signal)
})

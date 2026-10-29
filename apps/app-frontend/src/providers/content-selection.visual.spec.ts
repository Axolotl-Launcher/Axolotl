import { expect, it, vi } from 'vitest'
import { ref } from 'vue'

import { get_version_many } from '@/helpers/cache.js'
import { compareContentIdentities } from '@/helpers/content-identity'
import { getCurseForgeFiles } from '@/helpers/curseforge'
import { list as listInstances, queue_content_batch } from '@/helpers/instance'
import type { GameInstance } from '@/helpers/types'

import { createContentSelection } from './content-selection'
import type { ContentSelectionItem } from './content-selection-types'

it('concurrent additions recheck the live cart before committing a shared identity', async () => {
    vi.mocked(compareContentIdentities).mockImplementation((candidate, existing) =>
        candidate.title === existing.title
            ? ({ source: 'curated_mapping', confidence: 'exact', identity: existing } as never)
            : null,
    )
    try {
        const selection = createContentSelection({
            addNotification: vi.fn(),
            handleError: vi.fn(),
            downloadManager: { jobs: ref([]) } as never,
        })
        selection.setTarget(instance)
        const results = await Promise.all(
            ['modrinth', 'curseforge'].map((provider) =>
                selection.add({
                    key: `${provider}:same`,
                    provider: provider as 'modrinth' | 'curseforge',
                    projectId: 'same',
                    providerProjectId: 'same',
                    contentType: 'mod',
                    title: 'Same resource',
                }),
            ),
        )
        expect(results.filter(Boolean)).toHaveLength(1)
        expect(selection.items.value.size).toBe(1)
    } finally {
        vi.mocked(compareContentIdentities).mockImplementation(() => null)
    }
})

it('rejects a version prepared for a different target before resolving its identity', async () => {
    const selection = createContentSelection({
        addNotification: vi.fn(),
        handleError: vi.fn(),
        downloadManager: { jobs: ref([]) } as never,
    })
    selection.setTarget({ ...instance, id: 'new-target' })
    expect(
        await selection.add({
            key: 'modrinth:old',
            provider: 'modrinth',
            projectId: 'old',
            providerProjectId: 'old',
            contentType: 'mod',
            title: 'Old target release',
            versionId: 'old-release',
            targetInstanceId: instance.id,
        }),
    ).toBe(false)
    expect(selection.items.value.size).toBe(0)
})

it('discarding pending additions after target changes or clearing the cart cannot repopulate another instance', async () => {
    for (const change of ['target', 'clear']) {
        let finish!: (value: unknown[]) => void
        vi.mocked(get_version_many).mockImplementationOnce(
            () =>
                new Promise((resolve) => {
                    finish = resolve
                }),
        )
        const selection = createContentSelection({
            addNotification: vi.fn(),
            handleError: vi.fn(),
            downloadManager: { jobs: ref([]) } as never,
        })
        selection.setTarget(instance)
        const pending = selection.add({
            key: 'modrinth:late',
            provider: 'modrinth',
            projectId: 'late',
            providerProjectId: 'late',
            contentType: 'mod',
            title: 'Late',
            versionId: 'release',
        })
        if (change === 'target') selection.setTarget({ ...instance, id: 'another' })
        else selection.clear()
        finish([])
        expect(await pending).toBe(false)
        expect(selection.items.value.size).toBe(0)
    }
})

it('cancelling a second mixed-batch review keeps the original world and resource selections', async () => {
    const selection = createContentSelection({
        addNotification: vi.fn(),
        handleError: vi.fn(),
        downloadManager: { jobs: ref([]) } as never,
    })
    selection.setTarget(instance)
    selection.setPreviewModal({
        showBatch: vi.fn().mockResolvedValue({ primaryKeys: [] }),
    } as never)
    selection.setResourceInstallPreview({
        showInstallSelection: vi.fn().mockResolvedValue(null),
    } as never)
    await selection.add({
        key: 'curseforge:world',
        provider: 'curseforge',
        projectId: '42',
        providerProjectId: '42',
        contentType: 'world',
        title: 'World',
    })
    await selection.add({
        key: 'modrinth:mod',
        provider: 'modrinth',
        projectId: 'mod',
        providerProjectId: 'mod',
        contentType: 'mod',
        title: 'Mod',
    })
    vi.mocked(getCurseForgeFiles).mockResolvedValueOnce({
        files: [{ id: 123, isAvailable: true, hashes: [] }],
        pagination: {},
    } as never)
    expect(await selection.installSelected()).toBe(false)
    expect([...selection.items.value.keys()]).toEqual(['curseforge:world', 'modrinth:mod'])
})

vi.mock('@modrinth/ui', () => ({
    createContext: () => [vi.fn(), vi.fn()],
    defineMessages: (messages: unknown) => messages,
    useVIntl: () => ({
        formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
    }),
    usesTargetGameVersion: () => false,
}))
vi.mock('@/store/state', () => ({
    useTheming: () => ({ getFeatureFlag: () => true }),
}))
vi.mock('@/helpers/cache.js', () => ({
    get_project_many: vi.fn(),
    get_version_many: vi.fn(),
}))
vi.mock('@/helpers/content-identity', () => ({
    compareContentIdentities: vi.fn(() => null),
    contentIdentityFromInput: (input: unknown) => input,
    contentIdentityInputsFromSnapshot: vi.fn(() => []),
    resolveContentIdentities: vi.fn(() => []),
}))
vi.mock('@/helpers/curseforge', () => ({
    getCurseForgeFile: vi.fn(),
    getCurseForgeProjects: vi.fn(),
    getCurseForgeFiles: vi.fn(),
}))
vi.mock('@/helpers/instance', () => ({
    get_content_snapshot: vi.fn().mockResolvedValue(null),
    list: vi.fn().mockResolvedValue([]),
    queue_content_batch: vi.fn(),
}))
vi.mock('@/helpers/resources', () => ({
    resourceAnalysisOpen: vi.fn().mockResolvedValue('analysis'),
    resourceAnalysisClose: vi.fn().mockResolvedValue(undefined),
    resourceAnalysisUpdate: vi.fn().mockResolvedValue({ nodes: [] }),
    resourcePrepareOperation: vi.fn(),
    resourceProjectDocuments: vi.fn().mockResolvedValue([]),
    sourceResource: (provider: string, projectId: string) => ({
        kind: 'source',
        project: { source: provider, projectId },
    }),
}))
vi.mock('@/helpers/settings', () => ({
    getBrowseDefaultInstanceId: vi.fn(() => null),
    setBrowseDefaultInstanceId: vi.fn(),
}))

const instance = {
    id: 'instance-1',
    name: 'Example',
    loader: 'neoforge',
    game_version: '1.21.1',
} as GameInstance

it('routes pending-version cart items into the resource preview as automatic releases', async () => {
    const showInstall = vi
        .fn()
        .mockResolvedValue({ job: { job_id: 'job-1' }, activeRoots: ['modrinth:pending'] })
    const selection = createContentSelection({
        addNotification: vi.fn(),
        handleError: vi.fn(),
        downloadManager: { jobs: ref([]) } as never,
    })
    selection.setPreviewModal({} as never)
    selection.setResourceInstallPreview({ showInstallSelection: showInstall } as never)
    selection.setTarget(instance)

    const item: ContentSelectionItem = {
        key: 'modrinth:pending',
        provider: 'modrinth',
        projectId: 'pending',
        providerProjectId: 'pending',
        contentType: 'mod',
        title: 'Pending mod',
        slug: 'pending',
    }
    await expect(selection.add(item)).resolves.toBe(true)

    await expect(selection.installSelected()).resolves.toBe(true)
    expect(showInstall).toHaveBeenCalledTimes(1)
    expect(showInstall).toHaveBeenCalledWith({
        analysis: expect.objectContaining({ prepare: expect.any(Function) }),
        instanceId: 'instance-1',
        instanceName: 'Example',
        selections: [
            {
                id: 'modrinth:pending',
                resource: {
                    kind: 'source',
                    project: { source: 'modrinth', projectId: 'pending' },
                },
                release: { kind: 'automatic' },
                resourceType: 'mod',
                scope: { kind: 'instance' },
            },
        ],
        installDependencies: true,
    })
    expect(selection.items.value.size).toBe(0)
    expect(selection.state.value).toBe('idle')
})

it('resolves pending worlds before any queue and keeps failed items selected', async () => {
    const showInstallSelection = vi.fn()
    const selection = createContentSelection({
        addNotification: vi.fn(),
        handleError: vi.fn(),
        downloadManager: { jobs: ref([]) } as never,
    })
    selection.setTarget(instance)
    selection.setPreviewModal({ showBatch: vi.fn() } as never)
    selection.setResourceInstallPreview({ showInstallSelection } as never)
    await selection.add({
        key: 'curseforge:world',
        provider: 'curseforge',
        projectId: '42',
        providerProjectId: '42',
        contentType: 'world',
        title: 'World',
    })
    await selection.add({
        key: 'modrinth:pending',
        provider: 'modrinth',
        projectId: 'pending',
        providerProjectId: 'pending',
        contentType: 'mod',
        title: 'Mod',
    })
    vi.mocked(getCurseForgeFiles).mockRejectedValueOnce(new Error('source unavailable'))
    const queuedBefore = vi.mocked(queue_content_batch).mock.calls.length
    await expect(selection.installSelected()).resolves.toBe(false)
    expect(showInstallSelection).not.toHaveBeenCalled()
    expect(queue_content_batch).toHaveBeenCalledTimes(queuedBefore)
    expect(selection.items.value.size).toBe(2)
    expect(selection.state.value).toBe('error')
})

it('queues only confirmed roots in a mixed batch and clears excluded worlds', async () => {
    const showInstallSelection = vi
        .fn()
        .mockResolvedValue({ job: { job_id: 'resource-job' }, activeRoots: ['modrinth:a'] })
    const showBatch = vi
        .fn()
        .mockResolvedValue({ primaryKeys: ['curseforge:world'], approvedIds: [] })
    const selection = createContentSelection({
        addNotification: vi.fn(),
        handleError: vi.fn(),
        downloadManager: { jobs: ref([]) } as never,
    })
    selection.setTarget(instance)
    selection.setPreviewModal({ showBatch } as never)
    selection.setResourceInstallPreview({ showInstallSelection } as never)
    for (const id of ['a', 'b']) {
        await selection.add({
            key: `modrinth:${id}`,
            provider: 'modrinth',
            projectId: id,
            providerProjectId: id,
            contentType: 'mod',
            title: id,
        })
    }
    for (const id of ['world', 'excluded']) {
        await selection.add({
            key: `curseforge:${id}`,
            provider: 'curseforge',
            projectId: id === 'world' ? '42' : '43',
            providerProjectId: id === 'world' ? '42' : '43',
            contentType: 'world',
            title: id,
        })
    }
    vi.mocked(getCurseForgeFiles).mockResolvedValue({
        files: [{ id: 123, isAvailable: true, hashes: [] }],
        pagination: {},
    } as never)
    vi.mocked(queue_content_batch).mockResolvedValueOnce({ job_id: 'world-job' } as never)
    await expect(selection.installSelected()).resolves.toBe(true)
    expect(selection.items.value.size).toBe(0)
    expect(selection.isInstalling('modrinth:a')).toBe(true)
    expect(selection.isInstalling('modrinth:b')).toBe(false)
    expect(selection.isInstalling('curseforge:world')).toBe(true)
    expect(selection.isInstalling('curseforge:excluded')).toBe(false)
    expect(queue_content_batch).toHaveBeenLastCalledWith(
        'instance-1',
        [
            {
                type: 'curse_forge_world',
                request: { instanceId: 'instance-1', projectId: 42, fileId: 123 },
            },
        ],
        expect.anything(),
    )
})

it('changing the cart during world review cannot install an unrelated resource snapshot', async () => {
    let finish!: (value: { primaryKeys: string[] }) => void
    const showBatch = vi.fn(
        () =>
            new Promise((resolve) => {
                finish = resolve
            }),
    )
    const showInstallSelection = vi.fn()
    const selection = createContentSelection({
        addNotification: vi.fn(),
        handleError: vi.fn(),
        downloadManager: { jobs: ref([]) } as never,
    })
    selection.setTarget(instance)
    selection.setPreviewModal({ showBatch } as never)
    selection.setResourceInstallPreview({ showInstallSelection } as never)
    await selection.add({
        key: 'curseforge:world',
        provider: 'curseforge',
        projectId: '42',
        providerProjectId: '42',
        contentType: 'world',
        title: 'World',
        versionId: '123',
        sha1: 'a'.repeat(40),
    })
    await selection.add({
        key: 'modrinth:first',
        provider: 'modrinth',
        projectId: 'first',
        providerProjectId: 'first',
        contentType: 'mod',
        title: 'First',
    })
    const pending = selection.installSelected()
    await vi.waitFor(() => expect(showBatch).toHaveBeenCalled())
    await selection.add({
        key: 'modrinth:late',
        provider: 'modrinth',
        projectId: 'late',
        providerProjectId: 'late',
        contentType: 'mod',
        title: 'Late addition',
    })
    finish({ primaryKeys: ['curseforge:world'] })
    await expect(pending).resolves.toBe(false)
    expect(showInstallSelection).not.toHaveBeenCalled()
    expect(selection.items.value.size).toBe(3)
    selection.clear()
})

it('preserves the unqueued world when its batch fails after resource queue acceptance', async () => {
    const selection = createContentSelection({
        addNotification: vi.fn(),
        handleError: vi.fn(),
        downloadManager: { jobs: ref([]) } as never,
    })
    selection.setTarget(instance)
    selection.setPreviewModal({
        showBatch: vi
            .fn()
            .mockResolvedValue({ primaryKeys: ['curseforge:world'], approvedIds: [] }),
    } as never)
    selection.setResourceInstallPreview({
        showInstallSelection: vi
            .fn()
            .mockResolvedValue({ job: { job_id: 'resource-job' }, activeRoots: ['modrinth:a'] }),
    } as never)
    await selection.add({
        key: 'modrinth:a',
        provider: 'modrinth',
        projectId: 'a',
        providerProjectId: 'a',
        contentType: 'mod',
        title: 'Mod',
    })
    await selection.add({
        key: 'curseforge:world',
        provider: 'curseforge',
        projectId: '42',
        providerProjectId: '42',
        contentType: 'world',
        title: 'World',
    })
    vi.mocked(getCurseForgeFiles).mockResolvedValueOnce({
        files: [{ id: 123, isAvailable: true, hashes: [] }],
        pagination: {},
    } as never)
    vi.mocked(queue_content_batch).mockRejectedValueOnce(new Error('queue failed'))
    await expect(selection.installSelected()).resolves.toBe(false)
    expect([...selection.items.value.keys()]).toEqual(['curseforge:world'])
    expect([...selection.errorKeys.value]).toEqual(['curseforge:world'])
    expect(selection.isInstalling('modrinth:a')).toBe(true)
})

it('reports accepted jobs even when only a newly added optional root remains', async () => {
    const selection = createContentSelection({
        addNotification: vi.fn(),
        handleError: vi.fn(),
        downloadManager: { jobs: ref([]) } as never,
    })
    selection.setTarget(instance)
    selection.setResourceInstallPreview({
        showInstallSelection: vi.fn().mockResolvedValue({
            job: { job_id: 'optional-job' },
            activeRoots: ['added-optional'],
        }),
    } as never)
    await selection.add({
        key: 'modrinth:a',
        provider: 'modrinth',
        projectId: 'a',
        providerProjectId: 'a',
        contentType: 'mod',
        title: 'Mod',
    })
    await expect(selection.installSelected()).resolves.toBe(true)
    expect(selection.items.value.size).toBe(0)
    expect(selection.isInstalling('modrinth:a')).toBe(false)
})

it('ignores instance refreshes whose page context expired before the response', async () => {
    let resolve!: (value: GameInstance[]) => void
    vi.mocked(listInstances).mockImplementationOnce(
        () =>
            new Promise((done) => {
                resolve = done
            }),
    )
    const selection = createContentSelection({
        addNotification: vi.fn(),
        handleError: vi.fn(),
        downloadManager: { jobs: ref([]) } as never,
    })
    selection.setTarget(instance)
    let current = true
    const pending = selection.refreshInstances('late-target', () => current)
    current = false
    resolve([{ ...instance, id: 'late-target', install_stage: 'installed' }])
    await pending
    expect(selection.targetInstance.value?.id).toBe(instance.id)
    expect(selection.instances.value).toEqual([])
})

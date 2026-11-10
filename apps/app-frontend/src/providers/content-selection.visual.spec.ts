import { expect, it, vi } from 'vitest'
import { effectScope, nextTick, ref } from 'vue'

import { get_project_many, get_version_many } from '@/helpers/cache.js'
import {
    compareContentIdentities,
    contentIdentityInputsFromSnapshot,
    resolveContentIdentities,
} from '@/helpers/content-identity'
import { getCurseForgeFiles, getCurseForgeProjects } from '@/helpers/curseforge'
import {
    get_content_snapshot,
    type InstanceContentSnapshot,
    list as listInstances,
    queue_content_batch,
} from '@/helpers/instance'
import type { GameInstance } from '@/helpers/types'
import type { ContentAnalysisReview } from '@/providers/content-analysis'

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
        let finish!: (value: Awaited<ReturnType<typeof get_version_many>>) => void
        vi.mocked(get_version_many).mockImplementationOnce(
            () =>
                new Promise<Awaited<ReturnType<typeof get_version_many>>>((resolve) => {
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

it('does not let an older snapshot overwrite a refreshed installed identity', async () => {
    let finishOld!: (value: never) => void
    vi.mocked(get_content_snapshot)
        .mockImplementationOnce(
            () =>
                new Promise((resolve) => {
                    finishOld = resolve
                }),
        )
        .mockResolvedValueOnce({ items: [{ tag: 'new' }] } as never)
    vi.mocked(get_project_many).mockResolvedValue([] as never)
    vi.mocked(getCurseForgeProjects).mockResolvedValue([] as never)
    vi.mocked(contentIdentityInputsFromSnapshot).mockImplementation((items) => [
        {
            provider: 'modrinth',
            projectId: (items[0] as unknown as { tag: string }).tag,
            contentType: 'mod',
        },
    ])
    vi.mocked(resolveContentIdentities).mockImplementation(async (inputs) => inputs)
    const selection = createContentSelection({
        addNotification: vi.fn(),
        handleError: vi.fn(),
        downloadManager: { jobs: ref([]) } as never,
    })
    selection.setTarget(instance)
    const old = selection.refreshInstalledIdentities()
    await selection.refreshInstalledIdentities()
    expect(selection.isInstalledIdentity('modrinth', 'new')).toBe(true)
    finishOld({ items: [{ tag: 'old' }] } as never)
    await old
    expect(selection.isInstalledIdentity('modrinth', 'new')).toBe(true)
    expect(selection.isInstalledIdentity('modrinth', 'old')).toBe(false)
    vi.mocked(contentIdentityInputsFromSnapshot).mockReset().mockReturnValue([])
    vi.mocked(resolveContentIdentities)
        .mockReset()
        .mockReturnValue([] as never)
})

function selectedMod(versionId?: string): ContentSelectionItem {
    return {
        key: 'modrinth:test',
        provider: 'modrinth',
        projectId: 'test',
        providerProjectId: 'test',
        contentType: 'mod',
        title: 'Test',
        versionId,
        identity: { provider: 'modrinth', projectId: 'test', contentType: 'mod', sha1: 'same' },
    }
}

function identitySnapshot(projectId = 'installed', hash = 'same'): InstanceContentSnapshot {
    return {
        instanceId: instance.id,
        revision: 1,
        pack: null,
        pendingManualDownloads: [],
        warnings: [],
        items: [
            {
                provider: 'curseforge',
                providerProjectId: projectId,
                projectType: 'mod',
                expectedRelativePath: 'mods/test.jar',
                materializationState: 'present',
                content: { id: hash, file_name: 'test.jar', size: 100, provider_refs: [] },
            },
        ],
    } as unknown as InstanceContentSnapshot
}

it('keeps first and unchanged snapshots, recognition and history changes out of an open review', async () => {
    const scope = effectScope()
    const jobs = ref([{ job_id: 'history', status: 'succeeded', instance_id: instance.id }])
    const selection = scope.run(() =>
        createContentSelection({
            addNotification: vi.fn(),
            handleError: vi.fn(),
            downloadManager: { jobs } as never,
        }),
    )!
    let review!: ContentAnalysisReview
    let finish!: (value: null) => void
    selection.setResourceInstallPreview({
        showInstallSelection: (input: { analysis: ContentAnalysisReview }) => {
            review = input.analysis
            return new Promise((resolve) => {
                finish = resolve
            })
        },
    } as never)
    selection.setTarget(instance)
    await selection.add(selectedMod())
    const pending = selection.installSelected()
    vi.mocked(get_project_many).mockResolvedValue([] as never)
    vi.mocked(getCurseForgeProjects).mockResolvedValue([] as never)
    const snapshot = identitySnapshot()
    try {
        vi.mocked(get_content_snapshot).mockResolvedValue(snapshot)
        await selection.refreshInstalledIdentities()
        expect(review.signal.aborted).toBe(false)
        snapshot.revision++
        snapshot.items[0].artifactRef = {
            release: { project: { source: 'curseforge', projectId: 'installed' }, releaseId: 'v1' },
            artifactId: 'artifact',
        }
        snapshot.items[0].overrideKind = 'version'
        await selection.refreshInstalledIdentities()
        jobs.value = []
        await nextTick()
        expect(review.signal.aborted).toBe(false)
        jobs.value = [{ job_id: 'new-job', status: 'running', instance_id: instance.id }]
        await nextTick()
        const reads = vi.mocked(get_content_snapshot).mock.calls.length
        jobs.value = [{ ...jobs.value[0], status: 'succeeded' }]
        await vi.waitFor(() => expect(get_content_snapshot).toHaveBeenCalledTimes(reads + 1))
        expect(review.signal.aborted).toBe(false)
        selection.setTarget({ ...instance, game_version: '1.21.2' })
        expect(review.signal.aborted).toBe(true)
    } finally {
        finish(null)
        await pending
        scope.stop()
        vi.mocked(get_content_snapshot).mockResolvedValue(null as never)
    }
})

it('invalidates an open review for changed bytes, paths, removals and completed installs', async () => {
    for (const change of ['hash', 'path', 'remove', 'completed-job']) {
        const scope = effectScope()
        const jobs = ref([{ job_id: 'active', status: 'running', instance_id: instance.id }])
        const selection = scope.run(() =>
            createContentSelection({
                addNotification: vi.fn(),
                handleError: vi.fn(),
                downloadManager: { jobs } as never,
            }),
        )!
        let review!: ContentAnalysisReview
        let finish!: (value: null) => void
        selection.setResourceInstallPreview({
            showInstallSelection: (input: { analysis: ContentAnalysisReview }) => {
                review = input.analysis
                return new Promise((resolve) => {
                    finish = resolve
                })
            },
        } as never)
        selection.setTarget(instance)
        const snapshot = identitySnapshot()
        vi.mocked(get_content_snapshot).mockResolvedValue(snapshot)
        await selection.refreshInstalledIdentities()
        await selection.add(selectedMod())
        const pending = selection.installSelected()
        try {
            if (change === 'path') snapshot.items[0].expectedRelativePath += '.disabled'
            else if (change === 'remove') snapshot.items = []
            else snapshot.items[0].content!.id = 'changed'
            if (change === 'completed-job') {
                jobs.value = [{ ...jobs.value[0], status: 'succeeded' }]
                await vi.waitFor(() => expect(review.signal.aborted).toBe(true))
            } else {
                await selection.refreshInstalledIdentities()
                expect(review.signal.aborted).toBe(true)
            }
        } finally {
            finish(null)
            await pending
            scope.stop()
        }
    }
    vi.mocked(get_content_snapshot).mockResolvedValue(null as never)
})

it('rechecks an addition against a newer identity snapshot instead of accepting an old empty result', async () => {
    const scope = effectScope()
    const addNotification = vi.fn()
    const selection = scope.run(() =>
        createContentSelection({
            addNotification,
            handleError: vi.fn(),
            downloadManager: { jobs: ref([]) } as never,
        }),
    )!
    let finishOld!: (value: InstanceContentSnapshot) => void
    vi.mocked(get_content_snapshot).mockImplementationOnce(
        () =>
            new Promise((resolve) => {
                finishOld = resolve
            }),
    )
    vi.mocked(get_content_snapshot).mockResolvedValue(identitySnapshot())
    vi.mocked(contentIdentityInputsFromSnapshot).mockImplementation((items) =>
        items.map((item) => ({
            provider: item.provider!,
            projectId: item.providerProjectId!,
            contentType: 'mod',
            sha1: item.content!.id,
        })),
    )
    vi.mocked(resolveContentIdentities).mockImplementation(async (inputs) => inputs as never)
    vi.mocked(compareContentIdentities).mockImplementation((candidate, existing) =>
        candidate.sha1 === existing.sha1
            ? { source: 'sha1', confidence: 'exact', identity: existing }
            : null,
    )
    selection.setTarget(instance)
    try {
        const adding = selection.add(selectedMod('version'))
        await vi.waitFor(() => expect(finishOld).toBeDefined())
        await selection.refreshInstalledIdentities()
        finishOld({ ...identitySnapshot(), items: [] })
        expect(await adding).toBe(false)
        expect(addNotification).toHaveBeenCalledOnce()
        expect(selection.items.value.size).toBe(0)
    } finally {
        scope.stop()
        vi.mocked(get_content_snapshot).mockResolvedValue(null as never)
        vi.mocked(contentIdentityInputsFromSnapshot).mockReturnValue([])
        vi.mocked(resolveContentIdentities).mockReturnValue([] as never)
        vi.mocked(compareContentIdentities).mockReturnValue(null)
    }
})

it('preserves successful identities on a failed refresh and retries on the next addition', async () => {
    const scope = effectScope()
    const selection = scope.run(() =>
        createContentSelection({
            addNotification: vi.fn(),
            handleError: vi.fn(),
            downloadManager: { jobs: ref([]) } as never,
        }),
    )!
    vi.mocked(get_content_snapshot).mockResolvedValue(identitySnapshot())
    vi.mocked(contentIdentityInputsFromSnapshot).mockImplementation((items) =>
        items.map((item) => ({
            provider: item.provider!,
            projectId: item.providerProjectId!,
            contentType: 'mod',
        })),
    )
    vi.mocked(resolveContentIdentities).mockImplementation(async (inputs) => inputs as never)
    selection.setTarget(instance)
    try {
        await selection.refreshInstalledIdentities()
        expect(selection.isInstalledIdentity('curseforge', 'installed')).toBe(true)
        vi.mocked(get_content_snapshot).mockRejectedValueOnce(new Error('temporary failure'))
        await selection.refreshInstalledIdentities()
        expect(selection.isInstalledIdentity('curseforge', 'installed')).toBe(true)
        const reads = vi.mocked(get_content_snapshot).mock.calls.length
        await selection.add(selectedMod('version'))
        expect(get_content_snapshot).toHaveBeenCalledTimes(reads + 1)
        expect(selection.isInstalledIdentity('curseforge', 'installed')).toBe(true)
    } finally {
        scope.stop()
        vi.mocked(get_content_snapshot).mockResolvedValue(null as never)
        vi.mocked(contentIdentityInputsFromSnapshot).mockReturnValue([])
        vi.mocked(resolveContentIdentities).mockReturnValue([] as never)
    }
})

it('does not reuse a retired identity request after switching away and back to the same instance', async () => {
    const scope = effectScope()
    const selection = scope.run(() =>
        createContentSelection({
            addNotification: vi.fn(),
            handleError: vi.fn(),
            downloadManager: { jobs: ref([]) } as never,
        }),
    )!
    let finishOld!: (value: InstanceContentSnapshot) => void
    vi.mocked(get_content_snapshot).mockImplementationOnce(
        () =>
            new Promise((resolve) => {
                finishOld = resolve
            }),
    )
    vi.mocked(get_content_snapshot).mockResolvedValue(identitySnapshot('new'))
    vi.mocked(contentIdentityInputsFromSnapshot).mockImplementation((items) =>
        items.map((item) => ({
            provider: item.provider!,
            projectId: item.providerProjectId!,
            contentType: 'mod',
        })),
    )
    vi.mocked(resolveContentIdentities).mockImplementation(async (inputs) => inputs as never)
    selection.setTarget(instance)
    const old = selection.refreshInstalledIdentities()
    try {
        selection.setTarget({ ...instance, id: 'other' })
        selection.setTarget(instance)
        await selection.add(selectedMod('version'))
        expect(selection.isInstalledIdentity('curseforge', 'new')).toBe(true)
        finishOld(identitySnapshot('old'))
        await old
        expect(selection.isInstalledIdentity('curseforge', 'new')).toBe(true)
        expect(selection.isInstalledIdentity('curseforge', 'old')).toBe(false)
    } finally {
        scope.stop()
        vi.mocked(get_content_snapshot).mockResolvedValue(null as never)
        vi.mocked(contentIdentityInputsFromSnapshot).mockReturnValue([])
        vi.mocked(resolveContentIdentities).mockReturnValue([] as never)
    }
})

it('clearing the cart cancels an in-flight version lookup before it can insert a release', async () => {
    const selection = createContentSelection({
        addNotification: vi.fn(),
        handleError: vi.fn(),
        downloadManager: { jobs: ref([]) } as never,
    })
    selection.setTarget(instance)
    let finish!: (id: string) => void
    const pending = selection.add(
        {
            key: 'modrinth:late-filter',
            provider: 'modrinth',
            providerProjectId: 'late-filter',
            projectId: 'late-filter',
            contentType: 'mod',
            title: 'Late',
            identity: { title: 'Late' } as never,
        },
        () =>
            new Promise((resolve) => {
                finish = resolve
            }),
    )
    selection.clear()
    finish('filtered-release')
    expect(await pending).toBe(false)
    expect(selection.items.value.size).toBe(0)
})
it('independent concurrent version lookups both commit without canceling one another', async () => {
    const selection = createContentSelection({
        addNotification: vi.fn(),
        handleError: vi.fn(),
        downloadManager: { jobs: ref([]) } as never,
    })
    selection.setTarget(instance)
    const finish: Array<(id: string) => void> = []
    const pending = ['one', 'two'].map((id) =>
        selection.add(
            {
                key: `modrinth:${id}`,
                provider: 'modrinth',
                providerProjectId: id,
                projectId: id,
                contentType: 'mod',
                title: id,
                identity: { title: id } as never,
            },
            () =>
                new Promise((resolve) => {
                    finish.push(resolve)
                }),
        ),
    )
    finish[1]('two-release')
    finish[0]('one-release')
    expect(await Promise.all(pending)).toEqual([true, true])
    expect([...selection.items.value.values()].map((item) => item.versionId).sort()).toEqual([
        'one-release',
        'two-release',
    ])
})

it('does not surface a late version lookup failure after its selection was cleared', async () => {
    const selection = createContentSelection({
        addNotification: vi.fn(),
        handleError: vi.fn(),
        downloadManager: { jobs: ref([]) } as never,
    })
    selection.setTarget(instance)
    let fail!: (error: Error) => void
    const pending = selection.add(
        {
            key: 'modrinth:failed-filter',
            provider: 'modrinth',
            providerProjectId: 'failed-filter',
            projectId: 'failed-filter',
            contentType: 'mod',
            title: 'Late failure',
        },
        () =>
            new Promise((_resolve, reject) => {
                fail = reject
            }),
    )
    selection.clear()
    fail(new Error('offline'))
    expect(await pending).toBe(false)
    expect(selection.items.value.size).toBe(0)
})

it('keeps the last requested version when two lookups for the same project finish in reverse order', async () => {
    const selection = createContentSelection({
        addNotification: vi.fn(),
        handleError: vi.fn(),
        downloadManager: { jobs: ref([]) } as never,
    })
    selection.setTarget(instance)
    const finish: Array<(id: string) => void> = []
    const item = {
        key: 'modrinth:same',
        provider: 'modrinth' as const,
        providerProjectId: 'same',
        projectId: 'same',
        contentType: 'mod' as const,
        title: 'Same',
        identity: { title: 'Same' } as never,
    }
    const first = selection.add(
        item,
        () =>
            new Promise((resolve) => {
                finish.push(resolve)
            }),
    )
    const latest = selection.add(
        item,
        () =>
            new Promise((resolve) => {
                finish.push(resolve)
            }),
    )
    finish[1]('latest-release')
    expect(await latest).toBe(true)
    finish[0]('older-release')
    expect(await first).toBe(false)
    expect(selection.items.value.get(item.key)?.versionId).toBe('latest-release')
})

it.each([false, true])(
    'removing a project cancels only that project lookup (remove pending: %s)',
    async (removePending) => {
        const selection = createContentSelection({
            addNotification: vi.fn(),
            handleError: vi.fn(),
            downloadManager: { jobs: ref([]) } as never,
        })
        selection.setTarget(instance)
        const item = (id: string) => ({
            key: `modrinth:${id}`,
            provider: 'modrinth' as const,
            providerProjectId: id,
            projectId: id,
            contentType: 'mod' as const,
            title: id,
            identity: { title: id } as never,
        })
        const selected = item('selected')
        expect(await selection.add(selected)).toBe(true)
        const pendingItem = item('pending')
        let finish!: (id: string) => void
        let current!: () => boolean
        const pending = selection.add(pendingItem, (isCurrent) => {
            current = isCurrent
            return new Promise((resolve) => {
                finish = resolve
            })
        })
        selection.remove(removePending ? pendingItem.key : selected.key)
        expect(current()).toBe(!removePending)
        finish('filtered-release')
        expect(await pending).toBe(!removePending)
        expect(selection.items.value.has(pendingItem.key)).toBe(!removePending)
        expect(selection.items.value.has(selected.key)).toBe(removePending)
        if (!removePending)
            expect(selection.items.value.get(pendingItem.key)?.versionId).toBe('filtered-release')
    },
)

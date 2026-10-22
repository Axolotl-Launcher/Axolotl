import { expect, it, vi } from 'vitest'

import { get_project, get_version_many } from '@/helpers/cache.js'
import { getCurseForgeFile, getCurseForgeFiles, getCurseForgeProject } from '@/helpers/curseforge'
import {
    install_create_instance,
    installJobInstanceId,
    wait_for_install_job,
} from '@/helpers/install'
import { get, get_install_candidates } from '@/helpers/instance'
import { resourceReleaseDocuments, resourceReleases } from '@/helpers/resources'

import { createContentInstall } from './content-install'

vi.mock('@modrinth/ui', () => ({
    createContext: () => [vi.fn(), vi.fn()],
    defineMessages: (messages: unknown) => messages,
    defineMessage: (message: unknown) => message,
    useDebugLogger: () => vi.fn(),
    useVIntl: () => ({
        formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
    }),
}))
vi.mock('@/helpers/resources.ts', async (original) => ({
    ...(await original<typeof import('@/helpers/resources')>()),
    resourceReleases: vi.fn(),
    resourceReleaseDocuments: vi.fn(),
}))
vi.mock('@/store/state', () => ({ useTheming: () => ({ getFeatureFlag: () => false }) }))
vi.mock('@/helpers/analytics', () => ({ trackEvent: vi.fn() }))
vi.mock('@/helpers/events.js', () => ({ instance_listener: vi.fn().mockResolvedValue(() => {}) }))
vi.mock('@/helpers/cache.js', () => ({
    get_project: vi.fn(),
    get_version_many: vi.fn(),
    get_organization: vi.fn(),
    get_team: vi.fn(),
}))
vi.mock('@/helpers/instance', () => ({
    get_install_candidates: vi.fn(),
    get: vi.fn(),
    list: vi.fn(),
    edit: vi.fn(),
}))
vi.mock('@/helpers/instance-icons', () => ({
    getDisplayInstanceIcon: () => ({ url: '', frameless: false }),
}))
vi.mock('@/helpers/tags', () => ({ get_game_versions: vi.fn().mockResolvedValue([]) }))
vi.mock('@/helpers/install', () => ({
    install_create_instance: vi.fn(),
    install_create_modpack_instance: vi.fn(),
    installJobInstanceId: vi.fn(),
    wait_for_install_job: vi.fn(),
}))
vi.mock('@/helpers/curseforge', () => ({
    getCurseForgeFile: vi.fn(),
    getCurseForgeFiles: vi.fn(),
    getCurseForgeProject: vi.fn(),
    getCurseForgeImageUrl: (url: string | undefined) => url,
    getCurseForgeDownloadFailureDetails: () => null,
    installCurseForgeModpack: vi.fn(),
    queueCurseForgeWorld: vi.fn(),
    summarizeCurseForgeInstall: vi.fn(),
}))
vi.mock('./content-install-dep-notifications', () => ({
    createContentInstallDepNotifications: () => ({ notifyInstalledDependencies: vi.fn() }),
}))

function installationContext() {
    const context = createContentInstall({
        router: { push: vi.fn() } as never,
        handleError: vi.fn(),
        addNotification: vi.fn(),
    })
    const modal = { show: vi.fn(), hide: vi.fn(() => context.handleCancel()) }
    context.setContentInstallModal(modal)
    return { context, modal }
}

function project(id: string) {
    return { id, title: id, project_type: 'mod', versions: [`${id}-release`] } as never
}

function release(id: string) {
    return {
        id: `${id}-release`,
        date_published: '2026-01-01',
        game_versions: ['1.20.1'],
        loaders: ['fabric'],
    } as never
}

it('version loading failure closes and settles its own session once', async () => {
    vi.clearAllMocks()
    vi.mocked(get_project).mockResolvedValue(project('failed'))
    vi.mocked(get_version_many).mockRejectedValueOnce(new Error('version request failed'))
    const { context, modal } = installationContext()
    const callback = vi.fn()
    await expect(context.install('failed', null, null, 'test', callback)).rejects.toThrow(
        'version request failed',
    )
    expect(context.loading.value).toBe(false)
    expect(modal.hide).toHaveBeenCalled()
    expect(callback).toHaveBeenCalledOnce()
    context.handleCancel()
    expect(callback).toHaveBeenCalledOnce()
})

it('late project versions cannot overwrite a newer install session', async () => {
    vi.clearAllMocks()
    let finish!: (versions: never[]) => void
    vi.mocked(get_project).mockImplementation(async (id) => project(id))
    vi.mocked(get_version_many).mockImplementation((ids) =>
        ids[0] === 'old-release'
            ? new Promise((resolve) => {
                  finish = resolve
              })
            : Promise.resolve([release('new')]),
    )
    vi.mocked(get_install_candidates).mockImplementation(
        async (id) =>
            [
                {
                    id,
                    name: id,
                    game_version: '1.20.1',
                    loader: 'fabric',
                    installed: false,
                    compatible: true,
                },
            ] as never,
    )
    const { context } = installationContext()
    const oldCallback = vi.fn()
    const newCallback = vi.fn()
    const pending = context.install('old', null, null, 'test', oldCallback)
    await vi.waitFor(() => expect(finish).toBeTypeOf('function'))
    await context.install('new', null, null, 'test', newCallback)
    finish([release('old')])
    await pending
    expect(context.instances.value[0].name).toBe('new')
    expect(context.loading.value).toBe(false)
    expect(oldCallback).toHaveBeenCalledOnce()
    expect(newCallback).not.toHaveBeenCalled()
})

it('creation waiting for Minecraft cannot install or settle a newer project session', async () => {
    vi.clearAllMocks()
    vi.mocked(get_project).mockImplementation(async (id) => project(id))
    vi.mocked(get_version_many).mockImplementation(async (ids) => [
        release(ids[0].replace('-release', '')),
    ])
    vi.mocked(get_install_candidates).mockResolvedValue([])
    vi.mocked(install_create_instance).mockResolvedValue({ job_id: 'creation' } as never)
    vi.mocked(installJobInstanceId).mockReturnValue('created')
    let finish!: () => void
    vi.mocked(wait_for_install_job).mockImplementationOnce(
        () =>
            new Promise((resolve) => {
                finish = () => resolve({} as never)
            }),
    )
    const { context, modal } = installationContext()
    const oldCallback = vi.fn(),
        newCallback = vi.fn()
    const preview = vi.fn()
    context.setResourceInstallPreview({ showInstallSelection: preview } as never)
    await context.install('old', null, null, 'test', oldCallback)
    const creation = context.handleCreateAndInstall({
        name: 'Created',
        iconPath: null,
        iconPreviewUrl: null,
        loader: 'fabric',
        gameVersion: '1.20.1',
    })
    await vi.waitFor(() => expect(finish).toBeTypeOf('function'))
    expect(oldCallback).not.toHaveBeenCalled()
    await context.install('new', null, null, 'test', newCallback)
    const hideCount = modal.hide.mock.calls.length
    finish()
    await creation
    expect(preview).not.toHaveBeenCalled()
    expect(oldCallback).toHaveBeenCalledOnce()
    expect(newCallback).not.toHaveBeenCalled()
    expect(modal.hide).toHaveBeenCalledTimes(hideCount)
})

it.each(['modrinth', 'curseforge'] as const)(
    'cancelled %s content review opens and explains the retained created instance',
    async (source) => {
        vi.clearAllMocks()
        vi.mocked(get_project).mockResolvedValue(project('example'))
        vi.mocked(get_version_many).mockResolvedValue([release('example')])
        vi.mocked(getCurseForgeProject).mockResolvedValue({
            id: 42,
            name: 'Example',
            classId: 6,
            slug: 'example',
            authors: [],
        } as never)
        vi.mocked(getCurseForgeFile).mockResolvedValue({
            id: 7,
            displayName: 'Release',
            fileName: 'example.jar',
            fileDate: '2025-01-01',
            gameVersions: ['1.20.1', 'Fabric'],
            isAvailable: true,
            hashes: [],
        } as never)
        vi.mocked(get_install_candidates).mockResolvedValue([])
        vi.mocked(install_create_instance).mockResolvedValue({ job_id: 'creation' } as never)
        vi.mocked(installJobInstanceId).mockReturnValue('created')
        vi.mocked(wait_for_install_job).mockResolvedValue({} as never)
        const push = vi.fn(),
            notify = vi.fn(),
            callback = vi.fn()
        const context = createContentInstall({
            router: { push } as never,
            handleError: vi.fn(),
            addNotification: notify,
        })
        context.setContentInstallModal({ show: vi.fn(), hide: () => context.handleCancel() })
        context.setResourceInstallPreview({
            showInstallSelection: vi.fn().mockResolvedValue(null),
        } as never)
        if (source === 'curseforge')
            await context.installCurseForge('42', '7', null, 'test', callback)
        else await context.install('example', null, null, 'test', callback)
        await context.handleCreateAndInstall({
            name: 'Created',
            iconPath: null,
            iconPreviewUrl: null,
            loader: 'fabric',
            gameVersion: '1.20.1',
        })
        expect(push).toHaveBeenCalledWith('/instance/created')
        expect(notify).toHaveBeenCalledWith(
            expect.objectContaining({ title: 'Created instance retained' }),
        )
        expect(callback).toHaveBeenCalledOnce()
    },
)

it('cancelled version requests cannot repopulate the install target', async () => {
    vi.clearAllMocks()
    let finish!: (versions: never[]) => void
    vi.mocked(get_project).mockResolvedValue(project('cancelled'))
    vi.mocked(get_version_many).mockImplementationOnce(
        () =>
            new Promise((resolve) => {
                finish = resolve
            }),
    )
    const { context, modal } = installationContext()
    const callback = vi.fn()
    const pending = context.install('cancelled', null, null, 'test', callback)
    await vi.waitFor(() => expect(finish).toBeTypeOf('function'))
    const shows = modal.show.mock.calls.length
    context.handleCancel()
    finish([release('cancelled')])
    await pending
    expect(context.instances.value).toEqual([])
    expect(context.loading.value).toBe(false)
    expect(modal.show).toHaveBeenCalledTimes(shows)
    expect(callback).toHaveBeenCalledOnce()
})

it('fixed CurseForge files are read by ID without loading the latest file page', async () => {
    vi.clearAllMocks()
    vi.mocked(getCurseForgeProject).mockResolvedValue({
        id: 42,
        name: 'CF project',
        classId: 6,
        slug: 'cf-project',
        authors: [],
    } as never)
    vi.mocked(getCurseForgeFile).mockResolvedValue({
        id: 7,
        displayName: 'Old release',
        fileName: 'old.jar',
        fileDate: '2025-01-01',
        gameVersions: ['1.20.1', 'Fabric'],
        isAvailable: true,
        hashes: [],
    } as never)
    vi.mocked(get_install_candidates).mockResolvedValue([])
    const { context } = installationContext()
    await context.installCurseForge('42', '7')
    expect(getCurseForgeFile).toHaveBeenCalledExactlyOnceWith(42, 7)
    expect(getCurseForgeFiles).not.toHaveBeenCalled()
    context.handleCancel()
})

it('closes the install target after queueing without reporting failure or settling the callback twice', async () => {
    vi.clearAllMocks()
    vi.mocked(get_project).mockResolvedValue({
        id: 'abcdefgh',
        title: 'Example',
        project_type: 'mod',
        versions: ['release'],
    })
    vi.mocked(get_version_many).mockResolvedValue([
        {
            id: 'release',
            date_published: '2026-01-01',
            game_versions: ['1.20.1'],
            loaders: ['fabric'],
        },
    ])
    vi.mocked(get_install_candidates).mockResolvedValue([
        {
            id: 'instance',
            name: 'Instance',
            game_version: '1.20.1',
            loader: 'fabric',
            icon_path: null,
            installed: false,
            compatible: true,
        },
    ] as Awaited<ReturnType<typeof get_install_candidates>>)
    const handleError = vi.fn()
    const push = vi.fn()
    const context = createContentInstall({
        router: { push } as unknown as Parameters<typeof createContentInstall>[0]['router'],
        handleError,
        addNotification: vi.fn(),
    })
    const callback = vi.fn()
    const hide = vi.fn(() => context.handleCancel())
    context.setContentInstallModal({ show: vi.fn(), hide })
    const showInstallSelection = vi
        .fn()
        .mockResolvedValue({ job: { job_id: 'job' }, activeRoots: ['modrinth:abcdefgh'] })
    context.setResourceInstallPreview({ showInstallSelection } as unknown as Parameters<
        typeof context.setResourceInstallPreview
    >[0])
    await context.install('abcdefgh', null, null, 'test', callback)
    await context.handleInstallToInstance(context.instances.value[0])
    expect(showInstallSelection).toHaveBeenCalledWith(
        expect.objectContaining({ installDependencies: false }),
    )
    expect(hide).toHaveBeenCalledOnce()
    expect(callback).toHaveBeenCalledExactlyOnceWith('release', ['abcdefgh'])
    expect(handleError).not.toHaveBeenCalled()
    context.handleNavigate(context.instances.value[0])
    expect(push).toHaveBeenCalledWith('/instance/instance')
    expect(callback).toHaveBeenCalledOnce()
    const navigationCallback = vi.fn()
    await context.install('abcdefgh', null, null, 'test', navigationCallback)
    context.handleNavigate(context.instances.value[0])
    expect(navigationCallback).toHaveBeenCalledExactlyOnceWith()
    context.handleCancel()
    expect(navigationCallback).toHaveBeenCalledOnce()
})

it('direct instance installation settles the caller when resource review is cancelled or the instance vanished', async () => {
    vi.mocked(get_project).mockResolvedValue({
        id: 'abcdefgh',
        title: 'Example',
        project_type: 'mod',
        versions: ['release'],
    })
    vi.mocked(get_version_many).mockResolvedValue([
        {
            id: 'release',
            date_published: '2026-01-01',
            game_versions: ['1.20.1'],
            loaders: ['fabric'],
        },
    ] as never)
    vi.mocked(get).mockResolvedValue({
        id: 'instance',
        name: 'Target',
        loader: 'fabric',
        game_version: '1.20.1',
    } as never)
    const context = createContentInstall({
        router: { push: vi.fn() } as never,
        handleError: vi.fn(),
        addNotification: vi.fn(),
    })
    context.setResourceInstallPreview({
        showInstallSelection: vi.fn().mockResolvedValue(null),
    } as never)
    const callback = vi.fn()
    await context.install('abcdefgh', 'release', 'instance', 'test', callback)
    expect(callback).toHaveBeenCalledExactlyOnceWith()
    vi.mocked(get).mockResolvedValue(null)
    const missing = vi.fn()
    await context.install('abcdefgh', 'release', 'missing', 'test', missing)
    expect(missing).toHaveBeenCalledExactlyOnceWith()
})

it('CF install target selection includes compatible files from the second source page', async () => {
    vi.clearAllMocks()
    const actual =
        await vi.importActual<typeof import('@/helpers/curseforge')>('@/helpers/curseforge')
    vi.mocked(getCurseForgeFiles).mockImplementation(actual.getCurseForgeFiles)
    vi.mocked(getCurseForgeProject).mockResolvedValue({
        id: 42,
        name: 'Many releases',
        slug: 'many',
        classId: 6,
        authors: [],
    } as never)
    const page = (ids: number[], cursor: string | null) =>
        ({
            entries: ids.map((id) => ({
                reference: {
                    project: { source: 'curseforge', projectId: '42' },
                    releaseId: String(id),
                },
            })),
            cursor,
            hasMore: cursor !== null,
            total: { value: 51, precision: 'exact' },
            exhaustedSources: [],
            sourceFailures: [],
            requestId: 'test',
        }) as Awaited<ReturnType<typeof resourceReleases>>
    vi.mocked(resourceReleases)
        .mockResolvedValueOnce(
            page(
                Array.from({ length: 50 }, (_, i) => i + 1),
                'next',
            ),
        )
        .mockResolvedValueOnce(page([51], null))
    vi.mocked(resourceReleaseDocuments).mockImplementation(
        async (_source, ids) =>
            ids.map((id) => ({
                protocol: {
                    id: Number(id),
                    modId: 42,
                    isAvailable: true,
                    displayName: id,
                    fileName: `${id}.jar`,
                    releaseType: 1,
                    fileDate: '2026-01-01',
                    gameVersions: Number(id) === 51 ? ['1.20.1', 'Fabric'] : ['1.21.1', 'Forge'],
                    fileLength: 100,
                    hashes: [],
                    dependencies: [],
                },
            })) as Awaited<ReturnType<typeof resourceReleaseDocuments>>,
    )
    vi.mocked(get_install_candidates).mockResolvedValue([
        {
            id: 'old-instance',
            name: 'Old',
            game_version: '1.20.1',
            loader: 'fabric',
            installed: false,
            compatible: true,
        },
    ] as never)
    const { context } = installationContext()
    const showInstallSelection = vi.fn().mockResolvedValue(null)
    context.setResourceInstallPreview({ showInstallSelection } as never)
    await context.installCurseForge('42')
    expect(resourceReleases).toHaveBeenCalledTimes(2)
    expect(get_install_candidates).toHaveBeenCalledWith(
        'curseforge:42',
        'mod',
        expect.arrayContaining([
            expect.objectContaining({ game_version: '1.20.1', loader: 'fabric' }),
        ]),
    )
    await context.handleInstallToInstance(context.instances.value[0])
    expect(showInstallSelection).toHaveBeenCalledWith(
        expect.objectContaining({
            selections: expect.arrayContaining([
                expect.objectContaining({
                    release: {
                        kind: 'release',
                        reference: {
                            project: { source: 'curseforge', projectId: '42' },
                            releaseId: '51',
                        },
                    },
                }),
            ]),
        }),
    )
    context.handleCancel()
})

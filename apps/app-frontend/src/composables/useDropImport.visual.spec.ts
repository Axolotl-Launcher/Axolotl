import { beforeEach, expect, it, vi } from 'vitest'
import { computed, ref } from 'vue'

import { syncConfiguredDirectLinks } from '@/helpers/direct-link-sync'
import {
    classifyDroppedItem,
    extractModMetadata,
    resolveGamedirs,
    scanLauncherInstances,
} from '@/helpers/drop'
import { import_instance } from '@/helpers/import.js'
import { wait_for_install_job } from '@/helpers/install'
import {
    add_project_from_path,
    check_symlink_capability,
    get as getInstance,
} from '@/helpers/instance'

import { type DropImportOptions, useDropImport } from './useDropImport'

const dropCallbacks = vi.hoisted(() => ({
    onBatchStart: undefined as ((paths: string[]) => void) | undefined,
}))

vi.mock('@modrinth/ui', () => ({
    useDebugLogger: () => vi.fn(),
    useGlobalDrop: (options: { onBatchStart: (paths: string[]) => void }) => {
        dropCallbacks.onBatchStart = options.onBatchStart
        return { isDragging: ref(false), isProcessing: ref(false), finishBatch: vi.fn() }
    },
    useInstanceContext: () => ({ isInInstance: ref(false), instanceId: ref(null) }),
    useVIntl: () => ({
        formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
    }),
}))
vi.mock('@/helpers/direct-link-sync', () => ({ syncConfiguredDirectLinks: vi.fn() }))
vi.mock('@/helpers/drop', () => ({
    classifyDroppedItem: vi.fn(),
    classifyDroppedItemWithExtraction: vi.fn(),
    detectFileLock: vi.fn(),
    extractModMetadata: vi.fn(),
    extractZipToTemp: vi.fn(),
    lookupModHash: vi.fn(),
    removeTempDir: vi.fn(),
    resolveGamedirs: vi.fn(),
    scanLauncherInstances: vi.fn(),
}))
vi.mock('@/helpers/import.js', () => ({ import_instance: vi.fn() }))
vi.mock('@/helpers/install', () => ({
    install_create_modpack_instance: vi.fn(),
    install_job_cancel: vi.fn(),
    wait_for_install_job: vi.fn(),
}))
vi.mock('@/helpers/instance', () => ({
    add_project_from_path: vi.fn(),
    check_symlink_capability: vi.fn(),
    get: vi.fn(),
    import_world_save: vi.fn(),
    install_datapack_to_world: vi.fn(),
    list: vi.fn(),
}))
vi.mock('@/helpers/instance-icons', () => ({ getDisplayInstanceIcon: vi.fn() }))

beforeEach(() => {
    vi.resetAllMocks()
    localStorage.removeItem('axolotl-minecraft-directories')
})

function setup() {
    const addNotification = vi.fn(() => ({ id: 'notification' }))
    const flow = useDropImport({
        notificationManager: { addNotification, removeNotification: vi.fn() },
        popupNotificationManager: { addPopupNotification: vi.fn() },
        onSkinsPage: computed(() => false),
        onSchematicWorkshopPage: computed(() => false),
        onSettingsPage: computed(() => false),
        isSchematicFile: () => false,
        fileDrop: vi.fn(),
        trackEvent: vi.fn(),
        router: { push: vi.fn() },
        contentInstall: {
            incompatibilityWarningVersions: ref([]),
            incompatibilityWarningCurrentGameVersion: ref(''),
            incompatibilityWarningCurrentLoader: ref(''),
            incompatibilityWarningProjectType: ref(''),
            incompatibilityWarningProjectName: ref(''),
            incompatibilityWarningMessage: ref(''),
            incompatibilityWarningInstalling: ref(false),
        },
        installModpackFromPath: vi.fn(),
    } as unknown as DropImportOptions)
    const showImportMethods = vi.fn()
    flow.confirmDropModal.value = { show: vi.fn(), hide: vi.fn() }
    flow.symlinkCardsModal.value = { show: showImportMethods, hide: vi.fn() }
    return { flow, showImportMethods, addNotification }
}

it.each([null, { minecraft_version: 'unrecognized declaration', loader: 'fabric' }])(
    'imports a mod with unknown compatibility without a conflict prompt: %j',
    async (metadata) => {
        const { flow, addNotification } = setup()
        const show = vi.fn()
        flow.incompatibilityWarningModal.value = { show }
        flow.pendingInstall.value = { type: 'mod', filePath: 'C:/Downloads/mod.jar' }
        vi.mocked(extractModMetadata).mockResolvedValue(metadata ? JSON.stringify(metadata) : null)
        vi.mocked(getInstance).mockResolvedValue({
            id: 'instance',
            game_version: '1.21.1',
            loader: 'fabric',
        } as Awaited<ReturnType<typeof getInstance>>)

        await flow.handleGenericInstall('instance')

        expect(show).not.toHaveBeenCalled()
        expect(flow.pendingDropIncompatibility.value).toBeNull()
        expect(add_project_from_path).toHaveBeenCalledExactlyOnceWith(
            'instance',
            'C:/Downloads/mod.jar',
            'mod',
            undefined,
        )
        expect(addNotification).toHaveBeenCalledWith(expect.objectContaining({ type: 'success' }))
    },
)

it.each([{ minecraft_version: '1.20.1' }, { loader: 'forge' }])(
    'still prompts for an explicitly incompatible mod while the other dimension is unknown: %j',
    async (metadata) => {
        const { flow } = setup()
        const show = vi.fn()
        flow.incompatibilityWarningModal.value = { show }
        flow.pendingInstall.value = { type: 'mod', filePath: 'C:/Downloads/mod.jar' }
        vi.mocked(extractModMetadata).mockResolvedValue(JSON.stringify(metadata))
        vi.mocked(getInstance).mockResolvedValue({
            id: 'instance',
            game_version: '1.21.1',
            loader: 'fabric',
        } as Awaited<ReturnType<typeof getInstance>>)

        await flow.handleGenericInstall('instance')

        expect(show).toHaveBeenCalledOnce()
        expect(flow.pendingDropIncompatibility.value).toMatchObject({
            filePath: 'C:/Downloads/mod.jar',
            instId: 'instance',
        })
        expect(add_project_from_path).not.toHaveBeenCalled()
    },
)

it.each(['D:\\Minecraft\\.minecraft\\versions\\1.21.1', '/games/.minecraft/versions/1.21.1/'])(
    'opens import methods for the selected version directory %s',
    async (path) => {
        const { flow, showImportMethods, addNotification } = setup()
        flow.dropFilePath.value = path
        flow.dropClassification.value = {
            item_type: 'launcher',
            launcher_type: 'Generic',
            base_path: path,
        }
        vi.mocked(scanLauncherInstances).mockResolvedValue([
            {
                launcherType: 'Generic',
                launcherName: 'Generic',
                instances: [{ name: '1.21.1', path, version: '1.21.1', loader: 'fabric' }],
            },
        ])

        await flow.handleDropConfirm('instance')

        expect(scanLauncherInstances).toHaveBeenCalledWith('Generic', path)
        expect(resolveGamedirs).not.toHaveBeenCalled()
        expect(check_symlink_capability).not.toHaveBeenCalled()
        expect(syncConfiguredDirectLinks).not.toHaveBeenCalled()
        expect(showImportMethods).toHaveBeenCalledWith(
            expect.objectContaining({
                instances: [expect.objectContaining({ name: '1.21.1', path })],
            }),
        )
        expect(addNotification).not.toHaveBeenCalled()
        expect(flow.scanningInstances.value).toBe(false)

        vi.mocked(import_instance).mockResolvedValue({ job_id: 'import-job' })
        vi.mocked(wait_for_install_job).mockResolvedValue(undefined)
        await flow.onSymlinkMethodConfirmed(false)
        expect(import_instance).toHaveBeenCalledWith(
            'Generic',
            path,
            '1.21.1',
            false,
            path,
            undefined,
            undefined,
            undefined,
            null,
        )
        expect(wait_for_install_job).toHaveBeenCalledWith('import-job')
        expect(addNotification).toHaveBeenCalledWith(expect.objectContaining({ type: 'success' }))
    },
)

it('imports a chosen instance candidate without falling through to content installation', async () => {
    const { flow, showImportMethods } = setup()
    const path = 'D:\\Minecraft\\.minecraft\\versions\\1.21.1'
    flow.dropFilePath.value = path
    flow.dropClassification.value = {
        item_type: 'multiple',
        file_path: path,
        choices: [{ itemType: 'instance' }, { itemType: 'world_save' }],
    }
    vi.mocked(scanLauncherInstances).mockResolvedValue([
        {
            launcherType: 'Generic',
            launcherName: 'Generic',
            instances: [{ name: '1.21.1', path, version: '1.21.1', loader: '' }],
        },
    ])

    await flow.handleDropConfirm('instance')

    expect(scanLauncherInstances).toHaveBeenCalledWith('Generic', path)
    expect(showImportMethods).toHaveBeenCalledOnce()
})

it('uses copy for new GUI imports even if an old method choice still carries link settings', async () => {
    const { flow } = setup()
    const path = 'D:\\Minecraft\\.minecraft\\versions\\1.21.1'
    flow.dropFilePath.value = path
    flow.dropClassification.value = {
        item_type: 'launcher',
        launcher_type: 'Generic',
        base_path: path,
    }
    vi.mocked(scanLauncherInstances).mockResolvedValue([
        {
            launcherType: 'Generic',
            launcherName: 'Generic',
            instances: [{ name: '1.21.1', path, version: '1.21.1', loader: '' }],
        },
    ])
    vi.mocked(import_instance).mockResolvedValue({ job_id: 'copy-job' })

    await flow.handleDropConfirm('instance')
    await flow.onSymlinkMethodConfirmed([
        { instanceName: '1.21.1', instancePath: path, symlink: true, gameDirOverride: path },
    ])

    expect(import_instance).toHaveBeenCalledWith(
        'Generic',
        path,
        '1.21.1',
        false,
        path,
        undefined,
        undefined,
        undefined,
        null,
    )
})

it('shows a scan error when the selected version cannot be read', async () => {
    const { flow, showImportMethods, addNotification } = setup()
    const path = 'D:\\Minecraft\\.minecraft\\versions\\1.21.1'
    flow.dropFilePath.value = path
    flow.dropClassification.value = {
        item_type: 'launcher',
        launcher_type: 'Generic',
        base_path: path,
    }
    vi.mocked(scanLauncherInstances).mockRejectedValue(new Error('Cannot read version directory'))

    await flow.handleDropConfirm('instance')

    expect(showImportMethods).not.toHaveBeenCalled()
    expect(addNotification).toHaveBeenCalledWith(
        expect.objectContaining({ type: 'error', text: 'Cannot read version directory' }),
    )
    expect(flow.scanningInstances.value).toBe(false)
})

it.each(['instance'])(
    'converts a Minecraft root through %s without adding a watched directory',
    async (type) => {
        const { flow, showImportMethods } = setup()
        const path = 'D:\\Minecraft\\.minecraft'
        flow.dropFilePath.value = path
        flow.dropClassification.value = {
            item_type: 'launcher',
            launcher_type: 'Generic',
            base_path: path,
        }
        vi.mocked(scanLauncherInstances).mockResolvedValue([
            {
                launcherType: 'Generic',
                launcherName: 'Generic',
                instances: [
                    {
                        name: '1.21.1',
                        path: `${path}\\versions\\1.21.1`,
                        version: '1.21.1',
                        loader: '',
                    },
                ],
            },
        ])

        await flow.handleDropConfirm(type)

        expect(scanLauncherInstances).toHaveBeenCalledWith('Generic', path)
        expect(resolveGamedirs).not.toHaveBeenCalled()
        expect(syncConfiguredDirectLinks).not.toHaveBeenCalled()
        expect(showImportMethods).toHaveBeenCalledOnce()
        vi.mocked(import_instance).mockResolvedValue({ job_id: 'root-import-job' })
        await flow.onSymlinkMethodConfirmed(false)
        expect(import_instance).toHaveBeenCalledWith(
            'Generic',
            path,
            '1.21.1',
            false,
            `${path}\\versions\\1.21.1`,
            undefined,
            undefined,
            undefined,
            null,
        )
        expect(wait_for_install_job).toHaveBeenCalledWith('root-import-job')
    },
)

it('adds the root only after accepting the prompt and preserves configured root modes', async () => {
    const { flow, showImportMethods, addNotification } = setup()
    const path = 'D:\\Minecraft\\.minecraft'
    flow.dropFilePath.value = path
    flow.dropClassification.value = {
        item_type: 'launcher',
        launcher_type: 'Generic',
        base_path: path,
    }
    const existing = { path: 'D:\\Existing\\.minecraft', mode: 'shared' }
    localStorage.setItem('axolotl-minecraft-directories', JSON.stringify([existing]))
    vi.mocked(syncConfiguredDirectLinks).mockResolvedValue({
        imported: 1,
        updated: 0,
        removed: 0,
        missing: 0,
        errors: [],
    })

    await flow.handleDropConfirm('dot_minecraft')

    expect(syncConfiguredDirectLinks).toHaveBeenCalledWith([existing, { path, mode: 'automatic' }])
    expect(JSON.parse(localStorage.getItem('axolotl-minecraft-directories')!)).toEqual([
        existing,
        { path, mode: 'automatic' },
    ])
    expect(scanLauncherInstances).not.toHaveBeenCalled()
    expect(import_instance).not.toHaveBeenCalled()
    expect(showImportMethods).not.toHaveBeenCalled()
    expect(addNotification).toHaveBeenCalledWith(expect.objectContaining({ type: 'success' }))
})

it('reports failed folder additions without saving the directory or reporting success', async () => {
    const { flow, addNotification } = setup()
    flow.dropFilePath.value = 'D:\\Minecraft\\.minecraft'
    vi.mocked(syncConfiguredDirectLinks).mockRejectedValue(new Error('Cannot read game directory'))

    await flow.handleDropConfirm('dot_minecraft')

    expect(localStorage.getItem('axolotl-minecraft-directories')).toBeNull()
    expect(addNotification).toHaveBeenCalledWith(
        expect.objectContaining({ type: 'error', text: 'Cannot read game directory' }),
    )
    expect(flow.scanningInstances.value).toBe(false)
})

it('expands batch Minecraft roots and version directories into native import candidates', async () => {
    const { flow, showImportMethods } = setup()
    const paths = ['D:\\Minecraft\\.minecraft', 'D:\\Other\\.minecraft\\versions\\1.21.1']
    vi.mocked(classifyDroppedItem).mockImplementation(async (path) => ({
        item_type: 'launcher',
        launcher_type: 'Generic',
        base_path: path,
    }))
    vi.mocked(scanLauncherInstances).mockImplementation(async (_, path) => [
        {
            launcherType: 'Generic',
            launcherName: 'Generic',
            instances: [
                {
                    name: '1.21.1',
                    path: path.endsWith('.minecraft') ? `${path}\\versions\\1.21.1` : path,
                    version: '1.21.1',
                    loader: '',
                },
            ],
        },
    ])

    dropCallbacks.onBatchStart!(paths)
    await vi.waitFor(() => expect(showImportMethods).toHaveBeenCalledOnce())

    expect(scanLauncherInstances).toHaveBeenCalledTimes(2)
    expect(resolveGamedirs).not.toHaveBeenCalled()
    expect(syncConfiguredDirectLinks).not.toHaveBeenCalled()
    expect(flow.batchItems.value.filter((item) => item.itemType === 'instance')).toHaveLength(2)
    expect(showImportMethods).toHaveBeenCalledOnce()
    flow.cancelBatchScan()
})

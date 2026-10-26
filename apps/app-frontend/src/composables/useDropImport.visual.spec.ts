import { afterEach, expect, it, vi } from 'vitest'
import { effectScope, ref } from 'vue'

import { type DropImportOptions, useDropImport } from './useDropImport'

const fixture = vi.hoisted(() => ({ classify: vi.fn(), extract: vi.fn() }))
vi.mock('@modrinth/ui', async () => {
    const { ref } = await import('vue')
    return {
        useDebugLogger: () => () => {},
        useVIntl: () => ({
            formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
        }),
        useInstanceContext: () => ({ isInInstance: ref(false), instanceId: ref(null) }),
        useGlobalDrop: () => ({
            isDragging: ref(false),
            isProcessing: ref(false),
            finishBatch: vi.fn(),
        }),
    }
})
vi.mock('@/helpers/drop', () => ({
    classifyDroppedItem: fixture.classify,
    classifyDroppedItemWithExtraction: fixture.extract,
    detectFileLock: vi.fn(),
    extractModMetadata: vi.fn(),
    extractZipToTemp: vi.fn(),
    lookupModHash: vi.fn(),
    removeTempDir: vi.fn(),
    resolveGamedirs: vi.fn(),
    scanLauncherInstances: vi.fn(),
}))
vi.mock('@/helpers/direct-link-sync', () => ({ syncConfiguredDirectLinks: vi.fn() }))
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
vi.mock('@/helpers/version-compatibility', () => ({ assessLocalModCompatibility: vi.fn() }))

const scopes: ReturnType<typeof effectScope>[] = []
afterEach(() => {
    scopes.splice(0).forEach((scope) => scope.stop())
    vi.clearAllMocks()
})

it.each(['nested', 'forced'] as const)(
    'keeps alternative import choices after %s analysis finds a server',
    async (kind) => {
        const addPopupNotification = vi.fn<(notification: unknown) => { id: string }>(() => ({
            id: 'prompt',
        }))
        const options = {
            notificationManager: {
                addNotification: vi.fn(() => ({ id: 'notice' })),
                removeNotification: vi.fn(),
            },
            popupNotificationManager: { addPopupNotification, removeNotification: vi.fn() },
            contentInstall: {},
            fileDrop: vi.fn(),
            installModpackFromPath: vi.fn(),
            onSkinsPage: ref(false),
            onSchematicWorkshopPage: ref(false),
            onSettingsPage: ref(false),
            isSchematicFile: () => false,
            trackEvent: vi.fn(),
            router: { push: vi.fn() },
        } as unknown as DropImportOptions
        const scope = effectScope()
        scopes.push(scope)
        const flow = scope.run(() => useDropImport(options))!
        const showChoices = vi.fn()
        const showServer = vi.fn()
        flow.confirmDropModal.value = { show: showChoices, hide: vi.fn() }
        flow.serverImportModal.value = { show: showServer }
        flow.dropFilePath.value = '/fixture/server.zip'
        const result = { item_type: 'server', file_path: '/fixture/server.zip' }
        fixture.classify.mockResolvedValue(result)
        fixture.extract.mockResolvedValue(result)
        if (kind === 'nested')
            flow.showNestedUnpackPrompt({ item_type: 'unknown', reason: 'nested archives' })
        else flow.showForceAnalysisPrompt({ item_type: 'unknown', reason: 'analysis required' })
        const prompt = addPopupNotification.mock.calls[0][0] as {
            buttons: Array<{ action: () => Promise<void> }>
        }
        await prompt.buttons[0].action()
        expect(showChoices).toHaveBeenCalledOnce()
        expect(showServer).not.toHaveBeenCalled()
        expect(flow.dropClassification.value).toEqual(result)
    },
)

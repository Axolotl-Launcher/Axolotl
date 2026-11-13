import type { Labrinth } from '@modrinth/api-client'
import type { InstallationSettingsContext } from '@modrinth/ui'
import { QueryClient, VueQueryPlugin } from '@tanstack/vue-query'
import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import { type Ref, ref } from 'vue'

import type { GameInstance } from '@/helpers/types'

import InstallationSettings from './InstallationSettings.vue'

const state = vi.hoisted(() => ({
    instance: undefined as unknown as Ref<GameInstance>,
    info: undefined as unknown,
    context: undefined as unknown as InstallationSettingsContext,
    updateModrinth: vi.fn(),
    updateCurseForge: vi.fn(),
}))

vi.mock('@/providers/instance-settings', () => ({
    injectInstanceSettings: () => ({
        instance: state.instance,
        offline: false,
        isMinecraftServer: ref(false),
        onUnlinked: vi.fn(),
        closeModal: vi.fn(),
    }),
}))
vi.mock('@/store/state', () => ({ useTheming: () => ({ getFeatureFlag: () => false }) }))
vi.mock('@/helpers/analytics', () => ({ trackEvent: vi.fn() }))
vi.mock('@/helpers/tags', () => ({ get_game_versions: async () => [] }))
vi.mock('@/helpers/metadata', () => ({ get_loader_versions: async () => ({ versions: [] }) }))
vi.mock('@/helpers/cache', () => ({ get_project_versions: async () => [], get_version: vi.fn() }))
vi.mock('@/helpers/curseforge', () => ({
    updateManagedCurseForgeModpack: state.updateCurseForge,
}))
vi.mock('@/helpers/install', () => ({
    install_duplicate_instance: vi.fn(),
    install_existing_instance: vi.fn(),
    install_pack_to_existing_instance: vi.fn(),
    installJobInstanceId: vi.fn(),
    wait_for_install_job: vi.fn(),
}))
vi.mock('@/helpers/instance', () => ({
    get_linked_modpack_info: async () => state.info,
    edit: vi.fn(),
    list: async () => [],
    update_managed_modrinth_version: state.updateModrinth,
    update_repair_modrinth: vi.fn(),
}))
vi.mock('@/components/ui/SymlinkInstanceWarning.vue', () => ({ default: { render: () => null } }))
vi.mock('@modrinth/ui', () => ({
    InstallationSettingsLayout: { render: () => null },
    commonMessages: {},
    defineMessages: (messages: unknown) => messages,
    formatLoaderLabel: (loader: string) => loader,
    injectFilePicker: () => ({}),
    injectNotificationManager: () => ({ handleError: vi.fn() }),
    instanceInstallablePlatforms: [],
    loaderSupportState: () => 'supported',
    loaderVersionsForGameVersion: () => [],
    provideAppBackup: vi.fn(),
    provideInstallationSettings: (context: InstallationSettingsContext) =>
        (state.context = context),
    scopedLoaderMetadataQueryKey: (...args: string[]) => args,
    useDebugLogger: () => vi.fn(),
    useVIntl: () => ({
        formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
    }),
}))

const cleanup: (() => void)[] = []
afterEach(() =>
    cleanup
        .splice(0)
        .reverse()
        .forEach((dispose) => dispose()),
)

it.each([
    ['curseforge', true],
    ['curseforge', false],
    ['modrinth', true],
    ['modrinth', false],
] as const)(
    'uses the installed %s identity when update available is %s',
    async (provider, available) => {
        const installed = provider === 'curseforge' ? '9190157' : 'installed83'
        const target = provider === 'curseforge' ? '9200000' : 'latest85'
        state.instance = ref({
            id: 'instance',
            name: 'Pack',
            game_version: '1.21.1',
            loader: 'neoforge',
            loader_version: '21.1.259',
            install_stage: 'installed',
            link: { type: `${provider}_modpack`, project_id: '925200', version_id: installed },
        } as unknown as GameInstance)
        state.info = {
            project: { id: 'display-project', title: 'Pack' },
            // CurseForge display metadata may come from an associated Modrinth release.
            version: { id: 'display-only-modrinth-id', version_number: '8.3' },
            update: available
                ? provider === 'curseforge'
                    ? {
                          provider,
                          current_file_id: Number(installed),
                          target_file_id: Number(target),
                      }
                    : { provider, current_version_id: installed, target_version_id: target }
                : null,
        }
        state.updateModrinth.mockReset().mockResolvedValue(undefined)
        state.updateCurseForge.mockReset().mockResolvedValue(undefined)
        const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
        const wrapper = mount(InstallationSettings, {
            global: { plugins: [[VueQueryPlugin, { queryClient: client }]] },
        })
        cleanup.push(() => {
            wrapper.unmount()
            client.clear()
        })
        await flushPromises()
        await vi.waitFor(() => expect(state.context.modpack.value?.versionNumber).toBe('8.3'))
        expect(state.context.updaterModalProps.value.currentVersionId).toBe(installed)
        await state.context.onModpackVersionConfirm({ id: target } as Labrinth.Versions.v2.Version)
        if (provider === 'curseforge') {
            expect(state.updateCurseForge).toHaveBeenCalledWith('instance', Number(target))
            expect(state.updateModrinth).not.toHaveBeenCalled()
        } else {
            expect(state.updateModrinth).toHaveBeenCalledWith('instance', target)
            expect(state.updateCurseForge).not.toHaveBeenCalled()
        }
        state.instance.value.link!.version_id = target
        expect(state.context.updaterModalProps.value.currentVersionId).toBe(target)
    },
)

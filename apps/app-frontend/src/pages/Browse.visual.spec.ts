import { flushPromises, mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'

import Browse from './Browse.vue'

const api = vi.hoisted(() => ({
    instance: vi.fn().mockResolvedValue(null),
    installedIds: vi.fn().mockResolvedValue([]),
    handleError: vi.fn(),
    capability: vi.fn(),
    categories: vi.fn(),
    gameVersions: vi.fn().mockResolvedValue([]),
    archiveVersions: vi.fn().mockResolvedValue([]),
    planetAvailable: false,
    refresh: vi.fn(),
    saveMemory: vi.fn(),
    type: 'mod',
    source: 'all',
    extraQuery: null as unknown,
    context: null as unknown,
    enabled: null as unknown,
    query: {} as Record<string, string>,
    memory: {
        modpack: {
            filters: [{ type: 'license', option: 'open_source' }],
            toggledGroups: [],
            overriddenProvidedFilterTypes: [],
        },
    } as Record<string, unknown>,
}))
vi.mock('@tauri-apps/api/core', () => ({
    Channel: vi.fn(),
    invoke: vi.fn().mockResolvedValue([]),
    convertFileSrc: (path: string) => path,
}))
vi.mock('vue-router', async () => {
    const { reactive } = await import('vue')
    const route = reactive({
        params: { projectType: 'mod' },
        query: api.query,
        path: '/browse/mod',
        fullPath: '/browse/mod',
    })
    return {
        useRoute: () => route,
        useRouter: () => ({ push: vi.fn(), replace: vi.fn() }),
        onBeforeRouteLeave: vi.fn(),
    }
})
vi.mock('@/helpers/curseforge', () => ({
    getCurseForgeCapability: api.capability,
    getCurseForgeCategories: api.categories,
    getCurseForgeImageUrl: (url: string) => url,
}))
vi.mock('@/helpers/planet-minecraft', () => ({
    planetMinecraftConnectorAvailable: async () => api.planetAvailable,
}))
vi.mock('@/helpers/mcarchive', () => ({ getMcArchiveGameVersions: api.archiveVersions }))
vi.mock('@/helpers/browse-filter-memory', () => ({
    getBrowseFilterMemory: (type: string) => api.memory[type],
    setBrowseFilterMemory: api.saveMemory,
}))
vi.mock('@/helpers/settings.ts', () => ({
    getLastBrowseContentSource: () => api.source,
    getLastBrowseContentDisplayMode: () => 'list',
    isBrowseContentProjectType: () => true,
    setLastBrowseContentProjectType: vi.fn(),
    setLastBrowseContentSource: vi.fn(),
    setLastBrowseContentDisplayMode: vi.fn(),
    update: vi.fn(),
}))
vi.mock('@/store/navigation-return', () => ({
    isBrowseReturnSourcePath: () => false,
    useNavigationReturnStore: () => ({ consumeBrowseReturnSnapshot: () => null }),
}))
vi.mock('@/composables/useNetworkStatus', async () => {
    const { ref } = await import('vue')
    return { useNetworkStatus: () => ({ offline: ref(false) }) }
})
vi.mock('@/composables/browse/use-search-result-translation', async () => {
    const { ref } = await import('vue')
    return {
        useSearchResultTranslation: () => ({
            translationActive: ref(false),
            translationLoading: ref(false),
            toggle: vi.fn(),
        }),
    }
})
vi.mock('@/composables/browse/use-app-server-browse', async () => {
    const { ref } = await import('vue')
    return {
        useAppServerBrowse: () => ({
            serverPings: ref({}),
            contextMenuRef: ref(),
            updateServerHits: vi.fn(),
        }),
    }
})
vi.mock('@/helpers/tags', () => ({
    get_categories: async () => [],
    get_loaders: async () => [],
    get_game_versions: api.gameVersions,
}))
vi.mock('@/helpers/events.js', () => ({ instance_listener: async () => () => {} }))
vi.mock('@/helpers/content-search', () => ({
    resolveMcmodUrl: async () => null,
    translateSearchHitTitles: async (hits: unknown[]) => hits,
}))
vi.mock('@/helpers/instance', () => ({
    get_installed_project_ids: api.installedIds,
    get: api.instance,
    get_projects: async () => [],
    list: async () => [],
    kill: vi.fn(),
}))
vi.mock('@/helpers/worlds', () => ({
    getServerAddress: () => null,
    get_instance_worlds: async () => [],
}))
vi.mock('@/helpers/metadata', () => ({ get_loader_versions: vi.fn() }))
vi.mock('@/composables/instances/use-server-status-query', () => ({
    fetchCachedServerStatus: vi.fn(),
    getFreshCachedServerStatus: vi.fn(),
}))
vi.mock('@/helpers/content-favorites', () => ({ isFavoriteContentType: () => false }))
vi.mock('@/helpers/process', () => ({ get_by_instance_id: async () => [] }))
vi.mock('@/helpers/translation', () => ({
    autoTranslateHintMessages: {},
    getTranslationSettings: async () => ({ enabled: false }),
    getTranslationErrorKind: vi.fn(),
    noteManualTranslateClick: vi.fn(),
    prepareDescription: vi.fn(),
    translateInBatches: vi.fn(),
    validateTranslatedDescription: vi.fn(),
}))
vi.mock('@/i18n.config', async () => ({
    default: { global: { locale: (await import('vue')).ref('en-US') } },
}))
vi.mock('@/store/state.js', () => ({
    useTheming: () => ({ featureFlags: {}, getFeatureFlag: () => false }),
}))
vi.mock('@/store/breadcrumbs', () => ({
    useBreadcrumbs: () => ({ setName: vi.fn(), setNameIcon: vi.fn(), setContext: vi.fn() }),
}))
vi.mock('@/composables/useContentFavorites', () => ({
    useContentFavorites: () => ({
        load: async () => [],
        isPending: () => false,
        isFavorite: () => false,
    }),
}))
vi.mock('@/providers/content-install', () => ({
    injectContentInstall: () => ({ install: vi.fn() }),
}))
vi.mock('@/providers/content-selection', async () => {
    const { ref } = await import('vue')
    return {
        makeContentSelectionKey: () => '',
        injectContentSelection: () => ({
            targetInstance: ref(null),
            instances: ref([]),
            selectedProjects: ref([]),
            selectedCount: ref(0),
            state: ref('idle'),
            progress: ref(null),
            refreshInstances: vi.fn(),
            refreshInstalledIdentities: vi.fn(),
            setTarget: vi.fn(),
            isInstalling: () => false,
            isSelected: () => false,
            isInstalledIdentity: () => false,
        }),
    }
})
vi.mock('@/providers/server-install', async () => {
    const { ref } = await import('vue')
    return { injectServerInstall: () => ({ installingServerProjects: ref({}) }) }
})
vi.mock('@/providers/setup/server-install-content', async () => {
    const { ref } = await import('vue')
    return {
        provideServerInstallContent: vi.fn(),
        createServerInstallContent: () => ({
            enforceSetupModpackRoute: vi.fn(),
            serverContextServerData: ref(null),
            isServerContext: ref(false),
            isSetupServerContext: ref(false),
            isFromWorlds: ref(false),
            serverIdQuery: ref(''),
            serverFlowFrom: ref(''),
            effectiveServerWorldId: ref(''),
            serverBrowseHeading: ref(''),
            queuedServerInstallProjectIds: ref(new Set()),
            serverContentProjectIds: ref(new Set()),
            queuedServerInstallCount: ref(0),
            selectedServerInstallProjects: ref([]),
            isInstallingQueuedServerInstalls: ref(false),
            watchServerContextChanges: vi.fn(),
            initServerContext: async () => {},
        }),
    }
})
vi.mock('@/components/browse/BrowseInstanceSelector.vue', () => ({
    default: { render: () => null },
}))
vi.mock('@/components/multiplayer/servers/modpack/CreateModpackServerModal.vue', () => ({
    default: { render: () => null },
}))
vi.mock('@/components/ui/InstanceIndicator.vue', () => ({ default: { render: () => null } }))
vi.mock('@/components/ui/InstanceIcon.vue', () => ({ default: { render: () => null } }))
vi.mock('@/components/ui/ContextMenu.vue', () => ({ default: { render: () => null } }))
vi.mock('./UpgradeProjectReturnBar.vue', () => ({ default: { render: () => null } }))
vi.mock('@tanstack/vue-query', () => ({ useQueryClient: () => ({}) }))
vi.mock('@modrinth/ui', async () => {
    const { defineComponent, h, ref, computed } = await import('vue')
    return {
        ...Object.fromEntries(
            [
                'Avatar',
                'PopoutMenu',
                'BrowsePageLayout',
                'BrowseSidebar',
                'BrowseInstallHeader',
                'CreationFlowModal',
                'NavTabs',
                'OverflowMenu',
                'ProjectBackgroundGradient',
                'ProjectHeader',
                'ProjectPageSkeleton',
                'ProjectSidebarCompatibility',
                'ProjectSidebarCreators',
                'ProjectSidebarDetails',
                'ProjectSidebarLinks',
                'ProjectSidebarServerInfo',
                'ProjectSidebarTags',
                'SelectedProjectsFloatingBar',
            ].map((name) => [
                name,
                {
                    render: () =>
                        name === 'BrowsePageLayout'
                            ? h('nav', [
                                  h('span', 'Categories'),
                                  h('input', { placeholder: 'Search' }),
                              ])
                            : name === 'ProjectPageSkeleton'
                              ? h('div', { 'data-testid': 'project-skeleton' })
                              : null,
                },
            ]),
        ),
        getSelectedInstallPreferences: vi.fn(),
        isVersionTypeMatch: () => false,
        preferencesDiffer: () => false,
        stripServerRuntimeInstallFilters: (value: unknown) => value,
        stripServerRuntimeInstallOverrides: (value: unknown) => value,
        provideBrowseManager: (context: unknown) => {
            api.context = context
        },
        useDebugLogger: () => () => {},
        useBrowseSearch: (options: {
            projectType: { value: string }
            enabled: unknown
            getExtraQueryParams: () => Record<string, unknown>
        }) => {
            api.enabled = options.enabled
            api.extraQuery = options.getExtraQueryParams
            return {
                query: ref(''),
                currentFilters: ref([]),
                serverCurrentFilters: ref([]),
                toggledGroups: ref([]),
                serverToggledGroups: ref([]),
                overriddenProvidedFilterTypes: ref([]),
                filters: computed(() => [{ id: 'license', options: [{ id: 'open_source' }] }]),
                serverFilterTypes: computed(() => []),
                currentPage: ref(1),
                projectHits: ref([]),
                serverHits: ref([]),
                committed: ref(null),
                totalHits: ref(0),
                loading: ref(true),
                searchError: ref(null),
                initializeFilters: vi.fn(),
                refreshSearch: api.refresh,
            }
        },
        commonMessages: {
            retryButton: { defaultMessage: 'Retry' },
            backButton: { defaultMessage: 'Back' },
            installButton: { defaultMessage: 'Install' },
            followButton: { defaultMessage: 'Follow' },
            reportButton: { defaultMessage: 'Report' },
            changelogLabel: { defaultMessage: 'Changelog' },
        },
        commonProjectSettingsMessages: {
            description: { defaultMessage: 'Description' },
            versions: { defaultMessage: 'Versions' },
            gallery: { defaultMessage: 'Gallery' },
        },
        defineMessage: (value: unknown) => value,
        defineMessages: (value: unknown) => value,
        formatDependencyProjectFilterOption: vi.fn(),
        formatProjectTypeSentence: vi.fn(),
        getLatestMatchingInstallVersion: vi.fn(),
        getTargetInstallPreferences: vi.fn(),
        requestInstall: vi.fn(),
        usesTargetGameVersion: () => true,
        EmptyState: defineComponent({
            props: ['heading'],
            setup:
                (props, { slots }) =>
                () =>
                    h('section', [props.heading, slots.actions?.()]),
        }),
        Button: defineComponent({
            setup:
                (_, { slots }) =>
                () =>
                    h('button', slots.default?.()),
        }),
        injectNotificationManager: () => ({
            handleError: api.handleError,
            addNotification: vi.fn(),
        }),
        injectPopupNotificationManager: () => ({ addPopupNotification: vi.fn() }),
        useVIntl: () => ({
            formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
        }),
    }
})

it('mounts the browse shell before capabilities resolve and preserves type-specific filters while categories load', async () => {
    api.saveMemory.mockClear()
    let resolveCapability!: (value: unknown) => void
    api.capability.mockImplementation(
        () =>
            new Promise((done) => {
                resolveCapability = done
            }),
    )
    api.categories.mockResolvedValue([])
    const wrapper = mount(Browse, {
        global: { stubs: { Teleport: true }, directives: { tooltip: {} } },
    })
    try {
        expect(wrapper.text()).toContain('Categories')
        expect(wrapper.find('input').exists()).toBe(true)
        expect(api.refresh).not.toHaveBeenCalled()
        resolveCapability({ status: 'available', configured: true, key_source: 'user' })
        await flushPromises()
        expect(api.refresh).toHaveBeenCalledTimes(1)
        let resolveCategories!: (value: unknown) => void
        api.categories.mockImplementationOnce(
            () =>
                new Promise((done) => {
                    resolveCategories = done
                }),
        )
        const { useRoute } = await import('vue-router')
        useRoute().params.projectType = 'modpack'
        await flushPromises()
        expect((api.enabled as { value: boolean }).value).toBe(false)
        expect(api.saveMemory.mock.calls.some(([type]) => type === 'modpack')).toBe(false)
        resolveCategories([])
        await flushPromises()
        expect(
            (api.context as { currentFilters: { value: unknown } }).currentFilters.value,
        ).toEqual([{ type: 'license', option: 'open_source' }])
        expect(api.refresh).toHaveBeenCalledTimes(2)
    } finally {
        wrapper.unmount()
    }
})

it('retries the selected source and clears the previous source categories after preparation fails', async () => {
    const { useRoute } = await import('vue-router')
    useRoute().params.projectType = 'mod'
    api.source = 'modrinth'
    api.refresh.mockClear()
    api.capability.mockResolvedValue({ status: 'available', configured: true, key_source: 'user' })
    api.categories.mockRejectedValueOnce(new Error('offline')).mockResolvedValue([])
    const wrapper = mount(Browse, {
        global: { stubs: { Teleport: true }, directives: { tooltip: {} } },
    })
    try {
        await flushPromises()
        const context = api.context as {
            currentFilters: { value: unknown[] }
            refreshSearch: () => Promise<void>
        }
        context.currentFilters.value = [{ type: 'category_main', option: 'magic' }]
        await flushPromises()
        api.saveMemory.mockClear()
        const page = wrapper.vm as unknown as { selectContentSource: (source: string) => void }
        page.selectContentSource('curseforge')
        await flushPromises()
        expect(api.refresh).toHaveBeenCalledTimes(1)
        expect((api.enabled as { value: boolean }).value).toBe(false)
        await context.refreshSearch()
        expect(context.currentFilters.value).toEqual([])
        expect(api.saveMemory).toHaveBeenLastCalledWith('mod', {
            filters: [],
            toggledGroups: [],
            overriddenProvidedFilterTypes: [],
        })
        expect((api.extraQuery as () => { source: string })().source).toBe('curseforge')
        expect(api.refresh).toHaveBeenCalledTimes(2)
    } finally {
        wrapper.unmount()
        api.source = 'all'
    }
})

it.each(['missing', 'unavailable'])(
    'continues browsing when the linked instance is %s',
    async (failure) => {
        const { useRoute } = await import('vue-router')
        const route = useRoute()
        route.params.projectType = 'mod'
        route.query.i = 'deleted-instance'
        api.source = 'modrinth'
        api.refresh.mockClear()
        api.handleError.mockClear()
        api.instance.mockReset()
        if (failure === 'missing') api.instance.mockResolvedValue(null)
        else api.instance.mockRejectedValue(new Error('Instance unavailable'))
        api.capability.mockResolvedValue({
            status: 'available',
            configured: true,
            key_source: 'user',
        })
        const wrapper = mount(Browse, {
            global: { stubs: { Teleport: true }, directives: { tooltip: {} } },
        })
        try {
            await flushPromises()
            expect(api.instance).toHaveBeenCalledWith('deleted-instance')
            expect(api.handleError).toHaveBeenCalledTimes(1)
            expect((api.enabled as { value: boolean }).value).toBe(true)
            expect(api.refresh).toHaveBeenCalledTimes(1)
            expect(wrapper.find('input').exists()).toBe(true)
        } finally {
            wrapper.unmount()
            delete route.query.i
            api.source = 'all'
            api.instance.mockReset().mockResolvedValue(null)
        }
    },
)

it.each(['mcarchive', 'planet_minecraft'] as const)(
    'searches %s before optional version options finish',
    async (source) => {
        const { useRoute } = await import('vue-router')
        const route = useRoute()
        route.params.projectType = 'mod'
        route.query = { source }
        api.source = source
        api.planetAvailable = true
        api.refresh.mockClear()
        api.capability.mockResolvedValue({ configured: true })
        let finish!: (value: unknown[]) => void
        const pending = new Promise<unknown[]>((resolve) => {
            finish = resolve
        })
        if (source === 'mcarchive') api.archiveVersions.mockReturnValueOnce(pending)
        else api.gameVersions.mockReturnValueOnce(pending)
        const wrapper = mount(Browse, {
            global: { stubs: { Teleport: true }, directives: { tooltip: {} } },
        })
        try {
            await flushPromises()
            expect(api.refresh).toHaveBeenCalledTimes(1)
            expect((api.context as { filtersReady: { value: boolean } }).filtersReady.value).toBe(
                false,
            )
            finish([])
            await flushPromises()
            expect((api.context as { filtersReady: { value: boolean } }).filtersReady.value).toBe(
                true,
            )
            expect(api.refresh).toHaveBeenCalledTimes(1)
        } finally {
            wrapper.unmount()
            api.source = 'all'
            api.planetAvailable = false
            route.query = {}
        }
    },
)

it('waits for metadata when a version filter is supplied in the URL', async () => {
    const { useRoute } = await import('vue-router')
    const route = useRoute()
    route.params.projectType = 'mod'
    route.query = { source: 'modrinth', v: '1.21.1' }
    api.refresh.mockClear()
    api.capability.mockResolvedValue({ configured: true })
    let finish!: (value: unknown[]) => void
    api.gameVersions.mockReturnValueOnce(
        new Promise((resolve) => {
            finish = resolve
        }),
    )
    const wrapper = mount(Browse, {
        global: { stubs: { Teleport: true }, directives: { tooltip: {} } },
    })
    try {
        await flushPromises()
        expect(api.refresh).not.toHaveBeenCalled()
        finish([{ version: '1.21.1', version_type: 'release' }])
        await flushPromises()
        expect(api.refresh).toHaveBeenCalledTimes(1)
    } finally {
        wrapper.unmount()
        route.query = {}
    }
})

it('retries optional filter metadata without restarting the search', async () => {
    const { useRoute } = await import('vue-router')
    const route = useRoute()
    route.params.projectType = 'mod'
    route.query = { source: 'planet_minecraft' }
    api.planetAvailable = true
    api.refresh.mockClear()
    api.capability.mockResolvedValue({ configured: true })
    api.gameVersions.mockRejectedValueOnce(new Error('offline')).mockResolvedValue([])
    const wrapper = mount(Browse, {
        global: { stubs: { Teleport: true }, directives: { tooltip: {} } },
    })
    try {
        await flushPromises()
        expect(api.refresh).toHaveBeenCalledTimes(1)
        expect(
            (wrapper.vm as unknown as { filterMetadataFailed: boolean }).filterMetadataFailed,
        ).toBe(true)
        ;(wrapper.vm as unknown as { retryFilterMetadata: () => void }).retryFilterMetadata()
        await flushPromises()
        expect((api.context as { filtersReady: { value: boolean } }).filtersReady.value).toBe(true)
        expect(api.refresh).toHaveBeenCalledTimes(1)
    } finally {
        wrapper.unmount()
        api.planetAvailable = false
        route.query = {}
    }
})

it('keeps filters disabled until the implicit installed-content filter is initialized', async () => {
    const { useRoute } = await import('vue-router')
    const route = useRoute()
    route.params.projectType = 'mod'
    route.query = { source: 'modrinth', i: 'target', ai: 'true' }
    api.instance.mockResolvedValue({
        id: 'target',
        name: 'Target',
        install_stage: 'installed',
        game_version: '1.21.1',
        loader: 'fabric',
    })
    api.capability.mockResolvedValue({ configured: true })
    api.refresh.mockClear()
    let finish!: (ids: string[]) => void
    api.installedIds.mockReturnValueOnce(
        new Promise((resolve) => {
            finish = resolve
        }),
    )
    const wrapper = mount(Browse, {
        global: { stubs: { Teleport: true }, directives: { tooltip: {} } },
    })
    try {
        await flushPromises()
        expect(api.refresh).not.toHaveBeenCalled()
        expect((api.context as { filtersReady: { value: boolean } }).filtersReady.value).toBe(false)
        finish([])
        await flushPromises()
        expect(api.refresh).toHaveBeenCalledTimes(1)
        expect((api.context as { filtersReady: { value: boolean } }).filtersReady.value).toBe(true)
    } finally {
        wrapper.unmount()
        route.query = {}
        api.instance.mockResolvedValue(null)
    }
})

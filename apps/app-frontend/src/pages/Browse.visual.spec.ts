import type { BrowseManagerContext } from '@modrinth/ui'
import { flushPromises, mount } from '@vue/test-utils'
import { IntlMessageFormat } from 'intl-messageformat'
import { expect, it, vi } from 'vitest'
import { computed, defineComponent, h, reactive, ref } from 'vue'

import zhMessages from '@/locales/zh-CN/index.json'

import Browse from './Browse.vue'

const api = vi.hoisted(() => ({
    release: vi.fn().mockResolvedValue('selected-release'),
    add: vi.fn(
        async (
            item: { versionId?: string },
            resolve?: (isCurrent: () => boolean) => Promise<string | null | undefined>,
        ) => {
            if (resolve) {
                const id = await resolve(() => true)
                if (id === null) return false
                item.versionId = id
            }
            return true
        },
    ),
    instance: vi.fn().mockResolvedValue(null),
    installedIds: vi.fn().mockResolvedValue([]),
    contentSnapshot: vi.fn().mockResolvedValue({ items: [] }),
    startContentRefresh: vi.fn().mockResolvedValue({}),
    contentRefreshListener: null as
        null | ((event: { instanceId: string; patch: { kind: string } }) => void),
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
    automaticSort: false as boolean | undefined,
    supportedSortNames: null as unknown,
    selection: null as unknown,
    locale: 'en',
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
vi.mock('vue-router', async (original) => {
    const router = await original<typeof import('vue-router')>()
    const { reactive } = await import('vue')
    const route = reactive({
        params: { projectType: 'mod' },
        query: api.query,
        path: '/browse/mod',
        fullPath: '/browse/mod',
    })
    return {
        ...router,
        useRoute: () => route,
        useRouter: () => ({ push: vi.fn(), replace: vi.fn() }),
        onBeforeRouteLeave: vi.fn(),
    }
})
vi.mock('@/helpers/project-install-filters', async (original) => ({
    ...(await original<typeof import('@/helpers/project-install-filters')>()),
    filteredProjectRelease: api.release,
}))
vi.mock('@/helpers/curseforge', () => ({
    getCurseForgeFiles: vi.fn(),
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
vi.mock('@/helpers/content-refresh', () => ({
    resourceStartContentRefresh: api.startContentRefresh,
    listenContentRefresh: async (
        listener: (event: { instanceId: string; patch: { kind: string } }) => void,
    ) => {
        api.contentRefreshListener = listener
        return () => {
            api.contentRefreshListener = null
        }
    },
}))
vi.mock('@/helpers/content-search', () => ({
    resolveMcmodUrl: async () => null,
    translateSearchHitTitles: async (hits: unknown[]) => hits,
}))
vi.mock('@/helpers/instance', () => ({
    get_installed_project_ids: api.installedIds,
    get_content_snapshot: api.contentSnapshot,
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
        injectContentSelection: () =>
            (api.selection = {
                add: api.add,
                targetInstance: ref(null),
                instances: ref([]),
                selectedProjects: ref([]),
                selectedCount: ref(0),
                state: ref('idle'),
                progress: ref({ completed: 0, total: 0 }),
                refreshInstances: vi.fn(),
                refreshInstalledIdentities: vi.fn(),
                setTarget: vi.fn(),
                isInstalling: () => false,
                isSelected: () => false,
                isInstalledIdentity: () => false,
            }),
    }
})

it.each(['en', 'zh-CN'])(
    'uses localized stage feedback instead of a world-only 0/0 count while reviewing mods (%s)',
    async (locale) => {
        api.locale = locale
        const { useRoute } = await import('vue-router')
        const route = useRoute()
        route.params.projectType = 'mod'
        route.query = { source: 'modrinth' }
        api.capability.mockResolvedValue({ configured: true })
        const wrapper = mount(Browse, {
            global: { stubs: { Teleport: true }, directives: { tooltip: {} } },
        })
        const selection = reactive(
            api.selection as {
                targetInstance: unknown
                state: unknown
                progress: unknown
            },
        )
        try {
            await flushPromises()
            selection.targetInstance = {
                id: 'target',
                name: 'Target',
                game_version: '1.20.4',
                loader: 'fabric',
            }
            const context = () =>
                (
                    api.context as {
                        installContext: {
                            value: { processingLabel: string; isInstallingSelected: boolean }
                        }
                    }
                ).installContext.value
            selection.state = 'reviewing'
            await flushPromises()
            expect(context().isInstallingSelected).toBe(true)
            expect(context().processingLabel).toBe(
                locale === 'zh-CN' ? '正在预览内容变更…' : 'Reviewing content changes…',
            )
            selection.state = 'queueing'
            await flushPromises()
            expect(context().processingLabel).toBe(
                locale === 'zh-CN' ? '正在加入下载队列…' : 'Adding to download queue…',
            )
            selection.state = 'validating'
            selection.progress = { completed: 1, total: 2 }
            await flushPromises()
            expect(context().processingLabel).toBe(
                locale === 'zh-CN' ? '正在准备 1/2' : 'Preparing 1/2',
            )
        } finally {
            wrapper.unmount()
            route.query = {}
            api.selection = null
            api.locale = 'en'
            api.refresh.mockClear()
        }
    },
)
vi.mock('@/providers/server-install', async () => {
    const { ref } = await import('vue')
    return { injectServerInstall: () => ({ installingServerProjects: ref({}) }) }
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
vi.mock('@tanstack/vue-query', async (original) => ({
    ...(await original<typeof import('@tanstack/vue-query')>()),
    useQueryClient: () => ({}),
}))
vi.mock('@modrinth/ui', async () => {
    const { defineComponent, h, ref, computed } = await import('vue')
    return {
        ...Object.fromEntries(
            [
                'Avatar',
                'PopoutMenu',
                'BrowsePageLayout',
                'BrowseSidebar',
                'BrowseSourceSelector',
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
        provideBrowseManager: (context: unknown) => {
            api.context = context
        },
        useDebugLogger: () => () => {},
        useBrowseSearch: (options: {
            projectType: { value: string }
            enabled: unknown
            automaticSort?: boolean
            supportedSortNames?: { value: readonly string[] | undefined }
            getExtraQueryParams: () => Record<string, unknown>
        }) => {
            api.enabled = options.enabled
            api.automaticSort = options.automaticSort
            api.supportedSortNames = options.supportedSortNames
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
            installingLabel: { defaultMessage: 'Installing' },
            installingContentLabel: { defaultMessage: 'Installing content' },
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
            formatMessage: (
                message: { id?: string; defaultMessage: string },
                values?: Record<string, string | number>,
            ) =>
                new IntlMessageFormat(
                    api.locale === 'zh-CN'
                        ? ((
                              zhMessages as Record<
                                  string,
                                  { message?: string; defaultMessage?: string }
                              >
                          )[message.id ?? '']?.message ?? message.defaultMessage)
                        : message.defaultMessage,
                    api.locale,
                ).format(values),
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

it('renders the shared browse layout with the desktop page context before and after a route update', async () => {
    const [{ default: BrowsePageLayout }, { provideBrowseManager }, { I18N_INJECTION_KEY }] =
        await Promise.all([
            import('@modrinth/ui/src/layouts/shared/browse-tab/layout.vue'),
            import('@modrinth/ui/src/layouts/shared/browse-tab/providers/browse-manager'),
            import('@modrinth/ui/src/providers/i18n'),
        ])
    api.capability.mockResolvedValue({ status: 'available', configured: true, key_source: 'user' })
    api.categories.mockResolvedValue([])
    const page = mount(Browse, {
        global: { stubs: { Teleport: true }, directives: { tooltip: {} } },
    })
    let layout: ReturnType<typeof mount> | undefined
    try {
        await flushPromises()
        const context = api.context as BrowseManagerContext
        const Harness = defineComponent({
            setup() {
                provideBrowseManager({
                    ...context,
                    effectiveLayout: computed(() => 'list'),
                    maxResults: ref(20),
                    pageCount: computed(() => 1),
                })
                return () => h(BrowsePageLayout)
            },
        })
        layout = mount(Harness, {
            global: {
                provide: {
                    [I18N_INJECTION_KEY as symbol]: {
                        locale: ref('en-US'),
                        t: (key: string) => key,
                        setLocale: () => {},
                    },
                },
                stubs: {
                    BrowseToolbar: true,
                    BrowseResultControls: true,
                    ProjectCardList: true,
                    ProjectCardSkeleton: true,
                    Pagination: true,
                },
                directives: { tooltip: {} },
            },
        })
        expect(layout.getComponent({ name: 'BrowseToolbar' }).props('showTabs')).toBe(true)
        context.projectType.value = 'modpack'
        await flushPromises()
        expect(layout.getComponent({ name: 'BrowseToolbar' }).props('showTabs')).toBe(true)
    } finally {
        layout?.unmount()
        page.unmount()
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
            expect(api.automaticSort).toBe(true)
            expect((api.supportedSortNames as { value: readonly string[] }).value).toEqual([
                'relevance',
            ])
            expect((api.context as { filtersReady: { value: boolean } }).filtersReady.value).toBe(
                false,
            )
            finish([])
            await flushPromises()
            expect((api.context as { filtersReady: { value: boolean } }).filtersReady.value).toBe(
                true,
            )
            expect(api.refresh).toHaveBeenCalledTimes(1)
            const page = wrapper.vm as unknown as { selectContentSource: (source: string) => void }
            page.selectContentSource('modrinth')
            await flushPromises()
            expect(
                (api.supportedSortNames as { value: readonly string[] | undefined }).value,
            ).toBeUndefined()
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

it('updates installed content when a manual import is recognized in the background', async () => {
    const { useRoute } = await import('vue-router')
    const route = useRoute()
    route.params.projectType = 'mod'
    route.query = { source: 'modrinth', i: 'target' }
    api.instance.mockResolvedValue({
        id: 'target',
        name: 'Target',
        install_stage: 'installed',
        game_version: '1.21.1',
        loader: 'fabric',
    })
    api.installedIds.mockResolvedValue([])
    api.contentSnapshot.mockResolvedValue({ items: [] })
    const wrapper = mount(Browse, {
        global: { stubs: { Teleport: true }, directives: { tooltip: {} } },
    })
    try {
        await flushPromises()
        const selection = api.selection as {
            targetInstance: { value: unknown }
            refreshInstalledIdentities: ReturnType<typeof vi.fn>
        }
        selection.targetInstance.value = {
            id: 'target',
            name: 'Target',
            install_stage: 'installed',
            game_version: '1.21.1',
            loader: 'fabric',
        }
        const vm = wrapper.vm as unknown as {
            refreshInstalledProjectIds: () => Promise<boolean>
            allInstalledIds: Set<string>
        }
        await vm.refreshInstalledProjectIds()
        expect(api.startContentRefresh).toHaveBeenCalledWith('target', 'normal')
        expect(vm.allInstalledIds.has('abc')).toBe(false)
        api.contentSnapshot.mockResolvedValue({
            items: [
                {
                    projectType: 'mod',
                    provider: null,
                    providerProjectId: null,
                    materializationState: 'present',
                    expectedRelativePath: 'mods/example.jar',
                    content: null,
                    artifactRef: { release: { project: { source: 'modrinth', projectId: 'abc' } } },
                },
            ],
        })
        api.contentRefreshListener?.({ instanceId: 'other', patch: { kind: 'entry' } })
        await new Promise((resolve) => setTimeout(resolve, 150))
        expect(vm.allInstalledIds.has('abc')).toBe(false)
        api.contentRefreshListener?.({ instanceId: 'target', patch: { kind: 'entry' } })
        await vi.waitFor(() => expect(vm.allInstalledIds.has('abc')).toBe(true))
        expect(selection.refreshInstalledIdentities).toHaveBeenCalled()
        api.contentSnapshot.mockResolvedValue({ items: [] })
        api.contentRefreshListener?.({ instanceId: 'target', patch: { kind: 'content' } })
        await vi.waitFor(() => expect(vm.allInstalledIds.has('abc')).toBe(false))
    } finally {
        wrapper.unmount()
        route.query = {}
        api.instance.mockResolvedValue(null)
        api.contentSnapshot.mockResolvedValue({ items: [] })
    }
})

it('ignores retired hosting query parameters while keeping the local instance install target', async () => {
    const { useRoute } = await import('vue-router')
    const route = useRoute()
    route.params.projectType = 'mod'
    route.query = {
        source: 'modrinth',
        i: 'target',
        sid: 'old-host',
        wid: 'old-world',
        shi: 'true',
    }
    api.instance.mockResolvedValue({
        id: 'target',
        name: 'Local instance',
        install_stage: 'installed',
        game_version: '1.21.1',
        loader: 'fabric',
    })
    api.capability.mockResolvedValue({ configured: true })
    api.refresh.mockClear()
    const wrapper = mount(Browse, {
        global: { stubs: { Teleport: true }, directives: { tooltip: {} } },
    })
    try {
        await flushPromises()
        const selection = api.selection as {
            targetInstance: { value: unknown }
            refreshInstances: ReturnType<typeof vi.fn>
        }
        selection.targetInstance.value = {
            id: 'target',
            name: 'Local instance',
            game_version: '1.21.1',
            loader: 'fabric',
        }
        await flushPromises()
        const context = api.context as {
            installContext: { value: { name: string; gameVersion: string; backUrl: string } }
            selectableProjectTypes: { value: { href: string }[] }
        }
        expect(selection.refreshInstances).toHaveBeenCalledWith('target', expect.any(Function))
        expect(context.installContext.value).toMatchObject({
            name: 'Local instance',
            gameVersion: '1.21.1',
            backUrl: '/instance/target',
        })
        expect(context.selectableProjectTypes.value.length).toBeGreaterThan(1)
        expect(
            context.selectableProjectTypes.value.every(({ href }) => !href.includes('sid=')),
        ).toBe(true)
        expect((api.extraQuery as () => Record<string, unknown>)()).not.toHaveProperty('wid')
        expect(api.refresh).toHaveBeenCalled()
    } finally {
        wrapper.unmount()
        route.query = {}
        api.instance.mockResolvedValue(null)
        api.selection = null
    }
})

it('keeps the installing action label for public modpack downloads', async () => {
    const { useRoute } = await import('vue-router')
    const route = useRoute()
    route.params.projectType = 'modpack'
    route.query = { source: 'modrinth' }
    api.capability.mockResolvedValue({ configured: true })
    const wrapper = mount(Browse, {
        global: { stubs: { Teleport: true }, directives: { tooltip: {} } },
    })
    try {
        await flushPromises()
        const selection = api.selection as { isInstalling: () => boolean }
        selection.isInstalling = () => true
        const context = api.context as {
            getCardActions: (
                project: unknown,
                type: string,
            ) => { key: string; label: string; disabled: boolean }[]
        }
        const action = context
            .getCardActions(
                { provider: 'modrinth', project_id: 'public-pack', title: 'Public pack' },
                'modpack',
            )
            .find((entry) => entry.key === 'install')
        expect(action).toMatchObject({ label: 'Installing', disabled: true })
    } finally {
        wrapper.unmount()
        route.params.projectType = 'mod'
        route.query = {}
        api.selection = null
    }
})

it('provides the project type tab visibility required by the shared browse layout', async () => {
    api.capability.mockResolvedValue({ configured: true })
    const wrapper = mount(Browse, {
        global: { stubs: { Teleport: true }, directives: { tooltip: {} } },
    })
    try {
        await flushPromises()
        const context = api.context as { showProjectTypeTabs?: { value: boolean } }
        expect(context.showProjectTypeTabs?.value).toBe(true)
    } finally {
        wrapper.unmount()
    }
})

it('freezes the unlocked browse version when adding a NeoForge mod to a Fabric instance', async () => {
    const ui = await import('@modrinth/ui')
    vi.mocked(ui.getSelectedInstallPreferences).mockReturnValue({
        gameVersions: ['1.21.1'],
        loaders: ['neoforge'],
    })
    vi.mocked(ui.getTargetInstallPreferences).mockReturnValue({
        gameVersions: ['26.3'],
        loaders: ['fabric'],
    })
    api.capability.mockResolvedValue({ configured: true })
    const wrapper = mount(Browse, {
        global: { stubs: { Teleport: true }, directives: { tooltip: {} } },
    })
    try {
        await flushPromises()
        const selection = api.selection as { targetInstance: { value: unknown } }
        selection.targetInstance.value = {
            id: 'fabric',
            game_version: '26.3',
            loader: 'fabric',
            name: 'Fabric',
        }
        await flushPromises()
        const vm = wrapper.vm as unknown as {
            toggleContentSelection: (project: object, type: string) => Promise<void>
        }
        await vm.toggleContentSelection(
            { provider: 'modrinth', project_id: 'chosen', title: 'Chosen' },
            'mod',
        )
        expect(api.release).toHaveBeenLastCalledWith(
            'modrinth',
            'chosen',
            { gameVersions: ['1.21.1'], loaders: ['neoforge'] },
            [],
            'mod',
        )
        expect(api.add).toHaveBeenLastCalledWith(
            expect.objectContaining({
                provider: 'modrinth',
                providerProjectId: 'chosen',
                versionId: 'selected-release',
            }),
            expect.any(Function),
        )
    } finally {
        wrapper.unmount()
        vi.mocked(ui.getSelectedInstallPreferences).mockReset()
        vi.mocked(ui.getTargetInstallPreferences).mockReset()
    }
})

it('leaves absent browse axes inheritable and serializes only explicitly cleared provided axes', async () => {
    const ui = await import('@modrinth/ui')
    vi.mocked(ui.getSelectedInstallPreferences).mockReturnValue({})
    const wrapper = mount(Browse, {
        global: { stubs: { Teleport: true }, directives: { tooltip: {} } },
    })
    try {
        await flushPromises()
        const vm = wrapper.vm as unknown as { getProjectBrowseQuery: () => Record<string, unknown> }
        const query = vm.getProjectBrowseQuery()
        expect(query).not.toHaveProperty('g')
        expect(query).not.toHaveProperty('l')
        const context = api.context as { overriddenProvidedFilterTypes: { value: string[] } }
        context.overriddenProvidedFilterTypes.value = ['mod_loader']
        expect(vm.getProjectBrowseQuery()).toMatchObject({ l: '' })
        expect(vm.getProjectBrowseQuery()).not.toHaveProperty('g')
    } finally {
        wrapper.unmount()
        vi.mocked(ui.getSelectedInstallPreferences).mockReset()
    }
})

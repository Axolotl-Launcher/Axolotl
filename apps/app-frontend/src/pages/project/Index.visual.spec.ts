import { flushPromises, mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'

import Project from './Index.vue'

const api = vi.hoisted(() => ({
    project: vi.fn(),
    versions: vi.fn().mockResolvedValue([]),
    push: vi.fn(),
    query: { b: '/browse/mod?q=kept', i: 'target-instance' },
}))
vi.mock('@tauri-apps/api/core', () => ({
    invoke: vi.fn().mockResolvedValue([]),
    convertFileSrc: (path: string) => path,
}))
vi.mock('vue-router', () => ({
    useRoute: () => ({
        params: { id: 'one' },
        query: api.query,
        path: '/project/one',
        fullPath: '/project/one',
    }),
    useRouter: () => ({ push: api.push, replace: vi.fn() }),
}))
vi.mock('@/helpers/cache.js', () => ({
    get_project: api.project,
    get_project_v3: async () => null,
    get_team: async () => [],
    get_version_many: api.versions,
    get_version: vi.fn(),
    get_organization: vi.fn(),
}))
vi.mock('@/helpers/tags', () => ({
    get_categories: async () => [],
    get_loaders: async () => [],
    get_game_versions: async () => [],
}))
vi.mock('@/helpers/events', () => ({ process_listener: async () => () => {} }))
vi.mock('@/helpers/content-search', () => ({ resolveMcmodUrl: async () => null }))
vi.mock('@/helpers/instance', () => ({
    get: async () => null,
    get_projects: async () => [],
    list: async () => [],
    kill: vi.fn(),
}))
vi.mock('@/helpers/worlds', () => ({ getServerAddress: () => null }))
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
vi.mock('@/store/state.js', () => ({ useTheming: () => ({ featureFlags: {} }) }))
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
vi.mock('@/providers/setup/server-install-content', () => ({
    createServerInstallContent: () => ({
        serverContextServerData: { value: null },
        isServerContext: { value: false },
        queuedServerInstallProjectIds: { value: new Set() },
        serverContentProjectIds: { value: new Set() },
        isInstallingQueuedServerInstalls: { value: false },
        watchServerContextChanges: vi.fn(),
        initServerContext: async () => {},
        getProjectInstallContext: () => null,
    }),
}))
vi.mock('@/components/browse/BrowseInstanceSelector.vue', () => ({
    default: { render: () => null },
}))
vi.mock('@/components/multiplayer/servers/modpack/CreateModpackServerModal.vue', () => ({
    default: { render: () => null },
}))
vi.mock('@/components/ui/InstanceIndicator.vue', () => ({ default: { render: () => null } }))
vi.mock('@/components/ui/ContextMenu.vue', () => ({ default: { render: () => null } }))
vi.mock('./UpgradeProjectReturnBar.vue', () => ({ default: { render: () => null } }))
vi.mock('@tanstack/vue-query', () => ({ useQueryClient: () => ({}) }))
vi.mock('@modrinth/ui', async () => {
    const { defineComponent, h } = await import('vue')
    return {
        ...Object.fromEntries(
            [
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
            ].map((name) => [name, { render: () => null }]),
        ),
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
        injectNotificationManager: () => ({ handleError: vi.fn(), addNotification: vi.fn() }),
        injectPopupNotificationManager: () => ({ addPopupNotification: vi.fn() }),
        useVIntl: () => ({
            formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
        }),
    }
})

it('retains an actionable error page and browse context across failed retries', async () => {
    api.project.mockClear()
    api.project.mockRejectedValue(new Error('offline'))
    const wrapper = mount(Project, {
        global: {
            stubs: { EmptyState: false, Button: false, Teleport: true },
            directives: { tooltip: {} },
        },
    })
    try {
        await vi.waitFor(() => expect(wrapper.text()).toContain('The project could not be loaded'))
        await wrapper
            .findAll('button')
            .find((button) => button.text() === 'Retry')!
            .trigger('click')
        await flushPromises()
        expect(api.project).toHaveBeenCalledTimes(2)
        expect(wrapper.text()).toContain('The project could not be loaded')
        await wrapper
            .findAll('button')
            .find((button) => button.text() === 'Back')!
            .trigger('click')
        expect(api.push).toHaveBeenCalledWith('/browse/mod?q=kept')
        expect(api.query.i).toBe('target-instance')
    } finally {
        wrapper.unmount()
    }
})

it('loads releases from the revalidated project index on each visit', async () => {
    api.project.mockReset().mockResolvedValue({
        id: 'one',
        slug: 'one',
        title: 'Pack',
        project_type: 'modpack',
        versions: ['new-release', 'old-release'],
        team: 'team',
        categories: [],
        game_versions: [],
        loaders: [],
        gallery: [],
    })
    api.versions.mockClear()
    for (let visit = 0; visit < 2; visit++) {
        const wrapper = mount(Project, {
            global: {
                stubs: { Teleport: true, RouterView: true },
                directives: { tooltip: {} },
            },
        })
        try {
            await vi.waitFor(() => expect(api.versions).toHaveBeenCalledTimes(visit + 1))
            expect(api.project).toHaveBeenLastCalledWith('one', 'revalidate')
            expect(api.versions).toHaveBeenLastCalledWith(
                ['new-release', 'old-release'],
                'must_revalidate',
            )
        } finally {
            wrapper.unmount()
        }
    }
})

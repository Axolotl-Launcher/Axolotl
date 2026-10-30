import { flushPromises, mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { defineComponent, h } from 'vue'

import CurseForge from './CurseForge.vue'
import Project from './Index.vue'

const api = vi.hoisted(() => ({
    project: vi.fn(),
    projectV3: vi.fn().mockResolvedValue(null),
    background: false,
    versions: vi.fn().mockResolvedValue([]),
    snapshot: vi.fn().mockResolvedValue({ items: [] }),
    instance: vi.fn().mockResolvedValue({
        id: 'target-instance',
        name: 'Target instance',
        loader: 'fabric',
        game_version: '1.21.8',
    }),
    team: vi.fn().mockResolvedValue([]),
    refreshInstances: vi.fn().mockResolvedValue(undefined),
    cfProject: vi.fn(),
    cfDescription: vi.fn(),
    cfFiles: vi.fn(),
    params: { id: 'one' },
    push: vi.fn(),
    query: { b: '/browse/mod?q=kept', i: 'target-instance' },
}))
vi.mock('@tauri-apps/api/core', async (importOriginal) => ({
    ...(await importOriginal<typeof import('@tauri-apps/api/core')>()),
    invoke: vi.fn().mockResolvedValue([]),
    convertFileSrc: (path: string) => path,
}))
vi.mock('vue-router', () => ({
    useRoute: () => ({
        params: api.params,
        query: api.query,
        path: '/project/one',
        fullPath: '/project/one',
    }),
    useRouter: () => ({ push: api.push, replace: vi.fn() }),
}))
vi.mock('@/helpers/cache.js', () => ({
    get_project: api.project,
    get_project_v3: api.projectV3,
    get_team: api.team,
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
vi.mock('@/helpers/curseforge', () => ({
    getCurseForgeProject: api.cfProject,
    getCurseForgeDescription: api.cfDescription,
    getCurseForgeFilesPage: api.cfFiles,
    getCurseForgeFiles: vi.fn(),
    getCurseForgeImageUrl: (url: string) => url,
}))
vi.mock('@/components/ui/ResourceDescription.vue', () => ({
    default: defineComponent({
        setup:
            (_, { slots }) =>
            () =>
                h('div', slots.default?.()),
    }),
}))
vi.mock('@/components/ui/TranslatedProjectDescription.vue', () => ({
    default: defineComponent({
        props: ['description'],
        setup: (props) => () => h('p', props.description),
    }),
}))
vi.mock('@/helpers/instance', () => ({
    get: api.instance,
    get_content_snapshot: api.snapshot,
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
vi.mock('@/store/state.js', () => ({
    useTheming: () => ({ featureFlags: { project_background: api.background } }),
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
            refreshInstances: api.refreshInstances,
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
    return { injectServerInstall: () => ({ installingServerProjects: ref([]) }) }
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
vi.mock('./Changelog.vue', () => ({ default: { render: () => null } }))
vi.mock('./Gallery.vue', () => ({ default: { render: () => null } }))
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
                'ProjectPageSkeleton',
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
                        name === 'ProjectPageSkeleton'
                            ? h('div', { 'data-testid': 'project-skeleton' })
                            : null,
                },
            ]),
        ),
        ProjectSidebarCompatibility: {
            render: () => h('section', { 'data-testid': 'project-compatibility' }),
        },
        ProjectPageSkeleton: {
            props: ['title'],
            setup: (props: { title?: string }) => () =>
                h('div', { 'data-testid': 'project-skeleton' }, props.title),
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
        Card: defineComponent({
            setup:
                (_, { slots }) =>
                () =>
                    h('div', slots.default?.()),
        }),
        ProjectHeader: defineComponent({
            props: ['project'],
            setup:
                (props, { slots }) =>
                () =>
                    h('header', { 'data-testid': 'project-header' }, [
                        h('h1', props.project.title),
                        h('p', props.project.description),
                        slots.actions?.(),
                    ]),
        }),
        ProjectPageVersions: { render: () => null },
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
            formatMessage: (message?: { defaultMessage: string }) => message?.defaultMessage ?? '',
        }),
    }
})

it('renders CurseForge metadata while description and file requests are pending', async () => {
    api.params.id = '42'
    api.cfProject.mockResolvedValue({
        id: 42,
        name: 'CurseForge details',
        slug: 'cf-project',
        summary: 'Project summary',
        classId: 6,
        categories: [],
        screenshots: [],
        latestFilesIndexes: [],
        links: {},
        authors: [],
    })
    api.cfDescription.mockImplementationOnce(() => new Promise(() => {}))
    api.cfFiles.mockImplementationOnce(() => new Promise(() => {}))
    const wrapper = mount(CurseForge, {
        global: { stubs: { Teleport: true }, directives: { tooltip: {} } },
    })
    try {
        await vi.waitFor(() => expect(wrapper.text()).toContain('CurseForge details'))
        expect(wrapper.text()).toContain('Project summary')
        expect(
            wrapper
                .findAll('button')
                .find((button) => button.text() === 'Translate')
                ?.attributes('disabled'),
        ).toBeDefined()
    } finally {
        wrapper.unmount()
        api.params.id = 'one'
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

it('keeps installation busy until the target instance has been synchronized', async () => {
    api.project.mockResolvedValue({
        id: 'one',
        slug: 'one',
        title: 'Target project',
        project_type: 'mod',
        versions: [],
        team: 'team',
        categories: [],
        game_versions: [],
        loaders: [],
        gallery: [],
    })
    api.snapshot.mockClear()
    api.refreshInstances.mockClear().mockImplementationOnce(() => new Promise(() => {}))
    const wrapper = mount(Project, {
        global: {
            stubs: {
                Teleport: true,
                RouterView: defineComponent({
                    name: 'TargetProjectDetails',
                    props: ['project', 'installing'],
                    setup: (props) => () => h('p', props.project.title),
                }),
            },
            directives: { tooltip: {} },
        },
    })
    try {
        await vi.waitFor(() =>
            expect(api.refreshInstances).toHaveBeenCalledWith(
                'target-instance',
                expect.any(Function),
            ),
        )
        expect(api.snapshot).toHaveBeenCalledExactlyOnceWith('target-instance', true)
        await flushPromises()
        expect(wrapper.findComponent({ name: 'TargetProjectDetails' }).props('installing')).toBe(
            true,
        )
    } finally {
        wrapper.unmount()
    }
})

it.each(['cross-provider version', 'snapshot failure'])(
    'preserves the requested instance when reading %s',
    async (scenario) => {
        api.project.mockResolvedValue({
            id: 'one',
            slug: 'one',
            title: 'Target project',
            project_type: 'mod',
            versions: [],
            team: 'team',
            categories: [],
            game_versions: [],
            loaders: [],
            gallery: [],
        })
        api.refreshInstances.mockReset().mockResolvedValue(undefined)
        if (scenario === 'snapshot failure') {
            api.snapshot.mockRejectedValueOnce(new Error('snapshot unavailable'))
        } else {
            api.snapshot.mockResolvedValueOnce({
                items: [
                    {
                        materializationState: 'present',
                        provider: 'curseforge',
                        providerProjectId: '42',
                        providerReleaseId: '123',
                        content: {
                            provider_refs: [
                                {
                                    provider: 'modrinth',
                                    project_id: 'one',
                                    version_id: 'installed-release',
                                },
                            ],
                        },
                    },
                ],
            })
        }
        const wrapper = mount(Project, {
            global: {
                stubs: {
                    Teleport: true,
                    RouterView: defineComponent({
                        name: 'TargetProjectDetails',
                        props: ['instance', 'installedVersion', 'installing'],
                        setup: () => () => null,
                    }),
                },
                directives: { tooltip: {} },
            },
        })
        try {
            await vi.waitFor(() =>
                expect(api.refreshInstances).toHaveBeenCalledWith(
                    'target-instance',
                    expect.any(Function),
                ),
            )
            await flushPromises()
            const details = wrapper.findComponent({ name: 'TargetProjectDetails' })
            expect(details.props('instance').id).toBe('target-instance')
            expect(details.props('installing')).toBe(false)
            expect(details.props('installedVersion')).toBe(
                scenario === 'snapshot failure' ? null : 'installed-release',
            )
        } finally {
            wrapper.unmount()
        }
    },
)

it('shows the project while release data is still loading', async () => {
    api.project.mockReset().mockResolvedValue({
        id: 'one',
        slug: 'one',
        title: 'Pack',
        project_type: 'modpack',
        versions: ['release'],
        team: 'team',
        categories: [],
        game_versions: [],
        loaders: [],
        gallery: [],
    })
    let resolveVersions!: (versions: unknown[]) => void
    api.versions
        .mockReset()
        .mockImplementation(() => new Promise((resolve) => (resolveVersions = resolve)))

    const wrapper = mount(Project, {
        global: {
            stubs: { Teleport: true, RouterView: true },
            directives: { tooltip: {} },
        },
    })
    try {
        await vi.waitFor(() =>
            expect(wrapper.get('[data-testid="project-header"] h1').text()).toBe('Pack'),
        )
        expect(wrapper.find('[data-testid="project-skeleton"]').exists()).toBe(false)
        resolveVersions([])
        await flushPromises()
    } finally {
        wrapper.unmount()
    }
})

it('shows the header before v3 classification even with the project background enabled', async () => {
    api.background = true
    api.project.mockReset().mockResolvedValue({
        id: 'one',
        slug: 'one',
        title: 'Pack',
        project_type: 'modpack',
        versions: [],
        team: 'team',
        categories: [],
        game_versions: [],
        loaders: [],
        gallery: [],
    })
    let resolve!: (value: unknown) => void
    api.projectV3.mockImplementationOnce(
        () =>
            new Promise((done) => {
                resolve = done
            }),
    )
    const wrapper = mount(Project, {
        global: { stubs: { Teleport: true, RouterView: true }, directives: { tooltip: {} } },
    })
    try {
        await vi.waitFor(() =>
            expect(wrapper.get('[data-testid="project-header"] h1').text()).toBe('Pack'),
        )
        expect(wrapper.find('[data-testid="project-skeleton"]').exists()).toBe(false)
        resolve({ minecraft_server: { region: 'asia' } })
        await flushPromises()
        expect(wrapper.get('[data-testid="project-header"] h1').text()).toBe('Pack')
    } finally {
        wrapper.unmount()
        api.background = false
    }
})

it('keeps the title and offers local recovery when v3 details fail', async () => {
    api.projectV3
        .mockReset()
        .mockRejectedValueOnce(new Error('v3 unavailable'))
        .mockResolvedValueOnce({ minecraft_server: null })
    const wrapper = mount(Project, {
        global: { stubs: { Teleport: true, RouterView: true }, directives: { tooltip: {} } },
    })
    try {
        await vi.waitFor(() =>
            expect(wrapper.text()).toContain('Some project details could not be loaded.'),
        )
        expect(wrapper.get('[data-testid="project-header"] h1').text()).toBe('Pack')
        expect(api.projectV3).toHaveBeenCalledTimes(1)
        expect(wrapper.find('[data-testid="project-compatibility"]').exists()).toBe(false)
        await wrapper
            .findAll('button')
            .find((button) => button.text() === 'Retry')!
            .trigger('click')
        await flushPromises()
        expect(api.projectV3).toHaveBeenCalledTimes(2)
        expect(api.projectV3).toHaveBeenLastCalledWith('one', 'revalidate')
        expect(wrapper.find('[data-testid="project-compatibility"]').exists()).toBe(true)
        expect(wrapper.text()).not.toContain('Some project details could not be loaded.')
    } finally {
        wrapper.unmount()
    }
})

it('shows the navigation title before the main project request completes', async () => {
    let resolve!: (value: unknown) => void
    api.project.mockReset().mockImplementationOnce(
        () =>
            new Promise((done) => {
                resolve = done
            }),
    )
    window.history.replaceState(
        { projectPreview: { targetPath: '/project/one', title: 'Known title' } },
        '',
    )
    const wrapper = mount(Project, {
        global: { stubs: { Teleport: true, RouterView: true }, directives: { tooltip: {} } },
    })
    try {
        expect(wrapper.text()).toContain('Known title')
        wrapper.unmount()
        resolve({ id: 'one', title: 'Late project' })
        await flushPromises()
    } finally {
        wrapper.unmount()
        window.history.replaceState({}, '')
    }
})

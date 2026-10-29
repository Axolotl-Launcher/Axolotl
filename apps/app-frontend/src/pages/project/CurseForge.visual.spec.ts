import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, expect, it, vi } from 'vitest'
import type { VNode } from 'vue'
import { defineComponent, h, reactive, ref } from 'vue'

import CurseForge from './CurseForge.vue'

const api = vi.hoisted(() => ({
    project: vi.fn(),
    description: vi.fn(),
    files: vi.fn(),
    allFiles: vi.fn(),
    route: { path: '/project/curseforge/42', params: { id: '42' }, query: {} },
}))
vi.mock('vue-router', () => ({
    useRoute: () => reactive(api.route),
    useRouter: () => ({ push: vi.fn(), replace: vi.fn() }),
}))
vi.mock('@/helpers/curseforge', () => ({
    getCurseForgeProject: api.project,
    getCurseForgeDescription: api.description,
    getCurseForgeFilesPage: api.files,
    getCurseForgeFiles: api.allFiles,
    getCurseForgeImageUrl: (url: string) => url,
}))
vi.mock('@/helpers/tags', () => ({
    get_loaders: async () => [],
    get_game_versions: async () => [],
}))
vi.mock('@/helpers/content-search', () => ({ resolveMcmodUrl: async () => null }))
vi.mock('@/helpers/translation', () => ({
    autoTranslateHintMessages: {},
    getTranslationSettings: async () => ({ auto_translate: false }),
    getTranslationErrorKind: vi.fn(),
    noteManualTranslateClick: vi.fn(),
    prepareDescription: vi.fn(),
    translateInBatches: vi.fn(),
    validateTranslatedDescription: vi.fn(),
}))
vi.mock('@/i18n.config', () => ({ default: { global: { locale: ref('en-US') } } }))
vi.mock('@/store/state.js', () => ({ useTheming: () => ({ featureFlags: {} }) }))
vi.mock('@/store/breadcrumbs', () => ({
    useBreadcrumbs: () => ({ setName: vi.fn(), setNameIcon: vi.fn() }),
}))
vi.mock('@/composables/useContentFavorites', () => ({
    useContentFavorites: () => ({
        load: async () => [],
        isPending: () => false,
        isFavorite: () => false,
    }),
}))
vi.mock('@/providers/content-install', () => ({
    injectContentInstall: () => ({ installCurseForge: vi.fn(), installCurseForgeWorld: vi.fn() }),
}))
vi.mock('@/providers/content-selection', () => ({
    makeContentSelectionKey: () => '',
    injectContentSelection: () => ({
        targetInstance: ref(null),
        instances: ref([]),
        selectedCount: ref(0),
        selectedProjects: ref([]),
        state: ref('idle'),
        progress: ref(null),
        refreshInstances: vi.fn(),
        isSelected: () => false,
        isInstalling: () => false,
    }),
}))
vi.mock('@/components/browse/BrowseInstanceSelector.vue', () => ({
    default: { render: () => null },
}))
vi.mock('@/components/ui/ResourceDescription.vue', () => ({
    default: {
        name: 'ResourceDescription',
        setup:
            (_: unknown, { slots }: { slots: { default?: () => VNode[] } }) =>
            () =>
                h('div', slots.default?.()),
    },
}))
vi.mock('@/components/ui/TranslatedProjectDescription.vue', () => ({
    default: {
        name: 'TranslatedProjectDescription',
        props: ['description'],
        setup: (props: { description: string }) => () => h('p', props.description),
    },
}))
vi.mock('./Gallery.vue', () => ({ default: { render: () => null } }))
vi.mock('./Changelog.vue', () => ({ default: { render: () => null } }))
vi.mock('./UpgradeProjectReturnBar.vue', () => ({ default: { render: () => null } }))
vi.mock('@modrinth/ui', () => ({
    ...Object.fromEntries(
        [
            'BrowseInstallHeader',
            'Card',
            'Button',
            'NavTabs',
            'OverflowMenu',
            'ProjectBackgroundGradient',
            'ProjectHeader',
            'ProjectPageSkeleton',
            'ProjectPageVersions',
            'ProjectSidebarCompatibility',
            'ProjectSidebarCreators',
            'ProjectSidebarDetails',
            'ProjectSidebarLinks',
            'ProjectSidebarTags',
            'SelectedProjectsFloatingBar',
        ].map((name) => [
            name,
            {
                name,
                setup:
                    (_: unknown, { slots }: { slots: { default?: () => VNode[] } }) =>
                    () =>
                        h('div', slots.default?.()),
            },
        ]),
    ),
    defineMessage: (value: unknown) => value,
    defineMessages: (value: unknown) => value,
    commonMessages: { retryButton: { defaultMessage: 'Retry' } },
    usesTargetGameVersion: () => true,
    injectNotificationManager: () => ({ handleError: vi.fn(), addNotification: vi.fn() }),
    useVIntl: () => ({
        formatMessage: (message: { defaultMessage: string }) => message?.defaultMessage ?? '',
    }),
}))

const stubs = {
    Teleport: true,
    UpgradeProjectReturnBar: true,
    BrowseInstanceSelector: true,
    ProjectSidebarCompatibility: true,
    ProjectSidebarLinks: true,
    ProjectSidebarTags: true,
    ProjectSidebarCreators: true,
    ProjectSidebarDetails: true,
    ProjectHeader: defineComponent({
        props: ['project'],
        setup: (props) => () => h('h1', props.project.title),
    }),
    ProjectPageSkeleton: defineComponent({
        props: ['variant'],
        setup: (props) => () => h('div', { 'data-loading': props.variant ?? 'page' }),
    }),
    ResourceDescription: defineComponent({
        setup:
            (_: unknown, { slots }: { slots: { default?: () => VNode[] } }) =>
            () =>
                h('div', slots.default?.()),
    }),
    TranslatedProjectDescription: defineComponent({
        props: ['description'],
        setup: (props) => () => h('p', props.description),
    }),
    Button: defineComponent({
        setup:
            (_: unknown, { slots }: { slots: { default?: () => VNode[] } }) =>
            () =>
                h('button', slots.default?.()),
    }),
    NavTabs: true,
    ProjectPageVersions: true,
}
function render() {
    return mount(CurseForge, { global: { stubs, directives: { tooltip: {} } } })
}
function project(id = 42) {
    return {
        id,
        name: `Project ${id}`,
        slug: `${id}`,
        classId: 6,
        summary: 'Summary',
        categories: [],
        screenshots: [],
        authors: [],
        latestFilesIndexes: [],
        links: {},
        downloadCount: 0,
    }
}
beforeEach(() => {
    vi.clearAllMocks()
    Object.assign(reactive(api.route), {
        path: '/project/curseforge/42',
        params: { id: '42' },
        query: {},
    })
    api.project.mockResolvedValue(project())
    api.files.mockResolvedValue({ files: [], pagination: { totalCount: 0 } })
    api.allFiles.mockResolvedValue({ files: [] })
})

it('shows the title without waiting for description or files and retries only the description', async () => {
    let reject!: (reason: unknown) => void
    api.description
        .mockImplementationOnce(
            () =>
                new Promise((_, no) => {
                    reject = no
                }),
        )
        .mockResolvedValue('Recovered description')
    api.files.mockImplementationOnce(() => new Promise(() => {}))
    const wrapper = render()
    try {
        await flushPromises()
        expect(wrapper.get('h1').text()).toBe('Project 42')
        expect(wrapper.find('[data-loading="description"]').exists()).toBe(true)
        reject(new Error('offline'))
        await flushPromises()
        expect(wrapper.text()).toContain('The description could not be loaded.')
        await wrapper
            .findAll('button')
            .find((button) => button.text() === 'Retry')!
            .trigger('click')
        await flushPromises()
        expect(wrapper.text()).toContain('Recovered description')
        expect(api.project).toHaveBeenCalledTimes(1)
        expect(api.description).toHaveBeenCalledTimes(2)
    } finally {
        wrapper.unmount()
    }
})

it('discards an old description after navigating to another project', async () => {
    let resolve!: (value: string) => void
    api.description
        .mockImplementationOnce(
            () =>
                new Promise((done) => {
                    resolve = done
                }),
        )
        .mockResolvedValue('Current description')
    const wrapper = render()
    try {
        await flushPromises()
        api.project.mockResolvedValue(project(43))
        Object.assign(reactive(api.route), { path: '/project/curseforge/43', params: { id: '43' } })
        await flushPromises()
        resolve('Stale description')
        await flushPromises()
        expect(wrapper.get('h1').text()).toBe('Project 43')
        expect(wrapper.text()).toContain('Current description')
        expect(wrapper.text()).not.toContain('Stale description')
    } finally {
        wrapper.unmount()
    }
})

it('finishes the initial file page before requesting the complete versions list', async () => {
    api.route.path = '/project/curseforge/42/versions'
    api.description.mockResolvedValue('Description')
    let resolve!: (value: unknown) => void
    api.files.mockImplementationOnce(
        () =>
            new Promise((done) => {
                resolve = done
            }),
    )
    const wrapper = render()
    try {
        await flushPromises()
        expect(api.allFiles).not.toHaveBeenCalled()
        resolve({ files: [], pagination: { totalCount: 100 } })
        await flushPromises()
        expect(api.allFiles).toHaveBeenCalledTimes(1)
    } finally {
        wrapper.unmount()
    }
})

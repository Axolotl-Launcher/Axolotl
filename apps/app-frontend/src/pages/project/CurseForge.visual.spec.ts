import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, expect, it, vi } from 'vitest'
import type { VNode } from 'vue'
import { defineComponent, h, reactive, ref } from 'vue'

import CurseForge from './CurseForge.vue'

const api = vi.hoisted(() => ({
    selection: null as unknown,
    handleError: vi.fn(),
    remove: vi.fn(),
    release: vi.fn().mockResolvedValue('chosen-file'),
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
vi.mock('@/helpers/project-install-filters', async (original) => ({
    ...(await original<typeof import('@/helpers/project-install-filters')>()),
    filteredProjectRelease: api.release,
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
    makeContentSelectionKey: () => 'project-key',
    injectContentSelection: () =>
        (api.selection = {
            items: ref(new Map()),
            add: api.add,
            remove: api.remove,
            targetInstance: ref(null),
            instances: ref([]),
            selectedCount: ref(0),
            selectedProjects: ref([]),
            state: ref('idle'),
            progress: ref(null),
            refreshInstances: vi.fn(),
            isSelected: (key: string) =>
                (api.selection as { items: { value: Map<string, unknown> } }).items.value.has(key),
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
    injectNotificationManager: () => ({ handleError: api.handleError, addNotification: vi.fn() }),
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

it('uses filtered file IDs and replaces an already selected project with an explicit different file', async () => {
    Object.assign(reactive(api.route), { query: { b: '/browse/mod', g: '1.21.1', l: 'neoforge' } })
    const wrapper = render()
    try {
        await flushPromises()
        const selection = api.selection as {
            targetInstance: { value: unknown }
            isSelected: () => boolean
            items: { value: Map<string, object> }
        }
        selection.targetInstance.value = { id: 'fabric', game_version: '26.3', loader: 'fabric' }
        const vm = wrapper.vm as unknown as {
            installSelected: (id: string | null) => Promise<void>
        }
        await vm.installSelected(null)
        expect(api.release).toHaveBeenLastCalledWith(
            'curseforge',
            '42',
            { gameVersions: ['1.21.1'], loaders: ['neoforge'] },
            [],
            'mod',
        )
        expect(api.add).toHaveBeenLastCalledWith(
            expect.objectContaining({ versionId: 'chosen-file' }),
            expect.any(Function),
        )
        selection.items.value.set('project-key', {})
        api.remove.mockClear()
        api.release.mockClear()
        await vm.installSelected(null)
        expect(api.remove).toHaveBeenCalledOnce()
        expect(api.release).not.toHaveBeenCalled()
        selection.items.value.set('project-key', { versionId: 'chosen-file' })
        await vm.installSelected('explicit-file')
        expect(api.add).toHaveBeenLastCalledWith(
            expect.objectContaining({ versionId: 'explicit-file' }),
            expect.any(Function),
        )
    } finally {
        wrapper.unmount()
    }
})

it('keeps a filtered install busy and reports a failed lookup without an unhandled rejection', async () => {
    Object.assign(reactive(api.route), { query: { b: '/browse/mod', g: '1.21.1', l: 'neoforge' } })
    const wrapper = render()
    try {
        await flushPromises()
        const selection = api.selection as { targetInstance: { value: unknown } }
        selection.targetInstance.value = { id: 'fabric', game_version: '26.3', loader: 'fabric' }
        let fail!: (error: Error) => void
        api.release.mockImplementationOnce(
            () =>
                new Promise((_resolve, reject) => {
                    fail = reject
                }),
        )
        const vm = wrapper.vm as unknown as {
            installSelected: (id: string | null) => Promise<void>
            installing: boolean
        }
        const pending = vm.installSelected(null)
        expect(vm.installing).toBe(true)
        await vm.installSelected(null)
        expect(api.release).toHaveBeenCalledTimes(1)
        const error = new Error('lookup offline')
        fail(error)
        await pending
        expect(vm.installing).toBe(false)
        expect(api.handleError).toHaveBeenCalledWith(error)
    } finally {
        wrapper.unmount()
    }
})

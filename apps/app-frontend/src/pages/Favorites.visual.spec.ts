import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'

import type { ContentFavorite } from '@/helpers/content-favorites'
import type { SourceProjectMetadata } from '@/helpers/resources'

import Favorites from './Favorites.vue'

const fixture = vi.hoisted(() => ({
    query: {} as Record<string, string>,
    push: vi.fn(),
    replace: vi.fn(),
    load: vi.fn(),
    metadata: vi.fn(),
    prefetch: vi.fn(),
    resetStore: () => {},
    refresh: null as null | (() => void),
}))
vi.mock('vue-router', () => ({
    useRoute: () => ({
        query: fixture.query,
        fullPath: '/browse/favorites',
        path: '/browse/favorites',
    }),
    useRouter: () => ({ replace: fixture.replace, push: fixture.push }),
    onBeforeRouteLeave: () => {},
}))
vi.mock('@/store/breadcrumbs', () => ({
    useBreadcrumbs: () => ({ setName: vi.fn(), setContext: vi.fn() }),
}))
vi.mock('@/store/navigation-return', () => ({
    isBrowseReturnSourcePath: () => false,
    useNavigationReturnStore: () => ({ consumeBrowseReturnSnapshot: () => null }),
}))
vi.mock('@/helpers/page-transition', () => ({
    runAfterPageTransitionSettle: (run: () => void) => {
        fixture.refresh = run
        run()
    },
}))
vi.mock('@/helpers/cache.js', () => ({ get_project: vi.fn(), get_version_many: vi.fn() }))
vi.mock('@/helpers/curseforge', () => ({ getCurseForgeFiles: vi.fn() }))
vi.mock('@/helpers/settings', () => ({
    getLastBrowseContentDisplayMode: () => 'list',
    setLastBrowseContentDisplayMode: vi.fn(),
}))
vi.mock('@/helpers/content-favorites', () => ({
    contentFavoriteKey: (provider: string, id: string) => `${provider}:${id}`,
    prefetchContentFavorites: fixture.prefetch,
    resolveContentFavorite: fixture.metadata,
}))
vi.mock('@/composables/useContentFavorites', async () => {
    const { ref } = await import('vue')
    const favorites = ref<ContentFavorite[]>([])
    const loaded = ref(false)
    const metadata = new Map<string, SourceProjectMetadata>()
    fixture.resetStore = () => {
        favorites.value = []
        loaded.value = false
        metadata.clear()
    }
    return {
        useContentFavorites: () => {
            return {
                favorites,
                loaded,
                isPending: () => false,
                getMetadata: (favorite: ContentFavorite) => metadata.get(favorite.project_id),
                setMetadata: (favorite: ContentFavorite, value: SourceProjectMetadata) =>
                    metadata.set(favorite.project_id, value),
                load: async () => {
                    const result = await fixture.load()
                    if (JSON.stringify(favorites.value) !== JSON.stringify(result))
                        favorites.value = result
                    loaded.value = true
                    return result
                },
            }
        },
    }
})
vi.mock('@/providers/content-selection', async () => {
    const { ref } = await import('vue')
    return {
        makeContentSelectionKey: () => '',
        injectContentSelection: () => ({
            targetInstance: ref(null),
            instances: ref([]),
            state: ref('idle'),
            progress: ref(null),
            selectedProjects: ref([]),
            selectedCount: ref(0),
            refreshInstances: vi.fn(async () => {}),
            refreshInstalledIdentities: vi.fn(),
            isInstalling: () => false,
            isSelected: () => false,
        }),
    }
})
vi.mock('@/components/browse/BrowseInstanceSelector.vue', () => ({
    default: { render: () => null },
}))
vi.mock('@/components/ui/InstanceIcon.vue', () => ({ default: { render: () => null } }))
vi.mock('@modrinth/ui', async () => {
    const { defineComponent, h, ref } = await import('vue')
    const container = defineComponent({
        setup:
            (_, { slots }) =>
            () =>
                h('div', slots.default?.()),
    })
    return {
        ...Object.fromEntries(
            [
                'BrowseInstallHeader',
                'NavTabs',
                'Pagination',
                'PopoutMenu',
                'ProjectCardList',
                'SelectedProjectsFloatingBar',
                'StyledInput',
            ].map((name) => [name, container]),
        ),
        ProjectCardSkeleton: defineComponent({
            setup: () => () => h('div', { 'data-skeleton': '' }),
        }),
        Button: defineComponent({
            setup:
                (_, { slots }) =>
                () =>
                    h('button', slots.default?.()),
        }),
        Admonition: defineComponent({
            props: ['header'],
            setup:
                (props, { slots }) =>
                () =>
                    h('aside', { role: 'alert' }, [props.header, slots.default?.()]),
        }),
        EmptyState: defineComponent({
            props: ['heading'],
            setup:
                (props, { slots }) =>
                () =>
                    h('section', [props.heading, slots.actions?.()]),
        }),
        ProjectCard: defineComponent({
            props: ['title'],
            setup: (props) => () => h('article', { 'data-favorite': '' }, props.title),
        }),
        getLatestMatchingInstallVersion: vi.fn(),
        getTargetInstallPreferences: vi.fn(),
        commonMessages: { retryButton: { defaultMessage: 'Retry' } },
        defineMessage: (value: unknown) => value,
        defineMessages: (value: unknown) => value,
        injectNotificationManager: () => ({ handleError: vi.fn() }),
        useStickyObserver: () => ({ isStuck: ref(false) }),
        useVIntl: () => ({
            formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
        }),
    }
})

const wrappers: ReturnType<typeof mount>[] = []
afterEach(() => {
    wrappers.splice(0).forEach((wrapper) => wrapper.unmount())
    vi.clearAllMocks()
    fixture.query = {}
    fixture.resetStore()
    fixture.prefetch.mockReset().mockResolvedValue(new Map())
})
function render() {
    const wrapper = mount(Favorites, { global: { directives: { tooltip: {} } } })
    wrappers.push(wrapper)
    return wrapper
}

fixture.prefetch.mockResolvedValue(new Map())

const savedFavorite: ContentFavorite = {
    provider: 'modrinth',
    project_id: 'one',
    content_type: 'mod',
    saved_at: 1,
}
const savedMetadata = {
    project: { source: 'modrinth', projectId: 'one' },
    title: 'Saved mod',
    summary: 'A saved project',
    categories: [],
    gallery: [],
} as unknown as SourceProjectMetadata

it('shows a saved list immediately after remount while background refresh is pending or fails', async () => {
    fixture.load.mockResolvedValue([savedFavorite])
    fixture.metadata.mockResolvedValue(savedMetadata)
    const first = render()
    await vi.waitFor(() => expect(first.text()).toContain('Saved mod'))
    first.unmount()
    wrappers.splice(wrappers.indexOf(first), 1)
    let fail!: (error: Error) => void
    fixture.load.mockReturnValue(
        new Promise((_, reject) => {
            fail = reject
        }),
    )
    const second = render()
    expect(second.text()).toContain('Saved mod')
    expect(second.find('[data-skeleton]').exists()).toBe(false)
    fail(new Error('offline'))
    await vi.waitFor(() => expect(second.text()).toContain('Failed to load favorites'))
    expect(second.text()).toContain('Saved mod')
})

it('renders persistent cached metadata before the remote refresh completes', async () => {
    fixture.load.mockResolvedValue([savedFavorite])
    fixture.prefetch.mockImplementation(async (_, __, policy) => {
        if (policy === 'onlyCached') return new Map([['modrinth:one', savedMetadata]])
        return new Promise(() => {})
    })
    const wrapper = render()
    await vi.waitFor(() => expect(wrapper.text()).toContain('Saved mod'))
    expect(fixture.metadata).not.toHaveBeenCalled()
    expect(wrapper.find('[aria-busy="true"]').exists()).toBe(true)
})

it('keeps the loading state for unmatched partial cache until the full collection is resolved', async () => {
    fixture.query = { q: 'Other mod' }
    fixture.load.mockResolvedValue([savedFavorite, { ...savedFavorite, project_id: 'two' }])
    fixture.prefetch.mockImplementation(async (_, __, policy) => {
        if (policy === 'onlyCached') return new Map([['modrinth:one', savedMetadata]])
        return new Promise(() => {})
    })
    const wrapper = render()
    await vi.waitFor(() => expect(fixture.prefetch).toHaveBeenCalledTimes(2))
    expect(wrapper.find('[data-skeleton]').exists()).toBe(true)
    expect(wrapper.text()).not.toContain('No matching favorites')
})

it('does not reset the requested page while only part of the cache is available', async () => {
    fixture.query = { page: '2' }
    fixture.load.mockResolvedValue([savedFavorite, { ...savedFavorite, project_id: 'two' }])
    fixture.prefetch.mockImplementation(async (_, __, policy) => {
        if (policy === 'onlyCached') return new Map([['modrinth:one', savedMetadata]])
        return new Promise(() => {})
    })
    const wrapper = render()
    await vi.waitFor(() => expect(wrapper.text()).toContain('Saved mod'))
    expect(fixture.replace).not.toHaveBeenCalled()
})

it('does not refresh a disposed page when the deferred transition callback fires', async () => {
    fixture.load.mockResolvedValue([])
    const wrapper = render()
    await vi.waitFor(() => expect(wrapper.text()).toContain('No favorites yet'))
    wrapper.unmount()
    wrappers.splice(wrappers.indexOf(wrapper), 1)
    fixture.load.mockClear()
    fixture.refresh!()
    await flushPromises()
    expect(fixture.load).not.toHaveBeenCalled()
})

it('keeps cached metadata when the source is unavailable during refresh', async () => {
    fixture.load.mockResolvedValue([savedFavorite])
    fixture.prefetch.mockImplementation(async (_, __, policy) =>
        policy === 'onlyCached' ? new Map([['modrinth:one', savedMetadata]]) : new Map(),
    )
    fixture.metadata.mockRejectedValue(new Error('source offline'))
    const wrapper = render()
    await vi.waitFor(() => expect(wrapper.text()).toContain('Saved mod'))
    await flushPromises()
    expect(wrapper.text()).toContain('Saved mod')
    expect(wrapper.text()).not.toContain('This project is currently unavailable')
})

it('shows a collection read failure with retry, then reports a genuine empty collection', async () => {
    fixture.load.mockRejectedValueOnce(new Error('read failed')).mockResolvedValue([])
    const wrapper = render()
    await vi.waitFor(() => expect(wrapper.text()).toContain('Failed to load favorites'))
    expect(wrapper.text()).not.toContain('No favorites yet')
    await wrapper
        .findAll('button')
        .find((button) => button.text() === 'Retry')!
        .trigger('click')
    await vi.waitFor(() => expect(wrapper.text()).toContain('No favorites yet'))
    expect(wrapper.text()).not.toContain('Failed to load favorites')
})

it('preserves existing favorites after refresh failure and keeps item metadata failure local', async () => {
    fixture.load.mockResolvedValue([
        { provider: 'modrinth', project_id: 'one', content_type: 'mod', saved_at: 1 },
    ])
    fixture.metadata.mockRejectedValue(new Error('metadata unavailable'))
    const wrapper = render()
    await vi.waitFor(() => expect(wrapper.findAll('[data-favorite]')).toHaveLength(1))
    expect(wrapper.text()).not.toContain('Failed to load favorites')
    fixture.load.mockRejectedValue(new Error('refresh failed'))
    fixture.refresh!()
    await flushPromises()
    await vi.waitFor(() => expect(wrapper.find('[role="alert"]').exists()).toBe(true))
    expect(wrapper.findAll('[data-favorite]')).toHaveLength(1)
})

it('starts browsing from an empty collection while preserving the installation target', async () => {
    fixture.query = {
        i: 'selected-instance',
        q: 'old query',
        kind: 'shader',
        page: '3',
        server: 'target',
    }
    fixture.load.mockResolvedValue([])
    const wrapper = render()
    await vi.waitFor(() => expect(wrapper.text()).toContain('No favorites yet'))
    await wrapper
        .findAll('button')
        .find((button) => button.text() === 'Browse resources')!
        .trigger('click')
    expect(fixture.push).toHaveBeenCalledWith({
        path: '/browse/mod',
        query: { i: 'selected-instance', server: 'target' },
    })
})

it('clears search and filters together without deleting favorites or losing the target', async () => {
    fixture.query = { i: 'selected-instance', q: 'no-match', kind: 'shader', page: '2' }
    fixture.load.mockResolvedValue([
        { provider: 'modrinth', project_id: 'one', content_type: 'mod', saved_at: 1 },
    ])
    fixture.metadata.mockRejectedValue(new Error('unavailable'))
    const wrapper = render()
    await vi.waitFor(() => expect(wrapper.text()).toContain('No matching favorites'))
    await wrapper
        .findAll('button')
        .find((button) => button.text() === 'Clear search and filters')!
        .trigger('click')
    expect(fixture.replace).toHaveBeenCalledWith({
        query: { i: 'selected-instance', q: undefined, kind: undefined, page: undefined },
    })
    expect(fixture.push).not.toHaveBeenCalled()
})

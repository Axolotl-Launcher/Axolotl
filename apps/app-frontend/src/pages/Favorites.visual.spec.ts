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
    setFavorites: (_values: ContentFavorite[]) => {},
    refresh: null as null | (() => void),
}))
vi.mock('vue-router', async () => {
    const { reactive } = await import('vue')
    return {
        useRoute: () => ({
            query: reactive(fixture.query),
            fullPath: '/browse/favorites',
            path: '/browse/favorites',
        }),
        useRouter: () => ({ replace: fixture.replace, push: fixture.push }),
        onBeforeRouteLeave: () => {},
    }
})
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
    fixture.setFavorites = (values) => {
        favorites.value = values
    }
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
        Pagination: defineComponent({
            props: ['page', 'count'],
            setup: (props) => () =>
                h('nav', { 'data-page': props.page, 'data-count': props.count }),
        }),
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
            props: ['title', 'link'],
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

it.each([
    ['mcarchive', undefined, '/project/mcarchive/one'],
    ['planet_minecraft', undefined, '/project/planet-minecraft/one'],
    ['modrinth', 'confirmed', '/resource/confirmed'],
] as const)('passes a navigation title for %s favorites', async (provider, resourceId, path) => {
    fixture.load.mockResolvedValue([{ ...savedFavorite, provider, resource_id: resourceId }])
    fixture.metadata.mockResolvedValue({
        ...savedMetadata,
        project: { source: provider, projectId: 'one' },
    })
    const wrapper = render()
    await vi.waitFor(() => expect(wrapper.find('[data-favorite]').exists()).toBe(true))
    const card = wrapper.findComponent('[data-favorite]')
    expect(card.props('link')).toEqual({
        path,
        query: expect.objectContaining({ b: '/browse/favorites' }),
        state: { projectPreview: { targetPath: path, title: 'Saved mod' } },
    })
})

const archiveFavorite: ContentFavorite = { ...savedFavorite, provider: 'mcarchive' }
const planetFavorite: ContentFavorite = {
    ...savedFavorite,
    provider: 'planet_minecraft',
    project_id: 'two',
    saved_at: 2,
}
function metadataFor(favorite: ContentFavorite, title: string): SourceProjectMetadata {
    return {
        ...savedMetadata,
        project: { source: favorite.provider, projectId: favorite.project_id },
        title,
    }
}

it('publishes archive projects while unrelated batched sources and other items are pending', async () => {
    const modrinth = { ...savedFavorite, project_id: 'three' }
    fixture.load.mockResolvedValue([archiveFavorite, planetFavorite, modrinth])
    let finishPlanet!: (value: SourceProjectMetadata) => void
    let finishBatch!: (value: Map<string, SourceProjectMetadata>) => void
    fixture.prefetch.mockImplementation(async (_, __, policy) =>
        policy === 'onlyCached'
            ? new Map()
            : new Promise((resolve) => {
                  finishBatch = resolve
              }),
    )
    fixture.metadata.mockImplementation(async (favorite: ContentFavorite) =>
        favorite.provider === 'mcarchive'
            ? metadataFor(favorite, 'Archive ready')
            : new Promise((resolve) => {
                  finishPlanet = resolve
              }),
    )
    const wrapper = render()
    await vi.waitFor(() => expect(wrapper.text()).toContain('Archive ready'))
    expect(wrapper.findAll('[data-skeleton]')).toHaveLength(2)
    expect(wrapper.findAll('[data-favorite], [data-skeleton]').map((row) => row.text())).toEqual([
        'Archive ready',
        '',
        '',
    ])
    finishBatch(new Map([['modrinth:three', metadataFor(modrinth, 'Batched ready')]]))
    await flushPromises()
    expect(wrapper.text()).toContain('Batched ready')
    expect(wrapper.findAll('[data-skeleton]')).toHaveLength(1)
    finishPlanet(metadataFor(planetFavorite, 'Planet ready'))
    await vi.waitFor(() => expect(wrapper.findAll('[data-favorite]')).toHaveLength(3))
    expect(wrapper.findAll('[data-favorite]').map((row) => row.text())).toEqual([
        'Archive ready',
        'Planet ready',
        'Batched ready',
    ])
})

it('keeps the complete filtered view until the latest filter and metadata can commit together', async () => {
    fixture.query = { q: 'Saved' }
    fixture.load.mockResolvedValue([archiveFavorite, planetFavorite])
    fixture.metadata.mockImplementation(async (favorite: ContentFavorite) =>
        metadataFor(favorite, `Saved ${favorite.project_id}`),
    )
    const wrapper = render()
    await vi.waitFor(() => expect(wrapper.findAll('[data-favorite]')).toHaveLength(2))
    let finish!: (value: SourceProjectMetadata) => void
    fixture.metadata.mockImplementation(async (favorite: ContentFavorite) =>
        favorite.provider === 'mcarchive'
            ? metadataFor(favorite, 'New one')
            : new Promise((resolve) => {
                  finish = resolve
              }),
    )
    fixture.refresh!()
    const { useRoute } = await import('vue-router')
    useRoute().query.q = 'New'
    await flushPromises()
    expect(wrapper.text()).toContain('Saved one')
    expect(wrapper.text()).not.toContain('New one')
    expect(wrapper.text()).toContain('Updating favorites')
    useRoute().query.q = 'New two'
    finish(metadataFor(planetFavorite, 'New two'))
    await vi.waitFor(() => expect(wrapper.findAll('[data-favorite]')).toHaveLength(1))
    expect(wrapper.find('[data-favorite]').text()).toBe('New two')
    expect(wrapper.text()).not.toContain('Saved one')
})

it('does not publish partial matches or an empty state on a cold filtered visit', async () => {
    fixture.query = { q: 'Saved' }
    fixture.load.mockResolvedValue([archiveFavorite, planetFavorite])
    let fail!: (reason: Error) => void
    fixture.metadata.mockImplementation(async (favorite: ContentFavorite) =>
        favorite.provider === 'mcarchive'
            ? metadataFor(favorite, 'Saved archive')
            : new Promise((_, reject) => {
                  fail = reject
              }),
    )
    const wrapper = render()
    await vi.waitFor(() => expect(fixture.metadata).toHaveBeenCalled())
    await flushPromises()
    expect(wrapper.find('[data-skeleton]').exists()).toBe(true)
    expect(wrapper.find('[data-favorite]').exists()).toBe(false)
    expect(wrapper.text()).not.toContain('No matching favorites')
    fail(new Error('unavailable'))
    await vi.waitFor(() => expect(wrapper.text()).toContain('Saved archive'))
    expect(wrapper.findAll('[data-favorite]')).toHaveLength(1)
})

it('keeps pagination based on the saved collection while page metadata is pending', async () => {
    fixture.query = { page: '2' }
    const favorites = Array.from({ length: 25 }, (_, index) => ({
        ...archiveFavorite,
        project_id: String(index),
    }))
    fixture.load.mockResolvedValue(favorites)
    fixture.metadata.mockImplementation(() => new Promise(() => {}))
    const wrapper = render()
    await vi.waitFor(() => expect(wrapper.find('[data-count="2"]').exists()).toBe(true))
    expect(wrapper.find('[data-page="2"]').exists()).toBe(true)
    expect(wrapper.findAll('[data-skeleton]')).toHaveLength(5)
    expect(fixture.replace).not.toHaveBeenCalled()
})

it('discards a removed favorite and its late metadata response', async () => {
    fixture.load.mockResolvedValue([archiveFavorite])
    let finish!: (value: SourceProjectMetadata) => void
    let oldSignal!: AbortSignal
    fixture.metadata.mockImplementationOnce((_favorite, signal) => {
        oldSignal = signal
        return new Promise((resolve) => {
            finish = resolve
        })
    })
    const wrapper = render()
    await vi.waitFor(() => expect(finish).toBeDefined())
    fixture.load.mockResolvedValue([])
    fixture.setFavorites([])
    await flushPromises()
    expect(oldSignal.aborted).toBe(true)
    finish(metadataFor(archiveFavorite, 'Late archive'))
    await flushPromises()
    expect(wrapper.text()).not.toContain('Late archive')
    expect(wrapper.text()).toContain('No favorites yet')
})

it('continues local search and type filtering after a collection refresh fails', async () => {
    fixture.load.mockResolvedValue([archiveFavorite])
    fixture.metadata.mockResolvedValue(metadataFor(archiveFavorite, 'Saved archive'))
    const wrapper = render()
    await vi.waitFor(() => expect(wrapper.text()).toContain('Saved archive'))
    await flushPromises()
    fixture.load.mockRejectedValue(new Error('offline'))
    fixture.refresh!()
    await vi.waitFor(() => expect(wrapper.text()).toContain('Failed to load favorites'))
    const { useRoute } = await import('vue-router')
    const query = useRoute().query
    query.q = 'does not match'
    await flushPromises()
    expect(wrapper.text()).toContain('No matching favorites')
    expect(wrapper.find('[data-favorite]').exists()).toBe(false)
    query.q = ''
    query.kind = 'shader'
    await flushPromises()
    expect(wrapper.find('[data-favorite]').exists()).toBe(false)
    query.kind = 'all'
    await flushPromises()
    expect(wrapper.text()).toContain('Saved archive')
})

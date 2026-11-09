import { flushPromises, mount, type VueWrapper } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'

import { get_project, get_version_many } from '@/helpers/cache.js'
import type { ContentFavorite } from '@/helpers/content-favorites'
import type { SourceProjectMetadata } from '@/helpers/resources'

import Favorites from './Favorites.vue'

const fixture = vi.hoisted(() => ({
    instanceEvent: null as
        null | ((event: { event: string; instance_id: string }) => Promise<void>),
    localeRef: null as unknown as { value: string },
    translate: vi.fn(async (hits: { title: string }[], _locale: string) => hits),
    query: {} as Record<string, string>,
    push: vi.fn(),
    replace: vi.fn(),
    load: vi.fn(),
    metadata: vi.fn(),
    prefetch: vi.fn(),
    resetStore: () => {},
    setFavorites: (_values: ContentFavorite[]) => {},
    setGroups: (_values: { id: string; name: string }[]) => {},
    setMemberships: (_values: Record<string, string[]>) => {},
    updateFavoriteGroups: vi.fn(async () => {}),
    setTarget: (_value: unknown) => {},
    addToSelection: vi.fn(async (_item: { key: string }) => true),
    installSelected: vi.fn(async () => true),
    refreshInstances: vi.fn(async () => null as unknown),
    refreshInstalledIdentities: vi.fn(async () => {}),
    showInstanceSelector: vi.fn(),
    installedKeys: new Set<string>(),
    resetSelection: () => {},
    renderCardActions: false,
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
vi.mock('@/i18n.config', async () => {
    const { ref } = await import('vue')
    fixture.localeRef = ref('en-US')
    return { default: { global: { locale: fixture.localeRef } } }
})
vi.mock('@/helpers/content-search', () => ({ translateSearchHitTitles: fixture.translate }))
vi.mock('@/helpers/events.js', () => ({
    instance_listener: async (
        listener: (event: { event: string; instance_id: string }) => Promise<void>,
    ) => {
        fixture.instanceEvent = listener
        return () => {}
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
vi.mock('@/composables/useFavoriteGroups', async () => {
    const { ref } = await import('vue')
    const groups = ref<{ id: string; name: string }[]>([])
    const memberships = ref(new Map<string, string[]>())
    const loaded = ref(true)
    fixture.setGroups = (values) => {
        groups.value = values
    }
    fixture.setMemberships = (values) => {
        memberships.value = new Map(Object.entries(values))
    }
    return {
        useFavoriteGroups: () => ({
            groups,
            memberships,
            loaded,
            groupsFor: (key: string) => memberships.value.get(key) ?? [],
            isInGroup: (key: string, id: string) => (memberships.value.get(key) ?? []).includes(id),
            createGroup: vi.fn(async (name: string) => ({ id: `created:${name}`, name })),
            renameGroup: vi.fn(async (id: string, name: string) => ({ id, name })),
            deleteGroup: vi.fn(async () => {}),
            updateFavoriteGroups: fixture.updateFavoriteGroups,
            refresh: vi.fn(async () => {}),
        }),
    }
})
vi.mock('@/providers/content-selection', async () => {
    const { ref } = await import('vue')
    const targetInstance = ref<unknown>(null)
    const selected = new Set<string>()
    const selectedCount = ref(0)
    fixture.setTarget = (value) => {
        targetInstance.value = value
    }
    fixture.refreshInstances.mockImplementation(async () => targetInstance.value)
    fixture.resetSelection = () => {
        selected.clear()
        selectedCount.value = 0
    }
    return {
        makeContentSelectionKey: (provider: string, projectId: string) =>
            `${provider}:${projectId}`,
        injectContentSelection: () => ({
            targetInstance,
            instances: ref([]),
            state: ref('idle'),
            progress: ref({ completed: 0, total: 0 }),
            selectedProjects: ref([]),
            selectedCount,
            refreshInstances: fixture.refreshInstances,
            refreshInstalledIdentities: fixture.refreshInstalledIdentities,
            isInstalling: () => false,
            isSelected: (key: string) => selected.has(key),
            isInstalledIdentity: (provider: string, projectId: string) =>
                fixture.installedKeys.has(`${provider}:${projectId}`),
            add: async (item: { key: string }) => {
                const accepted = await fixture.addToSelection(item)
                if (accepted) {
                    selected.add(item.key)
                    selectedCount.value = selected.size
                }
                return accepted
            },
            remove: (key: string) => {
                selected.delete(key)
                selectedCount.value = selected.size
            },
            clear: () => {
                selected.clear()
                selectedCount.value = 0
            },
            installSelected: fixture.installSelected,
            setTarget: vi.fn(),
        }),
    }
})
vi.mock('@/components/browse/BrowseInstanceSelector.vue', async () => {
    const { defineComponent } = await import('vue')
    return {
        default: defineComponent({
            setup: (_props, { expose }) => {
                expose({ show: fixture.showInstanceSelector })
                return () => null
            },
        }),
    }
})
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
                'IconButton',
                'NavTabs',
                'Pagination',
                'ProjectCardList',
                'SearchSidebarFilter',
                'SelectedProjectsFloatingBar',
                'StyledInput',
                'TagItem',
            ].map((name) => [name, container]),
        ),
        PopoutMenu: defineComponent({
            setup:
                (_, { slots }) =>
                () =>
                    h('div', [slots.default?.(), slots.menu?.({ hide: () => {} })]),
        }),
        Checkbox: defineComponent({
            props: ['modelValue', 'label'],
            emits: ['update:modelValue'],
            setup:
                (props, { emit }) =>
                () =>
                    h(
                        'button',
                        {
                            'data-checkbox': '',
                            'aria-checked': String(!!props.modelValue),
                            onClick: () => emit('update:modelValue', !props.modelValue),
                        },
                        props.label,
                    ),
        }),
        ConfirmModal: defineComponent({
            props: ['title', 'description', 'proceedLabel'],
            setup: (props, { expose }) => {
                expose({ show: () => {}, hide: () => {} })
                return () =>
                    h('div', { 'data-confirm-modal': '' }, [props.title, props.description])
            },
        }),
        NewModal: defineComponent({
            props: ['header'],
            setup: (props, { slots, expose }) => {
                expose({ show: () => {}, hide: () => {} })
                return () =>
                    h('div', { 'data-modal': '' }, [
                        props.header,
                        slots.default?.(),
                        slots.actions?.(),
                    ])
            },
        }),
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
        SelectAllButton: defineComponent({
            props: ['count', 'progress', 'disabled'],
            emits: ['select'],
            setup:
                (props, { emit }) =>
                () =>
                    props.count > 0 || props.progress
                        ? h(
                              'button',
                              { onClick: () => emit('select') },
                              props.progress
                                  ? `Selecting ${props.progress.completed}/${props.progress.total}`
                                  : `Select all (${props.count})`,
                          )
                        : null,
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
            setup:
                (props, { slots }) =>
                () =>
                    h('article', { 'data-favorite': '' }, [
                        props.title,
                        fixture.renderCardActions ? slots.actions?.() : undefined,
                    ]),
        }),
        getLatestMatchingInstallVersion: vi.fn(() => ({ id: 'version' })),
        getTargetInstallPreferences: vi.fn(() => ({})),
        commonMessages: {
            retryButton: { defaultMessage: 'Retry' },
            cancelButton: { defaultMessage: 'Cancel' },
            installButton: { defaultMessage: 'Install' },
            installedLabel: { defaultMessage: 'Installed' },
            installingContentLabel: { defaultMessage: 'Installing content' },
            validatingLabel: { defaultMessage: 'Validating' },
        },
        defineMessage: (value: unknown) => value,
        defineMessages: (value: unknown) => value,
        formatCategory: (_format: unknown, category: string) => category,
        injectNotificationManager: () => ({ handleError: vi.fn() }),
        useStickyObserver: () => ({ isStuck: ref(false) }),
        useVIntl: () => ({
            formatMessage: (
                message: { defaultMessage?: string },
                values?: Record<string, unknown>,
            ) =>
                String(message.defaultMessage ?? '').replace(/\{(\w+)\}/g, (_match, key) =>
                    String(values?.[key] ?? `{${key}}`),
                ),
        }),
    }
})

const wrappers: ReturnType<typeof mount>[] = []
afterEach(() => {
    wrappers.splice(0).forEach((wrapper) => wrapper.unmount())
    vi.clearAllMocks()
    fixture.query = {}
    fixture.resetStore()
    fixture.setGroups([])
    fixture.setMemberships({})
    fixture.setTarget(null)
    fixture.renderCardActions = false
    fixture.installedKeys.clear()
    fixture.localeRef.value = 'en-US'
    fixture.translate.mockReset().mockImplementation(async (hits) => hits)
    fixture.resetSelection()
    fixture.addToSelection.mockReset().mockResolvedValue(true)
    fixture.installSelected.mockReset().mockResolvedValue(true)
    fixture.updateFavoriteGroups.mockReset().mockResolvedValue(undefined)
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
const bulkTarget = {
    id: 'instance-1',
    name: 'Example',
    loader: 'neoforge',
    game_version: '1.21.1',
    icon_path: null,
}

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
    const card = wrapper.findComponent('[data-favorite]') as unknown as VueWrapper<
        Record<string, unknown>
    >
    expect(card.props()).toMatchObject({
        link: {
            path,
            query: expect.objectContaining({ b: '/browse/favorites' }),
            state: { projectPreview: { targetPath: path, title: 'Saved mod' } },
        },
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
    expect(wrapper.text()).not.toContain('Updating favorites')
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

it('filters favorites by a custom group and by ungrouped', async () => {
    const second = { ...savedFavorite, project_id: 'two', saved_at: 2 }
    fixture.load.mockResolvedValue([savedFavorite, second])
    fixture.metadata.mockImplementation(async (favorite: ContentFavorite) =>
        metadataFor(favorite, favorite.project_id === 'one' ? 'First' : 'Second'),
    )
    fixture.setGroups([{ id: 'group-a', name: 'Group A' }])
    fixture.setMemberships({ 'modrinth:one': ['group-a'] })
    fixture.query = { group: 'group-a' }

    const wrapper = render()
    await vi.waitFor(() => expect(wrapper.findAll('[data-favorite]')).toHaveLength(1))
    expect(wrapper.find('[data-favorite]').text()).toBe('First')
    expect(wrapper.text()).toContain('Group A')

    const { useRoute } = await import('vue-router')
    useRoute().query.group = 'none'
    await vi.waitFor(() => expect(wrapper.findAll('[data-favorite]')).toHaveLength(1))
    expect(wrapper.find('[data-favorite]').text()).toBe('Second')
})

it('filters by categories with AND semantics', async () => {
    const second = { ...savedFavorite, project_id: 'two', saved_at: 2 }
    fixture.load.mockResolvedValue([savedFavorite, second])
    fixture.metadata.mockImplementation(async (favorite: ContentFavorite) => ({
        ...metadataFor(favorite, favorite.project_id === 'one' ? 'First' : 'Second'),
        categories: favorite.project_id === 'one' ? ['adventure', 'technology'] : ['adventure'],
    }))
    fixture.query = { cat: 'adventure' }

    const wrapper = render()
    await vi.waitFor(() => expect(wrapper.findAll('[data-favorite]')).toHaveLength(2))

    const { useRoute } = await import('vue-router')
    useRoute().query.cat = 'adventure,technology'
    await vi.waitFor(() => expect(wrapper.findAll('[data-favorite]')).toHaveLength(1))
    expect(wrapper.find('[data-favorite]').text()).toBe('First')
})

it('toggles group membership from the card group menu', async () => {
    fixture.renderCardActions = true
    fixture.load.mockResolvedValue([savedFavorite])
    fixture.metadata.mockResolvedValue(metadataFor(savedFavorite, 'Saved mod'))
    fixture.setGroups([{ id: 'group-a', name: 'Group A' }])

    const wrapper = render()
    await vi.waitFor(() => expect(wrapper.find('[data-favorite]').exists()).toBe(true))
    const checkbox = wrapper
        .findAll('[data-checkbox]')
        .find((node) => node.text().includes('Group A'))!
    await checkbox.trigger('click')

    expect(fixture.updateFavoriteGroups).toHaveBeenCalledWith('modrinth:one', ['group-a'], [])
})

it('selects every favorite in the filtered result', async () => {
    const favorites = Array.from({ length: 21 }, (_, index) => ({
        ...savedFavorite,
        project_id: `mod-${index}`,
        saved_at: index + 1,
    }))
    fixture.load.mockResolvedValue(favorites)
    fixture.metadata.mockImplementation(async (favorite: ContentFavorite) =>
        metadataFor(favorite, `Mod ${favorite.project_id}`),
    )
    fixture.setTarget(bulkTarget)
    vi.mocked(get_project).mockResolvedValue({ versions: ['v1'] } as never)
    vi.mocked(get_version_many).mockResolvedValue([{ id: 'v1' }] as never)
    fixture.query = { kind: 'mod' }

    const wrapper = render()
    await vi.waitFor(() => expect(wrapper.text()).toContain('Select all (21)'))
    await wrapper
        .findAll('button')
        .find((button) => button.text().includes('Select all (21)'))!
        .trigger('click')
    await vi.waitFor(() => expect(fixture.addToSelection).toHaveBeenCalledTimes(21))
    expect(fixture.installSelected).not.toHaveBeenCalled()
})

it('scopes select all to the active group and content type', async () => {
    const modOne = {
        ...savedFavorite,
        project_id: 'one',
        content_type: 'mod' as const,
        saved_at: 1,
    }
    const modTwo = {
        ...savedFavorite,
        project_id: 'two',
        content_type: 'mod' as const,
        saved_at: 2,
    }
    const pack = {
        ...savedFavorite,
        project_id: 'three',
        content_type: 'resourcepack' as const,
        saved_at: 3,
    }
    fixture.load.mockResolvedValue([modOne, modTwo, pack])
    fixture.metadata.mockImplementation(async (favorite: ContentFavorite) =>
        metadataFor(favorite, favorite.project_id),
    )
    fixture.setGroups([{ id: 'group-a', name: 'Group A' }])
    fixture.setMemberships({
        'modrinth:one': ['group-a'],
        'modrinth:two': ['group-a'],
        'modrinth:three': ['group-a'],
    })
    fixture.setTarget(bulkTarget)
    vi.mocked(get_project).mockResolvedValue({ versions: ['v1'] } as never)
    vi.mocked(get_version_many).mockResolvedValue([{ id: 'v1' }] as never)
    fixture.query = { group: 'group-a', kind: 'mod' }

    const wrapper = render()
    await vi.waitFor(() => expect(wrapper.text()).toContain('Select all (2)'))
    await wrapper
        .findAll('button')
        .find((button) => button.text().includes('Select all (2)'))!
        .trigger('click')
    await vi.waitFor(() => expect(fixture.addToSelection).toHaveBeenCalledTimes(2))
    expect(fixture.addToSelection).toHaveBeenCalledWith(
        expect.objectContaining({ provider: 'modrinth', projectId: 'one' }),
    )
    expect(fixture.addToSelection).toHaveBeenCalledWith(
        expect.objectContaining({ provider: 'modrinth', projectId: 'two' }),
    )
    expect(fixture.installSelected).not.toHaveBeenCalled()
})

it('scopes select all to the active category filter', async () => {
    const second = { ...savedFavorite, project_id: 'two', saved_at: 2 }
    fixture.load.mockResolvedValue([savedFavorite, second])
    fixture.metadata.mockImplementation(async (favorite: ContentFavorite) => ({
        ...metadataFor(favorite, favorite.project_id),
        categories: favorite.project_id === 'one' ? ['adventure'] : ['technology'],
    }))
    fixture.setTarget(bulkTarget)
    vi.mocked(get_project).mockResolvedValue({ versions: ['v1'] } as never)
    vi.mocked(get_version_many).mockResolvedValue([{ id: 'v1' }] as never)
    fixture.query = { cat: 'adventure' }

    const wrapper = render()
    await vi.waitFor(() => expect(wrapper.text()).toContain('Select all (1)'))
    await wrapper
        .findAll('button')
        .find((button) => button.text().includes('Select all (1)'))!
        .trigger('click')
    await vi.waitFor(() => expect(fixture.addToSelection).toHaveBeenCalledTimes(1))
    expect(fixture.addToSelection).toHaveBeenCalledWith(
        expect.objectContaining({ projectId: 'one' }),
    )
    expect(fixture.installSelected).not.toHaveBeenCalled()
})

it('skips favorites already installed in the target instance', async () => {
    const second = { ...savedFavorite, project_id: 'two', saved_at: 2 }
    fixture.load.mockResolvedValue([savedFavorite, second])
    fixture.metadata.mockImplementation(async (favorite: ContentFavorite) =>
        metadataFor(favorite, favorite.project_id),
    )
    fixture.setTarget(bulkTarget)
    fixture.installedKeys.add('modrinth:one')
    vi.mocked(get_project).mockResolvedValue({ versions: ['v1'] } as never)
    vi.mocked(get_version_many).mockResolvedValue([{ id: 'v1' }] as never)

    const wrapper = render()
    await vi.waitFor(() => expect(wrapper.text()).toContain('Select all (1)'))
    await wrapper
        .findAll('button')
        .find((button) => button.text().includes('Select all (1)'))!
        .trigger('click')
    await vi.waitFor(() => expect(fixture.addToSelection).toHaveBeenCalledTimes(1))
    expect(fixture.addToSelection).toHaveBeenCalledWith(
        expect.objectContaining({ projectId: 'two' }),
    )
    expect(fixture.installSelected).not.toHaveBeenCalled()
})

it('opens the instance picker without adding anything when no target is selected', async () => {
    const second = { ...savedFavorite, project_id: 'two', saved_at: 2 }
    fixture.load.mockResolvedValue([savedFavorite, second])
    fixture.metadata.mockImplementation(async (favorite: ContentFavorite) =>
        metadataFor(favorite, favorite.project_id),
    )

    const wrapper = render()
    await vi.waitFor(() => expect(wrapper.text()).toContain('Select all (2)'))
    await wrapper
        .findAll('button')
        .find((button) => button.text().includes('Select all (2)'))!
        .trigger('click')
    await vi.waitFor(() => expect(fixture.showInstanceSelector).toHaveBeenCalled())
    expect(fixture.addToSelection).not.toHaveBeenCalled()
    expect(fixture.installSelected).not.toHaveBeenCalled()
})

it('shows progress and rejects a second select-all while one is running', async () => {
    let resolveAdd!: (value: boolean) => void
    fixture.addToSelection.mockReturnValueOnce(
        new Promise<boolean>((resolve) => {
            resolveAdd = resolve
        }),
    )
    fixture.load.mockResolvedValue([savedFavorite])
    fixture.metadata.mockResolvedValue(metadataFor(savedFavorite, 'Saved mod'))
    fixture.setTarget(bulkTarget)
    vi.mocked(get_project).mockResolvedValue({ versions: ['v1'] } as never)
    vi.mocked(get_version_many).mockResolvedValue([{ id: 'v1' }] as never)

    const wrapper = render()
    await vi.waitFor(() => expect(wrapper.text()).toContain('Select all (1)'))
    await wrapper
        .findAll('button')
        .find((button) => button.text().includes('Select all (1)'))!
        .trigger('click')
    await vi.waitFor(() => expect(wrapper.text()).toContain('Selecting 0/1'))
    await wrapper
        .findAll('button')
        .find((button) => button.text().includes('Selecting'))!
        .trigger('click')
    resolveAdd(true)
    await vi.waitFor(() => expect(wrapper.text()).not.toContain('Selecting'))
    expect(fixture.addToSelection).toHaveBeenCalledTimes(1)
    expect(fixture.installSelected).not.toHaveBeenCalled()
})

it('shows and searches the Chinese name of a favorite', async () => {
    fixture.localeRef.value = 'zh-CN'
    fixture.translate.mockImplementation(async (hits) =>
        hits.map((hit) => ({ ...hit, title: `钠 (${hit.title})` })),
    )
    fixture.load.mockResolvedValue([savedFavorite])
    fixture.metadata.mockResolvedValue(metadataFor(savedFavorite, 'Saved mod'))
    fixture.query = { q: '钠' }

    const wrapper = render()

    await vi.waitFor(() => expect(wrapper.text()).toContain('钠 (Saved mod)'))
    expect(fixture.translate.mock.calls[0][1]).toBe('zh-CN')
})

it('looks each favorite title up only once while projects are published', async () => {
    const second = { ...savedFavorite, project_id: 'two', saved_at: 2 }
    fixture.load.mockResolvedValue([savedFavorite, second])
    fixture.metadata.mockImplementation(async (favorite: ContentFavorite) =>
        metadataFor(favorite, favorite.project_id),
    )

    const wrapper = render()
    await vi.waitFor(() => expect(wrapper.text()).toContain('two'))

    const looked = fixture.translate.mock.calls.flatMap(([hits]) => hits.map((hit) => hit.title))
    expect(looked.sort()).toEqual(['one', 'two'])
})

it('retries a favorite title whose lookup failed', async () => {
    fixture.localeRef.value = 'zh-CN'
    let publishSecond!: () => void
    const secondPublished = new Promise<void>((resolve) => (publishSecond = resolve))
    const second = { ...savedFavorite, project_id: 'two', saved_at: 2 }
    fixture.load.mockResolvedValue([savedFavorite, second])
    fixture.metadata.mockImplementation(async (favorite: ContentFavorite) => {
        if (favorite.project_id === 'two') await secondPublished
        return metadataFor(favorite, favorite.project_id)
    })
    fixture.translate
        .mockRejectedValueOnce(new Error('dictionary unavailable'))
        .mockImplementation(async (hits) =>
            hits.map((hit) => ({ ...hit, title: `译 (${hit.title})` })),
        )

    const wrapper = render()
    await vi.waitFor(() => expect(fixture.translate).toHaveBeenCalledTimes(1))
    await flushPromises()
    publishSecond()

    await vi.waitFor(() => expect(wrapper.text()).toContain('译 (one)'))
    expect(wrapper.text()).toContain('译 (two)')
})

it('looks favorite titles up again when the language changes', async () => {
    fixture.load.mockResolvedValue([savedFavorite])
    fixture.metadata.mockResolvedValue(metadataFor(savedFavorite, 'Saved mod'))
    fixture.translate.mockImplementation(async (hits, locale) =>
        locale === 'zh-CN' ? hits.map((hit) => ({ ...hit, title: `钠 (${hit.title})` })) : hits,
    )

    const wrapper = render()
    await vi.waitFor(() => expect(wrapper.text()).toContain('Saved mod'))
    expect(wrapper.text()).not.toContain('钠')

    fixture.localeRef.value = 'zh-CN'
    await vi.waitFor(() => expect(wrapper.text()).toContain('钠 (Saved mod)'))

    fixture.localeRef.value = 'en-US'
    await vi.waitFor(() => expect(wrapper.text()).not.toContain('钠'))
})

it('drops a search match when the language changes back to a cached one', async () => {
    fixture.load.mockResolvedValue([savedFavorite])
    fixture.metadata.mockResolvedValue(metadataFor(savedFavorite, 'Saved mod'))
    fixture.translate.mockImplementation(async (hits, locale) =>
        locale === 'zh-CN' ? hits.map((hit) => ({ ...hit, title: `钠 (${hit.title})` })) : hits,
    )
    fixture.query = { q: '钠' }

    const wrapper = render()
    await vi.waitFor(() => expect(fixture.translate).toHaveBeenCalledTimes(1))
    await flushPromises()
    expect(wrapper.text()).not.toContain('Saved mod')

    fixture.localeRef.value = 'zh-CN'
    await vi.waitFor(() => expect(wrapper.text()).toContain('钠 (Saved mod)'))

    fixture.localeRef.value = 'en-US'
    await vi.waitFor(() => expect(wrapper.text()).not.toContain('Saved mod'))
})

it('counts a favorite as installed through the project it was saved from', async () => {
    const saved = {
        ...savedFavorite,
        project_id: 'saved-on-modrinth',
        projects: [{ source: 'curseforge' as const, projectId: '42' }],
    }
    fixture.load.mockResolvedValue([saved])
    fixture.metadata.mockResolvedValue({
        ...savedMetadata,
        project: { source: 'curseforge', projectId: 'presented-project' },
    })
    fixture.setTarget(bulkTarget)
    fixture.installedKeys.add('modrinth:saved-on-modrinth')

    const wrapper = render()
    await vi.waitFor(() => expect(wrapper.text()).toContain('Saved mod'))
    await flushPromises()

    expect(wrapper.text()).not.toContain('Select all')
})

it('refreshes the installed state when content in the target instance changes', async () => {
    fixture.load.mockResolvedValue([])
    fixture.setTarget(bulkTarget)
    render()
    await vi.waitFor(() => expect(fixture.instanceEvent).not.toBeNull())
    await vi.waitFor(() => expect(fixture.refreshInstalledIdentities).toHaveBeenCalledTimes(1))

    await fixture.instanceEvent!({ event: 'content_changed', instance_id: 'other-instance' })
    await fixture.instanceEvent!({ event: 'edited', instance_id: 'instance-1' })
    expect(fixture.refreshInstalledIdentities).toHaveBeenCalledTimes(1)

    await fixture.instanceEvent!({
        event: 'content_install_finished',
        instance_id: 'instance-1',
    })
    expect(fixture.refreshInstalledIdentities).toHaveBeenCalledTimes(2)
})

it('refreshes installed identities after restoring a target instance on mount', async () => {
    fixture.load.mockResolvedValue([])
    fixture.setTarget(bulkTarget)

    render()
    await vi.waitFor(() => expect(fixture.refreshInstalledIdentities).toHaveBeenCalledTimes(1))
})

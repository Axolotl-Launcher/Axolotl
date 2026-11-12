import { mount } from '@vue/test-utils'
import { beforeEach, expect, it, vi } from 'vitest'
import { computed, defineComponent, nextTick, ref } from 'vue'

import { useSearch } from '#ui/utils/search'

import { useBrowseSearch, type UseBrowseSearchOptions } from './use-browse-search'

const navigation = vi.hoisted(() => ({
    route: { path: '/browse/mod', query: {} as Record<string, string | string[]> },
    replace: vi.fn(),
}))
vi.mock('vue-router', () => ({
    useRoute: () => navigation.route,
    useRouter: () => ({ replace: navigation.replace }),
}))
vi.mock('#ui/composables/i18n', async () => {
    const { ref } = await import('vue')
    return {
        LOCALES: [],
        defineMessage: (value: unknown) => value,
        defineMessages: (value: unknown) => value,
        useVIntl: () => ({
            locale: ref('en-US'),
            formatMessage: (value: { defaultMessage: string }) => value.defaultMessage,
        }),
    }
})
vi.mock('#ui/composables/debug-logger', () => ({ useDebugLogger: () => () => {} }))

beforeEach(() => {
    navigation.route.query = {}
    navigation.replace.mockReset()
    localStorage.removeItem('browse-max-results')
})

function browse(overrides: Partial<UseBrowseSearchOptions> = {}) {
    const search = vi.fn().mockResolvedValue({ projectHits: [], serverHits: [], total_hits: 100 })
    let state!: ReturnType<typeof useBrowseSearch>
    const projectType = ref('mod')
    const wrapper = mount(
        defineComponent({
            setup() {
                state = useBrowseSearch({
                    enabled: ref(false),
                    deferInitialization: true,
                    automaticSort: true,
                    tags: ref({ categories: [], gameVersions: [], loaders: [] }),
                    projectType,
                    search,
                    persistentQueryParams: [],
                    ...overrides,
                })
                return () => null
            },
        }),
    )
    const request = async () => {
        // Initial prerequisites have resolved, as in the desktop page.
        const enabled = overrides.enabled
        if (enabled) enabled.value = true
        await state.refreshSearch()
        return new URLSearchParams(search.mock.lastCall?.[0])
    }
    return { state, search, projectType, wrapper, request }
}

it('uses downloads for empty and whitespace input, relevance for keywords, and no explicit URL sort', async () => {
    const enabled = ref(false)
    const view = browse({ enabled })
    try {
        view.state.initializeFilters()
        expect((await view.request()).get('index')).toBe('downloads')
        expect(navigation.replace.mock.lastCall?.[0].query.s).toBeUndefined()
        view.state.query.value = '  sodium  '
        await nextTick()
        const keywordRequest = await view.request()
        expect(keywordRequest.get('index')).toBe('relevance')
        expect(keywordRequest.get('query')).toBe('sodium')
        view.state.query.value = ' \t '
        await nextTick()
        const emptyRequest = await view.request()
        expect(emptyRequest.get('index')).toBe('downloads')
        expect(emptyRequest.has('query')).toBe(false)
        expect(navigation.replace.mock.lastCall?.[0].query).not.toHaveProperty('q')
        view.state.query.value = 'fabric'
        await nextTick()
        view.state.clearSearch()
        expect(view.state.effectiveCurrentSortType.value.name).toBe('downloads')
    } finally {
        view.wrapper.unmount()
    }
})

it.each(['relevance', 'downloads', 'updated'])(
    'retains explicit %s through editing, clearing, and URL restoration',
    async (name) => {
        const view = browse({ enabled: ref(false) })
        let saved!: Record<string, string | string[]>
        try {
            view.state.initializeFilters()
            view.state.effectiveCurrentSortType.value = view.state.effectiveSortTypes.value.find(
                (sort) => sort.name === name,
            )!
            view.state.query.value = 'sodium'
            await nextTick()
            expect((await view.request()).get('index')).toBe(name)
            view.state.clearSearch()
            await nextTick()
            expect((await view.request()).get('index')).toBe(name)
            view.state.currentPage.value = 3
            await view.request()
            saved = navigation.replace.mock.lastCall?.[0].query
            expect(saved.s).toEqual([name])
            expect(saved.page).toEqual(['3'])
        } finally {
            view.wrapper.unmount()
        }
        navigation.route.query = saved
        const restored = browse({ enabled: ref(false) })
        try {
            restored.state.initializeFilters()
            await nextTick()
            expect(restored.state.currentPage.value).toBe(3)
            expect((await restored.request()).get('index')).toBe(name)
            restored.state.query.value = 'iris'
            await nextTick()
            expect((await restored.request()).get('index')).toBe(name)
            expect(restored.state.currentPage.value).toBe(1)
        } finally {
            restored.wrapper.unmount()
        }
    },
)

it('restores automatic keyword sorting and pagination without turning it into a manual choice', async () => {
    navigation.route.query = { q: 'sodium', page: '3' }
    const view = browse({ enabled: ref(false) })
    try {
        view.state.initializeFilters()
        await nextTick()
        const request = await view.request()
        expect(request.get('index')).toBe('relevance')
        expect(request.get('offset')).toBe('40')
        expect(navigation.replace.mock.lastCall?.[0].query.s).toBeUndefined()
        view.state.clearSearch()
        await nextTick()
        expect((await view.request()).get('index')).toBe('downloads')
    } finally {
        view.wrapper.unmount()
    }
})

it('falls back for limited sources without losing a manual choice on returning to a capable source', async () => {
    const limited = ref(false)
    const view = browse({
        enabled: ref(false),
        supportedSortNames: computed(() => (limited.value ? ['relevance'] : undefined)),
    })
    try {
        expect((await view.request()).get('index')).toBe('downloads')
        limited.value = true
        await nextTick()
        expect(view.state.effectiveSortTypes.value.map((sort) => sort.name)).toEqual(['relevance'])
        expect((await view.request()).get('index')).toBe('relevance')
        limited.value = false
        await nextTick()
        view.state.effectiveCurrentSortType.value = view.state.effectiveSortTypes.value.find(
            (sort) => sort.name === 'updated',
        )!
        limited.value = true
        await nextTick()
        expect((await view.request()).get('index')).toBe('relevance')
        limited.value = false
        await nextTick()
        expect((await view.request()).get('index')).toBe('updated')
    } finally {
        view.wrapper.unmount()
    }
})

it('resets content types to automatic browsing and leaves server sorting independent', async () => {
    const view = browse({ enabled: ref(false) })
    try {
        view.state.effectiveCurrentSortType.value = view.state.effectiveSortTypes.value.find(
            (sort) => sort.name === 'updated',
        )!
        view.state.query.value = 'sodium'
        view.projectType.value = 'resourcepack'
        await nextTick()
        expect((await view.request()).get('index')).toBe('downloads')
        expect(view.state.query.value).toBe('')
        view.projectType.value = 'server'
        await nextTick()
        expect((await view.request()).get('index')).toBe('relevance')
        view.state.effectiveCurrentSortType.value = view.state.effectiveSortTypes.value.find(
            (sort) => sort.name === 'follows',
        )!
        view.state.query.value = 'survival'
        await nextTick()
        expect((await view.request()).get('index')).toBe('follows')
        expect(navigation.replace.mock.lastCall?.[0].query.ss).toEqual(['follows'])
    } finally {
        view.wrapper.unmount()
    }
})

it('preserves the existing generic search defaults unless automatic sorting is enabled', () => {
    let state!: ReturnType<typeof useSearch>
    const wrapper = mount(
        defineComponent({
            setup() {
                state = useSearch(ref(['mod']), ref({}), ref([]))
                return () => null
            },
        }),
    )
    try {
        expect(new URLSearchParams(state.requestParams.value).get('index')).toBe('relevance')
        expect(state.createPageParams()).not.toHaveProperty('s')
    } finally {
        wrapper.unmount()
    }
})

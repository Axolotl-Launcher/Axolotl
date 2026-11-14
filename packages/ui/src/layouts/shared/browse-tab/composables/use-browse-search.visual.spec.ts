import { flushPromises, mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { defineComponent, nextTick, ref } from 'vue'

import type { BrowseSearchResponse } from '../types'
import type { UseBrowseSearchOptions } from './use-browse-search'
import { useBrowseSearch } from './use-browse-search'

vi.mock('vue-router', () => ({
    useRoute: () => ({ query: {}, path: '/browse/mod' }),
    useRouter: () => ({ replace: vi.fn() }),
}))
vi.mock('#ui/composables/i18n', () => ({
    useVIntl: () => ({ formatMessage: () => 'Failed' }),
    defineMessages: (messages: unknown) => messages,
}))
vi.mock('#ui/composables/debug-logger', () => ({ useDebugLogger: () => () => {} }))
vi.mock('#ui/utils/search', async () => {
    const { computed, ref } = await import('vue')
    return {
        LOADER_FILTER_TYPES: [],
        useSearch: () => {
            const query = ref('')
            const currentPage = ref(1)
            return {
                query,
                currentPage,
                currentSortType: ref({ name: 'relevance', display: 'Relevance' }),
                currentFilters: ref([]),
                toggledGroups: ref([]),
                maxResults: ref(20),
                overriddenProvidedFilterTypes: ref([]),
                filters: computed(() => []),
                availableSortTypes: computed(() => []),
                resetSort: vi.fn(),
                requestParams: computed(() => `${query.value}:${currentPage.value}`),
                initializeQueryParams: vi.fn(),
                createPageParams: () => ({}),
            }
        },
    }
})
vi.mock('#ui/utils/server-search', async () => {
    const { computed, ref } = await import('vue')
    return {
        useServerSearch: () => ({
            serverCurrentSortType: ref({ name: 'relevance', display: 'Relevance' }),
            serverCurrentFilters: ref([]),
            serverToggledGroups: ref([]),
            serverSortTypes: [],
            serverFilterTypes: computed(() => []),
            serverRequestParams: computed(() => ''),
            initializeQueryParams: vi.fn(),
            createServerPageParams: () => ({}),
        }),
    }
})

function deferred<T>() {
    let resolve!: (value: T) => void
    let reject!: (reason: unknown) => void
    const promise = new Promise<T>((yes, no) => {
        resolve = yes
        reject = no
    })
    return { promise, resolve, reject }
}
function response(total = 45): BrowseSearchResponse {
    return { projectHits: [], serverHits: [], total_hits: total, per_page: 20 }
}
function cursorResponse(
    page: number,
    overrides: Partial<BrowseSearchResponse> = {},
): BrowseSearchResponse {
    return {
        projectHits: [],
        serverHits: [],
        total_hits: 400,
        total_precision: 'unknown',
        has_more: true,
        per_page: 20,
        page,
        ...overrides,
    }
}

function mountSearch(search: UseBrowseSearchOptions['search']) {
    let state!: ReturnType<typeof useBrowseSearch>
    const wrapper = mount(
        defineComponent({
            setup() {
                state = useBrowseSearch({
                    projectType: ref('mod'),
                    tags: ref({}),
                    search,
                    persistentQueryParams: [],
                    syncQueryParams: false,
                })
                return () => null
            },
        }),
    )
    return { state, wrapper }
}

it('retains the committed page through debounce, stale responses, failure and empty success', async () => {
    vi.useFakeTimers()
    const pending = [
        deferred<BrowseSearchResponse>(),
        deferred<BrowseSearchResponse>(),
        deferred<BrowseSearchResponse>(),
    ]
    let state!: ReturnType<typeof useBrowseSearch>
    const search = vi
        .fn()
        .mockResolvedValueOnce(response())
        .mockImplementationOnce(() => pending[0].promise)
        .mockImplementationOnce(() => pending[1].promise)
        .mockImplementationOnce(() => pending[2].promise)
    const wrapper = mount(
        defineComponent({
            setup() {
                state = useBrowseSearch({
                    projectType: ref('mod'),
                    tags: ref({}),
                    search,
                    persistentQueryParams: [],
                    syncQueryParams: false,
                })
                return () => null
            },
        }),
    )
    try {
        await state.refreshSearch()
        expect(state.totalHits.value).toBe(45)
        state.query.value = 'A'
        await nextTick()
        expect(state.loading.value).toBe(true)
        await vi.advanceTimersByTimeAsync(200)
        state.query.value = 'B'
        await nextTick()
        pending[0].resolve(response(100))
        await flushPromises()
        expect(state.totalHits.value).toBe(45)
        await vi.advanceTimersByTimeAsync(200)
        pending[1].reject(new Error('offline'))
        await flushPromises()
        expect(state.totalHits.value).toBe(45)
        expect(state.pageCount.value).toBe(3)
        expect(state.cursorPaged.value).toBe(false)
        expect(state.searchError.value).toBe('Failed')
        const retry = state.refreshSearch()
        pending[2].resolve(response(0))
        await retry
        expect(state.totalHits.value).toBe(0)
        expect(state.pageCount.value).toBe(1)
        expect(state.searchError.value).toBeNull()
    } finally {
        wrapper.unmount()
        vi.useRealTimers()
    }
})

it('does not search until prerequisites and restored filters are ready', async () => {
    vi.useFakeTimers()
    const enabled = ref(false)
    const search = vi.fn().mockResolvedValue(response())
    let state!: ReturnType<typeof useBrowseSearch>
    const wrapper = mount(
        defineComponent({
            setup() {
                state = useBrowseSearch({
                    enabled,
                    deferInitialization: true,
                    projectType: ref('mod'),
                    tags: ref({}),
                    search,
                    persistentQueryParams: [],
                    syncQueryParams: false,
                })
                return () => null
            },
        }),
    )
    try {
        state.query.value = 'restored'
        await state.refreshSearch()
        await vi.advanceTimersByTimeAsync(500)
        expect(search).not.toHaveBeenCalled()
        enabled.value = true
        await state.refreshSearch()
        await vi.advanceTimersByTimeAsync(500)
        expect(search).toHaveBeenCalledTimes(1)
        expect(search.mock.calls[0][0]).toBe('restored:1')
        state.query.value = 'new'
        enabled.value = false
        await vi.advanceTimersByTimeAsync(500)
        expect(search).toHaveBeenCalledTimes(1)
    } finally {
        wrapper.unmount()
        vi.useRealTimers()
    }
})

it('counts the pages a cursor source reports a total for instead of one page ahead', async () => {
    const search = vi
        .fn()
        .mockResolvedValueOnce(cursorResponse(1, { total_precision: 'exact' }))
        .mockResolvedValueOnce(cursorResponse(2, { total_hits: 20 }))
    const { state, wrapper } = mountSearch(search)
    try {
        await state.refreshSearch()
        expect(state.pageCount.value).toBe(20)
        expect(state.pageCountKnown.value).toBe(true)
        expect(state.cursorPaged.value).toBe(true)

        state.currentPage.value = 2
        await state.refreshSearch()
        // Only the first page of a query reports a total, so the bar keeps the
        // range it was told about instead of shrinking to "one page ahead".
        expect(state.pageCount.value).toBe(20)
        expect(state.pageCountKnown.value).toBe(true)
    } finally {
        wrapper.unmount()
    }
})

it('follows the page the source resolved instead of the page it was asked for', async () => {
    const search = vi.fn().mockResolvedValueOnce(cursorResponse(4, { total_precision: 'exact' }))
    const { state, wrapper } = mountSearch(search)
    try {
        state.currentPage.value = 5
        await state.refreshSearch()
        // A cursor source that runs out answers with the last page it has.
        expect(state.currentPage.value).toBe(4)
    } finally {
        wrapper.unmount()
    }
})

it('treats an exhausted cursor as the end of the results', async () => {
    const search = vi
        .fn()
        .mockResolvedValueOnce(cursorResponse(12, { has_more: false, total_hits: 20 }))
    const { state, wrapper } = mountSearch(search)
    try {
        state.currentPage.value = 20
        await state.refreshSearch()
        expect(state.currentPage.value).toBe(12)
        expect(state.pageCount.value).toBe(12)
        expect(state.pageCountKnown.value).toBe(true)
    } finally {
        wrapper.unmount()
    }
})

it('does not let an estimated total end the result set', async () => {
    const search = vi
        .fn()
        .mockResolvedValueOnce(
            cursorResponse(1, { total_precision: 'estimate', total_hits: 10000 }),
        )
    const { state, wrapper } = mountSearch(search)
    try {
        await state.refreshSearch()
        // CurseForge reports 10000 for anything larger, so the number can size the
        // strip but cannot be treated as the end.
        expect(state.pageCount.value).toBe(500)
        expect(state.pageCountKnown.value).toBe(false)
    } finally {
        wrapper.unmount()
    }
})

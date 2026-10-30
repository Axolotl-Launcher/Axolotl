import { flushPromises, mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { defineComponent, nextTick, ref } from 'vue'

import type { BrowseSearchResponse } from '../types'
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
                sortTypes: [],
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

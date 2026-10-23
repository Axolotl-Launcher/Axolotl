import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import type {
    FilterType,
    FilterValue,
    ProjectType,
    SortType,
    Tags,
} from '@modrinth/ui/src/utils/search.ts'
import { LOADER_FILTER_TYPES } from '@modrinth/ui/src/utils/search.ts'
import { useServerSearch } from '@modrinth/ui/src/utils/server-search.ts'
import {
    computed,
    type ComputedRef,
    onScopeDispose,
    type Ref,
    ref,
    type ShallowRef,
    shallowRef,
    watch,
} from 'vue'
import { useRoute, useRouter } from 'vue-router'

import type { PreviewClock } from '../../lib/clock'
import { useSearch } from './search'
import type { SearchHit } from './search-engine'

/**
 * Port of `useBrowseSearch` (packages/ui/src/layouts/shared/browse-tab/composables/
 * use-browse-search.ts): debounced searching on parameter changes, stale-result discarding,
 * loading/error state, cursor style page counts and query-string sync. The debounce runs on
 * the preview clock instead of a raw timer.
 */
export interface BrowseSearchResponse {
    projectHits: SearchHit[]
    serverHits: unknown[]
    total_hits: number
    has_more?: boolean
    per_page: number
    page?: number
}

export interface BrowseSearchRequest {
    params: string
    query: string
    filters: FilterValue[]
    sort: string
    limit: number
    page: number
}

export interface UseBrowseSearchOptions {
    clock: PreviewClock
    projectType: Ref<string>
    tags: Ref<Tags>
    providedFilters: ComputedRef<FilterValue[]>
    installContextLoader?: ComputedRef<string | undefined>
    search: (request: BrowseSearchRequest, signal: AbortSignal) => Promise<BrowseSearchResponse>
    formatError?: (error: unknown) => string
    persistentQueryParams: string[]
    getExtraQueryParams?: () => Record<string, string | undefined>
    displayMode: Ref<'list' | 'compact' | 'grid'>
    sortOptions?: readonly SortType[]
}

export function useBrowseSearch(options: UseBrowseSearchOptions) {
    const { formatMessage } = useVIntl()
    const text = defineMessages({
        failed: {
            id: 'browse.search.failed',
            defaultMessage:
                'The search could not be completed. Check your connection or filters and try again.',
        },
    })
    const route = useRoute()
    const router = useRouter()

    const projectTypes = computed(() => [options.projectType.value] as ProjectType[])
    const isServerType = computed(() => options.projectType.value === 'server')

    const {
        query,
        currentSortType,
        currentFilters,
        toggledGroups,
        maxResults,
        currentPage,
        overriddenProvidedFilterTypes,
        filters,
        effectiveFilters,
        sortTypes,
        requestParams,
        createPageParams,
    } = useSearch(projectTypes, options.tags, options.providedFilters, {
        sortOptions: options.sortOptions,
        persistentQueryParams: options.persistentQueryParams,
    })

    const {
        serverCurrentSortType,
        serverCurrentFilters,
        serverToggledGroups,
        serverSortTypes,
        serverFilterTypes,
        serverRequestParams,
        createServerPageParams,
    } = useServerSearch({
        tags: options.tags,
        query,
        maxResults,
        currentPage,
        providedFilters: options.providedFilters,
    })

    const effectiveRequestParams = computed(() =>
        isServerType.value ? serverRequestParams.value : requestParams.value,
    )
    const effectiveSortTypes = computed(
        () => (isServerType.value ? serverSortTypes : sortTypes) as readonly SortType[],
    )
    const effectiveCurrentSortType = computed<SortType>({
        get: () => (isServerType.value ? serverCurrentSortType.value : currentSortType.value),
        set: (value: SortType) => {
            if (isServerType.value) serverCurrentSortType.value = value
            else currentSortType.value = value
        },
    })

    const effectiveLayout = computed<'list' | 'compact' | 'grid'>(() => options.displayMode.value)

    const selectedFilterTags = computed(() =>
        currentFilters.value
            .filter(
                (filter) =>
                    filter.type.startsWith('category_') ||
                    (LOADER_FILTER_TYPES as readonly string[]).includes(filter.type),
            )
            .map((filter) => filter.option),
    )
    const excludeLoaders = computed(
        () =>
            currentFilters.value.some((filter) =>
                (LOADER_FILTER_TYPES as readonly string[]).includes(filter.type),
            ) ||
            !!options.installContextLoader?.value ||
            ['resourcepack', 'datapack'].includes(options.projectType.value),
    )
    const loadersNotForThisType = computed(
        () =>
            options.tags.value?.loaders
                ?.filter(
                    (loader) => !loader.supported_project_types.includes(options.projectType.value),
                )
                ?.map((loader) => loader.name) ?? [],
    )
    const deprioritizedTags = computed(() => [
        ...selectedFilterTags.value,
        ...loadersNotForThisType.value,
    ])

    const loading = ref(true)
    const projectHits = shallowRef<SearchHit[]>([])
    const serverHits = shallowRef<unknown[]>([])
    const totalHits = ref(0)
    const searchError = ref<string | null>(null)
    const cursorHasMore = ref<boolean | undefined>(undefined)

    const pageCount = computed(() => {
        if (cursorHasMore.value !== undefined)
            return currentPage.value + (cursorHasMore.value ? 1 : 0)
        if (totalHits.value === 0) return 1
        return Math.ceil(totalHits.value / maxResults.value)
    })

    let searchVersion = 0
    let cancelDebounce: (() => void) | null = null
    let searchAbortController: AbortController | null = null

    const providedFiltersOrEmpty = computed(() => options.providedFilters.value ?? [])

    watch(
        [
            query,
            maxResults,
            options.projectType,
            currentSortType,
            serverCurrentSortType,
            currentFilters,
            serverCurrentFilters,
            overriddenProvidedFilterTypes,
            providedFiltersOrEmpty,
        ],
        () => {
            currentPage.value = 1
        },
        { deep: true },
    )

    watch(effectiveRequestParams, () => {
        searchVersion++
        searchAbortController?.abort()
        searchAbortController = null
        cancelDebounce?.()
        cancelDebounce = options.clock.after(200, () => {
            cancelDebounce = null
            void refreshSearch()
        })
    })

    async function refreshSearch() {
        searchAbortController?.abort()
        const abortController = new AbortController()
        searchAbortController = abortController
        const version = ++searchVersion
        loading.value = true
        searchError.value = null
        try {
            const response = await options.search(
                {
                    params: effectiveRequestParams.value,
                    query: query.value,
                    filters: isServerType.value
                        ? serverCurrentFilters.value
                        : effectiveFilters.value,
                    sort: effectiveCurrentSortType.value.name,
                    limit: maxResults.value,
                    page: currentPage.value,
                },
                abortController.signal,
            )
            if (version !== searchVersion) return
            if (isServerType.value) serverHits.value = response.serverHits
            else projectHits.value = response.projectHits
            totalHits.value = response.total_hits
            cursorHasMore.value = response.has_more
            if (response.page !== undefined) currentPage.value = response.page
            if (currentPage.value > pageCount.value) currentPage.value = pageCount.value
            updateUrlParams()
            loading.value = false
        } catch (error) {
            if (abortController.signal.aborted) return
            if (version === searchVersion) {
                searchError.value = options.formatError?.(error) ?? formatMessage(text.failed)
                loading.value = false
            }
        } finally {
            if (searchAbortController === abortController) searchAbortController = null
        }
    }

    onScopeDispose(() => {
        searchVersion++
        cancelDebounce?.()
        searchAbortController?.abort()
    })

    function updateUrlParams() {
        const persistentParams: Record<string, string | (string | null)[] | null | undefined> = {}
        for (const [key, value] of Object.entries(route.query)) {
            if (options.persistentQueryParams.includes(key)) persistentParams[key] = value
        }
        for (const [key, value] of Object.entries(options.getExtraQueryParams?.() ?? {})) {
            persistentParams[key] = value
        }
        const params = {
            ...persistentParams,
            ...(isServerType.value ? createServerPageParams() : createPageParams()),
        }
        if (!route.path.startsWith('/browse/')) return
        void router.replace({ path: route.path, query: params })
    }

    /** The app scrolls the document, which never scrolls inside the launcher window. */
    async function setPage(page: number) {
        currentPage.value = page
    }

    function clearSearch() {
        query.value = ''
        currentPage.value = 1
    }

    function onFilterChange() {}

    watch(
        () => options.projectType.value,
        () => {
            effectiveCurrentSortType.value =
                effectiveSortTypes.value.find((sortType) => sortType.name === 'relevance') ??
                effectiveSortTypes.value[0]
            query.value = ''
        },
    )

    return {
        query,
        filters: filters as ComputedRef<FilterType[]>,
        currentFilters,
        toggledGroups,
        overriddenProvidedFilterTypes,
        serverFilterTypes,
        serverCurrentFilters,
        serverToggledGroups,
        effectiveSortTypes,
        effectiveCurrentSortType,
        loading,
        projectHits: projectHits as ShallowRef<SearchHit[]>,
        serverHits,
        totalHits,
        searchError,
        pageCount,
        maxResults,
        currentPage,
        isServerType,
        effectiveLayout,
        deprioritizedTags,
        excludeLoaders,
        refreshSearch,
        setPage,
        clearSearch,
        onFilterChange,
    }
}

export type BrowseSearchState = ReturnType<typeof useBrowseSearch>

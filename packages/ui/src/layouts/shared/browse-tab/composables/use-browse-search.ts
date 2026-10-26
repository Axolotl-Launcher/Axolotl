import type { ComputedRef, Ref, ShallowRef } from 'vue'
import { computed, nextTick, onScopeDispose, ref, shallowRef, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { useDebugLogger } from '#ui/composables/debug-logger'
import { defineMessages, useVIntl } from '#ui/composables/i18n'
import type {
    EnvironmentSearchOverride,
    FilterType,
    FilterValue,
    ProjectType,
    SortType,
    Tags,
} from '#ui/utils/search'
import { LOADER_FILTER_TYPES, useSearch } from '#ui/utils/search'
import { useServerSearch } from '#ui/utils/server-search'

import type { BrowseDisplayMode, BrowseSearchResponse } from '../types'

export interface UseBrowseSearchOptions {
    enabled?: Ref<boolean>
    deferInitialization?: boolean
    formatError?: (error: unknown) => string
    projectType: Ref<string>
    tags: Ref<Tags>
    providedFilters?: ComputedRef<FilterValue[]>
    environmentOverride?: ComputedRef<EnvironmentSearchOverride | undefined>
    installContextLoader?: ComputedRef<string | undefined>
    search: (params: string, signal: AbortSignal) => Promise<BrowseSearchResponse>
    invalidateSearch?: () => void
    syncQueryParams?: boolean
    persistentQueryParams: string[]
    getExtraQueryParams?: () => Record<string, string | undefined>
    maxResultsOptions?: ComputedRef<number[]>
    displayMode?: Ref<BrowseDisplayMode> | ComputedRef<BrowseDisplayMode>
    initialSearchResponse?: BrowseSearchResponse
    sortOptions?: readonly SortType[]
}

export interface BrowseCommittedSearch {
    response: BrowseSearchResponse
    projectType: string
    sortType: SortType
    page: number
    maxResults: number
    deprioritizedTags: string[]
    excludeLoaders: boolean
}

export interface BrowseSearchState {
    query: Ref<string>

    filters: ComputedRef<FilterType[]>
    currentFilters: Ref<FilterValue[]>
    toggledGroups: Ref<string[]>
    overriddenProvidedFilterTypes: Ref<string[]>

    serverFilterTypes: ComputedRef<FilterType[]>
    serverCurrentFilters: Ref<FilterValue[]>
    serverToggledGroups: Ref<string[]>

    effectiveSortTypes: ComputedRef<readonly SortType[]>
    effectiveCurrentSortType: Ref<SortType>

    loading: Ref<boolean>
    committed: ShallowRef<BrowseCommittedSearch | null>
    searchError: Ref<string | null>
    projectHits: ShallowRef<BrowseSearchResponse['projectHits']>
    serverHits: ShallowRef<BrowseSearchResponse['serverHits']>
    totalHits: Ref<number>
    pageCount: ComputedRef<number>

    maxResults: Ref<number>
    currentPage: Ref<number>

    isServerType: ComputedRef<boolean>
    effectiveLayout: ComputedRef<'list' | 'compact' | 'grid'>
    deprioritizedTags: ComputedRef<string[]>
    excludeLoaders: ComputedRef<boolean>

    initializeFilters: () => void
    refreshSearch: () => Promise<void>
    setPage: (page: number) => Promise<void>
    clearSearch: () => void
    onFilterChange: () => void
}

export function useBrowseSearch(options: UseBrowseSearchOptions): BrowseSearchState {
    const { formatMessage } = useVIntl()
    const messages = defineMessages({
        failed: {
            id: 'browse.search.failed',
            defaultMessage:
                'The search could not be completed. Check your connection or filters and try again.',
        },
    })
    const debug = useDebugLogger('BrowseSearch')
    const route = useRoute()
    const router = useRouter()

    debug('init, projectType:', options.projectType.value)

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
        sortTypes,
        requestParams,
        createPageParams,
        initializeQueryParams,
    } = useSearch(
        projectTypes,
        options.tags,
        options.providedFilters ?? computed(() => []),
        options.environmentOverride ?? computed(() => undefined),
        options.sortOptions,
        options.deferInitialization,
    )

    const {
        serverCurrentSortType,
        serverCurrentFilters,
        serverToggledGroups,
        serverSortTypes,
        serverFilterTypes,
        serverRequestParams,
        createServerPageParams,
        initializeQueryParams: initializeServerQueryParams,
    } = useServerSearch({
        tags: options.tags,
        deferInitialization: options.deferInitialization,
        query,
        maxResults,
        currentPage,
        providedFilters: options.providedFilters,
    })

    const effectiveRequestParams = computed(() =>
        isServerType.value ? serverRequestParams.value : requestParams.value,
    )
    const effectiveSortTypes = computed(() =>
        isServerType.value ? (serverSortTypes as readonly SortType[]) : sortTypes,
    )
    const effectiveCurrentSortType = computed({
        get: () => (isServerType.value ? serverCurrentSortType.value : currentSortType.value),
        set: (v: SortType) => {
            if (isServerType.value) serverCurrentSortType.value = v
            else currentSortType.value = v
        },
    })

    const effectiveMaxResultsOptions = computed(
        () => options.maxResultsOptions?.value ?? [5, 10, 15, 20, 50, 100],
    )

    watch(effectiveMaxResultsOptions, (opts) => {
        if (!opts.includes(maxResults.value)) {
            maxResults.value = opts.reduce((prev, curr) =>
                Math.abs(curr - maxResults.value) <= Math.abs(prev - maxResults.value)
                    ? curr
                    : prev,
            )
        }
    })

    const effectiveDisplayMode = computed(() => options.displayMode?.value ?? 'list')
    const effectiveLayout = computed<'list' | 'compact' | 'grid'>(() =>
        effectiveDisplayMode.value === 'grid' || effectiveDisplayMode.value === 'gallery'
            ? 'grid'
            : effectiveDisplayMode.value === 'compact'
              ? 'compact'
              : 'list',
    )

    const selectedFilterTags = computed(() =>
        currentFilters.value
            .filter(
                (f) =>
                    f.type.startsWith('category_') ||
                    LOADER_FILTER_TYPES.includes(f.type as (typeof LOADER_FILTER_TYPES)[number]),
            )
            .map((f) => f.option),
    )
    const excludeLoaders = computed(
        () =>
            currentFilters.value.some((f) =>
                LOADER_FILTER_TYPES.includes(f.type as (typeof LOADER_FILTER_TYPES)[number]),
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

    const initialSearchResponse = options.initialSearchResponse
    const loading = ref(!initialSearchResponse)
    const committed = shallowRef<BrowseCommittedSearch | null>(
        initialSearchResponse
            ? {
                  response: initialSearchResponse,
                  projectType: options.projectType.value,
                  sortType: effectiveCurrentSortType.value,
                  page: currentPage.value,
                  maxResults: maxResults.value,
                  deprioritizedTags: deprioritizedTags.value,
                  excludeLoaders: excludeLoaders.value,
              }
            : null,
    )
    const projectHits = computed({
        get: () => committed.value?.response.projectHits ?? [],
        set: (hits: BrowseSearchResponse['projectHits']) => {
            if (committed.value)
                committed.value = {
                    ...committed.value,
                    response: { ...committed.value.response, projectHits: hits },
                }
        },
    })
    const serverHits = computed({
        get: () => committed.value?.response.serverHits ?? [],
        set: (hits: BrowseSearchResponse['serverHits']) => {
            if (committed.value)
                committed.value = {
                    ...committed.value,
                    response: { ...committed.value.response, serverHits: hits },
                }
        },
    })
    const totalHits = computed(() => committed.value?.response.total_hits ?? 0)
    const searchError = ref<string | null>(null)
    const pageCount = computed(() => {
        const snapshot = committed.value
        if (!snapshot) return 1
        if (snapshot.response.has_more !== undefined)
            return snapshot.page + (snapshot.response.has_more ? 1 : 0)
        return Math.max(1, Math.ceil(snapshot.response.total_hits / snapshot.maxResults))
    })

    let searchVersion = 0
    let searchDebounceTimer: ReturnType<typeof setTimeout> | null = null
    let searchAbortController: AbortController | null = null

    const providedFiltersOrEmpty = computed(() => options.providedFilters?.value ?? [])

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

    function invalidatePendingSearch() {
        searchVersion++
        searchAbortController?.abort()
        searchAbortController = null
        if (searchDebounceTimer) clearTimeout(searchDebounceTimer)
        searchDebounceTimer = null
    }

    watch(
        effectiveRequestParams,
        () => {
            invalidatePendingSearch()
            if (options.enabled?.value === false) return
            loading.value = true
            searchDebounceTimer = setTimeout(() => {
                searchDebounceTimer = null
                void refreshSearch(true)
            }, 200)
        },
        { flush: 'sync' },
    )

    if (options.enabled)
        watch(
            options.enabled,
            (enabled) => {
                if (!enabled) invalidatePendingSearch()
            },
            { flush: 'sync' },
        )

    function initializeFilters() {
        initializeQueryParams()
        initializeServerQueryParams()
    }

    async function refreshSearch(reuseCache = false) {
        if (options.enabled?.value === false) return
        invalidatePendingSearch()
        if (!reuseCache) options.invalidateSearch?.()
        const abortController = new AbortController()
        searchAbortController = abortController
        const version = ++searchVersion
        const context = {
            projectType: options.projectType.value,
            sortType: effectiveCurrentSortType.value,
            page: currentPage.value,
            maxResults: maxResults.value,
            deprioritizedTags: deprioritizedTags.value,
            excludeLoaders: excludeLoaders.value,
        }
        debug('refreshSearch start', {
            version,
            projectType: options.projectType.value,
            params: effectiveRequestParams.value.substring(0, 100),
        })

        // Keep the previous committed response visible while its replacement loads.
        loading.value = true
        searchError.value = null

        try {
            const response = await options.search(
                effectiveRequestParams.value,
                abortController.signal,
            )

            if (version !== searchVersion) {
                debug('refreshSearch stale, discarding', { version, current: searchVersion })
                return
            }

            const count =
                response.has_more !== undefined
                    ? context.page + (response.has_more ? 1 : 0)
                    : Math.max(1, Math.ceil(response.total_hits / context.maxResults))
            const page = Math.min(response.page ?? context.page, count)
            committed.value = { ...context, response, page }
            currentPage.value = page
            debug('refreshSearch complete', {
                version,
                hits: response.total_hits,
                projectHits: response.projectHits.length,
                serverHits: response.serverHits.length,
            })

            if (options.syncQueryParams !== false) updateUrlParams()
            loading.value = false
        } catch (err) {
            if (abortController.signal.aborted) {
                debug('refreshSearch cancelled', { version })
                return
            }
            debug('refreshSearch error', err)
            console.error('Browse search error:', err)
            if (version === searchVersion) {
                searchError.value = options.formatError?.(err) ?? formatMessage(messages.failed)
                loading.value = false
            }
        } finally {
            if (searchAbortController === abortController) {
                searchAbortController = null
            }
        }
    }

    onScopeDispose(() => {
        searchVersion++
        if (searchDebounceTimer) clearTimeout(searchDebounceTimer)
        searchAbortController?.abort()
    })

    function updateUrlParams() {
        debug('updateUrlParams', { path: route.path })
        const persistentParams: Record<string, string | (string | null)[] | null | undefined> = {}

        for (const [key, value] of Object.entries(route.query)) {
            if (options.persistentQueryParams.includes(key)) {
                persistentParams[key] = value
            }
        }

        const extraParams = options.getExtraQueryParams?.() ?? {}
        for (const [key, value] of Object.entries(extraParams)) {
            persistentParams[key] = value
        }

        const params = {
            ...persistentParams,
            ...(isServerType.value ? createServerPageParams() : createPageParams()),
        }

        router.replace({ path: route.path, query: params })
    }

    async function setPage(newPageNumber: number) {
        currentPage.value = newPageNumber
        await nextTick()
        window.scrollTo({ top: 0, behavior: 'smooth' })
    }

    function clearSearch() {
        query.value = ''
        currentPage.value = 1
    }

    function onFilterChange() {
        nextTick(() => window.scrollTo({ top: 0, behavior: 'smooth' }))
    }

    watch(
        () => options.projectType.value,
        (newType, oldType) => {
            debug('projectType changed', { from: oldType, to: newType })
            effectiveCurrentSortType.value =
                effectiveSortTypes.value.find((sortType) => sortType.name === 'relevance') ??
                effectiveSortTypes.value[0]
            query.value = ''
        },
    )

    return {
        query,
        filters,
        currentFilters,
        toggledGroups,
        overriddenProvidedFilterTypes,
        serverFilterTypes,
        serverCurrentFilters,
        serverToggledGroups,
        effectiveSortTypes,
        effectiveCurrentSortType,
        loading,
        committed,
        projectHits,
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
        initializeFilters,
        refreshSearch,
        setPage,
        clearSearch,
        onFilterChange,
    }
}

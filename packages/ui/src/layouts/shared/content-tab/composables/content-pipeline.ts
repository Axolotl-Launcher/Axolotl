import Fuse from 'fuse.js'
import type { ComputedRef, Ref } from 'vue'
import { computed, onBeforeUnmount, ref, shallowRef, watch } from 'vue'

import { defineMessages, useVIntl } from '#ui/composables/i18n'
import { commonProjectTypeCategoryMessages, normalizeProjectType } from '#ui/utils/common-messages'

import type { ContentItem } from '../types'
import {
    CONTENT_FILTER_PRUNE_DEBOUNCE_MS,
    type ContentFilterSelections,
    DUPLICATE_FILTER_EMPTY_GRACE_MS,
    pruneContentFilterSelections,
    shouldKeepDuplicateFilterOption,
} from './content-filter-state'
import type { ContentFilterOption } from './content-filtering'
import { hasContentUpdate, isDisabledContentItem, isEnabledContentItem } from './content-filtering'

// Re-export utility functions and types for convenience
export type { ContentFilterOption } from './content-filtering'

// ---- window 级内存持久化（导航切换保留，关软件丢弃） ----

const memory: Record<string, Map<string, unknown>> = ((
    window as unknown as { __ctMemory?: Record<string, Map<string, unknown>> }
).__ctMemory ??= {})
function getMap<K, V>(namespace: string): Map<K, V> {
    if (!memory[namespace]) memory[namespace] = new Map<string, unknown>()
    return memory[namespace] as Map<K, V>
}

// ---- types ----

export interface ContentPipelineConfig {
    items: Ref<ContentItem[]>
    modpackItems?: Ref<ContentItem[] | undefined>
    duplicateItems?: Ref<ContentItem[] | undefined>
    sortItems: (items: ContentItem[]) => ContentItem[]
    getItemId: (item: ContentItem) => string
    showTypeFilters?: boolean
    showUpdateFilter?: boolean
    memoryKey?: string
    searchKeys?: string[]
    initialFilters?: ContentFilterSelections
    filterOptionsReady?: Ref<boolean> | ComputedRef<boolean>
}

interface PipelineResult {
    filteredItems: ContentItem[]
    filteredModpackItems: ContentItem[]
    filterCounts: Record<string, number>
    row1FilterOptions: ContentFilterOption[]
    row2FilterOptions: ContentFilterOption[]
    totalCount: number
}

// ---- messages ----

const filterMessages = defineMessages({
    updates: {
        id: 'content.filter.updates',
        defaultMessage: 'Update available',
    },
    duplicates: {
        id: 'content.filter.duplicates',
        defaultMessage: 'Duplicates',
    },
    enabled: {
        id: 'content.filter.enabled',
        defaultMessage: 'Enabled',
    },
    disabled: {
        id: 'content.filter.disabled',
        defaultMessage: 'Disabled',
    },
})

// ---- composable ----

export function useContentPipeline(config: ContentPipelineConfig) {
    const { formatMessage } = useVIntl()

    const {
        items,
        modpackItems,
        duplicateItems,
        sortItems,
        getItemId,
        showTypeFilters = false,
        showUpdateFilter = false,
        memoryKey = '',
        searchKeys = ['project.title', 'owner.name', 'file_name'],
        initialFilters,
        filterOptionsReady,
    } = config

    // ---- filter state ----

    function normalizeTypeFilters(value: string | string[] | null | undefined): string[] {
        if (!value) return []
        return Array.isArray(value) ? value : [value]
    }

    const filterMemory = getMap<string, { type: string[]; status: string[] }>('filter')
    const savedFilters = memoryKey ? filterMemory.get(memoryKey) : undefined
    const selectedTypeFilter = ref<string[]>(
        normalizeTypeFilters(initialFilters?.typeFilters ?? savedFilters?.type),
    )
    const selectedStatusFilters = ref<string[]>(
        initialFilters?.statusFilters ?? savedFilters?.status ?? [],
    )
    watch(
        () => memoryKey,
        (key) => {
            if (initialFilters) return
            if (key) {
                const entry = filterMemory.get(key)
                selectedTypeFilter.value = normalizeTypeFilters(entry?.type)
                selectedStatusFilters.value = entry?.status ?? []
            }
        },
        { immediate: true },
    )

    watch([selectedTypeFilter, selectedStatusFilters], () => {
        if (memoryKey) {
            filterMemory.set(memoryKey, {
                type: [...selectedTypeFilter.value],
                status: [...selectedStatusFilters.value],
            })
        }
    })

    // ---- search state ----

    const searchMemory = getMap<string, string>('search')
    const searchKey = memoryKey ? `${memoryKey}:search` : ''
    const searchQuery = ref(searchKey ? (searchMemory.get(searchKey) ?? '') : '')

    watch(searchQuery, (val) => {
        if (searchKey) searchMemory.set(searchKey, val)
    })

    // ---- Fuse instance ----

    const fuse = new Fuse<ContentItem>([], {
        keys: searchKeys,
        threshold: 0.4,
        distance: 100,
    })

    // ---- sorted items (computed because they only depend on items + sortMode) ----

    const sortedItems = computed(() => sortItems(items.value))

    const modpackItemsNoUpdate = computed(() => {
        const raw = modpackItems?.value ?? []
        return sortItems(
            raw.map((item) => ({
                ...item,
                update: null,
            })),
        )
    })

    const filterValidationOptions = computed(() => {
        const type = new Set<string>()
        const status = new Set<string>()
        const allItems = [...modpackItemsNoUpdate.value, ...sortedItems.value]
        const duplicateItemIds = new Set(
            (duplicateItems?.value ?? []).map((item) => getItemId(item)),
        )

        for (const item of allItems) {
            type.add(normalizeProjectType(item.project_type))
            if (showUpdateFilter && hasContentUpdate(item)) status.add('updates')
            if (isEnabledContentItem(item)) status.add('enabled')
            if (isDisabledContentItem(item)) status.add('disabled')
        }
        if (allItems.some((item) => duplicateItemIds.has(getItemId(item)))) {
            type.add('duplicates')
        }

        return {
            type: [...type],
            status: [...status],
        }
    })

    const modpackChildIdSet = computed(() => {
        return new Set(
            (modpackItems?.value ?? []).map((item) => getItemId(item).replace(/\.disabled$/, '')),
        )
    })

    const searchableItemCount = computed(() => {
        const modpackList = modpackItems?.value ?? []
        const regularItems = items.value.filter(
            (item) => !modpackChildIdSet.value.has(getItemId(item)),
        )
        return modpackList.length + regularItems.length
    })

    // ---- single-pass pipeline result ----

    const result = shallowRef<PipelineResult>({
        filteredItems: [],
        filteredModpackItems: [],
        filterCounts: {},
        row1FilterOptions: [],
        row2FilterOptions: [],
        totalCount: 0,
    })

    let pipelineTimer: ReturnType<typeof setTimeout> | null = null

    function runPipeline(): PipelineResult {
        const query = searchQuery.value.trim()
        const typeFilters = selectedTypeFilter.value
        const statusFilters = selectedStatusFilters.value

        // Step 1: Fuse search once (old code calls fuse.search 3 times, we call once)
        let fuseResults: ContentItem[] | null = null
        if (query) {
            fuseResults = fuse.search(query).map((r) => r.item)
        }

        // Helper: replicate old search(source) behavior:
        // - no query: return source as-is
        // - with query: return all Fuse results (source parameter is ignored)
        function search(source: ContentItem[]): ContentItem[] {
            if (!query) return source
            return fuseResults!
        }

        // Step 2: Compute searchedAllItems (for filter UI — counts, options, totalCount)
        // Old code: [...modpackSearched.filter(modpackChildIdSet), ...regularSearched.filter(!modpackChildIdSet)]
        const modpackChildIds = modpackChildIdSet.value
        const modpackSearched = search(modpackItemsNoUpdate.value).filter((item) =>
            modpackChildIds.has(getItemId(item).replace(/\.disabled$/, '')),
        )
        const regularSearched = search(sortedItems.value).filter(
            (item) => !modpackChildIds.has(getItemId(item).replace(/\.disabled$/, '')),
        )
        const searchedAllItems = [...modpackSearched, ...regularSearched]
        const duplicateItemIds = new Set(
            (duplicateItems?.value ?? []).map((item) => getItemId(item)),
        )
        const matchesTypeFilter = (item: ContentItem) =>
            typeFilters.includes(normalizeProjectType(item.project_type)) ||
            (typeFilters.includes('duplicates') && duplicateItemIds.has(getItemId(item)))

        // Step 3: Compute typeFilteredItems and statusFilteredItems from searchedAllItems
        const typeFiltered: ContentItem[] =
            typeFilters.length > 0 ? searchedAllItems.filter(matchesTypeFilter) : searchedAllItems
        const hasEnabled = typeFiltered.some(isEnabledContentItem)
        const hasDisabled = typeFiltered.some(isDisabledContentItem)
        const availableStatusFilters = new Set<string>()
        if (showUpdateFilter && typeFiltered.some((item) => hasContentUpdate(item))) {
            availableStatusFilters.add('updates')
        }
        if (hasEnabled) availableStatusFilters.add('enabled')
        if (hasDisabled) availableStatusFilters.add('disabled')
        const effectiveStatusFilters = statusFilters.filter((filter) =>
            availableStatusFilters.has(filter),
        )

        let statusFiltered = searchedAllItems
        if (effectiveStatusFilters.length > 0) {
            statusFiltered = searchedAllItems.filter((item) => {
                for (const f of effectiveStatusFilters) {
                    if (f === 'updates' && !hasContentUpdate(item)) return false
                    if (f === 'enabled' && !isEnabledContentItem(item)) return false
                    if (f === 'disabled' && !isDisabledContentItem(item)) return false
                }
                return true
            })
        }

        // Step 4: Compute filterCounts (matching old semantics)
        const counts: Record<string, number> = {}

        // type counts: from statusFiltered (status-filtered items, NOT type-filtered)
        for (const item of statusFiltered) {
            const type = normalizeProjectType(item.project_type)
            counts[type] = (counts[type] || 0) + 1
        }
        counts['duplicates'] = statusFiltered.filter((item) =>
            duplicateItemIds.has(getItemId(item)),
        ).length

        // status counts: from typeFiltered (type-filtered items, NOT status-filtered)
        counts['updates'] = typeFiltered.filter((m) => hasContentUpdate(m)).length
        counts['enabled'] = typeFiltered.filter(isEnabledContentItem).length
        counts['disabled'] = typeFiltered.filter(isDisabledContentItem).length

        // totalCount: from statusFiltered (same as old code)
        const totalCount = statusFiltered.length

        // Step 5: Build row1FilterOptions from searchedAllItems (ALL items, like old code)
        const row1: ContentFilterOption[] = []
        if (showTypeFilters) {
            const frequency: Record<string, number> = {}
            for (const item of searchedAllItems) {
                const normalized = normalizeProjectType(item.project_type)
                frequency[normalized] = (frequency[normalized] || 0) + 1
            }
            const types = Object.keys(frequency).sort((a, b) => frequency[b] - frequency[a])
            for (const type of types) {
                const msg =
                    commonProjectTypeCategoryMessages[
                        type as keyof typeof commonProjectTypeCategoryMessages
                    ]
                const label = msg
                    ? formatMessage(msg)
                    : type.charAt(0).toUpperCase() + type.slice(1) + 's'
                row1.push({ id: type, label })
            }
            if (searchedAllItems.some((item) => duplicateItemIds.has(getItemId(item)))) {
                row1.push({ id: 'duplicates', label: formatMessage(filterMessages.duplicates) })
            }
        }

        // Step 6: Build row2FilterOptions from typeFiltered (same as old code)
        const row2: ContentFilterOption[] = []
        if (showUpdateFilter && typeFiltered.some((m) => hasContentUpdate(m))) {
            row2.push({ id: 'updates', label: formatMessage(filterMessages.updates) })
        }
        if (hasEnabled && hasDisabled) {
            row2.push({ id: 'enabled', label: formatMessage(filterMessages.enabled) })
            row2.push({ id: 'disabled', label: formatMessage(filterMessages.disabled) })
        }

        // Step 7: Compute filteredItems and filteredModpackItems (matching old layout.vue)
        // Old filteredItems = applyFilters(search(sortedItems))
        // Old filteredModpackItems = applyFilters(search(modpackItemsNoUpdate).filter(modpackIds))
        function applyFilters(source: ContentItem[]): ContentItem[] {
            let result = source
            if (typeFilters.length > 0) {
                result = result.filter(matchesTypeFilter)
            }
            if (effectiveStatusFilters.length > 0) {
                result = result.filter((item) => {
                    for (const f of effectiveStatusFilters) {
                        if (f === 'updates' && !hasContentUpdate(item)) return false
                        if (f === 'enabled' && !isEnabledContentItem(item)) return false
                        if (f === 'disabled' && !isDisabledContentItem(item)) return false
                    }
                    return true
                })
            }
            return result
        }

        const filteredItems = applyFilters(search(sortedItems.value))

        const modpackIds = new Set(modpackItemsNoUpdate.value.map((item) => getItemId(item)))
        const filteredModpackItems =
            modpackItemsNoUpdate.value.length === 0
                ? []
                : applyFilters(
                      search(modpackItemsNoUpdate.value).filter((item) =>
                          modpackIds.has(getItemId(item)),
                      ),
                  )

        return {
            filteredItems,
            filteredModpackItems,
            filterCounts: counts,
            row1FilterOptions: row1,
            row2FilterOptions: row2,
            totalCount,
        }
    }

    // Trigger pipeline when any dependency changes (debounced)
    watch(
        [
            sortedItems,
            modpackItemsNoUpdate,
            duplicateItems,
            searchQuery,
            selectedTypeFilter,
            selectedStatusFilters,
        ],
        () => {
            if (pipelineTimer) clearTimeout(pipelineTimer)
            pipelineTimer = setTimeout(() => {
                result.value = runPipeline()
            }, 100)
        },
        { immediate: true },
    )

    // Update Fuse index asynchronously when items change (separate from pipeline)
    watch(
        () => [sortedItems.value, modpackItemsNoUpdate.value] as const,
        ([sorted, modpack]) => {
            const seenIds = new Set<string>()
            const collection: ContentItem[] = []

            for (const item of sorted) {
                const id = getItemId(item)
                if (!seenIds.has(id)) {
                    seenIds.add(id)
                    collection.push(item)
                }
            }

            for (const item of modpack) {
                const id = getItemId(item)
                if (!seenIds.has(id)) {
                    seenIds.add(id)
                    collection.push(item)
                }
            }

            // Use setTimeout to avoid blocking the main thread
            setTimeout(() => {
                fuse.setCollection(collection)
            }, 0)
        },
        { immediate: true },
    )

    // ---- filter API (compatible with old interface) ----

    const filteredItems = computed(() => result.value.filteredItems)
    const filteredModpackItems = computed(() => result.value.filteredModpackItems)
    const filterCounts = computed(() => result.value.filterCounts)
    const row1FilterOptions = computed(() => result.value.row1FilterOptions)
    const row2FilterOptions = computed(() => result.value.row2FilterOptions)
    const totalCount = computed(() => result.value.totalCount)

    const supportsDuplicateFilter = duplicateItems !== undefined
    let pruneTimer: ReturnType<typeof setTimeout> | null = null
    let duplicateEmptySince: number | null = null

    watch(
        () => duplicateItems?.value?.length ?? 0,
        (count) => {
            duplicateEmptySince = count === 0 ? Date.now() : null
        },
        { immediate: true },
    )

    // Restart the empty-set grace whenever the content set finishes settling, so
    // a slow refresh that ends empty still gets a full window before pruning.
    watch(
        () => filterOptionsReady?.value ?? true,
        (ready) => {
            if (ready && (duplicateItems?.value?.length ?? 0) === 0) {
                duplicateEmptySince = Date.now()
            }
        },
    )

    function applyContentFilterPrune() {
        if (pruneTimer) {
            clearTimeout(pruneTimer)
            pruneTimer = null
        }

        const options = filterValidationOptions.value
        const ready = filterOptionsReady?.value ?? true
        const hasDuplicateItems = (duplicateItems?.value?.length ?? 0) > 0
        const emptyGraceRemainingMs =
            duplicateEmptySince === null
                ? 0
                : Math.max(0, DUPLICATE_FILTER_EMPTY_GRACE_MS - (Date.now() - duplicateEmptySince))
        const keepDuplicatesOption = shouldKeepDuplicateFilterOption({
            supportsDuplicateFilter,
            ready,
            hasDuplicateItems,
            emptyGraceRemainingMs,
        })

        const typeOptions =
            keepDuplicatesOption && !options.type.includes('duplicates')
                ? [...options.type, 'duplicates']
                : options.type
        const pruned = pruneContentFilterSelections(
            {
                typeFilters: selectedTypeFilter.value,
                statusFilters: selectedStatusFilters.value,
            },
            { type: typeOptions, status: options.status },
            ready,
        )
        if (pruned.typeFilters.length !== selectedTypeFilter.value.length) {
            selectedTypeFilter.value = pruned.typeFilters
        }
        if (pruned.statusFilters.length !== selectedStatusFilters.value.length) {
            selectedStatusFilters.value = pruned.statusFilters
        }

        // Kept only because grace is open: re-run once it closes so a stably
        // empty duplicate set can drop the selection instead of pinning it.
        if (supportsDuplicateFilter && ready && !hasDuplicateItems && emptyGraceRemainingMs > 0) {
            pruneTimer = setTimeout(
                applyContentFilterPrune,
                emptyGraceRemainingMs + CONTENT_FILTER_PRUNE_DEBOUNCE_MS,
            )
        }
    }

    watch(
        [
            filterValidationOptions,
            () => filterOptionsReady?.value ?? true,
            () => duplicateItems?.value?.length ?? 0,
        ],
        () => {
            if (pruneTimer) clearTimeout(pruneTimer)
            pruneTimer = setTimeout(applyContentFilterPrune, CONTENT_FILTER_PRUNE_DEBOUNCE_MS)
        },
        { immediate: true },
    )

    onBeforeUnmount(() => {
        if (pruneTimer) clearTimeout(pruneTimer)
    })

    function toggleTypeFilter(filterId: string, event?: MouseEvent | KeyboardEvent) {
        if (event?.ctrlKey || event?.metaKey) {
            selectedTypeFilter.value = selectedTypeFilter.value.includes(filterId)
                ? selectedTypeFilter.value.filter((id) => id !== filterId)
                : [...selectedTypeFilter.value, filterId]
            return
        }

        selectedTypeFilter.value = [filterId]
    }

    function toggleStatusFilter(filterId: string) {
        if (filterId === 'enabled' || filterId === 'disabled') {
            const index = selectedStatusFilters.value.indexOf(filterId)
            const otherStatusFilter = filterId === 'enabled' ? 'disabled' : 'enabled'
            if (index === -1) {
                selectedStatusFilters.value = [
                    ...selectedStatusFilters.value.filter((f) => f !== otherStatusFilter),
                    filterId,
                ]
            } else {
                selectedStatusFilters.value = selectedStatusFilters.value.filter(
                    (_, i) => i !== index,
                )
            }
            return
        }

        const index = selectedStatusFilters.value.indexOf(filterId)
        if (index === -1) {
            selectedStatusFilters.value = [...selectedStatusFilters.value, filterId]
        } else {
            selectedStatusFilters.value = selectedStatusFilters.value.filter((_, i) => i !== index)
        }
    }

    return {
        searchQuery,
        searchableItemCount,
        sortedItems,
        modpackItemsNoUpdate,
        modpackChildIdSet,
        selectedTypeFilter,
        selectedStatusFilters,
        row1FilterOptions,
        row2FilterOptions,
        totalCount,
        filterCounts,
        filteredItems,
        filteredModpackItems,
        toggleTypeFilter,
        toggleStatusFilter,
    }
}

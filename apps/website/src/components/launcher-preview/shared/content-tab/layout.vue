<script setup lang="ts">
import ChevronUpIcon from '@modrinth/assets/icons/chevron-up.svg?component'
import CodeIcon from '@modrinth/assets/icons/code.svg?component'
import DownloadIcon from '@modrinth/assets/icons/download.svg?component'
import DropdownIcon from '@modrinth/assets/icons/dropdown.svg?component'
import FileIcon from '@modrinth/assets/icons/file.svg?component'
import LinkIcon from '@modrinth/assets/icons/link.svg?component'
import ShareIcon from '@modrinth/assets/icons/share.svg?component'
import TextCursorInputIcon from '@modrinth/assets/icons/text-cursor-input.svg?component'
import TrashIcon from '@modrinth/assets/icons/trash.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import type { Option as OverflowMenuOption } from '@modrinth/ui/src/components/base/OverflowMenu.vue'
import PopoutMenu from '@modrinth/ui/src/components/base/PopoutMenu.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import ContentEmptyState from '@modrinth/ui/src/layouts/shared/content-tab/components/ContentEmptyState.vue'
import ContentMetadataFilterBar from '@modrinth/ui/src/layouts/shared/content-tab/components/ContentMetadataFilterBar.vue'
import ContentToolbar from '@modrinth/ui/src/layouts/shared/content-tab/components/ContentToolbar.vue'
import ContentTypeFilter from '@modrinth/ui/src/layouts/shared/content-tab/components/ContentTypeFilter.vue'
import {
    deletePendingItems,
    disablePendingItems,
} from '@modrinth/ui/src/layouts/shared/content-tab/composables/content-deletion.ts'
import {
    duplicateContentGroups,
    duplicateContentRows,
    duplicateUpdateAnchor,
    visibleDuplicateGroups,
} from '@modrinth/ui/src/layouts/shared/content-tab/composables/content-duplicates.ts'
import {
    canToggleContentItem,
    clearPinnedContentViewPreferences,
    type ContentSortMode,
    type ContentViewState,
    getPinnedContentViewPreferences,
    hasContentUpdate,
    isDisabledContentItem,
    isEnabledContentItem,
    isPresentContentItem,
    setPinnedContentViewPreferences,
    sortContentItems,
    useBulkOperation,
    useChangingItems,
    useContentFolderGroups,
    useContentMetadataFilters,
    useContentPipeline,
    useContentSelection,
} from '@modrinth/ui/src/layouts/shared/content-tab/composables/index.ts'
import { injectContentManager } from '@modrinth/ui/src/layouts/shared/content-tab/providers/content-manager.ts'
import type {
    ContentCardTableItem,
    ContentItem,
    ContentWorldGroupMeta,
} from '@modrinth/ui/src/layouts/shared/content-tab/types.ts'
import { commonMessages, normalizeProjectType } from '@modrinth/ui/src/utils/common-messages.ts'
import { computed, markRaw, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'

import { usePreviewDomain } from '../../lib/state'
import { injectPreviewWindow } from '../../lib/window'
import ContentCardTable from './components/ContentCardTable.vue'
import ContentSelectionBar from './components/ContentSelectionBar.vue'
import ContentTableHeaderActions from './components/ContentTableHeaderActions.vue'
import ConfirmBulkUpdateModal from './components/modals/ConfirmBulkUpdateModal.vue'
import ConfirmDeletionModal from './components/modals/ConfirmDeletionModal.vue'
import ConfirmUnlinkModal from './components/modals/ConfirmUnlinkModal.vue'
import ContentDependencyWarningModal from './components/modals/ContentDependencyWarningModal.vue'

const { formatMessage, locale } = useVIntl()

const props = withDefaults(
    defineProps<{
        bottomPadding?: boolean
    }>(),
    {
        bottomPadding: true,
    },
)

const emit = defineEmits<{
    visibleItems: [items: ContentItem[]]
}>()

function handleVisibleTableItems(visibleTableItems: ContentCardTableItem[]) {
    const visibleContentItems = visibleTableItems
        .flatMap((item) =>
            item.isGroupHeader && item.groupKind === 'duplicate'
                ? (item.groupChildIds ?? [])
                : [item.id],
        )
        .map((id) => findContentItem(id))
        .filter((item): item is ContentItem => item !== undefined)

    emit('visibleItems', visibleContentItems)
}

const messages = defineMessages({
    dependencyDisableFailed: {
        id: 'content.page-layout.dependency-disable-failed',
        defaultMessage: 'Dependent content could not be disabled. Retry to finish the deletion.',
    },
    scrollToTop: { id: 'content.page-layout.scroll-to-top', defaultMessage: 'Scroll to top' },
    updateDuplicateGroup: {
        id: 'content.page-layout.update-duplicate-group',
        defaultMessage: 'Review updates for all {count, number} copies',
    },
    duplicateStatus: {
        id: 'content.page-layout.duplicate-status',
        defaultMessage:
            '{visible} matching · All copies: {updates} updates · {disabled} disabled · {missing} missing · {installing} in progress',
    },
    failedToLoad: {
        id: 'content.page-layout.failed-to-load',
        defaultMessage: 'Failed to load content',
    },
    sortAlphabetical: {
        id: 'content.page-layout.sort.alphabetical',
        defaultMessage: 'Alphabetical',
    },
    sortProjectNameAscending: {
        id: 'content.page-layout.sort.project-name-ascending',
        defaultMessage: 'Project name A-Z',
    },
    sortProjectNameDescending: {
        id: 'content.page-layout.sort.project-name-descending',
        defaultMessage: 'Project name Z-A',
    },
    sortFileNameAscending: {
        id: 'content.page-layout.sort.file-name-ascending',
        defaultMessage: 'File name A-Z',
    },
    sortFileNameDescending: {
        id: 'content.page-layout.sort.file-name-descending',
        defaultMessage: 'File name Z-A',
    },
    sortDateAddedNewest: {
        id: 'content.page-layout.sort.date-added-newest',
        defaultMessage: 'Newest first',
    },
    sortDateAddedOldest: {
        id: 'content.page-layout.sort.date-added-oldest',
        defaultMessage: 'Oldest first',
    },
    updateAll: {
        id: 'content.page-layout.update-all',
        defaultMessage: 'Update all',
    },
    noContentFound: {
        id: 'content.page-layout.no-content-found',
        defaultMessage: 'No content found.',
    },
    shareProjectNames: {
        id: 'content.page-layout.share.project-names',
        defaultMessage: 'Project names',
    },
    shareFileNames: {
        id: 'content.page-layout.share.file-names',
        defaultMessage: 'File names',
    },
    shareProjectLinks: {
        id: 'content.page-layout.share.project-links',
        defaultMessage: 'Project links',
    },
    shareMarkdownLinks: {
        id: 'content.page-layout.share.markdown-links',
        defaultMessage: 'Markdown links',
    },
    share: {
        id: 'content.page-layout.share.label',
        defaultMessage: 'Share',
    },
    sortByLabel: {
        id: 'content.page-layout.sort.label',
        defaultMessage: 'Sort by {mode}',
    },
    viewOptions: {
        id: 'content.page-layout.view-options',
        defaultMessage: 'View options',
    },
    pinView: {
        id: 'content.page-layout.pin-view',
        defaultMessage: 'Pin current view',
    },
    unpinView: {
        id: 'content.page-layout.unpin-view',
        defaultMessage: 'Unpin current view',
    },
    resetView: {
        id: 'content.page-layout.reset-view',
        defaultMessage: 'Reset view',
    },
})

const ctx = injectContentManager()
// Menus open inside the launcher window, as the app's open inside its own.
const { layer: previewLayer } = injectPreviewWindow()
const skipNonEssentialWarnings = computed(() => ctx.skipNonEssentialWarnings?.value ?? false)

// window 级内存（导航切换保留，关软件丢弃）: the launcher window's, so a new preview starts empty.
const memory = usePreviewDomain('content-layout:memory', () => ({
    value: markRaw({}) as Record<string, Map<string, unknown>>,
}))
function getMap<K, V>(namespace: string): Map<K, V> {
    if (!memory[namespace]) memory[namespace] = new Map<string, unknown>()
    return memory[namespace] as Map<K, V>
}

function getItemId(item: ContentItem) {
    return ctx.getItemId?.(item) ?? item.file_path ?? item.file_name ?? item.id
}

function findContentItem(id: string): ContentItem | undefined {
    const item = ctx.items.value.find((i) => getItemId(i) === id)
    if (item) return item
    return ctx.modpackItems?.value?.find((i) => getItemId(i) === id)
}

const viewStateMemory = getMap<string, Partial<ContentViewState>>('viewState')
const initialPinnedView = ctx.instanceId ? getPinnedContentViewPreferences(ctx.instanceId) : null
const pinnedView = ref(initialPinnedView !== null)
const initialViewState = ctx.instanceId ? viewStateMemory.get(ctx.instanceId) : undefined

function updateSessionViewState(patch: Partial<ContentViewState>) {
    if (!ctx.instanceId) return
    viewStateMemory.set(ctx.instanceId, {
        ...(viewStateMemory.get(ctx.instanceId) ?? {}),
        ...patch,
    })
}

const sortMemory = getMap<string, ContentSortMode>('sort')
const sortMode = ref<ContentSortMode>(
    initialPinnedView?.sortMode ??
        (ctx.instanceId
            ? (sortMemory.get(ctx.instanceId) ?? 'project-name-asc')
            : 'project-name-asc'),
)

watch(sortMode, (val) => {
    if (ctx.instanceId) sortMemory.set(ctx.instanceId, val)
})

const sortLabels: Record<ContentSortMode, () => string> = {
    'project-name-asc': () => formatMessage(messages.sortProjectNameAscending),
    'project-name-desc': () => formatMessage(messages.sortProjectNameDescending),
    'file-name-asc': () => formatMessage(messages.sortFileNameAscending),
    'file-name-desc': () => formatMessage(messages.sortFileNameDescending),
    'date-added-newest': () => formatMessage(messages.sortDateAddedNewest),
    'date-added-oldest': () => formatMessage(messages.sortDateAddedOldest),
}

const sortOptions = computed(() => [
    { id: 'project-name-asc' as const, label: formatMessage(messages.sortProjectNameAscending) },
    { id: 'project-name-desc' as const, label: formatMessage(messages.sortProjectNameDescending) },
    { id: 'file-name-asc' as const, label: formatMessage(messages.sortFileNameAscending) },
    { id: 'file-name-desc' as const, label: formatMessage(messages.sortFileNameDescending) },
    { id: 'date-added-newest' as const, label: formatMessage(messages.sortDateAddedNewest) },
    { id: 'date-added-oldest' as const, label: formatMessage(messages.sortDateAddedOldest) },
])

function sortItems(items: ContentItem[]): ContentItem[] {
    return sortContentItems(items, sortMode.value, locale.value, getItemId)
}

const filterOptionsReady = computed(
    () =>
        ctx.filterOptionsReady?.value ??
        (!ctx.loading.value && ctx.items.value.length + (ctx.modpackItems?.value?.length ?? 0) > 0),
)

const {
    searchQuery,
    searchableItemCount,
    modpackItemsNoUpdate,
    modpackChildIdSet,
    selectedTypeFilter,
    selectedStatusFilters,
    row1FilterOptions,
    row2FilterOptions,
    totalCount,
    filterCounts,
    filteredItems: pipelineFilteredItems,
    filteredModpackItems: pipelineFilteredModpackItems,
    toggleTypeFilter,
    toggleStatusFilter,
} = useContentPipeline({
    items: ctx.items,
    modpackItems: ctx.modpackItems,
    duplicateItems: ctx.duplicateItems,
    sortItems,
    getItemId,
    showTypeFilters: true,
    showUpdateFilter: ctx.hasUpdateSupport,
    memoryKey: ctx.instanceId,
    initialFilters: initialPinnedView ?? undefined,
    filterOptionsReady,
})

const {
    metadataFilterCategories,
    getSelectedValues: getMetadataSelectedValues,
    setCategorySelection,
    isCategoryFiltering,
    applyMetadataFilters,
    excluded: metadataExcluded,
    setExcludedValues,
} = useContentMetadataFilters(
    computed(() => [...ctx.items.value, ...(ctx.modpackItems?.value ?? [])]),
    ctx.instanceId,
    initialPinnedView?.metadataExcluded,
    filterOptionsReady,
)

const metadataFilterSelectedValues = computed(() =>
    Object.fromEntries(
        metadataFilterCategories.value.map((category) => [
            category.key,
            getMetadataSelectedValues(category.key),
        ]),
    ),
)
const metadataFilteringKeys = computed(() =>
    metadataFilterCategories.value
        .filter((category) => isCategoryFiltering(category.key))
        .map((category) => category.key),
)
const metadataFilterExpanded = ref(initialViewState?.metadataFilterExpanded ?? false)
watch(metadataFilterExpanded, (expanded) => {
    updateSessionViewState({ metadataFilterExpanded: expanded })
})

const filteredItems = computed(() => applyMetadataFilters(pipelineFilteredItems.value))
const filteredModpackItems = computed(() =>
    applyMetadataFilters(pipelineFilteredModpackItems.value),
)

const activeMetadataFilterCount = computed(() => metadataFilteringKeys.value.length)

const { selectedIds, selectedItems, clearSelection, removeFromSelection } = useContentSelection(
    computed(() => {
        return [...modpackItemsNoUpdate.value, ...ctx.items.value]
    }),
    getItemId,
    computed(() => [...filteredItems.value, ...filteredModpackItems.value]),
)

const {
    isBulkOperating,
    bulkProgress,
    bulkTotal,
    bulkOperation,
    bulkWaiting,
    runBulk,
    runBulkWithWaiting,
} = useBulkOperation()

// Sync bulk operation state back to the content manager so providers can suppress refreshes
if (ctx.isBulkOperating) {
    watch(isBulkOperating, (val) => {
        ctx.isBulkOperating!.value = val
    })
}

const { isChanging, markChanging, unmarkChanging } = useChangingItems()

const bulkStatusMessage = ref<string | null>(null)
const bulkItemCount = ref(0)

// 整合包分组展开状态（导航切换保留，关软件丢弃）
const expandedGroupsMemory = getMap<string, Set<string>>('expandedGroups')

const refreshing = ref(false)

const expandedGroups = ref<Set<string>>(
    ctx.instanceId
        ? new Set(
              initialViewState?.expandedGroups ?? expandedGroupsMemory.get(ctx.instanceId) ?? [],
          )
        : new Set(),
)

watch(expandedGroups, (groups) => {
    if (ctx.instanceId) expandedGroupsMemory.set(ctx.instanceId, groups)
    updateSessionViewState({ expandedGroups: [...groups] })
})

function toggleGroupExpand(groupId: string) {
    const newSet = new Set(expandedGroups.value)
    if (newSet.has(groupId)) {
        newSet.delete(groupId)
    } else {
        newSet.add(groupId)
    }
    expandedGroups.value = newSet
}

watch(searchQuery, (query) => {
    if (query.trim()) {
        expandedGroups.value = new Set([...expandedGroups.value, 'modpack'])
    }
})

function getPinnedViewPreferences() {
    return {
        sortMode: sortMode.value,
        typeFilters: [...selectedTypeFilter.value],
        statusFilters: [...selectedStatusFilters.value],
        metadataExcluded: metadataExcluded.value,
    }
}

function togglePinnedView() {
    if (!ctx.instanceId) return
    if (pinnedView.value) {
        clearPinnedContentViewPreferences(ctx.instanceId)
        pinnedView.value = false
        return
    }

    pinnedView.value = setPinnedContentViewPreferences(ctx.instanceId, getPinnedViewPreferences())
}

function resetView() {
    sortMode.value = 'project-name-asc'
    searchQuery.value = ''
    selectedTypeFilter.value = []
    selectedStatusFilters.value = []
    setExcludedValues({})
    metadataFilterExpanded.value = false
    expandedGroups.value = new Set()
    if (ctx.instanceId) {
        clearPinnedContentViewPreferences(ctx.instanceId)
        updateSessionViewState({ scrollTop: 0, anchorId: undefined, anchorOffset: undefined })
    }
    pinnedView.value = false
}

watch(
    [sortMode, selectedTypeFilter, selectedStatusFilters, metadataExcluded],
    () => {
        if (pinnedView.value && ctx.instanceId) {
            setPinnedContentViewPreferences(ctx.instanceId, getPinnedViewPreferences())
        }
    },
    { deep: true },
)

const showScrollToTop = ref(false)
const SCROLL_THRESHOLD = 300

function getScrollContainer(): Element | null {
    return document.querySelector('.app-viewport')
}

function captureContentViewScroll() {
    if (!ctx.instanceId) return
    const container = getScrollContainer()
    if (!container) return

    const viewportTop = container.getBoundingClientRect().top
    const rows = [...document.querySelectorAll<HTMLElement>('[data-content-card-item-id]')]
    const anchor = rows
        .map((row) => ({ row, offset: row.getBoundingClientRect().top - viewportTop }))
        .filter(({ offset }) => offset >= 0)
        .sort((left, right) => left.offset - right.offset)[0]

    updateSessionViewState({
        scrollTop: container.scrollTop,
        anchorId: anchor?.row.dataset.contentCardItemId,
        anchorOffset: anchor?.offset,
    })
}

let scrollStateRestored = false

function restoreContentViewScroll() {
    if (scrollStateRestored || !ctx.instanceId || tableItems.value.length === 0) return
    const saved = viewStateMemory.get(ctx.instanceId)
    if (saved?.scrollTop === undefined) {
        scrollStateRestored = true
        return
    }

    void nextTick(() => {
        const container = getScrollContainer()
        const table = document.querySelector<HTMLElement>('[data-content-card-table]')
        if (!container || !table) return

        let top = saved.scrollTop ?? 0
        const anchorIndex = saved.anchorId
            ? tableItems.value.findIndex((item) => item.id === saved.anchorId)
            : -1
        if (anchorIndex >= 0) {
            const containerRect = container.getBoundingClientRect()
            const tableTop =
                table.getBoundingClientRect().top - containerRect.top + container.scrollTop
            top = tableTop + 48 + anchorIndex * 74 - (saved.anchorOffset ?? 0)
        }

        container.scrollTo({ top: Math.max(0, top) })
        scrollStateRestored = true
    })
}

function handleScroll() {
    const container = getScrollContainer()
    if (container) {
        showScrollToTop.value = container.scrollTop > SCROLL_THRESHOLD
    }
}

function scrollToTop() {
    const container = getScrollContainer()
    if (container) {
        container.scrollTo({ top: 0, behavior: 'smooth' })
    }
}

onMounted(() => {
    const container = getScrollContainer()
    if (container) {
        container.addEventListener('scroll', handleScroll, { passive: true })
        handleScroll()
    }
})

onBeforeUnmount(() => {
    captureContentViewScroll()
    const container = getScrollContainer()
    if (container) {
        container.removeEventListener('scroll', handleScroll)
    }
})

async function handleRefresh() {
    if (refreshing.value) return
    refreshing.value = true
    try {
        await ctx.refresh()
    } finally {
        refreshing.value = false
    }
}

function mapToTableItem(item: ContentItem, group?: string): ContentCardTableItem {
    const base = ctx.mapToTableItem(item)
    const id = getItemId(item)
    return {
        ...base,
        id,
        group,
        disabled: isChanging(id) || ctx.isBusy.value || item.installing === true,
        disabledTooltip: ctx.isBusy.value
            ? (ctx.busyMessage?.value ?? null)
            : (base.disabledTooltip ?? null),
        toggleDisabled: ctx.isBusy.value || base.toggleDisabled === true,
        toggleDisabledTooltip: ctx.isBusy.value
            ? (ctx.busyMessage?.value ?? null)
            : (base.toggleDisabledTooltip ?? null),
        installing: item.installing === true,
        pendingManualDownload: item.pendingManualDownload === true,
        hasUpdate:
            !group && hasContentUpdate(item) && item.instanceCapabilities?.canUpdate !== false,
        rollbackFileName: item.rollback?.file_name,
        hideSwitchVersion: base.hideSwitchVersion ?? !base.versionLink,
        overflowOptions: buildItemOverflowOptions(item),
    }
}

/**
 * Overflow-menu options for a content row. Deletion is always available as a
 * row-level trash button, so the overflow menu only carries non-destructive
 * actions.
 */
function buildItemOverflowOptions(item: ContentItem): OverflowMenuOption[] | undefined {
    const options = ctx.getOverflowOptions?.(item) ?? []
    return options.length > 0 ? options : undefined
}

function datapackTreePath(item: ContentItem): string {
    const path = item.file_path ?? item.file_name
    const segments = path.split(/[\\/]/).filter(Boolean)
    if (
        segments[0]?.toLocaleLowerCase() === 'saves' &&
        segments[2]?.toLocaleLowerCase() === 'datapacks'
    ) {
        return segments.slice(1, 2).concat(segments.slice(3)).join('/')
    }
    return path
}

const allDuplicates = computed(() =>
    duplicateContentGroups([...ctx.items.value, ...(ctx.modpackItems?.value ?? [])], getItemId),
)
const duplicates = computed(() =>
    visibleDuplicateGroups(
        allDuplicates.value,
        [...filteredItems.value, ...filteredModpackItems.value],
        getItemId,
    ),
)
const duplicateIds = computed(
    () => new Set([...duplicates.value.values()].flatMap((children) => children.map(getItemId))),
)
const nonDuplicateItems = computed(() =>
    filteredItems.value.filter((item) => !duplicateIds.value.has(getItemId(item))),
)

const { folderRows: schematicFolderRows, regularItems: schematicGroupRegularItems } =
    useContentFolderGroups({
        filteredItems: nonDuplicateItems,
        modpackChildIdSet,
        searchQuery,
        expandedGroups,
        persistExpandedGroups: (groups) => {
            if (ctx.instanceId) expandedGroupsMemory.set(ctx.instanceId, groups)
        },
        getItemId,
        mapToTableItem,
        isGroupedItem: (item) =>
            ['schematic', 'datapack'].includes(normalizeProjectType(item.project_type)),
        treePath: (item) => {
            const path = item.file_path ?? item.file_name
            const segments = path.split(/[\\/]/).filter(Boolean)
            if (segments[0]?.toLocaleLowerCase() === 'schematics') {
                return segments.slice(1).join('/')
            }
            return datapackTreePath(item)
        },
        folderGroupId: (path) => {
            const isWorldDatapackFolder = filteredItems.value.some((item) => {
                if (item.source_kind !== 'world_datapack') return false
                const itemPath = datapackTreePath(item)
                return itemPath === path || itemPath.startsWith(`${path}/`)
            })
            return isWorldDatapackFolder ? `datapack-folder:${path}` : `schematic-folder:${path}`
        },
        folderGroupIdPrefix: ['schematic-folder:', 'datapack-folder:'],
        locale,
    })

function getWorldGroupMeta(row: ContentCardTableItem): ContentWorldGroupMeta | undefined {
    if (!row.isGroupHeader || !row.group?.startsWith('datapack-folder:')) return undefined
    const child = row.groupChildIds
        ?.map((id) => findContentItem(id))
        .find((item) => item?.source_kind === 'world_datapack' && item?.groupMeta)
    return child?.groupMeta
}

const folderRows = computed<ContentCardTableItem[]>(() =>
    schematicFolderRows.value.map((row) => {
        const meta = getWorldGroupMeta(row)
        if (!meta) return row
        return {
            ...row,
            groupKind: 'world',
            groupMeta: meta,
            project: {
                ...row.project,
                title: meta.title ?? row.project.title,
                icon_url: meta.icon_url ?? row.project.icon_url,
            },
        }
    }),
)

const tableItems = computed<ContentCardTableItem[]>(() => {
    const items: ContentCardTableItem[] = []

    const modpackItems = filteredModpackItems.value.filter(
        (item) => !duplicateIds.value.has(getItemId(item)),
    )
    const modpack = ctx.modpack.value

    if (modpack && modpack.project && modpackItems.length > 0) {
        const groupItems: ContentCardTableItem[] = []
        const childIds = modpackItems.map((item) => getItemId(item))
        const presentChildren = modpackItems.filter(isPresentContentItem)
        const allChildrenDisabled =
            presentChildren.length > 0 && presentChildren.every(isDisabledContentItem)
        if (expandedGroups.value.has('modpack')) {
            for (const item of modpackItems) {
                groupItems.push(mapToTableItem(item, 'modpack'))
            }
        }

        items.push({
            id: '__modpack_group__',
            isGroupHeader: true,
            group: 'modpack',
            groupItemCount: modpackItems.length,
            groupChildIds: childIds,
            groupSwitchVersion: ctx.updateModpack,
            project: {
                id: modpack.project.id,
                slug: modpack.project.slug,
                title: modpack.project.title,
                icon_url: modpack.project.icon_url,
            },
            projectLink: modpack.projectLink,
            version: modpack.version
                ? {
                      id: modpack.version.id,
                      version_number: modpack.version.version_number,
                      file_name: '',
                      date_published: modpack.version.date_published,
                  }
                : undefined,
            versionLink: modpack.versionLink,
            owner: modpack.owner,
            enabled: true,
            hasUpdate: modpack.hasUpdate,
            disabled:
                modpack.disabled ||
                ctx.isBusy.value ||
                isBulkOperating.value ||
                allChildrenDisabled,
            disabledTooltip:
                modpack.disabledText ??
                (ctx.isBusy.value ? (ctx.busyMessage?.value ?? null) : null),
            downloads: modpack.project.downloads ?? null,
            categories: modpack.categories,
        })

        items.push(...groupItems)
    }

    items.push(
        ...duplicateContentRows({
            groups: allDuplicates.value,
            visibleGroups: duplicates.value,
            expanded: expandedGroups.value,
            getId: getItemId,
            mapItem: mapToTableItem,
            summary: (status, visible) =>
                formatMessage(messages.duplicateStatus, { ...status, visible }),
            updateLabel: (count) => formatMessage(messages.updateDuplicateGroup, { count }),
            canUpdate: !!ctx.updateDuplicateGroup,
            isBusy: (children) =>
                ctx.isBusy.value ||
                children.some((item) => isChanging(getItemId(item)) || item.installing === true),
        }),
    )

    items.push(...folderRows.value)
    for (const item of schematicGroupRegularItems.value) {
        items.push(mapToTableItem(item))
    }

    return items
})

watch(tableItems, restoreContentViewScroll, { flush: 'post', immediate: true })

const hasOutdatedProjects = computed(() => {
    const outdated = ctx.items.value.filter((p) => hasContentUpdate(p))
    const modpackHasUpdate =
        ctx.bulkUpdateIncludesModpack !== false && (ctx.modpack.value?.hasUpdate ?? false)
    return outdated.length > 0 || modpackHasUpdate
})

//  Deletion
const pendingDeletionItems = ref<ContentItem[]>([])
const confirmDeletionModal = ref<InstanceType<typeof ConfirmDeletionModal>>()
const contentDependencyWarningModal = ref<InstanceType<typeof ContentDependencyWarningModal>>()
const pendingDependencyWarningItems = ref<ContentCardTableItem[]>([])
const pendingDependencyWarningDependents = ref<
    Array<{
        item: ContentCardTableItem
        dependencies: ContentCardTableItem[]
    }>
>([])
const pendingDependencyWarningDisableTargets = ref<ContentItem[]>([])

function mapToDisplayItem(item: ContentItem) {
    return {
        ...ctx.mapToTableItem(item),
        id: getItemId(item),
    }
}

async function promptDeleteItems(items: ContentItem[], event?: MouseEvent) {
    if (items.length === 0) return
    pendingDeletionItems.value = items
    pendingDependencyWarningItems.value = []
    pendingDependencyWarningDependents.value = []
    pendingDependencyWarningDisableTargets.value = []
    const deletingIds = new Set(items.map(getItemId))

    const warning = ctx.getDeleteDependencyWarning
        ? await Promise.resolve()
              .then(() => ctx.getDeleteDependencyWarning!(items))
              .catch(() => null)
        : null
    if (warning) {
        const remainingDependents = warning.dependents.filter(
            (dependent) => !deletingIds.has(getItemId(dependent.item)),
        )

        if (remainingDependents.length === 0) {
            showDeletionConfirmation(event)
            return
        }

        const relevantDependencyIds = new Set(
            remainingDependents.flatMap((dependent) => dependent.dependencies.map(getItemId)),
        )
        const warningItems = items.filter((item) => relevantDependencyIds.has(getItemId(item)))
        if (warningItems.length === 0) {
            showDeletionConfirmation(event)
            return
        }

        pendingDependencyWarningItems.value = warningItems.map(mapToDisplayItem)
        pendingDependencyWarningDependents.value = remainingDependents.map((dependent) => ({
            item: mapToDisplayItem(dependent.item),
            dependencies: dependent.dependencies
                .filter((dependency) => relevantDependencyIds.has(getItemId(dependency)))
                .map(mapToDisplayItem),
        }))
        pendingDependencyWarningDisableTargets.value = remainingDependents.map(
            (dependent) => dependent.item,
        )
        contentDependencyWarningModal.value?.show()
        return
    }

    showDeletionConfirmation(event)
}

async function showDeletionConfirmation(event?: MouseEvent) {
    if ((event?.shiftKey || skipNonEssentialWarnings.value) && !ctx.isBusy.value) {
        void confirmDelete().catch((error) => {
            ctx.error.value = error instanceof Error ? error : new Error(String(error))
        })
    } else {
        await nextTick()
        confirmDeletionModal.value?.show()
    }
}

async function handleDeleteById(id: string, event?: MouseEvent) {
    const item = findContentItem(id)
    if (item) {
        await promptDeleteItems([item], event)
    }
}

async function showBulkDeleteModal(event?: MouseEvent) {
    await promptDeleteItems([...selectedItems.value], event)
}

async function confirmDependencyWarningDelete(disableDependentsAfterDeleting: boolean) {
    if (disableDependentsAfterDeleting) {
        pendingDependencyWarningDisableTargets.value =
            pendingDependencyWarningDisableTargets.value.filter((item) => item.enabled)
    } else {
        pendingDependencyWarningDisableTargets.value = []
    }

    await confirmDelete()
    pendingDependencyWarningItems.value = []
    pendingDependencyWarningDependents.value = []
}

async function disablePendingDependencyWarningDependents() {
    const items = pendingDependencyWarningDisableTargets.value.filter((item) => item.enabled)
    if (items.length === 0) return

    if (ctx.bulkDisableItems) {
        await disablePendingItems(
            pendingDependencyWarningDisableTargets,
            () => ctx.bulkDisableItems!(items),
            () => new Error(formatMessage(messages.dependencyDisableFailed)),
        )
        return
    }

    for (const item of items) {
        const id = getItemId(item)
        markChanging(id)
        try {
            await ctx.toggleEnabled(item)
            pendingDependencyWarningDisableTargets.value =
                pendingDependencyWarningDisableTargets.value.filter(
                    (target) => getItemId(target) !== id,
                )
        } finally {
            unmarkChanging(id)
        }
    }
}

async function confirmDelete() {
    if (ctx.isBusy.value) return
    const itemsToDelete = [...pendingDeletionItems.value]
    await runBulkWithWaiting('delete', itemsToDelete.length, () =>
        deletePendingItems(pendingDeletionItems, ctx.deleteItem, getItemId, removeFromSelection),
    )
    await disablePendingDependencyWarningDependents()
    clearSelection()
}

async function handleToggleEnabledById(id: string, _value: boolean) {
    if (ctx.isBusy.value) return
    const item = findContentItem(id)
    if (!item) return
    markChanging(id)
    try {
        await ctx.toggleEnabled(item)
    } catch (error) {
        ctx.error.value = error instanceof Error ? error : new Error(String(error))
    } finally {
        unmarkChanging(id)
    }
}

async function handleRollbackById(id: string) {
    if (ctx.isBusy.value || !ctx.rollbackItem) return
    const item = findContentItem(id)
    if (!item || !item.rollback) return
    markChanging(id)
    try {
        await ctx.rollbackItem(item)
    } finally {
        unmarkChanging(id)
    }
}

async function bulkEnable() {
    if (ctx.isBusy.value) return
    try {
        const items = selectedItems.value.filter(
            (item) => canToggleContentItem(item) && isDisabledContentItem(item),
        )
        if (items.length === 0) return
        if (ctx.bulkEnableItems) {
            await runBulkWithWaiting(
                'enable',
                items.length,
                async () => {
                    return await ctx.bulkEnableItems!(items)
                },
                clearSelection,
            )
            return
        }
        await runBulk('enable', items, (item) => ctx.toggleEnabled(item), {
            onComplete: clearSelection,
        })
    } catch (error) {
        ctx.error.value = error instanceof Error ? error : new Error(String(error))
    }
}

async function bulkDisable() {
    if (ctx.isBusy.value) return
    try {
        const items = selectedItems.value.filter(
            (item) => canToggleContentItem(item) && isEnabledContentItem(item),
        )
        if (items.length === 0) return
        if (ctx.bulkDisableItems) {
            await runBulkWithWaiting(
                'disable',
                items.length,
                async () => {
                    return await ctx.bulkDisableItems!(items)
                },
                clearSelection,
            )
            return
        }
        await runBulk('disable', items, (item) => ctx.toggleEnabled(item), {
            onComplete: clearSelection,
        })
    } catch (error) {
        ctx.error.value = error instanceof Error ? error : new Error(String(error))
    }
}

async function handleUpdateById(id: string) {
    if (ctx.isBusy.value || isChanging(id)) return
    markChanging(id)
    try {
        if (id === '__modpack_group__') {
            ctx.updateModpack?.()
            return
        }
        const duplicate = tableItems.value.find(
            (row) => row.id === id && row.isGroupHeader && row.groupKind === 'duplicate',
        )
        if (duplicate) {
            if (duplicate.disabled) return
            const anchor = findContentItem(duplicateUpdateAnchor(tableItems.value, id) ?? '')
            if (anchor) await ctx.updateDuplicateGroup?.(anchor)
            return
        }
        const item = findContentItem(id)
        if (item) {
            await ctx.updateItem?.(item)
        }
    } catch (error) {
        ctx.error.value = error instanceof Error ? error : new Error(String(error))
    } finally {
        unmarkChanging(id)
    }
}

function handleSwitchVersionById(id: string) {
    const item = findContentItem(id)
    if (item) {
        ctx.switchVersion?.(item)
    }
}

// Bulk updating
const confirmBulkUpdateModal = ref<InstanceType<typeof ConfirmBulkUpdateModal>>()
const pendingBulkUpdateItems = ref<ContentItem[]>([])
const pendingBulkUpdateAll = ref(false)

const hasBulkUpdateSupport = computed(
    () => !!(ctx.bulkUpdateAll || ctx.bulkUpdateItem || ctx.bulkUpdateItems),
)

function promptUpdateAll(event?: MouseEvent) {
    if (!hasBulkUpdateSupport.value) return
    const items = ctx.items.value.filter((item) => hasContentUpdate(item))
    const modpackHasUpdate =
        ctx.bulkUpdateIncludesModpack !== false && (ctx.modpack.value?.hasUpdate ?? false)
    if (items.length === 0 && !modpackHasUpdate) return
    pendingBulkUpdateItems.value = items
    pendingBulkUpdateAll.value = true
    if ((event?.shiftKey || skipNonEssentialWarnings.value) && !ctx.isBusy.value) {
        confirmBulkUpdate()
    } else {
        confirmBulkUpdateModal.value?.show()
    }
}

function promptUpdateSelected(event?: MouseEvent) {
    if (!hasBulkUpdateSupport.value) return
    const items = selectedItems.value.filter((item) => hasContentUpdate(item))
    if (items.length === 0) return
    pendingBulkUpdateItems.value = items
    pendingBulkUpdateAll.value = false
    if ((event?.shiftKey || skipNonEssentialWarnings.value) && !ctx.isBusy.value) {
        confirmBulkUpdate()
    } else {
        confirmBulkUpdateModal.value?.show()
    }
}

function shareSelectedItems(
    format: 'names' | 'file-names' | 'urls' | 'markdown',
    hide: () => void,
) {
    ctx.shareItems?.(selectedItems.value, format)
    hide()
}

async function confirmBulkUpdate() {
    if (ctx.isBusy.value) return
    const items = pendingBulkUpdateItems.value
    const modpackHasUpdate =
        ctx.bulkUpdateIncludesModpack !== false && (ctx.modpack.value?.hasUpdate ?? false)
    if (items.length === 0 && !modpackHasUpdate) return
    if (!hasBulkUpdateSupport.value) return

    try {
        if (pendingBulkUpdateAll.value && ctx.bulkUpdateAll) {
            const totalCount = items.length + (modpackHasUpdate ? 1 : 0)
            bulkItemCount.value = totalCount
            await runBulkWithWaiting('update', totalCount, ctx.bulkUpdateAll, () => {
                clearSelection()
                bulkItemCount.value = 0
                bulkStatusMessage.value = null
            })
        } else if (ctx.bulkUpdateItems) {
            bulkItemCount.value = items.length
            await runBulkWithWaiting(
                'update',
                items.length,
                async () => {
                    return await ctx.bulkUpdateItems(items)
                },
                () => {
                    clearSelection()
                    bulkItemCount.value = 0
                    bulkStatusMessage.value = null
                },
            )
        } else if (ctx.bulkUpdateItem) {
            await runBulk('update', items, ctx.bulkUpdateItem, { onComplete: clearSelection })
        }
    } finally {
        bulkItemCount.value = 0
        bulkStatusMessage.value = null
        pendingBulkUpdateItems.value = []
        pendingBulkUpdateAll.value = false
    }
}

const confirmUnlinkModal = ref<InstanceType<typeof ConfirmUnlinkModal>>()
</script>

<template>
    <div class="flex flex-col gap-4" :class="{ 'pb-6': props.bottomPadding }">
        <template v-if="!ctx.loading.value">
            <div
                v-if="ctx.error.value"
                class="flex w-full flex-col items-center justify-center gap-4 p-4"
            >
                <div class="universal-card flex flex-col items-center gap-4 p-6">
                    <h2 class="m-0 text-xl font-bold">
                        {{ formatMessage(messages.failedToLoad) }}
                    </h2>
                    <p class="text-[var(--color-text-tertiary)]">{{ ctx.error.value.message }}</p>
                    <Button type="colored" color="brand" @click="handleRefresh">{{
                        formatMessage(commonMessages.retryButton)
                    }}</Button>
                </div>
            </div>

            <template
                v-else-if="ctx.items.value.length > 0 || (ctx.modpackItems?.value?.length ?? 0) > 0"
            >
                <ContentToolbar
                    v-model:search-query="searchQuery"
                    :searchable-item-count="searchableItemCount"
                    :content-type-label="ctx.contentTypeLabel.value"
                    :busy="ctx.isBusy.value"
                    :busy-tooltip="ctx.busyMessage?.value"
                    :disable-add-content="ctx.disableAddContent?.value"
                    :disable-add-content-tooltip="ctx.disableAddContentTooltip"
                    :refreshing="refreshing"
                    :view-dependencies="!!ctx.viewDependencies"
                    @browse="ctx.browse"
                    @refresh="handleRefresh"
                    @view-dependencies="ctx.viewDependencies?.()"
                >
                    <template #actions><slot name="toolbar-actions" /></template>
                </ContentToolbar>

                <ContentTypeFilter
                    v-model:selected="selectedTypeFilter"
                    :options="row1FilterOptions"
                    :total-count="totalCount"
                    :filter-counts="filterCounts"
                    @toggle="toggleTypeFilter"
                />

                <ContentCardTable
                    v-model:selected-ids="selectedIds"
                    :items="tableItems"
                    :show-selection="true"
                    :expanded-groups="expandedGroups"
                    @update:enabled="handleToggleEnabledById"
                    @delete="handleDeleteById"
                    @update="handleUpdateById"
                    @switch-version="handleSwitchVersionById"
                    @rollback="handleRollbackById"
                    @toggle-expand="toggleGroupExpand"
                    @visible-items="handleVisibleTableItems"
                >
                    <template #header-project>
                        <div class="flex min-w-0 flex-1 items-center gap-4">
                            <div class="flex shrink-0 items-center gap-4">
                                <button
                                    class="relative pb-1 text-base font-semibold transition-colors"
                                    :class="
                                        selectedStatusFilters.length === 0
                                            ? 'text-brand'
                                            : 'text-[var(--color-text-tertiary)] hover:text-[var(--color-text-default)]'
                                    "
                                    :aria-pressed="selectedStatusFilters.length === 0"
                                    @click="selectedStatusFilters = []"
                                >
                                    {{ formatMessage(commonMessages.allProjectType) }}
                                    <span
                                        v-if="selectedStatusFilters.length === 0"
                                        class="absolute bottom-0 left-0 right-0 h-0.5 rounded-full bg-brand"
                                    />
                                </button>
                                <button
                                    v-for="option in row2FilterOptions"
                                    :key="option.id"
                                    class="relative pb-1 text-base font-semibold transition-colors"
                                    :class="
                                        selectedStatusFilters.includes(option.id)
                                            ? 'text-brand'
                                            : 'text-[var(--color-text-tertiary)] hover:text-[var(--color-text-default)]'
                                    "
                                    :aria-pressed="selectedStatusFilters.includes(option.id)"
                                    @click="toggleStatusFilter(option.id)"
                                >
                                    {{ option.label }}
                                    <span class="ml-1 text-sm font-normal opacity-70">{{
                                        filterCounts[option.id] ?? 0
                                    }}</span>
                                    <span
                                        v-if="selectedStatusFilters.includes(option.id)"
                                        class="absolute bottom-0 left-0 right-0 h-0.5 rounded-full bg-brand"
                                    />
                                </button>
                            </div>
                            <ContentMetadataFilterBar
                                v-model:expanded="metadataFilterExpanded"
                                :categories="metadataFilterCategories"
                                :model-value="metadataFilterSelectedValues"
                                :filtering-keys="metadataFilteringKeys"
                                :active-filter-count="activeMetadataFilterCount"
                                @update:category="setCategorySelection"
                            />
                        </div>
                    </template>
                    <template #header-actions>
                        <ContentTableHeaderActions
                            :sort-mode="sortMode"
                            :sort-label="
                                formatMessage(messages.sortByLabel, {
                                    mode: sortLabels[sortMode](),
                                })
                            "
                            :sort-options="sortOptions"
                            :view-options-label="formatMessage(messages.viewOptions)"
                            :pinned="pinnedView"
                            :pin-tooltip="
                                formatMessage(pinnedView ? messages.unpinView : messages.pinView)
                            "
                            :reset-tooltip="formatMessage(messages.resetView)"
                            :has-bulk-update-support="hasBulkUpdateSupport"
                            :has-outdated-projects="hasOutdatedProjects"
                            :bulk-update-tooltip="
                                ctx.bulkUpdateAllDescription ??
                                ctx.bulkUpdateAllLabel ??
                                formatMessage(messages.updateAll)
                            "
                            :is-bulk-operating="isBulkOperating"
                            @select-sort="(mode) => (sortMode = mode)"
                            @toggle-pin="togglePinnedView"
                            @reset-view="resetView"
                            @update-all="promptUpdateAll"
                        />
                    </template>
                    <template #empty>
                        <span>{{ formatMessage(messages.noContentFound) }}</span>
                    </template>
                </ContentCardTable>
            </template>

            <ContentEmptyState
                v-else
                :content-type-label="ctx.contentTypeLabel.value"
                :busy="ctx.isBusy.value"
                :busy-tooltip="ctx.busyMessage?.value"
                :refreshing="refreshing"
                :disable-add-content="ctx.disableAddContent?.value"
                :disable-add-content-tooltip="ctx.disableAddContentTooltip"
                @browse="ctx.browse"
                @refresh="handleRefresh"
            >
                <template #actions><slot name="toolbar-actions" /></template>
            </ContentEmptyState>
        </template>

        <ContentSelectionBar
            :selected-items="selectedItems"
            :content-type-label="ctx.contentTypeLabel.value"
            :is-busy="ctx.isBusy.value"
            :busy-tooltip="ctx.busyMessage?.value"
            :is-bulk-operating="isBulkOperating"
            :bulk-operation="bulkOperation"
            :bulk-progress="bulkProgress"
            :bulk-total="bulkTotal"
            :bulk-waiting="bulkWaiting"
            :bulk-status-message="bulkStatusMessage"
            :bulk-item-count="bulkItemCount"
            :aria-label="formatMessage(commonMessages.selectionActionsLabel)"
            :get-item-id="getItemId"
            @clear="clearSelection"
            @enable="bulkEnable"
            @disable="bulkDisable"
        >
            <template #actions>
                <Button
                    v-if="hasBulkUpdateSupport && selectedItems.some((m) => hasContentUpdate(m))"
                    v-tooltip="formatMessage(commonMessages.updateButton)"
                    type="quiet"
                    color="green"
                    interaction="filled"
                    @click="promptUpdateSelected"
                >
                    <DownloadIcon />
                    <span class="bar-label">{{ formatMessage(commonMessages.updateButton) }}</span>
                </Button>

                <PopoutMenu
                    v-if="ctx.shareItems"
                    placement="top-end"
                    :container="previewLayer ?? undefined"
                >
                    <Button type="quiet">
                        <ShareIcon />
                        <span class="bar-label">{{ formatMessage(messages.share) }}</span>
                        <DropdownIcon />
                    </Button>
                    <template #menu="{ hide }">
                        <div class="flex flex-col gap-1 p-1">
                            <Button
                                type="quiet"
                                class="w-full !justify-start"
                                @click="shareSelectedItems('names', hide)"
                            >
                                <TextCursorInputIcon />
                                {{ formatMessage(messages.shareProjectNames) }}
                            </Button>
                            <Button
                                type="quiet"
                                class="w-full !justify-start"
                                @click="shareSelectedItems('file-names', hide)"
                            >
                                <FileIcon />
                                {{ formatMessage(messages.shareFileNames) }}
                            </Button>
                            <Button
                                type="quiet"
                                class="w-full !justify-start"
                                @click="shareSelectedItems('urls', hide)"
                            >
                                <LinkIcon />
                                {{ formatMessage(messages.shareProjectLinks) }}
                            </Button>
                            <Button
                                type="quiet"
                                class="w-full !justify-start"
                                @click="shareSelectedItems('markdown', hide)"
                            >
                                <CodeIcon />
                                {{ formatMessage(messages.shareMarkdownLinks) }}
                            </Button>
                        </div>
                    </template>
                </PopoutMenu>
            </template>

            <template #actions-end>
                <div class="mx-1 h-6 w-px bg-surface-5" />

                <Button
                    v-tooltip="formatMessage(commonMessages.deleteLabel)"
                    type="quiet"
                    color="red"
                    interaction="filled"
                    @click="showBulkDeleteModal"
                >
                    <TrashIcon />
                    <span class="bar-label">{{ formatMessage(commonMessages.deleteLabel) }}</span>
                </Button>
            </template>
        </ContentSelectionBar>

        <ConfirmDeletionModal
            ref="confirmDeletionModal"
            :count="pendingDeletionItems.length"
            :item-type="ctx.contentTypeLabel.value"
            :variant="ctx.deletionContext ?? 'instance'"
            :backup-tip="
                pendingDeletionItems.map((i) => i.project?.title ?? i.file_name).join(', ')
            "
            :action-disabled="ctx.isBusy.value"
            :action-disabled-tooltip="ctx.busyMessage?.value ?? undefined"
            :symlink-target="ctx.symlinkTarget?.value"
            :action="confirmDelete"
        />
        <ContentDependencyWarningModal
            ref="contentDependencyWarningModal"
            :items="pendingDependencyWarningItems"
            :dependents="pendingDependencyWarningDependents"
            :item-type="ctx.contentTypeLabel.value"
            :variant="ctx.deletionContext ?? 'instance'"
            :backup-tip="
                pendingDeletionItems.map((i) => i.project?.title ?? i.file_name).join(', ')
            "
            :action-disabled="ctx.isBusy.value"
            :action-disabled-tooltip="ctx.busyMessage?.value ?? undefined"
            :symlink-target="ctx.symlinkTarget?.value"
            :action="confirmDependencyWarningDelete"
        />
        <ConfirmBulkUpdateModal
            v-if="hasBulkUpdateSupport"
            ref="confirmBulkUpdateModal"
            :count="
                pendingBulkUpdateItems.length +
                (pendingBulkUpdateAll &&
                ctx.bulkUpdateIncludesModpack !== false &&
                ctx.modpack.value?.hasUpdate
                    ? 1
                    : 0)
            "
            :action-label="ctx.bulkUpdateAllLabel"
            :scope-description="ctx.bulkUpdateAllDescription"
            :server="ctx.deletionContext === 'server'"
            :action-disabled="ctx.isBusy.value"
            :action-disabled-tooltip="ctx.busyMessage?.value ?? undefined"
            :symlink-target="ctx.symlinkTarget?.value"
            @update="confirmBulkUpdate"
        />
        <ConfirmUnlinkModal
            v-if="ctx.unlinkModpack"
            ref="confirmUnlinkModal"
            :server="ctx.deletionContext === 'server'"
            :backup-tip="ctx.modpack.value?.project.title"
            :action-disabled="ctx.isBusy.value"
            :action-disabled-tooltip="ctx.busyMessage?.value ?? undefined"
            :action="ctx.unlinkModpack"
        />

        <slot name="modals" />

        <Transition name="scroll-to-top">
            <button
                v-if="showScrollToTop"
                class="scroll-to-top-btn"
                :aria-label="formatMessage(messages.scrollToTop)"
                @click="scrollToTop"
            >
                <ChevronUpIcon class="size-5" />
            </button>
        </Transition>
    </div>
</template>

<style scoped>
.scroll-to-top-btn {
    @apply fixed bottom-10 left-24 z-50 flex items-center justify-center rounded-full bg-brand p-3 text-brand-inverted shadow-lg transition-all duration-200 hover:shadow-xl hover:brightness-110 active:scale-95;
}

.scroll-to-top-enter-active,
.scroll-to-top-leave-active {
    transition:
        opacity 0.2s ease,
        transform 0.2s ease;
}

.scroll-to-top-enter-from,
.scroll-to-top-leave-to {
    opacity: 0;
    transform: translateY(10px);
}
</style>

<script setup lang="ts">
import { DownloadIcon, ExternalIcon, PlusIcon } from '@modrinth/assets'
import {
    type BrowseManagerContext,
    BrowsePageLayout,
    Button,
    defineMessages,
    type FilterType,
    type FilterValue,
    injectBrowseManager,
    provideBrowseManager,
    type SortType,
    useVIntl,
} from '@modrinth/ui'
import { openUrl } from '@tauri-apps/plugin-opener'
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'

import {
    applySchematicDownload,
    getSchematicCapabilities,
    getSchematicImage,
    listAllSchematics,
    listSchematics,
    SCHEMATIC_SOURCE_IDS,
    SCHEMATIC_SOURCE_LABELS,
    type SchematicItem,
    type SchematicSourceId,
    type SchematicSourceStatus,
} from '@/helpers/schematic-browser'
import { get_game_versions } from '@/helpers/tags'

import SchematicDetailModal from './SchematicDetailModal.vue'

const props = defineProps<{
    /** Target instance for downloads; absent means browsing only. */
    instanceId?: string | null
    /**
     * The selected instance's Minecraft version. Schematics survive a version
     * mismatch far better than mods do (an older build opens fine on a newer
     * game; only new blocks do not exist), so this is never used to hide rows —
     * the download path only notes it.
     */
    instanceGameVersion?: string | null
}>()

const { formatMessage } = useVIntl()

const messages = defineMessages({
    loading: { id: 'app.schematic.loading', defaultMessage: 'Loading…' },
    noResults: { id: 'app.schematic.no-results', defaultMessage: 'No schematics found.' },
    sourceLabel: { id: 'app.schematic.filter.source', defaultMessage: 'Source' },
    allSources: { id: 'app.schematic.filter.all-sources', defaultMessage: 'All sources' },
    formatLabel: { id: 'app.schematic.filter.format', defaultMessage: 'Format' },
    versionLabel: { id: 'app.schematic.filter.game-version', defaultMessage: 'Game version' },
    sortDownloads: { id: 'app.schematic.sort.downloads', defaultMessage: 'Downloads' },
    sortNewest: { id: 'app.schematic.sort.newest', defaultMessage: 'Newest' },
    download: { id: 'app.schematic.detail.download', defaultMessage: 'Download' },
    openSource: { id: 'app.schematic.card.open-source', defaultMessage: 'Open source' },
    noInstance: {
        id: 'app.schematic.detail.no-instance',
        defaultMessage: 'Choose an instance to download into.',
    },
    cached: { id: 'app.schematic.cached', defaultMessage: 'cached' },
    savedTo: { id: 'app.schematic.detail.downloaded', defaultMessage: 'Saved to {path}' },
    selectForDownload: {
        id: 'app.schematic.card.select-for-download',
        defaultMessage: 'Select for download',
    },
    downloadSelected: {
        id: 'app.schematic.batch.download-count',
        defaultMessage: 'Download {count, plural, one {# schematic} other {# schematics}}',
    },
    versionNewer: {
        id: 'app.schematic.card.version-newer',
        defaultMessage: 'Made for {version}; newer blocks will not exist in this instance',
    },
    sourceFailed: {
        id: 'app.schematic.source-failed',
        defaultMessage: '{count, plural, one {# source failed} other {# sources failed}}',
    },
    batchFailed: {
        id: 'app.schematic.batch-failed',
        defaultMessage: 'Failed to download: {names}',
    },
})

/** Fallback page size; the layout's own selector wins once it has a value. */
const DEFAULT_PAGE_SIZE = 20

/**
 * The mods / modpacks tabs render `BrowsePageLayout` and let it ask the browse
 * manager for everything it draws. This component does the same: it takes the
 * manager the page already provided and overrides only the fields whose backing
 * store is the schematic IPC instead of the Modrinth search, so the toolbar,
 * sidebar, card list, pagination and empty/error states are the same code path.
 */
const base = injectBrowseManager()

const query = ref('')
const currentPage = ref(1)
const items = ref<SchematicItem[]>([])
const loading = ref(false)
const searchError = ref<string | null>(null)
const totalHits = ref(0)
const cached = ref(false)
const sourceStatus = ref<SchematicSourceStatus | null>(null)
/** Per-source failures reported alongside a merged "all sources" page. */
const sourceFailures = ref<{ sourceId: string; message: string }[]>([])
const currentFilters = ref<FilterValue[]>([])
const toggledGroups = ref<string[]>([])
const overridden = ref<string[]>([])
const sortType = ref<SortType>({
    name: 'downloads',
    display: formatMessage(messages.sortDownloads),
    method: 'downloads',
} as SortType)

const selectedItem = ref<SchematicItem | null>(null)
const detailVisible = ref(false)
const downloadingId = ref<string | null>(null)
const downloadedNotice = ref<string | null>(null)
/** Items ticked for the batch bar; ids are provider-qualified. */
const selectedIds = ref<string[]>([])
const batchRunning = ref(false)
const batchFailed = ref<string[]>([])
const gameVersionOptions = ref<
    { id: string; formatted_name: string; method: 'or'; value: string }[]
>([])

const selectedItems = computed(() =>
    items.value.filter((item) => selectedIds.value.includes(item.id)),
)

function toggleSelected(item: SchematicItem) {
    const index = selectedIds.value.indexOf(item.id)
    if (index === -1) selectedIds.value = [...selectedIds.value, item.id]
    else selectedIds.value = selectedIds.value.filter((id) => id !== item.id)
}

/**
 * Whether a download error is worth another attempt.
 *
 * The backend reports `"{code}: {message}"`, where codes like `http_403`,
 * `unsupported` and `not_found` are permanent; retrying those only wastes the
 * rate-limit budget. Network, timeout and 5xx stay retryable.
 */
function isRetryableDownloadError(error: unknown): boolean {
    const text = String(error)
    if (/http_4\d\d/.test(text)) return false
    if (/unsupported|not_found|parse/.test(text)) return false
    return true
}

/**
 * Downloads one item, retrying a transient failure three times before giving up.
 *
 * The sources rate-limit aggressively, so the retries are spaced by a short
 * backoff rather than fired immediately. The target instance is captured by the
 * caller: reading `props.instanceId` per attempt would let a mid-batch instance
 * switch send later items somewhere the user did not choose.
 */
async function downloadWithRetry(
    instanceId: string,
    item: SchematicItem,
    attempts = 3,
): Promise<boolean> {
    for (let attempt = 1; attempt <= attempts; attempt++) {
        try {
            await applySchematicDownload(instanceId, item.sourceId, item.id)
            return true
        } catch (error) {
            if (attempt === attempts || !isRetryableDownloadError(error)) {
                batchFailed.value = [...batchFailed.value, item.title]
                return false
            }
            await new Promise((resolve) => setTimeout(resolve, attempt * 800))
        }
    }
    return false
}

/**
 * Batch download. One transfer per source at a time: the sites rate-limit by
 * address, so running three at once against the same host is what triggers a
 * block in the first place.
 */
async function downloadSelected() {
    if (!props.instanceId || batchRunning.value || !selectedItems.value.length) return
    const instanceId = props.instanceId
    batchRunning.value = true
    batchFailed.value = []
    const bySource = new Map<string, SchematicItem[]>()
    for (const item of selectedItems.value) {
        const list = bySource.get(item.sourceId) ?? []
        list.push(item)
        bySource.set(item.sourceId, list)
    }
    const queues = [...bySource.values()].map(async (queue) => {
        for (const item of queue) await downloadWithRetry(instanceId, item)
    })
    await Promise.all(queues)
    batchRunning.value = false
    selectedIds.value = []
}

/**
 * Whether a row only names game versions newer than the instance.
 *
 * Used for the download tooltip, never to hide the row: the file still opens.
 */
function isNewerThanInstance(item: SchematicItem): boolean {
    const instanceVersion = props.instanceGameVersion
    if (!instanceVersion || !item.versions.length) return false
    return !item.versions.includes(instanceVersion)
}

async function downloadOne(item: SchematicItem) {
    if (!props.instanceId || downloadingId.value) return
    downloadingId.value = item.id
    try {
        const path = await applySchematicDownload(props.instanceId, item.sourceId, item.id)
        // Report where it landed the same way the detail modal does.
        searchError.value = null
        downloadedNotice.value = path
    } catch (error) {
        searchError.value = String(error)
    } finally {
        downloadingId.value = null
    }
}

function openExternal(url: string) {
    void openUrl(url)
}

let requestSeq = 0
let isUnmounted = false

function sourceFilterOptions() {
    // "All sources" first, then each site, matching how the mods tab lists its
    // providers.
    return [
        { id: 'all', formatted_name: formatMessage(messages.allSources) },
        ...SCHEMATIC_SOURCE_IDS.map((id) => ({
            id,
            formatted_name: SCHEMATIC_SOURCE_LABELS[id],
        })),
    ].map((option) => ({
        id: option.id,
        formatted_name: option.formatted_name,
        method: 'or' as const,
        value: option.id,
    }))
}

function formatFilterOptions() {
    return [
        { value: 'litematic' },
        { value: 'schem' },
        { value: 'schematic' },
        { value: 'nbt' },
    ].map(({ value }) => ({
        id: value,
        formatted_name: value.toUpperCase(),
        method: 'or' as const,
        value,
    }))
}

const filters = computed<FilterType[]>(() => [
    {
        id: 'schematic_source',
        formatted_name: formatMessage(messages.sourceLabel),
        options: sourceFilterOptions(),
        supported_project_types: [],
        query_param: 'source',
        supports_negative_filter: false,
        searchable: false,
        display: 'all',
        ordering: 1,
    },
    {
        id: 'schematic_format',
        formatted_name: formatMessage(messages.formatLabel),
        options: formatFilterOptions(),
        supported_project_types: [],
        query_param: 'format',
        supports_negative_filter: false,
        searchable: false,
        display: 'all',
        ordering: 2,
    },
    {
        id: 'game_version',
        formatted_name: formatMessage(messages.versionLabel),
        options: gameVersionOptions.value,
        supported_project_types: [],
        query_param: 'gv',
        supports_negative_filter: false,
        searchable: true,
        display: 'all',
        ordering: 3,
    },
])

/** Read one filter's selected values out of the shared filter state. */
function chosenValues(filterId: string): string[] {
    return currentFilters.value
        .filter((entry) => entry.type === filterId && !entry.negative)
        .map((entry) => entry.option)
}

/** `'all'` asks the backend to merge every source into one ordered page. */
type SchematicSourceSelection = SchematicSourceId | 'all'

const chosenSource = computed<SchematicSourceSelection>(() => {
    const picked = chosenValues('schematic_source')[0]
    if (picked === 'all') return 'all'
    return (picked as SchematicSourceId | undefined) ?? 'all'
})

const chosenFormats = computed(() => chosenValues('schematic_format'))
const chosenVersions = computed(() => chosenValues('game_version'))

async function loadGameVersions() {
    try {
        const versions = await get_game_versions()
        // The filter lists the release line only; snapshots would be hundreds
        // of options and no source we read tags them.
        gameVersionOptions.value = versions
            .filter((version) => version.version_type === 'release')
            .map((version) => ({
                id: version.version,
                formatted_name: version.version,
                method: 'or' as const,
                value: version.version,
            }))
    } catch {
        // A failed lookup only costs the version filter, not the listing.
    }
}

/** Page size the layout's selector currently asks for. */
const pageSize = computed(() => base.maxResults?.value ?? DEFAULT_PAGE_SIZE)

async function loadPage(targetPage: number) {
    requestSeq += 1
    const seq = requestSeq
    loading.value = true
    searchError.value = null
    try {
        const args = {
            page: targetPage,
            pageSize: pageSize.value,
            query: query.value || undefined,
            versions: chosenVersions.value.length ? chosenVersions.value : undefined,
        }
        // "All sources" is a backend call: merging three pages here would order
        // them differently on every render.
        const payload =
            chosenSource.value === 'all'
                ? await listAllSchematics(args)
                : await listSchematics({ ...args, sourceId: chosenSource.value })
        // A newer query owns the state now; this response must not write to it,
        // and neither must its `catch` / `finally` below.
        if (seq !== requestSeq || isUnmounted) return
        // The payload is camelCase all the way down (`sourceStatus.cached`), so
        // reading a top-level `cached` or snake_case `source_status` silently
        // yields undefined and hides the cache badge.
        cached.value = payload.sourceStatus?.cached ?? false
        // Page one replaces the list; later pages append to it.
        if (targetPage === 1) items.value = [...payload.items]
        else items.value.push(...payload.items)
        totalHits.value = payload.total?.value ?? items.value.length
        sourceStatus.value = payload.sourceStatus ?? null
        sourceFailures.value = payload.sourceFailures ?? []
        currentPage.value = targetPage
        void prefetchThumbnails(payload.items)
    } catch (error) {
        if (seq !== requestSeq || isUnmounted) return
        searchError.value = String(error)
    } finally {
        if (seq === requestSeq && !isUnmounted) loading.value = false
    }
}

/** First page, discarding what is loaded: a new query or filter starts over. */
function reload() {
    currentPage.value = 1
    void loadPage(1)
}

/**
 * Resolve thumbnails the webview cannot load by itself.
 *
 * The card is given the source URL first, so a site that does serve its images
 * costs nothing; the ones that reject a foreign origin are replaced with a
 * `data:` URI once the proxy answers.
 */
const thumbOverrides = ref<Record<string, string>>({})

async function prefetchThumbnails(list: SchematicItem[]) {
    for (const item of list) {
        const url = item.images[0]
        if (!url || thumbOverrides.value[item.id]) continue
        try {
            const dataUri = await getSchematicImage(url, item.detailUrl)
            thumbOverrides.value = { ...thumbOverrides.value, [item.id]: dataUri }
        } catch {
            // A missing thumbnail is cosmetic; the card falls back to the format
            // placeholder.
        }
    }
}

/**
 * The layout draws the list from `projectHits`, so each schematic is presented
 * in the shape `ProjectCard` expects. Nothing outside this mapping is fake: the
 * card shows the source as the author and the format as the first tag.
 */
const projectHits = computed(() =>
    items.value
        // An item whose format we could not read stays visible: two of the three
        // sites leave it `unknown`, and hiding those rows would make a format
        // filter silently drop an entire source.
        .filter(
            (item) =>
                !chosenFormats.value.length ||
                item.format === 'unknown' ||
                chosenFormats.value.includes(item.format),
        )
        .filter(
            (item) =>
                !chosenVersions.value.length ||
                item.versions.length === 0 ||
                item.versions.some((version) => chosenVersions.value.includes(version)),
        )
        .map((item) => ({
            project_id: item.id,
            project_type: 'schematic',
            provider: 'modrinth' as const,
            slug: item.id,
            title: item.title,
            description: item.description ?? '',
            author: SCHEMATIC_SOURCE_LABELS[item.sourceId] ?? item.sourceId,
            author_url: item.detailUrl,
            organization: null,
            organization_id: null,
            icon_url: thumbOverrides.value[item.id] ?? item.images[0] ?? '',
            categories: item.versions,
            display_categories: [item.format.toUpperCase()],
            versions: item.versions,
            downloads: item.downloads ?? 0,
            follows: 0,
            date_created: '',
            date_modified: '',
            latest_version: '',
            license: '',
            gallery: item.images,
            featured_gallery: null,
            color: null,
            installed: false,
            installing: false,
        })),
)

const pageCount = computed(() =>
    Math.max(1, Math.ceil(totalHits.value / Math.max(1, pageSize.value))),
)

function setPage(page: number) {
    // Request the page the pagination asked for; `reload()` resets to page one.
    currentPage.value = Math.max(1, page)
    void loadPage(currentPage.value)
}

const manager: BrowseManagerContext = {
    ...base,
    projectType: computed(() => 'schematic') as never,
    query,
    filters,
    currentFilters,
    toggledGroups,
    overriddenProvidedFilterTypes: overridden,
    loading,
    filtersReady: computed(() => true),
    projectHits: projectHits as never,
    totalHits,
    pageCount,
    currentPage,
    setPage,
    searchError,
    effectiveSortTypes: computed(() => [sortType.value]) as never,
    effectiveCurrentSortType: sortType as never,
    // The layout hands this to `ProjectCard` as its link; returning a callback
    // makes the card open the detail modal instead of navigating.
    getProjectLink: (result: { project_id: string }) => () => {
        const item = items.value.find((candidate) => candidate.id === result.project_id)
        if (item) openDetail(item)
    },
    getServerProjectLink: () => '#',
    serverHits: computed(() => []) as never,
    isServerType: computed(() => false) as never,
    // Same affordance the mods tab puts on its cards, with download standing in
    // for install: only a `direct` delivery can be applied, the other kinds are
    // opened on their source page.
    getCardActions: (result: { project_id: string }) => {
        const item = items.value.find((candidate) => candidate.id === result.project_id)
        if (!item) return []
        const pick = {
            key: 'select',
            label: formatMessage(messages.selectForDownload),
            compactLabel: formatMessage(messages.selectForDownload),
            icon: PlusIcon,
            type: 'quiet' as const,
            onClick: () => toggleSelected(item),
        }
        if (item.download.delivery !== 'direct') {
            return [
                {
                    key: 'open-source',
                    label: formatMessage(messages.openSource),
                    compactLabel: formatMessage(messages.openSource),
                    icon: ExternalIcon,
                    type: 'outlined' as const,
                    onClick: () => openExternal(item.detailUrl),
                },
            ]
        }
        return [
            {
                key: 'download',
                label: formatMessage(messages.download),
                compactLabel: formatMessage(messages.download),
                icon: DownloadIcon,
                color: 'green' as const,
                disabled: !props.instanceId || downloadingId.value === item.id,
                tooltip: !props.instanceId
                    ? formatMessage(messages.noInstance)
                    : isNewerThanInstance(item)
                      ? formatMessage(messages.versionNewer, {
                            version: item.versions[0] ?? '',
                        })
                      : undefined,
                onClick: () => void downloadOne(item),
            },
            pick,
        ]
    },
    /**
     * A download-shaped install context: the floating bar the mods tab shows for
     * a selection is the same component, so the projection reuses it by supplying
     * its own selection and its own "install" action.
     */
    installContext: computed(
        () =>
            ({
                showInstallHeader: false,
                name: 'schematic',
                loader: '',
                gameVersion: '',
                backUrl: '',
                backLabel: '',
                heading: '',
                selectedProjects: selectedItems.value.map((item) => ({
                    id: item.id,
                    name: item.title,
                    iconUrl: item.images[0] ?? null,
                })),
                installButtonLabel: formatMessage(messages.downloadSelected, {
                    count: selectedItems.value.length,
                }),
                installSelected: () => void downloadSelected(),
                clearSelected: () => {
                    selectedIds.value = []
                },
            }) as never,
    ),
    refreshSearch: () => reload(),
    clearSearch: () => {
        query.value = ''
        reload()
    },
}

provideBrowseManager(manager)

watch(query, () => reload())
watch(currentFilters, () => reload(), { deep: true })

onMounted(async () => {
    void loadGameVersions()
    await getSchematicCapabilities().catch(() => [])
    reload()
})

onUnmounted(() => {
    isUnmounted = true
})

function openDetail(item: SchematicItem) {
    selectedItem.value = item
    detailVisible.value = true
}
</script>

<template>
    <BrowsePageLayout>
        <template #search-bar-actions>
            <div
                data-onboarding-id="schematic-source-filter"
                class="flex flex-wrap items-center gap-2"
            >
                <Button
                    v-for="id in SCHEMATIC_SOURCE_IDS"
                    :key="id"
                    size="sm"
                    :type="chosenSource === id ? 'colored' : 'quiet'"
                    @click="
                        currentFilters = [
                            ...currentFilters.filter((f) => f.type !== 'schematic_source'),
                            { type: 'schematic_source', option: id },
                        ]
                    "
                >
                    {{ SCHEMATIC_SOURCE_LABELS[id] }}
                </Button>
            </div>
        </template>

        <template #above-results>
            <div
                data-onboarding-id="schematic-download-action"
                class="mb-3 flex flex-wrap items-center gap-2"
            >
                <span v-if="cached" class="rounded bg-surface-4 px-2 py-1 text-xs">{{
                    formatMessage(messages.cached)
                }}</span>
                <span
                    v-if="sourceStatus && sourceStatus.state !== 'ok'"
                    class="text-xs"
                    :title="sourceStatus.message"
                >
                    {{ sourceStatus.message ?? sourceStatus.state }}
                </span>
                <span
                    v-if="sourceFailures.length"
                    class="text-xs text-[var(--color-orange)]"
                    :title="sourceFailures.map((f) => f.message).join('\n')"
                >
                    {{ formatMessage(messages.sourceFailed, { count: sourceFailures.length }) }}
                </span>
                <span v-if="downloadedNotice" class="truncate text-xs">
                    {{ formatMessage(messages.savedTo, { path: downloadedNotice }) }}
                </span>
                <span v-if="batchFailed.length" class="truncate text-xs text-[var(--color-red)]">
                    {{ formatMessage(messages.batchFailed, { names: batchFailed.join(', ') }) }}
                </span>
            </div>
        </template>
    </BrowsePageLayout>

    <SchematicDetailModal
        v-model:visible="detailVisible"
        :item="selectedItem"
        :instance-id="props.instanceId ?? null"
        @close="detailVisible = false"
    />
</template>

<script setup lang="ts">
import { DownloadIcon, ExternalIcon } from '@modrinth/assets'
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
}>()

const { formatMessage } = useVIntl()

const messages = defineMessages({
    loading: { id: 'app.schematic.loading', defaultMessage: 'Loading…' },
    noResults: { id: 'app.schematic.no-results', defaultMessage: 'No schematics found.' },
    sourceLabel: { id: 'app.schematic.filter.source', defaultMessage: 'Source' },
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
})

const PAGE_SIZE = 20

/**
 * The mods / modpacks tabs render `BrowsePageLayout` and let it ask the browse
 * manager for everything it draws. This component does the same: it takes the
 * manager the page already provided and overrides only the fields whose backing
 * store is the schematic IPC instead of the Modrinth search, so the toolbar,
 * sidebar, card list, pagination and empty/error states are the same code path.
 */
const base = injectBrowseManager()

const source = ref<SchematicSourceId>('reden')
const query = ref('')
const currentPage = ref(1)
const items = ref<SchematicItem[]>([])
const loading = ref(false)
const searchError = ref<string | null>(null)
const totalHits = ref(0)
const cached = ref(false)
const sourceStatus = ref<SchematicSourceStatus | null>(null)
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
const gameVersionOptions = ref<
    { id: string; formatted_name: string; method: 'or'; value: string }[]
>([])

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
    return SCHEMATIC_SOURCE_IDS.map((id) => ({
        id,
        formatted_name: SCHEMATIC_SOURCE_LABELS[id],
        method: 'or' as const,
        value: id,
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

const chosenSource = computed<SchematicSourceId>(() => {
    const picked = chosenValues('schematic_source')[0]
    return (picked as SchematicSourceId | undefined) ?? source.value
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

async function load(reset: boolean) {
    requestSeq += 1
    const seq = requestSeq
    loading.value = true
    searchError.value = null
    const targetPage = reset ? 1 : currentPage.value + 1
    try {
        const payload = await listSchematics({
            sourceId: chosenSource.value,
            page: targetPage,
            pageSize: PAGE_SIZE,
            query: query.value || undefined,
        })
        if (seq !== requestSeq || isUnmounted) return
        cached.value = payload.cached
        if (reset) {
            items.value = []
            currentPage.value = 1
        }
        items.value.push(...payload.items)
        totalHits.value = payload.total?.value ?? payload.items.length
        sourceStatus.value = payload.source_status ?? null
        currentPage.value = targetPage
    } catch (error) {
        if (!isUnmounted) searchError.value = String(error)
    } finally {
        if (!isUnmounted) loading.value = false
    }
}

/**
 * The layout draws the list from `projectHits`, so each schematic is presented
 * in the shape `ProjectCard` expects. Nothing outside this mapping is fake: the
 * card shows the source as the author and the format as the first tag.
 */
const projectHits = computed(() =>
    items.value
        .filter((item) => !chosenFormats.value.length || chosenFormats.value.includes(item.format))
        .filter(
            (item) =>
                !chosenVersions.value.length ||
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
            icon_url: item.images[0] ?? '',
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

const pageCount = computed(() => Math.max(1, Math.ceil(totalHits.value / PAGE_SIZE)))

function setPage(page: number) {
    currentPage.value = Math.max(1, page)
    void load(true)
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
    installContext: ref(null) as never,
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
                type: 'standard' as const,
                disabled: !props.instanceId || downloadingId.value === item.id,
                tooltip: props.instanceId ? undefined : formatMessage(messages.noInstance),
                onClick: () => void downloadOne(item),
            },
        ]
    },
    refreshSearch: () => void load(true),
    clearSearch: () => {
        query.value = ''
        void load(true)
    },
}

provideBrowseManager(manager)

watch(query, () => void load(true))
watch(currentFilters, () => void load(true), { deep: true })

onMounted(async () => {
    void loadGameVersions()
    await getSchematicCapabilities().catch(() => [])
    await load(true)
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
            <div class="flex flex-wrap items-center gap-2">
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
            <div v-if="sourceStatus && sourceStatus.state !== 'ok'" class="mb-3 text-xs">
                {{ sourceStatus.message ?? sourceStatus.state }}
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

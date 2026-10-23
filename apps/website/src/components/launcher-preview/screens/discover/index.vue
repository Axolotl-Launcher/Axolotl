<script setup lang="ts">
import ModrinthIcon from '@modrinth/assets/branding/logo.svg?component'
import BookmarkIcon from '@modrinth/assets/icons/bookmark.svg?component'
import BookmarkFilledIcon from '@modrinth/assets/icons/bookmark-filled.svg?component'
import CheckIcon from '@modrinth/assets/icons/check.svg?component'
import ChevronDownIcon from '@modrinth/assets/icons/chevron-down.svg?component'
import FileArchiveIcon from '@modrinth/assets/icons/file-archive.svg?component'
import GenericListIcon from '@modrinth/assets/icons/generic-list.svg?component'
import GlobeIcon from '@modrinth/assets/icons/globe.svg?component'
import GridIcon from '@modrinth/assets/icons/grid.svg?component'
import LanguagesIcon from '@modrinth/assets/icons/languages.svg?component'
import ListIcon from '@modrinth/assets/icons/list.svg?component'
import PlusIcon from '@modrinth/assets/icons/plus.svg?component'
import SpinnerIcon from '@modrinth/assets/icons/spinner.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import EmptyState from '@modrinth/ui/src/components/base/EmptyState.vue'
import PopoutMenu from '@modrinth/ui/src/components/base/PopoutMenu.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import {
    getTargetInstallPreferences,
    usesTargetGameVersion,
} from '@modrinth/ui/src/layouts/shared/browse-tab/composables/install-logic.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import type { FilterValue, Tags } from '@modrinth/ui/src/utils/search.ts'
import { computed, nextTick, onMounted, onUnmounted, ref, shallowRef, watch } from 'vue'
import { type LocationQuery, onBeforeRouteLeave, useRoute, useRouter } from 'vue-router'

import { modrinthSearchText, translateSearchHitTitles } from '../../lib/content-search'
import { usePreview } from '../../lib/state'
import type { GameInstance } from '../../lib/types'
import { injectPreviewWindow } from '../../lib/window'
import InstanceIcon from '../../shell/InstanceIcon.vue'
import { type BrowseInstallContext, type CardAction, provideBrowseContext } from './browse-context'
import {
    type BrowseSearchRequest,
    type BrowseSearchResponse,
    useBrowseSearch,
} from './browse-search'
import BrowseContextMenu from './components/BrowseContextMenu.vue'
import BrowseInstanceSelector from './components/BrowseInstanceSelector.vue'
import BrowseLayout from './components/BrowseLayout.vue'
import BrowseSidebar from './components/BrowseSidebar.vue'
import InstallModals from './components/InstallModals.vue'
import { createContentInstall, displayInstanceIcon, provideContentInstall } from './install'
import { messages } from './messages'
import { runResourceSearch, type SearchHit, SourceFailure, sourceName } from './search-engine'
import {
    type BrowseContentSource,
    type BrowseDisplayMode,
    type ContentSelectionType,
    isFavoriteContentType,
    isSnapshotType,
    makeContentSelectionKey,
    SNAPSHOT_TYPES,
    useDiscover,
} from './state'

/**
 * Discover (apps/app-frontend/src/pages/Browse.vue with the shared browse-tab layout and
 * sidebar). Results come from the Modrinth snapshot through the launcher's resource search
 * rules; the window reproduces a build without a CurseForge key, so the sources are
 * "All sources" and Modrinth and the Maps tab reports that maps are unavailable.
 */
const { formatMessage, locale } = useVIntl()
const { addNotification, handleError } = injectNotificationManager()
const route = useRoute()
const router = useRouter()
const preview = usePreview()
const discover = useDiscover()
const selection = discover.selection
const { layer, root } = injectPreviewWindow()

const contentInstall = createContentInstall({ preview, discover, router })
provideContentInstall(contentInstall)

const WORLD = 'world'
const projectType = ref<string>(String(route.params.projectType))
const isWorldMapBrowse = computed(() => projectType.value === WORLD)
const isWorldMapContext = computed(() => route.query.from === 'world-maps')
const isFromWorlds = computed(() => route.query.from === 'worlds')

function rememberBrowseContentProjectType(type: string) {
    if (
        !route.query.i &&
        !route.query.sid &&
        !route.query.wid &&
        !route.query.from &&
        ['modpack', 'mod', 'resourcepack', 'datapack', 'shader', WORLD].includes(type)
    ) {
        discover.lastProjectType.value = type
    }
}
watch(projectType, rememberBrowseContentProjectType, { immediate: true })

// Content source: without a CurseForge key the app defaults to Modrinth.
function resolveInitialContentSource(): BrowseContentSource {
    const remembered = discover.contentSource.value
    if (projectType.value === WORLD) return 'modrinth'
    if (projectType.value === 'mod' && route.query.source === 'mcarchive') return 'mcarchive'
    if (route.query.source === 'modrinth') return 'modrinth'
    if (remembered === 'modrinth') return 'modrinth'
    if (remembered === 'mcarchive' && projectType.value === 'mod') return 'mcarchive'
    return 'modrinth'
}
const contentSource = ref<BrowseContentSource>(resolveInitialContentSource())

// Tags (`get_categories`, `get_loaders`, `get_game_versions`).
const tagData = isWorldMapBrowse.value ? null : await discover.ensureTags()
const baseTags = shallowRef<Tags>({
    gameVersions: (tagData?.gameVersions ?? []) as Tags['gameVersions'],
    loaders: (tagData?.loaders ?? []) as Tags['loaders'],
    categories: (tagData?.categories ?? []) as Tags['categories'],
})
watch(isWorldMapBrowse, async (worldMaps) => {
    if (worldMaps || baseTags.value.gameVersions.length) return
    const data = await discover.ensureTags()
    baseTags.value = {
        gameVersions: data.gameVersions as Tags['gameVersions'],
        loaders: data.loaders as Tags['loaders'],
        categories: data.categories as Tags['categories'],
    }
})
const tags = computed<Tags>(() => ({
    gameVersions: contentSource.value === 'mcarchive' ? [] : baseTags.value.gameVersions,
    loaders: contentSource.value === 'mcarchive' ? [] : baseTags.value.loaders,
    categories: contentSource.value === 'mcarchive' ? [] : baseTags.value.categories,
}))

// Instance context ---------------------------------------------------------------------------
const instance = ref<GameInstance | null>(null)
const instanceSelector = ref<InstanceType<typeof BrowseInstanceSelector>>()
const pendingRouteInstanceSwitch = ref<GameInstance | null>(null)
const activeInstance = computed(() => selection.targetInstance.value ?? instance.value)
const mcArchiveAvailable = computed(() => {
    const gameVersion = activeInstance.value?.game_version
    if (!gameVersion) return false
    const metadata = baseTags.value.gameVersions.find((version) => version.version === gameVersion)
    return metadata ? ['alpha', 'beta'].includes(metadata.version_type) : false
})
const instanceHideInstalled = ref(false)
const newlyInstalled = ref<string[]>([])
const hiddenInstanceProjectIds = ref<Set<string>>(new Set())

const scopedInstalledProjectIds = computed(() => [
    ...discover.installedProjectIds(activeInstance.value?.id),
])
const allInstalledIds = computed(
    () => new Set([...newlyInstalled.value, ...scopedInstalledProjectIds.value]),
)

function syncHiddenInstanceProjectIds() {
    hiddenInstanceProjectIds.value = new Set([
        ...scopedInstalledProjectIds.value,
        ...newlyInstalled.value,
    ])
}

function initInstanceContext() {
    selection.refreshInstances((route.query.i as string | undefined) ?? undefined)
    if (route.query.i) {
        instance.value = preview.instances.get(String(route.query.i)) ?? null
        if (
            instance.value?.install_stage === 'installed' &&
            (selection.selectedCount.value === 0 ||
                selection.targetInstance.value?.id === instance.value.id)
        ) {
            selection.setTarget(instance.value)
        } else if (
            instance.value?.install_stage === 'installed' &&
            selection.selectedCount.value > 0 &&
            selection.targetInstance.value?.id !== instance.value.id
        ) {
            pendingRouteInstanceSwitch.value = instance.value
        }
    }
    if (route.query.ai && projectType.value !== 'modpack') {
        instanceHideInstalled.value = route.query.ai === 'true'
    }
    syncHiddenInstanceProjectIds()
}
initInstanceContext()

const instanceFilters = computed(() => {
    const filters: FilterValue[] = []
    if (activeInstance.value) {
        const gameVersion = activeInstance.value.game_version
        if (gameVersion && usesTargetGameVersion(projectType.value))
            filters.push({ type: 'game_version', option: gameVersion })
        const platform = activeInstance.value.loader
        if (
            platform &&
            projectType.value === 'mod' &&
            ['fabric', 'forge', 'quilt', 'neoforge'].includes(platform)
        )
            filters.push({ type: 'mod_loader', option: platform })
        if (instanceHideInstalled.value && hiddenInstanceProjectIds.value.size > 0) {
            for (const id of hiddenInstanceProjectIds.value)
                filters.push({ type: 'project_id', option: `project_id:${id}`, negative: true })
        }
    }
    return filters
})

const combinedProvidedFilters = computed(() => {
    if (contentSource.value === 'mcarchive') return []
    if (projectType.value === 'modpack' || projectType.value === 'server') return []
    return instanceFilters.value
})

// Breadcrumbs -----------------------------------------------------------------------------------
const browseTitle = computed(() =>
    formatMessage(
        isFromWorlds.value
            ? messages.discoverServers
            : isWorldMapBrowse.value
              ? messages.discoverMaps
              : messages.discoverContent,
    ),
)
watch(browseTitle, (title) => preview.breadcrumbs.setName('BrowseTitle', title), {
    immediate: true,
})
function setInstanceContext(target: GameInstance | null) {
    if (!target) {
        preview.breadcrumbs.setContext(null)
        return
    }
    const instanceLink = `/instance/${encodeURIComponent(target.id)}`
    preview.breadcrumbs.setContext({
        name: target.name,
        link:
            isFromWorlds.value || isWorldMapContext.value ? `${instanceLink}/worlds` : instanceLink,
        iconUrl: displayInstanceIcon(target).url,
    })
}
setInstanceContext(instance.value)

function resetInstanceContext() {
    if (!instance.value) return
    instance.value = null
    instanceHideInstalled.value = false
    newlyInstalled.value = []
    hiddenInstanceProjectIds.value = new Set()
    preview.breadcrumbs.setName('BrowseTitle', formatMessage(messages.discoverContent))
    preview.breadcrumbs.setContext(null)
}

watch(
    () => route.params.projectType as string,
    (newType) => {
        if (!newType || newType === projectType.value || !route.path.startsWith('/browse/')) return
        projectType.value = newType
        root.value?.querySelector('.app-viewport')?.scrollTo({ top: 0 })
    },
)
watch(
    () => route.query.i,
    (instanceId) => {
        if (!instanceId && route.path.startsWith('/browse')) resetInstanceContext()
    },
)

// Tabs --------------------------------------------------------------------------------------------
/** helpers/browse-project-tabs.ts `supportsDataPacks`. */
function supportsDataPacks(gameVersion: string | undefined) {
    const match = gameVersion?.match(/^1\.(\d+)/)
    return match ? Number(match[1]) >= 13 : false
}

const selectableProjectTypes = computed(() => {
    const params: Record<string, string> = {}
    for (const key of ['i', 'ai', 'from', 'sid', 'wid']) {
        const value = route.query[key]
        if (typeof value === 'string') params[key] = value
    }
    const queryString = new URLSearchParams(params).toString()
    const suffix = queryString ? `?${queryString}` : ''
    if (isFromWorlds.value)
        return [{ label: formatMessage(messages.tabServers), href: `/browse/server${suffix}` }]
    if (isWorldMapContext.value)
        return [{ label: formatMessage(messages.tabMaps), href: `/browse/${WORLD}${suffix}` }]
    const target = activeInstance.value
    const hasInstanceContext = !!instance.value
    return [
        {
            label: formatMessage(messages.tabModpacks),
            href: `/browse/modpack${suffix}`,
            shown: !hasInstanceContext,
        },
        {
            label: formatMessage(messages.tabMods),
            href: `/browse/mod${suffix}`,
            shown: !target || target.loader !== 'vanilla',
        },
        { label: formatMessage(messages.tabResourcepacks), href: `/browse/resourcepack${suffix}` },
        {
            label: formatMessage(messages.tabDatapacks),
            href: `/browse/datapack${suffix}`,
            shown: !target || supportsDataPacks(target.game_version),
        },
        { label: formatMessage(messages.tabMaps), href: `/browse/${WORLD}${suffix}` },
        { label: formatMessage(messages.tabShaders), href: `/browse/shader${suffix}` },
        {
            label: formatMessage(messages.tabServers),
            href: `/browse/server${suffix}`,
            shown: !hasInstanceContext,
        },
        {
            label: formatMessage(messages.tabFavorites),
            href: `/browse/favorites${suffix}`,
            onboardingId: 'browse-favorites-tab',
        },
    ]
})

// Install context ---------------------------------------------------------------------------------
const installContext = computed<BrowseInstallContext | null>(() => {
    const target = activeInstance.value
    if (!target) return null
    const icon = displayInstanceIcon(target)
    const processing = ['validating', 'reviewing', 'queueing'].includes(selection.state.value)
    return {
        showInstallHeader: !!instance.value,
        name: target.name,
        loader: target.loader,
        gameVersion: target.game_version,
        iconSrc: icon.url,
        iconFrameless: icon.frameless,
        backUrl: instance.value
            ? `/instance/${encodeURIComponent(instance.value.id)}${isFromWorlds.value || isWorldMapContext.value ? '/worlds' : ''}`
            : route.fullPath,
        backLabel: formatMessage(messages.backToInstance),
        heading: formatMessage(
            isFromWorlds.value
                ? messages.addServersToInstance
                : commonMessages.installingContentLabel,
        ),
        selectedProjects: selection.selectedProjects.value,
        isInstallingSelected: processing,
        installProgress: selection.progress.value,
        installButtonLabel: formatMessage(messages.installSelected, {
            count: selection.selectedCount.value,
        }),
        processingLabel: formatMessage(messages.preparingSelected, {
            completed: selection.progress.value.completed,
            total: selection.progress.value.total,
        }),
        clearSelected: selection.clear,
        installSelected: installSelected,
    }
})

function installSelected() {
    return selection.installSelected(() =>
        addNotification({ title: formatMessage(messages.selectionTargetChanged), type: 'error' }),
    )
}

async function selectTargetInstance(target: GameInstance) {
    selection.setTarget(target)
    newlyInstalled.value = []
    hiddenInstanceProjectIds.value = new Set()
    if (route.query.i) {
        instance.value = target
        await router.replace({ query: { ...route.query, i: target.id } })
        preview.breadcrumbs.setContext({
            name: target.name,
            link: `/instance/${encodeURIComponent(target.id)}`,
            iconUrl: displayInstanceIcon(target).url,
        })
    }
    syncHiddenInstanceProjectIds()
    await searchState.refreshSearch()
}

async function cancelTargetInstanceSwitch() {
    const target = selection.targetInstance.value
    if (!target || !route.query.i || route.query.i === target.id) return
    instance.value = target
    await router.replace({ query: { ...route.query, i: target.id } })
    preview.breadcrumbs.setContext({
        name: target.name,
        link: `/instance/${encodeURIComponent(target.id)}`,
        iconUrl: displayInstanceIcon(target).url,
    })
}

// Card actions --------------------------------------------------------------------------------------
const CART_CONTENT_TYPES = new Set(['mod', 'resourcepack', 'datapack', 'shader', WORLD])
const installingProjectIds = ref<Set<string>>(new Set())

function setProjectInstalling(projectId: string, installing: boolean) {
    const key = `${activeInstance.value?.id ?? ''}\0${projectId}`
    const next = new Set(installingProjectIds.value)
    if (installing) next.add(key)
    else next.delete(key)
    installingProjectIds.value = next
}

function isProjectInstalling(projectId: string) {
    return installingProjectIds.value.has(`${activeInstance.value?.id ?? ''}\0${projectId}`)
}

function toggleContentSelection(project: SearchHit, contentType: string) {
    const target = activeInstance.value
    if (!target) {
        instanceSelector.value?.show()
        return
    }
    const key = makeContentSelectionKey('modrinth', project.project_id)
    if (selection.isSelected(key)) {
        selection.remove(key)
        return
    }
    selection.add(
        {
            key,
            provider: 'modrinth',
            projectId: project.project_id,
            providerProjectId: project.project_id,
            contentType: contentType as ContentSelectionType,
            title: project.title,
            iconUrl: project.icon_url,
            slug: project.slug,
            preferences: getTargetInstallPreferences(
                { gameVersion: target.game_version, loader: target.loader },
                contentType,
            ),
        },
        (title) =>
            addNotification({
                title: formatMessage(messages.selectionDuplicate, { project: title }),
                type: 'error',
            }),
    )
}

function createFavoriteCardAction(project: SearchHit, contentType: string): CardAction {
    const favorite = discover.isFavorite('modrinth', project.project_id)
    const pending = discover.isFavoritePending('modrinth', project.project_id)
    return {
        key: 'favorite',
        label: formatMessage(favorite ? messages.removeFromFavorites : messages.addToFavorites),
        icon: pending ? SpinnerIcon : favorite ? BookmarkFilledIcon : BookmarkIcon,
        iconClass: pending ? 'animate-spin' : undefined,
        disabled: pending,
        color: favorite ? 'brand' : undefined,
        type: 'transparent',
        circular: true,
        tooltip: formatMessage(favorite ? messages.removeFromFavorites : messages.addToFavorites),
        onClick: async () => {
            if (!isFavoriteContentType(contentType)) return
            await discover.toggleFavorite({
                provider: 'modrinth',
                project_id: project.project_id,
                content_type: contentType,
                slug: project.slug,
            })
        },
    }
}

function getCardActions(result: SearchHit, currentProjectType: string): CardAction[] {
    if (currentProjectType === 'server') return []
    const key = makeContentSelectionKey('modrinth', result.project_id)
    const isInstalling = isProjectInstalling(result.project_id) || selection.isInstalling(key)
    const isSelected = selection.isSelected(key)
    const isInstalled =
        (!!activeInstance.value && allInstalledIds.value.has(result.project_id)) ||
        (!!activeInstance.value && selection.isInstalledIdentity('modrinth', result.project_id))
    const favoriteAction = isFavoriteContentType(currentProjectType)
        ? createFavoriteCardAction(result, currentProjectType)
        : null

    if (CART_CONTENT_TYPES.has(currentProjectType)) {
        return [
            {
                key: 'install',
                label: formatMessage(
                    isInstalled
                        ? commonMessages.installedLabel
                        : isSelected
                          ? messages.selected
                          : activeInstance.value
                            ? commonMessages.installButton
                            : messages.chooseInstance,
                ),
                compactLabel:
                    !isInstalled && !isSelected && !activeInstance.value
                        ? formatMessage(messages.add)
                        : undefined,
                icon: isSelected || isInstalled ? CheckIcon : PlusIcon,
                disabled: isInstalled,
                color: isSelected ? 'green' : 'brand',
                type: 'outlined',
                onClick: () => toggleContentSelection(result, currentProjectType),
            },
            ...(favoriteAction ? [favoriteAction] : []),
        ]
    }

    const isModpack = result.project_type === 'modpack'
    const shouldUseInstallIcon = !!instance.value || isModpack
    const label = isInstalling
        ? messages.installingToServer
        : isInstalled
          ? commonMessages.installedLabel
          : shouldUseInstallIcon
            ? commonMessages.installButton
            : messages.addToAnInstance
    return [
        {
            key: 'install',
            label: formatMessage(label),
            compactLabel:
                !isInstalling && !isInstalled && !shouldUseInstallIcon
                    ? formatMessage(messages.add)
                    : undefined,
            icon: isInstalling ? SpinnerIcon : isInstalled ? CheckIcon : PlusIcon,
            iconClass: isInstalling ? 'animate-spin' : undefined,
            disabled: isInstalled || isInstalling,
            color: 'brand',
            type: 'outlined',
            onClick: async () => {
                setProjectInstalling(result.project_id, true)
                try {
                    await contentInstall.install(
                        result.project_id,
                        null,
                        instance.value ? instance.value.id : null,
                        'SearchCard',
                        (versionId, installedProjectIds) => {
                            setProjectInstalling(result.project_id, false)
                            if (versionId)
                                newlyInstalled.value = [
                                    ...new Set([
                                        ...newlyInstalled.value,
                                        ...(installedProjectIds ?? [result.project_id]),
                                    ]),
                                ]
                        },
                        (profile) => {
                            void router.push(isModpack ? '/downloads' : `/instance/${profile}`)
                        },
                    )
                } catch (error) {
                    setProjectInstalling(result.project_id, false)
                    handleError(error)
                }
            },
        },
    ]
}

// Search ---------------------------------------------------------------------------------------------
const resourceSearchNotice = ref('')

function wait(ms: number, signal: AbortSignal) {
    return new Promise<void>((resolve, reject) => {
        const cancel = preview.clock.after(ms, resolve)
        signal.addEventListener(
            'abort',
            () => {
                cancel()
                reject(new DOMException('Aborted', 'AbortError'))
            },
            { once: true },
        )
    })
}

async function search(
    request: BrowseSearchRequest,
    signal: AbortSignal,
): Promise<BrowseSearchResponse> {
    const type = projectType.value
    await wait(380, signal)
    if (type === 'server') throw new Error(formatMessage(messages.serverSourceUnavailable))
    if (type === WORLD) {
        return {
            projectHits: [],
            serverHits: [],
            total_hits: 0,
            has_more: false,
            per_page: request.limit,
            page: 1,
        }
    }
    if (!isSnapshotType(type)) throw new SourceFailure('modrinth')
    const pool = await discover.ensurePool(type)
    if (request.filters.some((filter) => filter.type === 'compatible_dependency_project_ids')) {
        await Promise.all(pool.map((card) => discover.ensureDetail(card.id)))
    }
    // `ResourceService::search` asks the source for the dictionary's reading of the query.
    const text = await modrinthSearchText(request.query)
    signal.throwIfAborted()
    const page = runResourceSearch({
        projectType: type,
        pool,
        text,
        filters: request.filters,
        filterTypes: searchState.filters.value,
        sort: request.sort,
        limit: request.limit,
        page: request.page,
        source: contentSource.value,
        installedIds: allInstalledIds.value,
        details: (id) => discover.details.value[id],
    })
    if (displayMode.value === 'grid') {
        await Promise.all(page.hits.map((hit) => discover.ensureDetail(hit.project_id)))
        signal.throwIfAborted()
        const rerun = runResourceSearch({
            projectType: type,
            pool,
            text,
            filters: request.filters,
            filterTypes: searchState.filters.value,
            sort: request.sort,
            limit: request.limit,
            page: request.page,
            source: contentSource.value,
            installedIds: allInstalledIds.value,
            details: (id) => discover.details.value[id],
        })
        page.hits = rerun.hits
    }
    page.hits = await translateSearchHitTitles(page.hits, locale.value)
    signal.throwIfAborted()
    resourceSearchNotice.value = page.failures
        .map((source) => formatMessage(messages.sourceFailed, { source: sourceName(source) }))
        .join('; ')
    return {
        projectHits: page.hits.filter((hit) => !shouldHideInstalledProject(hit.project_id)),
        serverHits: [],
        total_hits: page.total,
        has_more: page.hasMore,
        per_page: request.limit,
        page: page.page,
    }
}

function shouldHideInstalledProject(projectId: string) {
    return (
        !!activeInstance.value &&
        instanceHideInstalled.value &&
        allInstalledIds.value.has(projectId)
    )
}

const displayMode = ref<BrowseDisplayMode>(discover.displayMode.value)
const displayModeOptions = computed(() => [
    { id: 'list' as const, label: formatMessage(messages.listView), icon: ListIcon },
    {
        id: 'compact' as const,
        label: formatMessage(messages.compactListView),
        icon: GenericListIcon,
    },
    { id: 'grid' as const, label: formatMessage(messages.gridView), icon: GridIcon },
])
function setDisplayMode(mode: BrowseDisplayMode) {
    const wasGrid = displayMode.value === 'grid'
    displayMode.value = mode
    discover.displayMode.value = mode
    if (mode === 'grid' && !wasGrid) void searchState.refreshSearch()
}

const searchState = useBrowseSearch({
    clock: preview.clock,
    projectType,
    tags,
    providedFilters: combinedProvidedFilters,
    installContextLoader: computed(() => installContext.value?.loader),
    search: async (request, signal) => {
        resourceSearchNotice.value = ''
        return search(request, signal)
    },
    formatError: (error) =>
        error instanceof SourceFailure
            ? formatMessage(messages.sourceFailed, { source: sourceName(error.source) })
            : error instanceof Error
              ? error.message
              : String(error),
    persistentQueryParams: ['i', 'ai', 'shi', 'sid', 'wid', 'from', 'source', 'b'],
    getExtraQueryParams: () => ({
        ai: instanceHideInstalled.value ? 'true' : undefined,
        source:
            isWorldMapBrowse.value || contentSource.value === 'all'
                ? undefined
                : contentSource.value,
    }),
    displayMode,
})

// Filter memory (helpers/browse-filter-memory.ts) -----------------------------------------------------
const NON_FILTER_BROWSE_QUERY_PARAMS = new Set([
    'i',
    'ai',
    'shi',
    'sid',
    'wid',
    'from',
    'source',
    'q',
    's',
    'ss',
    'm',
    'o',
    'page',
    'b',
])

function hasExplicitFilterQuery() {
    return Object.keys(route.query).some((key) => !NON_FILTER_BROWSE_QUERY_PARAMS.has(key))
}

function availableRememberedFilters(type: string, filters: FilterValue[]) {
    const filterTypes =
        type === 'server' ? searchState.serverFilterTypes.value : searchState.filters.value
    return filters.filter((filter) => {
        const filterType = filterTypes.find((candidate) => candidate.id === filter.type)
        return (
            filterType &&
            (filterType.allows_custom_options ||
                filterType.options.some((option) => option.id === filter.option))
        )
    })
}

function applyRememberedFilters(type: string, resetWhenMissing: boolean) {
    const memory = discover.filterMemory.value[type]
    if (!memory && !resetWhenMissing) return
    if (type === 'server') {
        searchState.serverCurrentFilters.value = memory
            ? availableRememberedFilters(type, memory.filters)
            : [{ type: 'server_status', option: 'online' }]
        searchState.serverToggledGroups.value = memory ? [...memory.toggledGroups] : []
        return
    }
    const filterTypeIds = new Set(searchState.filters.value.map((filter) => filter.id))
    searchState.currentFilters.value = memory
        ? availableRememberedFilters(type, memory.filters)
        : []
    searchState.toggledGroups.value = memory ? [...memory.toggledGroups] : []
    searchState.overriddenProvidedFilterTypes.value = memory
        ? memory.overriddenProvidedFilterTypes.filter((filterType) => filterTypeIds.has(filterType))
        : []
}

if (!hasExplicitFilterQuery()) applyRememberedFilters(projectType.value, false)

watch(projectType, (type) => applyRememberedFilters(type, true))

watch(
    [
        projectType,
        searchState.currentFilters,
        searchState.toggledGroups,
        searchState.overriddenProvidedFilterTypes,
        searchState.serverCurrentFilters,
        searchState.serverToggledGroups,
    ],
    () => {
        const isServer = projectType.value === 'server'
        discover.filterMemory.value = {
            ...discover.filterMemory.value,
            [projectType.value]: {
                filters: (isServer
                    ? searchState.serverCurrentFilters.value
                    : searchState.currentFilters.value
                ).map((filter) => ({ ...filter })),
                toggledGroups: [
                    ...(isServer
                        ? searchState.serverToggledGroups.value
                        : searchState.toggledGroups.value),
                ],
                overriddenProvidedFilterTypes: isServer
                    ? []
                    : [...searchState.overriddenProvidedFilterTypes.value],
            },
        }
    },
    { deep: true },
)

// Translation (composables/browse/use-search-result-translation.ts) ------------------------------
const translationActive = ref(false)
const translationLoading = ref(false)
let translationVersion = 0

watch(searchState.projectHits, () => {
    translationVersion++
    translationActive.value = false
    translationLoading.value = false
})

function toggleTranslation() {
    if (translationActive.value) {
        translationVersion++
        translationActive.value = false
        translationLoading.value = false
        return
    }
    if (discover.noteManualTranslateClick()) {
        addNotification({
            title: formatMessage(messages.autoTranslateHintTitle),
            text: formatMessage(messages.autoTranslateHintText),
            type: 'info',
        })
    }
    const version = ++translationVersion
    translationLoading.value = true
    // English needs no translation; other languages read the mcimirror description cache,
    // which holds nothing for these results, so they stay in their original language.
    const delay = /^en/.test(locale.value) ? 60 : 900
    preview.clock.after(delay, () => {
        if (version !== translationVersion) return
        translationLoading.value = false
        translationActive.value = false
    })
}

// Source menu ----------------------------------------------------------------------------------------
const sourceOptions = computed(() => [
    { id: 'all' as const, label: messages.allSources, icon: GlobeIcon },
    { id: 'modrinth' as const, label: messages.modrinthSource, icon: ModrinthIcon },
    ...(projectType.value === 'mod' && mcArchiveAvailable.value
        ? [{ id: 'mcarchive' as const, label: messages.mcArchiveSource, icon: FileArchiveIcon }]
        : []),
])
const sourceIcon = computed(() => {
    if (!sourceOptions.value.some((option) => option.id === contentSource.value)) return GlobeIcon
    return contentSource.value === 'modrinth'
        ? ModrinthIcon
        : contentSource.value === 'mcarchive'
          ? FileArchiveIcon
          : GlobeIcon
})
const currentSourceLabel = computed(
    () =>
        sourceOptions.value.find((option) => option.id === contentSource.value)?.label ??
        messages.allSources,
)

function selectContentSource(source: BrowseContentSource) {
    if (isWorldMapBrowse.value) return
    if (
        source === 'all' ||
        source === 'modrinth' ||
        (source === 'mcarchive' && mcArchiveAvailable.value)
    ) {
        contentSource.value = source
        discover.contentSource.value = source
    }
}

watch(contentSource, async () => {
    searchState.projectHits.value = []
    searchState.totalHits.value = 0
    searchState.currentPage.value = 1
    searchState.loading.value = true
    searchState.currentFilters.value = searchState.currentFilters.value.filter(
        (filter) => !filter.type.startsWith('category_'),
    )
    await searchState.refreshSearch()
})

watch(projectType, (type) => {
    if (type !== 'mod' && contentSource.value === 'mcarchive') {
        contentSource.value = 'all'
        discover.contentSource.value = 'all'
    }
})

watch(
    [
        () => searchState.query.value,
        () => searchState.currentFilters.value,
        () => projectType.value,
    ],
    () => {
        if (activeInstance.value) syncHiddenInstanceProjectIds()
    },
    { deep: true },
)

// Lifecycle ---------------------------------------------------------------------------------------------
let unmounted = false
const browseReturn =
    discover.browseReturn.value?.url === route.fullPath ? discover.browseReturn.value : null
discover.browseReturn.value = null
let pendingReturnScroll = !!browseReturn

watch(
    searchState.loading,
    async (isLoading) => {
        if (isLoading || !pendingReturnScroll || !browseReturn) return
        pendingReturnScroll = false
        await nextTick()
        requestAnimationFrame(() => {
            if (unmounted) return
            root.value?.querySelector('.app-viewport')?.scrollTo({ top: browseReturn.scrollTop })
        })
    },
    { flush: 'post' },
)

onMounted(() => {
    if (pendingRouteInstanceSwitch.value) {
        instanceSelector.value?.requestSwitch(pendingRouteInstanceSwitch.value)
        pendingRouteInstanceSwitch.value = null
    }
    // The first search waits for the page transition to settle (helpers/page-transition.ts).
    preview.clock.after(220, () => {
        if (unmounted) return
        if (browseReturn) searchState.currentPage.value = Math.max(1, browseReturn.page)
        void searchState.refreshSearch()
    })
    void Promise.all(SNAPSHOT_TYPES.map((type) => discover.ensurePool(type).catch(() => [])))
})

onUnmounted(() => {
    unmounted = true
})

onBeforeRouteLeave((to) => {
    if (to.path.startsWith('/project')) {
        const viewport = root.value?.querySelector<HTMLElement>('.app-viewport')
        discover.browseReturn.value = {
            url: route.fullPath,
            scrollTop: viewport?.scrollTop ?? 0,
            page: searchState.currentPage.value,
        }
    }
    preview.breadcrumbs.setContext({
        name: '?BrowseTitle',
        link: `/browse/${projectType.value}`,
        query: Object.fromEntries(
            Object.entries(route.query).filter(
                (entry): entry is [string, string] => typeof entry[1] === 'string',
            ),
        ),
    })
})

// Context ------------------------------------------------------------------------------------------------
const contextMenu = ref<InstanceType<typeof BrowseContextMenu>>()

function getProjectLink(result: SearchHit) {
    const details: LocationQuery = { ...route.query, b: route.fullPath }
    delete details.g
    delete details.l
    delete details.c
    delete details.page
    return { path: `/project/${result.project_id ?? result.slug}`, query: details }
}

const lockedFilterMessages = computed(() => ({
    gameVersion: formatMessage(messages.gameVersionProvidedByInstance),
    modLoader: formatMessage(messages.modLoaderProvidedByInstance),
    environment: formatMessage(messages.environmentProvidedByServer),
    syncButton: formatMessage(messages.syncFilterButton),
    providedBy: formatMessage(messages.providedByInstance),
}))

provideBrowseContext({
    tags,
    projectType,
    ...searchState,
    getProjectLink,
    selectableProjectTypes,
    showProjectTypeTabs: computed(() => true),
    getCardActions,
    installContext,
    providedFilters: combinedProvidedFilters,
    hideInstalled: computed({
        get: () => instanceHideInstalled.value,
        set: (value: boolean) => {
            instanceHideInstalled.value = value
            if (value) syncHiddenInstanceProjectIds()
        },
    }),
    showHideInstalled: computed(() => !isWorldMapBrowse.value && !!activeInstance.value),
    hideInstalledLabel: computed(() =>
        formatMessage(
            isFromWorlds.value
                ? messages.hideAddedServers
                : commonMessages.hideInstalledContentLabel,
        ),
    ),
    advancedFiltersCollapsed: discover.advancedFiltersCollapsed,
    displayMode,
    displayModeOptions,
    displayModeTooltip: computed(() => formatMessage(messages.switchView)),
    setDisplayMode,
    onContextMenu: (event, result) => contextMenu.value?.show(event, result),
    lockedFilterMessages,
    findProject: (id) => {
        const card = discover.findCard(id)
        return card
            ? {
                  id: card.id,
                  title: card.title,
                  icon_url: `/launcher-preview/projects/${card.id}.webp`,
              }
            : null
    },
    searchProjects: async (query, types) => {
        const wanted = types.filter(isSnapshotType)
        const pools = await Promise.all(
            wanted.map((type) => discover.ensurePool(type).then((pool) => ({ type, pool }))),
        )
        const needle = query.toLowerCase()
        return pools.flatMap(({ type, pool }) =>
            pool
                .filter(
                    (card) =>
                        card.title.toLowerCase().includes(needle) ||
                        (card.slug ?? '').toLowerCase().includes(needle) ||
                        card.id.toLowerCase() === needle,
                )
                .map((card) => ({
                    project_id: card.id,
                    title: card.title,
                    icon_url: `/launcher-preview/projects/${card.id}.webp`,
                    project_type: type,
                })),
        )
    },
})
</script>

<template>
    <div data-onboarding-id="browse-content" class="flex flex-col gap-3 p-6">
        <BrowseLayout v-if="!isWorldMapBrowse">
            <template #nav-tabs-actions>
                <Button
                    type="quiet"
                    size="xl"
                    :disabled="translationLoading"
                    @click="toggleTranslation"
                    ><SpinnerIcon v-if="translationLoading" class="animate-spin" />
                    <LanguagesIcon v-else />
                    {{
                        formatMessage(
                            translationLoading
                                ? messages.translating
                                : translationActive
                                  ? messages.showOriginal
                                  : messages.translateProject,
                        )
                    }}
                </Button>
            </template>
            <template #search-bar-actions>
                <Button
                    v-if="projectType !== 'server' && projectType !== 'modpack'"
                    size="md"
                    class="flex min-w-0 items-center gap-2"
                    @click="instanceSelector?.show()"
                    ><InstanceIcon
                        v-if="activeInstance"
                        class="shrink-0"
                        size="1.25rem"
                        :icon-path="activeInstance.icon_path"
                        :instance-id="activeInstance.id"
                        :loader="activeInstance.loader"
                    />
                    <PlusIcon v-else class="size-5 shrink-0" />
                    <span class="max-w-40 truncate font-medium">
                        {{ activeInstance?.name ?? formatMessage(messages.chooseInstance) }}
                    </span>
                    <span
                        aria-hidden="true"
                        class="flex size-4 shrink-0 items-center justify-center text-[var(--color-text-tertiary)]"
                    >
                        <ChevronDownIcon class="size-4" />
                    </span>
                </Button>
                <PopoutMenu
                    v-if="projectType !== 'server'"
                    placement="bottom-end"
                    :container="layer ?? undefined"
                >
                    <Button size="md" class="flex items-center gap-2"
                        ><component :is="sourceIcon" class="h-5 w-5" />
                        <span>{{ formatMessage(currentSourceLabel) }}</span>
                    </Button>
                    <template #menu>
                        <div class="flex w-min flex-col gap-1 p-1">
                            <Button
                                v-for="option in sourceOptions"
                                :key="option.id"
                                :type="contentSource === option.id ? 'colored' : 'quiet'"
                                class="flex w-full items-center !justify-start gap-2 text-left"
                                @click="selectContentSource(option.id)"
                            >
                                <component :is="option.icon" class="h-4 w-4" />
                                {{ formatMessage(option.label) }}
                            </Button>
                        </div>
                    </template>
                </PopoutMenu>
            </template>
            <template #after>
                <BrowseContextMenu ref="contextMenu" />
            </template>
            <template #above-results>
                <div
                    v-if="resourceSearchNotice"
                    role="status"
                    class="mb-3 rounded-lg border border-solid border-surface-4 bg-surface-2 p-3 text-sm"
                >
                    {{ resourceSearchNotice }}
                </div>
            </template>
        </BrowseLayout>
        <EmptyState
            v-else
            type="empty-inbox"
            :heading="formatMessage(messages.mapsUnavailable)"
            :description="formatMessage(messages.mapsUnavailableDescription)"
        />
        <BrowseInstanceSelector
            ref="instanceSelector"
            :instances="selection.instances.value"
            :selected-instance="activeInstance"
            :selected-count="selection.selectedCount.value"
            :install-current="installSelected"
            :clear-current="selection.clear"
            @select="selectTargetInstance"
            @cancel-switch="cancelTargetInstanceSwitch"
        />
        <InstallModals />
        <Teleport defer to="#sidebar-teleport-target">
            <BrowseSidebar />
        </Teleport>
    </div>
</template>

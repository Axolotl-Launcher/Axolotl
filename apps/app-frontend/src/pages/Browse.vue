<script setup lang="ts">
import type { Labrinth } from '@modrinth/api-client'
import {
    BookmarkFilledIcon,
    BookmarkIcon,
    CheckIcon,
    ChevronDownIcon,
    ClipboardCopyIcon,
    CurseForgeIcon,
    ExternalIcon,
    FileArchiveIcon,
    GenericListIcon,
    GlobeIcon,
    GridIcon,
    LanguagesIcon,
    ListIcon,
    ModrinthIcon,
    PlusIcon,
    SpinnerIcon,
} from '@modrinth/assets'
import type {
    BrowseDisplayMode,
    BrowseDisplayModeOption,
    BrowseInstallContentType,
    CardAction,
    ProjectType,
    Tags,
} from '@modrinth/ui'
import {
    BrowsePageLayout,
    BrowseSidebar,
    Button,
    commonMessages,
    CreationFlowModal,
    defineMessages,
    EmptyState,
    getLatestMatchingInstallVersion,
    getSelectedInstallPreferences,
    getTargetInstallPreferences,
    injectNotificationManager,
    isVersionTypeMatch,
    PopoutMenu,
    preferencesDiffer,
    provideBrowseManager,
    requestInstall,
    stripServerRuntimeInstallFilters,
    stripServerRuntimeInstallOverrides,
    useBrowseSearch,
    useDebugLogger,
    usesTargetGameVersion,
    useVIntl,
} from '@modrinth/ui'
import type { Ref } from 'vue'
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import type { LocationQuery } from 'vue-router'
import { onBeforeRouteLeave, useRoute, useRouter } from 'vue-router'

import BrowseInstanceSelector from '@/components/browse/BrowseInstanceSelector.vue'
import ContextMenu from '@/components/ui/ContextMenu.vue'
import InstanceIcon from '@/components/ui/InstanceIcon.vue'
import { useAppServerBrowse } from '@/composables/browse/use-app-server-browse'
import { useSearchResultTranslation } from '@/composables/browse/use-search-result-translation'
import { useContentFavorites } from '@/composables/useContentFavorites'
import { useNetworkStatus } from '@/composables/useNetworkStatus'
import {
    type BrowseFilterMemory,
    getBrowseFilterMemory,
    setBrowseFilterMemory,
} from '@/helpers/browse-filter-memory'
import { createBrowseProjectTabs, getBrowseProjectTabOptions } from '@/helpers/browse-project-tabs'
import {
    cancel_search_request,
    get_project,
    get_project_v3,
    get_search_results_v3,
    get_version_many,
} from '@/helpers/cache.js'
import { type FavoriteContentType, isFavoriteContentType } from '@/helpers/content-favorites'
import { translateSearchHitTitles } from '@/helpers/content-search'
import {
    type CurseForgeCategory,
    getCurseForgeCapability,
    getCurseForgeCategories,
    getCurseForgeImageUrl,
} from '@/helpers/curseforge'
import {
    CF_EXTRA_CATEGORY_HEADER,
    curseForgeCategoryValue,
    findUnmappedCurseForgeCategories,
    localizeCurseForgeCategoryName,
    localizeCurseForgeLabel,
    resolveCurseForgeCategoryIdsFromFilterValues,
} from '@/helpers/curseforge-category-map'
import { instance_listener } from '@/helpers/events.js'
import {
    get as getInstance,
    get_installed_project_ids as getInstalledProjectIds,
} from '@/helpers/instance'
import { getDisplayInstanceIcon } from '@/helpers/instance-icons'
import { instanceServerProjectId, isServerProjectLink } from '@/helpers/instance-project'
import { getMcArchiveGameVersions, type McArchiveGameVersion } from '@/helpers/mcarchive'
import { get_loader_versions as getLoaderManifest } from '@/helpers/metadata'
import { planetMinecraftConnectorAvailable } from '@/helpers/planet-minecraft'
import { createProjectDetailsQuery } from '@/helpers/project-links'
import { createProjectPreviewState } from '@/helpers/project-preview'
import {
    isResourceOperationError,
    resourceOperationError,
    resourceSearchFailure,
    resourceSourceName,
} from '@/helpers/resource-operation-logic'
import {
    loadResourceSearchCategories,
    parseResourceSearchFilter,
} from '@/helpers/resource-search-filter'
import { ResourceSearchPages } from '@/helpers/resource-search-pages'
import {
    resourceSearch,
    type ResourceSearchQuery,
    type ResourceType,
    type SourceId,
} from '@/helpers/resources'
import {
    type BrowseContentSource,
    getLastBrowseContentDisplayMode,
    getLastBrowseContentSource,
    isBrowseContentProjectType,
    setLastBrowseContentDisplayMode,
    setLastBrowseContentProjectType,
    setLastBrowseContentSource,
    update as updateSettings,
} from '@/helpers/settings.ts'
import { get_categories, get_game_versions, get_loaders } from '@/helpers/tags'
import { autoTranslateHintMessages, noteManualTranslateClick } from '@/helpers/translation'
import type { GameInstance } from '@/helpers/types'
import { get_instance_worlds } from '@/helpers/worlds'
import i18n from '@/i18n.config'
import { injectContentInstall } from '@/providers/content-install'
import { injectContentSelection, makeContentSelectionKey } from '@/providers/content-selection'
import { injectServerInstall } from '@/providers/server-install'
import {
    createServerInstallContent,
    provideServerInstallContent,
} from '@/providers/setup/server-install-content'
import { useBreadcrumbs } from '@/store/breadcrumbs'
import { isBrowseReturnSourcePath, useNavigationReturnStore } from '@/store/navigation-return'
import { useTheming } from '@/store/state'

const { addNotification, handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()
const { installingServerProjects, playServerProject, showAddServerToInstanceModal } =
    injectServerInstall()
const { install: installVersion, installCurseForge } = injectContentInstall()
const contentSelection = injectContentSelection()
const contentFavorites = useContentFavorites()
const debugLog = useDebugLogger('Browse')
const router = useRouter()
const route = useRoute()

void contentFavorites.load().catch(handleError)
const projectType = ref<ProjectType>(route.params.projectType as ProjectType)
const WORLD_BROWSE_PROJECT_TYPE = 'world' as ProjectType
const isWorldMapBrowse = computed(() => projectType.value === WORLD_BROWSE_PROJECT_TYPE)
const isWorldMapContext = computed(() => route.query.from === 'world-maps')

function rememberBrowseContentProjectType(type: ProjectType) {
    if (
        !route.query.i &&
        !route.query.sid &&
        !route.query.wid &&
        !route.query.from &&
        isBrowseContentProjectType(type)
    ) {
        setLastBrowseContentProjectType(type)
    }
}

watch(projectType, rememberBrowseContentProjectType, { immediate: true })

const curseForgeClassIds: Partial<Record<ProjectType, number>> = {
    mod: 6,
    plugin: 5,
    resourcepack: 12,
    datapack: 6945,
    shader: 6552,
    modpack: 4471,
    [WORLD_BROWSE_PROJECT_TYPE]: 17,
}

const searchEnabled = ref(false)
const browseInitialized = ref(false)
const filterMetadataStatus = ref<'loading' | 'ready' | 'error'>('loading')
const filtersReady = computed(
    () => browseInitialized.value && filterMetadataStatus.value === 'ready',
)
const filterMetadataFailed = computed(() => filterMetadataStatus.value === 'error')
let filterMetadataVersion = 0
const curseForgeCapability = ref<Awaited<ReturnType<typeof getCurseForgeCapability>>>({
    status: 'missing_key',
    configured: false,
    key_source: 'missing',
})
const planetMinecraftAvailable = ref(false)
const rememberedContentSource = getLastBrowseContentSource()

function resolveInitialContentSource(): BrowseContentSource {
    if (route.params.projectType === WORLD_BROWSE_PROJECT_TYPE) {
        return curseForgeCapability.value.configured ? 'curseforge' : 'modrinth'
    }
    if (route.params.projectType === 'mod' && route.query.source === 'mcarchive') {
        return 'mcarchive'
    }
    if (
        route.params.projectType === 'mod' &&
        route.query.source === 'planet_minecraft' &&
        planetMinecraftAvailable.value
    ) {
        return 'planet_minecraft'
    }
    if (curseForgeCapability.value.configured && route.query.source === 'curseforge') {
        return 'curseforge'
    }
    if (route.query.source === 'modrinth') {
        return 'modrinth'
    }
    if (rememberedContentSource === 'curseforge' && curseForgeCapability.value.configured) {
        return 'curseforge'
    }
    if (rememberedContentSource === 'modrinth') {
        return 'modrinth'
    }
    if (rememberedContentSource === 'mcarchive' && route.params.projectType === 'mod') {
        return 'mcarchive'
    }
    if (
        rememberedContentSource === 'planet_minecraft' &&
        route.params.projectType === 'mod' &&
        planetMinecraftAvailable.value
    ) {
        return 'planet_minecraft'
    }
    return curseForgeCapability.value.configured ? 'all' : 'modrinth'
}

const contentSource = ref<BrowseContentSource>('modrinth')
const sourceBeforeWorldMapBrowse = ref<BrowseContentSource>(
    isWorldMapBrowse.value ? (getLastBrowseContentSource() ?? 'all') : contentSource.value,
)
if (isWorldMapBrowse.value && curseForgeCapability.value.configured) {
    contentSource.value = 'curseforge'
}
const curseForgeCategoriesByClass = ref<Record<number, CurseForgeCategory[]>>({})
const curseForgeCategoryRequests = new Map<number, Promise<void>>()

async function ensureCurseForgeCategories(projectTypeValue: ProjectType) {
    if (!curseForgeCapability.value.configured) return
    const classId = curseForgeClassIds[projectTypeValue]
    if (!classId || curseForgeCategoriesByClass.value[classId]) return

    const pending = curseForgeCategoryRequests.get(classId)
    if (pending) return pending
    const request = getCurseForgeCategories(classId)
        .then((classCategories) => {
            if (isUnmounted) return
            curseForgeCategoriesByClass.value = {
                ...curseForgeCategoriesByClass.value,
                [classId]: classCategories,
            }
        })
        .finally(() => curseForgeCategoryRequests.delete(classId))
    curseForgeCategoryRequests.set(classId, request)
    return request
}

const themeStore = useTheming()
const navReturn = useNavigationReturnStore()
const serverSetupModalRef = ref<InstanceType<typeof CreationFlowModal> | null>(null)
const serverInstallContent = createServerInstallContent({ serverSetupModalRef })
provideServerInstallContent(serverInstallContent)
const {
    serverIdQuery,
    serverFlowFrom,
    isFromWorlds,
    isServerContext,
    isSetupServerContext,
    effectiveServerWorldId,
    serverContextServerData,
    serverContentProjectIds,
    queuedServerInstallProjectIds,
    queuedServerInstallCount,
    selectedServerInstallProjects,
    isInstallingQueuedServerInstalls,
    queuedInstallProgress,
    serverBackUrl,
    serverBackLabel,
    serverBrowseHeading,
    clearQueuedServerInstalls,
    removeQueuedServerInstall,
    flushQueuedServerInstalls,
    discardQueuedServerInstallsAndBack,
    installQueuedServerInstallsAndBack,
    initServerContext,
    watchServerContextChanges,
    searchServerModpacks,
    getServerProjectVersions,
    enforceSetupModpackRoute,
    getQueuedServerInstallPlans,
    setQueuedServerInstallPlans,
    openServerModpackInstallFlow,
    onServerFlowBack,
    handleServerModpackFlowCreate,
    markServerProjectInstalled,
} = serverInstallContent

const categories = ref<Labrinth.Tags.v2.Category[]>([])
const loaders = ref<Labrinth.Tags.v2.Loader[]>([])
const availableGameVersions = ref<Labrinth.Tags.v2.GameVersion[]>([])
const mcArchiveGameVersions = ref<McArchiveGameVersion[]>([])

async function loadFilterMetadata(type: ProjectType, source: BrowseContentSource) {
    if (type === WORLD_BROWSE_PROJECT_TYPE) return
    const requests: Promise<unknown>[] = [
        get_game_versions().then((versions) => {
            if (!isUnmounted) availableGameVersions.value = versions
        }),
    ]
    if (source !== 'mcarchive' && source !== 'planet_minecraft') {
        requests.push(
            get_categories().then((result) => {
                if (!isUnmounted) categories.value = result
            }),
            get_loaders().then((result) => {
                if (!isUnmounted) loaders.value = result
            }),
        )
    }
    if (source === 'curseforge' || source === 'all') requests.push(ensureCurseForgeCategories(type))
    if (source === 'mcarchive') requests.push(refreshMcArchiveGameVersions())
    await Promise.all(requests)
}

async function prepareFilterMetadata(type = projectType.value, source = contentSource.value) {
    const version = ++filterMetadataVersion
    filterMetadataStatus.value = 'loading'
    const isCurrent = () =>
        !isUnmounted &&
        version === filterMetadataVersion &&
        type === projectType.value &&
        source === contentSource.value
    try {
        await loadFilterMetadata(type, source)
        if (isCurrent()) filterMetadataStatus.value = 'ready'
    } catch (error) {
        if (isCurrent()) filterMetadataStatus.value = 'error'
        throw error
    }
}

function retryFilterMetadata() {
    if (!searchEnabled.value) void refreshBrowse()
    else void prepareFilterMetadata().catch(() => {})
}

async function refreshMcArchiveGameVersions() {
    if (mcArchiveGameVersions.value.length > 0) return
    const versions = await getMcArchiveGameVersions()
    if (!isUnmounted) mcArchiveGameVersions.value = versions
}

const curseForgeCategoryTags = computed(() => {
    const classId = curseForgeClassIds[projectType.value]
    if (!classId) return []

    const classCategories = curseForgeCategoriesByClass.value[classId] ?? []
    const categoriesById = new Map(classCategories.map((category) => [category.id, category]))
    return classCategories
        .filter((category) => !category.isClass)
        .map((category) => {
            const parent = category.parentCategoryId
                ? categoriesById.get(category.parentCategoryId)
                : undefined
            const isResolution =
                classId === 12 && category.displayIndex != null && category.displayIndex < 0
            const displayName = localizeCurseForgeCategoryName(category)
            const header = isResolution
                ? 'resolutions'
                : parent && parent.id !== classId
                  ? parent.slug
                  : 'categories'
            const headerDisplayName =
                header === 'resolutions' || header === 'categories'
                    ? undefined
                    : localizeCurseForgeLabel(parent?.slug, parent?.name, header)
            return {
                icon: getCurseForgeImageUrl(category.iconUrl, 32) ?? '',
                icon_url: getCurseForgeImageUrl(category.iconUrl, 32),
                name: category.slug,
                display_name: displayName,
                header_display_name: headerDisplayName,
                display_index: category.displayIndex,
                project_type: projectType.value,
                header,
            }
        })
})

const allSourceCategoryTags = computed(() => {
    const classId = curseForgeClassIds[projectType.value]
    const modrinthCategories = categories.value ?? []
    if (!classId) return modrinthCategories

    const classCategories = curseForgeCategoriesByClass.value[classId] ?? []
    if (classCategories.length === 0) return modrinthCategories

    const unmapped = findUnmappedCurseForgeCategories(
        modrinthCategories.map((category) => category.name),
        classCategories,
    )

    const extraTags = unmapped.map((category) => ({
        icon: getCurseForgeImageUrl(category.iconUrl, 32) ?? '',
        icon_url: getCurseForgeImageUrl(category.iconUrl, 32),
        name: curseForgeCategoryValue(category.id),
        display_name: localizeCurseForgeCategoryName(category),
        display_index: category.displayIndex,
        project_type: projectType.value,
        header: CF_EXTRA_CATEGORY_HEADER,
    }))

    return [...modrinthCategories, ...extraTags]
})

const tags: Ref<Tags> = computed(() => ({
    gameVersions:
        contentSource.value === 'mcarchive'
            ? mcArchiveGameVersions.value.map((version) => ({
                  version: version.name,
                  version_type: version.versionType ?? 'release',
                  date: '',
                  major: false,
              }))
            : (availableGameVersions.value ?? []),
    loaders:
        contentSource.value === 'mcarchive' || contentSource.value === 'planet_minecraft'
            ? []
            : (loaders.value ?? []),
    categories:
        contentSource.value === 'mcarchive' || contentSource.value === 'planet_minecraft'
            ? []
            : contentSource.value === 'curseforge'
              ? curseForgeCategoryTags.value
              : contentSource.value === 'all'
                ? allSourceCategoryTags.value
                : (categories.value ?? []),
}))

const instance = ref<GameInstance | null>(null)
const instanceSelector = ref()
const pendingRouteInstanceSwitch = ref<GameInstance | null>(null)
const activeInstance = computed(() => contentSelection.targetInstance.value ?? instance.value)
const mcArchiveAvailable = computed(() => {
    const gameVersion = activeInstance.value?.game_version
    if (!gameVersion) return false
    const metadata = availableGameVersions.value.find((version) => version.version === gameVersion)
    return metadata ? isVersionTypeMatch(metadata.version_type, gameVersion, 'ancient') : false
})
const installedProjectIds = ref<string[] | null>(null)
const installedProjectIdsInstanceId = ref<string | null>(null)
let installedProjectRequestId = 0
const instanceHideInstalled = ref(false)
const newlyInstalled = ref<string[]>([])
const hiddenInstanceProjectIds = ref<Set<string>>(new Set())
const hiddenInstanceProjectIdsInitialized = ref(false)
const isServerInstance = ref(false)

if (isFromWorlds.value && route.params.projectType !== 'server') {
    router.replace({
        path: '/browse/server',
        query: route.query,
    })
}

enforceSetupModpackRoute(route.params.projectType as string | undefined)

const scopedInstalledProjectIds = computed(() =>
    installedProjectIdsInstanceId.value === activeInstance.value?.id
        ? (installedProjectIds.value ?? [])
        : [],
)

const allInstalledIds = computed(
    () => new Set([...newlyInstalled.value, ...scopedInstalledProjectIds.value]),
)

function syncHiddenInstanceProjectIds() {
    hiddenInstanceProjectIds.value = new Set([
        ...scopedInstalledProjectIds.value,
        ...newlyInstalled.value,
    ])
    hiddenInstanceProjectIdsInitialized.value = true
}

watch(
    installedProjectIds,
    (ids) => {
        if (!ids) return
        if (!hiddenInstanceProjectIdsInitialized.value) {
            syncHiddenInstanceProjectIds()
        }
    },
    { immediate: true },
)

watchServerContextChanges()

let initialInstanceFilterPromise: Promise<void> = Promise.resolve()
let initialInstalledProjectsPromise: Promise<boolean> = Promise.resolve(false)

async function refreshInstalledProjectIds() {
    const requestId = ++installedProjectRequestId
    const instanceId = activeInstance.value?.id
    if (!instanceId) {
        installedProjectIds.value = null
        installedProjectIdsInstanceId.value = null
        return false
    }
    const requestInstanceId = instanceId
    if (installedProjectIdsInstanceId.value !== requestInstanceId) {
        installedProjectIds.value = null
        installedProjectIdsInstanceId.value = null
        hiddenInstanceProjectIds.value = new Set()
        hiddenInstanceProjectIdsInitialized.value = false
    }

    if (route.query.from === 'worlds') {
        const worlds = await get_instance_worlds(requestInstanceId).catch(handleError)
        if (!worlds) return false
        if (
            isUnmounted ||
            requestId !== installedProjectRequestId ||
            activeInstance.value?.id !== requestInstanceId
        )
            return false

        const serverProjectIds = worlds
            .filter((w) => w.type === 'server' && 'project_id' in w && w.project_id)
            .map((w) => (w as { project_id: string }).project_id)
        debugLog('installedServerProjectIds loaded', { count: serverProjectIds.length })
        const changed =
            installedProjectIdsInstanceId.value !== requestInstanceId ||
            !sameProjectIds(installedProjectIds.value, serverProjectIds)
        installedProjectIds.value = serverProjectIds
        installedProjectIdsInstanceId.value = requestInstanceId
        return changed
    }

    const ids = await getInstalledProjectIds(requestInstanceId).catch(handleError)
    if (
        ids &&
        !isUnmounted &&
        requestId === installedProjectRequestId &&
        activeInstance.value?.id === requestInstanceId
    ) {
        debugLog('installedProjectIds loaded', { count: ids.length })
        const changed =
            installedProjectIdsInstanceId.value !== requestInstanceId ||
            !sameProjectIds(installedProjectIds.value, ids)
        installedProjectIds.value = ids
        installedProjectIdsInstanceId.value = requestInstanceId
        if (changed) await contentSelection.refreshInstalledIdentities()
        return changed
    }
    return false
}

function sameProjectIds(previous: string[] | null, current: string[]) {
    const currentIds = new Set(current)
    return (
        previous !== null &&
        previous.length === current.length &&
        new Set(previous).size === currentIds.size &&
        previous.every((id) => currentIds.has(id))
    )
}

let instanceContextVersion = 0
async function initInstanceContext() {
    const version = ++instanceContextVersion
    initialInstanceFilterPromise = Promise.resolve()
    initialInstalledProjectsPromise = Promise.resolve(false)
    const requestedInstanceId = route.query.i
    const isCurrent = () =>
        !isUnmounted && version === instanceContextVersion && route.query.i === requestedInstanceId

    debugLog('initInstanceContext', {
        queryI: route.query.i,
        queryAi: route.query.ai,
        querySid: route.query.sid,
        queryWid: route.query.wid,
        queryFrom: route.query.from,
    })
    await initServerContext()
    if (!isCurrent()) return
    await contentSelection.refreshInstances(
        (requestedInstanceId as string | undefined) ?? undefined,
        isCurrent,
    )
    if (!isCurrent()) return

    if (requestedInstanceId) {
        try {
            const selectedInstance = await getInstance(requestedInstanceId as string)
            if (!isCurrent()) return
            instance.value = selectedInstance ?? null
            if (!instance.value) throw new Error('The selected instance could not be loaded.')
        } catch (error) {
            if (!isCurrent()) return
            instance.value = null
            handleError(error)
        }
        if (
            instance.value?.install_stage === 'installed' &&
            (contentSelection.selectedCount.value === 0 ||
                contentSelection.targetInstance.value?.id === instance.value.id)
        ) {
            contentSelection.setTarget(instance.value)
        } else if (
            instance.value?.install_stage === 'installed' &&
            contentSelection.selectedCount.value > 0 &&
            contentSelection.targetInstance.value?.id !== instance.value.id
        ) {
            pendingRouteInstanceSwitch.value = instance.value
        }
        debugLog('instance loaded', {
            name: instance.value?.name,
            loader: instance.value?.loader,
            gameVersion: instance.value?.game_version,
        })

        isServerInstance.value = isServerProjectLink(instance.value?.link)
        const projectId = instanceServerProjectId(instance.value?.link)
        if (projectId) {
            debugLog('checking linked project for server status', projectId)
            initialInstanceFilterPromise = get_project_v3(projectId, 'must_revalidate')
                .then((projectV3) => {
                    if (isCurrent() && projectV3?.minecraft_server != null) {
                        debugLog('instance is a server instance')
                        isServerInstance.value = true
                    }
                })
                .catch((error) => {
                    handleError(error)
                    throw error
                })
        }
    }
    const activeProjectId = instanceServerProjectId(activeInstance.value?.link)
    if (!instance.value && activeProjectId) {
        isServerInstance.value = isServerProjectLink(activeInstance.value?.link)
        initialInstanceFilterPromise = get_project_v3(activeProjectId, 'must_revalidate')
            .then((projectV3) => {
                if (isCurrent()) isServerInstance.value = projectV3?.minecraft_server != null
            })
            .catch((error) => {
                handleError(error)
                throw error
            })
    }

    if (route.query.ai && !(route.params.projectType === 'modpack')) {
        debugLog('setting instanceHideInstalled from query', route.query.ai)
        instanceHideInstalled.value = route.query.ai === 'true'
    }
    initialInstalledProjectsPromise = refreshInstalledProjectIds()
    await initialInstanceFilterPromise
}

const instanceFilters = computed(() => {
    const filters = []

    if (activeInstance.value) {
        const gameVersion = activeInstance.value.game_version
        if (gameVersion && usesTargetGameVersion(projectType.value)) {
            filters.push({ type: 'game_version', option: gameVersion })
        }

        const platform = activeInstance.value.loader
        const supportedModLoaders = ['fabric', 'forge', 'quilt', 'neoforge']

        if (platform && projectType.value === 'mod' && supportedModLoaders.includes(platform)) {
            filters.push({ type: 'mod_loader', option: platform })
        }

        if (isServerInstance.value) {
            filters.push({ type: 'environment', option: 'client' })
        }

        if (instanceHideInstalled.value && hiddenInstanceProjectIds.value.size > 0) {
            for (const id of hiddenInstanceProjectIds.value) {
                filters.push({ type: 'project_id', option: `project_id:${id}`, negative: true })
            }
        }
    }

    return filters
})

const serverHideInstalled = ref(false)
const hideSelectedServerInstalls = ref(false)
if (route.query.shi) {
    serverHideInstalled.value = route.query.shi === 'true'
}
const hiddenServerContentProjectIds = ref<Set<string>>(new Set())
const hiddenServerContentProjectIdsInitialized = ref(false)

function shouldHideInstalledProject(projectId: string): boolean {
    if (isServerContext.value) {
        return serverHideInstalled.value && hiddenServerContentProjectIds.value.has(projectId)
    }
    return (
        !!activeInstance.value &&
        instanceHideInstalled.value &&
        allInstalledIds.value.has(projectId)
    )
}

function syncHiddenServerContentProjectIds() {
    hiddenServerContentProjectIds.value = new Set(serverContentProjectIds.value)
    hiddenServerContentProjectIdsInitialized.value = true
}

watch(
    serverContentProjectIds,
    () => {
        if (!hiddenServerContentProjectIdsInitialized.value) {
            syncHiddenServerContentProjectIds()
        }
    },
    { immediate: true },
)

const serverContextFilters = computed(() => {
    const filters: { type: string; option: string; negative?: boolean }[] = []
    if (!serverContextServerData.value) return filters
    const pt = projectType.value

    if (pt !== 'modpack') {
        const gameVersion = serverContextServerData.value.mc_version
        if (gameVersion && usesTargetGameVersion(pt)) {
            filters.push({ type: 'game_version', option: gameVersion })
        }

        const platform = serverContextServerData.value.loader?.toLowerCase()
        if (platform && ['fabric', 'forge', 'quilt', 'neoforge'].includes(platform))
            filters.push({ type: 'mod_loader', option: platform })
        if (platform && ['paper', 'purpur'].includes(platform))
            filters.push({ type: 'plugin_loader', option: platform })

        if (pt === 'mod') filters.push({ type: 'environment', option: 'server' })

        if (hideSelectedServerInstalls.value && queuedServerInstallProjectIds.value.size > 0) {
            for (const id of queuedServerInstallProjectIds.value) {
                filters.push({ type: 'project_id', option: `project_id:${id}`, negative: true })
            }
        }
    }

    if (pt === 'modpack') {
        filters.push(
            { type: 'environment', option: 'client' },
            { type: 'environment', option: 'server' },
        )
    }

    if (serverHideInstalled.value && hiddenServerContentProjectIds.value.size > 0) {
        for (const id of hiddenServerContentProjectIds.value) {
            filters.push({ type: 'project_id', option: `project_id:${id}`, negative: true })
        }
    }

    return filters
})

const combinedProvidedFilters = computed(() => {
    if (contentSource.value === 'mcarchive') return []
    if (isServerContext.value) return serverContextFilters.value
    if (projectType.value === 'modpack' || projectType.value === 'server') return []
    return instanceFilters.value
})

const {
    serverPings,
    contextMenuRef,
    updateServerHits,
    getServerModpackContent,
    getServerCardActions,
    handleRightClick,
    handleOptionsClick,
} = useAppServerBrowse({
    instance,
    isFromWorlds,
    allInstalledIds,
    newlyInstalled,
    installingServerProjects,
    playServerProject,
    showAddServerToInstanceModal,
    handleError,
    router,
})

const { offline } = useNetworkStatus()

const messages = defineMessages({
    serverSourceUnavailable: {
        id: 'app.browse.server-source-unavailable',
        defaultMessage: 'The server content source is unavailable. Try again later.',
    },
    filterMetadataFailed: {
        id: 'app.browse.filter-metadata-failed',
        defaultMessage: 'Filter options could not be loaded.',
    },
    categoriesFailed: {
        id: 'app.browse.categories-failed',
        defaultMessage: 'CurseForge categories could not be loaded. Try again later.',
    },
    confirmedSource: {
        id: 'app.resources.search.confirmed-source',
        defaultMessage: 'Open the confirmed {source} project',
    },
    candidateSource: {
        id: 'app.resources.search.candidate-source',
        defaultMessage: 'Open the {source} candidate. Its identity is not yet verified.',
    },
    sourceWindowExhausted: {
        id: 'app.resources.search.window-exhausted',
        defaultMessage: '{source} has reached its search result window.',
    },
    add: {
        id: 'app.browse.add',
        defaultMessage: 'Add',
    },
    chooseInstance: {
        id: 'app.browse.choose-instance',
        defaultMessage: 'Choose instance',
    },
    installSelected: {
        id: 'app.browse.install-selected',
        defaultMessage: 'Install {count} content',
    },
    preparingSelected: {
        id: 'app.browse.preparing-selected',
        defaultMessage: 'Preparing {completed}/{total}',
    },
    selected: {
        id: 'app.browse.selected',
        defaultMessage: 'Selected',
    },
    addToFavorites: {
        id: 'app.content-favorites.add',
        defaultMessage: 'Add to favorites',
    },
    removeFromFavorites: {
        id: 'app.content-favorites.remove',
        defaultMessage: 'Remove from favorites',
    },
    noCompatibleVersion: {
        id: 'app.browse.no-compatible-version',
        defaultMessage: 'No compatible version was found for the selected instance.',
    },
    addServersToInstance: {
        id: 'app.browse.add-servers-to-instance',
        defaultMessage: 'Adding server to instance',
    },
    addToAnInstance: {
        id: 'app.browse.add-to-an-instance',
        defaultMessage: 'Add to an instance',
    },
    discoverContent: {
        id: 'app.browse.discover-content',
        defaultMessage: 'Discover content',
    },
    discoverServers: {
        id: 'app.browse.discover-servers',
        defaultMessage: 'Discover servers',
    },
    discoverMaps: {
        id: 'app.browse.discover-maps',
        defaultMessage: 'Discover maps',
    },
    mapsProjectType: {
        id: 'app.browse.project-type.maps',
        defaultMessage: 'Maps',
    },
    mapsUnavailable: {
        id: 'app.browse.maps-unavailable',
        defaultMessage: 'Maps unavailable',
    },
    mapsUnavailableDescription: {
        id: 'app.browse.maps-unavailable-description',
        defaultMessage: 'Configure a CurseForge API key to browse and install maps.',
    },

    fuzzySearchPrefix: {
        id: 'app.browse.fuzzy-search.prefix',
        defaultMessage: 'No results found for “{original}”. Showing results for',
    },
    fuzzySearchSuffix: {
        id: 'app.browse.fuzzy-search.suffix',
        defaultMessage: 'instead',
    },
    environmentProvidedByServer: {
        id: 'search.filter.locked.server-environment.title',
        defaultMessage: 'Only client-side mods can be added to the server instance',
    },
    gameVersionProvidedByInstance: {
        id: 'search.filter.locked.instance-game-version.title',
        defaultMessage: 'Game version is provided by the instance',
    },
    gameVersionProvidedByServer: {
        id: 'search.filter.locked.server-game-version.title',
        defaultMessage: 'Game version is provided by the server',
    },
    hideAddedServers: {
        id: 'app.browse.hide-added-servers',
        defaultMessage: 'Hide servers already added',
    },
    installingToServer: {
        id: 'app.browse.server.installing',
        defaultMessage: 'Installing',
    },
    backToInstance: {
        id: 'app.browse.back-to-instance',
        defaultMessage: 'Back to instance',
    },
    serverInstanceContentWarning: {
        id: 'app.browse.server-instance-content-warning',
        defaultMessage:
            'Adding content can break compatibility when joining the server. Any added content will also be lost when you update the server instance content.',
    },
    modLoaderProvidedByInstance: {
        id: 'search.filter.locked.instance-loader.title',
        defaultMessage: 'Loader is provided by the instance',
    },
    modpacksProjectType: {
        id: 'app.browse.project-type.modpacks',
        defaultMessage: 'Modpacks',
    },
    modsProjectType: {
        id: 'app.browse.project-type.mods',
        defaultMessage: 'Mods',
    },
    resourcepacksProjectType: {
        id: 'app.browse.project-type.resourcepacks',
        defaultMessage: 'Resource Packs',
    },
    datapacksProjectType: {
        id: 'app.browse.project-type.datapacks',
        defaultMessage: 'Data Packs',
    },
    shadersProjectType: {
        id: 'app.browse.project-type.shaders',
        defaultMessage: 'Shaders',
    },
    serversProjectType: {
        id: 'app.browse.project-type.servers',
        defaultMessage: 'Servers',
    },
    favoritesProjectType: {
        id: 'app.browse.project-type.favorites',
        defaultMessage: 'Favorites',
    },
    modLoaderProvidedByServer: {
        id: 'search.filter.locked.server-loader.title',
        defaultMessage: 'Loader is provided by the server',
    },
    providedByInstance: {
        id: 'search.filter.locked.instance',
        defaultMessage: 'Provided by the instance',
    },
    providedByServer: {
        id: 'search.filter.locked.server',
        defaultMessage: 'Provided by the server',
    },
    syncFilterButton: {
        id: 'search.filter.locked.instance.sync',
        defaultMessage: 'Sync with instance',
    },
    allSources: {
        id: 'app.browse.source.all',
        defaultMessage: 'All sources',
    },
    compactListView: {
        id: 'app.browse.display-mode.compact-list',
        defaultMessage: 'Compact list',
    },
    gridView: {
        id: 'app.browse.display-mode.grid',
        defaultMessage: 'Grid',
    },
    listView: {
        id: 'app.browse.display-mode.list',
        defaultMessage: 'List',
    },
    modrinthSource: {
        id: 'app.browse.source.modrinth',
        defaultMessage: 'Modrinth',
    },
    switchView: {
        id: 'app.browse.display-mode.switch',
        defaultMessage: 'Switch view',
    },
    curseForgeSource: {
        id: 'app.browse.source.curseforge',
        defaultMessage: 'CurseForge',
    },
    mcArchiveSource: {
        id: 'app.browse.source.mcarchive',
        defaultMessage: 'MCArchive',
    },
    planetMinecraftSource: {
        id: 'app.browse.source.planet-minecraft',
        defaultMessage: 'Planet Minecraft',
    },
    translateProject: {
        id: 'app.project.translation.translate',
        defaultMessage: 'Translate',
    },
    showOriginal: {
        id: 'app.project.translation.show-original',
        defaultMessage: 'Show original',
    },
    translating: {
        id: 'app.project.translation.translating',
        defaultMessage: 'Translating…',
    },
})

const sourceIcon = computed(() => {
    if (!sourceOptions.value.some((option) => option.id === contentSource.value)) {
        return GlobeIcon
    }
    switch (contentSource.value) {
        case 'modrinth':
            return ModrinthIcon
        case 'curseforge':
            return CurseForgeIcon
        case 'mcarchive':
            return FileArchiveIcon
        case 'planet_minecraft':
            return GlobeIcon
        default:
            return GlobeIcon
    }
})

const sourceOptions = computed(() =>
    isWorldMapBrowse.value
        ? [{ id: 'curseforge' as const, label: messages.curseForgeSource, icon: CurseForgeIcon }]
        : [
              { id: 'all' as const, label: messages.allSources, icon: GlobeIcon },
              { id: 'modrinth' as const, label: messages.modrinthSource, icon: ModrinthIcon },
              ...(curseForgeCapability.value.configured
                  ? [
                        {
                            id: 'curseforge' as const,
                            label: messages.curseForgeSource,
                            icon: CurseForgeIcon,
                        },
                    ]
                  : []),
              ...(projectType.value === 'mod' && mcArchiveAvailable.value
                  ? [
                        {
                            id: 'mcarchive' as const,
                            label: messages.mcArchiveSource,
                            icon: FileArchiveIcon,
                        },
                    ]
                  : []),
              ...(projectType.value === 'mod' && planetMinecraftAvailable.value
                  ? [
                        {
                            id: 'planet_minecraft' as const,
                            label: messages.planetMinecraftSource,
                            icon: GlobeIcon,
                        },
                    ]
                  : []),
          ],
)

const currentSourceLabel = computed(() => {
    const current = sourceOptions.value.find((opt) => opt.id === contentSource.value)
    return current?.label ?? messages.allSources
})

const breadcrumbs = useBreadcrumbs()
const browseTitle = computed(() =>
    formatMessage(
        isFromWorlds.value
            ? messages.discoverServers
            : isWorldMapBrowse.value
              ? messages.discoverMaps
              : messages.discoverContent,
    ),
)
watch(
    browseTitle,
    (title) => {
        breadcrumbs.setName('BrowseTitle', title)
    },
    { immediate: true },
)
watch(
    instance,
    () => {
        if (instance.value) {
            const instanceLink = `/instance/${encodeURIComponent(instance.value.id)}`
            const instanceIcon = getDisplayInstanceIcon(
                instance.value.icon_path,
                instance.value.loader,
            ).url
            breadcrumbs.setContext({
                name: instance.value.name,
                link:
                    isFromWorlds.value || isWorldMapContext.value
                        ? `${instanceLink}/worlds`
                        : instanceLink,
                iconUrl: instanceIcon,
            })
        } else {
            breadcrumbs.setContext(null)
        }
    },
    { immediate: true },
)

onBeforeRouteLeave((to) => {
    try {
        const currentPage = searchState.committed.value?.page ?? searchState.currentPage.value
        if (isBrowseReturnSourcePath(to.path)) {
            const viewport = document.querySelector<HTMLElement>('.app-viewport')
            navReturn.saveBrowseReturnSnapshot({
                url: route.fullPath,
                scrollTop: viewport?.scrollTop ?? 0,
                state: { currentPage },
            })
        }

        breadcrumbs.setContext({
            name: '?BrowseTitle',
            link: `/browse/${projectType.value}`,
            query: route.query,
        })
    } catch (error) {
        // Never abort leave navigation because bookkeeping failed.
        console.warn('[browse] onBeforeRouteLeave bookkeeping failed', error)
    }
})

function resetInstanceContext() {
    if (!instance.value) return

    debugLog('instance context removed, resetting')
    instance.value = null
    installedProjectRequestId += 1
    installedProjectIds.value = null
    installedProjectIdsInstanceId.value = null
    instanceHideInstalled.value = false
    newlyInstalled.value = []
    hiddenInstanceProjectIds.value = new Set()
    hiddenInstanceProjectIdsInitialized.value = false
    isServerInstance.value = false
    breadcrumbs.setName('BrowseTitle', formatMessage(messages.discoverContent))
    breadcrumbs.setContext(null)
}

watch(
    () => route.params.projectType as ProjectType,
    async (newType) => {
        if (isSetupServerContext.value) {
            enforceSetupModpackRoute(newType)
            if (newType !== 'modpack') return
        }

        if (!newType || newType === projectType.value) return

        debugLog('projectType route param changed', { from: projectType.value, to: newType })
        projectType.value = newType
        // SPA tab switch reuses this instance; remount used to reset scroll.
        document.querySelector('.app-viewport')?.scrollTo({ top: 0 })
    },
)

watch(
    () => route.query.i,
    (instanceId) => {
        if (!instanceId && route.path.startsWith('/browse')) {
            resetInstanceContext()
        }
    },
)

const selectableProjectTypes = computed(() => {
    const params: LocationQuery = {}

    if (route.query.i) params.i = route.query.i
    if (route.query.ai) params.ai = route.query.ai
    if (route.query.from) params.from = route.query.from
    if (route.query.sid) params.sid = route.query.sid
    if (effectiveServerWorldId.value) params.wid = effectiveServerWorldId.value

    const queryString = new URLSearchParams(params as Record<string, string>).toString()
    const suffix = queryString ? `?${queryString}` : ''

    if (isSetupServerContext.value) {
        return [
            {
                label: formatMessage(messages.modpacksProjectType),
                href: `/browse/modpack${suffix}`,
            },
        ]
    }

    if (isFromWorlds.value) {
        return [
            { label: formatMessage(messages.serversProjectType), href: `/browse/server${suffix}` },
        ]
    }
    if (isWorldMapContext.value) {
        return [
            {
                label: formatMessage(messages.mapsProjectType),
                href: `/browse/${WORLD_BROWSE_PROJECT_TYPE}${suffix}`,
            },
        ]
    }

    return createBrowseProjectTabs(
        {
            modpacks: formatMessage(messages.modpacksProjectType),
            mods: formatMessage(messages.modsProjectType),
            resourcepacks: formatMessage(messages.resourcepacksProjectType),
            datapacks: formatMessage(messages.datapacksProjectType),
            maps: formatMessage(messages.mapsProjectType),
            shaders: formatMessage(messages.shadersProjectType),
            servers: formatMessage(messages.serversProjectType),
            favorites: formatMessage(messages.favoritesProjectType),
        },
        suffix,
        getBrowseProjectTabOptions({
            instance: activeInstance.value,
            hasInstanceContext: !!instance.value,
            isServerInstance: isServerInstance.value,
        }),
    )
})

const installContext = computed(() => {
    if (isServerContext.value && serverContextServerData.value) {
        return {
            name: serverContextServerData.value.name,
            loader: serverContextServerData.value.loader ?? '',
            gameVersion: serverContextServerData.value.mc_version ?? '',
            serverId: serverIdQuery.value,
            upstream: serverContextServerData.value.upstream,
            iconSrc: null as string | null,
            isMedal: serverContextServerData.value.is_medal,
            backUrl: serverBackUrl.value,
            backLabel: serverBackLabel.value,
            heading: serverBrowseHeading.value,
            queuedCount: queuedServerInstallCount.value,
            selectedProjects: selectedServerInstallProjects.value,
            isInstallingSelected: isInstallingQueuedServerInstalls.value,
            skipNonEssentialWarnings: themeStore.getFeatureFlag('skip_non_essential_warnings'),
            installProgress: queuedInstallProgress.value,
            clearQueued: clearQueuedServerInstalls,
            clearSelected: clearQueuedServerInstalls,
            onBack: flushQueuedServerInstalls,
            discardSelectedAndBack: discardQueuedServerInstallsAndBack,
            installSelected: installQueuedServerInstallsAndBack,
        }
    }
    if (activeInstance.value) {
        const target = activeInstance.value
        const displayIcon = getDisplayInstanceIcon(target.icon_path, target.loader)
        const processing = ['validating', 'reviewing', 'queueing'].includes(
            contentSelection.state.value,
        )
        return {
            showInstallHeader: !!instance.value,
            name: target.name,
            loader: target.loader,
            gameVersion: target.game_version,
            iconSrc: displayIcon.url,
            iconFrameless: displayIcon.frameless,
            backUrl: instance.value
                ? `/instance/${encodeURIComponent(instance.value.id)}${isFromWorlds.value || isWorldMapContext.value ? '/worlds' : ''}`
                : route.fullPath,
            backLabel: formatMessage(messages.backToInstance),
            heading: formatMessage(
                isFromWorlds.value
                    ? messages.addServersToInstance
                    : commonMessages.installingContentLabel,
            ),
            warning:
                isServerInstance.value && !isFromWorlds.value
                    ? formatMessage(messages.serverInstanceContentWarning)
                    : undefined,
            selectedProjects: contentSelection.selectedProjects.value,
            isInstallingSelected: processing,
            installProgress: contentSelection.progress.value,
            installButtonLabel: formatMessage(messages.installSelected, {
                count: contentSelection.selectedCount.value,
            }),
            processingLabel: formatMessage(messages.preparingSelected, {
                completed: contentSelection.progress.value.completed,
                total: contentSelection.progress.value.total,
            }),
            clearSelected: contentSelection.clear,
            installSelected: contentSelection.installSelected,
        }
    }
    return null
})

const installingProjectIds = ref<Set<string>>(new Set())
const CART_CONTENT_TYPES = new Set(['mod', 'resourcepack', 'datapack', 'shader', 'world'])

function projectInstallingKey(projectId: string, instanceId?: string | null) {
    return `${instanceId ?? activeInstance.value?.id ?? ''}\0${projectId}`
}

function setProjectInstalling(projectId: string, installing: boolean, instanceId?: string | null) {
    const key = projectInstallingKey(projectId, instanceId)
    const next = new Set(installingProjectIds.value)
    if (installing) {
        next.add(key)
    } else {
        next.delete(key)
    }
    installingProjectIds.value = next
}

async function selectTargetInstance(target: GameInstance) {
    contentSelection.setTarget(target)
    newlyInstalled.value = []
    hiddenInstanceProjectIds.value = new Set()
    hiddenInstanceProjectIdsInitialized.value = false
    if (route.query.i) {
        instance.value = target
        await router.replace({ query: { ...route.query, i: target.id } })
        breadcrumbs.setContext({
            name: target.name,
            link: `/instance/${encodeURIComponent(target.id)}`,
            iconUrl: getDisplayInstanceIcon(target.icon_path, target.loader).url,
        })
    }
    await refreshInstalledProjectIds()
    await searchState.refreshSearch()
}

async function cancelTargetInstanceSwitch() {
    const target = contentSelection.targetInstance.value
    if (!target || !route.query.i || route.query.i === target.id) return
    instance.value = target
    await router.replace({ query: { ...route.query, i: target.id } })
    breadcrumbs.setContext({
        name: target.name,
        link: `/instance/${encodeURIComponent(target.id)}`,
        iconUrl: getDisplayInstanceIcon(target.icon_path, target.loader).url,
    })
}

async function toggleContentSelection(
    project: (Labrinth.Search.v2.ResultSearchProject & Labrinth.Search.v3.ResultSearchProject) & {
        provider: BrowseContentProvider
        provider_project_id?: string
        latest_version?: string | null
    },
    contentType: string,
) {
    const target = activeInstance.value
    if (!target) {
        instanceSelector.value?.show()
        return
    }
    const providerProjectId =
        project.provider === 'curseforge'
            ? (project.provider_project_id ?? project.project_id.replace(/^curseforge:/, ''))
            : project.project_id
    const key = makeContentSelectionKey(project.provider, providerProjectId)
    if (contentSelection.isSelected(key)) {
        contentSelection.remove(key)
        return
    }

    const preferences = getInstanceInstallTargetPreferences(contentType)
    const added = await contentSelection.add({
        key,
        provider: project.provider,
        projectId: project.provider === 'modrinth' ? project.project_id : providerProjectId,
        providerProjectId,
        contentType: contentType as 'mod' | 'resourcepack' | 'datapack' | 'shader' | 'world',
        title: project.title,
        iconUrl: project.icon_url,
        slug: project.slug,
        preferences,
    })

    if (!added) return
}

const serverInstallQueue = {
    get: getQueuedServerInstallPlans,
    set: setQueuedServerInstallPlans,
}

function getCurrentSelectedInstallPreferences(projectTypeValue: string) {
    return getSelectedInstallPreferences({
        contentType: projectTypeValue,
        selectedFilters: searchState.currentFilters.value,
        providedFilters: combinedProvidedFilters.value,
        overriddenProvidedFilterTypes: searchState.overriddenProvidedFilterTypes.value,
    })
}

function getServerInstallTargetPreferences(contentType: BrowseInstallContentType) {
    return getTargetInstallPreferences(
        {
            gameVersion: serverContextServerData.value?.mc_version,
            loader: serverContextServerData.value?.loader,
        },
        contentType,
    )
}

function getInstanceInstallTargetPreferences(projectTypeValue: string) {
    return getTargetInstallPreferences(
        {
            gameVersion: activeInstance.value?.game_version,
            loader: activeInstance.value?.loader,
        },
        projectTypeValue,
    )
}

async function getInstallProjectVersions(projectId: string) {
    const project = await get_project(projectId, 'must_revalidate')
    return (await get_version_many(
        project.versions,
        'must_revalidate',
    )) as Labrinth.Versions.v2.Version[]
}

async function chooseInstanceInstallVersion(
    project: Labrinth.Search.v2.ResultSearchProject & Labrinth.Search.v3.ResultSearchProject,
    projectTypeValue: string,
) {
    const targetInstance = activeInstance.value
    if (!targetInstance) {
        return { versionId: null as string | null }
    }

    const selectedPreferences = getCurrentSelectedInstallPreferences(projectTypeValue)
    const targetPreferences = getInstanceInstallTargetPreferences(projectTypeValue)
    if (!preferencesDiffer(selectedPreferences, targetPreferences)) {
        return { versionId: null as string | null }
    }

    const selectedVersion = getLatestMatchingInstallVersion(
        await getInstallProjectVersions(project.project_id),
        selectedPreferences,
    )

    if (!selectedVersion) {
        return { versionId: null as string | null }
    }

    return { versionId: selectedVersion.id }
}

type BrowseContentProvider = 'modrinth' | 'curseforge' | 'mcarchive' | 'planet_minecraft'

function isBrowseContentProvider(provider: unknown): provider is BrowseContentProvider {
    return (
        provider === 'modrinth' ||
        provider === 'curseforge' ||
        provider === 'mcarchive' ||
        provider === 'planet_minecraft'
    )
}

function getPrimaryCardActions(
    result: Labrinth.Search.v2.ResultSearchProject | Labrinth.Search.v3.ResultSearchProject,
    currentProjectType: string,
): CardAction[] {
    if (currentProjectType === 'server') {
        return getServerCardActions(result as Labrinth.Search.v3.ResultSearchProject)
    }

    // Non-server project actions
    const projectResult = result as (Labrinth.Search.v2.ResultSearchProject &
        Labrinth.Search.v3.ResultSearchProject) & {
        installed?: boolean
        installing?: boolean
        provider?: unknown
        provider_project_id?: string
    }
    if (!isBrowseContentProvider(projectResult.provider)) return []
    if (projectResult.provider === 'mcarchive' || projectResult.provider === 'planet_minecraft')
        return !isServerContext.value && isFavoriteContentType(currentProjectType)
            ? [
                  createFavoriteCardAction(
                      projectResult,
                      projectResult.provider_project_id ??
                          projectResult.project_id.split(':').slice(1).join(':'),
                      currentProjectType,
                  ),
              ]
            : []
    const providerProjectId =
        projectResult.provider === 'curseforge'
            ? (projectResult.provider_project_id ??
              projectResult.project_id.replace(/^curseforge:/, ''))
            : projectResult.project_id
    const selectionKey = makeContentSelectionKey(projectResult.provider, providerProjectId)
    const isInstalling =
        installingProjectIds.value.has(projectInstallingKey(projectResult.project_id)) ||
        contentSelection.isInstalling(selectionKey)
    const isSelected = contentSelection.isSelected(selectionKey)
    const isInstalled =
        (isServerContext.value
            ? serverContentProjectIds.value.has(projectResult.project_id || '')
            : !!activeInstance.value &&
              allInstalledIds.value.has(projectResult.project_id || '')) ||
        (!isServerContext.value &&
            !!activeInstance.value &&
            contentSelection.isInstalledIdentity(
                projectResult.provider,
                providerProjectId,
                projectResult.slug,
            )) ||
        (isServerContext.value &&
            serverContextServerData.value?.upstream?.project_id === projectResult.project_id)
    const favoriteAction =
        !isServerContext.value && isFavoriteContentType(currentProjectType)
            ? createFavoriteCardAction(projectResult, providerProjectId, currentProjectType)
            : null

    if (
        isServerContext.value &&
        projectResult.provider === 'modrinth' &&
        ['modpack', 'mod', 'plugin', 'datapack'].includes(currentProjectType)
    ) {
        const isQueued = queuedServerInstallProjectIds.value.has(projectResult.project_id)
        const isInstallingSelection = isInstallingQueuedServerInstalls.value
        const validatingInstall =
            isInstalling && currentProjectType !== 'modpack' && !isInstallingSelection
        const installLabel = isInstalled
            ? commonMessages.installedLabel
            : isQueued
              ? isInstalling || isInstallingSelection
                  ? validatingInstall
                      ? commonMessages.validatingLabel
                      : messages.installingToServer
                  : commonMessages.selectedLabel
              : isInstalling || isInstallingSelection
                ? validatingInstall
                    ? commonMessages.validatingLabel
                    : messages.installingToServer
                : commonMessages.installButton
        return [
            {
                key: 'install',
                label: formatMessage(installLabel),
                icon:
                    isInstalling || isInstallingSelection
                        ? SpinnerIcon
                        : isQueued || isInstalled
                          ? CheckIcon
                          : PlusIcon,
                iconClass: isInstalling || isInstallingSelection ? 'animate-spin' : undefined,
                disabled: isInstalled || isInstalling || isInstallingSelection,
                color: isQueued && !isInstalling && !isInstallingSelection ? 'green' : 'brand',
                type: 'outlined',
                onClick: async () => {
                    const installInstanceId = activeInstance.value?.id ?? null
                    if (isQueued) {
                        removeQueuedServerInstall(projectResult.project_id)
                        return
                    }

                    const contentType = currentProjectType as BrowseInstallContentType
                    const isModpack = contentType === 'modpack'
                    const shouldShowInstalling = isModpack || !isQueued
                    if (shouldShowInstalling) {
                        setProjectInstalling(projectResult.project_id, true, installInstanceId)
                    }
                    try {
                        await requestInstall({
                            project: projectResult,
                            contentType,
                            mode: isModpack ? 'immediate' : 'queue',
                            selectedFilters: isModpack
                                ? []
                                : stripServerRuntimeInstallFilters(
                                      searchState.currentFilters.value,
                                  ),
                            providedFilters: isModpack ? [] : combinedProvidedFilters.value,
                            overriddenProvidedFilterTypes: isModpack
                                ? []
                                : stripServerRuntimeInstallOverrides(
                                      searchState.overriddenProvidedFilterTypes.value,
                                  ),
                            targetPreferences: getServerInstallTargetPreferences(contentType),
                            getProjectVersions: getInstallProjectVersions,
                            queue: serverInstallQueue,
                            install: (plan) =>
                                openServerModpackInstallFlow({
                                    projectId: plan.projectId,
                                    versionId: plan.versionId,
                                    name: plan.project.name,
                                    iconUrl: plan.project.icon_url ?? undefined,
                                }),
                        })
                    } catch (err) {
                        handleError(err as Error)
                    } finally {
                        if (shouldShowInstalling) {
                            setProjectInstalling(projectResult.project_id, false, installInstanceId)
                        }
                    }
                },
            },
            ...(favoriteAction ? [favoriteAction] : []),
        ]
    }

    const isModpack =
        projectResult.project_types?.includes('modpack') || projectResult.project_type === 'modpack'
    if (CART_CONTENT_TYPES.has(currentProjectType)) {
        if (
            currentProjectType === WORLD_BROWSE_PROJECT_TYPE &&
            projectResult.provider !== 'curseforge'
        ) {
            return []
        }
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
                onClick: async () => {
                    try {
                        await toggleContentSelection(projectResult, currentProjectType)
                    } catch (error) {
                        handleError(error)
                    }
                },
            },
            ...(favoriteAction ? [favoriteAction] : []),
        ]
    }
    const shouldUseInstallIcon = !!instance.value || isModpack
    const installActionLabel = isInstalling
        ? messages.installingToServer
        : isInstalled
          ? commonMessages.installedLabel
          : shouldUseInstallIcon
            ? commonMessages.installButton
            : messages.addToAnInstance
    const compactInstallLabel =
        !isInstalling && !isInstalled && !shouldUseInstallIcon
            ? formatMessage(messages.add)
            : undefined

    return [
        {
            key: 'install',
            label: formatMessage(installActionLabel),
            compactLabel: compactInstallLabel,
            icon: isInstalling ? SpinnerIcon : isInstalled ? CheckIcon : PlusIcon,
            iconClass: isInstalling ? 'animate-spin' : undefined,
            disabled: isInstalled || isInstalling,
            color: 'brand',
            type: 'outlined',
            onClick: async () => {
                const installInstanceId = instance.value?.id ?? null
                setProjectInstalling(projectResult.project_id, true, installInstanceId)
                try {
                    const selectedInstall =
                        instance.value && projectResult.provider === 'modrinth'
                            ? await chooseInstanceInstallVersion(projectResult, currentProjectType)
                            : { versionId: null as string | null }
                    if (selectedInstall === null) {
                        setProjectInstalling(projectResult.project_id, false, installInstanceId)
                        return
                    }
                    const selectedPreferences =
                        getCurrentSelectedInstallPreferences(currentProjectType)
                    const installContent =
                        projectResult.provider === 'curseforge'
                            ? installCurseForge
                            : projectResult.provider === 'modrinth'
                              ? installVersion
                              : null
                    if (!installContent) return
                    await installContent(
                        projectResult.provider_project_id ?? projectResult.project_id,
                        selectedInstall.versionId,
                        instance.value ? instance.value.id : null,
                        'SearchCard',
                        (versionId, installedProjectIds) => {
                            setProjectInstalling(projectResult.project_id, false, installInstanceId)
                            if (versionId && activeInstance.value?.id === installInstanceId) {
                                onSearchResultsInstalled(
                                    installedProjectIds ?? [projectResult.project_id],
                                )
                            }
                        },
                        (profile) => {
                            router.push(isModpack ? '/downloads' : `/instance/${profile}`)
                        },
                        {
                            preferredLoader:
                                instance.value?.loader ?? selectedPreferences.loaders?.[0],
                            preferredGameVersion:
                                instance.value?.game_version ??
                                selectedPreferences.gameVersions?.[0],
                        },
                    )
                } catch (err) {
                    setProjectInstalling(projectResult.project_id, false, installInstanceId)
                    handleError(err)
                }
            },
        },
    ]
}

function getCardActions(
    result: Labrinth.Search.v2.ResultSearchProject | Labrinth.Search.v3.ResultSearchProject,
    currentProjectType: string,
): CardAction[] {
    return getPrimaryCardActions(result, currentProjectType)
}

function createFavoriteCardAction(
    project: Labrinth.Search.v2.ResultSearchProject & {
        provider: BrowseContentProvider
    },
    providerProjectId: string,
    contentType: FavoriteContentType,
): CardAction {
    const favorite = contentFavorites.isFavorite(project.provider, providerProjectId)
    const pending = contentFavorites.isPending(project.provider, providerProjectId)
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
            try {
                await contentFavorites.toggle({
                    provider: project.provider,
                    project_id: providerProjectId,
                    content_type: contentType,
                    slug: project.slug,
                })
            } catch (error) {
                handleError(error)
            }
        },
    }
}

function onSearchResultInstalled(id: string) {
    if (isServerContext.value) {
        markServerProjectInstalled(id)
        return
    }
    if (!newlyInstalled.value.includes(id)) {
        newlyInstalled.value = [...newlyInstalled.value, id]
    }
}

function onSearchResultsInstalled(ids: string[]) {
    if (isServerContext.value) {
        for (const id of ids) {
            markServerProjectInstalled(id)
        }
        return
    }
    newlyInstalled.value = Array.from(new Set([...newlyInstalled.value, ...ids]))
}

function extractQuotedFilterValues(source: string): string[] {
    return [...source.matchAll(/[`"]([^`"]+)[`"]/g)].map((match) => match[1])
}

function getSearchFilterValues(filters: string, field: string) {
    const values: string[] = []
    const pattern = new RegExp(
        `${field}\\s*(?:=\\s*[\`"]([^\`"]+)[\`"]|IN\\s*\\[([^\\]]+)\\])`,
        'g',
    )
    for (const match of filters.matchAll(pattern)) {
        if (match[1]) {
            values.push(match[1])
        } else if (match[2]) {
            values.push(...extractQuotedFilterValues(match[2]))
        }
    }
    return values
}

const CURSEFORGE_LOADER_SLUGS = new Set([
    'forge',
    'fabric',
    'quilt',
    'neoforge',
    'liteloader',
    'bukkit',
    'spigot',
    'paper',
    'bungeecord',
    'velocity',
    'sponge',
    'waterfall',
    'folia',
    'purpur',
    'iris',
    'optifine',
    'canvas',
    'geyser',
])

function resourceEnvironmentSides(environments: string[]): {
    client: Labrinth.Projects.v2.Environment
    server: Labrinth.Projects.v2.Environment
} {
    const sides: Record<
        string,
        { client: Labrinth.Projects.v2.Environment; server: Labrinth.Projects.v2.Environment }
    > = {
        client_only: { client: 'required', server: 'unsupported' },
        server_only: { client: 'unsupported', server: 'required' },
        dedicated_server_only: { client: 'unsupported', server: 'required' },
        client_only_server_optional: { client: 'required', server: 'optional' },
        server_only_client_optional: { client: 'optional', server: 'required' },
        client_and_server: { client: 'required', server: 'required' },
        client_or_server: { client: 'optional', server: 'optional' },
        client_or_server_prefers_both: { client: 'optional', server: 'optional' },
    }
    const values = environments.map(
        (environment) => sides[environment] ?? { client: 'unknown', server: 'unknown' },
    )
    return {
        client:
            values.length && values.every((value) => value.client === values[0].client)
                ? values[0].client
                : 'unknown',
        server:
            values.length && values.every((value) => value.server === values[0].server)
                ? values[0].server
                : 'unknown',
    }
}
const resourceSearchNotice = computed(
    () => searchState.committed.value?.response.resourceNotice ?? '',
)
const resourceSearchPages = new ResourceSearchPages(resourceSearch)

async function search(requestParams: string, signal: AbortSignal) {
    const params = new URLSearchParams(requestParams)
    if (projectType.value === 'server') {
        const requestId = crypto.randomUUID()
        const cancel = () => {
            void cancel_search_request(requestId).catch(handleError)
        }
        signal.addEventListener('abort', cancel, { once: true })
        try {
            const response = await get_search_results_v3(
                requestParams,
                'must_revalidate',
                requestId,
            )
            signal.throwIfAborted()
            const result = response?.result
            if (!result) throw new Error(formatMessage(messages.serverSourceUnavailable))
            updateServerHits(result.hits)
            return {
                projectHits: [],
                serverHits: result.hits,
                total_hits: result.total_hits,
                per_page: result.hits_per_page,
                resourceNotice: '',
            }
        } finally {
            signal.removeEventListener('abort', cancel)
        }
    }
    const limit = Math.min(Number(params.get('limit') ?? 20), 100)
    const filters = params.get('new_filters') ?? ''
    const categories = getSearchFilterValues(filters, 'categories')
    const loaders = categories.filter((value) => CURSEFORGE_LOADER_SLUGS.has(value))
    const typeMap: Record<string, ResourceType> = {
        mod: 'mod',
        plugin: 'plugin',
        resourcepack: 'resourcePack',
        datapack: 'dataPack',
        shader: 'shader',
        modpack: 'modPack',
        world: 'world',
    }
    const sortMap: Record<string, ResourceSearchQuery['sort']> = {
        relevance: 'relevance',
        downloads: 'downloads',
        follows: 'followers',
        newest: 'projectPublished',
        created: 'created',
        updated: 'projectUpdated',
        firstPublished: 'firstPublished',
        latestPublished: 'latestPublished',
    }
    const classId = curseForgeClassIds[projectType.value]
    const categoryFailure =
        classId && categories.some((value) => !CURSEFORGE_LOADER_SLUGS.has(value))
            ? await loadResourceSearchCategories(contentSource.value, () =>
                  ensureCurseForgeCategories(projectType.value),
              )
            : null
    signal.throwIfAborted()
    if (categoryFailure) throw new Error(formatMessage(messages.categoriesFailed))
    const query: ResourceSearchQuery = {
        filter: parseResourceSearchFilter(
            filters,
            (value) => [
                ...(!value.startsWith('cf:') ? [`modrinth:${value}`] : []),
                ...resolveCurseForgeCategoryIdsFromFilterValues(
                    [value],
                    classId ? (curseForgeCategoriesByClass.value[classId] ?? []) : [],
                    new Set(loaders),
                ).map((id) => `curseforge:${id}`),
            ],
            (value) => CURSEFORGE_LOADER_SLUGS.has(value),
        ),
        text: params.get('query') ?? '',
        types: [typeMap[projectType.value]],
        sources: contentSource.value === 'all' ? [] : [contentSource.value],
        target: {
            gameVersions: [],
            loaders: [],
            channels: [],
        },
        categories: [],
        sort: sortMap[params.get('index') ?? 'relevance'] ?? 'relevance',
        pageSize: limit,
    }
    const page = await resourceSearchPages.get(
        query,
        Math.floor(Number(params.get('offset') ?? 0) / limit),
        signal,
    )
    signal.throwIfAborted()
    if (page.entries.length === 0 && page.sourceFailures.length > 0)
        throw new Error(
            page.sourceFailures
                .map((failure) => resourceSearchFailure(failure, query, formatMessage))
                .join('; '),
        )
    const hits = page.entries
        .map(({ entry, projects }) => {
            const metadata = projects.find(
                (project) =>
                    project.project.source === entry.defaultProject.source &&
                    project.project.projectId === entry.defaultProject.projectId,
            )!
            const provider = metadata.project.source
            const id = metadata.project.projectId
            const projectId = provider === 'modrinth' ? id : `${provider}:${id}`
            return {
                project_id: projectId,
                provider_project_id: id,
                provider,
                project_type: projectType.value,
                slug: metadata.slug ?? id,
                title: metadata.title,
                description: metadata.summary,
                author: metadata.authors.join(', '),
                icon_url: metadata.iconUrl ?? '',
                categories: [...metadata.categories, ...metadata.loaders],
                display_categories: metadata.categories,
                versions: metadata.gameVersions,
                downloads: metadata.downloads ?? 0,
                follows: metadata.followers ?? 0,
                date_created: metadata.createdAt ?? '',
                date_modified: metadata.updatedAt ?? '',
                first_published_at: metadata.firstPublishedAt,
                latest_published_at: metadata.latestPublishedAt,
                resource_dates: {
                    createdAt: metadata.createdAt,
                    projectPublishedAt: metadata.projectPublishedAt,
                    updatedAt: metadata.updatedAt,
                    firstPublishedAt: metadata.firstPublishedAt,
                    latestPublishedAt: metadata.latestPublishedAt,
                },
                latest_version: '',
                license: metadata.license ?? '',
                client_side: resourceEnvironmentSides(metadata.environments).client,
                server_side: resourceEnvironmentSides(metadata.environments).server,
                gallery: metadata.gallery,
                featured_gallery: metadata.gallery[0] ?? null,
                color: null,
                resource_entry: entry,
                providers: [...new Set(projects.map((project) => project.project.source))],
                source_candidates: projects.map((project) => ({
                    source: project.project.source,
                    projectId: project.project.projectId,
                    slug: project.slug,
                    confirmed: entry.status === 'confirmed',
                })),
                installed: allInstalledIds.value.has(projectId),
            } as Labrinth.Search.v2.ResultSearchProject & {
                provider: SourceId
                provider_project_id: string
                installed: boolean
            }
        })
        .filter((hit) => !shouldHideInstalledProject(hit.project_id))
    signal.throwIfAborted()
    return {
        projectHits: hits,
        serverHits: [],
        total_hits: page.total.value ?? hits.length,
        total_precision: page.total.precision,
        has_more: page.hasMore,
        per_page: limit,
        page: resourceSearchPages.indexOf(page) + 1,
        resourceNotice: [
            ...page.sourceFailures.map((failure) =>
                resourceSearchFailure(failure, query, formatMessage),
            ),
            ...page.exhaustedSources.map((source) =>
                formatMessage(messages.sourceWindowExhausted, {
                    source: resourceSourceName(source),
                }),
            ),
        ].join('; '),
    }
}

const isServerFilterContext = computed(() => isServerContext.value || isServerInstance.value)

const lockedFilterMessages = computed(() => ({
    gameVersion: formatMessage(
        isServerFilterContext.value
            ? messages.gameVersionProvidedByServer
            : messages.gameVersionProvidedByInstance,
    ),
    modLoader: formatMessage(
        isServerFilterContext.value
            ? messages.modLoaderProvidedByServer
            : messages.modLoaderProvidedByInstance,
    ),
    environment: formatMessage(messages.environmentProvidedByServer),
    syncButton: formatMessage(messages.syncFilterButton),
    providedBy: formatMessage(
        isServerFilterContext.value ? messages.providedByServer : messages.providedByInstance,
    ),
}))

type BrowseReturnState = {
    currentPage: number
}

const browseReturnSnapshot = navReturn.consumeBrowseReturnSnapshot<BrowseReturnState>(
    route.fullPath,
)

const displayMode = ref<BrowseDisplayMode>(getLastBrowseContentDisplayMode())

const displayModeOptions = computed<BrowseDisplayModeOption[]>(() => [
    { id: 'list', label: formatMessage(messages.listView), icon: ListIcon },
    { id: 'compact', label: formatMessage(messages.compactListView), icon: GenericListIcon },
    { id: 'grid', label: formatMessage(messages.gridView), icon: GridIcon },
])

const displayModeTooltip = computed(() => formatMessage(messages.switchView))

function setDisplayMode(mode: BrowseDisplayMode) {
    if (mode === 'gallery') return
    displayMode.value = mode
    setLastBrowseContentDisplayMode(mode)
}

const searchState = useBrowseSearch({
    enabled: searchEnabled,
    deferInitialization: true,
    invalidateSearch: () => resourceSearchPages.invalidate(searchState.currentPage.value - 1),
    formatError: (error) =>
        isResourceOperationError(error)
            ? resourceOperationError(error, formatMessage)
            : error instanceof Error
              ? error.message
              : resourceOperationError(error, formatMessage),
    sortOptions: [
        { name: 'relevance', display: 'Relevance' },
        { name: 'downloads', display: 'Downloads' },
        { name: 'follows', display: 'Followers' },
        { name: 'newest', display: 'Project published' },
        { name: 'created', display: 'Project created' },
        { name: 'updated', display: 'Project modified' },
        { name: 'firstPublished', display: 'First release published' },
        { name: 'latestPublished', display: 'Latest release published' },
    ],
    projectType,
    tags,
    providedFilters: combinedProvidedFilters,
    installContextLoader: computed(() => installContext.value?.loader),
    search: async (requestParams: string, signal: AbortSignal) => {
        const response = await search(requestParams, signal)
        signal.throwIfAborted()
        return response
    },
    persistentQueryParams: ['i', 'ai', 'shi', 'sid', 'wid', 'from', 'source'],
    getExtraQueryParams: () => ({
        sid: serverIdQuery.value || undefined,
        wid: effectiveServerWorldId.value || undefined,
        ai: instanceHideInstalled.value ? 'true' : undefined,
        shi: serverHideInstalled.value ? 'true' : undefined,
        source:
            isWorldMapBrowse.value || contentSource.value === 'all'
                ? undefined
                : contentSource.value,
    }),
    displayMode,
})

function restoreBrowseReturnPage() {
    if (!browseReturnSnapshot || !Number.isInteger(browseReturnSnapshot.state.currentPage)) return

    searchState.currentPage.value = Math.max(1, browseReturnSnapshot.state.currentPage)
}

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

function availableRememberedFilters(type: ProjectType, filters: BrowseFilterMemory['filters']) {
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

function applyRememberedFilters(type: ProjectType, resetWhenMissing: boolean) {
    const memory = getBrowseFilterMemory(type)
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

watch(
    [
        projectType,
        filtersReady,
        browseInitialized,
        searchState.currentFilters,
        searchState.toggledGroups,
        searchState.overriddenProvidedFilterTypes,
        searchState.serverCurrentFilters,
        searchState.serverToggledGroups,
    ],
    () => {
        if (!filtersReady.value || !browseInitialized.value) return
        const isServer = projectType.value === 'server'
        setBrowseFilterMemory(projectType.value, {
            filters: isServer
                ? searchState.serverCurrentFilters.value
                : searchState.currentFilters.value,
            toggledGroups: isServer
                ? searchState.serverToggledGroups.value
                : searchState.toggledGroups.value,
            overriddenProvidedFilterTypes: isServer
                ? []
                : searchState.overriddenProvidedFilterTypes.value,
        })
    },
    { deep: true },
)

/** Translation state for search result titles and descriptions. */
const {
    translationActive,
    translationLoading,
    toggle: toggleResultTranslation,
} = useSearchResultTranslation({
    projectHits: searchState.projectHits,
    serverHits: searchState.serverHits,
    projectType: computed(() => searchState.committed.value?.projectType ?? projectType.value),
    enrichTitles: async (hits, locale) => {
        const localizable = hits.filter(
            (hit): hit is typeof hit & { provider: 'modrinth' | 'curseforge' } =>
                hit.provider === 'modrinth' || hit.provider === 'curseforge',
        )
        const localized = await translateSearchHitTitles(localizable, locale)
        const titles = new Map(
            localized.map((hit) => [`${hit.provider}:${hit.project_id}`, hit.title]),
        )
        return hits.map((hit) => ({
            ...hit,
            title: titles.get(`${hit.provider}:${hit.project_id}`) ?? hit.title,
        }))
    },
    locale: i18n.global.locale,
    onError: (error) => debugLog('Search result translation failed', error),
})

async function maybeHintAutoTranslate() {
    if (!(await noteManualTranslateClick())) return
    addNotification({
        title: formatMessage(autoTranslateHintMessages.title),
        text: formatMessage(autoTranslateHintMessages.text),
        type: 'info',
    })
}

function toggleTranslation() {
    if (!translationActive.value) void maybeHintAutoTranslate()
    toggleResultTranslation()
}

let sourceRequestVersion = 0
let initializedFilterType = projectType.value
let initializedFilterSource = contentSource.value
let normalizingSource = false
async function prepareBrowseContext(previousType = projectType.value) {
    const type = projectType.value
    const version = ++sourceRequestVersion
    filterMetadataStatus.value = 'loading'
    searchEnabled.value = false
    searchState.loading.value = true
    searchState.searchError.value = null
    try {
        normalizingSource = true
        try {
            if (type === WORLD_BROWSE_PROJECT_TYPE && previousType !== type) {
                sourceBeforeWorldMapBrowse.value = contentSource.value
                if (curseForgeCapability.value.configured) contentSource.value = 'curseforge'
            } else if (previousType === WORLD_BROWSE_PROJECT_TYPE && type !== previousType) {
                contentSource.value = sourceBeforeWorldMapBrowse.value
            }
            if (
                type !== 'mod' &&
                (contentSource.value === 'mcarchive' || contentSource.value === 'planet_minecraft')
            ) {
                contentSource.value = 'all'
                setLastBrowseContentSource('all')
            }
        } finally {
            normalizingSource = false
        }
        await prepareFilterMetadata(type, contentSource.value)
        if (version !== sourceRequestVersion || isUnmounted) return
        if (type !== initializedFilterType) applyRememberedFilters(type, true)
        else if (contentSource.value !== initializedFilterSource) {
            searchState.currentFilters.value = searchState.currentFilters.value.filter(
                (filter) => !filter.type.startsWith('category_'),
            )
        }
        searchState.currentPage.value = 1
        await nextTick()
        if (version !== sourceRequestVersion || isUnmounted) return
        initializedFilterType = type
        initializedFilterSource = contentSource.value
        searchEnabled.value = true
        await searchState.refreshSearch()
    } catch (error) {
        if (version !== sourceRequestVersion || isUnmounted) return
        searchState.loading.value = false
        searchState.searchError.value = resourceOperationError(error, formatMessage)
    }
}
watch(
    [projectType, contentSource],
    (_, [previousType]) => {
        if (normalizingSource || !browseInitialized.value) return
        void prepareBrowseContext(previousType)
    },
    { flush: 'sync' },
)

watch(
    mcArchiveAvailable,
    (available) => {
        if (!filtersReady.value) return
        if (!available && contentSource.value === 'mcarchive') {
            contentSource.value = 'all'
            setLastBrowseContentSource('all')
        }
    },
    { immediate: !route.query.i },
)

function selectContentSource(source: string) {
    if (isWorldMapBrowse.value) return
    if (
        source === 'all' ||
        source === 'modrinth' ||
        source === 'curseforge' ||
        (source === 'mcarchive' && projectType.value === 'mod' && mcArchiveAvailable.value) ||
        (source === 'planet_minecraft' &&
            projectType.value === 'mod' &&
            planetMinecraftAvailable.value)
    ) {
        contentSource.value = source
        setLastBrowseContentSource(source)
    }
}

watch(
    [
        () => searchState.query.value,
        () => searchState.currentFilters.value,
        () => searchState.serverCurrentFilters.value,
        () => projectType.value,
    ],
    () => {
        if (isServerContext.value) {
            syncHiddenServerContentProjectIds()
        } else if (activeInstance.value) {
            syncHiddenInstanceProjectIds()
        }
    },
    { deep: true },
)

watch(queuedServerInstallCount, (count) => {
    if (count === 0) {
        hideSelectedServerInstalls.value = false
    }
})

type UnlistenFn = () => void

let isUnmounted = false
let unlistenInstances: UnlistenFn | null = null
let pendingBrowseReturnScroll = browseReturnSnapshot !== null

async function restoreBrowseReturnScroll() {
    if (!browseReturnSnapshot) return

    await nextTick()
    await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()))
    if (isUnmounted) return
    document.querySelector<HTMLElement>('.app-viewport')?.scrollTo({
        top: browseReturnSnapshot.scrollTop,
    })
    navReturn.completeBrowseReturnNavigation(route.fullPath)
}

watch(
    searchState.loading,
    (isLoading) => {
        if (isLoading || !pendingBrowseReturnScroll) return

        pendingBrowseReturnScroll = false
        void restoreBrowseReturnScroll()
    },
    { flush: 'post' },
)

let initialization: Promise<void> | null = null
async function initializeBrowse() {
    if (initialization) return initialization
    searchEnabled.value = false
    searchState.loading.value = true
    searchState.searchError.value = null
    const contextKey = () =>
        JSON.stringify([
            route.params.projectType,
            route.query.i,
            route.query.sid,
            route.query.wid,
            route.query.from,
            route.query.source,
        ])
    const requestedContext = contextKey()
    let restart = false
    initialization = (async () => {
        try {
            const [capability, planetAvailable] = await Promise.all([
                getCurseForgeCapability(),
                planetMinecraftConnectorAvailable().catch(() => false),
                initInstanceContext(),
            ])
            if (isUnmounted) return
            if (contextKey() !== requestedContext) {
                restart = true
                return
            }
            curseForgeCapability.value = capability
            planetMinecraftAvailable.value = planetAvailable
            contentSource.value = resolveInitialContentSource()
            if (isWorldMapBrowse.value)
                sourceBeforeWorldMapBrowse.value = getLastBrowseContentSource() ?? 'all'
            const memory = getBrowseFilterMemory(projectType.value)
            const metadataRequest = prepareFilterMetadata()
            const requiresFilterMetadata =
                hasExplicitFilterQuery() ||
                !!memory?.filters.length ||
                !!activeInstance.value ||
                isServerContext.value ||
                projectType.value === 'server'
            if (requiresFilterMetadata) await metadataRequest
            else void metadataRequest.catch(() => {})
            await initialInstanceFilterPromise
            if (instanceHideInstalled.value) {
                await initialInstalledProjectsPromise
                if (activeInstance.value && installedProjectIds.value === null)
                    throw new Error('Installed content could not be loaded.')
            }
            if (isUnmounted) return
            if (contextKey() !== requestedContext) {
                restart = true
                return
            }
            searchState.initializeFilters()
            if (!hasExplicitFilterQuery()) applyRememberedFilters(projectType.value, false)
            await nextTick()
            restoreBrowseReturnPage()
            await nextTick()
            initializedFilterType = projectType.value
            initializedFilterSource = contentSource.value
            browseInitialized.value = true
            searchEnabled.value = true
            await searchState.refreshSearch()
            if (pendingRouteInstanceSwitch.value) {
                instanceSelector.value?.requestSwitch(pendingRouteInstanceSwitch.value)
                pendingRouteInstanceSwitch.value = null
            }
        } catch (error) {
            if (contextKey() !== requestedContext) {
                restart = true
                return
            }
            if (!isUnmounted) {
                searchState.loading.value = false
                searchState.searchError.value = resourceOperationError(error, formatMessage)
            }
        } finally {
            initialization = null
            if (restart && !isUnmounted) void initializeBrowse()
        }
    })()
    return initialization
}

async function refreshBrowse() {
    if (!searchEnabled.value) {
        if (browseInitialized.value) return prepareBrowseContext()
        return initializeBrowse()
    }
    await searchState.refreshSearch()
}

onMounted(() => {
    if (pendingRouteInstanceSwitch.value) {
        instanceSelector.value?.requestSwitch(pendingRouteInstanceSwitch.value)
        pendingRouteInstanceSwitch.value = null
    }
    void initializeBrowse()

    instance_listener(async (event: { event: string; instance_id: string }) => {
        if (
            activeInstance.value &&
            event.instance_id === activeInstance.value.id &&
            ['synced', 'content_install_finished', 'content_install_failed'].includes(event.event)
        ) {
            if (await refreshInstalledProjectIds()) {
                await searchState.refreshSearch()
            }
        }
    })
        .then((unlisten) => {
            if (isUnmounted) {
                unlisten()
                return
            }

            unlistenInstances = unlisten
        })
        .catch(handleError)
})

onUnmounted(() => {
    isUnmounted = true
    unlistenInstances?.()
})

function getProjectBrowseQuery() {
    return createProjectDetailsQuery(route.query, route.fullPath)
}

const advancedFiltersCollapsed = computed({
    get: () => themeStore.getFeatureFlag('advanced_filters_collapsed'),
    set: (value) => {
        themeStore.featureFlags['advanced_filters_collapsed'] = value
        updateSettings({ feature_flags: { advanced_filters_collapsed: value } }).catch(handleError)
    },
})

provideBrowseManager({
    tags,
    projectType,
    ...searchState,
    filtersReady,
    refreshSearch: refreshBrowse,
    advancedFiltersCollapsed,
    getProjectLink: (
        result: Labrinth.Search.v2.ResultSearchProject & {
            provider: 'modrinth' | 'curseforge' | 'mcarchive' | 'planet_minecraft'
            resource_entry?: { resourceId: string | null; status: string }
            provider_project_id?: string
        },
    ) => {
        const path =
            result.resource_entry?.status === 'confirmed' && result.resource_entry.resourceId
                ? `/resource/${result.resource_entry.resourceId}`
                : result.provider === 'curseforge'
                  ? `/project/curseforge/${result.provider_project_id}`
                  : result.provider === 'mcarchive'
                    ? `/project/mcarchive/${result.slug}`
                    : result.provider === 'planet_minecraft'
                      ? `/project/planet-minecraft/${result.provider_project_id}`
                      : result.provider === 'modrinth'
                        ? `/project/${result.project_id ?? result.slug}`
                        : route.path
        return {
            path,
            query: getProjectBrowseQuery(),
            state: createProjectPreviewState(path, result.title),
        }
    },
    getServerProjectLink: (result: Labrinth.Search.v3.ResultSearchProject) => {
        const path = `/project/${result.slug ?? result.project_id}`
        return {
            path,
            query: getProjectBrowseQuery(),
            state: createProjectPreviewState(path, result.name),
        }
    },
    selectableProjectTypes,
    showProjectTypeTabs: computed(() => !isServerContext.value),
    variant: 'app',
    getCardActions,
    installContext,
    providedFilters: combinedProvidedFilters,
    hideInstalled: computed({
        get: () =>
            isServerContext.value ? serverHideInstalled.value : instanceHideInstalled.value,
        set: (val: boolean) => {
            if (isServerContext.value) {
                serverHideInstalled.value = val
                if (val) syncHiddenServerContentProjectIds()
            } else {
                instanceHideInstalled.value = val
                if (val) syncHiddenInstanceProjectIds()
            }
        },
    }),
    showHideInstalled: computed(
        () =>
            !isWorldMapBrowse.value &&
            ((isServerContext.value && projectType.value !== 'modpack') || !!activeInstance.value),
    ),
    hideInstalledLabel: computed(() =>
        formatMessage(
            isFromWorlds.value
                ? messages.hideAddedServers
                : commonMessages.hideInstalledContentLabel,
        ),
    ),
    hideSelected: hideSelectedServerInstalls,
    showHideSelected: computed(
        () =>
            isServerContext.value &&
            projectType.value !== 'modpack' &&
            queuedServerInstallCount.value > 0,
    ),
    hideSelectedLabel: computed(() => formatMessage(commonMessages.hideSelectedContentLabel)),
    onInstalled: onSearchResultInstalled,
    serverPings,
    getServerModpackContent,
    onContextMenu: (event, result) => {
        if ('provider' in result && result.provider !== 'modrinth') return
        if (!('provider' in result) && projectType.value !== 'server') return
        handleRightClick(event, result)
    },
    offline,
    lockedFilterMessages,
    displayMode,
    displayModeOptions,
    displayModeTooltip,
    setDisplayMode,
})
</script>

<template>
    <div data-onboarding-id="browse-content" class="flex flex-col gap-3 p-6">
        <BrowsePageLayout
            v-if="!filtersReady || !isWorldMapBrowse || curseForgeCapability.configured"
        >
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
                    v-if="!isServerContext && projectType !== 'server' && projectType !== 'modpack'"
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
                    v-if="projectType !== 'server' && !isWorldMapBrowse"
                    placement="bottom-end"
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
                                class="flex w-full items-center gap-2 !justify-start text-left"
                                :disabled="!browseInitialized"
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
                <ContextMenu ref="contextMenuRef" @option-clicked="handleOptionsClick">
                    <template #open_link>
                        <GlobeIcon /> {{ formatMessage(commonMessages.openInModrinthButton) }}
                        <ExternalIcon />
                    </template>
                    <template #copy_link>
                        <ClipboardCopyIcon /> {{ formatMessage(commonMessages.copyLinkButton) }}
                    </template>
                </ContextMenu>
            </template>
            <template #above-results>
                <div
                    v-if="filterMetadataFailed && searchEnabled"
                    role="alert"
                    class="mb-3 flex items-center gap-3 rounded-lg border border-solid border-surface-4 bg-surface-2 p-3 text-sm"
                >
                    <span>{{ formatMessage(messages.filterMetadataFailed) }}</span>
                    <Button @click="retryFilterMetadata">{{
                        formatMessage(commonMessages.retryButton)
                    }}</Button>
                </div>
                <div
                    v-if="resourceSearchNotice"
                    role="status"
                    class="mb-3 rounded-lg border border-solid border-surface-4 bg-surface-2 p-3 text-sm"
                >
                    {{ resourceSearchNotice }}
                </div>
            </template>
        </BrowsePageLayout>
        <EmptyState
            v-else
            type="empty-inbox"
            :heading="formatMessage(messages.mapsUnavailable)"
            :description="formatMessage(messages.mapsUnavailableDescription)"
        />
        <CreationFlowModal
            v-if="isServerContext && projectType === 'modpack'"
            ref="serverSetupModalRef"
            :type="serverFlowFrom === 'reset-server' ? 'reset-server' : 'server-onboarding'"
            :available-loaders="[
                'vanilla',
                'fabric',
                'neoforge',
                'forge',
                'quilt',
                'paper',
                'purpur',
            ]"
            :show-snapshot-toggle="true"
            :on-back="onServerFlowBack"
            :search-modpacks="searchServerModpacks"
            :get-project-versions="getServerProjectVersions"
            :get-loader-manifest="getLoaderManifest"
            @hide="() => {}"
            @browse-modpacks="() => {}"
            @create="handleServerModpackFlowCreate"
        />
        <BrowseInstanceSelector
            ref="instanceSelector"
            :instances="contentSelection.instances.value"
            :selected-instance="activeInstance"
            :selected-count="contentSelection.selectedCount.value"
            :install-current="contentSelection.installSelected"
            :clear-current="contentSelection.clear"
            @select="selectTargetInstance"
            @cancel-switch="cancelTargetInstanceSwitch"
        />
        <Teleport to="#sidebar-teleport-target">
            <BrowseSidebar />
        </Teleport>
    </div>
</template>

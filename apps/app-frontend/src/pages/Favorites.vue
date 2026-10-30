<script setup lang="ts">
import type { Labrinth } from '@modrinth/api-client'
import {
    BookmarkFilledIcon,
    BookmarkIcon,
    CheckIcon,
    ChevronDownIcon,
    GenericListIcon,
    GridIcon,
    ListIcon,
    PlusIcon,
    SearchIcon,
    SpinnerIcon,
} from '@modrinth/assets'
import {
    Admonition,
    type BrowseInstallContext,
    BrowseInstallHeader,
    Button,
    commonMessages,
    defineMessages,
    EmptyState,
    getLatestMatchingInstallVersion,
    getTargetInstallPreferences,
    injectNotificationManager,
    NavTabs,
    Pagination,
    PopoutMenu,
    ProjectCard,
    ProjectCardList,
    ProjectCardSkeleton,
    SelectedProjectsFloatingBar,
    StyledInput,
    useStickyObserver,
    useVIntl,
} from '@modrinth/ui'
import { computed, nextTick, onBeforeUnmount, onMounted, ref, shallowRef, watch } from 'vue'
import { onBeforeRouteLeave, useRoute, useRouter } from 'vue-router'

import BrowseInstanceSelector from '@/components/browse/BrowseInstanceSelector.vue'
import InstanceIcon from '@/components/ui/InstanceIcon.vue'
import { useContentFavorites } from '@/composables/useContentFavorites'
import { createBrowseProjectTabs, getBrowseProjectTabOptions } from '@/helpers/browse-project-tabs'
import { get_project, get_version_many } from '@/helpers/cache.js'
import {
    type ContentFavorite,
    contentFavoriteKey,
    type FavoriteContentType,
    type FavoriteProvider,
    prefetchContentFavorites,
    resolveContentFavorite,
} from '@/helpers/content-favorites'
import { getCurseForgeFiles } from '@/helpers/curseforge'
import { getDisplayInstanceIcon } from '@/helpers/instance-icons'
import { runAfterPageTransitionSettle } from '@/helpers/page-transition'
import { createProjectDetailsQuery } from '@/helpers/project-links'
import { createProjectPreviewState } from '@/helpers/project-preview'
import type { SourceProjectMetadata } from '@/helpers/resources'
import {
    getLastBrowseContentDisplayMode,
    setLastBrowseContentDisplayMode,
} from '@/helpers/settings'
import type { GameInstance } from '@/helpers/types'
import { injectContentSelection, makeContentSelectionKey } from '@/providers/content-selection'
import { useBreadcrumbs } from '@/store/breadcrumbs'
import { isBrowseReturnSourcePath, useNavigationReturnStore } from '@/store/navigation-return'

type FavoriteFilter = 'all' | FavoriteContentType
type FavoriteDisplayMode = 'list' | 'compact' | 'grid'

type FavoriteProject = {
    favorite: ContentFavorite
    provider: FavoriteProvider
    projectId: string
    title: string
    description: string
    slug?: string
    iconUrl?: string
    downloads?: number
    categories: string[]
    dateCreated?: string
    dateModified?: string
    banner?: string
    color?: string | number
    environment?: {
        clientSide: Labrinth.Projects.v2.Environment
        serverSide: Labrinth.Projects.v2.Environment
    }
    unavailable: boolean
}

type FavoriteRow = { favorite: ContentFavorite; project?: FavoriteProject }
type FavoritesView = {
    search: string
    filter: FavoriteFilter
    page: number
    projects: FavoriteProject[]
}

type FavoritesReturnState = {
    projects: FavoriteProject[]
}

const PAGE_SIZE = 20
const curseForgeLoaderTypes: Record<string, number> = {
    forge: 1,
    fabric: 4,
    quilt: 5,
    neoforge: 6,
}

const route = useRoute()
const router = useRouter()
const breadcrumbs = useBreadcrumbs()
const navReturn = useNavigationReturnStore()
const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const contentSelection = injectContentSelection()
const contentFavorites = useContentFavorites()
const instanceSelector = ref<InstanceType<typeof BrowseInstanceSelector>>()
const stickyChromeRef = ref<HTMLElement | null>(null)
const { isStuck: isChromeStuck } = useStickyObserver(stickyChromeRef, 'FavoritesToolbarChrome')
const projects = shallowRef<FavoriteProject[]>([])
let projectsController: AbortController | null = null
let disposed = false
const loadingProjects = ref(false)
const installingKeys = ref(new Set<string>())
const displayMode = ref<FavoriteDisplayMode>(getLastBrowseContentDisplayMode())
let projectRequestId = 0
const browseReturnSnapshot = navReturn.consumeBrowseReturnSnapshot<FavoritesReturnState>(
    route.fullPath,
)
if (browseReturnSnapshot) projects.value = browseReturnSnapshot.state.projects
const projectsLoadFailed = ref(false)
const hasProjectsSnapshot = ref(!!browseReturnSnapshot)
const metadataComplete = ref(false)
const committedView = shallowRef<FavoritesView | null>(null)

const messages = defineMessages({
    refreshing: {
        id: 'app.content-favorites.refreshing',
        defaultMessage: 'Updating favorites…',
    },
    browseResources: {
        id: 'app.content-favorites.browse-resources',
        defaultMessage: 'Browse resources',
    },
    clearFilters: {
        id: 'app.content-favorites.clear-filters',
        defaultMessage: 'Clear search and filters',
    },
    loadFailed: {
        id: 'app.content-favorites.load-failed',
        defaultMessage: 'Failed to load favorites',
    },
    title: {
        id: 'app.content-favorites.title',
        defaultMessage: 'Favorites',
    },
    search: {
        id: 'app.content-favorites.search',
        defaultMessage: 'Search favorites',
    },
    allContentTypes: {
        id: 'app.content-favorites.type.all',
        defaultMessage: 'All content types',
    },
    mods: {
        id: 'app.browse.project-type.mods',
        defaultMessage: 'Mods',
    },
    resourcepacks: {
        id: 'app.browse.project-type.resourcepacks',
        defaultMessage: 'Resource Packs',
    },
    datapacks: {
        id: 'app.browse.project-type.datapacks',
        defaultMessage: 'Data Packs',
    },
    shaders: {
        id: 'app.browse.project-type.shaders',
        defaultMessage: 'Shaders',
    },
    modpacks: {
        id: 'app.browse.project-type.modpacks',
        defaultMessage: 'Modpacks',
    },
    maps: {
        id: 'app.browse.project-type.maps',
        defaultMessage: 'Maps',
    },
    servers: {
        id: 'app.browse.project-type.servers',
        defaultMessage: 'Servers',
    },
    chooseInstance: {
        id: 'app.browse.choose-instance',
        defaultMessage: 'Choose instance',
    },
    backToInstance: {
        id: 'app.browse.back-to-instance',
        defaultMessage: 'Back to instance',
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
    noCompatibleVersion: {
        id: 'app.browse.no-compatible-version',
        defaultMessage: 'No compatible version was found for the selected instance.',
    },
    remove: {
        id: 'app.content-favorites.remove',
        defaultMessage: 'Remove from favorites',
    },
    view: {
        id: 'app.browse.display-mode.switch',
        defaultMessage: 'Switch view',
    },
    listView: {
        id: 'app.browse.display-mode.list',
        defaultMessage: 'List',
    },
    compactView: {
        id: 'app.browse.display-mode.compact-list',
        defaultMessage: 'Compact list',
    },
    gridView: {
        id: 'app.browse.display-mode.grid',
        defaultMessage: 'Grid',
    },
    recentlySaved: {
        id: 'app.content-favorites.sort.recently-saved',
        defaultMessage: 'Recently saved',
    },
    emptyTitle: {
        id: 'app.content-favorites.empty-title',
        defaultMessage: 'No favorites yet',
    },
    emptyDescription: {
        id: 'app.content-favorites.empty-description',
        defaultMessage:
            'Bookmark mods, resource packs, data packs, and shaders to install them later.',
    },
    noMatchesTitle: {
        id: 'app.content-favorites.no-matches-title',
        defaultMessage: 'No matching favorites',
    },
    noMatchesDescription: {
        id: 'app.content-favorites.no-matches-description',
        defaultMessage: 'Try a different search or content type.',
    },
    unavailableTitle: {
        id: 'app.content-favorites.unavailable-title',
        defaultMessage: '{provider} project {projectId}',
    },
    unavailableDescription: {
        id: 'app.content-favorites.unavailable-description',
        defaultMessage: 'This project is currently unavailable. You can remove it from favorites.',
    },
})

function queryValue(value: unknown): string {
    return typeof value === 'string' ? value : ''
}

function parseFavoriteFilter(value: unknown): FavoriteFilter {
    return value === 'mod' || value === 'resourcepack' || value === 'datapack' || value === 'shader'
        ? value
        : 'all'
}

function parsePage(value: unknown): number {
    const page = Number.parseInt(queryValue(value), 10)
    return Number.isFinite(page) && page > 0 ? page : 1
}

function updateQuery(values: Record<string, string | undefined>) {
    void router.replace({
        query: {
            ...route.query,
            ...values,
        },
    })
}

function browseResources() {
    const { q: _q, kind: _kind, page: _page, ...query } = route.query
    void router.push({ path: '/browse/mod', query })
}

function clearFilters() {
    updateQuery({ q: undefined, kind: undefined, page: undefined })
}

const search = computed({
    get: () => queryValue(route.query.q),
    set: (value: string) => updateQuery({ q: value || undefined, page: undefined }),
})

const filter = computed<FavoriteFilter>({
    get: () => parseFavoriteFilter(route.query.kind),
    set: (value) => updateQuery({ kind: value === 'all' ? undefined : value, page: undefined }),
})

const currentPage = computed({
    get: () => parsePage(route.query.page),
    set: (page: number) => updateQuery({ page: page > 1 ? String(page) : undefined }),
})

const favoriteTypeOptions = computed(() => [
    { id: 'all' as const, label: formatMessage(messages.allContentTypes) },
    { id: 'mod' as const, label: formatMessage(messages.mods) },
    { id: 'resourcepack' as const, label: formatMessage(messages.resourcepacks) },
    { id: 'datapack' as const, label: formatMessage(messages.datapacks) },
    { id: 'shader' as const, label: formatMessage(messages.shaders) },
])

const currentFavoriteTypeLabel = computed(
    () => favoriteTypeOptions.value.find((option) => option.id === filter.value)?.label,
)

const displayModeOptions = computed(() => [
    { id: 'list' as const, label: formatMessage(messages.listView), icon: ListIcon },
    { id: 'compact' as const, label: formatMessage(messages.compactView), icon: GenericListIcon },
    { id: 'grid' as const, label: formatMessage(messages.gridView), icon: GridIcon },
])

const currentDisplayMode = computed(() =>
    displayModeOptions.value.find((option) => option.id === displayMode.value),
)

const projectTabs = computed(() => {
    const query = new URLSearchParams()
    for (const [key, value] of Object.entries(route.query)) {
        if (key === 'kind' || key === 'q' || key === 'page') continue
        if (Array.isArray(value)) {
            for (const entry of value) {
                if (entry != null) query.append(key, entry)
            }
        } else if (value) {
            query.set(key, value)
        }
    }
    const suffix = query.size ? `?${query.toString()}` : ''
    return createBrowseProjectTabs(
        {
            modpacks: formatMessage(messages.modpacks),
            mods: formatMessage(messages.mods),
            resourcepacks: formatMessage(messages.resourcepacks),
            datapacks: formatMessage(messages.datapacks),
            maps: formatMessage(messages.maps),
            shaders: formatMessage(messages.shaders),
            servers: formatMessage(messages.servers),
            favorites: formatMessage(messages.title),
        },
        suffix,
        getBrowseProjectTabOptions({
            instance: contentSelection.targetInstance.value,
            hasInstanceContext: !!route.query.i,
            isServerInstance:
                contentSelection.targetInstance.value?.link?.type === 'server_project' ||
                contentSelection.targetInstance.value?.link?.type === 'server_project_modpack',
        }),
    )
})

function typeLabel(type: FavoriteContentType): string {
    return formatMessage(
        {
            mod: messages.mods,
            resourcepack: messages.resourcepacks,
            datapack: messages.datapacks,
            shader: messages.shaders,
        }[type],
    )
}

function toUnavailable(favorite: ContentFavorite): FavoriteProject {
    const provider =
        favorite.provider === 'curseforge'
            ? 'CurseForge'
            : favorite.provider === 'mcarchive'
              ? 'MCArchive'
              : favorite.provider === 'planet_minecraft'
                ? 'Planet Minecraft'
                : 'Modrinth'
    return {
        favorite,
        provider: favorite.provider,
        projectId: favorite.project_id,
        title: formatMessage(messages.unavailableTitle, {
            provider,
            projectId: favorite.project_id,
        }),
        description: formatMessage(messages.unavailableDescription),
        categories: [typeLabel(favorite.content_type)],
        unavailable: true,
    }
}

function toProject(favorite: ContentFavorite, metadata: SourceProjectMetadata): FavoriteProject {
    return {
        favorite,
        provider: metadata.project.source,
        projectId: metadata.project.projectId,
        title: metadata.title,
        description: metadata.summary,
        slug: metadata.slug ?? undefined,
        iconUrl: metadata.iconUrl ?? undefined,
        downloads: metadata.downloads ?? undefined,
        categories: [typeLabel(favorite.content_type), ...metadata.categories],
        dateCreated: metadata.createdAt ?? undefined,
        dateModified: metadata.updatedAt ?? undefined,
        banner: metadata.gallery[0],
        unavailable: false,
    }
}

function restoreCachedProjects() {
    if (!contentFavorites.loaded.value) return
    const cached = contentFavorites.favorites.value.flatMap((favorite) => {
        const metadata = contentFavorites.getMetadata(favorite)
        if (metadata) return [toProject(favorite, metadata)]
        const previous = projects.value.find(
            (project) => JSON.stringify(project.favorite) === JSON.stringify(favorite),
        )
        return previous ? [previous] : []
    })
    if (cached.length || contentFavorites.favorites.value.length === 0) {
        projects.value = cached
        hasProjectsSnapshot.value = true
    }
}

if (!browseReturnSnapshot) restoreCachedProjects()
metadataComplete.value =
    contentFavorites.loaded.value &&
    contentFavorites.favorites.value.every((favorite) =>
        projects.value.some(
            (project) => JSON.stringify(project.favorite) === JSON.stringify(favorite),
        ),
    )

async function refreshProjects() {
    if (disposed) return
    const requestId = ++projectRequestId
    projectsController?.abort()
    projectsController = new AbortController()
    const signal = projectsController.signal
    loadingProjects.value = true
    projectsLoadFailed.value = false
    const isCurrent = () => requestId === projectRequestId && !signal.aborted && !disposed
    try {
        await contentFavorites.load(true)
        if (!isCurrent()) return
        metadataComplete.value = false
        const favorites = [...contentFavorites.favorites.value]
        hasProjectsSnapshot.value = true
        const candidates = (favorite: ContentFavorite) => [
            { source: favorite.provider, projectId: favorite.project_id },
            ...(favorite.projects ?? []),
        ]
        const restoreLocal = (local: ReadonlyMap<string, SourceProjectMetadata>) => {
            if (!isCurrent()) return
            for (const favorite of favorites) {
                const metadata = candidates(favorite)
                    .map((project) => local.get(`${project.source}:${project.projectId}`))
                    .find(Boolean)
                if (metadata && !contentFavorites.getMetadata(favorite))
                    contentFavorites.setMetadata(favorite, metadata)
            }
            restoreCachedProjects()
        }
        const local = await prefetchContentFavorites(
            favorites,
            signal,
            'onlyCached',
            (_source, metadata) => restoreLocal(metadata),
        )
        if (!isCurrent()) return
        restoreLocal(local)

        const settledSources = new Set<FavoriteProvider>()
        const resolutions = new Map<string, Promise<FavoriteProject>>()
        const publish = (project: FavoriteProject) => {
            if (
                !isCurrent() ||
                !contentFavorites.favorites.value.some(
                    (favorite) => JSON.stringify(favorite) === JSON.stringify(project.favorite),
                )
            )
                return
            const key = contentFavoriteKey(project.favorite.provider, project.favorite.project_id)
            projects.value = [
                ...projects.value.filter(
                    (previous) =>
                        contentFavoriteKey(
                            previous.favorite.provider,
                            previous.favorite.project_id,
                        ) !== key,
                ),
                project,
            ]
        }
        const schedule = (cached: ReadonlyMap<string, SourceProjectMetadata>) => {
            if (!isCurrent()) return
            for (const favorite of favorites) {
                const key = contentFavoriteKey(favorite.provider, favorite.project_id)
                const references = candidates(favorite)
                if (
                    resolutions.has(key) ||
                    references.some(
                        (project) =>
                            (project.source === 'modrinth' || project.source === 'curseforge') &&
                            !settledSources.has(project.source),
                    )
                )
                    continue
                const resolution = (async (): Promise<FavoriteProject> => {
                    try {
                        const metadata =
                            references
                                .map((project) =>
                                    cached.get(`${project.source}:${project.projectId}`),
                                )
                                .find(Boolean) ?? (await resolveContentFavorite(favorite, signal))
                        if (isCurrent()) contentFavorites.setMetadata(favorite, metadata)
                        return toProject(favorite, metadata)
                    } catch {
                        const metadata = contentFavorites.getMetadata(favorite)
                        return metadata ? toProject(favorite, metadata) : toUnavailable(favorite)
                    }
                })().then((project) => {
                    publish(project)
                    return project
                })
                resolutions.set(key, resolution)
            }
        }
        schedule(new Map())
        const cached = await prefetchContentFavorites(
            favorites,
            signal,
            'useFresh',
            (source, metadata) => {
                settledSources.add(source)
                schedule(metadata)
            },
        )
        if (!isCurrent()) return
        // A source can be absent from the prefetch batch for historical favorites.
        settledSources.add('modrinth')
        settledSources.add('curseforge')
        schedule(cached)
        const resolved = await Promise.all(resolutions.values())
        if (isCurrent()) {
            projects.value = resolved
            metadataComplete.value = true
        }
    } catch {
        if (requestId === projectRequestId && !signal.aborted) projectsLoadFailed.value = true
    } finally {
        if (requestId === projectRequestId) loadingProjects.value = false
    }
}

onBeforeUnmount(() => {
    disposed = true
    projectsController?.abort()
})

const activeFavoriteIdentities = computed(
    () => new Set(contentFavorites.favorites.value.map((favorite) => JSON.stringify(favorite))),
)

const availableProjects = computed(() =>
    projects.value
        .filter((project) => activeFavoriteIdentities.value.has(JSON.stringify(project.favorite)))
        .sort((left, right) => right.favorite.saved_at - left.favorite.saved_at),
)

function matchesFilter(project: FavoriteProject, query: string, kind: FavoriteFilter) {
    if (kind !== 'all' && project.favorite.content_type !== kind) return false
    const normalizedSearch = query.trim().toLocaleLowerCase()
    return (
        !normalizedSearch ||
        [project.title, project.description, project.provider, project.projectId]
            .join('\n')
            .toLocaleLowerCase()
            .includes(normalizedSearch)
    )
}

watch(
    [search, filter, currentPage, loadingProjects],
    () => {
        if (!loadingProjects.value && metadataComplete.value) {
            const matching = availableProjects.value.filter((project) =>
                matchesFilter(project, search.value, filter.value),
            )
            committedView.value = {
                search: search.value,
                filter: filter.value,
                page: Math.min(
                    currentPage.value,
                    Math.max(1, Math.ceil(matching.length / PAGE_SIZE)),
                ),
                projects: matching,
            }
        } else if (
            !search.value.trim() &&
            filter.value === 'all' &&
            (!committedView.value ||
                (!committedView.value.search.trim() && committedView.value.filter === 'all'))
        ) {
            committedView.value = {
                search: '',
                filter: 'all',
                page: currentPage.value,
                projects: [],
            }
        }
    },
    { immediate: true },
)

const displayRows = computed<FavoriteRow[]>(() => {
    const view = committedView.value
    if (!view) return []
    if (view.search.trim() || view.filter !== 'all')
        return view.projects
            .filter((project) =>
                activeFavoriteIdentities.value.has(JSON.stringify(project.favorite)),
            )
            .map((project) => ({ favorite: project.favorite, project }))
    const known = new Map(
        availableProjects.value.map((project) => [JSON.stringify(project.favorite), project]),
    )
    return contentFavorites.favorites.value.map((favorite) => ({
        favorite,
        project: known.get(JSON.stringify(favorite)),
    }))
})
const pageCount = computed(() => Math.max(1, Math.ceil(displayRows.value.length / PAGE_SIZE)))
const displayedPage = computed(() =>
    Math.min(committedView.value?.page ?? currentPage.value, pageCount.value),
)
const pagedRows = computed(() => {
    const offset = (displayedPage.value - 1) * PAGE_SIZE
    return displayRows.value.slice(offset, offset + PAGE_SIZE)
})

watch([pageCount, loadingProjects], ([count, loading]) => {
    if (!loading && metadataComplete.value && currentPage.value > count) currentPage.value = count
})

const installContext = computed<BrowseInstallContext | null>(() => {
    const target = contentSelection.targetInstance.value
    if (!target) return null
    const icon = getDisplayInstanceIcon(target.icon_path, target.loader)
    const processing = ['validating', 'reviewing', 'queueing'].includes(
        contentSelection.state.value,
    )
    return {
        showInstallHeader: !!route.query.i,
        name: target.name,
        loader: target.loader,
        gameVersion: target.game_version,
        iconSrc: icon.url,
        iconFrameless: icon.frameless,
        backUrl: route.query.i ? `/instance/${encodeURIComponent(target.id)}` : route.fullPath,
        backLabel: formatMessage(messages.backToInstance),
        heading: formatMessage(commonMessages.installingContentLabel),
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
})

function isInstalling(project: FavoriteProject) {
    const key = makeContentSelectionKey(project.provider, project.projectId)
    return installingKeys.value.has(key) || contentSelection.isInstalling(key)
}

function isSelected(project: FavoriteProject) {
    return contentSelection.isSelected(makeContentSelectionKey(project.provider, project.projectId))
}

function installLabel(project: FavoriteProject) {
    if (isInstalling(project)) return formatMessage(commonMessages.validatingLabel)
    if (isSelected(project)) return formatMessage(messages.selected)
    return formatMessage(commonMessages.installButton)
}

function setInstalling(key: string, installing: boolean) {
    const next = new Set(installingKeys.value)
    if (installing) next.add(key)
    else next.delete(key)
    installingKeys.value = next
}

function getInstallPreferences(target: GameInstance, type: FavoriteContentType) {
    return getTargetInstallPreferences(
        { gameVersion: target.game_version, loader: target.loader },
        type,
    )
}

async function getModrinthVersions(projectId: string) {
    const project = (await get_project(
        projectId,
        'must_revalidate',
    )) as Labrinth.Projects.v2.Project
    return (await get_version_many(
        project.versions,
        'must_revalidate',
    )) as Labrinth.Versions.v2.Version[]
}

async function toggleProjectSelection(project: FavoriteProject) {
    if (project.unavailable) return
    if (project.provider === 'mcarchive' || project.provider === 'planet_minecraft') {
        await router.push(getProjectLink(project)!)
        return
    }
    if (!contentSelection.targetInstance.value) {
        await contentSelection.refreshInstances(queryValue(route.query.i) || undefined)
        instanceSelector.value?.show()
        return
    }

    const target = contentSelection.targetInstance.value
    const key = makeContentSelectionKey(project.provider, project.projectId)
    if (contentSelection.isSelected(key)) {
        contentSelection.remove(key)
        return
    }

    setInstalling(key, true)
    try {
        const preferences = getInstallPreferences(target, project.favorite.content_type)
        const versionId =
            project.provider === 'modrinth'
                ? getLatestMatchingInstallVersion(
                      await getModrinthVersions(project.projectId),
                      preferences,
                  )?.id
                : (
                      await getCurseForgeFiles(Number(project.projectId), {
                          gameVersion: target.game_version,
                          modLoaderType:
                              project.favorite.content_type === 'mod'
                                  ? curseForgeLoaderTypes[target.loader]
                                  : undefined,
                      })
                  ).files
                      .find((file) => file.isAvailable)
                      ?.id.toString()
        if (contentSelection.targetInstance.value?.id !== target.id) return
        if (!versionId) throw new Error(formatMessage(messages.noCompatibleVersion))

        await contentSelection.add({
            key,
            provider: project.provider,
            projectId: project.projectId,
            providerProjectId: project.projectId,
            versionId,
            contentType: project.favorite.content_type,
            title: project.title,
            iconUrl: project.iconUrl,
            slug: project.slug,
            preferences,
            targetInstanceId: target.id,
        })
    } catch (error) {
        if (contentSelection.targetInstance.value?.id === target.id) handleError(error)
    } finally {
        setInstalling(key, false)
    }
}

async function removeFavorite(project: FavoriteProject) {
    try {
        await contentFavorites.remove(project.favorite.provider, project.favorite.project_id)
    } catch (error) {
        handleError(error)
    }
}

async function selectTargetInstance(target: GameInstance) {
    contentSelection.setTarget(target)
    await contentSelection.refreshInstalledIdentities()
    await router.replace({ query: { ...route.query, i: target.id } })
}

function setDisplayMode(mode: FavoriteDisplayMode) {
    displayMode.value = mode
    setLastBrowseContentDisplayMode(mode)
}

function getProjectLink(project: FavoriteProject) {
    if (project.unavailable) return undefined
    const path = project.favorite.resource_id
        ? `/resource/${project.favorite.resource_id}`
        : project.provider === 'curseforge'
          ? `/project/curseforge/${project.projectId}`
          : project.provider === 'mcarchive'
            ? `/project/mcarchive/${project.slug ?? project.projectId}`
            : project.provider === 'planet_minecraft'
              ? `/project/planet-minecraft/${project.projectId}`
              : `/project/${project.slug ?? project.projectId}`
    return {
        path,
        query: createProjectDetailsQuery(route.query, route.fullPath),
        state: createProjectPreviewState(path, project.title),
    }
}

onBeforeRouteLeave((to) => {
    if (isBrowseReturnSourcePath(to.path)) {
        const viewport = document.querySelector<HTMLElement>('.app-viewport')
        navReturn.saveBrowseReturnSnapshot<FavoritesReturnState>({
            url: route.fullPath,
            scrollTop: viewport?.scrollTop ?? 0,
            state: { projects: projects.value },
        })
    }

    breadcrumbs.setContext({
        name: formatMessage(messages.title),
        link: '/browse/favorites',
        query: route.query,
    })
})

watch(
    () =>
        contentFavorites.favorites.value
            .map((favorite) =>
                JSON.stringify([
                    favorite.provider,
                    favorite.project_id,
                    favorite.resource_id,
                    favorite.locator,
                    favorite.projects,
                ]),
            )
            .join('|'),
    () => void refreshProjects(),
)

onMounted(() => {
    breadcrumbs.setName('FavoritesTitle', formatMessage(messages.title))
    void contentSelection
        .refreshInstances(queryValue(route.query.i) || undefined)
        .catch(handleError)
    if (browseReturnSnapshot) {
        void nextTick().then(
            () =>
                new Promise<void>((resolve) => {
                    requestAnimationFrame(() => {
                        document.querySelector<HTMLElement>('.app-viewport')?.scrollTo({
                            top: browseReturnSnapshot.scrollTop,
                        })
                        navReturn.completeBrowseReturnNavigation(route.fullPath)
                        resolve()
                    })
                }),
        )
    }
    // Match Browse: first network fetch waits for the page-enter paint.
    runAfterPageTransitionSettle(() => {
        void refreshProjects()
    })
})
</script>

<template>
    <div data-onboarding-id="browse-favorites-content" class="flex flex-col gap-3 p-6">
        <div
            ref="stickyChromeRef"
            class="browse-toolbar-chrome sticky top-0 z-20 -mx-6 -mt-6 flex flex-col gap-3 rounded-tl-[--radius-xl] rounded-tr-[--radius-xl] border-0 border-b border-solid border-surface-5 bg-[color-mix(in_srgb,var(--surface-1-opaque,var(--surface-1))_20%,transparent)] backdrop-blur-xl"
            :class="isChromeStuck ? 'border-t shadow-[0_8px_16px_-12px_rgba(0,0,0,0.35)]' : ''"
        >
            <div v-if="installContext?.showInstallHeader" class="p-3 pb-0">
                <BrowseInstallHeader :install-context="installContext" />
            </div>

            <div class="flex flex-col gap-3 p-3">
                <div class="flex min-w-0 items-center gap-2">
                    <NavTabs :links="projectTabs" class="min-w-0" />
                </div>

                <div class="flex flex-wrap items-center gap-2">
                    <StyledInput
                        v-model="search"
                        :icon="SearchIcon"
                        type="text"
                        autocomplete="off"
                        clearable
                        :placeholder="formatMessage(messages.search)"
                        wrapper-class="flex-1"
                        input-class="h-12"
                    />
                    <Button
                        size="md"
                        class="flex min-w-0 items-center gap-2"
                        @click="instanceSelector?.show()"
                        ><InstanceIcon
                            v-if="contentSelection.targetInstance.value"
                            class="shrink-0"
                            size="1.25rem"
                            :icon-path="contentSelection.targetInstance.value.icon_path"
                            :instance-id="contentSelection.targetInstance.value.id"
                            :loader="contentSelection.targetInstance.value.loader"
                        />
                        <PlusIcon v-else class="size-5 shrink-0" />
                        <span class="max-w-40 truncate font-medium">
                            {{
                                contentSelection.targetInstance.value?.name ??
                                formatMessage(messages.chooseInstance)
                            }}
                        </span>
                        <span
                            aria-hidden="true"
                            class="flex size-4 shrink-0 items-center justify-center text-[var(--color-text-tertiary)]"
                        >
                            <ChevronDownIcon class="size-4" />
                        </span>
                    </Button>
                    <PopoutMenu placement="bottom-end">
                        <Button size="md" class="flex items-center gap-2"
                            ><BookmarkIcon class="size-5" />
                            <span>{{ currentFavoriteTypeLabel }}</span>
                        </Button>
                        <template #menu>
                            <div class="flex w-48 flex-col gap-1 p-1">
                                <Button
                                    v-for="option in favoriteTypeOptions"
                                    :key="option.id"
                                    :type="filter === option.id ? 'colored' : 'quiet'"
                                    class="flex w-full !justify-start text-left"
                                    :aria-pressed="filter === option.id"
                                    @click="filter = option.id"
                                >
                                    {{ option.label }}
                                </Button>
                            </div>
                        </template>
                    </PopoutMenu>
                </div>

                <div class="flex flex-wrap items-center gap-2">
                    <span class="text-sm font-medium text-[var(--color-text-tertiary)]">{{
                        formatMessage(messages.recentlySaved)
                    }}</span>
                    <PopoutMenu
                        :tooltip="formatMessage(messages.view)"
                        placement="bottom-end"
                        class="ml-auto"
                    >
                        <Button circular icon-only :aria-label="formatMessage(messages.view)"
                            ><component :is="currentDisplayMode?.icon" />
                        </Button>
                        <template #menu>
                            <div class="flex w-44 flex-col gap-1 p-1">
                                <Button
                                    v-for="option in displayModeOptions"
                                    :key="option.id"
                                    :type="displayMode === option.id ? 'colored' : 'quiet'"
                                    class="flex w-full items-center gap-2 !justify-start text-left"
                                    :aria-pressed="displayMode === option.id"
                                    @click="setDisplayMode(option.id)"
                                >
                                    <component :is="option.icon" class="size-4" />
                                    {{ option.label }}
                                </Button>
                            </div>
                        </template>
                    </PopoutMenu>
                    <Pagination
                        :page="displayedPage"
                        :count="pageCount"
                        @switch-page="currentPage = $event"
                    />
                </div>
            </div>
        </div>

        <p
            v-if="loadingProjects && committedView"
            role="status"
            class="m-0 text-sm text-[var(--color-text-tertiary)]"
        >
            {{ formatMessage(messages.refreshing) }}
        </p>
        <Admonition
            v-if="projectsLoadFailed && hasProjectsSnapshot"
            type="warning"
            :header="formatMessage(messages.loadFailed)"
        >
            <Button type="outlined" :loading="loadingProjects" @click="refreshProjects">{{
                formatMessage(commonMessages.retryButton)
            }}</Button>
        </Admonition>
        <EmptyState
            v-if="projectsLoadFailed && !hasProjectsSnapshot"
            type="error"
            :heading="formatMessage(messages.loadFailed)"
        >
            <template #actions>
                <Button type="outlined" :loading="loadingProjects" @click="refreshProjects">{{
                    formatMessage(commonMessages.retryButton)
                }}</Button>
            </template>
        </EmptyState>
        <section
            v-else-if="!committedView || (!contentFavorites.loaded.value && !hasProjectsSnapshot)"
            aria-busy="true"
        >
            <ProjectCardList :layout="displayMode">
                <ProjectCardSkeleton v-for="index in 6" :key="index" :layout="displayMode" />
            </ProjectCardList>
        </section>
        <EmptyState
            v-else-if="
                contentFavorites.loaded.value && contentFavorites.favorites.value.length === 0
            "
            type="empty-inbox"
            :heading="formatMessage(messages.emptyTitle)"
            :description="formatMessage(messages.emptyDescription)"
        >
            <template #actions>
                <Button type="colored" color="brand" @click="browseResources">{{
                    formatMessage(messages.browseResources)
                }}</Button>
            </template>
        </EmptyState>
        <EmptyState
            v-else-if="displayRows.length === 0"
            type="no-search-result"
            :heading="formatMessage(messages.noMatchesTitle)"
            :description="formatMessage(messages.noMatchesDescription)"
        >
            <template #actions>
                <Button type="outlined" @click="clearFilters">{{
                    formatMessage(messages.clearFilters)
                }}</Button>
            </template>
        </EmptyState>
        <ProjectCardList v-else :layout="displayMode" :aria-busy="loadingProjects">
            <template
                v-for="row in pagedRows"
                :key="contentFavoriteKey(row.favorite.provider, row.favorite.project_id)"
            >
                <ProjectCardSkeleton v-if="!row.project" :layout="displayMode" />
                <ProjectCard
                    v-else
                    :layout="displayMode"
                    :link="getProjectLink(row.project)"
                    :title="row.project.title"
                    :summary="row.project.description"
                    :icon-url="row.project.iconUrl"
                    :downloads="row.project.downloads"
                    :tags="row.project.categories"
                    :date-published="row.project.dateCreated"
                    :date-updated="row.project.dateModified"
                    :banner="row.project.banner"
                    :color="row.project.color"
                    :environment="row.project.environment"
                    :provider="row.project.provider"
                >
                    <template #actions>
                        <div class="flex gap-2">
                            <Button
                                v-if="!row.project.unavailable"
                                color="brand"
                                type="outlined"
                                :size="displayMode === 'compact' ? '2xs' : 'md'"
                                :disabled="isInstalling(row.project)"
                                @click.stop="toggleProjectSelection(row.project)"
                            >
                                <SpinnerIcon
                                    v-if="isInstalling(row.project)"
                                    class="animate-spin"
                                />
                                <CheckIcon v-else-if="isSelected(row.project)" />
                                <PlusIcon v-else />
                                {{ installLabel(row.project) }}
                            </Button>
                            <Button
                                v-tooltip="formatMessage(messages.remove)"
                                circular
                                icon-only
                                color="brand"
                                type="quiet"
                                :size="displayMode === 'compact' ? '2xs' : 'md'"
                                :disabled="
                                    contentFavorites.isPending(
                                        row.project.provider,
                                        row.project.projectId,
                                    )
                                "
                                :label="formatMessage(messages.remove)"
                                @click.stop="removeFavorite(row.project)"
                            >
                                <SpinnerIcon
                                    v-if="
                                        contentFavorites.isPending(
                                            row.project.provider,
                                            row.project.projectId,
                                        )
                                    "
                                    class="animate-spin"
                                />
                                <BookmarkFilledIcon v-else />
                            </Button>
                        </div>
                    </template>
                </ProjectCard>
            </template>
        </ProjectCardList>

        <div class="flex justify-end">
            <Pagination
                :page="displayedPage"
                :count="pageCount"
                @switch-page="currentPage = $event"
            />
        </div>

        <SelectedProjectsFloatingBar :install-context="installContext" />
        <BrowseInstanceSelector
            ref="instanceSelector"
            :instances="contentSelection.instances.value"
            :selected-instance="contentSelection.targetInstance.value"
            :selected-count="contentSelection.selectedCount.value"
            :install-current="contentSelection.installSelected"
            :clear-current="contentSelection.clear"
            @select="selectTargetInstance"
        />
    </div>
</template>

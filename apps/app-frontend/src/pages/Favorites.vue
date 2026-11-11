<script setup lang="ts">
import type { Labrinth } from '@modrinth/api-client'
import {
    BookmarkFilledIcon,
    BookmarkIcon,
    CheckIcon,
    ChevronDownIcon,
    FolderIcon,
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
    Checkbox,
    commonMessages,
    defineMessages,
    EmptyState,
    type FilterType,
    type FilterValue,
    formatCategory,
    getTargetInstallPreferences,
    injectNotificationManager,
    NavTabs,
    Pagination,
    PopoutMenu,
    ProjectCard,
    ProjectCardList,
    ProjectCardSkeleton,
    SelectAllButton,
    SelectedProjectsFloatingBar,
    StyledInput,
    useStickyObserver,
    useVIntl,
} from '@modrinth/ui'
import { computed, nextTick, onBeforeUnmount, onMounted, ref, shallowRef, watch } from 'vue'
import { onBeforeRouteLeave, useRoute, useRouter } from 'vue-router'

import BrowseInstanceSelector from '@/components/browse/BrowseInstanceSelector.vue'
import FavoriteGroupModal from '@/components/ui/favorites/FavoriteGroupModal.vue'
import FavoritesSidebar, {
    type FavoriteGroupEntry,
} from '@/components/ui/favorites/FavoritesSidebar.vue'
import InstanceIcon from '@/components/ui/InstanceIcon.vue'
import { useContentFavorites } from '@/composables/useContentFavorites'
import { useFavoriteGroups } from '@/composables/useFavoriteGroups'
import { createBrowseProjectTabs, getBrowseProjectTabOptions } from '@/helpers/browse-project-tabs'
import {
    type ContentFavorite,
    contentFavoriteKey,
    type FavoriteContentType,
    type FavoriteProvider,
    prefetchContentFavorites,
    resolveContentFavorite,
} from '@/helpers/content-favorites'
import { translateSearchHitTitles } from '@/helpers/content-search'
import { instance_listener } from '@/helpers/events.js'
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
import i18n from '@/i18n.config'
import { injectContentSelection, makeContentSelectionKey } from '@/providers/content-selection'
import { useBreadcrumbs } from '@/store/breadcrumbs'
import { isBrowseReturnSourcePath, useNavigationReturnStore } from '@/store/navigation-return'

type FavoriteFilter = 'all' | FavoriteContentType
type FavoriteDisplayMode = 'list' | 'compact' | 'grid'
type FavoriteGroupFilter = 'all' | 'none' | string

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
    tags: string[]
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
    group: FavoriteGroupFilter
    categories: string[]
    page: number
    projects: FavoriteProject[]
}

type FavoritesReturnState = {
    projects: FavoriteProject[]
}

const PAGE_SIZE = 20
const GROUP_ALL: FavoriteGroupFilter = 'all'
const GROUP_NONE: FavoriteGroupFilter = 'none'
const INSTALLED_CONTENT_EVENTS = [
    'synced',
    'content_changed',
    'content_install_finished',
    'content_install_failed',
]
const FAVORITE_PROJECT_TYPES: FilterType['supported_project_types'] = [
    'mod',
    'resourcepack',
    'datapack',
    'shader',
]

const route = useRoute()
const router = useRouter()
const breadcrumbs = useBreadcrumbs()
const navReturn = useNavigationReturnStore()
const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const contentSelection = injectContentSelection()
const contentFavorites = useContentFavorites()
const favoriteGroups = useFavoriteGroups()
const instanceSelector = ref<InstanceType<typeof BrowseInstanceSelector>>()
const favoriteGroupModal = ref<InstanceType<typeof FavoriteGroupModal> | null>(null)
const groupModalTarget = ref<
    { type: 'create'; assignFavoriteKey?: string } | { type: 'rename'; id: string } | null
>(null)
const selectingFiltered = ref<{ completed: number; total: number } | null>(null)
const stickyChromeRef = ref<HTMLElement | null>(null)
const { isStuck: isChromeStuck } = useStickyObserver(stickyChromeRef, 'FavoritesToolbarChrome')
const projects = shallowRef<FavoriteProject[]>([])
const localizedTitles = shallowRef<ReadonlyMap<string, string>>(new Map())
const requestedTitleLookups = new Set<string>()
let projectsController: AbortController | null = null
let unlistenInstances: (() => void) | null = null
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
    schematics: { id: 'app.browse.schematics-project-type', defaultMessage: 'Schematics' },
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
    remove: {
        id: 'app.content-favorites.remove',
        defaultMessage: 'Remove from favorites',
    },
    allFavorites: {
        id: 'app.content-favorites.groups.all',
        defaultMessage: 'All favorites',
    },
    ungrouped: {
        id: 'app.content-favorites.groups.ungrouped',
        defaultMessage: 'No group',
    },
    manageGroups: {
        id: 'app.content-favorites.groups.manage',
        defaultMessage: 'Groups',
    },
    categories: {
        id: 'app.content-favorites.filters.categories',
        defaultMessage: 'Categories',
    },
    newGroup: {
        id: 'app.content-favorites.groups.new',
        defaultMessage: 'New group',
    },
    noGroups: {
        id: 'app.content-favorites.groups.none',
        defaultMessage: 'No groups yet',
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
        defaultMessage: 'Try a different search, content type, group, or category.',
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

function parseFavoriteGroup(value: unknown): FavoriteGroupFilter {
    const group = queryValue(value)
    return group || GROUP_ALL
}

function parseCategories(value: unknown): string[] {
    const raw = Array.isArray(value) ? value.filter((entry) => typeof entry === 'string') : [value]
    return [
        ...new Set(
            raw
                .flatMap((entry) => queryValue(entry).split(','))
                .map((entry) => entry.trim())
                .filter(Boolean),
        ),
    ]
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
    const { q: _q, kind: _kind, group: _group, cat: _cat, page: _page, ...query } = route.query
    void router.push({ path: '/browse/mod', query })
}

function clearFilters() {
    updateQuery({
        q: undefined,
        kind: undefined,
        group: undefined,
        cat: undefined,
        page: undefined,
    })
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

const groupFilter = computed<FavoriteGroupFilter>({
    get: () => parseFavoriteGroup(route.query.group),
    set: (value) =>
        updateQuery({ group: value === GROUP_ALL ? undefined : value, page: undefined }),
})

const categoryFilters = computed<FilterValue[]>({
    get: () =>
        parseCategories(route.query.cat).map((category) => ({
            type: 'category',
            option: category,
            negative: false,
        })),
    set: (filters) =>
        updateQuery({
            cat:
                filters
                    .filter((entry) => !entry.negative)
                    .map((entry) => entry.option)
                    .join(',') || undefined,
            page: undefined,
        }),
})

const selectedCategories = computed(() =>
    categoryFilters.value.filter((entry) => !entry.negative).map((entry) => entry.option),
)

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
        if (key === 'kind' || key === 'q' || key === 'group' || key === 'cat' || key === 'page')
            continue
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
            schematics: formatMessage(messages.schematics),
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
        tags: [],
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
        tags: metadata.categories,
        dateCreated: metadata.createdAt ?? undefined,
        dateModified: metadata.updatedAt ?? undefined,
        banner: metadata.gallery[0],
        unavailable: false,
    }
}

// A translation depends on the language and on the source title and slug, so a
// change to any of them is looked up again.
function projectTitleKey(project: FavoriteProject) {
    return [
        i18n.global.locale.value,
        project.provider,
        project.projectId,
        project.slug,
        project.title,
    ].join(':')
}

function displayTitle(project: FavoriteProject) {
    return localizedTitles.value.get(projectTitleKey(project)) ?? project.title
}

// Projects are published one at a time, so only the new ones are looked up.
watch(
    [projects, () => i18n.global.locale.value] as const,
    ([current, locale]) => {
        const pending = current.flatMap((project) => {
            const { provider } = project
            const key = projectTitleKey(project)
            if (
                project.unavailable ||
                requestedTitleLookups.has(key) ||
                (provider !== 'modrinth' && provider !== 'curseforge')
            )
                return []
            requestedTitleLookups.add(key)
            return [{ key, provider, slug: project.slug, title: project.title }]
        })
        if (pending.length === 0) return
        translateSearchHitTitles(pending, locale, { rethrowLookupFailure: true })
            .then((translated) => {
                const next = new Map(localizedTitles.value)
                for (const { key, title } of translated) next.set(key, title)
                localizedTitles.value = next
            })
            .catch(() => {
                for (const { key } of pending) requestedTitleLookups.delete(key)
            })
    },
    { immediate: true },
)

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

const chromeHeight = ref(0)
let chromeObserver: ResizeObserver | null = null

onBeforeUnmount(() => {
    disposed = true
    unlistenInstances?.()
    unlistenInstances = null
    projectsController?.abort()
    chromeObserver?.disconnect()
    chromeObserver = null
})

const activeFavoriteIdentities = computed(
    () => new Set(contentFavorites.favorites.value.map((favorite) => JSON.stringify(favorite))),
)

const availableProjects = computed(() =>
    projects.value
        .filter((project) => activeFavoriteIdentities.value.has(JSON.stringify(project.favorite)))
        .sort((left, right) => right.favorite.saved_at - left.favorite.saved_at),
)

const favoriteGroupEntries = computed<FavoriteGroupEntry[]>(() => {
    const favoriteKeys = contentFavorites.favorites.value.map((favorite) =>
        contentFavoriteKey(favorite.provider, favorite.project_id),
    )
    const counts = new Map<string, number>()
    let ungrouped = 0
    for (const key of favoriteKeys) {
        const groups = favoriteGroups.groupsFor(key)
        if (groups.length === 0) ungrouped++
        for (const groupId of groups) {
            counts.set(groupId, (counts.get(groupId) ?? 0) + 1)
        }
    }
    const entries: FavoriteGroupEntry[] = [
        {
            id: GROUP_ALL,
            name: formatMessage(messages.allFavorites),
            count: favoriteKeys.length,
            custom: false,
        },
    ]
    if (favoriteGroups.groups.value.length > 0) {
        entries.push({
            id: GROUP_NONE,
            name: formatMessage(messages.ungrouped),
            count: ungrouped,
            custom: false,
        })
    }
    for (const group of favoriteGroups.groups.value) {
        entries.push({
            id: group.id,
            name: group.name,
            count: counts.get(group.id) ?? 0,
            custom: true,
        })
    }
    return entries
})

const categoryOptions = computed(() => {
    const counts = new Map<string, number>()
    for (const project of availableProjects.value) {
        for (const tag of project.tags) {
            counts.set(tag, (counts.get(tag) ?? 0) + 1)
        }
    }
    return [...counts.entries()]
        .map(([tag, count]) => ({ tag, label: formatCategory(formatMessage, tag), count }))
        .sort((left, right) => left.label.localeCompare(right.label))
})

const categoryFilterType = computed<FilterType>(() => ({
    id: 'category',
    formatted_name: formatMessage(messages.categories),
    options: categoryOptions.value.map(({ tag, label }) => ({
        id: tag,
        formatted_name: label,
        method: 'and',
        value: `categories:${tag}`,
    })),
    supported_project_types: FAVORITE_PROJECT_TYPES,
    query_param: 'cat',
    supports_negative_filter: false,
    searchable: true,
    display: 'all',
}))

function favoriteKeyOf(project: FavoriteProject) {
    // Groups follow the favorite the user saved, not the resource's default
    // project: a favorite can resolve to a different counterpart project.
    return contentFavoriteKey(project.favorite.provider, project.favorite.project_id)
}

function matchesGroup(project: FavoriteProject, group: FavoriteGroupFilter) {
    if (group === GROUP_ALL) return true
    const groups = favoriteGroups.groupsFor(favoriteKeyOf(project))
    if (group === GROUP_NONE) return groups.length === 0
    return groups.includes(group)
}

function matchesFilter(
    project: FavoriteProject,
    query: string,
    kind: FavoriteFilter,
    group: FavoriteGroupFilter,
    categories: string[],
) {
    if (kind !== 'all' && project.favorite.content_type !== kind) return false
    if (!matchesGroup(project, group)) return false
    if (categories.length && !categories.every((category) => project.tags.includes(category))) {
        return false
    }
    const normalizedSearch = query.trim().toLocaleLowerCase()
    return (
        !normalizedSearch ||
        [displayTitle(project), project.description, project.provider, project.projectId]
            .join('\n')
            .toLocaleLowerCase()
            .includes(normalizedSearch)
    )
}

watch(
    [
        search,
        filter,
        groupFilter,
        categoryFilters,
        favoriteGroups.loaded,
        favoriteGroups.memberships,
        currentPage,
        loadingProjects,
        localizedTitles,
        () => i18n.global.locale.value,
    ],
    () => {
        const groupsReady = groupFilter.value === GROUP_ALL || favoriteGroups.loaded.value
        if (!loadingProjects.value && metadataComplete.value && groupsReady) {
            const matching = availableProjects.value.filter((project) =>
                matchesFilter(
                    project,
                    search.value,
                    filter.value,
                    groupFilter.value,
                    selectedCategories.value,
                ),
            )
            committedView.value = {
                search: search.value,
                filter: filter.value,
                group: groupFilter.value,
                categories: selectedCategories.value,
                page: Math.min(
                    currentPage.value,
                    Math.max(1, Math.ceil(matching.length / PAGE_SIZE)),
                ),
                projects: matching,
            }
        } else if (
            !search.value.trim() &&
            filter.value === 'all' &&
            groupFilter.value === GROUP_ALL &&
            selectedCategories.value.length === 0 &&
            (!committedView.value ||
                (!committedView.value.search.trim() &&
                    committedView.value.filter === 'all' &&
                    committedView.value.group === GROUP_ALL &&
                    committedView.value.categories.length === 0))
        ) {
            committedView.value = {
                search: '',
                filter: 'all',
                group: GROUP_ALL,
                categories: [],
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
    if (
        view.search.trim() ||
        view.filter !== 'all' ||
        view.group !== GROUP_ALL ||
        view.categories.length
    )
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

watch([() => favoriteGroups.groups.value.map((group) => group.id).join(','), groupFilter], () => {
    const group = groupFilter.value
    if (
        favoriteGroups.loaded.value &&
        group !== GROUP_ALL &&
        group !== GROUP_NONE &&
        !favoriteGroups.groups.value.some((candidate) => candidate.id === group)
    ) {
        groupFilter.value = GROUP_ALL
    }
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

function isInstalled(project: FavoriteProject) {
    // A favorite can present another source's project than the one installed,
    // so every project it is known by has to be checked.
    const known: { provider: FavoriteProvider; projectId: string; slug?: string }[] = [
        { provider: project.provider, projectId: project.projectId, slug: project.slug },
        { provider: project.favorite.provider, projectId: project.favorite.project_id },
        ...(project.favorite.projects ?? []).map(({ source, projectId }) => ({
            provider: source,
            projectId,
        })),
    ]
    return known.some(
        ({ provider, projectId, slug }) =>
            (provider === 'modrinth' || provider === 'curseforge') &&
            contentSelection.isInstalledIdentity(provider, projectId, slug),
    )
}

function installLabel(project: FavoriteProject) {
    if (isInstalling(project)) return formatMessage(commonMessages.validatingLabel)
    if (isInstalled(project)) return formatMessage(commonMessages.installedLabel)
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

async function addProjectToSelection(project: FavoriteProject, target: GameInstance) {
    if (project.provider !== 'modrinth' && project.provider !== 'curseforge') return false
    const key = makeContentSelectionKey(project.provider, project.projectId)

    // Queue immediately like the Discover tabs: resolve the concrete version in
    // the install review/plan instead of serially fetching versions per favorite.
    return contentSelection.add({
        key,
        provider: project.provider,
        projectId: project.projectId,
        providerProjectId: project.projectId,
        contentType: project.favorite.content_type,
        title: displayTitle(project),
        iconUrl: project.iconUrl,
        slug: project.slug,
        preferences: getInstallPreferences(target, project.favorite.content_type),
        targetInstanceId: target.id,
    })
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
    if (isInstalled(project)) return

    setInstalling(key, true)
    try {
        await addProjectToSelection(project, target)
    } catch (error) {
        if (contentSelection.targetInstance.value?.id === target.id) handleError(error)
    } finally {
        setInstalling(key, false)
    }
}

const selectionBusy = computed(() =>
    ['validating', 'reviewing', 'queueing'].includes(contentSelection.state.value),
)

const filteredSelectionCandidates = computed(() => {
    const view = committedView.value
    if (!view || loadingProjects.value || !metadataComplete.value) return []
    return view.projects.filter(
        (project) =>
            activeFavoriteIdentities.value.has(JSON.stringify(project.favorite)) &&
            !project.unavailable &&
            (project.provider === 'modrinth' || project.provider === 'curseforge') &&
            !isInstalling(project) &&
            !contentSelection.isSelected(
                makeContentSelectionKey(project.provider, project.projectId),
            ) &&
            !isInstalled(project),
    )
})

async function selectAllFiltered() {
    if (selectingFiltered.value || selectionBusy.value) return

    let target = contentSelection.targetInstance.value
    if (!target) {
        try {
            target = await contentSelection.refreshInstances(queryValue(route.query.i) || undefined)
            if (target) await contentSelection.refreshInstalledIdentities()
        } catch (error) {
            handleError(error)
            return
        }
        if (!target) {
            // Match the per-card action: pick an instance first, then click again.
            instanceSelector.value?.show()
            return
        }
    }

    const candidates = [...filteredSelectionCandidates.value]
    if (candidates.length === 0) return

    selectingFiltered.value = { completed: 0, total: candidates.length }
    try {
        for (const project of candidates) {
            if (disposed || contentSelection.targetInstance.value?.id !== target.id) break
            const key = makeContentSelectionKey(project.provider, project.projectId)
            setInstalling(key, true)
            try {
                await addProjectToSelection(project, target)
            } catch (error) {
                if (contentSelection.targetInstance.value?.id === target.id) handleError(error)
            } finally {
                setInstalling(key, false)
            }
            if (selectingFiltered.value) {
                selectingFiltered.value = {
                    completed: selectingFiltered.value.completed + 1,
                    total: candidates.length,
                }
            }
        }
    } finally {
        selectingFiltered.value = null
    }
}

function openCreateGroup(assignFavoriteKey?: string) {
    groupModalTarget.value = { type: 'create', assignFavoriteKey }
    favoriteGroupModal.value?.show({ mode: 'create' })
}

function createGroupFromMenu(project: FavoriteProject, hide: () => void) {
    hide()
    openCreateGroup(favoriteKeyOf(project))
}

function openRenameGroup(groupId: string) {
    const group = favoriteGroups.groups.value.find((candidate) => candidate.id === groupId)
    if (!group) return
    groupModalTarget.value = { type: 'rename', id: groupId }
    favoriteGroupModal.value?.show({ mode: 'rename', name: group.name })
}

async function submitGroup(name: string) {
    const target = groupModalTarget.value
    groupModalTarget.value = null
    favoriteGroupModal.value?.hide()
    if (!target) return
    try {
        if (target.type === 'rename') {
            await favoriteGroups.renameGroup(target.id, name)
            return
        }
        const group = await favoriteGroups.createGroup(name)
        if (target.assignFavoriteKey) {
            await favoriteGroups.updateFavoriteGroups(target.assignFavoriteKey, [group.id], [])
        }
    } catch (error) {
        handleError(error)
    }
}

async function deleteGroup(groupId: string) {
    try {
        await favoriteGroups.deleteGroup(groupId)
        if (groupFilter.value === groupId) groupFilter.value = GROUP_ALL
    } catch (error) {
        handleError(error)
    }
}

function toggleFavoriteGroup(project: FavoriteProject, groupId: string) {
    const key = favoriteKeyOf(project)
    const remove = favoriteGroups.isInGroup(key, groupId)
    void favoriteGroups.updateFavoriteGroups(key, remove ? [] : [groupId], remove ? [groupId] : [])
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
        state: createProjectPreviewState(path, displayTitle(project)),
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
    if (stickyChromeRef.value && typeof ResizeObserver !== 'undefined') {
        chromeHeight.value = stickyChromeRef.value.offsetHeight
        chromeObserver = new ResizeObserver(() => {
            chromeHeight.value = stickyChromeRef.value?.offsetHeight ?? 0
        })
        chromeObserver.observe(stickyChromeRef.value)
    }
    void (async () => {
        try {
            const instance = await contentSelection.refreshInstances(
                queryValue(route.query.i) || undefined,
            )
            if (instance) await contentSelection.refreshInstalledIdentities()
        } catch (error) {
            handleError(error)
        }
    })()
    // Match Browse: content installed from here has to change the card in place.
    instance_listener(async (event: { event: string; instance_id: string }) => {
        if (
            event.instance_id === contentSelection.targetInstance.value?.id &&
            INSTALLED_CONTENT_EVENTS.includes(event.event)
        ) {
            await contentSelection.refreshInstalledIdentities().catch(handleError)
        }
    })
        .then((unlisten) => {
            if (disposed) unlisten()
            else unlistenInstances = unlisten
        })
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
    <div class="flex flex-col gap-3 p-6">
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
                    <NavTabs :links="projectTabs" route-sync class="min-w-0" />
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
                    <div class="ml-auto flex flex-wrap items-center gap-2">
                        <SelectAllButton
                            :count="filteredSelectionCandidates.length"
                            :progress="selectingFiltered"
                            :disabled="selectionBusy"
                            @select="selectAllFiltered"
                        />
                        <PopoutMenu :tooltip="formatMessage(messages.view)" placement="bottom-end">
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
        </div>

        <div
            class="favorites-layout"
            :style="{ '--favorites-sidebar-top': `${chromeHeight + 12}px` }"
        >
            <FavoritesSidebar
                v-if="contentFavorites.favorites.value.length > 0"
                :entries="favoriteGroupEntries"
                :active-group="groupFilter"
                :category-filter-type="categoryFilterType"
                :category-filters="categoryFilters"
                @select-group="groupFilter = $event"
                @create-group="openCreateGroup()"
                @rename-group="openRenameGroup"
                @delete-group="deleteGroup"
                @update:category-filters="categoryFilters = $event"
            />

            <div
                class="flex min-w-0 flex-col gap-3"
                :class="{
                    'col-span-full': contentFavorites.favorites.value.length === 0,
                }"
            >
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
                        <Button
                            type="outlined"
                            :loading="loadingProjects"
                            @click="refreshProjects"
                            >{{ formatMessage(commonMessages.retryButton) }}</Button
                        >
                    </template>
                </EmptyState>
                <section
                    v-else-if="
                        !committedView || (!contentFavorites.loaded.value && !hasProjectsSnapshot)
                    "
                    aria-busy="true"
                >
                    <ProjectCardList :layout="displayMode">
                        <ProjectCardSkeleton
                            v-for="index in 6"
                            :key="index"
                            :layout="displayMode"
                        />
                    </ProjectCardList>
                </section>
                <EmptyState
                    v-else-if="
                        contentFavorites.loaded.value &&
                        contentFavorites.favorites.value.length === 0
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
                            :title="displayTitle(row.project)"
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
                                        :color="
                                            isSelected(row.project) || isInstalled(row.project)
                                                ? 'green'
                                                : 'brand'
                                        "
                                        type="outlined"
                                        :size="displayMode === 'compact' ? '2xs' : 'md'"
                                        :disabled="
                                            isInstalling(row.project) || isInstalled(row.project)
                                        "
                                        @click.stop="toggleProjectSelection(row.project)"
                                    >
                                        <SpinnerIcon
                                            v-if="isInstalling(row.project)"
                                            class="animate-spin"
                                        />
                                        <CheckIcon
                                            v-else-if="
                                                isSelected(row.project) || isInstalled(row.project)
                                            "
                                        />
                                        <PlusIcon v-else />
                                        {{ installLabel(row.project) }}
                                    </Button>
                                    <PopoutMenu placement="bottom-end">
                                        <Button
                                            v-tooltip="formatMessage(messages.manageGroups)"
                                            circular
                                            icon-only
                                            type="quiet"
                                            :size="displayMode === 'compact' ? '2xs' : 'md'"
                                            :aria-label="formatMessage(messages.manageGroups)"
                                        >
                                            <FolderIcon />
                                        </Button>
                                        <template #menu="{ hide }">
                                            <div class="flex w-48 flex-col gap-1 p-1">
                                                <p
                                                    v-if="favoriteGroups.groups.value.length === 0"
                                                    class="m-0 px-2 py-1 text-sm text-[var(--color-text-tertiary)]"
                                                >
                                                    {{ formatMessage(messages.noGroups) }}
                                                </p>
                                                <Checkbox
                                                    v-for="group in favoriteGroups.groups.value"
                                                    :key="group.id"
                                                    :model-value="
                                                        favoriteGroups.isInGroup(
                                                            favoriteKeyOf(row.project),
                                                            group.id,
                                                        )
                                                    "
                                                    :label="group.name"
                                                    class="w-full px-2 py-1"
                                                    @update:model-value="
                                                        toggleFavoriteGroup(row.project, group.id)
                                                    "
                                                    @click.stop
                                                />
                                                <div class="my-1 h-px bg-surface-4" />
                                                <Button
                                                    type="quiet"
                                                    class="w-full !justify-start"
                                                    @click.stop="
                                                        createGroupFromMenu(row.project, hide)
                                                    "
                                                >
                                                    <PlusIcon />
                                                    {{ formatMessage(messages.newGroup) }}
                                                </Button>
                                            </div>
                                        </template>
                                    </PopoutMenu>
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
            </div>
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
        <FavoriteGroupModal ref="favoriteGroupModal" @submit="submitGroup" />
    </div>
</template>

<style scoped>
.favorites-layout {
    display: grid;
    grid-template-columns: minmax(14rem, 16rem) minmax(0, 1fr);
    gap: var(--gap-md);
    align-items: start;
    min-width: 0;
}

@media (max-width: 900px) {
    .favorites-layout {
        grid-template-columns: minmax(0, 1fr);
    }
}
</style>

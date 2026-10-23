<script setup lang="ts">
import BookmarkIcon from '@modrinth/assets/icons/bookmark.svg?component'
import BookmarkFilledIcon from '@modrinth/assets/icons/bookmark-filled.svg?component'
import CheckIcon from '@modrinth/assets/icons/check.svg?component'
import ChevronDownIcon from '@modrinth/assets/icons/chevron-down.svg?component'
import GenericListIcon from '@modrinth/assets/icons/generic-list.svg?component'
import GridIcon from '@modrinth/assets/icons/grid.svg?component'
import ListIcon from '@modrinth/assets/icons/list.svg?component'
import PlusIcon from '@modrinth/assets/icons/plus.svg?component'
import SearchIcon from '@modrinth/assets/icons/search.svg?component'
import SpinnerIcon from '@modrinth/assets/icons/spinner.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import EmptyState from '@modrinth/ui/src/components/base/EmptyState.vue'
import NavTabs from '@modrinth/ui/src/components/base/NavTabs.vue'
import Pagination from '@modrinth/ui/src/components/base/Pagination.vue'
import PopoutMenu from '@modrinth/ui/src/components/base/PopoutMenu.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import ProjectCardList from '@modrinth/ui/src/components/project/ProjectCardList.vue'
import ProjectCardSkeleton from '@modrinth/ui/src/components/project/ProjectCardSkeleton.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { useStickyObserver } from '@modrinth/ui/src/composables/sticky-observer.ts'
import {
    getLatestMatchingInstallVersion,
    getTargetInstallPreferences,
} from '@modrinth/ui/src/layouts/shared/browse-tab/composables/install-logic.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { computed, onMounted, onScopeDispose, ref, shallowRef, watch } from 'vue'
import { onBeforeRouteLeave, useRoute, useRouter } from 'vue-router'

import { type ProjectDetail, projectIcon } from '../../data'
import { usePreview } from '../../lib/state'
import type { GameInstance } from '../../lib/types'
import { injectPreviewWindow } from '../../lib/window'
import InstanceIcon from '../../shell/InstanceIcon.vue'
import type { BrowseInstallContext } from './browse-context'
import BrowseInstallHeader from './components/BrowseInstallHeader.vue'
import BrowseInstanceSelector from './components/BrowseInstanceSelector.vue'
import InstallModals from './components/InstallModals.vue'
import ProjectCard from './components/ProjectCard.vue'
import SelectedProjectsFloatingBar from './components/SelectedProjectsFloatingBar.vue'
import { createContentInstall, displayInstanceIcon, provideContentInstall } from './install'
import { messages } from './messages'
import { projectCategories } from './search-engine'
import {
    type BrowseDisplayMode,
    type ContentFavorite,
    type FavoriteContentType,
    makeContentSelectionKey,
    useDiscover,
} from './state'

/**
 * Favorites (apps/app-frontend/src/pages/Favorites.vue): the saved content of this session,
 * resolved against the Modrinth snapshot, with search, the content type menu, the display
 * mode, pagination and the install cart.
 */
type FavoriteFilter = 'all' | FavoriteContentType

type FavoriteProject = {
    favorite: ContentFavorite
    provider: 'modrinth'
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
    unavailable: boolean
}

const PAGE_SIZE = 20

const route = useRoute()
const router = useRouter()
const preview = usePreview()
const discover = useDiscover()
const selection = discover.selection
const { formatMessage } = useVIntl()
const { addNotification, handleError } = injectNotificationManager()
const { layer } = injectPreviewWindow()
const contentInstall = createContentInstall({ preview, discover, router })
provideContentInstall(contentInstall)

const instanceSelector = ref<InstanceType<typeof BrowseInstanceSelector>>()
const stickyChromeRef = ref<HTMLElement | null>(null)
const { isStuck: isChromeStuck } = useStickyObserver(stickyChromeRef, 'FavoritesToolbarChrome')
const projects = shallowRef<FavoriteProject[]>([])
const loadingProjects = ref(false)
const hasProjectsSnapshot = ref(false)
const installingKeys = ref(new Set<string>())
let projectRequestId = 0

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
    void router.replace({ query: { ...route.query, ...values } })
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
    { id: 'mod' as const, label: formatMessage(messages.tabMods) },
    { id: 'resourcepack' as const, label: formatMessage(messages.tabResourcepacks) },
    { id: 'datapack' as const, label: formatMessage(messages.tabDatapacks) },
    { id: 'shader' as const, label: formatMessage(messages.tabShaders) },
])

const currentFavoriteTypeLabel = computed(
    () => favoriteTypeOptions.value.find((option) => option.id === filter.value)?.label,
)

const displayModeOptions = computed(() => [
    { id: 'list' as const, label: formatMessage(messages.listView), icon: ListIcon },
    {
        id: 'compact' as const,
        label: formatMessage(messages.compactListView),
        icon: GenericListIcon,
    },
    { id: 'grid' as const, label: formatMessage(messages.gridView), icon: GridIcon },
])

const displayMode = computed(() => discover.displayMode.value)
const currentDisplayMode = computed(() =>
    displayModeOptions.value.find((option) => option.id === displayMode.value),
)

/** helpers/browse-project-tabs.ts `supportsDataPacks`. */
function supportsDataPacks(gameVersion: string | undefined) {
    const match = gameVersion?.match(/^1\.(\d+)/)
    return match ? Number(match[1]) >= 13 : false
}

const projectTabs = computed(() => {
    const query = new URLSearchParams()
    for (const [key, value] of Object.entries(route.query)) {
        if (key === 'kind' || key === 'q' || key === 'page') continue
        if (Array.isArray(value)) {
            for (const entry of value) if (entry != null) query.append(key, entry)
        } else if (value) {
            query.set(key, value)
        }
    }
    const suffix = query.size ? `?${query.toString()}` : ''
    const instance = selection.targetInstance.value
    const hasInstanceContext = !!route.query.i
    return [
        {
            label: formatMessage(messages.tabModpacks),
            href: `/browse/modpack${suffix}`,
            shown: !hasInstanceContext,
        },
        {
            label: formatMessage(messages.tabMods),
            href: `/browse/mod${suffix}`,
            shown: !instance || instance.loader !== 'vanilla',
        },
        { label: formatMessage(messages.tabResourcepacks), href: `/browse/resourcepack${suffix}` },
        {
            label: formatMessage(messages.tabDatapacks),
            href: `/browse/datapack${suffix}`,
            shown: !instance || supportsDataPacks(instance.game_version),
        },
        { label: formatMessage(messages.tabMaps), href: `/browse/world${suffix}` },
        { label: formatMessage(messages.tabShaders), href: `/browse/shader${suffix}` },
        {
            label: formatMessage(messages.tabServers),
            href: `/browse/server${suffix}`,
            shown: !hasInstanceContext,
        },
        {
            label: formatMessage(messages.favoritesTitle),
            href: `/browse/favorites${suffix}`,
            onboardingId: 'browse-favorites-tab',
        },
    ]
})

function typeLabel(type: FavoriteContentType) {
    return formatMessage(
        {
            mod: messages.tabMods,
            resourcepack: messages.tabResourcepacks,
            datapack: messages.tabDatapacks,
            shader: messages.tabShaders,
        }[type],
    )
}

function toUnavailable(favorite: ContentFavorite): FavoriteProject {
    return {
        favorite,
        provider: 'modrinth',
        projectId: favorite.project_id,
        title: formatMessage(messages.favoritesUnavailableTitle, {
            provider: 'Modrinth',
            projectId: favorite.project_id,
        }),
        description: formatMessage(messages.favoritesUnavailableDescription),
        categories: [typeLabel(favorite.content_type)],
        unavailable: true,
    }
}

function toProject(favorite: ContentFavorite, detail: ProjectDetail): FavoriteProject {
    return {
        favorite,
        provider: 'modrinth',
        projectId: detail.id,
        title: detail.title,
        description: detail.description,
        slug: detail.slug ?? undefined,
        iconUrl: projectIcon(detail.id),
        downloads: detail.downloads,
        categories: [typeLabel(favorite.content_type), ...projectCategories(detail)],
        dateCreated: detail.published ?? undefined,
        dateModified: detail.updated ?? detail.date_modified ?? undefined,
        banner: detail.gallery[0]?.url,
        unavailable: false,
    }
}

/** `restoreCachedProjects`: favorites whose metadata is already loaded show at once. */
function restoreCachedProjects() {
    if (!discover.favoritesLoaded.value) return
    const cached = discover.favorites.value.flatMap((favorite) => {
        const detail = discover.details.value[favorite.project_id]
        if (detail) return [toProject(favorite, detail)]
        const previous = projects.value.find(
            (project) => JSON.stringify(project.favorite) === JSON.stringify(favorite),
        )
        return previous ? [previous] : []
    })
    if (cached.length || discover.favorites.value.length === 0) {
        projects.value = cached
        hasProjectsSnapshot.value = true
    }
}

restoreCachedProjects()

/** Resolves the favorites against the snapshot (the app's favorites table + resource metadata). */
async function refreshProjects() {
    const requestId = ++projectRequestId
    loadingProjects.value = true
    try {
        const favorites = [...discover.favorites.value]
        restoreCachedProjects()
        const resolved = await Promise.all(
            favorites.map(async (favorite): Promise<FavoriteProject> => {
                const detail = await discover.ensureDetail(favorite.project_id)
                return detail ? toProject(favorite, detail) : toUnavailable(favorite)
            }),
        )
        if (requestId === projectRequestId) {
            projects.value = resolved
            hasProjectsSnapshot.value = true
        }
    } catch (error) {
        handleError(error)
    } finally {
        if (requestId === projectRequestId) loadingProjects.value = false
    }
}

const activeFavoriteKeys = computed(
    () =>
        new Set(
            discover.favorites.value.map(
                (favorite) => `${favorite.provider}:${favorite.project_id}`,
            ),
        ),
)

const availableProjects = computed(() =>
    projects.value
        .filter((project) =>
            activeFavoriteKeys.value.has(
                `${project.favorite.provider}:${project.favorite.project_id}`,
            ),
        )
        .sort((left, right) => right.favorite.saved_at - left.favorite.saved_at),
)

const filteredProjects = computed(() => {
    const normalizedSearch = search.value.trim().toLocaleLowerCase()
    return availableProjects.value.filter((project) => {
        if (filter.value !== 'all' && project.favorite.content_type !== filter.value) return false
        if (!normalizedSearch) return true
        return [project.title, project.description, project.provider, project.projectId]
            .join('\n')
            .toLocaleLowerCase()
            .includes(normalizedSearch)
    })
})

const pageCount = computed(() => Math.max(1, Math.ceil(filteredProjects.value.length / PAGE_SIZE)))
const pagedProjects = computed(() => {
    const page = Math.min(currentPage.value, pageCount.value)
    const offset = (page - 1) * PAGE_SIZE
    return filteredProjects.value.slice(offset, offset + PAGE_SIZE)
})

watch([pageCount, loadingProjects], ([count, loading]) => {
    if (!loading && currentPage.value > count) currentPage.value = count
})

function installSelected() {
    return selection.installSelected(() =>
        addNotification({ title: formatMessage(messages.selectionTargetChanged), type: 'error' }),
    )
}

const installContext = computed<BrowseInstallContext | null>(() => {
    const target = selection.targetInstance.value
    if (!target) return null
    const icon = displayInstanceIcon(target)
    const processing = ['validating', 'reviewing', 'queueing'].includes(selection.state.value)
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
        installSelected,
    }
})

function isInstalling(project: FavoriteProject) {
    const key = makeContentSelectionKey(project.provider, project.projectId)
    return installingKeys.value.has(key) || selection.isInstalling(key)
}

function isSelected(project: FavoriteProject) {
    return selection.isSelected(makeContentSelectionKey(project.provider, project.projectId))
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

async function toggleProjectSelection(project: FavoriteProject) {
    if (project.unavailable) return
    if (!selection.targetInstance.value) {
        selection.refreshInstances(queryValue(route.query.i) || undefined)
        instanceSelector.value?.show()
        return
    }
    const target = selection.targetInstance.value
    const key = makeContentSelectionKey(project.provider, project.projectId)
    if (selection.isSelected(key)) {
        selection.remove(key)
        return
    }
    setInstalling(key, true)
    try {
        const preferences = getTargetInstallPreferences(
            { gameVersion: target.game_version, loader: target.loader },
            project.favorite.content_type,
        )
        const detail = await discover.ensureDetail(project.projectId)
        const versionId = (
            getLatestMatchingInstallVersion(
                (detail?.versions ?? []) as unknown as Parameters<
                    typeof getLatestMatchingInstallVersion
                >[0],
                preferences,
            ) as unknown as { id: string } | undefined
        )?.id
        if (selection.targetInstance.value?.id !== target.id) return
        if (!versionId) throw new Error(formatMessage(messages.noCompatibleVersion))
        selection.add(
            {
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
            },
            (title) =>
                addNotification({
                    title: formatMessage(messages.selectionDuplicate, { project: title }),
                    type: 'error',
                }),
        )
    } catch (error) {
        if (selection.targetInstance.value?.id === target.id) handleError(error)
    } finally {
        setInstalling(key, false)
    }
}

async function removeFavorite(project: FavoriteProject) {
    try {
        await discover.removeFavorite(project.favorite.provider, project.favorite.project_id)
    } catch (error) {
        handleError(error)
    }
}

async function selectTargetInstance(target: GameInstance) {
    selection.setTarget(target)
    await router.replace({ query: { ...route.query, i: target.id } })
}

function setDisplayMode(mode: BrowseDisplayMode) {
    discover.displayMode.value = mode
}

/** helpers/project-links.ts `createProjectDetailsQuery`. */
function getProjectLink(project: FavoriteProject) {
    if (project.unavailable) return undefined
    const query = { ...route.query, b: route.fullPath }
    delete query.g
    delete query.l
    delete query.c
    delete query.page
    return { path: `/project/${project.slug ?? project.projectId}`, query }
}

onBeforeRouteLeave(() => {
    preview.breadcrumbs.setContext({
        name: formatMessage(messages.favoritesTitle),
        link: '/browse/favorites',
        query: Object.fromEntries(
            Object.entries(route.query).filter(
                (entry): entry is [string, string] => typeof entry[1] === 'string',
            ),
        ),
    })
})

watch(
    () =>
        discover.favorites.value
            .map((favorite) => `${favorite.provider}:${favorite.project_id}`)
            .join('|'),
    () => void refreshProjects(),
)

let cancelFirstLoad: (() => void) | null = null
onMounted(() => {
    preview.breadcrumbs.setName('FavoritesTitle', formatMessage(messages.favoritesTitle))
    selection.refreshInstances(queryValue(route.query.i) || undefined)
    loadingProjects.value = true
    // Match Browse: the first read waits for the page-enter paint.
    cancelFirstLoad = preview.clock.after(220, () => {
        cancelFirstLoad = null
        void refreshProjects()
    })
})
onScopeDispose(() => cancelFirstLoad?.())
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
                        :placeholder="formatMessage(messages.favoritesSearch)"
                        wrapper-class="flex-1"
                        input-class="h-12"
                    />
                    <Button
                        size="md"
                        class="flex min-w-0 items-center gap-2"
                        @click="instanceSelector?.show()"
                        ><InstanceIcon
                            v-if="selection.targetInstance.value"
                            class="shrink-0"
                            size="1.25rem"
                            :icon-path="selection.targetInstance.value.icon_path"
                            :instance-id="selection.targetInstance.value.id"
                            :loader="selection.targetInstance.value.loader"
                        />
                        <PlusIcon v-else class="size-5 shrink-0" />
                        <span class="max-w-40 truncate font-medium">
                            {{
                                selection.targetInstance.value?.name ??
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
                    <PopoutMenu placement="bottom-end" :container="layer ?? undefined">
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
                        :tooltip="formatMessage(messages.switchView)"
                        placement="bottom-end"
                        class="ml-auto"
                        :container="layer ?? undefined"
                    >
                        <Button circular icon-only :aria-label="formatMessage(messages.switchView)"
                            ><component :is="currentDisplayMode?.icon" />
                        </Button>
                        <template #menu>
                            <div class="flex w-44 flex-col gap-1 p-1">
                                <Button
                                    v-for="option in displayModeOptions"
                                    :key="option.id"
                                    :type="displayMode === option.id ? 'colored' : 'quiet'"
                                    class="flex w-full items-center !justify-start gap-2 text-left"
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
                        :page="currentPage"
                        :count="pageCount"
                        @switch-page="currentPage = $event"
                    />
                </div>
            </div>
        </div>

        <section
            v-if="
                !hasProjectsSnapshot ||
                (loadingProjects &&
                    filteredProjects.length === 0 &&
                    discover.favorites.value.length > 0)
            "
            aria-busy="true"
        >
            <ProjectCardList :layout="displayMode">
                <ProjectCardSkeleton v-for="index in 6" :key="index" :layout="displayMode" />
            </ProjectCardList>
        </section>
        <EmptyState
            v-else-if="discover.favoritesLoaded.value && discover.favorites.value.length === 0"
            type="empty-inbox"
            :heading="formatMessage(messages.favoritesEmptyTitle)"
            :description="formatMessage(messages.favoritesEmptyDescription)"
        >
            <template #actions>
                <Button type="colored" color="brand" @click="browseResources">{{
                    formatMessage(messages.favoritesBrowseResources)
                }}</Button>
            </template>
        </EmptyState>
        <EmptyState
            v-else-if="filteredProjects.length === 0"
            type="no-search-result"
            :heading="formatMessage(messages.favoritesNoMatchesTitle)"
            :description="formatMessage(messages.favoritesNoMatchesDescription)"
        >
            <template #actions>
                <Button type="outlined" @click="clearFilters">{{
                    formatMessage(messages.favoritesClearFilters)
                }}</Button>
            </template>
        </EmptyState>
        <ProjectCardList v-else :layout="displayMode" :aria-busy="loadingProjects">
            <ProjectCard
                v-for="project in pagedProjects"
                :key="`${project.provider}:${project.projectId}`"
                :layout="displayMode"
                :link="getProjectLink(project)"
                :title="project.title"
                :summary="project.description"
                :icon-url="project.iconUrl"
                :downloads="project.downloads"
                :tags="project.categories"
                :date-published="project.dateCreated"
                :date-updated="project.dateModified"
                :banner="project.banner"
                :provider="project.provider"
            >
                <template #actions>
                    <div class="flex gap-2">
                        <Button
                            v-if="!project.unavailable"
                            color="brand"
                            type="outlined"
                            :size="displayMode === 'compact' ? '2xs' : 'md'"
                            :disabled="isInstalling(project)"
                            @click.stop="toggleProjectSelection(project)"
                        >
                            <SpinnerIcon v-if="isInstalling(project)" class="animate-spin" />
                            <CheckIcon v-else-if="isSelected(project)" />
                            <PlusIcon v-else />
                            {{ installLabel(project) }}
                        </Button>
                        <Button
                            v-tooltip="formatMessage(messages.removeFromFavorites)"
                            circular
                            icon-only
                            color="brand"
                            type="quiet"
                            :size="displayMode === 'compact' ? '2xs' : 'md'"
                            :disabled="
                                discover.isFavoritePending(project.provider, project.projectId)
                            "
                            :label="formatMessage(messages.removeFromFavorites)"
                            @click.stop="removeFavorite(project)"
                        >
                            <SpinnerIcon
                                v-if="
                                    discover.isFavoritePending(project.provider, project.projectId)
                                "
                                class="animate-spin"
                            />
                            <BookmarkFilledIcon v-else />
                        </Button>
                    </div>
                </template>
            </ProjectCard>
        </ProjectCardList>

        <div class="flex justify-end">
            <Pagination
                :page="currentPage"
                :count="pageCount"
                @switch-page="currentPage = $event"
            />
        </div>

        <SelectedProjectsFloatingBar :install-context="installContext" />
        <BrowseInstanceSelector
            ref="instanceSelector"
            :instances="selection.instances.value"
            :selected-instance="selection.targetInstance.value"
            :selected-count="selection.selectedCount.value"
            :install-current="installSelected"
            :clear-current="selection.clear"
            @select="selectTargetInstance"
        />
        <InstallModals />
    </div>
</template>

import type { BrowseInstallPreferences } from '@modrinth/ui/src/layouts/shared/browse-tab/composables/install-logic.ts'
import type { FilterValue } from '@modrinth/ui/src/utils/search.ts'
import { computed, effectScope, ref, shallowRef, watch } from 'vue'

import {
    type DiscoverType,
    loadDiscover,
    loadProject,
    loadTags,
    type ProjectCard,
    type ProjectDetail,
    type ProjectTagData,
} from '../../data'
import { type PreviewState, usePreviewDomain } from '../../lib/state'
import type { InstallJob } from '../../lib/state/downloads'
import type { GameInstance } from '../../lib/types'

/** Project types that have a Modrinth snapshot pool. */
export const SNAPSHOT_TYPES: DiscoverType[] = [
    'mod',
    'modpack',
    'resourcepack',
    'shader',
    'datapack',
]

export function isSnapshotType(type: string): type is DiscoverType {
    return (SNAPSHOT_TYPES as string[]).includes(type)
}

export type FavoriteContentType = 'mod' | 'resourcepack' | 'datapack' | 'shader'
export type FavoriteProvider = 'modrinth'

export function isFavoriteContentType(type: string): type is FavoriteContentType {
    return ['mod', 'resourcepack', 'datapack', 'shader'].includes(type)
}

/** A saved project (apps/app-frontend/src/helpers/content-favorites.ts `ContentFavorite`). */
export interface ContentFavorite {
    provider: FavoriteProvider
    project_id: string
    content_type: FavoriteContentType
    slug: string | null
    saved_at: number
}

export type BrowseDisplayMode = 'list' | 'compact' | 'grid'
export type BrowseContentSource = 'all' | 'modrinth' | 'mcarchive'

export interface BrowseFilterMemory {
    filters: FilterValue[]
    toggledGroups: string[]
    overriddenProvidedFilterTypes: string[]
}

export type ContentSelectionType = 'mod' | 'resourcepack' | 'datapack' | 'shader'
export type ContentSelectionState = 'idle' | 'validating' | 'reviewing' | 'queueing' | 'error'

/** One project in the install cart (providers/content-selection-types.ts `ContentSelectionItem`). */
export interface ContentSelectionItem {
    key: string
    provider: 'modrinth'
    projectId: string
    providerProjectId: string
    versionId?: string
    contentType: ContentSelectionType
    title: string
    iconUrl?: string | null
    preferences?: BrowseInstallPreferences
    targetInstanceId?: string
    slug?: string | null
    versionPending?: boolean
}

/** What the "Review content changes" modal reports back after `Queue changes`. */
export interface ReviewedInstall {
    jobs: { key: string; jobId: string }[]
    activeRoots: string[]
}

export type InstallSelectionReviewer = (input: {
    instance: GameInstance
    items: ContentSelectionItem[]
}) => Promise<ReviewedInstall | null>

export function makeContentSelectionKey(provider: string, projectId: string) {
    return `${provider}:${projectId}`
}

function installJobKey(instanceId: string, itemKey: string) {
    return `${instanceId}\0${itemKey}`
}

const ACTIVE_JOB_PHASES = new Set(['queued', 'resolving_project', 'downloading', 'installing'])

/**
 * Discover's state for the lifetime of the preview window: the snapshot data it has loaded,
 * the browse preferences the app keeps in local storage (display mode, last tab, source, filter
 * memory), content favorites, and the install cart (providers/content-selection.ts).
 */
function createDiscoverDomain(preview: PreviewState) {
    const scope = effectScope(true)

    // Snapshot data ---------------------------------------------------------------------------
    const tags = shallowRef<ProjectTagData | null>(null)
    const pools = shallowRef<Partial<Record<DiscoverType, ProjectCard[]>>>({})
    const totals = shallowRef<Partial<Record<DiscoverType, number>>>({})
    const details = shallowRef<Record<string, ProjectDetail>>({})
    let tagsRequest: Promise<ProjectTagData> | null = null
    const poolRequests = new Map<DiscoverType, Promise<ProjectCard[]>>()
    const detailRequests = new Map<string, Promise<ProjectDetail | null>>()

    function ensureTags() {
        if (tags.value) return Promise.resolve(tags.value)
        tagsRequest ??= loadTags().then((value) => {
            tags.value = value
            return value
        })
        return tagsRequest
    }

    function ensurePool(type: DiscoverType) {
        const cached = pools.value[type]
        if (cached) return Promise.resolve(cached)
        let request = poolRequests.get(type)
        if (!request) {
            request = loadDiscover(type).then((data) => {
                pools.value = { ...pools.value, [type]: data.hits }
                totals.value = { ...totals.value, [type]: data.total_hits }
                return data.hits
            })
            poolRequests.set(type, request)
        }
        return request
    }

    function ensureDetail(id: string): Promise<ProjectDetail | null> {
        const cached = details.value[id]
        if (cached) return Promise.resolve(cached)
        let request = detailRequests.get(id)
        if (!request) {
            request = loadProject(id)
                .then((detail) => {
                    details.value = { ...details.value, [id]: detail }
                    return detail
                })
                .catch(() => null)
            detailRequests.set(id, request)
        }
        return request
    }

    /** Finds a project card in the loaded pools by id or slug. */
    function findCard(idOrSlug: string): ProjectCard | null {
        for (const pool of Object.values(pools.value)) {
            const hit = pool?.find((card) => card.id === idOrSlug || card.slug === idOrSlug)
            if (hit) return hit
        }
        return null
    }

    /** Resolves a route id (project id or slug) to a snapshot project id. */
    async function resolveProjectId(idOrSlug: string): Promise<string> {
        if (details.value[idOrSlug]) return idOrSlug
        const card = findCard(idOrSlug)
        if (card) return card.id
        await Promise.all(SNAPSHOT_TYPES.map((type) => ensurePool(type).catch(() => [])))
        return findCard(idOrSlug)?.id ?? idOrSlug
    }

    // Browse preferences (helpers/browse-display-mode.ts, helpers/settings.ts, browse-filter-memory.ts)
    const displayMode = ref<BrowseDisplayMode>('list')
    const lastProjectType = ref<string>('modpack')
    const contentSource = ref<BrowseContentSource | null>(null)
    const filterMemory = ref<Record<string, BrowseFilterMemory>>({})
    const advancedFiltersCollapsed = ref(true)
    const autoTranslateHintShown = ref(false)
    let manualTranslateClicks = 0
    const browseReturn = ref<{ url: string; scrollTop: number; page: number } | null>(null)

    /** helpers/translation.ts `noteManualTranslateClick` (threshold of three manual uses). */
    function noteManualTranslateClick() {
        if (autoTranslateHintShown.value) return false
        manualTranslateClicks += 1
        if (manualTranslateClicks < 3) return false
        autoTranslateHintShown.value = true
        return true
    }

    // Favorites (composables/useContentFavorites.ts) -------------------------------------------
    const favorites = ref<ContentFavorite[]>([])
    const favoritesLoaded = ref(true)
    const pendingFavorites = ref<Set<string>>(new Set())

    function favoriteKey(provider: string, projectId: string) {
        return `${provider}:${projectId}`
    }

    function isFavorite(provider: string, projectId: string) {
        return favorites.value.some(
            (favorite) => favorite.provider === provider && favorite.project_id === projectId,
        )
    }

    function isFavoritePending(provider: string, projectId: string) {
        return pendingFavorites.value.has(favoriteKey(provider, projectId))
    }

    function setPending(key: string, pending: boolean) {
        const next = new Set(pendingFavorites.value)
        if (pending) next.add(key)
        else next.delete(key)
        pendingFavorites.value = next
    }

    /** The favorites table write (`resource_favorites_toggle`), a short database round trip. */
    function toggleFavorite(input: {
        provider: FavoriteProvider
        project_id: string
        content_type: FavoriteContentType
        slug?: string | null
    }) {
        const key = favoriteKey(input.provider, input.project_id)
        if (pendingFavorites.value.has(key)) return Promise.resolve()
        setPending(key, true)
        return new Promise<void>((resolve) => {
            preview.clock.after(180, () => {
                if (isFavorite(input.provider, input.project_id)) {
                    favorites.value = favorites.value.filter(
                        (favorite) =>
                            favorite.provider !== input.provider ||
                            favorite.project_id !== input.project_id,
                    )
                } else {
                    favorites.value = [
                        {
                            provider: input.provider,
                            project_id: input.project_id,
                            content_type: input.content_type,
                            slug: input.slug ?? null,
                            saved_at: Date.now(),
                        },
                        ...favorites.value,
                    ]
                }
                setPending(key, false)
                resolve()
            })
        })
    }

    function removeFavorite(provider: FavoriteProvider, projectId: string) {
        if (!isFavorite(provider, projectId)) return Promise.resolve()
        const favorite = favorites.value.find(
            (entry) => entry.provider === provider && entry.project_id === projectId,
        )
        return toggleFavorite({
            provider,
            project_id: projectId,
            content_type: favorite?.content_type ?? 'mod',
        })
    }

    // Installed content -----------------------------------------------------------------------
    /** Projects installed in an instance (its content list, read lazily by the downloads state). */
    function installedProjectIds(instanceId: string | null | undefined): Set<string> {
        if (!instanceId) return new Set()
        void preview.downloads.ensureContents()
        return new Set(
            (preview.downloads.contents.value[instanceId] ?? [])
                .filter((item) => !item.removed)
                .map((item) => item.project_id)
                .filter((id): id is string => !!id),
        )
    }

    function installedVersionId(instanceId: string | null | undefined, projectId: string) {
        if (!instanceId) return null
        return (
            (preview.downloads.contents.value[instanceId] ?? []).find(
                (item) => item.project_id === projectId && !item.removed,
            )?.version_id ?? null
        )
    }

    // Content selection (providers/content-selection.ts) ---------------------------------------
    const defaultInstanceId = ref<string | null>(null)
    const targetInstanceId = ref<string | null>(null)
    const items = ref<Map<string, ContentSelectionItem>>(new Map())
    const selectionState = ref<ContentSelectionState>('idle')
    const progress = ref({ completed: 0, total: 0 })
    const errorKeys = ref<Set<string>>(new Set())
    const jobIdsByKey = ref<Map<string, string>>(new Map())
    let reviewer: InstallSelectionReviewer | null = null

    const instances = computed(() =>
        preview.instances.instances.value.filter(
            (instance) => instance.install_stage === 'installed',
        ),
    )
    const targetInstance = computed<GameInstance | null>(() =>
        targetInstanceId.value ? (preview.instances.get(targetInstanceId.value) ?? null) : null,
    )
    const selectedProjects = computed(() =>
        Array.from(items.value.values()).map((item) => ({
            id: item.key,
            name: item.title,
            iconUrl: item.iconUrl,
        })),
    )
    const selectedCount = computed(() => items.value.size)

    function refreshInstances(preferredId?: string | null) {
        const available = instances.value
        const requestedId = preferredId ?? targetInstanceId.value ?? defaultInstanceId.value
        const current = available.find((instance) => instance.id === targetInstanceId.value) ?? null
        const selected =
            items.value.size > 0 && current
                ? current
                : (available.find((instance) => instance.id === requestedId) ?? null)
        targetInstanceId.value = selected?.id ?? null
        defaultInstanceId.value = selected?.id ?? null
        return selected
    }

    function setTarget(instance: GameInstance | null) {
        targetInstanceId.value = instance?.id ?? null
        defaultInstanceId.value = instance?.id ?? null
    }

    function isSelected(key: string) {
        return items.value.has(key)
    }

    function isInstalledIdentity(provider: string, projectId: string) {
        if (provider !== 'modrinth') return false
        return installedProjectIds(targetInstanceId.value).has(projectId)
    }

    function isInstalling(key: string) {
        const instanceId = targetInstanceId.value
        if (!instanceId) return false
        const jobId = jobIdsByKey.value.get(installJobKey(instanceId, key))
        if (!jobId) return false
        const job = preview.downloads.jobs.value.find((candidate) => candidate.id === jobId)
        return !!job && ACTIVE_JOB_PHASES.has(job.phase)
    }

    function add(item: ContentSelectionItem, onDuplicate: (title: string) => void) {
        const instance = targetInstance.value
        if (!instance) return false
        if (item.targetInstanceId && item.targetInstanceId !== instance.id) return false
        if (
            installedProjectIds(instance.id).has(item.providerProjectId) ||
            Array.from(items.value.values()).some(
                (selected) =>
                    selected.key !== item.key &&
                    selected.providerProjectId === item.providerProjectId,
            )
        ) {
            onDuplicate(item.title)
            return false
        }
        const next = new Map(items.value)
        next.set(item.key, {
            ...item,
            targetInstanceId: instance.id,
            versionPending: !item.versionId,
        })
        items.value = next
        const nextErrors = new Set(errorKeys.value)
        nextErrors.delete(item.key)
        errorKeys.value = nextErrors
        return true
    }

    function remove(key: string) {
        const next = new Map(items.value)
        next.delete(key)
        items.value = next
        const nextErrors = new Set(errorKeys.value)
        nextErrors.delete(key)
        errorKeys.value = nextErrors
    }

    function clear() {
        items.value = new Map()
        errorKeys.value = new Set()
    }

    function setInstallSelectionReviewer(next: InstallSelectionReviewer) {
        reviewer = next
    }

    /** Pages mount their own install modals; a leaving page only releases its own reviewer. */
    function releaseInstallSelectionReviewer(previous: InstallSelectionReviewer) {
        if (reviewer === previous) reviewer = null
    }

    async function installSelected(onTargetChanged: () => void): Promise<boolean> {
        const instance = targetInstance.value
        const selected = [...items.value.values()]
        if (
            !instance ||
            selected.length === 0 ||
            ['reviewing', 'queueing', 'validating'].includes(selectionState.value)
        )
            return false
        if (selected.some((item) => item.targetInstanceId !== instance.id)) {
            onTargetChanged()
            return false
        }
        if (!reviewer) return false
        selectionState.value = 'reviewing'
        errorKeys.value = new Set()
        try {
            progress.value = { completed: 0, total: 0 }
            const result = await reviewer({ instance, items: selected })
            if (!result) return false
            const next = new Map(jobIdsByKey.value)
            for (const { key, jobId } of result.jobs)
                next.set(installJobKey(instance.id, key), jobId)
            jobIdsByKey.value = next
            for (const item of selected) remove(item.key)
            return result.jobs.length > 0
        } finally {
            selectionState.value = errorKeys.value.size ? 'error' : 'idle'
        }
    }

    scope.run(() => {
        // Finished jobs release their cart keys (content-selection.ts watch on download jobs).
        watch(
            () => preview.downloads.jobs.value.map((job) => `${job.id}:${job.phase}`).join('|'),
            () => {
                const active = new Set(
                    preview.downloads.jobs.value
                        .filter((job) => ACTIVE_JOB_PHASES.has(job.phase))
                        .map((job) => job.id),
                )
                let next: Map<string, string> | undefined
                for (const [key, jobId] of jobIdsByKey.value) {
                    if (active.has(jobId)) continue
                    next ??= new Map(jobIdsByKey.value)
                    next.delete(key)
                }
                if (next) jobIdsByKey.value = next
            },
        )
        // An instance removed elsewhere stops being the browse target.
        watch(
            () => preview.instances.instances.value.map((instance) => instance.id).join('|'),
            () => {
                if (targetInstanceId.value && !preview.instances.get(targetInstanceId.value)) {
                    targetInstanceId.value = null
                    clear()
                }
            },
        )
    })

    // Modpack installs: a new instance, its pack download, then the instance becomes ready ----
    const modpackJobs = ref<Map<string, { instanceId: string; contentId: string }>>(new Map())

    function trackModpackJob(job: InstallJob, instanceId: string) {
        const next = new Map(modpackJobs.value)
        next.set(job.id, { instanceId, contentId: `${job.project_id}-${job.version_id}` })
        modpackJobs.value = next
    }

    scope.run(() => {
        watch(
            () => preview.downloads.history.value.map((job) => `${job.id}:${job.phase}`).join('|'),
            () => {
                if (!modpackJobs.value.size) return
                let next: Map<string, { instanceId: string; contentId: string }> | undefined
                for (const job of preview.downloads.history.value) {
                    const tracked = modpackJobs.value.get(job.id)
                    if (!tracked) continue
                    next ??= new Map(modpackJobs.value)
                    next.delete(job.id)
                    const instance = preview.instances.get(tracked.instanceId)
                    if (job.phase === 'completed') {
                        // The pack archive itself is not a content file of the new instance.
                        preview.downloads.removeContent(tracked.instanceId, tracked.contentId)
                        if (instance) instance.install_stage = 'installed'
                    } else if (instance) {
                        instance.install_stage = 'not_installed'
                    }
                }
                if (next) modpackJobs.value = next
            },
        )
    })

    function dispose() {
        scope.stop()
    }

    return {
        value: {
            // data
            tags,
            pools,
            totals,
            details,
            ensureTags,
            ensurePool,
            ensureDetail,
            findCard,
            resolveProjectId,
            // preferences
            displayMode,
            lastProjectType,
            contentSource,
            filterMemory,
            advancedFiltersCollapsed,
            noteManualTranslateClick,
            browseReturn,
            // favorites
            favorites,
            favoritesLoaded,
            isFavorite,
            isFavoritePending,
            toggleFavorite,
            removeFavorite,
            // installed content
            installedProjectIds,
            installedVersionId,
            // content selection
            selection: {
                instances,
                targetInstance,
                targetInstanceId,
                items,
                selectedProjects,
                selectedCount,
                state: selectionState,
                progress,
                errorKeys,
                refreshInstances,
                setTarget,
                add,
                remove,
                clear,
                isSelected,
                isInstalledIdentity,
                isInstalling,
                installSelected,
                setInstallSelectionReviewer,
                releaseInstallSelectionReviewer,
            },
            // modpack installs
            trackModpackJob,
        },
        dispose,
    }
}

export type DiscoverDomain = ReturnType<typeof createDiscoverDomain>['value']

export function useDiscover(): DiscoverDomain {
    return usePreviewDomain('discover.v2', createDiscoverDomain)
}

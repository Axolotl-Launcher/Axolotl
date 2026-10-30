import type { Labrinth } from '@modrinth/api-client'
import { type BrowseInstallPreferences, type BrowseSelectedProject, useVIntl } from '@modrinth/ui'
import { computed, getCurrentScope, onScopeDispose, ref, watch } from 'vue'

import type ContentInstallPreviewModal from '@/components/ui/ContentInstallPreviewModal.vue'
import type ResourceOperationPreviewModal from '@/components/ui/ResourceOperationPreviewModal.vue'
import { get_project_many, get_version_many } from '@/helpers/cache.js'
import {
    compareContentIdentities,
    type ContentIdentity,
    contentIdentityFromInput,
    type ContentIdentityInput,
    contentIdentityInputsFromSnapshot,
    resolveContentIdentities,
} from '@/helpers/content-identity'
import { getCurseForgeFile, getCurseForgeFiles, getCurseForgeProjects } from '@/helpers/curseforge'
import { installJobInstanceId } from '@/helpers/install'
import {
    get_content_snapshot,
    type InstallContentBatchItem,
    list as listInstances,
    queue_content_batch,
} from '@/helpers/instance'
import { type ResourceSelection, sourceResource } from '@/helpers/resources'
import { getBrowseDefaultInstanceId, setBrowseDefaultInstanceId } from '@/helpers/settings'
import type { GameInstance } from '@/helpers/types'
import { createContentAnalysis } from '@/providers/content-analysis'
import {
    type ContentSelectionContext,
    type ContentSelectionItem,
    contentSelectionMessages as messages,
    type ContentSelectionProvider,
    type ContentSelectionState,
    type ContentSelectionType,
    type CreateContentSelectionOptions,
    injectContentSelection,
    provideContentSelection,
} from '@/providers/content-selection-types'
import { useTheming } from '@/store/state'

export {
    type ContentSelectionContext,
    type ContentSelectionItem,
    type ContentSelectionProvider,
    type ContentSelectionState,
    type ContentSelectionType,
    type CreateContentSelectionOptions,
    injectContentSelection,
    provideContentSelection,
}

export const activeJobStatuses = new Set(['queued', 'running', 'canceling', 'waiting_for_user'])

function selectedItemKey(provider: ContentSelectionProvider, projectId: string) {
    return `${provider}:${projectId}`
}

// Install jobs are scoped to an instance. Keep the project key provider-qualified,
// but include the target instance when tracking the queued job so installing the
// same project in another instance does not lock its action.
function installJobKey(instanceId: string, itemKey: string) {
    return `${instanceId}\0${itemKey}`
}

function resourceSelections(resources: ContentSelectionItem[]): ResourceSelection[] {
    return resources.map((item) => ({
        id: item.key,
        resource: sourceResource(item.provider, item.providerProjectId),
        release: item.versionId
            ? {
                  kind: 'release',
                  reference: {
                      project: {
                          source: item.provider,
                          projectId: item.providerProjectId,
                      },
                      releaseId: item.versionId,
                  },
              }
            : { kind: 'automatic' },
        resourceType: (
            {
                mod: 'mod',
                plugin: 'plugin',
                datapack: 'dataPack',
                resourcepack: 'resourcePack',
                shader: 'shader',
            } as const
        )[item.contentType as Exclude<typeof item.contentType, 'world'>],
        scope: { kind: 'instance' },
    }))
}

export function createContentSelection({
    addNotification,
    handleError,
    downloadManager,
}: CreateContentSelectionOptions): ContentSelectionContext {
    const { formatMessage } = useVIntl()
    const themeStore = useTheming()
    const instances = ref<GameInstance[]>([])
    const targetInstance = ref<GameInstance | null>(null)
    const items = ref<Map<string, ContentSelectionItem>>(new Map())
    const state = ref<ContentSelectionState>('idle')
    const progress = ref({ completed: 0, total: 0 })
    const errorKeys = ref<Set<string>>(new Set())
    const jobIdsByKey = ref<Map<string, string>>(new Map())
    const installedIdentityCache = new Map<string, ContentIdentity[]>()
    const installedIdentityKeys = ref<Set<string>>(new Set())
    const installedIdentitySlugs = ref<Set<string>>(new Set())
    const installedIdentityRequests = new Map<string, Promise<ContentIdentity[]>>()
    let installedIdentityRequestId = 0
    let selectionGeneration = 0
    let resourcePreview: InstanceType<typeof ResourceOperationPreviewModal> | null = null
    let previewModal: InstanceType<typeof ContentInstallPreviewModal> | null = null
    const analysis = createContentAnalysis()
    const snapshotGeneration = ref(0)
    const stopAnalysis = watch(
        () => {
            const instance = targetInstance.value
            if (!instance) return null
            const selected = [...items.value.values()].filter(
                (item) => item.targetInstanceId === instance.id && item.contentType !== 'world',
            )
            return {
                instanceId: instance.id,
                context: JSON.stringify([
                    instance.game_version,
                    instance.loader,
                    instance.loader_version,
                    instance.update_channel,
                    snapshotGeneration.value,
                    downloadManager.jobs.value
                        .filter((job) => installJobInstanceId(job) === instance.id)
                        .map(({ job_id, status }) => [job_id, status]),
                ]),
                selections: resourceSelections(selected),
                installDependencies: themeStore.getFeatureFlag('auto_install_dependencies'),
            }
        },
        (input) => analysis.update(input),
        { flush: 'sync' },
    )
    if (getCurrentScope())
        onScopeDispose(() => {
            stopAnalysis()
            analysis.clear()
        })

    const selectedProjects = computed<BrowseSelectedProject[]>(() =>
        Array.from(items.value.values()).map((item) => ({
            id: item.key,
            name: item.title,
            iconUrl: item.iconUrl,
        })),
    )
    const selectedCount = computed(() => items.value.size)

    async function refreshInstances(
        preferredId?: string | null,
        isCurrent: () => boolean = () => true,
    ) {
        const available = (await listInstances()).filter(
            (instance) => instance.install_stage === 'installed',
        )
        if (!isCurrent()) return targetInstance.value
        instances.value = available
        const requestedId = preferredId ?? targetInstance.value?.id ?? getBrowseDefaultInstanceId()
        const current =
            available.find((instance) => instance.id === targetInstance.value?.id) ?? null
        const selected =
            items.value.size > 0 && current
                ? current
                : (available.find((instance) => instance.id === requestedId) ?? null)
        if (selected?.id !== targetInstance.value?.id) {
            selectionGeneration++
            installedIdentityRequestId += 1
            installedIdentityCache.clear()
            installedIdentityKeys.value = new Set()
            installedIdentitySlugs.value = new Set()
        }
        targetInstance.value = selected
        setBrowseDefaultInstanceId(selected?.id ?? null)
        return selected
    }

    function setTarget(instance: GameInstance | null) {
        if (instance?.id !== targetInstance.value?.id) {
            selectionGeneration++
            installedIdentityRequestId += 1
            installedIdentityCache.clear()
            installedIdentityKeys.value = new Set()
            installedIdentitySlugs.value = new Set()
        }
        targetInstance.value = instance
        setBrowseDefaultInstanceId(instance?.id ?? null)
    }

    async function resolveSelectionIdentity(item: ContentSelectionItem) {
        let sha1 = item.sha1 ?? undefined
        if (!sha1 && item.provider === 'modrinth') {
            const version = (await get_version_many([item.versionId]).catch(() => []))[0]
            sha1 = version?.files?.[0]?.hashes?.sha1
        }
        if (!sha1 && item.provider === 'curseforge') {
            const file = await getCurseForgeFile(
                Number(item.providerProjectId),
                Number(item.versionId),
            ).catch(() => null)
            sha1 = file?.hashes.find((hash) => hash.algo === 1)?.value
        }
        const input: ContentIdentityInput = {
            provider: item.provider,
            projectId: item.providerProjectId,
            contentType: item.contentType,
            slug: item.slug,
            title: item.title,
            fileName: item.fileName,
            sha1,
        }
        return (await resolveContentIdentities([input]))[0] ?? contentIdentityFromInput(input)
    }

    async function getInstalledIdentities(instance: GameInstance) {
        const cached = installedIdentityCache.get(instance.id)
        if (cached) return cached
        const pending = installedIdentityRequests.get(instance.id)
        if (pending) return pending
        const request = fetchInstalledIdentities(instance).finally(() => {
            if (installedIdentityRequests.get(instance.id) === request)
                installedIdentityRequests.delete(instance.id)
        })
        installedIdentityRequests.set(instance.id, request)
        return request
    }

    async function fetchInstalledIdentities(instance: GameInstance) {
        const requestId = ++installedIdentityRequestId
        const canCommit = () =>
            requestId === installedIdentityRequestId && targetInstance.value?.id === instance.id
        const snapshot = await get_content_snapshot(instance.id).catch(() => null)
        if (!snapshot) {
            // A stale/offline instance snapshot must not turn an add operation into a
            // false authoritative conflict. Provider-qualified cart de-duplication
            // still applies while the failed lookup remains unknown.
            installedIdentityCache.set(instance.id, [])
            if (canCommit()) {
                installedIdentityKeys.value = new Set()
                installedIdentitySlugs.value = new Set()
            }
            return []
        }
        const unresolvedInputs = contentIdentityInputsFromSnapshot(snapshot.items)
        const modrinthIds = unresolvedInputs
            .filter((input) => input.provider === 'modrinth' && !input.slug)
            .map((input) => input.projectId)
        const curseForgeIds = unresolvedInputs
            .filter((input) => input.provider === 'curseforge' && !input.slug)
            .map((input) => Number(input.projectId))
            .filter((id) => Number.isSafeInteger(id))
        const [modrinthProjects, curseForgeProjects] = await Promise.all([
            get_project_many([...new Set(modrinthIds)])
                .catch(() => [])
                .then((projects) => (projects ?? []) as Labrinth.Projects.v2.Project[]),
            getCurseForgeProjects([...new Set(curseForgeIds)]).catch(() => []),
        ])
        const inputs = contentIdentityInputsFromSnapshot(snapshot.items, {
            modrinth: new Map(
                modrinthProjects.map((project) => [
                    project.id,
                    { slug: project.slug, title: project.title },
                ]),
            ),
            curseforge: new Map(
                curseForgeProjects.map((project) => [
                    String(project.id),
                    { slug: project.slug, title: project.name },
                ]),
            ),
        })
        const identities = await resolveContentIdentities(inputs)
        installedIdentityCache.set(instance.id, identities)
        if (!canCommit()) return identities
        const keys = new Set<string>()
        const slugs = new Set<string>()
        for (const identity of identities) {
            keys.add(selectedItemKey(identity.provider, identity.projectId))
            if (identity.slug) {
                slugs.add(selectedItemKey(identity.provider, identity.slug.toLowerCase()))
            }
            for (const counterpart of identity.counterparts ?? []) {
                keys.add(selectedItemKey(counterpart.provider, counterpart.projectId))
                if (counterpart.slug) {
                    slugs.add(selectedItemKey(counterpart.provider, counterpart.slug.toLowerCase()))
                }
            }
        }
        installedIdentityKeys.value = keys
        installedIdentitySlugs.value = slugs
        return identities
    }

    async function refreshInstalledIdentities() {
        snapshotGeneration.value++
        if (!targetInstance.value) {
            installedIdentityKeys.value = new Set()
            installedIdentitySlugs.value = new Set()
            return
        }
        installedIdentityCache.delete(targetInstance.value.id)
        await getInstalledIdentities(targetInstance.value)
    }

    async function findConflicts(
        item: ContentSelectionItem,
        instance: GameInstance,
        { skipInstalled = false }: { skipInstalled?: boolean } = {},
    ) {
        const candidate = item.identity ?? (await resolveSelectionIdentity(item))
        const existing = [
            ...(await Promise.all(
                [...items.value.values()]
                    .filter((selected) => selected.key !== item.key)
                    .map(async (selected) => ({
                        item: selected,
                        identity: selected.identity ?? (await resolveSelectionIdentity(selected)),
                    })),
            )),
            ...(skipInstalled
                ? []
                : (await getInstalledIdentities(instance)).map((identity) => ({
                      item: null,
                      identity,
                  }))),
        ]
        return existing.flatMap(({ item: existingItem, identity }) => {
            const match = compareContentIdentities(candidate, identity)
            return match ? [{ candidate, existing: existingItem ?? identity, match }] : []
        })
    }

    async function add(item: ContentSelectionItem) {
        const instance = targetInstance.value
        if (!instance) throw new Error(formatMessage(messages.noTarget))
        if (item.targetInstanceId && item.targetInstanceId !== instance.id) return false
        const generation = selectionGeneration
        const isCurrent = () =>
            generation === selectionGeneration && targetInstance.value?.id === instance.id
        const versionPending = !item.versionId
        const identity =
            item.identity ??
            (versionPending
                ? contentIdentityFromInput({
                      provider: item.provider,
                      projectId: item.providerProjectId,
                      contentType: item.contentType,
                      slug: item.slug,
                      title: item.title,
                      fileName: item.fileName,
                  })
                : await resolveSelectionIdentity(item))
        if (!isCurrent()) return false
        const reviewedConflicts = new Set<string>()
        while (isCurrent()) {
            const currentItems = items.value
            const conflicts = await findConflicts({ ...item, identity }, instance, {
                skipInstalled: versionPending,
            })
            if (!isCurrent()) return false
            if (currentItems !== items.value) continue
            const hardConflict = conflicts.find((conflict) => conflict.match.source !== 'heuristic')
            if (hardConflict) {
                addNotification({
                    title: formatMessage(messages.duplicateContent, { project: item.title }),
                    type: 'error',
                })
                return false
            }
            const heuristicConflict = conflicts.find(
                (conflict) => conflict.match.source === 'heuristic',
            )
            if (heuristicConflict) {
                const conflictKey = selectedItemKey(
                    heuristicConflict.existing.provider,
                    heuristicConflict.existing.projectId,
                )
                if (!reviewedConflicts.has(conflictKey)) {
                    if (!previewModal || !heuristicConflict.existing) {
                        addNotification({
                            title: formatMessage(messages.conflictUnavailable),
                            type: 'error',
                        })
                        return false
                    }
                    const confirmed = await previewModal.showConflict({
                        candidate: {
                            title: item.title,
                            provider: item.provider,
                            contentType: item.contentType,
                            iconUrl: item.iconUrl,
                        },
                        existing: [
                            {
                                title: heuristicConflict.existing.title ?? '',
                                provider: heuristicConflict.existing.provider,
                                fileName: heuristicConflict.existing.fileName ?? undefined,
                            },
                        ],
                        source: 'heuristic',
                        confidence:
                            heuristicConflict.match.confidence === 'high' ? 'high' : 'possible',
                    })
                    if (!confirmed || !isCurrent()) return false
                    reviewedConflicts.add(conflictKey)
                }
            }
            if (currentItems !== items.value) continue
            const next = new Map(items.value)
            next.set(item.key, {
                ...item,
                identity,
                sha1: identity.sha1,
                targetInstanceId: instance.id,
                versionPending,
            })
            items.value = next
            const nextErrors = new Set(errorKeys.value)
            nextErrors.delete(item.key)
            errorKeys.value = nextErrors
            return true
        }
        return false
    }

    function remove(key: string) {
        selectionGeneration++
        const next = new Map(items.value)
        next.delete(key)
        items.value = next
        const nextErrors = new Set(errorKeys.value)
        nextErrors.delete(key)
        errorKeys.value = nextErrors
    }

    function clear() {
        selectionGeneration++
        items.value = new Map()
        errorKeys.value = new Set()
        installedIdentityRequestId += 1
        installedIdentityCache.clear()
        installedIdentityKeys.value = new Set()
        installedIdentitySlugs.value = new Set()
    }

    function updateVersion(
        key: string,
        versionId: string,
        preferences?: BrowseInstallPreferences,
        sha1?: string,
    ) {
        const current = items.value.get(key)
        if (!current) return
        const next = new Map(items.value)
        const updated: ContentSelectionItem = {
            ...current,
            versionId,
            versionPending: false,
            preferences: preferences ?? current.preferences,
        }
        if (sha1) {
            updated.sha1 = sha1
        }
        next.set(key, updated)
        items.value = next
    }

    function isSelected(key: string) {
        return items.value.has(key)
    }

    function isInstalling(key: string) {
        const instanceId = targetInstance.value?.id
        if (!instanceId) return false
        const jobId = jobIdsByKey.value.get(installJobKey(instanceId, key))
        if (!jobId) return false
        const job = downloadManager.jobs.value.find((candidate) => candidate.job_id === jobId)
        return !job || activeJobStatuses.has(job.status)
    }

    async function installSelected() {
        const instance = targetInstance.value
        const selected = [...items.value.values()]
        if (
            !instance ||
            selected.length === 0 ||
            state.value === 'reviewing' ||
            state.value === 'queueing' ||
            state.value === 'validating'
        )
            return false
        if (selected.some((item) => item.targetInstanceId !== instance.id)) {
            addNotification({ title: formatMessage(messages.targetChanged), type: 'error' })
            return false
        }
        const worlds = selected.filter((item) => item.contentType === 'world')
        const resources = selected.filter((item) => item.contentType !== 'world')
        if ((resources.length && !resourcePreview) || (worlds.length && !previewModal)) return false
        state.value = 'reviewing'
        errorKeys.value = new Set()
        const queuedKeys = new Set<string>()
        let acceptedJobs = 0
        try {
            let selectionSnapshot = JSON.stringify([...items.value.values()])
            const isCurrentSelection = () =>
                targetInstance.value?.id === instance.id &&
                JSON.stringify([...items.value.values()]) === selectionSnapshot
            state.value = 'validating'
            progress.value = { completed: 0, total: worlds.length }
            for (const item of worlds) {
                const projectId = Number(item.providerProjectId)
                if (!Number.isSafeInteger(projectId) || projectId <= 0)
                    throw new Error(formatMessage(messages.previewFailed))
                if (!item.versionId) {
                    const page = await getCurseForgeFiles(projectId, {})
                    if (!isCurrentSelection()) return false
                    const file = page.files.find((file) => file.isAvailable)
                    if (!file) throw new Error(formatMessage(messages.previewFailed))
                    item.versionId = String(file.id)
                    updateVersion(
                        item.key,
                        item.versionId,
                        undefined,
                        file.hashes.find((hash) => hash.algo === 1)?.value,
                    )
                    selectionSnapshot = JSON.stringify([...items.value.values()])
                }
                const fileId = Number(item.versionId)
                if (!Number.isSafeInteger(fileId) || fileId <= 0)
                    throw new Error(formatMessage(messages.previewFailed))
                progress.value = { ...progress.value, completed: progress.value.completed + 1 }
            }
            state.value = 'reviewing'
            let includedWorlds = worlds
            if (worlds.length) {
                const review = await previewModal!.showBatch({
                    primaries: worlds.map((item) => ({
                        key: item.key,
                        title: item.title,
                        iconUrl: item.iconUrl,
                        versionNumber: item.versionId,
                        provider: 'CurseForge',
                        contentType: item.contentType,
                        removable: true,
                    })),
                    instanceName: instance.name,
                    installDependencies: false,
                    dependencies: [],
                    skipped: [],
                })
                if (!review) return false
                if (!isCurrentSelection()) return false
                includedWorlds = worlds.filter((item) => review.primaryKeys.includes(item.key))
            }
            if (resources.length) {
                if (!isCurrentSelection()) return false
                const selections = resourceSelections(resources)
                const result = await resourcePreview!.showInstallSelection({
                    instanceId: instance.id,
                    instanceName: instance.name,
                    selections,
                    analysis: analysis.review(),
                    installDependencies: themeStore.getFeatureFlag('auto_install_dependencies'),
                })
                if (!result) return false
                const { job, activeRoots } = result
                acceptedJobs++
                const next = new Map(jobIdsByKey.value)
                for (const item of resources) {
                    if (activeRoots.includes(item.key)) {
                        next.set(installJobKey(instance.id, item.key), job.job_id)
                        queuedKeys.add(item.key)
                    }
                    remove(item.key)
                }
                jobIdsByKey.value = next
            }
            if (includedWorlds.length) {
                state.value = 'queueing'
                const batchItems: InstallContentBatchItem[] = includedWorlds.map((item) => ({
                    type: 'curse_forge_world',
                    request: {
                        instanceId: instance.id,
                        projectId: Number(item.providerProjectId),
                        fileId: Number(item.versionId),
                    },
                }))
                const job = await queue_content_batch(instance.id, batchItems, {
                    title: instance.name,
                    iconUrl: includedWorlds.length === 1 ? includedWorlds[0].iconUrl : null,
                })
                acceptedJobs++
                const next = new Map(jobIdsByKey.value)
                for (const item of includedWorlds) {
                    next.set(installJobKey(instance.id, item.key), job.job_id)
                    queuedKeys.add(item.key)
                    remove(item.key)
                }
                jobIdsByKey.value = next
            }
            for (const item of worlds) {
                if (!includedWorlds.includes(item)) remove(item.key)
            }
            installedIdentityCache.delete(instance.id)
            return acceptedJobs > 0
        } catch (error) {
            errorKeys.value = new Set(
                selected
                    .filter((item) => items.value.has(item.key) && !queuedKeys.has(item.key))
                    .map((item) => item.key),
            )
            handleError(error)
            return false
        } finally {
            state.value = errorKeys.value.size ? 'error' : 'idle'
        }
    }

    watch(
        () => downloadManager.jobs.value.map(({ job_id, status }) => ({ job_id, status })),
        (jobs) => {
            const terminalJobIds = new Set(
                jobs.filter((job) => !activeJobStatuses.has(job.status)).map((job) => job.job_id),
            )
            if (!terminalJobIds.size) return
            let next: Map<string, string> | undefined
            for (const [key, jobId] of jobIdsByKey.value) {
                if (!terminalJobIds.has(jobId)) continue
                next ??= new Map(jobIdsByKey.value)
                next.delete(key)
            }
            if (next) jobIdsByKey.value = next
        },
    )

    return {
        instances,
        targetInstance,
        items,
        selectedProjects,
        selectedCount,
        state,
        progress,
        errorKeys,
        refreshInstances,
        refreshInstalledIdentities,
        setTarget,
        add,
        remove,
        clear,
        isSelected,
        isInstalledIdentity(provider, projectId, slug) {
            return (
                installedIdentityKeys.value.has(selectedItemKey(provider, projectId)) ||
                (!!slug &&
                    installedIdentitySlugs.value.has(selectedItemKey(provider, slug.toLowerCase())))
            )
        },
        isInstalling,
        installSelected,
        setResourceInstallPreview(modal) {
            resourcePreview = modal
        },
        setPreviewModal(modal) {
            previewModal = modal
        },
    }
}

export function makeContentSelectionKey(provider: ContentSelectionProvider, projectId: string) {
    return selectedItemKey(provider, projectId)
}

import { createContext } from '@modrinth/ui'
import { computed, type ComputedRef, type Ref, ref } from 'vue'

import { setCurseForgeManualDownloads } from '@/helpers/curseforge-manual'
import {
    isRegressiveActiveJobSnapshot,
    mergeRefreshedDownloadJobs,
    reconcileAcceptedDownloadJob,
} from '@/helpers/download-job-refresh'
import { download_request_listener, install_job_listener, loading_listener } from '@/helpers/events'
import {
    download_history_clear,
    download_job_cancel,
    download_job_delete,
    download_job_get,
    download_job_list,
    download_job_resume,
    download_job_retry,
    type DownloadRequestUpdate,
    install_job_skip_missing_content,
    installJobInstanceId,
    type InstallJobSnapshot,
} from '@/helpers/install'
import { ACTIVE_INSTALL_JOB_STATUSES, isActiveInstallJobStatus } from '@/helpers/install-job-status'
import { preserveMonotonicProgress } from '@/helpers/install-progress'
import { type ContentChangeIntent, queue_content_change } from '@/helpers/instance'
import type { LoadingBar } from '@/helpers/state'
import { progress_bars_list } from '@/helpers/state'

export const downloadBarTypes = new Set([
    'java_download',
    'pack_file_download',
    'pack_download',
    'minecraft_download',
    'instance_update',
    'launcher_update',
])

export interface DownloadManager {
    initializationState: Ref<'loading' | 'ready' | 'error'>
    jobs: Ref<InstallJobSnapshot[]>
    pendingContentChanges: Ref<PendingContentChange[]>
    legacyDownloads: Ref<LoadingBar[]>
    activeJobs: ComputedRef<InstallJobSnapshot[]>
    historyJobs: ComputedRef<InstallJobSnapshot[]>
    activeCount: ComputedRef<number>
    queuedCount: ComputedRef<number>
    start: () => Promise<void>
    refresh: () => Promise<void>
    cancel: (jobId: string) => Promise<void>
    retry: (jobId: string) => Promise<void>
    resume: (jobId: string) => Promise<void>
    skipMissingContent: (jobId: string) => Promise<void>
    remove: (jobId: string) => Promise<void>
    clearHistory: () => Promise<void>
    trackJob: (job: InstallJobSnapshot) => void
    queueContentChange: (request: QueueContentChangeRequest) => Promise<InstallJobSnapshot>
    /**
     * Insert a synthetic job created on the frontend (e.g. a server download
     * that does not go through the backend install-pipeline). The job is kept
     * in-memory and will disappear on refresh — callers must update it via
     * `setSyntheticJob` to keep it alive.
     */
    addSyntheticJob: (job: InstallJobSnapshot) => void
    /**
     * Replace a synthetic job (identified by `job_id`) with a fresh snapshot.
     * This is a no-op for backend-tracked jobs.
     */
    setSyntheticJob: (job: InstallJobSnapshot) => void
    /**
     * Register a cancel handler for a synthetic job.  When `cancel()` is
     * called with this jobId the handler is invoked *before* the job is
     * removed from the list, allowing the caller to abort the underlying
     * operation (e.g. stop a server install).
     */
    onSyntheticCancel: (jobId: string, handler: () => void | Promise<void>) => void
    /**
     * Unregister a previously registered cancel handler.
     */
    offSyntheticCancel: (jobId: string) => void
    dispose: () => void
}

export interface PendingContentChange {
    id: string
    instanceId: string
    intent: ContentChangeIntent
    contentIds: string[]
    updateAll: boolean
}

export interface QueueContentChangeRequest {
    instanceId: string
    intent: ContentChangeIntent
    displayTitle: string
    displayIcon?: string
}

export function createDownloadManager(handleError: (error: unknown) => void): DownloadManager {
    const jobs = ref<InstallJobSnapshot[]>([])
    const initializationState = ref<'loading' | 'ready' | 'error'>('loading')
    const pendingContentChanges = ref<PendingContentChange[]>([])
    const legacyDownloads = ref<LoadingBar[]>([])
    let started = false
    let startPromise: Promise<void> | undefined
    let disposed = false
    let unlistenJobs: (() => void) | null = null
    let unlistenRequests: (() => void) | null = null
    let unlistenLoading: (() => void) | null = null
    let initializing = false
    const pendingInitialUpdates: Array<
        | { kind: 'job'; job: InstallJobSnapshot }
        | { kind: 'request'; update: DownloadRequestUpdate }
    > = []
    const pendingRequestUpdatesByJob = new Map<string, DownloadRequestUpdate[]>()
    const pendingRequestUpdates: DownloadRequestUpdate[] = []
    const pendingProgressUpdateIndexes = new Map<string, number>()
    const jobRevisions = new Map<string, number>()
    let requestFlushTimer: ReturnType<typeof setTimeout> | null = null
    let legacyRefreshTimer: ReturnType<typeof setTimeout> | null = null
    let pendingContentChangeId = 0

    function bumpJobRevision(jobId: string) {
        jobRevisions.set(jobId, (jobRevisions.get(jobId) ?? 0) + 1)
    }

    function persistManualDownloadsFromJob(job: InstallJobSnapshot) {
        if (job.status !== 'waiting_for_user' && job.status !== 'succeeded') return
        const instanceId = installJobInstanceId(job)
        const hasManualDownloadHistory = job.items.some((item) => item.manual_url)
        if (!instanceId || !hasManualDownloadHistory) return
        const manualItems = job.items
            .filter(
                (item) =>
                    item.status === 'skipped' &&
                    item.manual_url &&
                    item.project_id &&
                    item.version_id,
            )
            .map((item) => ({
                projectId: Number(item.project_id),
                fileId: Number(item.version_id),
                fileName: item.name,
                websiteUrl: item.manual_url ?? undefined,
            }))
        setCurseForgeManualDownloads(instanceId, manualItems)
    }

    function setJob(job: InstallJobSnapshot, force = false) {
        if (disposed) return
        if (initializing && !force) {
            pendingInitialUpdates.push({ kind: 'job', job })
            return
        }
        // Preserve event arrival order. A full snapshot is authoritative over
        // download-request updates received before it; otherwise a delayed frame
        // flush can regress a completed item back to downloading.
        flushRequestUpdates()
        const current = jobs.value.find((candidate) => candidate.job_id === job.job_id)
        if (current && isRegressiveActiveJobSnapshot(current, job, ACTIVE_INSTALL_JOB_STATUSES)) {
            return
        }
        if (current && current.modified.localeCompare(job.modified) > 0) return
        const currentIndex = jobs.value.findIndex((candidate) => candidate.job_id === job.job_id)
        if (currentIndex !== -1) {
            job = preserveMonotonicProgress(jobs.value[currentIndex], job)
            // Progress snapshots are frequent. Keep an existing job in its current
            // position instead of rebuilding and sorting the whole list on every
            // update. Jobs created within the same second have identical timestamps;
            // sorting those snapshots repeatedly makes cards jump and can cause an
            // expanded details view to be patched onto a neighbouring card.
            const nextJobs = [...jobs.value]
            nextJobs[currentIndex] = job
            jobs.value = nextJobs
        } else {
            jobs.value = [job, ...jobs.value].sort((a, b) => b.created.localeCompare(a.created))
        }
        bumpJobRevision(job.job_id)
        const pending = pendingRequestUpdatesByJob.get(job.job_id)
        if (pending) {
            pendingRequestUpdatesByJob.delete(job.job_id)
            for (const update of pending) updateRequest(update)
        }
        persistManualDownloadsFromJob(job)
    }

    async function queueContentChange({
        instanceId,
        intent,
        displayTitle,
        displayIcon,
    }: QueueContentChangeRequest) {
        const contentIds =
            intent.type === 'update_selected'
                ? intent.targets.map((target) => target.content_id)
                : intent.type === 'update_all_user_added'
                  ? []
                  : [intent.content_id]
        const pending: PendingContentChange = {
            id: `content-change:${++pendingContentChangeId}`,
            instanceId,
            intent,
            contentIds,
            updateAll: intent.type === 'update_all_user_added',
        }
        pendingContentChanges.value = [...pendingContentChanges.value, pending]
        try {
            const job = await queue_content_change(instanceId, intent, displayTitle, displayIcon)
            setJob(job, true)
            return job
        } finally {
            pendingContentChanges.value = pendingContentChanges.value.filter(
                (candidate) => candidate.id !== pending.id,
            )
        }
    }

    function updateRequest(update: DownloadRequestUpdate) {
        if (disposed) return
        if (initializing) {
            pendingInitialUpdates.push({ kind: 'request', update })
            return
        }
        const key = `${update.job_id}\0${update.id}`
        if (update.type === 'progress') {
            const pendingIndex = pendingProgressUpdateIndexes.get(key)
            if (pendingIndex != null) {
                pendingRequestUpdates[pendingIndex] = update
                scheduleRequestFlush()
                return
            }
            pendingProgressUpdateIndexes.set(key, pendingRequestUpdates.length)
        } else {
            pendingProgressUpdateIndexes.delete(key)
        }
        pendingRequestUpdates.push(update)
        scheduleRequestFlush()
    }

    function scheduleRequestFlush() {
        if (requestFlushTimer !== null) return
        requestFlushTimer = setTimeout(() => {
            requestFlushTimer = null
            flushRequestUpdates()
        }, 16)
    }

    function flushRequestUpdates() {
        if (pendingRequestUpdates.length === 0) return
        const updates = pendingRequestUpdates.splice(0)
        pendingProgressUpdateIndexes.clear()
        const next = [...jobs.value]
        const mutableJobs = new Map<number, InstallJobSnapshot>()
        for (const update of updates) {
            const jobIndex = next.findIndex((job) => job.job_id === update.job_id)
            if (jobIndex === -1) {
                const pending = pendingRequestUpdatesByJob.get(update.job_id) ?? []
                pending.push(update)
                pendingRequestUpdatesByJob.set(update.job_id, pending)
                continue
            }
            let job = mutableJobs.get(jobIndex)
            if (!job) {
                const current = next[jobIndex]
                job = { ...current, items: [...current.items] }
                next[jobIndex] = job
                mutableJobs.set(jobIndex, job)
            }
            applyRequestUpdateToJob(update, job)
        }
        for (const job of mutableJobs.values()) syncLiveByteProgress(job)
        for (const job of mutableJobs.values()) bumpJobRevision(job.job_id)
        jobs.value = next
    }

    function syncLiveByteProgress(job: InstallJobSnapshot) {
        const itemTotal = job.items.reduce((sum, item) => sum + (item.bytes_total ?? 0), 0)
        const total = Math.max(job.summary.bytes_total ?? 0, itemTotal) || null
        const downloaded = job.items.reduce((sum, item) => sum + item.bytes_downloaded, 0)
        const current = Math.max(
            job.summary.bytes_downloaded,
            total == null ? downloaded : Math.min(downloaded, total),
        )
        job.summary = { ...job.summary, bytes_downloaded: current, bytes_total: total }
        if (job.phase === 'downloading_pack_file') {
            const packTotal = job.progress?.total || itemTotal
            if (packTotal > 0) {
                job.progress = {
                    current: Math.min(Math.max(job.progress?.current ?? 0, downloaded), packTotal),
                    total: packTotal,
                }
            }
        }
        if (job.phase === 'downloading_content' && job.progress?.secondary) {
            job.progress = {
                ...job.progress,
                secondary: {
                    ...job.progress.secondary,
                    current: Math.min(
                        Math.max(job.progress.secondary.current, current),
                        job.progress.secondary.total,
                    ),
                },
            }
        }
    }

    function applyRequestUpdateToJob(update: DownloadRequestUpdate, job: InstallJobSnapshot) {
        const normalizePath = (value: string) => value.replaceAll('\\', '/').replace(/^\/+/, '')
        const updatePath = normalizePath(update.id)
        let itemIndex = job.items.findIndex((item) => {
            if (item.id === update.id) return true
            const itemPath = normalizePath(item.id)
            return updatePath.endsWith(`/${itemPath}`) || itemPath.endsWith(`/${updatePath}`)
        })
        if (itemIndex === -1 && job.kind === 'change_content' && job.items.length === 1)
            itemIndex = 0
        const current = itemIndex === -1 ? null : job.items[itemIndex]
        let item: InstallJobSnapshot['items'][number]

        switch (update.type) {
            case 'started':
                item = {
                    ...(current ?? { id: update.id, name: update.name, bytes_downloaded: 0 }),
                    status: 'downloading',
                    bytes_total: current?.bytes_total ?? update.bytes_total,
                    attempt: update.attempt,
                    max_attempts: update.max_attempts,
                    error: null,
                    request_url: update.url,
                    source: update.source,
                }
                break
            case 'progress':
                if (!current) return
                item = {
                    ...current,
                    status: update.status,
                    bytes_downloaded: Math.max(current.bytes_downloaded, update.bytes),
                }
                job.summary = {
                    ...job.summary,
                    speed_bytes_per_second: update.speed_bytes_per_second,
                    eta_seconds: update.eta_seconds,
                }
                break
            case 'finished':
                if (!current) return
                item = {
                    ...current,
                    status: 'verifying',
                    bytes_downloaded: Math.max(current.bytes_downloaded, update.bytes),
                    bytes_total:
                        current.bytes_total ?? Math.max(current.bytes_downloaded, update.bytes),
                }
                break
            case 'failed':
                if (!current) return
                item = { ...current, status: 'failed' }
                break
        }

        if (itemIndex === -1) job.items.push(item)
        else job.items[itemIndex] = item
    }

    async function refresh(failOnError = false) {
        // A list request can race with realtime install/download events. Record
        // the local revision at dispatch so its older response cannot roll a job
        // back to the state it had before those events arrived.
        const revisionsAtDispatch = new Map(jobRevisions)
        const page = await download_job_list({ limit: 250 }).catch((error) => {
            if (failOnError) throw error
            handleError(error)
            return null
        })
        if (page && !disposed) {
            const refreshedJobs = mergeRefreshedDownloadJobs(
                page.jobs,
                jobs.value,
                revisionsAtDispatch,
                jobRevisions,
                syntheticIds,
                ACTIVE_INSTALL_JOB_STATUSES,
            )
            jobs.value = refreshedJobs.map((job) => {
                const current = jobs.value.find((candidate) => candidate.job_id === job.job_id)
                return current ? preserveMonotonicProgress(current, job) : job
            })
            const seenInstances = new Set<string>()
            for (const job of page.jobs) {
                if (job.status !== 'succeeded') continue
                const instanceId = installJobInstanceId(job)
                if (!instanceId || seenInstances.has(instanceId)) continue
                seenInstances.add(instanceId)
                persistManualDownloadsFromJob(job)
            }
        }
    }

    async function refreshLegacyDownloads(failOnError = false) {
        const bars = await progress_bars_list().catch((error) => {
            if (failOnError) throw error
            handleError(error)
            return {}
        })
        if (disposed) return
        legacyDownloads.value = Object.values(bars)
            .filter((bar) => downloadBarTypes.has(bar.bar_type?.type ?? ''))
            .map((bar) => ({
                ...bar,
                title:
                    bar.title ??
                    bar.bar_type?.pack_name ??
                    bar.bar_type?.instance_name ??
                    bar.message,
            }))
    }

    function scheduleLegacyRefresh() {
        if (legacyRefreshTimer !== null) return
        legacyRefreshTimer = setTimeout(() => {
            legacyRefreshTimer = null
            void refreshLegacyDownloads()
        }, 300)
    }

    function stopSubscriptions() {
        unlistenRequests?.()
        unlistenJobs?.()
        unlistenLoading?.()
        unlistenRequests = unlistenJobs = unlistenLoading = null
    }

    function start(): Promise<void> {
        if (startPromise) return startPromise
        if (started || disposed) return Promise.resolve()
        started = true
        initializing = true
        initializationState.value = 'loading'
        startPromise = (async () => {
            const subscriptions = await Promise.allSettled([
                download_request_listener((update: DownloadRequestUpdate) =>
                    updateRequest(update),
                ).then((unlisten) => {
                    if (disposed) unlisten()
                    else unlistenRequests = unlisten
                }),
                install_job_listener((job: InstallJobSnapshot) => setJob(job)).then((unlisten) => {
                    if (disposed) unlisten()
                    else unlistenJobs = unlisten
                }),
                loading_listener(() => {
                    if (!disposed) scheduleLegacyRefresh()
                }).then((unlisten) => {
                    if (disposed) unlisten()
                    else unlistenLoading = unlisten
                }),
            ])
            const failure = subscriptions.find((result) => result.status === 'rejected')
            if (failure) {
                stopSubscriptions()
                throw failure.reason
            }
            if (disposed) return
            // All subscriptions must be active before snapshots are dispatched.
            // Events received in the meantime are replayed after both snapshots.
            const snapshots = await Promise.allSettled([
                refresh(true),
                refreshLegacyDownloads(true),
            ])
            const snapshotFailure = snapshots.find((result) => result.status === 'rejected')
            if (snapshotFailure) throw snapshotFailure.reason
            if (!disposed) initializationState.value = 'ready'
        })()
            .catch((error) => {
                stopSubscriptions()
                started = false
                if (!disposed) initializationState.value = 'error'
                throw error
            })
            .finally(() => {
                initializing = false
                for (const update of pendingInitialUpdates.splice(0)) {
                    if (update.kind === 'job') setJob(update.job)
                    else updateRequest(update.update)
                }
                startPromise = undefined
            })
        return startPromise
    }

    async function cancel(jobId: string) {
        if (syntheticIds.has(jobId)) {
            // Remove the ID *before* removing the job from the list so that any
            // in-flight progress listener callback that calls setSyntheticJob
            // will see the missing ID and become a no-op, preventing the job
            // from being re-inserted.
            syntheticIds.delete(jobId)
            const handler = syntheticCancelHandlers.get(jobId)
            if (handler) {
                try {
                    await handler()
                } catch {
                    // Handler errors are non-fatal; still remove the job from the list.
                }
            }
            jobs.value = jobs.value.filter((job) => job.job_id !== jobId)
            return
        }
        const revision = jobRevisions.get(jobId) ?? 0
        const job = await download_job_cancel(jobId)
        await reconcileJob(job, revision)
    }

    async function retry(jobId: string) {
        const revision = jobRevisions.get(jobId) ?? 0
        const job = await download_job_retry(jobId)
        await reconcileJob(job, revision)
    }

    async function resume(jobId: string) {
        const revision = jobRevisions.get(jobId) ?? 0
        const job = await download_job_resume(jobId)
        await reconcileJob(job, revision)
    }

    async function skipMissingContent(jobId: string) {
        const revision = jobRevisions.get(jobId) ?? 0
        const job = await install_job_skip_missing_content(jobId)
        await reconcileJob(job, revision)
    }

    /**
     * The job may already have reached a terminal state (or been removed) by
     * the time the retry/cancel command returns. Fetch the freshest snapshot so
     * the UI never shows a stale queued/running spinner, and drop the row
     * entirely when the job no longer exists.
     */
    async function reconcileJob(job: InstallJobSnapshot, revision: number) {
        await reconcileAcceptedDownloadJob(
            job,
            download_job_get,
            setJob,
            (id) => {
                jobs.value = jobs.value.filter((candidate) => candidate.job_id !== id)
            },
            handleError,
            { atDispatch: revision, current: () => jobRevisions.get(job.job_id) ?? 0 },
        )
    }

    async function remove(jobId: string) {
        await download_job_delete(jobId)
        jobs.value = jobs.value.filter((job) => job.job_id !== jobId)
    }

    async function clearHistory() {
        await download_history_clear()
        jobs.value = jobs.value.filter((job) => isActiveInstallJobStatus(job.status))
    }

    const activeJobs = computed(() =>
        jobs.value.filter((job) => isActiveInstallJobStatus(job.status)),
    )
    const historyJobs = computed(() =>
        jobs.value.filter((job) => !isActiveInstallJobStatus(job.status)),
    )

    const syntheticIds = new Set<string>()
    const syntheticCancelHandlers = new Map<string, () => void | Promise<void>>()

    function addSyntheticJob(job: InstallJobSnapshot) {
        syntheticIds.add(job.job_id)
        // A server can be installed again after a previous synthetic record has
        // moved to history. Replace that record instead of creating duplicate
        // job IDs, which would make keyed download cards share a details view.
        jobs.value = [
            job,
            ...jobs.value.filter((candidate) => candidate.job_id !== job.job_id),
        ].sort((a, b) => b.created.localeCompare(a.created))
    }

    function setSyntheticJob(job: InstallJobSnapshot) {
        if (!syntheticIds.has(job.job_id)) return
        setJob(job)
    }

    function onSyntheticCancel(jobId: string, handler: () => void | Promise<void>) {
        syntheticCancelHandlers.set(jobId, handler)
    }

    function offSyntheticCancel(jobId: string) {
        syntheticCancelHandlers.delete(jobId)
    }

    return {
        initializationState,
        jobs,
        pendingContentChanges,
        legacyDownloads,
        activeJobs,
        historyJobs,
        activeCount: computed(() => activeJobs.value.length + legacyDownloads.value.length),
        queuedCount: computed(() => jobs.value.filter((job) => job.status === 'queued').length),
        start,
        refresh,
        cancel,
        retry,
        resume,
        skipMissingContent,
        remove,
        clearHistory,
        trackJob: setJob,
        queueContentChange,
        addSyntheticJob,
        setSyntheticJob,
        onSyntheticCancel,
        offSyntheticCancel,
        dispose() {
            disposed = true
            pendingContentChanges.value = []
            initializing = false
            pendingInitialUpdates.length = 0
            pendingRequestUpdatesByJob.clear()
            pendingProgressUpdateIndexes.clear()
            jobRevisions.clear()
            syntheticCancelHandlers.clear()
            if (requestFlushTimer !== null) {
                clearTimeout(requestFlushTimer)
                requestFlushTimer = null
            }
            if (legacyRefreshTimer !== null) {
                clearTimeout(legacyRefreshTimer)
                legacyRefreshTimer = null
            }
            pendingRequestUpdates.length = 0
            stopSubscriptions()
        },
    }
}

export const [injectDownloadManager, provideDownloadManager] = createContext<DownloadManager>(
    'root',
    'downloadManager',
)

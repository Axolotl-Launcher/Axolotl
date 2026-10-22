import { CheckIcon, CopyIcon, UpdatedIcon, XIcon } from '@modrinth/assets'
import {
    defineMessages,
    type PopupNotificationButton,
    type PopupNotificationProgressItem,
    type PopupNotificationProgressType,
    useVIntl,
} from '@modrinth/ui'
import { convertFileSrc } from '@tauri-apps/api/core'
import { computed, ref, watch } from 'vue'
import type { Router } from 'vue-router'

import {
    install_job_dismiss,
    install_job_repair_cache_and_retry,
    install_job_retry,
    install_job_support_details,
    installJobInstanceId,
    type InstallJobSnapshot,
    type InstallJobStatus,
    type InstallPhaseId,
    type InstallProgress,
} from '@/helpers/install'
import {
    effectiveInstallProgress,
    hasDeterminateInstallProgress,
    installProgressTextSource,
    preserveMonotonicProgress,
} from '@/helpers/install-progress'
import { get_many as getInstances } from '@/helpers/instance'
import type { DownloadManager } from '@/providers/download-manager'

import { installPhaseMessage } from './install-job-messages'

const messages = defineMessages({
    installs: {
        id: 'app.action-bar.installs',
        defaultMessage: 'Installs',
    },
    cancel: {
        id: 'app.action-bar.install.cancel',
        defaultMessage: 'Cancel',
    },
    retry: {
        id: 'app.action-bar.install.retry',
        defaultMessage: 'Retry',
    },
    copyDetails: {
        id: 'app.action-bar.install.copy-details',
        defaultMessage: 'Copy details',
    },
    copied: {
        id: 'app.action-bar.install.copied-details',
        defaultMessage: 'Copied',
    },
    dismiss: {
        id: 'app.action-bar.install.dismiss',
        defaultMessage: 'Dismiss',
    },
    completed: {
        id: 'app.downloads.status.succeeded',
        defaultMessage: 'Completed',
    },
    openInstance: {
        id: 'app.action-bar.install.open-instance',
        defaultMessage: 'Open instance',
    },
    unknownInstance: {
        id: 'app.action-bar.install.unknown-instance',
        defaultMessage: 'Unknown instance',
    },
    deletedInstance: {
        id: 'app.action-bar.install.deleted-instance',
        defaultMessage: 'Deleted instance',
    },
    cacheRepairTitle: {
        id: 'app.action-bar.install.cache-repair.title',
        defaultMessage: 'Project metadata cache error',
    },
    cacheRepairDescription: {
        id: 'app.action-bar.install.cache-repair.description',
        defaultMessage:
            'The launcher could not read the local CurseForge project cache, so installation was stopped.',
    },
    cacheRepairAction: {
        id: 'app.action-bar.install.cache-repair.action',
        defaultMessage: 'Clear project cache and retry',
    },
    cacheRepairing: {
        id: 'app.action-bar.install.cache-repair.in-progress',
        defaultMessage: 'Clearing project cache…',
    },
    cacheRepairFailed: {
        id: 'app.action-bar.install.cache-repair.failed',
        defaultMessage: 'Project cache cleanup failed; retry was not started.',
    },
})

const javaStepMessages = defineMessages({
    resolving: {
        id: 'app.install.phase.preparing_java.resolving',
        defaultMessage: 'Preparing Java {version}',
    },
    fetching_metadata: {
        id: 'app.install.phase.preparing_java.fetching-metadata',
        defaultMessage: 'Fetching Java {version}',
    },
    downloading: {
        id: 'app.install.phase.preparing_java.downloading',
        defaultMessage: 'Downloading Java {version}',
    },
    extracting: {
        id: 'app.install.phase.preparing_java.extracting',
        defaultMessage: 'Extracting Java {version}',
    },
    validating: {
        id: 'app.install.phase.preparing_java.validating',
        defaultMessage: 'Validating Java {version}',
    },
})

const failureSummaryMessages = defineMessages({
    canceled: {
        id: 'app.action-bar.install.summary.canceled',
        defaultMessage: 'Canceled',
    },
    appClosed: {
        id: 'app.action-bar.install.summary.app-closing',
        defaultMessage: 'Canceled due to app closing',
    },
    downloadFailed: {
        id: 'app.action-bar.install.summary.download-failed',
        defaultMessage: "Download couldn't finish",
    },
    fileDownloadFailed: {
        id: 'app.action-bar.install.summary.file-download-failed',
        defaultMessage: "Couldn't download {file}",
    },
    modrinthUnreachable: {
        id: 'app.action-bar.install.summary.modrinth-unreachable',
        defaultMessage: "Couldn't reach Modrinth",
    },
    packDownloadFailed: {
        id: 'app.action-bar.install.summary.pack-download-failed',
        defaultMessage: "Couldn't download pack",
    },
    badModpackFile: {
        id: 'app.action-bar.install.summary.bad-modpack-file',
        defaultMessage: "Couldn't read modpack",
    },
    unrecognizedFormat: {
        id: 'app.action-bar.install.summary.unrecognized-format',
        defaultMessage: 'This does not appear to be a modpack archive',
    },
    invalidModpack: {
        id: 'app.action-bar.install.summary.invalid-modpack',
        defaultMessage: 'Modpack data invalid',
    },
    contentDownloadFailed: {
        id: 'app.action-bar.install.summary.content-download-failed',
        defaultMessage: "Couldn't download files",
    },
    corruptDownload: {
        id: 'app.action-bar.install.summary.corrupt-download',
        defaultMessage: 'Downloaded file is corrupt',
    },
    invalidModpackFiles: {
        id: 'app.action-bar.install.summary.invalid-modpack-files',
        defaultMessage: 'Modpack files have invalid metadata',
    },
    noWritePermission: {
        id: 'app.action-bar.install.summary.no-write-permission',
        defaultMessage: 'No permission to write',
    },
    couldNotSaveFiles: {
        id: 'app.action-bar.install.summary.could-not-save-files',
        defaultMessage: "Couldn't save files",
    },
    invalidFilePath: {
        id: 'app.action-bar.install.summary.invalid-file-path',
        defaultMessage: 'File path is invalid',
    },
    instanceNotFound: {
        id: 'app.action-bar.install.summary.instance-not-found',
        defaultMessage: "Instance couldn't be found",
    },
    cleanupIncomplete: {
        id: 'app.action-bar.install.summary.cleanup-incomplete',
        defaultMessage: "Cleanup didn't finish",
    },
    javaSetupFailed: {
        id: 'app.action-bar.install.summary.java-setup-failed',
        defaultMessage: "Java setup couldn't finish",
    },
    minecraftSetupFailed: {
        id: 'app.action-bar.install.summary.minecraft-setup-failed',
        defaultMessage: 'Minecraft setup failed',
    },
    loaderSetupFailed: {
        id: 'app.action-bar.install.summary.loader-setup-failed',
        defaultMessage: 'Loader setup failed',
    },
    localDataError: {
        id: 'app.action-bar.install.summary.local-data-error',
        defaultMessage: "Couldn't update local data",
    },
    unexpectedError: {
        id: 'app.action-bar.install.summary.unexpected-error',
        defaultMessage: 'Something went wrong',
    },
})

const visibleJobStatuses = new Set<InstallJobStatus>([
    'queued',
    'running',
    'canceling',
    'waiting_for_user',
    'failed',
    'interrupted',
])
const retainedJobStatuses = new Set<InstallJobStatus>(['succeeded', 'canceled'])
const activeJobStatuses = new Set<InstallJobStatus>([
    'queued',
    'running',
    'canceling',
    'waiting_for_user',
])
const cancelableJobStatuses = new Set<InstallJobStatus>(['queued', 'running', 'waiting_for_user'])
const copyDetailsStallMs = 30_000

interface ProgressSnapshot {
    signature: string
    changedAt: number
    timeout: number | null
}

function getDisplayIconUrl(icon: string | null | undefined): string | null {
    if (!icon) return null
    if (/^(https?:|data:|blob:|asset:|tauri:)/.test(icon)) return icon
    return convertFileSrc(icon)
}

export async function useInstallJobNotifications(opts: {
    router: Router
    manager: DownloadManager
    handleError: (err: unknown) => void
    onChange: (resummon?: boolean) => void
}) {
    const { formatMessage } = useVIntl()
    const jobs = ref<InstallJobSnapshot[]>([])
    const iconUrls = ref<Record<string, string | null>>({})
    const instanceNames = ref<Record<string, string>>({})
    const copiedJobIds = ref<Set<string>>(new Set())
    const repairingJobIds = ref<Set<string>>(new Set())
    const jobOrder = new Map<string, number>()
    let refreshRequest = 0
    let metadataRequest = 0
    let nextJobOrder = 0
    const copiedResetTimeouts = new Map<string, number>()
    const progressSnapshots = new Map<string, ProgressSnapshot>()

    function getTitle(job: InstallJobSnapshot): string {
        if (job.error?.code === 'cache_repair_required') {
            return formatMessage(messages.cacheRepairTitle)
        }
        if (job.display?.title) return job.display.title
        if (job.details.type === 'instance') return job.details.name
        if (job.details.type === 'modpack' && job.details.title) return job.details.title
        const instanceId = installJobInstanceId(job)
        return job.instance_deleted
            ? formatMessage(messages.deletedInstance)
            : ((instanceId ? instanceNames.value[instanceId] : null) ??
                  formatMessage(messages.unknownInstance))
    }

    function getText(job: InstallJobSnapshot): string {
        if (job.error?.code === 'cache_repair_required') {
            return formatMessage(messages.cacheRepairDescription)
        }
        if (job.status === 'succeeded') {
            return formatMessage(messages.completed)
        }
        if (job.status === 'canceled') {
            return formatMessage(failureSummaryMessages.canceled)
        }
        if (job.status === 'failed' || job.status === 'interrupted') {
            return getFailureSummary(job)
        }
        if (job.phase === 'preparing_java' && job.details.type === 'java') {
            return formatMessage(javaStepMessages[job.details.step], {
                version: job.details.major_version,
            })
        }
        return formatMessage(installPhaseMessage(job.phase))
    }

    function getFailureSummary(job: InstallJobSnapshot): string {
        const code = job.error?.code
        const phase = job.error?.phase ?? job.phase

        if (code === 'app_closed' || (job.status === 'interrupted' && code === 'interrupted')) {
            return formatMessage(failureSummaryMessages.appClosed)
        }
        if (job.rollback_error || code === 'rollback_error') {
            return formatMessage(failureSummaryMessages.cleanupIncomplete)
        }
        if (code === 'canceled') {
            return formatMessage(failureSummaryMessages.canceled)
        }
        if (hasPermissionError(job)) {
            return formatMessage(failureSummaryMessages.noWritePermission)
        }

        switch (code) {
            case 'network_error':
                if (job.error?.context?.file_path) {
                    return formatMessage(failureSummaryMessages.fileDownloadFailed, {
                        file: job.error.context.file_path,
                    })
                }
                return formatMessage(
                    phase === 'downloading_pack_file'
                        ? failureSummaryMessages.packDownloadFailed
                        : failureSummaryMessages.downloadFailed,
                )
            case 'api_error':
                return formatMessage(failureSummaryMessages.modrinthUnreachable)
            case 'unrecognized_format':
                return formatMessage(failureSummaryMessages.unrecognizedFormat)
            case 'pack_error':
                return formatMessage(
                    phase === 'downloading_pack_file'
                        ? failureSummaryMessages.packDownloadFailed
                        : failureSummaryMessages.invalidModpack,
                )
            case 'archive_error':
                return formatMessage(failureSummaryMessages.badModpackFile)
            case 'parse_error':
                return formatMessage(failureSummaryMessages.invalidModpack)
            case 'content_error':
                return formatMessage(failureSummaryMessages.invalidModpackFiles)
            case 'hash_error':
                return formatMessage(failureSummaryMessages.corruptDownload)
            case 'filesystem_error':
                return formatMessage(failureSummaryMessages.couldNotSaveFiles)
            case 'path_error':
                return formatMessage(failureSummaryMessages.invalidFilePath)
            case 'instance_error':
                return formatMessage(failureSummaryMessages.instanceNotFound)
            case 'java_error':
                return formatMessage(failureSummaryMessages.javaSetupFailed)
            case 'loader_error':
            case 'processor_error':
                return formatMessage(failureSummaryMessages.loaderSetupFailed)
            case 'database_error':
                return formatMessage(failureSummaryMessages.localDataError)
            case 'launcher_error':
            case 'metadata_error':
                return getFailureSummaryForPhase(phase)
            default:
                return getFailureSummaryForPhase(phase)
        }
    }

    function getFailureSummaryForPhase(phase: InstallPhaseId): string {
        switch (phase) {
            case 'downloading_pack_file':
                return formatMessage(failureSummaryMessages.packDownloadFailed)
            case 'resolving_pack':
            case 'reading_pack_manifest':
                return formatMessage(failureSummaryMessages.invalidModpack)
            case 'downloading_content':
                return formatMessage(failureSummaryMessages.contentDownloadFailed)
            case 'extracting_overrides':
                return formatMessage(failureSummaryMessages.couldNotSaveFiles)
            case 'resolving_minecraft':
            case 'downloading_minecraft':
                return formatMessage(failureSummaryMessages.minecraftSetupFailed)
            case 'resolving_loader':
            case 'running_loader_processors':
                return formatMessage(failureSummaryMessages.loaderSetupFailed)
            case 'preparing_java':
                return formatMessage(failureSummaryMessages.javaSetupFailed)
            case 'preparing_instance':
                return formatMessage(failureSummaryMessages.instanceNotFound)
            case 'rolling_back':
                return formatMessage(failureSummaryMessages.cleanupIncomplete)
            default:
                return formatMessage(failureSummaryMessages.unexpectedError)
        }
    }

    function hasPermissionError(job: InstallJobSnapshot): boolean {
        const message = job.error?.message.toLowerCase() ?? ''
        return (
            message.includes('permission denied') ||
            message.includes('access is denied') ||
            message.includes('operation not permitted')
        )
    }

    function getProgressType(job: InstallJobSnapshot): PopupNotificationProgressType | undefined {
        if (!hasDeterminateInstallProgress(getEffectiveProgress(job))) return undefined
        if (
            job.phase === 'preparing_java' &&
            job.details.type === 'java' &&
            job.details.step === 'downloading'
        ) {
            return 'bytes'
        }
        if (job.phase === 'downloading_content') {
            return installProgressTextSource(job).type === 'bytes' ? 'bytes' : 'count'
        }
        if (
            job.phase === 'exporting' ||
            job.phase === 'downloading_pack_file' ||
            job.phase === 'extracting_overrides' ||
            job.phase === 'downloading_minecraft'
        ) {
            return 'bytes'
        }
        if (job.phase === 'running_loader_processors') {
            return 'count'
        }
        return 'percentage'
    }

    function getEffectiveProgress(job: InstallJobSnapshot): InstallProgress | null | undefined {
        return effectiveInstallProgress(job)
    }

    function getProgress(job: InstallJobSnapshot): number {
        if (job.status === 'succeeded') return 1
        if (job.status === 'failed' || job.status === 'interrupted') return 0
        const progress = getEffectiveProgress(job)
        if (!hasDeterminateInstallProgress(progress)) return 0
        return Math.max(0, Math.min(1, progress.current / progress.total))
    }

    function isFailedJob(job: InstallJobSnapshot): boolean {
        return job.status === 'failed' || job.status === 'interrupted'
    }

    function isFinishedJob(job: InstallJobSnapshot): boolean {
        return isFailedJob(job) || retainedJobStatuses.has(job.status)
    }

    function canShowStalledProgressDetails(job: InstallJobSnapshot): boolean {
        return (
            job.status === 'running' &&
            job.phase !== 'preparing_instance' &&
            job.phase !== 'finalizing' &&
            job.phase !== 'rolling_back'
        )
    }

    function getJobSortRank(job: InstallJobSnapshot): number {
        if (isFinishedJob(job)) return 0
        if (job.status === 'queued' || job.phase === 'preparing_instance') return 2
        return 1
    }

    function progressSignature(job: InstallJobSnapshot): string {
        const progress = job.progress
        const secondary = progress?.secondary
        return [
            job.status,
            job.phase,
            JSON.stringify(job.details),
            progress?.current ?? '',
            progress?.total ?? '',
            secondary?.current ?? '',
            secondary?.total ?? '',
        ].join(':')
    }

    function clearCopied(jobId: string) {
        if (!copiedJobIds.value.has(jobId)) {
            return
        }

        const timeout = copiedResetTimeouts.get(jobId)
        if (timeout != null) {
            window.clearTimeout(timeout)
            copiedResetTimeouts.delete(jobId)
        }

        const nextCopiedJobIds = new Set(copiedJobIds.value)
        nextCopiedJobIds.delete(jobId)
        copiedJobIds.value = nextCopiedJobIds
    }

    function clearProgressSnapshot(jobId: string) {
        const snapshot = progressSnapshots.get(jobId)
        if (snapshot?.timeout != null) {
            window.clearTimeout(snapshot.timeout)
        }
        progressSnapshots.delete(jobId)
    }

    function scheduleStaleProgressRefresh(jobId: string) {
        const snapshot = progressSnapshots.get(jobId)
        if (!snapshot) {
            return
        }

        snapshot.timeout = window.setTimeout(() => {
            const snapshot = progressSnapshots.get(jobId)
            if (!snapshot) {
                return
            }

            snapshot.timeout = null
            opts.onChange()
        }, copyDetailsStallMs)
    }

    function syncProgressSnapshots(nextJobs: InstallJobSnapshot[]) {
        const trackedJobIds = new Set<string>()
        const now = Date.now()

        for (const job of nextJobs) {
            if (!canShowStalledProgressDetails(job)) {
                continue
            }

            trackedJobIds.add(job.job_id)
            const signature = progressSignature(job)
            const snapshot = progressSnapshots.get(job.job_id)
            if (snapshot?.signature === signature) {
                continue
            }

            clearProgressSnapshot(job.job_id)
            clearCopied(job.job_id)
            progressSnapshots.set(job.job_id, {
                signature,
                changedAt: now,
                timeout: null,
            })
            scheduleStaleProgressRefresh(job.job_id)
        }

        for (const jobId of progressSnapshots.keys()) {
            if (!trackedJobIds.has(jobId)) {
                clearProgressSnapshot(jobId)
            }
        }
    }

    function hasStalledProgress(job: InstallJobSnapshot): boolean {
        const snapshot = progressSnapshots.get(job.job_id)
        return !!snapshot && Date.now() - snapshot.changedAt >= copyDetailsStallMs
    }

    function shouldShowCopyDetails(job: InstallJobSnapshot): boolean {
        return isFailedJob(job) || (canShowStalledProgressDetails(job) && hasStalledProgress(job))
    }

    function isCopied(job: InstallJobSnapshot): boolean {
        return copiedJobIds.value.has(job.job_id)
    }

    function setCopied(job: InstallJobSnapshot) {
        copiedJobIds.value = new Set([...copiedJobIds.value, job.job_id])
        const existingTimeout = copiedResetTimeouts.get(job.job_id)
        if (existingTimeout != null) {
            window.clearTimeout(existingTimeout)
        }
        copiedResetTimeouts.set(
            job.job_id,
            window.setTimeout(() => {
                copiedResetTimeouts.delete(job.job_id)
                if (!copiedJobIds.value.has(job.job_id)) {
                    return
                }
                const nextCopiedJobIds = new Set(copiedJobIds.value)
                nextCopiedJobIds.delete(job.job_id)
                copiedJobIds.value = nextCopiedJobIds
                opts.onChange()
            }, 1_000),
        )
        opts.onChange()
    }

    async function copyJobDetails(job: InstallJobSnapshot) {
        const details = await install_job_support_details(job.job_id).catch((error) => {
            opts.handleError(error)
            return null
        })
        if (!details) {
            return
        }
        try {
            await navigator.clipboard.writeText(details)
            setCopied(job)
        } catch (error) {
            opts.handleError(error)
        }
    }

    function getButtons(job: InstallJobSnapshot): PopupNotificationButton[] {
        const buttons: PopupNotificationButton[] = []

        if (cancelableJobStatuses.has(job.status)) {
            buttons.push({
                label: formatMessage(messages.cancel),
                icon: XIcon,
                color: 'red',
                keepOpen: true,
                action: async () => {
                    await opts.manager.cancel(job.job_id).catch(opts.handleError)
                },
            })
        }

        if (isFailedJob(job)) {
            const requiresCacheRepair = job.error?.code === 'cache_repair_required'
            const repairing = repairingJobIds.value.has(job.job_id)
            buttons.push({
                label: formatMessage(
                    requiresCacheRepair
                        ? repairing
                            ? messages.cacheRepairing
                            : messages.cacheRepairAction
                        : messages.retry,
                ),
                icon: UpdatedIcon,
                color: 'brand',
                keepOpen: true,
                action: async () => {
                    if (repairingJobIds.value.has(job.job_id)) return
                    if (!requiresCacheRepair) {
                        await install_job_retry(job.job_id).catch(opts.handleError)
                        await refresh()
                        return
                    }

                    repairingJobIds.value = new Set([...repairingJobIds.value, job.job_id])
                    opts.onChange()
                    try {
                        const snapshot = await install_job_repair_cache_and_retry(job.job_id)
                        setJobs(
                            jobs.value.map((item) =>
                                item.job_id === job.job_id ? snapshot : item,
                            ),
                        )
                        opts.onChange()
                        await refresh()
                    } catch (error) {
                        opts.handleError(
                            new Error(
                                `${formatMessage(messages.cacheRepairFailed)} ${String(error)}`,
                            ),
                        )
                    } finally {
                        const next = new Set(repairingJobIds.value)
                        next.delete(job.job_id)
                        repairingJobIds.value = next
                        opts.onChange()
                    }
                },
            })
        }

        if (shouldShowCopyDetails(job)) {
            const copied = isCopied(job)
            buttons.push({
                label: formatMessage(copied ? messages.copied : messages.copyDetails),
                icon: copied ? CheckIcon : CopyIcon,
                color: 'standard',
                keepOpen: true,
                action: async () => {
                    await copyJobDetails(job)
                },
            })
        }

        return buttons
    }

    function setJobs(nextJobs: InstallJobSnapshot[]) {
        const currentJobs = new Map(jobs.value.map((job) => [job.job_id, job]))
        nextJobs = nextJobs.map((job) => {
            const current = currentJobs.get(job.job_id)
            return current ? preserveMonotonicProgress(current, job) : job
        })

        for (const job of nextJobs) {
            if (!jobOrder.has(job.job_id)) {
                jobOrder.set(job.job_id, nextJobOrder++)
            }
        }

        const currentJobIds = new Set(jobs.value.map((job) => job.job_id))
        const visibleJobs = nextJobs.filter(
            (job) =>
                visibleJobStatuses.has(job.status) ||
                (retainedJobStatuses.has(job.status) && currentJobIds.has(job.job_id)),
        )
        syncProgressSnapshots(visibleJobs)

        jobs.value = visibleJobs.sort(
            (a, b) =>
                getJobSortRank(a) - getJobSortRank(b) ||
                a.created.localeCompare(b.created) ||
                (jobOrder.get(a.job_id) ?? 0) - (jobOrder.get(b.job_id) ?? 0),
        )
    }

    const progressItems = computed<PopupNotificationProgressItem[]>(() =>
        jobs.value.map((job) => {
            const progress = getEffectiveProgress(job)
            const finished = isFinishedJob(job)

            return {
                id: job.job_id,
                title: getTitle(job),
                text: getText(job),
                iconUrl: iconUrls.value[job.job_id] ?? null,
                progress: getProgress(job),
                waiting:
                    ['queued', 'running'].includes(job.status) &&
                    !hasDeterminateInstallProgress(progress),
                showProgress: !finished,
                wrapText: isFailedJob(job),
                progressType: finished ? undefined : getProgressType(job),
                progressCurrent:
                    !finished && hasDeterminateInstallProgress(progress)
                        ? progress.current
                        : undefined,
                progressTotal:
                    !finished && hasDeterminateInstallProgress(progress)
                        ? progress.total
                        : undefined,
                buttons: getButtons(job),
                onDismiss: finished
                    ? async () => {
                          await install_job_dismiss(job.job_id).catch(opts.handleError)
                          await refresh()
                      }
                    : undefined,
            }
        }),
    )

    const buttons = computed<PopupNotificationButton[] | undefined>(() => undefined)

    // Same ids with a different status or phase mean the job moved on, so the UI
    // has to treat it as new information (for example after a dismissed download
    // notification was remembered across restarts).
    const stateSignature = computed(() =>
        jobs.value
            .map((job) => `${job.job_id}:${job.status}:${job.phase}`)
            .sort()
            .join('|'),
    )

    async function refreshMetadata(notify = true) {
        const request = ++metadataRequest
        const sourceJobs = jobs.value.filter((job) => !job.instance_deleted)
        const instanceIds = Array.from(
            new Set(
                sourceJobs
                    .map((job) => installJobInstanceId(job))
                    .filter((instanceId): instanceId is string => !!instanceId),
            ),
        )
        let metadataError: unknown = null
        const instances = instanceIds.length
            ? await getInstances(instanceIds).catch((error) => {
                  metadataError = error
                  return []
              })
            : []

        if (request !== metadataRequest) {
            return
        }
        if (metadataError) {
            opts.handleError(metadataError)
            return
        }

        const instanceIconUrls = new Map(
            instances.map((instance) => [instance.id, getDisplayIconUrl(instance.icon_path)]),
        )
        instanceNames.value = Object.fromEntries(
            instances.map((instance) => [instance.id, instance.name]),
        )
        iconUrls.value = Object.fromEntries(
            sourceJobs.map((job) => [
                job.job_id,
                getDisplayIconUrl(job.display?.icon) ??
                    instanceIconUrls.get(installJobInstanceId(job) ?? '') ??
                    null,
            ]),
        )

        if (notify) {
            opts.onChange()
        }
    }

    async function refresh(notify = true) {
        const request = ++refreshRequest
        await opts.manager.refresh().catch((error) => {
            opts.handleError(error)
        })
        const nextJobs = opts.manager.jobs.value

        if (request !== refreshRequest) {
            return
        }

        setJobs(nextJobs)
        await refreshMetadata(false)

        if (request !== refreshRequest) {
            return
        }

        if (notify) {
            opts.onChange()
        }
    }

    let lastCompositionSignature: string | null = null
    let lastMetadataRefreshAt = 0
    let lastEnteredPackDownload = false

    setJobs(opts.manager.jobs.value)
    await refreshMetadata(false)
    const stopJobsWatch = watch(
        () => opts.manager.jobs.value,
        (nextJobs) => {
            const previousPhases = new Map(jobs.value.map((job) => [job.job_id, job.phase]))
            const enteredPackDownload = nextJobs.some(
                (job) =>
                    job.phase === 'downloading_pack_file' &&
                    previousPhases.get(job.job_id) !== 'downloading_pack_file',
            )
            const signature = nextJobs
                .map((job) => `${job.job_id}:${job.status}:${job.phase}`)
                .join('|')
            const compositionChanged = signature !== lastCompositionSignature
            lastCompositionSignature = signature
            setJobs(nextJobs)
            if (enteredPackDownload !== lastEnteredPackDownload) {
                lastEnteredPackDownload = enteredPackDownload
                opts.onChange(enteredPackDownload)
            }
            if (compositionChanged || Date.now() - lastMetadataRefreshAt >= 2000) {
                lastMetadataRefreshAt = Date.now()
                void refreshMetadata()
            }
        },
    )

    return {
        active: computed(() => jobs.value.some((job) => activeJobStatuses.has(job.status))),
        hasItems: computed(() => jobs.value.length > 0),
        title: computed(() => formatMessage(messages.installs)),
        progressItems,
        stateSignature,
        buttons,
        refresh,
        dispose: () => {
            for (const timeout of copiedResetTimeouts.values()) {
                window.clearTimeout(timeout)
            }
            for (const jobId of progressSnapshots.keys()) {
                clearProgressSnapshot(jobId)
            }
            stopJobsWatch()
        },
    }
}

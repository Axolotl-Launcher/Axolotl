import { computed, ref } from 'vue'

import { loadInstanceContent, loadProject, projectIcon } from '../../data'
import { type PreviewState, usePreview, usePreviewDomain } from '../../lib/state'
import type { InstallJob } from '../../lib/state/downloads'
import { isActive, type LocalJob, useLocalJobs } from './local-jobs'

/**
 * The page's view of a job: the fields of the app's `InstallJobSnapshot`
 * (`helpers/install.ts`) that `pages/Downloads.vue` reads, built from the
 * shared content-install queue and from the library's local jobs.
 */
export type InstallJobStatus =
    | 'queued'
    | 'running'
    | 'canceling'
    | 'waiting_for_user'
    | 'succeeded'
    | 'failed'
    | 'interrupted'
    | 'canceled'

export type JobProvider =
    'modrinth' | 'curse_forge' | 'minecraft' | 'java' | 'application' | 'local'

export type DownloadItemStatus =
    | 'queued'
    | 'downloading'
    | 'verifying'
    | 'writing'
    | 'completed'
    | 'failed'
    | 'canceled'
    | 'skipped'

export interface DownloadItem {
    id: string
    name: string
    status: DownloadItemStatus
    bytes_downloaded: number
    bytes_total: number | null
    attempt: number | null
    max_attempts: number | null
    error: string | null
    request_url: string | null
    source: string | null
    project_id: string | null
    version_id: string | null
}

export interface ProgressValue {
    current: number
    total: number
    secondary?: { current: number; total: number } | null
}

export interface DownloadJob {
    job_id: string
    kind: 'install_content' | 'duplicate_instance' | 'install_existing_instance'
    provider: JobProvider
    status: InstallJobStatus
    phase: string
    progress: ProgressValue | null
    summary: {
        files_completed: number
        files_total: number | null
        bytes_downloaded: number
        bytes_total: number | null
        source: string | null
        fallback_count: number
        speed_bytes_per_second: number | null
        eta_seconds: number | null
    }
    items: DownloadItem[]
    display: { title: string; icon: string | null } | null
    /** The backend's `details`: instance jobs carry the instance's name. */
    details: { type: 'empty' } | { type: 'instance'; name: string }
    instance_id: string | null
    instance_deleted: boolean
    created: string
    modified: string
    finished: string | null
    error: { message: string } | null
    rollback_error: { message: string } | null
}

/** `MODRINTH_CDN_ATTEMPTS` in `packages/app-lib/src/util/fetch.rs`. */
const CDN_ATTEMPTS = 3
/** Error the backend records for a canceled job (`finish_canceled_job`). */
const CANCELED_MESSAGE = 'Install was canceled'

function cdnUrl(projectId: string, versionId: string, fileName: string) {
    return `https://cdn.modrinth.com/data/${projectId}/versions/${versionId}/${encodeURIComponent(fileName)}`
}

function contentStatus(phase: InstallJob['phase']): InstallJobStatus {
    switch (phase) {
        case 'queued':
            return 'queued'
        case 'completed':
            return 'succeeded'
        case 'failed':
            return 'failed'
        case 'canceled':
            return 'canceled'
        default:
            return 'running'
    }
}

/**
 * A content install (`InstallRequest::InstallContent`): it runs in the
 * `downloading_content` phase with the file count as progress and the bytes as
 * secondary progress, and ends in `finalizing` once it succeeded, failed or was
 * canceled (`lifecycle.rs`, `recovery.rs`).
 */
export function contentJobView(
    job: InstallJob,
    options: { deleted: boolean; created?: string; canceling?: boolean },
): DownloadJob {
    const status = options.canceling ? 'canceling' : contentStatus(job.phase)
    const finished = status === 'succeeded' || status === 'failed' || status === 'canceled'
    const started = job.phase !== 'queued' && job.phase !== 'resolving_project'
    // A job canceled before its first request has no file rows.
    const transferred = started && (job.phase !== 'canceled' || job.bytes_done > 0)
    const bytesDone =
        job.phase === 'completed' || job.phase === 'installing' ? job.size : job.bytes_done
    let itemStatus: DownloadItemStatus = 'downloading'
    if (job.phase === 'installing') itemStatus = 'writing'
    else if (job.phase === 'completed') itemStatus = 'completed'
    else if (job.phase === 'failed') itemStatus = 'failed'
    else if (job.phase === 'canceled') itemStatus = 'canceled'
    const items: DownloadItem[] = transferred
        ? [
              {
                  id: `mods/${job.file_name}`,
                  name: job.file_name,
                  status: itemStatus,
                  bytes_downloaded: bytesDone,
                  bytes_total: job.size,
                  attempt: job.phase === 'failed' ? CDN_ATTEMPTS : 1,
                  max_attempts: CDN_ATTEMPTS,
                  error: job.phase === 'failed' ? job.error : null,
                  request_url: cdnUrl(job.project_id, job.version_id, job.file_name),
                  source: 'official',
                  project_id: job.project_id,
                  version_id: job.version_id,
              },
          ]
        : []
    const running = status === 'running' || status === 'canceling'
    const speed = running && job.phase === 'downloading' && job.speed > 0 ? job.speed : null
    let error: { message: string } | null = null
    if (job.phase === 'failed') error = { message: job.error ?? '' }
    else if (job.phase === 'canceled') error = { message: CANCELED_MESSAGE }
    return {
        job_id: job.id,
        kind: 'install_content',
        provider: 'modrinth',
        status,
        phase: finished ? 'finalizing' : 'downloading_content',
        progress:
            finished || !started
                ? null
                : {
                      current: job.phase === 'installing' ? 1 : 0,
                      total: 1,
                      secondary: { current: bytesDone, total: job.size },
                  },
        summary: {
            files_completed: job.phase === 'installing' || job.phase === 'completed' ? 1 : 0,
            files_total: started || finished ? 1 : null,
            bytes_downloaded: bytesDone,
            bytes_total: started || finished ? job.size : null,
            source: started || finished ? 'official' : null,
            fallback_count: 0,
            speed_bytes_per_second: speed,
            eta_seconds: speed ? Math.max(0, (job.size - bytesDone) / speed) : null,
        },
        items,
        display: { title: job.project_title, icon: job.project_icon },
        details: { type: 'empty' },
        instance_id: job.instance_id,
        instance_deleted: options.deleted,
        created: options.created ?? job.created_at,
        modified: job.finished_at ?? options.created ?? job.created_at,
        finished: job.finished_at,
        error,
        rollback_error: null,
    }
}

/** A duplicate or repair job of the library (no download requests of its own). */
export function localJobView(job: LocalJob, deleted: boolean): DownloadJob {
    return {
        job_id: job.job_id,
        kind: job.kind,
        provider: job.provider,
        status: job.status,
        phase: job.phase,
        progress: job.progress,
        summary: {
            files_completed: 0,
            files_total: null,
            bytes_downloaded: 0,
            bytes_total: null,
            source: null,
            fallback_count: 0,
            speed_bytes_per_second: null,
            eta_seconds: null,
        },
        items: [],
        display: { title: job.title, icon: job.icon },
        details: { type: 'instance', name: job.title },
        instance_id: job.instance_id,
        instance_deleted: deleted,
        created: job.created,
        modified: job.modified,
        finished: job.finished,
        error: job.error ? { message: job.error } : null,
        rollback_error: null,
    }
}

const MINUTE = 60 * 1000
const HOUR = 60 * MINUTE
const DAY = 24 * HOUR

/**
 * Earlier jobs of the preview user, recorded like the app's download history:
 * the hand-built Fabric 26.3 instance got its mods one install at a time (the
 * files `data/instance-content.json` lists for it), one install timed out and
 * one was canceled. Projects, versions, files and sizes come from the snapshot.
 */
async function seededHistory(): Promise<InstallJob[]> {
    const instanceId = 'fabric-26-3'
    const instanceName = 'Fabric 26.3'
    const [content, failedProject, canceledProject] = await Promise.all([
        loadInstanceContent(),
        loadProject('1IjD5062'),
        loadProject('LQ3K71Q1'),
    ])
    const rows = content.instances['custom-fabric'] ?? []
    const completed: [string, number, number][] = [
        ['1bokaNcj', 2 * DAY + 3 * HOUR, 9_000],
        ['YL57xq9U', 8 * DAY + 5 * HOUR, 11_000],
        ['AANobbMI', 9 * DAY - 20 * MINUTE, 8_000],
        ['P7dR8mSH', 9 * DAY - 15 * MINUTE, 7_000],
    ]
    const now = Date.now()
    const iso = (ago: number) => new Date(now - ago).toISOString()
    const records: InstallJob[] = []
    for (const [projectId, finishedAgo, duration] of completed) {
        const row = rows.find((entry) => entry[3] === projectId)
        if (!row) continue
        const [fileName, , size, , versionId, versionNumber, title, projectType] = row
        records.push({
            id: `job-${projectId.toLowerCase()}-${versionId?.toLowerCase()}`,
            project_id: projectId,
            project_title: title ?? fileName,
            project_icon: projectIcon(projectId),
            project_type: projectType,
            version_id: versionId ?? '',
            version_name: versionNumber ?? '',
            file_name: fileName,
            size,
            instance_id: instanceId,
            instance_name: instanceName,
            phase: 'completed',
            progress: 1,
            speed: 0,
            bytes_done: size,
            created_at: iso(finishedAgo + duration),
            finished_at: iso(finishedAgo),
            error: null,
        })
    }

    function fromProject(
        project: Awaited<ReturnType<typeof loadProject>>,
        phase: 'failed' | 'canceled',
        finishedAgo: number,
        share: number,
    ): InstallJob | null {
        const version =
            project.versions.find(
                (entry) =>
                    entry.game_versions.includes('26.3') &&
                    entry.loaders.includes('fabric') &&
                    entry.version_type === 'release',
            ) ?? null
        const file = version?.files.find((entry) => entry.primary) ?? version?.files[0]
        if (!version || !file) return null
        const url = cdnUrl(project.id, version.id, file.filename)
        return {
            id: `job-${project.id.toLowerCase()}-${version.id.toLowerCase()}`,
            project_id: project.id,
            project_title: project.title,
            project_icon: projectIcon(project.id),
            project_type: project.project_type,
            version_id: version.id,
            version_name: version.version_number,
            file_name: file.filename,
            size: file.size,
            instance_id: instanceId,
            instance_name: instanceName,
            phase,
            progress: share,
            speed: 0,
            bytes_done: Math.round(file.size * share),
            created_at: iso(finishedAgo + (phase === 'failed' ? 95_000 : 6_000)),
            finished_at: iso(finishedAgo),
            error:
                phase === 'failed'
                    ? `Error fetching URL: error sending request for url (${url}): operation timed out`
                    : null,
        }
    }

    const failed = fromProject(failedProject, 'failed', 26 * HOUR, 0.38)
    const canceled = fromProject(canceledProject, 'canceled', 5 * DAY + 2 * HOUR, 0.21)
    if (failed) records.push(failed)
    if (canceled) records.push(canceled)
    return records
}

/**
 * State the download manager keeps for the whole session (`providers/
 * download-manager.ts`): its startup snapshot of earlier jobs, creation times of
 * retried jobs and the jobs whose cancellation is still being processed.
 */
export function createDownloadsDomain(preview: PreviewState) {
    const initializationState = ref<'loading' | 'ready' | 'error'>('loading')
    /** `retry_job_as_new` starts a new job now; the shared queue copies the old start time. */
    const createdAt = ref(new Map<string, string>())
    /** Jobs between `download_job_cancel` and their `canceled` snapshot. */
    const canceling = ref(new Map<string, InstallJob>())

    seededHistory()
        .then((records) => {
            const known = new Set(preview.downloads.history.value.map((job) => job.id))
            preview.downloads.history.value = [
                ...preview.downloads.history.value,
                ...records.filter((job) => !known.has(job.id)),
            ]
            initializationState.value = 'ready'
        })
        .catch(() => {
            initializationState.value = 'ready'
        })

    return { value: { initializationState, createdAt, canceling } }
}

export type DownloadsDomain = ReturnType<typeof createDownloadsDomain>['value']

export function useDownloadsDomain(): DownloadsDomain {
    return usePreviewDomain('downloads', createDownloadsDomain)
}

/** Every job the page can show, newest first (`jobs.sort((a, b) => b.created.localeCompare(a.created))`). */
export function useDownloadJobs() {
    const preview = usePreview()
    const domain = useDownloadsDomain()
    const localJobs = useLocalJobs()

    const jobs = computed<DownloadJob[]>(() => {
        const deleted = (id: string | null) => Boolean(id) && !preview.instances.get(id!)
        const canceling = domain.canceling.value
        const content = [
            ...preview.downloads.jobs.value.filter((job) => !canceling.has(job.id)),
            ...[...canceling.values()],
            ...preview.downloads.history.value.filter((job) => !canceling.has(job.id)),
        ].map((job) =>
            contentJobView(job, {
                deleted: Boolean(job.finished_at) && deleted(job.instance_id),
                created: domain.createdAt.value.get(job.id),
                canceling: canceling.has(job.id),
            }),
        )
        const local = localJobs.jobs.value.map((job) =>
            localJobView(job, !isActive(job) && deleted(job.instance_id)),
        )
        return [...content, ...local].sort((a, b) => b.created.localeCompare(a.created))
    })

    const activeJobs = computed(() => jobs.value.filter((job) => isActive(job)))
    const historyJobs = computed(() => jobs.value.filter((job) => !isActive(job)))

    return { preview, domain, localJobs, jobs, activeJobs, historyJobs }
}

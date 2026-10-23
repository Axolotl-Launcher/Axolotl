import type { PreviewState } from '../../lib/state'
import type { InstallJob, InstallPhase } from '../../lib/state/downloads'

/**
 * Install jobs of the creation flow (`install_create_instance`,
 * `install_create_modpack_instance`, `install_import_instance`) on the preview's
 * download list. The shared queue only runs content downloads, so these jobs are
 * appended to `downloads.jobs` directly and advanced on the preview clock through
 * the same phases; the titlebar's download chip, its popout and the Downloads page
 * all read that list. A job removed from the list by the user (cancel) stops here.
 */
export interface InstallJobSpec {
    title: string
    icon: string | null
    /** `minecraft` for game installs and imports, `modpack` for pack installs. */
    projectType?: string
    instanceId: string
    instanceName: string
    versionName: string
    fileName: string
    /** Bytes downloaded during the `downloading` phase. */
    downloadBytes: number
    /** Extra milliseconds spent in `installing` (copying, extracting overrides). */
    installMs?: number
    onPhase?: (phase: InstallPhase) => void
}

export type InstallJobResult = 'completed' | 'canceled'

/** Files the launcher downloads for a game version it has not cached yet. */
export const GAME_FILES = { files: 42, bytes: 48 * 1024 * 1024 }

const RESOLVE_MS = 900
const MIN_DOWNLOAD_MS = 1600
const MAX_DOWNLOAD_MS = 5200
const INSTALL_MS = 1100

function randomId() {
    return `job-${Math.random().toString(36).slice(2, 10)}`
}

export function runInstallJob(
    preview: PreviewState,
    spec: InstallJobSpec,
): Promise<InstallJobResult> {
    const { downloads, clock } = preview
    const job: InstallJob = {
        id: randomId(),
        project_id: '',
        project_title: spec.title,
        project_icon: spec.icon,
        project_type: spec.projectType ?? 'minecraft',
        version_id: '',
        version_name: spec.versionName,
        file_name: spec.fileName,
        size: spec.downloadBytes,
        instance_id: spec.instanceId,
        instance_name: spec.instanceName,
        phase: 'resolving_project',
        progress: 0,
        speed: 0,
        bytes_done: 0,
        created_at: new Date().toISOString(),
        finished_at: null,
        error: null,
    }
    downloads.jobs.value = [...downloads.jobs.value, { ...job }]
    spec.onPhase?.(job.phase)

    // Download time follows the size, within the range the shared queue uses.
    const downloadMs = Math.min(
        MAX_DOWNLOAD_MS,
        Math.max(MIN_DOWNLOAD_MS, (spec.downloadBytes / (24 * 1024 * 1024)) * 1000),
    )
    const steps: { phase: InstallPhase; duration: number }[] = [
        { phase: 'resolving_project', duration: RESOLVE_MS },
        { phase: 'downloading', duration: spec.downloadBytes > 0 ? downloadMs : 0 },
        { phase: 'installing', duration: INSTALL_MS + (spec.installMs ?? 0) },
    ]

    return new Promise<InstallJobResult>((resolve) => {
        let step = 0
        let elapsed = 0

        function publish() {
            downloads.jobs.value = downloads.jobs.value.map((entry) =>
                entry.id === job.id ? { ...job } : entry,
            )
        }

        const stop = clock.every((delta) => {
            if (!downloads.jobs.value.some((entry) => entry.id === job.id)) {
                // Canceled from the action bar or the Downloads page.
                stop()
                resolve('canceled')
                return
            }
            elapsed += delta
            while (step < steps.length && elapsed >= steps[step].duration) {
                elapsed -= steps[step].duration
                step++
                if (step < steps.length) {
                    job.phase = steps[step].phase
                    if (job.phase === 'installing') {
                        job.progress = 1
                        job.bytes_done = job.size
                        job.speed = 0
                    }
                    spec.onPhase?.(job.phase)
                }
            }
            if (step >= steps.length) {
                stop()
                job.phase = 'completed'
                job.progress = 1
                job.bytes_done = job.size
                job.speed = 0
                job.finished_at = new Date().toISOString()
                downloads.jobs.value = downloads.jobs.value.filter((entry) => entry.id !== job.id)
                downloads.history.value = [{ ...job }, ...downloads.history.value]
                spec.onPhase?.('completed')
                resolve('completed')
                return
            }
            if (job.phase === 'downloading' && steps[step].duration > 0) {
                const share = Math.min(1, elapsed / steps[step].duration)
                job.progress = share
                job.bytes_done = Math.round(job.size * share)
                job.speed = Math.round(job.size / (steps[step].duration / 1000))
            }
            publish()
        })
    })
}

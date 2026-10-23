import type { DownloadManager } from '../app-helpers.ts'

/**
 * The install job snapshot the app's download manager shows (`helpers/install.ts`).
 * The preview creates no server download jobs (`injectDownloadManager` has no
 * provider there), so only the fields written here are typed.
 */
type InstallJobSnapshot = {
    provider: 'modrinth' | 'curse_forge' | 'minecraft' | 'java' | 'application' | 'local'
    status: string
    phase: string
} & Record<string, unknown>

export interface ServerDownloadJobDisplay {
    title: string
    icon: string | null
    provider: InstallJobSnapshot['provider']
}

export interface ServerDownloadProgress {
    downloaded: number
    total: number | null
}

/**
 * Drives a synthetic install job in the global Downloads page so that a
 * server download (modpack or vanilla) appears in the sidebar and history
 * exactly like a backend-tracked install job.
 *
 * A single shared implementation is used by every server-download flow so the
 * snapshot shape, status transitions, and cancel handling stay consistent.
 */
export interface ServerDownloadBridge {
    update(progress: ServerDownloadProgress, speed: number | null, eta: number | null): void
    complete(success: boolean, progress?: ServerDownloadProgress): void
    cancel(handler: () => void | Promise<void>): void
}

export function createServerDownloadBridge(
    downloadManager: DownloadManager,
    jobId: string,
    display: ServerDownloadJobDisplay,
): ServerDownloadBridge {
    const created = new Date().toISOString()

    function snapshot(
        status: InstallJobSnapshot['status'],
        phase: InstallJobSnapshot['phase'],
        progress: ServerDownloadProgress,
        speed: number | null,
        eta: number | null,
        finished?: string,
    ): InstallJobSnapshot {
        const modified = new Date().toISOString()
        return {
            job_id: jobId,
            instance_id: null,
            instance_deleted: false,
            kind: 'create_instance',
            status,
            execution_mode: 'normal',
            provider: display.provider,
            target: { type: 'new_instance' },
            phase,
            details: { type: 'empty' },
            created,
            modified,
            finished,
            display: { title: display.title, icon: display.icon },
            summary: {
                files_completed: 0,
                files_total: null,
                bytes_downloaded: progress.downloaded,
                bytes_total: progress.total,
                speed_bytes_per_second: speed,
                eta_seconds: eta,
                source: null,
                fallback_count: 0,
            },
            items: [],
        }
    }

    downloadManager.addSyntheticJob(
        snapshot('running', 'downloading_content', { downloaded: 0, total: null }, null, null),
    )

    return {
        update(progress, speed, eta) {
            downloadManager.setSyntheticJob(
                snapshot('running', 'downloading_content', progress, speed, eta),
            )
        },
        complete(success, progress) {
            downloadManager.offSyntheticCancel(jobId)
            const finalProgress = progress ?? { downloaded: 0, total: null }
            const finished = new Date().toISOString()
            downloadManager.setSyntheticJob(
                snapshot(
                    success ? 'succeeded' : 'failed',
                    success ? 'completed' : 'downloading_content',
                    finalProgress,
                    null,
                    null,
                    finished,
                ),
            )
        },
        cancel(handler) {
            downloadManager.onSyntheticCancel(jobId, async () => {
                downloadManager.offSyntheticCancel(jobId)
                await handler()
            })
        },
    }
}

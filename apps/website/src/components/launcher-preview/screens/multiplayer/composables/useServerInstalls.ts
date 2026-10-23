import type { Labrinth } from '@modrinth/api-client'
import { reactive } from 'vue'

import { get_version } from '../app-helpers.ts'
import {
    type InstallModpackOptions,
    serverEventListener,
    type ServerInfoData,
    servers,
} from '../backend.ts'
import { resolveServerLauncher } from '../servers/server-flow-utils.ts'
import { createServerDownloadBridge } from './server-download-bridge.ts'
import { refresh as refreshServerList } from './useServers.ts'

const LOG_CAPACITY = 500

export interface ActiveServerInstall {
    progress: { downloaded: number; total: number | null } | null
    log: string[]
}

/**
 * Why a server's start controls are replaced by download actions:
 * - `installing`: files are transferring right now (this app session)
 * - `interrupted`: the manifest was flagged mid-install and never finished
 *   (app closed while downloading); resumable
 * - `failed`: the last install attempt errored; retryable
 */
export type ServerSetupStatus = 'installing' | 'interrupted' | 'failed'

/** Module-level singleton: install activity is global to the app. */
const activeInstalls = reactive<Record<string, ActiveServerInstall | undefined>>({})

export function activeInstallFor(serverId: string): ActiveServerInstall | null {
    return activeInstalls[serverId] ?? null
}

export function serverSetupStatus(server: ServerInfoData): ServerSetupStatus | null {
    if (activeInstalls[server.id]) return 'installing'
    if (server.installState === 'incomplete') return 'interrupted'
    if (server.installState === 'failed') return 'failed'
    return null
}

/**
 * [SERVER-DOWNLOAD-BRIDGE] Runs a modpack server install in the background.
 *
 * During the install, progress and log events are tracked in the shared
 * `activeInstalls` registry so any surface (wizard, servers list, detail
 * page) can render live state.
 *
 * Additionally, when a `DownloadManager` reference is provided, a synthetic
 * install job is injected so the server download also appears in the global
 * Downloads page (/downloads) and the sidebar download badge.
 *
 * NOTE: The download manager MUST be captured during Vue setup context and
 * passed in — `injectDownloadManager()` only works inside synchronous setup
 * scope.  After any `await` the injection context is lost and the call
 * would throw.
 */
export async function startModpackServerInstall(
    serverId: string,
    options: InstallModpackOptions,
    downloadManager?: import('@/providers/download-manager').DownloadManager | null,
): Promise<void> {
    if (activeInstalls[serverId]) {
        throw new Error('This server already has an install running')
    }
    const entry: ActiveServerInstall = { progress: null, log: [] }
    activeInstalls[serverId] = entry

    // [SERVER-DOWNLOAD-BRIDGE] Create a synthetic job that mirrors this server
    // install in the global Downloads page.  The job_id uses a `server-` prefix
    // to avoid collisions with backend-tracked install jobs.  The shared bridge
    // centralises the snapshot shape used by every server download (modpack and
    // vanilla) so they stay consistent in the sidebar.
    const syntheticJobId = `server-${serverId}`
    const bridge = downloadManager
        ? createServerDownloadBridge(downloadManager, syntheticJobId, {
              title: options.modpackTitle ?? 'Server',
              icon: options.modpackIconUrl ?? null,
              provider: options.modpackProjectId ? 'modrinth' : 'local',
          })
        : null
    if (!bridge) {
        console.warn(
            `[SERVER-DOWNLOAD-BRIDGE] No download manager available — job ${syntheticJobId} will NOT appear in sidebar`,
        )
    }

    // [SERVER-DOWNLOAD-BRIDGE] Listen for backend progress events and forward
    // them to both the local `entry` (used by wizard UI) and the synthetic
    // job (used by the global Downloads page).
    let prevBytes = 0
    let prevTime = performance.now()
    let smoothedSpeed = 0
    const unlistenProgress = await serverEventListener((id, payload) => {
        if (id !== serverId || payload.event !== 'download_progress') return
        entry.progress = { downloaded: payload.downloaded, total: payload.total ?? null }

        const now2 = performance.now()
        const dt = (now2 - prevTime) / 1000
        if (payload.downloaded < prevBytes) {
            smoothedSpeed = 0
            prevBytes = payload.downloaded
            prevTime = now2
        } else if (dt >= 0.3) {
            const rawSpeed = (payload.downloaded - prevBytes) / dt
            smoothedSpeed = smoothedSpeed === 0 ? rawSpeed : smoothedSpeed * 0.7 + rawSpeed * 0.3
            prevBytes = payload.downloaded
            prevTime = now2
        }

        const speed = smoothedSpeed > 0 ? smoothedSpeed : null
        const total = payload.total ?? null
        const eta =
            speed && total != null && total > payload.downloaded
                ? (total - payload.downloaded) / speed
                : null

        bridge?.update({ downloaded: payload.downloaded, total }, speed, eta)
    })
    const unlistenLogs = await serverEventListener((id, payload) => {
        if (id !== serverId || payload.event !== 'log') return
        entry.log.push(payload.line)
        if (entry.log.length > LOG_CAPACITY) entry.log.splice(0, entry.log.length - LOG_CAPACITY)
    })

    // [SERVER-DOWNLOAD-BRIDGE] Register a cancel handler so the Downloads page
    // can abort the running server install when the user clicks Cancel.
    bridge?.cancel(async () => {
        console.log(`[SERVER-DOWNLOAD-BRIDGE] Cancelling server install ${serverId}`)
        await servers.stop(serverId).catch(() => {})
    })

    let installSucceeded = false
    try {
        await servers.installModpack(serverId, options)
        installSucceeded = true
    } finally {
        unlistenProgress()
        unlistenLogs()
        // [SERVER-DOWNLOAD-BRIDGE] Mark the synthetic job as succeeded or failed
        // so it transitions out of the active tab and into history.
        bridge?.complete(installSucceeded, entry.progress ?? undefined)
        activeInstalls[serverId] = undefined
        void refreshServerList()
    }
}

/**
 * [SERVER-DOWNLOAD-BRIDGE] Resumes an interrupted or failed modpack install
 * for a server created from a modpack: re-resolves the pack file and launcher
 * jar from the recorded source project, then reruns the same background install.
 *
 * The optional `downloadManager` parameter is forwarded to
 * `startModpackServerInstall` so the synthetic job appears in sidebar.
 * See the note in `startModpackServerInstall` for why this must be passed
 * explicitly.
 */
export async function resumeModpackInstall(
    server: ServerInfoData,
    downloadManager?: import('@/providers/download-manager').DownloadManager | null,
): Promise<void> {
    const modpack = server.modpack
    if (!modpack?.versionId) {
        throw new Error('This server has no modpack source to resume from')
    }
    const version = (await get_version(modpack.versionId)) as Labrinth.Versions.v2.Version
    const primaryFile = version.files.find((file) => file.primary) ?? version.files[0]
    if (!primaryFile?.url) {
        throw new Error('Modpack has no downloadable file')
    }
    const jar =
        server.serverType === 'neoforge'
            ? null
            : await resolveServerLauncher(
                  server.serverType,
                  server.gameVersion,
                  server.loaderVersion,
              )
    if (!jar && server.serverType !== 'neoforge') {
        throw new Error(
            `No server launcher available for ${server.serverType} on ${server.gameVersion}`,
        )
    }
    await startModpackServerInstall(
        server.id,
        {
            mrpackUrl: primaryFile.url,
            mrpackSha1: primaryFile.hashes?.sha1,
            jarUrl: jar?.url,
            jarFilename: jar?.filename,
            jarSha1: jar?.sha1,
            modpackProjectId: modpack.projectId,
            modpackVersionId: modpack.versionId,
            modpackTitle: modpack.title,
            modpackIconUrl: modpack.iconUrl,
        },
        downloadManager,
    )
}

import {
    forgePromotionsSlimUrl,
    SERVER_TYPES,
    type ServerTypeId,
} from '../../../lib/server-core.ts'
import type { DownloadManager } from '../app-helpers.ts'
import { serverEventListener, servers } from '../backend.ts'
import { fetchJson, resolveServerLauncher } from '../servers/server-flow-utils.ts'
import { createServerDownloadBridge, type ServerDownloadBridge } from './server-download-bridge.ts'
import { refresh } from './useServers.ts'

export interface ServerInstallInputs {
    gameVersion: string
    loaderVersion?: string
    javaPath?: string
    memoryMb?: number
}

/**
 * A server-type install strategy encapsulates the "how" of materializing a
 * server's launchable files. The shared orchestrator (`runServerInstall`) owns
 * the cross-cutting concerns — the sidebar download job, progress/log event
 * forwarding, and cancellation — so every strategy only drives the backend for
 * its specific loader. This is what lets vanilla, Fabric, Paper, Forge and
 * (via `ModpackServerInstallStrategy`) modpack servers share one flow.
 */
export interface ServerInstallStrategy {
    readonly id: ServerTypeId | 'modpack'
    install(serverId: string, inputs: ServerInstallInputs): Promise<void>
}

/** `direct` install-mode types: a single launcher jar is downloaded and booted. */
export class JarServerInstallStrategy implements ServerInstallStrategy {
    readonly id: ServerTypeId
    constructor(private readonly type: ServerTypeId) {
        this.id = type
    }
    async install(serverId: string, inputs: ServerInstallInputs): Promise<void> {
        const jar = await resolveServerLauncher(this.type, inputs.gameVersion, inputs.loaderVersion)
        if (!jar) {
            throw new Error(
                `No server launcher available for ${this.type} on ${inputs.gameVersion}`,
            )
        }
        await servers.downloadFile(serverId, jar.url, jar.filename, jar.sha1)
    }
}

/** Resolves the recommended (or, failing that, latest) Forge build for a game version. */
export async function resolveForgeBuild(gameVersion: string): Promise<string> {
    const promos = await fetchJson<{ promos: Record<string, string> }>(forgePromotionsSlimUrl())
    const build =
        promos.promos[`${gameVersion}-recommended`] ?? promos.promos[`${gameVersion}-latest`]
    if (!build) throw new Error(`No Forge build available for Minecraft ${gameVersion}`)
    return build
}

/** `installer` install-mode Forge: download the installer and run it headlessly. */
export class ForgeServerInstallStrategy implements ServerInstallStrategy {
    readonly id = 'forge' as const
    async install(serverId: string, inputs: ServerInstallInputs): Promise<void> {
        const build = await resolveForgeBuild(inputs.gameVersion)
        await servers.installForge(serverId, inputs.gameVersion, build, inputs.javaPath)
    }
}

export class NeoForgeServerInstallStrategy implements ServerInstallStrategy {
    readonly id = 'neoforge' as const
    async install(serverId: string, inputs: ServerInstallInputs): Promise<void> {
        if (!inputs.loaderVersion) throw new Error('Select a NeoForge version first')
        await servers.installNeoForge(
            serverId,
            inputs.gameVersion,
            inputs.loaderVersion,
            inputs.javaPath,
        )
    }
}

/** Picks the install strategy for a server type from its declared install mode. */
export function getServerInstallStrategy(type: ServerTypeId): ServerInstallStrategy {
    const def = SERVER_TYPES[type]
    if (def.installMode === 'installer') {
        if (type === 'forge') return new ForgeServerInstallStrategy()
        if (type === 'neoforge') return new NeoForgeServerInstallStrategy()
        throw new Error(`Server type '${type}' installer is not supported yet`)
    }
    return new JarServerInstallStrategy(type)
}

export interface RunServerInstallOptions {
    serverId: string
    name: string
    inputs: ServerInstallInputs
    strategy: ServerInstallStrategy
    downloadManager: DownloadManager | null
    onProgress?: (progress: { downloaded: number; total: number | null }) => void
    onLog?: (line: string) => void
}

/**
 * Runs a server install end to end: registers a sidebar download job, forwards
 * backend progress/log events to the caller, and invokes the strategy. It does
 * not write `eula.txt` or perform cleanup — those depend on flow state and stay
 * with the caller.
 */
export async function runServerInstall(options: RunServerInstallOptions): Promise<void> {
    const { serverId, name, inputs, strategy, downloadManager, onProgress, onLog } = options

    const bridge: ServerDownloadBridge | null = downloadManager
        ? createServerDownloadBridge(downloadManager, `server-${serverId}`, {
              title: name,
              icon: null,
              provider: 'minecraft',
          })
        : null
    bridge?.cancel(async () => {
        await servers.stop(serverId).catch(() => {})
    })

    const unlisten = await serverEventListener((eventServerId, payload) => {
        if (eventServerId !== serverId) return
        if (payload.event === 'download_progress') {
            const progress = { downloaded: payload.downloaded, total: payload.total ?? null }
            onProgress?.(progress)
            bridge?.update(progress, null, null)
        } else if (payload.event === 'log') {
            onLog?.(payload.line)
        }
    })

    try {
        await strategy.install(serverId, inputs)
        bridge?.complete(true)
        // The server was created up front, but the shared list only refreshes on
        // demand. When the modal was closed mid-install the `@created` event
        // never fires, so refresh here (context-free) to render the new server
        // as soon as the install finishes — regardless of modal state.
        await refresh().catch(() => {})
    } catch (error) {
        bridge?.complete(false)
        throw error
    } finally {
        unlisten()
    }
}

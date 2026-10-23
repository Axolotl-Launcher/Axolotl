import {
    type FabricInstallerVersionsResponse,
    fabricInstallerVersionsUrl,
    FORGE_MAVEN_URL,
    forgePromotionsSlimUrl,
    latestStablePaperBuild,
    neoForgeInstaller,
    type PaperBuildsResponse,
    paperBuildsUrl,
    quiltInstallerVersionsUrl,
    resolveServerJar,
    type ServerJarDownload,
    type ServerTypeId,
    type VanillaVersionInfo,
} from '../../../lib/server-core.ts'
import { desktopOnlyError, get_game_versions } from '../app-helpers.ts'
import { serverEventListener, type ServerEventPayload } from '../backend.ts'

/** Best-effort conversion of an unknown error into a user-presentable string. */
export function toErrorMessage(error: unknown): string {
    if (error instanceof Error) return error.message
    if (typeof error === 'string') return error
    if (error && typeof error === 'object') {
        const record = error as Record<string, unknown>
        for (const key of ['message', 'error', 'description'] as const) {
            if (typeof record[key] === 'string') return record[key]
        }
    }
    try {
        return JSON.stringify(error)
    } catch {
        return String(error)
    }
}

/** Extracts the Java major version from strings like `17`, `1.8`, or `21.0.1`. */
export function javaMajorFromVersion(version: string): number | null {
    const parts = version
        .split(/[._]/)
        .map(Number)
        .filter((value) => Number.isInteger(value) && value >= 0)
    if (parts.length === 0) return null
    if (parts[0] === 1 && parts.length > 1) return parts[1]
    return parts[0]
}

/**
 * Waits for the server to emit a `stopped` event, resolving with the payload
 * or `null` after a timeout. Used to run the first start during setup and know
 * when the JVM has exited.
 */
export async function waitForServerStop(serverId: string): Promise<ServerEventPayload | null> {
    return new Promise((resolve) => {
        void serverEventListener((eventServerId, payload) => {
            if (eventServerId !== serverId || payload.event !== 'stopped') return
            resolve(payload)
        }).then((unlisten) => {
            setTimeout(
                () => {
                    unlisten()
                    resolve(null)
                },
                10 * 60 * 1000,
            )
        })
    })
}

/**
 * The app reads server metadata from Mojang, Fabric, Quilt, PaperMC and Forge
 * as it downloads server files. The preview makes no requests to those
 * services, so a server download stops here with the desktop app's notice.
 */
export async function fetchJson<T>(_url: string): Promise<T> {
    throw desktopOnlyError('server-download')
}

/**
 * Resolves the server launcher jar download for a modpack server. Vanilla
 * pulls the Mojang server jar; Fabric and Quilt use their meta service launcher
 * jars with the newest stable installer.
 */
export async function resolveServerLauncher(
    type: ServerTypeId,
    gameVersion: string,
    loaderVersion?: string,
): Promise<ServerJarDownload | null> {
    switch (type) {
        case 'vanilla': {
            const manifest = (await get_game_versions()) as {
                versions: { id: string; url: string }[]
            }
            const entry = manifest.versions.find((v) => v.id === gameVersion)
            if (!entry) return null
            const versionInfo = await fetchJson<VanillaVersionInfo>(entry.url)
            return resolveServerJar('vanilla', { gameVersion, vanillaVersionInfo: versionInfo })
        }
        case 'fabric':
        case 'quilt': {
            const installers = await fetchJson<FabricInstallerVersionsResponse[]>(
                type === 'fabric' ? fabricInstallerVersionsUrl() : quiltInstallerVersionsUrl(),
            )
            const installerVersion = installers[0]?.version
            return resolveServerJar(type, { gameVersion, loaderVersion, installerVersion })
        }
        case 'paper': {
            const builds = await fetchJson<PaperBuildsResponse>(paperBuildsUrl(gameVersion))
            const build = latestStablePaperBuild(builds)
            if (!build) return null
            return resolveServerJar(type, { gameVersion, paperBuild: build })
        }
        case 'forge': {
            // The Forge "launcher" is the installer jar; the backend runs it
            // headlessly (`--installServer`) to materialize the server files.
            const promos = await fetchJson<{ promos: Record<string, string> }>(
                forgePromotionsSlimUrl(),
            )
            const build =
                promos.promos[`${gameVersion}-recommended`] ??
                promos.promos[`${gameVersion}-latest`]
            if (!build) return null
            const filename = `forge-${gameVersion}-${build}-installer.jar`
            return {
                url: `${FORGE_MAVEN_URL}/${gameVersion}-${build}/${filename}`,
                filename,
                sha1: undefined,
            }
        }
        case 'neoforge': {
            return loaderVersion ? neoForgeInstaller(gameVersion, loaderVersion) : null
        }
        default:
            return null
    }
}

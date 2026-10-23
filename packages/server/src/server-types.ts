import type {
    PaperBuild,
    ResolveServerJarInput,
    ServerJarDownload,
    ServerTypeDefinition,
    ServerTypeId,
} from './types.ts'

const FABRIC_META_URL = 'https://meta.fabricmc.net/v2'
const QUILT_META_URL = 'https://meta.quiltmc.org/v3'
const PAPER_API_URL = 'https://fill.papermc.io/v3'
const PAPER_PROJECT = 'paper'

/** Server capabilities shared by creation flows and download resolution. */
export const SERVER_TYPES: Record<ServerTypeId, ServerTypeDefinition> = {
    vanilla: {
        id: 'vanilla',
        label: 'Vanilla',
        installMode: 'direct',
        needsLoaderVersion: false,
        implemented: true,
    },
    fabric: {
        id: 'fabric',
        label: 'Fabric',
        installMode: 'direct',
        needsLoaderVersion: true,
        implemented: true,
    },
    paper: {
        id: 'paper',
        label: 'Paper',
        installMode: 'direct',
        needsLoaderVersion: false,
        implemented: true,
    },
    forge: {
        id: 'forge',
        label: 'Forge',
        installMode: 'installer',
        needsLoaderVersion: false,
        implemented: true,
    },
    neoforge: {
        id: 'neoforge',
        label: 'NeoForge',
        installMode: 'installer',
        needsLoaderVersion: true,
        implemented: true,
    },
    quilt: {
        id: 'quilt',
        label: 'Quilt',
        installMode: 'installer',
        needsLoaderVersion: true,
        implemented: false,
    },
}

export function listServerTypes(): ServerTypeDefinition[] {
    return Object.values(SERVER_TYPES)
}

export function isServerTypeSupported(type: ServerTypeId): boolean {
    return SERVER_TYPES[type].implemented
}

/** Base URL of the Forge Maven repository hosting installer and launcher artifacts. */
export const FORGE_MAVEN_URL = 'https://maven.minecraftforge.net/net/minecraftforge/forge'

/** NeoForge 1.20.1 used the `forge` artifact before switching to `neoforge`. */
export function neoForgeInstaller(gameVersion: string, loaderVersion: string): ServerJarDownload {
    const artifact = gameVersion === '1.20.1' ? 'forge' : 'neoforge'
    const version = gameVersion === '1.20.1' ? `${gameVersion}-${loaderVersion}` : loaderVersion
    const filename = `${artifact}-${version}-installer.jar`
    return {
        url: `https://maven.neoforged.net/releases/net/neoforged/${artifact}/${version}/${filename}`,
        filename,
    }
}

/** Base URL of the Forge web host that publishes the promotions manifest. */
export const FORGE_FILES_URL = 'https://files.minecraftforge.net/net/minecraftforge/forge'

/** URL of the Forge promotions manifest, mapping `<mc>-recommended`/`latest` to a build. */
export function forgePromotionsSlimUrl(): string {
    return `${FORGE_FILES_URL}/promotions_slim.json`
}

/** URL of the Fabric server launcher jar for a specific game/loader/installer combination. */
export function fabricServerJarUrl(
    gameVersion: string,
    loaderVersion: string,
    installerVersion: string,
): string {
    return `${FABRIC_META_URL}/versions/loader/${gameVersion}/${loaderVersion}/${installerVersion}/server/jar`
}

export function fabricInstallerVersionsUrl(): string {
    return `${FABRIC_META_URL}/versions/installer`
}

export function fabricLoaderVersionsForGameUrl(gameVersion: string): string {
    return `${FABRIC_META_URL}/versions/loader/${gameVersion}`
}

/** URL of the Quilt server launcher jar for a specific game/loader/installer combination. */
export function quiltServerJarUrl(
    gameVersion: string,
    loaderVersion: string,
    installerVersion: string,
): string {
    return `${QUILT_META_URL}/versions/loader/${gameVersion}/${loaderVersion}/${installerVersion}/server/jar`
}

export function quiltInstallerVersionsUrl(): string {
    return `${QUILT_META_URL}/versions/installer`
}

export function quiltLoaderVersionsForGameUrl(gameVersion: string): string {
    return `${QUILT_META_URL}/versions/loader/${gameVersion}`
}

export function paperBuildsUrl(gameVersion: string): string {
    return `${PAPER_API_URL}/projects/${PAPER_PROJECT}/versions/${gameVersion}/builds`
}

/**
 * Resolves the server jar download for a server type from metadata the caller
 * fetched. Returns null when the type needs an installer step or required
 * metadata is missing.
 */
export function resolveServerJar(
    type: ServerTypeId,
    input: ResolveServerJarInput,
): ServerJarDownload | null {
    switch (type) {
        case 'vanilla': {
            const server = input.vanillaVersionInfo?.downloads.server
            if (!server) return null
            return { url: server.url, filename: 'server.jar', sha1: server.sha1, size: server.size }
        }
        case 'fabric': {
            if (!input.loaderVersion || !input.installerVersion) return null
            return {
                url: fabricServerJarUrl(
                    input.gameVersion,
                    input.loaderVersion,
                    input.installerVersion,
                ),
                filename: 'fabric-server.jar',
            }
        }
        case 'quilt': {
            if (!input.loaderVersion || !input.installerVersion) return null
            return {
                url: quiltServerJarUrl(
                    input.gameVersion,
                    input.loaderVersion,
                    input.installerVersion,
                ),
                filename: 'quilt-server.jar',
            }
        }
        case 'paper': {
            const download = input.paperBuild?.downloads['server:default']
            if (!download) return null
            return { url: download.url, filename: 'server.jar' }
        }
        default:
            return null
    }
}

export type PaperBuildsResponse = PaperBuild[]

/** The newest stable build from a Fill v3 builds response (builds are newest first). */
export function latestStablePaperBuild(response: PaperBuildsResponse): PaperBuild | null {
    return response?.find((build) => build.channel === 'STABLE') ?? null
}

export interface FabricInstallerVersionsResponse {
    version: string
    stable: boolean
}

/** The newest installer version from the `/v2/versions/installer` response (a top-level array). */
export function pickFabricInstallerVersion(
    response: FabricInstallerVersionsResponse[],
): string | null {
    return response?.[0]?.version ?? null
}

/** Minimum Java major version required to run a given game version. */
/**
 * Minimum Java major version required to run a given game version.
 * Handles both the legacy `1.x` scheme and the year-based scheme (`26.2`,
 * `26w14a`), which needs Java 25.
 */
export function requiredJavaMajorVersion(gameVersion: string): number {
    const yearSnapshot = /^(\d{2})w/.exec(gameVersion)
    if (yearSnapshot) {
        return Number(yearSnapshot[1]) >= 26 ? 25 : 21
    }

    const match = /^(\d+)(?:\.(\d+))?(?:\.(\d+))?/.exec(gameVersion)
    if (!match) return 25
    const major = Number(match[1])
    const minor = Number(match[2] ?? 0)
    const patch = Number(match[3] ?? 0)

    // Year-based releases (26.1+) require Java 25
    if (major >= 21) return 25
    // Legacy 1.x releases
    if (major === 1) {
        if (minor > 20 || (minor === 20 && patch >= 5)) return 21
        if (minor >= 17) return 17
        return 8
    }
    return 25
}

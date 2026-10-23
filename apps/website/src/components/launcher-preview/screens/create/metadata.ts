/**
 * Launcher metadata for the creation flow, read from the website's snapshot
 * (`LP/data`) the way the app reads its tag cache and launcher-meta manifests.
 *
 * - Game versions: the app lists `tags.gameVersions`, Modrinth's
 *   `/v2/tag/game_version` list (newest first, `version_type` release / snapshot /
 *   beta / alpha), which is `loadTags()`. Versions of the Mojang manifest snapshot
 *   (`loadMinecraft()`) that the tag list does not carry yet are merged in by date.
 * - Loader manifests: `loaders.json` keeps, per loader and game version, the
 *   launcher-meta builds and which of them are stable. A game version listed
 *   without builds would be kept as supported with its builds unresolved
 *   (`unresolvedBuilds`); the install then resolves the selected channel the way
 *   `install_create_instance` does for `stable` / `latest`.
 */

export interface GameVersionTag {
    version: string
    version_type: string
    date: string
}

export interface LoaderVersionEntry {
    id: string
    stable: boolean
}

export interface LoaderManifestGameVersion {
    id: string
    stable: boolean
    loaders: LoaderVersionEntry[]
    /** Listed by the manifest, but the snapshot recorded no builds for it. */
    unresolvedBuilds: boolean
}

export interface LoaderManifest {
    gameVersions: LoaderManifestGameVersion[]
}

/** Loaders the app's instance flow offers (`clientInstallableLoaders`). */
export const clientInstallableLoaders = [
    'fabric',
    'neoforge',
    'forge',
    'quilt',
    'optifine',
    'cleanroom',
    'lite_loader',
    'legacy_fabric',
    'babric',
] as const

export function toApiLoaderName(loader: string): string {
    return loader === 'neoforge' ? 'neo' : loader
}

let gameVersionsPromise: Promise<GameVersionTag[]> | null = null

export function loadGameVersions(): Promise<GameVersionTag[]> {
    if (!gameVersionsPromise) {
        gameVersionsPromise = import('../../data').then(async (data) => {
            const [tags, minecraft] = await Promise.all([data.loadTags(), data.loadMinecraft()])
            const byId = new Map<string, GameVersionTag>()
            for (const tag of tags.gameVersions) {
                byId.set(tag.version, {
                    version: tag.version,
                    version_type: tag.version_type,
                    date: tag.date,
                })
            }
            for (const release of minecraft.releases) {
                if (!byId.has(release.id)) {
                    byId.set(release.id, {
                        version: release.id,
                        version_type: 'release',
                        date: release.releaseTime,
                    })
                }
            }
            for (const snapshot of minecraft.snapshots) {
                if (!byId.has(snapshot.id)) {
                    byId.set(snapshot.id, {
                        version: snapshot.id,
                        version_type: 'snapshot',
                        date: snapshot.releaseTime,
                    })
                }
            }
            for (const old of minecraft.old ?? []) {
                if (!byId.has(old.id)) {
                    byId.set(old.id, {
                        version: old.id,
                        version_type: old.type,
                        date: old.releaseTime,
                    })
                }
            }
            return [...byId.values()].sort((a, b) => Date.parse(b.date) - Date.parse(a.date))
        })
    }
    return gameVersionsPromise
}

let manifestsPromise: Promise<Record<string, LoaderManifest>> | null = null

/**
 * Manifests keyed by the launcher-meta loader name (`neo` for NeoForge). Each game
 * version keeps its builds newest first and marks the builds the manifest flags as
 * stable (`stableVersions`).
 */
export function loadLoaderManifests(): Promise<Record<string, LoaderManifest>> {
    if (!manifestsPromise) {
        manifestsPromise = import('../../data').then(async (data) => {
            const raw = await data.loadLoaders()
            const manifests: Record<string, LoaderManifest> = {}
            for (const [loader, games] of Object.entries(raw)) {
                manifests[loader] = {
                    gameVersions: Object.entries(games).map(([id, entry]) => {
                        const stable = new Set(entry.stableVersions ?? [])
                        const loaders = (entry.versions ?? []).map((build) => ({
                            id: build,
                            stable: stable.has(build),
                        }))
                        return {
                            id,
                            stable: entry.stable,
                            loaders,
                            unresolvedBuilds: loaders.length === 0,
                        }
                    }),
                }
            }
            return manifests
        })
    }
    return manifestsPromise
}

/** `loaderVersionsForGameVersion` of `loader-metadata.ts`. */
export function loaderVersionsForGameVersion(
    manifest: LoaderManifest | undefined,
    gameVersion: string,
): LoaderVersionEntry[] {
    return manifest?.gameVersions.find((entry) => entry.id === gameVersion)?.loaders ?? []
}

export type LoaderMetadataStatus = 'unknown' | 'loading' | 'success' | 'error'
export type LoaderSupportState = 'unknown' | 'loading' | 'supported' | 'unsupported' | 'error'

/** `loaderSupportState`: a listed game version is supported. */
export function loaderSupportState(
    status: LoaderMetadataStatus,
    manifest: LoaderManifest | undefined,
    gameVersion: string,
): LoaderSupportState {
    if (status !== 'success') return status
    const entry = manifest?.gameVersions.find((item) => item.id === gameVersion)
    if (!entry) return 'unsupported'
    return entry.loaders.length > 0 || entry.unresolvedBuilds ? 'supported' : 'unsupported'
}

/** Whether the manifest lists `gameVersion` without recorded builds. */
export function hasUnresolvedBuilds(manifest: LoaderManifest | undefined, gameVersion: string) {
    return manifest?.gameVersions.find((item) => item.id === gameVersion)?.unresolvedBuilds ?? false
}

export function loaderMetadataCacheKey(loader: string, gameVersion?: string): string {
    return gameVersion ? `${loader}:${gameVersion}` : loader
}

/** Same set as `creation-flow-modal/version-types.ts`. */
export const aprilFoolsVersions = new Set([
    '15w14a',
    '1.RV-Pre1',
    '3D Shareware v1.34',
    '20w14∞',
    '22w13oneblockatatime',
    '23w13a_or_b',
    '24w14potato',
    '25w14craftmine',
    '26w14a',
])

export type GameVersionType = 'release' | 'snapshot' | 'alpha' | 'ancient'

export function isVersionTypeMatch(
    versionType: string,
    versionId: string,
    selectedType: GameVersionType,
): boolean {
    switch (selectedType) {
        case 'release':
            return versionType === 'release'
        case 'snapshot':
            return versionType === 'snapshot' && !aprilFoolsVersions.has(versionId)
        case 'alpha':
            return aprilFoolsVersions.has(versionId)
        case 'ancient':
            return (
                !aprilFoolsVersions.has(versionId) &&
                (versionType === 'alpha' ||
                    versionType === 'beta' ||
                    versionType.startsWith('old_'))
            )
    }
}

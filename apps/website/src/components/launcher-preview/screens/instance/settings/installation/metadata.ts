import { reactive } from 'vue'

/*
 * The launcher metadata the Installation tab queries, read from the website's
 * snapshot (`LP/data`): `get_game_versions()` is Modrinth's game version tag list
 * and `get_loader_versions(loader, gameVersion)` the launcher-meta loader
 * manifest (the newest builds per game version the snapshot recorded). Loaders
 * the snapshot has no manifest for resolve to an empty manifest.
 */

export interface GameVersionTag {
    version: string
    version_type: string
    date: string
    major?: boolean
}

export interface LoaderMetadataVersion {
    id: string
    stable: boolean
}

export interface LoaderMetadataManifest {
    gameVersions: { id: string; loaders: LoaderMetadataVersion[] }[]
}

export type LoaderMetadataStatus = 'unknown' | 'loading' | 'success' | 'error'

let gameVersionsPromise: Promise<GameVersionTag[]> | null = null

export function getGameVersions(): Promise<GameVersionTag[]> {
    gameVersionsPromise ??= import('../../../../data').then(async (data) => {
        const tags = await data.loadTags()
        return tags.gameVersions as GameVersionTag[]
    })
    return gameVersionsPromise
}

type RawManifests = Record<
    string,
    Record<string, { stable: boolean; versions: string[]; stableVersions?: string[] }>
>

let manifestsPromise: Promise<RawManifests> | null = null

export function getLoaderVersions(
    loader: string,
    gameVersion: string,
): Promise<LoaderMetadataManifest> {
    manifestsPromise ??= import('../../../../data').then(
        async (data) => (await data.loadLoaders()) as unknown as RawManifests,
    )
    return manifestsPromise.then((manifests) => {
        const entry = manifests[loader]?.[gameVersion]
        if (!entry) return { gameVersions: [] }
        const stable = new Set(entry.stableVersions ?? [])
        return {
            gameVersions: [
                {
                    id: gameVersion,
                    loaders: entry.versions.map((id) => ({ id, stable: stable.has(id) })),
                },
            ],
        }
    })
}

/**
 * A small query cache standing in for `useQuery`: results are kept per key with
 * the pending state the template reads.
 */
interface QueryEntry<T> {
    status: 'pending' | 'success' | 'error'
    data: T | undefined
}

const loaderQueries = reactive<Record<string, QueryEntry<LoaderMetadataManifest>>>({})

function queryKey(loader: string, gameVersion: string) {
    return `${loader}:${gameVersion}`
}

/** Starts loading the manifest of `loader` for `gameVersion` unless it is cached. */
export function fetchLoaderVersions(loader: string, gameVersion: string) {
    const key = queryKey(loader, gameVersion)
    if (loaderQueries[key]) return
    loaderQueries[key] = { status: 'pending', data: undefined }
    getLoaderVersions(loader, gameVersion).then(
        (manifest) => {
            loaderQueries[key] = { status: 'success', data: manifest }
        },
        () => {
            loaderQueries[key] = { status: 'error', data: undefined }
        },
    )
}

export function loaderVersionsQuery(
    loader: string,
    gameVersion: string,
): QueryEntry<LoaderMetadataManifest> | undefined {
    return loaderQueries[queryKey(loader, gameVersion)]
}

/** `loaderVersionsForGameVersion` of `creation-flow-modal/loader-metadata.ts`. */
export function loaderVersionsForGameVersion(
    manifest: LoaderMetadataManifest | undefined,
    gameVersion: string,
): LoaderMetadataVersion[] {
    if (!manifest) return []
    return manifest.gameVersions.find((version) => version.id === gameVersion)?.loaders ?? []
}

export type LoaderSupportState = 'unknown' | 'loading' | 'supported' | 'unsupported' | 'error'

/** `loaderSupportState` of `creation-flow-modal/loader-metadata.ts`. */
export function loaderSupportState(
    status: LoaderMetadataStatus,
    manifest: LoaderMetadataManifest | undefined,
    gameVersion: string,
): LoaderSupportState {
    if (status !== 'success') return status
    return loaderVersionsForGameVersion(manifest, gameVersion).length > 0
        ? 'supported'
        : 'unsupported'
}

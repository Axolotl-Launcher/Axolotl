import { createContext } from './create-context.ts'

export interface ImportableLauncher {
    name: string
    path: string
    instances: { name: string; path: string }[]
    launcherType?: string
}

export type ImportPlanStage = 'resolving' | 'scanning' | 'done' | 'error'

export interface ImportPlanCounts {
    files: number
    bytes: number
}

export interface ImportPlanSnapshot {
    requestId: string
    stage: ImportPlanStage
    gameVersion: string | null
    loader: string | null
    loaderVersion: string | null
    importPath: string
    minecraftRoot: string
    modCount: number
    cache: ImportPlanCounts
    local: ImportPlanCounts
    network: ImportPlanCounts
    migrate: ImportPlanCounts
    error?: string | null
}

export interface ImportPlanRequest {
    requestId: string
    launcherType: string
    basePath: string
    instanceFolder: string
    instancePath?: string | null
    gameVersion?: string | null
    loader?: string | null
    loaderVersion?: string | null
}

export const KNOWN_IMPORT_PLAN_LOADERS = [
    'fabric',
    'forge',
    'neoforge',
    'quilt',
    'cleanroom',
] as const

export function importPlanDefaultGameVersion(detected: string | null | undefined) {
    return detected ?? ''
}

export function importPlanDefaultLoader(detected: string | null | undefined) {
    if (detected && (KNOWN_IMPORT_PLAN_LOADERS as readonly string[]).includes(detected)) {
        return detected
    }
    return 'vanilla'
}

export function importPlanDefaultLoaderVersion(detected: string | null | undefined) {
    return detected?.trim() ? detected : 'latest'
}

export function isImportPlanLoaderRecognized(loader: string | null | undefined) {
    return !!loader && (KNOWN_IMPORT_PLAN_LOADERS as readonly string[]).includes(loader)
}

export interface ImportPlanWarningsInput {
    gameVersion?: string | null
    loader?: string | null
    loaderVersion?: string | null
    detectedGameVersion?: string | null
    detectedLoader?: string | null
    detectedLoaderVersion?: string | null
    detectedModCount?: number
    gameVersionTouched?: boolean
    loaderTouched?: boolean
    loaderVersionTouched?: boolean
}

export function importPlanWarnings(
    snapshot: ImportPlanSnapshot | null,
    selected: ImportPlanWarningsInput,
) {
    const loader = selected.loader || 'vanilla'
    const loaderVersion = selected.loaderVersion || ''
    const gameVersion = selected.gameVersion || ''
    const detectedGameVersion = importPlanDefaultGameVersion(
        selected.detectedGameVersion ?? snapshot?.gameVersion,
    )
    const detectedLoader = importPlanDefaultLoader(selected.detectedLoader ?? snapshot?.loader)
    const detectedLoaderVersion = importPlanDefaultLoaderVersion(
        selected.detectedLoaderVersion ?? snapshot?.loaderVersion,
    )
    const gameVersionCustom = gameVersion !== detectedGameVersion
    const loaderTypeCustom = loader !== detectedLoader
    const loaderVersionCustom = loader !== 'vanilla' && loaderVersion !== detectedLoaderVersion
    const loaderVersionMissing =
        loader !== 'vanilla' && (!loaderVersion || loaderVersion === 'latest')
    const loaderTypeUnrecognized =
        (loader === 'vanilla' || !isImportPlanLoaderRecognized(loader)) &&
        ((selected.detectedModCount ?? 0) > 0 || (snapshot?.modCount ?? 0) > 0)

    return {
        gameVersionCustom,
        loaderTypeCustom,
        loaderVersionCustom,
        loaderVersionMissing,
        loaderTypeUnrecognized,
        hasWarnings:
            gameVersionCustom ||
            loaderTypeCustom ||
            loaderVersionCustom ||
            loaderVersionMissing ||
            loaderTypeUnrecognized,
    }
}

export function reduceImportPlanSnapshot(
    previous: ImportPlanSnapshot | null,
    incoming: ImportPlanSnapshot | null,
    activeRequestId: string,
) {
    if (!incoming || incoming.requestId !== activeRequestId) return previous
    return incoming
}

export interface SymlinkMethodInstance {
    name: string
    path?: string
    launcherType?: string
    basePath?: string
    versionPath?: string
    compatibleMode?: boolean
}

export interface SymlinkMethodChoice {
    instanceName: string
    instancePath?: string
    symlink: boolean
    gameVersion?: string | null
    loader?: string | null
    loaderVersion?: string | null
    gameDirOverride?: string | null
}

export interface InstanceImportProvider {
    /**
     * Returns launchers with instances already populated. `onLauncher` receives
     * each launcher as soon as it has been found, before the slowest one finishes.
     */
    getDetectedLaunchers: (
        onLauncher?: (launcher: ImportableLauncher) => void,
    ) => Promise<ImportableLauncher[]>
    /** Only needed for manually-added launcher paths */
    getImportableInstances: (
        launcherName: string,
        path: string,
    ) => Promise<{ name: string; path: string }[]>
    /** Resolve available loader version ids for a loader and game version */
    getLoaderVersions: (loader: string, gameVersion: string) => Promise<string[]>
    /** Open a filesystem path in the platform file manager */
    openPath: (path: string) => Promise<void>
    /** Start a non-blocking import statistics scan and return its request id */
    startImportPlan: (request: ImportPlanRequest) => Promise<string>
    /** Cancel a previously started import statistics scan */
    cancelImportPlan: (requestId: string) => Promise<void>
    /** Subscribe to import_plan events; resolves to an unsubscribe function */
    listenImportPlan: (callback: (snapshot: ImportPlanSnapshot) => void) => Promise<() => void>
    /** Perform the actual import */
    importInstances: (
        selections: {
            launcher: string
            path: string
            instanceNames: string[]
            instancePaths: string[]
            launcherType?: string
            symlink?: boolean
            gameVersion?: string | null
            loader?: string | null
            loaderVersion?: string | null
            gameDirOverride?: string | null
        }[],
    ) => Promise<void>
    /** Open a directory picker (platform-specific) */
    selectDirectory: () => Promise<string | null>
    /** Open a multi-directory picker */
    selectDirectories: () => Promise<string[] | null>
}

export const [injectInstanceImport, provideInstanceImport] =
    createContext<InstanceImportProvider>('InstanceImport')

import { formatPreviewMessage } from '../../lib/i18n'
import { desktopAppMessages } from '../../lib/messages'
import { currentPreview } from '../../lib/state'
import { simulatedMachine } from '../instance/settings/java/machine'
import { serverPreviewMessages } from './server-preview-messages'

/**
 * The app helpers the ported server screens call (`helpers/metadata`,
 * `helpers/jre`, `helpers/utils`, `helpers/library-display-mode`,
 * `providers/download-manager`, `composables/useMultiplayerSession`), answered
 * from the preview's data and its simulated Mac.
 */

export type DesktopOnlyAction = 'server-download' | 'open-folder'

/** What the preview leaves to the desktop app; the message says so in the window's language. */
export function desktopOnlyError(action: DesktopOnlyAction): Error {
    return new Error(
        formatPreviewMessage(
            action === 'server-download'
                ? serverPreviewMessages.serverDownload
                : serverPreviewMessages.openFolder,
        ),
    )
}

function wait(ms: number) {
    return new Promise<void>((resolve) => currentPreview().clock.after(ms, resolve))
}

// ---- helpers/metadata ------------------------------------------------------

/** `get_game_versions()`: Mojang's version manifest, newest first. */
export async function get_game_versions() {
    const data = await import('../../data')
    const minecraft = await data.loadMinecraft()
    const versions = [
        ...minecraft.releases.map((entry) => ({ ...entry, type: 'release' })),
        ...minecraft.snapshots.map((entry) => ({ ...entry, type: 'snapshot' })),
        ...(minecraft.old ?? []),
    ].sort((a, b) => Date.parse(b.releaseTime) - Date.parse(a.releaseTime))
    return {
        latest: minecraft.latest,
        versions: versions.map((entry) => ({ id: entry.id, type: entry.type, url: '' })),
    }
}

/** `get_loader_versions(loader)`: the daedalus manifest of a loader, per game version. */
export async function get_loader_versions(loader: string, _gameVersion?: string) {
    const data = await import('../../data')
    const manifests = await data.loadLoaders()
    const manifest = manifests[loader as keyof typeof manifests]
    if (!manifest) return { gameVersions: [] }
    return {
        gameVersions: Object.entries(manifest).map(([id, entry]) => ({
            id,
            stable: entry.stable,
            loaders: entry.versions.map((version) => ({
                id: version,
                stable: entry.stableVersions ? entry.stableVersions.includes(version) : true,
            })),
        })),
    }
}

// ---- helpers/jre -----------------------------------------------------------

/** `get_java_default_versions()`: the detected runtime chosen for each major version. */
export async function get_java_default_versions() {
    await wait(180)
    const machine = simulatedMachine(currentPreview())
    const seen = new Set<number>()
    return machine.javas
        .filter((java) => !seen.has(java.parsed_version) && seen.add(java.parsed_version))
        .map((java) => ({ ...java }))
}

/** `find_filtered_jres(major)`. */
export async function find_filtered_jres(major: number | null) {
    await wait(260)
    return simulatedMachine(currentPreview())
        .javas.filter((java) => major == null || java.parsed_version === major)
        .map((java) => ({ ...java }))
}

/** `get_jre(path)`: the runtime at `path`. */
export async function get_jre(path: string) {
    await wait(420)
    const java = simulatedMachine(currentPreview()).javas.find(
        (entry) => entry.path === path.trim(),
    )
    if (!java) throw new Error(`Not a valid Java executable: ${path}`)
    return { ...java }
}

/** `get_max_memory()`: the machine's memory in KiB. */
export async function get_max_memory() {
    const { MACHINE_TOTAL_MEMORY } = await import('../instance/settings/java/machine')
    return MACHINE_TOTAL_MEMORY / 1024
}

// ---- helpers/utils ---------------------------------------------------------

/** `openPath`: Finder belongs to the desktop app; the preview says so. */
export async function openPath(_path: string) {
    currentPreview().notifications.addNotification({
        type: 'info',
        title: formatPreviewMessage(serverPreviewMessages.openFolder),
        text: formatPreviewMessage(desktopAppMessages.download),
    })
}

/** `convertFileSrc`: icons the preview shows are already URLs. */
export function convertFileSrc(path: string) {
    return path
}

// ---- helpers/cache ---------------------------------------------------------

/** `get_version`: modpack server versions are resolved by the desktop app. */
export async function get_version(_id: string): Promise<never> {
    throw desktopOnlyError('server-download')
}

// ---- helpers/library-display-mode ----------------------------------------

export type LibraryDisplayMode = 'standard' | 'cards'

const LIBRARY_DISPLAY_MODE_STORAGE_KEY = 'axolotl-library-display-mode'

export function getLastLibraryDisplayMode(): LibraryDisplayMode {
    try {
        return globalThis.localStorage?.getItem(LIBRARY_DISPLAY_MODE_STORAGE_KEY) === 'cards'
            ? 'cards'
            : 'standard'
    } catch {
        return 'standard'
    }
}

export function setLastLibraryDisplayMode(mode: LibraryDisplayMode) {
    try {
        globalThis.localStorage?.setItem(LIBRARY_DISPLAY_MODE_STORAGE_KEY, mode)
    } catch {
        // Storage can be unavailable; the choice then lasts for this visit.
    }
}

// ---- providers/download-manager -------------------------------------------

/** The part of the app's download manager the server flows use. */
export interface DownloadManager {
    addSyntheticJob(job: unknown): void
    setSyntheticJob(jobId: string, job: unknown): void
    onSyntheticCancel(jobId: string, handler: () => void | Promise<void>): void
    offSyntheticCancel(jobId: string): void
}

/**
 * The preview downloads no server files, so server flows run without a
 * sidebar job, as they do in the app outside the download provider.
 */
export function injectDownloadManager(): DownloadManager {
    throw new Error('No download manager in the preview')
}

/** `exportErrorLogs`: the report downloads where the browser saves files. */
export async function exportErrorLogs(report: string, title: string) {
    const { download } = await import('../../lib/platform/fs')
    download(`${title}.txt`, new Blob([report], { type: 'text/plain' }))
}

/** `highlightInFolder`: Finder belongs to the desktop app; the preview says so. */
export async function highlightInFolder(path: string) {
    await openPath(path)
}

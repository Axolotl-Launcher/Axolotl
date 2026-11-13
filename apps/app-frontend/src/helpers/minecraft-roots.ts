import { syncConfiguredDirectLinks } from './direct-link-sync'
import type { DirectLinkSyncReport, ExternalMinecraftRoot } from './instance'

/** Where the external `.minecraft` folders the launcher manages in place are kept. */
export const MINECRAFT_ROOTS_STORAGE_KEY = 'axolotl-minecraft-directories'

/** The configured roots; a bare string from an older build reads as an automatic root. */
export function readMinecraftRoots(): ExternalMinecraftRoot[] {
    try {
        const raw = localStorage.getItem(MINECRAFT_ROOTS_STORAGE_KEY)
        const stored: unknown = raw ? JSON.parse(raw) : []
        return (Array.isArray(stored) ? stored : []).flatMap((value) => {
            if (typeof value === 'string') return [{ path: value, mode: 'automatic' as const }]
            if (!value || typeof value.path !== 'string') return []
            return [
                {
                    path: value.path,
                    mode:
                        value.mode === 'shared'
                            ? ('shared' as const)
                            : value.mode === 'automatic'
                              ? ('automatic' as const)
                              : ('isolated' as const),
                },
            ]
        })
    } catch {
        return []
    }
}

export function writeMinecraftRoots(roots: readonly ExternalMinecraftRoot[]) {
    localStorage.setItem(MINECRAFT_ROOTS_STORAGE_KEY, JSON.stringify(roots))
}

const rootKey = (path: string) => path.replaceAll('\\', '/').replace(/\/+$/, '').toLowerCase()

let additions: Promise<unknown> = Promise.resolve()

/**
 * Adds `path` to the configured roots, reconciles them and stores the result when `keep`
 * accepts the report. Additions run one at a time, each from the roots the last one stored,
 * so two folders added together cannot drop each other.
 */
export function addMinecraftRoot(
    path: string,
    keep: (report: DirectLinkSyncReport) => boolean = () => true,
): Promise<{ report: DirectLinkSyncReport; added: boolean; kept: boolean }> {
    const run = additions.then(async () => {
        const roots = readMinecraftRoots()
        const added = !roots.some((root) => rootKey(root.path) === rootKey(path))
        if (added) roots.push({ path, mode: 'automatic' })
        const report = await syncConfiguredDirectLinks(roots)
        const kept = keep(report)
        if (kept) writeMinecraftRoots(roots)
        return { report, added, kept }
    })
    additions = run.catch(() => {})
    return run
}

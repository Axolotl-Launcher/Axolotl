import type { ExternalMinecraftRoot } from './instance'

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

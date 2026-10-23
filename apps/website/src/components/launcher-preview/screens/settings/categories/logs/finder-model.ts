import { HOME_DIR } from './launcher-logs'

/**
 * What the Finder shows of this Mac: the user's standard folders in the
 * sidebar, and the files the launcher reveals or creates in them.
 */
export type FinderItemKind = 'log' | 'zip'

export interface FinderItem {
    name: string
    kind: FinderItemKind
    bytes: number
    /** Epoch milliseconds. */
    modified: number
}

export const FINDER_FAVORITES = ['Desktop', 'Documents', 'Downloads'] as const
export type FinderFavorite = (typeof FINDER_FAVORITES)[number]

export function favoritePath(folder: FinderFavorite) {
    return `${HOME_DIR}/${folder}`
}

export function favoriteOf(path: string): FinderFavorite | null {
    return FINDER_FAVORITES.find((folder) => favoritePath(folder) === path) ?? null
}

export function parentOf(path: string) {
    const index = path.lastIndexOf('/')
    return index > 0 ? path.slice(0, index) : '/'
}

export function baseName(path: string) {
    return path.slice(path.lastIndexOf('/') + 1)
}

/** The folders from the startup disk down to `path`, as the path bar lists them. */
export function pathSegments(path: string) {
    return path.split('/').filter(Boolean)
}

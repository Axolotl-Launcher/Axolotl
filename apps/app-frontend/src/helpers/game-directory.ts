import type { GameInstance } from './types'

export const MANAGED_GAME_DIRECTORY_KEY = 'managed'

export type GameDirectoryKind = 'managed' | 'external'

export interface GameDirectoryInfo {
    key: string
    kind: GameDirectoryKind
    path: string | null
    shortLabel: string | null
}

export type GameDirectoryFields = Pick<
    GameInstance,
    'linked_dot_minecraft' | 'game_dir_override' | 'symlink_target'
>

function nonEmptyPath(value: string | null | undefined): string | null {
    const trimmed = value?.trim()
    return trimmed ? trimmed : null
}

function isWindowsPath(path: string): boolean {
    return /^[A-Za-z]:[\\/]/.test(path) || path.startsWith('\\\\') || path.includes('\\')
}

function collapsePathSeparators(path: string): string {
    const normalizedSeparators = path.replace(/\\/g, '/')
    const uncPrefix = normalizedSeparators.startsWith('//') ? '//' : ''
    const withoutPrefix = normalizedSeparators.slice(uncPrefix.length)
    const collapsed = `${uncPrefix}${withoutPrefix.replace(/\/+/g, '/')}`

    if (collapsed === '/' || /^[A-Za-z]:\/$/.test(collapsed)) return collapsed
    return collapsed.replace(/\/+$/, '')
}

function normalizeGameDirectoryPath(path: string): Omit<GameDirectoryInfo, 'kind'> {
    const windows = isWindowsPath(path)
    let normalized = collapsePathSeparators(path.trim())
    const segments = normalized.split('/')

    if (segments.length >= 2 && segments[segments.length - 2]?.toLowerCase() === 'versions') {
        segments.splice(-2, 2)
        normalized = segments.join('/')
        if (/^[A-Za-z]:$/.test(normalized)) normalized += '/'
    }

    const displaySeparator = windows ? '\\' : '/'
    const displayPath = windows ? normalized.replace(/\//g, '\\') : normalized
    const displaySegments = normalized.split('/').filter(Boolean)
    const basename = displaySegments.at(-1) ?? normalized
    const parent = displaySegments.at(-2)
    const shortLabel =
        basename.toLowerCase() === '.minecraft' && parent
            ? `${parent}${displaySeparator}.minecraft`
            : basename
    const keyPath = windows ? normalized.toLowerCase() : normalized

    return {
        key: `external:${keyPath}`,
        path: displayPath,
        shortLabel,
    }
}

export function resolveGameDirectory(instance: GameDirectoryFields): GameDirectoryInfo {
    const path =
        nonEmptyPath(instance.linked_dot_minecraft) ??
        nonEmptyPath(instance.game_dir_override) ??
        nonEmptyPath(instance.symlink_target)

    if (!path) {
        return {
            key: MANAGED_GAME_DIRECTORY_KEY,
            kind: 'managed',
            path: null,
            shortLabel: null,
        }
    }

    return {
        kind: 'external',
        ...normalizeGameDirectoryPath(path),
    }
}

export function hasMultipleGameDirectories(instances: readonly GameDirectoryFields[]): boolean {
    const keys = new Set<string>()
    for (const instance of instances) {
        keys.add(resolveGameDirectory(instance).key)
        if (keys.size >= 2) return true
    }
    return false
}

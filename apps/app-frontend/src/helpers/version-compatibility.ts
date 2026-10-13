export interface VersionRange {
    type: 'exact' | 'gte' | 'lte' | 'range' | 'wildcard'
    major: number
    minor: number
    patch?: number
    /**
     * For `wildcard` ranges, whether the minor component is pinned: `1.20.x`
     * pins it, while `1.x` leaves it free. Unset for every other range type.
     */
    minorPinned?: boolean
    upperMajor?: number
    upperMinor?: number
    upperPatch?: number
    upperInclusive?: boolean
}

export function parseVersion(versionStr: string): {
    major: number
    minor: number
    patch: number
} | null {
    const match = versionStr.match(/^(\d+)\.(\d+)(?:\.(\d+))?$/)
    if (!match) return null
    return {
        major: parseInt(match[1], 10),
        minor: parseInt(match[2], 10),
        patch: match[3] ? parseInt(match[3], 10) : 0,
    }
}

export function parseVersionRange(rangeStr: string): VersionRange | null {
    const trimmed = rangeStr.trim()

    if (trimmed.startsWith('>=')) {
        const version = parseVersion(trimmed.slice(2))
        if (!version) return null
        return { type: 'gte', ...version }
    }

    if (trimmed.startsWith('<=')) {
        const version = parseVersion(trimmed.slice(2))
        if (!version) return null
        return { type: 'lte', ...version }
    }

    if (trimmed.startsWith('>')) {
        const version = parseVersion(trimmed.slice(1))
        if (!version) return null
        return { type: 'gte', ...version, patch: version.patch + 1 }
    }

    if (trimmed.startsWith('<')) {
        const version = parseVersion(trimmed.slice(1))
        if (!version) return null
        return { type: 'lte', ...version, patch: version.patch - 1 }
    }

    const bracketMatch = trimmed.match(/^\[(\d+\.\d+(?:\.\d+)?)\s*,\s*(\d+\.\d+(?:\.\d+)?)\)$/)
    if (bracketMatch) {
        const lower = parseVersion(bracketMatch[1])
        const upper = parseVersion(bracketMatch[2])
        if (!lower || !upper) return null
        return {
            type: 'range',
            ...lower,
            upperMajor: upper.major,
            upperMinor: upper.minor,
            upperPatch: upper.patch,
            upperInclusive: false,
        }
    }

    const closedBracketMatch = trimmed.match(
        /^\[(\d+\.\d+(?:\.\d+)?)\s*,\s*(\d+\.\d+(?:\.\d+)?)\]$/,
    )
    if (closedBracketMatch) {
        const lower = parseVersion(closedBracketMatch[1])
        const upper = parseVersion(closedBracketMatch[2])
        if (!lower || !upper) return null
        return {
            type: 'range',
            ...lower,
            upperMajor: upper.major,
            upperMinor: upper.minor,
            upperPatch: upper.patch,
            upperInclusive: true,
        }
    }

    if (trimmed.includes('x') || trimmed.includes('X')) {
        const match = trimmed.match(/^(\d+)\.(\d+)\.x$/i)
        if (match) {
            return {
                type: 'wildcard',
                major: parseInt(match[1], 10),
                minor: parseInt(match[2], 10),
                minorPinned: true,
            }
        }
        const minorWildcard = trimmed.match(/^(\d+)\.x$/i)
        if (minorWildcard) {
            return {
                type: 'wildcard',
                major: parseInt(minorWildcard[1], 10),
                minor: 0,
                minorPinned: false,
            }
        }
    }

    if (trimmed.includes('-')) {
        const parts = trimmed.split('-').map((p) => p.trim())
        if (parts.length === 2) {
            const lower = parseVersion(parts[0])
            const upper = parseVersion(parts[1])
            if (!lower || !upper) return null
            return {
                type: 'range',
                ...lower,
                upperMajor: upper.major,
                upperMinor: upper.minor,
                upperPatch: upper.patch,
                upperInclusive: true,
            }
        }
    }

    const exact = parseVersion(trimmed)
    if (exact) {
        return { type: 'exact', ...exact }
    }

    return null
}

function compareVersions(
    a: { major: number; minor: number; patch: number },
    b: { major: number; minor: number; patch: number },
): number {
    if (a.major !== b.major) return a.major - b.major
    if (a.minor !== b.minor) return a.minor - b.minor
    return a.patch - b.patch
}

export function compareSemanticVersions(left: string, right: string): number | null {
    const leftVersion = parseVersion(left)
    const rightVersion = parseVersion(right)
    if (!leftVersion || !rightVersion) return null
    return compareVersions(leftVersion, rightVersion)
}

export function isVersionInRange(instanceVersion: string, modRange: string): boolean {
    const instanceParsed = parseVersion(instanceVersion)
    if (!instanceParsed) return false

    const range = parseVersionRange(modRange)
    if (!range) {
        return instanceVersion.includes(modRange) || modRange.includes(instanceVersion)
    }

    switch (range.type) {
        case 'exact': {
            const rangeVersion = { major: range.major, minor: range.minor, patch: range.patch ?? 0 }
            return compareVersions(instanceParsed, rangeVersion) === 0
        }
        case 'gte': {
            const rangeVersion = { major: range.major, minor: range.minor, patch: range.patch ?? 0 }
            return compareVersions(instanceParsed, rangeVersion) >= 0
        }
        case 'lte': {
            const rangeVersion = { major: range.major, minor: range.minor, patch: range.patch ?? 0 }
            return compareVersions(instanceParsed, rangeVersion) <= 0
        }
        case 'range': {
            const lowerVersion = { major: range.major, minor: range.minor, patch: range.patch ?? 0 }
            const upperVersion = {
                major: range.upperMajor!,
                minor: range.upperMinor!,
                patch: range.upperPatch ?? 0,
            }
            const lowerCompare = compareVersions(instanceParsed, lowerVersion)
            const upperCompare = compareVersions(instanceParsed, upperVersion)

            if (lowerCompare < 0) return false
            if (range.upperInclusive) {
                return upperCompare <= 0
            }
            return upperCompare < 0
        }
        case 'wildcard': {
            if (instanceParsed.major !== range.major) return false
            if (range.minorPinned) {
                return instanceParsed.minor === range.minor
            }
            return true
        }
        default:
            return false
    }
}

export const LOADER_COMPATIBILITY: Record<string, string[]> = {
    fabric: ['fabric', 'quilt'],
    quilt: ['fabric', 'quilt'],
    forge: ['forge'],
    neoforge: ['neoforge'],
}

export function areLoadersCompatible(modLoader: string, instanceLoader: string): boolean {
    if (!modLoader || !instanceLoader) return true

    const modLower = modLoader.toLowerCase()
    const instLower = instanceLoader.toLowerCase()

    if (modLower === instLower) return true

    const compatibleWithMod = LOADER_COMPATIBILITY[modLower]
    if (compatibleWithMod && compatibleWithMod.includes(instLower)) return true

    return false
}

export interface VersionRange {
    type: 'exact' | 'gte' | 'lte' | 'range' | 'wildcard'
    major: number
    minor: number
    patch?: number
    upperMajor?: number
    upperMinor?: number
    upperPatch?: number
    upperInclusive?: boolean
    lowerInclusive?: boolean
    wildcardLevel?: 'minor' | 'patch'
}

export function parseVersion(versionStr: string): {
    major: number
    minor: number
    patch: number
} | null {
    const match = versionStr.match(/^(\d+)(?:\.(\d+))?(?:\.(\d+))?$/)
    if (!match) return null
    return {
        major: parseInt(match[1], 10),
        minor: match[2] ? parseInt(match[2], 10) : 0,
        patch: match[3] ? parseInt(match[3], 10) : 0,
    }
}

export function parseVersionRange(rangeStr: string): VersionRange | null {
    const trimmed = rangeStr.trim()
    if (trimmed.startsWith('[') && trimmed.endsWith(']') && !trimmed.includes(',')) {
        const exact = parseVersion(trimmed.slice(1, -1).trim())
        return exact ? { type: 'exact', ...exact } : null
    }

    if (trimmed.startsWith('>=')) {
        const version = parseVersion(trimmed.slice(2).trim())
        if (!version) return null
        return { type: 'gte', ...version }
    }

    if (trimmed.startsWith('<=')) {
        const version = parseVersion(trimmed.slice(2).trim())
        if (!version) return null
        return { type: 'lte', ...version }
    }

    if (trimmed.startsWith('>')) {
        const version = parseVersion(trimmed.slice(1).trim())
        if (!version) return null
        return { type: 'gte', ...version, lowerInclusive: false }
    }

    if (trimmed.startsWith('<')) {
        const version = parseVersion(trimmed.slice(1).trim())
        if (!version) return null
        return { type: 'lte', ...version, upperInclusive: false }
    }

    const bracketMatch = trimmed.match(
        /^([[(])\s*(\d+(?:\.\d+){0,2})?\s*,\s*(\d+(?:\.\d+){0,2})?\s*([\])])$/,
    )
    if (bracketMatch) {
        const lower = bracketMatch[2] ? parseVersion(bracketMatch[2]) : null
        const upper = bracketMatch[3] ? parseVersion(bracketMatch[3]) : null
        if (!lower && !upper) return null
        if (!lower && upper)
            return { type: 'lte', ...upper, upperInclusive: bracketMatch[4] === ']' }
        if (lower && !upper)
            return { type: 'gte', ...lower, lowerInclusive: bracketMatch[1] === '[' }
        if (!lower || !upper || compareVersions(lower, upper) > 0) return null
        return {
            type: 'range',
            ...lower,
            upperMajor: upper.major,
            upperMinor: upper.minor,
            upperPatch: upper.patch,
            lowerInclusive: bracketMatch[1] === '[',
            upperInclusive: bracketMatch[4] === ']',
        }
    }

    if (/[x*]/i.test(trimmed)) {
        const match = trimmed.match(/^(\d+)\.(\d+)\.[x*]$/i)
        if (match) {
            return {
                type: 'wildcard',
                major: parseInt(match[1], 10),
                minor: parseInt(match[2], 10),
                wildcardLevel: 'patch',
            }
        }
        const minorWildcard = trimmed.match(/^(\d+)\.[x*]$/i)
        if (minorWildcard) {
            return {
                type: 'wildcard',
                major: parseInt(minorWildcard[1], 10),
                minor: 0,
                wildcardLevel: 'minor',
            }
        }
    }

    if (trimmed.includes('-')) {
        const parts = trimmed.split('-').map((p) => p.trim())
        if (parts.length === 2) {
            const lower = parseVersion(parts[0])
            const upper = parseVersion(parts[1])
            if (!lower || !upper || compareVersions(lower, upper) > 0) return null
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

export type VersionCompatibility = 'compatible' | 'incompatible' | 'unknown'

export function assessVersionRange(
    instanceVersion: string,
    modRange: string,
): VersionCompatibility {
    if (['*', 'x', 'X'].includes(modRange.trim()) && parseVersion(instanceVersion))
        return 'compatible'
    if (modRange.includes('||')) {
        const results = modRange
            .split('||')
            .map((range) => assessVersionRange(instanceVersion, range.trim()))
        return results.includes('compatible')
            ? 'compatible'
            : results.includes('unknown')
              ? 'unknown'
              : 'incompatible'
    }
    const bounds = modRange.trim().match(/(?:>=|<=|>|<|=)\s*\d+(?:\.\d+){0,2}/g)
    if (
        bounds &&
        bounds.length > 1 &&
        modRange.replace(/(?:>=|<=|>|<|=)\s*\d+(?:\.\d+){0,2}/g, '').trim() === ''
    ) {
        const results = bounds.map((range) =>
            assessVersionRange(instanceVersion, range.replace(/\s/g, '').replace(/^=/, '')),
        )
        return results.includes('incompatible')
            ? 'incompatible'
            : results.includes('unknown')
              ? 'unknown'
              : 'compatible'
    }
    const parsed = parseVersion(instanceVersion)
    const range = parseVersionRange(modRange)
    if (!parsed)
        return modRange.trim() === instanceVersion && !!instanceVersion ? 'compatible' : 'unknown'
    if (!range) return 'unknown'
    return matchesParsedRange(parsed, range) ? 'compatible' : 'incompatible'
}

export function isVersionInRange(instanceVersion: string, modRange: string): boolean {
    return assessVersionRange(instanceVersion, modRange) === 'compatible'
}

function matchesParsedRange(
    instanceParsed: NonNullable<ReturnType<typeof parseVersion>>,
    range: VersionRange,
): boolean {
    switch (range.type) {
        case 'exact': {
            const rangeVersion = { major: range.major, minor: range.minor, patch: range.patch ?? 0 }
            return compareVersions(instanceParsed, rangeVersion) === 0
        }
        case 'gte': {
            const rangeVersion = { major: range.major, minor: range.minor, patch: range.patch ?? 0 }
            const comparison = compareVersions(instanceParsed, rangeVersion)
            return range.lowerInclusive === false ? comparison > 0 : comparison >= 0
        }
        case 'lte': {
            const rangeVersion = { major: range.major, minor: range.minor, patch: range.patch ?? 0 }
            const comparison = compareVersions(instanceParsed, rangeVersion)
            return range.upperInclusive === false ? comparison < 0 : comparison <= 0
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

            if (lowerCompare < 0 || (lowerCompare === 0 && range.lowerInclusive === false))
                return false
            if (range.upperInclusive) {
                return upperCompare <= 0
            }
            return upperCompare < 0
        }
        case 'wildcard': {
            if (instanceParsed.major !== range.major) return false
            if (range.wildcardLevel === 'patch') {
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
    quilt: ['quilt'],
    forge: ['forge'],
    neoforge: ['neoforge'],
}

export function areLoadersCompatible(modLoader: string, instanceLoader: string): boolean {
    if (!modLoader || !instanceLoader) return false

    const modLower = modLoader.toLowerCase()
    const instLower = instanceLoader.toLowerCase()

    if (modLower === instLower) return true

    const compatibleWithMod = LOADER_COMPATIBILITY[modLower]
    if (compatibleWithMod && compatibleWithMod.includes(instLower)) return true

    return false
}

export function assessLocalModCompatibility(
    declared: { minecraft_version?: string; loader?: string } | null,
    target: { game_version?: string; loader?: string },
) {
    return {
        gameVersion:
            declared?.minecraft_version && target.game_version
                ? assessVersionRange(target.game_version, declared.minecraft_version)
                : ('unknown' as VersionCompatibility),
        loader:
            declared?.loader && target.loader
                ? areLoadersCompatible(declared.loader, target.loader)
                    ? ('compatible' as const)
                    : ('incompatible' as const)
                : ('unknown' as const),
    }
}

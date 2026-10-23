/**
 * `formatVersionsForDisplay` (packages/utils/projects.ts): collapses a project's game versions
 * into ranges ("1.20–1.21.1", "1.19.x") against the full game version list, oldest last.
 */
export interface GameVersionTag {
    version: string
    version_type: string
    date: string
    major?: boolean
}

type VersionRange = { major: string; minor: number[] }

const mcVersionRegex = /^([0-9]+.[0-9]+)(.[0-9]+)?$/

function groupVersions(versions: string[], consecutive = false) {
    return versions
        .slice()
        .reverse()
        .reduce((ranges: VersionRange[], version: string) => {
            const matchesVersion = version.match(mcVersionRegex)
            if (matchesVersion) {
                const majorVersion = matchesVersion[1]
                const minorVersion = matchesVersion[2]
                const minorNumeric = minorVersion ? parseInt(minorVersion.replace('.', '')) : 0
                const prevInRange = ranges.find(
                    (range) =>
                        range.major === majorVersion &&
                        (!consecutive || range.minor.at(-1) === minorNumeric - 1),
                )
                if (prevInRange) {
                    prevInRange.minor.push(minorNumeric)
                    return ranges
                }
                return [...ranges, { major: majorVersion, minor: [minorNumeric] }]
            }
            return ranges
        }, [])
        .reverse()
}

function validateRange(range: string): string {
    switch (range) {
        case 'rd-132211–b1.8.1':
            return 'All legacy versions'
        case 'a1.0.4–b1.8.1':
            return 'All alpha and beta versions'
        case 'a1.0.4–a1.2.6':
            return 'All alpha versions'
        case 'b1.0–b1.8.1':
            return 'All beta versions'
        case 'rd-132211–inf20100618':
            return 'All pre-alpha versions'
    }
    const splitRange = range.split('–')
    if (splitRange && splitRange[0] === splitRange[1]) return splitRange[0]
    return range
}

function groupConsecutiveIndices(versions: string[], referenceList: GameVersionTag[]) {
    if (!versions || versions.length === 0) return []
    const referenceMap = new Map<string, number>()
    referenceList.forEach((item, index) => referenceMap.set(item.version, index))
    const sortedList = versions
        .slice()
        .sort((a, b) => (referenceMap.get(a) ?? 0) - (referenceMap.get(b) ?? 0))
    const ranges: string[] = []
    let start = sortedList[0]
    let previous = sortedList[0]
    for (let i = 1; i < sortedList.length; i++) {
        const current = sortedList[i]
        if (referenceMap.get(current) !== (referenceMap.get(previous) ?? 0) + 1) {
            ranges.push(validateRange(`${previous}–${start}`))
            start = current
        }
        previous = current
    }
    ranges.push(validateRange(`${previous}–${start}`))
    return ranges
}

function formatMinecraftMinorVersion(major: string, minor: number): string {
    return minor === 0 ? major : `${major}.${minor}`
}

export function formatVersionsForDisplay(
    gameVersions: string[],
    allGameVersions: GameVersionTag[],
) {
    const inputVersions = gameVersions.slice()
    const allVersions = allGameVersions.slice()
    const allSnapshots = allVersions.filter((version) => version.version_type === 'snapshot')
    const allReleases = allVersions.filter((version) => version.version_type === 'release')
    const allLegacy = allVersions.filter(
        (version) => version.version_type !== 'snapshot' && version.version_type !== 'release',
    )
    {
        const indices = allVersions.reduce<Record<string, number>>((map, gameVersion, index) => {
            map[gameVersion.version] = index
            return map
        }, {})
        inputVersions.sort((a, b) => (indices[a] ?? 0) - (indices[b] ?? 0))
    }
    const releaseVersions = inputVersions.filter((projectVersion) =>
        allReleases.some((gameVersion) => gameVersion.version === projectVersion),
    )
    const dateString = allReleases.find((version) => version.version === releaseVersions[0])?.date
    const latestReleaseVersionDate = dateString ? Date.parse(dateString) : 0
    const latestSnapshot = inputVersions.find((projectVersion) =>
        allSnapshots.some(
            (gameVersion) =>
                gameVersion.version === projectVersion &&
                (!latestReleaseVersionDate ||
                    latestReleaseVersionDate < Date.parse(gameVersion.date)),
        ),
    )
    const allReleasesGrouped = groupVersions(
        allReleases.map((release) => release.version),
        false,
    )
    const projectVersionsGrouped = groupVersions(releaseVersions, true)
    const releaseVersionsAsRanges = projectVersionsGrouped.map(({ major, minor }) => {
        if (minor.length === 1) return formatMinecraftMinorVersion(major, minor[0])
        const range = allReleasesGrouped.find((entry) => entry.major === major)
        if (range?.minor.every((value, index) => value === minor[index])) return `${major}.x`
        return `${formatMinecraftMinorVersion(major, minor[0])}–${formatMinecraftMinorVersion(major, minor[minor.length - 1])}`
    })
    const legacyVersionsAsRanges = groupConsecutiveIndices(
        inputVersions.filter((projectVersion) =>
            allLegacy.some((gameVersion) => gameVersion.version === projectVersion),
        ),
        allLegacy,
    )
    let output = [...legacyVersionsAsRanges]
    if (releaseVersionsAsRanges.length === 0) {
        const snapshotVersions = inputVersions.filter((projectVersion) =>
            allSnapshots.some((gameVersion) => gameVersion.version === projectVersion),
        )
        const snapshotVersionsAsRanges =
            snapshotVersions.length > 3
                ? groupConsecutiveIndices(snapshotVersions, allSnapshots)
                : snapshotVersions
        output = [...snapshotVersionsAsRanges, ...output]
    } else {
        output = [...releaseVersionsAsRanges, ...output]
    }
    if (releaseVersionsAsRanges.length > 0 && latestSnapshot) output = [latestSnapshot, ...output]
    return output
}

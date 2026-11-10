import type { BrowseInstallPreferences } from '@modrinth/ui'
import type { LocationQuery } from 'vue-router'

import { getCurseForgeFiles } from './curseforge'
import { type ResourceRelease, resourceReleases, type SourceId, sourceResource } from './resources'

export function projectFilterValues(value: LocationQuery[string]): string[] {
    return (Array.isArray(value) ? value : [value]).filter(
        (item): item is string => typeof item === 'string' && item.length > 0,
    )
}

/** Missing axes inherit the instance; present but empty axes explicitly unlock it. */
export function projectInstallPreferences(
    query: LocationQuery,
    fallback: BrowseInstallPreferences = {},
): BrowseInstallPreferences {
    return {
        gameVersions: Object.hasOwn(query, 'g')
            ? projectFilterValues(query.g)
            : fallback.gameVersions,
        loaders: Object.hasOwn(query, 'l') ? projectFilterValues(query.l) : fallback.loaders,
    }
}

export async function filteredProjectRelease(
    source: SourceId,
    projectId: string,
    preferences: BrowseInstallPreferences,
    channels: string[] = [],
    contentType = 'mod',
) {
    if (source === 'curseforge' && contentType === 'shader') {
        const files = await getCurseForgeFiles(Number(projectId), {})
        const any = (selected: string[] | undefined, values: string[]) =>
            !selected?.length || selected.some((value) => values.includes(value))
        const file = files.files.find(
            (file) =>
                file.isAvailable &&
                any(preferences.gameVersions, file.gameVersions) &&
                any(
                    preferences.loaders,
                    file.gameVersions.map((tag) => tag.toLowerCase().replaceAll(' ', '')),
                ) &&
                any(channels, [
                    file.releaseType === 1 ? 'release' : file.releaseType === 2 ? 'beta' : 'alpha',
                ]),
        )
        return file ? String(file.id) : null
    }
    const target = {
        gameVersions: preferences.gameVersions ?? [],
        loaders:
            source === 'curseforge' && contentType !== 'mod' ? [] : (preferences.loaders ?? []),
        channels,
    }
    // CurseForge accepts one version/loader per request. Multi-selection and
    // channels are OR filters applied to every page, without target fallback.
    const requestTarget =
        source === 'curseforge'
            ? {
                  gameVersions: target.gameVersions.length === 1 ? target.gameVersions : [],
                  loaders:
                      target.loaders.length === 1 &&
                      ['forge', 'fabric', 'quilt', 'neoforge'].includes(target.loaders[0])
                          ? target.loaders
                          : [],
                  channels: [],
              }
            : target
    const any = (selected: string[], values: string[]) =>
        !selected.length || selected.some((value) => values.includes(value))
    let cursor: string | null = null
    const seen = new Set<string>()
    let preferred: ResourceRelease | null = null
    const rank = (channel: string) => (channel === 'release' ? 0 : channel === 'beta' ? 1 : 2)
    do {
        const page = await resourceReleases(
            sourceResource(source, projectId),
            requestTarget,
            cursor,
        )
        const matches = page.entries.filter(
            (release) =>
                any(target.gameVersions, release.compatibility.gameVersions) &&
                any(target.loaders, release.compatibility.loaders) &&
                any(channels, [release.channel]) &&
                release.artifacts.some(
                    (artifact) => artifact.role === 'primary' && artifact.available !== false,
                ),
        )
        if (!matches.length && !preferred && page.exhaustedSources?.length) {
            throw new Error(
                'Source release window is incomplete; choose a specific game version or file',
            )
        }
        if (source !== 'modrinth' && matches[0]) return matches[0].reference.releaseId
        for (const release of matches) {
            if (
                !preferred ||
                rank(release.channel) < rank(preferred.channel) ||
                (rank(release.channel) === rank(preferred.channel) &&
                    Date.parse(release.publishedAt ?? '') > Date.parse(preferred.publishedAt ?? ''))
            )
                preferred = release
        }
        if (
            preferred?.channel === 'release' ||
            !page.hasMore ||
            !page.cursor ||
            seen.has(page.cursor)
        )
            return preferred?.reference.releaseId ?? null
        cursor = page.cursor
        seen.add(cursor)
    } while (cursor)
    return preferred?.reference.releaseId ?? null
}

export function installPreferencesEqual(
    left: BrowseInstallPreferences = {},
    right: BrowseInstallPreferences = {},
) {
    const values = (items: string[] | undefined) => [...new Set(items ?? [])].sort()
    return (
        JSON.stringify([values(left.gameVersions), values(left.loaders)]) ===
        JSON.stringify([values(right.gameVersions), values(right.loaders)])
    )
}

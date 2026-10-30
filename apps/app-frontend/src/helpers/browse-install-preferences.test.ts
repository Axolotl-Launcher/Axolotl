import type { Labrinth } from '@modrinth/api-client'
import {
    getLatestMatchingInstallVersion,
    getTargetInstallPreferences,
} from '@modrinth/ui/src/layouts/shared/browse-tab/composables/install-logic.ts'
import { expect, it } from 'vitest'

function version(
    id: string,
    loaders: string[],
    options: {
        datePublished?: string
        versionType?: Labrinth.Versions.v2.Version['version_type']
    } = {},
): Labrinth.Versions.v2.Version {
    return {
        id,
        date_published: options.datePublished ?? '2026-08-18T00:00:00Z',
        version_type: options.versionType ?? 'release',
        game_versions: ['1.21.1'],
        loaders,
    } as Labrinth.Versions.v2.Version
}

it('resource packs ignore the target game version and match the Minecraft loader', () => {
    const preferences = getTargetInstallPreferences(
        { gameVersion: '26.2', loader: 'neoforge' },
        'resourcepack',
    )

    expect(preferences).toEqual({
        gameVersions: [],
        loaders: ['minecraft'],
    })
    expect(
        getLatestMatchingInstallVersion(
            [
                version('minecraft-resource-pack', ['minecraft']),
                version('neoforge-mod', ['neoforge']),
            ],
            preferences,
        )?.id,
    ).toBe('minecraft-resource-pack')
})

it('automatic installs prefer a compatible release over a newer beta', () => {
    const preferences = getTargetInstallPreferences(
        { gameVersion: '1.21.1', loader: 'neoforge' },
        'mod',
    )

    expect(
        getLatestMatchingInstallVersion(
            [
                version('newer-beta', ['neoforge'], {
                    datePublished: '2026-06-01T00:00:00Z',
                    versionType: 'beta',
                }),
                version('stable-release', ['neoforge'], {
                    datePublished: '2026-05-01T00:00:00Z',
                }),
            ],
            preferences,
        )?.id,
    ).toBe('stable-release')
})

it('shader packs use their Iris compatibility tag instead of the target mod loader', () => {
    const preferences = getTargetInstallPreferences(
        { gameVersion: '1.21.1', loader: 'neoforge' },
        'shader',
    )

    expect(preferences).toEqual({
        gameVersions: ['1.21.1'],
        loaders: ['iris'],
    })
    expect(
        getLatestMatchingInstallVersion(
            [version('iris-shader-pack', ['iris']), version('neoforge-mod', ['neoforge'])],
            preferences,
        )?.id,
    ).toBe('iris-shader-pack')
})

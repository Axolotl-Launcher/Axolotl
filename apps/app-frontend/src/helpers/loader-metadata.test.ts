import { expect, it } from 'vitest'

import {
    createLatestRequestGuard,
    gameVersionSelectorText,
    isLoaderSupportStateDisabled,
    loaderMetadataCacheKey,
    loaderMetadataQueryKey,
    loaderSupportState,
    loaderVersionSelectorText,
    loaderVersionsForGameVersion,
    loaderVersionSummaryState,
    preserveOrSelectGameVersion,
    scopedLoaderMetadataQueryKey,
} from '../../../../packages/ui/src/components/flows/creation-flow-modal/loader-metadata.ts'
import {
    clientInstallableLoaders,
    instanceInstallablePlatforms,
} from '../../../../packages/ui/src/utils/loaders.ts'

const manifest = (gameVersion: string, versions: string[]) => ({
    gameVersions: [
        {
            id: gameVersion,
            loaders: versions.map((id) => ({ id, stable: true })),
        },
    ],
})

it('shares the complete client loader list with instance installation settings', () => {
    expect([...clientInstallableLoaders]).toEqual([
        'fabric',
        'neoforge',
        'forge',
        'quilt',
        'optifine',
        'cleanroom',
        'lite_loader',
        'legacy_fabric',
        'babric',
    ])
    expect([...instanceInstallablePlatforms]).toEqual(['vanilla', ...clientInstallableLoaders])
})

it('instance settings scopes Forge and Fabric queries by Minecraft version', () => {
    const forgeA = scopedLoaderMetadataQueryKey('instance-settings', 'forge', '1.20.1')
    const forgeB = scopedLoaderMetadataQueryKey('instance-settings', 'forge', '26.2')
    const forgeAReturn = scopedLoaderMetadataQueryKey('instance-settings', 'forge', '1.20.1')
    const fabricA = scopedLoaderMetadataQueryKey('instance-settings', 'fabric', '1.20.1')
    const fabricB = scopedLoaderMetadataQueryKey('instance-settings', 'fabric', '1.21.1')

    expect(forgeA).toEqual(['instance-settings', 'loader-versions', 'forge', '1.20.1'])
    expect(forgeB).toEqual(['instance-settings', 'loader-versions', 'forge', '26.2'])
    expect(forgeAReturn).toEqual(forgeA)
    expect(forgeA).not.toEqual(forgeB)
    expect(fabricA).not.toEqual(fabricB)

    const cache = new Map([
        [JSON.stringify(forgeA), manifest('1.20.1', ['47.4.22'])],
        [JSON.stringify(forgeB), manifest('26.2', ['65.1.1', '65.1.0'])],
        [JSON.stringify(fabricA), manifest('1.20.1', ['0.18.4'])],
        [JSON.stringify(fabricB), manifest('1.21.1', ['0.18.4', '0.17.3'])],
    ])
    const ids = (key: readonly string[], gameVersion: string) =>
        loaderVersionsForGameVersion(cache.get(JSON.stringify(key)), gameVersion).map(
            (version) => version.id,
        )

    expect(ids(forgeA, '1.20.1')).toEqual(['47.4.22'])
    expect(ids(forgeB, '26.2')).toEqual(['65.1.1', '65.1.0'])
    expect(ids(forgeAReturn, '1.20.1')).toEqual(['47.4.22'])
    expect(ids(fabricA, '1.20.1')).toEqual(['0.18.4'])
    expect(ids(fabricB, '1.21.1')).toEqual(['0.18.4', '0.17.3'])
})

it('isolates loader metadata by loader and Minecraft version', () => {
    const forge262Key = loaderMetadataCacheKey('forge', '26.2')
    const forge1201Key = loaderMetadataCacheKey('forge', '1.20.1')
    const cache = {
        [forge262Key]: manifest('26.2', ['65.1.1', '65.1.0']),
        [forge1201Key]: manifest('1.20.1', ['47.4.22', '47.4.21']),
    }

    expect(forge262Key).not.toBe(forge1201Key)
    expect(loaderMetadataQueryKey('forge', '26.2')).toEqual([
        'creation-flow',
        'loader-versions',
        'forge',
        '26.2',
    ])
    expect(loaderMetadataQueryKey('forge', '1.20.1')).toEqual([
        'creation-flow',
        'loader-versions',
        'forge',
        '1.20.1',
    ])

    expect(
        loaderVersionsForGameVersion(cache[forge262Key], '26.2').map((version) => version.id),
    ).toEqual(['65.1.1', '65.1.0'])
    expect(
        loaderVersionsForGameVersion(cache[forge1201Key], '1.20.1').map((version) => version.id),
    ).toEqual(['47.4.22', '47.4.21'])
    expect(
        loaderVersionsForGameVersion(cache[forge262Key], '26.2').map((version) => version.id),
    ).toEqual(['65.1.1', '65.1.0'])
})

it('rejects stale loader metadata requests after rapid selection changes', () => {
    const guard = createLatestRequestGuard()
    const minecraftARequest = guard.begin()
    const minecraftBRequest = guard.begin()
    const minecraftAReturnRequest = guard.begin()

    expect(guard.isCurrent(minecraftARequest)).toBe(false)
    expect(guard.isCurrent(minecraftBRequest)).toBe(false)
    expect(guard.isCurrent(minecraftAReturnRequest)).toBe(true)

    const fabricRequest = guard.begin()
    const forgeRequest = guard.begin()
    const neoForgeRequest = guard.begin()
    expect(guard.isCurrent(fabricRequest)).toBe(false)
    expect(guard.isCurrent(forgeRequest)).toBe(false)
    expect(guard.isCurrent(neoForgeRequest)).toBe(true)
})

it('does not treat missing, loading, or errored metadata as unsupported', () => {
    expect(loaderSupportState('unknown', undefined, '1.20.1')).toBe('unknown')
    expect(loaderSupportState('loading', undefined, '1.20.1')).toBe('loading')
    expect(loaderSupportState('error', undefined, '1.20.1')).toBe('error')
})

it('marks only a successfully resolved empty loader set as unsupported', () => {
    expect(loaderSupportState('success', manifest('1.20.1', ['47.4.22']), '1.20.1')).toBe(
        'supported',
    )
    expect(loaderSupportState('success', manifest('unsupported', []), 'unsupported')).toBe(
        'unsupported',
    )
})

it('restores loader support state across Minecraft version changes', () => {
    const cache = {
        [loaderMetadataCacheKey('forge', '1.20.1')]: manifest('1.20.1', ['47.4.22']),
        [loaderMetadataCacheKey('forge', 'unsupported')]: manifest('unsupported', []),
    }

    const support = (gameVersion: string) =>
        loaderSupportState(
            'success',
            cache[loaderMetadataCacheKey('forge', gameVersion) as keyof typeof cache],
            gameVersion,
        )

    expect(support('1.20.1')).toBe('supported')
    expect(support('unsupported')).toBe('unsupported')
    expect(support('1.20.1')).toBe('supported')
})

it('disables loader chips until compatibility is positively confirmed', () => {
    expect(isLoaderSupportStateDisabled('unknown')).toBe(true)
    expect(isLoaderSupportStateDisabled('loading')).toBe(true)
    expect(isLoaderSupportStateDisabled('unsupported')).toBe(true)
    expect(isLoaderSupportStateDisabled('supported')).toBe(false)
    expect(isLoaderSupportStateDisabled('error')).toBe(true)
})

it('does not replace an explicit Minecraft version when loader options change', () => {
    expect(preserveOrSelectGameVersion('26.2', ['26.1.2', '1.21.11'])).toBe('26.2')
    expect(preserveOrSelectGameVersion(null, ['26.1.2', '1.21.11'])).toBe('26.1.2')
    expect(preserveOrSelectGameVersion(null, [])).toBe(null)
})

it('tracks loader chips through pending, supported, and unsupported responses', () => {
    const pendingB = loaderSupportState('loading', undefined, '26.2')
    expect(isLoaderSupportStateDisabled(pendingB)).toBe(true)

    const unsupportedB = loaderSupportState('success', manifest('unsupported', []), 'unsupported')
    expect(unsupportedB).toBe('unsupported')
    expect(isLoaderSupportStateDisabled(unsupportedB)).toBe(true)

    const pendingC = loaderSupportState('loading', undefined, '1.20.1')
    expect(isLoaderSupportStateDisabled(pendingC)).toBe(true)

    const supportedC = loaderSupportState('success', manifest('1.20.1', ['47.4.22']), '1.20.1')
    expect(supportedC).toBe('supported')
    expect(isLoaderSupportStateDisabled(supportedC)).toBe(false)
})

it('uses the loading label for both loader version selector placeholders while pending', () => {
    expect(
        loaderVersionSelectorText(true, false, {
            loading: 'Loading',
            empty: 'No versions available',
            placeholder: 'Select loader version',
            searchPlaceholder: 'Search loader version...',
        }),
    ).toEqual({ placeholder: 'Loading', searchPlaceholder: 'Loading' })
})

it('hides an old loader version behind the loading summary until the new version resolves', () => {
    expect(loaderVersionSummaryState(true, '47.4.22')).toBe('loading')
    expect(loaderVersionSummaryState(false, '65.1.0')).toBe('selected')
    expect(loaderVersionSummaryState(false, null)).toBe('empty')
})

it('shows a loader-specific message after game version metadata resolves empty', () => {
    const labels = {
        loading: 'Loading',
        empty: 'No game versions support this loader',
        error: 'Failed to load game versions',
        placeholder: 'Select game version',
        searchPlaceholder: 'Search game version...',
    }

    expect(gameVersionSelectorText('loading', labels)).toEqual({
        placeholder: 'Loading',
        searchPlaceholder: 'Loading',
    })
    expect(gameVersionSelectorText('empty', labels)).toEqual({
        placeholder: 'No game versions support this loader',
        searchPlaceholder: 'No game versions support this loader',
    })
    expect(gameVersionSelectorText('error', labels)).toEqual({
        placeholder: 'Failed to load game versions',
        searchPlaceholder: 'Failed to load game versions',
    })
})

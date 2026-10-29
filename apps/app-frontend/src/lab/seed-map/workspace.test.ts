import { expect, it } from 'vitest'

import {
    compareSeedMapVersions,
    fallbackSeedMapProfiles,
    SEED_MAP_DEFAULT_VERSION,
    seedMapTileConcurrency,
} from './backend.ts'
import { SEED_MAP_BIOME_NAMES, SEED_MAP_BIOMES, seedMapBiomeGroups } from './biomes.ts'
import { featureMask, SEED_MAP_FEATURES, visibleFeatureDefinitions } from './features.ts'
import {
    applyShareQuery,
    createDefaultSeedMapWorkspace,
    createShareQuery,
    isCurrentSeedMapEpoch,
    sanitizeSeedMapWorkspace,
    SEED_MAP_MIN_ZOOM,
} from './workspace.ts'

it('seed map starts near spawn with a lightweight layer set', () => {
    const workspace = createDefaultSeedMapWorkspace()
    expect(workspace.zoom).toBe(1)
    expect(workspace.terrainEstimation).toBe(true)
    expect(workspace.gameVersion).toBe(SEED_MAP_DEFAULT_VERSION)
    expect(workspace.elevation).toBe(128)
    expect(workspace.visibleFeatures).toEqual([
        'village',
        'ruined-portal',
        'stronghold',
        'fortress',
        'bastion',
        'end-city',
    ])
    expect(workspace.displayMode).toBe('structures')
    expect(workspace.selectedOres).toEqual(['diamond'])
})

it('game version choices sort newest releases first and beta releases last', () => {
    const versions = [
        'b1.7',
        '1.9.4',
        '1.21.9',
        '26.1',
        '1.20',
        '1.21.11',
        '26.1.2',
        'b1.8',
        '26.3',
        '26.2',
    ]
    expect(versions.sort(compareSeedMapVersions)).toEqual([
        '26.3',
        '26.2',
        '26.1.2',
        '26.1',
        '1.21.11',
        '1.21.9',
        '1.20',
        '1.9.4',
        'b1.8',
        'b1.7',
    ])
})

it('removed end-island layer is discarded from saved selections while other data survives', () => {
    const state = sanitizeSeedMapWorkspace({
        ...createDefaultSeedMapWorkspace(),
        visibleFeatures: ['end-island', 'end-city', 'nether-fossil', 'abandoned-camp'],
        completedFeatures: ['end-island:1200:1300', 'end-city:1500:1600'],
        markers: [{ id: 'home', name: 'Home', x: 12, z: -34, color: '#22C55E' }],
    })
    expect(state.visibleFeatures).toEqual(['end-city', 'nether-fossil', 'abandoned-camp'])
    expect(featureMask(['end-island'] as never[])).toBe(0)
    expect(state.completedFeatures.length).toBe(2)
    expect(state.markers[0].name).toBe('Home')
})

it('seed map workspace recovers from invalid persisted values', () => {
    const state = sanitizeSeedMapWorkspace({
        seed: '  mushroom island  ',
        edition: 'java',
        zoom: 99,
        center: { x: Number.NaN, z: 72 },
        visibleFeatures: ['village', 'invalid'],
    })
    expect(state.seed).toBe('mushroom island')
    expect(state.zoom).toBe(4)
    expect(state.center).toEqual({ x: 0, z: 72 })
    expect(state.visibleFeatures).toEqual(['village'])
    expect(state.showSpawn).toBe(true)
    expect(state.highlightedBiomes).toEqual([])
})

it('seed map uses fixed surface and underground elevation slices', () => {
    expect(sanitizeSeedMapWorkspace({ elevation: 62 }).elevation).toBe(128)
    expect(sanitizeSeedMapWorkspace({ elevation: 128 }).elevation).toBe(128)
    expect(sanitizeSeedMapWorkspace({ elevation: 16 }).elevation).toBe(16)
    expect(sanitizeSeedMapWorkspace({ elevation: -32 }).elevation).toBe(-32)
})

it('seed map keeps every offered ore kind and drops unknown ones', () => {
    const state = sanitizeSeedMapWorkspace({
        ...createDefaultSeedMapWorkspace(),
        selectedOres: ['diamond', 'iron_vein', 'copper_vein', 'coal', 'bogus'],
    })
    expect(state.selectedOres).toEqual(['diamond', 'iron_vein', 'copper_vein', 'coal'])
})

it('seed map supports a closer maximum zoom', () => {
    const close = sanitizeSeedMapWorkspace({ zoom: SEED_MAP_MIN_ZOOM })
    const tooClose = sanitizeSeedMapWorkspace({ zoom: SEED_MAP_MIN_ZOOM - 10 })
    expect(close.zoom).toBe(SEED_MAP_MIN_ZOOM)
    expect(tooClose.zoom).toBe(SEED_MAP_MIN_ZOOM)
})

it('seed map exposes every categorized cubiomes biome exactly once', () => {
    const groups = seedMapBiomeGroups()
    expect(SEED_MAP_BIOMES.length).toBe(Object.keys(SEED_MAP_BIOME_NAMES).length)
    expect(groups.length).toBe(17)
    expect(groups.flatMap((group) => group.biomes).length).toBe(SEED_MAP_BIOMES.length)
    expect(new Set(SEED_MAP_BIOMES.map((biome) => biome.id)).size).toBe(SEED_MAP_BIOMES.length)
    expect(
        groups.filter((group) => group.dimension === 'nether').map((group) => group.category),
    ).toEqual(['nether'])
    expect(
        groups.filter((group) => group.dimension === 'end').map((group) => group.category),
    ).toEqual(['end'])
    expect(SEED_MAP_BIOMES.filter((biome) => !SEED_MAP_BIOME_NAMES[biome.id])).toEqual([])
})

it('seed map migrates legacy biome highlighting and terrain defaults', () => {
    const state = sanitizeSeedMapWorkspace({
        version: 1,
        highlightBiomeEnabled: true,
        highlightedBiome: 21,
        terrainEstimation: false,
    })
    expect(state.highlightedBiomes).toEqual([21])
    expect(state.highlightBiomeEnabled).toBe(true)
    expect(state.terrainEstimation).toBe(true)
    expect(state.version).toBe(4)
})

it('seed map share queries preserve useful map state', () => {
    const workspace = createDefaultSeedMapWorkspace()
    const shared = applyShareQuery(
        {
            seed: '1234',
            edition: 'java-large-biomes',
            version: '1.21.1',
            dimension: 'nether',
            mode: 'ores',
            ores: 'netherite',
            yMin: '12',
            yMax: '24',
            x: '-500',
            z: '312',
            zoom: '1.25',
        },
        workspace,
    )
    expect(shared.seed).toBe('1234')
    expect(shared.edition).toBe('java-large-biomes')
    expect(shared.gameVersion).toBe('1.21.1')
    expect(shared.dimension).toBe('nether')
    expect(shared.displayMode).toBe('ores')
    expect(shared.selectedOres).toEqual(['netherite'])
    expect(shared.oreYMin).toBe(12)
    expect(shared.oreYMax).toBe(24)
    expect(shared.zoom).toBe(1.25)
    expect(shared.center).toEqual({ x: -500, z: 312 })
})

it('fallback profiles gate dimensions, ores, and large biomes by version', () => {
    const profiles = fallbackSeedMapProfiles()
    const modern = profiles.find(
        (profile) => profile.edition === 'java' && profile.version === '1.21.3',
    )
    expect(modern?.available).toBe(false)
    expect(modern?.dimensions).toEqual(['overworld', 'nether', 'end'])
    expect(modern?.ores).toBe(true)
    const legacy = profiles.find(
        (profile) => profile.edition === 'java' && profile.version === '1.9',
    )
    expect(legacy?.dimensions).toEqual(['overworld', 'end'])
    expect(legacy?.ores).toBe(false)
    const oldest = profiles.find(
        (profile) => profile.edition === 'java' && profile.version === '1.8',
    )
    expect(oldest?.dimensions).toEqual(['overworld'])
    expect(
        !profiles.some(
            (profile) => profile.edition === 'java-large-biomes' && profile.version === '1.0',
        ),
    ).toBeTruthy()
})

it('feature masks match the selected map layers', () => {
    const mask = featureMask(['village', 'stronghold', 'slime-chunk'])
    const expected = SEED_MAP_FEATURES.filter((feature) =>
        ['village', 'stronghold', 'slime-chunk'].includes(feature.kind),
    ).reduce((value, feature) => value | feature.mask, 0)
    expect(mask).toBe(expected)
})

it('seed map ignores a response from an older tile request epoch', () => {
    expect(isCurrentSeedMapEpoch(8, 8)).toBe(true)
    expect(isCurrentSeedMapEpoch(7, 8)).toBe(false)
})

it('seed map tile concurrency reserves capacity for the interface', () => {
    expect(seedMapTileConcurrency(4, 4)).toBe(2)
    expect(seedMapTileConcurrency(4, 8)).toBe(6)
    expect(seedMapTileConcurrency(4, 16)).toBe(8)
    expect(seedMapTileConcurrency(1, 16)).toBe(4)
})

it('dense feature layers are omitted until the map is close enough', () => {
    const close = visibleFeatureDefinitions(
        ['slime-chunk', 'village', 'stronghold'],
        'overworld',
        4,
    )
    const medium = visibleFeatureDefinitions(
        ['slime-chunk', 'village', 'stronghold'],
        'overworld',
        64,
    )
    const far = visibleFeatureDefinitions(
        ['slime-chunk', 'village', 'stronghold'],
        'overworld',
        256,
    )
    expect(close.map((feature) => feature.kind)).toEqual(['slime-chunk', 'village', 'stronghold'])
    expect(medium.map((feature) => feature.kind)).toEqual(['stronghold'])
    expect(far).toEqual([])
})

it('slime chunks are the first layer in every dimension', () => {
    for (const dimension of ['overworld', 'nether', 'end'] as const) {
        const visible = visibleFeatureDefinitions(
            ['village', 'fortress', 'end-city', 'slime-chunk'],
            dimension,
            4,
        )
        expect(visible[0]?.kind).toBe('slime-chunk')
    }
})

it('v3 migration retains version, markers and completion records', () => {
    const previous = {
        ...createDefaultSeedMapWorkspace(),
        version: 3,
        gameVersion: '26.2',
        markers: [{ id: 'camp', name: 'Home', x: -20, z: 42, color: '#FF0000' }],
        completedFeatures: ['village:1:2'],
        completedOres: ['diamond:3:-40:5'],
    }
    const migrated = sanitizeSeedMapWorkspace(previous)
    expect(migrated.version).toBe(4)
    expect(migrated.gameVersion).toBe('26.2')
    expect(migrated.markers).toEqual(
        previous.markers.map((marker) => ({ ...marker, dimension: 'overworld' })),
    )
    expect(migrated.completedFeatures).toEqual(previous.completedFeatures)
    expect(migrated.completedOres).toEqual(previous.completedOres)
})

it('markers preserve dimension on reload and legacy or invalid dimensions default to overworld', () => {
    const marker = { id: 'camp', name: 'Camp', x: -20, z: 42, color: '#FF0000' }
    const workspace = sanitizeSeedMapWorkspace({
        ...createDefaultSeedMapWorkspace(),
        dimension: 'end',
        markers: [
            marker,
            { ...marker, id: 'nether', dimension: 'nether' },
            { ...marker, id: 'end', dimension: 'end' },
            { ...marker, id: 'invalid', dimension: 'unknown' },
        ],
    })
    expect(workspace.markers.map((item) => item.dimension)).toEqual([
        'overworld',
        'nether',
        'end',
        'overworld',
    ])
    expect(sanitizeSeedMapWorkspace(JSON.parse(JSON.stringify(workspace))).markers).toEqual(
        workspace.markers,
    )
})

it('sharing underground view preserves height and individual layers', () => {
    const workspace = {
        ...createDefaultSeedMapWorkspace(),
        displayMode: 'underground' as const,
        undergroundY: -32,
        undergroundLayers: 5,
    }
    const restored = applyShareQuery(createShareQuery(workspace))
    expect(restored.displayMode).toBe('underground')
    expect(restored.undergroundY).toBe(-32)
    expect(restored.undergroundLayers).toBe(5)
})

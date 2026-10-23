import type { SeedMapDimension } from './biomes.ts'

export type SeedMapFeatureKind =
    | 'village'
    | 'outpost'
    | 'shipwreck'
    | 'monument'
    | 'mansion'
    | 'ancient-city'
    | 'trail-ruins'
    | 'trial-chambers'
    | 'ruined-portal'
    | 'stronghold'
    | 'slime-chunk'
    | 'desert-pyramid'
    | 'jungle-temple'
    | 'swamp-hut'
    | 'igloo'
    | 'ocean-ruin'
    | 'buried-treasure'
    | 'mineshaft'
    | 'desert-well'
    | 'geode'
    | 'fortress'
    | 'bastion'
    | 'end-city'
    | 'end-gateway'
    | 'abandoned-camp'
    | 'nether-fossil'

export type SeedMapFeature = {
    kind: SeedMapFeatureKind
    x: number
    z: number
    approximate: boolean
    endShip?: boolean
}

export type SeedMapFeatureDefinition = {
    kind: SeedMapFeatureKind
    mask: number
    dimensions: readonly SeedMapDimension[]
    maxScale: number
}

const OVERWORLD: readonly SeedMapDimension[] = ['overworld']
const NETHER: readonly SeedMapDimension[] = ['nether']
const END: readonly SeedMapDimension[] = ['end']
const ALL_DIMENSIONS: readonly SeedMapDimension[] = ['overworld', 'nether', 'end']
const OVERWORLD_AND_NETHER: readonly SeedMapDimension[] = ['overworld', 'nether']

/**
 * Map layers in display order. `maxScale` bounds how far the map can zoom out
 * before a dense layer stops being queried and drawn.
 */
export const SEED_MAP_FEATURES: readonly SeedMapFeatureDefinition[] = [
    { kind: 'slime-chunk', mask: 1 << 10, dimensions: ALL_DIMENSIONS, maxScale: 4 },
    { kind: 'village', mask: 1 << 0, dimensions: OVERWORLD, maxScale: 16 },
    { kind: 'outpost', mask: 1 << 1, dimensions: OVERWORLD, maxScale: 64 },
    { kind: 'shipwreck', mask: 1 << 2, dimensions: OVERWORLD, maxScale: 16 },
    { kind: 'monument', mask: 1 << 3, dimensions: OVERWORLD, maxScale: 16 },
    { kind: 'mansion', mask: 1 << 4, dimensions: OVERWORLD, maxScale: 64 },
    { kind: 'ancient-city', mask: 1 << 5, dimensions: OVERWORLD, maxScale: 16 },
    { kind: 'trail-ruins', mask: 1 << 6, dimensions: OVERWORLD, maxScale: 16 },
    { kind: 'trial-chambers', mask: 1 << 7, dimensions: OVERWORLD, maxScale: 4 },
    { kind: 'ruined-portal', mask: 1 << 8, dimensions: OVERWORLD_AND_NETHER, maxScale: 4 },
    { kind: 'stronghold', mask: 1 << 9, dimensions: OVERWORLD, maxScale: 64 },
    { kind: 'desert-pyramid', mask: 1 << 11, dimensions: OVERWORLD, maxScale: 16 },
    { kind: 'jungle-temple', mask: 1 << 12, dimensions: OVERWORLD, maxScale: 16 },
    { kind: 'swamp-hut', mask: 1 << 13, dimensions: OVERWORLD, maxScale: 16 },
    { kind: 'igloo', mask: 1 << 14, dimensions: OVERWORLD, maxScale: 16 },
    { kind: 'ocean-ruin', mask: 1 << 15, dimensions: OVERWORLD, maxScale: 4 },
    { kind: 'buried-treasure', mask: 1 << 16, dimensions: OVERWORLD, maxScale: 16 },
    { kind: 'mineshaft', mask: 1 << 17, dimensions: OVERWORLD, maxScale: 4 },
    { kind: 'desert-well', mask: 1 << 18, dimensions: OVERWORLD, maxScale: 16 },
    { kind: 'geode', mask: 1 << 19, dimensions: OVERWORLD, maxScale: 4 },
    { kind: 'fortress', mask: 1 << 20, dimensions: NETHER, maxScale: 64 },
    { kind: 'bastion', mask: 1 << 21, dimensions: NETHER, maxScale: 16 },
    { kind: 'end-city', mask: 1 << 22, dimensions: END, maxScale: 16 },
    { kind: 'end-gateway', mask: 1 << 23, dimensions: END, maxScale: 4 },
    { kind: 'abandoned-camp', mask: 1 << 24, dimensions: OVERWORLD, maxScale: 16 },
    { kind: 'nether-fossil', mask: 1 << 25, dimensions: NETHER, maxScale: 4 },
]

export function visibleFeatureDefinitions(
    kinds: SeedMapFeatureKind[],
    dimension: SeedMapDimension,
    scale: number,
): SeedMapFeatureDefinition[] {
    return SEED_MAP_FEATURES.filter(
        (feature) =>
            kinds.includes(feature.kind) &&
            feature.dimensions.includes(dimension) &&
            scale <= feature.maxScale,
    )
}

export function featureMask(kinds: SeedMapFeatureKind[]): number {
    return SEED_MAP_FEATURES.reduce(
        (mask, feature) => (kinds.includes(feature.kind) ? mask | feature.mask : mask),
        0,
    )
}

export function featureKey(feature: Pick<SeedMapFeature, 'kind' | 'x' | 'z'>): string {
    return `${feature.kind}:${feature.x}:${feature.z}`
}

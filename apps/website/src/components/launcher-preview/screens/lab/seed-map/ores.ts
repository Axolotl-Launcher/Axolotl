import type { SeedMapDimension } from './biomes.ts'

export type SeedMapDisplayMode = 'structures' | 'ores' | 'underground'

export type SeedMapOreKind =
    | 'diamond'
    | 'iron'
    | 'iron_vein'
    | 'copper'
    | 'copper_vein'
    | 'gold'
    | 'redstone'
    | 'lapis'
    | 'coal'
    | 'netherite'
    | 'emerald'
    | 'quartz'
    | 'nether_gold'
    | 'andesite'
    | 'diorite'
    | 'granite'
    | 'dirt'
    | 'gravel'
    | 'infested'
    | 'deepslate'
    | 'tuff'
    | 'clay'
    | 'blackstone'
    | 'magma'
    | 'soul_sand'

export type SeedMapOreDefinition = {
    kind: SeedMapOreKind
    dimension: Exclude<SeedMapDimension, 'end'>
    yMin: number
    yMax: number
    bandRows: number
    image: string
    texture: string
    deepslateTexture?: string
}

export type SeedMapOreHit = {
    ore: SeedMapOreKind
    x: number
    y: number
    z: number
    yMin: number
    yMax: number
    status: 'terrain_unverified'
}

export type SeedMapOreChunk = {
    cx: number
    cz: number
}

const ORE_ASSET_ROOT = '/launcher-preview/seed-map-assets/ores'

export const SEED_MAP_ORE_MAX_SCALE = 0.25
export const SEED_MAP_ORE_CACHE_LIMIT = 6_000
export const SEED_MAP_ORE_CACHE_TARGET = 5_400

/**
 * Ore candidates use the bundled engine's versioned configurations. The
 * `bandRows` value reflects each distribution's Y span and scales the scan
 * budget so wide bands do not overwhelm a single pass.
 */
export const SEED_MAP_ORES: readonly SeedMapOreDefinition[] = [
    {
        kind: 'diamond',
        dimension: 'overworld',
        yMin: -144,
        yMax: 16,
        bandRows: 160,
        image: `${ORE_ASSET_ROOT}/diamond-ore.png`,
        texture: `${ORE_ASSET_ROOT}/ore-flat/diamond_ore.png`,
        deepslateTexture: `${ORE_ASSET_ROOT}/ore-flat/deepslate_diamond_ore.png`,
    },
    {
        kind: 'iron',
        dimension: 'overworld',
        yMin: -64,
        yMax: 319,
        bandRows: 384,
        image: `${ORE_ASSET_ROOT}/iron-ore.png`,
        texture: `${ORE_ASSET_ROOT}/ore-flat/iron_ore.png`,
        deepslateTexture: `${ORE_ASSET_ROOT}/ore-flat/deepslate_iron_ore.png`,
    },
    {
        kind: 'iron_vein',
        dimension: 'overworld',
        yMin: -60,
        yMax: -8,
        bandRows: 128,
        image: `${ORE_ASSET_ROOT}/block-of-raw-iron.png`,
        texture: `${ORE_ASSET_ROOT}/ore-flat/iron_ore.png`,
        deepslateTexture: `${ORE_ASSET_ROOT}/ore-flat/deepslate_iron_ore.png`,
    },
    {
        kind: 'copper',
        dimension: 'overworld',
        yMin: -16,
        yMax: 112,
        bandRows: 144,
        image: `${ORE_ASSET_ROOT}/copper-ore.png`,
        texture: `${ORE_ASSET_ROOT}/ore-flat/copper_ore.png`,
        deepslateTexture: `${ORE_ASSET_ROOT}/ore-flat/deepslate_copper_ore.png`,
    },
    {
        kind: 'copper_vein',
        dimension: 'overworld',
        yMin: 0,
        yMax: 50,
        bandRows: 128,
        image: `${ORE_ASSET_ROOT}/block-of-raw-copper.png`,
        texture: `${ORE_ASSET_ROOT}/ore-flat/copper_ore.png`,
        deepslateTexture: `${ORE_ASSET_ROOT}/ore-flat/deepslate_copper_ore.png`,
    },
    {
        kind: 'gold',
        dimension: 'overworld',
        yMin: -64,
        yMax: 32,
        bandRows: 104,
        image: `${ORE_ASSET_ROOT}/gold-ore.png`,
        texture: `${ORE_ASSET_ROOT}/ore-flat/gold_ore.png`,
        deepslateTexture: `${ORE_ASSET_ROOT}/ore-flat/deepslate_gold_ore.png`,
    },
    {
        kind: 'redstone',
        dimension: 'overworld',
        yMin: -96,
        yMax: 15,
        bandRows: 112,
        image: `${ORE_ASSET_ROOT}/redstone-ore.png`,
        texture: `${ORE_ASSET_ROOT}/ore-flat/redstone_ore.png`,
        deepslateTexture: `${ORE_ASSET_ROOT}/ore-flat/deepslate_redstone_ore.png`,
    },
    {
        kind: 'lapis',
        dimension: 'overworld',
        yMin: -64,
        yMax: 64,
        bandRows: 136,
        image: `${ORE_ASSET_ROOT}/lapis-lazuli-ore.png`,
        texture: `${ORE_ASSET_ROOT}/ore-flat/lapis_ore.png`,
        deepslateTexture: `${ORE_ASSET_ROOT}/ore-flat/deepslate_lapis_ore.png`,
    },
    {
        kind: 'coal',
        dimension: 'overworld',
        yMin: 0,
        yMax: 319,
        bandRows: 328,
        image: `${ORE_ASSET_ROOT}/coal-ore.png`,
        texture: `${ORE_ASSET_ROOT}/ore-flat/coal_ore.png`,
        deepslateTexture: `${ORE_ASSET_ROOT}/ore-flat/deepslate_coal_ore.png`,
    },
    {
        kind: 'netherite',
        dimension: 'nether',
        yMin: 8,
        yMax: 119,
        bandRows: 128,
        image: `${ORE_ASSET_ROOT}/ancient-debris.png`,
        texture: `${ORE_ASSET_ROOT}/ore-flat/ancient_debris_side.png`,
    },
    {
        kind: 'emerald',
        dimension: 'overworld',
        yMin: -64,
        yMax: 319,
        bandRows: 384,
        image: 'minecraft:emerald_ore',
        texture: 'minecraft:emerald_ore',
    },
    {
        kind: 'quartz',
        dimension: 'nether',
        yMin: 0,
        yMax: 127,
        bandRows: 128,
        image: 'minecraft:nether_quartz_ore',
        texture: 'minecraft:nether_quartz_ore',
    },
    {
        kind: 'nether_gold',
        dimension: 'nether',
        yMin: 0,
        yMax: 127,
        bandRows: 128,
        image: 'minecraft:nether_gold_ore',
        texture: 'minecraft:nether_gold_ore',
    },
    {
        kind: 'andesite',
        dimension: 'overworld',
        yMin: -64,
        yMax: 319,
        bandRows: 384,
        image: 'minecraft:andesite',
        texture: 'minecraft:andesite',
    },
    {
        kind: 'diorite',
        dimension: 'overworld',
        yMin: -64,
        yMax: 319,
        bandRows: 384,
        image: 'minecraft:diorite',
        texture: 'minecraft:diorite',
    },
    {
        kind: 'granite',
        dimension: 'overworld',
        yMin: -64,
        yMax: 319,
        bandRows: 384,
        image: 'minecraft:granite',
        texture: 'minecraft:granite',
    },
    {
        kind: 'dirt',
        dimension: 'overworld',
        yMin: -64,
        yMax: 319,
        bandRows: 384,
        image: 'minecraft:dirt',
        texture: 'minecraft:dirt',
    },
    {
        kind: 'gravel',
        dimension: 'overworld',
        yMin: -64,
        yMax: 319,
        bandRows: 384,
        image: 'minecraft:gravel',
        texture: 'minecraft:gravel',
    },
    {
        kind: 'infested',
        dimension: 'overworld',
        yMin: -64,
        yMax: 319,
        bandRows: 384,
        image: 'minecraft:infested_stone',
        texture: 'minecraft:infested_stone',
    },
    {
        kind: 'deepslate',
        dimension: 'overworld',
        yMin: -64,
        yMax: 319,
        bandRows: 384,
        image: 'minecraft:deepslate',
        texture: 'minecraft:deepslate',
    },
    {
        kind: 'tuff',
        dimension: 'overworld',
        yMin: -64,
        yMax: 319,
        bandRows: 384,
        image: 'minecraft:tuff',
        texture: 'minecraft:tuff',
    },
    {
        kind: 'clay',
        dimension: 'overworld',
        yMin: -64,
        yMax: 319,
        bandRows: 384,
        image: 'minecraft:clay',
        texture: 'minecraft:clay',
    },
    {
        kind: 'blackstone',
        dimension: 'nether',
        yMin: 0,
        yMax: 127,
        bandRows: 128,
        image: 'minecraft:blackstone',
        texture: 'minecraft:blackstone',
    },
    {
        kind: 'magma',
        dimension: 'nether',
        yMin: 0,
        yMax: 127,
        bandRows: 128,
        image: 'minecraft:magma_block',
        texture: 'minecraft:magma_block',
    },
    {
        kind: 'soul_sand',
        dimension: 'nether',
        yMin: 0,
        yMax: 127,
        bandRows: 128,
        image: 'minecraft:soul_sand',
        texture: 'minecraft:soul_sand',
    },
]

const ORE_BY_KIND = new Map(SEED_MAP_ORES.map((ore) => [ore.kind, ore]))

export function seedMapOreDefinition(kind: SeedMapOreKind): SeedMapOreDefinition {
    const definition = ORE_BY_KIND.get(kind)
    if (!definition) throw new Error(`Unknown seed-map ore: ${kind}`)
    return definition
}

export function seedMapOresForDimension(dimension: SeedMapDimension): SeedMapOreDefinition[] {
    if (dimension === 'end') return []
    return SEED_MAP_ORES.filter((ore) => ore.dimension === dimension)
}

export function seedMapOreScanBudget(kinds: readonly SeedMapOreKind[]): number {
    const maxBandRows = Math.max(1, ...kinds.map((kind) => seedMapOreDefinition(kind).bandRows))
    return Math.max(60, Math.round(33_600 / maxBandRows))
}

export function seedMapOreYRange(kinds: readonly SeedMapOreKind[]): [number, number] {
    if (kinds.length === 0) return [-64, 319]
    return [
        Math.min(...kinds.map((kind) => seedMapOreDefinition(kind).yMin)),
        Math.max(...kinds.map((kind) => seedMapOreDefinition(kind).yMax)),
    ]
}

export function seedMapOreKey(hit: SeedMapOreHit): string {
    return `${hit.ore}:${hit.x}:${hit.y}:${hit.z}`
}

export function seedMapOreColumnKey(hit: Pick<SeedMapOreHit, 'ore' | 'x' | 'z'>): string {
    return `${hit.ore}:${hit.x}:${hit.z}`
}

export function preferSeedMapOreHit(
    current: SeedMapOreHit | undefined,
    candidate: SeedMapOreHit,
): SeedMapOreHit {
    if (!current) return candidate
    const currentRange = current.yMax - current.yMin
    const candidateRange = candidate.yMax - candidate.yMin
    return candidateRange < currentRange ? candidate : current
}

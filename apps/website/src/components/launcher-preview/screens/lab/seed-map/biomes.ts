import { defineMessages } from '@modrinth/ui/src/composables/i18n.ts'

export type SeedMapDimension = 'overworld' | 'nether' | 'end'

export type SeedMapBiomeCategory =
    | 'beach'
    | 'cave'
    | 'desert'
    | 'forest'
    | 'ice'
    | 'jungle'
    | 'mesa'
    | 'mountains'
    | 'mushroom'
    | 'ocean'
    | 'plains'
    | 'river'
    | 'savanna'
    | 'swamp'
    | 'taiga'
    | 'nether'
    | 'end'

export type SeedMapBiome = {
    id: number
    dimensions: readonly SeedMapDimension[]
    category: SeedMapBiomeCategory
    color: string
}

export type SeedMapBiomeGroup = {
    category: SeedMapBiomeCategory
    dimension: SeedMapDimension
    biomes: SeedMapBiome[]
}

const OVERWORLD: readonly SeedMapDimension[] = ['overworld']
const NETHER: readonly SeedMapDimension[] = ['nether']
const END: readonly SeedMapDimension[] = ['end']

/**
 * Java biome ids as used by cubiomes, grouped for the picker. Colors mirror
 * the native renderer's cubiomes palette so the picker and map stay aligned.
 */
export const SEED_MAP_BIOMES: readonly SeedMapBiome[] = [
    { id: 16, dimensions: OVERWORLD, category: 'beach', color: '#FADE55' },
    { id: 26, dimensions: OVERWORLD, category: 'beach', color: '#FAF0C0' },
    { id: 25, dimensions: OVERWORLD, category: 'beach', color: '#A2A284' },
    { id: 183, dimensions: OVERWORLD, category: 'cave', color: '#031F29' },
    { id: 174, dimensions: OVERWORLD, category: 'cave', color: '#4E3012' },
    { id: 175, dimensions: OVERWORLD, category: 'cave', color: '#283C00' },
    { id: 187, dimensions: OVERWORLD, category: 'cave', color: '#C8D232' },
    { id: 2, dimensions: OVERWORLD, category: 'desert', color: '#FA9418' },
    { id: 27, dimensions: OVERWORLD, category: 'forest', color: '#307444' },
    { id: 185, dimensions: OVERWORLD, category: 'forest', color: '#FF91C8' },
    { id: 29, dimensions: OVERWORLD, category: 'forest', color: '#40511A' },
    { id: 132, dimensions: OVERWORLD, category: 'forest', color: '#2D8E49' },
    { id: 4, dimensions: OVERWORLD, category: 'forest', color: '#056621' },
    { id: 178, dimensions: OVERWORLD, category: 'forest', color: '#47726C' },
    { id: 155, dimensions: OVERWORLD, category: 'forest', color: '#589C6C' },
    { id: 186, dimensions: OVERWORLD, category: 'forest', color: '#696D95' },
    { id: 188, dimensions: OVERWORLD, category: 'forest', color: '#EB9234' },
    { id: 34, dimensions: OVERWORLD, category: 'forest', color: '#5B7352' },
    { id: 181, dimensions: OVERWORLD, category: 'ice', color: '#B0B3CE' },
    { id: 11, dimensions: OVERWORLD, category: 'ice', color: '#A0A0FF' },
    { id: 140, dimensions: OVERWORLD, category: 'ice', color: '#B4DCDC' },
    { id: 168, dimensions: OVERWORLD, category: 'jungle', color: '#849500' },
    { id: 21, dimensions: OVERWORLD, category: 'jungle', color: '#507B0A' },
    { id: 23, dimensions: OVERWORLD, category: 'jungle', color: '#60930F' },
    { id: 37, dimensions: OVERWORLD, category: 'mesa', color: '#D94515' },
    { id: 165, dimensions: OVERWORLD, category: 'mesa', color: '#FF6D3D' },
    { id: 38, dimensions: OVERWORLD, category: 'mesa', color: '#B09765' },
    { id: 180, dimensions: OVERWORLD, category: 'mountains', color: '#DCDCC8' },
    { id: 177, dimensions: OVERWORLD, category: 'mountains', color: '#60A445' },
    { id: 179, dimensions: OVERWORLD, category: 'mountains', color: '#C4C4C4' },
    { id: 182, dimensions: OVERWORLD, category: 'mountains', color: '#7B8F74' },
    { id: 131, dimensions: OVERWORLD, category: 'mountains', color: '#888888' },
    { id: 3, dimensions: OVERWORLD, category: 'mountains', color: '#606060' },
    { id: 14, dimensions: OVERWORLD, category: 'mushroom', color: '#FF00FF' },
    { id: 46, dimensions: OVERWORLD, category: 'ocean', color: '#202070' },
    { id: 49, dimensions: OVERWORLD, category: 'ocean', color: '#202038' },
    { id: 50, dimensions: OVERWORLD, category: 'ocean', color: '#404090' },
    { id: 48, dimensions: OVERWORLD, category: 'ocean', color: '#000040' },
    { id: 24, dimensions: OVERWORLD, category: 'ocean', color: '#000030' },
    { id: 10, dimensions: OVERWORLD, category: 'ocean', color: '#7070D6' },
    { id: 45, dimensions: OVERWORLD, category: 'ocean', color: '#000090' },
    { id: 0, dimensions: OVERWORLD, category: 'ocean', color: '#000070' },
    { id: 44, dimensions: OVERWORLD, category: 'ocean', color: '#0000AC' },
    { id: 1, dimensions: OVERWORLD, category: 'plains', color: '#8DB360' },
    { id: 12, dimensions: OVERWORLD, category: 'plains', color: '#FFFFFF' },
    { id: 129, dimensions: OVERWORLD, category: 'plains', color: '#B5DB88' },
    { id: 7, dimensions: OVERWORLD, category: 'river', color: '#0000FF' },
    { id: 35, dimensions: OVERWORLD, category: 'savanna', color: '#BDB25F' },
    { id: 36, dimensions: OVERWORLD, category: 'savanna', color: '#A79D64' },
    { id: 163, dimensions: OVERWORLD, category: 'savanna', color: '#E5DA87' },
    { id: 184, dimensions: OVERWORLD, category: 'swamp', color: '#2CCC8E' },
    { id: 6, dimensions: OVERWORLD, category: 'swamp', color: '#07F9B2' },
    { id: 32, dimensions: OVERWORLD, category: 'taiga', color: '#596651' },
    { id: 160, dimensions: OVERWORLD, category: 'taiga', color: '#818E79' },
    { id: 30, dimensions: OVERWORLD, category: 'taiga', color: '#31554A' },
    { id: 5, dimensions: OVERWORLD, category: 'taiga', color: '#0B6A5F' },
    { id: 8, dimensions: NETHER, category: 'nether', color: '#572526' },
    { id: 170, dimensions: NETHER, category: 'nether', color: '#4D3A2E' },
    { id: 171, dimensions: NETHER, category: 'nether', color: '#981A11' },
    { id: 172, dimensions: NETHER, category: 'nether', color: '#49907B' },
    { id: 173, dimensions: NETHER, category: 'nether', color: '#645F63' },
    { id: 9, dimensions: END, category: 'end', color: '#8080FF' },
    { id: 40, dimensions: END, category: 'end', color: '#4B4BAB' },
    { id: 41, dimensions: END, category: 'end', color: '#C9C959' },
    { id: 42, dimensions: END, category: 'end', color: '#B5B536' },
    { id: 43, dimensions: END, category: 'end', color: '#7070CC' },
]

/** Display names for the Java biome ids shown in the picker. */
export const SEED_MAP_BIOME_NAMES: Readonly<Record<number, string>> = {
    0: 'Ocean',
    1: 'Plains',
    2: 'Desert',
    3: 'Windswept Hills',
    4: 'Forest',
    5: 'Taiga',
    6: 'Swamp',
    7: 'River',
    8: 'Nether Wastes',
    9: 'The End',
    10: 'Frozen Ocean',
    11: 'Frozen River',
    12: 'Snowy Plains',
    14: 'Mushroom Fields',
    16: 'Beach',
    21: 'Jungle',
    23: 'Sparse Jungle',
    24: 'Deep Ocean',
    25: 'Stony Shore',
    26: 'Snowy Beach',
    27: 'Birch Forest',
    29: 'Dark Forest',
    30: 'Snowy Taiga',
    32: 'Old Growth Pine Taiga',
    34: 'Windswept Forest',
    35: 'Savanna',
    36: 'Savanna Plateau',
    37: 'Badlands',
    38: 'Wooded Badlands',
    40: 'Small End Islands',
    41: 'End Midlands',
    42: 'End Highlands',
    43: 'End Barrens',
    44: 'Warm Ocean',
    45: 'Lukewarm Ocean',
    46: 'Cold Ocean',
    48: 'Deep Lukewarm Ocean',
    49: 'Deep Cold Ocean',
    50: 'Deep Frozen Ocean',
    129: 'Sunflower Plains',
    131: 'Windswept Gravelly Hills',
    132: 'Flower Forest',
    140: 'Ice Spikes',
    155: 'Old Growth Birch Forest',
    160: 'Old Growth Spruce Taiga',
    163: 'Windswept Savanna',
    165: 'Eroded Badlands',
    168: 'Bamboo Jungle',
    170: 'Soul Sand Valley',
    171: 'Crimson Forest',
    172: 'Warped Forest',
    173: 'Basalt Deltas',
    174: 'Dripstone Caves',
    175: 'Lush Caves',
    177: 'Meadow',
    178: 'Grove',
    179: 'Snowy Slopes',
    180: 'Jagged Peaks',
    181: 'Frozen Peaks',
    182: 'Stony Peaks',
    183: 'Deep Dark',
    184: 'Mangrove Swamp',
    185: 'Cherry Grove',
    186: 'Pale Garden',
    187: 'Sulfur Caves',
    188: 'Dappled Forest',
}

export function seedMapBiomeGroups(): SeedMapBiomeGroup[] {
    const groups = new Map<SeedMapBiomeCategory, SeedMapBiomeGroup>()
    for (const biome of SEED_MAP_BIOMES) {
        const dimension = biome.dimensions[0] ?? 'overworld'
        const group = groups.get(biome.category) ?? {
            category: biome.category,
            dimension,
            biomes: [],
        }
        group.biomes.push(biome)
        groups.set(biome.category, group)
    }
    return [...groups.values()]
}

export function seedMapBiomeSlug(name: string): string {
    return name.toLocaleLowerCase().replaceAll(' ', '-')
}

/** Static descriptors so FormatJS can extract biome labels (dynamic ids cannot). */
export const seedMapBiomeMessages = defineMessages({
    ocean: { id: 'app.lab.seed-map.biome.ocean', defaultMessage: 'Ocean' },
    plains: { id: 'app.lab.seed-map.biome.plains', defaultMessage: 'Plains' },
    desert: { id: 'app.lab.seed-map.biome.desert', defaultMessage: 'Desert' },
    'windswept-hills': {
        id: 'app.lab.seed-map.biome.windswept-hills',
        defaultMessage: 'Windswept Hills',
    },
    forest: { id: 'app.lab.seed-map.biome.forest', defaultMessage: 'Forest' },
    taiga: { id: 'app.lab.seed-map.biome.taiga', defaultMessage: 'Taiga' },
    swamp: { id: 'app.lab.seed-map.biome.swamp', defaultMessage: 'Swamp' },
    river: { id: 'app.lab.seed-map.biome.river', defaultMessage: 'River' },
    'nether-wastes': {
        id: 'app.lab.seed-map.biome.nether-wastes',
        defaultMessage: 'Nether Wastes',
    },
    'the-end': { id: 'app.lab.seed-map.biome.the-end', defaultMessage: 'The End' },
    'frozen-ocean': {
        id: 'app.lab.seed-map.biome.frozen-ocean',
        defaultMessage: 'Frozen Ocean',
    },
    'frozen-river': {
        id: 'app.lab.seed-map.biome.frozen-river',
        defaultMessage: 'Frozen River',
    },
    'snowy-plains': {
        id: 'app.lab.seed-map.biome.snowy-plains',
        defaultMessage: 'Snowy Plains',
    },
    'mushroom-fields': {
        id: 'app.lab.seed-map.biome.mushroom-fields',
        defaultMessage: 'Mushroom Fields',
    },
    beach: { id: 'app.lab.seed-map.biome.beach', defaultMessage: 'Beach' },
    jungle: { id: 'app.lab.seed-map.biome.jungle', defaultMessage: 'Jungle' },
    'sparse-jungle': {
        id: 'app.lab.seed-map.biome.sparse-jungle',
        defaultMessage: 'Sparse Jungle',
    },
    'deep-ocean': { id: 'app.lab.seed-map.biome.deep-ocean', defaultMessage: 'Deep Ocean' },
    'stony-shore': { id: 'app.lab.seed-map.biome.stony-shore', defaultMessage: 'Stony Shore' },
    'snowy-beach': { id: 'app.lab.seed-map.biome.snowy-beach', defaultMessage: 'Snowy Beach' },
    'birch-forest': {
        id: 'app.lab.seed-map.biome.birch-forest',
        defaultMessage: 'Birch Forest',
    },
    'dark-forest': { id: 'app.lab.seed-map.biome.dark-forest', defaultMessage: 'Dark Forest' },
    'snowy-taiga': { id: 'app.lab.seed-map.biome.snowy-taiga', defaultMessage: 'Snowy Taiga' },
    'old-growth-pine-taiga': {
        id: 'app.lab.seed-map.biome.old-growth-pine-taiga',
        defaultMessage: 'Old Growth Pine Taiga',
    },
    'windswept-forest': {
        id: 'app.lab.seed-map.biome.windswept-forest',
        defaultMessage: 'Windswept Forest',
    },
    savanna: { id: 'app.lab.seed-map.biome.savanna', defaultMessage: 'Savanna' },
    'savanna-plateau': {
        id: 'app.lab.seed-map.biome.savanna-plateau',
        defaultMessage: 'Savanna Plateau',
    },
    badlands: { id: 'app.lab.seed-map.biome.badlands', defaultMessage: 'Badlands' },
    'wooded-badlands': {
        id: 'app.lab.seed-map.biome.wooded-badlands',
        defaultMessage: 'Wooded Badlands',
    },
    'small-end-islands': {
        id: 'app.lab.seed-map.biome.small-end-islands',
        defaultMessage: 'Small End Islands',
    },
    'end-midlands': { id: 'app.lab.seed-map.biome.end-midlands', defaultMessage: 'End Midlands' },
    'end-highlands': {
        id: 'app.lab.seed-map.biome.end-highlands',
        defaultMessage: 'End Highlands',
    },
    'end-barrens': { id: 'app.lab.seed-map.biome.end-barrens', defaultMessage: 'End Barrens' },
    'warm-ocean': { id: 'app.lab.seed-map.biome.warm-ocean', defaultMessage: 'Warm Ocean' },
    'lukewarm-ocean': {
        id: 'app.lab.seed-map.biome.lukewarm-ocean',
        defaultMessage: 'Lukewarm Ocean',
    },
    'cold-ocean': { id: 'app.lab.seed-map.biome.cold-ocean', defaultMessage: 'Cold Ocean' },
    'deep-lukewarm-ocean': {
        id: 'app.lab.seed-map.biome.deep-lukewarm-ocean',
        defaultMessage: 'Deep Lukewarm Ocean',
    },
    'deep-cold-ocean': {
        id: 'app.lab.seed-map.biome.deep-cold-ocean',
        defaultMessage: 'Deep Cold Ocean',
    },
    'deep-frozen-ocean': {
        id: 'app.lab.seed-map.biome.deep-frozen-ocean',
        defaultMessage: 'Deep Frozen Ocean',
    },
    'sunflower-plains': {
        id: 'app.lab.seed-map.biome.sunflower-plains',
        defaultMessage: 'Sunflower Plains',
    },
    'windswept-gravelly-hills': {
        id: 'app.lab.seed-map.biome.windswept-gravelly-hills',
        defaultMessage: 'Windswept Gravelly Hills',
    },
    'flower-forest': {
        id: 'app.lab.seed-map.biome.flower-forest',
        defaultMessage: 'Flower Forest',
    },
    'ice-spikes': { id: 'app.lab.seed-map.biome.ice-spikes', defaultMessage: 'Ice Spikes' },
    'old-growth-birch-forest': {
        id: 'app.lab.seed-map.biome.old-growth-birch-forest',
        defaultMessage: 'Old Growth Birch Forest',
    },
    'old-growth-spruce-taiga': {
        id: 'app.lab.seed-map.biome.old-growth-spruce-taiga',
        defaultMessage: 'Old Growth Spruce Taiga',
    },
    'windswept-savanna': {
        id: 'app.lab.seed-map.biome.windswept-savanna',
        defaultMessage: 'Windswept Savanna',
    },
    'eroded-badlands': {
        id: 'app.lab.seed-map.biome.eroded-badlands',
        defaultMessage: 'Eroded Badlands',
    },
    'bamboo-jungle': {
        id: 'app.lab.seed-map.biome.bamboo-jungle',
        defaultMessage: 'Bamboo Jungle',
    },
    'soul-sand-valley': {
        id: 'app.lab.seed-map.biome.soul-sand-valley',
        defaultMessage: 'Soul Sand Valley',
    },
    'crimson-forest': {
        id: 'app.lab.seed-map.biome.crimson-forest',
        defaultMessage: 'Crimson Forest',
    },
    'warped-forest': {
        id: 'app.lab.seed-map.biome.warped-forest',
        defaultMessage: 'Warped Forest',
    },
    'basalt-deltas': {
        id: 'app.lab.seed-map.biome.basalt-deltas',
        defaultMessage: 'Basalt Deltas',
    },
    'dripstone-caves': {
        id: 'app.lab.seed-map.biome.dripstone-caves',
        defaultMessage: 'Dripstone Caves',
    },
    'lush-caves': { id: 'app.lab.seed-map.biome.lush-caves', defaultMessage: 'Lush Caves' },
    meadow: { id: 'app.lab.seed-map.biome.meadow', defaultMessage: 'Meadow' },
    grove: { id: 'app.lab.seed-map.biome.grove', defaultMessage: 'Grove' },
    'snowy-slopes': {
        id: 'app.lab.seed-map.biome.snowy-slopes',
        defaultMessage: 'Snowy Slopes',
    },
    'jagged-peaks': { id: 'app.lab.seed-map.biome.jagged-peaks', defaultMessage: 'Jagged Peaks' },
    'frozen-peaks': { id: 'app.lab.seed-map.biome.frozen-peaks', defaultMessage: 'Frozen Peaks' },
    'stony-peaks': { id: 'app.lab.seed-map.biome.stony-peaks', defaultMessage: 'Stony Peaks' },
    'deep-dark': { id: 'app.lab.seed-map.biome.deep-dark', defaultMessage: 'Deep Dark' },
    'mangrove-swamp': {
        id: 'app.lab.seed-map.biome.mangrove-swamp',
        defaultMessage: 'Mangrove Swamp',
    },
    'cherry-grove': { id: 'app.lab.seed-map.biome.cherry-grove', defaultMessage: 'Cherry Grove' },
    'pale-garden': { id: 'app.lab.seed-map.biome.pale-garden', defaultMessage: 'Pale Garden' },
    'dappled-forest': {
        id: 'app.lab.seed-map.biome.dappled-forest',
        defaultMessage: 'Dappled Forest',
    },
    'sulfur-caves': { id: 'app.lab.seed-map.biome.sulfur-caves', defaultMessage: 'Sulfur Caves' },
})

export function seedMapBiomeMessage(name: string) {
    return seedMapBiomeMessages[seedMapBiomeSlug(name) as keyof typeof seedMapBiomeMessages]
}

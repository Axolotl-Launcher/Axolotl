/**
 * Java Edition banner patterns, grouped the way the Minecraft Wiki documents
 * them. Pattern ids are the `minecraft:banner_pattern` registry ids used by the
 * `banner_patterns` item component, and each texture mask is stored in
 * `assets/banner-pattern-masks.json`.
 */
export const BANNER_PATTERN_CATEGORIES = [
    'base',
    'stripes',
    'halves',
    'shapes',
    'borders',
    'background',
    'gradients',
    'icons',
] as const

export type BannerPatternCategory = (typeof BANNER_PATTERN_CATEGORIES)[number]

export interface BannerPatternDefinition {
    id: string
    category: BannerPatternCategory
    /** Banner pattern item the loom consumes (never used up) for this pattern. */
    patternItem?: string
    /**
     * Short code the 1.8 - 1.12.2 `Patterns` list stores instead of the pattern
     * name, taken from the game's own pattern enum. 1.13 replaced these codes
     * with the registry ids, so patterns added from 1.16 on have none.
     */
    legacyCode?: string
    /**
     * First game version that ships this pattern. Omitted for the 1.13 base set;
     * older versions have no such registry entry and drop the pattern silently.
     */
    since?: string
}

const BANNER_PATTERN_DEFINITIONS = [
    { id: 'base', category: 'base', legacyCode: 'b' },
    { id: 'stripe_bottom', category: 'stripes', legacyCode: 'bs' },
    { id: 'stripe_top', category: 'stripes', legacyCode: 'ts' },
    { id: 'stripe_left', category: 'stripes', legacyCode: 'ls' },
    { id: 'stripe_right', category: 'stripes', legacyCode: 'rs' },
    { id: 'stripe_center', category: 'stripes', legacyCode: 'cs' },
    { id: 'stripe_middle', category: 'stripes', legacyCode: 'ms' },
    { id: 'stripe_downright', category: 'stripes', legacyCode: 'drs' },
    { id: 'stripe_downleft', category: 'stripes', legacyCode: 'dls' },
    { id: 'small_stripes', category: 'stripes', legacyCode: 'ss' },
    { id: 'cross', category: 'stripes', legacyCode: 'cr' },
    { id: 'straight_cross', category: 'stripes', legacyCode: 'sc' },
    { id: 'diagonal_left', category: 'halves', legacyCode: 'ld' },
    { id: 'diagonal_right', category: 'halves', legacyCode: 'rud' },
    { id: 'diagonal_up_left', category: 'halves', legacyCode: 'lud' },
    { id: 'diagonal_up_right', category: 'halves', legacyCode: 'rd' },
    { id: 'half_vertical', category: 'halves', legacyCode: 'vh' },
    { id: 'half_vertical_right', category: 'halves', legacyCode: 'vhr' },
    { id: 'half_horizontal', category: 'halves', legacyCode: 'hh' },
    { id: 'half_horizontal_bottom', category: 'halves', legacyCode: 'hhb' },
    { id: 'square_bottom_left', category: 'shapes', legacyCode: 'bl' },
    { id: 'square_bottom_right', category: 'shapes', legacyCode: 'br' },
    { id: 'square_top_left', category: 'shapes', legacyCode: 'tl' },
    { id: 'square_top_right', category: 'shapes', legacyCode: 'tr' },
    { id: 'triangle_bottom', category: 'shapes', legacyCode: 'bt' },
    { id: 'triangle_top', category: 'shapes', legacyCode: 'tt' },
    { id: 'triangles_bottom', category: 'shapes', legacyCode: 'bts' },
    { id: 'triangles_top', category: 'shapes', legacyCode: 'tts' },
    { id: 'circle', category: 'shapes', legacyCode: 'mc' },
    { id: 'rhombus', category: 'shapes', legacyCode: 'mr' },
    { id: 'border', category: 'borders', legacyCode: 'bo' },
    {
        id: 'curly_border',
        category: 'borders',
        patternItem: 'minecraft:bordure_indented_banner_pattern',
        legacyCode: 'cbo',
    },
    {
        id: 'bricks',
        category: 'background',
        patternItem: 'minecraft:field_masoned_banner_pattern',
        legacyCode: 'bri',
    },
    { id: 'gradient', category: 'gradients', legacyCode: 'gra' },
    { id: 'gradient_up', category: 'gradients', legacyCode: 'gru' },
    {
        id: 'creeper',
        category: 'icons',
        patternItem: 'minecraft:creeper_banner_pattern',
        legacyCode: 'cre',
    },
    {
        id: 'skull',
        category: 'icons',
        patternItem: 'minecraft:skull_banner_pattern',
        legacyCode: 'sku',
    },
    {
        id: 'flower',
        category: 'icons',
        patternItem: 'minecraft:flower_banner_pattern',
        legacyCode: 'flo',
    },
    {
        id: 'mojang',
        category: 'icons',
        patternItem: 'minecraft:mojang_banner_pattern',
        legacyCode: 'moj',
    },
    {
        id: 'globe',
        category: 'icons',
        patternItem: 'minecraft:globe_banner_pattern',
        // 19w11a, so it predates the 1.16 piglin pattern listed below it.
        since: '1.14',
    },
    {
        id: 'piglin',
        category: 'icons',
        patternItem: 'minecraft:piglin_banner_pattern',
        since: '1.16',
    },
    {
        id: 'flow',
        category: 'icons',
        patternItem: 'minecraft:flow_banner_pattern',
        since: '1.20.5',
    },
    {
        id: 'guster',
        category: 'icons',
        patternItem: 'minecraft:guster_banner_pattern',
        since: '1.20.5',
    },
] as const satisfies readonly BannerPatternDefinition[]

export type BannerPatternId = (typeof BANNER_PATTERN_DEFINITIONS)[number]['id']

/** A pattern entry with its id narrowed to the known {@link BannerPatternId} union. */
export type BannerPattern = BannerPatternDefinition & { id: BannerPatternId }

/**
 * Widened view of the pattern table so `patternItem` stays reachable on every
 * entry, while `id` keeps its literal union type.
 */
export const BANNER_PATTERNS: readonly BannerPattern[] = BANNER_PATTERN_DEFINITIONS

/** Survival limit for the number of patterns a single banner can hold. */
export const MAX_BANNER_LAYERS = 6

const PATTERNS_BY_ID = new Map<string, BannerPattern>(
    BANNER_PATTERNS.map((pattern) => [pattern.id, pattern]),
)

const PATTERNS_BY_LEGACY_CODE = new Map<string, BannerPattern>(
    BANNER_PATTERNS.filter((pattern) => pattern.legacyCode).map((pattern) => [
        pattern.legacyCode as string,
        pattern,
    ]),
)

export function isBannerPatternId(value: string): value is BannerPatternId {
    return PATTERNS_BY_ID.has(value)
}

export function bannerPattern(id: BannerPatternId): BannerPattern {
    const pattern = PATTERNS_BY_ID.get(id)
    if (!pattern) throw new Error(`Unknown banner pattern: ${id}`)
    return pattern
}

/** Accepts `stripe_bottom`, `minecraft:stripe_bottom`, a 1.12 short code and mixed case. */
export function normalizeBannerPatternId(value: string): BannerPatternId | null {
    const normalized = value
        .trim()
        .toLowerCase()
        .replace(/^minecraft:/, '')
    if (isBannerPatternId(normalized)) return normalized
    return PATTERNS_BY_LEGACY_CODE.get(normalized)?.id ?? null
}

export function patternsInCategory(category: BannerPatternCategory): BannerPattern[] {
    return BANNER_PATTERNS.filter((pattern) => pattern.category === category)
}

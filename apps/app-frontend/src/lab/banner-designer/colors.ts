/**
 * Vanilla Java Edition dye colors, in the order used by the game's dye color
 * index (0 = white … 15 = black). The hex values are the diffuse colors the
 * game multiplies banner textures by, so previews match the in-game tint.
 */
export const DYE_COLORS = [
    'white',
    'orange',
    'magenta',
    'light_blue',
    'yellow',
    'lime',
    'pink',
    'gray',
    'light_gray',
    'cyan',
    'purple',
    'blue',
    'brown',
    'green',
    'red',
    'black',
] as const

export type DyeColor = (typeof DYE_COLORS)[number]

export interface DyeColorDefinition {
    id: DyeColor
    /** Index stored in the pre-1.20.5 `Base` and `Color` NBT fields. */
    legacyIndex: number
    hex: string
    bannerItem: string
    dyeItem: string
}

const DYE_COLOR_HEX: Record<DyeColor, string> = {
    white: '#f9fffe',
    orange: '#f9801d',
    magenta: '#c74ebd',
    light_blue: '#3ab3da',
    yellow: '#fed83d',
    lime: '#80c71f',
    pink: '#f38baa',
    gray: '#474f52',
    light_gray: '#9d9d97',
    cyan: '#169c9c',
    purple: '#8932b8',
    blue: '#3c44aa',
    brown: '#835432',
    green: '#5e7c16',
    red: '#b02e26',
    black: '#1d1d21',
}

export const DYE_COLOR_DEFINITIONS: readonly DyeColorDefinition[] = DYE_COLORS.map(
    (id, legacyIndex) => ({
        id,
        legacyIndex,
        hex: DYE_COLOR_HEX[id],
        bannerItem: `minecraft:${id}_banner`,
        dyeItem: `minecraft:${id}_dye`,
    }),
)

const DEFINITIONS_BY_ID = new Map(DYE_COLOR_DEFINITIONS.map((color) => [color.id, color]))

export function isDyeColor(value: string): value is DyeColor {
    return DEFINITIONS_BY_ID.has(value as DyeColor)
}

export function dyeColorDefinition(color: DyeColor): DyeColorDefinition {
    const definition = DEFINITIONS_BY_ID.get(color)
    if (!definition) throw new Error(`Unknown dye color: ${color}`)
    return definition
}

/** Accepts `red`, `minecraft:red` and mixed case, returning a known dye color. */
export function normalizeDyeColor(value: string): DyeColor | null {
    const normalized = value
        .trim()
        .toLowerCase()
        .replace(/^minecraft:/, '')
    return isDyeColor(normalized) ? normalized : null
}

export function dyeColorFromLegacyIndex(index: number): DyeColor | null {
    const definition = DYE_COLOR_DEFINITIONS[index]
    return definition ? definition.id : null
}

export function hexToRgb(hex: string): [number, number, number] {
    const value = Number.parseInt(hex.replace('#', ''), 16)
    return [(value >> 16) & 0xff, (value >> 8) & 0xff, value & 0xff]
}

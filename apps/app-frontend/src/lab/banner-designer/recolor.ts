import { DYE_COLORS, type DyeColor } from './colors.ts'
import type { BannerDesign } from './types.ts'

export interface BannerColorUsage {
    color: DyeColor
    /** Pattern layers painted in this color. */
    layers: number
    /** Whether the base of the banner is filled in this color. */
    base: boolean
}

export interface BannerColorReplaceOptions {
    /** Colors that change; every other color keeps its own. */
    colors: readonly DyeColor[]
    /** Color every match becomes. */
    target: DyeColor
    /** Whether the base is recolored when its color is being replaced. Defaults to true. */
    includeBase?: boolean
}

/** The colors a design paints, in dye order, with how often each one appears. */
export function describeColorUsage(design: BannerDesign): BannerColorUsage[] {
    return DYE_COLORS.map((color) => ({
        color,
        layers: design.layers.filter((layer) => layer.color === color).length,
        base: design.baseColor === color,
    })).filter((usage) => usage.layers > 0 || usage.base)
}

/**
 * Rewrites every layer painted in one of `colors` to `target`. The base follows
 * the same rule unless `includeBase` is off, which leaves a matching base alone.
 * Returns `design` itself when nothing changes, so callers can skip a history
 * entry for a replacement that has no effect.
 */
export function replaceBannerColors(
    design: BannerDesign,
    options: BannerColorReplaceOptions,
): BannerDesign {
    const colors = new Set(options.colors)
    const { target } = options
    const includeBase = options.includeBase ?? true

    let changed = false
    const layers = design.layers.map((layer) => {
        if (!colors.has(layer.color) || layer.color === target) return layer
        changed = true
        return { ...layer, color: target }
    })

    const baseColor =
        includeBase && colors.has(design.baseColor) && design.baseColor !== target
            ? target
            : design.baseColor
    if (baseColor !== design.baseColor) changed = true

    return changed ? { baseColor, layers } : design
}

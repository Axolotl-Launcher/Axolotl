import { DYE_COLORS, type DyeColor, dyeColorDefinition } from './colors.ts'
import { bannerPattern } from './patterns.ts'
import type { BannerDesign } from './types.ts'

export interface BannerDyeTotal {
    color: DyeColor
    count: number
    item: string
}

export interface BannerMaterials {
    /** Banner item the design starts from, always one. Its color is the base color. */
    bannerItem: string
    /**
     * Dyes the loom consumes, one per pattern layer. The base color is not
     * counted: a banner gets its base color from the wool it is crafted with.
     */
    dyes: BannerDyeTotal[]
    /** Banner pattern items the loom requires, never consumed, one entry per kind. */
    patternItems: string[]
    layerCount: number
}

export function describeBannerMaterials(design: BannerDesign): BannerMaterials {
    const totals = new Map<DyeColor, number>()
    for (const layer of design.layers) {
        totals.set(layer.color, (totals.get(layer.color) ?? 0) + 1)
    }

    const patternItems: string[] = []
    for (const layer of design.layers) {
        const item = bannerPattern(layer.pattern).patternItem
        if (item && !patternItems.includes(item)) patternItems.push(item)
    }

    return {
        bannerItem: dyeColorDefinition(design.baseColor).bannerItem,
        dyes: DYE_COLORS.filter((color) => totals.has(color)).map((color) => ({
            color,
            count: totals.get(color) ?? 0,
            item: dyeColorDefinition(color).dyeItem,
        })),
        patternItems,
        layerCount: design.layers.length,
    }
}

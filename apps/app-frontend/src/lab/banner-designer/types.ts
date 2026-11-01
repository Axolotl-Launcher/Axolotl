import type { DyeColor } from './colors.ts'
import type { BannerPatternId } from './patterns.ts'

export interface BannerLayer {
    id: string
    pattern: BannerPatternId
    color: DyeColor
}

export interface BannerDesign {
    baseColor: DyeColor
    /** Patterns in application order: later layers paint over earlier ones. */
    layers: BannerLayer[]
}

let layerSequence = 0

export function createBannerLayer(pattern: BannerPatternId, color: DyeColor): BannerLayer {
    layerSequence += 1
    return { id: `banner-layer-${layerSequence}`, pattern, color }
}

export function cloneBannerDesign(design: BannerDesign): BannerDesign {
    return {
        baseColor: design.baseColor,
        layers: design.layers.map((layer) => createBannerLayer(layer.pattern, layer.color)),
    }
}

export function designsMatch(left: BannerDesign, right: BannerDesign): boolean {
    return (
        left.baseColor === right.baseColor &&
        left.layers.length === right.layers.length &&
        left.layers.every(
            (layer, index) =>
                layer.pattern === right.layers[index].pattern &&
                layer.color === right.layers[index].color,
        )
    )
}

/**
 * The design as it will look once `replacement` stands in for `layerId`. The
 * slot is kept, so a pattern that sits under other layers previews under them
 * too instead of being painted on top. An id the design does not hold appends,
 * which is where a layer that is being added lands.
 */
export function withLayerReplaced(
    design: BannerDesign,
    layerId: string | null,
    replacement: BannerLayer,
): BannerDesign {
    const index = layerId ? design.layers.findIndex((layer) => layer.id === layerId) : -1
    if (index < 0) {
        return { baseColor: design.baseColor, layers: [...design.layers, replacement] }
    }
    const layers = [...design.layers]
    layers[index] = replacement
    return { baseColor: design.baseColor, layers }
}

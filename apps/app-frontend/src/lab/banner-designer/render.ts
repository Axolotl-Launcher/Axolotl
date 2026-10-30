import { dyeColorDefinition, hexToRgb } from './colors.ts'
import { BANNER_FACE_HEIGHT, BANNER_FACE_WIDTH, patternMask } from './masks.ts'
import type { BannerDesign } from './types.ts'

/**
 * Renders the front face of a banner into RGBA pixels, replicating the game's
 * layer compositing: the base color fills the face and every pattern layer is
 * tinted with its dye color and painted over the result with its mask alpha.
 */
export function renderBannerPixels(design: BannerDesign): Uint8ClampedArray<ArrayBuffer> {
    const pixelCount = BANNER_FACE_WIDTH * BANNER_FACE_HEIGHT
    const pixels = new Uint8ClampedArray(pixelCount * 4)
    const [baseRed, baseGreen, baseBlue] = hexToRgb(dyeColorDefinition(design.baseColor).hex)

    for (let index = 0; index < pixelCount; index++) {
        const offset = index * 4
        pixels[offset] = baseRed
        pixels[offset + 1] = baseGreen
        pixels[offset + 2] = baseBlue
        pixels[offset + 3] = 255
    }

    for (const layer of design.layers) {
        const [red, green, blue] = hexToRgb(dyeColorDefinition(layer.color).hex)
        const mask = patternMask(layer.pattern)
        for (let index = 0; index < pixelCount; index++) {
            const coverage = mask ? mask[index] / 255 : 1
            if (coverage <= 0) continue
            const offset = index * 4
            if (coverage >= 1) {
                pixels[offset] = red
                pixels[offset + 1] = green
                pixels[offset + 2] = blue
                pixels[offset + 3] = 255
                continue
            }
            const remaining = 1 - coverage
            pixels[offset] = red * coverage + pixels[offset] * remaining
            pixels[offset + 1] = green * coverage + pixels[offset + 1] * remaining
            pixels[offset + 2] = blue * coverage + pixels[offset + 2] * remaining
            pixels[offset + 3] = 255
        }
    }

    return pixels
}

/** Pixels of a single layer painted over the base color, used for layer cards. */
export function renderLayerPixels(
    design: BannerDesign,
    layerId: string,
): Uint8ClampedArray<ArrayBuffer> {
    const layer = design.layers.find((entry) => entry.id === layerId)
    return renderBannerPixels({
        baseColor: design.baseColor,
        layers: layer ? [layer] : [],
    })
}

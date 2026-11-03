import { describe, expect, it } from 'vitest'

import { DYE_COLOR_DEFINITIONS, dyeColorDefinition } from './colors.ts'
import { BANNER_FACE_HEIGHT, BANNER_FACE_WIDTH, patternMask } from './masks.ts'
import { BANNER_PATTERNS, type BannerPatternId } from './patterns.ts'
import { renderBannerPixels } from './render.ts'
import { type BannerDesign, createBannerLayer } from './types.ts'

function pixelAt(pixels: Uint8ClampedArray, x: number, y: number) {
    const offset = (y * BANNER_FACE_WIDTH + x) * 4
    return [pixels[offset], pixels[offset + 1], pixels[offset + 2], pixels[offset + 3]]
}

/** Vitest's matchers do not narrow types, so a mask a test needs is unwrapped here. */
function maskOf(id: BannerPatternId): Uint8Array {
    const mask = patternMask(id)
    if (!mask) throw new Error(`missing mask for ${id}`)
    return mask
}

describe('banner rendering', () => {
    it('fills the face with the base color when there are no layers', () => {
        const pixels = renderBannerPixels({ baseColor: 'red', layers: [] })
        expect(pixels.length).toBe(BANNER_FACE_WIDTH * BANNER_FACE_HEIGHT * 4)
        expect(pixelAt(pixels, 0, 0)).toEqual([176, 46, 38, 255])
        expect(pixelAt(pixels, BANNER_FACE_WIDTH - 1, BANNER_FACE_HEIGHT - 1)).toEqual([
            176, 46, 38, 255,
        ])
    })

    it('paints a pattern over the base color with the layer color', () => {
        const design: BannerDesign = {
            baseColor: 'white',
            layers: [createBannerLayer('stripe_top', 'black')],
        }
        const pixels = renderBannerPixels(design)
        expect(pixelAt(pixels, 10, 2)).toEqual([29, 29, 33, 255])
        expect(pixelAt(pixels, 10, 30)).toEqual([249, 255, 254, 255])
    })

    it('lets a later layer cover an earlier one', () => {
        const design: BannerDesign = {
            baseColor: 'white',
            layers: [createBannerLayer('stripe_top', 'black'), createBannerLayer('base', 'red')],
        }
        const pixels = renderBannerPixels(design)
        expect(pixelAt(pixels, 10, 2)).toEqual([176, 46, 38, 255])
    })

    it('has a mask for every pattern except the solid field', () => {
        for (const pattern of BANNER_PATTERNS) {
            if (pattern.id === 'base') {
                expect(patternMask(pattern.id)).toBe(null)
                continue
            }
            const mask = maskOf(pattern.id)
            expect(mask.length).toBe(BANNER_FACE_WIDTH * BANNER_FACE_HEIGHT)
            expect(
                mask.some((alpha) => alpha > 0),
                `${pattern.id} mask is empty`,
            ).toBeTruthy()
        }
    })

    it('crops the front face without the banner side columns', () => {
        expect(BANNER_FACE_WIDTH).toBe(20)
        expect(BANNER_FACE_HEIGHT).toBe(40)
        const border = maskOf('border')
        for (let x = 0; x < BANNER_FACE_WIDTH; x++) {
            expect(border[x] > 0, `top edge at ${x} is unpainted`).toBeTruthy()
            expect(
                border[(BANNER_FACE_HEIGHT - 1) * BANNER_FACE_WIDTH + x] > 0,
                `bottom edge at ${x} is unpainted`,
            ).toBeTruthy()
        }
        for (let y = 0; y < BANNER_FACE_HEIGHT; y++) {
            expect(border[y * BANNER_FACE_WIDTH] > 0, `left edge at ${y} is unpainted`).toBeTruthy()
            expect(
                border[y * BANNER_FACE_WIDTH + BANNER_FACE_WIDTH - 1] > 0,
                `right edge at ${y} is unpainted`,
            ).toBeTruthy()
        }
    })

    it('centres a symmetric pattern in the face', () => {
        const mask = maskOf('stripe_center')
        const painted: number[] = []
        for (let x = 0; x < BANNER_FACE_WIDTH; x++) {
            if (mask[x] > 0) painted.push(x)
        }
        expect(painted.length > 0).toBeTruthy()
        expect(painted[0]).toBe(BANNER_FACE_WIDTH - 1 - painted[painted.length - 1])
    })

    it('defines legacy indexes that match the dye order', () => {
        DYE_COLOR_DEFINITIONS.forEach((definition, index) => {
            expect(definition.legacyIndex).toBe(index)
            expect(dyeColorDefinition(definition.id).bannerItem).toBe(
                `minecraft:${definition.id}_banner`,
            )
        })
    })
})

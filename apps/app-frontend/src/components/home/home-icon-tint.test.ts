import { expect, it } from 'vitest'

import { pickIconTint } from './home-icon-tint.ts'

function pixels(...colors: [number, number, number, number, number][]): number[] {
    // [r, g, b, a, count]
    return colors.flatMap(([r, g, b, a, count]) =>
        Array.from({ length: count }, () => [r, g, b, a]).flat(),
    )
}

it('gives a low-saturation icon no tint, so the card keeps the accent colour', () => {
    // A beige tile with black letters, like the Fabulously Optimized icon.
    expect(pickIconTint(pixels([232, 220, 192, 255, 80], [20, 20, 20, 255, 20]))).toBe(null)
    // A grey icon with a small coloured accent is still grey.
    expect(pickIconTint(pixels([128, 128, 128, 255, 95], [220, 30, 30, 255, 5]))).toBe(null)
    // Fully transparent or empty images have nothing to say.
    expect(pickIconTint(pixels([255, 0, 0, 0, 10]))).toBe(null)
    expect(pickIconTint([])).toBe(null)
})

it('takes the hue of a coloured icon and normalises its strength', () => {
    const blue = pickIconTint(pixels([30, 90, 220, 255, 60], [240, 240, 240, 255, 40]))
    expect(blue).toBe('hsl(221 76% 50%)')

    // A dark, dull green still comes out as a bright green.
    const green = pickIconTint(pixels([20, 80, 30, 255, 100]))
    const [, saturation, lightness] = green!.match(/\d+/g)!.map(Number)
    expect(green).toMatch(/^hsl\(1[23]\d /)
    expect(saturation).toBeGreaterThanOrEqual(50)
    expect(lightness).toBeGreaterThanOrEqual(50)
})

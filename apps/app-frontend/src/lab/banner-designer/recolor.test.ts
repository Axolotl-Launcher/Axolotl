import { expect, it } from 'vitest'

import type { DyeColor } from './colors.ts'
import { describeColorUsage, replaceBannerColors } from './recolor.ts'
import { type BannerDesign, createBannerLayer } from './types.ts'

function design(
    baseColor: DyeColor,
    layers: [Parameters<typeof createBannerLayer>[0], DyeColor][],
) {
    return {
        baseColor,
        layers: layers.map(([pattern, color]) => createBannerLayer(pattern, color)),
    } satisfies BannerDesign
}

it('color usage lists each painted color once, in dye order', () => {
    const usage = describeColorUsage(
        design('white', [
            ['stripe_center', 'red'],
            ['cross', 'white'],
            ['border', 'red'],
        ]),
    )

    expect(usage).toEqual([
        { color: 'white', layers: 1, base: true },
        { color: 'red', layers: 2, base: false },
    ])
})

it('color usage reports a plain banner as its base color only', () => {
    expect(describeColorUsage(design('blue', []))).toEqual([
        { color: 'blue', layers: 0, base: true },
    ])
})

it('replacing a color rewrites every layer painted in it', () => {
    const before = design('white', [
        ['stripe_center', 'red'],
        ['cross', 'blue'],
        ['border', 'red'],
    ])

    const after = replaceBannerColors(before, { colors: ['red'], target: 'green' })

    expect(after.layers.map((layer) => layer.color)).toEqual(['green', 'blue', 'green'])
    expect(after.baseColor).toBe('white')
})

it('replacing several colors at once leaves the others untouched', () => {
    const before = design('white', [
        ['stripe_center', 'red'],
        ['cross', 'blue'],
        ['border', 'black'],
    ])

    const after = replaceBannerColors(before, { colors: ['red', 'blue'], target: 'lime' })

    expect(after.layers.map((layer) => layer.color)).toEqual(['lime', 'lime', 'black'])
})

it('the base follows a replacement unless it is excluded', () => {
    const before = design('white', [['stripe_center', 'white']])

    const included = replaceBannerColors(before, { colors: ['white'], target: 'black' })
    expect(included.baseColor).toBe('black')
    expect(included.layers[0].color).toBe('black')

    const excluded = replaceBannerColors(before, {
        colors: ['white'],
        target: 'black',
        includeBase: false,
    })
    expect(excluded.baseColor).toBe('white')
    expect(excluded.layers[0].color).toBe('black')
})

it('a base color that is not being replaced keeps its color', () => {
    const before = design('yellow', [['stripe_center', 'red']])

    const after = replaceBannerColors(before, { colors: ['red'], target: 'black' })

    expect(after.baseColor).toBe('yellow')
})

it('a replacement that changes nothing returns the original design', () => {
    const before = design('white', [['stripe_center', 'red']])

    expect(replaceBannerColors(before, { colors: ['red'], target: 'red' })).toBe(before)
    expect(replaceBannerColors(before, { colors: ['blue'], target: 'red' })).toBe(before)
    expect(
        replaceBannerColors(before, { colors: ['white'], target: 'red', includeBase: false }),
    ).toBe(before)
})

it('replacing keeps layer identities so the layer list stays stable', () => {
    const before = design('white', [['stripe_center', 'red']])

    const after = replaceBannerColors(before, { colors: ['red'], target: 'green' })

    expect(after.layers[0].id).toBe(before.layers[0].id)
    expect(after.layers[0].pattern).toBe('stripe_center')
})

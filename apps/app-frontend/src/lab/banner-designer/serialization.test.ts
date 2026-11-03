import { describe, expect, it } from 'vitest'

import { describeBannerMaterials } from './materials.ts'
import {
    BannerParseError,
    MAX_IMPORT_BYTES,
    parseBannerText,
    toDesignJson,
    toGiveCommand,
    toItemData,
    toPatternsComponent,
    unsupportedPatterns,
} from './serialization.ts'
import { type BannerDesign, createBannerLayer } from './types.ts'

const design: BannerDesign = {
    baseColor: 'white',
    layers: [createBannerLayer('stripe_bottom', 'red'), createBannerLayer('creeper', 'black')],
}

function codes(run: () => unknown): string {
    try {
        run()
    } catch (error) {
        if (error instanceof BannerParseError) return error.code
        throw error
    }
    return 'none'
}

describe('banner export', () => {
    it('writes the modern component syntax', () => {
        expect(toPatternsComponent(design)).toBe(
            '[{pattern:"minecraft:stripe_bottom",color:"red"},{pattern:"minecraft:creeper",color:"black"}]',
        )
        expect(toGiveCommand(design)).toBe(
            '/give @p minecraft:white_banner[minecraft:banner_patterns=[{pattern:"minecraft:stripe_bottom",color:"red"},{pattern:"minecraft:creeper",color:"black"}]] 1',
        )
    })

    it('leaves a single space where a plain banner has no item data', () => {
        const plain: BannerDesign = { baseColor: 'blue', layers: [] }
        expect(toItemData(plain)).toBe('')
        expect(toGiveCommand(plain)).toBe('/give @p minecraft:blue_banner 1')
        // Only the modern dialect can come out without item data; the older ones
        // always carry the block entity tag, and 1.12 keeps it after the damage value.
        expect(toGiveCommand(plain, { version: 'legacy' })).toBe(
            '/give @p minecraft:blue_banner{BlockEntityTag:{Base:11b,Patterns:[]}} 1',
        )
        expect(toGiveCommand(plain, { version: 'classic' })).toBe(
            '/give @p minecraft:banner 1 0 {BlockEntityTag:{Base:11b,Patterns:[]}}',
        )
    })

    it('writes the pre-1.20.5 item NBT with short pattern codes', () => {
        expect(toItemData(design, { version: 'legacy' })).toBe(
            '{BlockEntityTag:{Base:0b,Patterns:[{Pattern:"bs",Color:14b},{Pattern:"cre",Color:15b}]}}',
        )
        expect(toGiveCommand(design, { version: 'legacy', target: '@s', count: 2 })).toBe(
            '/give @s minecraft:white_banner{BlockEntityTag:{Base:0b,Patterns:[{Pattern:"bs",Color:14b},{Pattern:"cre",Color:15b}]}} 2',
        )
    })

    it('writes a design file that round trips', () => {
        const parsed = parseBannerText(toDesignJson(design))
        expect(parsed.baseColor).toBe('white')
        expect(parsed.layers.map((layer) => [layer.pattern, layer.color])).toEqual([
            ['stripe_bottom', 'red'],
            ['creeper', 'black'],
        ])
    })
})

describe('banner export version support', () => {
    it('flags patterns the legacy item data cannot carry', () => {
        const newer: BannerDesign = {
            baseColor: 'white',
            layers: [createBannerLayer('flow', 'red'), createBannerLayer('stripe_top', 'blue')],
        }
        expect(unsupportedPatterns(newer, 'legacy')).toEqual(['flow'])
        expect(unsupportedPatterns(newer, 'modern')).toEqual([])
    })

    it('keeps the 1.13 pattern set and the 1.14 to 1.20 additions in legacy', () => {
        const older: BannerDesign = {
            baseColor: 'white',
            layers: [
                createBannerLayer('bricks', 'red'),
                createBannerLayer('piglin', 'black'),
                createBannerLayer('globe', 'blue'),
            ],
        }
        expect(unsupportedPatterns(older, 'legacy')).toEqual([])
    })

    it('reports each unsupported pattern once', () => {
        const repeated: BannerDesign = {
            baseColor: 'white',
            layers: [
                createBannerLayer('guster', 'red'),
                createBannerLayer('guster', 'blue'),
                createBannerLayer('flow', 'black'),
            ],
        }
        expect(unsupportedPatterns(repeated, 'legacy')).toEqual(['guster', 'flow'])
    })

    it('allows 1.14 to 1.20 patterns in legacy but flags the 1.20.5 ones', () => {
        const mixed: BannerDesign = {
            baseColor: 'white',
            layers: [
                createBannerLayer('stripe_top', 'red'),
                createBannerLayer('piglin', 'black'),
                createBannerLayer('globe', 'blue'),
                createBannerLayer('flow', 'white'),
            ],
        }
        expect(unsupportedPatterns(mixed, 'modern')).toEqual([])
        expect(unsupportedPatterns(mixed, 'legacy')).toEqual(['flow'])
    })

    it('writes one short code per legacy layer', () => {
        const design: BannerDesign = {
            baseColor: 'light_gray',
            layers: [
                createBannerLayer('stripe_top', 'red'),
                createBannerLayer('diagonal_up_right', 'blue'),
            ],
        }
        expect(toItemData(design, { version: 'legacy' })).toBe(
            '{BlockEntityTag:{Base:8b,Patterns:[{Pattern:"ts",Color:14b},{Pattern:"rd",Color:11b}]}}',
        )
    })

    it('writes the 1.12 command with one banner item and a damage value', () => {
        const classic: BannerDesign = {
            baseColor: 'red',
            layers: [
                createBannerLayer('stripe_top', 'white'),
                createBannerLayer('border', 'white'),
            ],
        }
        expect(toGiveCommand(classic, { version: 'classic' })).toBe(
            '/give @p minecraft:banner 1 0 {BlockEntityTag:{Base:14b,Patterns:[{Pattern:"ts",Color:0b},{Pattern:"bo",Color:0b}]}}',
        )
        // The tag itself is the same one 1.13 through 1.20.4 read.
        expect(toItemData(classic, { version: 'classic' })).toBe(
            toItemData(classic, { version: 'legacy' }),
        )
    })

    it('keeps the count before the damage value in the 1.12 command', () => {
        expect(toGiveCommand(design, { version: 'classic', target: '@s', count: 3 })).toBe(
            '/give @s minecraft:banner 3 0 {BlockEntityTag:{Base:0b,Patterns:[{Pattern:"bs",Color:14b},{Pattern:"cre",Color:15b}]}}',
        )
    })

    it('flags 1.14 and later patterns for the 1.12 target', () => {
        const newer: BannerDesign = {
            baseColor: 'white',
            layers: [
                createBannerLayer('bricks', 'red'),
                createBannerLayer('piglin', 'black'),
                createBannerLayer('globe', 'blue'),
            ],
        }
        expect(unsupportedPatterns(newer, 'classic')).toEqual(['piglin', 'globe'])
        expect(unsupportedPatterns(newer, 'legacy')).toEqual([])
    })
})

describe('banner import', () => {
    it('reads the modern give command', () => {
        const parsed = parseBannerText(
            '/give @p minecraft:red_banner[minecraft:banner_patterns=[{pattern:"minecraft:stripe_top",color:"blue"}]] 1',
        )
        expect(parsed.baseColor).toBe('red')
        expect(parsed.layers.map((layer) => [layer.pattern, layer.color])).toEqual([
            ['stripe_top', 'blue'],
        ])
    })

    it('reads component data without a command', () => {
        const parsed = parseBannerText(
            '[minecraft:banner_patterns=[{pattern:"minecraft:skull",color:"white"}]]',
        )
        expect(parsed.baseColor).toBe('white')
        expect(parsed.layers[0].pattern).toBe('skull')
    })

    it('reads legacy item NBT', () => {
        const parsed = parseBannerText(
            '{BlockEntityTag:{Base:14b,Patterns:[{Pattern:"curly_border",Color:0b}]}}',
        )
        expect(parsed.baseColor).toBe('red')
        expect(parsed.layers[0].pattern).toBe('curly_border')
        expect(parsed.layers[0].color).toBe('white')
    })

    it('reads a numeric base colour in a design file the way it reads layer colours', () => {
        const parsed = parseBannerText('{"baseColor":14,"layers":[{"pattern":"cross","color":0}]}')
        expect(parsed.baseColor).toBe('red')
        expect(parsed.layers[0].color).toBe('white')
        expect(codes(() => parseBannerText('{"baseColor":99,"layers":[]}'))).toBe('invalid-color')
    })

    it('falls back to the item id when the legacy NBT has no base color', () => {
        const parsed = parseBannerText(
            '/give @p minecraft:lime_banner{BlockEntityTag:{Patterns:[{Pattern:"bricks",Color:4b}]}}',
        )
        expect(parsed.baseColor).toBe('lime')
        expect(parsed.layers[0].color).toBe('yellow')
    })

    it('reads the 1.12 give command, which keeps the data tag last', () => {
        const parsed = parseBannerText(
            '/give @p minecraft:banner 1 8 {BlockEntityTag:{Base:8,Patterns:[{Pattern:"ts",Color:14}]}}',
        )
        expect(parsed.baseColor).toBe('light_gray')
        expect(parsed.layers[0].pattern).toBe('stripe_top')
        expect(parsed.layers[0].color).toBe('red')
    })

    it('takes the base color from the damage value of a plain banner', () => {
        const parsed = parseBannerText(
            '/give @p minecraft:banner 1 14 {BlockEntityTag:{Patterns:[{Pattern:"bs",Color:0}]}}',
        )
        expect(parsed.baseColor).toBe('red')
        expect(parsed.layers[0].pattern).toBe('stripe_bottom')
    })

    it('reads the 1.12 command this exporter writes', () => {
        const parsed = parseBannerText(
            '/give @p minecraft:banner 1 0 {BlockEntityTag:{Base:14b,Patterns:[{Pattern:"ts",Color:0b},{Pattern:"bo",Color:0b}]}}',
        )
        expect(parsed.baseColor).toBe('red')
        expect(parsed.layers.map((layer) => [layer.pattern, layer.color])).toEqual([
            ['stripe_top', 'white'],
            ['border', 'white'],
        ])
    })

    it('keeps a selector that carries a quoted space together with the item', () => {
        const parsed = parseBannerText(
            '/give @a[name="a b"] minecraft:red_banner[minecraft:banner_patterns=[{pattern:"cross",color:"white"}]] 1',
        )
        expect(parsed.baseColor).toBe('red')
        expect(parsed.layers.map((layer) => [layer.pattern, layer.color])).toEqual([
            ['cross', 'white'],
        ])
    })

    it('rejects give commands for items that are not banners', () => {
        expect(codes(() => parseBannerText('/give @p minecraft:diamond'))).toBe('unsupported')
        expect(codes(() => parseBannerText('/give @p minecraft:red_dye 1'))).toBe('unsupported')
    })

    it('rejects unknown patterns and colors', () => {
        expect(codes(() => parseBannerText(''))).toBe('empty')
        expect(codes(() => parseBannerText('hello'))).toBe('unsupported')
        expect(
            codes(() =>
                parseBannerText('[minecraft:banner_patterns=[{pattern:"nope",color:"red"}]]'),
            ),
        ).toBe('invalid-pattern')
        expect(
            codes(() =>
                parseBannerText('[minecraft:banner_patterns=[{pattern:"cross",color:"pink2"}]]'),
            ),
        ).toBe('invalid-color')
        expect(codes(() => parseBannerText('{broken'))).toBe('invalid-data')
    })

    it('rejects text far larger than a design', () => {
        const huge = `{BlockEntityTag:{Base:0b,Patterns:[${'{Pattern:"cross",Color:0b},'.repeat(20000)}]}}`
        expect(codes(() => parseBannerText(huge))).toBe('too-large')
    })

    it('measures the import cap in bytes rather than UTF-16 code units', () => {
        // Two bytes each once encoded, so this is under the cap as a string
        // length and over it as a payload.
        const wide = 'ä'.repeat(MAX_IMPORT_BYTES / 2 + 1)
        expect(wide.length).toBeLessThan(MAX_IMPORT_BYTES)
        expect(codes(() => parseBannerText(wide))).toBe('too-large')
    })

    it('caps imported layers at the survival limit', () => {
        const layers = Array.from(
            { length: 8 },
            (_, index) => `{Pattern:"cross",Color:${index}b}`,
        ).join(',')
        const parsed = parseBannerText(`{BlockEntityTag:{Base:0b,Patterns:[${layers}]}}`)
        expect(parsed.layers.length).toBe(6)
    })
})

describe('banner materials', () => {
    it('counts dyes, pattern items and the banner itself', () => {
        const materials = describeBannerMaterials(design)
        expect(materials.bannerItem).toBe('minecraft:white_banner')
        expect(materials.dyes).toEqual([
            { color: 'red', count: 1, item: 'minecraft:red_dye' },
            { color: 'black', count: 1, item: 'minecraft:black_dye' },
        ])
        expect(materials.patternItems).toEqual(['minecraft:creeper_banner_pattern'])
    })

    it('merges repeated layer colors without counting the base color', () => {
        const materials = describeBannerMaterials({
            baseColor: 'red',
            layers: [createBannerLayer('cross', 'red'), createBannerLayer('border', 'red')],
        })
        expect(materials.bannerItem).toBe('minecraft:red_banner')
        expect(materials.dyes).toEqual([{ color: 'red', count: 2, item: 'minecraft:red_dye' }])
        expect(materials.patternItems).toEqual([])
    })

    it('lists each pattern item once', () => {
        const materials = describeBannerMaterials({
            baseColor: 'white',
            layers: [
                createBannerLayer('creeper', 'black'),
                createBannerLayer('creeper', 'red'),
                createBannerLayer('globe', 'blue'),
            ],
        })
        expect(materials.patternItems).toEqual([
            'minecraft:creeper_banner_pattern',
            'minecraft:globe_banner_pattern',
        ])
    })
})

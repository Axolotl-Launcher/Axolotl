import { describe, expect, it } from 'vitest'

import { isDyeColor } from './colors.ts'
import { presetNameMessages, presetSectionMessages } from './messages.ts'
import { isBannerPatternId, MAX_BANNER_LAYERS } from './patterns.ts'
import { BANNER_PRESETS, type BannerPresetSection, presetsInSection } from './presets.ts'

const GLYPH_SECTIONS: BannerPresetSection[] = ['uppercase', 'lowercase', 'digits', 'suits']

describe('banner presets', () => {
    it('uses unique ids', () => {
        const ids = BANNER_PRESETS.map((preset) => preset.id)
        expect(new Set(ids).size).toBe(ids.length)
    })

    it('stays inside the layer limit and uses known patterns and dyes', () => {
        for (const preset of BANNER_PRESETS) {
            const design = preset.design()
            expect(
                isDyeColor(design.baseColor),
                `${preset.id} has an unknown base color`,
            ).toBeTruthy()
            expect(
                design.layers.length <= MAX_BANNER_LAYERS,
                `${preset.id} has ${design.layers.length} layers`,
            ).toBeTruthy()
            for (const layer of design.layers) {
                expect(
                    isBannerPatternId(layer.pattern),
                    `${preset.id} has an unknown pattern ${layer.pattern}`,
                ).toBeTruthy()
                expect(
                    isDyeColor(layer.color),
                    `${preset.id} has an unknown dye ${layer.color}`,
                ).toBeTruthy()
            }
        }
    })

    it('covers the whole alphabet, the digits and the four suits', () => {
        expect(presetsInSection('uppercase').length).toBe(26)
        expect(presetsInSection('lowercase').length).toBe(26)
        expect(presetsInSection('digits').length).toBe(10)
        expect(presetsInSection('suits').length).toBe(4)

        const labels = (section: BannerPresetSection) =>
            presetsInSection(section)
                .map((preset) => preset.label)
                .join('')

        expect(labels('uppercase')).toBe('ABCDEFGHIJKLMNOPQRSTUVWXYZ')
        expect(labels('lowercase')).toBe('abcdefghijklmnopqrstuvwxyz')
        expect(labels('digits')).toBe('0123456789')
    })

    it('names every preset, either with a literal glyph or a translated label', () => {
        for (const preset of BANNER_PRESETS) {
            if (preset.label !== undefined) {
                expect(preset.label.length).toBe(1)
                continue
            }
            expect(presetNameMessages[preset.id], `${preset.id} has no name message`).toBeTruthy()
        }
        for (const section of GLYPH_SECTIONS) {
            for (const preset of presetsInSection(section)) {
                expect(preset.label === undefined).toBe(preset.section === 'suits')
            }
        }
    })

    it('has a section title for every section in use', () => {
        for (const preset of BANNER_PRESETS) {
            expect(
                presetSectionMessages[preset.section],
                `${preset.section} has no section title`,
            ).toBeTruthy()
        }
    })
})

import type { DyeColor } from './colors.ts'
import type { BannerPatternId } from './patterns.ts'
import { type BannerDesign, createBannerLayer } from './types.ts'

/** Presets are grouped in the picker; the order here is the order of the sections. */
export type BannerPresetSection = 'patterns' | 'uppercase' | 'lowercase' | 'digits' | 'suits'

export interface BannerPreset {
    id: string
    section: BannerPresetSection
    /** Literal caption for glyph presets such as `A` or `7`, which need no translation. */
    label?: string
    /** Base color the preset starts from; also drives the preset card preview. */
    design: () => BannerDesign
}

function preset(
    baseColor: DyeColor,
    layers: [BannerPatternId, DyeColor][],
): BannerPreset['design'] {
    return () => ({
        baseColor,
        layers: layers.map(([pattern, color]) => createBannerLayer(pattern, color)),
    })
}

/**
 * Designs documented by the Minecraft Wiki: the banners that generate in
 * structures, single-pattern showcases for the pattern items, and the letter,
 * number and card suit alphabets from the banner design tutorial.
 */
export const BANNER_PRESETS: readonly BannerPreset[] = [
    // End City towers: magenta with black chevrons top and bottom.
    {
        id: 'end-city',
        section: 'patterns',
        design: preset('magenta', [
            ['triangle_bottom', 'black'],
            ['triangle_top', 'black'],
        ]),
    },
    // Woodland mansion master bedroom.
    {
        id: 'mansion-bedroom',
        section: 'patterns',
        design: preset('light_gray', [['flower', 'white']]),
    },
    { id: 'creeper', section: 'patterns', design: preset('green', [['creeper', 'black']]) },
    { id: 'skull', section: 'patterns', design: preset('black', [['skull', 'white']]) },
    { id: 'mojang', section: 'patterns', design: preset('red', [['mojang', 'white']]) },
    { id: 'globe', section: 'patterns', design: preset('blue', [['globe', 'white']]) },
    { id: 'snout', section: 'patterns', design: preset('pink', [['piglin', 'black']]) },
    {
        id: 'gradient',
        section: 'patterns',
        design: preset('white', [['gradient', 'light_blue']]),
    },

    // Capital letters, from the Minecraft Wiki banner design tutorial.
    {
        id: 'letter-upper-a',
        section: 'uppercase',
        label: 'A',
        design: preset('white', [
            ['stripe_right', 'black'],
            ['stripe_left', 'black'],
            ['stripe_middle', 'black'],
            ['stripe_top', 'black'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'letter-upper-b',
        section: 'uppercase',
        label: 'B',
        design: preset('black', [
            ['mojang', 'white'],
            ['stripe_right', 'black'],
            ['curly_border', 'white'],
            ['stripe_left', 'black'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'letter-upper-c',
        section: 'uppercase',
        label: 'C',
        design: preset('white', [
            ['stripe_top', 'black'],
            ['stripe_left', 'black'],
            ['stripe_bottom', 'black'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'letter-upper-d',
        section: 'uppercase',
        label: 'D',
        design: preset('white', [
            ['stripe_top', 'black'],
            ['stripe_bottom', 'black'],
            ['stripe_right', 'black'],
            ['curly_border', 'white'],
            ['stripe_left', 'black'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'letter-upper-e',
        section: 'uppercase',
        label: 'E',
        design: preset('white', [
            ['stripe_middle', 'black'],
            ['stripe_top', 'black'],
            ['stripe_bottom', 'black'],
            ['stripe_left', 'black'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'letter-upper-f',
        section: 'uppercase',
        label: 'F',
        design: preset('white', [
            ['stripe_left', 'black'],
            ['stripe_middle', 'black'],
            ['stripe_top', 'black'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'letter-upper-g',
        section: 'uppercase',
        label: 'G',
        design: preset('white', [
            ['diagonal_right', 'black'],
            ['half_horizontal', 'white'],
            ['stripe_bottom', 'black'],
            ['stripe_left', 'black'],
            ['stripe_top', 'black'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'letter-upper-h',
        section: 'uppercase',
        label: 'H',
        design: preset('white', [
            ['stripe_left', 'black'],
            ['stripe_right', 'black'],
            ['stripe_middle', 'black'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'letter-upper-i',
        section: 'uppercase',
        label: 'I',
        design: preset('white', [
            ['stripe_center', 'black'],
            ['stripe_top', 'black'],
            ['stripe_bottom', 'black'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'letter-upper-j',
        section: 'uppercase',
        label: 'J',
        design: preset('white', [
            ['half_horizontal_bottom', 'black'],
            ['half_vertical', 'white'],
            ['stripe_center', 'white'],
            ['stripe_bottom', 'black'],
            ['stripe_right', 'black'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'letter-upper-k',
        section: 'uppercase',
        label: 'K',
        design: preset('white', [
            ['stripe_downright', 'black'],
            ['diagonal_right', 'white'],
            ['stripe_downleft', 'black'],
            ['stripe_left', 'black'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'letter-upper-l',
        section: 'uppercase',
        label: 'L',
        design: preset('white', [
            ['stripe_bottom', 'black'],
            ['stripe_left', 'black'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'letter-upper-m',
        section: 'uppercase',
        label: 'M',
        design: preset('white', [
            ['triangle_top', 'black'],
            ['triangles_top', 'white'],
            ['stripe_left', 'black'],
            ['stripe_right', 'black'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'letter-upper-n',
        section: 'uppercase',
        label: 'N',
        design: preset('white', [
            ['stripe_downright', 'black'],
            ['stripe_right', 'black'],
            ['stripe_left', 'black'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'letter-upper-o',
        section: 'uppercase',
        label: 'O',
        design: preset('white', [
            ['stripe_left', 'black'],
            ['stripe_right', 'black'],
            ['stripe_top', 'black'],
            ['stripe_bottom', 'black'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'letter-upper-p',
        section: 'uppercase',
        label: 'P',
        design: preset('white', [
            ['stripe_right', 'black'],
            ['half_horizontal_bottom', 'white'],
            ['stripe_middle', 'black'],
            ['stripe_top', 'black'],
            ['stripe_left', 'black'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'letter-upper-q',
        section: 'uppercase',
        label: 'Q',
        design: preset('black', [
            ['rhombus', 'white'],
            ['stripe_left', 'black'],
            ['stripe_right', 'black'],
            ['square_bottom_right', 'black'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'letter-upper-r',
        section: 'uppercase',
        label: 'R',
        design: preset('white', [
            ['half_horizontal', 'black'],
            ['skull', 'white'],
            ['stripe_downright', 'black'],
            ['stripe_middle', 'black'],
            ['stripe_left', 'black'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'letter-upper-s',
        section: 'uppercase',
        label: 'S',
        design: preset('white', [
            ['triangle_top', 'black'],
            ['triangle_bottom', 'black'],
            ['rhombus', 'white'],
            ['stripe_downright', 'black'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'letter-upper-t',
        section: 'uppercase',
        label: 'T',
        design: preset('white', [
            ['stripe_center', 'black'],
            ['stripe_top', 'black'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'letter-upper-u',
        section: 'uppercase',
        label: 'U',
        design: preset('white', [
            ['stripe_bottom', 'black'],
            ['stripe_right', 'black'],
            ['stripe_left', 'black'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'letter-upper-v',
        section: 'uppercase',
        label: 'V',
        design: preset('white', [
            ['stripe_downleft', 'black'],
            ['stripe_left', 'black'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'letter-upper-w',
        section: 'uppercase',
        label: 'W',
        design: preset('white', [
            ['triangle_bottom', 'black'],
            ['triangles_bottom', 'white'],
            ['stripe_right', 'black'],
            ['stripe_left', 'black'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'letter-upper-x',
        section: 'uppercase',
        label: 'X',
        design: preset('white', [
            ['cross', 'black'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'letter-upper-y',
        section: 'uppercase',
        label: 'Y',
        design: preset('white', [
            ['cross', 'black'],
            ['half_horizontal_bottom', 'white'],
            ['stripe_center', 'black'],
            ['triangle_top', 'white'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'letter-upper-z',
        section: 'uppercase',
        label: 'Z',
        design: preset('white', [
            ['stripe_downleft', 'black'],
            ['stripe_top', 'black'],
            ['stripe_bottom', 'black'],
            ['border', 'white'],
        ]),
    },

    // Lower case letters, from the Minecraft Wiki banner design tutorial.
    {
        id: 'letter-lower-a',
        section: 'lowercase',
        label: 'a',
        design: preset('black', [
            ['stripe_downright', 'white'],
            ['stripe_right', 'black'],
            ['diagonal_up_left', 'white'],
            ['mojang', 'black'],
            ['stripe_top', 'white'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'letter-lower-b',
        section: 'lowercase',
        label: 'b',
        design: preset('black', [
            ['creeper', 'white'],
            ['stripe_right', 'black'],
            ['half_horizontal', 'white'],
            ['stripe_left', 'black'],
            ['stripe_middle', 'black'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'letter-lower-c',
        section: 'lowercase',
        label: 'c',
        design: preset('black', [
            ['rhombus', 'white'],
            ['diagonal_right', 'white'],
            ['stripe_left', 'black'],
            ['half_horizontal', 'white'],
            ['stripe_middle', 'black'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'letter-lower-d',
        section: 'lowercase',
        label: 'd',
        design: preset('black', [
            ['creeper', 'white'],
            ['stripe_left', 'black'],
            ['half_horizontal', 'white'],
            ['stripe_right', 'black'],
            ['stripe_middle', 'black'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'letter-lower-e',
        section: 'lowercase',
        label: 'e',
        design: preset('black', [
            ['stripe_downright', 'white'],
            ['mojang', 'black'],
            ['half_horizontal', 'white'],
            ['stripe_middle', 'black'],
            ['triangles_bottom', 'black'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'letter-lower-f',
        section: 'lowercase',
        label: 'f',
        design: preset('white', [
            ['square_top_right', 'black'],
            ['rhombus', 'white'],
            ['stripe_center', 'black'],
            ['straight_cross', 'black'],
            ['curly_border', 'white'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'letter-lower-g',
        section: 'lowercase',
        label: 'g',
        design: preset('white', [
            ['stripe_bottom', 'black'],
            ['rhombus', 'white'],
            ['stripe_right', 'black'],
            ['stripe_top', 'white'],
            ['circle', 'black'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'letter-lower-h',
        section: 'lowercase',
        label: 'h',
        design: preset('white', [
            ['stripe_right', 'black'],
            ['half_horizontal', 'white'],
            ['stripe_left', 'black'],
            ['stripe_center', 'white'],
            ['stripe_middle', 'black'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'letter-lower-i',
        section: 'lowercase',
        label: 'i',
        design: preset('black', [
            ['stripe_middle', 'white'],
            ['half_horizontal', 'black'],
            ['stripe_top', 'white'],
            ['stripe_left', 'white'],
            ['stripe_right', 'white'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'letter-lower-j',
        section: 'lowercase',
        label: 'j',
        design: preset('white', [
            ['square_bottom_left', 'black'],
            ['creeper', 'white'],
            ['stripe_center', 'black'],
            ['stripe_middle', 'white'],
            ['stripe_top', 'white'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'letter-lower-k',
        section: 'lowercase',
        label: 'k',
        design: preset('white', [
            ['stripe_downright', 'black'],
            ['diagonal_right', 'white'],
            ['half_horizontal', 'white'],
            ['stripe_middle', 'black'],
            ['stripe_left', 'black'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'letter-lower-l',
        section: 'lowercase',
        label: 'l',
        design: preset('white', [
            ['triangle_bottom', 'black'],
            ['cross', 'white'],
            ['triangles_bottom', 'black'],
            ['half_vertical_right', 'white'],
            ['stripe_center', 'black'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'letter-lower-m',
        section: 'lowercase',
        label: 'm',
        design: preset('black', [
            ['small_stripes', 'white'],
            ['stripe_left', 'black'],
            ['stripe_right', 'black'],
            ['half_horizontal', 'white'],
            ['stripe_middle', 'black'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'letter-lower-n',
        section: 'lowercase',
        label: 'n',
        design: preset('black', [
            ['stripe_center', 'white'],
            ['half_horizontal', 'white'],
            ['stripe_middle', 'black'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'letter-lower-o',
        section: 'lowercase',
        label: 'o',
        design: preset('black', [
            ['creeper', 'white'],
            ['stripe_left', 'black'],
            ['stripe_right', 'black'],
            ['half_horizontal', 'white'],
            ['stripe_middle', 'black'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'letter-lower-p',
        section: 'lowercase',
        label: 'p',
        design: preset('black', [
            ['stripe_middle', 'white'],
            ['stripe_right', 'black'],
            ['stripe_bottom', 'white'],
            ['stripe_left', 'black'],
            ['stripe_top', 'white'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'letter-lower-q',
        section: 'lowercase',
        label: 'q',
        design: preset('black', [
            ['stripe_middle', 'white'],
            ['stripe_left', 'black'],
            ['stripe_bottom', 'white'],
            ['stripe_right', 'black'],
            ['stripe_top', 'white'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'letter-lower-r',
        section: 'lowercase',
        label: 'r',
        design: preset('white', [
            ['rhombus', 'black'],
            ['stripe_downleft', 'white'],
            ['half_vertical', 'black'],
            ['stripe_left', 'white'],
            ['half_horizontal', 'white'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'letter-lower-s',
        section: 'lowercase',
        label: 's',
        design: preset('white', [
            ['stripe_bottom', 'black'],
            ['rhombus', 'white'],
            ['stripe_downright', 'black'],
            ['stripe_middle', 'black'],
            ['diagonal_left', 'white'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'letter-lower-t',
        section: 'lowercase',
        label: 't',
        design: preset('white', [
            ['square_bottom_right', 'black'],
            ['rhombus', 'white'],
            ['straight_cross', 'black'],
            ['curly_border', 'white'],
            ['stripe_top', 'white'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'letter-lower-u',
        section: 'lowercase',
        label: 'u',
        design: preset('black', [
            ['creeper', 'white'],
            ['creeper', 'white'],
            ['stripe_left', 'black'],
            ['stripe_right', 'black'],
            ['half_horizontal', 'white'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'letter-lower-v',
        section: 'lowercase',
        label: 'v',
        design: preset('black', [
            ['triangles_bottom', 'white'],
            ['stripe_center', 'black'],
            ['rhombus', 'white'],
            ['half_horizontal', 'white'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'letter-lower-w',
        section: 'lowercase',
        label: 'w',
        design: preset('white', [
            ['triangle_bottom', 'black'],
            ['cross', 'white'],
            ['stripe_left', 'black'],
            ['stripe_right', 'black'],
            ['half_horizontal', 'white'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'letter-lower-x',
        section: 'lowercase',
        label: 'x',
        design: preset('white', [
            ['rhombus', 'black'],
            ['triangle_bottom', 'black'],
            ['triangle_top', 'white'],
            ['triangles_bottom', 'white'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'letter-lower-y',
        section: 'lowercase',
        label: 'y',
        design: preset('white', [
            ['diagonal_up_right', 'black'],
            ['stripe_downleft', 'white'],
            ['stripe_downright', 'black'],
            ['stripe_right', 'black'],
            ['stripe_top', 'white'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'letter-lower-z',
        section: 'lowercase',
        label: 'z',
        design: preset('white', [
            ['stripe_bottom', 'black'],
            ['rhombus', 'black'],
            ['diagonal_left', 'white'],
            ['half_horizontal', 'black'],
            ['stripe_top', 'white'],
            ['border', 'white'],
        ]),
    },

    // Digits, from the Minecraft Wiki banner design tutorial.
    {
        id: 'digit-0',
        section: 'digits',
        label: '0',
        design: preset('black', [
            ['rhombus', 'white'],
            ['stripe_left', 'black'],
            ['stripe_right', 'black'],
            ['stripe_downleft', 'black'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'digit-1',
        section: 'digits',
        label: '1',
        design: preset('white', [
            ['square_top_left', 'black'],
            ['curly_border', 'white'],
            ['stripe_center', 'black'],
            ['stripe_bottom', 'black'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'digit-2',
        section: 'digits',
        label: '2',
        design: preset('white', [
            ['stripe_top', 'black'],
            ['rhombus', 'white'],
            ['stripe_downleft', 'black'],
            ['curly_border', 'white'],
            ['stripe_bottom', 'black'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'digit-3',
        section: 'digits',
        label: '3',
        design: preset('black', [
            ['rhombus', 'white'],
            ['stripe_middle', 'black'],
            ['mojang', 'white'],
            ['stripe_right', 'black'],
            ['curly_border', 'white'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'digit-4',
        section: 'digits',
        label: '4',
        design: preset('white', [
            ['rhombus', 'black'],
            ['diagonal_up_right', 'white'],
            ['half_horizontal_bottom', 'black'],
            ['stripe_bottom', 'white'],
            ['stripe_right', 'black'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'digit-5',
        section: 'digits',
        label: '5',
        design: preset('white', [
            ['stripe_bottom', 'black'],
            ['rhombus', 'white'],
            ['stripe_downright', 'black'],
            ['stripe_top', 'black'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'digit-6',
        section: 'digits',
        label: '6',
        design: preset('black', [
            ['rhombus', 'white'],
            ['creeper', 'white'],
            ['stripe_middle', 'black'],
            ['stripe_bottom', 'black'],
            ['stripe_left', 'black'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'digit-7',
        section: 'digits',
        label: '7',
        design: preset('white', [
            ['square_bottom_left', 'black'],
            ['stripe_top', 'black'],
            ['stripe_downleft', 'black'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'digit-8',
        section: 'digits',
        label: '8',
        design: preset('black', [
            ['skull', 'white'],
            ['creeper', 'white'],
            ['stripe_left', 'black'],
            ['stripe_right', 'black'],
            ['stripe_middle', 'black'],
            ['border', 'white'],
        ]),
    },
    {
        id: 'digit-9',
        section: 'digits',
        label: '9',
        design: preset('white', [
            ['half_horizontal', 'black'],
            ['skull', 'white'],
            ['circle', 'white'],
            ['stripe_middle', 'black'],
            ['stripe_right', 'black'],
            ['border', 'white'],
        ]),
    },

    // Card suits, from the Minecraft Wiki banner design tutorial.
    {
        id: 'suit-heart',
        section: 'suits',
        design: preset('red', [
            ['curly_border', 'white'],
            ['stripe_bottom', 'white'],
            ['rhombus', 'red'],
            ['triangle_top', 'white'],
            ['stripe_top', 'white'],
            ['stripe_top', 'white'],
        ]),
    },
    {
        id: 'suit-diamond',
        section: 'suits',
        design: preset('white', [['rhombus', 'red']]),
    },
    {
        id: 'suit-spade',
        section: 'suits',
        design: preset('black', [
            ['curly_border', 'white'],
            ['stripe_top', 'white'],
            ['rhombus', 'black'],
            ['stripe_bottom', 'white'],
            ['skull', 'black'],
        ]),
    },
    {
        id: 'suit-club',
        section: 'suits',
        design: preset('white', [
            ['skull', 'black'],
            ['stripe_middle', 'black'],
            ['circle', 'black'],
            ['triangle_bottom', 'black'],
            ['border', 'white'],
            ['stripe_bottom', 'white'],
        ]),
    },
]

/** Presets of one picker section, in declaration order. */
export function presetsInSection(section: BannerPresetSection): BannerPreset[] {
    return BANNER_PRESETS.filter((entry) => entry.section === section)
}

import { defineMessages, type MessageDescriptor } from '@modrinth/ui'

import type { DyeColor } from './colors.ts'
import type { BannerPatternCategory, BannerPatternId } from './patterns.ts'
import type { BannerPresetSection } from './presets.ts'

/** Minecraft Wiki pattern names, keyed by registry id. */
export const patternNameMessages: Record<BannerPatternId, MessageDescriptor> = defineMessages({
    base: { id: 'app.lab.banner-designer.pattern.base', defaultMessage: 'Solid field' },
    stripe_bottom: {
        id: 'app.lab.banner-designer.pattern.stripe-bottom',
        defaultMessage: 'Base',
    },
    stripe_top: { id: 'app.lab.banner-designer.pattern.stripe-top', defaultMessage: 'Chief' },
    stripe_left: {
        id: 'app.lab.banner-designer.pattern.stripe-left',
        defaultMessage: 'Pale dexter',
    },
    stripe_right: {
        id: 'app.lab.banner-designer.pattern.stripe-right',
        defaultMessage: 'Pale sinister',
    },
    stripe_center: { id: 'app.lab.banner-designer.pattern.stripe-center', defaultMessage: 'Pale' },
    stripe_middle: { id: 'app.lab.banner-designer.pattern.stripe-middle', defaultMessage: 'Fess' },
    stripe_downright: {
        id: 'app.lab.banner-designer.pattern.stripe-downright',
        defaultMessage: 'Bend',
    },
    stripe_downleft: {
        id: 'app.lab.banner-designer.pattern.stripe-downleft',
        defaultMessage: 'Bend sinister',
    },
    small_stripes: {
        id: 'app.lab.banner-designer.pattern.small-stripes',
        defaultMessage: 'Paly',
    },
    cross: { id: 'app.lab.banner-designer.pattern.cross', defaultMessage: 'Saltire' },
    straight_cross: {
        id: 'app.lab.banner-designer.pattern.straight-cross',
        defaultMessage: 'Cross',
    },
    diagonal_left: {
        id: 'app.lab.banner-designer.pattern.diagonal-left',
        defaultMessage: 'Per bend sinister',
    },
    diagonal_right: {
        id: 'app.lab.banner-designer.pattern.diagonal-right',
        defaultMessage: 'Per bend',
    },
    diagonal_up_left: {
        id: 'app.lab.banner-designer.pattern.diagonal-up-left',
        defaultMessage: 'Per bend inverted',
    },
    diagonal_up_right: {
        id: 'app.lab.banner-designer.pattern.diagonal-up-right',
        defaultMessage: 'Per bend sinister inverted',
    },
    half_vertical: {
        id: 'app.lab.banner-designer.pattern.half-vertical',
        defaultMessage: 'Per pale',
    },
    half_vertical_right: {
        id: 'app.lab.banner-designer.pattern.half-vertical-right',
        defaultMessage: 'Per pale inverted',
    },
    half_horizontal: {
        id: 'app.lab.banner-designer.pattern.half-horizontal',
        defaultMessage: 'Per fess',
    },
    half_horizontal_bottom: {
        id: 'app.lab.banner-designer.pattern.half-horizontal-bottom',
        defaultMessage: 'Per fess inverted',
    },
    square_bottom_left: {
        id: 'app.lab.banner-designer.pattern.square-bottom-left',
        defaultMessage: 'Base dexter canton',
    },
    square_bottom_right: {
        id: 'app.lab.banner-designer.pattern.square-bottom-right',
        defaultMessage: 'Base sinister canton',
    },
    square_top_left: {
        id: 'app.lab.banner-designer.pattern.square-top-left',
        defaultMessage: 'Chief dexter canton',
    },
    square_top_right: {
        id: 'app.lab.banner-designer.pattern.square-top-right',
        defaultMessage: 'Chief sinister canton',
    },
    triangle_bottom: {
        id: 'app.lab.banner-designer.pattern.triangle-bottom',
        defaultMessage: 'Chevron',
    },
    triangle_top: {
        id: 'app.lab.banner-designer.pattern.triangle-top',
        defaultMessage: 'Inverted chevron',
    },
    triangles_bottom: {
        id: 'app.lab.banner-designer.pattern.triangles-bottom',
        defaultMessage: 'Base indented',
    },
    triangles_top: {
        id: 'app.lab.banner-designer.pattern.triangles-top',
        defaultMessage: 'Chief indented',
    },
    circle: { id: 'app.lab.banner-designer.pattern.circle', defaultMessage: 'Roundel' },
    rhombus: { id: 'app.lab.banner-designer.pattern.rhombus', defaultMessage: 'Lozenge' },
    border: { id: 'app.lab.banner-designer.pattern.border', defaultMessage: 'Bordure' },
    curly_border: {
        id: 'app.lab.banner-designer.pattern.curly-border',
        defaultMessage: 'Bordure indented',
    },
    bricks: { id: 'app.lab.banner-designer.pattern.bricks', defaultMessage: 'Field masoned' },
    gradient: { id: 'app.lab.banner-designer.pattern.gradient', defaultMessage: 'Gradient' },
    gradient_up: {
        id: 'app.lab.banner-designer.pattern.gradient-up',
        defaultMessage: 'Base gradient',
    },
    creeper: {
        id: 'app.lab.banner-designer.pattern.creeper',
        defaultMessage: 'Creeper charge',
    },
    skull: { id: 'app.lab.banner-designer.pattern.skull', defaultMessage: 'Skull charge' },
    flower: { id: 'app.lab.banner-designer.pattern.flower', defaultMessage: 'Flower charge' },
    mojang: { id: 'app.lab.banner-designer.pattern.mojang', defaultMessage: 'Thing' },
    globe: { id: 'app.lab.banner-designer.pattern.globe', defaultMessage: 'Globe' },
    piglin: { id: 'app.lab.banner-designer.pattern.piglin', defaultMessage: 'Snout' },
    flow: { id: 'app.lab.banner-designer.pattern.flow', defaultMessage: 'Flow' },
    guster: { id: 'app.lab.banner-designer.pattern.guster', defaultMessage: 'Guster' },
})

export const colorNameMessages: Record<DyeColor, MessageDescriptor> = defineMessages({
    white: { id: 'app.lab.banner-designer.color.white', defaultMessage: 'White' },
    orange: { id: 'app.lab.banner-designer.color.orange', defaultMessage: 'Orange' },
    magenta: { id: 'app.lab.banner-designer.color.magenta', defaultMessage: 'Magenta' },
    light_blue: { id: 'app.lab.banner-designer.color.light-blue', defaultMessage: 'Light blue' },
    yellow: { id: 'app.lab.banner-designer.color.yellow', defaultMessage: 'Yellow' },
    lime: { id: 'app.lab.banner-designer.color.lime', defaultMessage: 'Lime' },
    pink: { id: 'app.lab.banner-designer.color.pink', defaultMessage: 'Pink' },
    gray: { id: 'app.lab.banner-designer.color.gray', defaultMessage: 'Gray' },
    light_gray: {
        id: 'app.lab.banner-designer.color.light-gray',
        defaultMessage: 'Light gray',
    },
    cyan: { id: 'app.lab.banner-designer.color.cyan', defaultMessage: 'Cyan' },
    purple: { id: 'app.lab.banner-designer.color.purple', defaultMessage: 'Purple' },
    blue: { id: 'app.lab.banner-designer.color.blue', defaultMessage: 'Blue' },
    brown: { id: 'app.lab.banner-designer.color.brown', defaultMessage: 'Brown' },
    green: { id: 'app.lab.banner-designer.color.green', defaultMessage: 'Green' },
    red: { id: 'app.lab.banner-designer.color.red', defaultMessage: 'Red' },
    black: { id: 'app.lab.banner-designer.color.black', defaultMessage: 'Black' },
})

export const patternCategoryMessages: Record<BannerPatternCategory, MessageDescriptor> =
    defineMessages({
        base: {
            id: 'app.lab.banner-designer.pattern-category.base',
            defaultMessage: 'Base',
        },
        stripes: {
            id: 'app.lab.banner-designer.pattern-category.stripes',
            defaultMessage: 'Stripes',
        },
        halves: {
            id: 'app.lab.banner-designer.pattern-category.halves',
            defaultMessage: 'Halves',
        },
        shapes: {
            id: 'app.lab.banner-designer.pattern-category.shapes',
            defaultMessage: 'Shapes',
        },
        borders: {
            id: 'app.lab.banner-designer.pattern-category.borders',
            defaultMessage: 'Borders',
        },
        background: {
            id: 'app.lab.banner-designer.pattern-category.background',
            defaultMessage: 'Background',
        },
        gradients: {
            id: 'app.lab.banner-designer.pattern-category.gradients',
            defaultMessage: 'Gradients',
        },
        icons: {
            id: 'app.lab.banner-designer.pattern-category.icons',
            defaultMessage: 'Icons',
        },
    })

export const presetSectionMessages: Record<BannerPresetSection, MessageDescriptor> = defineMessages(
    {
        patterns: {
            id: 'app.lab.banner-designer.preset-section.patterns',
            defaultMessage: 'Banner patterns',
        },
        uppercase: {
            id: 'app.lab.banner-designer.preset-section.uppercase',
            defaultMessage: 'Uppercase letters',
        },
        lowercase: {
            id: 'app.lab.banner-designer.preset-section.lowercase',
            defaultMessage: 'Lowercase letters',
        },
        digits: {
            id: 'app.lab.banner-designer.preset-section.digits',
            defaultMessage: 'Numbers',
        },
        suits: {
            id: 'app.lab.banner-designer.preset-section.suits',
            defaultMessage: 'Card suits',
        },
    },
)

export const presetNameMessages: Record<string, MessageDescriptor> = defineMessages({
    'end-city': {
        id: 'app.lab.banner-designer.preset.end-city',
        defaultMessage: 'End City banner',
    },
    'mansion-bedroom': {
        id: 'app.lab.banner-designer.preset.mansion-bedroom',
        defaultMessage: 'Mansion bedroom banner',
    },
    creeper: { id: 'app.lab.banner-designer.preset.creeper', defaultMessage: 'Creeper charge' },
    skull: { id: 'app.lab.banner-designer.preset.skull', defaultMessage: 'Skull charge' },
    mojang: { id: 'app.lab.banner-designer.preset.mojang', defaultMessage: 'Mojang logo' },
    globe: { id: 'app.lab.banner-designer.preset.globe', defaultMessage: 'Globe' },
    snout: { id: 'app.lab.banner-designer.preset.snout', defaultMessage: 'Snout' },
    gradient: { id: 'app.lab.banner-designer.preset.gradient', defaultMessage: 'Gradient fade' },
    'suit-heart': { id: 'app.lab.banner-designer.preset.suit-heart', defaultMessage: 'Heart' },
    'suit-diamond': {
        id: 'app.lab.banner-designer.preset.suit-diamond',
        defaultMessage: 'Diamond',
    },
    'suit-spade': { id: 'app.lab.banner-designer.preset.suit-spade', defaultMessage: 'Spade' },
    'suit-club': { id: 'app.lab.banner-designer.preset.suit-club', defaultMessage: 'Club' },
})

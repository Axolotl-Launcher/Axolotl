import { expect, it } from 'vitest'

import {
    buildFontFamilyOptions,
    canonicalFontFamily,
    cssFontFamilyName,
    FOLLOW_DEFAULT_FONT,
    type FontFamilyOption,
    fontSettingToOptionValue,
    optionValueToFontSetting,
    resolveFontFamily,
    type SystemFontFamily,
} from './font-family.ts'

const FALLBACK = 'Inter, sans-serif'

function isDivider(option: ReturnType<typeof buildFontFamilyOptions>[number]): boolean {
    return !('value' in option)
}

function values(options: ReturnType<typeof buildFontFamilyOptions>): string[] {
    return options
        .filter((option): option is FontFamilyOption => 'value' in option)
        .map((o) => o.value)
}

it('quotes and escapes family names', () => {
    expect(cssFontFamilyName('Inter')).toBe('"Inter"')
    expect(cssFontFamilyName('  Fira Sans  ')).toBe('"Fira Sans"')
    expect(cssFontFamilyName('A"B')).toBe('"A\\"B"')
    expect(cssFontFamilyName('back\\slash')).toBe('"back\\\\slash"')
    expect(cssFontFamilyName('bad\u0000name')).toBe('"badname"')
})

it('rejects family names that cannot be used', () => {
    expect(cssFontFamilyName('')).toBe(null)
    expect(cssFontFamilyName('   ')).toBe(null)
    expect(cssFontFamilyName('x'.repeat(129))).toBe(null)
})

it('appends the fallback stack after the chosen family', () => {
    expect(resolveFontFamily(null, FALLBACK)).toBe(FALLBACK)
    expect(resolveFontFamily(undefined, FALLBACK)).toBe(FALLBACK)
    expect(resolveFontFamily('', FALLBACK)).toBe(FALLBACK)
    expect(resolveFontFamily('Fira Sans', FALLBACK)).toBe('"Fira Sans", Inter, sans-serif')
    expect(resolveFontFamily('x'.repeat(129), FALLBACK)).toBe(FALLBACK)
})

it('round trips the picker sentinel through the stored setting', () => {
    expect(fontSettingToOptionValue(null)).toBe(FOLLOW_DEFAULT_FONT)
    expect(fontSettingToOptionValue('Fira Code')).toBe('Fira Code')
    expect(optionValueToFontSetting(FOLLOW_DEFAULT_FONT)).toBe(null)
    expect(optionValueToFontSetting('  Fira Code  ')).toBe('Fira Code')
})

it('lists the default entry first and sorts the rest case insensitively', () => {
    const options = buildFontFamilyOptions(
        [
            { family: 'zeta', monospaced: false },
            { family: 'Alpha', monospaced: false },
        ],
        { defaultLabel: 'Launcher default' },
    )

    expect(values(options)).toEqual([FOLLOW_DEFAULT_FONT, 'Alpha', 'zeta'])
    expect((options[0] as FontFamilyOption).label).toBe('Launcher default')
})

it('merges duplicate families and keeps the monospace flag', () => {
    const fonts: SystemFontFamily[] = [
        { family: 'Fira Code', monospaced: false },
        { family: 'fira code', monospaced: true },
        { family: '   ', monospaced: false },
        { family: 'Inter', monospaced: false },
    ]

    const options = buildFontFamilyOptions(fonts, {
        defaultLabel: 'Default',
        monospaceLabel: 'Monospace',
        groupMonospaced: true,
    })

    expect(values(options)).toEqual([FOLLOW_DEFAULT_FONT, 'Fira Code', 'Inter'])
    expect((options[1] as FontFamilyOption).subLabel).toBe('Monospace')
    expect(isDivider(options[2])).toBe(true)
})

it('groups monospaced families ahead of the rest with a divider', () => {
    const options = buildFontFamilyOptions(
        [
            { family: 'Inter', monospaced: false },
            { family: 'Consolas', monospaced: true },
        ],
        {
            defaultLabel: 'Default',
            monospaceLabel: 'Monospace',
            groupMonospaced: true,
        },
    )

    expect(values(options)).toEqual([FOLLOW_DEFAULT_FONT, 'Consolas', 'Inter'])
    expect(isDivider(options[2])).toBe(true)
    expect((options[1] as FontFamilyOption).searchTerms?.includes('Monospace')).toBe(true)
    expect((options[3] as FontFamilyOption).subLabel).toBe(undefined)
})

it('omits the divider when only one group has entries', () => {
    const options = buildFontFamilyOptions([{ family: 'Consolas', monospaced: true }], {
        defaultLabel: 'Default',
        monospaceLabel: 'Monospace',
        groupMonospaced: true,
    })

    expect(options.some(isDivider)).toBe(false)
})

it('re-adds a selected family that is no longer installed', () => {
    const options = buildFontFamilyOptions([{ family: 'Inter', monospaced: false }], {
        defaultLabel: 'Default',
        missingLabel: 'Not installed',
        selected: 'JetBrains Mono',
    })

    expect(values(options)).toEqual([FOLLOW_DEFAULT_FONT, 'JetBrains Mono', 'Inter'])
    expect((options[1] as FontFamilyOption).subLabel).toBe('Not installed')
})

it('does not duplicate a selected family that is installed', () => {
    const options = buildFontFamilyOptions([{ family: 'JetBrains Mono', monospaced: true }], {
        defaultLabel: 'Default',
        missingLabel: 'Not installed',
        selected: 'JetBrains Mono',
    })

    expect(values(options)).toEqual([FOLLOW_DEFAULT_FONT, 'JetBrains Mono'])
    expect((options[1] as FontFamilyOption).subLabel).toBe(undefined)
})

it('canonicalises a stored family that differs only in case', () => {
    const fonts: SystemFontFamily[] = [{ family: 'JetBrains Mono', monospaced: true }]

    expect(canonicalFontFamily(fonts, 'jetbrains mono')).toBe('JetBrains Mono')
    expect(canonicalFontFamily(fonts, '  Inter  ')).toBe('Inter')
    expect(canonicalFontFamily(fonts, null)).toBe(FOLLOW_DEFAULT_FONT)
    expect(canonicalFontFamily(fonts, '')).toBe(FOLLOW_DEFAULT_FONT)
    expect(canonicalFontFamily(fonts, 'JetBrains Mono')).toBe('JetBrains Mono')
})

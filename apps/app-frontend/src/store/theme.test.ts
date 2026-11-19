import { expect, it } from 'vitest'

import {
    applySystemAccentHue,
    DEFAULT_CUSTOM_ACCENT_COLOR,
    deriveCustomThemeVariables,
    hexToHsl,
} from './theme.ts'

it('applies the system hue with the custom accent saturation and lightness', () => {
    const system = hexToHsl('#0078d4')
    const custom = hexToHsl(DEFAULT_CUSTOM_ACCENT_COLOR)
    const result = hexToHsl(applySystemAccentHue('#0078d4'))

    expect(Math.abs(result.h - system.h) < 1).toBeTruthy()
    expect(Math.abs(result.s - custom.s) < 1).toBeTruthy()
    expect(Math.abs(result.l - custom.l) < 1).toBeTruthy()
})

it('preserves hue values at the red boundary', () => {
    const result = hexToHsl(applySystemAccentHue('#ff0000'))

    expect(result.h < 1 || result.h > 359).toBeTruthy()
})

it('derives the surface ramp between the background and panel colors', () => {
    const variables = deriveCustomThemeVariables({
        background: '#101215',
        surface: '#262b34',
        text: '#ffffff',
    })

    expect(variables['--surface-1']).toBe('#101215')
    expect(variables['--surface-3']).toBe('#262b34')
    expect(variables['--surface-1-5']).toBe('#16181d')
    expect(variables['--surface-2']).toBe('#1b1f25')
    expect(variables['--surface-2-5']).toBe('#21252c')
    expect(variables['--color-text-primary']).toBe('#ffffff')
})

it('steps raised surfaces toward white on dark palettes and black on light ones', () => {
    const dark = deriveCustomThemeVariables({
        background: '#101215',
        surface: '#262b34',
        text: '#ffffff',
    })
    const light = deriveCustomThemeVariables({
        background: '#ebebeb',
        surface: '#c9c9c9',
        text: '#111111',
    })

    expect(Number.parseInt(dark['--surface-5'].slice(1), 16)).toBeGreaterThan(
        Number.parseInt(dark['--surface-3'].slice(1), 16),
    )
    expect(Number.parseInt(light['--surface-5'].slice(1), 16)).toBeLessThan(
        Number.parseInt(light['--surface-3'].slice(1), 16),
    )
})

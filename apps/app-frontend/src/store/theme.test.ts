import { expect, it } from 'vitest'

import { applySystemAccentHue, DEFAULT_CUSTOM_ACCENT_COLOR, hexToHsl } from './theme.ts'

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

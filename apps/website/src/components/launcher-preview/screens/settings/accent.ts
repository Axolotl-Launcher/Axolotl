/**
 * Accent colour maths of the desktop app's theme store
 * (apps/app-frontend/src/store/theme.ts): the presets, the `custom:#rrggbb`
 * setting, the system-accent hue transfer and the two variants derived from a
 * base colour (one clamped dark enough for light surfaces, one lifted for dark).
 */

export const ACCENT_COLOR_OPTIONS = ['pink', 'orange', 'green', 'blue', 'purple'] as const
export const DEFAULT_CUSTOM_ACCENT_COLOR = '#db2777'

export type AccentColor = (typeof ACCENT_COLOR_OPTIONS)[number]
export type CustomAccentColor = `custom:#${string}`
export type AccentColorSetting = AccentColor | 'system' | CustomAccentColor

export function parseCustomAccentColor(value: string): string | null {
    const match = /^custom:(#[0-9a-fA-F]{6})$/.exec(value)
    return match ? match[1].toLowerCase() : null
}

export function hexToHsl(hex: string): { h: number; s: number; l: number } {
    const n = parseInt(hex.slice(1), 16)
    const r = ((n >> 16) & 255) / 255
    const g = ((n >> 8) & 255) / 255
    const b = (n & 255) / 255
    const max = Math.max(r, g, b)
    const min = Math.min(r, g, b)
    const l = (max + min) / 2
    const d = max - min
    const s = d === 0 ? 0 : d / (1 - Math.abs(2 * l - 1))
    let h = 0
    if (d !== 0) {
        if (max === r) h = ((g - b) / d) % 6
        else if (max === g) h = (b - r) / d + 2
        else h = (r - g) / d + 4
        h *= 60
        if (h < 0) h += 360
    }
    return { h, s: s * 100, l: l * 100 }
}

export function hslToHex(h: number, s: number, l: number): string {
    const sn = s / 100
    const ln = l / 100
    const c = (1 - Math.abs(2 * ln - 1)) * sn
    const hp = (((h % 360) + 360) % 360) / 60
    const x = c * (1 - Math.abs((hp % 2) - 1))
    let [r, g, b] = [0, 0, 0]
    if (hp < 1) [r, g, b] = [c, x, 0]
    else if (hp < 2) [r, g, b] = [x, c, 0]
    else if (hp < 3) [r, g, b] = [0, c, x]
    else if (hp < 4) [r, g, b] = [0, x, c]
    else if (hp < 5) [r, g, b] = [x, 0, c]
    else [r, g, b] = [c, 0, x]
    const m = ln - c / 2
    const toHex = (channel: number) =>
        Math.round((channel + m) * 255)
            .toString(16)
            .padStart(2, '0')
    return `#${toHex(r)}${toHex(g)}${toHex(b)}`
}

/** Keeps the system colour's hue, at the saturation and lightness of the default accent. */
export function applySystemAccentHue(systemHex: string): string {
    const { h } = hexToHsl(systemHex)
    const { s, l } = hexToHsl(DEFAULT_CUSTOM_ACCENT_COLOR)
    return hslToHex(h, s, l)
}

const clamp = (value: number, min: number, max: number) => Math.min(Math.max(value, min), max)

export function deriveAccentVariants(hex: string): { light: string; dark: string } {
    const { h, s, l } = hexToHsl(hex)
    const lightL = clamp(l, 34, 57)
    return {
        light: hslToHex(h, s, lightL),
        dark: hslToHex(h, Math.min(s * 1.15, 100), Math.min(lightL + 18, 80)),
    }
}

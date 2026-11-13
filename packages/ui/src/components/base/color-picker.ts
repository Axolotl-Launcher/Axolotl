/**
 * Colour maths and palette data for `ColorPicker.vue`.
 *
 * Kept free of Vue so the conversions can be tested in node, and free of app
 * imports so the website can load the component without the desktop runtime.
 */

/** Hue in degrees, saturation and value as 0–1 fractions. */
export interface HsvColor {
    h: number
    s: number
    v: number
}

export interface DyeColorPreset {
    /** Vanilla dye identifier, used to look up the localized colour name. */
    id: string
    /** `#rrggbb`, matching the dye colours listed on the Minecraft Wiki. */
    hex: string
}

/** The sixteen vanilla dye colours, in the order the dye items are registered. */
export const MINECRAFT_DYE_COLORS: readonly DyeColorPreset[] = [
    { id: 'white', hex: '#f9fffe' },
    { id: 'orange', hex: '#f9801d' },
    { id: 'magenta', hex: '#c74ebd' },
    { id: 'light_blue', hex: '#3ab3da' },
    { id: 'yellow', hex: '#fed83d' },
    { id: 'lime', hex: '#80c71f' },
    { id: 'pink', hex: '#f38baa' },
    { id: 'gray', hex: '#474f52' },
    { id: 'light_gray', hex: '#9d9d97' },
    { id: 'cyan', hex: '#169c9c' },
    { id: 'purple', hex: '#8932b8' },
    { id: 'blue', hex: '#3c44aa' },
    { id: 'brown', hex: '#835432' },
    { id: 'green', hex: '#5e7c16' },
    { id: 'red', hex: '#b02e26' },
    { id: 'black', hex: '#1d1d21' },
]

export const RECENT_COLORS_LIMIT = 8
export const RECENT_COLORS_STORAGE_KEY = 'axolotl:color-picker:recent'

/**
 * Accepts `abc`, `#abc`, `aabbcc` and `#aabbcc` and returns the lowercase
 * six-digit form, or null when the value is not a hex colour.
 */
export function normalizeHex(value: string): string | null {
    const trimmed = value.trim().replace(/^#/, '')
    if (/^[0-9a-f]{3}$/i.test(trimmed)) {
        const [red, green, blue] = trimmed.toLowerCase()
        return `#${red}${red}${green}${green}${blue}${blue}`
    }
    if (/^[0-9a-f]{6}$/i.test(trimmed)) return `#${trimmed.toLowerCase()}`
    return null
}

export function hexToRgb(value: string): [number, number, number] | null {
    const hex = normalizeHex(value)
    if (!hex) return null
    return [
        Number.parseInt(hex.slice(1, 3), 16),
        Number.parseInt(hex.slice(3, 5), 16),
        Number.parseInt(hex.slice(5, 7), 16),
    ]
}

export function rgbToHex(red: number, green: number, blue: number): string {
    const channel = (value: number) =>
        Math.min(255, Math.max(0, Math.round(value)))
            .toString(16)
            .padStart(2, '0')
    return `#${channel(red)}${channel(green)}${channel(blue)}`
}

export function rgbToHsv(red: number, green: number, blue: number): HsvColor {
    const r = red / 255
    const g = green / 255
    const b = blue / 255
    const max = Math.max(r, g, b)
    const delta = max - Math.min(r, g, b)
    let h = 0
    if (delta !== 0) {
        if (max === r) h = ((g - b) / delta) % 6
        else if (max === g) h = (b - r) / delta + 2
        else h = (r - g) / delta + 4
        h *= 60
        if (h < 0) h += 360
    }
    return { h, s: max === 0 ? 0 : delta / max, v: max }
}

export function hsvToRgb(hue: number, saturation: number, value: number): [number, number, number] {
    const h = ((hue % 360) + 360) % 360
    const s = Math.min(1, Math.max(0, saturation))
    const v = Math.min(1, Math.max(0, value))
    const chroma = v * s
    const second = chroma * (1 - Math.abs(((h / 60) % 2) - 1))
    const floor = v - chroma
    const [r, g, b] =
        h < 60
            ? [chroma, second, 0]
            : h < 120
              ? [second, chroma, 0]
              : h < 180
                ? [0, chroma, second]
                : h < 240
                  ? [0, second, chroma]
                  : h < 300
                    ? [second, 0, chroma]
                    : [chroma, 0, second]
    return [
        Math.round((r + floor) * 255),
        Math.round((g + floor) * 255),
        Math.round((b + floor) * 255),
    ]
}

export function hexToHsv(value: string): HsvColor | null {
    const rgb = hexToRgb(value)
    return rgb ? rgbToHsv(...rgb) : null
}

export function hsvToHex(hue: number, saturation: number, value: number): string {
    return rgbToHex(...hsvToRgb(hue, saturation, value))
}

/**
 * Reads the shared recent-colour list. A missing, blocked or malformed store
 * yields an empty list rather than throwing, because the picker still has to
 * work in a webview that denies storage.
 */
export function readRecentColors(
    storageKey: string = RECENT_COLORS_STORAGE_KEY,
    limit: number = RECENT_COLORS_LIMIT,
): string[] {
    if (limit <= 0) return []
    const store = resolveStorage()
    if (!store) return []
    try {
        const parsed: unknown = JSON.parse(store.getItem(storageKey) ?? '[]')
        if (!Array.isArray(parsed)) return []
        const colors: string[] = []
        for (const entry of parsed) {
            if (typeof entry !== 'string') continue
            const hex = normalizeHex(entry)
            if (!hex || colors.includes(hex)) continue
            colors.push(hex)
            if (colors.length >= limit) break
        }
        return colors
    } catch {
        return []
    }
}

/**
 * Merges recent-colour lists into one, newest first, keeping the first
 * occurrence of each colour and stopping at `limit`.
 *
 * Storage can accept a write and still report the previous list, so the caller
 * keeps its in-session list and merges it with what storage returns instead of
 * trusting either source alone.
 */
export function mergeRecentColors(lists: readonly string[][], limit: number): string[] {
    const merged: string[] = []
    if (limit <= 0) return merged
    for (const list of lists) {
        for (const entry of list) {
            const hex = normalizeHex(entry)
            if (!hex || merged.includes(hex)) continue
            merged.push(hex)
            if (merged.length >= limit) return merged
        }
    }
    return merged
}

/**
 * Moves `value` to the front of the recent list, dropping the oldest entry past
 * the limit. Returns the updated list even when persisting fails.
 */
export function rememberRecentColor(
    value: string,
    storageKey: string = RECENT_COLORS_STORAGE_KEY,
    limit: number = RECENT_COLORS_LIMIT,
): string[] {
    const existing = readRecentColors(storageKey, limit)
    const hex = normalizeHex(value)
    if (!hex || limit <= 0) return existing
    const next = [hex, ...existing.filter((color) => color !== hex)].slice(0, limit)
    const store = resolveStorage()
    if (store) {
        try {
            store.setItem(storageKey, JSON.stringify(next))
        } catch {
            // A full or read-only store keeps the in-memory list working.
        }
    }
    return next
}

function resolveStorage(): Storage | null {
    try {
        // The probe has to sit inside the guard too: `typeof` avoids a
        // ReferenceError for an undeclared name, but a webview that blocks
        // storage still evaluates the getter and throws.
        return typeof localStorage === 'undefined' ? null : localStorage
    } catch {
        return null
    }
}

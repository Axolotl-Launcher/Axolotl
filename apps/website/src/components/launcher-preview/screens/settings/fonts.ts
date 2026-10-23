/**
 * Font family plumbing of the desktop app (apps/app-frontend/src/helpers/font-family.ts),
 * unchanged, plus the installed-family list `plugin:fonts|fonts_get_system_fonts`
 * returns. The preview window stands for a Mac, so the list is the families a
 * stock macOS install exposes to font pickers.
 */

/** Mirrors `--font-standard` in `packages/assets/styles/defaults.scss`. */
export const DEFAULT_UI_FONT_STACK =
    'Inter, -apple-system, BlinkMacSystemFont, Segoe UI, Oxygen, Ubuntu, Roboto, Cantarell, Fira Sans, Droid Sans, Helvetica Neue, sans-serif'

/** Mirrors `--mono-font` in `packages/assets/styles/defaults.scss`. */
export const DEFAULT_MONO_FONT_STACK =
    'ui-monospace, SFMono-Regular, SF Mono, Menlo, Consolas, Liberation Mono, monospace'

/** Picker value for "follow the launcher default"; persisted as `null`. */
export const FOLLOW_DEFAULT_FONT = ''

export interface SystemFontFamily {
    family: string
    monospaced: boolean
}

export interface FontFamilyOption {
    value: string
    label: string
    subLabel?: string
    searchTerms?: string[]
}

export type FontFamilyOptionOrDivider = FontFamilyOption | { type: 'divider' }

export interface FontFamilyOptionsConfig {
    defaultLabel: string
    selected?: string | null
    missingLabel?: string
    monospaceLabel?: string
    groupMonospaced?: boolean
}

const MAX_FONT_FAMILY_LENGTH = 128

export function cssFontFamilyName(name: string): string | null {
    const trimmed = name.replace(/\p{Cc}/gu, '').trim()
    if (!trimmed || trimmed.length > MAX_FONT_FAMILY_LENGTH) return null
    return `"${trimmed.replace(/\\/g, '\\\\').replace(/"/g, '\\"')}"`
}

export function resolveFontFamily(setting: string | null | undefined, fallbackStack: string) {
    if (!setting) return fallbackStack
    const family = cssFontFamilyName(setting)
    return family ? `${family}, ${fallbackStack}` : fallbackStack
}

export function optionValueToFontSetting(value: string): string | null {
    const trimmed = value.trim()
    return trimmed === FOLLOW_DEFAULT_FONT ? null : trimmed
}

function compareFamilies(first: SystemFontFamily, second: SystemFontFamily): number {
    const left = first.family.toLowerCase()
    const right = second.family.toLowerCase()
    if (left === right) return 0
    return left < right ? -1 : 1
}

function toOption(family: SystemFontFamily, monospaceLabel?: string): FontFamilyOption {
    return {
        value: family.family,
        label: family.family,
        ...(family.monospaced && monospaceLabel
            ? { subLabel: monospaceLabel, searchTerms: [monospaceLabel] }
            : {}),
    }
}

export function canonicalFontFamily(
    fonts: SystemFontFamily[],
    setting: string | null | undefined,
): string {
    const selected = optionValueToFontSetting(setting ?? '')
    if (!selected) return FOLLOW_DEFAULT_FONT
    const listed = fonts.find((font) => font.family.trim().toLowerCase() === selected.toLowerCase())
    return listed ? listed.family.trim() : selected
}

export function buildFontFamilyOptions(
    fonts: SystemFontFamily[],
    config: FontFamilyOptionsConfig,
): FontFamilyOptionOrDivider[] {
    const families = new Map<string, SystemFontFamily>()
    for (const font of fonts) {
        const family = font.family.trim()
        if (!family) continue
        const key = family.toLowerCase()
        const known = families.get(key)
        if (known) known.monospaced ||= font.monospaced
        else families.set(key, { family, monospaced: font.monospaced })
    }

    const sorted = [...families.values()].sort(compareFamilies)
    const selected = optionValueToFontSetting(config.selected ?? '')
    const options: FontFamilyOptionOrDivider[] = [
        { value: FOLLOW_DEFAULT_FONT, label: config.defaultLabel },
    ]

    if (selected && !families.has(selected.toLowerCase()) && config.missingLabel) {
        options.push({ value: selected, label: selected, subLabel: config.missingLabel })
    }

    if (config.groupMonospaced) {
        const monospaced = sorted.filter((font) => font.monospaced)
        const proportional = sorted.filter((font) => !font.monospaced)
        options.push(...monospaced.map((font) => toOption(font, config.monospaceLabel)))
        if (monospaced.length > 0 && proportional.length > 0) options.push({ type: 'divider' })
        options.push(...proportional.map((font) => toOption(font, config.monospaceLabel)))
        return options
    }

    options.push(...sorted.map((font) => toOption(font)))
    return options
}

/** What the host's font scan reports on a stock macOS install. */
export const PREVIEW_SYSTEM_FONTS: SystemFontFamily[] = [
    { family: 'American Typewriter', monospaced: false },
    { family: 'Andale Mono', monospaced: true },
    { family: 'Arial', monospaced: false },
    { family: 'Arial Rounded MT Bold', monospaced: false },
    { family: 'Avenir', monospaced: false },
    { family: 'Avenir Next', monospaced: false },
    { family: 'Baskerville', monospaced: false },
    { family: 'Charter', monospaced: false },
    { family: 'Courier', monospaced: true },
    { family: 'Courier New', monospaced: true },
    { family: 'Didot', monospaced: false },
    { family: 'Futura', monospaced: false },
    { family: 'Geneva', monospaced: false },
    { family: 'Georgia', monospaced: false },
    { family: 'Gill Sans', monospaced: false },
    { family: 'Helvetica', monospaced: false },
    { family: 'Helvetica Neue', monospaced: false },
    { family: 'Hiragino Sans', monospaced: false },
    { family: 'Hiragino Sans GB', monospaced: false },
    { family: 'Menlo', monospaced: true },
    { family: 'Monaco', monospaced: true },
    { family: 'Optima', monospaced: false },
    { family: 'Palatino', monospaced: false },
    { family: 'PingFang SC', monospaced: false },
    { family: 'PingFang TC', monospaced: false },
    { family: 'PT Mono', monospaced: true },
    { family: 'Songti SC', monospaced: false },
    { family: 'Times New Roman', monospaced: false },
    { family: 'Trebuchet MS', monospaced: false },
    { family: 'Verdana', monospaced: false },
]

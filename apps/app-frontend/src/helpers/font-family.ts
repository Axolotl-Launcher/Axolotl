/**
 * Font family plumbing shared by the appearance settings pickers and the theme
 * store. Kept free of imports so `node --test` can load it directly.
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
    localized?: string
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

/**
 * Quotes and escapes an installed family name so it cannot break out of the
 * font-family declaration it is spliced into. Returns null when the name has
 * nothing usable left.
 */
export function cssFontFamilyName(name: string): string | null {
    const trimmed = name.replace(/\p{Cc}/gu, '').trim()

    if (!trimmed || trimmed.length > MAX_FONT_FAMILY_LENGTH) {
        return null
    }

    return `"${trimmed.replace(/\\/g, '\\\\').replace(/"/g, '\\"')}"`
}

/**
 * The value written into `--font-standard` / `--mono-font`. The launcher stack
 * always trails the chosen family so an unresolvable name degrades to the
 * previous default instead of the user agent's own font.
 */
export function resolveFontFamily(
    setting: string | null | undefined,
    fallbackStack: string,
): string {
    if (!setting) {
        return fallbackStack
    }

    const family = cssFontFamilyName(setting)

    return family ? `${family}, ${fallbackStack}` : fallbackStack
}

export function fontSettingToOptionValue(setting: string | null | undefined): string {
    return setting ?? FOLLOW_DEFAULT_FONT
}

export function optionValueToFontSetting(value: string): string | null {
    const trimmed = value.trim()

    return trimmed === FOLLOW_DEFAULT_FONT ? null : trimmed
}

function compareFamilies(first: SystemFontFamily, second: SystemFontFamily): number {
    const left = first.family.toLowerCase()
    const right = second.family.toLowerCase()

    if (left === right) {
        return 0
    }

    return left < right ? -1 : 1
}

function toOption(family: SystemFontFamily, monospaceLabel?: string): FontFamilyOption {
    const label = family.localized ?? family.family
    const subLabel = family.monospaced ? monospaceLabel : undefined
    const searchTerms = label === family.family ? [] : [family.family]
    if (subLabel) searchTerms.push(subLabel)

    return {
        value: family.family,
        label,
        ...(subLabel ? { subLabel } : {}),
        ...(searchTerms.length ? { searchTerms } : {}),
    }
}

/**
 * The spelling the font list uses for a stored family. Font names are matched
 * case-insensitively so a stored value that only differs in case still selects
 * its option instead of leaving the picker blank.
 */
export function canonicalFontFamily(
    fonts: SystemFontFamily[],
    setting: string | null | undefined,
): string {
    const selected = optionValueToFontSetting(setting ?? '')
    if (!selected) {
        return FOLLOW_DEFAULT_FONT
    }

    const listed = fonts.find((font) => font.family.trim().toLowerCase() === selected.toLowerCase())

    return listed ? listed.family.trim() : selected
}

/**
 * Builds the picker options for the installed font collection: the default
 * entry first, the current selection re-added when it is no longer installed,
 * and — for the monospace picker — monospaced families ahead of the rest.
 */
export function buildFontFamilyOptions(
    fonts: SystemFontFamily[],
    config: FontFamilyOptionsConfig,
): FontFamilyOptionOrDivider[] {
    const families = new Map<string, SystemFontFamily>()
    for (const font of fonts) {
        const family = font.family.trim()

        if (!family) {
            continue
        }

        const key = family.toLowerCase()
        const known = families.get(key)
        const localized = font.localized?.trim() || undefined

        if (known) {
            known.monospaced ||= font.monospaced
            if (!known.localized && localized && localized !== known.family) {
                known.localized = localized
            }
        } else {
            families.set(key, {
                family,
                monospaced: font.monospaced,
                ...(localized && localized !== family ? { localized } : {}),
            })
        }
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

        if (monospaced.length > 0 && proportional.length > 0) {
            options.push({ type: 'divider' })
        }

        options.push(...proportional.map((font) => toOption(font, config.monospaceLabel)))

        return options
    }

    options.push(...sorted.map((font) => toOption(font)))

    return options
}

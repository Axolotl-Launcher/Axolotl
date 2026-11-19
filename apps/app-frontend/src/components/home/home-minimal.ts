/**
 * What the minimal Home shows and how it lays it out.
 *
 * The options live in the dashboard config (`settings.home_widgets.minimal`),
 * a JSON value the backend stores as is, so they need no schema change. A
 * config without them reads as the standard preset: the minimal Home looks the
 * way it did before the options existed.
 */

export const HOME_MINIMAL_PRESETS = ['standard', 'pure', 'classic', 'custom'] as const
export const HOME_MINIMAL_ELEMENTS = [
    'date',
    'greeting',
    'status',
    'meta',
    'switch',
    'recent',
] as const
export const HOME_MINIMAL_SURFACES = ['card', 'plain'] as const
export const HOME_MINIMAL_ARRANGEMENTS = ['row', 'stacked', 'bar'] as const
export const HOME_MINIMAL_POSITIONS = ['upper', 'center'] as const
export const HOME_MINIMAL_DENSITIES = ['comfortable', 'compact'] as const

export type HomeMinimalPreset = (typeof HOME_MINIMAL_PRESETS)[number]
export type HomeMinimalElement = (typeof HOME_MINIMAL_ELEMENTS)[number]
export type HomeMinimalSurface = (typeof HOME_MINIMAL_SURFACES)[number]
export type HomeMinimalArrangement = (typeof HOME_MINIMAL_ARRANGEMENTS)[number]
export type HomeMinimalPosition = (typeof HOME_MINIMAL_POSITIONS)[number]
export type HomeMinimalDensity = (typeof HOME_MINIMAL_DENSITIES)[number]

export type HomeMinimalLayout = {
    /** Which parts are shown. The icon, the name and the launch button always are. */
    show: Record<HomeMinimalElement, boolean>
    /** `card` sits the instance on a widget surface; `plain` puts it on the page. */
    surface: HomeMinimalSurface
    /**
     * `row`: a large icon beside the text; `stacked`: icon, name and button in
     * one centred column; `bar`: one slim horizontal strip, small icon, name and
     * details on the left, the launch button on the right (the layout the
     * minimal Home had before the redesign).
     */
    arrangement: HomeMinimalArrangement
    /** `upper` sits a little above the middle (optical centre); `center` is the true middle. */
    position: HomeMinimalPosition
    density: HomeMinimalDensity
}

export type HomeMinimalOptions = HomeMinimalLayout & { preset: HomeMinimalPreset }

/** The minimal Home as it shipped: everything on, on a card. */
export const HOME_MINIMAL_STANDARD: HomeMinimalLayout = {
    show: { date: true, greeting: true, status: true, meta: true, switch: true, recent: true },
    surface: 'card',
    arrangement: 'row',
    position: 'upper',
    density: 'comfortable',
}

/** One launchable instance in the middle of the page and nothing else. */
export const HOME_MINIMAL_PURE: HomeMinimalLayout = {
    show: {
        date: false,
        greeting: false,
        status: false,
        meta: false,
        switch: false,
        recent: false,
    },
    surface: 'plain',
    arrangement: 'stacked',
    position: 'center',
    density: 'comfortable',
}

/**
 * The minimal Home as it was before the redesign: the greeting with a short
 * accent rule, then one slim strip with the instance and its launch button.
 */
export const HOME_MINIMAL_CLASSIC: HomeMinimalLayout = {
    show: {
        date: false,
        greeting: true,
        status: false,
        meta: true,
        switch: true,
        recent: false,
    },
    surface: 'card',
    arrangement: 'bar',
    position: 'upper',
    density: 'comfortable',
}

const PRESET_LAYOUTS: Record<Exclude<HomeMinimalPreset, 'custom'>, HomeMinimalLayout> = {
    standard: HOME_MINIMAL_STANDARD,
    pure: HOME_MINIMAL_PURE,
    classic: HOME_MINIMAL_CLASSIC,
}

export const HOME_MINIMAL_DEFAULT_OPTIONS: HomeMinimalOptions = {
    preset: 'standard',
    ...cloneLayout(HOME_MINIMAL_STANDARD),
}

function cloneLayout(layout: HomeMinimalLayout): HomeMinimalLayout {
    return { ...layout, show: { ...layout.show } }
}

function isRecord(value: unknown): value is Record<string, unknown> {
    return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function pick<T extends string>(values: readonly T[], value: unknown, fallback: T): T {
    return values.includes(value as T) ? (value as T) : fallback
}

/**
 * Reads saved options. Anything missing or unknown falls back to the standard
 * layout, so a value from an older or newer version still opens a working page.
 */
export function normalizeHomeMinimalOptions(value: unknown): HomeMinimalOptions {
    if (!isRecord(value))
        return { ...HOME_MINIMAL_DEFAULT_OPTIONS, show: { ...HOME_MINIMAL_STANDARD.show } }

    const preset = pick(HOME_MINIMAL_PRESETS, value.preset, 'standard')
    const savedShow = isRecord(value.show) ? value.show : {}
    const show = Object.fromEntries(
        HOME_MINIMAL_ELEMENTS.map((element) => [
            element,
            typeof savedShow[element] === 'boolean'
                ? savedShow[element]
                : HOME_MINIMAL_STANDARD.show[element],
        ]),
    ) as Record<HomeMinimalElement, boolean>

    return {
        preset,
        show,
        surface: pick(HOME_MINIMAL_SURFACES, value.surface, HOME_MINIMAL_STANDARD.surface),
        arrangement: pick(
            HOME_MINIMAL_ARRANGEMENTS,
            value.arrangement,
            HOME_MINIMAL_STANDARD.arrangement,
        ),
        position: pick(HOME_MINIMAL_POSITIONS, value.position, HOME_MINIMAL_STANDARD.position),
        density: pick(HOME_MINIMAL_DENSITIES, value.density, HOME_MINIMAL_STANDARD.density),
    }
}

/** The layout the page draws: a preset's own values, or the saved custom ones. */
export function resolveHomeMinimalLayout(options: HomeMinimalOptions | null | undefined) {
    const normalized = options ?? HOME_MINIMAL_DEFAULT_OPTIONS
    if (normalized.preset !== 'custom') return cloneLayout(PRESET_LAYOUTS[normalized.preset])
    const { preset: _preset, ...layout } = normalized
    return cloneLayout(layout)
}

/**
 * Choosing a preset. Choosing "custom" starts from what is on screen, so the
 * page does not jump when the user begins to adjust it.
 */
export function setHomeMinimalPreset(
    options: HomeMinimalOptions | null | undefined,
    preset: HomeMinimalPreset,
): HomeMinimalOptions {
    if (preset === 'custom') return { preset, ...resolveHomeMinimalLayout(options) }
    return { preset, ...cloneLayout(PRESET_LAYOUTS[preset]) }
}

/** Changing one option makes the layout custom, starting from what is on screen. */
export function updateHomeMinimalLayout(
    options: HomeMinimalOptions | null | undefined,
    patch: Partial<Omit<HomeMinimalLayout, 'show'>> & {
        show?: Partial<Record<HomeMinimalElement, boolean>>
    },
): HomeMinimalOptions {
    const current = resolveHomeMinimalLayout(options)
    return {
        preset: 'custom',
        ...current,
        ...patch,
        show: { ...current.show, ...patch.show },
    }
}

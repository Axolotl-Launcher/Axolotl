export type SkinEditorTheme = {
    dark: boolean
    colors: Record<string, string>
    metrics: Record<string, string>
}

const themeMetricMap = {
    'gap-xs': '--gap-xs',
    'gap-sm': '--gap-sm',
    'gap-md': '--gap-md',
    'gap-lg': '--gap-lg',
    'radius-xs': '--radius-xs',
    'radius-sm': '--radius-sm',
    'radius-md': '--radius-md',
} as const

const themeColorMap = {
    ui: '--surface-2',
    back: '--surface-1',
    dark: '--surface-1',
    border: '--surface-5',
    selected: '--surface-3',
    elevated: '--surface-3',
    button: '--surface-4',
    bright_ui: '--surface-4',
    accent: '--color-brand',
    accent_highlight: '--color-brand-highlight',
    focus_ring: '--color-focus-ring',
    hover: '--surface-3',
    frame: '--surface-1',
    text: '--color-text-default',
    light: '--color-text-primary',
    accent_text: '--color-accent-contrast',
    bright_ui_text: '--color-text-primary',
    subtle_text: '--color-text-tertiary',
    grid: '--surface-5',
    wireframe: '--color-text-tertiary',
    checkerboard: '--surface-1-5',
    menu_separator: '--surface-5',
    bright_border: '--surface-5',
} as const

function readVariables(styles: CSSStyleDeclaration, variables: Record<string, string>) {
    return Object.fromEntries(
        Object.entries(variables).map(([name, variable]) => [
            name,
            styles.getPropertyValue(variable).trim(),
        ]),
    )
}

/**
 * `getComputedStyle().getPropertyValue()` returns a custom property's token
 * stream with `var()` substituted but *not* evaluated, so since the opacity model
 * landed `--color-brand-highlight` comes back as an unresolved
 * `color-mix(… , transparent)`. The Blockbench iframe assigns the string straight
 * to `background-color`, where it is invalid -- the `calc()` rung inside the mix
 * is not a valid `color-mix()` percentage there -- so the tint disappears.
 *
 * Resolve it to a concrete colour through a probe element's computed `color`,
 * which browsers always report as a used `rgb()`/`rgba()` value. The probe is a
 * child of `element` so it inherits the same token scope the variables are read
 * from, and it is removed before returning.
 */
function resolveColor(element: HTMLElement, variable: string): string {
    const probe = document.createElement('span')
    probe.style.color = `var(${variable})`
    probe.style.display = 'none'
    element.appendChild(probe)
    const resolved = getComputedStyle(probe).color
    probe.remove()
    return resolved
}

/**
 * Port of the app's `createSkinEditorTheme`. The launcher window carries the
 * theme classes and tokens in the preview, so they are read from it instead of
 * the document root.
 */
export function createSkinEditorTheme(root: HTMLElement): SkinEditorTheme {
    const styles = getComputedStyle(root)
    return {
        dark: root.classList.contains('dark-mode') || root.classList.contains('oled-mode'),
        colors: {
            ...readVariables(styles, themeColorMap),
            // Overrides the raw `color-mix()` that `readVariables` would return.
            accent_highlight: resolveColor(root, '--color-brand-highlight'),
        },
        metrics: readVariables(styles, themeMetricMap),
    }
}

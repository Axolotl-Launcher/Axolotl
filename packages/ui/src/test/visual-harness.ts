/**
 * The app and website load the design tokens through their own entry
 * stylesheets (`omorphia.scss` / `global.scss`), which a component-only mount
 * bypasses. Without these imports a visual assertion would measure unstyled
 * markup and pass regardless of the token values.
 */
// Import the SCSS entry directly from this module: Vite compiles it on import,
// whereas going through an intermediate stylesheet does not run the Sass
// compiler and silently yields no tokens. This mirrors the app entry, so both
// the theme tokens and the Tailwind utilities (`h-8`, `rounded-xl`, `px-3`)
// resolve exactly as they do in production — without Tailwind the utility
// classes are inert and any assertion about them would be vacuous.
import './visual-styles.scss'

import { mount, type MountingOptions } from '@vue/test-utils'
import type { Component } from 'vue'
import { ref } from 'vue'

import { tooltipDirective } from '../directives/tooltip'
import { I18N_INJECTION_KEY } from '../providers/i18n'

export const THEMES = ['light', 'dark', 'oled'] as const
export type Theme = (typeof THEMES)[number]

/** The classes `store/theme.ts` toggles on `<html>` in the desktop app. */
const THEME_CLASSES = THEMES.map((theme) => `${theme}-mode`)

export function applyTheme(theme: Theme): void {
    const root = document.documentElement
    root.classList.remove(...THEME_CLASSES, 'accent-pink')
    root.classList.add(`${theme}-mode`)
}

/**
 * Verifies the harness itself before any assertion trusts it: if the token
 * layer failed to load, every computed style downstream would be meaningless.
 *
 * The tokens are theme-scoped (`.light-properties` / `.dark-mode`), so a theme
 * has to be applied first — reading them on a bare document reports nothing.
 */
export function assertTokensLoaded(theme: Theme = 'dark'): void {
    applyTheme(theme)
    const surface = computedToken(document.documentElement, '--surface-2')

    if (!surface) {
        throw new Error(
            'Design tokens did not load. Check that the visual-styles.css entry imports @modrinth/assets/omorphia.scss.',
        )
    }
}

export async function mountThemed<P extends Record<string, unknown>>(
    component: Component,
    props: P,
    theme: Theme,
    options: MountingOptions<P> = {},
) {
    applyTheme(theme)

    const wrapper = mount(component, {
        props,
        attachTo: document.body,
        ...options,
        global: {
            ...options.global,
            provide: {
                [I18N_INJECTION_KEY as symbol]: {
                    locale: ref('en-US'),
                    t: (key: string) => key,
                    setLocale: () => {},
                },
                ...options.global?.provide,
            },
            directives: {
                ...options.global?.directives,
                tooltip: tooltipDirective,
            },
        },
    })

    // Vue applies classes asynchronously; wait for the render to settle before
    // anyone reads layout or computed styles.
    await wrapper.vm.$nextTick()

    return wrapper
}

/** Reads a resolved custom property off an element, for token assertions. */
export function computedToken(element: Element, token: string): string {
    return getComputedStyle(element).getPropertyValue(token).trim()
}

/** Waits for a condition to hold, polling frames, for effects that settle late. */
export async function waitFor(
    predicate: () => boolean,
    { timeoutMs = 2000, label = 'condition' } = {},
): Promise<void> {
    const deadline = performance.now() + timeoutMs
    while (performance.now() < deadline) {
        if (predicate()) return
        await new Promise((resolve) => requestAnimationFrame(resolve))
    }
    throw new Error(`Timed out after ${timeoutMs}ms waiting for ${label}`)
}

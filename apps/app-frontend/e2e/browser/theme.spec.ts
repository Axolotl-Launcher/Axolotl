import { browser, expect } from '@wdio/globals'

/**
 * Theme regression coverage for the desktop shell.
 *
 * The launcher's `index.html` ships a static `dark-mode accent-pink` class that
 * `setThemeClass()` replaces at startup. That is the same "static class plus
 * runtime rewrite" shape that broke the website, where a pinned class and the
 * runtime class ended up on <html> together and the dark token block won the
 * cascade, leaving white text on a light background.
 *
 * The launcher currently removes every theme class before adding the selected
 * one, so the invariant holds today. These tests pin that behaviour down so a
 * refactor towards a plain overwrite cannot reintroduce the website failure.
 *
 * Every assertion drives the real `themeStore.setThemeClass()` action rather than
 * editing the class list by hand: a hand-written class change would pass even if
 * the store regressed.
 */

const THEME_CLASSES = ['light-mode', 'dark-mode', 'oled-mode']

/**
 * Run the product's own theme action and report the resulting class list.
 *
 * Returns `reachable: false` when the store cannot be found, so a silently
 * skipped assertion cannot masquerade as a pass.
 */
interface ThemeStore {
    selectedTheme: string
    setThemeClass: () => void
}

interface VueAppHandle {
    config?: { globalProperties?: { $pinia?: { _s?: Map<string, unknown> } } }
}

async function applyThemeThroughStore(mode: 'system' | 'light' | 'dark' | 'oled') {
    return browser.execute((next: string) => {
        // Vue and Pinia do not expose typed handles on the DOM node, so the shape
        // is declared narrowly here instead of reaching for `any`.
        const host = document.querySelector('#app') as
            (Element & { __vue_app__?: VueAppHandle }) | null
        const pinia = host?.__vue_app__?.config?.globalProperties?.$pinia
        const store = pinia?._s?.get('themeStore') as ThemeStore | undefined
        if (!store) {
            return { reachable: false, classes: [] as string[] }
        }
        store.selectedTheme = next
        store.setThemeClass()
        return {
            reachable: true,
            classes: Array.from(document.documentElement.classList).filter((name) =>
                ['light-mode', 'dark-mode', 'oled-mode'].includes(name),
            ),
        }
    }, mode)
}

async function themeClasses() {
    return browser.execute((known: string[]) => {
        return Array.from(document.documentElement.classList).filter((name) => known.includes(name))
    }, THEME_CLASSES)
}

describe('desktop theme resolution', () => {
    it('exposes the theme store the settings screen drives', async () => {
        const result = await applyThemeThroughStore('dark')

        // Guard the whole suite: if the store lookup breaks, every theme assertion
        // below would be vacuous.
        expect(result.reachable).toBe(true)
    })

    it('starts with exactly one theme class despite the static markup', async () => {
        const classes = await themeClasses()

        expect(classes).toHaveLength(1)
    })

    for (const mode of ['light', 'dark', 'oled'] as const) {
        it(`replaces the theme class instead of stacking a second one (${mode})`, async () => {
            const result = await applyThemeThroughStore(mode)
            expect(result.reachable).toBe(true)

            // Exactly one: two classes is the website failure shape, where the later
            // token block wins and the surface and text come from different themes.
            expect(result.classes).toHaveLength(1)
            expect(result.classes[0]).toBe(`${mode}-mode`)
        })
    }

    it('keeps a single class when switching between two themes', async () => {
        const light = await applyThemeThroughStore('light')
        const dark = await applyThemeThroughStore('dark')

        expect(light.reachable).toBe(true)
        expect(dark.reachable).toBe(true)
        expect(light.classes).toHaveLength(1)
        expect(light.classes[0]).toBe('light-mode')
        expect(dark.classes).toHaveLength(1)
        expect(dark.classes[0]).toBe('dark-mode')
    })

    it('resolves surface and text tokens from the same theme', async () => {
        await applyThemeThroughStore('light')

        const tokens = await browser.execute(() => {
            const root = getComputedStyle(document.documentElement)
            return {
                surface: root.getPropertyValue('--surface-1').trim(),
                textPrimary: root.getPropertyValue('--color-text-primary').trim(),
            }
        })

        // Both tokens must exist and describe the light theme. A text colour from
        // the dark block over a light surface is exactly what the website shipped.
        expect(tokens.surface.toLowerCase()).toBe('#ebebeb')
        expect(tokens.textPrimary).toBeTruthy()
        expect(tokens.textPrimary.toLowerCase()).not.toBe('#fff7fb')
    })
})

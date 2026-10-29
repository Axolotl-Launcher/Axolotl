import { browser, expect } from '@wdio/globals'

/**
 * Shell behaviour that a component test cannot observe.
 *
 * The component visual specs mount a single component and assert its geometry.
 * That leaves the shell untested: how the theme class is resolved in a real
 * renderer, whether the app mounts a screen, and whether routing works. A
 * light-theme regression once shipped through exactly this gap on the website,
 * where two theme classes ended up on <html> and the dark token block won the
 * cascade, painting white text on a light background.
 *
 * `index.html` ships a static `dark-mode accent-pink` class that the app replaces
 * at startup, which is the same "static class plus runtime rewrite" shape as the
 * website bug. Keeping the single-class assertion here means the launcher cannot
 * drift into the same state unnoticed.
 */

const THEME_CLASSES = ['light-mode', 'dark-mode', 'oled-mode']

async function themeClasses() {
    return browser.execute((known: string[]) => {
        return Array.from(document.documentElement.classList).filter((name) => known.includes(name))
    }, THEME_CLASSES)
}

describe('desktop shell in a real renderer', () => {
    it('mounts the application root', async () => {
        await browser.waitUntil(async () => (await browser.$('#app')).isExisting(), {
            timeout: 60_000,
            timeoutMsg: 'The Vue application root never appeared',
        })
    })

    it('carries exactly one theme class', async () => {
        const classes = await themeClasses()

        // Two theme classes at once is the failure shape: the later token block in
        // the stylesheet wins, so the surface and its text can come from different
        // themes and the result still "renders". WebdriverIO's expect takes no
        // message argument, so the observed classes go into the assertion value
        // itself and are readable in the failure output.
        expect(`[${classes.join(' + ')}] has one theme class`).toBe(
            `[${classes[0] ?? ''}] has one theme class`,
        )
        expect(classes).toHaveLength(1)
    })

    it('renders content instead of an empty window', async () => {
        await browser.waitUntil(
            async () => {
                const text = await browser.execute(() => document.body.innerText.trim())
                return text.length > 0
            },
            { timeout: 60_000, timeoutMsg: 'The window stayed blank' },
        )
    })

    it('serves the route the shell landed on', async () => {
        const path = await browser.execute(() => window.location.pathname)

        // A blank path or a path with no rendered screen would leave the user
        // looking at an empty window with no error.
        expect(path).toBeTruthy()
        expect(path.startsWith('/')).toBe(true)
    })
})

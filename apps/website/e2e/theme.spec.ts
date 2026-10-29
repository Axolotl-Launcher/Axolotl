import { expect, type Page, test } from '@playwright/test'

/**
 * Theme resolution across a real page load.
 *
 * The invariant is not "the class looks right" but "exactly one theme class is
 * present, and the design tokens agree with it". A duplicated class is the exact
 * shape of the shipped regression: `light-mode` and `dark-mode` both on `<html>`,
 * with the dark token block winning the cascade.
 */

const THEME_CLASSES = ['light-mode', 'dark-mode', 'oled-mode'] as const
type ThemeName = 'light' | 'dark' | 'oled'

/** The theme classes currently on the root element. */
async function themeClasses(page: Page) {
    return page.evaluate(
        (known: string[]) => {
            return Array.from(document.documentElement.classList).filter((name) =>
                known.includes(name),
            )
        },
        THEME_CLASSES as unknown as string[],
    )
}

/** Read a CSS custom property as computed on the root element. */
async function token(page: Page, name: string) {
    return page.evaluate((prop: string) => {
        return getComputedStyle(document.documentElement).getPropertyValue(prop).trim()
    }, name)
}

/**
 * Seed a saved preference, then reload.
 *
 * The reload is the half that matters: it is what a returning visitor does, and
 * it is where the prerendered class string meets the hydration pass.
 */
async function loadWithTheme(page: Page, theme: ThemeName) {
    await page.goto('/')
    await page.evaluate((value: string) => {
        window.localStorage.clear()
        window.localStorage.setItem('axolotl-theme', value)
    }, theme)
    await page.reload({ waitUntil: 'load' })
    // Hydration is asynchronous; give the head manager a beat to settle so the
    // assertion measures the end state rather than a mid-flight one.
    await page.waitForTimeout(500)
}

test.describe('theme resolution on a real page load', () => {
    for (const theme of ['light', 'dark', 'oled'] as const) {
        test(`settles on exactly one theme class for a saved "${theme}" preference`, async ({
            page,
        }) => {
            await loadWithTheme(page, theme)

            const classes = await themeClasses(page)

            // Assert the count first so a re-appearance of the bug names itself
            // instead of failing on an opaque string comparison.
            expect(
                classes,
                `<html> carried ${classes.join(' + ') || 'no theme class'}; exactly one is required`,
            ).toHaveLength(1)
            expect(classes[0]).toBe(`${theme}-mode`)
        })
    }

    test('resolves light tokens when the preference is light', async ({ page }) => {
        await loadWithTheme(page, 'light')

        // A light background paired with a light foreground is the failure mode:
        // the tokens came from the dark block even though light-mode was set.
        expect((await token(page, '--surface-1')).toLowerCase()).toBe('#ebebeb')
        expect((await token(page, '--color-contrast')).toLowerCase()).toBe('#1a202c')
    })

    test('keeps the hero title readable against the light background', async ({ page }) => {
        await loadWithTheme(page, 'light')

        const heading = page.locator('h1.main-header')
        await expect(heading).toBeVisible()

        // Compare against the page background rather than trusting the token name:
        // this is what a visitor actually sees.
        const color = await heading.evaluate((el) => getComputedStyle(el).color)
        expect(color).not.toBe('rgb(255, 255, 255)')
        expect(color).toBe('rgb(26, 32, 44)')
    })
})

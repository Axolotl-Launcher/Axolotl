import { expect, type Page, test } from '@playwright/test'

/**
 * Reduced-motion rendering of the landing page's project showcase.
 *
 * The showcase is a marquee: five rows of project chips scroll sideways, and the
 * container fades both ends with `mask-image` so the moving chips are not cut off
 * hard at the edge. For a visitor who asks the system to reduce motion the rows
 * stop and wrap into static lists instead, and the fade has nothing left to hide:
 * all it does is dim the real chip at each end of every row. That is why the
 * component drops the mask under `prefers-reduced-motion: reduce`.
 *
 * The three screenshot baselines in `landing.visual.spec.ts` are captured in
 * normal mode, so none of them can see that rule. Deleting it would break reduced
 * motion rendering with every existing test still green. This suite is the guard,
 * and it is deliberately built out of computed-style assertions rather than
 * pictures: the reduced-motion layout is static, and a failed assertion can say
 * which property regressed and why it matters, where a pixel diff only says that
 * something moved.
 *
 * Determinisation mirrors `landing.visual.spec.ts` — the network stubs, the frozen
 * clock, the font wait and the image wait are the same inputs, and the full
 * explanation of why each one is needed lives there. The one addition is
 * `await img.decode()`: `complete` turns true when the bytes have arrived, but
 * the large showcase screenshots are still decoding at that point, so a capture
 * taken then records a partially painted page.
 */

const THEMES = ['light', 'dark', 'oled'] as const
type Theme = (typeof THEMES)[number]

/** See `landing.visual.spec.ts`: keeps the footer's copyright year out of the run. */
const FROZEN_NOW = new Date('2024-01-01T00:00:00.000Z')

/**
 * The catalogue fixture from `landing.visual.spec.ts`. The showcase sits below the
 * download section, so the page only reaches a stable layout once the hero button
 * and the download links have resolved; this is what lets the suite wait on them.
 */
const DOWNLOAD_CATALOG = {
    version: '1.0.0',
    release_tag: 'v1.0.0',
    downloads: [
        {
            filename: 'Axolotl-Launcher-1.0.0-x64-setup.exe',
            relative_path: 'dist/1.0.0/Axolotl-Launcher-1.0.0-x64-setup.exe',
        },
        {
            filename: 'Axolotl-Launcher-1.0.0-universal.dmg',
            relative_path: 'dist/1.0.0/Axolotl-Launcher-1.0.0-universal.dmg',
        },
        {
            filename: 'Axolotl-Launcher-1.0.0-amd64.AppImage',
            relative_path: 'dist/1.0.0/Axolotl-Launcher-1.0.0-amd64.AppImage',
        },
        {
            filename: 'Axolotl-Launcher-1.0.0_amd64.deb',
            relative_path: 'dist/1.0.0/Axolotl-Launcher-1.0.0_amd64.deb',
        },
        {
            filename: 'Axolotl-Launcher-1.0.0.x86_64.rpm',
            relative_path: 'dist/1.0.0/Axolotl-Launcher-1.0.0.x86_64.rpm',
        },
    ],
}

/** The same 64x64 grey PNG the visual suite uses; keeps third-party avatars off the network. */
const PLACEHOLDER_PNG = Buffer.from(
    'iVBORw0KGgoAAAANSUhEUgAAAEAAAABACAIAAAAlC+aJAAAAS0lEQVR42u3PMQ0AAAwDoPqv6UrYvQQckD4XAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAYHLAAnJEYd2Yg5XAAAAAElFTkSuQmCC',
    'base64',
)

const UPDATE_SERVER_ORIGIN = 'https://update.axlmc.org'
const IMAGE_CDN_HOST = 'cdn.modrinth.com'
const FONT_CDN_HOST = 'cdn-raw.modrinth.com'

/** The showcase container that carries the edge fade. */
const SHOWCASE = '.projects-showcase'

/**
 * A generated `mask-image` value, in whichever form the engine serialises it.
 *
 * Chromium reports the computed value as the full `linear-gradient(...)` (it keeps
 * the unprefixed property), and Blink autoprefixes, so an assertion that only
 * recognises one spelling would pass or fail depending on which property the
 * stylesheet happens to set. `none` is checked separately and by equality.
 */
const GRADIENT_MASK = /^(?:-webkit-)?(?:repeating-)?linear-gradient\(/i

/** Replace every input the page would otherwise take from the network. */
async function stubExternalRequests(page: Page, localOrigin: string) {
    await page.route('**/*', (route) => {
        const url = new URL(route.request().url())

        if (url.origin === localOrigin) return route.continue()
        if (url.origin === UPDATE_SERVER_ORIGIN) return route.fulfill({ json: DOWNLOAD_CATALOG })
        if (url.hostname === FONT_CDN_HOST) return route.continue()
        if (url.hostname === IMAGE_CDN_HOST) {
            return route.fulfill({ body: PLACEHOLDER_PNG, contentType: 'image/png' })
        }

        return route.abort()
    })
}

/**
 * Load the landing page with a saved theme and `prefers-reduced-motion` applied,
 * and leave it in a state where nothing is still moving or decoding.
 *
 * The media feature has to be emulated before the first navigation, exactly like
 * the clock: the stylesheet is parsed and applied during load, so a page that was
 * already painted in normal mode would have to be re-evaluated afterwards.
 */
async function openLandingPage(
    page: Page,
    theme: Theme,
    reducedMotion: 'reduce' | 'no-preference',
) {
    const localOrigin = new URL(test.info().project.use.baseURL ?? 'http://localhost:4173').origin

    await page.emulateMedia({ reducedMotion })
    await page.clock.setFixedTime(FROZEN_NOW)
    await stubExternalRequests(page, localOrigin)

    await page.goto('/')
    await page.evaluate((value: string) => {
        window.localStorage.clear()
        window.localStorage.setItem('axolotl-theme', value)
    }, theme)
    await page.reload({ waitUntil: 'load' })

    // Hydration finished and the stubbed catalogue reached the page.
    await expect(
        page.locator('#download a[href^="https://dist.axlmc.org/dist/1.0.0/"]').first(),
        'the stubbed download catalogue did not reach the page',
    ).toBeVisible({ timeout: 30_000 })
    await expect(
        page.locator('button.hero-download-button'),
        'the hero call to action stayed disabled',
    ).toBeEnabled()

    await page.evaluate(() => document.fonts.ready)
    await expect(page.locator(SHOWCASE), 'the project showcase is missing').toBeVisible()

    await page.evaluate(() => {
        for (const image of Array.from(document.images)) image.loading = 'eager'
    })
    await page.waitForFunction(
        () => Array.from(document.images).every((image) => image.complete),
        null,
        { polling: 500, timeout: 30_000 },
    )
    // `complete` only means the bytes arrived. The showcase screenshots are large
    // and decode asynchronously, so wait for the decoders too; otherwise a later
    // measurement can be taken against a half-painted page.
    await page.evaluate(async () => {
        await Promise.all(
            Array.from(document.images).map(async (image) => {
                try {
                    await image.decode()
                } catch {
                    // A broken image still has to settle the wait; its presence is
                    // guarded by the capture tests, not by this suite.
                }
            }),
        )
    })

    await page.evaluate(() => window.scrollTo(0, 0))
}

/** Read computed `mask-image` (unprefixed and -webkit-) from the showcase container. */
async function readMask(page: Page) {
    return page.evaluate((selector: string) => {
        const element = document.querySelector(selector)
        if (!element) throw new Error(`no element matches ${selector}`)
        const style = getComputedStyle(element)
        return {
            maskImage: style.maskImage,
            webkitMaskImage: style.getPropertyValue('-webkit-mask-image').trim(),
        }
    }, SHOWCASE)
}

/**
 * Read everything the reduced-motion branch of the stylesheet is responsible for,
 * straight from the per-row elements.
 */
async function readRows(page: Page) {
    return page.evaluate((selector: string) => {
        const rows = Array.from(document.querySelectorAll(`${selector} .row`))
        return rows.map((row) => {
            const contents = Array.from(row.querySelectorAll('.row__content'))
            const first = contents[0]
            if (!first) throw new Error('a showcase row has no content track')
            const style = getComputedStyle(first)
            return {
                trackDisplays: contents.map((track) => getComputedStyle(track).display),
                animationName: style.animationName,
                rowWrap: getComputedStyle(row).flexWrap,
                contentWrap: style.flexWrap,
            }
        })
    }, SHOWCASE)
}

test.describe('project showcase under prefers-reduced-motion', () => {
    test('drops the edge fade when the rows stop scrolling', async ({ page }) => {
        await openLandingPage(page, 'light', 'reduce')

        const mask = await readMask(page)

        // The guard for the bug this suite exists for: a mask that is still applied
        // here fades the first and last chip of every static row for no reason.
        // 1614px of the container was measured as faded when it was.
        expect(
            mask.maskImage,
            'With reduced motion the rows no longer scroll, so the edge fade has nothing to hide: ' +
                'mask-image must be none, or it dims the real first and last project chip of ' +
                'every row. Check the prefers-reduced-motion block on .projects-showcase in ' +
                'ProjectsShowcase.vue.',
        ).toBe('none')
        expect(
            mask.webkitMaskImage,
            'The -webkit-mask-image declaration must be cleared alongside mask-image; Blink ' +
                'honours it, so leaving it set keeps the fade applied in Chromium.',
        ).toBe('none')
    })

    test('keeps the edge fade in normal mode, where the rows do scroll', async ({ page }) => {
        await openLandingPage(page, 'light', 'no-preference')

        const mask = await readMask(page)

        // The control. Without it, deleting the mask outright - which also removes
        // the fade from the scrolling marquee it belongs to - would pass the test
        // above. Here the mask must still be the two-ended gradient.
        expect(
            mask.maskImage,
            'In normal mode the rows scroll and are clipped at both edges, so mask-image must ' +
                'remain the linear-gradient fade.',
        ).toMatch(GRADIENT_MASK)
        expect(
            mask.webkitMaskImage,
            'The scrolling marquee needs the -webkit- fallback as well.',
        ).toMatch(GRADIENT_MASK)
    })

    for (const theme of THEMES) {
        test(`renders static, wrapped rows in ${theme} mode`, async ({ page }) => {
            // Colour scheme is what the tokens key off; set it before navigation
            // so the first paint is already in the right theme.
            await page.emulateMedia({ colorScheme: theme === 'oled' ? 'dark' : theme })
            await openLandingPage(page, theme, 'reduce')

            const rows = await readRows(page)

            expect(rows.length, 'the showcase must render five rows').toBe(5)
            for (const [index, row] of rows.entries()) {
                // The premise of the whole change: nothing scrolls. If an animation
                // were still running, the mask would be needed again and the
                // assertion above would be testing the wrong rendering.
                expect(
                    row.animationName,
                    `row ${index + 1} is still animated under reduced motion, so the marquee never stopped`,
                ).toBe('none')
                expect(
                    row.rowWrap,
                    `row ${index + 1} does not wrap under reduced motion, so its chips stay clipped`,
                ).toBe('wrap')
                expect(
                    row.contentWrap,
                    `row ${index + 1} content track does not wrap under reduced motion`,
                ).toBe('wrap')
                // The second track is a visual duplicate that exists only to make
                // the loop seamless. A static list has to hide it, or every project
                // appears twice in a row that is supposed to read as one list.
                expect(
                    row.trackDisplays,
                    `row ${index + 1} must keep one visible content track and hide the duplicate`,
                ).toEqual(['flex', 'none'])
            }
        })
    }
})

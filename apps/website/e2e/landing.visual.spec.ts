import { existsSync } from 'node:fs'

import { expect, type Page, test, type TestInfo } from '@playwright/test'

/**
 * Screenshot baselines for the landing page.
 *
 * The redesign shipped with semantic assertions only: a test could see that a
 * token resolved and that a heading was readable, but nothing guarded colour,
 * spacing, stacking or overflow. This suite captures the whole page in each of
 * the three themes, so a visual regression fails the run instead of reaching
 * visitors unnoticed.
 *
 * Determinism is the whole game here, and every drifting input is pinned in this
 * file rather than in the product:
 * - The clock. The footer prints `new Date().getFullYear()`, so the page would
 *   change on its own every new year. `page.clock.setFixedTime` freezes it.
 * - The update server. The download section is driven by a live third-party
 *   catalogue (`update.axlmc.org`) whose version and asset names change with
 *   every release, so the response is replaced by a fixture.
 * - Third-party imagery and scripts. Project avatars and mod icons come from
 *   `cdn.modrinth.com` and the Howxm SDK from `static.howxm.com`; both are
 *   replaced or blocked, so the capture cannot depend on someone else's CDN.
 * - The design font. `packages/ui` declares Inter with `font-display: swap`
 *   over a CDN, so a capture taken too early would bake the fallback face in.
 *   The suite waits for the font and then asserts it really is Inter.
 * - Lazy images below the fold, which never load while they stay off screen.
 *
 * Baselines are stored per platform (see `snapshotPathTemplate`), because font
 * rasterisation differs between Windows and Linux. A platform without a baseline
 * fails with the command that records one; it never silently skips.
 */

const THEMES = ['light', 'dark', 'oled'] as const
type Theme = (typeof THEMES)[number]

/**
 * A fixed instant, deliberately in the past so that a frozen clock is visible in
 * the baseline: the footer reads "© 2024" rather than the year the baseline was
 * recorded. If the freeze ever stops working, the diff shows the current year.
 */
const FROZEN_NOW = new Date('2024-01-01T00:00:00.000Z')

/**
 * A stand-in for `https://update.axlmc.org/api/downloads/latest`.
 *
 * The shape is the one `normalizeDownloadCatalog` accepts: a plain semver, a
 * matching `v`-prefixed tag, and one attachment per package the download section
 * looks for (installer, disk image, AppImage, deb, rpm).
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

/**
 * A 64x64 solid grey PNG that stands in for third-party imagery. It keeps every
 * image slot occupied at its designed size, so layout and spacing stay under
 * test, while the pixels themselves cannot drift with someone else's CDN.
 */
const PLACEHOLDER_PNG = Buffer.from(
    'iVBORw0KGgoAAAANSUhEUgAAAEAAAABACAIAAAAlC+aJAAAAS0lEQVR42u3PMQ0AAAwDoPqv6UrYvQQckD4XAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAYHLAAnJEYd2Yg5XAAAAAElFTkSuQmCC',
    'base64',
)

const UPDATE_SERVER_ORIGIN = 'https://update.axlmc.org'
const IMAGE_CDN_HOST = 'cdn.modrinth.com'
/** Serves the Inter faces declared by `packages/ui`; the design depends on them. */
const FONT_CDN_HOST = 'cdn-raw.modrinth.com'

/** Whether this run may write baselines instead of comparing against them. */
function isRecordingBaselines(info: TestInfo) {
    return (
        info.config.updateSnapshots !== 'none' ||
        process.argv.includes('--update-snapshots') ||
        process.argv.includes('-u')
    )
}

/**
 * Replace every input the page would otherwise take from the network.
 *
 * Requests to the site itself must pass through untouched: they are the build
 * output under test. Everything else is stubbed, and anything not explicitly
 * allowed is aborted, so a newly added third-party script cannot quietly start
 * contributing pixels to the baseline.
 */
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
 * Load the page the way a returning visitor with `theme` saved does, and leave it
 * in a state where nothing is still moving.
 *
 * The reload is the half that matters: it is where the prerendered theme class
 * meets the hydration pass, and it is the state the baseline should record.
 */
async function openLandingPage(page: Page, theme: Theme) {
    const localOrigin = new URL(test.info().project.use.baseURL ?? 'http://localhost:4173').origin

    // Must be installed before the first navigation so the page never reads the
    // real clock, not even while prerendered markup is being hydrated.
    await page.clock.setFixedTime(FROZEN_NOW)
    await stubExternalRequests(page, localOrigin)

    await page.goto('/')
    await page.evaluate((value: string) => {
        window.localStorage.clear()
        window.localStorage.setItem('axolotl-theme', value)
    }, theme)
    await page.reload({ waitUntil: 'load' })

    // The download links only exist once the client has resolved the catalogue,
    // which happens after hydration. Waiting for them is therefore a stronger
    // readiness signal than any fixed delay: it proves hydration finished and the
    // stubbed response was applied.
    await expect(
        page.locator('#download a[href^="https://dist.axlmc.org/dist/1.0.0/"]').first(),
        'the stubbed download catalogue did not reach the page',
    ).toBeVisible({ timeout: 30_000 })
    await expect(
        page.locator('button.hero-download-button'),
        'the hero call to action stayed disabled',
    ).toBeEnabled()

    await page.evaluate(() => document.fonts.ready)

    // Images below the fold are `loading="lazy"` and would still be blank when
    // the full-page capture reaches them.
    await page.evaluate(() => {
        for (const image of Array.from(document.images)) image.loading = 'eager'
    })
    await page.waitForFunction(
        () => Array.from(document.images).every((image) => image.complete),
        null,
        { polling: 500, timeout: 30_000 },
    )
    // `complete` only reports that the bytes arrived. The showcase screenshots are
    // large and decode asynchronously, so the page can still be painting them when
    // the capture starts; two consecutive captures then disagree and
    // `toHaveScreenshot` fails with "two consecutive stable screenshots" rather
    // than a real difference. Waiting on the decoders closes that window.
    await page.evaluate(async () => {
        await Promise.all(
            Array.from(document.images).map(async (image) => {
                try {
                    await image.decode()
                } catch {
                    // A broken image must not wedge the wait; missing images are
                    // reported by the assertions above instead.
                }
            }),
        )
    })

    await page.evaluate(() => window.scrollTo(0, 0))
}

test.describe('landing page visual baselines', () => {
    for (const theme of THEMES) {
        test(`matches the recorded ${theme} landing page`, async ({ page }, testInfo) => {
            await openLandingPage(page, theme)

            // Guard the pinned environment. If one of these drifts, the diff would
            // otherwise be blamed on a style change that never happened.
            const environment = await page.evaluate(() => ({
                themeClasses: Array.from(document.documentElement.classList).filter((name) =>
                    name.endsWith('-mode'),
                ),
                userAgent: navigator.userAgent,
                locale: navigator.language,
                year: String(new Date().getFullYear()),
                fonts: document.fonts.status,
                interAvailable: document.fonts.check('400 1rem inter'),
                brokenImages: Array.from(document.images)
                    .filter((image) => image.complete && image.naturalWidth === 0)
                    .map((image) => image.currentSrc || image.src),
            }))

            expect(environment.themeClasses, 'exactly one theme class must be present').toEqual([
                `${theme}-mode`,
            ])
            // The platform-specific hero copy follows the user agent, so a machine
            // that reports a different one would render a different page.
            expect(environment.userAgent).toContain('Windows NT')
            expect(environment.locale).toBe('zh-CN')
            expect(environment.year, 'the clock must be frozen').toBe('2024')
            expect(environment.fonts).toBe('loaded')
            expect(environment.interAvailable, 'the design font must be Inter').toBe(true)
            expect(environment.brokenImages, 'no image may be missing from the capture').toEqual([])

            const name = `landing-${theme}.png`
            const baseline = testInfo.snapshotPath(name, { kind: 'screenshot' })

            if (!isRecordingBaselines(testInfo) && !existsSync(baseline)) {
                expect(
                    false,
                    `No visual baseline for this platform at ${baseline}. Baselines are recorded per platform, so one captured on another operating system cannot be compared here. Record this platform's baselines with:` +
                        '\n\n  pnpm --filter @axolotl/website test:e2e:update\n\n' +
                        'then review the new PNGs and commit them.',
                ).toBe(true)
            }

            await expect(page).toHaveScreenshot(name, {
                fullPage: true,
                animations: 'disabled',
                caret: 'hide',
                scale: 'css',
            })
        })
    }
})

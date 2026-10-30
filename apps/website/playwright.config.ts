import { defineConfig, devices } from '@playwright/test'

/**
 * Page-level tests for the static site.
 *
 * These complement the component visual specs rather than replace them. A
 * component spec mounts a single component and asserts its geometry; it never
 * sees server output, the inline theme script, or hydration. A regression in the
 * landing page shipped through exactly that blind spot: nuxt.config pinned
 * `dark-mode` in `htmlAttrs.class`, the inline script rewrote the class list to
 * the visitor's saved preference, and the client head manager re-applied the
 * configured string during hydration. `<html>` carried `light-mode` and
 * `dark-mode` together, the dark token block won the cascade, and a light-theme
 * visitor read white text on a light background.
 *
 * Only a real document load can observe that, so this runner drives the site the
 * way a visitor does.
 *
 * The site is served from the build output, never from a dev server. A dev server
 * renders unminified styles, injects its own client and serves whatever is in
 * memory, so a passing run there says nothing about the artifact that ships; a
 * stale one is indistinguishable from a real regression. `pnpm test:e2e` builds
 * first for that reason.
 */
export default defineConfig({
    testDir: './e2e',
    fullyParallel: false,
    // A stale dev server serving a previous build is indistinguishable from a real
    // regression at this level, so never reuse one across runs.
    workers: 1,
    reporter: [['list']],
    // Capturing and comparing a full-length page takes longer than a DOM assertion.
    timeout: 60_000,
    expect: { timeout: 20_000 },
    // Visual baselines are recorded per platform: glyph rasterisation differs
    // between Windows and Linux, so a capture taken on one can never match a run on
    // the other. The platform suffix keeps both sets side by side, and a platform
    // without a baseline fails with the command that records one.
    snapshotPathTemplate: '{testDir}/__visual__/{testFilePath}/{arg}-{platform}{ext}',
    // Missing baselines are never written silently: a run without `--update-snapshots`
    // must fail on an absent baseline rather than record whatever it happens to see.
    updateSnapshots: 'none',
    use: {
        baseURL: process.env.PLAYWRIGHT_BASE_URL ?? 'http://localhost:4173',
        trace: 'retain-on-failure',
    },
    projects: [
        {
            name: 'chromium',
            use: {
                ...devices['Desktop Chrome'],
                // A fixed viewport, locale, time zone and colour scheme keep the
                // capture independent of the machine that runs it. The device
                // descriptor also pins a Windows user agent, which the landing page
                // reads to pick its platform-specific copy and icon.
                viewport: { width: 1440, height: 900 },
                deviceScaleFactor: 1,
                locale: 'zh-CN',
                timezoneId: 'Asia/Shanghai',
                colorScheme: 'light',
                launchOptions: {
                    args: [
                        // Subpixel text antialiasing and the machine's colour profile
                        // both vary per machine; both are disabled so the pixels come
                        // from the design rather than from the host.
                        '--disable-lcd-text',
                        '--font-render-hinting=none',
                        '--force-color-profile=srgb',
                        '--hide-scrollbars',
                    ],
                },
            },
        },
    ],
    webServer: process.env.PLAYWRIGHT_BASE_URL
        ? undefined
        : {
              command: 'node e2e/static-server.mjs',
              url: 'http://localhost:4173',
              // Never reuse: a dev server left on this port would serve different
              // markup than the build output these tests are meant to observe.
              reuseExistingServer: false,
              timeout: 60_000,
              stdout: 'pipe',
              stderr: 'pipe',
          },
})

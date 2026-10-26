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
 */
export default defineConfig({
    testDir: './e2e',
    fullyParallel: false,
    // A stale dev server serving a previous build is indistinguishable from a real
    // regression at this level, so never reuse one across runs.
    workers: 1,
    reporter: [['list']],
    use: {
        baseURL: process.env.PLAYWRIGHT_BASE_URL ?? 'http://localhost:3000',
        trace: 'retain-on-failure',
    },
    projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
    webServer: process.env.PLAYWRIGHT_BASE_URL
        ? undefined
        : {
              command: 'pnpm --filter @axolotl/website dev',
              url: 'http://localhost:3000',
              reuseExistingServer: !process.env.CI,
              timeout: 180_000,
              stdout: 'ignore',
              stderr: 'pipe',
          },
})

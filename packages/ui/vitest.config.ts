import { playwright } from '@vitest/browser-playwright'
import { mergeConfig } from 'vite'
import { defineConfig } from 'vitest/config'

import viteConfig from './vite.config'

/**
 * Component-level visual regression for the shared design system.
 *
 * These run in a real browser so the Tailwind utilities and the theme tokens
 * from `packages/assets/styles/` resolve exactly as they do in the app; a
 * jsdom run would report computed styles that never exist in production. That
 * matters here because the refactor moves components between generations of
 * the same concept (buttons, modals, selects) and the risk is a silent visual
 * change rather than a type error.
 *
 * Extends the package's own Vite config so the `#ui/*` alias and the SVG
 * loader behave the same as in the real build.
 */
export default mergeConfig(
    viteConfig,
    defineConfig({
        optimizeDeps: {
            include: ['qrcode.vue'],
        },
        test: {
            include: ['src/**/*.visual.spec.ts'],
            browser: {
                enabled: true,
                provider: playwright(),
                instances: [{ browser: 'chromium' }],
                headless: true,
                // Vitest's default (63315) can land inside a Windows Hyper-V/WSL
                // reserved port range, where binding fails with EACCES rather
                // than the port merely being busy, so the run cannot start.
                api: 51234,
            },
        },
    }),
)

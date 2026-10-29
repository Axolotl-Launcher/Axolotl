import vue from '@vitejs/plugin-vue'
import { playwright } from '@vitest/browser-playwright'
import { resolve } from 'path'
import { fileURLToPath } from 'url'
import { defineConfig } from 'vite'
import { nodePolyfills } from 'vite-plugin-node-polyfills'
import svgLoader from 'vite-svg-loader'

/**
 * Browser-mode component tests for the desktop app.
 *
 * These exist so overlay behaviour -- tooltips, popovers, menus -- can be
 * asserted rather than eyeballed. The app had no browser test runner before,
 * which meant `vite build` passing was the only automated signal, and that
 * says nothing about whether a tooltip appears in the right place: it proves
 * the module resolved, not that it works. Anything that positions itself
 * against the viewport needs a real layout engine, hence Chromium.
 *
 * The plugin and alias list is declared here rather than merged from
 * `vite.config.ts`, because that config exports a callback (it varies by
 * `command`) and `mergeConfig` cannot merge one. Only what a component test
 * needs is restated: the Vue plugin, the SVG loader that components import
 * `.svg?component` through, and the `@` alias.
 */
const projectRootDir = resolve(fileURLToPath(new URL('.', import.meta.url)))

export default defineConfig({
    // Matches `vite.config.ts`: the shared stylesheet chain uses Sass `@import`.
    css: {
        preprocessorOptions: {
            scss: {
                silenceDeprecations: ['import'],
            },
        },
    },
    resolve: {
        alias: [
            {
                find: '@',
                replacement: resolve(projectRootDir, 'src'),
            },
        ],
    },
    plugins: [
        nodePolyfills({
            include: [
                'crypto',
                'path',
                'util',
                'stream',
                'buffer',
                'events',
                'string_decoder',
                'assert',
                'vm',
            ],
            globals: { Buffer: true, process: true, global: true },
        }),
        vue(),
        svgLoader({
            svgoConfig: {
                plugins: [
                    {
                        name: 'preset-default',
                        params: {
                            overrides: {
                                removeViewBox: false,
                                cleanupIds: {
                                    minify: false,
                                },
                            },
                        },
                    },
                ],
            },
        }),
    ],
    optimizeDeps: {
        // UI mock factories load this dependency after Vite's initial scan.
        include: [
            'deepslate/nbt',
            'deepslate/util',
            'deepagents/browser',
            'langchain',
            '@modrinth/ui > qrcode.vue',
            '@tauri-apps/api/app',
            '@tauri-apps/api/path',
            '@tauri-apps/plugin-http',
            'vue-i18n',
        ],
    },
    test: {
        // Two kinds of tests share this entry point: plain Node unit tests
        // (`*.test.ts`, migrated from `node --test`) and browser-mode component
        // tests (`*.visual.spec.ts`). They need different environments, so they
        // are separate projects; `pnpm test` runs both and `test:visual` filters
        // to the browser project.
        projects: [
            {
                extends: true,
                test: {
                    name: 'unit',
                    environment: 'node',
                    include: ['src/**/*.test.ts'],
                    // Vitest refuses two projects with different worker counts but the
                    // same group order; these two also want to run one after the other,
                    // since both compete for the same cores.
                    sequence: { groupOrder: 1 },
                    // `node --test` ran these without a deadline, and a few decode every
                    // bundled structure asset (the ancient city catalogue alone takes
                    // ~40s). The ceiling keeps those cases passing; no assertion changes.
                    testTimeout: 120_000,
                    // `testTimeout` does not cover hooks, and one suite starts a Vite dev
                    // server in `beforeAll`. Its 10s default is enough when that file runs
                    // alone and not under a full parallel run, which is what made the
                    // failure look intermittent.
                    hookTimeout: 120_000,
                },
            },
            {
                extends: true,
                test: {
                    name: 'visual',
                    include: ['src/**/*.visual.spec.ts'],
                    sequence: { groupOrder: 2 },
                    // Playwright actions run against a real layout engine, and the heaviest
                    // specs click every layer button twice. Under full-suite parallelism a
                    // case can take five times its isolated time and would otherwise ride
                    // the default budget, failing on scheduling rather than on behaviour.
                    // `SeedMapStructureDetails` builds the End city meshes and needs ~25-40s
                    // on its own, so the ceiling covers that worst case too. This raises the
                    // ceiling only; no assertion is relaxed.
                    testTimeout: 90_000,
                    // The suite is layout- and WebGL-bound, so running every file at once
                    // makes timings depend on how many browsers happen to be mid-frame.
                    // Two workers keeps the wall clock close to the fully parallel run
                    // while removing that contention.
                    maxWorkers: 2,
                    browser: {
                        enabled: true,
                        provider: playwright(),
                        instances: [{ browser: 'chromium' }],
                        headless: true,
                        // Vitest's default (63315) can land inside a Windows Hyper-V/WSL
                        // reserved port range, where binding fails with EACCES rather than
                        // the port merely being busy, so the run cannot start. Offset from
                        // the shared package's 51234 so both suites can run at once.
                        api: 51235,
                    },
                },
            },
        ],
    },
})

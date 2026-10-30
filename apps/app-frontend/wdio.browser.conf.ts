import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

/**
 * End-to-end configuration for the desktop application frontend.
 *
 * The component visual specs mount one component in isolation. They cannot see
 * the shell behaviour a user actually gets: theme resolution across a real page
 * load, persistence of preferences, or routing. This suite runs the real
 * renderer in Chrome against the Vite dev server, with Tauri commands
 * intercepted at `window.__TAURI_INTERNALS__.invoke` so the frontend renders
 * real screens without a Rust backend.
 *
 * Browser mode needs no compiled binary and no WebDriver driver, so it runs on a
 * fresh clone. Native mode (see `wdio.native.conf.ts`) additionally exercises the
 * Rust bridge and needs `tauri-plugin-wdio` compiled into the app.
 *
 * Run with: pnpm --filter @modrinth/app-frontend test:e2e
 */
const projectRoot = dirname(fileURLToPath(import.meta.url))
const repoRoot = resolve(projectRoot, '../..')

export const config: WebdriverIO.Config = {
    runner: 'local',
    specs: ['./e2e/browser/**/*.spec.ts'],
    maxInstances: 1,

    capabilities: [
        {
            browserName: 'tauri',
            'wdio:tauriServiceOptions': {
                mode: 'browser',
                devServerUrl: 'http://127.0.0.1:5201',
            },
        },
    ],

    services: [
        [
            '@wdio/tauri-service',
            {
                mode: 'browser',
                devServerUrl: 'http://127.0.0.1:5201',
                // The service starts and tears down Vite so the tests never depend
                // on a developer already running one.
                devServer: {
                    command: 'pnpm --filter @modrinth/app-frontend dev',
                    cwd: repoRoot,
                    timeoutMs: 180_000,
                    reuseExistingServer: !process.env.CI,
                },
            },
        ],
    ],

    framework: 'mocha',
    mochaOpts: {
        ui: 'bdd',
        timeout: 90_000,
    },

    reporters: ['spec'],
    logLevel: 'warn',
    bail: 0,
    waitforTimeout: 30_000,
    connectionRetryTimeout: 120_000,
    connectionRetryCount: 2,
}

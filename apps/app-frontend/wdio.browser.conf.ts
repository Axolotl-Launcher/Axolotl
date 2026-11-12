import { appendFileSync, mkdirSync, writeFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

import { browser } from '@wdio/globals'
import { createLogger, createServer, type ViteDevServer } from 'vite'

const projectRoot = dirname(fileURLToPath(import.meta.url))
const repoRoot = resolve(projectRoot, '../..')
const artifactDir = resolve(
    process.env.E2E_ARTIFACT_DIR ?? resolve(repoRoot, '.agents/plans/desktop-e2e'),
)
let server: ViteDevServer | undefined

/** Real frontend routes with an E2E-only recorded bridge; no Rust process runs. */
export const config: WebdriverIO.Config = {
    runner: 'local',
    specs: ['./e2e/browser/**/*.spec.ts'],
    maxInstances: 1,
    capabilities: [
        {
            browserName: 'chrome',
            browserVersion: '157.0.8103.50',
            'goog:chromeOptions': {
                args: [
                    '--headless=new',
                    '--no-sandbox',
                    '--disable-dev-shm-usage',
                    '--window-size=1440,1000',
                ],
            },
        },
    ],
    outputDir: resolve(artifactDir, 'webdriver'),
    framework: 'mocha',
    mochaOpts: { ui: 'bdd', timeout: 90_000 },
    reporters: ['spec'],
    logLevel: 'warn',
    bail: 0,
    waitforTimeout: 30_000,
    connectionRetryTimeout: 120_000,
    connectionRetryCount: 2,
    async onPrepare() {
        mkdirSync(artifactDir, { recursive: true })
        const logger = createLogger('info', { prefix: 'desktop-e2e' })
        for (const level of ['info', 'warn', 'error'] as const) {
            const original = logger[level].bind(logger)
            logger[level] = (message, options) => {
                appendFileSync(resolve(artifactDir, 'vite.log'), `[${level}] ${message}\n`)
                original(message, options)
            }
        }
        server = await createServer({
            configFile: resolve(projectRoot, 'e2e/vite.config.ts'),
            customLogger: logger,
        })
        try {
            await server.listen()
        } catch (error) {
            await server.close()
            throw error
        }
    },
    async before() {
        await browser.url('http://127.0.0.1:5203/')
    },
    async afterTest(test, _context, result) {
        const diagnostics = await browser.execute(() => ({
            errors: (window as unknown as { __e2eErrors?: string[] }).__e2eErrors,
            missingCommands:
                (window as unknown as { __demoUnhandled?: string[] }).__demoUnhandled ?? [],
            browserWarnings:
                (window as unknown as { __e2eBrowserWarnings?: string[] }).__e2eBrowserWarnings ??
                [],
            url: location.href,
            text: document.body.innerText,
        }))
        const { text: _text, ...browserLog } = diagnostics
        appendFileSync(
            resolve(artifactDir, 'browser.log'),
            `${JSON.stringify({ test: test.title, ...browserLog })}\n`,
        )
        const failed =
            !result.passed ||
            !diagnostics.errors ||
            diagnostics.errors.length > 0 ||
            diagnostics.missingCommands.length > 0
        if (failed) {
            mkdirSync(artifactDir, { recursive: true })
            const name = `${process.pid}-${test.title.replace(/[^a-z0-9]+/gi, '-').slice(0, 100)}`
            writeFileSync(
                resolve(artifactDir, `${name}.json`),
                JSON.stringify(diagnostics, null, 2),
            )
            await browser.saveScreenshot(resolve(artifactDir, `${name}.png`))
        }
    },
    async onComplete() {
        await server?.close()
    },
}

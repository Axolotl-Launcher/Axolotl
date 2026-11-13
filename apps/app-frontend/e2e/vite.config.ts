import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import { type ConfigEnv, defineConfig, mergeConfig, type UserConfig } from 'vite'

import appPackage from '../package.json' with { type: 'json' }
import appConfig from '../vite.config.ts'

// The product's own entry and router run after the test backend is installed.
export default defineConfig((env: ConfigEnv) => {
    const app = (appConfig as (env: ConfigEnv) => UserConfig)(env)
    if (app.server) delete app.server.headers
    return mergeConfig(app, {
        cacheDir: fileURLToPath(new URL('../node_modules/.vite-desktop-e2e', import.meta.url)),
        define: { __AXOLOTL_DEMO_VERSION__: JSON.stringify(appPackage.version) },
        server: { host: '127.0.0.1', port: 5203, strictPort: true },
        optimizeDeps: { include: ['@modrinth/ui > qrcode.vue'] },
        plugins: [
            {
                name: 'desktop-e2e-entry',
                transformIndexHtml: {
                    order: 'pre',
                    handler(html) {
                        return {
                            html: html.replace('/src/main.js', '/e2e/browser-entry.ts'),
                            tags: [
                                {
                                    tag: 'script',
                                    children: readFileSync(
                                        fileURLToPath(
                                            new URL('./runtime-monitor.js', import.meta.url),
                                        ),
                                        'utf8',
                                    ),
                                    injectTo: 'head-prepend',
                                },
                            ],
                        }
                    },
                },
            },
        ],
    })
})

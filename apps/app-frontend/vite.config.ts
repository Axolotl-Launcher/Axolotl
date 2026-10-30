import vue from '@vitejs/plugin-vue'
import { existsSync, readFileSync, statSync } from 'fs'
import { extname, resolve, sep } from 'path'
import { fileURLToPath } from 'url'
import { defineConfig } from 'vite'
import { nodePolyfills } from 'vite-plugin-node-polyfills'
import vueDevTools from 'vite-plugin-vue-devtools'
import svgLoader from 'vite-svg-loader'

import tauriConf from '../app/tauri.conf.json' with { type: 'json' }

const projectRootDir = resolve(fileURLToPath(new URL('.', import.meta.url)))
const appLibEnvDir = resolve(projectRootDir, '../../packages/app-lib')
const apiClientSource = resolve(projectRootDir, '../../packages/api-client/src/index.ts')
const blockbenchRoot = resolve(projectRootDir, '../../third-party/blockbench')

function blockbenchSkinDevAssets() {
    return {
        name: 'blockbench-skin-dev-assets',
        configureServer(server) {
            server.middlewares.use('/__blockbench_skin__', (request, response, next) => {
                const requestPath = decodeURIComponent((request.url ?? '/').split('?')[0])
                const relativePath =
                    requestPath === '/' ? 'index.html' : requestPath.replace(/^\/+/, '')
                const filePath = resolve(blockbenchRoot, relativePath)
                if (
                    !filePath.startsWith(`${blockbenchRoot}${sep}`) ||
                    !existsSync(filePath) ||
                    !statSync(filePath).isFile()
                ) {
                    next()
                    return
                }
                const contentTypes = {
                    '.css': 'text/css; charset=utf-8',
                    '.html': 'text/html; charset=utf-8',
                    '.ico': 'image/x-icon',
                    '.js': 'text/javascript; charset=utf-8',
                    '.json': 'application/json; charset=utf-8',
                    '.png': 'image/png',
                    '.svg': 'image/svg+xml',
                    '.ttf': 'font/ttf',
                    '.webp': 'image/webp',
                    '.woff': 'font/woff',
                    '.woff2': 'font/woff2',
                }
                response.setHeader(
                    'Content-Type',
                    contentTypes[extname(filePath)] ?? 'application/octet-stream',
                )
                response.end(readFileSync(filePath))
            })
        },
    }
}

// Load .env from app-lib manually instead of using Vite's envDir, which would auto-load .env.local and override values
const envFilePath = resolve(appLibEnvDir, '.env')
if (existsSync(envFilePath)) {
    for (const line of readFileSync(envFilePath, 'utf-8').split('\n')) {
        const trimmed = line.trim()
        if (!trimmed || trimmed.startsWith('#')) continue
        const eqIndex = trimmed.indexOf('=')
        if (eqIndex === -1) continue
        const key = trimmed.slice(0, eqIndex)
        const value = trimmed.slice(eqIndex + 1)
        if (!(key in process.env)) {
            process.env[key] = value
        }
    }
}

// https://vitejs.dev/config/
export default defineConfig(({ command }) => ({
    css: {
        preprocessorOptions: {
            scss: {
                // TODO: dont forget about this
                silenceDeprecations: ['import'],
            },
        },
    },
    resolve: {
        alias: [
            {
                find: '@modrinth/api-client',
                replacement: apiClientSource,
            },
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
        blockbenchSkinDevAssets(),
        // Vue DevTools injects transform/runtime work; keep it for `vite`/`tauri
        // dev` only so production `vite build` stays lean.
        ...(command === 'serve' ? [vueDevTools()] : []),
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
        include: [
            'deepagents/browser',
            'langchain',
            '@langchain/core/language_models/chat_models',
            '@langchain/core/messages',
            '@langchain/core/tools',
            '@langchain/core/utils/function_calling',
            'zod',
        ],
        exclude: ['ace-builds', 'vue3-ace-editor'],
    },

    // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
    // prevent vite from obscuring rust errors
    clearScreen: false,
    // tauri expects a fixed port, fail if that port is not available
    server: {
        host: '127.0.0.1',
        port: 5201,
        strictPort: true,
        headers: {
            'content-security-policy': Object.entries(tauriConf.app.security.csp)
                .map(([directive, sources]) => {
                    // An additional websocket connect-src is required for Vite dev tools to work
                    if (directive === 'connect-src') {
                        sources = Array.isArray(sources) ? sources : [sources]
                        sources.push('ws://127.0.0.1:5201')
                    }
                    return Array.isArray(sources)
                        ? `${directive} ${sources.join(' ')}`
                        : `${directive} ${sources}`
                })
                .join('; '),
        },
    },
    // to make use of `TAURI_ENV_DEBUG` and other env variables
    // https://v2.tauri.app/reference/environment-variables/#tauri-cli-hook-commands
    envPrefix: ['VITE_', 'TAURI_', 'MODRINTH_'],
    build: {
        rolldownOptions: {
            onwarn(warning, defaultHandler) {
                if (warning.code === 'INEFFECTIVE_DYNAMIC_IMPORT') return
                defaultHandler(warning)
            },
            output: {
                manualChunks(id) {
                    if (id.includes('node_modules/three')) return 'vendor-three'
                },
            },
        },
        // Tauri supports es2021
        target: process.env.TAURI_ENV_PLATFORM == 'windows' ? 'chrome105' : 'safari13',
        // don't minify for debug builds
        minify: !process.env.TAURI_ENV_DEBUG,
        // produce sourcemaps for debug builds
        sourcemap: !!process.env.TAURI_ENV_DEBUG,
    },
}))

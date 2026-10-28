import { resolve } from 'path'
import { fileURLToPath } from 'url'
import { type ConfigEnv, defineConfig, mergeConfig, type UserConfig } from 'vite'

import appConfig from '../vite.config'

const projectRoot = resolve(fileURLToPath(new URL('..', import.meta.url)))
// PostCSS finds the Tailwind config, and Tailwind its content globs, from the working directory.
process.chdir(projectRoot)

/**
 * The launcher frontend built as a static web demo: same sources and plugins as
 * the desktop build, a different entry that installs the in-browser backend, and
 * relative asset URLs so the output can be served from any path.
 */
export default defineConfig((env: ConfigEnv) => {
    const app = (appConfig as (env: ConfigEnv) => UserConfig)(env)
    // The desktop dev server sends the Tauri CSP; the demo talks to no backend.
    if (app.server) delete app.server.headers
    return mergeConfig(app, {
        root: projectRoot,
        base: './',
        resolve: {
            alias: [
                {
                    find: /^vue-router$/,
                    replacement: resolve(projectRoot, 'demo/shims/vue-router.ts'),
                },
                {
                    find: /^vue-router-original$/,
                    replacement: resolve(
                        projectRoot,
                        'node_modules/vue-router/dist/vue-router.mjs',
                    ),
                },
            ],
        },
        server: { port: 5202, strictPort: false },
        build: {
            outDir: resolve(projectRoot, 'demo/dist'),
            emptyOutDir: true,
            target: 'es2022',
            rolldownOptions: {
                input: resolve(projectRoot, 'demo/index.html'),
                // @modrinth/ui declares no `sideEffects`, so its barrel pulls in every
                // component; only its stylesheets have side effects.
                treeshake: {
                    moduleSideEffects: (id: string) =>
                        !id.includes('/packages/ui/') || /\.(css|scss)$|[?&]type=style/.test(id)
                            ? undefined
                            : false,
                },
            },
        },
    } satisfies UserConfig)
})

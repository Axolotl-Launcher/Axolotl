import baseConfig from '@modrinth/tooling-config/eslint/nuxt.mjs'

export default baseConfig
    .clone()
    .append({
        // Vitest's cache directory (`cacheDir: '.vite'` in vite.config.ts) holds
        // vendored dependency bundles. It is a build artifact, not source.
        ignores: ['.vite/**'],
    })
    .append({
        // This package is consumed by both the desktop app and the Nuxt website,
        // so it must stay platform-agnostic: platform capabilities arrive through
        // the `providers/` DI contracts. A direct Tauri import would break the
        // website build — `ImportInstanceStage.vue` did exactly that.
        rules: {
            'no-restricted-imports': [
                'error',
                {
                    patterns: [
                        {
                            group: ['@tauri-apps/*', '@tauri-apps/**'],
                            message:
                                'packages/ui must stay platform-agnostic. Add the capability to a provider in src/providers/ and inject it instead.',
                        },
                    ],
                },
            ],
        },
    })
    .append({
        // TresJS components are resolved by the TresJS Vite plugin and are never
        // imported by name, so the rule cannot see them.
        files: ['src/components/skin/**/*.vue'],
        rules: {
            'vue/no-undef-components': [
                'error',
                {
                    ignorePatterns: ['Group', 'primitive', 'Tres[A-Z].*'],
                },
            ],
        },
    })

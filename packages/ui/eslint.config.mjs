import baseConfig from '@modrinth/tooling-config/eslint/nuxt.mjs'

export default baseConfig
    .append({
        // Vitest's cache directory (`cacheDir: '.vite'` in vite.config.ts) holds
        // vendored dependency bundles. It is a build artifact, not source.
        ignores: ['.vite/**'],
    })
    .append({
        // This package is consumed by the desktop app only, but must stay
        // platform-agnostic: platform capabilities arrive through the
        // `providers/` DI contracts instead of direct Tauri imports.
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

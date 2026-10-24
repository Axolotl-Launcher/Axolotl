import config from '@modrinth/tooling-config/eslint/nuxt.mjs'
export default config
    .append({
        // Vendored editors (Monaco, Blockbench) and assets the launcher preview serves as-is.
        ignores: ['src/public/launcher-preview/**'],
    })
    .append({
        rules: {
            '@typescript-eslint/no-explicit-any': 'off',
            'import/no-unresolved': 'off',
            'no-undef': 'off',
        },
    })
    .append({
        // The desktop app frontend moved onto the shared barrel and gained this
        // guardrail; the website was left out of that refactor and accumulated 32
        // deep imports into the package's internals. Enforce the same boundary here.
        rules: {
            'no-restricted-imports': [
                'error',
                {
                    patterns: [
                        {
                            group: ['@modrinth/ui/src/*'],
                            message:
                                'Import from the "@modrinth/ui" barrel instead of reaching into its source. If the symbol is missing from the barrel, export it there.',
                        },
                    ],
                },
            ],
        },
    })
    .append({
        // Ported launcher screens also consume contracts not exported by the UI barrel.
        // Keep their direct imports inside the lazy preview boundary: the barrel's
        // editor/Three.js side effects otherwise enter the initial route bundle.
        files: ['src/components/launcher-preview/**/*.{vue,ts}'],
        rules: { 'no-restricted-imports': 'off' },
    })

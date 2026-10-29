import config from '@modrinth/tooling-config/eslint/nuxt.mjs'

export default config
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

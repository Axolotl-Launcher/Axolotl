import config from '@modrinth/tooling-config/eslint/nuxt.mjs'

export default config
    .clone()
    .append({
        rules: {
            // The shared package exposes a barrel; reaching past it couples the app
            // to internal file layout, which is how 45 deep imports accumulated.
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
        // `node --test` resolves `@modrinth/ui` to `node.ts`, a deliberate Node
        // stub that exports only the i18n helpers, so a pure-logic module that a
        // `node --test` file imports cannot use the barrel and must name its
        // module. Listed per file rather than as `src/**/*.test.ts` so the
        // boundary stays enforced everywhere else — a wide exception would let a
        // new deep import into any test pass unnoticed.
        files: [
            'src/helpers/browse-install-preferences.test.ts',
            'src/lab/schematic-preview/instance-files.ts',
        ],
        rules: {
            'no-restricted-imports': 'off',
        },
    })

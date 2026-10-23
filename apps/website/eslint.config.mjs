import config from '@modrinth/tooling-config/eslint/nuxt.mjs'
export default config.append([
    {
        // Vendored editors (Monaco, Blockbench) and assets the launcher preview serves as-is.
        ignores: ['src/public/launcher-preview/**'],
    },
    {
        rules: {
            '@typescript-eslint/no-explicit-any': 'off',
            'import/no-unresolved': 'off',
            'no-undef': 'off',
        },
    },
])

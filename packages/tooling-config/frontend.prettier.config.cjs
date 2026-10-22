/**
 * @see https://prettier.io/docs/configuration
 * @type {import("prettier").Config}
 */
module.exports = {
    semi: false,
    singleQuote: true,
    // Keep tracked JSON in the repository's space-indented format.
    useTabs: false,
    plugins: [
        // Keep the Tailwind plugin scoped to frontend packages. Its transitive
        // import-sort dependency can misparse TypeScript type annotations.
        // In typical JS/TS britleness fashion, the Tailwind CSS plugin
        // has a transitive dependency on an import sort plugin that breaks
        // TypeScript type annotations, for reasons unbeknownst to anyone.
        // Our frontend project was the only one enabling such plugin, so
        // to avoid this bug spreading to other parts of the monorepo, let's
        // keep it contained to it. See:
        // https://github.com/tailwindlabs/prettier-plugin-tailwindcss/issues/338
        // https://github.com/prettier/prettier-vscode/issues/3578
        'prettier-plugin-tailwindcss',
        'prettier-plugin-toml',
        'prettier-plugin-sql-cst',
        '@prettier/plugin-xml',
    ],
    overrides: [
        {
            files: ['*.jsonc'],
            options: {
                parser: 'jsonc',
                // By spec, JSONC only extends JSON with comment support, not trailing commas as Prettier likes to add
                trailingComma: 'none',
            },
        },
        {
            // TOML is space-indented throughout the repository and is checked by
            // the `tombi` CI job; tabs would fight both.
            files: ['*.toml'],
            options: {
                useTabs: false,
                tabWidth: 2,
            },
        },
    ],
    xmlWhitespaceSensitivity: 'ignore',
}

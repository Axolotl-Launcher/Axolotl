/**
 * @see https://prettier.io/docs/configuration
 * @type {import("prettier").Config}
 */
module.exports = {
    semi: false,
    singleQuote: true,
    // Match the repository's space-indented source and tracked JSON.
    useTabs: false,
    plugins: ['prettier-plugin-toml', 'prettier-plugin-sql-cst', '@prettier/plugin-xml'],
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

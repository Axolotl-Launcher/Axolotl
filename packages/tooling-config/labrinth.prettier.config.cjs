/**
 * @see https://prettier.io/docs/configuration
 * @type {import("prettier").Config}
 */
module.exports = {
    semi: false,
    singleQuote: true,
    // Match the repository's space-indented source and tracked JSON instead of
    // depending on `.editorconfig`'s `indent_style`.
    useTabs: false,
    plugins: ['prettier-plugin-toml', 'prettier-plugin-sql-cst', '@prettier/plugin-xml'],
    overrides: [
        {
            files: ['*.sql'],
            options: {
                parser: 'postgresql',
                sqlAcceptUnsupportedGrammar: true,
            },
        },
        {
            files: ['*.jsonc'],
            options: {
                parser: 'jsonc',
                // By spec, JSONC only extends JSON with comment support, not trailing commas as Prettier likes to add
                trailingComma: 'none',
            },
        },
    ],
    xmlWhitespaceSensitivity: 'ignore',
}

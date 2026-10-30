import preset from '@modrinth/tooling-config/tailwind/tailwind-preset.ts'
import type { Config } from 'tailwindcss'

import { DEFAULT_MONO_FONT_STACK, DEFAULT_UI_FONT_STACK } from './src/helpers/font-family.ts'

const config: Config = {
    content: [
        './src/components/**/*.{js,vue,ts}',
        './src/features/**/*.{js,vue,ts}',
        './src/layouts/**/*.vue',
        './src/pages/**/*.vue',
        './src/plugins/**/*.{js,ts}',
        './src/App.vue',
        './src/error.vue',
        // monorepo - TODO: migrate this to its own package
        '../../packages/**/*.{js,vue,ts}',
        '!../../packages/**/node_modules/**',
    ],
    presets: [preset],
    theme: {
        extend: {
            // Both utilities follow the tokens the appearance settings override, so
            // every `font-mono` / `font-sans` surface (logs, file editor, JSON views)
            // tracks the user's font choice.
            fontFamily: {
                mono: `var(--mono-font, ${DEFAULT_MONO_FONT_STACK})`,
                sans: `var(--font-standard, ${DEFAULT_UI_FONT_STACK})`,
            },
        },
    },
}

export default config

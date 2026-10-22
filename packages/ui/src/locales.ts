import type { CrowdinMessages } from './composables/i18n'

export { default as uiEnglishMessages } from './locales/en-US/index.json'

export const uiLocaleModules = import.meta.glob<{ default: CrowdinMessages }>(
    './locales/*/index.json',
    { eager: false },
)

export const metaLocaleModules = import.meta.glob<{ default: CrowdinMessages }>(
    './locales/*/meta.json',
    { eager: true },
)

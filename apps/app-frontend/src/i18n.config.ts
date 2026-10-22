import {
    buildLocaleMessages,
    createMessageCompiler,
    type CrowdinMessages,
    uiEnglishMessages,
    uiLocaleModules,
} from '@modrinth/ui'
import { createI18n } from 'vue-i18n'

import { createLocaleLoader } from '@/helpers/locale-loader'
import englishAppMessages from '@/locales/en-US/index.json'

const localeModules = import.meta.glob<{ default: CrowdinMessages }>('./locales/*/index.json')
const availableLocales = Object.keys(localeModules).map((path) => path.split('/').at(-2)!)

const i18n = createI18n({
    legacy: false,
    locale: 'en-US',
    fallbackLocale: 'en-US',
    messageCompiler: createMessageCompiler(),
    missingWarn: false,
    fallbackWarn: false,
    messages: buildLocaleMessages(
        { './locales/en-US/index.json': { default: englishAppMessages } },
        { './locales/en-US/index.json': { default: uiEnglishMessages } },
    ),
})

const localeLoader = createLocaleLoader(
    async (locale) => {
        const appPath = `./locales/${locale}/index.json`
        const [appMessages, uiMessages] = await Promise.all([
            localeModules[appPath](),
            uiLocaleModules[appPath](),
        ])
        i18n.global.setLocaleMessage(
            locale,
            buildLocaleMessages({ [appPath]: appMessages }, { [appPath]: uiMessages })[locale],
        )
    },
    (locale) => {
        i18n.global.locale.value = locale
    },
    ['en-US'],
)

export async function setAppLocale(locale: string): Promise<void> {
    await localeLoader.select(availableLocales.includes(locale) ? locale : 'en-US')
}

const STARTUP_LOCALE_KEY = 'axolotl.startup-locale'

export async function initializeLocale(): Promise<void> {
    let storedLocale: string | null = null
    try {
        storedLocale = localStorage.getItem(STARTUP_LOCALE_KEY)
    } catch {
        // The database preference remains authoritative when storage is unavailable.
    }
    await applyLocalePreference(storedLocale)
}

export function resolveInitialLocale(preferredLocales: readonly string[]): string {
    for (const preferredLocale of preferredLocales) {
        const normalizedLocale = preferredLocale.replace('_', '-')
        if (availableLocales.includes(normalizedLocale)) return normalizedLocale

        const language = normalizedLocale.split('-')[0].toLowerCase()
        if (language === 'zh') {
            const traditionalChinese = /-(tw|hk|mo)|-hant/i.test(normalizedLocale)
            return traditionalChinese ? 'zh-TW' : 'zh-CN'
        }

        const languageMatch = availableLocales.find((locale) =>
            locale.toLowerCase().startsWith(`${language}-`),
        )
        if (languageMatch) return languageMatch
    }

    return 'en-US'
}

export const SYSTEM_LOCALE_VALUE = 'system'
const FOLLOW_SYSTEM_LOCALE_KEY = 'axolotl.follow-system-locale'

function preferredNavigatorLocales(): readonly string[] {
    return navigator.languages?.length ? navigator.languages : [navigator.language ?? 'en-US']
}

export function getSystemResolvedLocale(): string {
    return resolveInitialLocale(preferredNavigatorLocales())
}

export function isFollowingSystemLocale(storedLocale: string | null | undefined): boolean {
    if (!storedLocale || storedLocale === SYSTEM_LOCALE_VALUE) return true
    try {
        return localStorage.getItem(FOLLOW_SYSTEM_LOCALE_KEY) === '1'
    } catch {
        return false
    }
}

export function setFollowSystemLocale(follow: boolean) {
    try {
        if (follow) localStorage.setItem(FOLLOW_SYSTEM_LOCALE_KEY, '1')
        else localStorage.removeItem(FOLLOW_SYSTEM_LOCALE_KEY)
    } catch {
        // localStorage may be unavailable; system-follow then relies on empty/'system' locale only.
    }
}

/** Applies the stored locale preference and returns the concrete locale used. */
export async function applyLocalePreference(
    storedLocale: string | null | undefined,
): Promise<string> {
    const locale = isFollowingSystemLocale(storedLocale)
        ? getSystemResolvedLocale()
        : availableLocales.includes(storedLocale ?? '')
          ? storedLocale!
          : 'en-US'
    try {
        localStorage.setItem(STARTUP_LOCALE_KEY, storedLocale ?? SYSTEM_LOCALE_VALUE)
    } catch {
        // Caching the preference only avoids loading an unused system dictionary next time.
    }
    await setAppLocale(locale)
    return locale
}

// English is available for the first synchronous render. Other dictionaries load
// alongside backend initialization; setupApp applies the authoritative DB preference.
export default i18n

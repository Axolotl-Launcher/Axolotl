import { injectI18n } from '@modrinth/ui/src/providers/i18n.ts'
import { reactive } from 'vue'

import { usePreviewDomain } from '../../lib/state/index'
import { useSettingsPage } from './state'

/**
 * The app's language preference (apps/app-frontend/src/i18n.config.ts) over
 * the preview, shared by every settings surface that changes the language (the
 * Language page and the onboarding welcome): the stored `settings.locale`, the
 * "follow the system language" flag the app keeps in local storage, and
 * applying a locale, which switches the window's language through the
 * preview's i18n context. The machine's language list is the visitor's browser
 * languages.
 */

export const SYSTEM_LOCALE_VALUE = 'system'

/** The locales the app bundles (apps/app-frontend/src/locales/*). */
export const availableLocales: readonly string[] = [
    'ar-SA',
    'cs-CZ',
    'da-DK',
    'de-CH',
    'de-DE',
    'en-US',
    'es-419',
    'es-ES',
    'fi-FI',
    'fil-PH',
    'fr-FR',
    'he-IL',
    'hu-HU',
    'id-ID',
    'it-IT',
    'ja-JP',
    'ko-KR',
    'ms-MY',
    'nl-NL',
    'no-NO',
    'pl-PL',
    'pt-BR',
    'pt-PT',
    'ro-RO',
    'ru-RU',
    'sr-CS',
    'sv-SE',
    'th-TH',
    'tr-TR',
    'uk-UA',
    'vi-VN',
    'zh-CN',
    'zh-TW',
]

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

function preferredNavigatorLocales(): readonly string[] {
    return navigator.languages?.length ? navigator.languages : [navigator.language ?? 'en-US']
}

export function getSystemResolvedLocale(): string {
    return resolveInitialLocale(preferredNavigatorLocales())
}

/**
 * The preference functions that need the window: `setAppLocale`,
 * `setFollowSystemLocale`, `isFollowingSystemLocale` and
 * `applyLocalePreference` with the app's names and semantics, plus
 * `getSystemResolvedLocale`. Call it from a component's setup.
 */
export function useAppLocale() {
    const i18n = injectI18n()
    const { settings, simulate } = useSettingsPage()
    // `axolotl.follow-system-locale` of the app's local storage.
    const preference = usePreviewDomain('settings:locale', () => ({
        value: reactive({ followSystem: false }),
    }))

    function isFollowingSystemLocale(storedLocale: string | null | undefined): boolean {
        if (!storedLocale || storedLocale === SYSTEM_LOCALE_VALUE) return true
        return preference.followSystem
    }

    function setFollowSystemLocale(follow: boolean) {
        preference.followSystem = follow
    }

    /** Switches the window's language; a locale the app lacks falls back to English. */
    async function switchWindowLocale(locale: string) {
        await i18n.setLocale(availableLocales.includes(locale) ? locale : 'en-US')
    }

    /** Stores the locale (`settings.locale`) and switches the window to it. */
    async function setAppLocale(locale: string): Promise<void> {
        await simulate(60)
        settings.locale = locale
        await switchWindowLocale(locale)
    }

    /**
     * Applies a stored preference (an empty or `system` value follows the
     * system language) without rewriting it, and returns the concrete locale.
     */
    async function applyLocalePreference(storedLocale: string | null | undefined) {
        const locale = isFollowingSystemLocale(storedLocale)
            ? getSystemResolvedLocale()
            : availableLocales.includes(storedLocale ?? '')
              ? storedLocale!
              : 'en-US'
        await switchWindowLocale(locale)
        return locale
    }

    return {
        locale: i18n.locale,
        getSystemResolvedLocale,
        isFollowingSystemLocale,
        setFollowSystemLocale,
        setAppLocale,
        applyLocalePreference,
    }
}

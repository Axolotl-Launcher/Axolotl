import {
    type CrowdinMessages,
    I18N_INJECTION_KEY,
    type I18nContext,
    LOCALES,
    transformCrowdinMessages,
    uiLocaleModules,
    useDebugLogger,
} from '@modrinth/ui'
import IntlMessageFormat from 'intl-messageformat'
import { LRUCache } from 'lru-cache'

import { resolveInitialLocale } from '~/i18n/resolve-locale'
import { siteLocaleMessages, SUPPORTED_SITE_LOCALES } from '~/locales/site'

const debug = useDebugLogger('i18n')
const DEFAULT_LOCALE = 'zh-CN'
const supportedLocales = LOCALES.filter((locale) =>
    SUPPORTED_SITE_LOCALES.includes(locale.code as (typeof SUPPORTED_SITE_LOCALES)[number]),
)
const supportedLocaleCodes = supportedLocales.map((locale) => locale.code)

function isSupportedLocale(locale: string): boolean {
    return supportedLocaleCodes.includes(locale)
}

function getBrowserLanguages(): readonly string[] {
    if (!import.meta.client) return []

    return navigator.languages?.length ? navigator.languages : [navigator.language]
}

const messageCache = new LRUCache<string, Record<string, string>>({ max: 10 })
const formatterCache = new LRUCache<string, IntlMessageFormat>({ max: 1000 })
const loadingPromises = new Map<string, Promise<void>>() // Dedupe concurrent loads

type LocaleModules = Record<string, () => Promise<{ default: CrowdinMessages }>>

// Find the loader for a locale code in a glob result (paths end with /{code}/index.json)
function findLocaleLoader(modules: LocaleModules, code: string) {
    for (const [path, loader] of Object.entries(modules)) {
        if (path.endsWith('/' + code + '/index.json')) {
            return loader
        }
    }
    return undefined
}

function formatIcuMessage(msg: string, locale: string, values: Record<string, unknown>) {
    const cacheKey = locale + ':' + msg
    let formatter = formatterCache.get(cacheKey)

    try {
        if (!formatter) {
            formatter = new IntlMessageFormat(msg, locale)
            formatterCache.set(cacheKey, formatter)
        }
        const result = formatter.format(values)
        if (import.meta.dev && typeof result !== 'string') {
            debug('formatIcuMessage: format returned non-string', typeof result)
        }
        return result as string
    } catch {
        return null
    }
}

async function loadLocale(code: string): Promise<void> {
    if (messageCache.has(code)) {
        debug('loadLocale: already cached', code)
        return
    }

    // Dedupe concurrent requests for the same locale
    const existing = loadingPromises.get(code)
    if (existing) {
        debug('loadLocale: already loading', code)
        return existing
    }

    debug('loadLocale: starting', code)

    const promise = (async () => {
        const uiLoader = findLocaleLoader(uiLocaleModules, code)

        debug('loadLocale: loaders found', {
            code,
            ui: !!uiLoader,
        })

        const uiData = await uiLoader?.().catch((e) => {
            debug('loadLocale: ui loader failed', code, e)
            return null
        })

        debug('loadLocale: data loaded', {
            code,
            uiKeys: uiData ? Object.keys(uiData.default).length : 0,
        })

        const mergedMessages: Record<string, string> = {}
        if (uiData) Object.assign(mergedMessages, transformCrowdinMessages(uiData.default))
        Object.assign(mergedMessages, siteLocaleMessages[code])

        debug('loadLocale: merged', code, 'total keys:', Object.keys(mergedMessages).length)

        if (Object.keys(mergedMessages).length > 0) {
            messageCache.set(code, mergedMessages)
        }
    })()

    loadingPromises.set(code, promise)
    try {
        await promise
    } finally {
        loadingPromises.delete(code)
    }
}

export default defineNuxtPlugin({
    name: 'i18n',
    enforce: 'pre',
    async setup(nuxtApp) {
        const locale = ref(DEFAULT_LOCALE)
        const savedLocale = useCookie<string | null>('locale').value

        // 首屏语言优先级：cookie > Accept-Language（SSR）> 默认中文。
        // SSR 阶段就决定语言，避免客户端挂载后整页跳变。
        const initialLocale = resolveInitialLocale({
            cookieLocale: savedLocale,
            acceptLanguage: import.meta.server
                ? useRequestHeaders(['accept-language'])['accept-language']
                : null,
            supported: supportedLocaleCodes,
            defaultLocale: DEFAULT_LOCALE,
        })

        function t(key: string, values?: Record<string, unknown>): string {
            const currentLocale = locale.value
            const localeMessages = messageCache.get(currentLocale)
            // 只有默认语言才回退到默认语言消息；其他语言缺失时返回 key，
            // 由 formatMessage 使用英文 defaultMessage
            const fallbackMessages =
                currentLocale === DEFAULT_LOCALE ? messageCache.get(DEFAULT_LOCALE) : undefined
            const msg = localeMessages?.[key] ?? fallbackMessages?.[key]

            if (!msg) {
                debug('t: key not found', {
                    key,
                    locale: currentLocale,
                    hasLocaleMessages: !!localeMessages,
                    hasFallbackMessages: !!fallbackMessages,
                })
                return key
            }

            if (!values || Object.keys(values).length === 0) return msg

            const formatted = formatIcuMessage(msg, currentLocale, values)
            if (formatted !== null) return formatted

            const fallbackMsg = fallbackMessages?.[key]
            if (fallbackMsg && fallbackMsg !== msg) {
                return formatIcuMessage(fallbackMsg, DEFAULT_LOCALE, values) ?? fallbackMsg
            }

            return msg
        }

        async function setLocale(newLocale: string): Promise<void> {
            debug('setLocale: called', { newLocale, currentLocale: locale.value })

            if (!isSupportedLocale(newLocale)) {
                debug('setLocale: invalid locale', newLocale)
                return
            }

            await loadLocale(newLocale)

            debug('setLocale: loaded', {
                newLocale,
                cacheHas: messageCache.has(newLocale),
                cacheKeys: messageCache.get(newLocale)
                    ? Object.keys(messageCache.get(newLocale)!).length
                    : 0,
            })

            locale.value = newLocale
            useCookie('locale', { maxAge: 31536000, path: '/' }).value = newLocale
            if (import.meta.client) document.documentElement.lang = newLocale
        }

        await loadLocale(DEFAULT_LOCALE)
        if (initialLocale !== DEFAULT_LOCALE) await loadLocale(initialLocale)
        locale.value = initialLocale
        if (import.meta.client) document.documentElement.lang = initialLocale

        debug('init: complete', { locale: locale.value })

        useHead({
            htmlAttrs: {
                lang: () => locale.value,
            },
        })

        const context: I18nContext = { locale, t, setLocale }
        // 共享层 `provideI18n` 包的是 Vue `provide()`，只在组件 `setup()` 内生效；
        // Nuxt 插件运行在组件之外，因此改为在 app 实例上注册，`inject()` 会按
        // app 级 provide 回落读取。
        nuxtApp.vueApp.provide(I18N_INJECTION_KEY, context)

        nuxtApp.hook('app:mounted', async () => {
            // The client re-resolves with the browser's own preferences, but only
            // gets to override the server when the server had nothing to go on --
            // see resolveInitialLocale. Re-deciding here regardless is what made a
            // page switch language right after the first paint.
            const preferredLocale = resolveInitialLocale({
                cookieLocale: savedLocale,
                acceptLanguage: import.meta.server
                    ? useRequestHeaders(['accept-language'])['accept-language']
                    : null,
                browserLanguages: getBrowserLanguages(),
                supported: supportedLocaleCodes,
                defaultLocale: DEFAULT_LOCALE,
            })
            if (preferredLocale !== locale.value) await setLocale(preferredLocale)
        })
    },
})

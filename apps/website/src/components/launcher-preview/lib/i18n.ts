import { transformCrowdinMessages } from '@modrinth/ui/src/composables/i18n.ts'
import { uiLocaleModules } from '@modrinth/ui/src/locales.ts'
import { type I18nContext, injectI18n, provideI18n } from '@modrinth/ui/src/providers/i18n.ts'
import IntlMessageFormat from 'intl-messageformat'
import { computed, ref, shallowRef } from 'vue'

/**
 * Translations owned by the launcher preview, keyed by locale.
 *
 * Every preview screen ships its own English descriptors and registers the
 * matching overlay when its chunk loads, so a visitor only downloads the
 * strings of the screens they open. English needs no overlay: a missing key
 * falls back to the descriptor's `defaultMessage`.
 */
const overlays: Record<string, Record<string, string>> = {}
const formatters = new Map<string, IntlMessageFormat>()

export function registerPreviewMessages(locale: string, messages: Record<string, string>) {
    overlays[locale] = Object.assign(overlays[locale] ?? {}, messages)
}

function format(message: string, locale: string, values?: Record<string, unknown>) {
    if (!/[{}'<>]/.test(message)) return message
    const cacheKey = `${locale}\u0000${message}`
    let formatter = formatters.get(cacheKey)
    try {
        if (!formatter) {
            formatter = new IntlMessageFormat(message, locale)
            formatters.set(cacheKey, formatter)
        }
        return formatter.format(values ?? {}) as string
    } catch {
        return message
    }
}

/** Shared UI strings of a language the site has not loaded (the app bundles them all). */
const uiMessages = shallowRef<Record<string, Record<string, string>>>({})

async function loadUiMessages(code: string) {
    if (uiMessages.value[code]) return
    const loader = Object.entries(uiLocaleModules).find(([path]) =>
        path.endsWith(`/${code}/index.json`),
    )?.[1]
    const data = loader ? await loader().catch(() => null) : null
    uiMessages.value = {
        ...uiMessages.value,
        [code]: data ? transformCrowdinMessages(data.default) : {},
    }
}

/**
 * Provides an i18n context for the preview subtree. The window follows the
 * site's language until a language is picked inside it (Settings → Language),
 * which changes only the window, as the app's setting changes only the app.
 * Preview keys resolve from the registered overlays, shared UI strings from the
 * site's catalogue or, for another language, from the shared UI locale files.
 */
export function providePreviewI18n(): I18nContext {
    const site = injectI18n()
    const picked = ref<string | null>(null)
    const current = computed(() => picked.value ?? site.locale.value)

    async function setLocale(code: string) {
        if (code === site.locale.value) {
            picked.value = null
            return
        }
        await loadUiMessages(code)
        picked.value = code
    }

    const locale = computed({
        get: () => current.value,
        set: (code: string) => void setLocale(code),
    })

    function t(key: string, values?: Record<string, unknown>): string {
        const code = current.value
        const message = overlays[code]?.[key]
        if (message !== undefined) return format(message, code, values)
        if (code === site.locale.value) return site.t(key, values)
        const shared = uiMessages.value[code]?.[key]
        return shared === undefined ? key : format(shared, code, values)
    }

    currentContext = { locale, t }
    return provideI18n({ locale, t, setLocale })
}

let currentContext: Pick<I18nContext, 'locale' | 't'> | null = null

/**
 * `formatMessage` outside a component's setup, in the window's language, for
 * the ported app modules that build error messages in plain functions.
 */
export function formatPreviewMessage(
    descriptor: { id: string; defaultMessage?: string },
    values?: Record<string, unknown>,
): string {
    const translated = currentContext?.t(descriptor.id, values)
    if (translated && translated !== descriptor.id) return translated
    return format(
        descriptor.defaultMessage ?? descriptor.id,
        currentContext?.locale.value ?? 'en-US',
        values,
    )
}

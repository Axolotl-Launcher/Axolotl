export interface ResolveLocaleInput {
    /** Locale previously persisted in the `locale` cookie, if any. */
    cookieLocale?: string | null
    /** Raw `Accept-Language` request header, typically only available during SSR. */
    acceptLanguage?: string | null
    /** Navigator languages, in the browser's own preference order. */
    browserLanguages?: readonly string[]
    /** Locale codes this site can render. */
    supported: readonly string[]
    /** Locale used when nothing else matches. */
    defaultLocale: string
}

function normalizeLanguageTag(tag: string, supported: readonly string[]): string | null {
    if (supported.includes(tag)) return tag

    const language = tag.toLowerCase()
    if (language.startsWith('zh')) return 'zh-CN'
    if (language.startsWith('en')) return 'en-US'

    return null
}

/**
 * Picks the first supported locale out of an `Accept-Language` style ordered list.
 * `zh*` tags normalize to `zh-CN` and `en*` tags to `en-US`, so regional variants
 * such as `zh-TW` or `en-GB` still resolve to a locale the site ships.
 */
function resolveFromTags(tags: readonly string[], supported: readonly string[]): string | null {
    for (const tag of tags) {
        const normalized = normalizeLanguageTag(tag.trim(), supported)
        if (normalized) return normalized
    }

    return null
}

function parseAcceptLanguage(header: string): string[] {
    const tags: string[] = []

    for (const part of header.split(',')) {
        const [code] = part.trim().split(';')
        if (code) tags.push(code)
    }

    return tags
}

/**
 * Resolves the locale to render, in priority order: a supported `locale` cookie,
 * then the `Accept-Language` header, then the browser's language preferences,
 * and finally `defaultLocale`.
 */
export function resolveLocale(input: ResolveLocaleInput): string {
    const { cookieLocale, acceptLanguage, browserLanguages, supported, defaultLocale } = input

    if (cookieLocale) {
        const fromCookie = normalizeLanguageTag(cookieLocale.trim(), supported)
        if (fromCookie && supported.includes(fromCookie)) return fromCookie
    }

    if (acceptLanguage) {
        const fromHeader = resolveFromTags(parseAcceptLanguage(acceptLanguage), supported)
        if (fromHeader) return fromHeader
    }

    if (browserLanguages?.length) {
        const fromBrowser = resolveFromTags(browserLanguages, supported)
        if (fromBrowser) return fromBrowser
    }

    return defaultLocale
}

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

/** Quality assumed for a tag that carries no `q` parameter, or a malformed one. */
const DEFAULT_QUALITY = 1

/**
 * Reads the `q` weight out of a tag's parameters. A missing, non-numeric or
 * out-of-range weight falls back to {@link DEFAULT_QUALITY} rather than throwing,
 * so a sloppy header still resolves instead of failing the request.
 */
function parseQuality(params: readonly string[]): number {
    for (const param of params) {
        const separator = param.indexOf('=')
        if (separator === -1) continue
        if (param.slice(0, separator).trim().toLowerCase() !== 'q') continue

        const raw = param.slice(separator + 1).trim()
        if (!raw) return DEFAULT_QUALITY

        const quality = Number(raw)
        if (!Number.isFinite(quality) || quality < 0 || quality > 1) return DEFAULT_QUALITY

        return quality
    }

    return DEFAULT_QUALITY
}

/**
 * Turns an `Accept-Language` header into tags ordered by their `q` weight,
 * highest first, dropping the `q=0` entries that explicitly say "not acceptable".
 * Equal weights keep their written order: the sort carries the original index and
 * compares it second, so it does not depend on the engine's sort being stable.
 */
function parseAcceptLanguage(header: string): string[] {
    const entries: { tag: string; quality: number; index: number }[] = []

    header.split(',').forEach((part, index) => {
        const [code, ...params] = part.split(';')
        const tag = code?.trim()
        if (!tag) return

        entries.push({ tag, quality: parseQuality(params), index })
    })

    entries.sort((a, b) => b.quality - a.quality || a.index - b.index)

    return entries.filter((entry) => entry.quality > 0).map((entry) => entry.tag)
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

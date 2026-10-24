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

/**
 * Maps a caller-supplied tag onto a locale the site can actually render.
 *
 * A regional variant only normalizes to its base locale when the caller lists
 * that base locale as supported: `zh-TW` becomes `zh-CN` only if `zh-CN` is in
 * `supported`. Returning a hard-coded base locale regardless would hand the
 * caller a code it explicitly said it cannot render.
 */
function normalizeLanguageTag(tag: string, supported: readonly string[]): string | null {
    if (supported.includes(tag)) return tag

    const language = tag.toLowerCase()
    const base = language.startsWith('zh') ? 'zh-CN' : language.startsWith('en') ? 'en-US' : null

    return base && supported.includes(base) ? base : null
}

/**
 * Picks the first supported locale out of an `Accept-Language` style ordered list.
 * `zh*` tags normalize to `zh-CN` and `en*` tags to `en-US` — but only when the
 * caller supports that base locale — so regional variants such as `zh-TW` or
 * `en-GB` still resolve to a locale the site ships.
 */
function resolveFromTags(tags: readonly string[], supported: readonly string[]): string | null {
    for (const tag of tags) {
        const normalized = normalizeLanguageTag(tag.trim(), supported)
        if (normalized) return normalized
    }

    return null
}

/** Quality assumed for a tag that carries no `q` parameter. */
const DEFAULT_QUALITY = 1

/** Returned for a `q` that is present but unusable, so the tag ranks last. */
const INVALID_QUALITY = 0

/**
 * Reads the `q` weight out of a tag's parameters.
 *
 * A tag with no `q` parameter is fully acceptable ({@link DEFAULT_QUALITY}). A tag
 * that carries one but cannot be read — empty, non-numeric, or outside 0..1 — is
 * not treated as fully acceptable: promoting it to 1 would let a malformed entry
 * outrank an entry that legitimately asked for 0.9. It is rejected instead, which
 * is also what `q=0` means.
 */
function parseQuality(params: readonly string[]): number {
    for (const param of params) {
        const separator = param.indexOf('=')
        if (separator === -1) continue
        if (param.slice(0, separator).trim().toLowerCase() !== 'q') continue

        const raw = param.slice(separator + 1).trim()
        if (!raw) return INVALID_QUALITY

        const quality = Number(raw)
        if (!Number.isFinite(quality) || quality < 0 || quality > 1) return INVALID_QUALITY

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
    // One entry per tag: a header may repeat a tag, and sending `en-US;q=0` next to
    // `en-US;q=0.1` must not let the second entry resurrect a rejected language. The
    // first occurrence decides the tag's position, the highest weight decides whether
    // it survives.
    const entries = new Map<string, { tag: string; quality: number; index: number }>()

    header.split(',').forEach((part, index) => {
        const [code, ...params] = part.split(';')
        const tag = code?.trim()
        // `*` is not a locale: it means "anything else", which `defaultLocale` already
        // covers, and treating it as a tag would make it compete with real ones.
        if (!tag || tag === '*') return

        const key = tag.toLowerCase()
        const quality = parseQuality(params)
        const seen = entries.get(key)
        entries.set(key, {
            // Keep the first spelling of the tag; the lower-cased key only groups
            // repeats, so a supported code is still matched exactly as written.
            tag: seen ? seen.tag : tag,
            quality: seen ? Math.max(seen.quality, quality) : quality,
            index: seen ? seen.index : index,
        })
    })

    return [...entries.values()]
        .filter((entry) => entry.quality > 0)
        .sort((a, b) => b.quality - a.quality || a.index - b.index)
        .map((entry) => entry.tag)
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
        if (fromCookie) return fromCookie
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

export interface InitialLocaleInput {
    /** Locale already persisted in the `locale` cookie. */
    cookieLocale?: string | null
    /** `Accept-Language` of the request; only available while rendering on the server. */
    acceptLanguage?: string | null
    /** `navigator.languages`; only available once the page is running in a browser. */
    browserLanguages?: readonly string[]
    supported: readonly string[]
    defaultLocale: string
}

/**
 * Picks the locale for the first paint and for the client that hydrates it.
 *
 * Both sides have to agree. The server only sees the request, so it decides from
 * the cookie and `Accept-Language`; the browser additionally knows
 * `navigator.languages`, which may disagree with the header. Letting the client
 * re-resolve on its own would then switch the language after the first paint and
 * re-render a page the visitor had already started reading.
 *
 * The client therefore only overrides the server's answer when the server had
 * nothing to go on: no cookie and no header. That keeps the browser preference
 * useful for a visitor whose header says nothing, without ever contradicting a
 * decision the server already made.
 */
export function resolveInitialLocale(input: InitialLocaleInput): string {
    const { cookieLocale, acceptLanguage, browserLanguages, supported, defaultLocale } = input

    const serverAnswer = resolveLocale({
        cookieLocale,
        acceptLanguage,
        supported,
        defaultLocale,
    })

    const serverHadEvidence = Boolean(cookieLocale?.trim()) || Boolean(acceptLanguage?.trim())
    if (serverHadEvidence) return serverAnswer

    return resolveLocale({ cookieLocale, browserLanguages, supported, defaultLocale })
}

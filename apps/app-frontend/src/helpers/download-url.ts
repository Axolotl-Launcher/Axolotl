import { normalizeModrinthCdnUrl } from './modrinth-cdn.ts'

/** Recover known public mirror paths; unknown retired URLs must not be opened. */
export function resolveDownloadUrl(input: string): string | undefined {
    let url: URL
    try {
        url = new URL(input)
    } catch {
        return input
    }
    if (url.hostname.replace(/\.$/, '') !== 'mod.tianpao.top') return normalizeModrinthCdnUrl(input)
    if (url.protocol !== 'https:' || url.port !== '' || url.username || url.password)
        return undefined
    const authority = /^https:\/\/mod\.tianpao\.top(?::443)?(?=\/|\?|#|$)/i.exec(input)
    if (!authority) return undefined
    const suffix = input.slice(authority[0].length)
    const path = suffix.split(/[?#]/)[0]
    if (path.startsWith('/data/') && url.pathname.startsWith('/data/'))
        return 'https://cdn.modrinth.com' + suffix
    if (path.startsWith('/files/') && url.pathname.startsWith('/files/'))
        return 'https://edge.forgecdn.net' + suffix
    if (path.startsWith('/media/') && url.pathname.startsWith('/media/'))
        return 'https://media.forgecdn.net' + suffix.slice('/media'.length)
    return undefined
}

/** Restore URL fields in desktop cache responses without altering stored records or prose. */
export function restoreCachedUrls<T>(value: T): T {
    const urlFields = new Set([
        'url',
        'raw_url',
        'icon_url',
        'avatar_url',
        'featured_gallery',
        'thumbnailUrl',
        'thumbnail_url',
    ])
    function visit(value: unknown, field?: string): unknown {
        if (typeof value === 'string')
            return field && (urlFields.has(field) || field === 'gallery')
                ? (resolveDownloadUrl(value) ?? '')
                : value
        if (Array.isArray(value)) return value.map((item) => visit(item, field))
        if (value && typeof value === 'object')
            return Object.fromEntries(
                Object.entries(value).map(([key, item]) => [key, visit(item, key)]),
            )
        return value
    }
    return visit(value) as T
}

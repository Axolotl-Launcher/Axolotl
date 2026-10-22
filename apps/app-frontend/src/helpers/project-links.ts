import type { LocationQuery, LocationQueryRaw } from 'vue-router'

export type ProjectBrowseFilter = 'category' | 'loader'

export function createProjectDetailsQuery(
    query: LocationQuery,
    browsePath: string,
): LocationQueryRaw {
    const details = { ...query, b: browsePath }
    delete details.g
    delete details.l
    delete details.c
    delete details.page
    return details
}

export function createProjectBrowseLocation(
    projectType: string,
    filter: ProjectBrowseFilter,
    value: string,
): { path: string; query: LocationQueryRaw } {
    const browseProjectType = projectType === 'minecraft_java_server' ? 'server' : projectType
    const query =
        filter === 'loader'
            ? { g: `categories:${value}` }
            : browseProjectType === 'server'
              ? { sc: value }
              : { f: `categories:${value}` }

    return {
        path: `/browse/${browseProjectType}`,
        query,
    }
}

const MODRINTH_HOSTNAMES = new Set(['modrinth.com', 'www.modrinth.com'])

const APP_LOCAL_HOSTNAMES = new Set(['localhost', 'tauri.localhost'])

const SUPPORTED_PROJECT_TYPES = new Set([
    'mod',
    'modpack',
    'resourcepack',
    'datapack',
    'plugin',
    'shader',
    'server',
    'project',
])

export function parseModrinthLink(
    href: string,
): { slug: string; pathSuffix: string; url: URL } | null {
    let url: URL
    try {
        url = new URL(href)
    } catch {
        return null
    }

    if (!MODRINTH_HOSTNAMES.has(url.hostname.toLowerCase())) {
        return null
    }

    const segments = url.pathname.split('/').filter((p) => p.length > 0)
    if (segments.length < 2) {
        return null
    }

    if (SUPPORTED_PROJECT_TYPES.has(segments[0].toLowerCase())) {
        const slug = segments[1]
        if (!slug) {
            return null
        }

        const rest: string[] = segments.slice(2)
        const pathSuffix = toValidAppSubpath(rest)
        if (pathSuffix === null) {
            return null
        }

        return { slug, pathSuffix, url }
    } else {
        return null
    }
}

/**
 * Returns the URL that should be handed to the native browser opener for an
 * anchor, or null when the anchor belongs to the launcher web view.
 *
 * Links without a scheme are resolved by Tauri against tauri.localhost. Use
 * the original href to recover the intended external host before opening it.
 */
export function getExternalLinkUrl(href: string, resolvedHref: string): string | null {
    const rawHref = href.trim()
    if (!rawHref || rawHref.startsWith('#') || rawHref.startsWith('?')) {
        return null
    }

    const linkoutUrl = getLinkoutUrl(rawHref, resolvedHref)
    if (linkoutUrl) return linkoutUrl

    if (rawHref.startsWith('/')) return null

    if (/^(?:https?:|mailto:|tel:)/i.test(rawHref) || rawHref.startsWith('//')) {
        return isAppLocalUrl(resolvedHref) ? null : resolvedHref
    }

    if (!isAppLocalUrl(resolvedHref)) return null

    try {
        const url = new URL(`https://${rawHref}`)
        if (
            !url.hostname.includes('.') ||
            url.hostname === 'localhost' ||
            url.hostname.endsWith('.localhost')
        ) {
            return null
        }
        return url.toString()
    } catch {
        return null
    }
}

function getLinkoutUrl(href: string, resolvedHref: string): string | null {
    let url: URL
    try {
        url = new URL(href, resolvedHref)
    } catch {
        return null
    }

    if (!isAppLocalUrl(url.href) || url.pathname !== '/linkout') return null

    const remoteUrl = url.searchParams.get('remoteUrl')
    if (!remoteUrl) return null

    let decodedUrl = remoteUrl
    for (let i = 0; i < 3; i++) {
        try {
            const nextUrl = decodeURIComponent(decodedUrl)
            if (nextUrl === decodedUrl) break
            decodedUrl = nextUrl
        } catch {
            return null
        }
    }

    try {
        const parsedUrl = new URL(decodedUrl)
        return parsedUrl.protocol === 'http:' || parsedUrl.protocol === 'https:'
            ? parsedUrl.href
            : null
    } catch {
        return null
    }
}

function isAppLocalUrl(href: string): boolean {
    try {
        return APP_LOCAL_HOSTNAMES.has(new URL(href).hostname.toLowerCase())
    } catch {
        return false
    }
}

const SUPPORTED_SUBPATHS = ['versions', 'gallery']

function toValidAppSubpath(rest: string[]): string | null {
    if (rest.length === 0) {
        return ''
    }

    const subroute = rest[0].toLowerCase()
    if (rest.length === 1 && SUPPORTED_SUBPATHS.includes(subroute)) {
        return `/${subroute}`
    }

    if (rest.length === 2 && subroute === 'version') {
        return `/version/${rest[1]}`
    }

    return null
}

export function mergeUrlQuery(routeQuery: LocationQuery, linkUrl: URL): LocationQueryRaw {
    const newQuery: LocationQueryRaw = { ...routeQuery }
    const keys = new Set<string>()
    linkUrl.searchParams.forEach((_value, key) => {
        keys.add(key)
    })
    for (const key of keys) {
        const values = linkUrl.searchParams.getAll(key)
        newQuery[key] = values.length === 1 ? values[0] : values
    }
    return newQuery
}

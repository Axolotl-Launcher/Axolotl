import { getVisibleSettingsCategoryDefinitions } from '@/components/ui/settings/settings-category-definitions'

const SAFE_QUERY = /^[A-Za-z0-9\-_.~%&=+]*$/

const PROJECT_TYPES = new Set([
    'mod',
    'modpack',
    'resourcepack',
    'datapack',
    'shader',
    'plugin',
    'server',
    'schematic',
])

const SETTINGS_TABS = new Set<string>(
    getVisibleSettingsCategoryDefinitions(false).map((category) => category.id),
)

const LAB_TOOLS = new Set([
    'skin-editor',
    'gradient-text',
    'recipe-generator',
    'seed-map',
    'schematic-preview',
    'mod-translation',
])

function safeQuery(query: string | null | undefined): Record<string, string> {
    if (!query || !SAFE_QUERY.test(query)) return {}
    return Object.fromEntries(new URLSearchParams(query))
}

function route(path: string, query?: string | null) {
    return { path, query: safeQuery(query ?? undefined) }
}

export function resolveOpenRoute(path: string, query?: string | null) {
    if (path === '/') return route('/')
    if (path === '/browse/favorites') return route('/browse/favorites')
    if (path === '/library' || path.startsWith('/library/')) {
        const tail = path.slice('/library'.length)
        if (['', '/downloaded', '/modpacks', '/servers', '/custom'].includes(tail)) {
            return route(path, query)
        }
        return null
    }
    if (path.startsWith('/browse/')) {
        const type = path.slice('/browse/'.length)
        if (PROJECT_TYPES.has(type) && !type.includes('/')) return route(path, query)
        return null
    }
    if (path.startsWith('/project/')) {
        const rest = path.slice('/project/'.length)
        const [id, tab] = rest.split('/')
        if (id && (!tab || ['versions', 'gallery', 'changelog'].includes(tab))) {
            return route(path, query)
        }
        return null
    }
    if (path.startsWith('/instance/')) {
        const id = path.slice('/instance/'.length).split('/')[0]
        const tail = path.slice(`/instance/${id}`.length)
        if (
            id &&
            [
                '',
                '/files',
                '/files/studio',
                '/logs',
                '/worlds',
                '/screenshots',
                '/upgrade',
            ].includes(tail)
        ) {
            return route(path, query)
        }
        return null
    }
    if (path.startsWith('/multiplayer')) {
        if (
            path === '/multiplayer/servers' ||
            path === '/multiplayer/rooms' ||
            path.startsWith('/multiplayer/servers/')
        ) {
            return route(path, query)
        }
        return null
    }
    if (path.startsWith('/lab')) {
        const tail = path.slice('/lab'.length)
        if (tail === '' || [...LAB_TOOLS].some((tool) => tail === `/${tool}`)) {
            return route(path, query)
        }
        return null
    }
    if (
        ['/downloads', '/create', '/skins', '/worlds', '/screenshots', '/help/drop'].includes(path)
    ) {
        return route(path, query)
    }
    return null
}

export function resolveSettingsRoute(tab?: string | null, entry?: string | null) {
    const cleanTab = tab && SETTINGS_TABS.has(tab) ? tab : 'interface'
    const cleanEntry =
        entry && /^[A-Za-z0-9\-_]+$/.test(entry) ? `settings-target-${entry}` : undefined
    return { path: '/settings', hash: `#${cleanTab}`, entry: cleanEntry }
}

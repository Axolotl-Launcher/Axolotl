import announcements from '../data/announcements.json'

type Route = { matches: (url: URL) => boolean; respond: (url: URL) => Response }

function json(body: unknown) {
    return new Response(JSON.stringify(body), {
        status: 200,
        headers: { 'content-type': 'application/json' },
    })
}

/** Web requests the launcher makes itself, answered from recordings (data/). */
const routes: Route[] = [
    {
        matches: (url) =>
            url.host === 'admin.axlmc.org' && url.pathname === '/api/public/announcements',
        respond: () => json({ announcements: announcements.announcements }),
    },
]

/**
 * The demo reaches no service: recorded endpoints answer locally, same-origin
 * requests (the demo's own files) pass through, and anything else fails the way
 * an offline request does.
 */
export function installDemoNetwork() {
    const pageFetch = window.fetch.bind(window)
    window.fetch = async (input, init) => {
        const href = input instanceof Request ? input.url : String(input)
        const url = new URL(href, window.location.href)
        if (url.origin === window.location.origin) return pageFetch(input, init)
        const route = routes.find((entry) => entry.matches(url))
        if (route) return route.respond(url)
        throw new TypeError('Failed to fetch')
    }
}

export type ContentTabMemory = Record<string, Map<string, unknown>>

// The browser keeps one store for the lifetime of the page, so moving between
// tabs and back restores the filters, search, sort mode and view preferences the
// user left behind; closing the app discards them. A server render must not share
// that store: one Node process answers many requests, and reusing the previous
// request's maps would render one visitor's UI state into another visitor's HTML.
// Server renders therefore get a throwaway store, one per call.
function getMemory(): ContentTabMemory {
    if (typeof window === 'undefined') return {}
    const scope = globalThis as unknown as { __ctMemory?: ContentTabMemory }
    return (scope.__ctMemory ??= {})
}

/**
 * Returns the map stored under `namespace`.
 *
 * Call this once per namespace and keep the result in a variable. On the server
 * every call yields a fresh store, so reading the same namespace twice in one
 * render returns two unconnected maps; writes made through the kept variable stay
 * visible for the rest of that render.
 */
export function getContentTabMap<K, V>(namespace: string): Map<K, V> {
    const memory = getMemory()
    let map = memory[namespace]
    if (!map) {
        map = new Map<string, unknown>()
        memory[namespace] = map
    }
    return map as Map<K, V>
}

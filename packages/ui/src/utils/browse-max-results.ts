/**
 * Persistence for the browse page's "cards per page" choice.
 *
 * Kept in its own module so it can be unit-tested without pulling in the search
 * composable's API-client dependencies.
 */

export const MAX_RESULTS_STORAGE_KEY = 'browse-max-results'
export const MAX_RESULTS_DEFAULT = 20
/** The largest page size the browse UI offers; nothing above it is a real choice. */
export const MAX_RESULTS_LIMIT = 100

/**
 * Read the persisted page size.
 *
 * Returns the default when storage is unavailable (server rendering, privacy
 * mode) or holds anything that is not a positive integer, so a corrupted entry
 * can never break the browse page. Anything above `MAX_RESULTS_LIMIT` is clamped
 * to it, because a hand-edited entry would otherwise reach `limit=` untouched.
 *
 * The `localStorage` access itself sits inside the `try`: in a sandboxed or
 * opaque-origin context merely reading the global can throw, and that must not
 * take the browse page down with it.
 */
export function readPersistedMaxResults(): number {
    try {
        if (typeof localStorage === 'undefined') return MAX_RESULTS_DEFAULT
        const raw = localStorage.getItem(MAX_RESULTS_STORAGE_KEY)
        if (!raw) return MAX_RESULTS_DEFAULT
        const parsed = Number.parseInt(raw, 10)
        if (!Number.isFinite(parsed) || parsed < 1) return MAX_RESULTS_DEFAULT
        // A hand-edited entry must not become an unbounded `limit=` request.
        return Math.min(parsed, MAX_RESULTS_LIMIT)
    } catch {
        return MAX_RESULTS_DEFAULT
    }
}

export function persistMaxResults(value: number): void {
    // Only a real user choice is worth persisting. A value clamped down because
    // the current tab offers fewer options must not overwrite the preference the
    // user set on a tab that offered more.
    if (!Number.isFinite(value) || value < 1) return
    try {
        if (typeof localStorage === 'undefined') return
        localStorage.setItem(MAX_RESULTS_STORAGE_KEY, String(Math.min(value, MAX_RESULTS_LIMIT)))
    } catch {
        // A full or blocked storage quota must not break browsing.
    }
}

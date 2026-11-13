import {
    isNavigationFailure,
    type NavigationFailure,
    NavigationFailureType,
    type RouteLocationRaw,
    type Router,
} from 'vue-router'

export interface NavigationOutcome {
    /** True when the router committed the target (duplicated counts as committed). */
    ok: boolean
    failure?: NavigationFailure | void
    error?: unknown
}

/**
 * Awaits a router navigation and classifies its outcome.
 *
 * `RouterLink` swallows the promise returned by the router, so components that
 * start an optimistic selection need to push themselves to know whether the
 * route actually committed and can roll the selection back on failure.
 */
export async function navigateSettled(
    router: Router,
    to: RouteLocationRaw,
    replace = false,
): Promise<NavigationOutcome> {
    try {
        const failure = replace ? await router.replace(to) : await router.push(to)
        if (!failure) return { ok: true }
        if (isNavigationFailure(failure, NavigationFailureType.duplicated)) {
            return { ok: true, failure }
        }
        return { ok: false, failure }
    } catch (error) {
        return { ok: false, error }
    }
}

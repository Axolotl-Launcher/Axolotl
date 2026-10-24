import { loadRouteLocation, type Router } from 'vue-router'

import type { PreviewState } from './state'

const PRIMARY_ROUTES = [
    '/screenshots',
    '/browse/mod',
    '/skins',
    '/multiplayer/servers',
    '/library',
    '/lab',
]

/** Prepare the visible rail destinations before the startup screen releases interaction. */
export async function preparePrimaryNavigation(
    router: Router,
    preview: PreviewState,
    signal: AbortSignal,
    onProgress: (progress: number) => void,
) {
    for (const [index, path] of PRIMARY_ROUTES.entries()) {
        signal.throwIfAborted()
        await loadRouteLocation(router.resolve(path))
        if (path === '/browse/mod') {
            const { prepareDiscover } = await import('../screens/discover/state')
            await prepareDiscover(preview)
        } else if (path === '/multiplayer/servers') {
            const { refresh } = await import('../screens/multiplayer/composables/useServers')
            await refresh()
        }
        onProgress((index + 1) / PRIMARY_ROUTES.length)
        // Let the device paint the splash between module evaluations.
        await new Promise<void>((resolve) =>
            requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
        )
    }
    signal.throwIfAborted()
}

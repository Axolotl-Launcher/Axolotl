import type { provideModrinthClient } from '@modrinth/ui/src/providers/api-client.ts'

import { formatPreviewMessage } from './i18n'
import { desktopAppMessages } from './messages'

type ModrinthClient = Parameters<typeof provideModrinthClient>[0]

/**
 * The API client the app provides to shared @modrinth/ui layouts. The preview
 * sends no requests from the window (sharing logs to mclo.gs, downloading
 * from a URL), so every call answers that the desktop app does this.
 */
export function createPreviewModrinthClient(): ModrinthClient {
    const call = () =>
        Promise.reject(new Error(formatPreviewMessage(desktopAppMessages.onlineService)))
    const node: unknown = new Proxy(call, {
        get: (_target, key) => (key === 'then' ? undefined : node),
        apply: () => call(),
    })
    return node as ModrinthClient
}

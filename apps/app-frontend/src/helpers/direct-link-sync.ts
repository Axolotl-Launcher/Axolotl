import {
    createDirectLinkSyncController,
    directLinkSyncChanged,
    startDirectLinkSyncOnFocus,
} from './direct-link-sync-controller'
import {
    type DirectLinkSyncReport,
    type ExternalMinecraftRoot,
    sync_direct_links,
} from './instance'

export const DIRECT_LINKS_SYNCED_EVENT = 'axolotl-direct-links-synced'
export const DIRECT_LINKS_CHANGED_EVENT = 'axolotl-direct-links-changed'

const sync = createDirectLinkSyncController(sync_direct_links, (report) => {
    window.dispatchEvent(
        new CustomEvent<DirectLinkSyncReport>(DIRECT_LINKS_SYNCED_EVENT, { detail: report }),
    )
    if (directLinkSyncChanged(report)) {
        window.dispatchEvent(
            new CustomEvent<DirectLinkSyncReport>(DIRECT_LINKS_CHANGED_EVENT, { detail: report }),
        )
    }
})

/**
 * Serializes explicit configuration/import requests, preserving the latest
 * roots when they change during an in-flight reconciliation.
 */
export function syncConfiguredDirectLinks(
    roots: readonly ExternalMinecraftRoot[],
): Promise<DirectLinkSyncReport> {
    return sync(roots)
}

export function startConfiguredDirectLinkSync(
    readRoots: () => readonly ExternalMinecraftRoot[],
    onError: (error: unknown) => void,
): () => void {
    return startDirectLinkSyncOnFocus(() => sync(readRoots(), true), onError, window)
}

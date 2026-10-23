import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { computed, ref } from 'vue'

import { computeServerStatus, type ServerStatus } from '../../../lib/server-core.ts'
import {
    serverEventListener,
    type ServerExitReason,
    type ServerInfoData,
    servers,
} from '../backend.ts'

/** Reacts to a classified server self-exit, e.g. opening the EULA dialog. */
type ServerExitReasonHandler = (serverId: string, reason: ServerExitReason) => void
let exitReasonHandler: ServerExitReasonHandler | null = null

/**
 * Registers the single handler invoked when a server exits with a classified
 * reason. Returns a disposer that only clears the handler while it is still
 * the registered one.
 */
export function setServerExitReasonHandler(handler: ServerExitReasonHandler | null) {
    const registered = handler
    exitReasonHandler = handler
    return () => {
        if (exitReasonHandler === registered) exitReasonHandler = null
    }
}

const serverList = ref<ServerInfoData[]>([])
const isRefreshing = ref(false)
const hasLoaded = ref(false)
const loadError = ref<unknown>(null)
let refreshPromise: Promise<void> | null = null
let refreshRequested = false
let listenerPromise: Promise<() => void> | null = null
const consoleOutputListeners = new Map<string, Set<() => void>>()

export interface ServerView extends ServerInfoData {
    status: ServerStatus
}

export function serverStatus(server: ServerInfoData): ServerStatus {
    return computeServerStatus({
        manifest: { id: server.id },
        isRunning: server.running,
        isStarting: server.starting ?? false,
        lastExitWasCrash: server.lastExitCrashed,
        eulaAccepted: server.eulaAccepted,
        eulaFileExists: server.eulaExists,
    })
}

async function ensureListener() {
    if (!listenerPromise) {
        listenerPromise = serverEventListener((serverId, payload) => {
            if (payload.event === 'console_output') {
                for (const listener of consoleOutputListeners.get(serverId) ?? []) {
                    listener()
                }
            } else if (
                payload.event === 'starting' ||
                payload.event === 'stopping' ||
                payload.event === 'started'
            ) {
                refreshRequested = true
                void refresh().catch(() => {})
            } else if (payload.event === 'stopped') {
                refreshRequested = true
                void refresh().catch(() => {})
                if (payload.reason) exitReasonHandler?.(serverId, payload.reason)
            }
        }).catch((error) => {
            listenerPromise = null
            throw error
        })
    }
    return listenerPromise
}

export function subscribeServerConsoleOutput(serverId: string, listener: () => void): () => void {
    const listeners = consoleOutputListeners.get(serverId) ?? new Set()
    listeners.add(listener)
    consoleOutputListeners.set(serverId, listeners)
    void ensureListener().catch(() => {})
    return () => {
        listeners.delete(listener)
        if (listeners.size === 0) consoleOutputListeners.delete(serverId)
    }
}

export function refresh(): Promise<void> {
    if (refreshPromise) return refreshPromise
    isRefreshing.value = true
    loadError.value = null
    refreshPromise = (async () => {
        try {
            await ensureListener()
            do {
                refreshRequested = false
                serverList.value = await servers.list()
            } while (refreshRequested)
            hasLoaded.value = true
        } catch (error) {
            loadError.value = error
            throw error
        } finally {
            isRefreshing.value = false
            refreshPromise = null
        }
    })()
    return refreshPromise
}

export function useServers() {
    const { handleError } = injectNotificationManager()

    const serverViews = computed<ServerView[]>(() =>
        serverList.value.map((server) => ({ ...server, status: serverStatus(server) })),
    )

    async function run(action: () => Promise<unknown>): Promise<boolean> {
        try {
            await action()
            return true
        } catch (error) {
            handleError(error)
            return false
        }
    }

    async function refreshList() {
        await run(refresh)
    }

    async function startServer(serverId: string) {
        await ensureListener()
        const ok = await run(() => servers.start(serverId))
        if (ok) await refresh()
        return ok
    }

    async function stopServer(serverId: string) {
        const ok = await run(() => servers.stop(serverId))
        if (ok) await refreshList()
        return ok
    }

    async function killServer(serverId: string) {
        const ok = await run(() => servers.kill(serverId))
        if (ok) await refreshList()
        return ok
    }

    async function deleteServer(serverId: string) {
        const ok = await run(() => servers.delete(serverId))
        if (ok) {
            await refresh()
        }
        return ok
    }

    return {
        servers: serverViews,
        rawServers: serverList,
        isRefreshing,
        hasLoaded,
        loadError,
        refresh: refreshList,
        startServer,
        stopServer,
        killServer,
        deleteServer,
    }
}

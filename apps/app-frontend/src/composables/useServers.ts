import { computeServerStatus, type ServerStatus } from '@modrinth/server'
import { injectNotificationManager } from '@modrinth/ui'
import { computed, ref } from 'vue'

import {
    base64ToBytes,
    serverEventListener,
    type ServerExitReason,
    type ServerInfoData,
    servers,
} from '@/helpers/servers'

import { ServerConsoleBuffer } from './server-console-buffer'

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
let listenerPromise: Promise<() => void> | null = null
const consoleOutputListeners = new Map<string, Set<(data: Uint8Array) => void>>()
const consoleOutputBuffers = new Map<string, ServerConsoleBuffer>()
const CONSOLE_OUTPUT_CAPACITY = 64 * 1024

export interface ServerView extends ServerInfoData {
    status: ServerStatus
}

export function serverStatus(server: ServerInfoData): ServerStatus {
    return computeServerStatus({
        manifest: { id: server.id },
        isRunning: server.running,
        isStarting: false,
        lastExitWasCrash: server.lastExitCrashed,
        eulaAccepted: server.eulaAccepted,
        eulaFileExists: server.eulaExists,
    })
}

async function ensureListener() {
    if (!listenerPromise) {
        listenerPromise = serverEventListener((serverId, payload) => {
            if (payload.event === 'console_output') {
                const data = base64ToBytes(payload.data)
                const buffer =
                    consoleOutputBuffers.get(serverId) ??
                    new ServerConsoleBuffer(CONSOLE_OUTPUT_CAPACITY)
                buffer.push(data)
                consoleOutputBuffers.set(serverId, buffer)
                for (const listener of consoleOutputListeners.get(serverId) ?? []) {
                    listener(data)
                }
            } else if (payload.event === 'started') {
                void refresh().catch(() => {})
            } else if (payload.event === 'stopped') {
                consoleOutputBuffers.delete(serverId)
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

export function subscribeServerConsoleOutput(
    serverId: string,
    listener: (data: Uint8Array) => void,
): () => void {
    const listeners = consoleOutputListeners.get(serverId) ?? new Set()
    listeners.add(listener)
    consoleOutputListeners.set(serverId, listeners)
    for (const data of consoleOutputBuffers.get(serverId)?.values() ?? []) listener(data)
    void ensureListener()
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
            serverList.value = await servers.list()
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
        return run(() => servers.stop(serverId))
    }

    async function killServer(serverId: string) {
        return run(() => servers.kill(serverId))
    }

    async function deleteServer(serverId: string) {
        const ok = await run(() => servers.delete(serverId))
        if (ok) {
            await refresh()
        }
        return ok
    }

    async function sendCommand(serverId: string, command: string) {
        return run(() => servers.sendCommand(serverId, command))
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
        sendCommand,
    }
}

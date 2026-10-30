import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { computed, readonly, shallowRef } from 'vue'

import { useMcpPreview } from './preview'

export interface McpState {
    revision: number
    enabled: boolean
    running: boolean
    full_access: boolean
    require_auth: boolean
    endpoint: string | null
    connection: {
        id: string
        client: string
        connected: boolean
        awaiting_approval: boolean
    } | null
    token: string | null
    error: string | null
}

const state = shallowRef<McpState>({
    revision: 0,
    enabled: false,
    running: false,
    full_access: false,
    require_auth: false,
    endpoint: null,
    connection: null,
    token: null,
    error: null,
})
const busy = shallowRef(false)
const loaded = shallowRef(false)
const operationError = shallowRef<string | null>(null)
let references = 0
let listening: Promise<UnlistenFn> | undefined

function apply(next: McpState) {
    if (next.revision < state.value.revision) return
    if (next.connection) useMcpPreview().disconnect()
    state.value = {
        ...next,
        token: next.running && next.require_auth ? (next.token ?? state.value.token) : null,
    }
}

async function action(command: string, args?: Record<string, unknown>) {
    operationError.value = null
    try {
        apply(await invoke<McpState>(`plugin:mcp|${command}`, args))
        return true
    } catch (error) {
        operationError.value = String(error)
        return false
    }
}

export async function startMcpStateSync() {
    references += 1
    listening ??= listen<McpState>('mcp-state-changed', ({ payload }) => apply(payload)).catch(
        (error) => {
            listening = undefined
            throw error
        },
    )
    try {
        await listening
        if (await action('mcp_get_state')) loaded.value = true
    } catch (error) {
        operationError.value = String(error)
    }
    let stopped = false
    return () => {
        if (stopped) return
        stopped = true
        references -= 1
        if (references === 0) {
            void listening?.then((unlisten) => unlisten()).catch(() => {})
            listening = undefined
        }
    }
}

export function useMcpRuntime() {
    return {
        state: readonly(state),
        loaded: readonly(loaded),
        busy: readonly(busy),
        error: computed(() => operationError.value ?? state.value.error),
        async configure(
            enabled: boolean,
            fullAccess: boolean,
            requireAuth = state.value.require_auth,
        ) {
            if (busy.value) return false
            busy.value = true
            try {
                return await action('mcp_configure', { enabled, fullAccess, requireAuth })
            } finally {
                busy.value = false
            }
        },
        approve(id: string) {
            return action('mcp_respond', { connectionId: id, allow: true })
        },
        disconnect(id: string) {
            return action('mcp_disconnect', { connectionId: id })
        },
    }
}

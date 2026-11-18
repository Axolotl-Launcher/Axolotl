import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { shallowRef } from 'vue'

import { invoke } from '@/helpers/invoke'

export interface ObservabilityMcpState {
    enabled: boolean
    running: boolean
    require_auth: boolean
    endpoint: string | null
    token: string | null
    error: string | null
}

export const OBSERVABILITY_MCP_EVENT = 'observability-mcp-state-changed'

const state = shallowRef<ObservabilityMcpState>({
    enabled: false,
    running: false,
    require_auth: false,
    endpoint: null,
    token: null,
    error: null,
})
const busy = shallowRef(false)
const loaded = shallowRef(false)
const operationError = shallowRef<string | null>(null)
let references = 0
let listening: Promise<UnlistenFn> | undefined

function apply(next: ObservabilityMcpState) {
    state.value = {
        ...next,
        token: next.running && next.require_auth ? (next.token ?? state.value.token) : null,
    }
}

async function action(command: string, args?: Record<string, unknown>) {
    operationError.value = null
    try {
        apply(await invoke<ObservabilityMcpState>(`plugin:observability-mcp|${command}`, args))
        return true
    } catch (error) {
        operationError.value = String(error)
        return false
    }
}

async function load() {
    if (references++ > 0) return
    listening ??= listen<ObservabilityMcpState>(OBSERVABILITY_MCP_EVENT, (event) =>
        apply(event.payload),
    )
    await action('observability_mcp_get_state')
    loaded.value = true
}

function release() {
    references = Math.max(0, references - 1)
}

export function useObservabilityMcpRuntime() {
    void load()

    return {
        state,
        loaded,
        busy,
        error: operationError,
        async configure(enabled: boolean, requireAuth: boolean) {
            busy.value = true
            try {
                return await action('observability_mcp_configure', {
                    enabled,
                    requireAuth,
                })
            } finally {
                busy.value = false
            }
        },
        release,
    }
}

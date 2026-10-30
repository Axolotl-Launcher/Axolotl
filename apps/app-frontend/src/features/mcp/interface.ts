import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { Router } from 'vue-router'

import { createMcpUiController, type UiAction } from './interface-dom'

function uiErrorMessage(error: unknown, action: UiAction) {
    if (error instanceof Error) return error.message
    if (typeof error === 'string' && error.trim()) return error
    if (error instanceof Event) {
        const target = error.target instanceof Element ? ` on ${error.target.tagName}` : ''
        return `Launcher UI ${action.kind} failed: ${error.type} event${target}`
    }
    return `Launcher UI ${action.kind} failed: unknown frontend error`
}

export async function startMcpUiBridge(router: Router) {
    const controller = createMcpUiController(router)
    let stopped = false
    const unlisten = await listen<{ request_id: string }>('mcp-ui-request', async ({ payload }) => {
        if (stopped) return
        try {
            const claim = await invoke<{ action: UiAction; scope: string }>(
                'plugin:mcp|mcp_ui_claim',
                { requestId: payload.request_id },
            )
            if (stopped) return
            let result: Record<string, unknown>
            try {
                result = { ok: true, data: await controller.execute(claim.action, claim.scope) }
            } catch (error) {
                result = {
                    ok: false,
                    error: uiErrorMessage(error, claim.action),
                }
            }
            await invoke('plugin:mcp|mcp_ui_respond', { requestId: payload.request_id, result })
        } catch {
            // Expired/revoked requests must not be executed or replayed.
        }
    }).catch((error) => {
        console.warn('Unable to start MCP UI bridge', error)
        controller.stop()
        return () => {}
    })
    return () => {
        stopped = true
        unlisten()
        controller.stop()
    }
}

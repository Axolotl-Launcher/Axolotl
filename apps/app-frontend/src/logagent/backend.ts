import { Channel, invoke } from '@tauri-apps/api/core'

import type {
    AnalysisEvent,
    AnalysisSource,
    ModelMessage,
    ModelStreamEvent,
    ModelTurn,
    PreparedSession,
    ToolRequest,
    UploadTicket,
} from './types'

export const prepareSession = (instanceId: string, runId: string, source: AnalysisSource) =>
    invoke<PreparedSession>('plugin:logs|logs_logagent_prepare', { instanceId, runId, source })

export const cancelSession = (runId: string) =>
    invoke<void>('plugin:logs|logs_logagent_cancel', { runId })

export async function modelTurn(
    runId: string,
    turn: ModelTurn,
    onEvent?: (event: ModelStreamEvent) => void,
) {
    const channel = new Channel<AnalysisEvent>()
    let receiving = true
    channel.onmessage = (event) => {
        if (receiving && event.run_id === runId) onEvent?.(event)
    }
    try {
        return await invoke<ModelMessage>('plugin:logs|logs_logagent_model_turn', {
            runId,
            turn,
            onEvent: channel,
        })
    } finally {
        receiving = false
    }
}

export const executeTool = (runId: string, request: ToolRequest) =>
    invoke<unknown>('plugin:logs|logs_logagent_tool', { runId, request })

export function runCloud(
    runId: string,
    ticket: UploadTicket | null,
    onEvent: (event: AnalysisEvent) => void,
) {
    const channel = new Channel<AnalysisEvent>()
    channel.onmessage = onEvent
    return invoke<{ content: string; ticket: UploadTicket | null }>(
        'plugin:logs|logs_logagent_cloud',
        { runId, ticket: ticket ? { ...ticket, success: true } : null, onEvent: channel },
    )
}

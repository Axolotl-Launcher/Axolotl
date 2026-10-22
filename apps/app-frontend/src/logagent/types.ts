export type AnalysisSource = 'custom' | 'logshare'

export interface AnalysisSettings {
    enabled: boolean
    provider_id: string
    model_id: string
    ai_source: AnalysisSource
}

export interface ToolCall {
    id: string
    name: string
    args: Record<string, unknown>
}

export interface ModelMessage {
    role: string
    content: string
    tool_calls?: ToolCall[]
    tool_call_id?: string
    opaque?: unknown
}

export interface ModelTurn {
    messages: ModelMessage[]
    tools: Record<string, unknown>[]
}

export interface PreparedSession {
    run_id: string
    provider_id: string
    model_id: string
    locale: string
    context: string
}

export interface AnalysisEvent {
    run_id: string
    event_type: string
    data: Record<string, unknown>
}

export interface UploadTicket {
    id: string
    url: string
    raw: string
    token: string
}

export interface ToolRequest {
    name: string
    file_id?: string
    start_line?: number
    end_line?: number
    query?: string
    context?: number
}

export interface AgentGateway {
    modelTurn(turn: ModelTurn, onEvent?: (event: ModelStreamEvent) => void): Promise<ModelMessage>
    tool(request: ToolRequest): Promise<unknown>
}

export interface ModelStreamEvent {
    event_type: string
    data: Record<string, unknown>
}

export interface LocalProgress {
    event_type: 'model_start' | 'model_end' | 'delta' | 'thinking' | 'tool' | 'tool_result'
    data: Record<string, unknown>
}

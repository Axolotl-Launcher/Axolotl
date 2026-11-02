/** Credential-free model traffic. Only Rust knows endpoints and credentials. */
export interface ModelImage {
    mime_type: 'image/png' | 'image/jpeg' | 'image/webp'
    data: string
}

export interface ModelCall {
    id: string
    name: string
    args: Record<string, unknown>
}

export interface ModelMessage {
    role: string
    content: string
    images?: ModelImage[]
    tool_calls?: ModelCall[]
    tool_call_id?: string
    tool_error?: boolean
    opaque?: unknown
}

export interface ModelTurn {
    messages: ModelMessage[]
    tools: Record<string, unknown>[]
}

export interface ModelStreamEvent {
    event_type: string
    data: Record<string, unknown>
}

export interface ModelGateway {
    modelTurn(
        turn: ModelTurn,
        onEvent?: (event: ModelStreamEvent) => void,
        signal?: AbortSignal,
    ): Promise<ModelMessage>
}

/** Copied from the shared Rust registry at the start of each run, never hand-maintained. */
export interface ToolDefinition {
    name: string
    description: string
    input_schema: Record<string, unknown>
    access: 'public' | 'controlled' | 'full_access'
    read_only: boolean
    idempotent: boolean
    destructive: boolean
    execution: { deferred: boolean; interrupts_mutation: boolean }
}

export interface ToolOutput {
    content: ({ type: 'text'; text: string } | { type: 'image'; data: string; mime_type: string })[]
    structured_content: unknown
    is_error: boolean
}

export interface ToolRequest {
    call_id: string
    name: string
    arguments: Record<string, unknown>
}

export interface ApprovalAction extends ToolRequest {
    description: string
}

export interface RuntimeJob {
    attempt_id: string
    thread_id: string
    provider_id: string
    model_id: string
    locale: string
    context: string
    instructions: string
    /** Authoritative history; a fresh attempt cannot execute an older checkpoint's writes. */
    messages: ModelMessage[]
    resume?: {
        call_ids: string[]
        decisions: ({ type: 'approve' } | { type: 'reject'; message: string })[]
    }
    model_turns: number
    tool_calls: number
    check_only: boolean
}

export type RuntimeEvent =
    | { type: 'text'; delta: string }
    | {
          type: 'step'
          step: { id: string; title: string; tool: string }
          status: 'running' | 'done' | 'failed'
      }
    | { type: 'progress'; steps: { title: string; detail: string; status: string }[] }

/** The Rust host enforces task identity, permission checks, budgets and write admission.
 * No worker-side decision is an approval permit. Approval is submitted by the UI only. */
export interface RuntimeGateway extends ModelGateway {
    catalog(): Promise<ToolDefinition[]>
    tool(request: ToolRequest, signal: AbortSignal): Promise<ToolOutput>
    prepareApproval(actions: ApprovalAction[]): Promise<void>
    emit(event: RuntimeEvent): Promise<void>
    checkpoint(request: CheckpointRequest): Promise<StoredCheckpoint[] | void>
}

export interface SerializedValue {
    type: string
    data: string
}
export interface StoredWrite {
    task_id: string
    index: number
    channel: string
    value: SerializedValue
}
export interface StoredCheckpoint {
    id: string
    checkpoint: SerializedValue
    metadata: SerializedValue
    parent_id?: string
    writes: StoredWrite[]
}

/** Host scopes every operation to the calling attempt. It ignores caller-supplied
 * task ownership; only namespace/checkpoint IDs are configurable by LangGraph. */
export type CheckpointRequest =
    | { operation: 'get' | 'list'; namespace: string; id?: string; before?: string; limit?: number }
    | { operation: 'put'; namespace: string; record: Omit<StoredCheckpoint, 'writes'> }
    | { operation: 'put_writes'; namespace: string; id: string; writes: StoredWrite[] }

export type RunOutcome = { status: 'awaiting' } | { status: 'completed'; message: ModelMessage }

import type {
    ApprovalAction,
    CheckpointRequest,
    ModelMessage,
    ModelTurn,
    RuntimeEvent,
    RuntimeGateway,
    RuntimeJob,
    StoredCheckpoint,
    ToolDefinition,
    ToolOutput,
    ToolRequest,
} from '../src/protocol.ts'

export const read: ToolDefinition = {
    name: 'instances_list',
    description: 'List actual instances',
    access: 'controlled',
    read_only: true,
    idempotent: true,
    destructive: false,
    input_schema: { type: 'object', properties: {}, additionalProperties: false },
    execution: { deferred: false, interrupts_mutation: false },
}
export const write: ToolDefinition = {
    ...read,
    name: 'instances_rename',
    description: 'Rename the specified instance',
    read_only: false,
    idempotent: false,
    input_schema: {
        type: 'object',
        properties: { name: { type: 'string' } },
        required: ['name'],
        additionalProperties: false,
    },
}

export function job(overrides: Partial<RuntimeJob> = {}): RuntimeJob {
    return {
        attempt_id: 'attempt',
        thread_id: 'thread',
        provider_id: 'provider',
        model_id: 'model',
        locale: 'en-US',
        context: 'Default instance: Test',
        instructions: '',
        messages: [{ role: 'user', content: 'Help with this instance' }],
        model_turns: 0,
        tool_calls: 0,
        check_only: false,
        ...overrides,
    }
}

/** Fake only the Rust transport; tests execute the installed DeepAgent graph,
 * tool middleware, interrupts and checkpoint serializer unchanged. */
export class FixtureHost implements RuntimeGateway {
    definitions = [read, write]
    turns: ModelTurn[] = []
    events: RuntimeEvent[] = []
    approvals: ApprovalAction[] = []
    calls: ToolRequest[] = []
    allowed = new Set<string>()
    records = new Map<string, StoredCheckpoint>()
    respond: (turn: ModelTurn, index: number) => ModelMessage | Promise<ModelMessage> = () => ({
        role: 'assistant',
        content: 'Done.',
    })
    output: ToolOutput = {
        content: [],
        structured_content: { instances: [{ id: 'actual' }] },
        is_error: false,
    }
    async catalog() {
        return this.definitions
    }
    async modelTurn(turn: ModelTurn, onEvent?: Parameters<RuntimeGateway['modelTurn']>[1]) {
        this.turns.push(turn)
        const result = await this.respond(turn, this.turns.length)
        if (result.content) onEvent?.({ event_type: 'delta', data: { text: result.content } })
        return result
    }
    async tool(request: ToolRequest) {
        const definition = this.definitions.find((item) => item.name === request.name)
        if (!definition?.read_only && !this.allowed.has(request.call_id))
            throw new Error('Write attempted before host approval')
        this.calls.push(request)
        return this.output
    }
    async prepareApproval(actions: ApprovalAction[]) {
        this.approvals.push(...actions)
    }
    async emit(event: RuntimeEvent) {
        this.events.push(event)
    }
    async checkpoint(request: CheckpointRequest) {
        const key = (id: string) => JSON.stringify([request.namespace, id])
        if (request.operation === 'put') {
            const existing = this.records.get(key(request.record.id))
            this.records.set(key(request.record.id), {
                ...structuredClone(request.record),
                writes: existing?.writes ?? [],
            })
        } else if (request.operation === 'put_writes') {
            const record = this.records.get(key(request.id))
            if (!record) throw new Error('Checkpoint not found')
            for (const write of request.writes) {
                const index = record.writes.findIndex(
                    (existing) =>
                        existing.task_id === write.task_id && existing.index === write.index,
                )
                if (index < 0) record.writes.push(structuredClone(write))
                else if (write.index < 0) record.writes[index] = structuredClone(write)
            }
        } else {
            const matching = [...this.records.entries()]
                .filter(([id]) => JSON.parse(id)[0] === request.namespace)
                .map(([, record]) => record)
                .filter(
                    (record) =>
                        (!request.id || record.id === request.id) &&
                        (!request.before || record.id < request.before),
                )
                .sort((a, b) => b.id.localeCompare(a.id))
            return structuredClone(
                matching.slice(0, request.operation === 'get' ? 1 : (request.limit ?? 1000)),
            )
        }
    }
}

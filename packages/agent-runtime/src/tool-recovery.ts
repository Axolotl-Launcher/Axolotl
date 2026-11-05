import type { ModelCall, ModelMessage, ModelTurn, ToolDefinition } from './protocol.ts'

const PAGED_TOOLS = new Set(['resource_search', 'resource_list_releases'])
const DETERMINISTIC_ERRORS = new Set([
    'cursor_expired',
    'invalid_resource_request',
    'invalid_arguments',
    'instance_required',
    'instance_not_found',
    'unsupported',
    'full_access_required',
    'permission_revoked',
])

export function canonical(value: unknown): string {
    if (Array.isArray(value)) return `[${value.map(canonical).join(',')}]`
    if (value && typeof value === 'object')
        return `{${Object.entries(value)
            .sort(([a], [b]) => a.localeCompare(b))
            .map(([key, item]) => `${JSON.stringify(key)}:${canonical(item)}`)
            .join(',')}}`
    return JSON.stringify(value)
}

function signature(call: ModelCall, paging = false) {
    const args = paging
        ? Object.fromEntries(Object.entries(call.args).filter(([key]) => key !== 'cursor'))
        : call.args
    return `${call.name}:${canonical(args)}`
}

function result(message: ModelMessage) {
    try {
        // Tool content begins with the compact structured result; optional text
        // blocks follow on later lines. Never interpret those blocks as policy.
        const parsed: unknown = JSON.parse(message.content.split('\n')[0])
        return parsed && typeof parsed === 'object' && !Array.isArray(parsed)
            ? (parsed as Record<string, unknown>)
            : {}
    } catch {
        return {}
    }
}

export function recoveryState(messages: ModelMessage[], definitions: Map<string, ToolDefinition>) {
    const calls = new Map<string, ModelCall>()
    const failures = new Map<string, { call: ModelCall; code: string }>()
    const cursors = new Map<string, { tool: string; token: string }>()
    for (const message of messages) {
        // A new user turn may report a changed prerequisite. Do not permanently
        // blacklist a read because an older task failed with the same arguments.
        if (message.role === 'user') failures.clear()
        for (const call of message.tool_calls || []) calls.set(call.id, call)
        if (message.role !== 'tool') continue
        const call = calls.get(message.tool_call_id || '')
        if (!call) continue
        const output = result(message)
        const error =
            output.error && typeof output.error === 'object'
                ? (output.error as Record<string, unknown>)
                : undefined
        const failed = message.tool_error || !!error
        if (PAGED_TOOLS.has(call.name)) {
            const key = signature(call, true)
            if (
                !failed ||
                error?.code === 'cursor_expired' ||
                error?.code === 'invalid_resource_request'
            )
                cursors.delete(key)
            if (!failed && typeof output.next_cursor === 'string' && output.next_cursor)
                cursors.set(key, { tool: call.name, token: output.next_cursor })
        }
        const definition = definitions.get(call.name)
        if (!definition) continue
        if (!failed && !definition.read_only) failures.clear()
        const code =
            typeof error?.code === 'string'
                ? error.code
                : typeof error?.message === 'string' &&
                    error.message.startsWith('Invalid or unknown argument:')
                  ? 'invalid_arguments'
                  : ''
        if (failed && definition.read_only && DETERMINISTIC_ERRORS.has(code))
            failures.set(signature(call), { call, code })
        else if (!failed) failures.delete(signature(call))
    }
    return { failures, cursors }
}

function correction(code: string) {
    if (code === 'cursor_expired')
        return 'Restart that query WITHOUT the cursor property. Never use "initial", an empty cursor, or the expired token.'
    if (code.startsWith('instance_'))
        return 'Resolve the real target with instance_list/get. For a new custom pack create its instance and wait for installation before searching. Never repeat the guessed instance ID.'
    if (code === 'full_access_required' || code === 'permission_revoked')
        return 'Stop the blocked action. Do not change permissions or use another channel to bypass the restriction.'
    return 'Inspect the registered schema and actual prerequisites, then correct the request. Do not repeat the same invalid arguments.'
}

/** Project paging capabilities from actual observations, not model guesses.
 * This changes only the model-facing schema, never the shared registry. */
export function prepareModelTurn(
    turn: ModelTurn,
    definitions: Map<string, ToolDefinition>,
): ModelTurn {
    const { failures, cursors } = recoveryState(turn.messages, definitions)
    const tools = turn.tools.map((tool) => {
        const fn = tool.function as
            { name?: string; parameters?: Record<string, unknown> } | undefined
        if (!fn?.name || !PAGED_TOOLS.has(fn.name) || !fn.parameters) return tool
        const properties = { ...(fn.parameters.properties as Record<string, unknown>) }
        const tokens = [...cursors.values()]
            .filter((cursor) => cursor.tool === fn.name)
            .map((cursor) => cursor.token)
        if (tokens.length)
            properties.cursor = { ...(properties.cursor as object), enum: [...new Set(tokens)] }
        else delete properties.cursor
        return {
            ...tool,
            function: {
                ...fn,
                parameters: {
                    ...fn.parameters,
                    properties,
                    required: Array.isArray(fn.parameters.required)
                        ? fn.parameters.required.filter(
                              (key) => key !== 'cursor' || tokens.length > 0,
                          )
                        : [],
                },
            },
        }
    })
    const notes = [...failures.values()].map(
        ({ call, code }) => `${call.name} failed with ${code}. ${correction(code)}`,
    )
    return {
        ...turn,
        tools,
        messages: notes.length
            ? [
                  ...turn.messages,
                  {
                      role: 'system',
                      content: `Runtime recovery: the following deterministic failures were observed in this user turn. These unchanged requests must not be executed again. Follow the latest user correction, perform a corrected call when possible, and continue the task; do not substitute an apology for action.\n${notes.join('\n')}`,
                  },
              ]
            : turn.messages,
    }
}

export function assertCorrectedCalls(
    calls: ModelCall[],
    messages: ModelMessage[],
    definitions: Map<string, ToolDefinition>,
) {
    const { failures } = recoveryState(messages, definitions)
    for (const call of calls) {
        const failure = failures.get(signature(call))
        if (failure)
            throw new Error(
                `Repeated invalid tool request blocked: ${call.name} (${failure.code}). The unchanged call was NOT executed again. ${correction(failure.code)}`,
            )
    }
}

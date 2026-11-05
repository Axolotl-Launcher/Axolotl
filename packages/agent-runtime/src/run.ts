import { AIMessage, type BaseMessage, ToolMessage } from '@langchain/core/messages'
import { tool } from '@langchain/core/tools'
import { Command } from '@langchain/langgraph'
import {
    createDeepAgent,
    createFilesystemMiddleware,
    registerHarnessProfile,
} from 'deepagents/browser'
import { createMiddleware, todoListMiddleware } from 'langchain'

import { HostCheckpointer } from './checkpointer.ts'
import { nodeInterrupt } from './interrupt.ts'
import { deserializeMessage, LauncherChatModel, serializeMessage } from './model.ts'
import { AGENT_PROMPT } from './prompt.ts'
import type {
    ApprovalAction,
    ApprovalDecision,
    ModelMessage,
    RunOutcome,
    RuntimeGateway,
    RuntimeJob,
    ToolDefinition,
    ToolOutput,
} from './protocol.ts'
import { assertCorrectedCalls, canonical, prepareModelTurn } from './tool-recovery.ts'

export const MODEL_LIMIT = 64
export const TOOL_LIMIT = 256
export const EXCLUDED_TOOLS = [
    'ls',
    'read_file',
    'write_file',
    'edit_file',
    'delete',
    'delete_file',
    'glob',
    'grep',
    'execute',
    'task',
]

registerHarnessProfile('axolotl-agent', {
    excludedTools: EXCLUDED_TOOLS,
    excludedMiddleware: ['HumanInTheLoopMiddleware', 'SummarizationMiddleware'],
    generalPurposeSubagent: { enabled: false },
})

function content(output: ToolOutput) {
    const blocks: (
        { type: 'text'; text: string } | { type: 'image_url'; image_url: { url: string } }
    )[] = [{ type: 'text', text: JSON.stringify(output.structured_content) }]
    for (const block of output.content) {
        if (block.type === 'image')
            blocks.push({
                type: 'image_url',
                image_url: { url: `data:${block.mime_type};base64,${block.data}` },
            })
        else if (block.text !== JSON.stringify(output.structured_content))
            blocks.push({ type: 'text', text: block.text })
    }
    return blocks
}

function approvalActions(
    interrupts: { value: unknown }[],
    messages: BaseMessage[],
    definitions: Map<string, ToolDefinition>,
): ApprovalAction[] {
    const assistant = messages
        .slice()
        .reverse()
        .find((message) => AIMessage.isInstance(message) && message.tool_calls?.length)
    if (!assistant || !AIMessage.isInstance(assistant))
        throw new Error('Approval has no matching tool calls')
    const calls = [...(assistant.tool_calls ?? [])]
    const actions: ApprovalAction[] = []
    for (const interrupt of interrupts) {
        const value = interrupt.value as {
            actionRequests?: { name: string; args: Record<string, unknown>; description?: string }[]
        }
        if (!Array.isArray(value?.actionRequests)) throw new Error('Invalid approval interrupt')
        for (const action of value.actionRequests) {
            const index = calls.findIndex(
                (call) =>
                    call.name === action.name && canonical(call.args) === canonical(action.args),
            )
            if (index < 0) throw new Error('Approval does not match a registered tool call')
            const call = calls.splice(index, 1)[0]
            if (!call?.id || !definitions.has(action.name))
                throw new Error('Approval does not match a registered tool call')
            actions.push({
                call_id: call.id,
                name: action.name,
                arguments: call.args,
                description: definitions.get(action.name)!.description,
            })
        }
    }
    if (!actions.length) throw new Error('Approval contains no actions')
    return actions
}

/** One invocation or HITL continuation. The Rust-owned worker lifecycle schedules
 * jobs; closing the visible window never aborts this function. */
export async function runAgent(
    job: RuntimeJob,
    gateway: RuntimeGateway,
    signal: AbortSignal,
): Promise<RunOutcome> {
    signal.throwIfAborted()
    if (
        job.resume &&
        (!job.resume.call_ids.length ||
            job.resume.call_ids.length !== job.resume.decisions.length ||
            new Set(job.resume.call_ids).size !== job.resume.call_ids.length)
    )
        throw new Error('Approval decisions must identify every reviewed action exactly once')
    const catalog = (await gateway.catalog()).filter(
        (definition) => !job.check_only || definition.read_only,
    )
    const definitions = new Map(catalog.map((definition) => [definition.name, definition]))
    if (
        definitions.size !== catalog.length ||
        catalog.some(
            (definition) =>
                EXCLUDED_TOOLS.includes(definition.name) || definition.name === 'write_todos',
        )
    )
        throw new Error('Invalid launcher tool catalog')
    let modelTurns = job.model_turns
    let toolCalls = job.tool_calls
    let events = Promise.resolve()
    let streamed = false
    let checklist = '[]'
    let latestInput: ModelMessage[] = []
    const pendingOperations = new Map<string, { id: string; title: string; tool: string }>()
    const emitModel = (event: { event_type: string; data: Record<string, unknown> }) => {
        if (signal.aborted || event.event_type !== 'delta' || typeof event.data.text !== 'string')
            return
        streamed = true
        const delta = event.data.text
        events = events.then(() => gateway.emit({ type: 'text', delta }))
        void events.catch(() => {})
    }
    const model = new LauncherChatModel(
        {
            modelTurn: async (turn, onEvent, modelSignal) => {
                if (++modelTurns > MODEL_LIMIT)
                    throw new Error(
                        'Model turn limit reached. Continue the task explicitly to proceed.',
                    )
                signal.throwIfAborted()
                streamed = false
                latestInput = turn.messages
                const response = await gateway.modelTurn(
                    prepareModelTurn(turn, definitions),
                    onEvent,
                    modelSignal ?? signal,
                )
                if (!streamed && response.content)
                    events = events.then(() =>
                        gateway.emit({ type: 'text', delta: response.content }),
                    )
                await events
                return response
            },
        },
        { onEvent: emitModel },
    )
    const execute = async (
        definition: ToolDefinition,
        args: Record<string, unknown>,
        callId: string,
    ) => {
        signal.throwIfAborted()
        if (++toolCalls > TOOL_LIMIT)
            throw new Error('Tool call limit reached. Continue the task explicitly to proceed.')
        if (!callId) throw new Error('Tool call has no stable identifier')
        const step = { id: callId, title: definition.name, tool: definition.name }
        await gateway.emit({ type: 'step', step, status: 'running' })
        let output: ToolOutput
        try {
            output = await gateway.tool(
                { call_id: callId, name: definition.name, arguments: args },
                signal,
            )
        } catch (error) {
            signal.throwIfAborted()
            // Validation/permission failures belong to this call. Let the model
            // correct the arguments instead of aborting the entire graph.
            const message = error instanceof Error ? error.message : String(error)
            output = {
                is_error: true,
                structured_content: { error: { message } },
                content: [{ type: 'text', text: message }],
            }
        }
        signal.throwIfAborted()
        const result = output.structured_content as {
            operation_id?: string
            status?: string
        } | null
        const deferred =
            typeof result?.operation_id === 'string' &&
            ['queued', 'running'].includes(result.status ?? '')
        const failed = output.is_error || result?.status === 'failed'
        if (deferred) pendingOperations.set(result!.operation_id!, step)
        else await gateway.emit({ type: 'step', step, status: failed ? 'failed' : 'done' })
        if (result?.operation_id && ['succeeded', 'failed'].includes(result.status ?? '')) {
            const original = pendingOperations.get(result.operation_id)
            if (original)
                await gateway.emit({
                    type: 'step',
                    step: original,
                    status: failed ? 'failed' : 'done',
                })
            pendingOperations.delete(result.operation_id)
        }
        return new ToolMessage({
            tool_call_id: callId,
            name: definition.name,
            status: failed ? 'error' : 'success',
            content: content(output),
            artifact: output.structured_content,
        })
    }
    const tools = catalog.map((definition) =>
        tool(
            async (args: Record<string, unknown>, config) =>
                execute(definition, args, config.toolCall?.id ?? ''),
            {
                name: definition.name,
                description: definition.description,
                schema: definition.input_schema,
            },
        ),
    )

    const checklistMiddleware = todoListMiddleware()
    const guard = createMiddleware({
        name: 'LauncherToolsOnly',
        stateSchema: checklistMiddleware.stateSchema,
        beforeModel: async (state) => {
            const todos = (
                state as typeof state & { todos?: { content: string; status: string }[] }
            ).todos
            if (!Array.isArray(todos)) return
            const signature = JSON.stringify(todos)
            if (signature === checklist) return
            checklist = signature
            await gateway.emit({
                type: 'progress',
                steps: todos.map((todo) => ({
                    title: todo.content,
                    detail: '',
                    status:
                        todo.status === 'completed'
                            ? 'done'
                            : todo.status === 'in_progress'
                              ? 'running'
                              : 'pending',
                })),
            })
        },
        afterModel: async (state) => {
            signal.throwIfAborted()
            const last = state.messages.at(-1)
            if (last && AIMessage.isInstance(last)) {
                assertCorrectedCalls(
                    (last.tool_calls || []).map((call) => ({
                        id: call.id || '',
                        name: call.name,
                        args: call.args,
                    })),
                    latestInput,
                    definitions,
                )
                toolCalls +=
                    last.tool_calls?.filter((call) => call.name === 'write_todos').length ?? 0
                if (toolCalls > TOOL_LIMIT)
                    throw new Error(
                        'Tool call limit reached. Continue the task explicitly to proceed.',
                    )
                const unavailable = last.tool_calls?.filter(
                    (call) => !definitions.has(call.name) && call.name !== 'write_todos',
                )
                if (unavailable?.length)
                    throw new Error(
                        `Model requested unavailable tools: ${unavailable.map((call) => call.name).join(', ')}`,
                    )
            }
        },
    })
    const interruptOn = Object.fromEntries(
        catalog
            .filter((definition) => definition.requires_approval ?? !definition.read_only)
            .map((definition) => [
                definition.name,
                { allowedDecisions: ['approve', 'reject'] as ('approve' | 'reject')[] },
            ]),
    )
    const approval = createMiddleware({
        name: 'LauncherWriteApproval',
        afterModel: {
            canJumpTo: ['model'],
            hook: async (state, runtime) => {
                const last = state.messages.at(-1)
                if (!last || !AIMessage.isInstance(last)) return
                const originalCalls = [...(last.tool_calls ?? [])]
                const reviewed = originalCalls.filter((call) => interruptOn[call.name])
                if (!reviewed.length) return
                // Use DeepAgent's interruptOn selection with a node-bound LangGraph
                // interrupt. The pinned stock HITL hook loses browser async context.
                const response = nodeInterrupt(
                    {
                        actionRequests: reviewed.map((call) => ({
                            name: call.name,
                            args: call.args,
                        })),
                        reviewConfigs: reviewed.map((call) => ({
                            actionName: call.name,
                            allowedDecisions: ['approve', 'reject'],
                        })),
                    },
                    runtime.configurable,
                ) as { decisions?: ApprovalDecision[] }
                const decisions = response?.decisions
                if (
                    !Array.isArray(decisions) ||
                    decisions.length !== reviewed.length ||
                    decisions.some((decision) => !['approve', 'reject'].includes(decision.type))
                )
                    throw new Error('Approval decisions do not match the pending actions')
                if (
                    !job.resume ||
                    reviewed.some((call, index) => call.id !== job.resume!.call_ids[index])
                )
                    throw new Error('Approval identities do not match the pending actions')
                if (decisions.every((decision) => decision.type === 'approve')) return
                const selected = new Map(reviewed.map((call, index) => [call.id, decisions[index]]))
                const results: ToolMessage[] = []
                for (const call of originalCalls) {
                    const decision = selected.get(call.id)
                    if (decision?.type === 'reject') {
                        results.push(
                            new ToolMessage({
                                tool_call_id: call.id ?? '',
                                name: call.name,
                                content:
                                    decision.message ||
                                    'User rejected this action. It was not executed. Do not retry it.',
                                status: 'error',
                            }),
                        )
                        continue
                    }
                    const definition = definitions.get(call.name)
                    if (
                        definition &&
                        (definition.read_only ||
                            definition.requires_approval === false ||
                            decision?.type === 'approve')
                    )
                        results.push(await execute(definition, call.args, call.id ?? ''))
                    else if (call.name === 'write_todos')
                        results.push(
                            new ToolMessage({
                                tool_call_id: call.id ?? '',
                                content:
                                    'Checklist update was skipped during partial approval; update it in a separate turn.',
                                status: 'error',
                            }),
                        )
                    else throw new Error('Approval does not cover the pending tool call')
                }
                return { messages: [last, ...results], jumpTo: 'model' as const }
            },
        },
    })
    const agent = createDeepAgent({
        model,
        tools,
        checkpointer: new HostCheckpointer(job.attempt_id, gateway),
        systemPrompt: `${AGENT_PROMPT}\nUser language: ${job.locale}\nDefault context:\n${job.context}\nStanding user preferences:\n${job.instructions}${job.check_only ? '\nThis run is READONLY verification. Do not propose or perform writes.' : ''}`,
        subagents: [],
        middleware: [
            createFilesystemMiddleware({
                toolTokenLimitBeforeEvict: null,
                humanMessageTokenLimitBeforeEvict: null,
            }),
            checklistMiddleware,
            guard,
            approval,
        ],
        interruptOn,
    })
    // New attempts get new graph threads; only explicit UI decisions resume interrupts.
    const config = {
        configurable: { thread_id: job.attempt_id },
        signal,
        recursionLimit: 1024,
        callbacks: [],
    }
    const input = job.resume
        ? new Command({ resume: { decisions: job.resume.decisions } })
        : { messages: job.messages.map(deserializeMessage) }
    const result = await agent.invoke(input, config)
    await events
    signal.throwIfAborted()
    const interrupts = (result as { __interrupt__?: { value: unknown }[] }).__interrupt__
    if (interrupts?.length) {
        await gateway.prepareApproval(approvalActions(interrupts, result.messages, definitions))
        return { status: 'awaiting' }
    }
    const last = result.messages.at(-1)
    if (pendingOperations.size)
        throw new Error(
            `Operation results are still unconfirmed: ${[...pendingOperations.keys()].join(', ')}`,
        )
    if (!last || !AIMessage.isInstance(last) || last.tool_calls?.length || !last.text.trim())
        throw new Error('Agent finished without a verified final response')
    return { status: 'completed', message: serializeMessage(last) }
}

import { AIMessage, ToolMessage } from '@langchain/core/messages'
import { expect, it, vi } from 'vitest'

import { diagnosticText, runLocalAgent } from './agent.ts'
import { LogAgentModel } from './model.ts'
import type { AgentGateway, ModelTurn } from './types.ts'

const session = {
    run_id: 'test',
    provider_id: 'test',
    model_id: 'test',
    locale: 'zh-CN',
    context: 'A crash occurred.',
}

it('executes three grep calls from the first model turn before requesting a diagnosis', async () => {
    let turns = 0
    const queries: string[] = []
    const output = await runLocalAgent(
        session,
        {
            modelTurn: async (turn) => {
                if (++turns === 1)
                    return {
                        role: 'assistant',
                        content: '',
                        tool_calls: ['error', 'exception', 'caused by'].map((query, index) => ({
                            id: `grep-${index}`,
                            name: 'grep_log_file',
                            args: { file_id: 'log-1', query },
                        })),
                    }
                expect(turn.messages.filter((message) => message.role === 'tool').length).toBe(3)
                return { role: 'assistant', content: 'Diagnosis after three searches.' }
            },
            tool: async (request) => {
                queries.push(request.query!)
                return 'evidence'
            },
        },
        new AbortController().signal,
        () => {},
    )
    expect(queries.sort()).toEqual(['caused by', 'error', 'exception'])
    expect(turns).toBe(2)
    expect(output).toMatch(/Diagnosis/)
})

it('forwards live text and factual tool progress while keeping private protocol data hidden', async () => {
    const events: import('./types.ts').LocalProgress[] = []
    let turns = 0
    const output = await runLocalAgent(
        session,
        {
            modelTurn: async (_turn, receive) => {
                if (++turns === 1) {
                    receive?.({ event_type: 'thinking', data: { content: '' } })
                    receive?.({
                        event_type: 'delta',
                        data: { content: 'Checking the crash context.' },
                    })
                    expect(events.at(-1)?.data.content).toBe('Checking the crash context.')
                    return {
                        role: 'assistant',
                        content: 'Checking the crash context.',
                        opaque: { reasoning: 'private' },
                        tool_calls: [
                            {
                                id: 'read',
                                name: 'read_log_file',
                                args: { file_id: 'log-1', start_line: 20 },
                            },
                        ],
                    }
                }
                receive?.({ event_type: 'delta', data: { content: 'Final diagnosis.' } })
                return { role: 'assistant', content: 'Final diagnosis.' }
            },
            tool: async () => ({ name: 'latest.log', lines: [{ line: 20, text: 'evidence' }] }),
        },
        new AbortController().signal,
        () => {},
        (event) => events.push(event),
    )
    expect(output).toBe('Final diagnosis.')
    expect(events.map((event) => event.event_type)).toEqual([
        'model_start',
        'thinking',
        'delta',
        'model_end',
        'tool',
        'tool_result',
        'model_start',
        'delta',
        'model_end',
    ])
    expect(events.find((event) => event.event_type === 'tool_result')?.data.lines).toBe(1)
    expect(JSON.stringify(events)).not.toMatch(/private|evidence/)
})

it('repeated provider tool-call IDs still execute every investigation round', async () => {
    let turns = 0
    const reads: string[] = []
    const output = await runLocalAgent(
        session,
        {
            modelTurn: async () => {
                turns++
                if (turns <= 2)
                    return {
                        role: 'assistant',
                        content: '',
                        tool_calls: [
                            {
                                id: 'call_0',
                                name: 'read_log_file',
                                args: { file_id: `log-${turns}` },
                            },
                        ],
                    }
                return { role: 'assistant', content: 'Final diagnosis after two reads.' }
            },
            tool: async (request) => {
                reads.push(request.file_id!)
                return 'evidence'
            },
        },
        new AbortController().signal,
        () => {},
    )
    expect(reads).toEqual(['log-1', 'log-2'])
    expect(turns).toBe(3)
    expect(output).toMatch(/Final diagnosis/)
})

it('an unsupported extraction tool is returned to the model as an error instead of ending analysis', async () => {
    let turns = 0
    const output = await runLocalAgent(
        session,
        {
            modelTurn: async (turn) => {
                if (++turns === 1)
                    return {
                        role: 'assistant',
                        content: '',
                        tool_calls: [{ id: 'extract-1', name: 'extract-diagnosis', args: {} }],
                    }
                const error = turn.messages.find((message) => message.role === 'tool')
                expect(error?.content ?? '').toMatch(/not available/)
                return { role: 'assistant', content: 'Final diagnosis.' }
            },
            tool: async () => {
                throw new Error('Unsupported tools must not reach Rust')
            },
        },
        new AbortController().signal,
        () => {},
    )
    expect(output).toBe('Final diagnosis.')
    expect(turns).toBe(2)
})

it('repeated unavailable tools stop with the tool name and do not execute', async () => {
    let turns = 0
    await expect(
        runLocalAgent(
            session,
            {
                modelTurn: async () => ({
                    role: 'assistant',
                    content: '',
                    tool_calls: [{ id: `call-${++turns}`, name: 'extract-diagnosis', args: {} }],
                }),
                tool: async () => {
                    throw new Error('Must not execute')
                },
            },
            new AbortController().signal,
            () => {},
        ),
    ).rejects.toThrow(/repeatedly requested unavailable tools: extract-diagnosis/)
    expect(turns).toBe(3)
})

it('the full 16-turn budget supports multiple investigation rounds before finalizing', async () => {
    let turns = 0
    const output = await runLocalAgent(
        session,
        {
            modelTurn: async () =>
                ++turns < 16
                    ? {
                          role: 'assistant',
                          content: '',
                          tool_calls: [
                              {
                                  id: `call-${turns}`,
                                  name: 'read_log_file',
                                  args: { file_id: 'log-1', start_line: turns * 20 },
                              },
                          ],
                      }
                    : { role: 'assistant', content: 'Diagnosis after fifteen tool rounds.' },
            tool: async () => 'evidence',
        },
        new AbortController().signal,
        () => {},
    )
    expect(turns).toBe(16)
    expect(output).toMatch(/Diagnosis/)
})

it('Deep Agents uses only diagnostic tools and preserves tool IDs and opaque protocol state', async () => {
    const turns: ModelTurn[] = []
    const used: string[] = []
    const gateway: AgentGateway = {
        async modelTurn(turn) {
            turns.push(turn)
            expect(
                turn.tools.map((tool) => (tool.function as { name: string }).name).sort(),
            ).toEqual(['grep_log_file', 'list_log_files', 'read_log_file'])
            if (turns.length === 1)
                return {
                    role: 'assistant',
                    content: '',
                    tool_calls: [
                        {
                            id: 'call-123',
                            name: 'read_log_file',
                            args: { file_id: 'log-1', start_line: 200 },
                        },
                    ],
                    opaque: { signature: 'provider-required' },
                }
            expect(turn.messages.find((message) => message.role === 'tool')?.tool_call_id).toBe(
                'call-123',
            )
            expect(turn.messages.find((message) => message.role === 'assistant')?.opaque).toEqual({
                signature: 'provider-required',
            })
            return {
                role: 'assistant',
                content: 'The log identifies an incompatible Java version.',
            }
        },
        async tool(request) {
            expect(request.file_id).toBe('log-1')
            return { line: 200, text: 'UnsupportedClassVersionError' }
        },
    }
    const output = await runLocalAgent(session, gateway, new AbortController().signal, (tool) =>
        used.push(tool),
    )
    expect(turns.length).toBe(2)
    expect(used).toEqual(['read_log_file'])
    expect(output).toMatch(/Java/)
})

it('an aborted run does not call the model', async () => {
    const controller = new AbortController()
    controller.abort()
    const gateway: AgentGateway = {
        modelTurn: async () => {
            throw new Error('must not run')
        },
        tool: async () => {
            throw new Error('must not run')
        },
    }
    await expect(runLocalAgent(session, gateway, controller.signal, () => {})).rejects.toThrow()
})

it('accepts standard content blocks without displaying reasoning blocks', () => {
    const message = new AIMessage({
        contentBlocks: [
            { type: 'reasoning', reasoning: 'Internal reasoning' },
            { type: 'text', text: 'Java is incompatible.\n' },
            { type: 'text', text: 'Select a supported Java version.' },
        ],
    })
    expect(diagnosticText(message)).toBe('Java is incompatible.\nSelect a supported Java version.')
})

it('the complete agent loop accepts a final response normalized into content blocks', async () => {
    const generate = LogAgentModel.prototype._generate
    vi.spyOn(
        LogAgentModel.prototype,
        '_generate',
        async function (this: LogAgentModel, ...args: Parameters<typeof generate>) {
            const result = await generate.apply(this, args)
            for (const generation of result.generations) {
                generation.message = new AIMessage({
                    ...generation.message,
                    contentBlocks: generation.message.contentBlocks,
                })
            }
            return result
        },
    )
    const output = await runLocalAgent(
        session,
        {
            modelTurn: async () => ({
                role: 'assistant',
                content: '诊断结果：Java 版本不兼容。',
                tool_calls: [],
            }),
            tool: async () => {
                throw new Error('No tool needed for this response')
            },
        },
        new AbortController().signal,
        () => {},
    )
    expect(output).toBe('诊断结果：Java 版本不兼容。')
})

it('does not treat tool output or unfinished tool calls as a final diagnosis', () => {
    expect(() =>
        diagnosticText(new ToolMessage({ content: 'log evidence', tool_call_id: 'call-1' })),
    ).toThrow(/without a final assistant reply/)
    expect(() =>
        diagnosticText(
            new AIMessage({
                content: 'Checking logs',
                tool_calls: [{ id: 'call-1', name: 'list_log_files', args: {} }],
            }),
        ),
    ).toThrow(/unresolved tool calls/)
    expect(() =>
        diagnosticText(
            new AIMessage({
                contentBlocks: [{ type: 'reasoning', reasoning: 'Internal reasoning' }],
            }),
        ),
    ).toThrow(/content: reasoning/)
})

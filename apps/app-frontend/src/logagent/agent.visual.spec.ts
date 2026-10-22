import { createHash } from 'node:crypto'

import { AIMessage } from '@langchain/core/messages'
import { expect, it, vi } from 'vitest'

import { diagnosticText, runLocalAgent } from './agent'

it('runs a complete diagnostic tool loop inside Chromium without Node APIs', async () => {
    const network = vi
        .spyOn(globalThis, 'fetch')
        .mockRejectedValue(new Error('Unexpected browser network request'))
    let turns = 0
    const output = await runLocalAgent(
        {
            run_id: 'browser-test',
            provider_id: 'mock',
            model_id: 'mock',
            locale: 'en-US',
            context: 'Crash log snapshot',
        },
        {
            modelTurn: async (turn) => {
                expect(
                    turn.tools.map((tool) => (tool.function as { name: string }).name).sort(),
                ).toEqual(['grep_log_file', 'list_log_files', 'read_log_file'])
                return ++turns === 1
                    ? {
                          role: 'assistant',
                          content: '',
                          tool_calls: [{ id: 'call-browser', name: 'list_log_files', args: {} }],
                      }
                    : {
                          role: 'assistant',
                          content: 'A diagnostic result from the browser runtime.',
                      }
            },
            tool: async () => [{ id: 'log-1', name: 'latest.log', first_line: 1 }],
        },
        new AbortController().signal,
        () => {},
    )
    expect(turns).toBe(2)
    expect(output).toContain('diagnostic result')
    expect(network).not.toHaveBeenCalled()
    network.mockRestore()
})

it('supports the hashing dependency used by Deep Agents context offloading', () => {
    expect(createHash('sha256').update('abc').digest('hex')).toBe(
        'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad',
    )
})

it('reads standard assistant text blocks in the browser', () => {
    const message = new AIMessage({ contentBlocks: [{ type: 'text', text: '诊断结果' }] })
    expect(diagnosticText(message)).toBe('诊断结果')
})

it('executes three grep calls from one model response and returns their results to the next turn', async () => {
    let turns = 0
    const queries: string[] = []
    const output = await runLocalAgent(
        {
            run_id: 'browser-test',
            provider_id: 'mock',
            model_id: 'mock',
            locale: 'en-US',
            context: 'Snapshot',
        },
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
                const results = turn.messages.filter((message) => message.role === 'tool')
                expect(results.map((message) => message.tool_call_id).sort()).toEqual([
                    'grep-0',
                    'grep-1',
                    'grep-2',
                ])
                expect(results.every((message) => message.content.includes('evidence'))).toBe(true)
                return { role: 'assistant', content: 'Diagnosis after all three searches.' }
            },
            tool: async (request) => {
                queries.push(request.query!)
                return { evidence: request.query }
            },
        },
        new AbortController().signal,
        () => {},
    )
    expect(queries.sort()).toEqual(['caused by', 'error', 'exception'])
    expect(turns).toBe(2)
    expect(output).toContain('Diagnosis')
})

it('recovers when the model asks for a reserved extraction tool instead of giving a diagnosis', async () => {
    let turns = 0
    const tool = vi.fn()
    const output = await runLocalAgent(
        {
            run_id: 'browser-test',
            provider_id: 'mock',
            model_id: 'mock',
            locale: 'en-US',
            context: 'Snapshot',
        },
        {
            modelTurn: async (turn) => {
                if (++turns === 1)
                    return {
                        role: 'assistant',
                        content: '',
                        tool_calls: [{ id: 'call-extract', name: 'extract-diagnosis', args: {} }],
                    }
                expect(turn.messages.find((message) => message.role === 'tool')?.content).toContain(
                    'not available',
                )
                return { role: 'assistant', content: 'Final diagnostic reply.' }
            },
            tool,
        },
        new AbortController().signal,
        () => {},
    )
    expect(output).toBe('Final diagnostic reply.')
    expect(tool).not.toHaveBeenCalled()
})

import { describe, expect, it } from 'vitest'

import type { ModelMessage } from '../src/protocol.ts'
import { runAgent } from '../src/run.ts'
import { assertCorrectedCalls, prepareModelTurn } from '../src/tool-recovery.ts'
import { FixtureHost, job, read, write } from './fixtures.ts'

const search = {
    ...read,
    name: 'resource_search',
    input_schema: {
        type: 'object',
        properties: { query: { type: 'string' }, cursor: { type: 'string' } },
        required: ['query'],
        additionalProperties: false,
    },
}

describe('tool recovery in the actual DeepAgent loop', () => {
    it('offers no cursor initially, then only a returned cursor, then removes exhausted paging', async () => {
        const host = new FixtureHost()
        host.definitions = [search]
        host.respond = (turn, index) => {
            const schema = (
                turn.tools.find(
                    (tool) => (tool.function as { name: string }).name === 'resource_search',
                )!.function as { parameters: { properties: { cursor?: { enum: string[] } } } }
            ).parameters
            if (index === 1) {
                expect(schema.properties.cursor).toBeUndefined()
                host.output = {
                    content: [],
                    structured_content: { items: ['first'], next_cursor: 'issued-token' },
                    is_error: false,
                }
                return {
                    role: 'assistant',
                    content: '',
                    tool_calls: [
                        { id: 'first', name: 'resource_search', args: { query: 'Create' } },
                    ],
                }
            }
            if (index === 2) {
                expect(schema.properties.cursor?.enum).toEqual(['issued-token'])
                host.output = {
                    content: [],
                    structured_content: { items: ['second'], next_cursor: null },
                    is_error: false,
                }
                return {
                    role: 'assistant',
                    content: '',
                    tool_calls: [
                        {
                            id: 'next',
                            name: 'resource_search',
                            args: { query: 'Create', cursor: 'issued-token' },
                        },
                    ],
                }
            }
            expect(schema.properties.cursor).toBeUndefined()
            return { role: 'assistant', content: 'Both pages read.' }
        }
        await runAgent(job(), host, new AbortController().signal)
        expect(host.calls.map((call) => call.arguments)).toEqual([
            { query: 'Create' },
            { query: 'Create', cursor: 'issued-token' },
        ])
        expect(search.input_schema.properties.cursor).toEqual({ type: 'string' })
    })

    it('injects a concrete correction and stops a stubborn model before a duplicate backend call', async () => {
        const host = new FixtureHost()
        host.definitions = [search]
        host.output = {
            content: [],
            structured_content: {
                error: { code: 'cursor_expired', message: 'Restart without cursor' },
            },
            is_error: true,
        }
        host.respond = (turn, index) => {
            if (index > 1)
                expect(turn.messages.at(-1)?.content).toContain('WITHOUT the cursor property')
            return {
                role: 'assistant',
                content: '',
                tool_calls: [
                    {
                        id: `call-${index}`,
                        name: 'resource_search',
                        args: { query: 'Create', cursor: 'initial' },
                    },
                ],
            }
        }
        await expect(runAgent(job(), host, new AbortController().signal)).rejects.toThrow(
            'Repeated invalid tool request blocked',
        )
        expect(host.calls).toHaveLength(1)
    })

    it('retains a new user correction and permits reads after an actual state change', () => {
        const definitions = new Map(
            [search, write].map((definition) => [definition.name, definition]),
        )
        const call = {
            id: 'failed',
            name: 'resource_search',
            args: { query: 'Create', cursor: 'initial' },
        }
        const messages: ModelMessage[] = [
            { role: 'user', content: 'Find Create' },
            { role: 'assistant', content: '', tool_calls: [call] },
            {
                role: 'tool',
                tool_call_id: call.id,
                tool_error: true,
                content: '{"error":{"code":"cursor_expired"}}',
            },
            { role: 'user', content: 'Do not pass cursor. Retry.' },
        ]
        const prepared = prepareModelTurn({ messages, tools: [] }, definitions)
        expect(prepared.messages.at(-1)?.content).toBe('Do not pass cursor. Retry.')
        expect(() => assertCorrectedCalls([call], messages, definitions)).not.toThrow()
        messages.pop()
        expect(() => assertCorrectedCalls([call], messages, definitions)).toThrow(
            'Repeated invalid',
        )
        messages.push(
            {
                role: 'assistant',
                content: '',
                tool_calls: [{ id: 'write', name: write.name, args: { name: 'Fix' } }],
            },
            { role: 'tool', tool_call_id: 'write', content: '{}' },
        )
        expect(() => assertCorrectedCalls([call], messages, definitions)).not.toThrow()
    })
})

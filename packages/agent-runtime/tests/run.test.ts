import { describe, expect, it } from 'vitest'

import { runAgent } from '../src/run.ts'
import { FixtureHost, job } from './fixtures.ts'

const signal = () => new AbortController().signal

describe('real DeepAgent graph over the host gateway', () => {
    it('streams text and exposes only registered tools plus internal todos', async () => {
        const host = new FixtureHost()
        const outcome = await runAgent(job(), host, signal())
        expect(outcome).toMatchObject({ status: 'completed', message: { content: 'Done.' } })
        expect(host.events).toEqual([{ type: 'text', delta: 'Done.' }])
        expect(
            host.turns[0].tools.map((item) => (item.function as { name: string }).name).sort(),
        ).toEqual(['instances_list', 'instances_rename', 'write_todos'])
        expect(host.records.size).toBeGreaterThan(0)
    })

    it('carries real tool results, IDs and provider state into the next turn', async () => {
        const host = new FixtureHost()
        host.respond = (_, index) =>
            index === 1
                ? {
                      role: 'assistant',
                      content: '',
                      tool_calls: [{ id: 'read-1', name: 'instances_list', args: {} }],
                      opaque: { signed: 'provider-state' },
                  }
                : { role: 'assistant', content: 'The actual instance is available.' }
        await runAgent(job(), host, signal())
        expect(host.calls).toEqual([{ call_id: 'read-1', name: 'instances_list', arguments: {} }])
        expect(host.turns[1].messages.find((message) => message.role === 'tool')).toMatchObject({
            tool_call_id: 'read-1',
            content: '{"instances":[{"id":"actual"}]}',
        })
        expect(
            host.turns[1].messages.find((message) => message.role === 'assistant')?.opaque,
        ).toEqual({ signed: 'provider-state' })
    })

    it('pauses all writes, survives runtime recreation, and respects partial rejection', async () => {
        const host = new FixtureHost()
        host.respond = () => ({
            role: 'assistant',
            content: '',
            tool_calls: [
                { id: 'write-1', name: 'instances_rename', args: { name: 'One' } },
                { id: 'write-2', name: 'instances_rename', args: { name: 'Two' } },
            ],
        })
        expect(await runAgent(job(), host, signal())).toEqual({ status: 'awaiting' })
        expect(host.calls).toHaveLength(0)
        expect(host.approvals.map((action) => action.call_id)).toEqual(['write-1', 'write-2'])
        host.allowed.add('write-1')
        host.respond = () => ({ role: 'assistant', content: 'One renamed. Two was not changed.' })
        const resumed = await runAgent(
            job({
                resume: {
                    call_ids: ['write-1', 'write-2'],
                    decisions: [
                        { type: 'approve' },
                        { type: 'reject', message: 'User excluded Two; do not retry this change.' },
                    ],
                },
                model_turns: 1,
            }),
            host,
            signal(),
        )
        expect(resumed.status).toBe('completed')
        expect(host.calls.map((call) => call.call_id)).toEqual(['write-1'])
        expect(
            host.turns[1].messages.some(
                (message) =>
                    message.role === 'tool' && message.content.includes('User excluded Two'),
            ),
        ).toBe(true)
    })

    it('uses independent graph threads for new attempts, without replaying old writes', async () => {
        const host = new FixtureHost()
        host.respond = () => ({
            role: 'assistant',
            content: '',
            tool_calls: [{ id: 'old-write', name: 'instances_rename', args: { name: 'Old' } }],
        })
        await runAgent(job(), host, signal())
        // Rust binds checkpoint transport to the new attempt; records are separate.
        const next = new FixtureHost()
        await runAgent(job({ attempt_id: 'manual-continuation' }), next, signal())
        expect(host.calls).toEqual([])
        expect(next.calls).toEqual([])
    })

    it('executes an approved batch through the SDK tool boundary exactly once', async () => {
        const host = new FixtureHost()
        host.respond = () => ({
            role: 'assistant',
            content: '',
            tool_calls: [{ id: 'approved', name: 'instances_rename', args: { name: 'Approved' } }],
        })
        await runAgent(job(), host, signal())
        host.allowed.add('approved')
        host.respond = () => ({ role: 'assistant', content: 'Rename finished.' })
        const result = await runAgent(
            job({
                resume: { call_ids: ['approved'], decisions: [{ type: 'approve' }] },
                model_turns: 1,
            }),
            host,
            signal(),
        )
        expect(result.status).toBe('completed')
        expect(host.calls).toEqual([
            { call_id: 'approved', name: 'instances_rename', arguments: { name: 'Approved' } },
        ])
    })

    it('retains auto-approved read results when a write in the batch is rejected', async () => {
        const host = new FixtureHost()
        host.respond = () => ({
            role: 'assistant',
            content: '',
            tool_calls: [
                { id: 'read', name: 'instances_list', args: {} },
                { id: 'rejected', name: 'instances_rename', args: { name: 'Never' } },
            ],
        })
        await runAgent(job(), host, signal())
        host.respond = () => ({ role: 'assistant', content: 'Read only. Rename rejected.' })
        await runAgent(
            job({
                resume: {
                    call_ids: ['rejected'],
                    decisions: [{ type: 'reject', message: 'Do not rename.' }],
                },
            }),
            host,
            signal(),
        )
        expect(host.calls.map((call) => call.call_id)).toEqual(['read'])
        const messages = host.turns[1].messages
        expect(
            messages
                .filter((message) => message.role === 'tool')
                .map((message) => message.tool_call_id)
                .sort(),
        ).toEqual(['read', 'rejected'])
        expect(
            messages
                .find((message) => message.role === 'assistant')
                ?.tool_calls?.map((call) => call.id),
        ).toEqual(['read', 'rejected'])
    })

    it('keeps deferred work pending and refuses to declare an unconfirmed result complete', async () => {
        const host = new FixtureHost()
        host.definitions = [
            { ...host.definitions[0], execution: { deferred: true, interrupts_mutation: false } },
        ]
        host.output = {
            content: [],
            structured_content: { operation_id: 'background-op', status: 'queued' },
            is_error: false,
        }
        host.respond = (_, index) =>
            index === 1
                ? {
                      role: 'assistant',
                      content: '',
                      tool_calls: [{ id: 'deferred', name: 'instances_list', args: {} }],
                  }
                : { role: 'assistant', content: 'I think it is done.' }
        await expect(runAgent(job(), host, signal())).rejects.toThrow(
            'still unconfirmed: background-op',
        )
        expect(
            host.events.filter((event) => event.type === 'step').map((event) => event.status),
        ).toEqual(['running'])
    })

    it('preserves screenshot images and tool errors for model reasoning', async () => {
        const host = new FixtureHost()
        host.output = {
            is_error: true,
            content: [{ type: 'image', mime_type: 'image/png', data: 'iVBORw0KGgo=' }],
            structured_content: { error: { code: 'example' } },
        }
        host.respond = (_, index) =>
            index === 1
                ? {
                      role: 'assistant',
                      content: '',
                      tool_calls: [{ id: 'screenshot', name: 'instances_list', args: {} }],
                  }
                : { role: 'assistant', content: 'The tool reported a failure.' }
        await runAgent(job(), host, signal())
        expect(
            host.events.some((event) => event.type === 'step' && event.status === 'failed'),
        ).toBe(true)
        expect(host.turns[1].messages.find((message) => message.role === 'tool')?.images).toEqual([
            { mime_type: 'image/png', data: 'iVBORw0KGgo=' },
        ])
        expect(host.turns[1].messages.find((message) => message.role === 'tool')?.tool_error).toBe(
            true,
        )
    })

    it('does not expose writes to readonly verification runs', async () => {
        const host = new FixtureHost()
        await runAgent(job({ check_only: true }), host, signal())
        expect(
            host.turns[0].tools.map((item) => (item.function as { name: string }).name),
        ).not.toContain('instances_rename')
    })

    it('enforces model and tool budgets before making another host call', async () => {
        const host = new FixtureHost()
        await expect(runAgent(job({ model_turns: 64 }), host, signal())).rejects.toThrow(
            'Model turn limit',
        )
        expect(host.turns).toHaveLength(0)
        const other = new FixtureHost()
        other.respond = () => ({
            role: 'assistant',
            content: '',
            tool_calls: [{ id: 'read', name: 'instances_list', args: {} }],
        })
        await expect(runAgent(job({ tool_calls: 256 }), other, signal())).rejects.toThrow(
            'Tool call limit',
        )
        expect(other.calls).toHaveLength(0)
    })

    it('aborts before a late model reply can start a write', async () => {
        const host = new FixtureHost()
        const controller = new AbortController()
        host.respond = () => {
            controller.abort(new Error('Stopped'))
            return {
                role: 'assistant',
                content: '',
                tool_calls: [{ id: 'late', name: 'instances_rename', args: { name: 'Never' } }],
            }
        }
        await expect(runAgent(job(), host, controller.signal)).rejects.toThrow()
        expect(host.calls).toEqual([])
        expect(host.approvals).toEqual([])
    })
})

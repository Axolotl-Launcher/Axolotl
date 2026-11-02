import { AIMessage } from '@langchain/core/messages'
import { emptyCheckpoint, WRITES_IDX_MAP } from '@langchain/langgraph-checkpoint'
import { describe, expect, it } from 'vitest'

import { HostCheckpointer } from '../src/checkpointer.ts'
import { FixtureHost } from './fixtures.ts'

describe('host-backed LangGraph checkpoint adapter', () => {
    it('restores real message instances, opaque state, and pending writes in a new saver', async () => {
        const host = new FixtureHost()
        const saver = new HostCheckpointer('attempt', host)
        const message = new AIMessage({
            content: 'Evidence',
            additional_kwargs: { axolotl_opaque: ['signed'] },
        })
        const checkpoint = {
            ...emptyCheckpoint(),
            id: 'cp-1',
            channel_values: { messages: [message] },
        }
        const config = await saver.put(
            { configurable: { thread_id: 'attempt' } },
            checkpoint,
            { source: 'loop', step: 1, parents: {} },
            {},
        )
        await saver.putWrites(config, [['messages', [message]]], 'tool-task')
        await saver.putWrites(config, [['messages', ['duplicate must not overwrite']]], 'tool-task')
        await saver.putWrites(config, [['__error__', 'first']], 'error-task')
        await saver.putWrites(config, [['__error__', 'latest']], 'error-task')
        const restored = await new HostCheckpointer('attempt', host).getTuple(config)
        const messages = restored!.checkpoint.channel_values.messages as AIMessage[]
        expect(AIMessage.isInstance(messages[0])).toBe(true)
        expect(messages[0].additional_kwargs.axolotl_opaque).toEqual(['signed'])
        expect(restored!.pendingWrites!.find(([task]) => task === 'tool-task')?.[2]).toEqual([
            message,
        ])
        expect(restored!.pendingWrites!.find(([task]) => task === 'error-task')?.[2]).toBe('latest')
        expect(
            [...host.records.values()][0].writes.find((write) => write.channel === '__error__')
                ?.index,
        ).toBe(WRITES_IDX_MAP.__error__)
    })

    it('keeps parent traversal and listing filters, with no worker deletion authority', async () => {
        const host = new FixtureHost()
        const saver = new HostCheckpointer('attempt', host)
        const first = await saver.put(
            { configurable: { thread_id: 'attempt' } },
            { ...emptyCheckpoint(), id: 'a' },
            { source: 'input', step: 0, parents: {} },
            {},
        )
        const second = await saver.put(
            first,
            { ...emptyCheckpoint(), id: 'b' },
            { source: 'loop', step: 1, parents: {} },
            {},
        )
        expect((await saver.getTuple(second))!.parentConfig).toEqual(first)
        const values = []
        for await (const item of saver.list(
            { configurable: { thread_id: 'attempt' } },
            { filter: { source: 'input' }, limit: 1 },
        ))
            values.push(item.checkpoint.id)
        expect(values).toEqual(['a'])
        await expect(saver.getTuple({ configurable: { thread_id: 'other' } })).rejects.toThrow(
            'another run',
        )
        await expect(saver.deleteThread('attempt')).rejects.toThrow('Only the Agent Window')
    })
})

import { AIMessage, HumanMessage, ToolMessage } from '@langchain/core/messages'
import { describe, expect, it } from 'vitest'

import { deserializeMessage, LauncherChatModel, serializeMessage } from '../src/model.ts'

describe('shared credential-free model adapter', () => {
    it('round-trips tool IDs, opaque provider state, text and image blocks', () => {
        const assistant = new AIMessage({
            content: 'Inspecting',
            tool_calls: [{ type: 'tool_call', id: 'c', name: 'ui_screenshot', args: {} }],
            additional_kwargs: { axolotl_opaque: { reasoning: 'signed' } },
        })
        const wire = serializeMessage(assistant)
        expect(serializeMessage(deserializeMessage(wire))).toEqual(wire)
        const tool = new ToolMessage({
            tool_call_id: 'c',
            content: [
                { type: 'text', text: 'Actual screen' },
                { type: 'image_url', image_url: { url: 'data:image/png;base64,iVBORw0KGgo=' } },
            ],
        })
        expect(serializeMessage(tool)).toMatchObject({
            tool_call_id: 'c',
            content: 'Actual screen',
            images: [{ mime_type: 'image/png', data: 'iVBORw0KGgo=' }],
        })
        expect(serializeMessage(deserializeMessage(serializeMessage(tool)))).toEqual(
            serializeMessage(tool),
        )
    })

    it('retains legacy LogAgent opaque state without changing diagnostic requests', () => {
        const message = new AIMessage({
            content: 'Evidence',
            additional_kwargs: { logagent_opaque: ['signed'] },
        })
        expect(serializeMessage(message).opaque).toEqual(['signed'])
        expect(serializeMessage(new HumanMessage('diagnose')).images).toBeUndefined()
    })

    it('does not silently discard unsupported or remote content', () => {
        expect(() =>
            serializeMessage(
                new HumanMessage({
                    content: [
                        {
                            type: 'image_url',
                            image_url: { url: 'https://example.com/private.png' },
                        },
                    ],
                }),
            ),
        ).toThrow('Unsupported')
        expect(() =>
            serializeMessage(new HumanMessage({ content: [{ type: 'audio', data: 'bytes' }] })),
        ).toThrow('Unsupported model content')
    })

    it('forwards cancellation and stream callbacks through one model request', async () => {
        const controller = new AbortController()
        let count = 0
        const events: string[] = []
        const model = new LauncherChatModel(
            {
                modelTurn: async (_, emit, signal) => {
                    count++
                    expect(signal).toBeDefined()
                    emit?.({ event_type: 'delta', data: { text: 'hello' } })
                    return { role: 'assistant', content: 'hello' }
                },
            },
            { onEvent: (event) => events.push(String(event.data.text)) },
        )
        expect(
            (await model.invoke([new HumanMessage('Hello')], { signal: controller.signal })).text,
        ).toBe('hello')
        expect(count).toBe(1)
        expect(events).toEqual(['hello'])
        controller.abort()
        await expect(
            model.invoke([new HumanMessage('late')], { signal: controller.signal }),
        ).rejects.toThrow()
        expect(count).toBe(1)
    })
})

import { expect, it } from 'vitest'

import {
    activeThread,
    commitStore,
    initialStore,
    loadStore,
    saveStore,
    STORAGE_KEY,
} from '../../../../../packages/agent-window/src/state.js'

function memoryStorage() {
    const values = new Map<string, string>()
    return {
        getItem: (key: string) => values.get(key) ?? null,
        setItem: (key: string, value: string) => {
            values.set(key, value)
        },
    }
}

it('restores provider-qualified selection and drafts but never resumes waiting requests automatically', () => {
    const storage = memoryStorage()
    const store = initialStore()
    const thread = activeThread(store)!
    thread.draft = '保留尚未发送的输入'
    thread.selection = { provider_id: 'openrouter', model_id: 'anthropic/claude-sonnet-4.6' }
    thread.messages.push({
        id: 'request',
        role: 'user',
        text: '检查更新',
        status: 'waiting',
        createdAt: Date.now(),
        instanceId: null,
        selection: thread.selection,
    })
    saveStore(store, storage)
    const restored = activeThread(loadStore(storage))!
    expect(restored.draft).toBe(thread.draft)
    expect(restored.selection).toEqual(thread.selection)
    expect(restored.messages[0].status).toBe('interrupted')
})

it('retains current input and messages when a transactional update cannot be saved', () => {
    const store = initialStore()
    activeThread(store)!.draft = '存储失败时不能丢掉'
    const storage = {
        getItem: () => null,
        setItem: () => {
            throw new Error('quota')
        },
    }
    const next = structuredClone(store)
    activeThread(next)!.draft = ''
    expect(() => commitStore(next, storage)).toThrow('quota')
    expect(activeThread(store)!.draft).toBe('存储失败时不能丢掉')
    expect(activeThread(store)!.messages).toHaveLength(0)
})

it('rejects an unsupported format and sanitizes saved history before displaying it', () => {
    const storage = memoryStorage()
    storage.setItem(STORAGE_KEY, JSON.stringify({ version: 999, threads: [] }))
    expect(() => loadStore(storage)).toThrow('Invalid conversation store')
    storage.setItem(
        STORAGE_KEY,
        JSON.stringify({
            version: 1,
            activeId: 'a',
            threads: [
                {
                    id: 'a',
                    selection: { provider_id: 'x' },
                    messages: [{ id: 'x', role: 'assistant', text: 'stale', status: 'waiting' }],
                },
                { id: 'a', messages: [] },
                null,
            ],
        }),
    )
    const restored = loadStore(storage)
    expect(restored.threads).toHaveLength(1)
    expect(activeThread(restored)!.selection).toBeNull()
    expect(activeThread(restored)!.messages[0]).toMatchObject({
        role: 'user',
        status: 'interrupted',
    })
})

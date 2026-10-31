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

it('rejects a failed commit without changing the caller’s current store', () => {
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

it('deduplicates message identities across every restored thread', () => {
    const storage = memoryStorage()
    const store = initialStore()
    const first = activeThread(store)!
    first.messages.push({ id: 'same', text: 'first', status: 'stopped' })
    const second = { ...structuredClone(first), id: 'second' }
    second.messages.push({ id: 'unique', text: 'second', status: 'stopped' })
    store.threads.push(second)
    saveStore(store, storage)
    const restored = loadStore(storage)
    expect(
        restored.threads.flatMap((thread) => thread.messages.map((message) => message.id)),
    ).toEqual(['same', 'unique'])
})

it('bounds retained history without discarding drafts or waiting work', () => {
    const storage = memoryStorage()
    const store = initialStore()
    const thread = activeThread(store)!
    thread.draft = 'keep this draft'
    thread.messages = Array.from({ length: 80 }, (_, index) => ({
        id: `old-${index}`,
        text: 'x'.repeat(50_000),
        status: 'stopped',
        createdAt: index,
    }))
    thread.messages.push({ id: 'pending', text: 'keep waiting', status: 'waiting', createdAt: 100 })
    const saved = commitStore(store, storage)
    expect(storage.getItem(STORAGE_KEY)!.length).toBeLessThanOrEqual(1_000_000)
    expect(activeThread(saved)!.draft).toBe('keep this draft')
    expect(activeThread(saved)!.messages.at(-1)?.id).toBe('pending')
    expect(activeThread(saved)!.messages.some((message) => message.id === 'old-0')).toBe(false)
})

it('keeps protected work intact and leaves storage unchanged when it exceeds the budget', () => {
    const storage = memoryStorage()
    const store = initialStore()
    saveStore(store, storage)
    const original = storage.getItem(STORAGE_KEY)
    const thread = activeThread(store)!
    thread.draft = 'important draft'
    thread.messages = Array.from({ length: 21 }, (_, index) => ({
        id: `pending-${index}`,
        text: 'x'.repeat(50_000),
        status: 'waiting',
        createdAt: index,
    }))
    expect(() => commitStore(store, storage)).toThrow('Conversation storage limit reached')
    expect(storage.getItem(STORAGE_KEY)).toBe(original)
    expect(thread.draft).toBe('important draft')
    expect(thread.messages).toHaveLength(21)
})

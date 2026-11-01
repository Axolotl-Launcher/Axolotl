import { expect, it } from 'vitest'

import {
    activeThread,
    initialStore,
    LEGACY_STORAGE_KEY,
    loadStore,
    newRun,
    newThread,
    newTurn,
    saveStore,
    STORAGE_KEY,
    threadStatus,
} from '../../../../../packages/agent-window/src/state.js'

function memoryStorage(entries: Record<string, string> = {}) {
    const values = new Map<string, string>(Object.entries(entries))
    return {
        values,
        getItem: (key: string) => values.get(key) ?? null,
        setItem: (key: string, value: string) => {
            values.set(key, value)
        },
    }
}

function turnWith(status: string, text = '检查更新') {
    const thread = newThread()
    const turn = newTurn(thread, { text })
    turn.run.status = status
    return turn
}

it('restores drafts and selections but never resumes a run by itself', () => {
    const storage = memoryStorage()
    const store = initialStore()
    const thread = activeThread(store)!
    thread.draft = '保留尚未发送的输入'
    thread.selection = { provider_id: 'openrouter', model_id: 'anthropic/claude-sonnet-4.6' }
    const reading = turnWith('running')
    reading.run.items.push({
        id: 'activity',
        type: 'activity',
        steps: [{ id: 'a', title: 'Read', detail: '', tool: 'logs_read', status: 'running' }],
        startedAt: 1,
        endedAt: null,
    })
    const writing = turnWith('applying')
    writing.run.items.push({
        id: 'plan',
        type: 'plan',
        plan: { title: 'Plan', changes: [{ name: 'Sodium', from: '1', to: '2' }] },
        state: 'applying',
        done: 0,
    })
    const waiting = turnWith('awaiting')
    waiting.run.items.push({
        id: 'pending',
        type: 'plan',
        plan: { title: 'Plan', changes: [] },
        state: 'pending',
        done: 0,
    })
    thread.turns.push(reading, writing, waiting)
    saveStore(store, storage)

    const restored = activeThread(loadStore(storage))!
    expect(restored.draft).toBe(thread.draft)
    expect(restored.selection).toEqual(thread.selection)
    const [first, second, third] = restored.turns
    expect(first.run.status).toBe('interrupted')
    expect(first.run.items[0].steps[0].status).toBe('stopped')
    // A write that was cut off has an unknown outcome until it is checked again.
    expect(second.run.status).toBe('detached')
    expect(second.run.items[0].state).toBe('unknown')
    expect(third.run.status).toBe('awaiting')
    expect(third.run.items[0].state).toBe('pending')
})

it('moves the first preview history into turns that did not run', () => {
    const storage = memoryStorage({
        [LEGACY_STORAGE_KEY]: JSON.stringify({
            version: 1,
            activeId: 'legacy',
            sidebarOpen: false,
            threads: [
                {
                    id: 'legacy',
                    title: '旧任务',
                    draft: '草稿',
                    messages: [{ id: 'm1', role: 'user', text: '检查更新', status: 'waiting' }],
                },
            ],
        }),
    })
    const store = loadStore(storage)
    expect(store.version).toBe(2)
    expect(store.sidebarOpen).toBe(false)
    const thread = activeThread(store)!
    expect(thread.title).toBe('旧任务')
    expect(thread.draft).toBe('草稿')
    expect(thread.turns).toHaveLength(1)
    expect(thread.turns[0].text).toBe('检查更新')
    expect(thread.turns[0].run.status).toBe('interrupted')
    saveStore(store, storage)
    expect(storage.values.has(STORAGE_KEY)).toBe(true)
})

it('rejects an unsupported format and rebuilds saved history field by field', () => {
    expect(() => loadStore(memoryStorage({ [STORAGE_KEY]: '{"version":9,"threads":[]}' }))).toThrow(
        'Invalid conversation store',
    )
    expect(() => loadStore(memoryStorage({ [STORAGE_KEY]: 'not json' }))).toThrow()

    const store = loadStore(
        memoryStorage({
            [STORAGE_KEY]: JSON.stringify({
                version: 2,
                activeId: 'missing',
                threads: [
                    {
                        id: 't1',
                        title: 42,
                        draft: { html: '<img src=x onerror=alert(1)>' },
                        selection: { provider_id: 1, model_id: 'x' },
                        turns: [
                            {
                                id: 'turn',
                                text: '<b>keep as text</b>',
                                attachments: [{ name: 'latest.log', size: -4 }, { size: 3 }],
                                run: {
                                    status: 'hacked',
                                    items: [
                                        { type: 'script', text: 'alert(1)' },
                                        {
                                            id: 'x',
                                            type: 'text',
                                            text: '<script>alert(1)</script>',
                                        },
                                        {
                                            type: 'plan',
                                            plan: {
                                                changes: [
                                                    { name: 7 },
                                                    { name: 'Sodium', glyph: 'ABCDEF' },
                                                ],
                                            },
                                        },
                                    ],
                                },
                            },
                            { id: 'turn', text: 'duplicate id' },
                        ],
                    },
                    { id: 't1', title: 'duplicate thread' },
                    null,
                ],
            }),
        }),
    )
    expect(store.threads).toHaveLength(1)
    expect(store.activeId).toBe('t1')
    const thread = store.threads[0]
    expect(thread.title).toBe('')
    expect(thread.draft).toBe('')
    expect(thread.selection).toBeNull()
    expect(thread.turns).toHaveLength(1)
    const [turn] = thread.turns
    expect(turn.text).toBe('<b>keep as text</b>')
    expect(turn.attachments).toEqual([{ id: expect.any(String), name: 'latest.log', size: 0 }])
    expect(turn.run.status).toBe('interrupted')
    expect(turn.run.items.map((item: { type: string }) => item.type)).toEqual(['text', 'plan'])
    expect(turn.run.items[1].plan.changes).toEqual([
        expect.objectContaining({ name: 'Sodium', glyph: 'ABC', manual: false }),
    ])
    expect(turn.run.items[1].state).toBe('unknown')
})

it('rejects a failed save without changing the caller’s store', () => {
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
    expect(() => saveStore(next, storage)).toThrow('quota')
    expect(activeThread(store)!.draft).toBe('存储失败时不能丢掉')
})

it('makes room by dropping old settled turns, never drafts, queues or open plans', () => {
    const storage = memoryStorage()
    const store = initialStore()
    const active = activeThread(store)!
    active.draft = 'draft'
    const waiting = turnWith('awaiting', 'keep waiting')
    active.turns.push(waiting)
    const queued = newThread()
    queued.queue.push({ id: 'q', text: 'later', attachments: [], createdAt: 1 })
    queued.updatedAt = 1
    store.threads.push(queued)
    const old = newThread()
    old.updatedAt = 2
    for (let index = 0; index < 650; index++) {
        const turn = turnWith('done', `old ${index}`)
        turn.createdAt = index
        old.turns.push(turn)
    }
    store.threads.push(old)

    const saved = saveStore(store, storage)
    const count = saved.threads.reduce(
        (total: number, thread: { turns: unknown[] }) => total + thread.turns.length,
        0,
    )
    expect(count).toBeLessThanOrEqual(600)
    expect(activeThread(saved)!.draft).toBe('draft')
    expect(activeThread(saved)!.turns[0].text).toBe('keep waiting')
    expect(
        saved.threads.find((thread: { id: string }) => thread.id === queued.id)!.queue,
    ).toHaveLength(1)
    const kept = saved.threads.find((thread: { id: string }) => thread.id === old.id)!.turns
    expect(kept[kept.length - 1].text).toBe('old 649')
    expect(kept[0].text).not.toBe('old 0')
})

it('leaves storage untouched when protected work alone exceeds the budget', () => {
    const storage = memoryStorage({ [STORAGE_KEY]: 'previous' })
    const store = initialStore()
    activeThread(store)!.draft = 'x'.repeat(50_000)
    for (let index = 0; index < 40; index++) {
        const thread = newThread()
        thread.draft = 'y'.repeat(50_000)
        store.threads.push(thread)
    }
    expect(() => saveStore(store, storage)).toThrow('Conversation storage limit reached')
    expect(storage.values.get(STORAGE_KEY)).toBe('previous')
})

it('summarises a thread by the state of its latest request', () => {
    const thread = newThread()
    expect(threadStatus(thread)).toBe('idle')
    const turn = newTurn(thread, { text: 'x' })
    thread.turns.push(turn)
    expect(threadStatus(thread)).toBe('active')
    for (const [status, expected] of [
        ['running', 'active'],
        ['applying', 'active'],
        ['awaiting', 'awaiting'],
        ['failed', 'attention'],
        ['detached', 'attention'],
        ['done', 'idle'],
        ['stopped', 'idle'],
    ]) {
        turn.run = { ...newRun(), status }
        expect(threadStatus(thread)).toBe(expected)
    }
})

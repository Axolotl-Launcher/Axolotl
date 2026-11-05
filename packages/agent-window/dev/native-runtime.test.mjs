import assert from 'node:assert/strict'
import test from 'node:test'
import { argumentRows, changeTitle } from '../src/tools.js'
import { setLocale, text } from '../src/messages.js'

import { applyEvent, applyNativeSnapshot, createRuntime, NativeRuntime } from '../src/agent.js'
import {
    createStorage,
    initialStore,
    mergeAcknowledged,
    newRun,
    parseNativeStore,
    parseStore,
    STORAGE_KEY,
} from '../src/state.js'

const memory = (raw) => ({ getItem: (key) => (key === STORAGE_KEY ? raw : null), setItem() {} })
const flush = () => new Promise((resolve) => setImmediate(resolve))

test('live write receipts stay applying until the native attempt settles', () => {
    const run = newRun()
    applyEvent(run, { type: 'plan', plan: { changes: [{ name: 'call' }] } })
    const receipts = { unknown: 0, actions: [{ call_id: 'call', state: 'running' }] }
    applyNativeSnapshot(run, 'running', receipts)
    assert.equal(run.status, 'applying')
    assert.equal(run.items[0].state, 'applying')
    applyNativeSnapshot(run, 'completed', {
        unknown: 0,
        actions: [{ call_id: 'call', state: 'applied' }],
    })
    assert.equal(run.status, 'done')
    assert.equal(run.items[0].state, 'applied')
})

test('approval arguments and step counts use localized labels, including old stored plans', () => {
    setLocale('zh-CN', {
        argumentName: '名称',
        argumentYes: '是',
        argumentEnabled: '启用',
        toolHelpInstanceCreate: '创建实例',
        stepsRunning: '处理中 · 共 {count} 步',
    })
    assert.equal(text('stepsRunning', { count: 3 }), '处理中 · 共 3 步')
    assert.equal(changeTitle({ meta: 'instance_create', note: '{"name":"我的实例"}' }), '创建实例')
    assert.deepEqual(argumentRows({ name: '我的实例', enabled: true }), [
        { label: '名称', value: '我的实例' },
        { label: '启用', value: '是' },
    ])
    setLocale('en-US')
})

test('native storage preserves frozen installation details without making IDs the display name', () => {
    setLocale('en-US', { argumentInstanceId: 'Instance' })
    const store = initialStore()
    store.threads[0].pinned = true
    store.threads[0].rank = 3
    const review = {
        mode: 'resources',
        intent: 'install',
        dependencies_checked: true,
        install_dependencies: true,
        instance: {
            id: 'local:target',
            name: '机械动力',
            game_version: '1.21.1',
            loader: 'neoforge',
        },
        actions: [
            {
                title: 'Create',
                version: '6.0.10',
                source: 'modrinth',
                target_path: 'mods/create.jar',
                dependency: false,
            },
        ],
        issues: [],
        exceptions: [],
    }
    store.threads[0].turns.push({
        id: 'turn',
        text: 'Install Create',
        attachments: [],
        run: {
            ...newRun(),
            status: 'awaiting',
            items: [
                {
                    id: 'plan',
                    type: 'plan',
                    state: 'pending',
                    plan: {
                        changes: [
                            {
                                name: 'call',
                                tool: 'resource_execute_plan',
                                arguments: { plan_id: 'opaque', revision: 1 },
                                review,
                            },
                        ],
                    },
                },
            ],
        },
    })
    const loaded = parseNativeStore(JSON.stringify(store))
    assert.equal(loaded.threads[0].rank, 3)
    const change = loaded.threads[0].turns[0].run.items[0].plan.changes[0]
    assert.deepEqual(change.review, review)
    assert.deepEqual(
        argumentRows({ instance_id: 'local:target', plan_id: 'opaque', revision: 1 }, [
            { id: 'local:target', name: '机械动力' },
        ]),
        [{ label: 'Instance', value: '机械动力' }],
    )
    assert.equal(
        argumentRows(change.arguments, [], true).some((row) => row.value === 'opaque'),
        true,
    )
    setLocale('en-US')
})

test('native missing backend never becomes demo, explicit preview does', async () => {
    const unavailable = await createRuntime({ native: true, preview: true }, {})
    assert.equal(unavailable.preview, false)
    assert.throws(() => unavailable.start())
    assert.equal((await createRuntime({ preview: true }, {})).preview, true)
})

test('native import is idempotently identified and acknowledged before use', async () => {
    const store = initialStore()
    const calls = []
    const host = {
        runtimeInvoke: async (name, args) => {
            calls.push([name, args])
            if (name === 'load') return { workspace: null, threads: [], workspace_revision: 4 }
            if (name === 'import') return false
            if (name === 'save') return 5
        },
    }
    const storage = createStorage(host, memory(JSON.stringify(store)))
    const loaded = await storage.load()
    assert.equal(loaded.activeId, store.activeId)
    assert.equal(calls[1][1].importId, STORAGE_KEY)
    assert.equal(calls[2][1].expectedRevision, 4)
    assert.deepEqual(
        calls.map(([name]) => name),
        ['load', 'import', 'save'],
    )
})

test('native storage failure does not fall back to browser save', async () => {
    let browserWrites = 0
    const storage = createStorage(
        {
            runtimeInvoke: async () => {
                throw Error('offline')
            },
        },
        {
            getItem: () => null,
            setItem: () => browserWrites++,
        },
    )
    await assert.rejects(storage.load(), /offline/)
    await assert.rejects(storage.save(initialStore()), /offline/)
    assert.equal(browserWrites, 0)
})

test('native polling deduplicates cursor replay and detach never cancels', async () => {
    const calls = []
    const events = []
    const runtime = new NativeRuntime(async (name, args) => {
        calls.push([name, args])
        if (name === 'snapshot')
            return { attempts: [{ id: 'attempt', state: 'completed' }], results: {} }
        return [
            { sequence: 3, attempt_id: 'attempt', payload: { type: 'text', delta: 'old' } },
            { sequence: 4, attempt_id: 'attempt', payload: { type: 'text', delta: 'new' } },
            { sequence: 5, attempt_id: 'attempt', payload: { type: 'done' } },
        ]
    })
    const handle = runtime.follow('thread', 'attempt', 3, (event) => events.push(event))
    await flush()
    handle.detach()
    assert.deepEqual(
        events.map((event) => event.type),
        ['text', 'done', 'snapshot'],
    )
    assert.equal(events[1].nativeResume.cursor, 5)
    assert.deepEqual(
        calls.map(([name]) => name),
        ['snapshot', 'events', 'snapshot'],
    )
})

test('detach discards a late polling response', async () => {
    let resolve
    const events = []
    const runtime = new NativeRuntime(
        () =>
            new Promise((done) => {
                resolve = done
            }),
    )
    const handle = runtime.follow('thread', 'attempt', 0, (event) => events.push(event))
    handle.detach()
    resolve([{ sequence: 1, attempt_id: 'attempt', payload: { type: 'text', delta: 'late' } }])
    await flush()
    assert.deepEqual(events, [])
})

test('done needs successful receipts before a plan is applied', () => {
    const run = newRun()
    applyEvent(run, { type: 'plan', plan: { changes: [{ name: 'call-1' }] } })
    run.status = 'applying'
    run.items[0].state = 'applying'
    applyEvent(run, { type: 'done' })
    assert.equal(run.items[0].state, 'unknown')
    run.status = 'applying'
    applyEvent(run, {
        type: 'done',
        applied: {
            selected: 1,
            applied: 1,
            failed: 0,
            unknown: 0,
            complete: true,
            actions: [{ call_id: 'call-1', state: 'applied' }],
        },
    })
    assert.equal(run.items[0].state, 'applied')
    applyEvent(run, {
        type: 'done',
        applied: {
            selected: 1,
            applied: 0,
            failed: 0,
            unknown: 1,
            complete: false,
            actions: [{ call_id: 'call-1', state: 'unknown' }],
        },
    })
    assert.equal(run.items[0].state, 'unknown')
})

test('approval forwards actual call IDs and batch revision', async () => {
    const calls = []
    const runtime = new NativeRuntime(async (name, args) => {
        calls.push([name, args])
        return name === 'snapshot' ? { attempts: [] } : []
    })
    const handle = runtime.approve(
        {
            thread: { id: 'thread' },
            resume: { attempt_id: 'attempt', cursor: 4 },
            planId: 'batch',
            revision: 3,
            selected: ['call-real'],
        },
        () => {},
    )
    await flush()
    handle.detach()
    assert.deepEqual(calls[0], [
        'decide',
        { batchId: 'batch', revision: 3, selected: ['call-real'] },
    ])
})

test('check submits read only and binds verification to prior attempt', async () => {
    globalThis.document = { documentElement: { lang: 'en-US' } }
    const calls = []
    const runtime = new NativeRuntime(async (name, args) => {
        calls.push([name, args])
        return name === 'snapshot' ? { attempts: [] } : []
    })
    const handle = runtime.check(
        {
            thread: { id: 'thread' },
            turn: { id: 'turn', text: 'check', attachments: [] },
            resume: { attempt_id: 'write-attempt' },
            model: { provider: { provider_id: 'provider' }, model: { id: 'model' } },
            history: [{ run: { resume: { attempt_id: 'visible-history' } } }],
        },
        () => {},
    )
    await flush()
    handle.detach()
    const submitted = calls.find(([name]) => name === 'submit')[1].request
    assert.equal(submitted.check_only, true)
    assert.equal(submitted.verify_attempt, 'write-attempt')
    assert.equal(submitted.computer_use, false)
    assert.equal(submitted.turn_id, 'turn')
    assert.deepEqual(submitted.history_attempts, ['visible-history'])
})

test('acknowledged rename wins an AI title race and retains concurrent draft and stream', () => {
    const base = initialStore()
    const next = structuredClone(base)
    next.threads[0].title = 'My task'
    next.threads[0].titleEdited = true
    const live = structuredClone(base)
    live.threads[0].title = 'Model generated title'
    live.threads[0].draft = 'Keep typing'
    live.threads[0].turns.push({ id: 'turn', run: { items: [{ id: 'text', text: 'streamed' }] } })
    const merged = mergeAcknowledged(base, next, live)
    assert.equal(merged.threads[0].title, 'My task')
    assert.equal(merged.threads[0].titleEdited, true)
    assert.equal(merged.threads[0].draft, 'Keep typing')
    assert.equal(merged.threads[0].turns[0].run.items[0].text, 'streamed')
})

test('worker loss without terminal journal event settles from snapshot', async () => {
    const events = []
    const runtime = new NativeRuntime(async (name) =>
        name === 'snapshot'
            ? {
                  attempts: [{ id: 'attempt', state: 'interrupted' }],
                  results: { attempt: { selected: 0, unknown: 0, actions: [] } },
              }
            : [],
    )
    const handle = runtime.follow('thread', 'attempt', 7, (event) => events.push(event))
    await flush()
    handle.detach()
    assert.equal(events.length, 1)
    assert.equal(events[0].type, 'snapshot')
    assert.equal(events[0].terminal, true)
    const run = newRun()
    applyNativeSnapshot(run, events[0].state, events[0].receipts)
    assert.equal(run.status, 'interrupted')
})

test('terminal snapshots restore cursor-end lifecycle and distinguish unknown writes', () => {
    for (const [state, expected] of [
        ['completed', 'done'],
        ['failed', 'failed'],
        ['cancelled', 'stopped'],
        ['interrupted', 'interrupted'],
    ]) {
        const run = newRun()
        applyNativeSnapshot(run, state, { selected: 0, unknown: 0, actions: [] })
        assert.equal(run.status, expected)
    }
    const run = newRun()
    applyEvent(run, { type: 'plan', plan: { changes: [{ name: 'call' }] } })
    run.items[0].state = 'unknown'
    applyNativeSnapshot(run, 'cancelled', {
        selected: 1,
        applied: 0,
        unknown: 1,
        actions: [{ call_id: 'call', state: 'unknown' }],
    })
    assert.equal(run.status, 'detached')
    assert.equal(run.items[0].state, 'unknown')
    applyNativeSnapshot(run, 'cancelled', {
        selected: 1,
        applied: 0,
        unknown: 0,
        not_executed: 1,
        actions: [{ call_id: 'call', state: 'not_executed' }],
    })
    assert.equal(run.status, 'stopped')
})

test('failed decision recovers awaiting and later only successful receipts mark applied', () => {
    const run = newRun()
    applyEvent(run, { type: 'plan', plan: { changes: [{ name: 'call' }] } })
    run.status = 'detached'
    run.items[0].state = 'unknown'
    applyNativeSnapshot(run, 'awaiting', { selected: 0, unknown: 0, actions: [] })
    assert.equal(run.status, 'awaiting')
    assert.equal(run.items[0].state, 'pending')
    applyNativeSnapshot(run, 'completed', {
        selected: 1,
        applied: 1,
        unknown: 0,
        actions: [{ call_id: 'call', state: 'applied' }],
    })
    assert.equal(run.status, 'done')
    assert.equal(run.items[0].state, 'applied')
})

test('confirming an existing title also takes ownership during the first model title', () => {
    const base = initialStore()
    base.threads[0].title = 'My task'
    const next = structuredClone(base)
    next.threads[0].titleEdited = true
    const live = structuredClone(base)
    live.threads[0].title = 'Model replacement'
    assert.equal(mergeAcknowledged(base, next, live).threads[0].title, 'My task')
})

test('legacy import never infers deletion of server-only threads without workspace', async () => {
    let saved
    const storage = createStorage(
        {
            runtimeInvoke: async (name, args) => {
                if (name === 'load')
                    return {
                        workspace: null,
                        workspace_revision: 0,
                        threads: [{ id: 'server-only' }],
                    }
                if (name === 'save') {
                    saved = args
                    return 1
                }
                return true
            },
        },
        memory(JSON.stringify(initialStore())),
    )
    await storage.load()
    assert.deepEqual(saved.deletedThreadIds, [])
})

test('explicit cancel uses terminal receipts to distinguish stopped from uncertain writes', async () => {
    const events = []
    const runtime = new NativeRuntime(async (name) =>
        name === 'snapshot'
            ? {
                  attempts: [{ id: 'attempt', state: 'cancelled' }],
                  results: {
                      attempt: {
                          selected: 1,
                          applied: 0,
                          unknown: 0,
                          not_executed: 1,
                          actions: [{ call_id: 'call', state: 'not_executed' }],
                      },
                  },
              }
            : [],
    )
    const handle = runtime.follow('thread', 'attempt', 4, (event) => events.push(event), false)
    await handle.stop()
    const run = newRun()
    applyEvent(run, { type: 'plan', plan: { changes: [{ name: 'call' }] } })
    run.status = 'applying'
    run.items[0].state = 'applying'
    applyNativeSnapshot(run, events[0].state, events[0].receipts)
    assert.equal(run.status, 'stopped')
})

test('native reload preserves history above browser budgets without inferring deletions', async () => {
    const workspace = initialStore()
    const first = workspace.threads[0]
    const emptyThread = structuredClone(first)
    const longAnswer = 'complete answer '.repeat(110000)
    const attachmentText = 'full log content '.repeat(20000)
    first.turns = Array.from({ length: 650 }, (_, index) => ({
        id: `turn-${index}`,
        createdAt: Date.now(),
        text: `request ${index}`,
        attachments: [],
        instanceId: null,
        selection: null,
        run: { ...newRun(), status: 'done', items: [] },
    }))
    first.turns[649].run.items = [
        { id: 'answer', type: 'text', text: longAnswer },
        { id: 'result', type: 'result', title: 'Result', text: longAnswer, next: [] },
    ]
    first.attachments = [
        {
            id: 'log',
            name: 'latest.log',
            kind: 'log',
            size: attachmentText.length,
            text: attachmentText,
            lines: 20000,
        },
    ]
    for (let index = 1; index < 120; index++)
        workspace.threads.push({
            ...structuredClone(emptyThread),
            id: `thread-${index}`,
        })
    const raw = JSON.stringify(workspace)
    assert.ok(raw.length > 1500000)
    const calls = []
    const storage = createStorage(
        {
            runtimeInvoke: async (name, args) => {
                calls.push([name, args])
                if (name === 'load') return { workspace, workspace_revision: 8, threads: [] }
                return 9
            },
        },
        memory(null),
    )
    const loaded = await storage.load()
    assert.equal(loaded.threads.length, 120)
    assert.equal(loaded.threads[0].turns.length, 650)
    assert.equal(loaded.threads[0].turns[649].run.items[0].text, longAnswer)
    assert.equal(loaded.threads[0].turns[649].run.items[1].text, longAnswer)
    assert.equal(loaded.threads[0].attachments[0].text, attachmentText)
    await storage.save(loaded)
    assert.deepEqual(calls[1][1].deletedThreadIds, [])
    // The browser preview retains its existing bounded history policy.
    const preview = parseStore(raw)
    assert.equal(preview.threads[0].turns.length, 600)
    assert.equal(preview.threads[0].attachments[0].text.length, 200000)
})

test('native reload preserves complete tool rows and exclusion zero', () => {
    const workspace = initialStore()
    const changes = Array.from({ length: 70 }, (_, index) => ({
        name: `call-${index}`,
        note: 'x'.repeat(10000),
    }))
    workspace.threads[0].turns = [
        {
            id: 'turn',
            text: 'review',
            run: {
                ...newRun(),
                status: 'awaiting',
                items: [
                    {
                        id: 'plan',
                        type: 'plan',
                        state: 'pending',
                        plan: { id: 'batch', changes },
                        excluded: [0, 69],
                    },
                ],
            },
        },
    ]
    const loaded = parseNativeStore(JSON.stringify(workspace))
    const plan = loaded.threads[0].turns[0].run.items[0]
    assert.equal(plan.plan.changes.length, 70)
    assert.equal(plan.plan.changes[69].note.length, 10000)
    assert.deepEqual(plan.excluded, [0, 69])
})

test('native invalid or duplicate records fail load rather than disappearing', async () => {
    const workspace = initialStore()
    workspace.threads.push(structuredClone(workspace.threads[0]))
    const calls = []
    const storage = createStorage(
        {
            runtimeInvoke: async (name) => {
                calls.push(name)
                return { workspace, workspace_revision: 2 }
            },
        },
        memory(null),
    )
    await assert.rejects(storage.load(), /Invalid or duplicate native thread/)
    assert.deepEqual(calls, ['load'])
    workspace.threads.pop()
    workspace.threads[0].turns = [{ id: 'bad-turn', text: 17 }]
    assert.throws(
        () => parseNativeStore(JSON.stringify(workspace)),
        /Invalid native conversation record/,
    )
})

test('native envelope rejects oversized UTF-8 without saving or pruning history', async () => {
    const workspace = initialStore()
    // Six million three-byte characters exceed 16 MiB but not 16M characters.
    workspace.threads[0].draft = '汉'.repeat(6000000)
    let called = false
    const storage = createStorage(
        {
            runtimeInvoke: async () => {
                called = true
            },
        },
        memory(null),
    )
    await assert.rejects(storage.save(workspace), /exceeds 16 MiB/)
    assert.equal(called, false)
    assert.equal(workspace.threads[0].draft.length, 6000000)
})

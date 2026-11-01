import { readdir, readFile } from 'node:fs/promises'

import { afterEach, beforeEach, expect, it, vi } from 'vitest'

import {
    applyEvent,
    beginApply,
    declinePlan,
    includedChanges,
    pendingPlan,
    stopRun,
    supersedePlan,
    toggleChange,
} from '../../../../../packages/agent-window/src/agent.js'
import { FLOWS, GENERIC } from '../../../../../packages/agent-window/src/demo-flows.js'
import { DemoRuntime } from '../../../../../packages/agent-window/src/demo-runtime.js'
import { markdownBlocks, parseMarkdown } from '../../../../../packages/agent-window/src/dom.js'
import { setLocale } from '../../../../../packages/agent-window/src/messages.js'
import { newRun } from '../../../../../packages/agent-window/src/state.js'
import { toolFamily, toolHelp } from '../../../../../packages/agent-window/src/tools.js'

type Event = { type: string; [key: string]: unknown }

beforeEach(() => {
    vi.useFakeTimers()
    setLocale('en-US')
})

afterEach(() => {
    vi.useRealTimers()
})

function request(text: string, extra: Record<string, unknown> = {}) {
    return {
        thread: { id: 'thread', title: '' },
        turn: { id: 'turn', text, attachments: [] },
        instance: { id: 'i', name: 'Morning Mist', game_version: '1.21.1', loader: 'fabric' },
        instances: [],
        model: null,
        history: [],
        resume: null,
        ...extra,
    }
}

async function play(start: (emit: (event: Event) => void) => unknown) {
    const events: Event[] = []
    const run = newRun()
    start((event) => {
        events.push(event)
        applyEvent(run, event)
    })
    await vi.runAllTimersAsync()
    return { events, run }
}

it('groups tool calls, streams the answer and waits for approval on a plan', () => {
    const run = newRun()
    applyEvent(run, { type: 'status', label: 'Reading…' })
    expect(run.status).toBe('running')
    applyEvent(run, {
        type: 'step',
        step: { id: 'a', title: 'Read', tool: 'logs_read' },
        status: 'running',
    })
    applyEvent(run, { type: 'step', step: { id: 'a', detail: 'ok' }, status: 'done' })
    applyEvent(run, { type: 'step', step: { id: 'b', title: 'Check' }, status: 'done' })
    applyEvent(run, { type: 'text', delta: 'The ' })
    applyEvent(run, { type: 'text', delta: 'answer' })
    applyEvent(run, { type: 'plan', plan: { title: 'Plan', changes: [{ name: 'Sodium' }] } })
    applyEvent(run, { type: 'done' })

    expect(run.items.map((item: { type: string }) => item.type)).toEqual([
        'activity',
        'text',
        'plan',
    ])
    expect(run.items[0].steps).toMatchObject([
        { id: 'a', title: 'Read', detail: 'ok', tool: 'logs_read', status: 'done' },
        { id: 'b', title: 'Check', status: 'done' },
    ])
    expect(run.items[0].endedAt).toEqual(expect.any(Number))
    expect(run.items[1].text).toBe('The answer')
    expect(run.status).toBe('awaiting')
    expect(pendingPlan(run)).toBe(run.items[2])

    expect(beginApply(run)).toBe(true)
    expect(beginApply(run)).toBe(false)
    applyEvent(run, { type: 'applied', done: 1 })
    applyEvent(run, { type: 'result', title: 'Done', text: 'ok' })
    applyEvent(run, { type: 'done' })
    expect(run.status).toBe('done')
    expect(run.items[2]).toMatchObject({ state: 'applied', done: 1 })
    expect(run.items.slice(-1)[0]).toMatchObject({ type: 'result', title: 'Done' })
})

it('marks unfinished steps and the run as failed on an error', () => {
    const run = newRun()
    applyEvent(run, { type: 'step', step: { id: 'a', title: 'Read' }, status: 'running' })
    applyEvent(run, { type: 'error', title: 'Timed out', text: 'Nothing changed' })
    expect(run.status).toBe('failed')
    expect(run.items[0].steps[0].status).toBe('failed')
    expect(run.items.slice(-1)[0]).toMatchObject({ type: 'error', retryable: true })
    applyEvent(run, { type: 'done' })
    expect(run.status).toBe('failed')
})

it('separates a harmless stop from one that interrupts writing', () => {
    const reading = newRun()
    applyEvent(reading, { type: 'step', step: { id: 'a', title: 'Read' }, status: 'running' })
    expect(stopRun(reading)).toBe(true)
    expect(reading.status).toBe('stopped')
    expect(reading.items[0].steps[0].status).toBe('stopped')
    expect(stopRun(reading)).toBe(false)

    const writing = newRun()
    applyEvent(writing, { type: 'plan', plan: { title: 'Plan', changes: [{ name: 'Sodium' }] } })
    beginApply(writing)
    applyEvent(writing, { type: 'step', step: { id: 'w', title: 'Write' }, status: 'running' })
    expect(stopRun(writing)).toBe(true)
    expect(writing.status).toBe('detached')
    expect(writing.items[0].state).toBe('unknown')
})

it('only declines or replaces a plan that is still waiting', () => {
    const run = newRun()
    expect(declinePlan(run)).toBe(false)
    applyEvent(run, { type: 'plan', plan: { title: 'Plan', changes: [] } })
    const copy = structuredClone(run)
    expect(declinePlan(run)).toBe(true)
    expect(run).toMatchObject({ status: 'declined', items: [{ state: 'declined' }] })
    expect(supersedePlan(run)).toBe(false)
    expect(supersedePlan(copy)).toBe(true)
    expect(copy).toMatchObject({ status: 'superseded', items: [{ state: 'superseded' }] })
})

async function registryTools() {
    const directory = new URL(
        '../../../../../packages/launcher-tools/src/registry/',
        import.meta.url,
    )
    const sources = await Promise.all([
        readFile(new URL('../registry.rs', directory), 'utf8'),
        ...(await readdir(directory)).map((name) => readFile(new URL(name, directory), 'utf8')),
    ])
    const registry = new Set(
        sources.flatMap((source) =>
            [...source.matchAll(/entry\(\s*"([a-z0-9_]+)"/g)].map((match) => match[1]),
        ),
    )
    expect(registry.size).toBeGreaterThan(100)
    return registry
}

it('only names tools that exist in the launcher tool registry', async () => {
    const registry = await registryTools()
    const flows = await readFile(
        new URL('../../../../../packages/agent-window/src/demo-flows.js', import.meta.url),
        'utf8',
    )
    const used = new Set([...flows.matchAll(/'([a-z]+(?:_[a-z]+)+)'/g)].map((match) => match[1]))
    expect(used.size).toBeGreaterThan(10)
    for (const tool of used) expect(registry, tool).toContain(tool)
})

it('puts every tool in the launcher tool registry in a family with its own sentence', async () => {
    // Without a catalog, text() returns the key, so the result names the sentence.
    for (const tool of await registryTools()) {
        expect(toolFamily(tool), tool).toMatch(/^toolFamily[A-Z]/)
        expect(toolHelp(tool), tool).not.toBe('toolGeneric')
    }
    expect(toolHelp('server_entry_list')).toBe('toolFamilyServerList')
    expect(toolHelp('server_start')).toBe('toolFamilyServer')
    expect(toolHelp('constructor')).toBe('toolGeneric')
})

it('keeps every scripted task available in both languages with the same tools', () => {
    for (const flow of FLOWS) {
        const tools = (script: { reads: unknown[][]; writes: unknown[][] }) =>
            [...script.reads, ...script.writes].map((entry) => entry[0])
        expect(tools(flow.zh), flow.id).toEqual(tools(flow.en))
        expect(flow.zh.plan.changes).toHaveLength(flow.en.plan.changes.length)
        expect(flow.zh.progress).toHaveLength(3)
    }
    expect(GENERIC.zh.reads).toHaveLength(GENERIC.en.reads.length)
})

it('plays a scripted task from first read to an applied plan', async () => {
    const runtime = new DemoRuntime()
    expect(runtime.preview).toBe(true)
    const { events, run } = await play((emit) =>
        runtime.start(request('My game crashes on launch'), emit),
    )
    expect(events[0]).toMatchObject({ type: 'status' })
    expect(events).toContainEqual({ type: 'title', title: 'Fix the launch crash' })
    const tools = events
        .filter((event) => event.type === 'step')
        .map((event) => (event.step as { tool: string }).tool)
    expect(tools).toContain('logs_read')
    expect(run.status).toBe('awaiting')
    expect(run.items.find((item: { type: string }) => item.type === 'text')!.text).toContain(
        'Entity Culling',
    )
    expect(run.resume).toMatchObject({ flow: 'crash', vars: { instance: 'Morning Mist' } })

    beginApply(run)
    await play((emit) => {
        runtime.approve(request('', { resume: run.resume }), (event: Event) => {
            emit(event)
            applyEvent(run, event)
        })
    })
    expect(run.status).toBe('done')
    expect(run.items.find((item: { type: string }) => item.type === 'plan')).toMatchObject({
        state: 'applied',
        done: 1,
    })
    expect(run.items.slice(-1)[0]).toMatchObject({ type: 'result', title: 'Rolled back' })
    expect(run.progress.every((step: { status: string }) => step.status === 'done')).toBe(true)
})

it('answers in the window language and borrows an instance only when one is needed', async () => {
    setLocale('zh-CN')
    const runtime = new DemoRuntime()
    const { events, run } = await play((emit) =>
        runtime.start(
            request('帮我备份世界', {
                instance: null,
                instances: [
                    { id: 'v', name: '原版生存', game_version: '1.21.4', loader: 'vanilla' },
                ],
            }),
            emit,
        ),
    )
    const steps = events.filter((event) => event.type === 'step' && event.status === 'done')
    expect((steps[0].step as { detail: string }).detail).toContain('原版生存')
    expect(run.items.find((item: { type: string }) => item.type === 'plan')!.plan.title).toBe(
        '山间小屋 · 世界备份',
    )

    const generic = await play((emit) =>
        runtime.start(request('讲个笑话', { instance: null }), emit),
    )
    expect(generic.run.status).toBe('done')
    expect(generic.run.items.some((item: { type: string }) => item.type === 'plan')).toBe(false)
    expect(generic.events.some((event) => event.type === 'title')).toBe(false)
})

it('says a chosen instance has gone instead of picking one as if none was chosen', async () => {
    const runtime = new DemoRuntime()
    const instances = [
        { id: 'v', name: 'Vanilla Survival', game_version: '1.21.4', loader: 'vanilla' },
    ]
    const firstStep = async (scope: string) => {
        const { events } = await play((emit) =>
            runtime.start(request('Back up my world', { instance: null, scope, instances }), emit),
        )
        return events.find((event) => event.type === 'step' && event.status === 'done')!.step as {
            title: string
            detail: string
        }
    }
    const missing = await firstStep('missing')
    expect(missing.title).toBe('Look for the chosen instance')
    expect(missing.detail).toContain('Vanilla Survival')
    expect(missing.detail).toContain('not in your list')
    expect((await firstStep('launcher')).title).toBe('Pick an instance')
})

it('stops without emitting anything further and can fail on request', async () => {
    const runtime = new DemoRuntime({ fail: 1 })
    const events: Event[] = []
    const handle = runtime.start(request('make it smoother'), (event: Event) => events.push(event))
    await vi.advanceTimersByTimeAsync(1500)
    handle.stop()
    const count = events.length
    await vi.runAllTimersAsync()
    expect(events).toHaveLength(count)

    const failed = await play((emit) => runtime.start(request('make it smoother'), emit))
    expect(failed.run.status).toBe('failed')
    expect(failed.run.items.slice(-1)[0]).toMatchObject({ type: 'error', retryable: true })
    const retried = await play((emit) => runtime.start(request('make it smoother'), emit))
    expect(retried.run.status).toBe('awaiting')
})

it('can fail on request right after proposing a plan, once', async () => {
    const runtime = new DemoRuntime({ fail: 'plan' })
    const failed = await play((emit) => runtime.start(request('make it smoother'), emit))
    expect(failed.events.slice(-2).map((event) => event.type)).toEqual(['plan', 'error'])
    expect(failed.run.status).toBe('failed')
    expect(failed.run.items.find((item) => item.type === 'plan')?.state).toBe('stopped')
    expect(pendingPlan(failed.run)).toBeNull()
    const retried = await play((emit) => runtime.start(request('make it smoother'), emit))
    expect(retried.run.status).toBe('awaiting')
})

it('re-proposes the same plan when the user replies to it', async () => {
    const runtime = new DemoRuntime()
    const first = await play((emit) => runtime.start(request('fps is low'), emit))
    supersedePlan(first.run)
    const reply = await play((emit) =>
        runtime.start(request('why these two?', { history: [{ run: first.run }] }), emit),
    )
    expect(reply.events.some((event) => event.type === 'title')).toBe(false)
    expect(reply.run.status).toBe('awaiting')
    expect(reply.run.items.find((item: { type: string }) => item.type === 'text')!.text).toContain(
        'rechecked',
    )
})

it('lets changes be taken out of a waiting plan, never all of them and never manual ones', () => {
    const run = newRun()
    applyEvent(run, {
        type: 'plan',
        plan: {
            title: 'Plan',
            changes: [
                { name: 'Sodium' },
                { name: 'Lithium' },
                { name: 'Render distance', manual: true },
            ],
        },
    })
    const plan = pendingPlan(run)
    expect(toggleChange(run, 2)).toBe(false)
    expect(toggleChange(run, 1)).toBe(true)
    expect(includedChanges(plan).map((change: { name: string }) => change.name)).toEqual(['Sodium'])
    expect(toggleChange(run, 0)).toBe(true)
    expect(beginApply(run)).toBe(false)
    expect(toggleChange(run, 1)).toBe(true)
    expect(plan.excluded).toEqual([0])
    expect(beginApply(run)).toBe(true)
    expect(toggleChange(run, 0)).toBe(false)
})

it('tells the demo which changes to skip and reports them', async () => {
    const runtime = new DemoRuntime()
    const first = await play((emit) => runtime.start(request('fps is low'), emit))
    const plan = pendingPlan(first.run)
    toggleChange(first.run, 1)
    beginApply(first.run)
    const names = plan.excluded.map((index: number) => plan.plan.changes[index].name)
    await play((emit) =>
        runtime.approve(
            request('', { resume: first.run.resume, excluded: names }),
            (event: Event) => {
                emit(event)
                applyEvent(first.run, event)
            },
        ),
    )
    expect(plan).toMatchObject({ state: 'applied', done: 1 })
    expect(first.run.items.slice(-1)[0].text).toContain('Lithium')
})

it('splits streamed Markdown into blocks, including an unfinished code fence', () => {
    const source = [
        '# Title',
        '',
        'Some **bold** text',
        'wrapped',
        '',
        '1. one',
        '2. two',
        '',
        '- a',
        '- b',
        '',
        '> quote',
        '',
        '---',
        '',
        '```log',
        'line one',
        '',
        'line two',
    ].join('\n')
    const blocks = markdownBlocks(source)
    expect(blocks.map((block: { type: string }) => block.type)).toEqual([
        'heading',
        'p',
        'ol',
        'ul',
        'quote',
        'rule',
        'code',
    ])
    expect(blocks[1].text).toBe('Some **bold** text\nwrapped')
    expect(blocks[2].items).toEqual(['one', 'two'])
    expect(blocks[6]).toMatchObject({ lang: 'log', text: 'line one\n\nline two' })
})

it('parses streamed Markdown again only from its last unsettled block, to the same blocks', () => {
    // Streamed a few characters at a time, these lines change blocks before
    // the last one: "-" and "2" join the list above once their marker is
    // complete, a heading needs its space, "--" becomes a rule, blank lines
    // gain text, and fences open, get their language and close.
    const source = [
        '#',
        '# Title',
        'Some **bold** text',
        'wrapped',
        '- a',
        '-',
        '- b',
        '',
        '1. one',
        '2) two',
        '10. ten',
        '',
        '> quote',
        '>',
        '> more',
        '',
        '---',
        '   ',
        '***',
        '```log',
        'line one',
        '',
        '\tat a.b.C.d(C.java:1)',
        '```',
        'after the fence',
        '  ```js',
        '  let x',
        '  ```',
        '```',
        'still open',
        '',
        '  x',
    ].join('\n')
    for (const sizes of [[1], [3, 7, 1, 12], [20, 5, 9]]) {
        let state = null
        let reused = 0
        for (let at = 0, step = 0; at < source.length; step++) {
            at = Math.min(source.length, at + sizes[step % sizes.length])
            const prefix = source.slice(0, at)
            state = parseMarkdown(prefix, state)
            expect(state.blocks).toEqual(markdownBlocks(prefix))
            reused = Math.max(reused, state.kept)
        }
        // Most blocks are taken over rather than parsed again.
        expect(reused).toBeGreaterThan(8)
    }
    // Text that is not a continuation is parsed from the start.
    const before = parseMarkdown('- a\n- b')
    const other = parseMarkdown('- a\n+ b', before)
    expect(other.kept).toBe(0)
    expect(other.blocks).toEqual(markdownBlocks('- a\n+ b'))
})

it('voids a waiting plan when the run fails and only approves a waiting run', () => {
    const run = newRun()
    applyEvent(run, {
        type: 'plan',
        plan: { title: 'Fix', changes: [{ name: 'Sodium', kind: 'update', from: '1', to: '2' }] },
    })
    expect(pendingPlan(run)).not.toBeNull()
    applyEvent(run, { type: 'error', title: 'Lost connection', text: '' })
    expect(run.status).toBe('failed')
    expect(run.items.find((item) => item.type === 'plan').state).toBe('stopped')
    expect(pendingPlan(run)).toBeNull()
    expect(beginApply(run)).toBe(false)
})

it('parses headings that end in a carriage return or a line separator', () => {
    for (const end of ['\r', '\u2028', '\u2029']) {
        const blocks = markdownBlocks(`# Title${end}\nBody`)
        expect(blocks.map((block) => block.type)).toEqual(['heading', 'p'])
    }
})

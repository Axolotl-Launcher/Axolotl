import { readdir, readFile } from 'node:fs/promises'

import { afterEach, beforeEach, expect, it, vi } from 'vitest'

import {
    applyEvent,
    beginApply,
    declinePlan,
    pendingPlan,
    stopRun,
    supersedePlan,
} from '../../../../../packages/agent-window/src/agent.js'
import { FLOWS, GENERIC } from '../../../../../packages/agent-window/src/demo-flows.js'
import { DemoRuntime } from '../../../../../packages/agent-window/src/demo-runtime.js'
import { setLocale } from '../../../../../packages/agent-window/src/messages.js'
import { newRun } from '../../../../../packages/agent-window/src/state.js'

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
    expect(run.items.at(-1)).toMatchObject({ type: 'result', title: 'Done' })
})

it('marks unfinished steps and the run as failed on an error', () => {
    const run = newRun()
    applyEvent(run, { type: 'step', step: { id: 'a', title: 'Read' }, status: 'running' })
    applyEvent(run, { type: 'error', title: 'Timed out', text: 'Nothing changed' })
    expect(run.status).toBe('failed')
    expect(run.items[0].steps[0].status).toBe('failed')
    expect(run.items.at(-1)).toMatchObject({ type: 'error', retryable: true })
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
    applyEvent(writing, { type: 'plan', plan: { title: 'Plan', changes: [] } })
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

it('only names tools that exist in the launcher tool registry', async () => {
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
    const flows = await readFile(
        new URL('../../../../../packages/agent-window/src/demo-flows.js', import.meta.url),
        'utf8',
    )
    const used = new Set([...flows.matchAll(/'([a-z]+(?:_[a-z]+)+)'/g)].map((match) => match[1]))
    expect(used.size).toBeGreaterThan(10)
    for (const tool of used) expect(registry, tool).toContain(tool)
})

it('keeps every scripted task available in both languages with the same tools', () => {
    for (const flow of FLOWS) {
        const tools = (script: { reads: string[][]; writes: string[][] }) =>
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
    expect(run.items.find((item: { type: string }) => item.type === 'text').text).toContain(
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
    expect(run.items.at(-1)).toMatchObject({ type: 'result', title: 'Rolled back' })
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
    expect(run.items.find((item: { type: string }) => item.type === 'plan').plan.title).toBe(
        '山间小屋 · 世界备份',
    )

    const generic = await play((emit) =>
        runtime.start(request('讲个笑话', { instance: null }), emit),
    )
    expect(generic.run.status).toBe('done')
    expect(generic.run.items.some((item: { type: string }) => item.type === 'plan')).toBe(false)
    expect(generic.events.some((event) => event.type === 'title')).toBe(false)
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
    expect(failed.run.items.at(-1)).toMatchObject({ type: 'error', retryable: true })
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
    expect(reply.run.items.find((item: { type: string }) => item.type === 'text').text).toContain(
        'rechecked',
    )
})

import { expect, it } from 'vitest'

import {
    countModTranslationJobs,
    jobFromSnapshot,
    modTranslationPercent,
    reduceModTranslationJob,
    replayContiguousTaskEvents,
} from './job-state.ts'
import { mapTaskEventsToTimeline } from './timeline.ts'
import type {
    ModTranslationProgress,
    ModTranslationTaskEvent,
    ModTranslationTaskSnapshot,
} from './types.ts'

function progress(overrides: Partial<ModTranslationProgress> = {}): ModTranslationProgress {
    return {
        taskId: 'TASK-1',
        phase: 'language',
        message: 'translated',
        completed: 10,
        total: 100,
        weightVerified: 10,
        weightTotal: 100,
        level: 'info',
        finished: false,
        ok: false,
        ...overrides,
    }
}

function event(
    sequence: number,
    overrides: Partial<ModTranslationTaskEvent> = {},
): ModTranslationTaskEvent {
    return {
        eventId: `EVENT-${sequence}`,
        taskId: 'TASK-1',
        sequence,
        occurredAt: `2026-08-07T00:00:0${sequence}Z`,
        eventType: 'progress',
        status: 'running',
        progress: progress(),
        ...overrides,
    }
}

function snapshot(events: ModTranslationTaskEvent[] = []): ModTranslationTaskSnapshot {
    return {
        taskId: 'TASK-1',
        inputPath: 'C:/mods/demo.jar',
        outputPath: 'C:/mods/demo-zh_cn.jar',
        inputHash: 'abc',
        startedAt: '2026-08-07T00:00:00Z',
        updatedAt: '2026-08-07T00:00:00Z',
        status: 'running',
        sequence: events.at(-1)?.sequence ?? 0,
        activities: [],
        events,
    }
}

it('percent uses verified weight and forces successful completion to 100', () => {
    expect(modTranslationPercent(progress({ weightVerified: 50, weightTotal: 100 }))).toBe(50)
    expect(modTranslationPercent(progress({ weightVerified: 100, weightTotal: 100 }))).toBe(99)
    expect(
        modTranslationPercent(
            progress({ weightVerified: 20, weightTotal: 100, finished: true, ok: true }),
        ),
    ).toBe(100)
})

it('snapshot replays ordered events', () => {
    const job = jobFromSnapshot(
        snapshot([
            event(1, { progress: progress({ phase: 'prepare', message: 'prepare' }) }),
            event(2, { progress: progress({ phase: 'language', message: 'language' }) }),
        ]),
    )
    expect(job.lastSequence).toBe(2)
    expect(job.phase).toBe('language')
    expect(job.timeline.length).toBe(2)
})

it('duplicate and stale events are ignored', () => {
    const initial = jobFromSnapshot(snapshot([event(1)]))
    expect(reduceModTranslationJob(initial, event(1))).toBe(initial)
    expect(reduceModTranslationJob(initial, event(0))).toBe(initial)
})

it('out-of-order events wait for the missing sequence and then replay in order', () => {
    const initial = jobFromSnapshot(snapshot())
    const first = replayContiguousTaskEvents(initial, [
        event(2, { progress: progress({ message: 'second' }) }),
    ])
    expect(first.job.lastSequence).toBe(0)
    expect(first.pending.length).toBe(1)
    const second = replayContiguousTaskEvents(first.job, [
        ...first.pending,
        event(1, { progress: progress({ message: 'first' }) }),
    ])
    expect(second.job.lastSequence).toBe(2)
    expect(second.job.message).toBe('second')
    expect(second.pending).toEqual([])
})

it('phase never regresses', () => {
    const initial = jobFromSnapshot(
        snapshot([event(1, { progress: progress({ phase: 'validation' }) })]),
    )
    const next = reduceModTranslationJob(
        initial,
        event(2, { progress: progress({ phase: 'language' }) }),
    )
    expect(next.phase).toBe('validation')
})

it('zero-weight phase events preserve the last measurable progress', () => {
    const initial = jobFromSnapshot(
        snapshot([
            event(1, {
                progress: progress({ completed: 6, total: 21, weightVerified: 6, weightTotal: 25 }),
            }),
        ]),
    )
    const next = reduceModTranslationJob(
        initial,
        event(2, {
            progress: progress({
                phase: 'repair',
                message: 'starting validation',
                completed: 0,
                total: 0,
                weightVerified: 0,
                weightTotal: 0,
            }),
        }),
    )
    expect(next.percent).toBe(24)
    expect(next.completed).toBe(6)
    expect(next.total).toBe(21)
    expect(next.weightVerified).toBe(6)
    expect(next.weightTotal).toBe(25)
})

it('job counts distinguish failed tasks from successful completion', () => {
    expect(
        countModTranslationJobs([
            { status: 'completed' },
            { status: 'failed' },
            { status: 'running' },
        ]),
    ).toEqual({ running: 1, completed: 1, failed: 1 })
})

it('failed packaging signal preserves the actual working phase and error', () => {
    const initial = jobFromSnapshot(
        snapshot([event(1, { progress: progress({ phase: 'repair' }) })]),
    )
    const next = reduceModTranslationJob(
        initial,
        event(2, {
            status: 'failed',
            progress: progress({ phase: 'packaging', finished: true, level: 'error' }),
            error: { code: 'UNSUPPORTED_RESOURCE', message: 'unsupported path' },
        }),
    )
    expect(next.phase).toBe('repair')
    expect(next.error?.code).toBe('UNSUPPORTED_RESOURCE')
})

it('successful task is always shown as 100 percent', () => {
    const next = reduceModTranslationJob(
        jobFromSnapshot(snapshot()),
        event(1, {
            status: 'completed',
            progress: progress({ finished: true, ok: true, weightVerified: 1, weightTotal: 10 }),
        }),
    )
    expect(next.percent).toBe(100)
})

it('timeline groups typed repair activity without exposing raw JSON as the title', () => {
    const entries = mapTaskEventsToTimeline([
        event(1, {
            eventType: 'activity',
            progress: null,
            activity: {
                taskId: 'TASK-1',
                pass: 1,
                kind: 'diagnosis',
                status: 'running',
                title: '发现 16 个疑难项',
                summary: '正在批量处理',
                count: 16,
                issueIds: ['a', 'b'],
                debug: { request: { entries: [] } },
            },
        }),
    ])
    expect(entries[0].title).toBe('发现 16 个疑难项')
    expect(entries[0].pass).toBe(1)
    expect(entries[0].debug).toEqual({ request: { entries: [] } })
})

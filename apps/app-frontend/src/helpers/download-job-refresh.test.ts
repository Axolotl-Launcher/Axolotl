import { expect, it } from 'vitest'

import {
    isRegressiveActiveJobSnapshot,
    mergeRefreshedDownloadJobs,
    reconcileAcceptedDownloadJob,
} from './download-job-refresh.ts'

const active = new Set(['queued', 'running', 'canceling', 'waiting_for_user'])
const job = (job_id: string, status: string, progress: number) => ({
    job_id,
    status,
    progress,
    created: '2026-09-12T00:00:00Z',
    modified: '2026-09-12T00:00:00Z',
})

it('accepted retry and resume snapshots survive a failed supplemental read, while actual absence removes them', async () => {
    const accepted = job('a', 'running', 10)
    const published: (typeof accepted)[] = [],
        removed: string[] = [],
        errors: unknown[] = []
    await reconcileAcceptedDownloadJob(
        accepted,
        async () => {
            throw new Error('IPC unavailable')
        },
        (job) => published.push(job),
        (id) => removed.push(id),
        (error) => errors.push(error),
    )
    expect(published).toEqual([accepted])
    expect(removed).toEqual([])
    expect(errors.length).toBe(1)
    await reconcileAcceptedDownloadJob(
        accepted,
        async () => null,
        (job) => published.push(job),
        (id) => removed.push(id),
        (error) => errors.push(error),
    )
    expect(removed).toEqual(['a'])
})

it('does not regress an active job to a same-timestamp queued snapshot', () => {
    const running = job('a', 'running', 20)
    const queued = job('a', 'queued', 0)
    expect(isRegressiveActiveJobSnapshot(running, queued, active)).toBe(true)
})

it('preserves same-second terminal events received before an accepted retry response when the read fails', async () => {
    const terminal = job('a', 'succeeded', 100)
    let current = terminal
    const errors: unknown[] = []
    await reconcileAcceptedDownloadJob(
        job('a', 'queued', 0),
        async () => {
            throw new Error('read failed')
        },
        (value) => {
            current = value
        },
        () => expect.fail('must not remove the live job'),
        (error) => errors.push(error),
        { atDispatch: 1, current: () => 2 },
    )
    expect(current).toBe(terminal)
    expect(errors.length).toBe(1)
})

it('a supplemental read cannot replace events received while it was in flight', async () => {
    let revision = 1
    let current = job('a', 'queued', 0)
    await reconcileAcceptedDownloadJob(
        current,
        async () => {
            current = job('a', 'succeeded', 100)
            revision++
            return job('a', 'running', 20)
        },
        (value) => {
            current = value
            revision++
        },
        () => expect.fail('must not remove the live job'),
        () => expect.fail('read succeeded'),
        { atDispatch: 1, current: () => revision },
    )
    expect(current.status).toBe('succeeded')
})

it('rejects same-second active snapshots after a terminal event', () => {
    const succeeded = job('a', 'succeeded', 100)
    const queued = job('a', 'queued', 0)
    expect(isRegressiveActiveJobSnapshot(succeeded, queued, active)).toBe(true)
    expect(isRegressiveActiveJobSnapshot(succeeded, job('a', 'running', 20), active)).toBe(true)
})

it('preserves realtime progress received while an active refresh was in flight', () => {
    const stale = job('a', 'running', 10)
    const live = job('a', 'running', 20)
    const merged = mergeRefreshedDownloadJobs(
        [stale],
        [live],
        new Map([['a', 1]]),
        new Map([['a', 2]]),
        new Set(),
        active,
    )
    expect(merged[0]).toBe(live)
})

it('accepts an authoritative terminal refresh result when nothing changed since dispatch', () => {
    const live = job('a', 'running', 20)
    const succeeded = job('a', 'succeeded', 100)
    const merged = mergeRefreshedDownloadJobs(
        [succeeded],
        [live],
        new Map([['a', 1]]),
        new Map([['a', 1]]),
        new Set(),
        active,
    )
    expect(merged[0]).toBe(succeeded)
})

it('does not overwrite a newly retried job with an older terminal list response', () => {
    const running = job('a', 'running', 10)
    const merged = mergeRefreshedDownloadJobs(
        [job('a', 'failed', 0)],
        [running],
        new Map([['a', 1]]),
        new Map([['a', 2]]),
        new Set(),
        active,
    )
    expect(merged[0]).toBe(running)
})

it('keeps a newly announced active job missing from the older response', () => {
    const announced = job('new', 'running', 1)
    const merged = mergeRefreshedDownloadJobs(
        [],
        [announced],
        new Map(),
        new Map([['new', 1]]),
        new Set(),
        active,
    )
    expect(merged).toEqual([announced])
})

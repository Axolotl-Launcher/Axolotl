import assert from 'node:assert/strict'
import test from 'node:test'

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

test('accepted retry and resume snapshots survive a failed supplemental read, while actual absence removes them', async () => {
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
    assert.deepEqual(published, [accepted])
    assert.deepEqual(removed, [])
    assert.equal(errors.length, 1)
    await reconcileAcceptedDownloadJob(
        accepted,
        async () => null,
        (job) => published.push(job),
        (id) => removed.push(id),
        (error) => errors.push(error),
    )
    assert.deepEqual(removed, ['a'])
})

test('does not regress an active job to a same-timestamp queued snapshot', () => {
    const running = job('a', 'running', 20)
    const queued = job('a', 'queued', 0)
    assert.equal(isRegressiveActiveJobSnapshot(running, queued, active), true)
})

test('preserves same-second terminal events received before an accepted retry response when the read fails', async () => {
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
        () => assert.fail('must not remove the live job'),
        (error) => errors.push(error),
        { atDispatch: 1, current: () => 2 },
    )
    assert.equal(current, terminal)
    assert.equal(errors.length, 1)
})

test('a supplemental read cannot replace events received while it was in flight', async () => {
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
        () => assert.fail('must not remove the live job'),
        () => assert.fail('read succeeded'),
        { atDispatch: 1, current: () => revision },
    )
    assert.equal(current.status, 'succeeded')
})

test('allows a queued snapshot when the job is not already active', () => {
    const succeeded = job('a', 'succeeded', 100)
    const queued = job('a', 'queued', 0)
    assert.equal(isRegressiveActiveJobSnapshot(succeeded, queued, active), false)
})

test('preserves realtime progress received while an active refresh was in flight', () => {
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
    assert.equal(merged[0], live)
})

test('accepts an authoritative terminal refresh result', () => {
    const live = job('a', 'running', 20)
    const succeeded = job('a', 'succeeded', 100)
    const merged = mergeRefreshedDownloadJobs(
        [succeeded],
        [live],
        new Map([['a', 1]]),
        new Map([['a', 2]]),
        new Set(),
        active,
    )
    assert.equal(merged[0], succeeded)
})

test('keeps a newly announced active job missing from the older response', () => {
    const announced = job('new', 'running', 1)
    const merged = mergeRefreshedDownloadJobs(
        [],
        [announced],
        new Map(),
        new Map([['new', 1]]),
        new Set(),
        active,
    )
    assert.deepEqual(merged, [announced])
})

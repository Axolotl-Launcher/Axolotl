import assert from 'node:assert/strict'
import test from 'node:test'

import {
    createDirectLinkSyncController,
    directLinkSyncChanged,
    startDirectLinkSyncOnFocus,
} from './direct-link-sync-controller.ts'
import type { DirectLinkSyncReport, ExternalMinecraftRoot } from './instance'

const report = (changes: Partial<DirectLinkSyncReport> = {}): DirectLinkSyncReport => ({
    imported: 0,
    updated: 0,
    removed: 0,
    missing: 0,
    errors: [],
    ...changes,
})
const roots = (path: string): ExternalMinecraftRoot[] => [{ path, mode: 'isolated' }]
const deferred = <T>() => {
    let resolve!: (value: T) => void
    const promise = new Promise<T>((done) => {
        resolve = done
    })
    return { promise, resolve }
}

test('focus during an existing scan shares its result without a trailing duplicate', async () => {
    const response = deferred<DirectLinkSyncReport>()
    let calls = 0
    const sync = createDirectLinkSyncController(
        () => {
            calls++
            return response.promise
        },
        () => {},
    )
    const first = sync(roots('a'), true)
    await Promise.resolve()
    const focused = sync(roots('a'), true)
    assert.equal(focused, first)
    response.resolve(report())
    await focused
    assert.equal(calls, 1)

    await sync(roots('a'), true)
    assert.equal(calls, 2)
})

test('configuration changes during a scan serialize the latest roots, including removal of all roots', async () => {
    const response = deferred<DirectLinkSyncReport>()
    const scanned: ExternalMinecraftRoot[][] = []
    const published: DirectLinkSyncReport[] = []
    const sync = createDirectLinkSyncController(
        async (value) => {
            scanned.push(value)
            return scanned.length === 1 ? response.promise : report({ removed: 1 })
        },
        (value) => published.push(value),
    )
    const original = roots('a')
    const first = sync(original)
    original[0].path = 'mutated'
    await Promise.resolve()
    sync(roots('b'))
    const last = sync([])
    assert.equal(last, first)
    response.resolve(report({ imported: 1 }))
    assert.equal((await last).removed, 1)
    assert.deepEqual(scanned, [roots('a'), []])
    assert.deepEqual(
        published.map((value) => [value.imported, value.removed]),
        [
            [1, 0],
            [0, 1],
        ],
    )
})

test('explicit same-root requests still reconcile changes made during an active scan', async () => {
    const response = deferred<DirectLinkSyncReport>()
    let calls = 0
    const sync = createDirectLinkSyncController(
        async () => {
            calls++
            return calls === 1 ? response.promise : report()
        },
        () => {},
    )
    const first = sync(roots('a'), true)
    await Promise.resolve()
    sync(roots('a'))
    response.resolve(report())
    await first
    assert.equal(calls, 2)
})

test('a request queued as the previous scan settles is not lost', async () => {
    const response = deferred<DirectLinkSyncReport>()
    const scanned: string[] = []
    const sync = createDirectLinkSyncController(
        async (value) => {
            scanned.push(value[0].path)
            return scanned.length === 1 ? response.promise : report()
        },
        () => {},
    )
    const first = sync(roots('a'))
    await Promise.resolve()
    response.resolve(report())
    let last!: Promise<DirectLinkSyncReport>
    queueMicrotask(() => {
        last = sync(roots('b'))
    })
    await first
    await last
    assert.deepEqual(scanned, ['a', 'b'])
})

test('failed scans release the worker so the next focus can retry', async () => {
    let calls = 0
    const sync = createDirectLinkSyncController(
        async () => {
            if (++calls === 1) throw new Error('scan failed')
            return report()
        },
        () => {},
    )
    await assert.rejects(sync(roots('a'), true), /scan failed/)
    await sync(roots('a'), true)
    assert.equal(calls, 2)
})

test('only instance changes require a list refresh; missing roots and errors remain completion reports', () => {
    assert.equal(directLinkSyncChanged(report()), false)
    assert.equal(directLinkSyncChanged(report({ missing: 1, errors: ['unavailable'] })), false)
    for (const change of [{ imported: 1 }, { updated: 1 }, { removed: 1 }]) {
        assert.equal(directLinkSyncChanged(report(change)), true)
    }
})

test('scans on startup and settled focus, coalesces focus bursts, and cancels on blur or teardown', (t) => {
    t.mock.timers.enable({ apis: ['setTimeout'] })
    const target = new EventTarget()
    let calls = 0
    const stop = startDirectLinkSyncOnFocus(
        async () => {
            calls++
        },
        assert.fail,
        target,
    )
    t.after(stop)
    assert.equal(calls, 1)
    target.dispatchEvent(new Event('focus'))
    t.mock.timers.tick(200)
    target.dispatchEvent(new Event('focus'))
    t.mock.timers.tick(299)
    assert.equal(calls, 1)
    t.mock.timers.tick(1)
    assert.equal(calls, 2)

    target.dispatchEvent(new Event('focus'))
    target.dispatchEvent(new Event('blur'))
    t.mock.timers.tick(300)
    assert.equal(calls, 2)

    target.dispatchEvent(new Event('focus'))
    stop()
    t.mock.timers.tick(300)
    target.dispatchEvent(new Event('focus'))
    t.mock.timers.tick(300)
    assert.equal(calls, 2)
})

test('a later focus reads current configuration and recovers after a background scan error', async (t) => {
    t.mock.timers.enable({ apis: ['setTimeout'] })
    const target = new EventTarget()
    const errors: unknown[] = []
    const scanned: string[] = []
    let configuration = 'a'
    const stop = startDirectLinkSyncOnFocus(
        async () => {
            scanned.push(configuration)
            if (configuration === 'a') throw new Error('unavailable')
        },
        (error) => errors.push(error),
        target,
    )
    t.after(stop)
    await Promise.resolve()
    assert.equal(errors.length, 1)
    configuration = 'b'
    target.dispatchEvent(new Event('focus'))
    t.mock.timers.tick(300)
    await Promise.resolve()
    assert.deepEqual(scanned, ['a', 'b'])
    assert.equal(errors.length, 1)
})

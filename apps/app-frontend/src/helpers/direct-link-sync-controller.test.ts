import { afterEach, expect, it, onTestFinished, vi } from 'vitest'

import {
    createDirectLinkSyncController,
    directLinkSyncChanged,
    startDirectLinkSyncOnFocus,
} from './direct-link-sync-controller.ts'
import type { DirectLinkSyncReport, ExternalMinecraftRoot } from './instance'

afterEach(() => {
    vi.useRealTimers()
})

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

it('focus during an existing scan shares its result without a trailing duplicate', async () => {
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
    expect(focused).toBe(first)
    response.resolve(report())
    await focused
    expect(calls).toBe(1)

    await sync(roots('a'), true)
    expect(calls).toBe(2)
})

it('configuration changes during a scan serialize the latest roots, including removal of all roots', async () => {
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
    expect(last).toBe(first)
    response.resolve(report({ imported: 1 }))
    expect((await last).removed).toBe(1)
    expect(scanned).toEqual([roots('a'), []])
    expect(published.map((value) => [value.imported, value.removed])).toEqual([
        [1, 0],
        [0, 1],
    ])
})

it('explicit same-root requests still reconcile changes made during an active scan', async () => {
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
    expect(calls).toBe(2)
})

it('a request queued as the previous scan settles is not lost', async () => {
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
    expect(scanned).toEqual(['a', 'b'])
})

it('failed scans release the worker so the next focus can retry', async () => {
    let calls = 0
    const sync = createDirectLinkSyncController(
        async () => {
            if (++calls === 1) throw new Error('scan failed')
            return report()
        },
        () => {},
    )
    await expect(sync(roots('a'), true)).rejects.toThrow(/scan failed/)
    await sync(roots('a'), true)
    expect(calls).toBe(2)
})

it('only instance changes require a list refresh; missing roots and errors remain completion reports', () => {
    expect(directLinkSyncChanged(report())).toBe(false)
    expect(directLinkSyncChanged(report({ missing: 1, errors: ['unavailable'] }))).toBe(false)
    for (const change of [{ imported: 1 }, { updated: 1 }, { removed: 1 }]) {
        expect(directLinkSyncChanged(report(change))).toBe(true)
    }
})

it('scans on startup and settled focus, coalesces focus bursts, and cancels on blur or teardown', () => {
    vi.useFakeTimers()
    const target = new EventTarget()
    let calls = 0
    const stop = startDirectLinkSyncOnFocus(
        async () => {
            calls++
        },
        expect.fail,
        target,
    )
    onTestFinished(stop)
    expect(calls).toBe(1)
    target.dispatchEvent(new Event('focus'))
    vi.advanceTimersByTime(200)
    target.dispatchEvent(new Event('focus'))
    vi.advanceTimersByTime(299)
    expect(calls).toBe(1)
    vi.advanceTimersByTime(1)
    expect(calls).toBe(2)

    target.dispatchEvent(new Event('focus'))
    target.dispatchEvent(new Event('blur'))
    vi.advanceTimersByTime(300)
    expect(calls).toBe(2)

    target.dispatchEvent(new Event('focus'))
    stop()
    vi.advanceTimersByTime(300)
    target.dispatchEvent(new Event('focus'))
    vi.advanceTimersByTime(300)
    expect(calls).toBe(2)
})

it('a later focus reads current configuration and recovers after a background scan error', async () => {
    vi.useFakeTimers()
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
    onTestFinished(stop)
    await Promise.resolve()
    expect(errors.length).toBe(1)
    configuration = 'b'
    target.dispatchEvent(new Event('focus'))
    vi.advanceTimersByTime(300)
    await Promise.resolve()
    expect(scanned).toEqual(['a', 'b'])
    expect(errors.length).toBe(1)
})

import assert from 'node:assert/strict'
import test from 'node:test'

import { createContentRefreshCoalescer } from './content-refresh-coalescer.ts'

function deferred() {
    let resolve!: (value: string) => void
    let reject!: (error: Error) => void
    const promise = new Promise<string>((resolvePromise, rejectPromise) => {
        resolve = resolvePromise
        reject = rejectPromise
    })
    return { promise, resolve, reject }
}

test('shares one request between concurrent refreshes of the same instance', async () => {
    const calls: string[] = []
    const pending = deferred()
    const refresh = createContentRefreshCoalescer(async (instanceId: string) => {
        calls.push(instanceId)
        return await pending.promise
    })

    const first = refresh('instance')
    const second = refresh('instance')

    assert.equal(calls.length, 1)
    pending.resolve('snapshot')
    assert.deepEqual(await Promise.all([first, second]), ['snapshot', 'snapshot'])
})

test('starts a new request once the previous refresh settled', async () => {
    let calls = 0
    const refresh = createContentRefreshCoalescer(async () => {
        calls += 1
        return `snapshot-${calls}`
    })

    assert.equal(await refresh('instance'), 'snapshot-1')
    assert.equal(await refresh('instance'), 'snapshot-2')
})

test('does not keep a failed refresh in flight', async () => {
    let calls = 0
    const refresh = createContentRefreshCoalescer(async () => {
        calls += 1
        if (calls === 1) throw new Error('scan failed')
        return 'snapshot'
    })

    await assert.rejects(refresh('instance'), /scan failed/)
    assert.equal(await refresh('instance'), 'snapshot')
})

test('keeps different instances independent', async () => {
    const pending = new Map<string, ReturnType<typeof deferred>>()
    const refresh = createContentRefreshCoalescer((instanceId: string) => {
        const request = deferred()
        pending.set(instanceId, request)
        return request.promise
    })

    const first = refresh('first')
    const second = refresh('second')

    assert.notEqual(first, second)
    pending.get('first')?.resolve('first snapshot')
    pending.get('second')?.resolve('second snapshot')
    assert.deepEqual(await Promise.all([first, second]), ['first snapshot', 'second snapshot'])
})

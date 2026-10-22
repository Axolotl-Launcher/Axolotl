import assert from 'node:assert/strict'
import test from 'node:test'

import { createThumbnailQueue } from './thumbnail-queue.ts'

test('cancels obsolete queued work without occupying slots needed by the current viewport', async () => {
    const queue = createThumbnailQueue(1)
    let finish!: () => void
    const calls: string[] = []
    const first = queue.enqueue(
        () =>
            new Promise<void>((resolve) => {
                calls.push('first')
                finish = resolve
            }),
    )
    const old = queue.enqueue(async () => {
        calls.push('obsolete')
    })
    const rejected = assert.rejects(old.promise, /released/)
    const current = queue.enqueue(async () => {
        calls.push('current')
    })
    assert.equal(old.cancel(), true)
    await rejected
    await Promise.resolve()
    assert.equal(first.cancel(), false)
    finish()
    await first.promise
    await current.promise
    assert.deepEqual(calls, ['first', 'current'])
})

test('failed tasks release capacity for the next thumbnail', async () => {
    const queue = createThumbnailQueue(1)
    const bad = queue.enqueue(async () => {
        throw new Error('decode')
    })
    const good = queue.enqueue(async () => 'image')
    await assert.rejects(bad.promise, /decode/)
    assert.equal(await good.promise, 'image')
})

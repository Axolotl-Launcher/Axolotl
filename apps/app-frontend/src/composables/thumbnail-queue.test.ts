import { expect, it } from 'vitest'

import { createThumbnailQueue } from './thumbnail-queue.ts'

it('cancels obsolete queued work without occupying slots needed by the current viewport', async () => {
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
    const rejected = expect(old.promise).rejects.toThrow(/released/)
    const current = queue.enqueue(async () => {
        calls.push('current')
    })
    expect(old.cancel()).toBe(true)
    await rejected
    await Promise.resolve()
    expect(first.cancel()).toBe(false)
    finish()
    await first.promise
    await current.promise
    expect(calls).toEqual(['first', 'current'])
})

it('failed tasks release capacity for the next thumbnail', async () => {
    const queue = createThumbnailQueue(1)
    const bad = queue.enqueue(async () => {
        throw new Error('decode')
    })
    const good = queue.enqueue(async () => 'image')
    await expect(bad.promise).rejects.toThrow(/decode/)
    expect(await good.promise).toBe('image')
})

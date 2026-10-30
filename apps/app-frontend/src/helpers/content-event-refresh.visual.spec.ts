import { afterEach, expect, it, vi } from 'vitest'

import { createContentEventRefresh } from './content-event-refresh'

afterEach(() => vi.useRealTimers())

it.each(['update review', 'toggle', 'delete', 'batch toggle'])(
    'coalesces both event kinds until the last %s operation ends',
    async () => {
        vi.useFakeTimers()
        let busy = true
        const refresh = vi.fn()
        const queue = createContentEventRefresh({
            busy: () => busy,
            revision: () => 1,
            refresh,
            delay: 180,
        })
        queue.enqueue({ event: 'synced' })
        queue.enqueue({ event: 'content_changed', revision: 2 })
        await vi.advanceTimersByTimeAsync(1000)
        expect(refresh).not.toHaveBeenCalled()
        queue.resume()
        expect(refresh).not.toHaveBeenCalled()
        busy = false
        queue.resume()
        queue.resume()
        await vi.advanceTimersByTimeAsync(180)
        expect(refresh).toHaveBeenCalledExactlyOnceWith(true)
    },
)

it('retains an event when an operation starts during the debounce and cancels on instance switch', async () => {
    vi.useFakeTimers()
    let busy = false
    const refresh = vi.fn()
    const queue = createContentEventRefresh({
        busy: () => busy,
        revision: () => 1,
        refresh,
        delay: 180,
    })
    queue.enqueue({ event: 'content_changed', revision: 2 })
    busy = true
    await vi.advanceTimersByTimeAsync(180)
    expect(refresh).not.toHaveBeenCalled()
    busy = false
    queue.resume()
    await vi.advanceTimersByTimeAsync(180)
    expect(refresh).toHaveBeenCalledExactlyOnceWith(false)
    queue.enqueue({ event: 'synced' })
    queue.clear()
    await vi.advanceTimersByTimeAsync(180)
    expect(refresh).toHaveBeenCalledOnce()
    queue.enqueue({ event: 'content_changed', revision: 1 })
    await vi.advanceTimersByTimeAsync(180)
    expect(refresh).toHaveBeenCalledOnce()
})

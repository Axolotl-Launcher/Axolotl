import { expect, it, vi } from 'vitest'

import { createUpdateFailureConsumer } from './app-update-failure'

it('queues failure feedback before clearing it and does not duplicate it during the session', async () => {
    const consume = createUpdateFailureConsumer()
    const events: string[] = []
    const notify = vi.fn(() => {
        events.push('notify')
    })
    const clear = vi.fn(async () => {
        events.push('clear')
    })
    const failure = { version: '1.2.3', error: '<b>authorization cancelled</b>' }
    expect(await consume(failure, notify, clear)).toBe(true)
    await consume(failure, notify, clear)
    expect(events).toEqual(['notify', 'clear', 'clear'])
    expect(notify).toHaveBeenCalledWith(failure)
    expect(notify).toHaveBeenCalledTimes(1)
    expect(await consume(null, notify, clear)).toBe(false)
})

it('retries a failed marker clear without repeating already queued feedback', async () => {
    const consume = createUpdateFailureConsumer()
    const notify = vi.fn()
    const clear = vi
        .fn()
        .mockRejectedValueOnce(new Error('disk unavailable'))
        .mockResolvedValueOnce(undefined)
    const failure = { version: '1.2.3', error: 'rpm failed' }
    await expect(consume(failure, notify, clear)).rejects.toThrow('disk unavailable')
    expect(await consume(failure, notify, clear)).toBe(true)
    expect(notify).toHaveBeenCalledTimes(1)
    expect(clear).toHaveBeenCalledTimes(2)
})

it('retains the marker when feedback cannot be queued and bounds displayed payloads', async () => {
    const consume = createUpdateFailureConsumer()
    const clear = vi.fn(async () => {})
    const failure = { version: 'a'.repeat(200), error: 'b'.repeat(4000) }
    await expect(
        consume(
            failure,
            () => {
                throw new Error('not ready')
            },
            clear,
        ),
    ).rejects.toThrow('not ready')
    expect(clear).not.toHaveBeenCalled()
    const notify = vi.fn()
    await consume(failure, notify, clear)
    expect(notify).toHaveBeenCalledWith({ version: 'a'.repeat(128), error: 'b'.repeat(2048) })
})

it('serializes concurrent consumption while preserving a newly queued failure', async () => {
    const consume = createUpdateFailureConsumer()
    let release!: () => void
    const pending = new Promise<void>((resolve) => {
        release = resolve
    })
    const notify = vi.fn()
    const first = consume({ version: '1', error: 'failed' }, notify, () => pending)
    expect(await consume({ version: '1', error: 'failed' }, notify, async () => {})).toBe(false)
    release()
    await first
    await consume({ version: '2', error: 'different attempt' }, notify, async () => {})
    expect(notify).toHaveBeenCalledTimes(2)
})

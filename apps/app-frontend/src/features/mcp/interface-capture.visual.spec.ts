import { afterEach, expect, it, vi } from 'vitest'

import { captureMcpUi } from './interface-capture'

const capture = vi.hoisted(() => vi.fn())
vi.mock('html-to-image', () => ({ toSvg: capture }))

afterEach(() => {
    vi.useRealTimers()
    capture.mockReset()
})

it('rejects stalled capture before the native request expires', async () => {
    vi.useFakeTimers()
    capture.mockImplementation(() => new Promise(() => {}))
    const result = captureMcpUi(new AbortController().signal)
    const failure = expect(result).rejects.toThrow('Screenshot capture timed out')
    await vi.advanceTimersByTimeAsync(10_000)
    await failure
    expect(vi.getTimerCount()).toBe(0)
})

it('propagates controller shutdown and releases deadline resources', async () => {
    vi.useFakeTimers()
    capture.mockImplementation(() => new Promise(() => {}))
    const controller = new AbortController()
    const result = captureMcpUi(controller.signal)
    const failure = expect(result).rejects.toThrow('Controller stopped')
    controller.abort(new Error('Controller stopped'))
    await failure
    expect(vi.getTimerCount()).toBe(0)
})

it('does not start rendering with an already cancelled controller', async () => {
    const controller = new AbortController()
    controller.abort(new Error('Cancelled before capture'))
    await expect(captureMcpUi(controller.signal)).rejects.toThrow('Cancelled before capture')
    expect(capture).not.toHaveBeenCalled()
})

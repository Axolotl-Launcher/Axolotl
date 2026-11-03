import { expect, it, vi } from 'vitest'

import { RESOURCE_ANALYSIS_TIMEOUT_MS, resourceAnalysisRequest } from './resource-analysis-request'

it('bounds a promise that ignores abort with a fixed total deadline', async () => {
    vi.useFakeTimers()
    const controller = new AbortController()
    try {
        const result = resourceAnalysisRequest(controller, () => new Promise(() => {}))
        const finished = expect(result).rejects.toMatchObject({ name: 'TimeoutError' })
        await vi.advanceTimersByTimeAsync(RESOURCE_ANALYSIS_TIMEOUT_MS - 1)
        expect(controller.signal.aborted).toBe(false)
        await vi.advanceTimersByTimeAsync(1)
        await finished
        expect(controller.signal.aborted).toBe(true)
        expect(vi.getTimerCount()).toBe(0)
    } finally {
        vi.useRealTimers()
    }
})

it('rejects a never-resolving request on cancel and releases its deadline timer', async () => {
    vi.useFakeTimers()
    const controller = new AbortController()
    try {
        const result = resourceAnalysisRequest(controller, () => new Promise(() => {}))
        controller.abort()
        await expect(result).rejects.toMatchObject({ name: 'AbortError' })
        expect(vi.getTimerCount()).toBe(0)
    } finally {
        vi.useRealTimers()
    }
})

it('keeps a successful request usable without a later deadline abort', async () => {
    vi.useFakeTimers()
    const controller = new AbortController()
    try {
        await expect(resourceAnalysisRequest(controller, async () => 'ready')).resolves.toBe(
            'ready',
        )
        expect(vi.getTimerCount()).toBe(0)
        await vi.advanceTimersByTimeAsync(RESOURCE_ANALYSIS_TIMEOUT_MS)
        expect(controller.signal.aborted).toBe(false)
    } finally {
        vi.useRealTimers()
    }
})

it('does not start a request whose owner has already canceled it', async () => {
    const controller = new AbortController()
    controller.abort()
    const action = vi.fn()
    await expect(resourceAnalysisRequest(controller, action)).rejects.toMatchObject({
        name: 'AbortError',
    })
    expect(action).not.toHaveBeenCalled()
})

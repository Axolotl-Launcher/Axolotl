import { afterEach, expect, it, vi } from 'vitest'

import { createNetworkStatus } from './useNetworkStatus.ts'

afterEach(() => {
    vi.useRealTimers()
    vi.unstubAllGlobals()
})

it('starts online unless the browser reports being offline', () => {
    expect(createNetworkStatus().offline.value).toBe(false)
    expect(createNetworkStatus(true).offline.value).toBe(true)
})

it('browser connectivity changes update global offline mode', () => {
    const status = createNetworkStatus()
    status.setBrowserOffline(true)
    expect(status.offline.value).toBe(true)
    status.markBrowserOnline()
    expect(status.browserOffline.value).toBe(false)
    expect(status.offline.value).toBe(false)
})

it('simulation survives real online events and cannot hide a real disconnection', () => {
    const status = createNetworkStatus()
    status.setSimulatedOffline(true)
    status.markBrowserOnline()
    expect(status.offline.value).toBe(true)
    expect(status.browserOffline.value).toBe(false)
    status.setBrowserOffline(true)
    status.setSimulatedOffline(false)
    expect(status.offline.value).toBe(true)
    status.markBrowserOnline()
    expect(status.offline.value).toBe(false)
})

it('coalesces retry clicks and reads connectivity after the debounce window', async () => {
    vi.useFakeTimers()
    const navigatorState = { onLine: false }
    vi.stubGlobal('navigator', navigatorState)
    const status = createNetworkStatus(true)
    const first = status.retryConnection()
    expect(status.retryConnection()).toBe(first)
    expect(status.retrying.value).toBe(true)
    await vi.advanceTimersByTimeAsync(599)
    expect(status.retrying.value).toBe(true)
    navigatorState.onLine = true
    await vi.advanceTimersByTimeAsync(1)
    expect(await first).toBe(true)
    expect(status.retrying.value).toBe(false)
    expect(status.offline.value).toBe(false)
})

it('keeps retries available while offline and does not cancel simulation', async () => {
    vi.useFakeTimers()
    vi.stubGlobal('navigator', { onLine: false })
    const status = createNetworkStatus(true)
    const retry = status.retryConnection()
    await vi.advanceTimersByTimeAsync(600)
    expect(await retry).toBe(false)
    expect(status.retrying.value).toBe(false)
    vi.stubGlobal('navigator', { onLine: true })
    status.setSimulatedOffline(true)
    const simulatedRetry = status.retryConnection()
    await vi.advanceTimersByTimeAsync(600)
    expect(await simulatedRetry).toBe(false)
    expect(status.browserOffline.value).toBe(false)
    expect(status.offline.value).toBe(true)
})

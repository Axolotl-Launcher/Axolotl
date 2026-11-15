import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { invoke } from '@/helpers/invoke'
import { initObservability, resetObservability } from '@/helpers/observability'

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }))

vi.mock('@tauri-apps/api/core', () => ({
    invoke: invokeMock,
}))

interface RecordedEvent {
    name: string
    attributes?: Record<string, unknown>
    duration_ms?: number
}

function statusResult(enabled: boolean) {
    return { enabled, signals: enabled ? ['logs'] : [], session_id: enabled ? 'session' : null }
}

function recordCalls(): unknown[][] {
    return invokeMock.mock.calls.filter(
        ([command]) => command === 'plugin:observability|observability_record',
    )
}

function recordedEvents(): RecordedEvent[] {
    return recordCalls().flatMap((call) => (call[1] as { events: RecordedEvent[] }).events)
}

async function enableObservability(): Promise<void> {
    invokeMock.mockImplementation((command: string) =>
        command === 'plugin:observability|observability_status'
            ? Promise.resolve(statusResult(true))
            : Promise.resolve(undefined),
    )
    expect(await initObservability()).toBe(true)
}

describe('instrumented invoke', () => {
    beforeEach(() => {
        vi.useFakeTimers()
        invokeMock.mockReset()
        resetObservability()
    })

    afterEach(() => {
        resetObservability()
        vi.useRealTimers()
    })

    it('passes the result through and records a successful command', async () => {
        await enableObservability()
        invokeMock.mockImplementation((command: string) =>
            command === 'plugin:observability|observability_record'
                ? Promise.resolve(undefined)
                : Promise.resolve('result'),
        )

        await expect(invoke<string>('get_instance', { id: 1 })).resolves.toBe('result')
        expect(invokeMock).toHaveBeenCalledWith('get_instance', { id: 1 })

        await vi.advanceTimersByTimeAsync(5_000)
        expect(recordedEvents()).toEqual([
            {
                name: 'command',
                attributes: { name: 'get_instance', outcome: 'ok' },
                duration_ms: expect.any(Number),
            },
        ])
    })

    it('rethrows failures and records only the error category', async () => {
        await enableObservability()
        const failure = new TypeError('private path C:/Users/admin/world')
        invokeMock.mockImplementation((command: string) =>
            command === 'plugin:observability|observability_record'
                ? Promise.resolve(undefined)
                : Promise.reject(failure),
        )

        await expect(invoke('get_instance')).rejects.toBe(failure)
        await vi.advanceTimersByTimeAsync(5_000)

        const events = recordedEvents()
        expect(events).toEqual([
            {
                name: 'command',
                attributes: {
                    name: 'get_instance',
                    outcome: 'error',
                    'error.kind': 'TypeError',
                },
                duration_ms: expect.any(Number),
            },
        ])
        expect(JSON.stringify(events)).not.toContain('private path')
    })

    it('does not report its own observability calls', async () => {
        await enableObservability()

        await invoke('plugin:observability|observability_record', { events: [] })
        await vi.advanceTimersByTimeAsync(5_000)

        const calls = recordCalls()
        expect(calls).toHaveLength(1)
        expect((calls[0][1] as { events: unknown[] }).events).toEqual([])
    })

    it('records nothing while the pipeline is disabled', async () => {
        invokeMock.mockImplementation((command: string) =>
            command === 'plugin:observability|observability_status'
                ? Promise.resolve(statusResult(false))
                : Promise.resolve('result'),
        )
        expect(await initObservability()).toBe(false)

        await expect(invoke('get_instance')).resolves.toBe('result')
        await vi.advanceTimersByTimeAsync(10_000)

        expect(recordCalls()).toHaveLength(0)
    })
})

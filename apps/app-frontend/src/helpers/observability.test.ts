import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import {
    initObservability,
    OBSERVABILITY_EVENT_ATTRIBUTES,
    recordEvent,
    recordNavigation,
    resetObservability,
} from '@/helpers/observability'

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }))

vi.mock('@tauri-apps/api/core', () => ({
    invoke: invokeMock,
}))

interface RecordedEvent {
    name: string
    attributes?: Record<string, unknown>
    duration_ms?: number
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
    invokeMock.mockResolvedValue({ enabled: true, signals: ['logs'], session_id: 'session' })
    expect(await initObservability()).toBe(true)
}

describe('frontend observability buffer', () => {
    beforeEach(() => {
        vi.useFakeTimers()
        invokeMock.mockReset()
        resetObservability()
    })

    afterEach(() => {
        resetObservability()
        vi.useRealTimers()
    })

    it('drops every event when the native pipeline is disabled', async () => {
        invokeMock.mockResolvedValue({ enabled: false, signals: [], session_id: null })

        expect(await initObservability()).toBe(false)
        recordEvent('page_view', { 'route.name': 'home' })
        await vi.advanceTimersByTimeAsync(10_000)

        expect(recordCalls()).toHaveLength(0)
        expect(invokeMock).toHaveBeenCalledTimes(1)
    })

    it('flushes queued events that were recorded before initialization', async () => {
        recordEvent('page_view', { 'route.name': 'startup' })
        await enableObservability()
        await vi.advanceTimersByTimeAsync(0)

        expect(recordedEvents()).toEqual([
            { name: 'page_view', attributes: { 'route.name': 'startup' } },
        ])
    })

    it('sends events in batches of at most twenty', async () => {
        await enableObservability()
        for (let index = 0; index < 25; index += 1) {
            recordEvent('page_view', { 'route.name': `route${index}` })
        }
        await vi.advanceTimersByTimeAsync(5_000)

        const calls = recordCalls()
        expect(calls).toHaveLength(2)
        expect((calls[0][1] as { events: RecordedEvent[] }).events).toHaveLength(20)
        expect((calls[1][1] as { events: RecordedEvent[] }).events).toHaveLength(5)
    })

    it('bounds the queue instead of growing without limit', async () => {
        await enableObservability()
        for (let index = 0; index < 100; index += 1) {
            recordEvent('command', { name: 'get_instance', outcome: 'ok' })
        }
        await vi.advanceTimersByTimeAsync(5_000)

        expect(recordedEvents()).toHaveLength(64)
    })

    it('strips attribute keys outside the event allowlist', async () => {
        await enableObservability()
        recordEvent('instance_start', {
            source: 'Home',
            'instance.loader': 'fabric',
            'instance.game_version': '1.21.1',
            server_name: 'Private Server',
            instance_name: 'Private Instance',
            title: 'Private Title',
        } as never)
        await vi.advanceTimersByTimeAsync(5_000)

        expect(recordedEvents()).toEqual([
            {
                name: 'instance_start',
                attributes: {
                    source: 'Home',
                    'instance.loader': 'fabric',
                    'instance.game_version': '1.21.1',
                },
            },
        ])
    })

    it('keeps identifiers containing paths out of navigation events', async () => {
        await enableObservability()
        recordNavigation('/home/admin/private-world')
        recordNavigation('instance')
        await vi.advanceTimersByTimeAsync(5_000)

        expect(recordedEvents()).toEqual([
            { name: 'page_view', attributes: { 'route.name': 'instance' } },
        ])
    })

    it('drops path-shaped values but keeps plugin command separators', async () => {
        await enableObservability()
        recordEvent('pack_install', { source: 'C:/Users/admin/private' })
        recordEvent('command', {
            name: 'plugin:updater|check',
            outcome: 'ok',
        })
        await vi.advanceTimersByTimeAsync(5_000)

        expect(recordedEvents()).toEqual([
            { name: 'pack_install' },
            {
                name: 'command',
                attributes: { name: 'plugin:updater|check', outcome: 'ok' },
            },
        ])
    })

    it('never allowlists user content attributes', () => {
        const forbidden = [
            'server_name',
            'instance_name',
            'title',
            'path',
            'url',
            'version_id',
            'project_id',
        ]
        for (const keys of Object.values(OBSERVABILITY_EVENT_ATTRIBUTES)) {
            for (const key of forbidden) {
                expect(keys).not.toContain(key)
            }
        }
    })
})

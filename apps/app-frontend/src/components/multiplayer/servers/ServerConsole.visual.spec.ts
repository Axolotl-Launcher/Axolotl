import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'

import type { ServerView } from '@/composables/useServers'
import type { ServerLogDelta } from '@/helpers/servers'

import ServerConsole from './ServerConsole.vue'

const emptyDelta = (): ServerLogDelta => ({
    cursor: { epoch: 1, sequence: 0 },
    reset: false,
    lines: [],
})
const backend = vi.hoisted(() => ({
    hydrate: vi.fn<() => Promise<ServerLogDelta>>(),
    unsubscribe: vi.fn(),
    append: vi.fn<(text: string) => Promise<void>>(async () => {}),
    clear: vi.fn(),
}))
backend.hydrate.mockImplementation(async () => emptyDelta())
vi.mock('@/composables/useServers', () => ({
    subscribeServerConsoleOutput: () => backend.unsubscribe,
    useServers: () => ({ sendCommand: vi.fn() }),
}))
vi.mock('@/helpers/servers', () => ({ servers: { getLogDelta: backend.hydrate } }))
vi.mock('@modrinth/ui', async () => {
    const { shallowRef } = await import('vue')
    return {
        ConsolePageLayout: {
            setup(_: unknown, { expose }: { expose: (value: unknown) => void }) {
                expose({ scrollToBottom: vi.fn() })
                return () => null
            },
        },
        JLineCommandInput: { render: () => null },
        createConsoleState: () => ({
            output: shallowRef([]),
            clear: backend.clear,
            addLegacyLog: backend.append,
        }),
        defineMessages: (value: unknown) => value,
        provideConsoleManager: vi.fn(),
        useVIntl: () => ({ formatMessage: () => '' }),
    }
})

let wrapper: ReturnType<typeof mount> | undefined
const server = { id: 'server', running: true, serverType: 'vanilla' } as ServerView
afterEach(() => {
    wrapper?.unmount()
    wrapper = undefined
    vi.useRealTimers()
    backend.hydrate.mockReset().mockImplementation(async () => emptyDelta())
    backend.unsubscribe.mockClear()
    backend.append.mockClear()
    backend.clear.mockClear()
})

it('does not start background polling when initial hydration completes after unmount', async () => {
    vi.useFakeTimers()
    let finish!: (delta: ServerLogDelta) => void
    backend.hydrate.mockReturnValueOnce(
        new Promise((resolve) => {
            finish = resolve
        }),
    )
    wrapper = mount(ServerConsole, { props: { server } })
    wrapper.unmount()
    wrapper = undefined
    finish(emptyDelta())
    await flushPromises()
    await vi.advanceTimersByTimeAsync(3000)
    expect(backend.hydrate).toHaveBeenCalledOnce()
    expect(backend.unsubscribe).toHaveBeenCalledOnce()
})

it('waits for a slow reconciliation and never schedules another one after unmount', async () => {
    vi.useFakeTimers()
    wrapper = mount(ServerConsole, { props: { server } })
    await flushPromises()
    let finish!: (delta: ServerLogDelta) => void
    backend.hydrate.mockReturnValueOnce(
        new Promise((resolve) => {
            finish = resolve
        }),
    )
    await vi.advanceTimersByTimeAsync(1000)
    await vi.advanceTimersByTimeAsync(5000)
    expect(backend.hydrate).toHaveBeenCalledTimes(2)
    wrapper.unmount()
    wrapper = undefined
    finish(emptyDelta())
    await flushPromises()
    await vi.advanceTimersByTimeAsync(3000)
    expect(backend.hydrate).toHaveBeenCalledTimes(2)
})

it('resumes reconciliation when a stopped server starts again', async () => {
    vi.useFakeTimers()
    wrapper = mount(ServerConsole, { props: { server } })
    await flushPromises()
    await wrapper.setProps({ server: { ...server, running: false } })
    await flushPromises()
    await vi.advanceTimersByTimeAsync(2000)
    expect(backend.hydrate).toHaveBeenCalledTimes(2)
    await wrapper.setProps({ server })
    await flushPromises()
    expect(backend.hydrate).toHaveBeenCalledTimes(3)
    await vi.advanceTimersByTimeAsync(1000)
    expect(backend.hydrate).toHaveBeenCalledTimes(4)
})

it('continues after a full rolling buffer and only appends the cursor delta', async () => {
    vi.useFakeTimers()
    const initial: ServerLogDelta = {
        cursor: { epoch: 1, sequence: 10000 },
        reset: true,
        lines: ['old'],
    }
    backend.hydrate
        .mockResolvedValueOnce(initial)
        .mockResolvedValueOnce({
            cursor: { epoch: 1, sequence: 10001 },
            reset: false,
            lines: ['new'],
        })
        .mockResolvedValueOnce({
            cursor: { epoch: 2, sequence: 1 },
            reset: true,
            lines: ['restarted'],
        })
    wrapper = mount(ServerConsole, { props: { server } })
    await flushPromises()
    await vi.advanceTimersByTimeAsync(1000)
    expect(backend.hydrate).toHaveBeenNthCalledWith(2, server.id, initial.cursor)
    expect(backend.append.mock.calls.map((args) => args[0])).toEqual(['old', 'new'])
    expect(backend.clear).toHaveBeenCalledTimes(1)
    await vi.advanceTimersByTimeAsync(1000)
    expect(backend.append).toHaveBeenLastCalledWith('restarted')
    expect(backend.clear).toHaveBeenCalledTimes(2)
})

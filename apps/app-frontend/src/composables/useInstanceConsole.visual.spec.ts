import { expect, it, vi } from 'vitest'

import { useInstanceConsole } from './useInstanceConsole'

const backend = vi.hoisted(() => ({ latest: vi.fn(), buffer: vi.fn(), clear: vi.fn() }))

vi.mock('@/helpers/logs', () => ({
    get_minecraft_latest_log_cursor: backend.latest,
    get_live_log_buffer: backend.buffer,
    clear_log_buffer: backend.clear,
    get_logs: vi.fn(),
}))

function pendingHistory() {
    let finish!: (output: { output: string }) => void
    backend.latest.mockReturnValueOnce(new Promise((resolve) => (finish = resolve)))
    backend.buffer.mockResolvedValueOnce('late line')
    return () => finish({ output: 'first line' })
}

it('does not populate the live console after leaving the logs tab', async () => {
    const console = useInstanceConsole('abandoned-logs')
    const finish = pendingHistory()
    let disposed = false
    const pending = console.hydrate(() => disposed)
    disposed = true
    finish()
    await pending

    expect(console.liveConsole.output.value).toEqual([])
})

it('reuses the pending reads for a freshly mounted tab and publishes the history once', async () => {
    backend.latest.mockClear()
    backend.buffer.mockClear()
    const previous = useInstanceConsole('reopened-logs')
    const finish = pendingHistory()
    let disposed = false
    const abandoned = previous.hydrate(() => disposed)
    disposed = true
    const current = useInstanceConsole('reopened-logs')
    const pending = current.hydrate(() => false)
    finish()
    await Promise.all([abandoned, pending])

    expect(backend.latest).toHaveBeenCalledOnce()
    expect(backend.buffer).toHaveBeenCalledOnce()
    expect(current.liveConsole.output.value.map((line) => line.text)).toEqual([
        'first line',
        'late line',
    ])
})

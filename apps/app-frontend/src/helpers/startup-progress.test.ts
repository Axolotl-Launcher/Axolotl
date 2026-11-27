import { afterEach, beforeEach, expect, it, vi } from 'vitest'

import { type LoadingBar, progress_bars_list } from '@/helpers/state'

import { createStartupProgress, type StartupLoadingProgress } from './startup-progress'

const events = vi.hoisted(() => ({
    listener: null as ((payload: StartupLoadingProgress) => void) | null,
    stop: vi.fn(),
    register: vi.fn(),
}))
vi.mock('@/helpers/events.js', () => ({
    loading_listener: (callback: (payload: StartupLoadingProgress) => void) => {
        events.listener = callback
        return events.register()
    },
}))
vi.mock('@/helpers/state', () => ({ progress_bars_list: vi.fn() }))

function deferred<T>() {
    let resolve!: (value: T) => void
    let reject!: (error: Error) => void
    const promise = new Promise<T>((yes, no) => {
        resolve = yes
        reject = no
    })
    return { promise, resolve, reject }
}

function migration(fraction = 0.4): StartupLoadingProgress {
    return {
        event: { type: 'database_migration', stage: 'recovery', completed: 169, total: 423 },
        fraction,
        loader_uuid: 'migration',
    }
}

beforeEach(() => {
    vi.clearAllMocks()
    events.register.mockResolvedValue(events.stop)
    vi.mocked(progress_bars_list).mockResolvedValue({})
})
afterEach(() => vi.useRealTimers())

it('registers before initialization and retains first progress before the splash mounts', async () => {
    const registered = deferred<() => void>()
    const initialized = deferred<void>()
    events.register.mockReturnValue(registered.promise)
    const initialize = vi.fn(() => initialized.promise)
    const startup = createStartupProgress()
    const pending = startup.initialize(initialize)
    expect(initialize).not.toHaveBeenCalled()
    registered.resolve(events.stop)
    await vi.waitFor(() => expect(initialize).toHaveBeenCalledOnce())
    events.listener!(migration())
    expect(startup.progress.value).toEqual(migration())
    initialized.resolve()
    await pending
    startup.dispose()
    expect(events.stop).toHaveBeenCalledOnce()
})

it('keeps waiting beyond 15 seconds without inventing progress', async () => {
    vi.useFakeTimers()
    const initialized = deferred<void>()
    const startup = createStartupProgress()
    let settled = false
    const pending = startup
        .initialize(() => initialized.promise)
        .then(() => {
            settled = true
        })
    await Promise.resolve()
    events.listener!(migration(0.25))
    await vi.advanceTimersByTimeAsync(60_000)
    expect(settled).toBe(false)
    expect(startup.progress.value?.fraction).toBe(0.25)
    initialized.resolve()
    await pending
    startup.dispose()
})

it('recovers active progress on a recreated webview without overwriting newer events', async () => {
    const snapshot = deferred<Record<string, LoadingBar>>()
    const initialized = deferred<void>()
    vi.mocked(progress_bars_list).mockReturnValue(snapshot.promise)
    const startup = createStartupProgress()
    const pending = startup.initialize(() => initialized.promise)
    await vi.waitFor(() => expect(progress_bars_list).toHaveBeenCalledOnce())
    events.listener!(migration(0.6))
    snapshot.resolve({
        migration: {
            loading_bar_uuid: 'migration',
            bar_type: migration().event,
            current: 1,
            total: 10,
        },
    })
    await Promise.resolve()
    expect(startup.progress.value?.fraction).toBe(0.6)
    initialized.resolve()
    await pending
    startup.dispose()
})

it('uses active snapshots when no live event has arrived', async () => {
    vi.mocked(progress_bars_list).mockResolvedValue({
        migration: {
            loading_bar_uuid: 'migration',
            bar_type: migration().event,
            current: 3,
            total: 10,
        },
    })
    const startup = createStartupProgress()
    const initialized = deferred<void>()
    const pending = startup.initialize(() => initialized.promise)
    await vi.waitFor(() => expect(startup.progress.value?.fraction).toBe(0.3))
    expect(startup.progress.value?.fraction).toBe(0.3)
    expect(startup.progress.value?.event.completed).toBe(169)
    initialized.resolve()
    await pending
    expect(startup.progress.value?.fraction).toBeNull()
    startup.dispose()
})

it('continues initialization if progress subscription fails', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {})
    events.register.mockRejectedValue(new Error('listener unavailable'))
    const startup = createStartupProgress()
    const initialize = vi.fn(async () => {})
    await startup.initialize(initialize)
    expect(initialize).toHaveBeenCalledOnce()
    expect(warn).toHaveBeenCalledOnce()
    startup.dispose()
    warn.mockRestore()
})

it('uses successful initialization to finish migration if the final event is lost', async () => {
    const initialized = deferred<void>()
    const startup = createStartupProgress()
    const pending = startup.initialize(() => initialized.promise)
    await Promise.resolve()
    events.listener!(migration(0.4))
    initialized.resolve()
    await pending
    expect(startup.progress.value?.fraction).toBeNull()
    startup.dispose()
})

it('returns a directory move warning received with successful initialization', async () => {
    const startup = createStartupProgress()
    const warning = {
        id: 'migration-warning',
        source: 'old-directory',
        destination: 'new-directory',
        missing_folders: ['profiles'],
    }

    await expect(startup.initialize(async () => warning)).resolves.toEqual(warning)
    startup.dispose()
})

it('does not revive an active migration from a snapshot arriving after initialization', async () => {
    const snapshot = deferred<Record<string, LoadingBar>>()
    vi.mocked(progress_bars_list).mockReturnValue(snapshot.promise)
    const startup = createStartupProgress()
    await startup.initialize(async () => {})
    snapshot.resolve({
        migration: {
            loading_bar_uuid: 'migration',
            bar_type: migration().event,
            current: 3,
            total: 10,
        },
    })
    await Promise.resolve()
    expect(startup.progress.value?.fraction).toBeNull()
    startup.dispose()
})

it('keeps a newer directory move when an old migration completion arrives late', async () => {
    const initialized = deferred<void>()
    const startup = createStartupProgress()
    const pending = startup.initialize(() => initialized.promise)
    await Promise.resolve()
    events.listener!(migration(1))
    const move = { event: { type: 'directory_move' }, loader_uuid: 'move', fraction: 0.2 }
    events.listener!(move)
    events.listener!({ ...migration(), fraction: null })
    expect(startup.progress.value).toEqual(move)
    initialized.resolve()
    await pending
    startup.dispose()
})

it('ignores unavailable early snapshots but propagates initialization failure and cleans up', async () => {
    vi.mocked(progress_bars_list).mockRejectedValue(new Error('Event state not initialized'))
    const startup = createStartupProgress()
    await expect(
        startup.initialize(async () => {
            events.listener!(migration(0.4))
            throw new Error('disk full')
        }),
    ).rejects.toThrow('disk full')
    expect(events.stop).toHaveBeenCalledOnce()
    events.listener!(migration(1))
    expect(startup.progress.value?.fraction).toBe(0.4)
})

it('unsubscribes if disposed while registration is still pending', async () => {
    const registered = deferred<() => void>()
    events.register.mockReturnValue(registered.promise)
    const initialize = vi.fn(async () => {})
    const startup = createStartupProgress()
    const pending = startup.initialize(initialize)
    startup.dispose()
    registered.resolve(events.stop)
    await pending
    expect(initialize).not.toHaveBeenCalled()
    expect(events.stop).toHaveBeenCalledOnce()
})

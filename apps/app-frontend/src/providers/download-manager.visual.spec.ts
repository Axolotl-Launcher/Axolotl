import { flushPromises } from '@vue/test-utils'
import { beforeEach, expect, it, vi } from 'vitest'

import {
    download_job_cancel,
    download_job_get,
    download_job_retry,
    type InstallJobSnapshot,
} from '@/helpers/install'

import { createDownloadManager } from './download-manager'

const backend = vi.hoisted(() => ({
    requests: vi.fn(),
    jobs: vi.fn(),
    loading: vi.fn(),
    list: vi.fn(),
    bars: vi.fn(),
}))
vi.mock('@modrinth/ui', () => ({ createContext: () => [vi.fn(), vi.fn()] }))
vi.mock('@/helpers/events', () => ({
    download_request_listener: backend.requests,
    install_job_listener: backend.jobs,
    loading_listener: backend.loading,
}))
vi.mock('@/helpers/install', () => ({
    download_job_list: backend.list,
    installJobInstanceId: (job: InstallJobSnapshot) => job.instance_id,
    download_history_clear: vi.fn(),
    download_job_cancel: vi.fn(),
    download_job_delete: vi.fn(),
    download_job_get: vi.fn(),
    download_job_resume: vi.fn(),
    download_job_retry: vi.fn(),
    install_job_skip_missing_content: vi.fn(),
}))
vi.mock('@/helpers/state', () => ({ progress_bars_list: backend.bars }))
vi.mock('@/helpers/instance', () => ({ queue_content_change: vi.fn() }))
vi.mock('@/helpers/curseforge-manual', () => ({ setCurseForgeManualDownloads: vi.fn() }))

function deferred<T>() {
    let resolve!: (value: T) => void
    const promise = new Promise<T>((yes) => {
        resolve = yes
    })
    return { promise, resolve }
}
function job(status: InstallJobSnapshot['status'], modified: string): InstallJobSnapshot {
    return {
        job_id: 'one',
        status,
        modified,
        created: '2026-10-19',
        items: [],
        summary: { bytes_downloaded: 0 },
        target: { type: 'new_instance' },
        kind: 'create_instance',
    } as unknown as InstallJobSnapshot
}
beforeEach(() => {
    vi.resetAllMocks()
    backend.requests.mockResolvedValue(vi.fn())
    backend.jobs.mockResolvedValue(vi.fn())
    backend.loading.mockResolvedValue(vi.fn())
    backend.list.mockResolvedValue({ jobs: [] })
    backend.bars.mockResolvedValue({})
})

it('rejects late same-second active events but permits an explicitly accepted same-second retry', async () => {
    const current = job('succeeded', '2026-10-28T00:00:00Z')
    backend.list.mockResolvedValue({ jobs: [current] })
    const manager = createDownloadManager(vi.fn())
    try {
        await manager.start()
        const running = job('running', current.modified)
        backend.jobs.mock.calls[0][0](running)
        expect(manager.jobs.value[0].status).toBe('succeeded')
        vi.mocked(download_job_retry).mockResolvedValue(running)
        vi.mocked(download_job_get).mockResolvedValue(running)
        await manager.retry('one')
        expect(manager.jobs.value[0].status).toBe('running')
    } finally {
        manager.dispose()
    }
})

it.each(['terminal', 'removed'])(
    'reconciles rejected cancellation with the authoritative %s state without repeating an obsolete error',
    async (state) => {
        const running = job('running', '2026-10-28T00:00:00Z')
        backend.list.mockResolvedValue({ jobs: [running] })
        const onError = vi.fn()
        const manager = createDownloadManager(onError)
        try {
            await manager.start()
            vi.mocked(download_job_cancel).mockRejectedValue({
                error: {
                    message:
                        'Invalid input: Only queued, running, or waiting install jobs can be canceled',
                },
            })
            vi.mocked(download_job_get).mockResolvedValue(
                state === 'terminal' ? job('succeeded', running.modified) : null,
            )
            await expect(manager.cancel('one')).resolves.toBeUndefined()
            expect(manager.activeJobs.value).toHaveLength(0)
            expect(onError).not.toHaveBeenCalled()
        } finally {
            manager.dispose()
        }
    },
)

it('retains real cancel errors and reports failed reconciliation reads', async () => {
    backend.list.mockResolvedValue({ jobs: [job('running', '2026-10-28T00:00:00Z')] })
    const onError = vi.fn()
    const manager = createDownloadManager(onError)
    try {
        await manager.start()
        vi.mocked(download_job_cancel).mockRejectedValue(new Error('permission denied'))
        vi.mocked(download_job_get).mockRejectedValue(new Error('read offline'))
        await expect(manager.cancel('one')).rejects.toThrow('permission denied')
        expect(onError).toHaveBeenCalledWith(expect.objectContaining({ message: 'read offline' }))
        expect(manager.activeJobs.value).toHaveLength(1)
        expect(manager.jobs.value[0].status).toBe('running')
    } finally {
        manager.dispose()
    }
})

it('confirms ambiguous terminal events after same-second retries instead of ending the new execution', async () => {
    const terminal = job('succeeded', '2026-10-28T00:00:00Z')
    const running = job('running', terminal.modified)
    backend.list.mockResolvedValue({ jobs: [terminal] })
    const manager = createDownloadManager(vi.fn())
    try {
        await manager.start()
        vi.mocked(download_job_retry).mockResolvedValue(running)
        vi.mocked(download_job_get).mockResolvedValue(running)
        await manager.retry('one')
        backend.jobs.mock.calls[0][0](terminal)
        await flushPromises()
        expect(manager.jobs.value[0].status).toBe('running')
        vi.mocked(download_job_get).mockResolvedValue(terminal)
        backend.jobs.mock.calls[0][0](terminal)
        await flushPromises()
        expect(manager.jobs.value[0].status).toBe('succeeded')
    } finally {
        manager.dispose()
    }
})

it('fairly polls more than eight stale jobs and continues after a timed-out status query', async () => {
    vi.useFakeTimers()
    const jobs = Array.from({ length: 9 }, (_, index) => ({
        ...job('running', '2026-10-28T00:00:00Z'),
        job_id: String(index),
    }))
    backend.list.mockResolvedValue({ jobs })
    const manager = createDownloadManager(vi.fn())
    try {
        await manager.start()
        vi.mocked(download_job_get).mockImplementation(
            async (id) => jobs.find((candidate) => candidate.job_id === id) ?? null,
        )
        await vi.advanceTimersByTimeAsync(60_000)
        expect(vi.mocked(download_job_get).mock.calls.some(([id]) => id === '8')).toBe(true)
        vi.mocked(download_job_get).mockReturnValueOnce(new Promise(() => {}))
        await vi.advanceTimersByTimeAsync(40_000)
        expect(download_job_get).toHaveBeenCalledTimes(24)
    } finally {
        manager.dispose()
        vi.useRealTimers()
    }
})

it('keeps a fast running cancellation acknowledgement pending without repeating cancellation or overriding a terminal event', async () => {
    vi.useFakeTimers()
    const running = job('running', '2026-10-28T00:00:00Z')
    backend.list.mockResolvedValue({ jobs: [running] })
    const manager = createDownloadManager(vi.fn())
    try {
        await manager.start()
        vi.mocked(download_job_cancel).mockResolvedValue(running)
        vi.mocked(download_job_get).mockResolvedValue(running)
        await manager.cancel('one')
        expect(manager.jobs.value[0].status).toBe('canceling')
        await manager.cancel('one')
        expect(download_job_cancel).toHaveBeenCalledOnce()
        await vi.advanceTimersByTimeAsync(30_000)
        expect(manager.jobs.value[0].status).toBe('canceling')
        backend.jobs.mock.calls[0][0](job('succeeded', running.modified))
        expect(manager.jobs.value[0].status).toBe('succeeded')
        expect(manager.activeJobs.value).toHaveLength(0)
    } finally {
        manager.dispose()
        vi.useRealTimers()
    }
})

it('retries accepted cancellation cleanup while suppressing concurrent cancellation requests', async () => {
    const running = job('running', '2026-10-28T00:00:00Z')
    const canceling = job('canceling', running.modified)
    const canceled = job('canceled', running.modified)
    backend.list.mockResolvedValue({ jobs: [running] })
    const manager = createDownloadManager(vi.fn())
    try {
        await manager.start()
        const accepted = deferred<InstallJobSnapshot>()
        const read = deferred<InstallJobSnapshot | null>()
        vi.mocked(download_job_cancel).mockReturnValueOnce(accepted.promise)
        vi.mocked(download_job_get).mockReturnValueOnce(read.promise)
        const cancellation = manager.cancel('one')
        expect(manager.jobs.value[0].status).toBe('canceling')
        await manager.cancel('one')
        expect(download_job_cancel).toHaveBeenCalledOnce()
        accepted.resolve(canceling)
        await flushPromises()
        await manager.cancel('one')
        expect(download_job_cancel).toHaveBeenCalledOnce()
        read.resolve(canceling)
        await cancellation

        // A late running event must not remove the user's cancellation intent.
        backend.jobs.mock.calls[0][0](running)
        expect(manager.jobs.value[0].status).toBe('canceling')
        vi.mocked(download_job_cancel).mockRejectedValueOnce(new Error('cleanup unavailable'))
        vi.mocked(download_job_get).mockRejectedValueOnce(new Error('read offline'))
        await expect(manager.cancel('one')).rejects.toThrow('cleanup unavailable')
        backend.jobs.mock.calls[0][0](running)
        expect(manager.jobs.value[0].status).toBe('canceling')
        vi.mocked(download_job_cancel).mockResolvedValueOnce(canceled)
        vi.mocked(download_job_get).mockResolvedValueOnce(canceled)
        await manager.cancel('one')
        expect(download_job_cancel).toHaveBeenCalledTimes(3)
        expect(manager.jobs.value[0].status).toBe('canceled')
        expect(manager.activeJobs.value).toHaveLength(0)
    } finally {
        manager.dispose()
    }
})

it.each(['event', 'refresh'])(
    'allows cleanup retry after a %s confirms a previously signaled cancellation',
    async (confirmation) => {
        const running = job('running', '2026-10-28T00:00:00Z')
        const canceling = job('canceling', running.modified)
        backend.list.mockResolvedValue({ jobs: [running] })
        const manager = createDownloadManager(vi.fn())
        try {
            await manager.start()
            vi.mocked(download_job_cancel).mockResolvedValue(running)
            vi.mocked(download_job_get).mockResolvedValue(running)
            await manager.cancel('one')
            if (confirmation === 'event') backend.jobs.mock.calls[0][0](canceling)
            else {
                backend.list.mockResolvedValue({ jobs: [canceling] })
                await manager.refresh()
            }
            vi.mocked(download_job_cancel).mockResolvedValue(canceling)
            vi.mocked(download_job_get).mockResolvedValue(canceling)
            await manager.cancel('one')
            expect(download_job_cancel).toHaveBeenCalledTimes(2)
            expect(manager.jobs.value[0].status).toBe('canceling')
        } finally {
            manager.dispose()
        }
    },
)

it('reconciles a rejected retry and keeps real errors without reviving an old accepted response', async () => {
    backend.list.mockResolvedValue({ jobs: [job('succeeded', '2026-10-28T00:00:00Z')] })
    const manager = createDownloadManager(vi.fn())
    try {
        await manager.start()
        vi.mocked(download_job_retry).mockRejectedValueOnce(new Error('retry denied'))
        vi.mocked(download_job_get).mockResolvedValue(job('succeeded', '2026-10-28T00:00:00Z'))
        await expect(manager.retry('one')).rejects.toThrow('retry denied')
        expect(download_job_get).toHaveBeenCalledOnce()
        const accepted = deferred<InstallJobSnapshot>()
        vi.mocked(download_job_retry).mockReturnValueOnce(accepted.promise)
        const retry = manager.retry('one')
        backend.jobs.mock.calls[0][0](job('succeeded', '2026-10-28T00:00:01Z'))
        accepted.resolve(job('running', '2026-10-28T00:00:00Z'))
        vi.mocked(download_job_get).mockRejectedValueOnce(new Error('read failed'))
        await retry
        expect(manager.jobs.value[0].status).toBe('succeeded')
    } finally {
        manager.dispose()
    }
})

it('polls stale active jobs, preserves events received during reads and stops on disposal', async () => {
    vi.useFakeTimers()
    backend.list.mockResolvedValue({ jobs: [job('running', '2026-10-28T00:00:00Z')] })
    const manager = createDownloadManager(vi.fn())
    try {
        await manager.start()
        const read = deferred<InstallJobSnapshot | null>()
        vi.mocked(download_job_get).mockReturnValueOnce(read.promise)
        await vi.advanceTimersByTimeAsync(30_000)
        expect(download_job_get).toHaveBeenCalledOnce()
        backend.jobs.mock.calls[0][0](job('succeeded', '2026-10-28T00:00:00Z'))
        read.resolve(job('running', '2026-10-28T00:00:00Z'))
        await flushPromises()
        expect(manager.jobs.value[0].status).toBe('succeeded')
        backend.jobs.mock.calls[0][0](job('running', '2026-10-28T00:00:01Z'))
        vi.mocked(download_job_get).mockResolvedValue(job('succeeded', '2026-10-28T00:00:01Z'))
        await vi.advanceTimersByTimeAsync(30_000)
        expect(manager.activeJobs.value).toHaveLength(0)
        manager.dispose()
        await vi.advanceTimersByTimeAsync(120_000)
        expect(download_job_get).toHaveBeenCalledTimes(2)
    } finally {
        manager.dispose()
        vi.useRealTimers()
    }
})

it('pack archive request progress advances the main bar without overwriting later phases', async () => {
    vi.useFakeTimers()
    const initial: InstallJobSnapshot = {
        ...job('running', '2026-10-21T01:00:00Z'),
        kind: 'create_modpack_instance',
        phase: 'downloading_pack_file',
        progress: { current: 0, total: 100 },
        summary: {
            files_completed: 0,
            bytes_downloaded: 0,
            bytes_total: 100,
            fallback_count: 0,
        },
    }
    backend.list.mockResolvedValue({ jobs: [initial] })
    const manager = createDownloadManager(vi.fn())
    try {
        await manager.start()
        const request = backend.requests.mock.calls[0][0]
        request({
            type: 'started',
            job_id: 'one',
            id: 'pack.mrpack',
            name: 'pack.mrpack',
            url: 'https://cdn.modrinth.com/data/pack.mrpack',
            source: 'official',
            bytes_total: 100,
            attempt: 1,
            max_attempts: 3,
        })
        request({
            type: 'progress',
            job_id: 'one',
            id: 'pack.mrpack',
            bytes: 25,
            status: 'downloading',
            speed_bytes_per_second: 10,
            eta_seconds: 8,
        })
        await vi.advanceTimersByTimeAsync(16)
        expect(manager.jobs.value[0].progress).toEqual({ current: 25, total: 100 })
        expect(manager.jobs.value[0].items[0].bytes_downloaded).toBe(25)

        backend.jobs.mock.calls[0][0]({ ...initial, modified: '2026-10-21T01:00:01Z' })
        expect(manager.jobs.value[0].progress?.current).toBe(25)

        backend.jobs.mock.calls[0][0]({
            ...manager.jobs.value[0],
            phase: 'reading_pack_manifest',
            progress: null,
            modified: '2026-10-21T01:00:02Z',
        })
        request({ type: 'finished', job_id: 'one', id: 'pack.mrpack', bytes: 100 })
        await vi.advanceTimersByTimeAsync(16)
        expect(manager.jobs.value[0].phase).toBe('reading_pack_manifest')
        expect(manager.jobs.value[0].progress).toBeNull()
    } finally {
        manager.dispose()
        vi.useRealTimers()
    }
})

it('subscribes concurrently, snapshots only after every subscription, then replays newer events', async () => {
    const requestListener = deferred<() => void>()
    const jobListener = deferred<() => void>()
    const loadingListener = deferred<() => void>()
    const snapshot = deferred<{ jobs: InstallJobSnapshot[] }>()
    backend.requests.mockReturnValue(requestListener.promise)
    backend.jobs.mockReturnValue(jobListener.promise)
    backend.loading.mockReturnValue(loadingListener.promise)
    backend.list.mockReturnValue(snapshot.promise)
    const manager = createDownloadManager(vi.fn())
    const start = manager.start()
    expect(manager.start()).toBe(start)
    expect(backend.requests).toHaveBeenCalledOnce()
    expect(backend.jobs).toHaveBeenCalledOnce()
    expect(backend.loading).toHaveBeenCalledOnce()
    requestListener.resolve(vi.fn())
    jobListener.resolve(vi.fn())
    await flushPromises()
    expect(backend.list).not.toHaveBeenCalled()
    loadingListener.resolve(vi.fn())
    await flushPromises()
    expect(backend.list).toHaveBeenCalledOnce()
    backend.jobs.mock.calls[0][0](job('succeeded', '2026-10-19T02:00:00Z'))
    snapshot.resolve({ jobs: [job('running', '2026-10-19T01:00:00Z')] })
    await start
    expect(manager.jobs.value[0].status).toBe('succeeded')
    manager.dispose()
})

it('releases subscriptions that resolve after disposal without dispatching snapshots', async () => {
    const late = deferred<() => void>()
    const cleanup = [vi.fn(), vi.fn(), vi.fn()]
    backend.requests.mockReturnValue(late.promise)
    backend.jobs.mockResolvedValue(cleanup[1])
    backend.loading.mockResolvedValue(cleanup[2])
    const manager = createDownloadManager(vi.fn())
    const start = manager.start()
    manager.dispose()
    late.resolve(cleanup[0])
    await start
    for (const dispose of cleanup) expect(dispose).toHaveBeenCalledOnce()
    expect(backend.list).not.toHaveBeenCalled()
    backend.jobs.mock.calls[0][0](job('running', '2026-10-19'))
    expect(manager.jobs.value).toEqual([])
})

it('cleans up partial subscription failure and permits a fresh start', async () => {
    const dispose = vi.fn()
    backend.requests.mockResolvedValue(dispose)
    backend.jobs.mockRejectedValueOnce(new Error('subscription failed'))
    const manager = createDownloadManager(vi.fn())
    await expect(manager.start()).rejects.toThrow('subscription failed')
    expect(dispose).toHaveBeenCalledOnce()
    expect(backend.list).not.toHaveBeenCalled()
    backend.jobs.mockResolvedValue(vi.fn())
    await manager.start()
    expect(backend.list).toHaveBeenCalledOnce()
    manager.dispose()
})

it('keeps failed initial snapshots recoverable instead of presenting an empty ready state', async () => {
    backend.list.mockRejectedValueOnce(new Error('snapshot failed'))
    const manager = createDownloadManager(vi.fn())
    await expect(manager.start()).rejects.toThrow('snapshot failed')
    expect(manager.initializationState.value).toBe('error')
    await manager.start()
    expect(manager.initializationState.value).toBe('ready')
    manager.dispose()
})

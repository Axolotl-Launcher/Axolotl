import { flushPromises } from '@vue/test-utils'
import { beforeEach, expect, it, vi } from 'vitest'

import type { InstallJobSnapshot } from '@/helpers/install'

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
function job(status: 'running' | 'succeeded', modified: string): InstallJobSnapshot {
    return {
        job_id: 'one',
        status,
        modified,
        created: '2026-10-19',
        items: [],
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

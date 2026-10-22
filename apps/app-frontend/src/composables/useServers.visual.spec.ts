import { expect, it, vi } from 'vitest'

const api = vi.hoisted(() => ({
    list: vi.fn(),
    listen: vi.fn().mockResolvedValue(() => {}),
    stop: vi.fn().mockResolvedValue(undefined),
}))
vi.mock('@/helpers/servers', () => ({
    servers: { list: api.list, stop: api.stop },
    serverEventListener: api.listen,
}))
vi.mock('@modrinth/ui', () => ({ injectNotificationManager: () => ({ handleError: vi.fn() }) }))

it('distinguishes initial failure from empty data and preserves the last successful list after a refresh failure', async () => {
    const { useServers } = await import('./useServers')
    const store = useServers()
    api.list.mockRejectedValueOnce(new Error('cannot read servers'))
    await store.refresh()
    expect(store.hasLoaded.value).toBe(false)
    expect(store.loadError.value).toBeTruthy()
    api.list.mockResolvedValueOnce([{ id: 'saved', name: 'Saved server' }])
    await store.refresh()
    expect(store.hasLoaded.value).toBe(true)
    expect(store.rawServers.value[0].name).toBe('Saved server')
    api.list.mockRejectedValueOnce(new Error('refresh failed'))
    await store.refresh()
    expect(store.hasLoaded.value).toBe(true)
    expect(store.loadError.value).toBeTruthy()
    expect(store.rawServers.value[0].id).toBe('saved')
})

it('shares an in-flight refresh so all callers wait for the same committed result', async () => {
    const { useServers } = await import('./useServers')
    const store = useServers()
    let finish!: (value: unknown[]) => void
    api.list.mockImplementationOnce(
        () =>
            new Promise((resolve) => {
                finish = resolve
            }),
    )
    const first = store.refresh()
    const second = store.refresh()
    await vi.waitFor(() => expect(finish).toBeDefined())
    expect(store.isRefreshing.value).toBe(true)
    finish([])
    await Promise.all([first, second])
    expect(store.isRefreshing.value).toBe(false)
    expect(store.loadError.value).toBeNull()
    expect(store.hasLoaded.value).toBe(true)
    expect(store.rawServers.value).toEqual([])
})

it('shows preparation as starting and refreshes the state after cancelling it', async () => {
    const { useServers } = await import('./useServers')
    const store = useServers()
    api.list.mockResolvedValueOnce([
        { id: 'preparing', running: false, starting: true, stopping: false },
    ])
    await store.refresh()
    expect(store.servers.value[0].status).toBe('starting')
    api.list.mockResolvedValueOnce([
        { id: 'preparing', running: false, starting: false, stopping: false, eulaAccepted: true },
    ])
    expect(await store.stopServer('preparing')).toBe(true)
    expect(api.stop).toHaveBeenCalledWith('preparing')
    expect(store.servers.value[0].status).toBe('ready')
})

it('reconciles a stop event received during an older list request', async () => {
    const { useServers } = await import('./useServers')
    const store = useServers()
    let finish!: (value: unknown[]) => void
    api.list.mockReturnValueOnce(
        new Promise((resolve) => {
            finish = resolve
        }),
    )
    api.list.mockResolvedValueOnce([{ id: 'server', running: false, eulaAccepted: true }])
    const calls = api.list.mock.calls.length
    const pending = store.refresh()
    await vi.waitFor(() => expect(api.list.mock.calls.length).toBeGreaterThan(calls))
    const onEvent = api.listen.mock.calls[0][0]
    onEvent('server', { event: 'stopped', crashed: false })
    finish([{ id: 'server', running: true }])
    await pending
    expect(store.servers.value[0].status).toBe('ready')
})

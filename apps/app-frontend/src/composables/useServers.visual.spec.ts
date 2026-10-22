import { expect, it, vi } from 'vitest'

const api = vi.hoisted(() => ({ list: vi.fn(), listen: vi.fn().mockResolvedValue(() => {}) }))
vi.mock('@/helpers/servers', () => ({
    servers: { list: api.list },
    serverEventListener: api.listen,
    base64ToBytes: () => new Uint8Array(),
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

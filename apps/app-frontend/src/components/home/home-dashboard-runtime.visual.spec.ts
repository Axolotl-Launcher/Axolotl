import { flushPromises, mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'

import { type HomeDashboardRuntime, provideHomeDashboardRuntime } from './home-dashboard-runtime'
const backend = vi.hoisted(() => ({
    list: vi.fn(),
    server: vi.fn(),
    instance: vi.fn(),
    process: vi.fn().mockResolvedValue(() => {}),
    processes: vi.fn().mockResolvedValue([]),
    favorites: vi.fn().mockResolvedValue([]),
    recent: vi.fn().mockResolvedValue([]),
    worlds: vi.fn().mockResolvedValue([]),
}))
vi.mock('@/helpers/events', () => ({
    process_listener: backend.process,
    instance_listener: backend.instance,
}))
vi.mock('@/helpers/process', () => ({ get_all: backend.processes }))
vi.mock('@/helpers/tags', () => ({ get_game_versions: async () => [] }))
vi.mock('@/helpers/servers', () => ({
    serverEventListener: backend.server,
    servers: { list: backend.list },
}))
vi.mock('@/helpers/worlds', () => ({
    get_favorite_worlds: backend.favorites,
    get_recent_worlds: backend.recent,
    get_instance_worlds: backend.worlds,
    get_instance_protocol_version: async () => null,
    refreshServerData: async () => {},
}))
it('ignores log/progress bursts and coalesces lifecycle refreshes on the home page', async () => {
    backend.list.mockResolvedValue([])
    backend.server.mockResolvedValue(() => {})
    backend.instance.mockResolvedValue(() => {})
    const wrapper = mount({
        setup() {
            provideHomeDashboardRuntime(vi.fn())
            return () => null
        },
    })
    try {
        await new Promise((resolve) => setTimeout(resolve, 150))
        await flushPromises()
        expect(backend.list).toHaveBeenCalledOnce()
        const callback = backend.server.mock.calls[0][0]
        for (let i = 0; i < 100; i++) {
            for (const event of ['log', 'console_output', 'download_progress'])
                callback('server', { event })
        }
        await flushPromises()
        expect(backend.list).toHaveBeenCalledOnce()
        for (let i = 0; i < 100; i++) callback('server', { event: 'started' })
        callback('server', { event: 'starting' })
        callback('server', { event: 'stopping' })
        await new Promise((resolve) => setTimeout(resolve, 150))
        expect(backend.list).toHaveBeenCalledTimes(2)
    } finally {
        wrapper.unmount()
    }
})

it('refreshes world locks and cached shortcut worlds when a game exits', async () => {
    const world = {
        type: 'singleplayer',
        instance_id: 'instance-one',
        name: 'World',
        path: 'World',
        locked: true,
    }
    backend.process.mockClear()
    backend.processes.mockResolvedValue([{ instance_id: 'instance-one' }])
    backend.favorites.mockResolvedValue([world])
    backend.recent.mockResolvedValue([world])
    backend.worlds.mockClear().mockResolvedValue([world])
    let runtime!: HomeDashboardRuntime
    const wrapper = mount({
        setup() {
            runtime = provideHomeDashboardRuntime(vi.fn())
            return () => null
        },
    })
    try {
        await new Promise((resolve) => setTimeout(resolve, 150))
        await flushPromises()
        expect(runtime.runningInstanceIds.value).toEqual(['instance-one'])
        expect(runtime.recentWorlds.value[0]).toMatchObject({ locked: true })
        await runtime.getInstanceWorlds('instance-one')

        const unlocked = { ...world, locked: false }
        backend.processes.mockResolvedValue([])
        backend.favorites.mockResolvedValue([unlocked])
        backend.recent.mockResolvedValue([unlocked])
        backend.worlds.mockResolvedValue([unlocked])
        await backend.process.mock.calls[0][0]({ event: 'finished' })

        expect(runtime.runningInstanceIds.value).toEqual([])
        expect(runtime.recentWorlds.value[0]).toMatchObject({ locked: false })
        expect(runtime.favoriteWorlds.value[0]).toMatchObject({ locked: false })
        expect(await runtime.getInstanceWorlds('instance-one')).toMatchObject([{ locked: false }])
        expect(backend.worlds).toHaveBeenCalledTimes(2)
    } finally {
        wrapper.unmount()
    }
})

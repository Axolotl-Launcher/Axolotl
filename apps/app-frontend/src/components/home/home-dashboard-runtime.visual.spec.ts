import { flushPromises, mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'

import { provideHomeDashboardRuntime } from './home-dashboard-runtime'
const backend = vi.hoisted(() => ({ list: vi.fn(), server: vi.fn(), instance: vi.fn() }))
vi.mock('@/helpers/events', () => ({
    process_listener: async () => () => {},
    instance_listener: backend.instance,
}))
vi.mock('@/helpers/process', () => ({ get_all: async () => [] }))
vi.mock('@/helpers/tags', () => ({ get_game_versions: async () => [] }))
vi.mock('@/helpers/servers', () => ({
    serverEventListener: backend.server,
    servers: { list: backend.list },
}))
vi.mock('@/helpers/worlds', () => ({
    get_favorite_worlds: async () => [],
    get_recent_worlds: async () => [],
    get_instance_worlds: async () => [],
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

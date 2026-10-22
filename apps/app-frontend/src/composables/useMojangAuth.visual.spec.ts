import { focusManager, QueryClient, VueQueryPlugin } from '@tanstack/vue-query'
import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { h, ref } from 'vue'

import { mojangAuthKeys } from '@/helpers/mojang-auth/monitor'

import { setupMojangAuthMonitor, useMojangAuthStatus } from './useMojangAuth'
import { useNetworkStatus } from './useNetworkStatus'

const backend = vi.hoisted(() => ({
    configuration: vi.fn(),
    scope: vi.fn(),
    official: vi.fn(),
    mirror: vi.fn(),
    apply: vi.fn(),
    selected: vi.fn(),
    listen: vi.fn(),
    dispose: vi.fn(),
}))
vi.mock('@/helpers/mojang-auth', () => ({
    getMojangAuthConfiguration: backend.configuration,
    getNetworkScope: backend.scope,
    probeOfficialMojangServices: backend.official,
    probeMirrorMojangServices: backend.mirror,
    applyMojangAuthSource: backend.apply,
    probeSelectedMojangSource: backend.selected,
}))
vi.mock('@tauri-apps/api/event', () => ({ listen: backend.listen }))

const healthy = () =>
    ['account', 'session', 'services', 'profiles'].map((service) => ({ service, reachable: true }))
const mounted: ReturnType<typeof mount>[] = []
const clients: QueryClient[] = []
beforeEach(() => {
    vi.useFakeTimers()
    backend.scope.mockReset().mockResolvedValue('network-1')
    backend.configuration.mockReset().mockResolvedValue({ source: 'auto', key: 'auto' })
    backend.official.mockReset().mockResolvedValue(healthy())
    backend.mirror.mockReset().mockResolvedValue(healthy())
    backend.apply.mockReset().mockResolvedValue(true)
    backend.selected.mockReset().mockResolvedValue(undefined)
    backend.dispose.mockReset()
    backend.listen.mockReset().mockResolvedValue(backend.dispose)
    useNetworkStatus().markBrowserOnline()
    focusManager.setFocused(true)
})
afterEach(() => {
    mounted.splice(0).forEach((wrapper) => wrapper.unmount())
    clients.splice(0).forEach((client) => client.clear())
    focusManager.setFocused(undefined)
    vi.useRealTimers()
})

function app(ready = ref(false)) {
    const client = new QueryClient({ defaultOptions: { queries: { gcTime: Infinity } } })
    clients.push(client)
    const consumer = {
        setup() {
            useMojangAuthStatus()
            return () => h('span', 'skin page')
        },
    }
    const wrapper = mount(
        {
            setup() {
                setupMojangAuthMonitor(ready)
                return () => h('main', [h('span', 'local shell ready'), h(consumer), h(consumer)])
            },
        },
        { global: { plugins: [[VueQueryPlugin, { queryClient: client }]] } },
    )
    mounted.push(wrapper)
    return { wrapper, ready, client }
}

it('renders the local shell before probing and consumers do not start duplicate probes', async () => {
    let finish!: (value: ReturnType<typeof healthy>) => void
    backend.official.mockReturnValueOnce(
        new Promise((resolve) => {
            finish = resolve
        }),
    )
    const { wrapper, ready } = app()
    await flushPromises()
    expect(wrapper.text()).toContain('local shell ready')
    expect(backend.official).not.toHaveBeenCalled()
    ready.value = true
    await flushPromises()
    expect(backend.official).toHaveBeenCalledOnce()
    expect(backend.apply).not.toHaveBeenCalled()
    expect(wrapper.text()).toContain('local shell ready')
    finish(healthy())
    await flushPromises()
    expect(backend.apply).toHaveBeenCalledWith('auto', false)
    expect(useMojangAuthStatus().reachable.value).toBe(true)
})

it('uses one retry scheduler and restores connectivity without reopening a page', async () => {
    backend.official.mockResolvedValueOnce([])
    backend.selected.mockRejectedValueOnce(new Error('offline'))
    app(ref(true))
    await flushPromises()
    expect(backend.apply).toHaveBeenNthCalledWith(1, 'auto', true)
    expect(useMojangAuthStatus().reachable.value).toBe(false)
    expect(useNetworkStatus().offline.value).toBe(false)
    await vi.advanceTimersByTimeAsync(14_999)
    expect(backend.official).toHaveBeenCalledOnce()
    await vi.advanceTimersByTimeAsync(1)
    await flushPromises()
    expect(backend.official).toHaveBeenCalledTimes(2)
    expect(backend.apply).toHaveBeenLastCalledWith('auto', false)
    expect(useNetworkStatus().offline.value).toBe(false)
    await vi.advanceTimersByTimeAsync(299_999)
    expect(backend.official).toHaveBeenCalledTimes(2)
    await vi.advanceTimersByTimeAsync(1)
    await flushPromises()
    expect(backend.official).toHaveBeenCalledTimes(3)
})

it('an unreachable authentication service does not disable unrelated network features', async () => {
    backend.configuration.mockResolvedValue({ source: 'official_only', key: 'official_only' })
    backend.selected.mockRejectedValue(new Error('Mojang unavailable'))
    app(ref(true))
    await flushPromises()
    expect(useMojangAuthStatus().reachable.value).toBe(false)
    expect(useNetworkStatus().browserOffline.value).toBe(false)
    expect(useNetworkStatus().offline.value).toBe(false)
    useNetworkStatus().setBrowserOffline(true)
    await flushPromises()
    expect(useNetworkStatus().offline.value).toBe(true)
})

it('pauses while browser-offline, checks on reconnect, and ignores unrelated settings events', async () => {
    app(ref(true))
    await flushPromises()
    useNetworkStatus().setBrowserOffline(true)
    await vi.advanceTimersByTimeAsync(5 * 60_000)
    expect(backend.official).toHaveBeenCalledOnce()
    useNetworkStatus().markBrowserOnline()
    await flushPromises()
    expect(backend.official).toHaveBeenCalledTimes(2)
    const onSettings = backend.listen.mock.calls[0][1]
    onSettings()
    await flushPromises()
    expect(backend.official).toHaveBeenCalledTimes(2)
    backend.configuration.mockResolvedValue({ source: 'official_only', key: 'official_only' })
    onSettings()
    await flushPromises()
    expect(backend.apply).toHaveBeenLastCalledWith('official_only', false)
    expect(backend.official).toHaveBeenCalledTimes(2)
})

it('disposes subscriptions that finish registering after the app unmounts', async () => {
    let finish!: (dispose: () => void) => void
    backend.listen.mockReturnValueOnce(
        new Promise((resolve) => {
            finish = resolve
        }),
    )
    const { wrapper } = app()
    wrapper.unmount()
    mounted.splice(mounted.indexOf(wrapper), 1)
    finish(backend.dispose)
    await flushPromises()
    expect(backend.dispose).toHaveBeenCalledOnce()
    await vi.advanceTimersByTimeAsync(10 * 60_000)
    expect(backend.official).not.toHaveBeenCalled()
})

it('manual account recovery uses the same worker even while a periodic probe is pending', async () => {
    let finish!: (value: ReturnType<typeof healthy>) => void
    backend.official.mockReturnValueOnce(
        new Promise((resolve) => {
            finish = resolve
        }),
    )
    const { client } = app(ref(true))
    await flushPromises()
    const refresh = client.refetchQueries(
        { queryKey: mojangAuthKeys.status, exact: true },
        { cancelRefetch: false },
    )
    expect(backend.official).toHaveBeenCalledOnce()
    finish(healthy())
    await refresh
    await flushPromises()
    expect(backend.selected).toHaveBeenCalledOnce()
    expect(useMojangAuthStatus().reachable.value).toBe(true)
})

it('checks again when the system proxy changes without a settings event', async () => {
    app(ref(true))
    await flushPromises()
    expect(backend.official).toHaveBeenCalledOnce()
    backend.scope.mockResolvedValue('network-2')
    backend.configuration.mockResolvedValue({ source: 'auto', key: 'auto-network-2' })
    await vi.advanceTimersByTimeAsync(5_000)
    await flushPromises()
    expect(backend.official).toHaveBeenCalledTimes(2)
    await vi.advanceTimersByTimeAsync(5_000)
    await flushPromises()
    expect(backend.official).toHaveBeenCalledTimes(2)
})

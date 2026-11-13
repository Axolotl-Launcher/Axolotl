import '@/test/visual-styles.scss'

import { tooltipDirective } from '@modrinth/ui/directives/tooltip'
import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import { defineComponent, h } from 'vue'
import { createMemoryHistory, createRouter, type RouteRecordRaw, RouterView } from 'vue-router'

import { useNetworkStatus } from '@/composables/useNetworkStatus'
import chinese from '@/locales/zh-CN/index.json'
import { discoverRoutes } from '@/routes/discover'
import { utilityRoutes } from '@/routes/utility'

import NetworkOffline from './NetworkOffline.vue'
import NetworkPageBoundary from './NetworkPageBoundary.vue'
import NetworkStatusBar from './NetworkStatusBar.vue'

const native = vi.hoisted(() => ({ openUrl: vi.fn(), platform: 'windows' }))
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: native.openUrl }))
vi.mock('@tauri-apps/plugin-os', () => ({ platform: () => native.platform }))
vi.mock('@modrinth/ui', async () => ({
    Button: (await import('../../../../../../packages/ui/src/components/base/buttons/Button.vue'))
        .default,
    defineMessages: (messages: unknown) => messages,
    useVIntl: () => ({
        formatMessage: (message: { id: string; defaultMessage: string }) =>
            (chinese as Record<string, { message: string }>)[message.id]?.message ??
            message.defaultMessage,
    }),
}))

const network = useNetworkStatus()
const wrappers: ReturnType<typeof mount>[] = []
afterEach(async () => {
    wrappers.splice(0).forEach((wrapper) => wrapper.unmount())
    if (network.retrying.value) await network.retryConnection()
    network.setSimulatedOffline(false)
    network.markBrowserOnline()
    vi.restoreAllMocks()
    native.openUrl.mockReset()
    native.platform = 'windows'
})

async function render(path = '/browse/mod?q=retained&i=instance') {
    const initialized = vi.fn()
    const RemotePage = defineComponent({
        setup() {
            initialized()
            return () => h('div', 'Page content')
        },
    })
    const routes = [...discoverRoutes, ...utilityRoutes].map((route) =>
        'redirect' in route
            ? route
            : ({ ...route, component: RemotePage, children: undefined } as RouteRecordRaw),
    )
    const router = createRouter({ history: createMemoryHistory(), routes })
    await router.push(path)
    const blocked = vi.fn()
    const Host = defineComponent({
        setup: () => () =>
            h('div', { style: 'height:600px;display:grid' }, [
                h(RouterView, null, {
                    default: ({
                        Component,
                        route,
                    }: {
                        Component: unknown
                        route: { meta: { requiresNetwork?: boolean } }
                    }) =>
                        h(
                            NetworkPageBoundary,
                            {
                                requiresNetwork: !!route.meta.requiresNetwork,
                                onBlocked: blocked,
                            },
                            { default: () => Component },
                        ),
                }),
                h(NetworkStatusBar),
            ]),
    })
    const wrapper = mount(Host, {
        attachTo: document.body,
        global: { plugins: [router], directives: { tooltip: tooltipDirective } },
    })
    wrappers.push(wrapper)
    await flushPromises()
    return { wrapper, router, initialized, blocked }
}

it.each([
    '/browse/mod?q=retained&i=instance',
    '/browse/schematic',
    '/browse/favorites',
    '/project/abc',
    '/create',
])(
    'enters %s offline without mounting remote content and recovers on the same route',
    async (path) => {
        network.setBrowserOffline(true)
        const { wrapper, router, initialized, blocked } = await render(path)
        expect(router.currentRoute.value.fullPath).toBe(path)
        expect(wrapper.get('h1').text()).toBe('无法连接')
        expect(initialized).not.toHaveBeenCalled()
        expect(blocked).toHaveBeenCalledOnce()
        window.dispatchEvent(new Event('online'))
        await flushPromises()
        expect(wrapper.find('h1').exists()).toBe(false)
        expect(initialized).toHaveBeenCalledOnce()
        expect(router.currentRoute.value.fullPath).toBe(path)
        window.dispatchEvent(new Event('offline'))
        await flushPromises()
        expect(wrapper.get('h1').text()).toBe('无法连接')
        expect(router.currentRoute.value.fullPath).toBe(path)
    },
)

it('allows navigation between offline pages and keeps local pages accessible', async () => {
    network.setBrowserOffline(true)
    const { wrapper, router, initialized } = await render()
    await router.push('/create')
    await flushPromises()
    expect(wrapper.get('h1').text()).toBe('无法连接')
    expect(initialized).not.toHaveBeenCalled()
    await router.push('/worlds')
    await flushPromises()
    expect(wrapper.text()).toContain('Page content')
    expect(wrapper.find('h1').exists()).toBe(false)
    expect(wrapper.find('[aria-label="已断开连接"]').exists()).toBe(true)
})

it('opens Windows network settings, reports failure, and disables concurrent retries', async () => {
    network.setSimulatedOffline(true)
    const { wrapper } = await render()
    const settings = wrapper.findAll('button')[0]
    native.openUrl.mockRejectedValueOnce(new Error('native settings unavailable'))
    await settings.trigger('click')
    await flushPromises()
    expect(native.openUrl).toHaveBeenCalledWith('ms-settings:network-status')
    expect(wrapper.get('[role="alert"]').text()).toContain('无法打开网络设置')
    native.openUrl.mockResolvedValueOnce(undefined)
    await settings.trigger('click')
    await flushPromises()
    expect(wrapper.find('[role="alert"]').exists()).toBe(false)
    const retry = wrapper.findAll('button')[1]
    await retry.trigger('click')
    expect(retry.attributes('disabled')).toBeDefined()
    expect(retry.attributes('aria-busy')).toBe('true')
    await vi.waitFor(() => expect(retry.attributes('disabled')).toBeUndefined())
    expect(wrapper.get('h1').text()).toBe('无法连接')
    network.setSimulatedOffline(false)
    await flushPromises()
    expect(wrapper.findComponent(NetworkOffline).exists()).toBe(false)
})

it('shows the floating indicator tooltip and removes it when simulation is disabled', async () => {
    network.setSimulatedOffline(true)
    const { wrapper } = await render()
    const indicator = wrapper.get('[aria-label="已断开连接"]')
    await indicator.trigger('mouseenter')
    await vi.waitFor(() =>
        expect(document.querySelector('[role="tooltip"]')?.textContent).toBe('已断开连接'),
    )
    network.setSimulatedOffline(false)
    await flushPromises()
    expect(wrapper.find('[aria-label="已断开连接"]').exists()).toBe(false)
    await vi.waitFor(() => expect(document.querySelector('[role="tooltip"]')).toBeNull())
})

it.each(['macos', 'linux'])(
    'only offers retry on %s without navigating to launcher settings',
    async (os) => {
        native.platform = os
        network.setSimulatedOffline(true)
        const { wrapper, router } = await render()
        const buttons = wrapper.findAll('button')
        expect(buttons).toHaveLength(1)
        expect(buttons[0].text()).toBe('重试')
        expect(wrapper.text()).not.toContain('网络连接设置')
        await buttons[0].trigger('click')
        expect(buttons[0].attributes('disabled')).toBeDefined()
        await vi.waitFor(() => expect(buttons[0].attributes('disabled')).toBeUndefined())
        expect(router.currentRoute.value.fullPath).toBe('/browse/mod?q=retained&i=instance')
        expect(native.openUrl).not.toHaveBeenCalled()
    },
)

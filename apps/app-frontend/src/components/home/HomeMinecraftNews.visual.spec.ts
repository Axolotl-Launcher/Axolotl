import { QueryClient, VueQueryPlugin } from '@tanstack/vue-query'
import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'

import { useNetworkStatus } from '@/composables/useNetworkStatus'
import type { MinecraftNewsItem } from '@/helpers/mc_news'

import HomeMinecraftNews from './HomeMinecraftNews.vue'

const backend = vi.hoisted(() => ({ news: vi.fn() }))
vi.mock('@/helpers/mc_news', () => ({ get_minecraft_news: backend.news }))
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn() }))
vi.mock('@modrinth/ui', () => ({
    defineMessages: (messages: unknown) => messages,
    injectNotificationManager: () => ({ handleError: vi.fn() }),
    useFormatDateTime: () => (value: string) => value,
    useVIntl: () => ({
        formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
    }),
}))

const network = useNetworkStatus()
const article: MinecraftNewsItem = {
    title: 'Minecraft &amp; updates',
    date: '2026-10-19T00:00:00Z',
    read_more_url: 'https://www.minecraft.net/en-us/article/test',
}
let wrapper: ReturnType<typeof mount> | undefined
let queryClient: QueryClient
function mountNews() {
    wrapper = mount(HomeMinecraftNews, {
        global: {
            plugins: [[VueQueryPlugin, { queryClient }]],
            directives: { tooltip: () => {} },
        },
    })
    return wrapper
}

beforeEach(() => {
    backend.news.mockReset()
    backend.news.mockResolvedValue([article])
    queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })
    network.setBrowserOffline(false)
    network.setNetworkReachable(false)
})
afterEach(() => {
    wrapper?.unmount()
    wrapper = undefined
    queryClient.clear()
    network.setBrowserOffline(false)
    network.setNetworkReachable(true)
})

it.each(['browser', 'native'] as const)(
    'loads news in place when %s connectivity recovers, without fetching while offline',
    async (source) => {
        if (source === 'browser') network.setBrowserOffline(true)
        let finish!: (items: MinecraftNewsItem[]) => void
        backend.news.mockReturnValue(
            new Promise((resolve) => {
                finish = resolve
            }),
        )
        const view = mountNews()
        await flushPromises()
        expect(backend.news).not.toHaveBeenCalled()
        expect(view.find('section').exists()).toBe(false)

        if (source === 'browser') window.dispatchEvent(new Event('online'))
        else network.setNetworkReachable(true)
        await flushPromises()
        expect(backend.news).toHaveBeenCalledExactlyOnceWith(12)
        expect(view.find('section').exists()).toBe(true)
        expect(view.find('ul[aria-hidden="true"]').exists()).toBe(true)
        finish([article])
        await flushPromises()
        expect(view.text()).toContain('Minecraft & updates')
        expect(view.find('ul[aria-hidden="true"]').exists()).toBe(false)
    },
)

it('retries a failed initial request after reconnecting without remounting the component', async () => {
    network.setNetworkReachable(true)
    backend.news.mockRejectedValueOnce(new Error('offline request failed'))
    const view = mountNews()
    await flushPromises()
    expect(backend.news).toHaveBeenCalledOnce()
    expect(view.find('section').exists()).toBe(false)

    network.setNetworkReachable(false)
    await flushPromises()
    network.setNetworkReachable(true)
    await flushPromises()
    expect(backend.news).toHaveBeenCalledTimes(2)
    expect(view.text()).toContain('Minecraft & updates')
})

it('reuses fresh cached news across reconnects and page remounts', async () => {
    network.setNetworkReachable(true)
    const view = mountNews()
    await flushPromises()
    expect(backend.news).toHaveBeenCalledOnce()
    network.setNetworkReachable(false)
    await flushPromises()
    expect(view.find('section').exists()).toBe(false)
    network.setNetworkReachable(true)
    await flushPromises()
    expect(view.text()).toContain('Minecraft & updates')
    view.unmount()
    mountNews()
    await flushPromises()
    expect(backend.news).toHaveBeenCalledOnce()
    expect(wrapper!.text()).toContain('Minecraft & updates')
})

it('does not request news on reconnect after leaving the page', async () => {
    mountNews().unmount()
    wrapper = undefined
    network.setNetworkReachable(true)
    await flushPromises()
    expect(backend.news).not.toHaveBeenCalled()
})

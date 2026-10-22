import { QueryClient, VueQueryPlugin } from '@tanstack/vue-query'
import { flushPromises, mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'

import HomePlayInsights from './HomePlayInsights.vue'

const backend = vi.hoisted(() => ({ playtime: vi.fn(), listen: vi.fn() }))
vi.mock('@/helpers/instance', () => ({ get_daily_playtime: backend.playtime }))
vi.mock('@/helpers/events', () => ({ process_listener: backend.listen }))
vi.mock('@modrinth/ui', async () => {
    const { defineComponent, h } = await import('vue')
    return {
        Button: defineComponent({
            setup:
                (_, { slots }) =>
                () =>
                    h('button', slots.default?.()),
        }),
        commonMessages: { retryButton: { defaultMessage: 'Retry' } },
        defineMessages: (messages: unknown) => messages,
        useVIntl: () => ({
            formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
        }),
    }
})

it('renders without Suspense while statistics load and releases a late process listener', async () => {
    let finish!: (result: unknown[]) => void
    let subscribed!: (dispose: () => void) => void
    backend.playtime.mockReturnValue(
        new Promise((resolve) => {
            finish = resolve
        }),
    )
    backend.listen.mockReturnValue(
        new Promise((resolve) => {
            subscribed = resolve
        }),
    )
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })
    const wrapper = mount(HomePlayInsights, {
        global: { plugins: [[VueQueryPlugin, { queryClient }]] },
    })
    expect(wrapper.find('section').exists()).toBe(true)
    expect(wrapper.text()).not.toContain('Play something')
    finish([])
    await flushPromises()
    expect(wrapper.text()).toContain('Play something')
    wrapper.unmount()
    const dispose = vi.fn()
    subscribed(dispose)
    await flushPromises()
    expect(dispose).toHaveBeenCalledOnce()
    queryClient.clear()
})

it('offers a local retry for failed statistics instead of displaying an empty result', async () => {
    backend.playtime.mockRejectedValueOnce(new Error('stats unavailable')).mockResolvedValue([])
    backend.listen.mockResolvedValue(vi.fn())
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })
    const wrapper = mount(HomePlayInsights, {
        global: { plugins: [[VueQueryPlugin, { queryClient }]] },
    })
    try {
        await flushPromises()
        expect(wrapper.text()).not.toContain('Play something')
        await wrapper.get('button').trigger('click')
        await flushPromises()
        expect(wrapper.text()).toContain('Play something')
    } finally {
        wrapper.unmount()
        queryClient.clear()
    }
})

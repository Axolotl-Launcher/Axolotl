import '@/test/visual-styles.scss'

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
        useFormatDateTime: () => (date: Date) => date.toDateString(),
        useVIntl: () => ({
            locale: { value: 'en-US' },
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
        global: {
            plugins: [[VueQueryPlugin, { queryClient }]],
            directives: { tooltip: () => {} },
        },
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
        global: {
            plugins: [[VueQueryPlugin, { queryClient }]],
            directives: { tooltip: () => {} },
        },
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

it('leads with the week as one big figure over a chart along the card’s foot', async () => {
    const today = new Date()
    const key = (date: Date) =>
        `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`
    backend.playtime.mockResolvedValue([
        { date: key(today), played_seconds: 2 * 3600, session_count: 1, top_instance_name: 'FO' },
    ])
    backend.listen.mockResolvedValue(vi.fn())
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })
    const wrapper = mount(HomePlayInsights, {
        attachTo: document.body,
        global: {
            plugins: [[VueQueryPlugin, { queryClient }]],
            directives: { tooltip: () => {} },
        },
    })
    try {
        await flushPromises()
        expect(wrapper.get('.home-insights-figure strong').text()).toBe('2')
        const days = wrapper.findAll('.home-insights-day')
        expect(days).toHaveLength(7)
        expect(wrapper.findAll('.home-insights-day.is-today .home-insights-bar')).toHaveLength(1)
        // The chart runs into the card's padding at its foot.
        const chart = getComputedStyle(wrapper.get('.home-insights-week').element)
        expect(Number.parseFloat(chart.marginBottom)).toBeLessThan(0)
    } finally {
        wrapper.unmount()
        queryClient.clear()
    }
})

it('draws bar heights strictly in proportion: one minute, an hour and ten hours', async () => {
    const monday = new Date()
    monday.setHours(12, 0, 0, 0)
    monday.setDate(monday.getDate() - ((monday.getDay() + 6) % 7))
    const key = (offset: number) => {
        const date = new Date(monday)
        date.setDate(monday.getDate() + offset)
        return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`
    }
    backend.playtime.mockResolvedValue([
        { date: key(0), played_seconds: 60, session_count: 1, top_instance_name: 'FO' },
        { date: key(1), played_seconds: 3600, session_count: 1, top_instance_name: 'FO' },
        { date: key(2), played_seconds: 36000, session_count: 1, top_instance_name: 'FO' },
    ])
    backend.listen.mockResolvedValue(vi.fn())
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })
    const wrapper = mount(HomePlayInsights, {
        attachTo: document.body,
        global: {
            plugins: [[VueQueryPlugin, { queryClient }]],
            directives: { tooltip: () => {} },
        },
    })
    try {
        await flushPromises()
        const heights = wrapper
            .findAll('.home-insights-day')
            .slice(0, 3)
            .map((day) => day.get('.home-insights-bar').element.getBoundingClientRect().height)
        const plot = wrapper.get('.home-insights-plot').element.getBoundingClientRect().height
        // Ten hours fills the plot, an hour is a tenth of it, a minute only the visible floor.
        expect(heights[2]).toBeCloseTo(plot, 0)
        expect(heights[1]).toBeCloseTo(plot / 10, 0)
        expect(heights[0]).toBeCloseTo(2, 0)
        expect(heights[0]).toBeLessThan(heights[1])
        // Values sit over their bars, outside them.
        const bar = wrapper.findAll('.home-insights-bar')[2].element.getBoundingClientRect()
        const value = wrapper.get('.home-insights-value.is-peak').element.getBoundingClientRect()
        expect(value.bottom).toBeLessThanOrEqual(bar.top + 0.5)
    } finally {
        wrapper.unmount()
        queryClient.clear()
    }
})

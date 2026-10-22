import { QueryClient, VueQueryPlugin } from '@tanstack/vue-query'
import { flushPromises, mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { defineComponent, h } from 'vue'

import type { GameInstance } from '@/helpers/types'

import HomeMinimal from './HomeMinimal.vue'

const backend = vi.hoisted(() => ({ processes: vi.fn(), listen: vi.fn() }))
vi.mock('@/helpers/process', () => ({ get_by_instance_id: backend.processes }))
vi.mock('@/helpers/events', () => ({ process_listener: backend.listen }))
vi.mock('@/helpers/instance', () => ({ kill: vi.fn(), run: vi.fn() }))
vi.mock('@/helpers/install', () => ({
    install_existing_instance: vi.fn(),
    install_pack_to_existing_instance: vi.fn(),
}))
vi.mock('@/helpers/analytics', () => ({ trackEvent: vi.fn() }))
vi.mock('@/store/error', () => ({ handleSevereError: vi.fn() }))
vi.mock('@/composables/useMinecraftLaunchError', () => ({ useMinecraftLaunchError: () => vi.fn() }))
vi.mock('@/composables/useNetworkStatus', async () => {
    const { ref } = await import('vue')
    return { useNetworkStatus: () => ({ offline: ref(false) }) }
})
vi.mock('@/components/home/HomeGreeting.vue', () => ({ default: { render: () => null } }))
vi.mock('@/components/ui/InstanceIcon.vue', () => ({ default: { render: () => null } }))
vi.mock('@modrinth/ui', async () => {
    const { defineComponent, h } = await import('vue')
    return {
        Button: defineComponent({
            setup:
                (_, { slots }) =>
                () =>
                    h('button', slots.default?.()),
        }),
        Card: defineComponent({
            setup:
                (_, { slots }) =>
                () =>
                    h('div', slots.default?.()),
        }),
        commonMessages: {
            retryButton: { defaultMessage: 'Retry' },
            playButton: { defaultMessage: 'Play' },
        },
        defineMessages: (messages: unknown) => messages,
        injectNotificationManager: () => ({ handleError: vi.fn() }),
        useRelativeTime: () => () => 'now',
        useVIntl: () => ({
            formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
        }),
    }
})

it('keeps the minimal home synchronous and makes a failed background process refresh retryable', async () => {
    backend.processes.mockResolvedValue([])
    backend.listen.mockResolvedValue(vi.fn())
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })
    const instance = {
        id: 'one',
        name: 'One',
        install_stage: 'installed',
        loader: 'vanilla',
        game_version: '1.21',
    } as GameInstance
    const wrapper = mount(HomeMinimal, {
        props: { instances: [instance], selectedInstanceId: instance.id },
        global: {
            plugins: [[VueQueryPlugin, { queryClient }]],
            directives: { tooltip: () => {} },
            stubs: {
                RouterLink: defineComponent({
                    setup:
                        (_, { slots }) =>
                        () =>
                            h('a', slots.default?.()),
                }),
            },
        },
    })
    try {
        expect(wrapper.find('section').exists()).toBe(true)
        await flushPromises()
        expect(wrapper.text()).toContain('Play')
        backend.processes.mockRejectedValueOnce(new Error('refresh failed'))
        await queryClient.refetchQueries({ queryKey: ['home', 'instance-processes', instance.id] })
        await flushPromises()
        const retry = wrapper.findAll('button').find((button) => button.text() === 'Retry')!
        expect(retry).toBeDefined()
        await retry.trigger('click')
        await flushPromises()
        expect(wrapper.text()).toContain('Play')
        expect(wrapper.text()).not.toContain('Retry')
    } finally {
        wrapper.unmount()
        queryClient.clear()
    }
})

import { QueryClient, VueQueryPlugin } from '@tanstack/vue-query'
import { mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { defineComponent, h } from 'vue'

import { DIRECT_LINKS_CHANGED_EVENT } from '@/helpers/direct-link-sync'
import { list } from '@/helpers/instance'
import type { GameInstance } from '@/helpers/types'

import { useInstanceList, useInstanceListUpdates } from './use-instance-list'

const events = vi.hoisted(() => ({
    instance: vi.fn<(callback: (event: { event: string }) => void) => Promise<() => void>>(),
    groups: vi.fn<(callback: () => void) => Promise<() => void>>(),
}))

vi.mock('@/helpers/events', () => ({
    instance_listener: events.instance,
    instance_groups_listener: events.groups,
}))
vi.mock('@/helpers/instance', async (importOriginal) => ({
    ...(await importOriginal<typeof import('@/helpers/instance')>()),
    list: vi.fn(),
}))

it('shares the list and coalesces notifications without losing changes during a read', async () => {
    events.instance.mockResolvedValue(() => {})
    events.groups.mockResolvedValue(() => {})
    const read = vi.mocked(list)
    const snapshot = (name: string) => [{ id: 'instance', name }] as GameInstance[]
    read.mockReset().mockResolvedValueOnce(snapshot('First'))
    const delayed = Promise.withResolvers<GameInstance[]>()
    read.mockReturnValueOnce(delayed.promise).mockResolvedValueOnce(snapshot('Final'))

    const client = new QueryClient()
    const onError = vi.fn()
    const observer = defineComponent({
        setup() {
            const { data } = useInstanceList(onError)
            return () => h('span', data.value?.[0]?.name)
        },
    })
    const root = defineComponent({
        setup() {
            useInstanceListUpdates(client, onError)
            return () => h('main', [h(observer), h(observer)])
        },
    })
    const wrapper = mount(root, {
        global: { plugins: [[VueQueryPlugin, { queryClient: client }]] },
    })

    try {
        await vi.waitFor(() => expect(wrapper.text()).toBe('FirstFirst'))
        expect(read).toHaveBeenCalledTimes(1)

        const onInstance = events.instance.mock.calls[0][0]
        for (let i = 0; i < 66; i++) onInstance({ event: 'edited' })
        window.dispatchEvent(new Event(DIRECT_LINKS_CHANGED_EVENT))
        await vi.waitFor(() => expect(read).toHaveBeenCalledTimes(2))
        expect(wrapper.text()).toBe('FirstFirst')

        onInstance({ event: 'removed' })
        events.groups.mock.calls[0][0]()
        expect(read).toHaveBeenCalledTimes(2)

        delayed.resolve(snapshot('Intermediate'))
        await vi.waitFor(() => expect(wrapper.text()).toBe('FinalFinal'))
        expect(read).toHaveBeenCalledTimes(3)
        expect(onError).not.toHaveBeenCalled()
    } finally {
        wrapper.unmount()
        client.clear()
    }
})

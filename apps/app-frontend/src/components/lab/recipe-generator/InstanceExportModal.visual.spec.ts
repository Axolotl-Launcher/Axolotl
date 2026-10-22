import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'

import InstanceExportModal from './InstanceExportModal.vue'

const backend = vi.hoisted(() => ({
    list: vi.fn(),
    worlds: vi.fn(),
}))

vi.mock('@/helpers/instance', () => ({ list: backend.list }))
vi.mock('@/helpers/worlds.ts', () => ({
    get_instance_worlds: backend.worlds,
    isSingleplayerWorld: (world: { type?: string }) => world.type !== 'server',
    sortWorlds: (worlds: unknown[]) => worlds,
}))
vi.mock('@/components/ui/InstanceIcon.vue', () => ({ default: { render: () => null } }))
vi.mock('@modrinth/ui', async () => {
    const { defineComponent, h, ref } = await import('vue')
    return {
        Avatar: defineComponent({ setup: () => () => h('span') }),
        Button: defineComponent({
            setup:
                (_, { slots }) =>
                () =>
                    h('button', slots.default?.()),
        }),
        NewModal: defineComponent({
            props: ['disableClose'],
            setup: (props, { slots, expose }) => {
                const open = ref(false)
                expose({ show: () => (open.value = true), hide: () => (open.value = false) })
                return () =>
                    open.value
                        ? h('dialog', { open: true, 'data-disable-close': props.disableClose }, [
                              slots.default?.(),
                              slots.actions?.(),
                          ])
                        : null
            },
        }),
        defineMessages: (messages: unknown) => messages,
        useRelativeTime: () => () => 'recently',
        useVIntl: () => ({
            formatMessage: (
                message: { defaultMessage: string },
                values?: Record<string, string>,
            ) => {
                let result = message.defaultMessage
                for (const [key, value] of Object.entries(values ?? {}))
                    result = result.replace(`{${key}}`, value)
                return result
            },
        }),
    }
})

const mounted: ReturnType<typeof mount>[] = []
afterEach(() => mounted.splice(0).forEach((wrapper) => wrapper.unmount()))

const instance = {
    id: 'instance-a',
    name: 'Instance A',
    install_stage: 'installed',
    game_version: '1.21.1',
    loader: 'neoforge',
}

async function openModal(
    onInstall: (target: { instanceId: string; worldPath: string }) => Promise<void>,
) {
    backend.list.mockResolvedValue([instance])
    backend.worlds.mockResolvedValue([
        { name: 'World A', path: 'saves/world-a', type: 'singleplayer' },
    ])
    const wrapper = mount(InstanceExportModal, {
        props: { onInstall, showSaveAs: false },
    })
    mounted.push(wrapper)
    await wrapper.vm.show()
    await flushPromises()
    await wrapper.get('button').trigger('click')
    await flushPromises()
    return wrapper
}

it('keeps the selected world and modal open when installation fails, then allows retry', async () => {
    let attempts = 0
    const wrapper = await openModal(async () => {
        attempts++
        if (attempts === 1) throw new Error('install failed')
    })

    await wrapper.get('button[aria-label="Install datapack into World A"]').trigger('click')
    await flushPromises()
    expect(wrapper.get('dialog').exists()).toBe(true)
    expect(wrapper.get('[role="alert"]').text()).toContain('install failed')
    expect(wrapper.get('button[aria-label="Install datapack into World A"]').exists()).toBe(true)

    await wrapper.get('button[aria-label="Install datapack into World A"]').trigger('click')
    await flushPromises()
    expect(wrapper.find('dialog').exists()).toBe(false)
    expect(attempts).toBe(2)
})

it('keeps the modal locked while the parent install promise is pending', async () => {
    let resolveInstall!: () => void
    const wrapper = await openModal(
        () =>
            new Promise<void>((resolve) => {
                resolveInstall = resolve
            }),
    )

    await wrapper.get('button[aria-label="Install datapack into World A"]').trigger('click')
    await flushPromises()
    expect(wrapper.get('dialog').attributes('data-disable-close')).toBe('true')
    resolveInstall()
    await flushPromises()
    expect(wrapper.find('dialog').exists()).toBe(false)
})

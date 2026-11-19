import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import { defineComponent, h, ref, Suspense } from 'vue'

import UpdateSettings from './UpdateSettings.vue'

const calls = vi.hoisted(() => ({ invoke: vi.fn(), handleError: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: calls.invoke }))
vi.mock('@tauri-apps/api/app', () => ({ getVersion: async () => '1.11.0' }))
vi.mock('@tauri-apps/plugin-http', () => ({ fetch: async () => ({ ok: false }) }))
vi.mock('@/helpers/utils.js', () => ({ isDev: async () => false, restartApp: vi.fn() }))
vi.mock('@/providers/app-update.ts', () => ({ checkForAppUpdate: vi.fn() }))
vi.mock('@/components/ui/announcement/UpdateAnnouncementHistory.vue', () => ({
    default: { render: () => null },
}))
vi.mock('@modrinth/ui', () => ({
    defineMessages: (messages: unknown) => messages,
    injectNotificationManager: () => ({ handleError: calls.handleError }),
    useVIntl: () => ({
        formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
    }),
    Card: defineComponent({
        setup:
            (_, { slots }) =>
            () =>
                h('div', slots.default?.()),
    }),
    Button: defineComponent({
        setup:
            (_, { slots }) =>
            () =>
                h('button', slots.default?.()),
    }),
    Toggle: defineComponent({
        props: ['modelValue'],
        setup: () => () => h('input', { type: 'checkbox' }),
    }),
    NewModal: defineComponent({
        props: ['header'],
        setup(props, { slots, expose }) {
            const visible = ref(false)
            expose({
                show: () => {
                    visible.value = true
                },
                hide: () => {
                    visible.value = false
                },
            })
            return () =>
                visible.value
                    ? h('div', { role: 'dialog' }, [
                          props.header,
                          slots.default?.(),
                          slots.actions?.(),
                      ])
                    : null
        },
    }),
}))

const wrappers: ReturnType<typeof mount>[] = []
afterEach(() => {
    wrappers.splice(0).forEach((wrapper) => wrapper.unmount())
    vi.resetAllMocks()
})
async function render() {
    calls.invoke.mockImplementation(async (command: string) => {
        if (command === 'get_update_channel') return 'release'
        if (command === 'get_update_preferences')
            return { immediateUpdateFetch: false, updatesPaused: false }
        if (command === 'set_update_channel') return
        throw new Error(`Unexpected native command: ${command}`)
    })
    const wrapper = mount(
        defineComponent({ render: () => h(Suspense, null, { default: () => h(UpdateSettings) }) }),
    )
    wrappers.push(wrapper)
    await flushPromises()
    return wrapper
}
it('changes update channels directly and offers restart without database choices', async () => {
    const wrapper = await render()
    await wrapper.get('[role="radio"][aria-checked="false"]').trigger('click')
    await flushPromises()
    expect(calls.invoke).toHaveBeenCalledWith('set_update_channel', { channel: 'beta' })
    expect(calls.invoke.mock.calls.map(([command]) => command)).toEqual([
        'get_update_channel',
        'get_update_preferences',
        'set_update_channel',
    ])
    expect(wrapper.get('[role="dialog"]').text()).toContain('Restart required')
    expect(wrapper.text()).toContain('Restart now')
    expect(wrapper.text()).not.toContain('Copy database')
    expect(wrapper.text()).not.toContain('Database isolation')
    expect(wrapper.text()).not.toContain('Start with empty database')
})
it('restores the selected channel when saving the change fails', async () => {
    const wrapper = await render()
    calls.invoke.mockRejectedValueOnce(new Error('cannot save channel'))
    await wrapper.get('[role="radio"][aria-checked="false"]').trigger('click')
    await flushPromises()
    expect(wrapper.findAll('[role="radio"]')[0].attributes('aria-checked')).toBe('true')
    expect(wrapper.find('[role="dialog"]').exists()).toBe(false)
    expect(calls.handleError).toHaveBeenCalledOnce()
})

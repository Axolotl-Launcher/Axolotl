import { flushPromises, mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { defineComponent, h } from 'vue'

import ConfirmActionModal from './ConfirmActionModal.vue'

const hide = vi.fn(),
    show = vi.fn()
vi.mock('./NewModal.vue', () => ({
    default: defineComponent({
        props: ['disableClose'],
        setup: (props, { expose, slots }) => {
            expose({
                hide: () => {
                    if (!props.disableClose) return hide()
                },
                show,
            })
            return () =>
                h('div', { 'data-locked': String(props.disableClose) }, [
                    slots.default?.(),
                    slots.actions?.(),
                ])
        },
    }),
}))
vi.mock('../base/buttons/Button.vue', () => ({
    default: defineComponent({
        setup:
            (_, { attrs, slots }) =>
            () =>
                h('button', attrs, slots.default?.()),
    }),
}))
vi.mock('../../composables/i18n', () => ({
    defineMessage: (value: unknown) => value,
    defineMessages: (value: unknown) => value,
    useVIntl: () => ({
        formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
    }),
}))

it('unlink confirmation stays open through the request, preserves errors and closes only after retry succeeds', async () => {
    hide.mockClear()
    let finish!: () => void
    const action = vi
        .fn()
        .mockRejectedValueOnce(new Error('database unavailable'))
        .mockImplementationOnce(
            () =>
                new Promise<void>((resolve) => {
                    finish = resolve
                }),
        )
    const wrapper = mount(ConfirmActionModal, {
        props: { header: 'Unlink', confirmLabel: 'Unlink', confirmAction: action },
        global: { directives: { tooltip: {} } },
    })
    try {
        await wrapper.findAll('button')[1].trigger('click')
        await flushPromises()
        expect(hide).not.toHaveBeenCalled()
        expect(wrapper.text()).toContain('The action could not be completed. Try again.')
        await wrapper.findAll('button')[1].trigger('click')
        expect(wrapper.find('[data-locked="true"]').exists()).toBe(true)
        expect(wrapper.findAll('button')[0].attributes('disabled')).toBeDefined()
        expect(hide).not.toHaveBeenCalled()
        finish()
        await flushPromises()
        expect(hide).toHaveBeenCalledOnce()
        expect(wrapper.find('[data-locked="false"]').exists()).toBe(true)
    } finally {
        wrapper.unmount()
    }
})

it('keeps the action locked until the close animation completes', async () => {
    let finish!: () => void
    hide.mockImplementationOnce(
        () =>
            new Promise<void>((resolve) => {
                finish = resolve
            }),
    )
    const action = vi.fn().mockResolvedValue(undefined)
    const wrapper = mount(ConfirmActionModal, {
        props: { header: 'Delete', confirmLabel: 'Delete', confirmAction: action },
        global: { directives: { tooltip: {} } },
    })
    try {
        await wrapper.findAll('button')[1].trigger('click')
        await flushPromises()
        expect(wrapper.find('[data-locked="false"]').exists()).toBe(true)
        expect(wrapper.findAll('button')[1].attributes('disabled')).toBeDefined()
        await wrapper.findAll('button')[1].trigger('click')
        wrapper.vm.show()
        expect(action).toHaveBeenCalledOnce()
        finish()
        await flushPromises()
        expect(wrapper.findAll('button')[1].attributes('disabled')).toBeUndefined()
    } finally {
        wrapper.unmount()
    }
})

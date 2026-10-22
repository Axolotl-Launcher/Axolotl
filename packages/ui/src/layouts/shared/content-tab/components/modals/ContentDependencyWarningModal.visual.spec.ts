import { flushPromises, mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { defineComponent, h } from 'vue'

import ContentDependencyWarningModal from './ContentDependencyWarningModal.vue'

const hide = vi.fn()
vi.mock('#ui/components/modal/NewModal.vue', () => ({
    default: defineComponent({
        props: ['disableClose'],
        setup: (props, { slots, expose }) => {
            expose({
                show: vi.fn(),
                hide: () => {
                    if (!props.disableClose) hide()
                },
            })
            return () =>
                h('section', { 'data-locked': String(props.disableClose) }, [
                    slots.default?.(),
                    slots.actions?.(),
                ])
        },
    }),
}))
vi.mock('#ui/components/base/buttons/Button.vue', () => ({
    default: defineComponent({
        setup:
            (_, { attrs, slots }) =>
            () =>
                h('button', attrs, slots.default?.()),
    }),
}))
vi.mock('#ui/composables/i18n', async (importOriginal) => ({
    ...(await importOriginal<object>()),
    defineMessages: (value: unknown) => value,
    defineMessage: (value: unknown) => value,
    useVIntl: () => ({
        formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
    }),
}))

it('dependency deletion unlocks and closes only after the accepted action settles, retaining failures for retry', async () => {
    hide.mockClear()
    let complete!: () => void
    const action = vi
        .fn()
        .mockRejectedValueOnce(new Error('delete failed'))
        .mockImplementationOnce(
            () =>
                new Promise<void>((resolve) => {
                    complete = resolve
                }),
        )
    const wrapper = mount(ContentDependencyWarningModal, {
        props: { itemType: 'mod', action },
        global: {
            stubs: {
                SymlinkWarningAdmonition: true,
                Admonition: true,
                Checkbox: true,
                ContentCardItem: true,
            },
            directives: { tooltip: {} },
        },
    })
    try {
        await wrapper.vm.show()
        await wrapper.findAll('button')[1].trigger('click')
        await flushPromises()
        expect(hide).not.toHaveBeenCalled()
        expect(wrapper.text()).toContain(
            'The selected content and dependencies could not be deleted. Try again.',
        )
        await wrapper.findAll('button')[1].trigger('click')
        expect(wrapper.attributes('data-locked')).toBe('true')
        complete()
        await flushPromises()
        expect(hide).toHaveBeenCalledOnce()
        expect(wrapper.attributes('data-locked')).toBe('false')
    } finally {
        wrapper.unmount()
    }
})

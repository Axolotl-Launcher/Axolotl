import { mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { nextTick } from 'vue'

import UnsavedSelectionModal from './UnsavedSelectionModal.vue'

vi.mock('@modrinth/assets', async () => {
    const { defineComponent, h } = await import('vue')
    return {
        XIcon: defineComponent({
            name: 'XIcon',
            setup: () => () => h('svg', { 'data-x-icon': '' }),
        }),
    }
})

vi.mock('@modrinth/ui', async () => {
    const { defineComponent, h } = await import('vue')

    const NewModal = defineComponent({
        name: 'NewModal',
        props: ['header', 'maxWidth', 'onHide'],
        setup(props, { slots, expose }) {
            expose({
                show: () => {},
                hide: () => props.onHide?.(),
            })
            return () =>
                h('div', { 'data-new-modal': '' }, [
                    h('span', { 'data-modal-header': '' }, props.header),
                    slots.default?.(),
                    slots.actions?.(),
                ])
        },
    })

    return {
        Button: defineComponent({
            name: 'Button',
            inheritAttrs: false,
            setup:
                (_, { slots, attrs }) =>
                () =>
                    h('button', attrs, slots.default?.()),
        }),
        defineMessages: (value: unknown) => value,
        NewModal,
        useVIntl: () => ({
            formatMessage: (
                message: { defaultMessage: string },
                values?: Record<string, string | number>,
            ) => {
                let text = message.defaultMessage
                for (const [key, value] of Object.entries(values ?? {})) {
                    text = text.replaceAll(`{${key}}`, String(value))
                }
                return text
            },
        }),
    }
})

function buttonByText(wrapper: ReturnType<typeof mount>, text: string) {
    const button = wrapper.findAll('button').find((candidate) => candidate.text().includes(text))
    if (!button) throw new Error(`No button containing "${text}"`)
    return button
}

it('renders the count of uninstalled selections', () => {
    const wrapper = mount(UnsavedSelectionModal, { props: { count: 3 } })
    expect(wrapper.text()).toContain('You have selected 3 item(s) that have not been installed.')
})

it('emits stay once when the stay button is clicked', async () => {
    const wrapper = mount(UnsavedSelectionModal, { props: { count: 1 } })
    await buttonByText(wrapper, 'Stay on this page').trigger('click')
    expect(wrapper.emitted('stay')).toHaveLength(1)
    expect(wrapper.emitted('discard')).toBeUndefined()
})

it('emits discard when the discard button is clicked', async () => {
    const wrapper = mount(UnsavedSelectionModal, { props: { count: 2 } })
    await buttonByText(wrapper, 'Discard and leave').trigger('click')
    expect(wrapper.emitted('discard')).toHaveLength(1)
    expect(wrapper.emitted('stay')).toBeUndefined()
})

it('treats hiding without an action as staying and never double-emits once settled', async () => {
    const wrapper = mount(UnsavedSelectionModal, { props: { count: 2 } })
    wrapper.vm.show()
    await nextTick()

    const modal = wrapper.findComponent({ name: 'NewModal' })
    const onHide = modal.props('onHide') as () => void

    onHide()
    expect(wrapper.emitted('stay')).toHaveLength(1)

    // The decision is settled; a repeated hide callback must not emit again.
    onHide()
    expect(wrapper.emitted('stay')).toHaveLength(1)
})

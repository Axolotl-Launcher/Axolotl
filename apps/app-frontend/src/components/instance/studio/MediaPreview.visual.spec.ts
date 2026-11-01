import { mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'

import MediaPreview from './MediaPreview.vue'

vi.mock('@modrinth/ui', async () => {
    const { defineComponent, h } = await import('vue')
    return {
        Button: defineComponent({
            name: 'Button',
            emits: ['click'],
            setup:
                (_, { slots, emit }) =>
                () =>
                    h('button', { onClick: () => emit('click') }, slots.default?.()),
        }),
        defineMessages: (messages: unknown) => messages,
        useVIntl: () => ({
            formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
        }),
    }
})

function mountPreview(props: Record<string, unknown>) {
    return mount(MediaPreview, {
        props: { url: 'blob:preview', name: 'block.png', loading: false, failed: false, ...props },
    })
}

it('renders an image from the object URL it is given', () => {
    const wrapper = mountPreview({ kind: 'image' })

    expect(wrapper.find('img').attributes('src')).toBe('blob:preview')
    expect(wrapper.find('img').attributes('alt')).toBe('block.png')
})

it('renders a video from the same object URL', () => {
    const wrapper = mountPreview({ kind: 'video', name: 'clip.mp4' })

    expect(wrapper.find('video').attributes('src')).toBe('blob:preview')
})

it('replaces an image the webview cannot decode with the fallback', async () => {
    const wrapper = mountPreview({ kind: 'image' })
    await wrapper.find('img').trigger('error')

    expect(wrapper.find('img').exists()).toBe(false)
    expect(wrapper.text()).toContain('This file could not be previewed.')
})

it('hands the file back to the system when the fallback is used', async () => {
    const wrapper = mountPreview({ kind: 'image', failed: true })

    await wrapper.find('button').trigger('click')
    expect(wrapper.emitted('openInSystem')).toHaveLength(1)
})

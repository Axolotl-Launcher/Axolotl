import { mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'

import MediaPreview from './MediaPreview.vue'

const PNG_BYTES = Uint8Array.from(
    atob(
        'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==',
    ),
    (character) => character.charCodeAt(0),
)
// A real object URL: an image the webview cannot decode would replace the
// preview with the fallback before the controls could be exercised.
const PNG_URL = URL.createObjectURL(new Blob([PNG_BYTES], { type: 'image/png' }))

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

function mountPreview({
    kind,
    ...overrides
}: Record<string, unknown> & { kind: 'image' | 'video' }) {
    return mount(MediaPreview, {
        props: {
            url: PNG_URL,
            name: 'block.png',
            bytes: 2048,
            loading: false,
            failed: false,
            ...overrides,
            kind,
        },
        global: { directives: { tooltip: {} } },
    })
}

it('renders an image from the object URL it is given', () => {
    const wrapper = mountPreview({ kind: 'image' })

    expect(wrapper.find('img').attributes('src')).toBe(PNG_URL)
    expect(wrapper.find('img').attributes('alt')).toBe('block.png')
})

it('renders a video from the same object URL', () => {
    const wrapper = mountPreview({ kind: 'video', name: 'clip.mp4' })

    expect(wrapper.find('video').attributes('src')).toBe(PNG_URL)
    expect(wrapper.find('button').exists()).toBe(false)
})

it('reports the image dimensions and file size once the image decodes', async () => {
    const wrapper = mountPreview({ kind: 'image' })

    await vi.waitFor(() => expect(wrapper.text()).toContain('1 × 1 · 2.0 KB'))
})

it('zooms in and out from the controls', async () => {
    const wrapper = mountPreview({ kind: 'image' })
    expect(wrapper.text()).toContain('100%')

    await wrapper.find('[aria-label="Zoom in"]').trigger('click')
    expect(wrapper.text()).toContain('125%')

    await wrapper.find('[aria-label="Zoom out"]').trigger('click')
    expect(wrapper.text()).toContain('100%')
})

it('returns to the fitted scale from actual size', async () => {
    const wrapper = mountPreview({ kind: 'image' })
    await wrapper.find('[aria-label="Zoom in"]').trigger('click')
    await wrapper.find('[aria-label="Zoom in"]').trigger('click')
    expect(wrapper.text()).toContain('156%')

    const actualSize = wrapper.findAll('button').find((button) => button.text() === 'Actual size')
    await actualSize!.trigger('click')
    expect(wrapper.text()).toContain('100%')
})

it('replaces an image the webview cannot decode with the fallback', async () => {
    const wrapper = mountPreview({ kind: 'image', url: 'blob:not-an-image' })
    await wrapper.find('img').trigger('error')

    expect(wrapper.find('img').exists()).toBe(false)
    expect(wrapper.text()).toContain('This file could not be previewed.')
})

it('hands the file back to the system when the fallback is used', async () => {
    const wrapper = mountPreview({ kind: 'image', failed: true })

    const button = wrapper
        .findAll('button')
        .find((candidate) => candidate.text() === 'Open in system')
    await button!.trigger('click')
    expect(wrapper.emitted('openInSystem')).toHaveLength(1)
})

import '@/test/visual-styles.scss'

import { mount } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import { nextTick } from 'vue'

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

const hosts: HTMLElement[] = []
const mounted: ReturnType<typeof mount>[] = []

// A detached node has no layout, and the crop gesture is measured against the
// rendered image, so every mount gets a real box in the document.
afterEach(() => {
    mounted.splice(0).forEach((wrapper) => wrapper.unmount())
    hosts.splice(0).forEach((host) => host.remove())
})

function mountPreview({
    kind,
    ...overrides
}: Record<string, unknown> & { kind: 'image' | 'video' }) {
    const host = document.createElement('div')
    host.style.width = '320px'
    host.style.height = '240px'
    // The component fills its parent, and the test wrapper has to stretch for
    // the stage to get a height the fit scale can measure.
    host.style.display = 'flex'
    document.body.appendChild(host)
    hosts.push(host)

    const wrapper = mount(MediaPreview, {
        props: {
            url: PNG_URL,
            name: 'block.png',
            bytes: 2048,
            loading: false,
            failed: false,
            ...overrides,
            kind,
        },
        attachTo: host,
        global: { directives: { tooltip: {} } },
    })
    mounted.push(wrapper)

    return wrapper
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

it('zooms with the wheel while the zoom key is held', async () => {
    const wrapper = mountPreview({ kind: 'image' })
    const stage = wrapper.find('.cursor-grab')

    stage.element.dispatchEvent(
        new WheelEvent('wheel', { deltaY: -100, ctrlKey: true, bubbles: true, cancelable: true }),
    )
    await nextTick()
    expect(wrapper.text()).toContain('110%')

    // macOS uses the command key, and a trackpad pinch arrives as ctrl+wheel.
    stage.element.dispatchEvent(
        new WheelEvent('wheel', { deltaY: 100, metaKey: true, bubbles: true, cancelable: true }),
    )
    await nextTick()
    expect(wrapper.text()).toContain('100%')
})

it('leaves a plain wheel scroll to the stage', async () => {
    const wrapper = mountPreview({ kind: 'image' })
    const stage = wrapper.find('.cursor-grab')

    stage.element.dispatchEvent(
        new WheelEvent('wheel', { deltaY: -100, bubbles: true, cancelable: true }),
    )
    await nextTick()
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

function solidImage(width: number, height: number): string {
    const canvas = document.createElement('canvas')
    canvas.width = width
    canvas.height = height
    const context = canvas.getContext('2d')
    if (!context) throw new Error('the test needs a 2D context')
    context.fillStyle = '#ff0000'
    context.fillRect(0, 0, width, height)
    return canvas.toDataURL('image/png')
}

function dispatchPointer(target: EventTarget, type: string, x: number, y: number) {
    target.dispatchEvent(
        new PointerEvent(type, {
            clientX: x,
            clientY: y,
            button: 0,
            bubbles: true,
            cancelable: true,
        }),
    )
}

it('crops the dragged region and emits the encoded bytes', async () => {
    const wrapper = mountPreview({ kind: 'image', url: solidImage(4, 4) })
    await vi.waitFor(() => expect(wrapper.text()).toContain('4 × 4'))

    const crop = wrapper.findAll('button').find((button) => button.text() === 'Crop')
    await crop!.trigger('click')

    const overlay = wrapper.find('.cursor-crosshair')
    const bounds = (overlay.element as HTMLElement).getBoundingClientRect()
    // Half of the displayed image, whatever the fitted scale happens to be.
    const half = { x: bounds.width / 2, y: bounds.height / 2 }
    dispatchPointer(overlay.element, 'pointerdown', bounds.left, bounds.top)
    dispatchPointer(window, 'pointermove', bounds.left + half.x, bounds.top + half.y)
    dispatchPointer(window, 'pointerup', bounds.left + half.x, bounds.top + half.y)
    // Raw events bypass Vue Test Utils' flush, so let the selection render.
    await nextTick()

    const apply = wrapper.findAll('button').find((button) => button.text() === 'Crop image')
    expect(apply!.attributes('disabled')).toBeUndefined()
    await apply!.trigger('click')
    await vi.waitFor(() => expect(wrapper.emitted('crop')).toHaveLength(1))

    const bytes = wrapper.emitted('crop')![0][0] as Uint8Array
    const bitmap = await createImageBitmap(new Blob([bytes as BlobPart], { type: 'image/png' }))
    expect({ width: bitmap.width, height: bitmap.height }).toEqual({ width: 2, height: 2 })
})

it('keeps cropping unavailable for a format it cannot write back', async () => {
    const wrapper = mountPreview({ kind: 'image', name: 'icon.svg', url: solidImage(4, 4) })
    await vi.waitFor(() => expect(wrapper.text()).toContain('4 × 4'))

    expect(wrapper.findAll('button').some((button) => button.text() === 'Crop')).toBe(false)
})

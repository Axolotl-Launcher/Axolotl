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
const EDITED_BYTES = Uint8Array.from([9, 9, 9])

vi.mock('@modrinth/ui', async () => {
    const { defineComponent, h } = await import('vue')
    return {
        // The real button drags in a tooltip frame that needs a mounted app; a
        // plain button keeps the assertions on the emitted events readable.
        Button: defineComponent({
            name: 'Button',
            props: { disabled: { type: Boolean, default: false } },
            emits: ['click'],
            setup:
                (props, { slots, emit }) =>
                () =>
                    h(
                        'button',
                        { disabled: props.disabled, onClick: () => emit('click') },
                        slots.default?.(),
                    ),
        }),
        defineMessages: (messages: unknown) => messages,
        useVIntl: () => ({
            formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
        }),
    }
})

// The editor runs a fabric canvas, which needs a layout this page spec does not
// provide; what matters here is that the page hands it the bytes and writes the
// result back.
vi.mock('./StudioImageEditor.vue', async () => {
    const { defineComponent, h } = await import('vue')
    return {
        default: defineComponent({
            name: 'StudioImageEditor',
            props: { content: Object, mimeType: String, name: String },
            emits: ['save', 'cancel', 'error'],
            setup:
                (_, { emit }) =>
                () =>
                    h('div', { 'data-testid': 'studio-image-editor' }, [
                        h('button', {
                            'data-testid': 'editor-save',
                            onClick: () => emit('save', EDITED_BYTES),
                        }),
                        h('button', {
                            'data-testid': 'editor-cancel',
                            onClick: () => emit('cancel'),
                        }),
                    ]),
        }),
    }
})

const hosts: HTMLElement[] = []
const mounted: ReturnType<typeof mount>[] = []

// A detached node has no layout, and the fitted scale is measured against the
// rendered stage, so every mount gets a real box in the document.
afterEach(() => {
    mounted.splice(0).forEach((wrapper) => wrapper.unmount())
    hosts.splice(0).forEach((host) => host.remove())
})

function mountPreview(overrides: Record<string, unknown> = {}) {
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
            kind: 'image',
            url: PNG_URL,
            name: 'block.png',
            size: 2048,
            content: PNG_BYTES,
            loading: false,
            failed: false,
            ...overrides,
        },
        attachTo: host,
        global: { directives: { tooltip: {} } },
    })
    mounted.push(wrapper)

    return wrapper
}

function findButton(wrapper: ReturnType<typeof mountPreview>, label: string) {
    return wrapper.findAll('button').find((button) => button.text() === label)
}

it('renders an image from the object URL it is given', () => {
    const wrapper = mountPreview()

    expect(wrapper.find('img').attributes('src')).toBe(PNG_URL)
    expect(wrapper.find('img').attributes('alt')).toBe('block.png')
})

it('renders a video from the same object URL', () => {
    const wrapper = mountPreview({ kind: 'video', name: 'clip.mp4', content: null })

    expect(wrapper.find('video').attributes('src')).toBe(PNG_URL)
    expect(wrapper.find('button').exists()).toBe(false)
})

it('reports the image dimensions and file size once the image decodes', async () => {
    const wrapper = mountPreview()

    await vi.waitFor(() => expect(wrapper.text()).toContain('1 × 1 · 2.0 KB'))
})

it('zooms in and out from the controls', async () => {
    const wrapper = mountPreview()
    expect(wrapper.text()).toContain('100%')

    await wrapper.find('[aria-label="Zoom in"]').trigger('click')
    expect(wrapper.text()).toContain('125%')

    await wrapper.find('[aria-label="Zoom out"]').trigger('click')
    expect(wrapper.text()).toContain('100%')
})

it('returns to the fitted scale from actual size', async () => {
    const wrapper = mountPreview()
    await wrapper.find('[aria-label="Zoom in"]').trigger('click')
    await wrapper.find('[aria-label="Zoom in"]').trigger('click')
    expect(wrapper.text()).toContain('156%')

    await findButton(wrapper, 'Actual size')!.trigger('click')
    expect(wrapper.text()).toContain('100%')
})

it('zooms with the wheel while the zoom key is held', async () => {
    const wrapper = mountPreview()
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
    const wrapper = mountPreview()
    const stage = wrapper.find('.cursor-grab')

    stage.element.dispatchEvent(
        new WheelEvent('wheel', { deltaY: -100, bubbles: true, cancelable: true }),
    )
    await nextTick()
    expect(wrapper.text()).toContain('100%')
})

it('replaces an image the webview cannot decode with the fallback', async () => {
    const wrapper = mountPreview({ url: 'blob:not-an-image' })
    await wrapper.find('img').trigger('error')

    expect(wrapper.find('img').exists()).toBe(false)
    expect(wrapper.text()).toContain('This file could not be previewed.')
})

it('hands the file back to the system when the fallback is used', async () => {
    const wrapper = mountPreview({ failed: true })

    await findButton(wrapper, 'Open in system')!.trigger('click')
    expect(wrapper.emitted('openInSystem')).toHaveLength(1)
})

it('opens the editor in place of the preview', async () => {
    const wrapper = mountPreview()
    await findButton(wrapper, 'Edit')!.trigger('click')

    expect(wrapper.find('[data-testid="studio-image-editor"]').exists()).toBe(true)
    expect(wrapper.find('img').exists()).toBe(false)
    // The editor brings its own zoom and save controls; two toolbars would
    // disagree about what the percentage refers to.
    expect(wrapper.find('[aria-label="Zoom in"]').exists()).toBe(false)
})

it('writes the edited bytes back and returns to the preview', async () => {
    const wrapper = mountPreview()
    await findButton(wrapper, 'Edit')!.trigger('click')
    await wrapper.find('[data-testid="editor-save"]').trigger('click')

    expect(wrapper.emitted('save')).toEqual([[EDITED_BYTES]])
    expect(wrapper.find('[data-testid="studio-image-editor"]').exists()).toBe(false)
    expect(wrapper.find('img').exists()).toBe(true)
})

it('discards the edit when it is cancelled', async () => {
    const wrapper = mountPreview()
    await findButton(wrapper, 'Edit')!.trigger('click')
    await wrapper.find('[data-testid="editor-cancel"]').trigger('click')

    expect(wrapper.emitted('save')).toBeUndefined()
    expect(wrapper.find('img').exists()).toBe(true)
})

it('keeps editing unavailable for a format it cannot write back', async () => {
    const wrapper = mountPreview({ name: 'icon.svg' })

    expect(wrapper.find('img').exists()).toBe(true)
    expect(findButton(wrapper, 'Edit')).toBeUndefined()
})

it('keeps editing unavailable while the bytes are missing', () => {
    const wrapper = mountPreview({ content: null })

    expect(findButton(wrapper, 'Edit')).toBeUndefined()
})

function largeImageUrl(width: number, height: number): string {
    const canvas = document.createElement('canvas')
    canvas.width = width
    canvas.height = height
    const context = canvas.getContext('2d')
    if (!context) throw new Error('the test needs a 2D context')
    context.fillRect(0, 0, width, height)
    return canvas.toDataURL('image/png')
}

it('fits an image inside the stage padding instead of overflowing it', async () => {
    const wrapper = mountPreview({ url: largeImageUrl(400, 400), name: 'large.png', content: null })
    const stage = wrapper.find('.overflow-auto')
    const element = stage.element as HTMLElement
    const style = getComputedStyle(element)
    // The padding is not part of the room the image may use; measuring the whole
    // client box would ask for a fit larger than the content area and scrollbars
    // would appear on an image that is supposed to be fitted.
    const available = {
        width:
            element.clientWidth -
            Number.parseFloat(style.paddingLeft) -
            Number.parseFloat(style.paddingRight),
        height:
            element.clientHeight -
            Number.parseFloat(style.paddingTop) -
            Number.parseFloat(style.paddingBottom),
    }
    const expected = Math.round(Math.min(available.width / 400, available.height / 400) * 100)

    await vi.waitFor(() => expect(wrapper.text()).toContain(`${expected}%`))
    expect(element.scrollWidth).toBeLessThanOrEqual(element.clientWidth)
    expect(element.scrollHeight).toBeLessThanOrEqual(element.clientHeight)
})

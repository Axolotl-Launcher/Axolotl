import { mount, type VueWrapper } from '@vue/test-utils'
import { afterAll, afterEach, beforeAll, expect, it, vi } from 'vitest'
import { defineComponent, h, nextTick, ref } from 'vue'

import type { ImageCropModalExposed } from './image-crop'
import ImageCropModal from './ImageCropModal.vue'

vi.mock('./NewModal.vue', () => ({
    default: defineComponent({
        props: {
            header: String,
            disableClose: Boolean,
            onAfterHide: Function,
        },
        setup(props, { expose, slots }) {
            const open = ref(false)
            expose({
                show() {
                    open.value = true
                },
                async hide() {
                    if (!open.value) return
                    open.value = false
                    props.onAfterHide?.()
                },
            })
            return () =>
                h('div', { 'data-modal-open': String(open.value) }, [
                    slots.default?.(),
                    slots.actions?.(),
                ])
        },
    }),
}))

vi.mock('../base/buttons/Button.vue', () => ({
    default: defineComponent({
        inheritAttrs: false,
        setup:
            (_, { attrs, slots }) =>
            () =>
                h('button', { ...attrs, type: 'button' }, slots.default?.()),
    }),
}))

vi.mock('../../composables/i18n', () => ({
    defineMessage: (value: unknown) => value,
    defineMessages: (value: unknown) => value,
    useVIntl: () => ({
        formatMessage: (
            message: { defaultMessage: string },
            values?: Record<string, string | number>,
        ) =>
            message.defaultMessage.replace(/\{(\w+)\}/g, (_, key: string) =>
                values && key in values ? String(values[key]) : `{${key}}`,
            ),
    }),
}))

let wrapper: VueWrapper | null = null

beforeAll(() => {
    vi.stubGlobal(
        'ResizeObserver',
        class {
            observe() {}
            unobserve() {}
            disconnect() {}
        },
    )
})

afterEach(() => {
    wrapper?.unmount()
    wrapper = null
    vi.restoreAllMocks()
})

afterAll(() => {
    vi.unstubAllGlobals()
})

function mountModal(props: Record<string, unknown> = {}) {
    wrapper = mount(ImageCropModal, {
        attachTo: document.body,
        props,
    })
    return wrapper
}

function exposed(): ImageCropModalExposed {
    return wrapper!.vm as unknown as ImageCropModalExposed
}

async function createTestImage(width = 400, height = 300): Promise<Blob> {
    const canvas = document.createElement('canvas')
    canvas.width = width
    canvas.height = height
    const context = canvas.getContext('2d')
    if (!context) throw new Error('No canvas context')

    context.fillStyle = '#ff0000'
    context.fillRect(0, 0, width / 2, height)
    context.fillStyle = '#0000ff'
    context.fillRect(width / 2, 0, width / 2, height)

    return new Promise((resolve, reject) =>
        canvas.toBlob(
            (blob) => (blob ? resolve(blob) : reject(new Error('Could not encode image'))),
            'image/png',
        ),
    )
}

async function createStripedImage(width = 400, height = 300): Promise<Blob> {
    const canvas = document.createElement('canvas')
    canvas.width = width
    canvas.height = height
    const context = canvas.getContext('2d')
    if (!context) throw new Error('No canvas context')

    for (let x = 0; x < width; x += 15) {
        context.fillStyle = (x / 15) % 2 === 0 ? '#ff0000' : '#0000ff'
        context.fillRect(x, 0, 15, height)
    }

    return new Promise((resolve, reject) =>
        canvas.toBlob(
            (blob) => (blob ? resolve(blob) : reject(new Error('Could not encode image'))),
            'image/png',
        ),
    )
}

async function waitForFrame() {
    await vi.waitFor(() => {
        expect(wrapper!.find('[data-image-crop-frame]').exists()).toBe(true)
        const frame = wrapper!.get('[data-image-crop-frame]').element as HTMLElement
        expect(frame.getBoundingClientRect().width).toBeGreaterThan(100)
    })
}

async function samplePixel(
    blob: Blob,
    x: number,
    y: number,
): Promise<[number, number, number, number]> {
    const bitmap = await createImageBitmap(blob)
    try {
        const canvas = document.createElement('canvas')
        canvas.width = bitmap.width
        canvas.height = bitmap.height
        const context = canvas.getContext('2d')
        if (!context) throw new Error('No canvas context')
        context.drawImage(bitmap, 0, 0)
        return Array.from(context.getImageData(x, y, 1, 1).data) as [number, number, number, number]
    } finally {
        bitmap.close()
    }
}

it('shows the workspace, result preview, and zoom controls', async () => {
    mountModal({ maxOutputDimension: 200 })
    const promise = exposed().show(await createTestImage(), {
        aspectRatioOptions: [{ id: 'square', label: '1:1', ratio: 1 }],
        defaultAspectRatioId: 'square',
    })

    await waitForFrame()
    try {
        expect(wrapper!.get('[data-image-crop-workspace]').exists()).toBe(true)
        expect(wrapper!.get('[data-image-crop-preview]').exists()).toBe(true)
        expect(wrapper!.get('[data-image-crop-zoom]').text()).toContain('100%')
        expect(wrapper!.get('[data-image-crop-confirm]').attributes('disabled')).toBeUndefined()

        await wrapper!.get('[data-image-crop-cancel]').trigger('click')
        expect(await promise).toBeNull()
    } finally {
        // The assertions above close the modal through the cancel path.
    }
})

it('exports the visible source rectangle with the requested aspect ratio', async () => {
    mountModal({ maxOutputDimension: 200 })
    const promise = exposed().show(await createTestImage(), {
        aspectRatioOptions: [{ id: 'square', label: '1:1', ratio: 1 }],
        defaultAspectRatioId: 'square',
    })

    await waitForFrame()
    await wrapper!.get('[data-image-crop-confirm]').trigger('click')
    const result = await promise

    expect(result).not.toBeNull()
    expect(result!.width).toBe(200)
    expect(result!.height).toBe(200)
    expect(result!.extension).toBe('png')

    const left = await samplePixel(result!.blob, 10, 10)
    const right = await samplePixel(result!.blob, 190, 10)
    expect(left[0]).toBeGreaterThan(left[2])
    expect(right[2]).toBeGreaterThan(right[0])
})

it('resolves null when the user cancels', async () => {
    mountModal()
    const promise = exposed().show(await createTestImage())

    await waitForFrame()
    await wrapper!.get('[data-image-crop-cancel]').trigger('click')

    expect(await promise).toBeNull()
})

it('changes the composition when the image is dragged', async () => {
    mountModal({ maxOutputDimension: 200 })
    const blob = await createStripedImage()
    const options = {
        aspectRatioOptions: [{ id: 'square', label: '1:1', ratio: 1 }],
        defaultAspectRatioId: 'square',
    }

    const firstPromise = exposed().show(blob, options)
    await waitForFrame()
    await wrapper!.get('[data-image-crop-confirm]').trigger('click')
    const first = await firstPromise
    const firstBytes = new Uint8Array(await first!.blob.arrayBuffer())

    const secondPromise = exposed().show(blob, options)
    await waitForFrame()
    const workspace = wrapper!.get('[data-image-crop-workspace]')
    workspace.element.dispatchEvent(
        new PointerEvent('pointerdown', {
            pointerId: 1,
            clientX: 300,
            clientY: 200,
            bubbles: true,
        }),
    )
    workspace.element.dispatchEvent(
        new PointerEvent('pointermove', {
            pointerId: 1,
            clientX: 360,
            clientY: 200,
            bubbles: true,
        }),
    )
    workspace.element.dispatchEvent(
        new PointerEvent('pointerup', {
            pointerId: 1,
            clientX: 360,
            clientY: 200,
            bubbles: true,
        }),
    )
    await nextTick()
    await wrapper!.get('[data-image-crop-confirm]').trigger('click')
    const second = await secondPromise
    const secondBytes = new Uint8Array(await second!.blob.arrayBuffer())

    expect(secondBytes).not.toEqual(firstBytes)
})

it('zooms with the keyboard and resets with zero', async () => {
    mountModal()
    const promise = exposed().show(await createTestImage())

    await waitForFrame()
    const workspace = wrapper!.get('[data-image-crop-workspace]')
    workspace.element.focus()
    await workspace.trigger('keydown', { key: '+' })
    expect(wrapper!.get('[data-image-crop-zoom]').text()).toContain('115%')

    await workspace.trigger('keydown', { key: '0' })
    expect(wrapper!.get('[data-image-crop-zoom]').text()).toContain('100%')

    await wrapper!.get('[data-image-crop-cancel]').trigger('click')
    await promise
})

it('switches aspect ratio presets', async () => {
    mountModal({ maxOutputDimension: 200 })
    const promise = exposed().show(await createTestImage(), {
        aspectRatioOptions: [
            { id: 'wide', label: '16:9', ratio: 16 / 9 },
            { id: 'square', label: '1:1', ratio: 1 },
        ],
        defaultAspectRatioId: 'wide',
    })

    await waitForFrame()
    await wrapper!.get('[data-aspect-id="square"]').trigger('click')
    await vi.waitFor(() => {
        const frame = wrapper!.get('[data-image-crop-frame]').element as HTMLElement
        const bounds = frame.getBoundingClientRect()
        expect(Math.abs(bounds.width / bounds.height - 1)).toBeLessThan(0.02)
    })

    await wrapper!.get('[data-image-crop-confirm]').trigger('click')
    const result = await promise
    expect(result!.width).toBe(200)
    expect(result!.height).toBe(200)
})

it('caps the output long edge without upscaling', async () => {
    mountModal({ maxOutputDimension: 120 })
    const promise = exposed().show(await createTestImage(400, 300), {
        aspectRatioOptions: [{ id: 'square', label: '1:1', ratio: 1 }],
        defaultAspectRatioId: 'square',
    })

    await waitForFrame()
    await wrapper!.get('[data-image-crop-confirm]').trigger('click')
    const result = await promise
    expect(Math.max(result!.width, result!.height)).toBe(120)
})

it('keeps the PNG extension when the webview falls back from WebP', async () => {
    const blob = await createTestImage()
    const originalToBlob = HTMLCanvasElement.prototype.toBlob
    vi.spyOn(HTMLCanvasElement.prototype, 'toBlob').mockImplementation(function (
        this: HTMLCanvasElement,
        callback: BlobCallback,
        _type?: string,
        _quality?: number,
    ) {
        originalToBlob.call(this, callback, 'image/png')
    })

    mountModal({ outputMimeType: 'image/webp' })
    const promise = exposed().show(blob, {
        aspectRatioOptions: [{ id: 'square', label: '1:1', ratio: 1 }],
        defaultAspectRatioId: 'square',
    })

    await waitForFrame()
    await wrapper!.get('[data-image-crop-confirm]').trigger('click')
    const result = await promise
    expect(result!.mimeType).toBe('image/png')
    expect(result!.extension).toBe('png')
})

it('shows an alert and disables confirmation when the image cannot be decoded', async () => {
    mountModal()
    const promise = exposed().show(new Blob([new Uint8Array([1, 2, 3])], { type: 'image/png' }))

    await vi.waitFor(() => {
        expect(wrapper!.find('[role="alert"]').exists()).toBe(true)
    })
    expect(wrapper!.get('[data-image-crop-confirm]').attributes('disabled')).toBeDefined()

    await wrapper!.get('[data-image-crop-cancel]').trigger('click')
    expect(await promise).toBeNull()
})

it('exports WebP when the webview supports WebP encoding', async () => {
    mountModal({ outputMimeType: 'image/webp', maxOutputDimension: 200 })
    const promise = exposed().show(await createTestImage(), {
        aspectRatioOptions: [{ id: 'square', label: '1:1', ratio: 1 }],
        defaultAspectRatioId: 'square',
    })

    await waitForFrame()
    await wrapper!.get('[data-image-crop-confirm]').trigger('click')
    const result = await promise

    expect(result!.mimeType).toBe('image/webp')
    expect(result!.extension).toBe('webp')
})

it('rejects a source file above the byte limit', async () => {
    mountModal({ maxSourceBytes: 10 })
    const promise = exposed().show(await createTestImage())

    await vi.waitFor(() => {
        expect(wrapper!.find('[data-image-crop-oversized]').exists()).toBe(true)
    })
    expect(wrapper!.find('[data-image-crop-auto-compress]').exists()).toBe(true)
    expect(wrapper!.find('[data-image-crop-keep-original]').exists()).toBe(true)

    await wrapper!.get('[data-image-crop-auto-compress]').trigger('click')
    await waitForFrame()
    await wrapper!.get('[data-image-crop-confirm]').trigger('click')
    expect(await promise).not.toBeNull()
})

it('keeps the original resolution when the user chooses that option', async () => {
    mountModal({ maxSourceBytes: 10 })
    const promise = exposed().show(await createTestImage())

    await vi.waitFor(() => {
        expect(wrapper!.find('[data-image-crop-oversized]').exists()).toBe(true)
    })
    await wrapper!.get('[data-image-crop-keep-original]').trigger('click')
    await waitForFrame()
    await wrapper!.get('[data-image-crop-confirm]').trigger('click')
    const result = await promise

    expect(result!.preserveOriginalQuality).toBe(true)
    expect(result!.width).toBe(400)
    expect(result!.height).toBe(225)
    expect(result!.blob.size).toBeGreaterThan(0)
})

it('keeps the full crop resolution for an oversized animated source', async () => {
    mountModal({ maxSourceBytes: 10, maxOutputDimension: 200 })
    const promise = exposed().showCrop(await createTestImage(), {
        aspectRatioOptions: [{ id: 'square', label: '1:1', ratio: 1 }],
        defaultAspectRatioId: 'square',
        maxOutputDimension: 200,
        animated: true,
    })

    await vi.waitFor(() => {
        expect(wrapper!.find('[data-image-crop-oversized]').exists()).toBe(true)
    })
    await wrapper!.get('[data-image-crop-keep-original]').trigger('click')
    await waitForFrame()
    await wrapper!.get('[data-image-crop-confirm]').trigger('click')
    const selection = await promise

    expect(selection!.preserveOriginalQuality).toBe(true)
    expect(selection!.outputWidth).toBe(300)
    expect(selection!.outputHeight).toBe(300)
})

it('rejects a source above the decoded pixel limit', async () => {
    mountModal({ maxSourcePixels: 100 })
    const promise = exposed().show(await createTestImage(400, 300))

    await vi.waitFor(() => {
        expect(wrapper!.find('[data-image-crop-oversized]').exists()).toBe(true)
    })

    await wrapper!.get('[data-image-crop-auto-compress]').trigger('click')
    await waitForFrame()
    await wrapper!.get('[data-image-crop-confirm]').trigger('click')
    expect(await promise).not.toBeNull()
})

it('locks closing while the crop is being encoded', async () => {
    const blob = await createTestImage()
    let release: (() => void) | null = null
    const originalToBlob = HTMLCanvasElement.prototype.toBlob
    vi.spyOn(HTMLCanvasElement.prototype, 'toBlob').mockImplementation(function (
        this: HTMLCanvasElement,
        callback: BlobCallback,
    ) {
        release = () => callback(new Blob(['encoded'], { type: 'image/png' }))
    })

    mountModal()
    const promise = exposed().show(blob, {
        aspectRatioOptions: [{ id: 'square', label: '1:1', ratio: 1 }],
        defaultAspectRatioId: 'square',
    })

    await waitForFrame()
    await wrapper!.get('[data-image-crop-confirm]').trigger('click')
    expect(wrapper!.get('[data-image-crop-cancel]').attributes('disabled')).toBeDefined()

    release?.()
    const result = await promise
    expect(result).not.toBeNull()
    expect(originalToBlob).toBeDefined()
})

it('returns normalized crop metadata for animated backgrounds', async () => {
    mountModal({ maxOutputDimension: 200 })
    const promise = exposed().showCrop(await createTestImage(), {
        aspectRatioOptions: [{ id: 'square', label: '1:1', ratio: 1 }],
        defaultAspectRatioId: 'square',
        maxOutputDimension: 200,
        animated: true,
    })

    await waitForFrame()
    expect(wrapper!.find('.image-crop-animated-preview').exists()).toBe(true)

    await wrapper!.get('[data-image-crop-confirm]').trigger('click')
    const selection = await promise

    expect(selection).not.toBeNull()
    expect(selection!.sourceWidth).toBe(400)
    expect(selection!.sourceHeight).toBe(300)
    expect(selection!.outputWidth).toBe(200)
    expect(selection!.outputHeight).toBe(200)
    expect(selection!.normalizedCrop.left).toBeCloseTo(0.125, 5)
    expect(selection!.normalizedCrop.width).toBeCloseTo(0.75, 5)
    expect(selection!.normalizedCrop.height).toBeCloseTo(1, 5)
    expect('blob' in selection!).toBe(false)
})

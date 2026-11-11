import { flushPromises, mount, type VueWrapper } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import { nextTick, reactive } from 'vue'

import {
    CUSTOM_BACKGROUND_EXTENSIONS,
    MAX_CUSTOM_BACKGROUND_BYTES,
    MAX_PRESERVED_BACKGROUND_BYTES,
} from '@/helpers/custom-background'

import LauncherBackgroundSettings from './LauncherBackgroundSettings.vue'

const mocks = vi.hoisted(() => ({
    open: vi.fn(),
    readFile: vi.fn(),
    writeFile: vi.fn(),
    mkdir: vi.fn(),
    exists: vi.fn(),
    remove: vi.fn(),
    invoke: vi.fn(),
    addNotification: vi.fn(),
    handleError: vi.fn(),
    cropShow: vi.fn(),
    cropShowSelection: vi.fn(),
    convertAnimatedBackground: vi.fn(),
    dragHandler: null as ((event: { payload: unknown }) => void) | null,
}))

vi.mock('@modrinth/ui', async () => {
    const { defineComponent, h } = await import('vue')
    return {
        Button: defineComponent({
            inheritAttrs: false,
            setup:
                (_, { attrs, slots }) =>
                () =>
                    h('button', { ...attrs, type: 'button' }, slots.default?.()),
        }),
        Slider: defineComponent({
            props: ['modelValue'],
            emits: ['update:modelValue'],
            setup: () => () => h('input', { type: 'range' }),
        }),
        ImageCropModal: defineComponent({
            setup(_, { expose }) {
                expose({
                    show: mocks.cropShow,
                    showCrop: mocks.cropShowSelection,
                })
                return () => h('div', { 'data-image-crop-modal': '' })
            },
        }),
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
        injectNotificationManager: () => ({
            addNotification: mocks.addNotification,
            handleError: mocks.handleError,
        }),
    }
})

vi.mock('@tauri-apps/api/core', () => ({
    convertFileSrc: (path: string) => path,
    invoke: mocks.invoke,
}))

vi.mock('@tauri-apps/api/path', () => ({
    appDataDir: async () => 'C:/data',
    join: async (...parts: string[]) => parts.join('/'),
}))

vi.mock('@tauri-apps/api/webview', () => ({
    getCurrentWebview: () => ({
        onDragDropEvent: async (handler: (event: { payload: unknown }) => void) => {
            mocks.dragHandler = handler
            return () => {}
        },
    }),
}))

vi.mock('@tauri-apps/plugin-dialog', () => ({ open: mocks.open }))

vi.mock('@tauri-apps/plugin-fs', () => ({
    exists: mocks.exists,
    mkdir: mocks.mkdir,
    readFile: mocks.readFile,
    remove: mocks.remove,
    writeFile: mocks.writeFile,
}))

vi.mock('@/helpers/background-conversion', () => ({
    convertAnimatedBackground: mocks.convertAnimatedBackground,
    isBackgroundConversionError: (error: unknown) =>
        typeof error === 'object' &&
        error !== null &&
        typeof (error as { code?: unknown }).code === 'string' &&
        typeof (error as { message?: unknown }).message === 'string',
}))

vi.mock('@/components/ui/headless/HeadlessTooltip.vue', async () => {
    const { defineComponent, h } = await import('vue')
    return {
        default: defineComponent({
            setup:
                (_, { slots }) =>
                () =>
                    h('div', slots.default?.()),
        }),
    }
})

let wrapper: VueWrapper | null = null

afterEach(() => {
    wrapper?.unmount()
    wrapper = null
    document
        .querySelectorAll('[data-testid="custom-background-overlay"]')
        .forEach((element) => element.remove())
    vi.resetAllMocks()
    mocks.dragHandler = null
})

function mountSettings() {
    const settings = reactive({
        custom_background_path: 'C:/data/backgrounds/old.png',
        custom_background_blur: 0,
        custom_background_opacity: 100,
        custom_background_component_opacity: 100,
    })
    wrapper = mount(LauncherBackgroundSettings, {
        attachTo: document.body,
        props: { settings },
    })
    return settings
}

const webpResult = {
    blob: new Blob(['webp'], { type: 'image/webp' }),
    bytes: new Uint8Array([9, 8, 7]),
    width: 1920,
    height: 1080,
    mimeType: 'image/webp' as const,
    extension: 'webp' as const,
}

const cropSelection = {
    sourceWidth: 64,
    sourceHeight: 32,
    crop: { left: 0, top: 0, width: 64, height: 32 },
    normalizedCrop: { left: 0, top: 0, width: 1, height: 1 },
    aspectRatio: 2,
    outputWidth: 64,
    outputHeight: 32,
    oversizedSource: false,
    preserveOriginalQuality: false,
}

const convertedBackground = {
    path: 'C:/data/backgrounds/launcher-background-1-abcd.webp',
    width: 64,
    height: 32,
    frameCount: 2,
    durationMs: 100,
    byteLength: 1234,
}

function backgroundWriteCall() {
    return mocks.writeFile.mock.calls.find(([path]) => !String(path).endsWith('.source'))
}

function sourceWriteCall() {
    return mocks.writeFile.mock.calls.find(([path]) => String(path).endsWith('.source'))
}

function overlayPanel() {
    return document.querySelector<HTMLElement>('[data-testid="custom-background-overlay-panel"]')
}

it('crops a chosen image before writing it and replaces the previous file', async () => {
    const settings = mountSettings()
    mocks.open.mockResolvedValue('C:/pictures/wall.png')
    mocks.readFile.mockResolvedValue(new Uint8Array([1, 2, 3]))
    mocks.exists.mockResolvedValue(true)
    mocks.cropShow.mockResolvedValue(webpResult)

    await wrapper!.get('[data-testid="custom-background-replace"]').trigger('click')
    await flushPromises()

    expect(mocks.open).toHaveBeenCalledOnce()
    const openOptions = mocks.open.mock.calls[0][0] as {
        filters: Array<{ name: string; extensions: string[] }>
    }
    expect(openOptions.filters[0].extensions).toEqual([...CUSTOM_BACKGROUND_EXTENSIONS])
    expect(openOptions.filters[1].extensions).toEqual(['*'])
    expect(mocks.cropShow).toHaveBeenCalledOnce()
    const [blob, options] = mocks.cropShow.mock.calls[0]
    expect(blob).toBeInstanceOf(Blob)
    expect((blob as Blob).type).toBe('image/png')
    expect(options.defaultAspectRatioId).toBe('window')
    expect(options.aspectRatioOptions.map((option: { id: string }) => option.id)).toEqual([
        'window',
        '16:9',
        '16:10',
    ])

    expect(mocks.writeFile).toHaveBeenCalledTimes(2)
    const backgroundWrite = backgroundWriteCall()
    expect(backgroundWrite).toBeDefined()
    const writtenPath = backgroundWrite![0] as string
    expect(writtenPath).toMatch(/launcher-background-\d+-[a-z0-9]+\.webp$/)
    expect(sourceWriteCall()![0]).toBe(writtenPath.replace(/\.[^./\\]+$/, '.source'))
    expect(sourceWriteCall()![1]).toEqual(new Uint8Array([1, 2, 3]))
    expect(settings.custom_background_path).toBe(writtenPath)
    expect(mocks.remove).toHaveBeenCalledWith('C:/data/backgrounds/old.png')
})

it('leaves the previous background untouched when cropping is cancelled', async () => {
    const settings = mountSettings()
    mocks.open.mockResolvedValue('C:/pictures/wall.png')
    mocks.readFile.mockResolvedValue(new Uint8Array([1, 2, 3]))
    mocks.cropShow.mockResolvedValue(null)

    await wrapper!.get('[data-testid="custom-background-replace"]').trigger('click')
    await flushPromises()

    expect(mocks.cropShow).toHaveBeenCalledOnce()
    expect(mocks.writeFile).not.toHaveBeenCalled()
    expect(mocks.remove).not.toHaveBeenCalled()
    expect(settings.custom_background_path).toBe('C:/data/backgrounds/old.png')
})

it('keeps the old background and reports an error when the write fails', async () => {
    const settings = mountSettings()
    mocks.open.mockResolvedValue('C:/pictures/wall.png')
    mocks.readFile.mockResolvedValue(new Uint8Array([1, 2, 3]))
    mocks.exists.mockResolvedValue(true)
    mocks.cropShow.mockResolvedValue(webpResult)
    mocks.writeFile.mockRejectedValue(new Error('disk full'))
    mocks.remove.mockResolvedValue(undefined)

    await wrapper!.get('[data-testid="custom-background-replace"]').trigger('click')
    await flushPromises()

    expect(mocks.handleError).toHaveBeenCalledOnce()
    expect(settings.custom_background_path).toBe('C:/data/backgrounds/old.png')
    expect(mocks.remove).not.toHaveBeenCalledWith('C:/data/backgrounds/old.png')
})

it('opens the cropper for a dropped image instead of storing it directly', async () => {
    mountSettings()
    await flushPromises()
    expect(mocks.dragHandler).not.toBeNull()

    const preview = wrapper!.get('[role="button"]').element as HTMLElement
    preview.style.width = '400px'
    preview.style.height = '200px'
    const bounds = preview.getBoundingClientRect()

    mocks.invoke.mockResolvedValue(new Uint8Array([4, 5, 6]).buffer)
    mocks.cropShow.mockResolvedValue(webpResult)

    mocks.dragHandler!({
        payload: {
            type: 'drop',
            paths: ['C:/pictures/dropped.png'],
            position: { x: bounds.left + 10, y: bounds.top + 10 },
        },
    })
    await flushPromises()

    expect(mocks.invoke).toHaveBeenCalledWith('plugin:files|file_read_dragged_file', {
        path: 'C:/pictures/dropped.png',
    })
    expect(mocks.cropShow).toHaveBeenCalledOnce()
    expect(mocks.writeFile).toHaveBeenCalledTimes(2)
})

it('converts an animated PNG through the native backend', async () => {
    const settings = mountSettings()
    mocks.open.mockResolvedValue('C:/pictures/animated.png')
    mocks.readFile.mockResolvedValue(
        new Uint8Array([
            0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0, 0x61, 0x63, 0x54, 0x4c, 0,
            0, 0, 0,
        ]),
    )
    mocks.exists.mockResolvedValue(true)
    mocks.cropShowSelection.mockResolvedValue(cropSelection)
    mocks.convertAnimatedBackground.mockResolvedValue(convertedBackground)

    await wrapper!.get('[data-testid="custom-background-replace"]').trigger('click')
    await flushPromises()

    expect(mocks.cropShow).not.toHaveBeenCalled()
    expect(mocks.cropShowSelection).toHaveBeenCalledOnce()
    const [blob, options] = mocks.cropShowSelection.mock.calls[0]
    expect((blob as Blob).type).toBe('image/png')
    expect(options.animated).toBe(true)
    expect(options.maxOutputDimension).toBe(1920)

    expect(mocks.convertAnimatedBackground).toHaveBeenCalledWith(
        expect.objectContaining({
            inputPath: 'C:/pictures/animated.png',
            outputPath: expect.stringMatching(/launcher-background-\d+-[a-z0-9]+\.webp$/),
            crop: cropSelection.normalizedCrop,
            outputWidth: 64,
            outputHeight: 32,
            backend: 'auto',
        }),
    )
    expect(settings.custom_background_path).toBe(convertedBackground.path)
    expect(mocks.remove).toHaveBeenCalledWith('C:/data/backgrounds/old.png')
})

it('shows a prominent status while an animated background converts', async () => {
    mountSettings()
    mocks.open.mockResolvedValue('C:/pictures/loop.gif')
    mocks.readFile.mockResolvedValue(new Uint8Array([0x47, 0x49, 0x46, 0x38, 0x39, 0x61]))
    mocks.exists.mockResolvedValue(true)
    mocks.cropShowSelection.mockResolvedValue(cropSelection)
    let resolveConversion!: (result: typeof convertedBackground) => void
    mocks.convertAnimatedBackground.mockReturnValue(
        new Promise((resolve) => {
            resolveConversion = resolve
        }),
    )

    await wrapper!.get('[data-testid="custom-background-replace"]').trigger('click')
    await flushPromises()

    const status = wrapper!.get('[data-testid="custom-background-converting"]')
    expect(status.text()).toContain('Converting animated background')
    expect(status.text()).toContain('This can take a few seconds')
    expect(wrapper!.find('[role="button"][aria-busy="true"]').exists()).toBe(true)

    resolveConversion(convertedBackground)
    await flushPromises()

    expect(wrapper!.find('[data-testid="custom-background-converting"]').exists()).toBe(false)
})

it('leaves the old background when an animated conversion is cancelled', async () => {
    const settings = mountSettings()
    mocks.open.mockResolvedValue('C:/pictures/loop.gif')
    mocks.readFile.mockResolvedValue(new Uint8Array([0x47, 0x49, 0x46, 0x38, 0x39, 0x61]))
    mocks.cropShowSelection.mockResolvedValue(null)

    await wrapper!.get('[data-testid="custom-background-replace"]').trigger('click')
    await flushPromises()

    expect(mocks.cropShowSelection).toHaveBeenCalledOnce()
    expect(mocks.convertAnimatedBackground).not.toHaveBeenCalled()
    expect(settings.custom_background_path).toBe('C:/data/backgrounds/old.png')
})

it('keeps the old background when an animated conversion fails', async () => {
    const settings = mountSettings()
    mocks.open.mockResolvedValue('C:/pictures/loop.gif')
    mocks.readFile.mockResolvedValue(new Uint8Array([0x47, 0x49, 0x46, 0x38, 0x39, 0x61]))
    mocks.exists.mockResolvedValue(true)
    mocks.cropShowSelection.mockResolvedValue(cropSelection)
    mocks.convertAnimatedBackground.mockRejectedValue({
        code: 'decode_failed',
        message: 'bad animation',
    })

    await wrapper!.get('[data-testid="custom-background-replace"]').trigger('click')
    await flushPromises()

    expect(mocks.addNotification).toHaveBeenCalledWith(expect.objectContaining({ type: 'error' }))
    expect(settings.custom_background_path).toBe('C:/data/backgrounds/old.png')
    expect(mocks.remove).not.toHaveBeenCalledWith('C:/data/backgrounds/old.png')
})

it('passes the oversized auto-compress choice to the backend', async () => {
    mountSettings()
    mocks.open.mockResolvedValue('C:/pictures/loop.gif')
    mocks.readFile.mockResolvedValue(new Uint8Array([0x47, 0x49, 0x46, 0x38, 0x39, 0x61]))
    mocks.exists.mockResolvedValue(true)
    mocks.cropShowSelection.mockResolvedValue({
        ...cropSelection,
        oversizedSource: true,
    })
    mocks.convertAnimatedBackground.mockResolvedValue(convertedBackground)

    await wrapper!.get('[data-testid="custom-background-replace"]').trigger('click')
    await flushPromises()

    expect(mocks.convertAnimatedBackground).toHaveBeenCalledWith(
        expect.objectContaining({ allowOversizedSource: true }),
    )
})

it('keeps the source quality when the user keeps an oversized animation', async () => {
    const settings = mountSettings()
    mocks.open.mockResolvedValue('C:/pictures/loop.gif')
    mocks.readFile.mockResolvedValue(new Uint8Array([0x47, 0x49, 0x46, 0x38, 0x39, 0x61]))
    mocks.exists.mockResolvedValue(true)
    mocks.cropShowSelection.mockResolvedValue({
        ...cropSelection,
        oversizedSource: true,
        preserveOriginalQuality: true,
    })
    mocks.convertAnimatedBackground.mockResolvedValue(convertedBackground)

    await wrapper!.get('[data-testid="custom-background-replace"]').trigger('click')
    await flushPromises()

    expect(mocks.convertAnimatedBackground).toHaveBeenCalledWith(
        expect.objectContaining({
            preserveOriginalQuality: true,
            allowOversizedSource: true,
            quality: 90,
            maxFrames: 1000,
            maxDurationMs: 60_000,
            maxOutputBytes: MAX_PRESERVED_BACKGROUND_BYTES,
        }),
    )
    expect(settings.custom_background_path).toBe(convertedBackground.path)
})

it('keeps the source quality for a still image', async () => {
    mountSettings()
    mocks.open.mockResolvedValue('C:/pictures/photo.jpg')
    mocks.readFile.mockResolvedValue(new Uint8Array([1, 2, 3]))
    mocks.exists.mockResolvedValue(true)
    mocks.cropShow.mockResolvedValue({
        ...webpResult,
        preserveOriginalQuality: true,
    })

    await wrapper!.get('[data-testid="custom-background-replace"]').trigger('click')
    await flushPromises()

    expect(mocks.writeFile).toHaveBeenCalledTimes(2)
    const [writtenPath] = backgroundWriteCall()!
    expect(writtenPath).toMatch(/\.webp$/)
})

it('explains that AVIF and MP4 need the future FFmpeg backend', async () => {
    mountSettings()
    mocks.open.mockResolvedValue('C:/pictures/clip.mp4')
    mocks.readFile.mockResolvedValue(new Uint8Array([0, 0, 0, 0x18]))

    await wrapper!.get('[data-testid="custom-background-replace"]').trigger('click')
    await flushPromises()

    expect(mocks.cropShow).not.toHaveBeenCalled()
    expect(mocks.cropShowSelection).not.toHaveBeenCalled()
    expect(mocks.convertAnimatedBackground).not.toHaveBeenCalled()
    expect(mocks.addNotification).toHaveBeenCalledWith(expect.objectContaining({ type: 'warning' }))
})

it('does not open a second picker while a crop session is pending', async () => {
    mountSettings()
    mocks.open.mockResolvedValue('C:/pictures/wall.png')
    mocks.readFile.mockResolvedValue(new Uint8Array([1, 2, 3]))
    let resolveCrop!: (result: typeof webpResult | null) => void
    mocks.cropShow.mockReturnValue(
        new Promise((resolve) => {
            resolveCrop = resolve
        }),
    )

    await wrapper!.get('[data-testid="custom-background-replace"]').trigger('click')
    await flushPromises()
    await wrapper!.get('[data-testid="custom-background-replace"]').trigger('click')
    await flushPromises()

    expect(mocks.open).toHaveBeenCalledOnce()

    resolveCrop(null)
    await flushPromises()
})

it('removes the current background only after its file is deleted', async () => {
    const settings = mountSettings()
    mocks.exists.mockResolvedValue(true)
    mocks.remove.mockResolvedValue(undefined)

    await wrapper!.get('[data-testid="custom-background-remove"]').trigger('click')
    await flushPromises()

    expect(mocks.remove).toHaveBeenCalledWith('C:/data/backgrounds/old.png')
    expect(settings.custom_background_path).toBeNull()
})

it('keeps the background when deleting its file fails', async () => {
    const settings = mountSettings()
    mocks.exists.mockResolvedValue(true)
    mocks.remove.mockRejectedValue(new Error('permission denied'))

    await wrapper!.get('[data-testid="custom-background-remove"]').trigger('click')
    await flushPromises()

    expect(mocks.handleError).toHaveBeenCalledOnce()
    expect(settings.custom_background_path).toBe('C:/data/backgrounds/old.png')
})

it('rejects an oversized encoded background before writing it', async () => {
    const settings = mountSettings()
    mocks.open.mockResolvedValue('C:/pictures/wall.png')
    mocks.readFile.mockResolvedValue(new Uint8Array([1, 2, 3]))
    mocks.cropShow.mockResolvedValue({
        ...webpResult,
        bytes: { byteLength: MAX_CUSTOM_BACKGROUND_BYTES + 1 } as unknown as Uint8Array,
    })

    await wrapper!.get('[data-testid="custom-background-replace"]').trigger('click')
    await flushPromises()

    expect(mocks.writeFile).not.toHaveBeenCalled()
    expect(mocks.addNotification).toHaveBeenCalledWith(expect.objectContaining({ type: 'error' }))
    expect(settings.custom_background_path).toBe('C:/data/backgrounds/old.png')
})

it('sniffs a supported image selected with an unknown extension', async () => {
    mountSettings()
    mocks.open.mockResolvedValue('C:/pictures/no-extension')
    mocks.readFile.mockResolvedValue(
        new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    )
    mocks.cropShow.mockResolvedValue(webpResult)

    await wrapper!.get('[data-testid="custom-background-replace"]').trigger('click')
    await flushPromises()

    expect(mocks.cropShow).toHaveBeenCalledOnce()
    expect((mocks.cropShow.mock.calls[0][0] as Blob).type).toBe('image/png')
})

it('rejects an unsupported file selected through the all-files filter', async () => {
    mountSettings()
    mocks.open.mockResolvedValue('C:/pictures/notes.txt')
    mocks.readFile.mockResolvedValue(new TextEncoder().encode('not an image'))

    await wrapper!.get('[data-testid="custom-background-replace"]').trigger('click')
    await flushPromises()

    expect(mocks.cropShow).not.toHaveBeenCalled()
    expect(mocks.writeFile).not.toHaveBeenCalled()
    expect(mocks.addNotification).toHaveBeenCalledWith(expect.objectContaining({ type: 'warning' }))
})

it('re-crops the stored source when the preview is clicked', async () => {
    mountSettings()
    mocks.exists.mockImplementation(
        async (path: string) => path === 'C:/data/backgrounds/old.source',
    )
    mocks.readFile.mockResolvedValue(
        new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    )
    mocks.cropShow.mockResolvedValue(webpResult)

    await wrapper!.get('[role="button"]').trigger('click')
    await flushPromises()

    expect(mocks.open).not.toHaveBeenCalled()
    expect(mocks.readFile).toHaveBeenCalledWith('C:/data/backgrounds/old.source')
    expect(mocks.cropShow).toHaveBeenCalledOnce()
})

it('keeps the preview collapsed until expanded', async () => {
    mountSettings()
    const preview = wrapper!.get('[role="button"]').element as HTMLElement
    expect(preview.className).toContain('h-32')
    expect(overlayPanel()).toBeNull()

    await wrapper!.get('[data-testid="custom-background-expand"]').trigger('click')
    await vi.waitFor(() => expect(overlayPanel()).not.toBeNull())

    const panel = overlayPanel()!
    expect(Number.parseFloat(panel.style.aspectRatio)).toBeCloseTo(
        window.innerWidth / window.innerHeight,
        3,
    )

    document.querySelector<HTMLElement>('[data-testid="custom-background-overlay-close"]')!.click()
    await flushPromises()
    expect(overlayPanel()).toBeNull()
})

it('auto-expands after hovering still for one second', async () => {
    mountSettings()
    const preview = wrapper!.get('[role="button"]').element as HTMLElement
    preview.dispatchEvent(new PointerEvent('pointerenter', { bubbles: true }))

    await new Promise((resolve) => setTimeout(resolve, 600))
    expect(overlayPanel()).toBeNull()

    preview.dispatchEvent(new PointerEvent('pointermove', { bubbles: true }))
    await new Promise((resolve) => setTimeout(resolve, 600))
    expect(overlayPanel()).toBeNull()

    await vi.waitFor(() => expect(overlayPanel()).not.toBeNull(), { timeout: 1_500 })

    document.querySelector<HTMLElement>('[data-testid="custom-background-overlay-close"]')!.click()
    await flushPromises()
})

it('collapses 280ms after leaving and cancels while hovering the overlay', async () => {
    mountSettings()
    const preview = wrapper!.get('[role="button"]').element as HTMLElement
    preview.dispatchEvent(new PointerEvent('pointerenter', { bubbles: true }))
    await vi.waitFor(() => expect(overlayPanel()).not.toBeNull(), { timeout: 1_500 })

    const panel = overlayPanel()!
    panel.dispatchEvent(new PointerEvent('pointerleave', { bubbles: true }))
    await new Promise((resolve) => setTimeout(resolve, 150))
    expect(overlayPanel()).not.toBeNull()

    panel.dispatchEvent(new PointerEvent('pointerenter', { bubbles: true }))
    await new Promise((resolve) => setTimeout(resolve, 300))
    expect(overlayPanel()).not.toBeNull()

    panel.dispatchEvent(new PointerEvent('pointerleave', { bubbles: true }))
    await vi.waitFor(() => expect(overlayPanel()).toBeNull(), { timeout: 1_000 })
})

it('keeps the expanded preview open when pinned', async () => {
    mountSettings()
    await wrapper!.get('[data-testid="custom-background-expand"]').trigger('click')
    await vi.waitFor(() => expect(overlayPanel()).not.toBeNull())

    overlayPanel()!.dispatchEvent(new PointerEvent('pointerleave', { bubbles: true }))
    await new Promise((resolve) => setTimeout(resolve, 450))
    await nextTick()
    expect(overlayPanel()).not.toBeNull()

    document.querySelector<HTMLElement>('[data-testid="custom-background-overlay-close"]')!.click()
    await flushPromises()
    expect(overlayPanel()).toBeNull()
})

import { flushPromises, mount, type VueWrapper } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'

import type { Skin } from '@/helpers/skins.ts'

import EditSkinModal from './EditSkinModal.vue'

const backend = vi.hoisted(() => ({
    save: vi.fn(),
    writeFile: vi.fn(),
    getNormalizedTexture: vi.fn(),
    normalizeTexture: vi.fn(),
    handleError: vi.fn(),
}))

vi.mock('@tauri-apps/plugin-dialog', () => ({ save: backend.save }))
vi.mock('@tauri-apps/plugin-fs', () => ({ writeFile: backend.writeFile }))
vi.mock('@/helpers/skins.ts', async (importOriginal) => {
    const actual = await importOriginal<typeof import('@/helpers/skins.ts')>()
    return {
        ...actual,
        determineModelType: async () => 'CLASSIC' as const,
        get_normalized_skin_texture: backend.getNormalizedTexture,
        normalize_skin_texture: backend.normalizeTexture,
    }
})
vi.mock('@modrinth/ui', async () => {
    const { defineComponent, h, ref } = await import('vue')
    const Button = defineComponent({
        props: { disabled: Boolean },
        setup(props, { slots }) {
            return () =>
                h('button', { type: 'button', disabled: props.disabled }, slots.default?.())
        },
    })
    const SlotButton = defineComponent({
        setup(_, { slots }) {
            return () => h('button', { type: 'button' }, slots.default?.())
        },
    })
    return {
        Button,
        CapeButton: SlotButton,
        CapeLikeTextButton: SlotButton,
        commonMessages: { cancelButton: { defaultMessage: 'Cancel' } },
        defineMessages: (messages: unknown) => messages,
        injectNotificationManager: () => ({ handleError: backend.handleError }),
        NewModal: defineComponent({
            setup(_, { slots, expose }) {
                expose({ show: () => {}, hide: () => {} })
                return () => h('div', [slots.default?.(), slots.actions?.()])
            },
        }),
        RadioButtons: defineComponent({ render: () => null }),
        SkinPreviewRenderer: defineComponent({ render: () => null }),
        useScrollIndicator: () => ({
            showTopFade: ref(false),
            showBottomFade: ref(false),
            checkScrollState: () => {},
            forceCheck: () => {},
        }),
        useVIntl: () => ({
            formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
        }),
    }
})

const skin = {
    texture_key: 'abc',
    name: 'My Skin',
    variant: 'CLASSIC' as const,
    texture: 'data:image/png;base64,AQID',
    source: 'custom' as const,
    is_equipped: false,
}

let wrapper: VueWrapper | undefined
afterEach(() => {
    wrapper?.unmount()
    wrapper = undefined
    vi.clearAllMocks()
})

async function openExport(skinToEdit: Skin = skin) {
    // Normalization changes the bytes, so exporting them instead of the stored texture fails.
    backend.getNormalizedTexture.mockResolvedValue('data:image/png;base64,BAUG')
    wrapper = mount(EditSkinModal, {
        props: { capes: [] },
        global: { directives: { tooltip: {} } },
    })
    await (
        wrapper.vm as unknown as { show: (e: MouseEvent, skin: typeof skinToEdit) => Promise<void> }
    ).show(new MouseEvent('click'), skinToEdit)
    await flushPromises()

    const button = wrapper
        .findAll('button')
        .find((candidate) => candidate.text().includes('Export texture'))
    expect(button).toBeTruthy()
    return button!
}

it('writes the stored skin texture to the file the user picks', async () => {
    backend.save.mockResolvedValue('/tmp/My Skin.png')

    const button = await openExport()
    await button.trigger('click')
    await flushPromises()

    expect(backend.save).toHaveBeenCalledWith({
        defaultPath: 'My Skin.png',
        filters: [{ name: 'PNG', extensions: ['png'] }],
    })
    await vi.waitFor(() => expect(backend.writeFile).toHaveBeenCalledTimes(1))
    const [path, bytes] = backend.writeFile.mock.calls[0]
    expect(path).toBe('/tmp/My Skin.png')
    expect(Array.from(bytes as Uint8Array)).toEqual([1, 2, 3])
})

it('writes nothing when the save dialog is cancelled', async () => {
    backend.save.mockResolvedValue(null)

    const button = await openExport()
    await button.trigger('click')
    await flushPromises()

    expect(backend.writeFile).not.toHaveBeenCalled()
})

it('disables export for textures that cannot be read without CORS', async () => {
    const button = await openExport({
        ...skin,
        texture: 'https://example.com/skin.png',
        source: 'custom_external',
    })

    expect(button.attributes('disabled')).toBeDefined()
})

it('exports the skin being edited after an earlier texture upload', async () => {
    backend.save.mockResolvedValue('/tmp/My Skin.png')
    wrapper = mount(EditSkinModal, {
        props: { capes: [] },
        global: { directives: { tooltip: {} } },
    })
    const vm = wrapper.vm as unknown as {
        show: (e: MouseEvent, skin: Skin) => Promise<void>
        showNew: (e: MouseEvent, texture: { original: string; normalized: string }) => Promise<void>
    }

    await vm.showNew(new MouseEvent('click'), {
        original: 'data:image/png;base64,BAUG',
        normalized: 'data:image/png;base64,BAUG',
    })
    await flushPromises()
    await vm.show(new MouseEvent('click'), skin)
    await flushPromises()

    const button = wrapper
        .findAll('button')
        .find((candidate) => candidate.text().includes('Export texture'))
    await button!.trigger('click')
    await flushPromises()

    await vi.waitFor(() => expect(backend.writeFile).toHaveBeenCalledTimes(1))
    const [, bytes] = backend.writeFile.mock.calls[0]
    expect(Array.from(bytes as Uint8Array)).toEqual([1, 2, 3])
})

function deferred<T>() {
    let resolve!: (value: T) => void
    let reject!: (error: Error) => void
    const promise = new Promise<T>((res, rej) => {
        resolve = res
        reject = rej
    })
    return { promise, resolve, reject }
}

async function upload(file: File) {
    const transfer = new DataTransfer()
    transfer.items.add(file)
    const input = wrapper!.get<HTMLInputElement>('input[type="file"]')
    input.element.files = transfer.files
    await input.trigger('change')
    await flushPromises()
}

async function showSkin(skinToEdit: Skin) {
    await (
        wrapper!.vm as unknown as { show: (event: MouseEvent, skin: Skin) => Promise<void> }
    ).show(new MouseEvent('click'), skinToEdit)
    await flushPromises()
}

async function expectExport(expected: number[]) {
    backend.save.mockResolvedValue('/tmp/export.png')
    const button = wrapper!
        .findAll('button')
        .find((candidate) => candidate.text().includes('Export texture'))!
    await button.trigger('click')
    await vi.waitFor(() => expect(backend.writeFile).toHaveBeenCalledTimes(1))
    expect(Array.from(backend.writeFile.mock.calls[0][1] as Uint8Array)).toEqual(expected)
}

it.each(['reading', 'normalizing'])(
    'ignores an upload still %s when another skin opens',
    async (stage) => {
        await openExport()
        const read = deferred<ArrayBuffer>()
        const normalize = deferred<Uint8Array>()
        const file = new File([new Uint8Array([7, 8, 9])], 'old.png', { type: 'image/png' })
        vi.spyOn(file, 'arrayBuffer').mockReturnValue(
            stage === 'reading' ? read.promise : Promise.resolve(new Uint8Array([7, 8, 9]).buffer),
        )
        backend.normalizeTexture.mockReturnValue(normalize.promise)
        await upload(file)

        await showSkin({ ...skin, name: 'Other', texture: 'data:image/png;base64,BAUG' })
        read.resolve(new Uint8Array([7, 8, 9]).buffer)
        normalize.resolve(new Uint8Array([7, 8, 9]))
        await flushPromises()

        await expectExport([4, 5, 6])
    },
)

it('keeps the newest upload when an older normalization finishes later', async () => {
    await openExport()
    const older = deferred<Uint8Array>()
    backend.normalizeTexture
        .mockReturnValueOnce(older.promise)
        .mockResolvedValue(new Uint8Array([7, 8, 9]))
    await upload(new File([new Uint8Array([4, 5, 6])], 'old.png'))
    await upload(new File([new Uint8Array([7, 8, 9])], 'new.png'))
    older.resolve(new Uint8Array([4, 5, 6]))
    await flushPromises()

    await expectExport([7, 8, 9])
})

it('invalidates a pending upload as soon as the editor hides', async () => {
    await openExport()
    const normalize = deferred<Uint8Array>()
    backend.normalizeTexture.mockReturnValue(normalize.promise)
    await upload(new File([new Uint8Array([7, 8, 9])], 'old.png'))
    ;(wrapper!.vm as unknown as { hide: () => void }).hide()
    normalize.resolve(new Uint8Array([7, 8, 9]))
    await flushPromises()

    await expectExport([1, 2, 3])
})

it('does not report errors from an upload belonging to a previous skin', async () => {
    await openExport()
    const normalize = deferred<Uint8Array>()
    backend.normalizeTexture.mockReturnValue(normalize.promise)
    await upload(new File([new Uint8Array([7, 8, 9])], 'old.png'))
    await showSkin({ ...skin, name: 'Other', texture: 'data:image/png;base64,BAUG' })
    normalize.reject(new Error('obsolete upload failed'))
    await flushPromises()

    expect(backend.handleError).not.toHaveBeenCalled()
    await expectExport([4, 5, 6])
})

it('captures the filename and texture together before waiting for the save dialog', async () => {
    const dialog = deferred<string | null>()
    backend.save.mockReturnValue(dialog.promise)
    const button = await openExport()
    await button.trigger('click')
    expect(backend.save).toHaveBeenCalledWith({
        defaultPath: 'My Skin.png',
        filters: [{ name: 'PNG', extensions: ['png'] }],
    })
    await showSkin({ ...skin, name: 'Other', texture: 'data:image/png;base64,BAUG' })
    dialog.resolve('/tmp/My Skin.png')

    await vi.waitFor(() => expect(backend.writeFile).toHaveBeenCalledTimes(1))
    expect(backend.writeFile.mock.calls[0][0]).toBe('/tmp/My Skin.png')
    expect(Array.from(backend.writeFile.mock.calls[0][1] as Uint8Array)).toEqual([1, 2, 3])
})

import { flushPromises, mount, type VueWrapper } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'

import type { Skin } from '@/helpers/skins.ts'

import EditSkinModal from './EditSkinModal.vue'

const backend = vi.hoisted(() => ({
    save: vi.fn(),
    writeFile: vi.fn(),
    getNormalizedTexture: vi.fn(),
}))

vi.mock('@tauri-apps/plugin-dialog', () => ({ save: backend.save }))
vi.mock('@tauri-apps/plugin-fs', () => ({ writeFile: backend.writeFile }))
vi.mock('@/helpers/skins.ts', async (importOriginal) => {
    const actual = await importOriginal<typeof import('@/helpers/skins.ts')>()
    return {
        ...actual,
        determineModelType: async () => 'CLASSIC' as const,
        get_normalized_skin_texture: backend.getNormalizedTexture,
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
        injectNotificationManager: () => ({ handleError: vi.fn() }),
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

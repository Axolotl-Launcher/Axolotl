import { flushPromises, mount, type VueWrapper } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'

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

async function openExport() {
    // Normalization changes the bytes, so exporting them instead of the stored texture fails.
    backend.getNormalizedTexture.mockResolvedValue('data:image/png;base64,BAUG')
    wrapper = mount(EditSkinModal, {
        props: { capes: [] },
        global: { directives: { tooltip: {} } },
    })
    await (
        wrapper.vm as unknown as { show: (e: MouseEvent, skin: typeof skin) => Promise<void> }
    ).show(new MouseEvent('click'), skin)
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

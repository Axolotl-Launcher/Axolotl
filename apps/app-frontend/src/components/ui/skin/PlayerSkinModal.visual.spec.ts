import { flushPromises, mount, type VueWrapper } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'

import PlayerSkinModal from './PlayerSkinModal.vue'

const backend = vi.hoisted(() => ({ lookup: vi.fn(), save: vi.fn() }))
vi.mock('@/helpers/skins', () => ({
    lookup_player_skin: backend.lookup,
    save_custom_skin: backend.save,
}))
vi.mock('@tauri-apps/plugin-dialog', () => ({ save: vi.fn() }))
vi.mock('@tauri-apps/plugin-fs', () => ({ writeFile: vi.fn() }))
vi.mock('@modrinth/ui', async () => {
    const { defineComponent, h } = await import('vue')
    return {
        defineMessages: (value: unknown) => value,
        useVIntl: () => ({
            formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
        }),
        SkinPreviewRenderer: defineComponent({ render: () => null }),
        Button: defineComponent({
            setup:
                (_, { slots }) =>
                () =>
                    h('button', { type: 'button' }, slots.default?.()),
        }),
        NewModal: defineComponent({
            setup(_, { slots, expose }) {
                expose({ show: () => {}, hide: () => {} })
                return () => h('div', [slots.default?.(), slots.actions?.()])
            },
        }),
    }
})

let wrapper: VueWrapper | undefined
afterEach(() => {
    wrapper?.unmount()
    vi.resetAllMocks()
})

const player = {
    status: 'found',
    name: 'Notch',
    uuid: '069a79f4-44e9-4726-a5be-fca90e38aaf5',
    variant: 'CLASSIC',
    texture: 'data:image/png;base64,AQID',
    preview_texture: 'data:image/png;base64,AQID',
}

it('saves the original texture without applying it, and prevents a repeated import', async () => {
    backend.lookup.mockResolvedValue(player)
    backend.save.mockResolvedValue({ texture_key: 'saved', is_equipped: false })
    wrapper = mount(PlayerSkinModal)
    await wrapper.get('input').setValue('Notch')
    await wrapper.get('form').trigger('submit')
    await flushPromises()
    const importButton = wrapper
        .findAll('button')
        .find((button) => button.text() === 'Add to saved skins')!
    await importButton.trigger('click')
    await flushPromises()
    expect(backend.save).toHaveBeenCalledWith(
        expect.objectContaining({ is_equipped: false, variant: 'CLASSIC' }),
        new Uint8Array([1, 2, 3]),
        'CLASSIC',
        undefined,
        true,
    )
    expect(wrapper.emitted('saved')?.[0]?.[0]).toEqual({
        applied: false,
        skin: { texture_key: 'saved', is_equipped: false },
    })
    expect(wrapper.get('[role="status"]').text()).toBe('Added to saved skins')
    expect(importButton.attributes('disabled')).toBeDefined()
})

it('discards a lookup that completes after the modal closes', async () => {
    let resolve!: (value: unknown) => void
    backend.lookup.mockReturnValue(
        new Promise((done) => {
            resolve = done
        }),
    )
    wrapper = mount(PlayerSkinModal)
    await wrapper.get('input').setValue('Notch')
    await wrapper.get('form').trigger('submit')
    ;(wrapper.vm as unknown as { hide: () => void }).hide()
    resolve(player)
    await flushPromises()
    expect(wrapper.find('strong').exists()).toBe(false)
    expect(backend.save).not.toHaveBeenCalled()
})

it('preserves the model, cape and equipped state of an already saved texture', async () => {
    backend.lookup.mockResolvedValue(player)
    const existing = {
        texture_key: 'saved',
        variant: 'SLIM' as const,
        texture: player.texture,
        cape_id: 'my-cape',
        source: 'custom' as const,
        is_equipped: true,
    }
    wrapper = mount(PlayerSkinModal, { props: { savedSkins: [existing] } })
    await wrapper.get('input').setValue('Notch')
    await wrapper.get('form').trigger('submit')
    await flushPromises()
    await wrapper
        .findAll('button')
        .find((button) => button.text() === 'Add to saved skins')!
        .trigger('click')
    await flushPromises()
    expect(backend.save).not.toHaveBeenCalled()
    expect(wrapper.emitted('saved')?.[0]?.[0]).toEqual({ applied: false, skin: existing })
})

it('shows a lookup error without allowing import', async () => {
    backend.lookup.mockResolvedValue({ status: 'not_found' })
    wrapper = mount(PlayerSkinModal)
    await wrapper.get('input').setValue('MissingPlayer')
    await wrapper.get('form').trigger('submit')
    await flushPromises()
    expect(wrapper.get('[role="alert"]').text()).toContain('Player not found')
    expect(wrapper.find('strong').exists()).toBe(false)
})

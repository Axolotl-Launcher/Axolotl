import { flushPromises, mount, type VueWrapper } from '@vue/test-utils'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { ref } from 'vue'

import SkinListSkeleton from '@/components/ui/skin/SkinListSkeleton.vue'
import VirtualSkinSectionList from '@/components/ui/skin/VirtualSkinSectionList.vue'
import { notifyAccountSelectionChanged } from '@/helpers/account-state'
import type { Skin } from '@/helpers/skins'

import Skins from './Skins.vue'

const api = vi.hoisted(() => ({ invoke: vi.fn(), handleError: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: api.invoke }))
vi.mock('@tauri-apps/api/webview', () => ({
    getCurrentWebview: () => ({ onDragDropEvent: async () => () => {} }),
}))
vi.mock('vue-router', () => ({
    useRouter: () => ({ push: vi.fn(), afterEach: () => () => {} }),
    onBeforeRouteLeave: vi.fn(),
}))
vi.mock('@modrinth/ui', async () => {
    const { defineComponent, h } = await import('vue')
    const empty = { render: () => null }
    return {
        defineMessages: (messages: unknown) => messages,
        commonMessages: {
            deleteLabel: { defaultMessage: 'Delete' },
            resetButton: { defaultMessage: 'Reset' },
        },
        createDefaultArmorPreviewConfig: () => ({}),
        cloneArmorPreviewConfig: (config: object) => ({ ...config }),
        useVIntl: () => ({
            formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
        }),
        injectNotificationManager: () => ({
            addNotification: vi.fn(),
            handleError: api.handleError,
        }),
        SkinPreviewRenderer: defineComponent({
            props: ['textureSrc', 'nametag'],
            setup:
                (props, { slots }) =>
                () =>
                    h('div', { 'data-test': 'preview' }, [
                        props.nametag,
                        slots['nametag-badge']?.(),
                    ]),
        }),
        ArmorPreviewControls: empty,
        NavTabs: empty,
        ConfirmModal: empty,
        Button: defineComponent({
            setup:
                (_props, { slots }) =>
                () =>
                    h('button', slots.default?.()),
        }),
    }
})
vi.mock('@/helpers/skin-armor-preview', () => ({
    loadSkinArmorPreview: () => ({}),
    saveSkinArmorPreview: vi.fn(),
}))
vi.mock('@/store/state', () => ({ useTheming: () => ({ hideNametagSkinsPage: false }) }))
vi.mock('@/composables/useMojangAuth', async () => {
    const { ref } = await import('vue')
    return { useMojangAuthStatus: () => ({ reachable: ref(true) }) }
})
vi.mock('@/helpers/rendering/skin-preview-renderer', () => ({
    generateSkinPreviews: async () => {},
    cancelSkinPreviews: vi.fn(),
}))
vi.mock('@/components/ui/skin/EditSkinModal.vue', () => ({ default: { render: () => null } }))
vi.mock('@/components/ui/skin/ArmorTrimTab.vue', () => ({ default: { render: () => null } }))
vi.mock('@/components/ui/skin/VirtualSkinSectionList.vue', async () => {
    const { defineComponent, h } = await import('vue')
    return {
        default: defineComponent({
            props: ['savedSkins'],
            emits: ['select'],
            setup: (props) => () =>
                h(
                    'div',
                    { 'data-test': 'skins' },
                    props.savedSkins.map((skin: Skin) => skin.texture_key).join(','),
                ),
        }),
    }
})

function deferred<T>() {
    let resolve!: (value: T) => void
    let reject!: (reason: Error) => void
    const promise = new Promise<T>((resolvePromise, rejectPromise) => {
        resolve = resolvePromise
        reject = rejectPromise
    })
    return { promise, resolve, reject }
}

const savedSkin: Skin = {
    texture_key: 'saved',
    texture: 'data:image/png;base64,saved',
    variant: 'CLASSIC',
    source: 'custom',
    is_equipped: true,
}
const otherSkin: Skin = { ...savedSkin, texture_key: 'other', is_equipped: false }
let onlineSkins = deferred<Skin[]>()
let onlineCapes = deferred<never[]>()
let wrapper: VueWrapper | undefined

beforeEach(() => {
    onlineSkins = deferred<Skin[]>()
    onlineCapes = deferred<never[]>()
    api.invoke.mockImplementation((command: string, args?: { cachedOnly?: boolean }) => {
        switch (command) {
            case 'plugin:auth|get_default_user':
                return Promise.resolve('account')
            case 'plugin:auth|get_users':
                if (!args?.cachedOnly) throw new Error('Account reads must use the cache')
                return Promise.resolve([
                    {
                        account_id: 'account',
                        account_type: 'microsoft',
                        profile: { name: 'Player' },
                    },
                ])
            case 'plugin:minecraft-skins|get_available_skins':
                return args?.cachedOnly
                    ? Promise.resolve([savedSkin, otherSkin])
                    : onlineSkins.promise
            case 'plugin:minecraft-skins|get_available_capes':
                return args?.cachedOnly ? Promise.resolve([]) : onlineCapes.promise
            case 'plugin:minecraft-skins|normalize_skin_texture':
                return Promise.resolve(new Uint8Array())
            case 'plugin:minecraft-skins|equip_skin':
                return Promise.resolve()
            default:
                throw new Error(`Unexpected command: ${command}`)
        }
    })
})

afterEach(() => {
    wrapper?.unmount()
    wrapper = undefined
    document.body.replaceChildren()
    vi.clearAllMocks()
})

function openSkins() {
    wrapper = mount(Skins, {
        attachTo: document.body,
        global: { provide: { accountsCard: ref(null) } },
    })
    return wrapper
}

it('renders the page shell immediately and saved skins while online reads are pending', async () => {
    const view = openSkins()
    expect(view.find('h1').text()).toBe('Skin selector')
    expect(view.findComponent(SkinListSkeleton).exists()).toBe(true)
    expect(view.text()).not.toContain('Please sign in')
    await flushPromises()
    expect(view.find('[data-test="skins"]').text()).toBe('saved,other')
    expect(view.find('[data-test="preview"]').text()).toBe('Player')
    expect(view.findComponent(SkinListSkeleton).exists()).toBe(false)
    expect(api.invoke).toHaveBeenCalledWith('plugin:minecraft-skins|get_available_capes', {
        cachedOnly: false,
    })
    expect(api.invoke).not.toHaveBeenCalledWith('plugin:minecraft-skins|get_available_skins', {
        cachedOnly: false,
    })
})

it('keeps a new preview selection when the background profile arrives', async () => {
    const view = openSkins()
    await flushPromises()
    onlineCapes.resolve([])
    await flushPromises()
    view.findComponent(VirtualSkinSectionList).vm.$emit('select', otherSkin)
    await flushPromises()
    expect(view.text()).toContain('Previewing')
    onlineSkins.resolve([{ ...savedSkin }, { ...otherSkin }])
    await flushPromises()
    expect(view.text()).toContain('Previewing')
    expect(view.text()).toContain('Apply')
})

it('retains the cached library when the online refresh fails', async () => {
    const view = openSkins()
    await flushPromises()
    onlineSkins.reject(new Error('Mojang is unavailable'))
    onlineCapes.reject(new Error('Mojang is unavailable'))
    await flushPromises()
    expect(view.find('[data-test="skins"]').text()).toBe('saved,other')
    expect(api.handleError).toHaveBeenCalled()
})

it('does not roll back a skin applied while the background profile is pending', async () => {
    const view = openSkins()
    await flushPromises()
    onlineCapes.resolve([])
    await flushPromises()
    view.findComponent(VirtualSkinSectionList).vm.$emit('select', otherSkin)
    await flushPromises()
    await view
        .findAll('button')
        .find((button) => button.text() === 'Apply')!
        .trigger('click')
    await flushPromises()
    onlineSkins.resolve([{ ...savedSkin }, { ...otherSkin }])
    await flushPromises()
    const skins = view.findComponent(VirtualSkinSectionList).props('savedSkins') as Skin[]
    expect(skins.find((skin) => skin.is_equipped)?.texture_key).toBe('other')
    expect(view.text()).not.toContain('Previewing')
})

it('updates the equipped skin from the background profile when no selection changed', async () => {
    const view = openSkins()
    await flushPromises()
    onlineCapes.resolve([])
    onlineSkins.resolve([{ ...otherSkin, is_equipped: true }])
    await flushPromises()
    expect(view.find('[data-test="skins"]').text()).toBe('other')
    expect(view.text()).not.toContain('Previewing')
})

it('discards an old account response after the selected account changes', async () => {
    const view = openSkins()
    await flushPromises()
    onlineCapes.resolve([])
    await flushPromises()
    const oldResponse = onlineSkins
    onlineSkins = deferred<Skin[]>()
    notifyAccountSelectionChanged()
    await flushPromises()
    oldResponse.resolve([{ ...savedSkin, texture_key: 'old-account' }])
    await flushPromises()
    expect(view.find('[data-test="skins"]').text()).toBe('saved,other')
})

it('does not start library reads if the page unmounts during account loading', async () => {
    const accounts = deferred<never[]>()
    api.invoke.mockImplementation((command: string) =>
        command === 'plugin:auth|get_users' ? accounts.promise : Promise.resolve('account'),
    )
    openSkins().unmount()
    wrapper = undefined
    accounts.resolve([])
    await flushPromises()
    expect(api.invoke.mock.calls.every(([command]) => command.startsWith('plugin:auth|'))).toBe(
        true,
    )
})

import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { defineComponent, h, Suspense } from 'vue'
import Draggable from 'vuedraggable'

import { useNetworkStatus } from '@/composables/useNetworkStatus'
import { ACCOUNT_ORDER_STORAGE_KEY, readAccountOrder } from '@/helpers/account-order'
import { notifyAccountSelectionChanged } from '@/helpers/account-state'
import { set_default_user } from '@/helpers/auth'

import AccountsCard from './AccountsCard.vue'

const backend = vi.hoisted(() => ({
    users: vi.fn(),
    defaultUser: vi.fn(),
    heads: vi.fn(),
    skins: vi.fn(),
    error: vi.fn(),
}))

vi.mock('@/helpers/auth', () => ({
    users: backend.users,
    get_default_user: backend.defaultUser,
    add_offline_user: vi.fn(),
    begin_yggdrasil_login: vi.fn(),
    delete_yggdrasil_password: vi.fn(),
    finish_yggdrasil_login: vi.fn(),
    get_yggdrasil_password: vi.fn(),
    list_yggdrasil_saved_logins: vi.fn(),
    login: vi.fn(),
    remove_user: vi.fn(),
    set_default_user: vi.fn(),
    set_yggdrasil_password: vi.fn(),
}))
vi.mock('@/helpers/skins', () => ({ get_available_skins: backend.skins }))
vi.mock('@/helpers/rendering/batch-skin-renderer.ts', () => ({
    getPlayerHeadUrl: backend.heads,
    retainPlayerHeadTextures: () => () => {},
}))
vi.mock('@/helpers/events', () => ({ process_listener: async () => () => {} }))
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => {} }))
vi.mock('@tanstack/vue-query', () => ({ useQueryClient: () => ({}) }))
vi.mock('@/helpers/analytics', () => ({ trackEvent: vi.fn() }))
vi.mock('@/store/error.js', () => ({
    handleSevereError: vi.fn(),
    useError: () => ({ showAntiPiracyNotice: vi.fn() }),
}))
vi.mock('@/composables/useAntiPiracyStatus', async () => {
    const { ref } = await import('vue')
    return {
        isOfflineAccountRestrictedError: () => false,
        useAntiPiracyStatus: () => ({
            status: ref({ restricted: false }),
            refresh: vi.fn(),
        }),
    }
})
vi.mock('vue-router', () => ({ useRoute: () => ({ fullPath: '/' }) }))
vi.mock('@/components/ui/MinecraftLoginModal.vue', () => ({ default: { render: () => null } }))
vi.mock('@modrinth/ui', async () => {
    const { h, defineComponent } = await import('vue')
    const container = defineComponent({
        setup(_, { slots }) {
            return () => h('div', [slots.title?.(), slots.default?.()])
        },
    })
    return {
        Accordion: container,
        Admonition: container,
        Avatar: defineComponent({
            props: ['src'],
            setup: (props) => () => h('img', { src: props.src }),
        }),
        Button: container,
        Checkbox: container,
        IconButton: container,
        NewModal: defineComponent({ render: () => null }),
        StyledInput: container,
        commonMessages: {},
        defineMessages: (messages: unknown) => messages,
        injectNotificationManager: () => ({ handleError: backend.error }),
        useVIntl: () => ({
            formatMessage: (message: { defaultMessage: string }) => message?.defaultMessage,
        }),
    }
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

function account(name: string, accountId = 'microsoft:saved-id', profileId = 'saved-id') {
    return {
        account_id: accountId,
        account_type: 'microsoft',
        profile: {
            id: profileId,
            name,
            skins: [
                { state: 'ACTIVE', url: 'https://example.test/skin', textureKey: 'saved-skin' },
            ],
        },
    }
}

const cleanup: (() => void)[] = []
beforeEach(() => {
    vi.resetAllMocks()
    localStorage.removeItem(ACCOUNT_ORDER_STORAGE_KEY)
    useNetworkStatus().setBrowserOffline(false)
    useNetworkStatus().markBrowserOnline()
    backend.defaultUser.mockResolvedValue('microsoft:saved-id')
    backend.skins.mockResolvedValue([])
})
afterEach(() =>
    cleanup
        .splice(0)
        .reverse()
        .forEach((stop) => stop()),
)

function mountCard() {
    const wrapper = mount(
        defineComponent({
            setup: () => () => h(Suspense, null, { default: () => h(AccountsCard) }),
        }),
        { global: { directives: { tooltip: {} } } },
    )
    cleanup.push(() => wrapper.unmount())
    return wrapper
}

it('shows an account selected outside the card without a restart', async () => {
    backend.defaultUser.mockResolvedValue(null)
    backend.users.mockResolvedValue([])
    backend.heads.mockResolvedValue('data:image/png;base64,avatar')
    const wrapper = mountCard()
    await flushPromises()
    expect(wrapper.text()).not.toContain('SignedInElsewhere')

    backend.defaultUser.mockResolvedValue('microsoft:saved-id')
    backend.users.mockResolvedValue([account('SignedInElsewhere')])
    notifyAccountSelectionChanged()
    await flushPromises()

    expect(wrapper.text()).toContain('SignedInElsewhere')
})

it('picks up an outside selection that lands during its own failed write', async () => {
    backend.users.mockResolvedValue([account('BeforeOutsideChange')])
    backend.heads.mockResolvedValue('data:image/png;base64,avatar')
    const write = deferred<void>()
    vi.mocked(set_default_user).mockReturnValue(write.promise)
    const wrapper = mountCard()
    await flushPromises()

    await wrapper
        .findAll('button')
        .find((button) => button.text().includes('BeforeOutsideChange'))!
        .trigger('click')
    await flushPromises()
    expect(set_default_user).toHaveBeenCalledOnce()

    backend.users.mockResolvedValue([account('AfterOutsideChange')])
    notifyAccountSelectionChanged()
    write.reject(new Error('write failed'))
    await flushPromises()

    expect(wrapper.text()).toContain('AfterOutsideChange')
})

it('shows saved accounts while online profiles and head rendering are still pending', async () => {
    const online = deferred<ReturnType<typeof account>[]>()
    const heads = deferred<string>()
    backend.users.mockImplementation((_: boolean, cachedOnly: boolean) =>
        cachedOnly ? Promise.resolve([account('SavedPlayer')]) : online.promise,
    )
    backend.heads.mockReturnValue(heads.promise)
    const wrapper = mountCard()
    await flushPromises()
    expect(wrapper.text()).toContain('SavedPlayer')
    expect(wrapper.text()).not.toContain('Not signed in')
    expect(backend.users).toHaveBeenCalledWith(false, true)
    expect(backend.users).toHaveBeenCalledWith(false)
    expect(backend.skins).not.toHaveBeenCalled()

    heads.resolve('data:image/png;base64,cached-avatar')
    await flushPromises()
    expect(wrapper.find('img').attributes('src')).toBe('data:image/png;base64,cached-avatar')
    online.resolve([account('UpdatedPlayer')])
    await flushPromises()
    expect(wrapper.text()).toContain('UpdatedPlayer')
    expect(backend.skins).toHaveBeenCalledOnce()
})

it('keeps local accounts and cached avatars when online refresh fails', async () => {
    const online = deferred<ReturnType<typeof account>[]>()
    backend.users.mockImplementation((_: boolean, cachedOnly: boolean) =>
        cachedOnly ? Promise.resolve([account('SavedPlayer')]) : online.promise,
    )
    backend.heads.mockResolvedValue('data:image/png;base64,cached-avatar')
    const wrapper = mountCard()
    await flushPromises()
    online.reject(new Error('Mojang unavailable'))
    await flushPromises()
    expect(wrapper.text()).toContain('SavedPlayer')
    expect(wrapper.find('img').attributes('src')).toBe('data:image/png;base64,cached-avatar')
    expect(backend.error).toHaveBeenCalledOnce()
})

it('does not let a late cached head overwrite the refreshed skin', async () => {
    const online = deferred<ReturnType<typeof account>[]>()
    const oldHead = deferred<string>()
    const newHead = deferred<string>()
    backend.users.mockImplementation((_: boolean, cachedOnly: boolean) =>
        cachedOnly ? Promise.resolve([account('SavedPlayer')]) : online.promise,
    )
    backend.heads.mockImplementation((skin: { texture_key: string }) =>
        skin.texture_key === 'saved-skin' ? oldHead.promise : newHead.promise,
    )
    const wrapper = mountCard()
    await flushPromises()
    const updated = account('UpdatedPlayer')
    updated.profile.skins[0].textureKey = 'updated-skin'
    online.resolve([updated])
    await flushPromises()
    newHead.resolve('data:image/png;base64,new-avatar')
    await flushPromises()
    oldHead.resolve('data:image/png;base64,old-avatar')
    await flushPromises()
    expect(wrapper.text()).toContain('UpdatedPlayer')
    expect(wrapper.find('img').attributes('src')).toBe('data:image/png;base64,new-avatar')
})

it('renders accounts in the order saved between starts', async () => {
    localStorage.setItem(
        ACCOUNT_ORDER_STORAGE_KEY,
        JSON.stringify(['microsoft:two', 'microsoft:one']),
    )
    backend.defaultUser.mockResolvedValue('microsoft:one')
    backend.users.mockResolvedValue([
        account('Alpha', 'microsoft:one', 'one'),
        account('Beta', 'microsoft:two', 'two'),
    ])
    backend.heads.mockResolvedValue('data:image/png;base64,avatar')
    const wrapper = mountCard()
    await flushPromises()

    const names = wrapper
        .findAll('p')
        .map((paragraph) => paragraph.text())
        .filter((text) => text === 'Alpha' || text === 'Beta')
    expect(names).toEqual(['Beta', 'Alpha'])
})

it('moves an account with the keyboard handle', async () => {
    backend.defaultUser.mockResolvedValue('microsoft:one')
    backend.users.mockResolvedValue([
        account('Alpha', 'microsoft:one', 'one'),
        account('Beta', 'microsoft:two', 'two'),
    ])
    backend.heads.mockResolvedValue('data:image/png;base64,avatar')
    const wrapper = mountCard()
    await flushPromises()

    const row = wrapper.findAll('button').find((button) => button.text().includes('Alpha'))!
    await row.trigger('keydown', { key: 'ArrowDown', altKey: true })
    await flushPromises()

    expect(readAccountOrder()).toEqual(['microsoft:two', 'microsoft:one'])
    expect(set_default_user).not.toHaveBeenCalled()
})

it('persists the order a drag leaves behind without selecting an account', async () => {
    backend.defaultUser.mockResolvedValue('microsoft:one')
    backend.users.mockResolvedValue([
        account('Alpha', 'microsoft:one', 'one'),
        account('Beta', 'microsoft:two', 'two'),
    ])
    backend.heads.mockResolvedValue('data:image/png;base64,avatar')
    const wrapper = mountCard()
    await flushPromises()

    const draggable = wrapper.findComponent(Draggable)
    const list = draggable.props('list') as Array<{ account_id: string }>
    expect(list.map((entry) => entry.account_id)).toEqual(['microsoft:one', 'microsoft:two'])

    draggable.vm.$emit('start')
    const [moved] = list.splice(0, 1)
    list.splice(1, 0, moved)
    draggable.vm.$emit('end')
    await flushPromises()

    expect(readAccountOrder()).toEqual(['microsoft:two', 'microsoft:one'])
    expect(set_default_user).not.toHaveBeenCalled()
})

it('does not pin the default order when a drag lands where it started', async () => {
    backend.defaultUser.mockResolvedValue('microsoft:one')
    backend.users.mockResolvedValue([
        account('Alpha', 'microsoft:one', 'one'),
        account('Beta', 'microsoft:two', 'two'),
    ])
    backend.heads.mockResolvedValue('data:image/png;base64,avatar')
    const wrapper = mountCard()
    await flushPromises()

    const draggable = wrapper.findComponent(Draggable)
    draggable.vm.$emit('start')
    draggable.vm.$emit('end')
    await flushPromises()

    expect(readAccountOrder()).toEqual([])
})

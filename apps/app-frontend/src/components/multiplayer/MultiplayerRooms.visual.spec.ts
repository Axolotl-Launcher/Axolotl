import { flushPromises, mount, type VueWrapper } from '@vue/test-utils'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { ref } from 'vue'

import type { HongshiState, MultiplayerState } from '@/helpers/multiplayer'

import MultiplayerRooms from './MultiplayerRooms.vue'
import ServerDetail from './servers/ServerDetail.vue'

const fixture = vi.hoisted(() => ({
    state: null as MultiplayerState | null,
    query: {} as Record<string, string>,
    ports: [] as unknown[],
    servers: null as unknown,
    host: vi.fn(),
    stop: vi.fn(),
    switchProvider: vi.fn(),
    notify: vi.fn(),
    push: vi.fn(),
}))

vi.mock('@/helpers/multiplayer', async (original) => ({
    ...(await original<typeof import('@/helpers/multiplayer')>()),
    multiplayer: {
        getState: async () => structuredClone(fixture.state),
        getNodes: async () => [
            {
                name: 'New region',
                address: 'relay.example.com',
                latency_ms: 1,
                reachable: true,
                cached: false,
            },
        ],
        getDetectedPorts: async () => structuredClone(fixture.ports),
        getPlayerName: async () => 'Player',
        hostHongshi: fixture.host,
        stop: fixture.stop,
        switchProvider: fixture.switchProvider,
    },
}))
vi.mock('@/helpers/terracotta', () => ({
    isValidTerracottaRoomCode: () => true,
    terracotta: { getPlatformKey: async () => 'macos_arm64', checkForUpdate: async () => null },
}))
vi.mock('@/helpers/utils', () => ({ exportErrorLogs: vi.fn(), openPath: vi.fn() }))
vi.mock('vue-router', () => ({
    useRoute: () => ({ params: { id: 'test-server' }, query: fixture.query }),
    useRouter: () => ({ push: fixture.push, replace: vi.fn() }),
}))
vi.mock('@/composables/useServers', () => ({
    useServers: () => ({ servers: fixture.servers, refresh: vi.fn(), stopServer: vi.fn() }),
}))
vi.mock('@/composables/useServerInstalls', () => ({ serverSetupStatus: () => null }))
vi.mock('@/composables/useServerLifecycle', async () => {
    const { ref } = await import('vue')
    return { useServerLifecycle: () => ({ eulaModal: ref(null) }) }
})
vi.mock('@/helpers/servers', () => ({ servers: {} }))
vi.mock('./servers/EulaModal.vue', () => ({ default: { render: () => null } }))
vi.mock('./servers/ServerConsole.vue', () => ({ default: { render: () => null } }))
vi.mock('./servers/ServerFilesPanel.vue', () => ({ default: { render: () => null } }))
vi.mock('./servers/ServerSettingsPanel.vue', async () => {
    const { defineComponent, ref, h } = await import('vue')
    return {
        default: defineComponent({
            setup: () => {
                const name = ref('Saved name')
                return () =>
                    h('input', {
                        'data-server-draft': '',
                        value: name.value,
                        onInput: (event: Event) => {
                            name.value = (event.target as HTMLInputElement).value
                        },
                    })
            },
        }),
    }
})
vi.mock('./servers/ServerIcon.vue', () => ({ default: { render: () => null } }))

vi.mock('@modrinth/ui', async () => {
    const { defineComponent, h } = await import('vue')
    const { default: ProgressBar } =
        await import('../../../../../packages/ui/src/components/base/ProgressBar.vue')
    const container = defineComponent({
        setup:
            (_, { slots }) =>
            () =>
                h('div', slots.default?.()),
    })
    return {
        ...Object.fromEntries(
            [
                'Admonition',
                'Card',
                'CopyCode',
                'NavTabs',
                'PopoutMenu',
                'OverflowMenu',
                'TagItem',
            ].map((name) => [name, container]),
        ),
        NavTabs: defineComponent({
            props: ['links'],
            setup:
                (props, { emit }) =>
                () =>
                    h(
                        'nav',
                        props.links.map((link: { label: string }, index: number) =>
                            h('button', { onClick: () => emit('tab-click', index) }, link.label),
                        ),
                    ),
        }),
        Button: defineComponent({
            props: ['disabled'],
            setup:
                (props, { slots }) =>
                () =>
                    h('button', { disabled: props.disabled }, slots.default?.()),
        }),
        StyledInput: defineComponent({
            props: ['modelValue'],
            emits: ['update:modelValue'],
            setup:
                (props, { emit }) =>
                () =>
                    h('input', {
                        value: props.modelValue,
                        onInput: (event: Event) =>
                            emit('update:modelValue', (event.target as HTMLInputElement).value),
                    }),
        }),
        Combobox: defineComponent({
            props: ['modelValue', 'options'],
            emits: ['update:modelValue'],
            setup:
                (props, { emit }) =>
                () =>
                    h(
                        'select',
                        {
                            value: props.modelValue,
                            onChange: (event: Event) =>
                                emit(
                                    'update:modelValue',
                                    (event.target as HTMLSelectElement).value,
                                ),
                        },
                        props.options.map((option: { value: string; label: string }) =>
                            h('option', { value: option.value }, option.label),
                        ),
                    ),
        }),
        ProgressBar,
        defineMessages: (messages: unknown) => messages,
        injectNotificationManager: () => ({
            handleError: fixture.notify,
            addNotification: fixture.notify,
        }),
        injectFilePicker: () => ({}),
        useVIntl: () => ({
            formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
        }),
    }
})

let wrappers: VueWrapper[] = []
const idle = (): HongshiState => ({
    supported: true,
    status: 'idle',
    local_port: null,
    node: null,
    public_address: null,
    created_at: null,
    last_exit_code: null,
    error_type: null,
    error_message: null,
    bound_instance_id: null,
    port_changed: false,
    binary_installed: false,
    download_progress: null,
})
const button = (wrapper: VueWrapper, text: string) =>
    wrapper.findAll('button').find((item) => item.text().includes(text))!
async function rooms() {
    const wrapper = mount(MultiplayerRooms, { global: { directives: { tooltip: () => {} } } })
    wrappers.push(wrapper)
    await flushPromises()
    return wrapper
}
beforeEach(() => {
    vi.clearAllMocks()
    localStorage.clear()
    localStorage.setItem('axolotl-multiplayer-provider', 'hongshi')
    fixture.query = {}
    fixture.ports = []
    fixture.state = {
        active_provider: null,
        providers: [],
        terracotta: { status: 'idle' },
        hongshi: idle(),
    } as MultiplayerState
    fixture.servers = ref([
        {
            id: 'test-server',
            name: 'Test server',
            status: 'running',
            port: 25577,
            gameVersion: '1.21.1',
        },
    ])
    fixture.push.mockResolvedValue(undefined)
    fixture.switchProvider.mockImplementation(async () => {
        fixture.state!.active_provider = null
    })
})
afterEach(() => {
    wrappers.forEach((wrapper) => wrapper.unmount())
    wrappers = []
})

it('first room automatically installs and can be cancelled while its request is pending', async () => {
    let reject!: (error: unknown) => void
    fixture.host.mockImplementation(() => {
        fixture.state!.active_provider = 'hongshi'
        fixture.state!.hongshi = { ...idle(), status: 'downloading', download_progress: 42 }
        return new Promise((_, fail) => {
            reject = fail
        })
    })
    fixture.stop.mockImplementation(async () => {
        fixture.state!.active_provider = null
        fixture.state!.hongshi = idle()
        reject({ message: 'RedStone operation cancelled' })
    })
    const wrapper = await rooms()
    expect(button(wrapper, 'Create public room').attributes('disabled')).toBeUndefined()
    await button(wrapper, 'Create public room').trigger('click')
    await vi.waitFor(() =>
        expect(wrapper.get('[role=progressbar]').attributes('aria-valuenow')).toBe('42'),
    )
    expect(button(wrapper, 'Cancel').attributes('disabled')).toBeUndefined()
    await button(wrapper, 'Cancel').trigger('click')
    await vi.waitFor(() => expect(button(wrapper, 'Create public room')).toBeTruthy())
    expect(fixture.host).toHaveBeenCalledExactlyOnceWith(25565, null, null)
    expect(fixture.stop).toHaveBeenCalledTimes(1)
    expect(fixture.notify).not.toHaveBeenCalled()
})

it('an unpublished macOS build keeps port and node inputs and succeeds on retry', async () => {
    fixture.host
        .mockImplementationOnce(async () => {
            fixture.state!.hongshi = { ...idle(), status: 'error', error_type: 'build_unavailable' }
            throw { message: 'not uploaded yet (hongshic-macos-arm64)' }
        })
        .mockImplementationOnce(async () => {
            fixture.state!.active_provider = 'hongshi'
            fixture.state!.hongshi = {
                ...idle(),
                binary_installed: true,
                status: 'open',
                public_address: 'relay.example.com:34575',
            }
        })
    const wrapper = await rooms()
    await wrapper.get('#hongshi-local-port').setValue('25578')
    await wrapper.findAll('select')[1].setValue('New region')
    await button(wrapper, 'Create public room').trigger('click')
    await vi.waitFor(() => expect(wrapper.text()).toContain('not been published'))
    expect((wrapper.get('#hongshi-local-port').element as HTMLInputElement).value).toBe('25578')
    expect((wrapper.findAll('select')[1].element as HTMLSelectElement).value).toBe('New region')
    await button(wrapper, 'Create public room').trigger('click')
    await vi.waitFor(() => expect(wrapper.text()).toContain('Public address'))
    expect(fixture.host).toHaveBeenNthCalledWith(2, 25578, 'New region', null)
})

it('an explicit manual port survives refreshed LAN detection, failure and retry', async () => {
    fixture.ports = [{ instance_id: 'game', instance_name: 'Detected game', port: 54321 }]
    fixture.host.mockImplementation(async () => {
        fixture.state!.hongshi = { ...idle(), status: 'error', error_type: 'build_unavailable' }
        throw { message: 'not uploaded yet' }
    })
    const wrapper = await rooms()
    expect((wrapper.findAll('select')[0].element as HTMLSelectElement).value).toBe('game')
    await wrapper.findAll('select')[0].setValue('manual')
    await wrapper.get('#hongshi-local-port').setValue('25582')
    await button(wrapper, 'Create public room').trigger('click')
    await vi.waitFor(() => expect(wrapper.text()).toContain('not been published'))
    expect((wrapper.get('#hongshi-local-port').element as HTMLInputElement).value).toBe('25582')
    expect(fixture.host).toHaveBeenCalledExactlyOnceWith(25582, null, null)
    await button(wrapper, 'Create public room').trigger('click')
    await vi.waitFor(() => expect(fixture.host).toHaveBeenCalledTimes(2))
    await flushPromises()
    expect((wrapper.get('#hongshi-local-port').element as HTMLInputElement).value).toBe('25582')
    expect(fixture.host).toHaveBeenNthCalledWith(2, 25582, null, null)
})

it('server sharing starts before navigation and preserves its port when the detail page unmounts', async () => {
    let resolve!: () => void
    fixture.state!.active_provider = 'terracotta'
    const confirm = vi.spyOn(window, 'confirm').mockReturnValue(true)
    fixture.host.mockImplementation(() => {
        fixture.state!.active_provider = 'hongshi'
        fixture.state!.hongshi = { ...idle(), status: 'downloading', download_progress: 15 }
        return new Promise<void>((done) => {
            resolve = done
        })
    })
    const detail = mount(ServerDetail, { global: { directives: { tooltip: () => {} } } })
    await flushPromises()
    await button(detail, 'Share online').trigger('click')
    await flushPromises()
    expect(confirm).toHaveBeenCalledTimes(1)
    expect(fixture.switchProvider).toHaveBeenCalledWith('hongshi')
    expect(fixture.host).toHaveBeenCalledExactlyOnceWith(25577, null, null)
    expect(fixture.push).toHaveBeenCalledWith({
        path: '/multiplayer/rooms',
        query: { provider: 'hongshi', port: '25577' },
    })
    detail.unmount()
    fixture.query = { provider: 'hongshi', port: '25577' }
    fixture.ports = [{ instance_id: 'another-game', instance_name: 'Other game', port: 54321 }]
    const wrapper = await rooms()
    expect(button(wrapper, 'Cancel')).toBeTruthy()
    fixture.state!.active_provider = null
    fixture.state!.hongshi = { ...idle(), status: 'error', error_type: 'install' }
    resolve()
    await vi.waitFor(() => expect(wrapper.find('#hongshi-local-port').exists()).toBe(true))
    expect((wrapper.get('#hongshi-local-port').element as HTMLInputElement).value).toBe('25577')
    confirm.mockRestore()
})

it('preserves a settings draft when visiting console and files, including a direct settings entry', async () => {
    fixture.query = { tab: 'settings' }
    const wrapper = mount(ServerDetail, {
        attachTo: document.body,
        global: { directives: { tooltip: () => {} } },
    })
    wrappers.push(wrapper)
    await flushPromises()
    await wrapper.get('[data-server-draft]').setValue('Unsaved name')
    await wrapper.get('nav').findAll('button')[0].trigger('click')
    expect(wrapper.get('[data-server-draft]').isVisible()).toBe(false)
    await wrapper.get('nav').findAll('button')[1].trigger('click')
    await wrapper.get('nav').findAll('button')[2].trigger('click')
    expect(wrapper.get<HTMLInputElement>('[data-server-draft]').element.value).toBe('Unsaved name')
    expect(wrapper.get('[data-server-draft]').isVisible()).toBe(true)
})

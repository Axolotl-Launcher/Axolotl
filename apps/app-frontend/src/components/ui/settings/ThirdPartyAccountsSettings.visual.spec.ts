import { QueryClient, VueQueryPlugin } from '@tanstack/vue-query'
import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { defineComponent, h, ref } from 'vue'

import {
    beginThirdPartyAuthorization,
    cancelThirdPartyAuthorization,
    checkThirdPartyAccount,
    disconnectThirdPartyAccount,
    getThirdPartyAccount,
    type ThirdPartyAccount,
    thirdPartyAccountKey,
} from '@/helpers/third-party-accounts'

import ThirdPartyAccountsSettings from './ThirdPartyAccountsSettings.vue'

const browser = vi.hoisted(() => ({ openUrl: vi.fn() }))
const notifications = vi.hoisted(() => ({ addNotification: vi.fn() }))
vi.mock('@tauri-apps/plugin-opener', () => browser)
vi.mock('@/helpers/third-party-accounts', async (importOriginal) => ({
    ...(await importOriginal<typeof import('@/helpers/third-party-accounts')>()),
    getThirdPartyAccount: vi.fn(),
    beginThirdPartyAuthorization: vi.fn(),
    cancelThirdPartyAuthorization: vi.fn(),
    disconnectThirdPartyAccount: vi.fn(),
    checkThirdPartyAccount: vi.fn(),
}))
vi.mock('@modrinth/ui', () => ({
    injectNotificationManager: () => notifications,
    defineMessages: (messages: unknown) => messages,
    useVIntl: () => ({
        formatMessage: (message: { defaultMessage: string }, values: Record<string, string> = {}) =>
            message.defaultMessage.replace(/\{(\w+)\}/g, (_, key) => values[key] ?? `{${key}}`),
    }),
    Card: defineComponent({
        setup:
            (_, { slots }) =>
            () =>
                h('div', slots.default?.()),
    }),
    Button: defineComponent({
        setup:
            (_, { attrs, slots }) =>
            () =>
                h('button', attrs, slots.default?.()),
    }),
    TeleportOverflowMenu: defineComponent({
        props: ['options', 'disabled'],
        setup: (props) => () =>
            h(
                'div',
                props.options
                    .filter(
                        (option: { id?: string; shown?: boolean }) =>
                            option.id && option.shown !== false,
                    )
                    .map((option: { id: string; label: string; action: () => void }) =>
                        h(
                            'button',
                            { disabled: props.disabled, onClick: option.action },
                            option.label,
                        ),
                    ),
            ),
    }),
    ConfirmActionModal: defineComponent({
        props: ['confirmAction', 'confirmDisabled', 'confirmLabel'],
        setup(props, { slots, expose }) {
            const visible = ref(false)
            expose({
                show: () => {
                    visible.value = true
                },
            })
            return () =>
                visible.value
                    ? h('section', { 'data-confirmation': '' }, [
                          slots.default?.(),
                          h(
                              'button',
                              {
                                  disabled: props.confirmDisabled,
                                  onClick: () => (visible.value = false),
                              },
                              'Cancel',
                          ),
                          h(
                              'button',
                              {
                                  disabled: props.confirmDisabled,
                                  onClick: async () => {
                                      try {
                                          await props.confirmAction()
                                          visible.value = false
                                      } catch {
                                          visible.value = true
                                      }
                                  },
                              },
                              props.confirmLabel,
                          ),
                      ])
                    : null
        },
    }),
}))

function disconnected(): ThirdPartyAccount {
    return {
        providerId: 'shimu',
        available: true,
        status: 'disconnected',
        user: null,
        expiresAt: null,
        flowId: null,
        authorization: null,
    }
}

function connected(): ThirdPartyAccount {
    return {
        ...disconnected(),
        status: 'connected',
        user: { userId: 1, userName: 'shimu-user', nickName: '拾木' },
        expiresAt: 259300,
    }
}

const wrappers: ReturnType<typeof mount>[] = []
const clients: QueryClient[] = []
beforeEach(() => {
    vi.resetAllMocks()
    vi.mocked(getThirdPartyAccount).mockResolvedValue(disconnected())
    vi.mocked(beginThirdPartyAuthorization).mockResolvedValue({
        flowId: 'test-state',
        authorizationUrl: 'https://www.mcimg.com/oauth/authorize?state=test-state',
    })
    vi.mocked(cancelThirdPartyAuthorization).mockResolvedValue()
    vi.mocked(disconnectThirdPartyAccount).mockResolvedValue()
    browser.openUrl.mockResolvedValue(undefined)
})
afterEach(() => {
    wrappers.splice(0).forEach((wrapper) => wrapper.unmount())
    clients.splice(0).forEach((client) => client.clear())
})

async function render() {
    const client = new QueryClient({
        defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
    })
    clients.push(client)
    const wrapper = mount(ThirdPartyAccountsSettings, {
        global: { plugins: [[VueQueryPlugin, { queryClient: client }]] },
    })
    wrappers.push(wrapper)
    await vi.waitFor(() => expect(wrapper.find('[role="status"]').exists()).toBe(true))
    return { wrapper, client }
}

function button(wrapper: ReturnType<typeof mount>, label: string) {
    return wrapper.findAll('button').find((button) => button.text() === label)!
}

it.each([
    ['HECODE', 'HECODE'],
    [' HECODE ', 'HECODE'],
    [null, 'HECODE'],
    ['拾木', '拾木 · HECODE'],
])(
    'shows the account identity without repeating a matching nickname (%s)',
    async (nickName, expected) => {
        vi.mocked(getThirdPartyAccount).mockResolvedValue({
            ...connected(),
            user: { userId: 1, userName: 'HECODE', nickName },
        })
        const { wrapper } = await render()
        expect(wrapper.get('#settings-target-third-party-shimu p').text()).toBe(expected)
    },
)

it('notifies once after a successful authorization, including when the callback remounts the page', async () => {
    const completed: ThirdPartyAccount = {
        ...connected(),
        authorization: 'succeeded',
        flowId: 'completed-authorization',
    }
    vi.mocked(getThirdPartyAccount).mockResolvedValue(completed)
    const { wrapper, client } = await render()
    expect(notifications.addNotification).toHaveBeenCalledTimes(1)
    expect(notifications.addNotification).toHaveBeenCalledWith({
        title: 'Connected to Shimu Space',
        text: '拾木 · shimu-user',
        type: 'success',
    })
    client.setQueryData(thirdPartyAccountKey, { ...completed })
    await flushPromises()
    wrapper.unmount()
    await render()
    expect(notifications.addNotification).toHaveBeenCalledTimes(1)
})

it.each(['waiting', 'failed', 'denied', null] as const)(
    'does not report success for an existing connection during %s',
    async (authorization) => {
        vi.mocked(getThirdPartyAccount).mockResolvedValue({
            ...connected(),
            authorization,
            flowId: 'unsuccessful-authorization',
        })
        await render()
        expect(notifications.addNotification).not.toHaveBeenCalled()
    },
)

it('keeps one service row in place as authorization changes to connected', async () => {
    const { wrapper, client } = await render()
    const row = wrapper.get('#settings-target-third-party-shimu').element
    vi.mocked(getThirdPartyAccount).mockResolvedValue({
        ...disconnected(),
        authorization: 'waiting',
        flowId: 'test-state',
    })
    await button(wrapper, 'Connect').trigger('click')
    await vi.waitFor(() =>
        expect(wrapper.get('[role="status"]').text()).toBe('Waiting for authorization'),
    )
    expect(beginThirdPartyAuthorization).toHaveBeenCalledTimes(1)
    expect(browser.openUrl).toHaveBeenCalledWith(
        'https://www.mcimg.com/oauth/authorize?state=test-state',
    )
    client.setQueryData(thirdPartyAccountKey, connected())
    await flushPromises()
    expect(wrapper.get('[role="status"]').text()).toBe('Connected')
    expect(wrapper.text()).toContain('shimu-user')
    expect(wrapper.get('#settings-target-third-party-shimu').element).toBe(row)
})

it('cancels a pending flow when the browser cannot be opened', async () => {
    const { wrapper } = await render()
    browser.openUrl.mockRejectedValueOnce(new Error('opener unavailable'))
    await button(wrapper, 'Connect').trigger('click')
    await vi.waitFor(() => expect(cancelThirdPartyAuthorization).toHaveBeenCalledWith('test-state'))
    await vi.waitFor(() => expect(wrapper.text()).toContain('The account operation failed'))
    expect(wrapper.get('[role="status"]').text()).toBe('Not connected')
})

it('preserves identity on expiry and on a failed connectivity check', async () => {
    vi.mocked(getThirdPartyAccount).mockResolvedValue({ ...connected(), status: 'expired' })
    const { wrapper, client } = await render()
    expect(wrapper.get('[role="status"]').text()).toBe('Sign in again')
    expect(wrapper.text()).toContain('shimu-user')
    expect(button(wrapper, 'Reconnect')).toBeDefined()
    vi.mocked(getThirdPartyAccount).mockResolvedValue(connected())
    client.setQueryData(thirdPartyAccountKey, connected())
    await flushPromises()
    vi.mocked(checkThirdPartyAccount).mockRejectedValueOnce(new Error('offline'))
    await button(wrapper, 'Check connection').trigger('click')
    await vi.waitFor(() => expect(wrapper.text()).toContain('Could not check the connection'))
    expect(wrapper.get('[role="status"]').text()).toBe('Connected')
    expect(wrapper.text()).toContain('shimu-user')
})

it('requires confirmation and closes it only after disconnect succeeds', async () => {
    vi.mocked(getThirdPartyAccount).mockResolvedValue(connected())
    const { wrapper } = await render()
    await button(wrapper, 'Disconnect').trigger('click')
    expect(disconnectThirdPartyAccount).not.toHaveBeenCalled()
    expect(wrapper.find('[data-confirmation]').exists()).toBe(true)
    let finish!: () => void
    vi.mocked(disconnectThirdPartyAccount).mockImplementationOnce(
        () =>
            new Promise<void>((resolve) => {
                finish = resolve
            }),
    )
    await wrapper.get('[data-confirmation]').findAll('button')[1].trigger('click')
    await vi.waitFor(() => expect(disconnectThirdPartyAccount).toHaveBeenCalledTimes(1))
    expect(wrapper.find('[data-confirmation]').exists()).toBe(true)
    vi.mocked(getThirdPartyAccount).mockResolvedValue(disconnected())
    finish()
    await vi.waitFor(() => expect(wrapper.find('[data-confirmation]').exists()).toBe(false))
    expect(wrapper.get('[role="status"]').text()).toBe('Not connected')
})

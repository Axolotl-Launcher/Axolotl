import '@/test/visual-styles.scss'

import { flushPromises, mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'

import HomePinnedServers from './HomePinnedServers.vue'

const backend = vi.hoisted(() => ({
    unpin: vi.fn(),
    link: vi.fn(),
    favoriteWorlds: null as unknown,
    localServers: null as unknown,
}))
vi.mock('@/helpers/worlds', () => ({
    getWorldIdentifier: () => 'world',
    set_world_display_status: backend.unpin,
    start_join_server: vi.fn(),
}))
vi.mock('@/helpers/instance', () => ({ kill: vi.fn() }))
vi.mock('@/helpers/servers', () => ({
    servers: {
        isLocalAddress: async () => true,
        setLinkedWorld: backend.link,
        updateSettings: backend.unpin,
    },
}))
vi.mock('@/helpers/analytics', () => ({ trackEvent: vi.fn() }))
vi.mock('@/store/error', () => ({ handleSevereError: vi.fn() }))
vi.mock('@/composables/useMinecraftLaunchError', () => ({ useMinecraftLaunchError: () => vi.fn() }))
vi.mock('@/composables/useOnlineModeWarning', async () => {
    const { ref } = await import('vue')
    return {
        useOnlineModeWarning: () => ({
            onlineModeWarningModal: ref(null),
            confirmOnlineModeLaunch: vi.fn(),
        }),
    }
})
vi.mock('@/composables/useServers', () => ({
    useServers: () => ({ startServer: vi.fn(), stopServer: vi.fn() }),
}))
vi.mock('@/components/multiplayer/servers/OnlineModeWarningModal.vue', () => ({
    default: { render: () => null },
}))
vi.mock('@/components/multiplayer/servers/ServerIcon.vue', () => ({
    default: { render: () => null },
}))
vi.mock('./HomeLocalServerPickerModal.vue', async () => {
    const { defineComponent, h, ref } = await import('vue')
    return {
        default: defineComponent({
            setup: (_, { expose, emit }) => {
                const visible = ref(false)
                expose({
                    show: () => {
                        visible.value = true
                    },
                })
                return () =>
                    visible.value
                        ? h(
                              'button',
                              {
                                  'data-link-server': '',
                                  onClick: () =>
                                      emit('select', { id: 'local-one', name: 'Local server' }),
                              },
                              'Choose local server',
                          )
                        : null
            },
        }),
    }
})
vi.mock('./home-dashboard-runtime', async () => {
    const { computed, ref } = await import('vue')
    return {
        useHomeDashboardRuntime: () => ({
            favoriteWorlds: backend.favoriteWorlds,
            localServers: backend.localServers,
            pinnedLocalServers: computed(() => []),
            runningInstanceIds: ref([]),
            getServerData: () => ({ refreshing: false, status: null }),
            refreshPinnedLocalServers: vi.fn(),
            refreshFavorites: vi.fn(),
        }),
    }
})
vi.mock('@modrinth/ui', async () => {
    const { defineComponent, h, ref } = await import('vue')
    return {
        Avatar: defineComponent({ setup: () => () => h('span') }),
        SmartClickable: (
            await import('../../../../../packages/ui/src/components/base/SmartClickable.vue')
        ).default,
        Button: (await import('../../../../../packages/ui/src/components/base/buttons/Button.vue'))
            .default,
        OverflowMenu: defineComponent({
            props: ['options'],
            setup: (props, { slots }) => {
                const open = ref(false)
                return () =>
                    h('div', [
                        h(
                            'button',
                            {
                                'data-more-trigger': '',
                                onClick: () => {
                                    open.value = !open.value
                                },
                            },
                            slots.default?.(),
                        ),
                        ...(open.value
                            ? props.options.map((option: { id: string; action: () => void }) =>
                                  h(
                                      'button',
                                      { 'data-menu-action': option.id, onClick: option.action },
                                      slots[option.id]?.(),
                                  ),
                              )
                            : []),
                    ])
            },
        }),
        defineMessages: (value: unknown) => value,
        injectNotificationManager: () => ({ handleError: vi.fn(), addNotification: vi.fn() }),
        useVIntl: () => ({
            formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
        }),
    }
})

it.each(['1x1', '2x1'] as const)(
    'keeps unpin and local association actions available at %s',
    async (dashboardSize) => {
        const { ref } = await import('vue')
        backend.unpin.mockReset().mockResolvedValue(undefined)
        backend.link.mockReset().mockResolvedValue(undefined)
        backend.favoriteWorlds = ref([
            {
                type: 'server',
                name: 'A long server name that must leave room for the menu',
                instance_id: 'instance-one',
                address: 'localhost:25565',
                index: 0,
            },
        ])
        backend.localServers = ref([])
        const wrapper = mount(HomePinnedServers, {
            attachTo: document.body,
            props: {
                instances: [{ id: 'instance-one', name: 'Instance' }] as never,
                dashboardSize,
            },
            global: { directives: { tooltip: {} } },
        })
        wrapper.element.setAttribute(
            'style',
            `width:${dashboardSize === '1x1' ? 250 : 520}px;background:var(--surface-2)`,
        )
        try {
            await flushPromises()
            const menu = wrapper.get('.home-server-menu')
            expect(getComputedStyle(menu.element).display).not.toBe('none')
            expect(menu.element.getBoundingClientRect().right).toBeLessThanOrEqual(
                wrapper.element.getBoundingClientRect().right,
            )
            await menu.get('[data-more-trigger]').trigger('click')
            await menu.get('[data-menu-action="link-local-server"]').trigger('click')
            await wrapper.get('[data-link-server]').trigger('click')
            expect(backend.link).toHaveBeenCalledWith('local-one', {
                instanceId: 'instance-one',
                address: 'localhost:25565',
            })
            ;(backend.localServers as ReturnType<typeof ref>).value = [
                {
                    id: 'local-one',
                    linkedWorld: { instanceId: 'instance-one', address: 'localhost:25565' },
                },
            ]
            await flushPromises()
            expect(menu.find('[data-menu-action="change-local-server"]').exists()).toBe(true)
            await menu.get('[data-menu-action="unlink-local-server"]').trigger('click')
            expect(backend.link).toHaveBeenCalledWith('local-one', null)
            await menu.get('[data-menu-action="unpin"]').trigger('click')
            expect(backend.unpin).toHaveBeenCalled()
        } finally {
            wrapper.unmount()
        }
    },
)

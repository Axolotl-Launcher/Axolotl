import { expect, it, vi } from 'vitest'

import type { GameInstance } from '@/helpers/types'
import { mountThemed } from '@/test/visual-harness'

import HomeRecentWorlds from './HomeRecentWorlds.vue'

vi.mock('@/components/home/home-dashboard-runtime', async () => {
    const { ref } = await import('vue')
    const lastPlayed = '2026-01-02T00:00:00.000Z'
    const world = (instanceId: string, name: string) => ({
        instance_id: instanceId,
        name,
        display_status: 'normal',
        type: 'singleplayer',
        path: `/worlds/${name}`,
        game_mode: 'survival',
        hardcore: false,
        locked: false,
        last_played: lastPlayed,
    })

    return {
        useHomeDashboardRuntime: () => ({
            gameVersions: ref([]),
            recentWorlds: ref([world('first', 'First'), world('second', 'Second')]),
            runningInstanceIds: ref([]),
            getServerData: () => undefined,
            getProtocolVersion: () => null,
            refreshServer: vi.fn(),
            refreshRecentWorlds: vi.fn(),
        }),
    }
})

vi.mock('@/components/ui/world/WorldItem.vue', async () => {
    const { defineComponent, h } = await import('vue')
    return {
        default: defineComponent({
            props: { world: { type: Object, required: true } },
            setup: (props) => () =>
                h(
                    'div',
                    { 'data-world-item': (props.world as { name: string }).name },
                    (props.world as { name: string }).name,
                ),
        }),
    }
})

vi.mock('@/components/ui/world/InstanceItem.vue', async () => {
    const { defineComponent, h } = await import('vue')
    return { default: defineComponent({ setup: () => () => h('div') }) }
})

vi.mock('@/helpers/worlds', () => ({
    getWorldIdentifier: (world: { name: string }) => world.name,
    hasServerQuickPlaySupport: () => false,
    hasWorldQuickPlaySupport: () => false,
    start_join_server: vi.fn(),
    start_join_singleplayer_world: vi.fn(),
}))
vi.mock('@/helpers/instance', () => ({ kill: vi.fn(), run: vi.fn() }))
vi.mock('@/helpers/analytics', () => ({ trackEvent: vi.fn() }))
vi.mock('@/composables/useMinecraftLaunchError', () => ({ useMinecraftLaunchError: () => vi.fn() }))
vi.mock('@/store/error', () => ({ handleSevereError: vi.fn() }))
vi.mock('@modrinth/ui', () => ({
    GAME_MODES: { survival: 'Survival' },
    defineMessages: (messages: unknown) => messages,
    injectNotificationManager: () => ({ handleError: vi.fn() }),
    useVIntl: () => ({
        formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
    }),
}))

const instances = [
    { id: 'first', name: 'First', last_played: '2026-01-02T00:00:00.000Z' },
    { id: 'second', name: 'Second', last_played: '2026-01-02T00:00:00.000Z' },
]

async function listStyle(size: '2x1' | '2x2' | '3x1') {
    const wrapper = await mountThemed(
        HomeRecentWorlds,
        {
            instances: instances as unknown as GameInstance[],
            dashboard: true,
            dashboardSize: size,
        },
        'dark',
    )

    const list = wrapper.find<HTMLElement>('.home-recent-list')
    const style = getComputedStyle(list.element)
    const computed = { display: style.display, columns: style.gridTemplateColumns }
    wrapper.unmount()
    return computed
}

it('shows two recent worlds per row in narrow widgets', async () => {
    for (const size of ['2x1', '2x2'] as const) {
        const style = await listStyle(size)
        expect(style.display, `${size} should use a grid`).toBe('grid')
        expect(
            style.columns.split(' ').filter(Boolean),
            `${size} should use two columns`,
        ).toHaveLength(2)
    }
})

it('keeps one recent world per row in wider widgets', async () => {
    const style = await listStyle('3x1')
    expect(style.display).toBe('flex')
})

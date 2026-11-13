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
            recentWorldsLoaded: ref(true),
            runningInstanceIds: ref([]),
            getServerData: () => undefined,
            getProtocolVersion: () => null,
            refreshServer: vi.fn(),
            refreshRecentWorlds: vi.fn(),
        }),
    }
})

vi.mock('@/components/ui/InstanceIcon.vue', () => ({ default: { render: () => null } }))
vi.mock('@/components/home/HomeRowMenu.vue', () => ({ default: { render: () => null } }))
vi.mock('@/components/home/home-row-menu', () => ({
    useHomeRowMenu: () => ({ instanceMenu: () => [], worldMenu: () => [] }),
}))

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
vi.mock('@modrinth/ui', async () => {
    const { defineComponent, h } = await import('vue')
    const passthrough = defineComponent({
        setup:
            (_, { slots }) =>
            () =>
                h('div', slots.default?.()),
    })
    return {
        Avatar: passthrough,
        ButtonLink: passthrough,
        OverflowMenu: passthrough,
        PopoutMenu: passthrough,
        GAME_MODES: { survival: 'Survival' },
        commonMessages: {
            playButton: { defaultMessage: 'Play' },
            stopButton: { defaultMessage: 'Stop' },
        },
        defineMessages: (messages: unknown) => messages,
        formatLoaderLabel: (loader: string) => loader,
        injectNotificationManager: () => ({ handleError: vi.fn() }),
        useRelativeTime: () => () => 'recently',
        useVIntl: () => ({
            formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
        }),
    }
})

const instances = [
    { id: 'first', name: 'First', last_played: '2026-01-02T00:00:00.000Z' },
    { id: 'second', name: 'Second', last_played: '2026-01-02T00:00:00.000Z' },
]

async function mountList(size: '2x1' | '2x2' | '3x1') {
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
    const result = {
        display: getComputedStyle(list.element).display,
        variant: list.attributes('data-variant'),
        text: list.text(),
    }
    wrapper.unmount()
    return result
}

// The list is a grid at every size, so worlds sit side by side whenever the
// widget is wide enough; the column count follows the measured width.
it('lays recent worlds out in a grid at every dashboard size', async () => {
    for (const size of ['2x1', '2x2', '3x1'] as const) {
        const list = await mountList(size)
        expect(list.display, `${size} should use a grid`).toBe('grid')
        expect(list.text, `${size} should list both worlds`).toContain('First')
        expect(list.text, `${size} should list both worlds`).toContain('Second')
    }
})

it('uses tiles in the tall widget and low cards in the one-row widgets', async () => {
    expect((await mountList('2x2')).variant).toBe('tile')
    expect((await mountList('2x1')).variant).toBe('card')
    expect((await mountList('3x1')).variant).toBe('card')
})

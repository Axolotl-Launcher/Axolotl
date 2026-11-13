import { mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { defineComponent, h, ref } from 'vue'

import type { GameInstance } from '@/helpers/types'

import HomePinnedInstances from './HomePinnedInstances.vue'

vi.mock('@/components/home/home-dashboard-runtime', () => ({
    useHomeDashboardRuntime: () => ({ runningInstanceIds: ref([]) }),
}))
vi.mock('@/components/ui/InstanceIcon.vue', () => ({ default: { render: () => null } }))
vi.mock('@/helpers/instance', () => ({ set_pinned: vi.fn(), run: vi.fn(), kill: vi.fn() }))
vi.mock('@/helpers/analytics', () => ({ trackEvent: vi.fn() }))
vi.mock('@/helpers/utils', () => ({
    copyToClipboard: vi.fn(),
    createInstanceShortcut: vi.fn(),
    showInstanceInFolder: vi.fn(),
}))
vi.mock('@/helpers/worlds', () => ({
    getWorldIdentifier: vi.fn(),
    set_world_display_status: vi.fn(),
    showWorldInFolder: vi.fn(),
}))
vi.mock('@/store/error', () => ({ handleSevereError: vi.fn() }))
vi.mock('@/composables/useMinecraftLaunchError', () => ({ useMinecraftLaunchError: () => vi.fn() }))
vi.mock('@modrinth/ui', () => {
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
        commonMessages: {
            playButton: { defaultMessage: 'Play' },
            stopButton: { defaultMessage: 'Stop' },
            openFolderButton: { defaultMessage: 'Open folder' },
            refreshButton: { defaultMessage: 'Refresh' },
        },
        defineMessages: (messages: unknown) => messages,
        formatLoaderLabel: (loader: string) => loader,
        injectNotificationManager: () => ({ handleError: vi.fn() }),
        useRelativeTime: () => () => 'now',
        useVIntl: () => ({
            formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
        }),
    }
})

const global = {
    directives: { tooltip: () => {} },
    stubs: {
        RouterLink: defineComponent({
            setup:
                (_, { slots }) =>
                () =>
                    h('a', slots.default?.()),
        }),
    },
}

function instance(id: string, gameDirOverride?: string): GameInstance {
    return {
        id,
        name: id,
        pinned_at: '2026-01-01T00:00:00.000Z',
        game_dir_override: gameDirOverride,
    } as unknown as GameInstance
}

it('uses the full pinned instance collection for the game-directory badge threshold', () => {
    const multiDirectory = mount(HomePinnedInstances, {
        props: {
            instances: [
                instance('managed'),
                instance('external', String.raw`D:\Minecraft\.minecraft`),
            ],
        },
        global,
    })
    const singleDirectory = mount(HomePinnedInstances, {
        props: {
            instances: [instance('managed-a'), instance('managed-b')],
        },
        global,
    })

    try {
        // Every row names its game directory once the pinned set spans two of them.
        expect(multiDirectory.findAll('.home-item-row')).toHaveLength(2)
        expect(multiDirectory.findAll('[data-game-directory-badge]')).toHaveLength(2)
        expect(singleDirectory.findAll('.home-item-row')).toHaveLength(2)
        expect(singleDirectory.find('[data-game-directory-badge]').exists()).toBe(false)
    } finally {
        multiDirectory.unmount()
        singleDirectory.unmount()
    }
})

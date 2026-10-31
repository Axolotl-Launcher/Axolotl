import { flushPromises, mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { defineComponent, h, ref, Suspense } from 'vue'

import type { GameInstance } from '@/helpers/types'

import Instance from './Instance.vue'

vi.mock('@/components/ui/InstanceIcon.vue', () => ({ default: { render: () => null } }))
vi.mock('@/composables/useMinecraftLaunchError', () => ({
    useMinecraftLaunchError: () => vi.fn(),
}))
vi.mock('@/composables/useNetworkStatus', () => ({
    useNetworkStatus: () => ({ offline: ref(false) }),
}))
vi.mock('@/helpers/analytics', () => ({ trackEvent: vi.fn() }))
vi.mock('@/helpers/events', () => ({ process_listener: vi.fn() }))
vi.mock('@/helpers/install', () => ({
    install_existing_instance: vi.fn(),
    install_pack_to_existing_instance: vi.fn(),
}))
vi.mock('@/helpers/instance', () => ({ kill: vi.fn(), run: vi.fn() }))
vi.mock('@/helpers/instance-icons', () => ({
    getDisplayInstanceIcon: () => ({ url: '', frameless: false }),
}))
vi.mock('@/helpers/process', () => ({ get_by_instance_id: vi.fn(async () => []) }))
vi.mock('@/helpers/utils.js', () => ({ showInstanceInFolder: vi.fn() }))
vi.mock('@/store/error.js', () => ({ handleSevereError: vi.fn() }))
vi.mock('vue-router', () => ({ useRouter: () => ({ push: vi.fn() }) }))
vi.mock('@modrinth/ui', async () => {
    const { defineComponent, h } = await import('vue')
    const control = defineComponent({
        setup:
            (_, { slots }) =>
            () =>
                h('div', slots.default?.()),
    })
    return {
        Avatar: control,
        Button: control,
        commonMessages: {
            playButton: { defaultMessage: 'Play' },
            stopButton: { defaultMessage: 'Stop' },
            repairButton: { defaultMessage: 'Repair' },
            installingLabel: { defaultMessage: 'Installing' },
        },
        defineMessages: (messages: unknown) => messages,
        injectNotificationManager: () => ({ handleError: vi.fn() }),
        useRelativeTime: () => () => 'now',
        useVIntl: () => ({
            formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
        }),
    }
})

const instance = {
    id: 'external',
    name: 'External',
    loader: 'fabric',
    game_version: '1.21',
    install_stage: 'installed',
    game_dir_override: String.raw`D:\Minecraft\.minecraft`,
} as GameInstance

function mountCard(showGameDirectory: boolean) {
    return mount(
        defineComponent({
            setup: () => () =>
                h(Suspense, null, {
                    default: () =>
                        h(Instance, {
                            instance,
                            compact: true,
                            playing: false,
                            showGameDirectory,
                        }),
                }),
        }),
        { global: { directives: { tooltip: () => {} } } },
    )
}

it('shows the game-directory badge only when the shared card opts in', async () => {
    const hidden = mountCard(false)
    const shown = mountCard(true)

    try {
        await flushPromises()
        expect(hidden.find('[data-game-directory-badge]').exists()).toBe(false)
        expect(shown.get('[data-game-directory-badge]').text()).toContain('Minecraft')
    } finally {
        hidden.unmount()
        shown.unmount()
    }
})

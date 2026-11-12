import { mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'

import WorldItem from './WorldItem.vue'

vi.mock('vue-router', () => ({ useRouter: () => ({ push: vi.fn() }) }))
vi.mock('@/helpers/utils', () => ({ copyToClipboard: vi.fn(), createInstanceShortcut: vi.fn() }))
vi.mock('@/helpers/worlds.ts', () => ({
    getWorldIdentifier: () => 'World',
    set_world_display_status: vi.fn(),
}))
vi.mock('@/components/ui/InstanceIcon.vue', () => ({ default: { render: () => null } }))
vi.mock('@modrinth/ui', async () => {
    const { defineComponent, h } = await import('vue')
    const empty = defineComponent({ render: () => h('span') })
    return {
        Avatar: empty,
        TagItem: empty,
        OverflowMenu: empty,
        SmartClickable: (
            await import('../../../../../../packages/ui/src/components/base/SmartClickable.vue')
        ).default,
        Button: (
            await import('../../../../../../packages/ui/src/components/base/buttons/Button.vue')
        ).default,
        commonMessages: {
            playButton: { defaultMessage: 'Play' },
            stopButton: { defaultMessage: 'Stop' },
            singleplayerLabel: { defaultMessage: 'Singleplayer' },
        },
        defineMessages: (messages: unknown) => messages,
        injectNotificationManager: () => ({ handleError: vi.fn(), addNotification: vi.fn() }),
        useFormatDateTime: () => () => '',
        useFormatNumber: () => String,
        useRelativeTime: () => () => '',
        useVIntl: () => ({
            formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
        }),
    }
})

it('distinguishes a locked world from a running instance and enables play after unlocking', async () => {
    const world = {
        type: 'singleplayer' as const,
        name: 'World',
        path: 'World',
        game_mode: 'survival' as const,
        hardcore: false,
        display_status: 'normal' as const,
        locked: true,
    }
    const wrapper = mount(WorldItem, {
        props: { world, supportsWorldQuickPlay: true, dashboardDensity: 'compact' },
        global: {
            stubs: { RouterLink: true },
            directives: {
                tooltip: (element, binding) => {
                    if (binding.value) element.setAttribute('title', binding.value)
                    else element.removeAttribute('title')
                },
            },
        },
    })
    try {
        const play = wrapper.get('button[aria-label="Play"]')
        expect(play.attributes('title')).toBe('World is in use')
        expect((play.element as HTMLButtonElement).disabled).toBe(true)
        await play.trigger('click')
        expect(wrapper.emitted('play')).toBeUndefined()

        await wrapper.setProps({ world: { ...world, locked: false }, playingInstance: true })
        expect(play.attributes('title')).toBe('Instance is already open')
        expect((play.element as HTMLButtonElement).disabled).toBe(true)

        await wrapper.setProps({ playingInstance: false })
        expect(play.attributes('title')).toBeUndefined()
        expect((play.element as HTMLButtonElement).disabled).toBe(false)
        await play.trigger('click')
        expect(wrapper.emitted('play')).toHaveLength(1)
    } finally {
        wrapper.unmount()
    }
})

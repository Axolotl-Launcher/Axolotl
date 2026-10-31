import { mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { defineComponent, h, ref } from 'vue'

import type { GameInstance } from '@/helpers/types'

import HomePinnedInstances from './HomePinnedInstances.vue'

vi.mock('@/components/home/home-dashboard-runtime', () => ({
    useHomeDashboardRuntime: () => ({ runningInstanceIds: ref([]) }),
}))
vi.mock('@/components/home/HomeInstanceCard.vue', () => ({
    default: defineComponent({
        props: ['instance', 'showGameDirectory'],
        setup: (props) => () =>
            h('div', {
                'data-home-instance-card': props.instance.id,
                'data-show-game-directory': String(props.showGameDirectory),
            }),
    }),
}))
vi.mock('@/helpers/instance', () => ({ set_pinned: vi.fn() }))
vi.mock('@modrinth/ui', () => ({
    ButtonLink: defineComponent({
        props: ['as', 'to', 'type', 'size'],
        setup:
            (_, { slots }) =>
            () =>
                h('div', slots.default?.()),
    }),
    defineMessages: (messages: unknown) => messages,
    injectNotificationManager: () => ({ handleError: vi.fn() }),
    useVIntl: () => ({
        formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
    }),
}))

function instance(id: string, gameDirOverride?: string): GameInstance {
    return {
        id,
        name: id,
        pinned_at: '2026-01-01T00:00:00.000Z',
        game_dir_override: gameDirOverride,
    } as GameInstance
}

it('uses the full pinned instance collection for the game-directory badge threshold', () => {
    const multiDirectory = mount(HomePinnedInstances, {
        props: {
            instances: [
                instance('managed'),
                instance('external', String.raw`D:\Minecraft\.minecraft`),
            ],
        },
    })
    const singleDirectory = mount(HomePinnedInstances, {
        props: {
            instances: [instance('managed-a'), instance('managed-b')],
        },
    })

    try {
        expect(
            multiDirectory
                .findAll('[data-home-instance-card]')
                .every((card) => card.attributes('data-show-game-directory') === 'true'),
        ).toBe(true)
        expect(
            singleDirectory
                .findAll('[data-home-instance-card]')
                .every((card) => card.attributes('data-show-game-directory') === 'false'),
        ).toBe(true)
    } finally {
        multiDirectory.unmount()
        singleDirectory.unmount()
    }
})

import { flushPromises, mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { defineComponent, h, ref, Suspense } from 'vue'

import type { GameInstance } from '@/helpers/types'

import Instance from './Instance.vue'

const backend = vi.hoisted(() => ({
    push: vi.fn(),
    run: vi.fn(),
    kill: vi.fn(),
}))

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
vi.mock('@/helpers/instance', () => ({ kill: backend.kill, run: backend.run }))
vi.mock('@/helpers/instance-icons', () => ({
    getDisplayInstanceIcon: () => ({ url: '', frameless: false }),
}))
vi.mock('@/helpers/process', () => ({ get_by_instance_id: vi.fn(async () => []) }))
vi.mock('@/helpers/utils.js', () => ({ showInstanceInFolder: vi.fn() }))
vi.mock('@/store/error.js', () => ({ handleSevereError: vi.fn() }))
vi.mock('vue-router', () => ({ useRouter: () => ({ push: backend.push }) }))
vi.mock('@modrinth/ui', async () => {
    const { defineComponent, h } = await import('vue')
    const control = defineComponent({
        props: ['modelValue', 'ariaLabel'],
        setup:
            (props, { slots, attrs }) =>
            () =>
                h(
                    'div',
                    {
                        ...attrs,
                        'data-model-value': String(props.modelValue),
                        'aria-label': props.ariaLabel,
                    },
                    slots.default?.(),
                ),
    })
    return {
        Avatar: control,
        Button: control,
        Checkbox: control,
        commonMessages: {
            playButton: { defaultMessage: 'Play' },
            stopButton: { defaultMessage: 'Stop' },
            repairButton: { defaultMessage: 'Repair' },
            installingLabel: { defaultMessage: 'Installing' },
            moreOptionsButton: { defaultMessage: 'More options' },
        },
        defineMessages: (messages: unknown) => messages,
        formatLoaderLabel: (loader: string) =>
            ({ neoforge: 'NeoForge', fabric: 'Fabric' })[loader] ?? loader,
        injectNotificationManager: () => ({ handleError: vi.fn() }),
        useRelativeTime: () => () => 'now',
        useVIntl: () => ({
            formatMessage: (
                message: { defaultMessage: string },
                values: Record<string, unknown> = {},
            ) =>
                message.defaultMessage.replace(/\{(\w+)\}/g, (_, key) =>
                    String(values[key] ?? key),
                ),
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

function mountList(overrides: Record<string, unknown> = {}) {
    return mount(
        defineComponent({
            setup: () => () =>
                h(Suspense, null, {
                    default: () =>
                        h(Instance, {
                            instance: {
                                ...instance,
                                loader: 'neoforge',
                                last_played: new Date('2026-11-01T00:00:00Z'),
                            },
                            variant: 'list',
                            playing: false,
                            showMore: true,
                            ...overrides,
                        }),
                }),
        }),
        { global: { directives: { tooltip: () => {} } } },
    )
}

it('renders a detailed list row with the formatted loader, version and last played time', async () => {
    const wrapper = mountList()
    try {
        await flushPromises()
        expect(wrapper.text()).toContain('External')
        expect(wrapper.get('[data-testid="instance-list-version"]').text()).toBe('NeoForge 1.21')
        expect(wrapper.get('[data-testid="instance-list-last-played"]').text()).toContain(
            'Played now',
        )
    } finally {
        wrapper.unmount()
    }
})

it('shows the never-played label when the list row has no last played date', async () => {
    const wrapper = mountList({
        instance: { ...instance, loader: 'neoforge', last_played: undefined },
    })
    try {
        await flushPromises()
        expect(wrapper.get('[data-testid="instance-list-last-played"]').text()).toContain(
            'Never played',
        )
    } finally {
        wrapper.unmount()
    }
})

it('plays from the list row without opening the instance page', async () => {
    backend.run.mockReset().mockResolvedValue(undefined)
    backend.push.mockReset()
    const wrapper = mountList()
    try {
        await flushPromises()
        const play = wrapper.get('[data-testid="instance-list-play"]')
        await play.trigger('click')
        expect(backend.run).toHaveBeenCalledWith('external')
        expect(backend.push).not.toHaveBeenCalled()
    } finally {
        wrapper.unmount()
    }
})

it('emits more from the list row without opening the instance page', async () => {
    const wrapper = mountList()
    try {
        await flushPromises()
        const more = wrapper.get('[data-testid="instance-list-more"]')
        await more.trigger('click')
        expect(wrapper.findComponent(Instance).emitted('more')).toHaveLength(1)
        expect(backend.push).not.toHaveBeenCalled()
    } finally {
        wrapper.unmount()
    }
})

it('renders the selection checkbox only in list select mode', async () => {
    const plain = mountList()
    const selecting = mountList({ selectMode: true, selected: true })

    try {
        await flushPromises()
        expect(plain.find('[aria-label="External"]').exists()).toBe(false)
        expect(selecting.get('[aria-label="External"]').attributes('data-model-value')).toBe('true')
        await selecting.get('[aria-label="External"]').trigger('click')
        expect(selecting.findComponent(Instance).emitted('toggleSelect')).toHaveLength(1)
    } finally {
        plain.unmount()
        selecting.unmount()
    }
})

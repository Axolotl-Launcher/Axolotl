import { QueryClient, VueQueryPlugin } from '@tanstack/vue-query'
import { flushPromises, mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { defineComponent, h, Suspense } from 'vue'

import { resourceProjectDocuments } from '@/helpers/resources'
import type { GameInstance } from '@/helpers/types'

import InstanceItem from './InstanceItem.vue'

vi.mock('@/helpers/resources', () => ({ resourceProjectDocuments: vi.fn() }))
vi.mock('@/helpers/events', () => ({ process_listener: vi.fn().mockResolvedValue(() => {}) }))
vi.mock('@/helpers/process', () => ({ get_by_instance_id: vi.fn().mockResolvedValue([]) }))
vi.mock('@/helpers/instance', () => ({ kill: vi.fn(), run: vi.fn() }))
vi.mock('@/helpers/analytics', () => ({ trackEvent: vi.fn() }))
vi.mock('@/helpers/utils', () => ({ showInstanceInFolder: vi.fn() }))
vi.mock('@/store/error', () => ({ handleSevereError: vi.fn() }))
vi.mock('@/composables/useMinecraftLaunchError', () => ({ useMinecraftLaunchError: () => vi.fn() }))
vi.mock('vue-router', () => ({ useRouter: () => ({ push: vi.fn() }) }))
vi.mock('@/components/ui/InstanceIcon.vue', () => ({ default: { render: () => null } }))
vi.mock('@modrinth/ui', async () => {
    const { defineComponent, h } = await import('vue')
    const slots = defineComponent({
        setup:
            (_, { slots }) =>
            () =>
                h('div', [slots.clickable?.(), slots.default?.()]),
    })
    return {
        Avatar: defineComponent({
            props: ['src'],
            setup: (props) => () => h('img', { src: props.src }),
        }),
        Button: slots,
        OverflowMenu: slots,
        SmartClickable: slots,
        commonMessages: {
            playButton: { defaultMessage: 'Play' },
            stopButton: { defaultMessage: 'Stop' },
            playedLabel: { defaultMessage: 'Played' },
        },
        defineMessages: (messages: unknown) => messages,
        injectNotificationManager: () => ({ handleError: vi.fn() }),
        useFormatDateTime: () => () => '',
        useRelativeTime: () => () => '',
        useVIntl: () => ({
            formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
        }),
    }
})

function instance(link: GameInstance['link'], id = 'instance'): GameInstance {
    return {
        id,
        name: 'Project Infinity',
        link,
        loader: 'neoforge',
        game_version: '1.21.1',
    } as GameInstance
}
function render(value: GameInstance) {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
    const root = defineComponent({
        props: ['instance'],
        setup: (props) => () =>
            h(Suspense, null, {
                default: () =>
                    h(InstanceItem, {
                        instance: props.instance,
                        playing: false,
                        lastPlayed: undefined,
                    }),
            }),
    })
    const wrapper = mount(root, {
        props: { instance: value },
        global: {
            plugins: [[VueQueryPlugin, { queryClient: client }]],
            directives: { tooltip: () => {} },
            stubs: {
                RouterLink: defineComponent({
                    props: ['to'],
                    setup:
                        (props, { slots }) =>
                        () =>
                            h('a', { href: props.to }, slots.default?.()),
                }),
            },
        },
    })
    return {
        wrapper,
        cleanup: () => {
            wrapper.unmount()
            client.clear()
        },
    }
}

it('loads CurseForge instance metadata through its own source at startup and links to that project', async () => {
    vi.mocked(resourceProjectDocuments)
        .mockReset()
        .mockResolvedValue([
            { metadata: { title: 'Project Infinity 0.1', iconUrl: 'https://example.com/cf.png' } },
        ] as never)
    const { wrapper, cleanup } = render(
        instance({ type: 'curseforge_modpack', project_id: '1266680', version_id: '1' }),
    )
    try {
        await flushPromises()
        expect(resourceProjectDocuments).toHaveBeenCalledWith(
            'curseforge',
            ['1266680'],
            expect.any(AbortSignal),
        )
        expect(resourceProjectDocuments).toHaveBeenCalledTimes(1)
        expect(wrapper.text()).toContain('Project Infinity 0.1')
        expect(wrapper.find('a[href="/project/curseforge/1266680"]').exists()).toBe(true)
        expect(wrapper.find('img').attributes('src')).toBe('https://example.com/cf.png')
    } finally {
        cleanup()
    }
})

it('a failed metadata lookup settles loading and keeps the local instance usable', async () => {
    vi.mocked(resourceProjectDocuments).mockReset().mockRejectedValue(new Error('project removed'))
    const { wrapper, cleanup } = render(
        instance({ type: 'curseforge_modpack', project_id: '1266680', version_id: '1' }),
    )
    try {
        await flushPromises()
        expect(wrapper.text()).not.toContain('Loading modpack')
        expect(wrapper.text()).toContain('NeoForge')
        expect(wrapper.find('a[href="/instance/instance"]').exists()).toBe(true)
    } finally {
        cleanup()
    }
})

it('an old provider response cannot overwrite a different instance with the same project ID', async () => {
    let finish!: (value: never) => void
    vi.mocked(resourceProjectDocuments)
        .mockReset()
        .mockImplementationOnce(
            () =>
                new Promise((resolve) => {
                    finish = resolve
                }),
        )
        .mockResolvedValue([{ metadata: { title: 'Modrinth project', iconUrl: null } }] as never)
    const { wrapper, cleanup } = render(
        instance({ type: 'curseforge_modpack', project_id: '12666800', version_id: '1' }),
    )
    try {
        await flushPromises()
        await wrapper.setProps({
            instance: instance(
                { type: 'modrinth_modpack', project_id: '12666800', version_id: 'abcdefgh' },
                'second',
            ),
        })
        await flushPromises()
        finish([{ metadata: { title: 'Old CurseForge project', iconUrl: null } }] as never)
        await flushPromises()
        expect(wrapper.text()).toContain('Modrinth project')
        expect(wrapper.text()).not.toContain('Old CurseForge project')
        expect(wrapper.find('a[href="/project/12666800"]').exists()).toBe(true)
    } finally {
        cleanup()
    }
})

it('imported project IDs without a provider never trigger an online lookup', async () => {
    vi.mocked(resourceProjectDocuments).mockReset()
    const { cleanup } = render(instance({ type: 'imported_modpack', project_id: '1266680' }))
    try {
        await flushPromises()
        expect(resourceProjectDocuments).not.toHaveBeenCalled()
    } finally {
        cleanup()
    }
})

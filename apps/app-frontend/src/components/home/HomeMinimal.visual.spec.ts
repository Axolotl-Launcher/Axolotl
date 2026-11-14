import '@/test/visual-styles.scss'

import { QueryClient, VueQueryPlugin } from '@tanstack/vue-query'
import { flushPromises, mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { computed, defineComponent, h, ref } from 'vue'

import type { GameInstance } from '@/helpers/types'

import { HOME_LAUNCH_KEY, type HomeLaunchContext } from './home-launch'
import {
    HOME_MINIMAL_CLASSIC,
    HOME_MINIMAL_PURE,
    HOME_MINIMAL_STANDARD,
    resolveHomeMinimalLayout,
    updateHomeMinimalLayout,
} from './home-minimal'
import HomeMinimal from './HomeMinimal.vue'

const backend = vi.hoisted(() => ({ processes: vi.fn(), listen: vi.fn() }))
vi.mock('@/helpers/process', () => ({ get_by_instance_id: backend.processes }))
vi.mock('@/helpers/events', () => ({ process_listener: backend.listen }))
vi.mock('@/helpers/instance', () => ({ kill: vi.fn(), run: vi.fn() }))
vi.mock('@/helpers/install', () => ({
    install_existing_instance: vi.fn(),
    install_pack_to_existing_instance: vi.fn(),
}))
vi.mock('@/helpers/analytics', () => ({ trackEvent: vi.fn() }))
vi.mock('@/store/error', () => ({ handleSevereError: vi.fn() }))
vi.mock('@/composables/useMinecraftLaunchError', () => ({ useMinecraftLaunchError: () => vi.fn() }))
vi.mock('@/composables/useNetworkStatus', async () => {
    const { ref } = await import('vue')
    return { useNetworkStatus: () => ({ offline: ref(false) }) }
})
vi.mock('@/components/home/HomeGreeting.vue', async () => {
    const { defineComponent, h } = await import('vue')
    return {
        default: defineComponent({
            props: ['showDate', 'showHeading', 'accentRule'],
            setup: (props) => () =>
                h('header', {
                    'data-greeting': '',
                    'data-show-date': String(props.showDate),
                    'data-show-heading': String(props.showHeading),
                    'data-accent-rule': String(props.accentRule),
                }),
        }),
    }
})
vi.mock('@/components/home/home-dashboard-runtime', () => ({
    useOptionalHomeDashboardRuntime: () => null,
}))
vi.mock('@/helpers/worlds', () => ({ get_recent_worlds: async () => [] }))
vi.mock('@/helpers/instance-icons', () => ({
    getDisplayInstanceIcon: () => ({ url: null, frameless: false }),
}))
vi.mock('@/components/ui/InstanceIcon.vue', () => ({ default: { render: () => null } }))
vi.mock('@modrinth/ui', async () => {
    const { defineComponent, h } = await import('vue')
    return {
        Button: defineComponent({
            setup:
                (_, { slots }) =>
                () =>
                    h('button', slots.default?.()),
        }),
        Card: defineComponent({
            setup:
                (_, { slots }) =>
                () =>
                    h('div', slots.default?.()),
        }),
        commonMessages: {
            retryButton: { defaultMessage: 'Retry' },
            playButton: { defaultMessage: 'Play' },
            stopButton: { defaultMessage: 'Stop' },
            installButton: { defaultMessage: 'Install' },
            installingLabel: { defaultMessage: 'Installing' },
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

function launchContext(selectedId: string | null) {
    const selectedInstanceId = ref(selectedId)
    const context: HomeLaunchContext = {
        selectedInstanceId: computed(() => selectedInstanceId.value),
        choose: vi.fn(),
        select: vi.fn((instance: GameInstance) => {
            selectedInstanceId.value = instance.id
        }),
        create: vi.fn(),
    }
    return context
}

const routerLinkStub = defineComponent({
    setup:
        (_, { slots }) =>
        () =>
            h('a', slots.default?.()),
})

it('keeps the minimal home synchronous and makes a failed background process refresh retryable', async () => {
    backend.processes.mockResolvedValue([])
    backend.listen.mockResolvedValue(vi.fn())
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })
    const instance = {
        id: 'one',
        name: 'One',
        install_stage: 'installed',
        loader: 'vanilla',
        game_version: '1.21',
    } as GameInstance
    const wrapper = mount(HomeMinimal, {
        props: { instances: [instance] },
        global: {
            plugins: [[VueQueryPlugin, { queryClient }]],
            provide: { [HOME_LAUNCH_KEY as symbol]: launchContext(instance.id) },
            directives: { tooltip: () => {} },
            stubs: { RouterLink: routerLinkStub },
        },
    })
    try {
        expect(wrapper.find('section').exists()).toBe(true)
        await flushPromises()
        expect(wrapper.text()).toContain('Play')
        backend.processes.mockRejectedValueOnce(new Error('refresh failed'))
        await queryClient.refetchQueries({ queryKey: ['home', 'instance-processes', instance.id] })
        await flushPromises()
        const retry = wrapper.findAll('button').find((button) => button.text() === 'Retry')!
        expect(retry).toBeDefined()
        await retry.trigger('click')
        await flushPromises()
        expect(wrapper.text()).toContain('Play')
        expect(wrapper.text()).not.toContain('Retry')
    } finally {
        wrapper.unmount()
        queryClient.clear()
    }
})

it('shows the selected game directory only when the full collection spans multiple directories', async () => {
    backend.processes.mockResolvedValue([])
    backend.listen.mockResolvedValue(vi.fn())
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })
    const selected = {
        id: 'managed',
        name: 'Managed',
        install_stage: 'installed',
        loader: 'vanilla',
        game_version: '1.21',
    } as GameInstance
    const external = {
        id: 'external',
        name: 'External',
        install_stage: 'installed',
        loader: 'fabric',
        game_version: '1.20.1',
        game_dir_override: String.raw`D:\Minecraft\.minecraft`,
    } as GameInstance

    const mountHome = (instances: GameInstance[]) =>
        mount(HomeMinimal, {
            props: { instances },
            global: {
                plugins: [[VueQueryPlugin, { queryClient }]],
                provide: { [HOME_LAUNCH_KEY as symbol]: launchContext(selected.id) },
                directives: { tooltip: () => {} },
                stubs: { RouterLink: routerLinkStub },
            },
        })

    const multiDirectory = mountHome([selected, external])
    const singleDirectory = mountHome([selected])
    try {
        await flushPromises()
        expect(multiDirectory.findAll('[data-game-directory-badge]')).toHaveLength(1)
        expect(singleDirectory.find('[data-game-directory-badge]').exists()).toBe(false)
    } finally {
        multiDirectory.unmount()
        singleDirectory.unmount()
        queryClient.clear()
    }
})

it('launches the last played instance until one is chosen, and switches in one click', async () => {
    backend.processes.mockResolvedValue([])
    backend.listen.mockResolvedValue(vi.fn())
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })
    const older = {
        id: 'older',
        name: 'Older',
        install_stage: 'installed',
        loader: 'fabric',
        game_version: '1.20.1',
        last_played: '2026-01-01T10:00:00Z',
    } as unknown as GameInstance
    const latest = {
        id: 'latest',
        name: 'Latest',
        install_stage: 'not_installed',
        loader: 'vanilla',
        game_version: '1.21',
        last_played: '2026-02-01T10:00:00Z',
    } as unknown as GameInstance
    const context = launchContext(null)
    const wrapper = mount(HomeMinimal, {
        props: { instances: [older, latest] },
        global: {
            plugins: [[VueQueryPlugin, { queryClient }]],
            provide: { [HOME_LAUNCH_KEY as symbol]: context },
            directives: { tooltip: () => {} },
            stubs: { RouterLink: routerLinkStub },
        },
    })
    try {
        await flushPromises()
        // Nothing chosen: the most recently played instance leads, and it is not installed.
        expect(wrapper.get('.home-launch-name').text()).toBe('Latest')
        expect(wrapper.get('.home-launch-card').attributes('data-phase')).toBe('not-installed')
        expect(wrapper.text()).toContain('Install')

        const switcher = wrapper
            .findAll('.home-launch-recent-card')
            .find((card) => card.get('.home-launch-recent-name').text() === 'Older')
        expect(switcher).toBeDefined()
        await switcher!.trigger('click')
        await flushPromises()
        expect(context.select).toHaveBeenCalledWith(older)
        expect(wrapper.get('.home-launch-name').text()).toBe('Older')
        expect(wrapper.text()).toContain('Play')

        backend.processes.mockResolvedValue([{ uuid: 'p', instance_id: 'older' }])
        await queryClient.refetchQueries({ queryKey: ['home', 'instance-processes', 'older'] })
        await flushPromises()
        expect(wrapper.get('.home-launch-card').attributes('data-phase')).toBe('running')
        expect(wrapper.text()).toContain('Stop')
    } finally {
        wrapper.unmount()
        queryClient.clear()
    }
})

it('asks for an instance when none was ever played', async () => {
    backend.processes.mockResolvedValue([])
    backend.listen.mockResolvedValue(vi.fn())
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })
    const fresh = {
        id: 'fresh',
        name: 'Fresh',
        install_stage: 'installed',
        loader: 'vanilla',
        game_version: '1.21',
        last_played: null,
    } as unknown as GameInstance
    const context = launchContext(null)
    const wrapper = mount(HomeMinimal, {
        props: { instances: [fresh] },
        global: {
            plugins: [[VueQueryPlugin, { queryClient }]],
            provide: { [HOME_LAUNCH_KEY as symbol]: context },
            directives: { tooltip: () => {} },
            stubs: { RouterLink: routerLinkStub },
        },
    })
    try {
        await flushPromises()
        expect(wrapper.get('.home-launch-card').classes()).toContain('is-empty')
        await wrapper.get('.home-launch-primary').trigger('click')
        expect(context.choose).toHaveBeenCalledOnce()
    } finally {
        wrapper.unmount()
        queryClient.clear()
    }
})

function mountMinimal(layout: Parameters<typeof resolveHomeMinimalLayout>[0] | null) {
    backend.processes.mockResolvedValue([])
    backend.listen.mockResolvedValue(vi.fn())
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })
    const instances = ['one', 'two'].map(
        (id, index) =>
            ({
                id,
                name: id === 'one' ? 'One' : 'Two',
                install_stage: 'installed',
                loader: 'vanilla',
                game_version: '1.21',
                last_played: `2026-0${index + 1}-01T10:00:00Z`,
            }) as unknown as GameInstance,
    )
    const wrapper = mount(HomeMinimal, {
        attachTo: document.body,
        props: {
            instances,
            layout: layout ? resolveHomeMinimalLayout(layout) : null,
        },
        global: {
            plugins: [[VueQueryPlugin, { queryClient }]],
            provide: { [HOME_LAUNCH_KEY as symbol]: launchContext('one') },
            directives: { tooltip: () => {} },
            stubs: { RouterLink: routerLinkStub },
        },
    })
    wrapper.element.setAttribute('style', 'width:960px')
    return { wrapper, queryClient }
}

it('draws the pure preset as one launchable instance in the middle and nothing else', async () => {
    const { wrapper, queryClient } = mountMinimal({ preset: 'pure', ...HOME_MINIMAL_PURE })
    try {
        await flushPromises()
        // Only the icon, the name and the launch button remain.
        expect(wrapper.find('[data-greeting]').exists()).toBe(false)
        expect(wrapper.find('.home-launch-status').exists()).toBe(false)
        expect(wrapper.find('.home-launch-meta').exists()).toBe(false)
        expect(wrapper.find('.home-launch-switch').exists()).toBe(false)
        expect(wrapper.find('.home-launch-recent').exists()).toBe(false)
        expect(wrapper.find('.home-launch-glow').exists()).toBe(false)
        expect(wrapper.get('.home-launch-name').text()).toBe('One')
        expect(wrapper.get('.home-launch-primary').text()).toBe('Play')

        // No card behind it, and the column is centred on the page with no leftover space.
        const card = wrapper.get('.home-launch-card')
        expect(card.classes()).not.toContain('home-surface')
        const stage = wrapper.get('.minimal-home-stage').element as HTMLElement
        const stageBox = stage.getBoundingClientRect()
        const cardBox = card.element.getBoundingClientRect()
        expect(
            Math.abs(cardBox.left + cardBox.width / 2 - (stageBox.left + stageBox.width / 2)),
            JSON.stringify({ stageBox, cardBox }),
        ).toBeLessThan(1)
        const style = getComputedStyle(stage)
        expect(style.paddingTop).toBe(style.paddingBottom)
        const icon = wrapper.get('.home-launch-icon').element.getBoundingClientRect()
        const button = wrapper.get('.home-launch-primary').element.getBoundingClientRect()
        // Stacked: icon over the button, both on the same centre line.
        expect(icon.bottom).toBeLessThanOrEqual(button.top)
        expect(
            Math.abs(icon.left + icon.width / 2 - (button.left + button.width / 2)),
            JSON.stringify({ icon, button }),
        ).toBeLessThan(1)
    } finally {
        wrapper.unmount()
        queryClient.clear()
    }
})

it('keeps the standard look when no options were ever saved', async () => {
    const { wrapper, queryClient } = mountMinimal(null)
    try {
        await flushPromises()
        expect(wrapper.find('[data-greeting]').exists()).toBe(true)
        expect(wrapper.get('.home-launch-card').classes()).toContain('home-surface')
        expect(wrapper.find('.home-launch-status').exists()).toBe(true)
        expect(wrapper.find('.home-launch-meta').exists()).toBe(true)
        expect(wrapper.find('.home-launch-switch').exists()).toBe(true)
        expect(wrapper.find('.home-launch-recent').exists()).toBe(true)
        expect(wrapper.get('.minimal-home-stage').attributes('data-position')).toBe(
            HOME_MINIMAL_STANDARD.position,
        )
    } finally {
        wrapper.unmount()
        queryClient.clear()
    }
})

it.each(['row', 'bar'] as const)(
    'keeps recent instances full width below a centred plain %s card',
    async (arrangement) => {
        const { wrapper, queryClient } = mountMinimal(
            updateHomeMinimalLayout(null, { surface: 'plain', arrangement }),
        )
        try {
            await flushPromises()
            expect(wrapper.get('.home-launch').attributes('data-fit')).toBeDefined()
            for (const width of [960, 380]) {
                wrapper.element.setAttribute('style', `width:${width}px`)
                const launch = wrapper.get('.home-launch').element.getBoundingClientRect()
                const card = wrapper.get('.home-launch-card').element.getBoundingClientRect()
                const recent = wrapper.get('.home-launch-recent').element.getBoundingClientRect()
                const header = wrapper
                    .get('.home-launch-recent-header')
                    .element.getBoundingClientRect()
                const tile = wrapper.get('.home-launch-recent-card').element.getBoundingClientRect()

                expect(recent.width).toBeGreaterThan(0)
                expect(Math.abs(recent.width - launch.width)).toBeLessThan(1)
                expect(Math.abs(recent.left - launch.left)).toBeLessThan(1)
                expect(
                    Math.abs(card.left + card.width / 2 - (launch.left + launch.width / 2)),
                ).toBeLessThan(1)
                expect(tile.top).toBeGreaterThanOrEqual(header.bottom)
                expect(tile.left).toBeGreaterThanOrEqual(recent.left)
                expect(tile.right).toBeLessThanOrEqual(recent.right + 1)
            }
        } finally {
            wrapper.unmount()
            queryClient.clear()
        }
    },
)

it('hides each part on its own without leaving an empty greeting behind', async () => {
    const { wrapper, queryClient } = mountMinimal(
        updateHomeMinimalLayout(null, { show: { greeting: false, switch: false } }),
    )
    try {
        await flushPromises()
        const greeting = wrapper.get('[data-greeting]')
        expect(greeting.attributes('data-show-date')).toBe('true')
        expect(greeting.attributes('data-show-heading')).toBe('false')
        expect(wrapper.find('.home-launch-switch').exists()).toBe(false)
        expect(wrapper.find('.home-launch-status').exists()).toBe(true)
        await wrapper.setProps({
            layout: resolveHomeMinimalLayout(
                updateHomeMinimalLayout(null, { show: { greeting: false, date: false } }),
            ),
        })
        expect(wrapper.find('[data-greeting]').exists()).toBe(false)
    } finally {
        wrapper.unmount()
        queryClient.clear()
    }
})

it('draws the classic strip: one slim row with the button beside the name, and the accent rule', async () => {
    const { wrapper, queryClient } = mountMinimal({ preset: 'classic', ...HOME_MINIMAL_CLASSIC })
    try {
        await flushPromises()
        expect(wrapper.get('[data-greeting]').attributes('data-accent-rule')).toBe('true')
        expect(wrapper.get('[data-greeting]').attributes('data-show-date')).toBe('false')
        const card = wrapper.get('.home-launch-card')
        // A plain card, no hero wash or icon glow, as before the redesign.
        expect(card.classes()).toContain('home-surface')
        expect(card.classes()).not.toContain('home-surface-hero')
        expect(wrapper.find('.home-launch-glow').exists()).toBe(false)
        expect(wrapper.find('.home-launch-status').exists()).toBe(false)
        expect(wrapper.find('.home-launch-recent').exists()).toBe(false)
        // Version and last played only, as plain text; no total time in the strip.
        expect(wrapper.findAll('.home-launch-meta .home-launch-meta-item')).toHaveLength(2)
        expect(wrapper.find('.home-launch-meta .home-chip').exists()).toBe(false)

        const cardBox = card.element.getBoundingClientRect()
        const name = wrapper.get('.home-launch-name').element.getBoundingClientRect()
        const button = wrapper.get('.home-launch-primary').element.getBoundingClientRect()
        const switchButton = wrapper.get('.home-launch-switch').element.getBoundingClientRect()
        expect(cardBox.height).toBeLessThan(100)
        expect(button.left).toBeGreaterThan(name.right)
        expect(
            Math.abs(button.top + button.height / 2 - (switchButton.top + switchButton.height / 2)),
        ).toBeLessThan(1)
        expect(button.top).toBeLessThan(name.bottom)
        expect(cardBox.right - switchButton.right).toBeLessThan(32)
    } finally {
        wrapper.unmount()
        queryClient.clear()
    }
})

it('wraps the strip under the name at narrow widths instead of overflowing', async () => {
    const { wrapper, queryClient } = mountMinimal({ preset: 'classic', ...HOME_MINIMAL_CLASSIC })
    wrapper.element.setAttribute('style', 'width:380px')
    try {
        await flushPromises()
        const card = wrapper.get('.home-launch-card').element as HTMLElement
        expect(card.scrollWidth).toBeLessThanOrEqual(card.clientWidth)
        const name = wrapper.get('.home-launch-name').element.getBoundingClientRect()
        const button = wrapper.get('.home-launch-primary').element.getBoundingClientRect()
        expect(button.top).toBeGreaterThanOrEqual(name.bottom)
    } finally {
        wrapper.unmount()
        queryClient.clear()
    }
})

it('says why it offers Retry when the running check fails, and recovers after it', async () => {
    backend.processes.mockResolvedValue([])
    backend.listen.mockResolvedValue(vi.fn())
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })
    const instance = {
        id: 'one',
        name: 'One',
        install_stage: 'installed',
        loader: 'vanilla',
        game_version: '1.21',
    } as GameInstance
    const wrapper = mount(HomeMinimal, {
        props: {
            instances: [instance],
            // A layout without the status line still explains a failure.
            layout: { ...HOME_MINIMAL_PURE },
        },
        global: {
            plugins: [[VueQueryPlugin, { queryClient }]],
            provide: { [HOME_LAUNCH_KEY as symbol]: launchContext(instance.id) },
            directives: { tooltip: () => {} },
            stubs: { RouterLink: routerLinkStub },
        },
    })
    try {
        await flushPromises()
        backend.processes.mockRejectedValueOnce(new Error('process query failed'))
        await queryClient.refetchQueries({ queryKey: ['home', 'instance-processes', instance.id] })
        await flushPromises()
        const status = wrapper.get('.home-launch-status')
        expect(status.text()).toBe("Couldn't check whether it is running")
        const retry = wrapper.findAll('button').find((button) => button.text() === 'Retry')!
        expect(retry.attributes('aria-describedby')).toBe(status.attributes('id'))
        await retry.trigger('click')
        await flushPromises()
        expect(wrapper.text()).toContain('Play')
        expect(wrapper.text()).not.toContain("Couldn't check")
    } finally {
        wrapper.unmount()
        queryClient.clear()
    }
})

it('lists only played instances under Recently played', async () => {
    backend.processes.mockResolvedValue([])
    backend.listen.mockResolvedValue(vi.fn())
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })
    const played = {
        id: 'played',
        name: 'Played',
        install_stage: 'installed',
        loader: 'vanilla',
        game_version: '1.21',
        last_played: '2026-01-01T10:00:00Z',
    } as unknown as GameInstance
    const never = (id: string) =>
        ({
            id,
            name: id,
            install_stage: 'installed',
            loader: 'vanilla',
            game_version: '1.21',
            last_played: null,
        }) as unknown as GameInstance
    const mountWith = (instances: GameInstance[], selected: string) =>
        mount(HomeMinimal, {
            props: { instances },
            global: {
                plugins: [[VueQueryPlugin, { queryClient }]],
                provide: { [HOME_LAUNCH_KEY as symbol]: launchContext(selected) },
                directives: { tooltip: () => {} },
                stubs: { RouterLink: routerLinkStub },
            },
        })
    // One played instance, the others never started: no "Recently played" at all.
    const onlyOne = mountWith([played, never('a'), never('b')], 'played')
    // A never-played instance chosen for Home: the played one is its only recent.
    const chosenNew = mountWith([played, never('a'), never('b')], 'a')
    try {
        await flushPromises()
        expect(onlyOne.find('.home-launch-recent').exists()).toBe(false)
        expect(onlyOne.text()).not.toContain('Never played')
        expect(chosenNew.findAll('.home-launch-recent-name').map((name) => name.text())).toEqual([
            'Played',
        ])
    } finally {
        onlyOne.unmount()
        chosenNew.unmount()
        queryClient.clear()
    }
})

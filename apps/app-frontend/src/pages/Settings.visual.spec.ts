import { flushPromises, mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { type Component, defineComponent, h, Suspense } from 'vue'

import Settings from './Settings.vue'

const fixture = vi.hoisted(() => ({
    finish: {} as Record<string, () => void>,
    replace: vi.fn().mockResolvedValue(undefined),
    route: { hash: '#interface', query: {} as Record<string, unknown> },
}))
vi.mock('@tauri-apps/api/app', () => ({ getVersion: async () => '1.0' }))
vi.mock('@tauri-apps/plugin-os', () => ({ platform: () => 'windows', version: () => '11' }))
vi.mock('@/store/state', () => ({ useTheming: () => ({ devMode: false }) }))
vi.mock('@/helpers/settings', () => ({ update: vi.fn() }))
vi.mock('@/providers/download-progress', async () => {
    const { ref } = await import('vue')
    return { injectAppUpdateDownloadProgress: () => ({ progress: ref(null), version: ref(null) }) }
})
vi.mock('vue-router', () => ({
    useRoute: () => fixture.route,
    useRouter: () => ({ replace: fixture.replace }),
}))
vi.mock('@modrinth/ui', () => ({
    defineMessages: (value: unknown) => value,
    ProgressBar: { render: () => null },
    useLoadingBarToken: () => {},
    injectNotificationManager: () => ({ handleError: vi.fn() }),
    useVIntl: () => ({
        formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
    }),
}))
vi.mock('@/components/ui/settings/settings-search-index', () => ({
    getSettingsSearchTargetId: (entry: { id: string; targetId?: string }) =>
        entry.targetId ?? entry.id,
}))
vi.mock('@/components/ui/settings/settings-registry', async () => {
    const { defineAsyncComponent, h, markRaw } = await import('vue')
    const message = (text: string) => ({ id: text, defaultMessage: text })
    const categories = [
        {
            id: 'interface',
            group: 'launcher',
            name: message('Appearance'),
            entries: [],
            content: markRaw({ render: () => h('p', 'Appearance content') }),
        },
        {
            id: 'content-downloads',
            group: 'launcher',
            name: message('Downloads'),
            entries: [
                {
                    id: 'resources-force-multiconnection',
                    targetId: 'settings-target-resources-force-multiconnection',
                    label: message('Force multi-connection downloads'),
                },
            ],
            content: markRaw({
                render: () => h('input', { id: 'settings-target-resources-force-multiconnection' }),
            }),
        },
        ...['slow', 'latest'].map((id) => ({
            id,
            group: 'launcher',
            name: message(id),
            icon: markRaw({ render: () => null }),
            entries: [{ id: `${id}-target`, label: message(`${id} setting`) }],
            content: defineAsyncComponent(
                () =>
                    new Promise<Component>((resolve) => {
                        fixture.finish[id] = () =>
                            resolve({ render: () => h('input', { id: `${id}-target` }) })
                    }),
            ),
        })),
    ]
    return {
        getVisibleSettingsCategories: () => categories,
        getVisibleSettingsGroups: () => [{ id: 'launcher', name: message('Launcher'), categories }],
        settingsPageTitle: message('Settings'),
    }
})

it('locates a linked setting within the requested category', async () => {
    fixture.route = {
        hash: '#content-downloads',
        query: { target: 'resources-force-multiconnection' },
    }
    const wrapper = mount(
        defineComponent({ render: () => h(Suspense, null, { default: () => h(Settings) }) }),
        { attachTo: document.body },
    )
    try {
        await vi.waitFor(() =>
            expect(
                wrapper.get('#settings-target-resources-force-multiconnection').classes(),
            ).toContain('settings-search-result-highlight'),
        )
        expect(wrapper.find('input[type="search"]').element).toHaveProperty('value', '')
    } finally {
        wrapper.unmount()
        fixture.route = { hash: '#interface', query: {} }
    }
})

it('waits for async settings to resolve and only locates the latest selected result', async () => {
    const wrapper = mount(
        defineComponent({ render: () => h(Suspense, null, { default: () => h(Settings) }) }),
        { attachTo: document.body },
    )
    try {
        await flushPromises()
        const input = wrapper.get<HTMLInputElement>('input[type="search"]')
        await input.setValue('setting')
        await vi.waitFor(() => expect(wrapper.findAll('.settings-search-result')).toHaveLength(2))
        await wrapper.findAll('.settings-search-result')[0].trigger('click')
        await vi.waitFor(() => expect(fixture.finish.slow).toBeDefined())
        await new Promise((resolve) => setTimeout(resolve, 350))
        expect(input.element.value).toBe('setting')
        await wrapper.findAll('.settings-search-result')[1].trigger('click')
        await vi.waitFor(() => expect(fixture.finish.latest).toBeDefined())
        fixture.finish.slow()
        await flushPromises()
        expect(input.element.value).toBe('setting')
        fixture.finish.latest()
        await vi.waitFor(() =>
            expect(wrapper.get('#latest-target').classes()).toContain(
                'settings-search-result-highlight',
            ),
        )
        expect(input.element.value).toBe('')
        expect(wrapper.find('#slow-target').exists()).toBe(false)
    } finally {
        wrapper.unmount()
    }
})

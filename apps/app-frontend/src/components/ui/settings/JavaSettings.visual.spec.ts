import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import { defineComponent, h, Suspense } from 'vue'

import JavaSettings from './JavaSettings.vue'

const backend = vi.hoisted(() => ({
    platform: 'windows',
    settings: {} as Record<string, unknown>,
    patches: [] as Record<string, unknown>[],
}))

vi.mock('@tauri-apps/plugin-os', () => ({ platform: () => backend.platform }))
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn(), confirm: vi.fn() }))
vi.mock('@/helpers/settings.ts', () => ({
    get: async () => structuredClone(backend.settings),
    update: async (patch: Record<string, unknown>) => {
        backend.patches.push(structuredClone(patch))
        Object.assign(backend.settings, structuredClone(patch))
    },
}))
vi.mock('@/helpers/jre', () => ({
    get_java_default_versions: async () => [],
    find_filtered_jres: vi.fn(),
    get_jre: vi.fn(),
    get_java_selection_info: vi.fn(),
    approve_java_selection: vi.fn(),
    remove_java_default_version: vi.fn(),
    set_java_default_version: vi.fn(),
    set_java_version: vi.fn(),
}))
vi.mock('@/helpers/analytics', () => ({ trackEvent: vi.fn() }))
vi.mock('@/helpers/gc/context', () => ({ collectGcContext: async () => ({}) }))
vi.mock('@/helpers/install', () => ({ wait_for_install_job: vi.fn() }))
vi.mock('@/helpers/java-argument-presets', () => ({ getJavaArgumentPresets: () => [] }))
vi.mock('@/composables/useMemorySlider', () => ({ default: () => ({ maxMemory: 8192 }) }))
vi.mock('@/components/ui/JavaSelector.vue', () => ({ default: { render: () => null } }))
vi.mock('@/components/ui/JavaArgumentsInput.vue', () => ({ default: { render: () => null } }))
vi.mock('@/components/ui/MemoryAllocationDisplay.vue', () => ({ default: { render: () => null } }))
vi.mock('./DownloadJavaModal.vue', () => ({ default: { render: () => null } }))
vi.mock('./InstalledJavaModal.vue', () => ({ default: { render: () => null } }))
vi.mock('@modrinth/ui', async () => {
    const { default: Toggle } = await import('@modrinth/ui/src/components/base/Toggle.vue')
    const container = defineComponent({
        setup:
            (_, { attrs, slots }) =>
            () =>
                h('div', attrs, slots.default?.()),
    })
    return {
        Toggle,
        Card: container,
        Button: container,
        Checkbox: container,
        Slider: container,
        defineMessages: (messages: unknown) => messages,
        injectNotificationManager: () => ({ handleError: vi.fn() }),
        useVIntl: () => ({
            formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
        }),
    }
})

const mounted: ReturnType<typeof mount>[] = []
afterEach(() => mounted.splice(0).forEach((wrapper) => wrapper.unmount()))

async function openSettings(platform = 'windows', enabled = false) {
    backend.platform = platform
    backend.settings = {
        minecraft_use_console_java: enabled,
        auto_set_java_high_performance_mode: true,
        extra_launch_args: ['-Dkeep=true'],
        memory: { maximum: 2048, automatic: true, optimize_before_launch: false },
    }
    backend.patches = []
    const wrapper = mount(
        defineComponent({
            setup: () => () => h(Suspense, {}, { default: () => h(JavaSettings) }),
        }),
        { global: { directives: { tooltip: () => {} } } },
    )
    mounted.push(wrapper)
    await flushPromises()
    return wrapper
}

it('saves the Minecraft executable preference and restores it on reopening', async () => {
    const wrapper = await openSettings()
    const toggle = wrapper.get('#settings-target-java-use-console')
    expect(toggle.attributes('aria-checked')).toBe('false')
    expect(backend.patches).toEqual([])
    await toggle.trigger('click')
    await flushPromises()
    expect(backend.patches).toEqual([{ minecraft_use_console_java: true }])
    const reopened = await openSettings('windows', true)
    expect(reopened.get('#settings-target-java-use-console').attributes('aria-checked')).toBe(
        'true',
    )
    await reopened.get('#settings-target-java-use-console').trigger('click')
    await flushPromises()
    expect(backend.patches).toEqual([{ minecraft_use_console_java: false }])
    expect(backend.settings.extra_launch_args).toEqual(['-Dkeep=true'])
})

it.each(['linux', 'macos'])('hides the Windows executable switch on %s', async (platform) => {
    const wrapper = await openSettings(platform)
    expect(wrapper.find('#settings-target-java-use-console').exists()).toBe(false)
    expect(backend.patches).toEqual([])
})

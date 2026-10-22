import '@/test/visual-styles.scss'

import { mount } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import { page } from 'vitest/browser'
import { defineComponent, h, nextTick, ref } from 'vue'
import { createMemoryHistory, createRouter } from 'vue-router'

import chinese from '@/locales/zh-CN/index.json'

import MinecraftCrashModal from './MinecraftCrashModal.vue'

const calls = vi.hoisted(() => ({
    logs: vi.fn(),
    prepare: vi.fn(),
    cancel: vi.fn(),
    enabled: true,
    chinese: false,
    process: undefined as
        | undefined
        | ((event: {
              instance_id: string
              uuid: string
              event: 'launched' | 'finished'
              crashed?: boolean
          }) => void),
}))

vi.mock('@modrinth/ui', async () => ({
    defineMessages: (messages: unknown) => messages,
    commonMessages: { closeButton: { defaultMessage: 'Close' } },
    useVIntl: () => ({
        formatMessage: (
            message: { id: string; defaultMessage: string },
            values: Record<string, unknown> = {},
        ) =>
            (calls.chinese
                ? ((chinese as Record<string, { message: string }>)[message.id]?.message ??
                  message.defaultMessage)
                : message.defaultMessage
            ).replace(/\{(\w+)(?:, number)?\}/g, (_, key) => String(values[key] ?? key)),
    }),
    injectModrinthClient: () => ({}),
    injectNotificationManager: () => ({ addNotification: vi.fn() }),
    Button: (await import('../../../../../packages/ui/src/components/base/buttons/Button.vue'))
        .default,
    Card: defineComponent({
        setup:
            (_, { slots }) =>
            () =>
                h('div', slots.default?.()),
    }),
    Combobox: defineComponent({ inheritAttrs: false, setup: () => () => h('select') }),
    LogViewport: defineComponent({ setup: () => () => h('div') }),
    ProgressBar: defineComponent({ setup: () => () => h('div') }),
    shareLogs: vi.fn(),
    NewModal: defineComponent({
        props: { onHide: Function },
        setup: (props, { slots, expose }) => {
            const visible = ref(false)
            expose({
                show: () => {
                    visible.value = true
                },
                hide: () => {
                    visible.value = false
                    props.onHide?.()
                },
            })
            return () => (visible.value ? h('div', [slots.default?.(), slots.actions?.()]) : null)
        },
    }),
}))
vi.mock('@modrinth/utils/highlightjs', () => ({ renderHighlightedString: (text: string) => text }))
vi.mock('@/components/ui/CrashModChangesModal.vue', () => ({
    default: defineComponent({ setup: () => () => null }),
}))
vi.mock('@/components/ui/settings/LogShareSettings.vue', () => ({
    default: defineComponent({
        emits: ['openProviders'],
        setup:
            (_, { emit }) =>
            () =>
                h('div', [
                    h(
                        'button',
                        {
                            onClick: () => {
                                calls.enabled = true
                            },
                        },
                        'Enable configured model',
                    ),
                    h('button', { onClick: () => emit('openProviders') }, 'Configure providers'),
                ]),
    }),
}))
vi.mock('@/helpers/events.js', () => ({
    process_listener: async (listener: typeof calls.process) => {
        calls.process = listener
        return () => {}
    },
}))
vi.mock('@/helpers/ai', () => ({
    getAIState: async () => ({
        settings: { enabled: true },
        providers: [
            {
                provider_id: 'codeflow',
                enabled: true,
                models: [{ id: 'gpt-6-luna', enabled: true }],
            },
        ],
    }),
}))
vi.mock('@/helpers/instance', () => ({ get: async () => ({ name: 'Test' }) }))
vi.mock('@/composables/useCrashAnalysis', () => ({
    clearCrashAnalysis: vi.fn(),
    refreshCrashAnalysis: async () => null,
}))
vi.mock('@/helpers/logs', () => ({
    get_crash_analysis_ai_settings: async () => ({
        enabled: calls.enabled,
        provider_id: 'codeflow',
        model_id: 'gpt-6-luna',
        ai_source: 'custom',
    }),
    get_log_share_settings: async () => ({
        ai_source: 'custom',
        auto_upload: false,
        show_progress: true,
    }),
    get_logs: calls.logs,
    get_output_by_filename: async () => 'log evidence',
    logshare_get_insights: vi.fn(),
    logshare_upload_crash: vi.fn(),
    record_shared_log: vi.fn(),
}))
vi.mock('@/logagent/backend.ts', () => ({
    prepareSession: calls.prepare,
    cancelSession: calls.cancel,
    runCloud: vi.fn(),
    modelTurn: vi.fn(),
    executeTool: vi.fn(),
}))

const cleanup: (() => void)[] = []
afterEach(() => {
    cleanup.splice(0).forEach((dispose) => dispose())
    vi.restoreAllMocks()
    vi.clearAllMocks()
    calls.enabled = true
    calls.chinese = false
})

it('offers configuration instead of a missing analyze button and keeps source changes local to this run', async () => {
    calls.enabled = false
    calls.logs.mockResolvedValue([])
    const router = createRouter({
        history: createMemoryHistory(),
        routes: [
            { path: '/', component: { render: () => null } },
            { path: '/settings', component: { render: () => null } },
        ],
    })
    await router.push('/')
    const wrapper = mount(MinecraftCrashModal, {
        global: { directives: { tooltip: () => {} }, plugins: [router] },
    })
    cleanup.push(() => wrapper.unmount())
    await wrapper.vm.handleWarning({
        kind: 'minecraft_crash',
        instance_id: 'unconfigured',
        instance_name: 'Test',
    })
    const controls = wrapper.get('[data-testid="crash-analysis-controls"]')
    expect(controls.text()).toContain('Configure analysis')
    await controls
        .findAll('button')
        .find((button) => button.text() === 'Change')!
        .trigger('click')
    await controls.get('[role="radio"][value="logshare"]').trigger('click')
    expect(controls.text()).toContain('Cloud LogShare')
    expect(controls.text()).toContain('Start analysis')
    expect(controls.find('[role="radiogroup"]').exists()).toBe(false)
    await controls
        .findAll('button')
        .find((button) => button.text() === 'Change')!
        .trigger('click')
    await controls.get('[role="radio"][value="custom"]').trigger('click')
    await controls
        .findAll('button')
        .find((button) => button.text() === 'Configure analysis')!
        .trigger('click')
    expect(wrapper.text()).toContain('Return to analysis')
    expect(router.currentRoute.value.fullPath).toBe('/')
    await wrapper
        .findAll('button')
        .find((button) => button.text() === 'Enable configured model')!
        .trigger('click')
    await wrapper
        .findAll('button')
        .find((button) => button.text() === 'Return to analysis')!
        .trigger('click')
    await vi.waitFor(() => expect(controls.text()).toContain('Start analysis'))
    expect(controls.text()).toContain('Local LogAgent')
})

it('keeps crash analysis controls readable in the sidebar', async () => {
    await page.viewport(1200, 900)
    calls.chinese = !!import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE
    calls.logs.mockResolvedValue([{ filename: 'latest.log', log_type: 'InfoLog', age: 1000 }])
    const router = createRouter({
        history: createMemoryHistory(),
        routes: [
            { path: '/', component: { render: () => null } },
            { path: '/settings', component: { render: () => null } },
        ],
    })
    await router.push('/')
    document.documentElement.classList.add('dark-mode')
    const wrapper = mount(MinecraftCrashModal, {
        attachTo: document.body,
        global: { directives: { tooltip: () => {} }, plugins: [router] },
    })
    cleanup.push(() => wrapper.unmount())
    await wrapper.vm.handleWarning({
        kind: 'minecraft_crash',
        instance_id: 'layout',
        instance_name: 'Minecraft 1.21.1',
    })
    wrapper.element.setAttribute(
        'style',
        'width: min(1100px, 100vw); background: var(--surface-1); color: var(--color-text-primary)',
    )
    const controls = wrapper.get('[data-testid="crash-analysis-controls"]')
    expect(controls.element.scrollWidth).toBeLessThanOrEqual(controls.element.clientWidth + 1)
    expect(controls.text()).toContain('codeflow / gpt-6-luna')
    if (import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE)
        await page.screenshot({
            element: wrapper.element,
            path: `${import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE}/logagent-crash-desktop.png`,
        })
    await controls
        .findAll('button')
        .find((button) => button.text() === (calls.chinese ? '更改' : 'Change'))!
        .trigger('click')
    expect(controls.findAll('[role="radio"]')).toHaveLength(2)
    if (import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE)
        await page.screenshot({
            element: controls.element,
            path: `${import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE}/logagent-crash-source.png`,
        })
})

it('restores this crash after visiting provider settings without resetting the selected source', async () => {
    calls.enabled = false
    calls.logs.mockResolvedValue([{ filename: 'latest.log', log_type: 'InfoLog', age: 1000 }])
    const router = createRouter({
        history: createMemoryHistory(),
        routes: [
            { path: '/', component: { render: () => null } },
            { path: '/settings', component: { render: () => null } },
        ],
    })
    await router.push('/')
    const wrapper = mount(MinecraftCrashModal, {
        global: { directives: { tooltip: () => {} }, plugins: [router] },
    })
    cleanup.push(() => wrapper.unmount())
    await wrapper.vm.handleWarning({
        kind: 'minecraft_crash',
        instance_id: 'provider-setup',
        instance_name: 'Preserved crash',
    })
    await wrapper
        .findAll('button')
        .find((button) => button.text() === 'Configure analysis')!
        .trigger('click')
    await wrapper
        .findAll('button')
        .find((button) => button.text() === 'Configure providers')!
        .trigger('click')
    await vi.waitFor(() => expect(router.currentRoute.value.fullPath).toBe('/settings#ai'))
    calls.enabled = true
    await router.push('/settings#launch-defaults')
    await vi.waitFor(() =>
        expect(wrapper.find('[data-testid="crash-analysis-controls"]').exists()).toBe(true),
    )
    expect(wrapper.text()).toContain('Preserved crash')
    expect(wrapper.get('[data-testid="crash-analysis-controls"]').text()).toContain(
        'Local LogAgent',
    )
    expect(wrapper.get('[data-testid="crash-analysis-controls"]').text()).toContain(
        'Start analysis',
    )
    expect(calls.logs).toHaveBeenCalledTimes(1)
})

it('keeps the active LogAgent workspace when log enumeration and duplicate crash notifications arrive late', async () => {
    let resolveLogs!: (logs: unknown[]) => void
    let rejectPreparation!: (error: Error) => void
    calls.logs.mockImplementationOnce(
        () =>
            new Promise((resolve) => {
                resolveLogs = resolve
            }),
    )
    calls.prepare.mockImplementationOnce(
        () =>
            new Promise((_, reject) => {
                rejectPreparation = reject
            }),
    )
    calls.cancel.mockResolvedValue(undefined)
    const router = createRouter({
        history: createMemoryHistory(),
        routes: [
            { path: '/', component: { render: () => null } },
            { path: '/settings', component: { render: () => null } },
        ],
    })
    await router.push('/')
    const wrapper = mount(MinecraftCrashModal, {
        global: { directives: { tooltip: () => {} }, plugins: [router] },
    })
    cleanup.push(() => wrapper.unmount())
    const payload = {
        kind: 'minecraft_crash' as const,
        instance_id: 'instance',
        instance_name: 'Test',
    }
    calls.process?.({ instance_id: 'instance', uuid: 'run-1', event: 'launched' })
    await wrapper.vm.handleWarning(payload)
    const pending = wrapper.vm.openAIAnalysis()
    await nextTick()
    expect(wrapper.find('.crash-modal-ai-output').exists()).toBe(true)
    expect(wrapper.get('[data-testid="crash-analysis-controls"]').text()).toContain(
        'Cancel analysis',
    )
    expect(wrapper.get('[data-testid="crash-analysis-controls"]').text()).not.toContain(
        'Start analysis',
    )
    resolveLogs([{ filename: 'latest.log', log_type: 'InfoLog', age: 1000 }])
    await vi.waitFor(() => expect(wrapper.text()).toContain('latest.log'))
    expect(wrapper.find('.crash-modal-ai-output').exists()).toBe(true)
    const time = Date.now()
    vi.spyOn(Date, 'now').mockReturnValue(time + 20_000)
    await wrapper.vm.handleWarning(payload)
    expect(wrapper.find('.crash-modal-ai-output').exists()).toBe(true)
    expect(calls.logs).toHaveBeenCalledTimes(1)
    rejectPreparation(new Error('diagnostic preparation failed'))
    await pending
    await nextTick()
    expect(wrapper.find('.crash-modal-ai-output').exists()).toBe(true)
    expect(wrapper.text()).toContain('analysis failed')
    expect(wrapper.get('[data-testid="crash-analysis-controls"]').text()).toContain('Analyze again')
    calls.process?.({ instance_id: 'instance', uuid: 'run-1', event: 'finished', crashed: true })
    await new Promise((resolve) => window.setTimeout(resolve, 2300))
    expect(wrapper.find('.crash-modal-ai-output').exists()).toBe(true)
    expect(wrapper.text()).toContain('analysis failed')
    expect(calls.logs).toHaveBeenCalledTimes(1)
})

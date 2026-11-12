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
    exportCrash: vi.fn(),
    highlight: vi.fn(),
    handleError: vi.fn(),
    upload: vi.fn(),
    insights: vi.fn(),
    enabled: true,
    source: 'custom',
    autoUpload: false,
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
    commonMessages: {
        closeButton: { defaultMessage: 'Close' },
        openInFolderButton: { defaultMessage: 'Open in folder' },
    },
    useVIntl: () => ({
        formatMessage: (
            message: { id: string; defaultMessage: string },
            values: Record<string, unknown> = {},
        ) =>
            (calls.chinese
                ? ((chinese as Record<string, { message?: string; defaultMessage?: string }>)[
                      message.id
                  ]?.message ?? message.defaultMessage)
                : message.defaultMessage
            ).replace(/\{(\w+)(?:, number)?\}/g, (_, key) => String(values[key] ?? key)),
    }),
    injectModrinthClient: () => ({}),
    injectNotificationManager: () => ({
        addNotification: vi.fn(),
        handleError: calls.handleError,
    }),
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
    export_crash_context: calls.exportCrash,
    get_crash_analysis_ai_settings: async () => ({
        enabled: calls.enabled,
        provider_id: 'codeflow',
        model_id: 'gpt-6-luna',
        ai_source: calls.source,
    }),
    get_log_share_settings: async () => ({
        share_provider: 'logshare',
        ai_source: calls.source,
        auto_upload: calls.autoUpload,
        show_progress: true,
    }),
    get_logs: calls.logs,
    get_output_by_filename: async () => 'log evidence',
    logshare_get_insights: calls.insights,
    logshare_upload_crash: calls.upload,
    record_shared_log: vi.fn(),
}))
vi.mock('@/helpers/utils', () => ({ highlightInFolder: calls.highlight }))
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
    calls.source = 'custom'
    calls.autoUpload = false
    calls.chinese = false
})

async function mountCrashModal(instanceId: string, instanceName = 'Test') {
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
        instance_id: instanceId,
        instance_name: instanceName,
    })
    return wrapper
}

function actionButton(wrapper: Awaited<ReturnType<typeof mountCrashModal>>, label: string) {
    return wrapper.findAll('button').find((button) => button.text() === label)!
}

it('exports the crash diagnostic package and opens its folder', async () => {
    calls.exportCrash.mockResolvedValue('C:/exports/crash.zip')
    calls.highlight.mockResolvedValue(undefined)
    const wrapper = await mountCrashModal('export', 'Export test')

    await actionButton(wrapper, 'Export diagnostic package').trigger('click')
    await vi.waitFor(() => expect(wrapper.text()).toContain('Diagnostic package saved to'))
    expect(calls.exportCrash).toHaveBeenCalledExactlyOnceWith('export', 'Export test')
    expect(wrapper.get('[role="status"]').text()).toContain('C:/exports/crash.zip')

    await wrapper.get('[aria-label="Open in folder"]').trigger('click')
    await vi.waitFor(() =>
        expect(calls.highlight).toHaveBeenCalledExactlyOnceWith('C:/exports/crash.zip'),
    )
})

it('does not report an export when the save dialog is cancelled', async () => {
    calls.exportCrash.mockResolvedValue(null)
    const wrapper = await mountCrashModal('cancel')

    const button = actionButton(wrapper, 'Export diagnostic package')
    await button.trigger('click')
    await vi.waitFor(() => expect(calls.exportCrash).toHaveBeenCalledOnce())
    expect(wrapper.find('[role="status"]').exists()).toBe(false)
    expect(button.attributes('aria-busy')).toBeUndefined()
    expect(button.attributes('disabled')).toBeUndefined()
})

it('surfaces export failures and lets the user retry', async () => {
    calls.exportCrash.mockRejectedValueOnce(new Error('Archive failed'))
    const wrapper = await mountCrashModal('failure')

    await actionButton(wrapper, 'Export diagnostic package').trigger('click')
    await vi.waitFor(() => expect(calls.handleError).toHaveBeenCalledOnce())
    expect(wrapper.find('[role="status"]').exists()).toBe(false)
    const button = actionButton(wrapper, 'Export diagnostic package')
    expect(button.attributes('aria-busy')).toBeUndefined()

    calls.exportCrash.mockResolvedValueOnce('C:/exports/retry.zip')
    await button.trigger('click')
    await vi.waitFor(() => expect(wrapper.text()).toContain('C:/exports/retry.zip'))
    expect(calls.exportCrash).toHaveBeenCalledTimes(2)
})

it('does not allow exporting from the crash window preview', async () => {
    const wrapper = await mountCrashModal('preview-source')
    wrapper.vm.showPreview()
    await nextTick()

    const button = actionButton(wrapper, 'Export diagnostic package')
    expect(button.attributes('disabled')).toBeDefined()
    await button.trigger('click')
    expect(calls.exportCrash).not.toHaveBeenCalled()
})

it('ignores an export that finishes after another crash replaces the window', async () => {
    let resolveExport!: (path: string) => void
    calls.exportCrash.mockImplementationOnce(
        () =>
            new Promise<string>((resolve) => {
                resolveExport = resolve
            }),
    )
    const wrapper = await mountCrashModal('stale')

    await actionButton(wrapper, 'Export diagnostic package').trigger('click')
    await vi.waitFor(() => expect(calls.exportCrash).toHaveBeenCalledOnce())
    await wrapper.vm.handleWarning({
        kind: 'minecraft_crash',
        instance_id: 'new-crash',
        instance_name: 'New crash',
    })
    resolveExport('C:/exports/stale.zip')
    await vi.waitFor(() =>
        expect(
            actionButton(wrapper, 'Export diagnostic package').attributes('aria-busy'),
        ).toBeUndefined(),
    )
    expect(wrapper.find('[role="status"]').exists()).toBe(false)
})

it('hides LogAgent when local analysis is not configured and preserves crash actions', async () => {
    calls.enabled = false
    const wrapper = await mountCrashModal('unconfigured')
    expect(wrapper.find('[data-testid="crash-analysis-controls"]').exists()).toBe(false)
    expect(wrapper.text()).not.toContain('Cloud LogShare')
    expect(wrapper.text()).not.toContain('Configure analysis')
    expect(actionButton(wrapper, 'Export diagnostic package').exists()).toBe(true)
    expect(actionButton(wrapper, 'Share diagnostic').exists()).toBe(true)
    await wrapper.vm.openAIAnalysis()
    expect(calls.prepare).not.toHaveBeenCalled()
    expect(wrapper.find('.crash-modal-ai-output').exists()).toBe(false)
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
    expect(controls.findAll('[role="radio"]')).toHaveLength(0)
    expect(controls.text()).not.toContain('Cloud LogShare')
})

it('uses the local model when the saved default is cloud', async () => {
    calls.source = 'logshare'
    const wrapper = await mountCrashModal('cloud-default')
    const controls = wrapper.get('[data-testid="crash-analysis-controls"]')
    expect(controls.text()).toContain('Local LogAgent')
    expect(controls.text()).not.toContain('Cloud LogShare')
})

it('loads structured LogShare analysis for the selected file without a LogAgent entry', async () => {
    calls.enabled = false
    calls.source = 'logshare'
    calls.autoUpload = true
    calls.upload.mockResolvedValue({
        id: 'sAbCdEf',
        url: 'https://logshare.cn/sAbCdEf',
        raw: 'raw',
        token: '',
        files: [{ name: 'latest.log' }, { name: 'crash-reports/crash.txt' }],
    })
    calls.insights.mockImplementation(async (_id: string, filename: string) => ({
        analysis: {
            problems: [
                { message: filename, solutions: [{ message: 'Install the missing dependency' }] },
            ],
        },
    }))
    calls.logs.mockResolvedValue([
        { filename: 'latest.log', log_type: 'InfoLog', age: 1000 },
        { filename: 'crash.txt', log_type: 'CrashReport', age: 1000 },
    ])
    const router = createRouter({
        history: createMemoryHistory(),
        routes: [{ path: '/', component: { render: () => null } }],
    })
    await router.push('/')
    const wrapper = mount(MinecraftCrashModal, {
        global: { directives: { tooltip: () => {} }, plugins: [router] },
    })
    cleanup.push(() => wrapper.unmount())
    await wrapper.vm.handleWarning({
        kind: 'minecraft_crash',
        instance_id: 'summary',
        instance_name: 'Test',
    })
    await vi.waitFor(() =>
        expect(calls.insights).toHaveBeenCalledWith('sAbCdEf', 'crash-reports/crash.txt'),
    )
    await vi.waitFor(() => expect(wrapper.text()).toContain('Install the missing dependency'))
    expect(wrapper.text()).not.toContain('[object Object]')
    expect(wrapper.find('[data-testid="crash-analysis-controls"]').exists()).toBe(false)
    await wrapper
        .findAll('button')
        .find((button) => button.text() === 'latest.log')!
        .trigger('click')
    await vi.waitFor(() => expect(calls.insights).toHaveBeenLastCalledWith('sAbCdEf', 'latest.log'))
    expect(calls.upload).toHaveBeenCalledTimes(1)
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

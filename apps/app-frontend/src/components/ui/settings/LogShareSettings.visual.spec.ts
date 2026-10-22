import '@/test/visual-styles.scss'

import { mount } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import { page } from 'vitest/browser'
import { createMemoryHistory, createRouter } from 'vue-router'

import chinese from '@/locales/zh-CN/index.json'

import LogShareSettings from './LogShareSettings.vue'

const calls = vi.hoisted(() => ({
    saveAI: vi.fn(),
    saveShare: vi.fn(),
    handleError: vi.fn(),
    settings: {
        enabled: true,
        provider_id: 'codeflow',
        model_id: 'gpt-6-luna',
        ai_source: 'custom',
    },
    chinese: false,
}))

vi.mock('@modrinth/ui', async () => ({
    Button: (await import('../../../../../../packages/ui/src/components/base/buttons/Button.vue'))
        .default,
    Card: (await import('../../../../../../packages/ui/src/components/base/Card.vue')).default,
    Toggle: (await import('../../../../../../packages/ui/src/components/base/Toggle.vue')).default,
    Combobox: (await import('../../../../../../packages/ui/src/components/base/Combobox.vue'))
        .default,
    defineMessages: (messages: unknown) => messages,
    useVIntl: () => ({
        formatMessage: (message: { id: string; defaultMessage: string }) =>
            calls.chinese
                ? ((chinese as Record<string, { message: string }>)[message.id]?.message ??
                  message.defaultMessage)
                : message.defaultMessage,
    }),
    injectNotificationManager: () => ({ handleError: calls.handleError }),
}))
vi.mock('@/helpers/ai', async () => {
    const { ref } = await import('vue')
    const state = {
        settings: { enabled: true },
        catalog_source: '',
        providers: [
            {
                provider_id: 'codeflow',
                custom_name: 'Codeflow',
                enabled: true,
                models: [{ id: 'gpt-6-luna', name: 'gpt-6-luna', enabled: true }],
            },
        ],
    }
    return { sharedAIState: ref(state), getAIState: async () => state }
})
vi.mock('@/helpers/logs', () => ({
    get_crash_analysis_ai_settings: async () => ({ ...calls.settings }),
    get_log_share_settings: async () => ({
        share_provider: 'logshare',
        ai_source: 'custom',
        auto_upload: true,
        multi_file: true,
        no_storage: false,
        show_progress: true,
    }),
    update_crash_analysis_ai_settings: calls.saveAI,
    update_log_share_settings: calls.saveShare,
}))

const wrappers: ReturnType<typeof mount>[] = []
afterEach(() => {
    wrappers.splice(0).forEach((wrapper) => wrapper.unmount())
    vi.clearAllMocks()
    calls.chinese = false
    calls.settings.ai_source = 'custom'
})

async function render(width = 900, localSetup = false) {
    if (!document.querySelector('#teleports')) {
        const teleports = document.createElement('div')
        teleports.id = 'teleports'
        document.body.append(teleports)
    }
    const router = createRouter({
        history: createMemoryHistory(),
        routes: [{ path: '/settings', component: { render: () => null } }],
    })
    await router.push('/settings#launch-defaults')
    document.documentElement.classList.add('dark-mode')
    const wrapper = mount(LogShareSettings, {
        props: { localSetup },
        attachTo: document.body,
        global: { plugins: [router], provide: { replayOnboarding: async () => {} } },
    })
    wrappers.push(wrapper)
    await vi.waitFor(() =>
        expect(
            wrapper
                .find(localSetup ? '#crash-analysis-ai-enabled' : '[role="radiogroup"]')
                .exists(),
        ).toBe(true),
    )
    wrapper.element.setAttribute(
        'style',
        `width: min(${width}px, 100vw); padding: 24px; background: var(--surface-2)`,
    )
    return wrapper
}

it('shows only relevant analysis settings and saves source selection once', async () => {
    const wrapper = await render()
    expect(wrapper.findAll('#settings-target-log-share')).toHaveLength(1)
    expect(wrapper.find('#crash-analysis-ai-model').exists()).toBe(true)
    expect(wrapper.find('#log-share-no-storage').exists()).toBe(false)
    expect(wrapper.find('#log-share-provider').exists()).toBe(true)
    expect(wrapper.find('#log-share-show-progress').exists()).toBe(true)
    await wrapper.get('button[value="logshare"]').trigger('click')
    await vi.waitFor(() => expect(wrapper.find('#log-share-no-storage').exists()).toBe(true))
    expect(calls.saveAI).toHaveBeenCalledTimes(1)
    expect(calls.saveAI.mock.calls[0][0].ai_source).toBe('logshare')
    expect(wrapper.find('#crash-analysis-ai-model').exists()).toBe(false)
    expect(wrapper.find('#log-share-provider').exists()).toBe(true)
})

it('retains the selected source when saving fails', async () => {
    calls.saveAI.mockRejectedValueOnce(new Error('save failed'))
    const wrapper = await render()
    await wrapper.get('button[value="logshare"]').trigger('click')
    await vi.waitFor(() => expect(calls.handleError).toHaveBeenCalled())
    expect(wrapper.get('button[value="custom"]').attributes('aria-checked')).toBe('true')
    expect(wrapper.find('#crash-analysis-ai-model').exists()).toBe(true)
})

it('configures local analysis from a cloud default without changing the saved source', async () => {
    calls.settings.ai_source = 'logshare'
    const wrapper = await render(600, true)
    expect(wrapper.find('#settings-target-log-share').exists()).toBe(false)
    expect(wrapper.find('[role="radiogroup"]').exists()).toBe(false)
    expect(wrapper.find('#crash-analysis-ai-model').exists()).toBe(true)
    expect(wrapper.find('#log-share-provider').exists()).toBe(false)
    await wrapper.get('#crash-analysis-ai-enabled').trigger('click')
    await vi.waitFor(() => expect(calls.saveAI).toHaveBeenCalledTimes(1))
    expect(calls.saveAI.mock.calls[0][0].ai_source).toBe('logshare')
    expect(wrapper.emitted('savingChange')?.[0]).toEqual([true])
})

it('keeps source choices readable at desktop and narrow widths', async () => {
    await page.viewport(1200, 1400)
    calls.chinese = !!import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE
    const wrapper = await render()
    if (import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE)
        await page.screenshot({
            element: wrapper.element,
            path: `${import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE}/logagent-settings-desktop.png`,
        })
    await page.viewport(390, 1500)
    wrapper.element.setAttribute(
        'style',
        'width: min(390px, 100vw); padding: 16px; background: var(--surface-2)',
    )
    for (const radio of wrapper.findAll('[role="radio"]'))
        expect(radio.element.scrollWidth).toBeLessThanOrEqual(radio.element.clientWidth + 1)
    if (import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE)
        await page.screenshot({
            element: wrapper.element,
            path: `${import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE}/logagent-settings-narrow.png`,
        })
})

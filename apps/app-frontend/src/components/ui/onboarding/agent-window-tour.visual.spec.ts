import { mount } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import { defineComponent, h, nextTick, ref } from 'vue'
import { createMemoryHistory, createRouter } from 'vue-router'

import AgentWindowButton from '@/features/agent-window/AgentWindowButton.vue'

import { type OnboardingMode, onboardingTours } from './onboardingConfig'
import { useOnboardingTour } from './useOnboardingTour'

const invoke = vi.fn().mockResolvedValue(undefined)
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...args: unknown[]) => invoke(...args) }))
vi.mock('@modrinth/ui', () => ({
    defineMessages: (messages: unknown) => messages,
    injectNotificationManager: () => ({ handleError: vi.fn() }),
    useVIntl: () => ({
        formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
        locale: ref('en-US'),
    }),
}))

const cleanup: (() => void)[] = []
afterEach(() => {
    cleanup
        .splice(0)
        .reverse()
        .forEach((dispose) => dispose())
    invoke.mockClear()
})

async function tour(missing = false) {
    const visible = ref(false)
    const mode = ref<OnboardingMode>('main')
    const agentStep = onboardingTours.main.findIndex(
        (step) => step.targetId === 'agent-window-open',
    )
    expect(agentStep).toBeGreaterThan(-1)
    const router = createRouter({
        history: createMemoryHistory(),
        routes: [{ path: '/', component: { render: () => null } }],
    })
    await router.push('/')
    let runtime: ReturnType<typeof useOnboardingTour>
    const component = defineComponent({
        setup() {
            runtime = useOnboardingTour(visible, mode, {
                complete: () => {},
                skip: () => {},
                closeSettings: () => {},
            })
            return () => h('div', missing ? [] : [h(AgentWindowButton)])
        },
    })
    const wrapper = mount(component, { attachTo: document.body, global: { plugins: [router] } })
    cleanup.push(() => wrapper.unmount())
    const replay = async () => {
        visible.value = false
        await nextTick()
        visible.value = true
        await nextTick()
        runtime.stepIndex.value = agentStep
        await nextTick()
    }
    await replay()
    return { wrapper, runtime: runtime!, agentStep, replay }
}

it('first-run and replay identify the agent entry without opening its separate window', async () => {
    const { wrapper, runtime, agentStep, replay } = await tour()
    for (let run = 0; run < 2; run++) {
        if (run) await replay()
        expect(runtime.targetRect.value?.width).toBeGreaterThan(0)
        wrapper
            .get('[data-onboarding-id="agent-window-open"]')
            .element.dispatchEvent(new MouseEvent('click', { bubbles: true }))
        await nextTick()
        expect(runtime.stepIndex.value).toBe(agentStep + 1)
        await new Promise((resolve) => window.setTimeout(resolve, 200))
    }
    expect(invoke).not.toHaveBeenCalled()
})

it('skips a missing agent entry when replaying a customized launcher', async () => {
    const { runtime, agentStep } = await tour(true)
    await vi.waitFor(() => expect(runtime.stepIndex.value).toBeGreaterThan(agentStep), {
        timeout: 4_000,
    })
})

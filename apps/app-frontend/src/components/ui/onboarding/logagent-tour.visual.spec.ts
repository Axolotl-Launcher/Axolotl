import { mount } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import { defineComponent, h, nextTick, ref } from 'vue'
import { createMemoryHistory, createRouter } from 'vue-router'

import type { OnboardingMode } from './onboardingConfig'
import { useOnboardingTour } from './useOnboardingTour'

vi.mock('@modrinth/ui', () => ({ defineMessages: (messages: unknown) => messages }))

const cleanup: (() => void)[] = []
afterEach(() =>
    cleanup
        .splice(0)
        .reverse()
        .forEach((dispose) => dispose()),
)

async function tour(missing = false) {
    const visible = ref(false)
    const mode = ref<OnboardingMode>('logagent')
    const complete = vi.fn()
    const router = createRouter({
        history: createMemoryHistory(),
        routes: [{ path: '/settings', component: { render: () => null } }],
    })
    await router.push('/settings#launch-defaults')
    const component = defineComponent({
        setup() {
            useOnboardingTour(visible, mode, { complete, skip: () => {}, closeSettings: () => {} })
            return () =>
                h(
                    'div',
                    missing
                        ? []
                        : ['source', 'sharing', 'cloud'].map((name) =>
                              h(
                                  'div',
                                  {
                                      'data-onboarding-id': `logagent-settings-${name}`,
                                      style: 'width: 320px; height: 60px',
                                  },
                                  name,
                              ),
                          ),
                )
        },
    })
    const wrapper = mount(component, { attachTo: document.body, global: { plugins: [router] } })
    cleanup.push(() => wrapper.unmount())
    const replay = async () => {
        visible.value = false
        await nextTick()
        visible.value = true
        await nextTick()
    }
    await replay()
    return { wrapper, complete, replay }
}

it('the contextual analysis tour can be replayed and inspect clicks do not activate settings', async () => {
    const { wrapper, complete, replay } = await tour()
    const originalAction = vi.fn()
    const target = wrapper.element.querySelector('[data-onboarding-id]')!
    target.addEventListener('click', originalAction)
    for (let run = 0; run < 2; run++) {
        if (run) await replay()
        for (let index = 0; index < 3; index++) {
            target.dispatchEvent(new MouseEvent('click', { bubbles: true }))
            await nextTick()
            await new Promise((resolve) => window.setTimeout(resolve, 200))
        }
        expect(complete).toHaveBeenCalledTimes(run + 1)
    }
    expect(originalAction).not.toHaveBeenCalled()
})

it('missing optional targets eventually complete instead of trapping the tour', async () => {
    const { complete } = await tour(true)
    await vi.waitFor(() => expect(complete).toHaveBeenCalledOnce(), {
        timeout: 12_000,
        interval: 100,
    })
}, 15_000)

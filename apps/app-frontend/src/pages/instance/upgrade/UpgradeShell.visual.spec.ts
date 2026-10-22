import { flushPromises, mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { h } from 'vue'

import type { GameInstance } from '@/helpers/types'

import UpgradeShell from './UpgradeShell.vue'
const backend = vi.hoisted(() => ({
    getPlan: vi.fn(),
    discard: vi.fn(),
    replace: vi.fn(),
    parked: false,
}))
vi.mock('@/helpers/instance-upgrade', () => ({
    get_instance_upgrade_plan: backend.getPlan,
    discard_instance_upgrade_plan: backend.discard,
}))
vi.mock('./install-job', () => ({
    recoverInstanceUpgradeJob: async () => null,
    isRecoverableUpgradeStatus: () => false,
}))
vi.mock('./UpgradeFlowFloatingBar.vue', () => ({ default: { render: () => null } }))
vi.mock('@/store/navigation-return', () => ({
    restoreUpgradeFlow: (_id: string, _path: string, hydrate: (snapshot: unknown) => void) => {
        const snapshot = { plan: { id: 'cached-plan' } }
        hydrate(snapshot)
        return snapshot
    },
    peekUpgradeFlow: () => (backend.parked ? { plan: { id: 'cached-plan' } } : null),
    parkUpgradeFlow: () => {},
}))
vi.mock('./flow', async () => {
    const { ref } = await import('vue')
    return {
        provideInstanceUpgradeFlow: () => {
            const plan = ref<{ id: string } | null>(null)
            return {
                plan,
                activeJobId: ref(null),
                jobRecoveryState: ref('idle'),
                result: ref(null),
                setPlan: (next: { id: string }) => {
                    plan.value = next
                },
                clearPlan: () => {
                    plan.value = null
                },
                hydrate: (snapshot: { plan: { id: string } }) => {
                    plan.value = snapshot.plan
                },
                setJobRecoveryState: () => {},
            }
        },
        isUpgradeRouteAvailable: (_requirement: unknown, flow: { plan: { value: unknown } }) =>
            Boolean(flow.plan.value),
        isUpgradeRouteRecoveryPending: () => false,
        attachUpgradeJobToFlow: () => {},
    }
})
vi.mock('vue-router', async () => {
    const { defineComponent, h } = await import('vue')
    return {
        RouterView: defineComponent({ setup: () => () => h('div', { 'data-upgrade-page': '' }) }),
        useRoute: () => ({
            params: { id: 'instance' },
            path: '/instance/instance/upgrade/compatibility',
            fullPath: '/instance/instance/upgrade/compatibility',
            meta: { upgradeRequirement: 'plan' },
        }),
        useRouter: () => ({ replace: backend.replace }),
        onBeforeRouteLeave: () => {},
    }
})
it('checks a parked plan before displaying controls and returns expired plans to selection', async () => {
    let reject!: (error: Error) => void
    backend.getPlan.mockReturnValue(
        new Promise((_, fail) => {
            reject = fail
        }),
    )
    backend.discard.mockResolvedValue(undefined)
    backend.replace.mockReset()
    const wrapper = mount(UpgradeShell, {
        props: { instance: { id: 'instance' } as GameInstance },
        global: { stubs: { RouterView: { render: () => h('div', { 'data-upgrade-page': '' }) } } },
    })
    try {
        expect(wrapper.find('[data-upgrade-page]').exists()).toBe(false)
        reject(new Error('plan expired'))
        await flushPromises()
        expect(backend.replace).toHaveBeenCalledWith('/instance/instance/upgrade')
    } finally {
        wrapper.unmount()
    }
})
it('keeps the returned plan alive when it is parked for project navigation', async () => {
    backend.parked = true
    backend.getPlan.mockResolvedValue({ id: 'cached-plan' })
    backend.discard.mockReset().mockResolvedValue(undefined)
    const wrapper = mount(UpgradeShell, {
        props: { instance: { id: 'instance' } as GameInstance },
        global: { stubs: { RouterView: { render: () => h('div', { 'data-upgrade-page': '' }) } } },
    })
    await flushPromises()
    expect(wrapper.find('[data-upgrade-page]').exists()).toBe(true)
    wrapper.unmount()
    expect(backend.discard).not.toHaveBeenCalled()
    backend.parked = false
})

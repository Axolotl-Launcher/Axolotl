import { VueQueryPlugin } from '@tanstack/vue-query'
import { flushPromises, mount } from '@vue/test-utils'
import { IntlMessageFormat } from 'intl-messageformat'
import { expect, it, vi } from 'vitest'
import { defineComponent, h } from 'vue'

import type {
    InstanceUpgradePlan,
    InstanceUpgradePlanningProgress,
} from '@/helpers/instance-upgrade'

import Select from './Select.vue'

type Options = {
    signal?: AbortSignal
    onProgress?: (progress: InstanceUpgradePlanningProgress) => void
}
const backend = vi.hoisted(() => ({
    plan: vi.fn(),
    controls: null as { onNext: () => Promise<void> | void } | null,
    flow: null as unknown as { busy: { value: boolean } },
}))

vi.mock('@/helpers/instance-upgrade', () => ({
    plan_instance_upgrade: backend.plan,
    get_instance_upgrade_plan: vi.fn(),
    discard_instance_upgrade_plan: vi.fn(async () => {}),
}))
vi.mock('@/helpers/instance-content', () => ({
    loadInstanceContentData: async () => ({ snapshot: { items: [{}, {}] } }),
}))
vi.mock('@/helpers/metadata', () => ({ get_loader_versions: vi.fn() }))
vi.mock('@/helpers/tags', () => ({
    get_game_versions: async () => [
        { version: '1.21.5', version_type: 'release' },
        { version: '1.21.4', version_type: 'release' },
    ],
}))
vi.mock('@modrinth/assets', () => ({ SpinnerIcon: { render: () => null } }))
vi.mock('vue-router', () => ({
    useRoute: () => ({ name: 'InstanceUpgrade', params: { id: 'instance' } }),
    useRouter: () => ({ push: vi.fn() }),
}))
vi.mock('./flow', async () => {
    const { ref } = await import('vue')
    const flow = {
        instance: ref({
            id: 'instance',
            game_version: '1.21.4',
            loader: 'vanilla',
            loader_version: null,
            loader_components: [],
        }),
        instanceId: ref('instance'),
        targetEnvironment: ref(null),
        busy: ref(false),
        error: ref(null),
        plan: ref(null),
        setPlan: vi.fn(),
        clearPlan: vi.fn(),
        setTargetEnvironment: vi.fn(),
        registerStepControls: (controls: { onNext: () => void } | null) => {
            if (controls) backend.controls = controls
        },
    }
    backend.flow = flow
    return { useInstanceUpgradeFlow: () => flow }
})
vi.mock('@modrinth/ui', () => ({
    Admonition: defineComponent({
        setup:
            (_, { slots }) =>
            () =>
                h('aside', slots.default?.()),
    }),
    Card: defineComponent({
        setup:
            (_, { slots }) =>
            () =>
                h('div', slots.default?.()),
    }),
    Combobox: { render: () => null },
    Button: defineComponent({
        setup:
            (_, { attrs, slots }) =>
            () =>
                h('button', attrs, slots.default?.()),
    }),
    defineMessages: (messages: unknown) => messages,
    formatLoaderLabel: (loader: string) => loader,
    loaderVersionsForGameVersion: () => [],
    scopedLoaderMetadataQueryKey: (...parts: string[]) => parts,
    useVIntl: () => ({
        formatMessage: (message: { defaultMessage: string }, values?: Record<string, unknown>) =>
            new IntlMessageFormat(message.defaultMessage, 'en').format(values) as string,
    }),
}))

async function startPlanning() {
    let options!: Options
    let finish!: (plan: InstanceUpgradePlan) => void
    backend.plan.mockImplementationOnce((_id: string, _target: unknown, value: Options) => {
        options = value
        return new Promise((resolve) => {
            finish = resolve
        })
    })
    const wrapper = mount(defineComponent({ setup: () => () => h(Select) }), {
        global: { plugins: [VueQueryPlugin] },
    })
    await vi.waitFor(() => expect(backend.controls).not.toBeNull())
    await flushPromises()
    void backend.controls!.onNext()
    await flushPromises()
    expect(backend.plan).toHaveBeenCalled()
    return { wrapper, options, finish: (plan: InstanceUpgradePlan) => finish(plan) }
}

it('shows each item as the planner checks it and stops the planner from the Stop button', async () => {
    backend.plan.mockReset()
    const { wrapper, options } = await startPlanning()
    try {
        options.onProgress!({ kind: 'phase', phase: 'reading_content', completed: 1, total: 2 })
        options.onProgress!({
            kind: 'items',
            items: [
                {
                    contentId: 'a',
                    relativePath: 'mods/alpha.jar',
                    projectType: 'mod',
                    provider: 'modrinth',
                    projectId: 'alpha',
                    status: null,
                },
                {
                    contentId: 'b',
                    relativePath: 'mods/beta.jar',
                    projectType: 'mod',
                    provider: 'modrinth',
                    projectId: 'beta',
                    status: null,
                },
                {
                    contentId: 'c',
                    relativePath: 'shaderpacks/gamma.zip',
                    projectType: 'shaderpack',
                    provider: 'modrinth',
                    projectId: 'gamma',
                    status: null,
                },
            ],
        })
        options.onProgress!({
            kind: 'candidates',
            provider: 'modrinth',
            projectId: 'gamma',
            stable: 1,
        })
        options.onProgress!({
            kind: 'candidates',
            provider: 'modrinth',
            projectId: 'alpha',
            stable: 2,
        })
        await new Promise(requestAnimationFrame)
        await flushPromises()
        const rows = wrapper.findAll('li').map((row) => row.text())
        expect(rows).toEqual([
            expect.stringContaining('Compatible release found'),
            expect.stringContaining('Checking…'),
            expect.stringContaining('Checking…'),
        ])
        expect(wrapper.text()).toContain('Reading content files (1/2)')

        await wrapper
            .findAll('button')
            .find((button) => button.text() === 'Stop analysis')!
            .trigger('click')
        await flushPromises()
        expect(options.signal!.aborted).toBe(true)
        expect(backend.flow.busy.value).toBe(false)
        expect(wrapper.find('[data-upgrade-planning]').exists()).toBe(false)
    } finally {
        wrapper.unmount()
    }
})

it('stops the planner when the page is left during analysis', async () => {
    backend.plan.mockReset()
    const { wrapper, options } = await startPlanning()
    expect(options.signal!.aborted).toBe(false)
    wrapper.unmount()
    expect(options.signal!.aborted).toBe(true)
})

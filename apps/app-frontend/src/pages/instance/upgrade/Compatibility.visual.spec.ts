import { VueQueryPlugin } from '@tanstack/vue-query'
import { flushPromises, mount } from '@vue/test-utils'
import { IntlMessageFormat } from 'intl-messageformat'
import { expect, it, vi } from 'vitest'
import { defineComponent, h, ref, unref } from 'vue'

import type {
    InstanceUpgradeIssue,
    InstanceUpgradePlan,
    InstanceUpgradeResolution,
} from '@/helpers/instance-upgrade'
import type { GameInstance } from '@/helpers/types'

import Compatibility from './Compatibility.vue'
import Customize from './Customize.vue'
import { isUpgradeRouteAvailable, provideInstanceUpgradeFlow } from './flow'

const backend = vi.hoisted(() => ({ update: vi.fn(), resolveCustom: vi.fn(), push: vi.fn() }))
vi.mock('@/helpers/instance-upgrade', () => ({
    update_instance_upgrade_resolution: backend.update,
    update_instance_upgrade_resolutions: vi.fn(),
    reset_instance_upgrade_resolution: vi.fn(),
    resolve_custom_instance_upgrade_solution: backend.resolveCustom,
    select_instance_upgrade_solution: vi.fn(),
}))
vi.mock('@/helpers/instance-content', () => ({
    loadInstanceContentData: async () => ({ snapshot: { items: [] } }),
    localContentIconUrl: () => '',
}))
vi.mock('@/helpers/upgrade-version-metadata', () => ({
    loadUpgradeVersionDisplayMetadata: async () => new Map(),
    upgradeVersionCacheKey: (...parts: string[]) => parts.join(':'),
    upgradeVersionDisplayLabel: (_metadata: unknown, release: { releaseId: string }) =>
        release.releaseId,
}))
vi.mock('@/store/navigation-return', () => ({
    parkUpgradeFlow: vi.fn(),
    upgradeProjectPath: () => null,
}))
vi.mock('vue-router', () => ({ useRouter: () => ({ push: backend.push }) }))
vi.mock('./UpgradeVersionChangelogPopout.vue', () => ({ default: { render: () => null } }))
vi.mock('@modrinth/ui', () => ({
    Accordion: defineComponent({
        setup:
            (_, { slots }) =>
            () =>
                h('section', [slots.title?.(), slots.default?.()]),
    }),
    Admonition: defineComponent({
        props: ['header'],
        setup:
            (props, { slots }) =>
            () =>
                h('aside', [props.header, slots.default?.()]),
    }),
    Avatar: { render: () => null },
    Button: defineComponent({
        setup:
            (_, { attrs, slots }) =>
            () =>
                h('button', attrs, slots.default?.()),
    }),
    Combobox: defineComponent({
        props: {
            modelValue: String,
            options: Array<{ label: string; value: string }>,
        },
        emits: ['update:modelValue'],
        setup:
            (props, { emit }) =>
            () =>
                h(
                    'select',
                    {
                        value: props.modelValue,
                        onChange: (event: Event) =>
                            emit('update:modelValue', (event.target as HTMLSelectElement).value),
                    },
                    props.options?.map(({ label, value }) => h('option', { value }, label)),
                ),
    }),
    defineMessages: (messages: unknown) => messages,
    formatLoaderLabel: (loader: string) => loader,
    useVIntl: () => ({
        formatMessage: (message: { defaultMessage: string }, values?: Record<string, unknown>) =>
            new IntlMessageFormat(message.defaultMessage, 'en').format(values) as string,
    }),
}))

function prereleaseIssue(projectId: string, root: boolean): InstanceUpgradeIssue {
    return {
        code: 'prerelease_only',
        message: 'Prerelease confirmation required',
        contentId: root ? 'root' : null,
        provider: 'modrinth',
        projectId,
        conflictingProjectId: null,
        dependencyRequirements: root
            ? []
            : [
                  {
                      rootContentId: 'root',
                      rootProvider: 'modrinth',
                      rootProjectId: 'root',
                      parentProvider: 'modrinth',
                      parentProjectId: 'root',
                      parentReleaseId: 'root-beta',
                      dependencyProvider: 'modrinth',
                      dependencyProjectId: projectId,
                      requiredReleaseId: null,
                      candidateReleaseId: `${projectId}-beta`,
                  },
              ],
    }
}

function plan(
    resolution: InstanceUpgradeResolution,
    blockingIssues: InstanceUpgradeIssue[],
): InstanceUpgradePlan {
    const environment = {
        gameVersion: '26.3',
        modLoader: 'fabric' as const,
        modLoaderVersion: null,
        shaderRuntime: 'none' as const,
    }
    return {
        id: 'plan',
        instanceId: 'instance',
        sourceRevision: 1,
        sourceFiles: [],
        sourceEnvironment: { ...environment, gameVersion: '26.2' },
        targetEnvironment: environment,
        items: [
            {
                contentId: 'root',
                relativePath: 'mods/root.jar',
                projectType: 'mod',
                provider: 'modrinth',
                projectId: 'root',
                currentReleaseId: 'old',
                currentEnabled: true,
                autoDependency: false,
                status: 'prerelease_only',
                resolution,
                candidateReleaseIds: ['root-beta'],
            },
        ],
        dependencyChanges: [],
        warnings: [],
        blockingIssues,
        newestSolution: null,
        minimalChangeSolution: null,
        selectedSolution: null,
        customConstraints: [],
    }
}

it('confirms newly exposed prerelease dependencies after allowing the root and then continues', async () => {
    backend.update.mockReset()
    backend.push.mockReset()
    const resolution: InstanceUpgradeResolution = {
        contentId: 'root',
        action: 'upgrade',
        allowPrerelease: false,
        confirmedPrereleaseDependencies: [],
    }
    const initial = plan(resolution, [prereleaseIssue('root', true)])
    let flow!: ReturnType<typeof provideInstanceUpgradeFlow>
    const wrapper = mount(
        defineComponent({
            setup() {
                flow = provideInstanceUpgradeFlow(ref({ id: 'instance' } as GameInstance))
                flow.setPlan(initial)
                return () => h(Compatibility)
            },
        }),
        { global: { plugins: [VueQueryPlugin], stubs: { RouterLink: true } } },
    )
    try {
        await flushPromises()
        const allowButton = () =>
            wrapper.findAll('button').find((button) => button.text().includes('Allow prerelease'))!
        const canContinue = () => unref(flow.controls.value!.canNext)
        expect(canContinue()).toBe(false)

        // Each solver response can reveal a further transitive dependency.
        for (const nextDependency of ['first', 'second', null]) {
            backend.update.mockImplementationOnce(
                async (_id: string, next: InstanceUpgradeResolution) =>
                    plan(next, nextDependency ? [prereleaseIssue(nextDependency, false)] : []),
            )
            expect(allowButton().classes()).not.toContain('text-green')
            await allowButton().trigger('click')
            await flushPromises()
            expect(canContinue()).toBe(nextDependency === null)
            if (nextDependency) {
                expect(wrapper.text()).not.toContain('Instance-wide issue')
                expect(wrapper.text()).toContain('Resolved 0/1')
            }
        }
        expect(backend.update).toHaveBeenCalledTimes(3)
        expect(backend.update.mock.calls[2][1].confirmedPrereleaseDependencies).toEqual([
            { provider: 'modrinth', projectId: 'first', versionId: 'first-beta' },
            { provider: 'modrinth', projectId: 'second', versionId: 'second-beta' },
        ])
        expect(allowButton().classes()).toContain('text-green')
        await allowButton().trigger('click')
        await flushPromises()
        expect(backend.update).toHaveBeenCalledTimes(3)
        await flow.controls.value!.onNext()
        expect(backend.push).toHaveBeenCalledWith('/instance/instance/upgrade/customize')
    } finally {
        wrapper.unmount()
    }
})

it('lets an inconclusive search reach version choices and verifies a revised plan before confirmation', async () => {
    backend.push.mockReset()
    backend.resolveCustom.mockReset()
    const resolution: InstanceUpgradeResolution = {
        contentId: 'root',
        action: 'upgrade',
        allowPrerelease: false,
        confirmedPrereleaseDependencies: [],
    }
    const limit: InstanceUpgradeIssue = {
        code: 'search_limit_reached',
        message: '',
        contentId: null,
        provider: null,
        projectId: null,
        conflictingProjectId: null,
        dependencyRequirements: [],
    }
    const initial = plan(resolution, [limit])
    initial.items[0].status = 'upgrade_available'
    initial.items[0].candidateReleaseIds = ['one', 'two']
    const customizing = ref(false)
    let flow!: ReturnType<typeof provideInstanceUpgradeFlow>
    backend.push.mockImplementationOnce(async () => {
        customizing.value = isUpgradeRouteAvailable('customize', flow)
    })
    const wrapper = mount(
        defineComponent({
            setup() {
                flow = provideInstanceUpgradeFlow(ref({ id: 'instance' } as GameInstance))
                flow.setPlan(initial)
                return () => h(customizing.value ? Customize : Compatibility)
            },
        }),
        { global: { plugins: [VueQueryPlugin], stubs: { RouterLink: true } } },
    )

    try {
        await flushPromises()
        expect(flow.controls.value?.nextLabel).toBe('Choose versions')
        expect(unref(flow.controls.value?.canNext)).toBe(true)
        expect(wrapper.text()).toContain('No compatible set of versions has been verified yet.')
        await flow.controls.value?.onNext()
        await flushPromises()
        expect(customizing.value).toBe(true)
        expect(unref(flow.controls.value?.canNext)).toBe(false)

        const button = () =>
            wrapper.findAll('button').find((item) => item.text().includes('Apply custom choices'))!
        expect(button().attributes('disabled')).toBeDefined()
        const candidate = (versionId: string) => ({
            contentId: 'root',
            provider: 'modrinth',
            projectId: 'root',
            versionId,
        })
        const conflict = {
            ...limit,
            code: 'incompatible_dependency' as const,
            message: 'Selected versions conflict',
        }
        backend.resolveCustom.mockResolvedValueOnce({
            ...initial,
            blockingIssues: [conflict],
            customConstraints: [candidate('one')],
        })
        await wrapper.get('select').setValue('one')
        await button().trigger('click')
        await flushPromises()
        expect(backend.resolveCustom).toHaveBeenLastCalledWith(initial.id, [candidate('one')])
        expect(isUpgradeRouteAvailable('customize', flow)).toBe(true)
        expect(unref(flow.controls.value?.canNext)).toBe(false)

        backend.resolveCustom.mockResolvedValueOnce({
            ...initial,
            blockingIssues: [],
            customConstraints: [candidate('two')],
            selectedSolution: {
                kind: 'custom',
                selections: [],
                dependencyChanges: [],
                warnings: [],
            },
        })
        await wrapper.get('select').setValue('two')
        await button().trigger('click')
        await flushPromises()
        expect(backend.resolveCustom).toHaveBeenLastCalledWith(initial.id, [candidate('two')])
        expect(unref(flow.controls.value?.canNext)).toBe(true)
        expect(isUpgradeRouteAvailable('selection', flow)).toBe(true)
    } finally {
        wrapper.unmount()
    }
})

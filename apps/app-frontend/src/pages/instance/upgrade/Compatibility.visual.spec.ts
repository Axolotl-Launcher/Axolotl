import { VueQueryPlugin } from '@tanstack/vue-query'
import { flushPromises, mount } from '@vue/test-utils'
import { IntlMessageFormat } from 'intl-messageformat'
import { expect, it, vi } from 'vitest'
import { defineComponent, h, ref, unref } from 'vue'

import type {
    InstanceUpgradeIssue,
    InstanceUpgradeItemStatus,
    InstanceUpgradePlan,
    InstanceUpgradeResolution,
} from '@/helpers/instance-upgrade'
import type { GameInstance } from '@/helpers/types'

import Compatibility from './Compatibility.vue'
import { provideInstanceUpgradeFlow } from './flow'

const backend = vi.hoisted(() => ({ update: vi.fn(), push: vi.fn() }))
vi.mock('@/helpers/instance-upgrade', () => ({
    update_instance_upgrade_resolution: backend.update,
    update_instance_upgrade_resolutions: vi.fn(),
    reset_instance_upgrade_resolution: vi.fn(),
}))
vi.mock('@/helpers/instance-content', () => ({
    loadInstanceContentData: async () => ({ snapshot: { items: [] } }),
    localContentIconUrl: () => '',
}))
vi.mock('@/helpers/upgrade-version-metadata', () => ({
    loadUpgradeVersionDisplayMetadata: async () => ({}),
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
    Avatar: { render: () => null },
    Button: defineComponent({
        setup:
            (_, { attrs, slots }) =>
            () =>
                h('button', attrs, slots.default?.()),
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
    status: InstanceUpgradeItemStatus = 'prerelease_only',
) {
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
                status,
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
    } satisfies InstanceUpgradePlan
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

it('offers keep/disable for an incompatible-dependency blocker and clears it on disable', async () => {
    backend.update.mockReset()
    backend.push.mockReset()
    const resolution: InstanceUpgradeResolution = {
        contentId: 'root',
        action: 'upgrade',
        allowPrerelease: false,
        confirmedPrereleaseDependencies: [],
    }
    const incompatibility: InstanceUpgradeIssue = {
        code: 'incompatible_dependency',
        message: 'root is incompatible with other',
        contentId: null,
        provider: 'modrinth',
        projectId: 'root',
        conflictingProjectId: 'other',
        dependencyRequirements: [],
    }
    const initial = plan(resolution, [incompatibility], 'incompatible_dependency')
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
        const canContinue = () => unref(flow.controls.value!.canNext)
        const disableButton = () =>
            wrapper.findAll('button').find((button) => button.text().includes('Disable current'))
        expect(canContinue()).toBe(false)
        expect(disableButton()).toBeTruthy()

        backend.update.mockImplementationOnce(
            async (_id: string, next: InstanceUpgradeResolution) =>
                plan(next, [], 'upgrade_available'),
        )
        await disableButton()!.trigger('click')
        await flushPromises()
        expect(backend.update).toHaveBeenCalledTimes(1)
        expect(backend.update.mock.calls[0][1].action).toBe('disable')
        expect(canContinue()).toBe(true)
    } finally {
        wrapper.unmount()
    }
})

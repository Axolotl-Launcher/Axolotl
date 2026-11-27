import { expect, it } from 'vitest'
import { ref } from 'vue'

import type { InstallJobSnapshot } from '@/helpers/install'
import type { InstanceUpgradePlan, InstanceUpgradeResult } from '@/helpers/instance-upgrade'

import {
    attachUpgradeJobToFlow,
    type InstanceUpgradeFlow,
    isUpgradeRouteAvailable,
    isUpgradeRouteRecoveryPending,
    upgradeDownloadsLocation,
    upgradeProgressDestination,
} from './flow.ts'

// The route helpers only read a subset of the plan, so these fixtures are partial.
function partialPlan(plan: object): InstanceUpgradePlan {
    return plan as unknown as InstanceUpgradePlan
}

function selectionFlow(plan: InstanceUpgradePlan | null): InstanceUpgradeFlow {
    return { plan: ref(plan) } as InstanceUpgradeFlow
}

it('selection route requires an unblocked plan with a selected solution', () => {
    const selectedSolution = { kind: 'newest', selections: [], dependencyChanges: [], warnings: [] }
    expect(isUpgradeRouteAvailable('selection', selectionFlow(null))).toBe(false)
    expect(
        isUpgradeRouteAvailable(
            'selection',
            selectionFlow(partialPlan({ blockingIssues: [], selectedSolution: null })),
        ),
    ).toBe(false)
    expect(
        isUpgradeRouteAvailable(
            'selection',
            selectionFlow(
                partialPlan({
                    blockingIssues: [{ code: 'dependency_conflict' }],
                    selectedSolution,
                }),
            ),
        ),
    ).toBe(false)
    expect(
        isUpgradeRouteAvailable(
            'selection',
            selectionFlow(partialPlan({ blockingIssues: [], selectedSolution })),
        ),
    ).toBe(true)
})

it('allows version choices after an inconclusive search without unlocking execution', () => {
    const issue = { code: 'search_limit_reached' }
    const item = {
        contentId: 'mod-a',
        provider: 'modrinth',
        projectId: 'a',
        candidateReleaseIds: ['a-1', 'a-2'],
        resolution: { action: 'upgrade' },
    }
    const limited = {
        blockingIssues: [issue],
        items: [item],
        customConstraints: [],
        selectedSolution: null,
    }
    const afterLimit = selectionFlow(partialPlan(limited))
    expect(isUpgradeRouteAvailable('customize', afterLimit)).toBe(true)
    expect(isUpgradeRouteAvailable('selection', afterLimit)).toBe(false)

    const conflict = { code: 'incompatible_dependency' }
    const confirmed = { ...limited, blockingIssues: [issue, conflict] }
    expect(isUpgradeRouteAvailable('customize', selectionFlow(partialPlan(confirmed)))).toBe(false)
    expect(
        isUpgradeRouteAvailable('customize', selectionFlow(partialPlan({ ...limited, items: [] }))),
    ).toBe(false)
    const chooseAgain = selectionFlow(
        partialPlan({ ...confirmed, customConstraints: [{ contentId: 'mod-a' }] }),
    )
    expect(isUpgradeRouteAvailable('customize', chooseAgain)).toBe(true)
    expect(isUpgradeRouteAvailable('selection', chooseAgain)).toBe(false)
})

it('upgrade execution and Progress recovery target focused Downloads', () => {
    expect(upgradeDownloadsLocation('job/a')).toEqual({
        path: '/downloads',
        query: { job: 'job/a' },
    })
    expect(upgradeProgressDestination('loading', null, 'instance/a')).toBe(null)
    expect(upgradeProgressDestination('ready', 'job/a', 'instance/a')).toEqual({
        path: '/downloads',
        query: { job: 'job/a' },
    })
    expect(upgradeProgressDestination('ready', null, 'instance/a')).toEqual({
        path: '/instance/instance%2Fa/upgrade',
    })
})

it('accepted upgrade job sets ownership, preserves backend result, and returns Downloads target', () => {
    let jobId: string | null = null
    let result: unknown = null
    const location = attachUpgradeJobToFlow(
        {
            setJob: (value) => (jobId = value),
            setResult: (value) => (result = value),
        },
        {
            job_id: 'job-a',
            status: 'succeeded',
            upgrade_result: { planId: 'plan-a' } as InstanceUpgradeResult,
        } as InstallJobSnapshot,
    )
    expect(jobId).toBe('job-a')
    expect(result).toEqual({ planId: 'plan-a' })
    expect(location).toEqual({ path: '/downloads', query: { job: 'job-a' } })
})

it('job route waits only while persisted job recovery is loading', () => {
    const loading = {
        jobRecoveryState: ref('loading'),
        activeJobId: ref(null),
    } as InstanceUpgradeFlow
    expect(isUpgradeRouteRecoveryPending('job', loading)).toBe(true)
    expect(isUpgradeRouteRecoveryPending('result', loading)).toBe(true)
    loading.jobRecoveryState.value = 'ready'
    expect(isUpgradeRouteRecoveryPending('job', loading)).toBe(false)
    expect(isUpgradeRouteAvailable('job', loading)).toBe(false)
    loading.activeJobId.value = 'job-a'
    expect(isUpgradeRouteAvailable('job', loading)).toBe(true)
})

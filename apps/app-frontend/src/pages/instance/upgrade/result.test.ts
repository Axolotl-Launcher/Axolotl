import { expect, it } from 'vitest'

import type { InstallJobSnapshot, InstallJobStatus } from '@/helpers/install'
import type { InstanceUpgradeResult, InstanceUpgradeSolution } from '@/helpers/instance-upgrade'

import {
    isSuccessfulUpgradeJob,
    summarizeUpgradeResult,
    upgradeResultLocation,
    upgradeResultMode,
} from './result.ts'

function result(source = 'source', target = 'target'): InstanceUpgradeResult {
    return {
        planId: 'plan',
        sourceInstanceId: source,
        targetInstanceId: target,
        backupInstanceId: null,
        solution: { kind: 'custom', selections: [], dependencyChanges: [], warnings: [] },
        compatibilityWarnings: [],
        externalChanges: [],
        skippedDueToExternalConflict: [],
    }
}

function job(
    status: InstallJobStatus,
    upgradeResult: InstanceUpgradeResult | null = result(),
    kind: InstallJobSnapshot['kind'] = 'upgrade_unmanaged_instance',
): InstallJobSnapshot {
    return {
        job_id: 'job/a',
        instance_id: upgradeResult?.targetInstanceId ?? 'source',
        kind,
        status,
        upgrade_result: upgradeResult,
    } as InstallJobSnapshot
}

it('successful upgrade result identifies copy and direct modes', () => {
    const copyJob = job('succeeded', result('source/a', 'target/b'))
    expect(isSuccessfulUpgradeJob(copyJob)).toBe(true)
    expect(upgradeResultMode(copyJob.upgrade_result!)).toBe('copy_and_upgrade')
    expect(upgradeResultMode(result('same', 'same'))).toBe('direct')
})

it('successful result links to persisted standalone source-instance page', () => {
    expect(upgradeResultLocation(job('succeeded', result('source/a', 'target/b')))).toEqual({
        path: '/instance/source%2Fa/upgrade/result',
        query: { job: 'job/a' },
    })
})

it('result summary follows executed selection actions and dependency kinds', () => {
    const solution = {
        selections: [
            ...Array.from({ length: 3 }, () => ({ action: 'upgrade' })),
            ...Array.from({ length: 2 }, () => ({ action: 'keep' })),
            { action: 'disable' },
        ],
        dependencyChanges: [
            ...Array.from({ length: 2 }, () => ({ kind: 'add' })),
            ...Array.from({ length: 3 }, () => ({ kind: 'upgrade' })),
            ...Array.from({ length: 4 }, () => ({ kind: 'remove' })),
            ...Array.from({ length: 5 }, () => ({ kind: 'keep' })),
        ],
    } as InstanceUpgradeSolution

    expect(summarizeUpgradeResult(solution)).toEqual({
        updated: 3,
        kept: 2,
        disabled: 1,
        dependencyAdded: 2,
        dependencyUpdated: 3,
        dependencyRemoved: 4,
    })
})

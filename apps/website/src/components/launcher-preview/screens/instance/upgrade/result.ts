import type { InstallJobSnapshot, InstanceUpgradeResult, InstanceUpgradeSolution } from './backend'

export type UpgradeResultMode = 'direct' | 'copy_and_upgrade'

export interface UpgradeResultSummary {
    updated: number
    kept: number
    disabled: number
    dependencyAdded: number
    dependencyUpdated: number
    dependencyRemoved: number
}

export function isSuccessfulUpgradeJob(job: InstallJobSnapshot): boolean {
    return (
        job.kind === 'upgrade_unmanaged_instance' &&
        job.status === 'succeeded' &&
        job.upgrade_result != null
    )
}

export function upgradeResultMode(result: InstanceUpgradeResult): UpgradeResultMode {
    return result.sourceInstanceId === result.targetInstanceId ? 'direct' : 'copy_and_upgrade'
}

export function summarizeUpgradeResult(solution: InstanceUpgradeSolution): UpgradeResultSummary {
    return {
        updated: solution.selections.filter((selection) => selection.action === 'upgrade').length,
        kept: solution.selections.filter((selection) => selection.action === 'keep').length,
        disabled: solution.selections.filter((selection) => selection.action === 'disable').length,
        dependencyAdded: solution.dependencyChanges.filter((change) => change.kind === 'add')
            .length,
        dependencyUpdated: solution.dependencyChanges.filter((change) => change.kind === 'upgrade')
            .length,
        dependencyRemoved: solution.dependencyChanges.filter((change) => change.kind === 'remove')
            .length,
    }
}

export function upgradeResultLocation(job: InstallJobSnapshot) {
    if (!isSuccessfulUpgradeJob(job))
        return { path: '/downloads', query: { job: job.job_id } } as const
    return {
        path: `/instance/${encodeURIComponent(job.upgrade_result!.sourceInstanceId)}/upgrade/result`,
        query: { job: job.job_id },
    } as const
}

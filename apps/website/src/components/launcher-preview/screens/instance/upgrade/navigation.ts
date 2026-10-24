import { upgradeDomain } from './backend'
import type { UpgradeFlowSnapshot } from './flow'

/** A parked flow survives visiting a project's details inside this preview window. */
export function parkUpgradeFlow(snapshot: UpgradeFlowSnapshot) {
    upgradeDomain().flows.set(snapshot.instanceId, snapshot)
}

export function upgradeProjectPath(provider: string | null, projectId: string | null) {
    return provider === 'modrinth' && projectId ? `/project/${encodeURIComponent(projectId)}` : null
}

import { currentPreview } from '../../../lib/state'
import { instanceContent } from '../state'

/*
 * The resource-history commands behind "Old files and rollback backups"
 * (`resource_scan_backups`, `resource_prepare_history_cleanup`,
 * `resource_execute_history_cleanup`), over the `.old` files the preview's
 * content updates keep for their rollbacks.
 */

/** `ResourceBackupFile` of the app's `helpers/resources.ts`. */
export type ResourceBackupFile = {
    id: string
    historyId: string | null
    relativePath: string
    originalFileName?: string | null
    integrity: { size?: number | null }
    rollbackAvailable: boolean
    recognized: boolean
    exists: boolean
}

export type ResourceHistoryCleanupPlan = {
    id: string
    revision: number
    instanceId: string
    files: ResourceBackupFile[]
}

const plans = new Map<string, ResourceHistoryCleanupPlan>()

function delay(ms: number) {
    return new Promise<void>((resolve) => currentPreview().clock.after(ms, resolve))
}

export async function resourceScanBackups(instanceId: string): Promise<ResourceBackupFile[]> {
    await delay(260)
    return instanceContent(currentPreview()).backupFiles(instanceId)
}

export async function resourcePrepareHistoryCleanup(
    instanceId: string,
    historyIds: string[],
): Promise<ResourceHistoryCleanupPlan> {
    await delay(140)
    const files = instanceContent(currentPreview())
        .backupFiles(instanceId)
        .filter((file) => historyIds.includes(file.id))
    const plan = {
        id: `cleanup-${Math.random().toString(36).slice(2, 10)}`,
        revision: 1,
        instanceId,
        files,
    }
    plans.set(plan.id, plan)
    return plan
}

export async function resourceExecuteHistoryCleanup(cleanupPlanId: string, revision: number) {
    await delay(320)
    const plan = plans.get(cleanupPlanId)
    plans.delete(cleanupPlanId)
    if (!plan || plan.revision !== revision) {
        throw new Error('The cleanup plan changed. Scan again before deleting backups.')
    }
    const domain = instanceContent(currentPreview())
    const removed: string[] = []
    const failures: [string, string][] = []
    for (const file of plan.files) {
        if (domain.discardBackup(plan.instanceId, file.id)) removed.push(file.id)
        else failures.push([file.id, 'The file no longer exists.'])
    }
    return { removed, failures }
}

/** `resourceOperationError`: the failure's own message. */
export function resourceOperationError(failure: unknown, _formatMessage?: unknown): string {
    return failure instanceof Error ? failure.message : String(failure)
}

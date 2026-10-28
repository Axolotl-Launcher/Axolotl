import { confirmedContentGroupKey, type ContentItem, type useVIntl } from '@modrinth/ui'

import { type InstallJobSnapshot, wait_for_install_job } from './install'
import { resourcePlanIssueFeedback } from './resource-operation-feedback'
import {
    resourcePrepareEntryOperation,
    resourceQueuePlan,
    type SourceReleaseRef,
} from './resources'

export { confirmedContentGroupKey } from '@modrinth/ui'

const instanceUpdates = new Map<string, Promise<boolean>>()
const pendingEntries = new Map<string, Map<string, Promise<boolean>>>()

export async function queueResourceEntryUpdates(
    instanceId: string,
    entryIds: string[],
    track: (job: InstallJobSnapshot) => void,
    formatMessage: ReturnType<typeof useVIntl>['formatMessage'],
): Promise<boolean> {
    const selected = [...new Set(entryIds)]
    if (!selected.length) return false
    const entries = pendingEntries.get(instanceId) ?? new Map<string, Promise<boolean>>()
    const existing = new Set(selected.flatMap((entry) => entries.get(entry) ?? []))
    const remaining = selected.filter((entry) => !entries.has(entry))
    if (remaining.length) {
        const previous = instanceUpdates.get(instanceId) ?? Promise.resolve(false)
        const task = previous
            .catch(() => false)
            .then(() => runResourceEntryUpdate(instanceId, remaining, track, formatMessage))
        instanceUpdates.set(instanceId, task)
        pendingEntries.set(instanceId, entries)
        for (const entry of remaining) entries.set(entry, task)
        const cleanup = () => {
            for (const entry of remaining) if (entries.get(entry) === task) entries.delete(entry)
            if (!entries.size) pendingEntries.delete(instanceId)
            if (instanceUpdates.get(instanceId) === task) instanceUpdates.delete(instanceId)
        }
        void task.then(cleanup, cleanup)
        existing.add(task)
    }
    return (await Promise.all(existing)).some(Boolean)
}

async function runResourceEntryUpdate(
    instanceId: string,
    entryIds: string[],
    track: (job: InstallJobSnapshot) => void,
    formatMessage: ReturnType<typeof useVIntl>['formatMessage'],
) {
    const prepare = () =>
        resourcePrepareEntryOperation(
            instanceId,
            'update',
            entryIds,
            null,
            false,
            undefined,
            undefined,
            false,
        )
    let queuedJob: InstallJobSnapshot
    for (let attempt = 0; ; attempt++) {
        try {
            const plan = await prepare()
            const blocking = plan.issues.filter((issue) => issue.blocking)
            if (blocking.length)
                throw new Error(
                    blocking
                        .map(
                            (issue) => resourcePlanIssueFeedback(issue, formatMessage, true).detail,
                        )
                        .join('\n'),
                )
            if (!plan.actions.length) return false
            queuedJob = await resourceQueuePlan(plan.id, plan.revision)
            break
        } catch (error) {
            const message = preparationErrorMessage(error)
            if (
                attempt >= 2 ||
                !/^(?:invalid resource request:\s*)?(?:Invalid input:\s*)?instance changed (?:while (?:preparing the plan|verifying installed resources)|after review)$/i.test(
                    message.trim(),
                )
            )
                throw error
        }
    }
    track(queuedJob)
    await wait_for_install_job(queuedJob.job_id)
    return true
}

function preparationErrorMessage(error: unknown): string {
    if (typeof error !== 'object' || error === null) return String(error)
    if ('message' in error) return String(error.message)
    if ('error' in error) return preparationErrorMessage(error.error)
    return String(error)
}

export interface ResourceEntryReview {
    instanceId: string
    instanceName: string
    entryIds: string[]
    intent: 'switchVersion' | 'update'
    release: SourceReleaseRef | null
    installDependencies: boolean
}

export async function reviewResourceEntryChanges(
    input: Omit<ResourceEntryReview, 'intent'>,
    show: (input: ResourceEntryReview) => Promise<InstallJobSnapshot | null>,
    track: (job: InstallJobSnapshot) => void,
): Promise<boolean> {
    const entryIds = [...new Set(input.entryIds)]
    if (!entryIds.length) return false
    const job = await show({
        ...input,
        entryIds,
        intent: input.release ? 'switchVersion' : 'update',
    })
    if (!job) return false
    track(job)
    return true
}

export function duplicateGroupUpdateEntries(items: ContentItem[], anchor: ContentItem): string[] {
    const group = confirmedContentGroupKey(anchor)
    if (!group) return []
    return [
        ...new Set(
            items
                .filter(
                    (item) =>
                        confirmedContentGroupKey(item) === group &&
                        item.instanceMaterializationState === 'present' &&
                        item.instanceCapabilities?.canUpdate !== false &&
                        ((item.resourceUpdate !== null && item.resourceUpdate !== undefined) ||
                            (item.update !== null && item.update !== undefined)),
                )
                .flatMap((item) => (item.instanceEntryId ? [item.instanceEntryId] : [])),
        ),
    ]
}

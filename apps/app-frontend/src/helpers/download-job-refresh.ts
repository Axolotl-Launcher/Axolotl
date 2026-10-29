export interface RefreshableDownloadJob {
    job_id: string
    status: string
    created: string
    modified?: string
}

export async function reconcileAcceptedDownloadJob<T extends { job_id: string }>(
    accepted: T,
    read: (id: string) => Promise<T | null>,
    publish: (job: T) => void,
    remove: (id: string) => void,
    onError: (error: unknown) => void,
    revision?: { atDispatch: number; current: () => number },
) {
    if (!revision || revision.current() === revision.atDispatch) publish(accepted)
    const atRead = revision?.current()
    try {
        const current = await read(accepted.job_id)
        if (revision && revision.current() !== atRead) return
        if (current) publish(current)
        else remove(accepted.job_id)
    } catch (error) {
        onError(error)
    }
}

/**
 * A queued snapshot can be returned by the command that created a job after a
 * realtime running snapshot has already arrived. The backend timestamps have
 * second precision, so the two snapshots can have the same `modified` value.
 * Never let that older lifecycle state move an already active job backwards.
 */
export function isRegressiveActiveJobSnapshot<T extends RefreshableDownloadJob>(
    current: T,
    next: T,
    activeStatuses: ReadonlySet<string>,
): boolean {
    return (
        (!activeStatuses.has(current.status) &&
            activeStatuses.has(next.status) &&
            current.modified !== undefined &&
            next.modified !== undefined &&
            current.modified >= next.modified) ||
        (current.modified !== undefined &&
            current.modified === next.modified &&
            activeStatuses.has(current.status) &&
            activeStatuses.has(next.status) &&
            current.status !== 'queued' &&
            next.status === 'queued')
    )
}

/**
 * Merge a request/response job listing without letting it roll back realtime
 * events that arrived after the request was dispatched.
 */
export function mergeRefreshedDownloadJobs<T extends RefreshableDownloadJob>(
    refreshed: T[],
    current: T[],
    revisionsAtDispatch: ReadonlyMap<string, number>,
    currentRevisions: ReadonlyMap<string, number>,
    syntheticIds: ReadonlySet<string>,
    activeStatuses: ReadonlySet<string>,
): T[] {
    const currentById = new Map(current.map((job) => [job.job_id, job]))
    const refreshedIds = new Set(refreshed.map((job) => job.job_id))
    const merged = refreshed.map((job) => {
        const local = currentById.get(job.job_id)
        const changedDuringRefresh =
            (currentRevisions.get(job.job_id) ?? 0) !== (revisionsAtDispatch.get(job.job_id) ?? 0)
        // An operation or event received after dispatch can belong to a newer
        // execution, including a retry in the same timestamp second.
        return local && changedDuringRefresh ? local : job
    })
    const changedActiveJobsMissingFromResponse = current.filter((job) => {
        if (syntheticIds.has(job.job_id) || refreshedIds.has(job.job_id)) return false
        return (
            activeStatuses.has(job.status) &&
            (currentRevisions.get(job.job_id) ?? 0) !== (revisionsAtDispatch.get(job.job_id) ?? 0)
        )
    })
    const activeSynthetics = current.filter(
        (job) =>
            syntheticIds.has(job.job_id) &&
            !refreshedIds.has(job.job_id) &&
            activeStatuses.has(job.status),
    )
    return [...merged, ...changedActiveJobsMissingFromResponse, ...activeSynthetics].sort((a, b) =>
        b.created.localeCompare(a.created),
    )
}

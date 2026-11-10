import type { ContentRefreshMode } from '@/helpers/content-refresh'
import type { InstanceContentSnapshot } from '@/helpers/instance'
import type { GameInstance } from '@/helpers/types'

type ContentScope = Pick<GameInstance, 'id' | 'install_stage'>

/** Cached warnings survive a page visit even when the new task has no failures. */
export function modsContentRetryMode(
    warnings: InstanceContentSnapshot['warnings'],
    failed: number,
): ContentRefreshMode | null {
    let retryEntries = failed > 0
    for (const warning of warnings) {
        if (
            warning.code === 'provider_metadata_unavailable' ||
            warning.code === 'pack_metadata_unavailable' ||
            warning.code === 'resource_history_unavailable'
        )
            return 'force_remote'
        if (warning.code === 'resource_update_unavailable') {
            if (!warning.entryId) return 'force_remote'
            retryEntries = true
        }
    }
    return retryEntries ? 'retry_failed' : null
}

export function modsContentScopeTransition(current: ContentScope, previous: ContentScope) {
    const changedInstance = current.id !== previous.id
    return {
        clear: changedInstance,
        suspend: current.install_stage !== 'installed',
        refresh:
            changedInstance ||
            (previous.install_stage !== 'installed' && current.install_stage === 'installed'),
    }
}

/**
 * Whether a refresh shows the loading state. An empty snapshot is only known
 * to mean "no content" once a refresh round has finished for the instance;
 * after that, later rounds update the list in place instead of flashing the
 * loading state over a confirmed empty list.
 */
export function modsContentLoading(state: {
    hasSnapshot: boolean
    hasItems: boolean
    settled: boolean
}) {
    return !state.hasSnapshot || (!state.hasItems && !state.settled)
}

/**
 * Tracks whether a refresh round watched by this view has finished. A task
 * that is already finished when the view first sees it belongs to an earlier
 * visit and says nothing about the current files, so only a round seen running
 * or started here counts. A round can finish before `start` returns, so
 * `started` also settles on a finish observed just before it.
 */
export function createContentRefreshSettlement() {
    let round: string | undefined
    let last: { id: string; running: boolean } | undefined
    let settled = false
    const check = () => {
        if (last && !last.running && last.id === round) settled = true
        return settled
    }
    return {
        get settled() {
            return settled
        },
        observe(task: { id: string; running: boolean } | null | undefined) {
            if (!task) return settled
            last = task
            if (task.running) round = task.id
            return check()
        },
        started(id: string) {
            round = id
            return check()
        },
        reset() {
            round = undefined
            last = undefined
            settled = false
        },
    }
}

export function modsContentPending(state: {
    busy: boolean
    hasSnapshot: boolean
    hasItems: boolean
    loading: boolean
}) {
    return state.busy ? !state.hasSnapshot || !state.hasItems : state.loading
}

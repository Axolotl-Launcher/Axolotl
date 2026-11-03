import type { GameInstance } from '@/helpers/types'

type ContentScope = Pick<GameInstance, 'id' | 'install_stage'>

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

export function modsContentPending(state: {
    busy: boolean
    hasSnapshot: boolean
    hasItems: boolean
    loading: boolean
}) {
    return state.busy ? !state.hasSnapshot || !state.hasItems : state.loading
}

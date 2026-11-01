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

export function modsContentPending(state: {
    busy: boolean
    hasSnapshot: boolean
    hasItems: boolean
    loading: boolean
}) {
    return state.busy ? !state.hasSnapshot || !state.hasItems : state.loading
}

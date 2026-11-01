import type { BannerDesign } from './types.ts'

/** How many steps back the editor can go. Older snapshots are dropped. */
export const MAX_HISTORY = 50

export interface DesignHistory {
    /** States that came before the current one, oldest first. */
    past: BannerDesign[]
    /** States that were undone, next one to restore first. */
    future: BannerDesign[]
}

export function emptyHistory(): DesignHistory {
    return { past: [], future: [] }
}

/** Records `snapshot` as the state to return to; a new edit drops the redo stack. */
export function recordHistory(history: DesignHistory, snapshot: BannerDesign): DesignHistory {
    return {
        past: [...history.past, snapshot].slice(-MAX_HISTORY),
        future: [],
    }
}

export interface HistoryStep {
    history: DesignHistory
    design: BannerDesign
}

/** Steps back one state, pushing `current` onto the redo stack. */
export function undoHistory(history: DesignHistory, current: BannerDesign): HistoryStep | null {
    const previous = history.past[history.past.length - 1]
    if (!previous) return null
    return {
        history: {
            past: history.past.slice(0, -1),
            future: [current, ...history.future].slice(0, MAX_HISTORY),
        },
        design: previous,
    }
}

/** Steps forward one state, pushing `current` back onto the undo stack. */
export function redoHistory(history: DesignHistory, current: BannerDesign): HistoryStep | null {
    const [next] = history.future
    if (!next) return null
    return {
        history: {
            past: [...history.past, current].slice(-MAX_HISTORY),
            future: history.future.slice(1),
        },
        design: next,
    }
}

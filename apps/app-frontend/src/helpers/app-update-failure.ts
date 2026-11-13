import type { PendingUpdateFailure } from './settings'

/** Queue feedback before clearing its durable marker; failed clears can retry without a duplicate toast. */
export function createUpdateFailureConsumer() {
    const shown = new Set<string>()
    let consuming = false
    return async (
        failure: PendingUpdateFailure | null | undefined,
        notify: (failure: PendingUpdateFailure) => void,
        clear: () => Promise<void>,
    ): Promise<boolean> => {
        if (!failure || consuming) return false
        consuming = true
        try {
            const bounded = {
                version: Array.from(failure.version).slice(0, 128).join(''),
                error: Array.from(failure.error).slice(0, 2048).join(''),
            }
            const key = JSON.stringify(bounded)
            if (!shown.has(key)) {
                notify(bounded)
                shown.add(key)
            }
            await clear()
            return true
        } finally {
            consuming = false
        }
    }
}

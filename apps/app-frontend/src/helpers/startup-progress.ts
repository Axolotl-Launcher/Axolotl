import type { UnlistenFn } from '@tauri-apps/api/event'
import { shallowRef } from 'vue'

import { loading_listener } from '@/helpers/events.js'
import { type LoadingBarType, progress_bars_list } from '@/helpers/state'

export interface StartupLoadingProgress {
    event: LoadingBarType
    loader_uuid: string | number
    fraction: number | null
    message?: string
}

function isStartupProgress(progress: StartupLoadingProgress) {
    return ['database_migration', 'directory_move', 'checking_for_updates'].includes(
        progress.event.type ?? '',
    )
}

/** Owns the startup listener before the splash component can mount. */
export function createStartupProgress() {
    const progress = shallowRef<StartupLoadingProgress | null>(null)
    let disposed = false
    let unlisten: UnlistenFn | undefined
    let revision = 0
    let initialized = false

    function update(payload: StartupLoadingProgress) {
        if (disposed || !isStartupProgress(payload)) return
        if (
            payload.fraction === null &&
            progress.value &&
            progress.value.loader_uuid !== payload.loader_uuid
        )
            return
        revision += 1
        progress.value =
            initialized && payload.event.type === 'database_migration'
                ? { ...payload, fraction: null }
                : payload
    }

    function dispose() {
        disposed = true
        unlisten?.()
        unlisten = undefined
    }

    async function initialize(initializeState: () => Promise<void>) {
        const stop = await loading_listener(update).catch((error: unknown) => {
            console.warn('Could not subscribe to startup progress', error)
            return undefined
        })
        if (disposed) {
            stop?.()
            return
        }
        unlisten = stop

        try {
            const initialization = initializeState()
            const snapshotRevision = revision
            // A recreated webview can join a migration already in progress.
            // On a fresh launch the event state may not exist yet; the listener
            // is ready before initialization and will receive its first event.
            void progress_bars_list()
                .then((bars) => {
                    if (disposed || revision !== snapshotRevision) return
                    const candidates = Object.values(bars)
                        .filter((bar) => bar.bar_type && bar.loading_bar_uuid != null)
                        .map((bar) => ({
                            event: bar.bar_type!,
                            loader_uuid: bar.loading_bar_uuid!,
                            fraction: bar.total ? (bar.current ?? 0) / bar.total : 0,
                            message: bar.message,
                        }))
                        .filter(isStartupProgress)
                    const latest =
                        candidates.find((bar) => bar.event.type === 'database_migration') ??
                        candidates[0]
                    if (latest) update(latest)
                })
                .catch(() => {})
            await initialization
            initialized = true
            if (
                progress.value?.event.type === 'database_migration' &&
                progress.value.fraction !== null
            ) {
                progress.value = { ...progress.value, fraction: null }
            }
        } catch (error) {
            dispose()
            throw error
        }
    }

    return { progress, initialize, dispose }
}

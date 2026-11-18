import { onScopeDispose } from 'vue'

type Cleanup = () => void | Promise<void>

/** Call during synchronous setup; subscriptions can be started after an await. */
export function useScopedEventListeners() {
    const cleanups: Cleanup[] = []
    let disposed = false

    function runCleanup(cleanup: Cleanup) {
        try {
            void Promise.resolve(cleanup()).catch(console.error)
        } catch (error) {
            console.error(error)
        }
    }

    onScopeDispose(() => {
        disposed = true
        for (const cleanup of cleanups.splice(0)) runCleanup(cleanup)
    })

    function addCleanup(cleanup: Cleanup) {
        if (disposed) runCleanup(cleanup)
        else cleanups.push(cleanup)
    }

    async function subscribe<Payload>(
        listen: (callback: (payload: Payload) => void) => Cleanup | Promise<Cleanup>,
        callback: (payload: Payload) => void,
    ) {
        if (disposed) return
        const unlisten = await listen((payload) => {
            if (!disposed) return callback(payload)
        })
        addCleanup(unlisten)
    }

    return { subscribe, addCleanup, isDisposed: () => disposed }
}
